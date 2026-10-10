use crate::engine::{Engine, remote};
use serde::Serialize;
use serde_json::Value;
use std::{path::PathBuf, process::Stdio, time::Duration};
use tauri::{AppHandle, Manager, State};
use tokio::io::AsyncReadExt;

const TEXT_LIMIT: u64 = 2 * 1024 * 1024;
const MEDIA_LIMIT: u64 = 1024 * 1024 * 1024;

pub(crate) struct Session {
    pub(crate) profile_id: String,
    pub(crate) remote: String,
    pub(crate) local: PathBuf,
    pub(crate) original: Option<Vec<u8>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    used: Option<u64>,
    total: Option<u64>,
    free: Option<u64>,
    message: Option<String>,
}

fn parse_usage(value: &Value) -> Usage {
    Usage {
        used: value["used"].as_u64(),
        total: value["total"].as_u64(),
        free: value["free"].as_u64(),
        message: None,
    }
}

#[tauri::command]
pub async fn storage_usage(engine: State<'_, Engine>, id: String) -> Result<Usage, String> {
    let profile = engine.profile(&id)?;
    // `about` asks the provider for account/disk quota. Never recursively count user files.
    let result = tokio::time::timeout(
        Duration::from_secs(20),
        engine.execute(
            &id,
            vec![
                "about".into(),
                remote(&profile, "")?,
                "--json".into(),
                "--retries".into(),
                "1".into(),
            ],
        ),
    )
    .await;
    match result {
        Ok(Ok(output)) => Ok(parse_usage(
            &serde_json::from_str::<Value>(&output).map_err(|_| "Неверный ответ о квоте")?,
        )),
        _ => Ok(Usage {
            used: None,
            total: None,
            free: None,
            message: Some("Объём не предоставлен сервисом или временно недоступен".into()),
        }),
    }
}

fn kind_for(name: &str) -> Option<&'static str> {
    let extension = name.rsplit('.').next()?.to_ascii_lowercase();
    match extension.as_str() {
        "mp4" | "webm" | "mov" | "m4v" | "mkv" | "avi" => Some("video"),
        "mp3" | "wav" | "ogg" | "flac" | "m4a" | "aac" => Some("audio"),
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "avif" | "heic" => Some("image"),
        "txt" | "md" | "csv" | "tsv" | "json" | "xml" | "yaml" | "yml" | "toml" | "ini" | "log"
        | "html" | "css" | "js" | "ts" | "tsx" | "jsx" | "py" | "rs" | "sql" | "sh" | "svg" => {
            Some("text")
        }
        _ => None,
    }
}

fn decode_text(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() as u64 > TEXT_LIMIT || bytes.contains(&0) {
        return Err("Редактор поддерживает текст UTF-8 до 2 MiB, без двоичных данных".into());
    }
    String::from_utf8(bytes.to_vec())
        .map_err(|_| "Файл не в UTF-8. Скачайте его для открытия во внешнем редакторе".into())
}

impl Engine {
    async fn preview_stat(&self, id: &str, source: &str) -> Result<Value, String> {
        let output = self
            .execute(
                id,
                vec![
                    "lsjson".into(),
                    source.into(),
                    "--stat".into(),
                    "--retries".into(),
                    "1".into(),
                ],
            )
            .await?;
        let stat: Value = serde_json::from_str(&output).map_err(|_| "Неверные метаданные файла")?;
        if stat["IsDir"].as_bool() != Some(false) {
            return Err("Выберите файл, а не папку".into());
        }
        Ok(stat)
    }

    // Bound the actual bytes read, even if a file grows or reports an incorrect size.
    pub(crate) async fn preview_download(
        &self,
        id: &str,
        source: &str,
        local: &PathBuf,
        limit: u64,
    ) -> Result<(), String> {
        if let Some(client) = self.delivery_client(id)? {
            let source = source
                .strip_prefix("storage:")
                .ok_or("Некорректный путь Delivery")?;
            return client.preview_download(source, local, limit).await;
        }
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let output_file = options.open(local).map_err(|e| e.to_string())?;
        let mut file = tokio::fs::File::from_std(output_file);
        let mut child = self
            .command(id)
            .args([
                "cat",
                source,
                "--head",
                &(limit + 1).to_string(),
                "--retries",
                "1",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let work = async {
            let mut stdout = child
                .stdout
                .take()
                .ok_or("Нет потока предпросмотра")?
                .take(limit + 1);
            let count = tokio::io::copy(&mut stdout, &mut file)
                .await
                .map_err(|e| e.to_string())?;
            if count > limit {
                return Err("Файл превышает лимит предпросмотра".into());
            }
            let status = child.wait().await.map_err(|e| e.to_string())?;
            if !status.success() {
                return Err("Не удалось загрузить файл. Проверьте сеть и права доступа".into());
            }
            file.sync_all().await.map_err(|e| e.to_string())?;
            Ok(())
        };
        let result = tokio::time::timeout(Duration::from_secs(300), work)
            .await
            .unwrap_or_else(|_| {
                Err(
                    "Загрузка предпросмотра превысила 5 минут. Скачайте файл обычным способом"
                        .into(),
                )
            });
        if result.is_err() {
            let _ = child.kill().await;
        }
        drop(child);
        drop(file);
        if result.is_err() {
            let _ = std::fs::remove_file(local);
        }
        result
    }

    pub(crate) async fn save_text_session(
        &self,
        session: &mut Session,
        text: String,
    ) -> Result<(), String> {
        let bytes = text.into_bytes();
        decode_text(&bytes)?;
        let original = session
            .original
            .as_ref()
            .ok_or("Этот файл нельзя редактировать")?;
        self.profile(&session.profile_id)?;
        self.preview_stat(&session.profile_id, &session.remote)
            .await?;
        let check = session
            .local
            .with_extension(format!("check-{}", uuid::Uuid::new_v4()));
        self.preview_download(&session.profile_id, &session.remote, &check, TEXT_LIMIT)
            .await?;
        let current = std::fs::read(&check).map_err(|e| e.to_string());
        let _ = std::fs::remove_file(&check);
        if current? != *original {
            return Err("Файл изменился в хранилище после открытия. Сохранение отменено; скопируйте свои правки и откройте файл заново".into());
        }
        let upload = session
            .local
            .with_extension(format!("save-{}", uuid::Uuid::new_v4()));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let written = (|| -> std::io::Result<()> {
            use std::io::Write;
            let mut file = options.open(&upload)?;
            file.write_all(&bytes)?;
            file.sync_all()
        })();
        if let Err(error) = written {
            let _ = std::fs::remove_file(&upload);
            return Err(error.to_string());
        }
        let result = self
            .execute(
                &session.profile_id,
                vec![
                    "copyto".into(),
                    upload.to_string_lossy().into_owned(),
                    session.remote.clone(),
                    "--ignore-times".into(),
                    "--retries".into(),
                    "1".into(),
                ],
            )
            .await;
        let _ = std::fs::remove_file(&upload);
        result?;
        // Verify the remote contents before displaying "Saved".
        self.preview_download(&session.profile_id, &session.remote, &check, TEXT_LIMIT)
            .await?;
        let saved = std::fs::read(&check).map_err(|e| e.to_string());
        let _ = std::fs::remove_file(&check);
        if saved? != bytes {
            return Err("Не удалось подтвердить сохранение. Правки оставлены в редакторе; проверьте файл в хранилище".into());
        }
        std::fs::write(&session.local, &bytes).map_err(|e| e.to_string())?;
        session.original = Some(bytes);
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    token: String,
    kind: String,
    local_path: Option<String>,
    text: Option<String>,
}

#[tauri::command]
pub async fn prepare_preview(
    app: AppHandle,
    engine: State<'_, Engine>,
    id: String,
    path: String,
) -> Result<Preview, String> {
    let profile = engine.profile(&id)?;
    let source = remote(&profile, &path)?;
    let name = path.rsplit('/').next().unwrap_or("");
    let kind =
        kind_for(name).ok_or("Для этого формата нет встроенного просмотра. Скачайте файл")?;
    let mut sessions = engine.previews.lock().await;
    if sessions.len() >= 3 {
        return Err("Закройте предыдущие окна просмотра".into());
    }
    let stat = engine.preview_stat(&id, &source).await?;
    let limit = if kind == "text" {
        TEXT_LIMIT
    } else {
        MEDIA_LIMIT
    };
    if stat["Size"].as_u64().is_none_or(|size| size > limit) {
        return Err(if kind == "text" {
            "Текст больше 2 MiB. Скачайте файл"
        } else {
            "Файл больше 1 GiB или его размер неизвестен. Скачайте файл"
        }
        .into());
    }
    let folder = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("previews");
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    if std::fs::symlink_metadata(&folder)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("Папка кэша не должна быть символической ссылкой".into());
    }
    // Only generated, regular cache files are eligible for age-based cleanup. No recursion.
    let mut cached = 0u64;
    for item in std::fs::read_dir(&folder)
        .map_err(|e| e.to_string())?
        .flatten()
    {
        let name = item.file_name().to_string_lossy().into_owned();
        if !name.is_ascii() || name.len() < 37 || uuid::Uuid::parse_str(&name[..36]).is_err() {
            continue;
        }
        let metadata = item.path().symlink_metadata().map_err(|e| e.to_string())?;
        if !metadata.file_type().is_file() {
            continue;
        }
        let stale = metadata
            .modified()
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > Duration::from_secs(86400));
        let in_use = sessions
            .values()
            .any(|session| session.local == item.path());
        if stale && !in_use && std::fs::remove_file(item.path()).is_ok() {
            continue;
        }
        cached = cached.saturating_add(metadata.len());
    }
    if cached.saturating_add(stat["Size"].as_u64().unwrap_or(limit)) > 3 * MEDIA_LIMIT {
        return Err(
            "Кэш просмотра заполнен (3 GiB). Закройте другие окна просмотра и повторите позже"
                .into(),
        );
    }
    let token = uuid::Uuid::new_v4().to_string();
    let extension = name
        .rsplit('.')
        .next()
        .unwrap_or("bin")
        .to_ascii_lowercase();
    let local = folder.join(format!("{token}.{extension}"));
    engine.preview_download(&id, &source, &local, limit).await?;
    let loaded = (|| -> Result<(Option<Vec<u8>>, Option<String>), String> {
        if kind == "text" {
            let bytes = std::fs::read(&local).map_err(|e| e.to_string())?;
            let text = decode_text(&bytes)?;
            Ok((Some(bytes), Some(text)))
        } else {
            app.asset_protocol_scope()
                .allow_file(&local)
                .map_err(|e| e.to_string())?;
            Ok((None, None))
        }
    })();
    let (original, text) = match loaded {
        Ok(value) => value,
        Err(error) => {
            let _ = std::fs::remove_file(&local);
            return Err(error);
        }
    };
    let local_path = if kind == "text" {
        None
    } else {
        Some(local.to_string_lossy().into_owned())
    };
    sessions.insert(
        token.clone(),
        Session {
            profile_id: id,
            remote: source,
            local,
            original,
        },
    );
    Ok(Preview {
        token,
        kind: kind.into(),
        local_path,
        text,
    })
}

#[tauri::command]
pub async fn close_preview(
    app: AppHandle,
    engine: State<'_, Engine>,
    token: String,
) -> Result<(), String> {
    if let Some(session) = engine.previews.lock().await.remove(&token) {
        let _ = app.asset_protocol_scope().forbid_file(&session.local);
        std::fs::remove_file(&session.local).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn save_preview_text(
    engine: State<'_, Engine>,
    token: String,
    text: String,
) -> Result<(), String> {
    let mut sessions = engine.previews.lock().await;
    let session = sessions
        .get_mut(&token)
        .ok_or("Окно редактора уже закрыто")?;
    engine.save_text_session(session, text).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quota_preserves_unknown_and_zero() {
        let unknown = parse_usage(&serde_json::json!({"used": 0}));
        assert_eq!(unknown.used, Some(0));
        assert_eq!(unknown.total, None);
        assert_eq!(parse_usage(&serde_json::json!({"total": -1})).total, None);
    }
    #[test]
    fn text_is_never_executed_and_binary_is_rejected() {
        assert_eq!(kind_for("page.HTML"), Some("text"));
        assert_eq!(kind_for("image.svg"), Some("text"));
        assert_eq!(kind_for("app.exe"), None);
        assert!(decode_text(&[0xff]).is_err());
        assert!(decode_text(b"a\0b").is_err());
        assert!(decode_text(&vec![b'a'; TEXT_LIMIT as usize + 1]).is_err());
        assert_eq!(decode_text("Привет".as_bytes()).unwrap(), "Привет");
    }
}
