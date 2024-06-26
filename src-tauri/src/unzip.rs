use std::{collections::HashMap, sync::Arc};

use futures::{pin_mut, FutureExt};
use serde::{Deserialize, Serialize};
use tauri::{
    async_runtime::{Mutex, Receiver, RwLock, Sender},
    Manager, State, Window,
};
use zip::{read::ZipFile, result::ZipError};

use crate::{config::AppConfig, error::AppError, prefixtree, FileInfo};

fn append_file_to_tree(file: &ZipFile, tree: &mut prefixtree::PrefixTree) {
    let path = file.enclosed_name().unwrap();
    tree.insert(&path);
}

fn file_to_event(file: &ZipFile, window: &Window) -> Result<(), AppError> {
    let size = file.size();
    let path = file.enclosed_name().unwrap();
    let depth = path.ancestors().count();
    let path = path.to_string_lossy().to_string();
    #[derive(Debug, Serialize, Deserialize, Clone)]
    struct Payload {
        path: String,
        depth: u32,
        size: u32,
    }
    window
        .emit(
            "unzip-file",
            Payload {
                depth: depth as u32,
                path,
                size: size as u32,
            },
        )
        .map_err(|e| AppError::EventError(e.to_string()))
}

#[tauri::command(async)]
#[specta::specta]
pub async fn file_password_submit(
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
/// cancellation only works if the channel buffer has enough space, for an immediate cancellation
/// this ensures we can't cancel twice
pub async fn cancel_unzip(cancel: State<'_, Arc<Mutex<Sender<()>>>>) -> Result<(), AppError> {
    Ok(cancel
        .try_lock()
        .map_err(|e| AppError::IoError(e.to_string()))?
        .send(())
        .await
        .map_err(|e| AppError::IoError(e.to_string()))?)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn try_unzip_prefixtree<'a>(
    window: Window,
    file: FileInfo,
    config: State<'_, RwLock<AppConfig>>,
    receiver: State<'_, Arc<Mutex<Receiver<Vec<u8>>>>>,
    cancel: State<'_, Arc<Mutex<Receiver<()>>>>,
) -> Result<prefixtree::PrefixTree, AppError> {
    {
        // drain cancellation
        _ = cancel.try_lock();
    }
    // decompress the file and return the file contents list
    let mut archive = zip::ZipArchive::new(
        std::fs::File::open(&file.path).map_err(|e| AppError::IoError(e.to_string()))?,
    )
    .map_err(|e| AppError::FailedUnzip(e.to_string()))?;

    let mut s: HashMap<[u8; 2], Vec<u8>> = HashMap::new();
    let mut tree = prefixtree::PrefixTree::new();

    for i in 0..archive.len() {
        let err = match archive.by_index(i) {
            Ok(file) if file.enclosed_name().is_some() => {
                append_file_to_tree(&file, &mut tree);
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
                let mut rc = cancel.lock().await;
                // cancel, if rc is received
                let password_fut = r.recv().fuse();
                let cancel_fut = rc.recv().fuse();
                pin_mut!(password_fut, cancel_fut);
                futures::select! {
                    password = password_fut => {
                        let password = password.ok_or(AppError::PasswordFail)?;
                        s.insert(vv, password.clone());
                        password
                    },
                    _ = cancel_fut => {
                        return Err(AppError::PasswordFail)
                    },
                }
            }
        } else {
            return Err(AppError::FailedUnzip(
                "unable to get password, should be unreachable".to_string(),
            ));
        };

        // access file with password
        match archive.by_index_decrypt(i, &password) {
            Ok(file) if file.enclosed_name().is_some() => {
                append_file_to_tree(&file, &mut tree);
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
    if let Some((index, _)) = write
        .recently_viewed
        .iter()
        .enumerate()
        .find(|f| f.1.eq(&file.path))
    {
        write.recently_viewed.remove(index);
    }
    write.recently_viewed.push_front(file.path);
    // cap at 5 elements max
    write.recently_viewed.remove(5);
    write.save()?;

    Ok(tree)
}

#[tauri::command(async)]
#[specta::specta]
// NOTE: this command is async so that it can be called
// from JS without blocking the main thread as it
// can be slow
pub async fn try_unzip(
    window: Window,
    file: FileInfo,
    config: State<'_, RwLock<AppConfig>>,
    receiver: State<'_, Arc<Mutex<Receiver<Vec<u8>>>>>,
    cancel: State<'_, Arc<Mutex<Receiver<()>>>>,
) -> Result<(), AppError> {
    {
        // drain cancellation
        _ = cancel.try_lock();
    }
    // decompress the file and return the file contents list
    let mut archive = zip::ZipArchive::new(
        std::fs::File::open(&file.path).map_err(|e| AppError::IoError(e.to_string()))?,
    )
    .map_err(|e| AppError::FailedUnzip(e.to_string()))?;

    let mut s: HashMap<[u8; 2], Vec<u8>> = HashMap::new();

    for i in 0..archive.len() {
        let err = match archive.by_index(i) {
            Ok(file) if file.enclosed_name().is_some() => {
                file_to_event(&file, &window)?;
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
                let mut rc = cancel.lock().await;
                // cancel, if rc is received
                let password_fut = r.recv().fuse();
                let cancel_fut = rc.recv().fuse();
                pin_mut!(password_fut, cancel_fut);
                futures::select! {
                    password = password_fut => {
                        let password = password.ok_or(AppError::PasswordFail)?;
                        s.insert(vv, password.clone());
                        password
                    },
                    _ = cancel_fut => {
                        return Err(AppError::PasswordFail)
                    },
                }
            }
        } else {
            return Err(AppError::FailedUnzip(
                "unable to get password, should be unreachable".to_string(),
            ));
        };

        // access file with password
        match archive.by_index_decrypt(i, &password) {
            Ok(file) if file.enclosed_name().is_some() => {
                file_to_event(&file, &window)?;
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
    if let Some((index, _)) = write
        .recently_viewed
        .iter()
        .enumerate()
        .find(|f| f.1.eq(&file.path))
    {
        write.recently_viewed.remove(index);
    }
    write.recently_viewed.push_front(file.path);
    // cap at 5 elements max
    write.recently_viewed.remove(5);
    write.save()?;

    Ok(())
}
