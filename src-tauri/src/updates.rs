//! Update only during the startup gate, before profiles/files/transfers are mounted.
use serde::Serialize;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tauri::{AppHandle, State, ipc::Channel};
use tauri_plugin_updater::UpdaterExt;

#[derive(Default)]
pub(crate) struct Startup {
    started: AtomicBool,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Progress {
    phase: &'static str,
    downloaded: u64,
    total: Option<u64>,
}

#[tauri::command]
pub(crate) async fn startup_update(
    app: AppHandle,
    startup: State<'_, Startup>,
    progress: Channel<Progress>,
) -> Result<String, String> {
    // A second request must never run an installer while the main UI is in use.
    if startup.started.swap(true, Ordering::SeqCst) {
        return Err("Проверка при запуске уже выполнена".into());
    }
    if cfg!(debug_assertions) {
        return Ok("dev".into());
    }
    let updater = app
        .updater_builder()
        .timeout(Duration::from_secs(15 * 60))
        .configure_client(|client| {
            client
                .connect_timeout(Duration::from_secs(10))
                .read_timeout(Duration::from_secs(60))
        })
        .build()
        .map_err(|_| "Не удалось подготовить проверку обновления")?;
    let update = tokio::time::timeout(Duration::from_secs(12), updater.check())
        .await
        .map_err(|_| "Проверка обновлений превысила время ожидания. Можно продолжить работу.")?
        .map_err(|_| "Не удалось проверить обновления. Проверьте сеть или продолжите работу.")?;
    let Some(update) = update else {
        return Ok("current".into());
    };
    validate_download_url(update.download_url.as_str())?;
    let _ = progress.send(Progress {
        phase: "download",
        downloaded: 0,
        total: None,
    });
    let mut downloaded = 0u64;
    let bytes = update.download(|chunk, total| {
        downloaded = downloaded.saturating_add(chunk as u64);
        let _ = progress.send(Progress { phase: "download", downloaded, total });
    }, || { let _ = progress.send(Progress { phase: "verify", downloaded: 0, total: None }); }).await
        .map_err(|_| "Обновление не скачано или подпись не прошла проверку. Текущая версия не заменена; можно продолжить работу.")?;
    // download() verifies the signature, including the signed version binding.
    let _ = progress.send(Progress {
        phase: "install",
        downloaded: 0,
        total: None,
    });
    update.install(bytes).map_err(|_| "Не удалось установить обновление. Проверьте права на папку приложения. Можно продолжить работу.")?;
    // Windows installer restarts the app itself. macOS needs an explicit relaunch.
    app.restart();
}
fn validate_download_url(value: &str) -> Result<(), String> {
    let url = tauri::Url::parse(value).map_err(|_| "Неверный адрес обновления")?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || !url
            .path()
            .starts_with("/prostoodin1/cloudloom-desktop/releases/download/cloudloom-")
    {
        return Err("Отклонён неизвестный источник обновления".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn release_downloads_are_restricted_to_our_https_channel() {
        assert!(validate_download_url("https://github.com/prostoodin1/cloudloom-desktop/releases/download/cloudloom-kestrel/app.exe").is_ok());
        for url in [
            "http://github.com/prostoodin1/cloudloom-desktop/releases/download/cloudloom-kestrel/app.exe",
            "https://evil.example/app.exe",
            "https://github.com/other/repo/releases/download/cloudloom-kestrel/app.exe",
            "https://user:pass@github.com/prostoodin1/cloudloom-desktop/releases/download/cloudloom-kestrel/app.exe",
        ] {
            assert!(validate_download_url(url).is_err());
        }
    }
}
