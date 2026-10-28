//! Opt-in Explorer integration. Credentials stay in the native process/environment.
use crate::engine::{Engine, Profile, remote};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};
use tauri::State;
use tokio::process::Child;
#[cfg(windows)]
pub(crate) fn winfsp_installed() -> bool {
    platform::installed()
}
#[path = "mount_icons.rs"]
mod icons;
#[cfg(all(test, windows))]
#[path = "mounts_tests.rs"]
mod integration;

#[cfg(windows)]
#[path = "mounts_windows.rs"]
mod platform;
#[cfg(not(windows))]
mod platform {
    use super::*;
    pub struct Job;
    impl Job {
        pub fn attach(_: &Child) -> Result<Self, String> {
            Err("Диски Проводника доступны только в Windows".into())
        }
    }
    pub fn letters() -> Vec<String> {
        vec![]
    }
    pub fn installed() -> bool {
        false
    }
    pub fn appearance(_: &Appearance, _: bool) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Options {
    pub id: String,
    pub letter: String,
    pub name: String,
    pub read_only: bool,
    pub auto_mount: bool,
    pub service_icon: bool,
}
#[derive(Clone, Serialize, Deserialize)]
struct Appearance {
    letter: String,
    owner: String,
    icon: String,
    label: String,
}
#[derive(Clone, Serialize, Deserialize)]
struct Record {
    options: Options,
    #[serde(default)]
    appearance: Option<Appearance>,
}
struct Session {
    child: Child,
    _job: platform::Job,
    endpoint: String,
    secret: String,
    cache: PathBuf,
    warning: Option<String>,
}
impl Session {
    async fn rc(&self, method: &str, value: Value) -> Result<Value, String> {
        // Never send RC credentials through a proxy or follow a redirect.
        let response = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(45))
            .build()
            .map_err(|_| "Ошибка локального соединения")?
            .post(format!("{}/{method}", self.endpoint))
            .basic_auth("cloudloom", Some(&self.secret))
            .json(&value)
            .send()
            .await
            .map_err(|_| "Движок диска не отвечает; кэш сохранён")?;
        if !response.status().is_success() {
            return Err("Движок не выполнил операцию. Проверьте доступ, свободную букву и WinFsp; кэш сохранён".into());
        }
        let bytes = response.bytes().await.map_err(|_| "Ответ движка прерван")?;
        if bytes.len() > 1024 * 1024 {
            return Err("Слишком большой ответ движка".into());
        }
        serde_json::from_slice(&bytes).map_err(|_| "Неверный ответ движка".into())
    }
    async fn pending(&self, check_dirty: bool) -> Result<u64, String> {
        let value = self.rc("vfs/stats", json!({})).await?;
        let cache = &value["diskCache"];
        let mut pending = 0;
        for field in ["uploadsQueued", "uploadsInProgress", "erroredFiles"] {
            pending += cache[field]
                .as_u64()
                .ok_or("Не удалось проверить очередь диска; отключение заблокировано")?;
        }
        if cache["outOfSpace"] == true {
            return Err("Недостаточно места для кэша; завершите загрузки перед отключением".into());
        }
        Ok(pending
            + if check_dirty {
                dirty_files(&self.cache.join("vfsMeta"))?
            } else {
                0
            })
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Disk {
    options: Options,
    mounted: bool,
    pending: Option<u64>,
    error: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Overview {
    windows: bool,
    win_fsp: bool,
    letters: Vec<String>,
    disks: Vec<Disk>,
}
#[derive(Default)]
struct Inner {
    initialized: bool,
    records: Vec<Record>,
    sessions: HashMap<String, Session>,
    errors: HashMap<String, String>,
}
#[derive(Default)]
pub(crate) struct Mounts {
    pub lifecycle: tokio::sync::Mutex<()>,
    inner: tokio::sync::Mutex<Inner>,
    pub active: AtomicUsize,
}

fn validate(options: &Options) -> Result<(), String> {
    if options.id.is_empty()
        || options.id.len() > 80
        || !options
            .id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err("Неверный профиль".into());
    }
    let b = options.letter.as_bytes();
    if b.len() != 1 || !(b'D'..=b'Z').contains(&b[0]) {
        return Err("Выберите букву от D до Z".into());
    }
    let name = options.name.trim();
    if name.is_empty()
        || name != options.name
        || name.encode_utf16().count() > 32
        || name
            .chars()
            .any(|c| c.is_control() || "\\/:*?\"<>|".contains(c))
        || name.ends_with('.')
    {
        return Err(
            "Название: 1–32 символа, без управляющих символов, \\ / : * ? \" < > | и точки в конце"
                .into(),
        );
    }
    Ok(())
}
fn records_path(engine: &Engine) -> PathBuf {
    engine.directory.join("explorer-disks.json")
}
fn save(engine: &Engine, records: &[Record]) -> Result<(), String> {
    // Write a sibling first; a failed write must not truncate the last valid settings.
    let path = records_path(engine);
    let temp = path.with_extension("pending");
    std::fs::write(
        &temp,
        serde_json::to_vec_pretty(records).map_err(|_| "Ошибка настроек дисков")?,
    )
    .map_err(|_| "Не удалось сохранить настройки дисков")?;
    std::fs::rename(temp, path).map_err(|_| "Не удалось заменить настройки дисков".into())
}
fn dirty_files(root: &Path) -> Result<u64, String> {
    match std::fs::symlink_metadata(root) {
        Ok(m) if m.is_dir() && !m.file_type().is_symlink() => {}
        Ok(_) => return Err("Неверная папка метаданных кэша".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(_) => return Err("Метаданные кэша недоступны".into()),
    }
    let mut dirs = vec![root.to_path_buf()];
    let mut dirty = 0;
    let mut visited = 0;
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(dir).map_err(|_| "Кэш диска недоступен")? {
            let entry = entry.map_err(|_| "Кэш диска недоступен")?;
            visited += 1;
            if visited > 100_000 {
                return Err(
                    "Кэш слишком велик для безопасной проверки; отключение заблокировано".into(),
                );
            }
            let kind = entry.file_type().map_err(|_| "Кэш диска недоступен")?;
            if kind.is_symlink() {
                return Err("Обнаружена ссылка в кэше; отключение заблокировано".into());
            }
            if kind.is_dir() {
                dirs.push(entry.path());
            } else {
                use std::io::Read;
                let mut bytes = vec![];
                std::fs::File::open(entry.path())
                    .map_err(|_| "Метаданные кэша недоступны")?
                    .take(2 * 1024 * 1024 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|_| "Метаданные кэша недоступны")?;
                if bytes.len() > 2 * 1024 * 1024 {
                    return Err("Неверные метаданные кэша".into());
                }
                let value: Value = serde_json::from_slice(&bytes)
                    .map_err(|_| "Метаданные кэша меняются; повторите проверку")?;
                match value["Dirty"].as_bool() {
                    Some(true) => dirty += 1,
                    Some(false) => {}
                    None => return Err("Неизвестный формат кэша; отключение заблокировано".into()),
                }
            }
        }
    }
    Ok(dirty)
}
fn supported(profile: &Profile) -> Result<(), String> {
    if !cfg!(windows) {
        return Err("Подключение в Проводник доступно только в Windows".into());
    }
    if profile.transport == "sftpgo-api" {
        return Err("Delivery API пока не поддерживает диск Проводника. Можно отдельно подключить WebDAV, если администратор предоставил его адрес. Обычная работа Delivery в Cloudloom не меняется".into());
    }
    Ok(())
}
impl Mounts {
    async fn start(
        &self,
        engine: &Engine,
        inner: &mut Inner,
        options: &Options,
    ) -> Result<(), String> {
        validate(options)?;
        let profile = engine.profile(&options.id)?;
        supported(&profile)?;
        if !platform::installed() {
            return Err("Установите WinFsp с официального сайта и перезапустите Cloudloom".into());
        }
        if inner.sessions.contains_key(&options.id) {
            return Err("Профиль уже подключён как диск".into());
        }
        if !platform::letters().contains(&options.letter) {
            return Err("Буква уже занята. Выберите другую".into());
        }
        if inner.sessions.keys().any(|id| {
            inner
                .records
                .iter()
                .any(|r| r.options.id == *id && r.options.name.eq_ignore_ascii_case(&options.name))
        }) {
            return Err("У подключённого диска уже есть такое название".into());
        }
        let cache = engine.directory.join("explorer-cache").join(&options.id);
        std::fs::create_dir_all(&cache).map_err(|_| "Не удалось создать кэш диска")?;
        if options.read_only && dirty_files(&cache.join("vfsMeta"))? > 0 {
            return Err("В кэше есть неотправленные изменения. Сначала подключите этот профиль с записью для восстановления отправки".into());
        }
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .map_err(|_| "Не удалось открыть локальный порт")?
            .local_addr()
            .map_err(|_| "Ошибка локального порта")?
            .port();
        let secret = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let mut child = engine
            .command(&options.id)
            .args([
                "rcd",
                "--rc-addr",
                &format!("127.0.0.1:{port}"),
                "--cache-dir",
                &cache.to_string_lossy(),
                "--log-level",
                "ERROR",
            ])
            .env("RCLONE_RC_USER", "cloudloom")
            .env("RCLONE_RC_PASS", &secret)
            .env("RCLONE_RC_NO_AUTH", "false")
            .env("RCLONE_RC_WEB_GUI", "false")
            .env("RCLONE_RC_SERVE", "false")
            .env("RCLONE_RC_ALLOW_ORIGIN", "")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|_| "Не удалось запустить движок диска")?;
        let job = match platform::Job::attach(&child) {
            Ok(job) => job,
            Err(error) => {
                let _ = child.kill().await;
                return Err(error);
            }
        };
        let mut session = Session {
            child,
            _job: job,
            endpoint: format!("http://127.0.0.1:{port}"),
            secret,
            cache,
            warning: None,
        };
        let mut ready = false;
        for _ in 0..40 {
            if session
                .child
                .try_wait()
                .map_err(|_| "Не удалось проверить движок")?
                .is_some()
            {
                break;
            }
            if let Ok(value) = session.rc("core/pid", json!({})).await {
                if value["pid"].as_u64() == session.child.id().map(u64::from) {
                    ready = true;
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        if !ready {
            return Err("Локальный движок не запустился".into());
        }
        let fs = remote(&profile, "")?;
        self.active.fetch_add(1, Ordering::SeqCst);
        let result = session.rc("mount/mount", json!({"fs":fs,"mountPoint":format!("{}:", options.letter),"mountOpt":{"NetworkMode":true,"VolumeName":options.name},"vfsOpt":{"CacheMode":3,"ReadOnly":options.read_only,"CacheMaxSize":10_737_418_240u64,"CacheMinFreeSpace":1_073_741_824u64,"WriteBack":2_000_000_000u64,"DirCacheTime":60_000_000_000u64,"PollInterval":30_000_000_000u64}})).await;
        if let Err(error) = result {
            self.active.fetch_sub(1, Ordering::SeqCst);
            return Err(error);
        }
        // From here the manager owns the process even if appearance/settings writes fail.
        inner.sessions.insert(options.id.clone(), session);
        self.active.store(inner.sessions.len(), Ordering::SeqCst);
        let (icon_key, icon_bytes) = icons::icon(&profile.provider);
        let icon = engine
            .directory
            .join("explorer-icons")
            .join(format!("{icon_key}.ico"));
        let appearance = Appearance {
            letter: options.letter.clone(),
            owner: uuid::Uuid::new_v4().to_string(),
            icon: format!("{},0", icon.display()),
            label: options.name.clone(),
        };
        if let Some(record) = inner
            .records
            .iter_mut()
            .find(|r| r.options.id == options.id)
        {
            record.appearance = Some(appearance.clone());
        }
        let warning = (|| {
            save(engine, &inner.records)?;
            if options.service_icon {
                std::fs::create_dir_all(icon.parent().unwrap())
                    .map_err(|_| "Не удалось создать значок")?;
                std::fs::write(&icon, icon_bytes).map_err(|_| "Не удалось сохранить значок")?;
            }
            platform::appearance(&appearance, options.service_icon)
        })()
        .err();
        inner.sessions.get_mut(&options.id).unwrap().warning = warning;
        inner.errors.remove(&options.id);
        Ok(())
    }
    pub async fn has_profile(&self, id: &str) -> bool {
        self.inner.lock().await.sessions.contains_key(id)
    }
    pub async fn forget(&self, engine: &Engine, id: &str) -> Result<(), String> {
        let mut inner = self.inner.lock().await;
        if !inner.initialized {
            return Err("Сначала откройте настройки дисков Проводника".into());
        }
        if inner.sessions.contains_key(id) {
            return Err("Сначала отключите диск Проводника этого профиля".into());
        }
        if dirty_files(
            &engine
                .directory
                .join("explorer-cache")
                .join(id)
                .join("vfsMeta"),
        )? > 0
        {
            return Err("В кэше диска есть неотправленные изменения. Подключите диск с записью и дождитесь отправки перед удалением профиля".into());
        }
        if let Some(record) = inner.records.iter().find(|r| r.options.id == id) {
            if let Some(a) = &record.appearance {
                if !platform::letters().contains(&a.letter) {
                    return Err(
                        "Оформление диска ещё используется. Сначала освободите его букву".into(),
                    );
                }
                platform_cleanup(a)?;
            }
        }
        let records: Vec<_> = inner
            .records
            .iter()
            .filter(|r| r.options.id != id)
            .cloned()
            .collect();
        save(engine, &records)?;
        inner.records = records;
        Ok(())
    }
}
async fn restore(engine: &Engine, mounts: &Mounts, inner: &mut Inner) -> Result<(), String> {
    if !inner.initialized {
        let path = records_path(&engine);
        inner.records = match std::fs::read(path) {
            Ok(bytes) => {
                if bytes.len() > 2 * 1024 * 1024 {
                    return Err("Файл настроек дисков слишком велик; исходный файл сохранён".into());
                }
                serde_json::from_slice(&bytes)
                    .map_err(|_| "Настройки дисков повреждены; исходный файл сохранён")?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
            Err(_) => return Err("Настройки дисков недоступны".into()),
        };
        let mut ids = std::collections::HashSet::new();
        if inner.records.len() > 128 {
            return Err("Слишком много настроек дисков".into());
        }
        for r in &inner.records {
            validate(&r.options)?;
            if !ids.insert(r.options.id.clone()) {
                return Err("Повторяющийся профиль в настройках дисков".into());
            }
        }
        for record in &mut inner.records {
            if let Some(a) = &record.appearance {
                if platform::letters().contains(&a.letter) {
                    platform_cleanup(a)?;
                    record.appearance = None;
                }
            }
        }
        inner.initialized = true;
        save(&engine, &inner.records)?;
        let auto: Vec<_> = inner
            .records
            .iter()
            .filter(|r| r.options.auto_mount)
            .map(|r| r.options.clone())
            .collect();
        for options in auto {
            if let Err(error) = mounts.start(engine, inner, &options).await {
                inner.errors.insert(options.id.clone(), error);
            }
        }
    }
    Ok(())
}
#[tauri::command]
pub(crate) async fn explorer_disks(
    engine: State<'_, Engine>,
    mounts: State<'_, Mounts>,
) -> Result<Overview, String> {
    let _lifecycle = mounts.lifecycle.lock().await;
    let mut inner = mounts.inner.lock().await;
    restore(&engine, &mounts, &mut inner).await?;
    let mut disks = vec![];
    let options: Vec<_> = inner.records.iter().map(|r| r.options.clone()).collect();
    for options in options {
        let mut error = inner.errors.get(&options.id).cloned();
        let mut pending = None;
        let mut dead = false;
        if let Some(session) = inner.sessions.get_mut(&options.id) {
            dead = session
                .child
                .try_wait()
                .map_err(|_| "Ошибка проверки диска")?
                .is_some();
            if dead {
                error = Some("Диск остановлен. Кэш сохранён; подключите повторно для отправки оставшихся изменений".into());
            } else {
                error = session.warning.clone();
                match session.pending(false).await {
                    Ok(n) => pending = Some(n),
                    Err(e) => error = Some(e),
                }
            }
        }
        if dead {
            inner.sessions.remove(&options.id);
            inner
                .errors
                .insert(options.id.clone(), error.clone().unwrap());
            if let Some(record) = inner
                .records
                .iter_mut()
                .find(|r| r.options.id == options.id)
            {
                if let Some(a) = &record.appearance {
                    platform_cleanup(a)?;
                    record.appearance = None;
                }
            }
            save(&engine, &inner.records)?;
        }
        disks.push(Disk {
            mounted: inner.sessions.contains_key(&options.id),
            options,
            pending,
            error,
        });
    }
    mounts.active.store(inner.sessions.len(), Ordering::SeqCst);
    Ok(Overview {
        windows: cfg!(windows),
        win_fsp: platform::installed(),
        letters: platform::letters(),
        disks,
    })
}
#[cfg(windows)]
fn platform_cleanup(a: &Appearance) -> Result<(), String> {
    platform::cleanup(a)
}
#[cfg(not(windows))]
fn platform_cleanup(_: &Appearance) -> Result<(), String> {
    Ok(())
}
#[tauri::command]
pub(crate) async fn connect_explorer(
    engine: State<'_, Engine>,
    mounts: State<'_, Mounts>,
    options: Options,
) -> Result<(), String> {
    let _lifecycle = mounts.lifecycle.lock().await;
    validate(&options)?;
    supported(&engine.profile(&options.id)?)?;
    let mut inner = mounts.inner.lock().await;
    if !inner.initialized {
        return Err("Сначала откройте настройки дисков".into());
    }
    if inner.sessions.contains_key(&options.id) {
        return Err("Отключите диск перед изменением настроек".into());
    }
    if let Some(old) = inner.records.iter().find(|r| r.options.id == options.id) {
        if old.appearance.is_some() {
            return Err("Предыдущее оформление ещё используется. Освободите букву и перезапустите приложение".into());
        }
    }
    let mut records = inner.records.clone();
    records.retain(|r| r.options.id != options.id);
    records.push(Record {
        options: options.clone(),
        appearance: None,
    });
    save(&engine, &records)?;
    inner.records = records;
    mounts.start(&engine, &mut inner, &options).await
}
#[tauri::command]
pub(crate) async fn disconnect_explorer(
    engine: State<'_, Engine>,
    mounts: State<'_, Mounts>,
    id: String,
) -> Result<(), String> {
    let _lifecycle = mounts.lifecycle.lock().await;
    stop(&engine, &mounts, id).await
}
async fn stop(engine: &Engine, mounts: &Mounts, id: String) -> Result<(), String> {
    let mut inner = mounts.inner.lock().await;
    let session = inner.sessions.get(&id).ok_or("Диск не подключён")?;
    // Include dirty metadata: files still open for writing are not yet in the upload queue.
    for _ in 0..2 {
        if session.pending(true).await? > 0 {
            return Err("Есть неотправленные изменения или ошибки. Закройте файлы в других программах и дождитесь отправки; диск не отключён".into());
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    let record = inner
        .records
        .iter()
        .find(|r| r.options.id == id)
        .ok_or("Нет настроек диска")?
        .clone();
    session
        .rc(
            "mount/unmount",
            json!({"mountPoint":format!("{}:", record.options.letter)}),
        )
        .await?;
    // A late write is recoverable from the persistent cache. Never delete that cache.
    let late = dirty_files(&session.cache.join("vfsMeta"));
    let mut session = inner.sessions.remove(&id).unwrap();
    let _ = session.child.kill().await;
    mounts.active.store(inner.sessions.len(), Ordering::SeqCst);
    if let Some(a) = &record.appearance {
        platform_cleanup(a)?;
    }
    if let Some(r) = inner.records.iter_mut().find(|r| r.options.id == id) {
        r.appearance = None;
        r.options.auto_mount = false;
    }
    save(&engine, &inner.records)?;
    if late? > 0 {
        return Err("Диск отключён, но появились поздние изменения. Они сохранены в кэше: подключите тот же профиль с записью для их отправки".into());
    }
    Ok(())
}
#[tauri::command]
pub(crate) async fn open_explorer(mounts: State<'_, Mounts>, id: String) -> Result<(), String> {
    let inner = mounts.inner.lock().await;
    if !inner.sessions.contains_key(&id) {
        return Err("Диск не подключён".into());
    }
    let options = &inner
        .records
        .iter()
        .find(|r| r.options.id == id)
        .ok_or("Диск не найден")?
        .options;
    #[cfg(windows)]
    {
        let mut command = std::process::Command::new("explorer.exe");
        use std::os::windows::process::CommandExt;
        command
            .creation_flags(0x08000000)
            .arg(format!("{}:\\", options.letter))
            .spawn()
            .map_err(|_| "Не удалось открыть Проводник")?;
    }
    #[cfg(not(windows))]
    let _ = options;
    Ok(())
}
#[tauri::command]
pub(crate) fn open_winfsp_help() -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("rundll32.exe")
            .creation_flags(0x08000000)
            .args([
                "url.dll,FileProtocolHandler",
                "https://github.com/winfsp/winfsp",
            ])
            .spawn()
            .map_err(|_| "Откройте https://winfsp.dev/rel/ в браузере")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_only_plain_drive_letters_and_labels() {
        let mut o = Options {
            id: "abc-123".into(),
            letter: "Z".into(),
            name: "Рабочий Google Drive".into(),
            read_only: true,
            auto_mount: false,
            service_icon: true,
        };
        assert!(validate(&o).is_ok());
        for letter in ["C", "z", "Z:", "Z:/foo", "\\\\host\\share", "A"] {
            o.letter = letter.into();
            assert!(validate(&o).is_err());
        }
        o.letter = "Z".into();
        for name in [
            "",
            "Name/dir",
            "Name\n",
            "trailing.",
            " padded ",
            "\\\\server\\share",
        ] {
            o.name = name.into();
            assert!(validate(&o).is_err());
        }
    }
    #[test]
    fn dirty_cache_is_detected_including_open_writes() {
        let root =
            std::env::temp_dir().join(format!("cloudloom-dirty-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("note"), br#"{"Dirty":true}"#).unwrap();
        assert_eq!(dirty_files(&root).unwrap(), 1);
        std::fs::write(root.join("note"), br#"{"Dirty":false}"#).unwrap();
        assert_eq!(dirty_files(&root).unwrap(), 0);
        std::fs::write(root.join("note"), b"broken").unwrap();
        assert!(dirty_files(&root).is_err());
        std::fs::remove_file(root.join("note")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
