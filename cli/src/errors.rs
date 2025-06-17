use error_stack::Result;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CLIError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to parse arguments: {0}")]
    Clap(#[from] clap::Error),

    #[error("Failed to read TOML from dir: {0}")]
    TomlRead(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Other error: {0}")]
    Other(String),
}

pub type CLIResult<T> = Result<T, CLIError>;
