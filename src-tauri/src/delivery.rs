//! SFTPGo user API only. No administrator endpoints, browser cookies, or remote code.
use futures_util::TryStreamExt;
use reqwest::{Client as Http, Method, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const MISSING: &str = "Файл, папка или бакет не найдены. Обновите список и проверьте путь.";
const UPLOAD_ATTEMPTS: u32 = 5;
fn valid_share_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
#[derive(Serialize, Deserialize)]
pub(crate) struct Credentials {
    pub site: String,
    pub user: String,
    pub password: String,
}
pub(crate) fn vault(id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new("studio.red.cloudloom.delivery", id)
        .map_err(|_| "Системное хранилище паролей недоступно".into())
}
pub(crate) fn site_url(input: &str) -> Result<tauri::Url, String> {
    let input = input.trim();
    let mut url = tauri::Url::parse(&if input.contains("://") {
        input.to_string()
    } else {
        format!("https://{input}")
    })
    .map_err(|_| "Укажите адрес сайта Delivery")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Нужен HTTPS-адрес сайта без логина и пароля в ссылке".into());
    }
    let path = url.path().trim_end_matches('/');
    let prefix = path
        .find("/web/")
        .or_else(|| path.strip_suffix("/web").map(str::len))
        .or_else(|| path.find("/api/v2/"))
        .or_else(|| path.strip_suffix("/api/v2").map(str::len));
    let path = format!("{}/", prefix.map(|i| &path[..i]).unwrap_or(path));
    url.set_path(&path);
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}
fn status_error(status: StatusCode) -> String {
    match status.as_u16() {
        401 => "Delivery не принял вход. Проверьте логин и пароль; при 2FA введите свежий код в дополнительных настройках или подключите профиль заново.".into(),
        403 => "Delivery запретил операцию этому аккаунту. Проверьте права пользователя и разрешение HTTP/API-входа.".into(),
        404 => MISSING.into(),
        409 => "В месте назначения уже есть одноимённый элемент".into(),
        413 => "Delivery отклонил файл из-за ограничения размера".into(),
        429 => "Слишком много запросов к Delivery. Подождите перед повторным входом.".into(),
        300..=399 => "Delivery перенаправляет запрос. Введите конечный HTTPS-адрес сайта; пароль на другой адрес не отправлен.".into(),
        _ => format!("Delivery не выполнил операцию (HTTP {}). Проверьте права и доступность сервера.", status.as_u16()),
    }
}
fn network_error(_: reqwest::Error) -> String {
    "Не удалось связаться с Delivery. Проверьте адрес, сеть и HTTPS-сертификат сервера.".into()
}
async fn checked(response: Response) -> Result<Response, String> {
    if response.status().is_success() {
        Ok(response)
    } else {
        Err(status_error(response.status()))
    }
}
async fn bounded_json(mut response: Response, limit: usize) -> Result<Value, String> {
    let mut bytes = vec![];
    while let Some(chunk) = response.chunk().await.map_err(network_error)? {
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err("Ответ Delivery слишком большой".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| "Delivery вернул неожиданный ответ. Проверьте адрес сайта".into())
}
pub(crate) struct Client {
    credentials: Credentials,
    base: tauri::Url,
    http: Http,
    token: tokio::sync::Mutex<Option<String>>,
}
impl Client {
    pub fn new(credentials: Credentials) -> Result<Self, String> {
        let base = site_url(&credentials.site)?;
        let http = Http::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(90))
            .user_agent("Cloudloom")
            .build()
            .map_err(|_| "Не удалось создать защищённое подключение".to_string())?;
        Ok(Self {
            credentials,
            base,
            http,
            token: tokio::sync::Mutex::new(None),
        })
    }
    fn endpoint(&self, endpoint: &str) -> tauri::Url {
        self.base
            .join(&format!("api/v2/user/{endpoint}"))
            .expect("fixed API path")
    }
    pub async fn login(&self, otp: Option<&str>) -> Result<(), String> {
        let mut token = self.token.lock().await;
        let mut request = self
            .http
            .get(self.endpoint("token"))
            .basic_auth(&self.credentials.user, Some(&self.credentials.password));
        if let Some(otp) = otp.filter(|v| !v.is_empty()) {
            request = request.header("X-SFTPGO-OTP", otp);
        }
        let response = checked(request.send().await.map_err(network_error)?).await?;
        let value = bounded_json(response, 64 * 1024).await?;
        let value = value["access_token"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 16384)
            .ok_or("Delivery не выдал токен пользовательского входа")?;
        *token = Some(value.into());
        Ok(())
    }
    async fn token(&self) -> Result<String, String> {
        let cached = self.token.lock().await.clone();
        if let Some(token) = cached {
            return Ok(token);
        }
        self.login(None).await?;
        self.token
            .lock()
            .await
            .clone()
            .ok_or("Подключите Delivery заново".into())
    }
    async fn request(
        &self,
        method: Method,
        endpoint: &str,
        query: &[(&str, &str)],
    ) -> Result<Response, String> {
        let token = self.token().await?;
        let build = |token| {
            self.http
                .request(method.clone(), self.endpoint(endpoint))
                .query(query)
                .bearer_auth(token)
        };
        let response = build(token).send().await.map_err(network_error)?;
        if response.status() == StatusCode::UNAUTHORIZED {
            self.login(None).await?;
            return checked(
                build(self.token().await?)
                    .send()
                    .await
                    .map_err(network_error)?,
            )
            .await;
        }
        checked(response).await
    }
    pub async fn list(&self, path: &str) -> Result<Vec<Value>, String> {
        let path = api_path(path)?;
        let response = self
            .request(Method::GET, "dirs", &[("path", &path)])
            .await?;
        let value = bounded_json(response, 8 * 1024 * 1024).await?;
        let rows = value
            .as_array()
            .filter(|r| r.len() <= 20000)
            .ok_or("Delivery вернул неверный список файлов")?;
        rows.iter()
            .filter(|v| {
                v["mode"].as_u64().is_some_and(|mode| {
                    mode & 134217728 == 0 && (mode & 2147483648 != 0 || mode & 2401763328 == 0)
                })
            })
            .map(entry)
            .collect()
    }
    pub(crate) async fn range(
        &self,
        path: &str,
        start: u64,
        end: u64,
        size: u64,
    ) -> Result<Response, String> {
        let path = api_path(path)?;
        let build = |token: String| {
            self.http
                .get(self.endpoint("files"))
                .query(&[("path", &path)])
                .bearer_auth(token)
                .header("Range", format!("bytes={start}-{end}"))
        };
        let mut response = build(self.token().await?)
            .send()
            .await
            .map_err(network_error)?;
        if response.status() == StatusCode::UNAUTHORIZED {
            self.login(None).await?;
            response = build(self.token().await?)
                .send()
                .await
                .map_err(network_error)?;
        }
        let response = checked(response).await?;
        let expected = format!("bytes {start}-{end}/{size}");
        if response.status() != StatusCode::PARTIAL_CONTENT
            || response
                .headers()
                .get("content-range")
                .and_then(|h| h.to_str().ok())
                != Some(expected.as_str())
            || response
                .content_length()
                .is_some_and(|length| length != end - start + 1)
        {
            return Err("Delivery не поддерживает безопасное чтение участков файла. Полное скачивание не начато".into());
        }
        Ok(response)
    }
    pub(crate) async fn create_share(
        &self,
        path: &str,
        password: &str,
        days: u64,
    ) -> Result<crate::sharing::Share, String> {
        let path = api_path(path)?;
        if path == "/" {
            return Err("Выберите отдельный файл или папку, а не всё хранилище".into());
        }
        self.stat(&path).await?;
        if !(1..=90).contains(&days) || password.len() > 256 {
            return Err("Срок ссылки: от 1 до 90 дней; пароль: до 256 байт".into());
        }
        let expires = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "Проверьте часы системы")?
            .as_millis() as u64
            + days * 86_400_000;
        let body = json!({"name": path.rsplit('/').next().unwrap_or("Файл"), "scope": 1, "paths": [path], "expires_at": expires, "password": password});
        let build = |token: String| {
            self.http
                .post(self.endpoint("shares"))
                .bearer_auth(token)
                .json(&body)
        };
        let mut response = build(self.token().await?)
            .send()
            .await
            .map_err(network_error)?;
        if response.status() == StatusCode::UNAUTHORIZED {
            self.login(None).await?;
            response = build(self.token().await?)
                .send()
                .await
                .map_err(network_error)?;
        }
        if response.status() == StatusCode::FORBIDDEN
            || response.status() == StatusCode::BAD_REQUEST
        {
            return Err("Delivery не разрешил создать ссылку. Проверьте право на обмен файлами; сервер может требовать пароль.".into());
        }
        let response = checked(response).await?;
        let id = response.headers().get("X-Object-ID").and_then(|v| v.to_str().ok()).filter(|id| valid_share_id(id)).ok_or("Delivery не вернул идентификатор ссылки; проверьте список ссылок на сайте сервиса перед повтором")?.to_string();
        let url = self
            .base
            .join(&format!("web/client/pubshares/{id}"))
            .map_err(|_| "Неверный адрес ссылки")?;
        Ok(crate::sharing::Share {
            url: url.to_string(),
            id: Some(id),
            expires_at: Some(expires),
            can_revoke: true,
        })
    }
    pub(crate) async fn revoke_share(&self, id: &str) -> Result<(), String> {
        if !valid_share_id(id) {
            return Err("Неверный идентификатор ссылки".into());
        }
        self.request(Method::DELETE, &format!("shares/{id}"), &[])
            .await?;
        Ok(())
    }
    pub async fn stat(&self, path: &str) -> Result<Value, String> {
        let path = api_path(path)?;
        if path == "/" {
            self.list("").await?;
            return Ok(json!({"Name":"", "Path":"", "Size":0, "IsDir":true}));
        }
        let (parent, name) = path.rsplit_once('/').ok_or("Некорректный путь")?;
        self.list(parent)
            .await?
            .into_iter()
            .find(|v| v["Name"] == name)
            .ok_or(MISSING.into())
    }
    async fn new_destination(&self, path: &str) -> Result<(), String> {
        match self.stat(path).await {
            Ok(_) => Err("В месте назначения уже есть одноимённый элемент".into()),
            Err(e) if e == MISSING => Ok(()),
            Err(e) => Err(e),
        }
    }
    pub async fn mkdir(&self, path: &str) -> Result<(), String> {
        let path = api_path(path)?;
        self.request(Method::POST, "dirs", &[("path", &path)])
            .await?;
        Ok(())
    }
    pub async fn rename(&self, source: &str, target: &str) -> Result<(), String> {
        let source = api_path(source)?;
        let target = api_path(target)?;
        if source == "/" || target == source || target.starts_with(&format!("{source}/")) {
            return Err("Нельзя переместить корень или папку внутрь себя".into());
        }
        self.new_destination(&target).await?;
        self.request(
            Method::POST,
            "file-actions/move",
            &[("path", &source), ("target", &target)],
        )
        .await?;
        Ok(())
    }
    pub async fn delete(&self, path: &str, directory: bool) -> Result<(), String> {
        let path = api_path(path)?;
        if path == "/" {
            return Err("Нельзя удалить корень хранилища".into());
        }
        self.request(
            Method::DELETE,
            if directory { "dirs" } else { "files" },
            &[("path", &path)],
        )
        .await?;
        Ok(())
    }
    pub async fn download(
        &self,
        path: &str,
        local: &Path,
        limit: u64,
        progress: Arc<AtomicU64>,
    ) -> Result<(), String> {
        // Publish only completed bytes. A dropped/cancelled future can leave its
        // unique partial file, but never a truncated file at the final name.
        let temporary =
            local.with_file_name(format!(".cloudloom-{}.partial", uuid::Uuid::new_v4()));
        let result = self
            .download_inner(path, &temporary, limit, progress, true)
            .await;
        if result.is_ok() {
            // Unlike rename on Unix, hard_link fails if the destination exists.
            let published = tokio::fs::hard_link(&temporary, local).await.map_err(|_| {
                "Не удалось сохранить файл без перезаписи. Проверьте место назначения".to_string()
            });
            let _ = tokio::fs::remove_file(&temporary).await;
            return published;
        }
        result
    }
    pub async fn preview_download(
        &self,
        path: &str,
        local: &Path,
        limit: u64,
    ) -> Result<(), String> {
        self.download_inner(path, local, limit, Arc::new(AtomicU64::new(0)), true)
            .await
    }
    async fn download_inner(
        &self,
        path: &str,
        local: &Path,
        limit: u64,
        progress: Arc<AtomicU64>,
        cleanup: bool,
    ) -> Result<(), String> {
        let path = api_path(path)?;
        let mut response = self
            .request(Method::GET, "files", &[("path", &path)])
            .await?;
        let expected = response.content_length();
        if response.content_length().is_some_and(|n| n > limit) {
            return Err("Файл превышает лимит загрузки".into());
        }
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = tokio::fs::File::from_std(
            options
                .open(local)
                .map_err(|_| "Не удалось создать локальный файл без перезаписи")?,
        );
        // We own this newly created file. Never remove an existing file on a failed create_new.
        let result = async {
            let mut bytes = 0u64;
            while let Some(chunk) = response.chunk().await.map_err(network_error)? {
                bytes = bytes
                    .checked_add(chunk.len() as u64)
                    .ok_or("Файл слишком большой")?;
                if bytes > limit {
                    return Err("Файл превышает лимит загрузки".into());
                }
                file.write_all(&chunk)
                    .await
                    .map_err(|_| "Не удалось записать локальный файл")?;
                progress.fetch_add(chunk.len() as u64, Ordering::Relaxed);
            }
            if expected.is_some_and(|expected| bytes != expected) {
                return Err("Delivery передал файл не полностью. Повторите передачу".into());
            }
            file.sync_all()
                .await
                .map_err(|_| "Не удалось сохранить локальный файл")?;
            Ok(())
        }
        .await;
        drop(file);
        if cleanup && result.is_err() {
            let _ = std::fs::remove_file(local);
        }
        result
    }
    pub async fn upload(
        &self,
        path: &str,
        local: &Path,
        overwrite: bool,
        progress: Arc<AtomicU64>,
    ) -> Result<(), String> {
        let path = api_path(path)?;
        if !overwrite {
            self.new_destination(&path).await?;
            let (parent, _) = path.rsplit_once('/').ok_or("Некорректный путь")?;
            let temporary = format!("{parent}/.cloudloom-{}.partial", uuid::Uuid::new_v4());
            self.new_destination(&temporary).await?;
            let result = async {
                self.upload_inner(&temporary, local, progress).await?;
                if !self.matches_local(&temporary, local).await? {
                    return Err("Проверка загруженного файла не прошла. Повторите передачу".into());
                }
                self.rename(&temporary, &path).await
            }
            .await;
            if result.is_err() {
                let _ = self.delete(&temporary, false).await;
            }
            return result;
        }
        self.upload_inner(&path, local, progress).await
    }

    // Compare bytes with bounded memory; equal size alone is not sufficient.
    async fn matches_local(&self, path: &str, local: &Path) -> Result<bool, String> {
        let metadata = tokio::fs::metadata(local)
            .await
            .map_err(|_| "Не удалось проверить локальный файл")?;
        let stat = self.stat(path).await?;
        if stat["IsDir"] != false || stat["Size"].as_u64() != Some(metadata.len()) {
            return Ok(false);
        }
        let path = api_path(path)?;
        let mut response = self
            .request(Method::GET, "files", &[("path", &path)])
            .await?;
        let mut file = tokio::fs::File::open(local)
            .await
            .map_err(|_| "Не удалось открыть локальный файл")?;
        let mut bytes = 0u64;
        while let Some(chunk) = response.chunk().await.map_err(network_error)? {
            bytes = bytes
                .checked_add(chunk.len() as u64)
                .ok_or("Файл слишком большой")?;
            if bytes > metadata.len() {
                return Ok(false);
            }
            // Reader chunks are bounded; never buffer the whole file.
            for part in chunk.chunks(64 * 1024) {
                let mut buffer = vec![0; part.len()];
                if file.read_exact(&mut buffer).await.is_err() || buffer != part {
                    return Ok(false);
                }
            }
        }
        let mut tail = [0u8; 1];
        Ok(bytes == metadata.len()
            && file
                .read(&mut tail)
                .await
                .map_err(|_| "Не удалось проверить файл")?
                == 0)
    }

    async fn upload_inner(
        &self,
        path: &str,
        local: &Path,
        progress: Arc<AtomicU64>,
    ) -> Result<(), String> {
        let metadata = tokio::fs::metadata(local)
            .await
            .map_err(|_| "Не удалось прочитать исходный файл")?;
        if !metadata.is_file() {
            return Err("Выберите обычный файл".into());
        }
        let length = metadata.len();
        let progress_base = progress.load(Ordering::Relaxed);
        let mut last_error = "Не удалось загрузить файл".to_string();
        for attempt in 1..=UPLOAD_ATTEMPTS {
            progress.store(progress_base, Ordering::Relaxed);
            let file = tokio::fs::File::open(local)
                .await
                .map_err(|_| "Не удалось открыть исходный файл")?;
            let stream = tokio_util::io::ReaderStream::new(file).inspect_ok({
                let progress = progress.clone();
                move |chunk| {
                    progress.fetch_add(chunk.len() as u64, Ordering::Relaxed);
                }
            });
            let response = self
                .http
                .post(self.endpoint("files/upload"))
                .query(&[("path", &path)])
                .bearer_auth(self.token().await?)
                .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
                .header(reqwest::header::CONTENT_LENGTH, length)
                .body(reqwest::Body::wrap_stream(stream))
                .send()
                .await;
            let retryable = match response {
                Ok(response) if response.status().is_success() => return Ok(()),
                Ok(response) if response.status() == StatusCode::UNAUTHORIZED => {
                    last_error = status_error(response.status());
                    let _ = self.login(None).await;
                    true
                }
                Ok(response) => {
                    let status = response.status();
                    last_error = status_error(status);
                    matches!(status.as_u16(), 408 | 425 | 429 | 500 | 502 | 503 | 504)
                }
                Err(error) => {
                    last_error = network_error(error);
                    true
                }
            };
            if !retryable || attempt == UPLOAD_ATTEMPTS {
                return Err(last_error);
            }
            tokio::time::sleep(Duration::from_secs(1u64 << (attempt - 1).min(3))).await;
        }
        Err(last_error)
    }
    pub async fn execute(&self, args: &[String]) -> Result<String, String> {
        let operation = args.first().map(String::as_str).unwrap_or("");
        let source = || {
            args.get(1)
                .and_then(|p| p.strip_prefix("storage:"))
                .ok_or("Некорректный путь Delivery".to_string())
        };
        let result = match operation {
            "lsjson" => {
                let path = source()?;
                if args.iter().any(|v| v == "--stat") {
                    self.stat(path).await?
                } else {
                    json!(self.list(path).await?)
                }
            }
            "about" => json!({}), // User API does not expose trustworthy account disk totals.
            "mkdir" => {
                self.mkdir(source()?).await?;
                json!(null)
            }
            "move" | "moveto" => {
                let target = args
                    .get(2)
                    .and_then(|p| p.strip_prefix("storage:"))
                    .ok_or("Некорректный путь назначения")?;
                self.rename(source()?, target).await?;
                json!(null)
            }
            "purge" | "deletefile" => {
                self.delete(source()?, operation == "purge").await?;
                json!(null)
            }
            "copyto" => {
                let local = args.get(1).ok_or("Нет исходного файла")?;
                let target = args
                    .get(2)
                    .and_then(|p| p.strip_prefix("storage:"))
                    .ok_or("Нет пути назначения")?;
                self.upload(target, Path::new(local), true, Arc::new(AtomicU64::new(0)))
                    .await?;
                json!(null)
            }
            _ => return Err("Эта операция Delivery не поддерживается".into()),
        };
        serde_json::to_string(&result).map_err(|_| "Неверный ответ Delivery".into())
    }
    pub async fn transfer(
        &self,
        remote: &str,
        local: &Path,
        upload: bool,
        directory: bool,
        progress: Arc<AtomicU64>,
        resume: bool,
    ) -> Result<(), String> {
        let remote = api_path(remote)?;
        let mut queue = vec![(remote, local.to_path_buf(), directory)];
        let mut count = 0;
        while let Some((remote, local, directory)) = queue.pop() {
            count += 1;
            if count > 100000 {
                return Err("Слишком много элементов в папке; передайте её частями".into());
            }
            if directory {
                if upload {
                    if resume {
                        match self.stat(&remote).await {
                            Ok(value) if value["IsDir"] == true => {}
                            Ok(_) => return Err("На месте папки уже находится файл".into()),
                            Err(error) if error == MISSING => self.mkdir(&remote).await?,
                            Err(error) => return Err(error),
                        }
                    } else {
                        self.new_destination(&remote).await?;
                        self.mkdir(&remote).await?;
                    }
                    let mut entries = tokio::fs::read_dir(&local)
                        .await
                        .map_err(|_| "Не удалось прочитать локальную папку")?;
                    while let Some(entry) = entries
                        .next_entry()
                        .await
                        .map_err(|_| "Не удалось прочитать локальную папку")?
                    {
                        let kind = entry
                            .file_type()
                            .await
                            .map_err(|_| "Не удалось прочитать тип файла")?;
                        if !kind.is_file() && !kind.is_dir() {
                            return Err("Символические ссылки не загружаются. Выберите обычные файлы и папки".into());
                        }
                        let name = entry
                            .file_name()
                            .into_string()
                            .map_err(|_| "Имя файла не в UTF-8")?;
                        crate::engine::safe_name(&name)?;
                        queue.push((
                            format!("{}/{name}", remote.trim_end_matches('/')),
                            entry.path(),
                            kind.is_dir(),
                        ));
                    }
                } else {
                    if !(resume && local.is_dir()) {
                        tokio::fs::create_dir(&local)
                            .await
                            .map_err(|_| "Не удалось создать папку без перезаписи")?;
                    }
                    for entry in self.list(&remote).await? {
                        let name = entry["Name"].as_str().ok_or("Некорректное имя файла")?;
                        crate::engine::safe_name(name)?;
                        safe_local_name(name)?;
                        queue.push((
                            format!("{}/{name}", remote.trim_end_matches('/')),
                            local.join(name),
                            entry["IsDir"] == true,
                        ));
                    }
                }
            } else if upload {
                let size = tokio::fs::metadata(&local)
                    .await
                    .map_err(|_| "Не удалось прочитать исходный файл")?
                    .len();
                if resume {
                    match self.stat(&remote).await {
                        Ok(value) if value["IsDir"] == false && self.matches_local(&remote, &local).await? => {
                            progress.fetch_add(size, Ordering::Relaxed);
                            continue;
                        }
                        Ok(value) if value["IsDir"] == true => {
                            return Err("На месте файла уже находится папка".into());
                        }
                        Ok(_) => return Err("Файл назначения отличается от исходного. Он сохранён; выберите другое имя или папку".into()),
                        Err(error) if error == MISSING => {
                            self.upload(&remote, &local, false, progress.clone())
                                .await?
                        }
                        Err(error) => return Err(error),
                    }
                } else {
                    self.upload(&remote, &local, false, progress.clone())
                        .await?;
                }
            } else {
                let size = self.stat(&remote).await?["Size"]
                    .as_u64()
                    .ok_or("Неизвестный размер файла")?;
                if resume && local.is_file() {
                    let local_size = std::fs::metadata(&local)
                        .map_err(|_| "Не удалось проверить локальный файл")?
                        .len();
                    if local_size == size && self.matches_local(&remote, &local).await? {
                        progress.fetch_add(size, Ordering::Relaxed);
                        continue;
                    }
                    return Err("Локальный файл отличается от исходного. Он сохранён; выберите другое место назначения".into());
                }
                self.download(&remote, &local, size, progress.clone())
                    .await?;
            }
        }
        Ok(())
    }
}
fn safe_local_name(name: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
        if name.ends_with(['.', ' '])
            || [
                "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
                "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8",
                "LPT9",
            ]
            .contains(&stem.as_str())
        {
            return Err(
                "Имя файла не поддерживается Windows; переименуйте его перед скачиванием".into(),
            );
        }
    }
    let _ = name;
    Ok(())
}
fn api_path(path: &str) -> Result<String, String> {
    let path = path.trim_start_matches('/');
    crate::engine::safe_path(path)?;
    if path.split('/').any(|part| part.is_empty()) && !path.is_empty() {
        return Err("Некорректный путь Delivery".into());
    }
    Ok(format!("/{path}"))
}
fn entry(value: &Value) -> Result<Value, String> {
    let name = value["name"]
        .as_str()
        .ok_or("Delivery вернул файл без имени")?;
    crate::engine::safe_name(name)?;
    let directory = value["mode"]
        .as_u64()
        .ok_or("Delivery вернул неизвестный тип файла")?
        & 2147483648
        != 0;
    Ok(
        json!({"Name":name,"Path":name,"Size":if directory {0} else {value["size"].as_u64().ok_or("Delivery не сообщил размер файла")?},"IsDir":directory,"ModTime":value["last_modified"].as_str().unwrap_or(""),"MimeType":""}),
    )
}

#[cfg(test)]
#[path = "delivery_integration_tests.rs"]
mod integration_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn addresses_accept_browser_links_but_never_insecure_or_embedded_secrets() {
        for url in [
            "files.example.com",
            "https://files.example.com/web/",
            "https://files.example.com/web/client/login?next=/x#form",
        ] {
            assert_eq!(
                site_url(url).unwrap().as_str(),
                "https://files.example.com/"
            );
        }
        assert_eq!(
            site_url("https://files.example.com/team/web/client/login")
                .unwrap()
                .as_str(),
            "https://files.example.com/team/"
        );
        for url in [
            "http://files.example.com",
            "https://user:pass@files.example.com",
            "file:///x",
        ] {
            assert!(site_url(url).is_err());
        }
        assert!(api_path("../outside").is_err());
        assert!(api_path("a\\b").is_err());
        assert!(entry(&json!({"name":"../escape","mode":0,"size":1})).is_err());
        assert!(entry(&json!({"name":"file","mode":0})).is_err());
        assert_eq!(
            entry(&json!({"name":"folder","mode":2147483648u64})).unwrap()["IsDir"],
            true
        );
        assert!(!status_error(StatusCode::UNAUTHORIZED).contains("access_token"));
    }
}
