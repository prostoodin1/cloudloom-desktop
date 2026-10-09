use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::PathBuf,
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::{Notify, Semaphore},
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub root: String,
    pub ready: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub profile_id: String,
    pub name: String,
    pub direction: String,
    pub status: String,
    pub bytes: u64,
    pub total: u64,
    pub speed: f64,
    pub error: Option<String>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectRequest {
    pub provider: String,
    pub name: String,
    pub root: String,
    pub options: HashMap<String, String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub size: i64,
    pub is_dir: bool,
    pub modified: String,
    pub mime: String,
}
struct Pending {
    profile: Profile,
    state: String,
}
pub struct Engine {
    binary: PathBuf,
    pub(crate) directory: PathBuf,
    password: String,
    profiles: Mutex<Vec<Profile>>,
    pending: tokio::sync::Mutex<HashMap<String, Pending>>,
    tasks: Mutex<Vec<Task>>,
    cancel: Mutex<HashMap<String, Arc<Notify>>>,
    slots: Arc<Semaphore>,
    pub(crate) previews: tokio::sync::Mutex<HashMap<String, crate::preview::Session>>,
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn safe_name(value: &str) -> Result<(), String> {
    if value.trim().is_empty()
        || value == "."
        || value == ".."
        || value.chars().any(|c| "/\\:\0<>\"|?*".contains(c))
    {
        return Err("Имя содержит недопустимые символы".into());
    }
    Ok(())
}
fn safe_path(value: &str) -> Result<(), String> {
    if value.starts_with('/')
        || value.contains('\\')
        || value.contains(':')
        || value.contains('\0')
        || value.split('/').any(|s| s == ".." || s == ".")
    {
        return Err("Некорректный путь".into());
    }
    Ok(())
}
pub(crate) fn remote(profile: &Profile, path: &str) -> Result<String, String> {
    safe_path(path)?;
    Ok(format!(
        "storage:{}",
        [profile.root.trim_matches('/'), path.trim_matches('/')]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("/")
    ))
}
fn engine_resource_name() -> &'static str {
    if cfg!(windows) {
        "resources/rclone.exe"
    } else {
        "resources/rclone"
    }
}

pub fn initialize(app: &AppHandle) -> Result<Engine, String> {
    let directory = app.path().app_local_data_dir().map_err(err)?;
    std::fs::create_dir_all(&directory).map_err(err)?;
    let vault = keyring::Entry::new("studio.red.cloudloom", "config-encryption-v1").map_err(err)?;
    let password = match vault.get_password() {
        Ok(p) => p,
        Err(keyring::Error::NoEntry) => {
            let p = format!("{}{}", uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
            vault.set_password(&p).map_err(err)?;
            p
        }
        Err(e) => {
            return Err(format!(
                "Не удалось открыть системное хранилище учётных данных: {e}"
            ));
        }
    };
    let binary = if cfg!(debug_assertions) {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(engine_resource_name())
    } else {
        app.path()
            .resource_dir()
            .map_err(err)?
            .join(engine_resource_name())
    };
    if !binary.is_file() {
        return Err("Отсутствует файловый движок rclone. Переустановите Cloudloom.".into());
    }
    let profiles = std::fs::read(directory.join("profiles.json"))
        .ok()
        .map(|data| serde_json::from_slice::<Vec<Profile>>(&data))
        .transpose()
        .map_err(err)?
        .unwrap_or_default();
    let mut tasks: Vec<Task> = std::fs::read(directory.join("tasks.json"))
        .ok()
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or_default();
    for task in &mut tasks {
        if matches!(task.status.as_str(), "active" | "queued") {
            task.status = "interrupted".into();
            task.error = Some("Передача прервана закрытием приложения. Запустите её повторно; уже совпадающие файлы будут пропущены.".into());
        }
    }
    Ok(Engine {
        binary,
        directory,
        password,
        profiles: Mutex::new(profiles),
        pending: tokio::sync::Mutex::new(HashMap::new()),
        tasks: Mutex::new(tasks),
        cancel: Mutex::new(HashMap::new()),
        slots: Arc::new(Semaphore::new(3)),
        previews: tokio::sync::Mutex::new(HashMap::new()),
    })
}
impl Engine {
    fn config(&self, id: &str) -> PathBuf {
        self.directory.join(format!("{id}.conf"))
    }
    pub(crate) fn command(&self, id: &str) -> Command {
        let mut cmd = Command::new(&self.binary);
        cmd.args([
            "--config",
            &self.config(id).to_string_lossy(),
            "--ask-password=false",
        ])
        .env("RCLONE_CONFIG_PASS", &self.password)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);
        cmd
    }
    pub(crate) async fn execute(&self, id: &str, args: Vec<String>) -> Result<String, String> {
        let output = tokio::time::timeout(
            Duration::from_secs(300),
            self.command(id).args(args).output(),
        )
        .await
        .map_err(|_| {
            "Операция превысила время ожидания. Проверьте сеть или повторите вход.".to_string()
        })?
        .map_err(err)?;
        if !output.status.success() {
            return Err(classify_error(&String::from_utf8_lossy(&output.stderr)));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
    pub(crate) fn profile(&self, id: &str) -> Result<Profile, String> {
        self.profiles
            .lock()
            .map_err(err)?
            .iter()
            .find(|p| p.id == id && p.ready)
            .cloned()
            .ok_or_else(|| "Профиль не найден".into())
    }
    async fn require_new_destination(&self, id: &str, destination: &str) -> Result<(), String> {
        match self.execute(id, vec!["lsjson".into(), destination.into(), "--stat".into(), "--retries".into(), "1".into()]).await {
            Ok(_) => Err("В месте назначения уже есть файл или папка с таким именем. Выберите другое имя или папку.".into()),
            Err(error) if error.starts_with("Файл, папка или бакет не найдены") => Ok(()),
            Err(error) => Err(error),
        }
    }
    fn save_profiles(&self) -> Result<(), String> {
        let data = serde_json::to_vec_pretty(&*self.profiles.lock().map_err(err)?).map_err(err)?;
        let temporary = self.directory.join("profiles.json.tmp");
        std::fs::write(&temporary, data).map_err(err)?;
        std::fs::rename(temporary, self.directory.join("profiles.json")).map_err(err)
    }
    fn emit_task(&self, app: &AppHandle, task: Task) {
        if let Ok(mut tasks) = self.tasks.lock() {
            if let Some(current) = tasks.iter_mut().find(|t| t.id == task.id) {
                *current = task.clone();
            } else {
                tasks.insert(0, task.clone());
            }
            tasks.truncate(100);
            if !matches!(task.status.as_str(), "active") {
                if let Ok(data) = serde_json::to_vec(&*tasks) {
                    let _ = std::fs::write(self.directory.join("tasks.json"), data);
                }
            }
        }
        let _ = app.emit("transfer", task);
    }
}
pub(crate) fn classify_error(text: &str) -> String {
    let lower = text.to_lowercase();
    if lower.contains("invalid_grant")
        || lower.contains("unauthorized")
        || lower.contains("invalid_client")
        || lower.contains("token has been expired")
    {
        "Авторизация истекла или OAuth-приложение недоступно. Переподключите профиль; для Google Drive может потребоваться собственный OAuth Client ID.".into()
    } else if lower.contains("403")
        || lower.contains("permission denied")
        || lower.contains("accessdenied")
        || lower.contains("access denied")
    {
        "Сервис отказал в доступе. Проверьте разрешения аккаунта и доступ к диску/бакету.".into()
    } else if lower.contains("quota") || lower.contains("429") {
        "Превышена квота сервиса. Подождите и повторите операцию.".into()
    } else if lower.contains("not found")
        || lower.contains("doesn't exist")
        || lower.contains("404")
    {
        "Файл, папка или бакет не найдены. Обновите список и проверьте путь.".into()
    } else if lower.contains("immutable") || lower.contains("already exists") {
        "В месте назначения уже есть файл с таким именем. Измените имя или выберите другую папку."
            .into()
    } else if lower.contains("timeout")
        || lower.contains("connection")
        || lower.contains("no such host")
    {
        "Не удалось связаться с сервисом. Проверьте сеть и повторите операцию.".into()
    } else if lower.contains("directory not empty") {
        "Папку не удалось удалить: она содержит файлы или недоступна.".into()
    } else {
        "Сервис не выполнил операцию. Проверьте подключение, параметры профиля и права доступа."
            .into()
    }
}
#[tauri::command]
pub fn list_profiles(engine: State<'_, Engine>) -> Result<Vec<Profile>, String> {
    Ok(engine.profiles.lock().map_err(err)?.clone())
}
#[tauri::command]
pub fn list_tasks(engine: State<'_, Engine>) -> Result<Vec<Task>, String> {
    Ok(engine.tasks.lock().map_err(err)?.clone())
}
#[tauri::command]
pub fn local_is_directory(path: String) -> Result<bool, String> {
    std::fs::metadata(path).map(|m| m.is_dir()).map_err(err)
}

fn service_help_url(provider: &str) -> Result<String, String> {
    let page = match provider {
        "drive" => "drive/#making-your-own-client-id",
        "google cloud storage" => "googlecloudstorage/",
        "alias" => "local/",
        "onedrive" | "dropbox" | "box" | "pcloud" | "yandex" | "webdav" | "s3" | "mega"
        | "koofr" | "hidrive" | "b2" | "azureblob" | "storj" | "seafile" | "sftp" => {
            return Ok(format!("https://rclone.org/{provider}/"));
        }
        _ => return Err("Нет инструкции для этого сервиса".into()),
    };
    Ok(format!("https://rclone.org/{page}"))
}

#[tauri::command]
pub fn open_service_help(provider: String) -> Result<(), String> {
    // Only fixed documentation URLs, never an arbitrary URL or command from the webview.
    let url = service_help_url(&provider)?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("rundll32.exe")
            .args(["url.dll,FileProtocolHandler", &url])
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|_| {
                "Не удалось открыть браузер. Инструкции доступны на rclone.org".to_string()
            })?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("/usr/bin/open")
            .arg(&url)
            .spawn()
            .map_err(|_| {
                "Не удалось открыть браузер. Инструкции доступны на rclone.org".to_string()
            })?;
        Ok(())
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = url;
        Err("Открытие инструкции поддерживается в macOS и Windows".into())
    }
}

#[tauri::command]
pub async fn connect_profile(
    engine: State<'_, Engine>,
    request: ConnectRequest,
) -> Result<Value, String> {
    if request.name.trim().is_empty() {
        return Err("Укажите название профиля".into());
    }
    let allowed = [
        "drive",
        "google cloud storage",
        "onedrive",
        "dropbox",
        "box",
        "pcloud",
        "yandex",
        "webdav",
        "s3",
        "alias",
        "mega",
        "koofr",
        "hidrive",
        "b2",
        "azureblob",
        "storj",
        "seafile",
        "sftp",
    ];
    if !allowed.contains(&request.provider.as_str()) {
        return Err("Провайдер не поддерживается".into());
    }
    if request.provider == "sftp" {
        let hosts = request
            .options
            .get("known_hosts_file")
            .ok_or("Укажите проверенный файл known_hosts")?;
        if hosts.trim().eq_ignore_ascii_case("none") || !PathBuf::from(hosts).is_file() {
            return Err(
                "Выберите существующий known_hosts с проверенным ключом SSH-сервера".into(),
            );
        }
        if request.options.get("pass").is_none_or(|s| s.is_empty())
            && request.options.get("key_file").is_none_or(|s| s.is_empty())
        {
            return Err("Укажите пароль SFTP или путь к приватному SSH-ключу".into());
        }
    }
    if request.provider == "seafile"
        && request
            .options
            .get("url")
            .is_none_or(|s| !s.starts_with("https://"))
    {
        return Err("Для Seafile укажите HTTPS-адрес сервера".into());
    }
    if request.provider == "google cloud storage"
        && request.root.trim().is_empty()
        && request
            .options
            .get("project_number")
            .is_none_or(|s| s.trim().is_empty())
    {
        return Err(
            "Укажите бакет или номер Google Cloud проекта, чтобы открыть хранилище.".into(),
        );
    }
    if request.provider != "alias" {
        safe_path(request.root.trim_matches('/'))?;
    }
    let id = uuid::Uuid::new_v4().simple().to_string();
    // Encrypt an empty config before any token or credential can be written.
    std::fs::write(engine.config(&id), "").map_err(err)?;
    let mut child = engine
        .command(&id)
        .args(["config", "encryption", "set"])
        .stdin(Stdio::piped())
        .spawn()
        .map_err(err)?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or("Не удалось открыть защищённую конфигурацию")?;
    stdin
        .write_all(format!("{}\n{}\n", engine.password, engine.password).as_bytes())
        .await
        .map_err(err)?;
    drop(stdin);
    let out = child.wait_with_output().await.map_err(err)?;
    if !out.status.success() {
        return Err("Не удалось зашифровать настройки профиля".into());
    }
    engine
        .execute(
            &id,
            vec!["config".into(), "encryption".into(), "check".into()],
        )
        .await?;
    let profile = Profile {
        id: id.clone(),
        name: request.name.trim().to_string(),
        provider: request.provider.clone(),
        root: if request.provider == "alias" {
            String::new()
        } else {
            request.root.trim_matches('/').into()
        },
        ready: false,
    };
    let mut args = vec![
        "config".into(),
        "create".into(),
        "storage".into(),
        request.provider.clone(),
        "--non-interactive".into(),
        "--obscure".into(),
    ];
    let valid_keys = [
        "client_id",
        "client_secret",
        "scope",
        "project_number",
        "url",
        "vendor",
        "user",
        "pass",
        "provider",
        "access_key_id",
        "secret_access_key",
        "endpoint",
        "region",
        "remote",
        "password",
        "2fa",
        "account",
        "key",
        "access_grant",
        "library",
        "library_key",
        "host",
        "port",
        "key_file",
        "key_file_pass",
        "known_hosts_file",
    ];
    for (key, value) in request.options {
        if valid_keys.contains(&key.as_str()) && !value.is_empty() {
            args.push(key);
            args.push(value);
        }
    }
    if request.provider == "koofr" {
        args.extend(["provider".into(), "koofr".into()]);
    }
    if request.provider == "storj" {
        args.extend(["provider".into(), "existing".into()]);
    }
    if request.provider == "alias" {
        let local = PathBuf::from(&request.root)
            .canonicalize()
            .map_err(|_| "Выберите существующую локальную папку".to_string())?;
        if !local.is_dir() {
            return Err("Выберите папку".into());
        }
        args.extend(["remote".into(), local.to_string_lossy().into_owned()]);
    }
    if [
        "drive",
        "google cloud storage",
        "onedrive",
        "dropbox",
        "box",
        "pcloud",
        "yandex",
        "hidrive",
    ]
    .contains(&request.provider.as_str())
    {
        args.extend(["config_is_local".into(), "true".into()]);
    }
    if request.provider == "drive" {
        args.extend(["scope".into(), "drive".into()]);
    }
    let result = engine.execute(&id, args).await;
    match result {
        Ok(output) => finish_connect(&engine, profile, &output).await,
        Err(error) => {
            let _ = std::fs::remove_file(engine.config(&id));
            Err(error)
        }
    }
}
async fn finish_connect(
    engine: &Engine,
    mut profile: Profile,
    output: &str,
) -> Result<Value, String> {
    let response: Value = serde_json::from_str(output.trim()).unwrap_or(json!({}));
    let state = response["State"].as_str().unwrap_or("");
    if !state.is_empty() {
        let id = profile.id.clone();
        engine.pending.lock().await.insert(
            id.clone(),
            Pending {
                profile,
                state: state.into(),
            },
        );
        let option = &response["Option"];
        return Ok(
            json!({"id":id,"question":{"name":option["Name"],"help":option["Help"],"default":option["Default"],"password":option["IsPassword"],"exclusive":option["Exclusive"],"examples":option["Examples"]},"error":response["Error"]}),
        );
    }
    // Confirm actual access before presenting the account as a connected profile.
    engine
        .execute(
            &profile.id,
            vec![
                "lsjson".into(),
                remote(&profile, "")?,
                "--max-depth".into(),
                "1".into(),
                "--retries".into(),
                "1".into(),
            ],
        )
        .await?;
    profile.ready = true;
    engine.profiles.lock().map_err(err)?.push(profile.clone());
    engine.save_profiles()?;
    Ok(json!({"profile":profile}))
}
#[tauri::command]
pub async fn continue_connect(
    engine: State<'_, Engine>,
    id: String,
    answer: String,
) -> Result<Value, String> {
    let pending = engine
        .pending
        .lock()
        .await
        .remove(&id)
        .ok_or("Сессия входа истекла. Начните заново.")?;
    let args = vec![
        "config".into(),
        "update".into(),
        "storage".into(),
        "--non-interactive".into(),
        "--continue".into(),
        "--state".into(),
        pending.state,
        "--result".into(),
        answer,
    ];
    let output = engine.execute(&id, args).await?;
    finish_connect(&engine, pending.profile, &output).await
}
#[tauri::command]
pub async fn cancel_connect(engine: State<'_, Engine>, id: String) -> Result<(), String> {
    if engine.pending.lock().await.remove(&id).is_some() {
        let _ = std::fs::remove_file(engine.config(&id));
    }
    Ok(())
}
#[tauri::command]
pub async fn disconnect_profile(engine: State<'_, Engine>, id: String) -> Result<(), String> {
    engine.profile(&id)?;
    if engine
        .tasks
        .lock()
        .map_err(err)?
        .iter()
        .any(|t| t.profile_id == id && matches!(t.status.as_str(), "queued" | "active"))
    {
        return Err("Сначала завершите или отмените передачи этого профиля".into());
    }
    engine.profiles.lock().map_err(err)?.retain(|p| p.id != id);
    engine.save_profiles()?;
    std::fs::remove_file(engine.config(&id)).map_err(err)?;
    Ok(())
}
#[tauri::command]
pub async fn list_files(
    engine: State<'_, Engine>,
    id: String,
    path: String,
) -> Result<Vec<FileEntry>, String> {
    let profile = engine.profile(&id)?;
    let output = engine
        .execute(
            &id,
            vec![
                "lsjson".into(),
                remote(&profile, &path)?,
                "--max-depth".into(),
                "1".into(),
                "--no-mimetype".into(),
                "--retries".into(),
                "2".into(),
            ],
        )
        .await?;
    let rows: Vec<Value> = serde_json::from_str(&output).map_err(err)?;
    Ok(rows
        .into_iter()
        .map(|row| FileEntry {
            name: row["Name"].as_str().unwrap_or("").into(),
            path: row["Path"].as_str().unwrap_or("").into(),
            size: row["Size"].as_i64().unwrap_or(0),
            is_dir: row["IsDir"].as_bool().unwrap_or(false),
            modified: row["ModTime"].as_str().unwrap_or("").into(),
            mime: row["MimeType"].as_str().unwrap_or("").into(),
        })
        .collect())
}
#[tauri::command]
pub async fn create_folder(
    engine: State<'_, Engine>,
    id: String,
    path: String,
    name: String,
) -> Result<(), String> {
    safe_name(&name)?;
    let profile = engine.profile(&id)?;
    if profile.provider == "google cloud storage" {
        return Err(
            "В GCS папки являются префиксами объектов. Создайте папку при загрузке файлов.".into(),
        );
    }
    engine
        .execute(
            &id,
            vec![
                "mkdir".into(),
                remote(
                    &profile,
                    &format!(
                        "{}{name}",
                        if path.is_empty() {
                            String::new()
                        } else {
                            format!("{path}/")
                        }
                    ),
                )?,
            ],
        )
        .await?;
    Ok(())
}
#[tauri::command]
pub async fn rename_item(
    engine: State<'_, Engine>,
    id: String,
    path: String,
    name: String,
) -> Result<(), String> {
    safe_name(&name)?;
    let profile = engine.profile(&id)?;
    let parent = path
        .rsplit_once('/')
        .map(|(p, _)| format!("{p}/"))
        .unwrap_or_default();
    let destination = remote(&profile, &format!("{parent}{name}"))?;
    engine.require_new_destination(&id, &destination).await?;
    engine
        .execute(
            &id,
            vec![
                "moveto".into(),
                remote(&profile, &path)?,
                destination,
                "--immutable".into(),
            ],
        )
        .await?;
    Ok(())
}
#[tauri::command]
pub async fn delete_item(
    engine: State<'_, Engine>,
    id: String,
    path: String,
    directory: bool,
) -> Result<(), String> {
    if path.trim_matches('/').is_empty() {
        return Err("Нельзя удалить корень хранилища".into());
    }
    let profile = engine.profile(&id)?;
    engine
        .execute(
            &id,
            vec![
                if directory { "purge" } else { "deletefile" }.into(),
                remote(&profile, &path)?,
            ],
        )
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn start_transfer(
    app: AppHandle,
    engine: State<'_, Engine>,
    id: String,
    direction: String,
    path: String,
    local: String,
    directory: bool,
    name: String,
) -> Result<String, String> {
    let profile = engine.profile(&id)?;
    safe_path(&path)?;
    safe_name(&name)?;
    if !["upload", "download"].contains(&direction.as_str()) {
        return Err("Некорректная операция".into());
    }
    let local_path = PathBuf::from(&local);
    if !local_path.is_absolute() {
        return Err("Выберите абсолютный локальный путь".into());
    }
    let (source, target) = if direction == "upload" {
        let canonical = local_path
            .canonicalize()
            .map_err(|_| "Локальный файл не найден".to_string())?;
        if canonical.is_dir() != directory {
            return Err("Тип исходного файла изменился".into());
        }
        let target = remote(&profile, &path)?;
        engine.require_new_destination(&id, &target).await?;
        (canonical.to_string_lossy().into_owned(), target)
    } else {
        if local_path.exists() {
            return Err(
                "В месте назначения уже есть файл или папка. Выберите другой путь для скачивания."
                    .into(),
            );
        }
        (remote(&profile, &path)?, local)
    };
    let task = Task {
        id: uuid::Uuid::new_v4().to_string(),
        profile_id: id.clone(),
        name,
        direction,
        status: "queued".into(),
        bytes: 0,
        total: 0,
        speed: 0.0,
        error: None,
    };
    let task_id = task.id.clone();
    let signal = Arc::new(Notify::new());
    engine
        .cancel
        .lock()
        .map_err(err)?
        .insert(task_id.clone(), signal.clone());
    engine.emit_task(&app, task.clone());
    tauri::async_runtime::spawn(async move {
        let engine = app.state::<Engine>();
        let permit = tokio::select! { result = engine.slots.clone().acquire_owned() => result.ok(), _ = signal.notified() => None };
        let mut task = task;
        if permit.is_none() {
            task.status = "cancelled".into();
            engine.emit_task(&app, task.clone());
            if let Ok(mut map) = engine.cancel.lock() {
                map.remove(&task.id);
            }
            return;
        }
        task.status = "active".into();
        engine.emit_task(&app, task.clone());
        let args = vec![
            if directory { "copy" } else { "copyto" }.into(),
            source,
            target,
            "--immutable".into(),
            "--create-empty-src-dirs".into(),
            "--use-json-log".into(),
            "--stats".into(),
            "1s".into(),
            "--stats-log-level".into(),
            "NOTICE".into(),
            "--log-level".into(),
            "NOTICE".into(),
            "--retries".into(),
            "3".into(),
            "--low-level-retries".into(),
            "5".into(),
            "--transfers".into(),
            "2".into(),
        ];
        match engine.command(&id).args(args).stdout(Stdio::null()).spawn() {
            Err(_) => {
                task.status = "error".into();
                task.error = Some("Не удалось запустить передачу".into());
            }
            Ok(mut child) => {
                let stderr = child.stderr.take().unwrap();
                let mut lines = BufReader::new(stderr).lines();
                let mut failure = String::new();
                loop {
                    tokio::select! {
                        _ = signal.notified() => { let _ = child.kill().await; task.status = "cancelled".into(); break; }
                        line = lines.next_line() => match line {
                            Ok(Some(line)) => { if let Ok(log) = serde_json::from_str::<Value>(&line) {
                                if let Some(stats) = log.get("stats") { task.bytes = stats["bytes"].as_u64().unwrap_or(task.bytes); task.total = stats["totalBytes"].as_u64().unwrap_or(task.total); task.speed = stats["speed"].as_f64().unwrap_or(0.0); engine.emit_task(&app, task.clone()); }
                                if log["level"].as_str() == Some("error") { failure = log["msg"].as_str().unwrap_or("").into(); }
                            } },
                            _ => { match child.wait().await { Ok(status) if status.success() => { task.status = "complete".into(); task.bytes = task.total; }, _ => { task.status = "error".into(); task.error = Some(classify_error(&failure)); } } break; }
                        }
                    }
                }
            }
        }
        task.speed = 0.0;
        engine.emit_task(&app, task.clone());
        if let Ok(mut map) = engine.cancel.lock() {
            map.remove(&task.id);
        }
    });
    Ok(task_id)
}
#[tauri::command]
pub fn cancel_transfer(engine: State<'_, Engine>, id: String) -> Result<(), String> {
    if let Some(signal) = engine.cancel.lock().map_err(err)?.get(&id) {
        signal.notify_one();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_path_escape_and_root_deletion_inputs() {
        for value in [
            "../secrets",
            "/absolute",
            "folder/../secrets",
            "a\\b",
            "C:evil",
        ] {
            assert!(safe_path(value).is_err(), "{value}");
        }
        assert!(safe_path("media/съёмка 1").is_ok());
    }
    #[test]
    fn names_cannot_inject_remote_or_traverse() {
        for value in ["..", "a/b", "C:", "a\\b", ""] {
            assert!(safe_name(value).is_err());
        }
        assert!(safe_name("Финальный фильм.mov").is_ok());
    }
    #[test]
    fn provider_errors_never_return_tokens() {
        assert!(!classify_error("unauthorized access_token=TOPSECRET").contains("TOPSECRET"));
    }
    #[test]
    fn help_links_cannot_open_arbitrary_urls_or_commands() {
        assert_eq!(
            service_help_url("drive").unwrap(),
            "https://rclone.org/drive/#making-your-own-client-id"
        );
        for provider in [
            "sftp",
            "b2",
            "koofr",
            "hidrive",
            "mega",
            "azureblob",
            "storj",
            "seafile",
        ] {
            assert!(
                service_help_url(provider)
                    .unwrap()
                    .starts_with("https://rclone.org/")
            );
        }
        for input in [
            "https://example.com",
            "../other",
            "drive&calc.exe",
            "sftp/../../",
            "",
        ] {
            assert!(service_help_url(input).is_err());
        }
    }
    #[test]
    fn expanded_service_fields_exist_in_bundled_engine() {
        let output = std::process::Command::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(engine_resource_name()),
        )
        .args(["config", "providers"])
        .output()
        .unwrap();
        assert!(output.status.success());
        let catalogue: Value = serde_json::from_slice(&output.stdout).unwrap();
        for (provider, keys) in [
            ("mega", vec!["user", "pass", "2fa"]),
            ("koofr", vec!["provider", "user", "password"]),
            ("hidrive", vec!["client_id", "client_secret"]),
            ("b2", vec!["account", "key"]),
            ("azureblob", vec!["account", "key"]),
            ("storj", vec!["provider", "access_grant"]),
            (
                "seafile",
                vec!["url", "user", "pass", "2fa", "library", "library_key"],
            ),
            (
                "sftp",
                vec![
                    "host",
                    "port",
                    "user",
                    "pass",
                    "key_file",
                    "key_file_pass",
                    "known_hosts_file",
                ],
            ),
        ] {
            let adapter = catalogue
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["Name"] == provider)
                .expect(provider);
            for key in keys {
                assert!(
                    adapter["Options"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|o| o["Name"] == key),
                    "{provider}.{key}"
                );
            }
        }
    }
    #[tokio::test]
    async fn encrypted_local_provider_real_file_roundtrip() {
        let directory =
            std::env::temp_dir().join(format!("cloudloom-test-{}", uuid::Uuid::new_v4()));
        let source = directory.join("source");
        let storage = directory.join("storage");
        let downloaded = directory.join("downloaded");
        for path in [&source, &storage, &downloaded] {
            std::fs::create_dir_all(path).unwrap();
        }
        std::fs::create_dir_all(source.join("Папка/empty")).unwrap();
        let content = vec![137u8; 2 * 1024 * 1024 + 37];
        std::fs::write(source.join("Папка/Видео.bin"), &content).unwrap();
        let engine = Engine {
            binary: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(engine_resource_name()),
            directory: directory.clone(),
            password: uuid::Uuid::new_v4().to_string(),
            profiles: Mutex::new(vec![]),
            pending: tokio::sync::Mutex::new(HashMap::new()),
            tasks: Mutex::new(vec![]),
            cancel: Mutex::new(HashMap::new()),
            slots: Arc::new(Semaphore::new(3)),
            previews: tokio::sync::Mutex::new(HashMap::new()),
        };
        let id = "test";
        std::fs::write(engine.config(id), "").unwrap();
        let mut child = engine
            .command(id)
            .args(["config", "encryption", "set"])
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        stdin
            .write_all(format!("{}\n{}\n", engine.password, engine.password).as_bytes())
            .await
            .unwrap();
        drop(stdin);
        assert!(child.wait_with_output().await.unwrap().status.success());
        engine
            .execute(
                id,
                vec![
                    "config".into(),
                    "create".into(),
                    "storage".into(),
                    "alias".into(),
                    "remote".into(),
                    storage.to_string_lossy().into_owned(),
                    "--non-interactive".into(),
                ],
            )
            .await
            .unwrap();
        assert!(
            std::fs::read_to_string(engine.config(id))
                .unwrap()
                .lines()
                .any(|line| line == "RCLONE_ENCRYPT_V0:")
        );
        engine
            .execute(
                id,
                vec![
                    "copy".into(),
                    source.to_string_lossy().into_owned(),
                    "storage:".into(),
                    "--create-empty-src-dirs".into(),
                    "--immutable".into(),
                ],
            )
            .await
            .unwrap();
        let listing = engine
            .execute(id, vec!["lsjson".into(), "storage:".into()])
            .await
            .unwrap();
        assert!(listing.contains("Папка"));
        engine
            .execute(
                id,
                vec![
                    "copy".into(),
                    "storage:".into(),
                    downloaded.to_string_lossy().into_owned(),
                    "--create-empty-src-dirs".into(),
                    "--immutable".into(),
                ],
            )
            .await
            .unwrap();
        assert_eq!(
            std::fs::read(downloaded.join("Папка/Видео.bin")).unwrap(),
            content
        );
        assert!(downloaded.join("Папка/empty").is_dir());
        engine
            .execute(
                id,
                vec![
                    "moveto".into(),
                    "storage:Папка/Видео.bin".into(),
                    "storage:Папка/Переименовано.bin".into(),
                    "--immutable".into(),
                ],
            )
            .await
            .unwrap();
        assert!(storage.join("Папка/Переименовано.bin").exists());
        std::fs::write(source.join("conflict.bin"), b"different content").unwrap();
        assert!(
            engine
                .require_new_destination(id, "storage:Папка/Переименовано.bin")
                .await
                .is_err()
        );
        assert!(
            engine
                .require_new_destination(id, "storage:Новая папка")
                .await
                .is_ok()
        );
        assert_eq!(
            std::fs::read(storage.join("Папка/Переименовано.bin")).unwrap(),
            content
        );
        engine
            .execute(id, vec!["purge".into(), "storage:Папка".into()])
            .await
            .unwrap();
        assert!(!storage.join("Папка").exists());
        // Exercise the same bounded download/save pipeline used by every provider.
        engine.profiles.lock().unwrap().push(Profile {
            id: id.into(),
            name: "Test".into(),
            provider: "alias".into(),
            root: "".into(),
            ready: true,
        });
        let initial = "Привет\r\nредактор\r\n".as_bytes().to_vec();
        std::fs::write(storage.join("note.txt"), &initial).unwrap();
        let cache = directory.join("preview.txt");
        engine
            .preview_download(id, "storage:note.txt", &cache, 2 * 1024 * 1024)
            .await
            .unwrap();
        assert_eq!(std::fs::read(&cache).unwrap(), initial);
        let mut session = crate::preview::Session {
            profile_id: id.into(),
            remote: "storage:note.txt".into(),
            local: cache,
            original: Some(initial),
        };
        engine
            .save_text_session(&mut session, "Правки в облаке\r\n".into())
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(storage.join("note.txt")).unwrap(),
            "Правки в облаке\r\n"
        );
        // Same-size content changes must not be skipped by size/mtime heuristics.
        engine
            .save_text_session(&mut session, "Правки в Облаке\r\n".into())
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(storage.join("note.txt")).unwrap(),
            "Правки в Облаке\r\n"
        );
        std::fs::write(storage.join("note.txt"), "Чужие правки").unwrap();
        assert!(
            engine
                .save_text_session(&mut session, "Не затирать".into())
                .await
                .unwrap_err()
                .contains("изменился")
        );
        assert_eq!(
            std::fs::read_to_string(storage.join("note.txt")).unwrap(),
            "Чужие правки"
        );
        let bounded = directory.join("bounded.txt");
        assert!(
            engine
                .preview_download(id, "storage:note.txt", &bounded, 3)
                .await
                .is_err()
        );
        assert!(!bounded.exists());
        // This directory was created by this test, outside all user storage.
        std::fs::remove_dir_all(directory).unwrap();
    }
}
