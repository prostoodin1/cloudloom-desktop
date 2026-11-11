use crate::{engine::Engine, mounts::Mounts};
use serde::{Deserialize, Serialize};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use tauri::{
    AppHandle, Emitter, Manager, State,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CloseMode {
    #[default]
    KeepOpen,
    Tray,
    Disconnect,
}

pub(crate) struct Background {
    mode: Mutex<CloseMode>,
    closing: AtomicBool,
}

impl Background {
    pub(crate) fn is_closing(&self) -> bool {
        self.closing.load(Ordering::Acquire)
    }
}

pub(crate) fn initialize(app: &AppHandle) -> tauri::Result<()> {
    let path = app.state::<Engine>().directory.join("window-settings.json");
    let mode = std::fs::read(path)
        .ok()
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or_default();
    app.manage(Background {
        mode: Mutex::new(mode),
        closing: AtomicBool::new(false),
    });
    let open = MenuItem::with_id(app, "open", "Открыть Cloudloom", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Отключить диски и выйти", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let mut builder = TrayIconBuilder::with_id("cloudloom")
        .tooltip("Cloudloom")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show(app),
            "quit" => close(app, true),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                show(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

fn show(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub(crate) fn close(app: &AppHandle, from_tray: bool) {
    if app
        .state::<crate::winfsp::Installer>()
        .busy
        .load(Ordering::Acquire)
    {
        show(app);
        let _ = app.emit("winfsp-installing", ());
        return;
    }
    let settings = app.state::<Background>();
    let mode = settings.mode.lock().map(|value| *value).unwrap_or_default();
    if !from_tray && matches!(mode, CloseMode::Tray) {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
        }
        return;
    }
    if settings.closing.swap(true, Ordering::AcqRel) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let engine = app.state::<Engine>();
        let mounts = app.state::<Mounts>();
        let result = if engine.transfers_running() {
            Err("Дождитесь завершения передач. Чтобы закрыть окно сейчас, выберите сворачивание в трей.".into())
        } else if !from_tray
            && matches!(mode, CloseMode::KeepOpen)
            && mounts.active.load(Ordering::Acquire) > 0
        {
            Err(
                "Отключите диски Проводника или выберите действие при закрытии в общих настройках."
                    .into(),
            )
        } else {
            crate::mounts::stop_for_exit(&engine, &mounts).await
        };
        match result {
            Ok(()) => app.exit(0),
            Err(error) => {
                app.state::<Background>()
                    .closing
                    .store(false, Ordering::Release);
                show(&app);
                let _ = app.emit("cloudloom-close-blocked", error);
            }
        }
    });
}

#[tauri::command]
pub(crate) fn background_settings(settings: State<'_, Background>) -> Result<CloseMode, String> {
    settings
        .mode
        .lock()
        .map(|value| *value)
        .map_err(|_| "Настройки недоступны".into())
}

#[tauri::command]
pub(crate) fn save_background_settings(
    engine: State<'_, Engine>,
    settings: State<'_, Background>,
    mode: CloseMode,
) -> Result<(), String> {
    let mut current = settings.mode.lock().map_err(|_| "Настройки недоступны")?;
    let destination = engine.directory.join("window-settings.json");
    let temporary = destination.with_extension("json.tmp");
    let bytes = serde_json::to_vec(&mode).map_err(|e| e.to_string())?;
    std::fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(temporary, destination).map_err(|e| e.to_string())?;
    *current = mode;
    Ok(())
}
