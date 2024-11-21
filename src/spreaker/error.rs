use std::fmt::Display;

#[derive(Debug)]
pub enum SpreakerError {
    RequestError(reqwest::Error),
    JsonError(reqwest::Error),
    Runtime(tokio::task::JoinError),
    IOError(std::io::Error),
}

impl From<reqwest::Error> for SpreakerError {
    fn from(e: reqwest::Error) -> Self {
        SpreakerError::RequestError(e)
    }
}

impl Display for SpreakerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpreakerError::RequestError(e) => write!(f, "Request error: {}", e),
            SpreakerError::JsonError(e) => write!(f, "Json error: {}", e),
            SpreakerError::Runtime(e) => write!(f, "Runtime error: {}", e),
            SpreakerError::IOError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for SpreakerError {}
