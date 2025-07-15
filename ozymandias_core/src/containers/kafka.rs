use std::vec;

use testcontainers::runners::AsyncRunner;
use testcontainers_modules::kafka::Kafka;
use tracing::error;

use super::service::ServiceManager;
use crate::error::{OzymandiasError, Result};
use crate::scenario::Service;

/// Create a Kafka service manager that provides access to the container and port mappings
pub async fn create_kafka_service(service: Service) -> Result<ServiceManager<Kafka>> {
    let kafka = Kafka::default().start().await.map_err(|e| {
        error!("Failed to start Kafka service: {}", e);
        OzymandiasError::ContainerStartError(e)
    })?;
    
    let mut service = service;
    service.ports = vec![9092, 9093];

    ServiceManager::new_from_container(kafka, service).await
}

#[cfg(test)]
mod tests {
    use crate::scenario::ServiceType;

    use super::*;

    #[tokio::test]
    async fn test_create_kafka_service_manager() {
        let service = Service {
            service_type: ServiceType::Kafka,
            image: "".into(),
            tag: None,
            container_name: None,
            ports: vec![],
            wait_for_log: None,
            alias: None,
            env: vec![],
            retry_config: None,
        };

        let service_manager = create_kafka_service(service).await;
        assert!(
            service_manager.is_ok(),
            "Failed to create kafka service manager",
        );

        let service_manager = service_manager.unwrap();

        // Check for port mapping
        let kafka_port = service_manager.get_port(9092);
        assert!(kafka_port.is_some(), "Failed to get port mapping for 9092");

        // Check connection string
        let connection = service_manager.get_connection_string(9092);
        assert!(
            connection.is_some(),
            "Failed to get connection string for port 9092"
        );

        // Stop the container
        service_manager
            .stop()
            .await
            .expect("Failed to stop the container");
    }
}
