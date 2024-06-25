use std::{collections::VecDeque, env, path::PathBuf};

use keyed_priority_queue::{Entry, KeyedPriorityQueue};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{async_runtime::RwLock, State};
use thiserror::Error;

#[derive(Error, Debug, Serialize, Deserialize, Type)]
enum AppError {
    #[error("failed to load config: {0}")]
    ConfigFailure(String),
    #[error("failed to unzip file: {0}")]
    FailedUnzip(String),
    #[error("invalid file path: {0}")]
    InvalidPath(String),
    #[error("io failure: {0}")]
    IoError(String),
}

#[derive(Serialize, Deserialize, Type)]
struct FileInfo {
    path: String,
}

#[tauri::command(async)]
#[specta::specta]
// NOTE: this command is async so that it can be called
// from JS without blocking the main thread as it
// can be slow
async fn try_unzip(
    file: FileInfo,
    config: State<'_, RwLock<AppConfig>>,
) -> Result<Vec<FileInfo>, AppError> {
    // decompress the file and return the file contents list
    let archive = zip::ZipArchive::new(
        std::fs::File::open(&file.path).map_err(|e| AppError::IoError(e.to_string()))?,
    )
    .map_err(|e| AppError::FailedUnzip(e.to_string()))?;

    // create tree, add root
    let mut tree = fs_tree::FsTree::new_dir();
    tree.insert("/", fs_tree::FsTree::new_dir());
    for i in archive.file_names() {
        let p = format!("/{i}");
        tree.insert(p, fs_tree::FsTree::new_dir());
    }

    for (t, p) in tree.iter() {
        let depth = p.ancestors().count();
        if depth != 0 {
            tracing::info!("tree({}): {:?}", depth, t);
        }
    }

    // handle insertion and removal of recently viewed
    let mut write = config.write().await;
    let size = write.recently_viewed_set.len();
    let mut remove_item = None;
    match write.recently_viewed_set.entry(file.path.clone()) {
        Entry::Occupied(entry) => {
            let index = *entry.get_priority();
            entry.set_priority(size);
            write.recently_viewed.push_front(file.path);
            // will cap at 5 max elements
            write.recently_viewed.remove(index);
            write.save()?;
        }
        Entry::Vacant(entry) => {
            entry.set_priority(size);
            write.recently_viewed.push_front(file.path);
            // will cap at 5 max elements
            remove_item = write.recently_viewed.remove(5);
            write.save()?;
        }
    };
    if let Some(path) = remove_item {
        write.recently_viewed_set.remove(&path);
    }

    Ok(vec![])
    // TODO: move to using streams instead of blocking command
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

#[derive(Serialize, Deserialize, Debug)]
struct AppConfig {
    multi_threaded_decompression: bool,
    recently_viewed: VecDeque<String>,

    // used for performance and maintaining ordering
    #[serde(skip_serializing, skip_deserializing)]
    recently_viewed_set: KeyedPriorityQueue<String, usize>,
    // TODO: add more config options
}

impl Default for AppConfig {
    fn default() -> Self {
        tracing::info!("using default config");
        Self {
            multi_threaded_decompression: false,
            recently_viewed: VecDeque::with_capacity(5),
            recently_viewed_set: KeyedPriorityQueue::with_capacity(5),
        }
    }
}

impl AppConfig {
    #[tracing::instrument]
    fn load() -> Result<Self, AppError> {
        tracing::info!("loading config");

        let config_dir = std::env::var("ZIPTAURI_CONFIG_DIR")
            .ok()
            .or_else(|| {
                directories::ProjectDirs::from("", "hoppscotch", "ziptauri")
                    .map(|p| p.config_dir().to_string_lossy().to_string())
            })
            .ok_or_else(|| {
                AppError::ConfigFailure("no valid home directory found for the system".to_string())
            })?;

        let mut c: AppConfig = serde_json::from_slice(
            &std::fs::read(PathBuf::from(config_dir).join("config.json"))
                .map_err(|e| AppError::IoError(e.to_string()))?,
        )
        .map_err(|e| AppError::IoError(e.to_string()))?;

        c.recently_viewed_set = c
            .recently_viewed
            .iter()
            .enumerate()
            .map(|(i, x)| (x.clone(), i))
            .collect();

        Ok(c)
    }

    #[tracing::instrument]
    fn save(&self) -> Result<(), AppError> {
        tracing::info!("saving config");

        let config_dir = std::env::var("ZIPTAURI_CONFIG_DIR")
            .ok()
            .or_else(|| {
                directories::ProjectDirs::from("", "hoppscotch", "ziptauri")
                    .map(|p| p.config_dir().to_string_lossy().to_string())
            })
            .ok_or_else(|| {
                AppError::ConfigFailure("no valid home directory found for the system".to_string())
            })?;

        let config_json =
            serde_json::to_string_pretty(self).map_err(|e| AppError::IoError(e.to_string()))?;

        // ensure the path is created before writing
        std::fs::create_dir_all(&config_dir).map_err(|e| AppError::IoError(e.to_string()))?;

        std::fs::write(PathBuf::from(config_dir).join("config.json"), config_json)
            .map_err(|e| AppError::IoError(e.to_string()))
    }

    #[tracing::instrument]
    fn load_or_default() -> Self {
        Self::load().unwrap_or_default()
    }
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
        .manage(RwLock::new(config))
        .invoke_handler(invoke_handler)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
