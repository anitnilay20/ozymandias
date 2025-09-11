use async_trait::async_trait;
use bollard::Docker;
use testcontainers_modules::kafka::Kafka;
use tracing::{error, info, warn};

use super::actor_trait::{Actor, ActorInfo, ActorStatus, HealthStatus};
use crate::{
    containers::{kafka::create_kafka_service, service::ServiceManager},
    error::Result,
    scenario::Service,
};

#[derive(Debug)]
pub enum KafkaContainerMessage {
    Start(Box<Service>),
    Stop,
    GetConnectionString { port: u16 },
    Pause,
    Resume,
    Status,
}

pub struct KafkaContainerActor {
    service_manager: Option<ServiceManager<Kafka>>,
    is_paused: bool,
    docker: Docker,
}

impl KafkaContainerActor {
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
impl Actor<KafkaContainerMessage> for KafkaContainerActor {
    async fn handle(&mut self, msg: KafkaContainerMessage) -> Result<()> {
        if self.is_paused {
            match msg {
                KafkaContainerMessage::Stop
                | KafkaContainerMessage::Resume
                | KafkaContainerMessage::Status => {
                    // Allow these messages even when paused
                }
                _ => {
                    info!(
                        "Kafka container actor is paused, ignoring message: {:?}",
                        msg
                    );
                    return Ok(());
                }
            }
        }

        match msg {
            KafkaContainerMessage::Start(service) => {
                info!("Starting Kafka container");
                let service_manager = create_kafka_service(*service).await?;
                self.service_manager = Some(service_manager);
                info!("Kafka container started successfully");
                Ok(())
            }
            KafkaContainerMessage::Stop => {
                info!("Stopping Kafka container");
                if let Some(service_manager) = self.service_manager.take() {
                    service_manager.stop().await?;
                    info!("Kafka container stopped successfully");
                } else {
                    info!("No Kafka container to stop");
                }
                self.is_paused = false; // Reset pause state when stopped
                Ok(())
            }
            KafkaContainerMessage::GetConnectionString { port } => {
                if let Some(ref service_manager) = self.service_manager {
                    if let Some(connection_string) = service_manager.get_connection_string(port) {
                        info!("Connection string for port {}: {}", port, connection_string);
                    } else {
                        error!("No connection string available for port {}", port);
                    }
                } else {
                    error!("No Kafka container running");
                }
                Ok(())
            }
            KafkaContainerMessage::Pause => {
                info!("Received pause message");
                self.pause().await
            }
            KafkaContainerMessage::Resume => {
                info!("Received resume message");
                self.resume().await
            }
            KafkaContainerMessage::Status => {
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
                    info!("No Kafka container running");
                }
                Ok(())
            }
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down Kafka container actor");
        if let Some(service_manager) = self.service_manager.take() {
            service_manager.stop().await?;
        }
        Ok(())
    }

    async fn pause(&mut self) -> Result<()> {
        info!("Pausing Kafka container using Docker API");

        if let Some(container_id) = self.get_container_id().await {
            match self.docker.pause_container(&container_id).await {
                Ok(_) => {
                    info!("Successfully paused Kafka container: {}", container_id);
                    self.is_paused = true;
                }
                Err(e) => {
                    error!("Failed to pause Kafka container {}: {}", container_id, e);
                    // Fallback to actor-level pause
                    warn!("Falling back to actor-level pause");
                    self.is_paused = true;
                }
            }
        } else {
            warn!("No Kafka container running to pause, using actor-level pause");
            self.is_paused = true;
        }

        Ok(())
    }

    async fn restart(&mut self) -> Result<()> {
        info!("Restarting Kafka container actor");
        Ok(())
    }

    async fn resume(&mut self) -> Result<()> {
        info!("Resuming Kafka container using Docker API");

        if let Some(container_id) = self.get_container_id().await {
            match self.docker.unpause_container(&container_id).await {
                Ok(_) => {
                    info!("Successfully resumed Kafka container: {}", container_id);
                    self.is_paused = false;
                }
                Err(e) => {
                    error!("Failed to resume Kafka container {}: {}", container_id, e);
                    // Fallback to actor-level resume
                    warn!("Falling back to actor-level resume");
                    self.is_paused = false;
                }
            }
        } else {
            warn!("No Kafka container running to resume, using actor-level resume");
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
                        HealthStatus::healthy("Kafka container is running normally")
                            .with_detail("container_id", &container_id)
                            .with_detail("paused", "false")
                    } else if paused {
                        HealthStatus::healthy("Kafka container is paused")
                            .with_detail("container_id", &container_id)
                            .with_detail("paused", "true")
                    } else {
                        HealthStatus::unhealthy("Kafka container is not running")
                            .with_detail("container_id", &container_id)
                            .with_detail("running", running.to_string())
                    }
                }
                Err(e) => HealthStatus::unhealthy(format!("Failed to inspect container: {}", e))
                    .with_detail("container_id", &container_id),
            }
        } else {
            HealthStatus::unhealthy("No Kafka container running")
        }
    }

    async fn get_info(&self) -> ActorInfo {
        let mut info = ActorInfo::new("KafkaContainerActor")
            .with_metadata("type", "container_actor")
            .with_metadata("service", "kafka");

        if let Some(container_id) = self.get_container_id().await {
            info = info.with_metadata("container_id", &container_id);
        }

        if let Some(ref service_manager) = self.service_manager {
            if let Some(conn_9092) = service_manager.get_connection_string(9092) {
                info = info.with_metadata("connection_9092", &conn_9092);
            }
            if let Some(conn_9093) = service_manager.get_connection_string(9093) {
                info = info.with_metadata("connection_9093", &conn_9093);
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

impl Default for KafkaContainerActor {
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
    async fn test_kafka_container_actor() {
        let (tx, rx) = mpsc::channel::<KafkaContainerMessage>(10);
        let actor = KafkaContainerActor::new();

        let handle = tokio::spawn(async move { run_actor(actor, rx).await });

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

        tx.send(KafkaContainerMessage::Start(Box::new(service)))
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        tx.send(KafkaContainerMessage::GetConnectionString { port: 9092 })
            .await
            .unwrap();

        tx.send(KafkaContainerMessage::Stop).await.unwrap();

        drop(tx);

        handle.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn test_kafka_container_actor_docker_operations() {
        let (tx, rx) = mpsc::channel::<KafkaContainerMessage>(10);
        let actor = KafkaContainerActor::new();

        let handle = tokio::spawn(async move { run_actor(actor, rx).await });

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

        // Start the container
        tx.send(KafkaContainerMessage::Start(Box::new(service)))
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Test Docker operations
        tx.send(KafkaContainerMessage::Status).await.unwrap();

        tx.send(KafkaContainerMessage::Pause).await.unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        tx.send(KafkaContainerMessage::Status).await.unwrap();

        tx.send(KafkaContainerMessage::Resume).await.unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        tx.send(KafkaContainerMessage::Status).await.unwrap();

        tx.send(KafkaContainerMessage::Stop).await.unwrap();

        drop(tx);

        handle.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn test_kafka_container_actor_pause_resume() {
        let mut actor = KafkaContainerActor::new();

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

        // This should be ignored due to pause
        let result = actor
            .handle(KafkaContainerMessage::Start(Box::new(service)))
            .await;
        assert!(result.is_ok());
        assert!(actor.service_manager.is_none()); // Should not have started

        // Resume and try again
        actor.resume().await.unwrap();
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

        // This should work after resume
        let result = actor
            .handle(KafkaContainerMessage::Start(Box::new(service)))
            .await;
        assert!(result.is_ok());
        assert!(actor.service_manager.is_some()); // Should have started

        // Cleanup
        actor.handle(KafkaContainerMessage::Stop).await.unwrap();
    }
}
