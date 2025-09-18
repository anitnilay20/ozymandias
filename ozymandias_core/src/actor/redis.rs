use async_trait::async_trait;
use bollard::Docker;
use testcontainers::GenericImage;
use tracing::{error, info, warn};

use super::actor_trait::{Actor, ActorInfo, ActorStatus, HealthStatus};
use crate::{
    containers::{redis::create_redis_cluster_container, service::ServiceManager},
    error::Result,
    scenario::Service,
};

#[derive(Debug)]
pub enum RedisContainerMessage {
    Start(Box<Service>),
    Stop,
    GetConnectionString { port: u16 },
    Pause,
    Resume,
    Status,
}

pub struct RedisContainerActor {
    service_manager: Option<ServiceManager<GenericImage>>,
    is_paused: bool,
    docker: Docker,
}

impl RedisContainerActor {
    pub fn new() -> Self {
        let docker = Docker::connect_with_socket_defaults().expect("Failed to connect to Docker");

        Self {
            service_manager: None,
            is_paused: false,
            docker,
        }
    }

    async fn get_container_id(&self) -> Option<String> {
        self.service_manager
            .as_ref()
            .map(|sm| sm.container.id().to_string())
    }
}

#[async_trait]
impl Actor<RedisContainerMessage> for RedisContainerActor {
    async fn handle(&mut self, msg: RedisContainerMessage) -> Result<()> {
        if self.is_paused {
            match msg {
                RedisContainerMessage::Stop
                | RedisContainerMessage::Resume
                | RedisContainerMessage::Status => {
                    // Allow these messages even when paused
                }
                _ => {
                    info!(
                        "Redis container actor is paused, ignoring message: {:?}",
                        msg
                    );
                    return Ok(());
                }
            }
        }

        match msg {
            RedisContainerMessage::Start(service) => {
                info!("Starting Redis container");
                let service_manager = create_redis_cluster_container(*service).await?;
                self.service_manager = Some(service_manager);
                info!("Redis container started successfully");
                Ok(())
            }
            RedisContainerMessage::Stop => {
                info!("Stopping Redis container");
                if let Some(service_manager) = self.service_manager.take() {
                    service_manager.stop().await?;
                    info!("Redis container stopped successfully");
                } else {
                    info!("No Redis container to stop");
                }
                self.is_paused = false; // Reset pause state when stopped
                Ok(())
            }
            RedisContainerMessage::GetConnectionString { port } => {
                if let Some(ref service_manager) = self.service_manager {
                    if let Some(connection_string) = service_manager.get_connection_string(port) {
                        info!("Connection string for port {}: {}", port, connection_string);
                    } else {
                        error!("No connection string available for port {}", port);
                    }
                } else {
                    error!("No Redis container running");
                }
                Ok(())
            }
            RedisContainerMessage::Pause => {
                info!("Received pause message");
                self.pause().await
            }
            RedisContainerMessage::Resume => {
                info!("Received resume message");
                self.resume().await
            }
            RedisContainerMessage::Status => {
                if let Some(container_id) = self.get_container_id().await {
                    match self.docker.inspect_container(&container_id, None).await {
                        Ok(container_info) => {
                            let state = container_info.state.unwrap_or_default();
                            let status = state
                                .status
                                .map(|s| format!("{:?}", s))
                                .unwrap_or_else(|| "Unknown".to_string());
                            let paused = state.paused.unwrap_or(false);
                            info!(
                                "Container {} status: {}, paused: {}",
                                container_id, status, paused
                            );
                        }
                        Err(e) => {
                            error!("Failed to get container status: {}", e);
                        }
                    }
                } else {
                    info!("No Redis container running");
                }
                Ok(())
            }
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down Redis container actor");
        if let Some(service_manager) = self.service_manager.take() {
            service_manager.stop().await?;
        }
        Ok(())
    }

    async fn pause(&mut self) -> Result<()> {
        info!("Pausing Redis container using Docker API");

        if let Some(container_id) = self.get_container_id().await {
            match self.docker.pause_container(&container_id).await {
                Ok(_) => {
                    info!("Successfully paused Redis container: {}", container_id);
                    self.is_paused = true;
                }
                Err(e) => {
                    error!("Failed to pause Redis container {}: {}", container_id, e);
                    // Fallback to actor-level pause
                    warn!("Falling back to actor-level pause");
                    self.is_paused = true;
                }
            }
        } else {
            warn!("No Redis container running to pause, using actor-level pause");
            self.is_paused = true;
        }

        Ok(())
    }

    async fn restart(&mut self) -> Result<()> {
        info!("Restarting Redis container actor");
        Ok(())
    }

    async fn resume(&mut self) -> Result<()> {
        info!("Resuming Redis container using Docker API");

        if let Some(container_id) = self.get_container_id().await {
            match self.docker.unpause_container(&container_id).await {
                Ok(_) => {
                    info!("Successfully resumed Redis container: {}", container_id);
                    self.is_paused = false;
                }
                Err(e) => {
                    error!("Failed to resume Redis container {}: {}", container_id, e);
                    // Fallback to actor-level resume
                    warn!("Falling back to actor-level resume");
                    self.is_paused = false;
                }
            }
        } else {
            warn!("No Redis container running to resume, using actor-level resume");
            self.is_paused = false;
        }

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
        if let Some(container_id) = self.get_container_id().await {
            match self.docker.inspect_container(&container_id, None).await {
                Ok(container_info) => {
                    let state = container_info.state.unwrap_or_default();
                    let running = state.running.unwrap_or(false);
                    let paused = state.paused.unwrap_or(false);

                    if running && !paused {
                        HealthStatus::healthy("Redis container is running normally")
                            .with_detail("container_id", &container_id)
                            .with_detail("paused", "false")
                    } else if paused {
                        HealthStatus::healthy("Redis container is paused")
                            .with_detail("container_id", &container_id)
                            .with_detail("paused", "true")
                    } else {
                        HealthStatus::unhealthy("Redis container is not running")
                            .with_detail("container_id", &container_id)
                            .with_detail("running", running.to_string())
                    }
                }
                Err(e) => HealthStatus::unhealthy(format!("Failed to inspect container: {}", e))
                    .with_detail("container_id", &container_id),
            }
        } else {
            HealthStatus::unhealthy("No Redis container running")
        }
    }

    async fn get_info(&self) -> ActorInfo {
        let mut info = ActorInfo::new("RedisContainerActor")
            .with_metadata("type", "container_actor")
            .with_metadata("service", "redis_cluster");

        if let Some(container_id) = self.get_container_id().await {
            info = info.with_metadata("container_id", &container_id);
        }

        if let Some(ref service_manager) = self.service_manager {
            for port in [7000, 7001, 7002, 7003, 7004, 7005] {
                if let Some(conn) = service_manager.get_connection_string(port) {
                    info = info.with_metadata(format!("connection_{}", port), conn);
                }
            }
        }

        info
    }

    async fn validate(&self) -> Result<Vec<String>> {
        let mut errors = Vec::new();

        // Check if docker client is accessible
        if let Err(e) = self.docker.ping().await {
            errors.push(format!("Docker client not accessible: {}", e));
        }

        // Check container state if running
        if let Some(container_id) = self.get_container_id().await {
            match self.docker.inspect_container(&container_id, None).await {
                Ok(container_info) => {
                    let state = container_info.state.unwrap_or_default();
                    if !state.running.unwrap_or(false) && !self.is_paused {
                        errors.push("Container is not running but actor is not paused".to_string());
                    }
                }
                Err(e) => {
                    errors.push(format!("Cannot inspect container {}: {}", container_id, e));
                }
            }
        }

        Ok(errors)
    }
}

impl Default for RedisContainerActor {
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
    async fn test_redis_container_actor_new() {
        let actor = RedisContainerActor::new();
        assert!(actor.service_manager.is_none());
        assert!(!actor.is_paused);
    }

    #[tokio::test]
    async fn test_redis_container_actor_default() {
        let actor = RedisContainerActor::default();
        assert!(actor.service_manager.is_none());
        assert!(!actor.is_paused);
    }

    #[tokio::test]
    async fn test_actor_status_methods() {
        let mut actor = RedisContainerActor::new();

        // Test initial status (stopped)
        let status = actor.status().await;
        assert_eq!(status, ActorStatus::Stopped);

        // Test health check when stopped
        let health = actor.health_check().await;
        assert!(!health.healthy);
        assert!(health.message.contains("No Redis container running"));

        // Test get_info when stopped
        let info = actor.get_info().await;
        assert_eq!(info.name, "RedisContainerActor");
        assert_eq!(
            info.metadata.get("type"),
            Some(&"container_actor".to_string())
        );
        assert_eq!(
            info.metadata.get("service"),
            Some(&"redis_cluster".to_string())
        );
        assert!(info.metadata.get("container_id").is_none());
    }

    #[tokio::test]
    async fn test_get_container_id_when_no_service() {
        let actor = RedisContainerActor::new();
        let container_id = actor.get_container_id().await;
        assert!(container_id.is_none());
    }

    #[tokio::test]
    async fn test_handle_stop_when_no_container() {
        let mut actor = RedisContainerActor::new();

        // Should not fail when stopping with no container
        let result = actor.handle(RedisContainerMessage::Stop).await;
        assert!(result.is_ok());
        assert!(!actor.is_paused); // Should reset pause state
    }

    #[tokio::test]
    async fn test_handle_get_connection_string_no_container() {
        let mut actor = RedisContainerActor::new();

        // Should not fail when getting connection string with no container
        let result = actor
            .handle(RedisContainerMessage::GetConnectionString { port: 7000 })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_status_no_container() {
        let mut actor = RedisContainerActor::new();

        // Should not fail when getting status with no container
        let result = actor.handle(RedisContainerMessage::Status).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_shutdown() {
        let mut actor = RedisContainerActor::new();

        // Should not fail when shutting down with no container
        let result = actor.shutdown().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_restart() {
        let mut actor = RedisContainerActor::new();

        // Should not fail when restarting with no container
        let result = actor.restart().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_pause_resume_no_container() {
        let mut actor = RedisContainerActor::new();

        // Test pause with no container
        actor.pause().await.unwrap();
        assert!(actor.is_paused);

        // Test resume with no container
        actor.resume().await.unwrap();
        assert!(!actor.is_paused);
    }

    #[tokio::test]
    async fn test_validation_docker_accessibility() {
        let actor = RedisContainerActor::new();

        // This may pass or fail depending on Docker availability
        let errors = actor.validate().await.unwrap();
        // Don't assert specific behavior since Docker may or may not be available in test env
        println!("Validation errors: {:?}", errors);
    }

    #[tokio::test]
    async fn test_allowed_messages_when_paused() {
        let mut actor = RedisContainerActor::new();

        // Pause the actor
        actor.pause().await.unwrap();
        assert!(actor.is_paused);

        // These messages should be allowed when paused
        assert!(actor.handle(RedisContainerMessage::Stop).await.is_ok());

        actor.pause().await.unwrap(); // Re-pause after stop resets pause state
        assert!(actor.handle(RedisContainerMessage::Resume).await.is_ok());

        actor.pause().await.unwrap(); // Re-pause
        assert!(actor.handle(RedisContainerMessage::Status).await.is_ok());
    }

    #[tokio::test]
    async fn test_message_debug_formatting() {
        // Test that messages can be formatted for debug
        let msg1 = RedisContainerMessage::Start(Box::new(Service {
            service_type: ServiceType::RedisCluster,
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

        let msg2 = RedisContainerMessage::Stop;
        let debug_str = format!("{:?}", msg2);
        assert!(debug_str.contains("Stop"));

        let msg3 = RedisContainerMessage::GetConnectionString { port: 7000 };
        let debug_str = format!("{:?}", msg3);
        assert!(debug_str.contains("GetConnectionString"));
        assert!(debug_str.contains("7000"));
    }

    #[tokio::test]
    async fn test_get_info_with_redis_ports() {
        let actor = RedisContainerActor::new();
        let info = actor.get_info().await;

        // When no service manager, no connection info should be present
        for port in [7000, 7001, 7002, 7003, 7004, 7005] {
            assert!(info.metadata.get(&format!("connection_{}", port)).is_none());
        }
    }

    #[tokio::test]
    async fn test_redis_container_actor() {
        let (tx, rx) = mpsc::channel::<RedisContainerMessage>(10);
        let actor = RedisContainerActor::new();

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

        tx.send(RedisContainerMessage::Start(Box::new(service)))
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        tx.send(RedisContainerMessage::GetConnectionString { port: 7000 })
            .await
            .unwrap();

        tx.send(RedisContainerMessage::Stop).await.unwrap();

        drop(tx);

        handle.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn test_redis_container_actor_docker_operations() {
        let (tx, rx) = mpsc::channel::<RedisContainerMessage>(10);
        let actor = RedisContainerActor::new();

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

        // Start the container
        tx.send(RedisContainerMessage::Start(Box::new(service)))
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Test Docker operations
        tx.send(RedisContainerMessage::Status).await.unwrap();

        tx.send(RedisContainerMessage::Pause).await.unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        tx.send(RedisContainerMessage::Status).await.unwrap();

        tx.send(RedisContainerMessage::Resume).await.unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        tx.send(RedisContainerMessage::Status).await.unwrap();

        tx.send(RedisContainerMessage::Stop).await.unwrap();

        drop(tx);

        handle.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn test_redis_container_actor_pause_resume() {
        let mut actor = RedisContainerActor::new();

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
            .handle(RedisContainerMessage::Start(Box::new(service)))
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
            .handle(RedisContainerMessage::Start(Box::new(service)))
            .await;
        assert!(result.is_ok());
        assert!(actor.service_manager.is_some()); // Should have started

        // Cleanup
        actor.handle(RedisContainerMessage::Stop).await.unwrap();
    }
}
