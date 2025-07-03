use testcontainers::{ContainerAsync, GenericImage};

use super::service::start_service;
use crate::error::Result;
use crate::scenario::Service;

pub async fn create_kafka_container(service: Service) -> Result<ContainerAsync<GenericImage>> {
    start_service(service).await
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::scenario::ServiceType;

    use super::*;

    #[tokio::test]
    async fn test_create_kafka_container() {
        let mut env = HashMap::new();
        // Generate this value beforehand and paste it in; don't generate dynamically here
        env.insert(
            "CLUSTER_ID".to_string(),
            "q3xFh1d8S8Cojhbmz3LZmw==".to_string(),
        );
        env.insert("CONFLUENT_USERNAME".to_string(), "user".to_string());
        env.insert("CONFLUENT_PASSWORD".to_string(), "pass".to_string());
        env.insert(
            "KAFKA_AUTO_CREATE_TOPICS_ENABLE".to_string(),
            "true".to_string(),
        );

        let service = Service {
            service_type: ServiceType::Kafka, // Or a new variant like `ConfluentStack` if preferred
            image: "confluentinc/confluent-local".into(),
            tag: Some("7.5.1".into()),
            container_name: Some("ozymandias-confluent".into()),
            ports: vec![2181, 9092, 8081, 8082, 8083, 8088, 9021],
            wait_for_log: Some("Server started, listening for requests...".into()), // Safe log pattern from Jetty
            alias: Some("confluent".into()),
            env: Some(env),
        };

        let container = create_kafka_container(service).await;
        assert!(
            container.is_ok(),
            "Failed to create kafka container: {:?}",
            container
        );
        let container = container.unwrap();
        // Since ContainerAsync does not have is_running, we just check that the container exists.
        assert!(
            !container.id().is_empty(),
            "Kafka container has an empty ID, likely not running"
        );
    }
}
