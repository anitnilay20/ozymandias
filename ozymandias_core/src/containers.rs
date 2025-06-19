use testcontainers::{
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
    ContainerAsync, GenericImage,
};

use crate::error::{OzymandiasError, Result};
use crate::scenario::Service;

use error_stack::Report;

pub async fn start_service(service: Service) -> Result<ContainerAsync<GenericImage>> {
    let mut container_builder = GenericImage::new(service.image, service.tag.unwrap());

    for port in &service.ports {
        container_builder = container_builder.with_exposed_port(port.tcp());
    }

    container_builder
        .with_wait_for(WaitFor::message_on_stdout(
            service.wait_for_log.unwrap_or_default(),
        ))
        .start()
        .await
        .map_err(|e| Report::new(OzymandiasError::ContainerStartError(e)))
}

#[cfg(test)]
mod tests {
    use crate::scenario::ServiceType;

    use super::*;

    #[tokio::test]
    async fn test_start_service() {
        let service = Service {
            service_type: ServiceType::RedisCluster,
            image: "grokzen/redis-cluster".to_string(),
            tag: Some("6.0.7".to_string()),
            container_name: None,
            ports: vec![7000, 7001, 7002, 7003, 7004, 7005],
            wait_for_log: Some("Ready to accept connections".to_string()),
            alias: None,
        };

        let container = start_service(service).await;
        assert!(container.is_ok());
    }
}
