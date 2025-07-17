use std::collections::HashMap;
use testcontainers::{
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
    ContainerAsync, GenericImage, ImageExt,
};
use tracing::{error, info};

use crate::error::{OzymandiasError, Result};
use crate::scenario::Service;

use error_stack::Report;

async fn start_service(service: Service) -> Result<ContainerAsync<GenericImage>> {
    let mut container_builder =
        GenericImage::new(service.image, service.tag.unwrap_or("latest".to_string()));

    for port in &service.ports {
        container_builder = container_builder.with_exposed_port(port.tcp());
    }

    container_builder = container_builder.with_wait_for(WaitFor::message_on_stdout(
        service.wait_for_log.unwrap_or_default(),
    ));

    let mut container_request = container_builder.with_env_var("XXX", "XXX");

    for (key, value) in service.env.iter() {
        container_request = container_request.with_env_var(key, value);
    }

    container_request
        .start()
        .await
        .map_err(|e| Report::new(OzymandiasError::ContainerStartError(e)))
}

/// A struct that manages a containerized service and provides access to its properties
pub struct ServiceManager<T: testcontainers::Image> {
    /// The container running the service
    pub container: ContainerAsync<T>,
    /// Mapping of container ports to host ports
    pub port_mappings: HashMap<u16, u16>,
    /// The original service configuration
    pub service: Service,
}

impl<T: testcontainers::Image> ServiceManager<T> {
    /// Create a new ServiceManager by starting the service in a container
    pub async fn new(service: Service) -> Result<ServiceManager<GenericImage>> {
        let tag = service.clone().tag.unwrap_or("latest".to_string());

        info!(
            "Starting service: {:?} ({}{})",
            service.service_type, service.image, tag,
        );

        let container = start_service(service.clone()).await?;
        let mut port_mappings = HashMap::new();

        // Get all port mappings
        for port in &service.ports {
            match container.get_host_port_ipv4(*port).await {
                Ok(host_port) => {
                    info!("Port mapping for {}: {}", port, host_port);
                    port_mappings.insert(*port, host_port);
                }
                Err(e) => {
                    error!("Failed to get port mapping for {}: {:?}", port, e);
                    // Continue even if we fail to get a port mapping
                }
            }
        }

        Ok(ServiceManager {
            container,
            port_mappings,
            service,
        })
    }

    pub async fn new_from_container(
        container: ContainerAsync<T>,
        service: Service,
    ) -> Result<ServiceManager<T>> {
        let mut port_mappings = HashMap::new();

        // Get all port mappings
        for port in &service.ports {
            match container.get_host_port_ipv4(*port).await {
                Ok(host_port) => {
                    info!("Port mapping for {}: {}", port, host_port);
                    port_mappings.insert(*port, host_port);
                }
                Err(e) => {
                    error!("Failed to get port mapping for {}: {:?}", port, e);
                    // Continue even if we fail to get a port mapping
                }
            }
        }

        Ok(ServiceManager {
            container,
            port_mappings,
            service,
        })
    }

    /// Get the host port that maps to a container port
    pub fn get_port(&self, container_port: u16) -> Option<u16> {
        self.port_mappings.get(&container_port).copied()
    }

    /// Get a connection string for the service (e.g., "localhost:1234")
    pub fn get_connection_string(&self, container_port: u16) -> Option<String> {
        self.get_port(container_port)
            .map(|port| format!("localhost:{port}"))
    }

    /// Stop the container
    pub async fn stop(self) -> Result<()> {
        info!("Stopping service: {:?}", self.service.service_type);
        self.container.stop().await.map_err(|e| {
            error!("Failed to stop container: {:?}", e);
            Report::new(OzymandiasError::ContainerStartError(e))
        })
    }

    /// Wait for a given duration to ensure the service is fully initialized
    pub async fn wait_ready(&self, duration_secs: u64) -> &Self {
        info!(
            "Waiting {} seconds for service to be fully ready: {:?}",
            duration_secs, self.service.service_type
        );
        tokio::time::sleep(std::time::Duration::from_secs(duration_secs)).await;
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::scenario::ServiceType;
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn test_service_manager() {
        let service = Service {
            service_type: ServiceType::RedisCluster,
            image: "grokzen/redis-cluster".to_string(),
            tag: Some("6.0.7".to_string()),
            container_name: None,
            ports: vec![7000, 7001, 7002, 7003, 7004, 7005],
            wait_for_log: Some("Ready to accept connections".to_string()),
            alias: None,
            env: Vec::new(),
            retry_config: None,
        };

        let service_manager = ServiceManager::<GenericImage>::new(service).await;
        assert!(service_manager.is_ok(), "Failed to create service manager");

        let service_manager = service_manager.unwrap();
        // Check that we have port mappings
        assert!(
            !service_manager.port_mappings.is_empty(),
            "No port mappings found"
        );

        // Check connection string
        let connection = service_manager.get_connection_string(7000);
        assert!(
            connection.is_some(),
            "Failed to get connection string for port 7000"
        );
        println!("Connection string: {}", connection.unwrap());

        // Wait for a short time and then stop the container
        tokio::time::sleep(Duration::from_secs(2)).await;
        let stop_result = service_manager.stop().await;
        assert!(stop_result.is_ok(), "Failed to stop container");
    }
}
