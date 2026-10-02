#![allow(non_snake_case)]

pub mod commands;
pub mod core;
pub mod domain;
pub mod infra;

use parking_lot::RwLock;
use std::sync::Arc;
use tauri::Manager;

use crate::commands::payload_commands::PayloadState;
use crate::commands::{adb_commands, payload_commands, scrcpy_commands};
use crate::core::adb::traits::{CommandExecutor, DeviceDetector};
use crate::core::device::service::DeviceService;
use crate::core::scrcpy::manager::ScrcpyManager;
use crate::core::task::history::TaskHistory;
use crate::core::task::queue::TaskQueue;
use crate::infra::config::AppConfig;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let log_dir = infra::logging::default_log_dir();
    let _log_guard = infra::logging::init_logging(&log_dir);

    std::panic::set_hook(Box::new(|info| {
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic payload".to_string()
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());
        tracing::error!("PANIC at {}: {}", location, payload);
    }));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let app_dir = app
                .path()
                .resource_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));

            let config = Arc::new(RwLock::new(AppConfig::default()));
            let device_service = Arc::new(DeviceService::new(config.clone(), app_dir.clone()));
            device_service.warm_adb_daemon();

            let task_history = Arc::new(TaskHistory::default());
            let task_queue =
                Arc::new(TaskQueue::new(device_service.clone(), task_history.clone()));
            let scrcpy_manager =
                Arc::new(ScrcpyManager::new(device_service.clone(), app_dir.clone()));
            let payload_state = Arc::new(PayloadState::default());

            app.manage(config);
            app.manage(device_service.clone());
            app.manage(Arc::clone(&device_service) as Arc<dyn DeviceDetector>);
            app.manage(Arc::clone(&device_service) as Arc<dyn CommandExecutor>);
            app.manage(task_history);
            app.manage(task_queue);
            app.manage(scrcpy_manager);
            app.manage(payload_state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            adb_commands::run_adb_command,
            adb_commands::run_adb_command_streaming,
            adb_commands::list_available_commands,
            scrcpy_commands::start_scrcpy,
            scrcpy_commands::stop_scrcpy,
            scrcpy_commands::get_scrcpy_status,
            scrcpy_commands::scrcpy_key_back,
            scrcpy_commands::scrcpy_key_home,
            scrcpy_commands::scrcpy_key_app_switch,
            scrcpy_commands::scrcpy_key_power,
            scrcpy_commands::scrcpy_volume_up,
            scrcpy_commands::scrcpy_volume_down,
            scrcpy_commands::scrcpy_volume_mute,
            scrcpy_commands::scrcpy_set_rotation,
            scrcpy_commands::scrcpy_screenshot,
            payload_commands::payload_init,
            payload_commands::payload_list_partitions,
            payload_commands::payload_extract_partitions,
            payload_commands::payload_cancel,
            payload_commands::payload_close,
            payload_commands::non_payload_extract_folder,
            payload_commands::non_payload_extract_zip,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
