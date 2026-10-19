#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("{0}")]
    Msg(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
    #[error(transparent)]
    Key(#[from] keyring::Error),
    #[error(transparent)]
    Join(#[from] tokio::task::JoinError),
}

impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type Res<T> = Result<T, AppError>;

pub fn err<T>(m: impl Into<String>) -> Res<T> {
    Err(AppError::Msg(m.into()))
}
