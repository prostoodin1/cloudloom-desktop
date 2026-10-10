use crate::engine::{Engine, remote};
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;
use tauri::State;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub(crate) supported: bool,
    pub delivery: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Share {
    pub url: String,
    pub id: Option<String>,
    pub expires_at: Option<u64>,
    pub can_revoke: bool,
}
impl Engine {
    pub(crate) async fn share_capabilities(&self, id: &str) -> Result<Capabilities, String> {
        let profile = self.profile(id)?;
        if self.delivery_client(id)?.is_some() {
            return Ok(Capabilities {
                supported: true,
                delivery: true,
            });
        }
        let output = tokio::time::timeout(
            Duration::from_secs(20),
            self.execute(
                id,
                vec!["backend".into(), "features".into(), remote(&profile, "")?],
            ),
        )
        .await
        .map_err(|_| "Не удалось проверить поддержку ссылок")??;
        let value: Value =
            serde_json::from_str(&output).map_err(|_| "Не удалось проверить поддержку ссылок")?;
        Ok(Capabilities {
            supported: value["Features"]["PublicLink"].as_bool() == Some(true),
            delivery: false,
        })
    }
}
#[tauri::command]
pub async fn sharing_capabilities(
    engine: State<'_, Engine>,
    id: String,
) -> Result<Capabilities, String> {
    engine.share_capabilities(&id).await
}

#[tauri::command]
pub(crate) async fn create_share(
    engine: State<'_, Engine>,
    id: String,
    path: String,
    password: Option<String>,
    days: Option<u64>,
) -> Result<Share, String> {
    let profile = engine.profile(&id)?;
    if path.is_empty() {
        return Err("Выберите файл или папку".into());
    }
    let source = remote(&profile, &path)?;
    if let Some(client) = engine.delivery_client(&id)? {
        return client
            .create_share(
                source.strip_prefix("storage:").ok_or("Некорректный путь")?,
                password.as_deref().unwrap_or(""),
                days.unwrap_or(7),
            )
            .await;
    }
    if password.as_ref().is_some_and(|p| !p.is_empty()) || days.is_some() {
        return Err("Этот сервис не поддерживает пароль и срок ссылки через Cloudloom".into());
    }
    if !engine.share_capabilities(&id).await?.supported {
        return Err("Сервис не предоставляет ссылки через это подключение".into());
    }
    let output = engine
        .execute(
            &id,
            vec!["link".into(), source, "--retries".into(), "1".into()],
        )
        .await?;
    let url = output
        .lines()
        .rev()
        .find(|s| !s.trim().is_empty())
        .ok_or("Сервис не вернул ссылку")?
        .trim();
    let parsed = tauri::Url::parse(url).map_err(|_| "Сервис вернул некорректную ссылку")?;
    if url.len() > 8192
        || parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err("Сервис не вернул безопасную HTTPS-ссылку".into());
    }
    Ok(Share {
        url: parsed.to_string(),
        id: None,
        expires_at: None,
        can_revoke: false,
    })
}
#[tauri::command]
pub async fn revoke_share(
    engine: State<'_, Engine>,
    id: String,
    share_id: String,
) -> Result<(), String> {
    engine.profile(&id)?;
    engine
        .delivery_client(&id)?
        .ok_or("Удалите ссылку на сайте сервиса")?
        .revoke_share(&share_id)
        .await
}
