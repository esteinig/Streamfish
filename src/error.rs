use thiserror::Error;

use crate::{client::error::ClientError, server::error::ServerError};

#[derive(Error, Debug)]
pub enum StreamfishError {
    /// Represents a failure to run client tasks
    #[error("Streamfish client error")]
    StreamfishClient(#[from] ClientError),
    /// Represents a failure to run server tasks
    #[error("Streamfish server error")]
    StreamfishServer(#[from] ServerError),
    /// Represents a failure to configure Streamfish
    #[error("Streamfish configuration error")]
    StreamfishConfig(#[from] StreamfishConfigError),
}


#[derive(Error, Debug)]
pub enum StreamfishConfigError {
    /// Represents a failure to open the configuration file
    #[error("Failed to open the configuration file")]
    TomlConfigFile(#[from] std::io::Error),
    /// Represents a failure to parse the configuration file
    #[error("Failed to parse the configuration file")]
    TomlConfigParse(#[from] toml::de::Error),
    /// Represents a failure to parse a target string from the configuration
    #[error("Failed to parse target bounds due to incorrect of start or end: {0}")]
    TargetBounds(String),
    /// Represents a failure to parse a target string from the configuration
    #[error("Failed to parse target due to incorrect format: {0}")]
    TargetFormat(String),

    /// Represents a failure to parse a target string from the configuration
    #[error("Failed to find target file: {0}")]
    TargetFileNotFound(String),
}