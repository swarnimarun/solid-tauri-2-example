mod config;
mod error;

use crate::config::AppConfig;
use crate::error::AppError;

use std::{collections::HashMap, sync::Arc};

use keyed_priority_queue::Entry;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{
    async_runtime::{channel, Mutex, Receiver, RwLock, Sender},
    Manager, State, Window,
};
use zip::{read::ZipFile, result::ZipError};

#[derive(Serialize, Deserialize, Type)]
struct FileInfoInTree {
    depth: usize,
    parent: Option<String>,
    name: String,
}

#[derive(Serialize, Deserialize, Type)]
struct FileInfo {
    path: String,
}

fn file_to_event(
    file: &ZipFile,
    tree: &mut fs_tree::FsTree,
    window: &Window,
) -> Result<(), AppError> {
    let size = file.size();
    let path = file.enclosed_name().unwrap();
    let depth = path.ancestors().count();
    // if zero depth then it's the root directory
    if depth == 0 {
        return Ok(());
    }
    let name = path
        .file_name()
        .expect("in zip paths that end in .. shouldn't be possible")
        .to_string_lossy()
        .to_string();
    let p = format!("/{}", path.display());
    let t = if file.is_file() {
        fs_tree::FsTree::Regular
    } else {
        fs_tree::FsTree::new_dir()
    };
    tree.insert(p, t);
    #[derive(Debug, Serialize, Deserialize, Clone)]
    struct Payload {
        depth: usize,
        name: String,
        size: u64,
    }
    window
        .emit("unzip_file", Payload { depth, name, size })
        .map_err(|e| AppError::EventError(e.to_string()))
}

#[tauri::command(async)]
#[specta::specta]
async fn file_password_submit(
    password: String,
    sender: State<'_, Arc<Mutex<Sender<Vec<u8>>>>>,
) -> Result<(), AppError> {
    Ok(sender
        .lock()
        .await
        .send(password.into_bytes())
        .await
        .map_err(|e| AppError::IoError(e.to_string()))?)
}

#[tauri::command(async)]
#[specta::specta]
// NOTE: this command is async so that it can be called
// from JS without blocking the main thread as it
// can be slow
async fn try_unzip(
    window: Window,
    file: FileInfo,
    config: State<'_, RwLock<AppConfig>>,
    receiver: State<'_, Arc<Mutex<Receiver<Vec<u8>>>>>,
) -> Result<(), AppError> {
    // decompress the file and return the file contents list
    let mut archive = zip::ZipArchive::new(
        std::fs::File::open(&file.path).map_err(|e| AppError::IoError(e.to_string()))?,
    )
    .map_err(|e| AppError::FailedUnzip(e.to_string()))?;

    // create tree, add root
    let mut tree = fs_tree::FsTree::new_dir();
    tree.insert("/", fs_tree::FsTree::new_dir());

    let mut s: HashMap<[u8; 2], Vec<u8>> = HashMap::new();

    for i in 0..archive.len() {
        let err = match archive.by_index(i) {
            Ok(file) if file.enclosed_name().is_some() => {
                file_to_event(&file, &mut tree, &window)?;
                continue;
            }
            Ok(file) => {
                tracing::error!(
                    "`SECURITY ISSUE` bad file path(breaks out of root directory): {}",
                    file.name()
                );
                continue;
            }
            // don't continue if error
            Err(err) => err,
        };

        // if false we know it requires a PASSWORD
        if !matches!(
            err,
            ZipError::UnsupportedArchive(ZipError::PASSWORD_REQUIRED)
        ) {
            tracing::info!("successfully unzipped");
            return Err(AppError::FailedUnzip(err.to_string()));
        }
        // manage caching of passwords for files
        let password = if let Some(vv) = archive
            .get_aes_verification_key_and_salt(i)
            .ok()
            .flatten()
            .map(|e| e.verification_value)
        {
            if let Some(password) = s.get(&vv) {
                // if the key is available
                password.clone()
            } else {
                // create alert for password for each file (for now!)
                let _ = window
                    .emit("file-password-request", ())
                    .map_err(|e| AppError::EventError(e.to_string()));
                // assume receiver is supposed to be fullfilled by JS invoke called on event push
                // TODO: add better timeout
                let mut r = receiver.lock().await;
                let password = r.recv().await.ok_or(AppError::PasswordFail)?;
                s.insert(vv, password.clone());
                password
            }
        } else {
            return Err(AppError::FailedUnzip(
                "unable to get password, should be unreachable".to_string(),
            ));
        };

        match archive.by_index_decrypt(i, &password) {
            Ok(file) if file.enclosed_name().is_some() => {
                file_to_event(&file, &mut tree, &window)?;
                continue;
            }
            Ok(file) => {
                tracing::error!(
                    "`SECURITY ISSUE` bad file path(breaks out of root directory): {}",
                    file.name()
                );
                continue;
            }
            // TODO: handle the error here properly
            Err(_) => panic!("failed to decrypt/unzip file"),
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

    Ok(())
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

    let (s, r) = channel::<Vec<u8>>(2);

    let invoke_handler = {
        let builder = tauri_specta::ts::builder().commands(tauri_specta::collect_commands![
            try_unzip,
            recently_used,
            file_password_submit
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
        // receiver
        .manage(Arc::new(Mutex::new(r)))
        .invoke_handler(invoke_handler)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
