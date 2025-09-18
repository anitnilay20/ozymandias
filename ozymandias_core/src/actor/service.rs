use async_trait::async_trait;
use testcontainers::GenericImage;
use tracing::{error, info};

use super::actor_trait::{Actor, ActorInfo, ActorStatus, HealthStatus};
use crate::{containers::service::ServiceManager, error::Result, scenario::Service};

#[derive(Debug)]
pub enum ServiceContainerMessage {
    Start(Box<Service>),
    Stop,
    GetConnectionString { port: u16 },
    WaitReady { duration_secs: u64 },
}

pub struct ServiceContainerActor {
    service_manager: Option<ServiceManager<GenericImage>>,
    is_paused: bool,
}

impl ServiceContainerActor {
    pub fn new() -> Self {
        Self {
            service_manager: None,
            is_paused: false,
        }
    }
}

#[async_trait]
impl Actor<ServiceContainerMessage> for ServiceContainerActor {
    async fn handle(&mut self, msg: ServiceContainerMessage) -> Result<()> {
        if self.is_paused {
            match msg {
                ServiceContainerMessage::Stop => {
                    // Allow stop even when paused
                }
                _ => {
                    info!(
                        "Service container actor is paused, ignoring message: {:?}",
                        msg
                    );
                    return Ok(());
                }
            }
        }

        match msg {
            ServiceContainerMessage::Start(service) => {
                info!(
                    "Starting generic service container: {:?}",
                    service.service_type
                );
                let service_manager = ServiceManager::<GenericImage>::new(*service).await?;
                self.service_manager = Some(service_manager);
                info!("Generic service container started successfully");
                Ok(())
            }
            ServiceContainerMessage::Stop => {
                info!("Stopping service container");
                if let Some(service_manager) = self.service_manager.take() {
                    service_manager.stop().await?;
                    info!("Service container stopped successfully");
                } else {
                    info!("No service container to stop");
                }
                self.is_paused = false; // Reset pause state when stopped
                Ok(())
            }
            ServiceContainerMessage::GetConnectionString { port } => {
                if let Some(ref service_manager) = self.service_manager {
                    if let Some(connection_string) = service_manager.get_connection_string(port) {
                        info!("Connection string for port {}: {}", port, connection_string);
                    } else {
                        error!("No connection string available for port {}", port);
                    }
                } else {
                    error!("No service container running");
                }
                Ok(())
            }
            ServiceContainerMessage::WaitReady { duration_secs } => {
                if let Some(ref service_manager) = self.service_manager {
                    info!("Waiting {} seconds for service to be ready", duration_secs);
                    service_manager.wait_ready(duration_secs).await;
                    info!("Service wait completed");
                } else {
                    error!("No service container running to wait for");
                }
                Ok(())
            }
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down service container actor");
        if let Some(service_manager) = self.service_manager.take() {
            service_manager.stop().await?;
        }
        Ok(())
    }

    async fn pause(&mut self) -> Result<()> {
        info!("Pausing service container actor");
        self.is_paused = true;
        Ok(())
    }

    async fn restart(&mut self) -> Result<()> {
        info!("Restarting service container actor");
        Ok(())
    }

    async fn resume(&mut self) -> Result<()> {
        info!("Resuming service container actor");
        self.is_paused = false;
        Ok(())
    }

    async fn status(&self) -> ActorStatus {
        match (&self.service_manager, self.is_paused) {
            (Some(_), false) => ActorStatus::Running,
            (Some(_), true) => ActorStatus::Paused,
            (None, _) => ActorStatus::Stopped,
        }
    }

    async fn health_check(&self) -> HealthStatus {
        if let Some(ref service_manager) = self.service_manager {
            let container_id = service_manager.container.id().to_string();
            HealthStatus::healthy("Generic service container is running")
                .with_detail("container_id", &container_id)
                .with_detail(
                    "service_type",
                    service_manager.service.service_type.to_string(),
                )
                .with_detail("paused", self.is_paused.to_string())
        } else {
            HealthStatus::unhealthy("No service container running")
        }
    }

    async fn get_info(&self) -> ActorInfo {
        let mut info = ActorInfo::new("ServiceContainerActor")
            .with_metadata("type", "generic_container_actor");

        if let Some(ref service_manager) = self.service_manager {
            let container_id = service_manager.container.id().to_string();
            info = info
                .with_metadata("container_id", &container_id)
                .with_metadata(
                    "service_type",
                    service_manager.service.service_type.to_string(),
                )
                .with_metadata("image", &service_manager.service.image);

            if let Some(ref tag) = service_manager.service.tag {
                info = info.with_metadata("tag", tag);
            }

            // Add port mappings
            for (container_port, host_port) in &service_manager.port_mappings {
                info = info.with_metadata(
                    format!("port_mapping_{}_{}", container_port, host_port),
                    format!("{}:{}", container_port, host_port),
                );
            }
        }

        info
    }

    async fn validate(&self) -> Result<Vec<String>> {
        let mut errors = Vec::new();

        if let Some(ref service_manager) = self.service_manager {
            // Check if container ID is available
            if service_manager.container.id().is_empty() {
                errors.push("Container ID is empty".to_string());
            }

            // Check if expected ports are mapped
            for port in &service_manager.service.ports {
                if !service_manager.port_mappings.contains_key(port) {
                    errors.push(format!("Port {} is not mapped", port));
                }
            }
        }

        Ok(errors)
    }
}

impl Default for ServiceContainerActor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use super::*;
    use crate::{actor::runner::run_actor, scenario::ServiceType};

    #[tokio::test]
    async fn test_service_container_actor_new() {
        let actor = ServiceContainerActor::new();
        assert!(actor.service_manager.is_none());
        assert!(!actor.is_paused);
    }

    #[tokio::test]
    async fn test_service_container_actor_default() {
        let actor = ServiceContainerActor::default();
        assert!(actor.service_manager.is_none());
        assert!(!actor.is_paused);
    }

    #[tokio::test]
    async fn test_actor_status_methods() {
        let mut actor = ServiceContainerActor::new();

        // Test initial status (stopped)
        let status = actor.status().await;
        assert_eq!(status, ActorStatus::Stopped);

        // Test health check when stopped
        let health = actor.health_check().await;
        assert!(!health.healthy);
        assert!(health.message.contains("No service container running"));

        // Test get_info when stopped
        let info = actor.get_info().await;
        assert_eq!(info.name, "ServiceContainerActor");
        assert_eq!(
            info.metadata.get("type"),
            Some(&"generic_container_actor".to_string())
        );
        assert!(info.metadata.get("container_id").is_none());
    }

    #[tokio::test]
    async fn test_handle_stop_when_no_container() {
        let mut actor = ServiceContainerActor::new();

        // Should not fail when stopping with no container
        let result = actor.handle(ServiceContainerMessage::Stop).await;
        assert!(result.is_ok());
        assert!(!actor.is_paused); // Should reset pause state
    }

    #[tokio::test]
    async fn test_handle_get_connection_string_no_container() {
        let mut actor = ServiceContainerActor::new();

        // Should not fail when getting connection string with no container
        let result = actor
            .handle(ServiceContainerMessage::GetConnectionString { port: 8080 })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_wait_ready_no_container() {
        let mut actor = ServiceContainerActor::new();

        // Should not fail when waiting with no container
        let result = actor
            .handle(ServiceContainerMessage::WaitReady { duration_secs: 1 })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_shutdown() {
        let mut actor = ServiceContainerActor::new();

        // Should not fail when shutting down with no container
        let result = actor.shutdown().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_restart() {
        let mut actor = ServiceContainerActor::new();

        // Should not fail when restarting with no container
        let result = actor.restart().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_pause_resume_directly() {
        let mut actor = ServiceContainerActor::new();

        // Test pause
        actor.pause().await.unwrap();
        assert!(actor.is_paused);
        assert_eq!(actor.status().await, ActorStatus::Stopped); // Still stopped since no service

        // Test resume
        actor.resume().await.unwrap();
        assert!(!actor.is_paused);
    }

    #[tokio::test]
    async fn test_validation_no_container() {
        let actor = ServiceContainerActor::new();

        let errors = actor.validate().await.unwrap();
        // Should have no errors when no container is running
        assert!(errors.is_empty());
    }

    #[tokio::test]
    async fn test_allowed_messages_when_paused() {
        let mut actor = ServiceContainerActor::new();

        // Pause the actor
        actor.pause().await.unwrap();
        assert!(actor.is_paused);

        // Stop message should be allowed when paused
        assert!(actor.handle(ServiceContainerMessage::Stop).await.is_ok());

        actor.pause().await.unwrap(); // Re-pause after stop resets pause state

        // Other messages should be ignored when paused
        let service = Service {
            service_type: ServiceType::Custom("test".to_string()),
            image: "test".into(),
            tag: None,
            container_name: None,
            ports: vec![],
            wait_for_log: None,
            alias: None,
            env: vec![],
            retry_config: None,
        };

        // These should be ignored due to pause
        assert!(actor
            .handle(ServiceContainerMessage::Start(Box::new(service)))
            .await
            .is_ok());
        assert!(actor.service_manager.is_none()); // Should not have started

        assert!(actor
            .handle(ServiceContainerMessage::GetConnectionString { port: 8080 })
            .await
            .is_ok());
        assert!(actor
            .handle(ServiceContainerMessage::WaitReady { duration_secs: 1 })
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_message_debug_formatting() {
        // Test that messages can be formatted for debug
        let msg1 = ServiceContainerMessage::Start(Box::new(Service {
            service_type: ServiceType::Custom("test".to_string()),
            image: "test".into(),
            tag: None,
            container_name: None,
            ports: vec![],
            wait_for_log: None,
            alias: None,
            env: vec![],
            retry_config: None,
        }));

        let debug_str = format!("{:?}", msg1);
        assert!(debug_str.contains("Start"));

        let msg2 = ServiceContainerMessage::Stop;
        let debug_str = format!("{:?}", msg2);
        assert!(debug_str.contains("Stop"));

        let msg3 = ServiceContainerMessage::GetConnectionString { port: 8080 };
        let debug_str = format!("{:?}", msg3);
        assert!(debug_str.contains("GetConnectionString"));
        assert!(debug_str.contains("8080"));

        let msg4 = ServiceContainerMessage::WaitReady { duration_secs: 5 };
        let debug_str = format!("{:?}", msg4);
        assert!(debug_str.contains("WaitReady"));
        assert!(debug_str.contains("5"));
    }

    #[tokio::test]
    async fn test_service_container_actor() {
        let (tx, rx) = mpsc::channel::<ServiceContainerMessage>(10);
        let actor = ServiceContainerActor::new();

        let handle = tokio::spawn(async move { run_actor(actor, rx).await });

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

        tx.send(ServiceContainerMessage::Start(Box::new(service)))
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        tx.send(ServiceContainerMessage::GetConnectionString { port: 7000 })
            .await
            .unwrap();

        tx.send(ServiceContainerMessage::WaitReady { duration_secs: 1 })
            .await
            .unwrap();

        tx.send(ServiceContainerMessage::Stop).await.unwrap();

        drop(tx);

        handle.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn test_service_container_actor_pause_resume() {
        let mut actor = ServiceContainerActor::new();

        // Test initial state
        assert!(!actor.is_paused);

        // Test pause
        actor.pause().await.unwrap();
        assert!(actor.is_paused);

        // Test resume
        actor.resume().await.unwrap();
        assert!(!actor.is_paused);

        // Test that paused actor ignores most messages
        actor.pause().await.unwrap();

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

        // This should be ignored due to pause
        let result = actor
            .handle(ServiceContainerMessage::Start(Box::new(service)))
            .await;
        assert!(result.is_ok());
        assert!(actor.service_manager.is_none()); // Should not have started

        // Resume and try again
        actor.resume().await.unwrap();
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

        // This should work after resume
        let result = actor
            .handle(ServiceContainerMessage::Start(Box::new(service)))
            .await;
        assert!(result.is_ok());
        assert!(actor.service_manager.is_some()); // Should have started

        // Cleanup
        actor.handle(ServiceContainerMessage::Stop).await.unwrap();
    }
}
