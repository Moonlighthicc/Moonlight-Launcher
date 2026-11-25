use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Poisoned lock: {0}")]
    LockPoisoned(String),

    #[error("Could not serialize the settings: {0}")]
    Serialize(String),

    #[error("Could not save the settings: {0}")]
    SaveFailed(String),

    #[error("Could not read the settings: {0}")]
    LoadFailed(String),

    #[error("An error occurred in a WThread: {0}")]
    WThreadErr(String),

    #[error("{0}")]
    Other(String),
}

impl CoreError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::LockPoisoned(_) => "CORE_LOCK",
            Self::Serialize(_) => "CORE_SERIALIZE",
            Self::SaveFailed(_) => "CORE_SAVE",
            Self::LoadFailed(_) => "CORE_LOAD",
            Self::WThreadErr(_) => "CORE_THREAD",
            Self::Other(_) => "GENERIC",
        }
    }

    pub fn params(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::LockPoisoned(s)
            | Self::Serialize(s)
            | Self::SaveFailed(s)
            | Self::LoadFailed(s)
            | Self::WThreadErr(s)
            | Self::Other(s) => vec![("error", s.clone())],
        }
    }
}
