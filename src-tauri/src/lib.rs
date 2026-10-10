mod delivery;
mod engine;
mod preview;
mod setup;
mod sharing;
mod streaming;
mod updates;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            use tauri::Manager;
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updates::Startup::default())
        .setup(|app| {
            use tauri::Manager;
            let state = engine::initialize(app.handle()).map_err(std::io::Error::other)?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            engine::list_profiles,
            engine::list_tasks,
            engine::connect_profile,
            engine::continue_connect,
            engine::cancel_connect,
            engine::disconnect_profile,
            engine::list_files,
            engine::create_folder,
            engine::rename_item,
            engine::move_items,
            engine::delete_item,
            engine::start_transfer,
            engine::cancel_transfer,
            engine::local_is_directory,
            engine::open_service_help,
            preview::storage_usage,
            preview::prepare_preview,
            preview::close_preview,
            preview::save_preview_text,
            sharing::sharing_capabilities,
            sharing::create_share,
            sharing::revoke_share,
            updates::startup_update,
        ])
        .run(tauri::generate_context!())
        .expect("Cloudloom could not start");
}
