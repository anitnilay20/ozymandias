use testcontainers::TestcontainersError;
use thiserror::Error;

use crate::scenario::ServiceType;

#[derive(Error, Debug)]
pub enum OzymandiasError {
    #[error("Failed to load scenario from TOML file: {0}")]
    TomlError(#[from] toml::de::Error),

    #[error("Failed to find file: {0}")]
    FileError(#[from] tokio::io::Error),

    #[error("Unable to parse TOML file: {0}")]
    TomlDeserializationError(String),

    #[error("Failed to start container: {0}")]
    ContainerStartError(#[from] TestcontainersError),

    #[error("Failed to send message to Kafka: {0}")]
    KafkaSendError(#[from] rdkafka::error::KafkaError),

    #[error("Service Image is missing for service type: {0}")]
    InvalidServiceImage(ServiceType),
}

pub type Result<T> = error_stack::Result<T, OzymandiasError>;
