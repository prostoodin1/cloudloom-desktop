mod background;
mod delivery;
mod delivery_dav;
mod engine;
mod mounts;
mod preview;
mod setup;
mod sharing;
mod streaming;
mod updates;
mod winfsp;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri::Manager;
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
        .manage(mounts::Mounts::default())
        .manage(winfsp::Installer::default())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                background::close(window.app_handle(), false);
            }
        })
        .setup(|app| {
            use tauri::Manager;
            let state = engine::initialize(app.handle()).map_err(std::io::Error::other)?;
            app.manage(state);
            background::initialize(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            background::background_settings,
            background::save_background_settings,
            engine::list_profiles,
            engine::list_tasks,
            engine::monthly_download_bytes,
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
            engine::retry_transfer,
            engine::cancel_transfer,
            engine::dismiss_transfer,
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
            mounts::explorer_disks,
            mounts::explorer_transfers,
            mounts::connect_explorer,
            mounts::disconnect_explorer,
            mounts::clear_explorer_cache,
            mounts::open_explorer,
            mounts::open_winfsp_help,
            winfsp::install_winfsp,
        ])
        .build(tauri::generate_context!())
        .expect("Cloudloom could not start")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested {
                code: None, api, ..
            } = event
            {
                api.prevent_exit();
                background::close(app, true);
            }
        });
}
