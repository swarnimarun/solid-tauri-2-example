mod config;
mod error;
mod prefixtree;
mod unzip;

use crate::config::AppConfig;
use crate::error::AppError;

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{
    async_runtime::{channel, Mutex, RwLock},
    State,
};

#[derive(Serialize, Deserialize, Type)]
struct FileInfo {
    path: String,
}

#[tauri::command(async)]
#[specta::specta]
async fn recently_used(config: State<'_, RwLock<AppConfig>>) -> Result<Vec<FileInfo>, AppError> {
    let read = config.read().await;
    tracing::error!("config: {:?}", read);
    Ok(read
        .recently_viewed
        .iter()
        .take(5)
        .cloned()
        .map(|path| FileInfo { path })
        .collect())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // setup logging
    tracing_subscriber::fmt()
        .with_line_number(true)
        .with_file(true)
        .with_thread_ids(true)
        .with_env_filter({
            use tracing::level_filters::LevelFilter;
            use tracing_subscriber::EnvFilter;
            let env_filter = EnvFilter::builder();
            // show upto INFO logs in debug builds by default
            #[cfg(debug_assertions)]
            let env_filter = env_filter.with_default_directive(LevelFilter::INFO.into());
            // show only WARN and ERROR logs in release builds(builds without debug assertions enabled)
            #[cfg(not(debug_assertions))]
            let env_filter = env_filter.with_default_directive(LevelFilter::WARN.into());
            env_filter.from_env_lossy()
        })
        .init();
    let config = AppConfig::load_or_default();

    let builder = tauri::Builder::default();

    // we don't need more than 2 because we only have one event processor on JS side
    // TODO: consider making this configurable?
    // Maybe we should also consider the payload configuration
    let (s, r) = channel::<Vec<u8>>(2);
    let (sc, rc) = channel::<()>(1);

    let invoke_handler = {
        let builder = tauri_specta::ts::builder().commands(tauri_specta::collect_commands![
            crate::unzip::try_unzip,
            crate::unzip::cancel_unzip,
            crate::unzip::file_password_submit,
            recently_used
        ]);

        #[cfg(all(debug_assertions, not(mobile)))]
        let builder = builder.path("../src/bindings.ts");

        builder
            .build()
            .expect("Failed to setup builder with tauri specta")
    };

    builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        // app config
        .manage(RwLock::new(config))
        // sender
        .manage(Arc::new(Mutex::new(s)))
        .manage(Arc::new(Mutex::new(sc)))
        // receiver
        .manage(Arc::new(Mutex::new(r)))
        .manage(Arc::new(Mutex::new(rc)))
        .invoke_handler(invoke_handler)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
