use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Failed to get the device code: {0}")]
    DeviceCodeFailed(String),

    #[error("Failed to authenticate with Microsoft: {0}")]
    AuthFailed(String),

    #[error("Failed to save the tokens: {0}")]
    SaveTokensFailed(String),

    #[error("Failed to delete the tokens: {0}")]
    DeleteTokensFailed(String),

    #[error("The blocking task failed: {0}")]
    SpawnBlocking(String),

    #[error("Authentication was cancelled or expired: {0}")]
    AuthCancelled(String),

    #[error("Session expired. Please sign in again: {0}")]
    SessionExpired(String),

    #[error(transparent)]
    CoreError(#[from] crate::core::errors::CoreError),
}

impl AuthError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::DeviceCodeFailed(_) => "AUTH_DEVICE_CODE",
            Self::AuthFailed(_) => "AUTH_FAILED",
            Self::SaveTokensFailed(_) => "AUTH_TOKENS_SAVE",
            Self::DeleteTokensFailed(_) => "AUTH_TOKENS_DEL",
            Self::SpawnBlocking(_) => "AUTH_BLOCKED",
            Self::AuthCancelled(_) => "AUTH_CANCELLED",
            Self::SessionExpired(_) => "AUTH_SESSION_EXPIRED",
            Self::CoreError(e) => e.code(),
        }
    }

    pub fn params(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::DeviceCodeFailed(s)
            | Self::AuthFailed(s)
            | Self::SaveTokensFailed(s)
            | Self::DeleteTokensFailed(s)
            | Self::SpawnBlocking(s)
            | Self::AuthCancelled(s)
            | Self::SessionExpired(s) => vec![("error", s.clone())],
            Self::CoreError(e) => e.params(),
        }
    }
}
