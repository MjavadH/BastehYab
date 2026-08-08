use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PublicError {
    pub code: ErrorCode,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NotImplemented,
    InvalidRequest,
    Internal,
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("requested capability is not implemented yet")]
    NotImplemented,
}

impl From<AppError> for PublicError {
    fn from(error: AppError) -> Self {
        match error {
            AppError::NotImplemented => Self {
                code: ErrorCode::NotImplemented,
                message: "This capability is not implemented yet.".to_owned(),
                retryable: false,
            },
        }
    }
}
