use keyed_priority_queue::KeyedPriorityQueue;
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, path::PathBuf};

use crate::error::AppError;

#[derive(Serialize, Deserialize, Debug)]
pub struct AppConfig {
    pub multi_threaded_decompression: bool,
    pub recently_viewed: VecDeque<String>,

    // used for performance and maintaining ordering
    #[serde(skip_serializing, skip_deserializing)]
    pub recently_viewed_set: KeyedPriorityQueue<String, usize>,
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
    pub fn load() -> Result<Self, AppError> {
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
    pub fn save(&self) -> Result<(), AppError> {
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
    pub fn load_or_default() -> Self {
        Self::load().unwrap_or_default()
    }
}
