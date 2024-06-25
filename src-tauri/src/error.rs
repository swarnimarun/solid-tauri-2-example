use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

#[derive(Error, Debug, Serialize, Deserialize, Type)]
pub enum AppError {
    #[error("failed to load config: {0}")]
    ConfigFailure(String),
    #[error("failed to unzip file: {0}")]
    FailedUnzip(String),
    #[error("invalid file path: {0}")]
    InvalidPath(String),
    #[error("io failure: {0}")]
    IoError(String),
    #[error("password not received.")]
    PasswordFail,
    #[error("event error: {0}")]
    EventError(String),
}
