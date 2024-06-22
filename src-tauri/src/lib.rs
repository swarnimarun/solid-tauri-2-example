use std::env;

use serde::{Deserialize, Serialize};

use specta::Type;

use thiserror::Error;

#[derive(Error, Debug, Serialize, Deserialize, Type)]
enum AppError {
    #[error("failed to unzip file {0}")]
    FailedUnzip(String),
    #[error("invalid file path {0}")]
    InvalidPath(String),
}

#[derive(Serialize, Deserialize, Type)]
struct FileInfo {
    path: String,
}

#[tauri::command]
#[specta::specta]
fn try_unzip(file: FileInfo) -> Result<Vec<FileInfo>, AppError> {
    Ok(vec![])
}

#[tauri::command]
#[specta::specta]
fn recently_used() -> Result<Vec<FileInfo>, AppError> {
    Ok(vec![])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // setup logging
    // tracing_subscriber::fmt()
    //     .with_env_filter({
    //         use tracing::level_filters::LevelFilter;
    //         use tracing_subscriber::EnvFilter;
    //         let env_filter = EnvFilter::builder();
    //         // show upto INFO logs in debug builds by default
    //         #[cfg(debug_assertions)]
    //         let env_filter = env_filter.with_default_directive(LevelFilter::INFO.into());
    //         // show only WARN and ERROR logs in release builds(builds without debug assertions enabled)
    //         #[cfg(not(debug_assertions))]
    //         let env_filter = env_filter.with_default_directive(LevelFilter::WARN.into());
    //         env_filter.from_env_lossy()
    //     })
    //     .init();

    // setup devtools for debug builds
    #[cfg(debug_assertions)]
    use tauri_plugin_devtools;
    #[cfg(debug_assertions)]
    let devtools = tauri_plugin_devtools::init();
    let mut builder = tauri::Builder::default();

    #[cfg(debug_assertions)]
    {
        builder = builder.plugin(devtools);
    }

    let invoke_handler = {
        let builder = tauri_specta::ts::builder()
            .commands(tauri_specta::collect_commands![try_unzip, recently_used]);

        #[cfg(all(debug_assertions, not(mobile)))]
        let builder = builder.path("../src/bindings.ts");

        builder
            .build()
            .expect("Failed to setup builder with tauri specta")
    };

    builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(invoke_handler)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
