use crate::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Debug};

/// Actor status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActorStatus {
    Stopped,
    Starting,
    Running,
    Paused,
    Stopping,
    Error(String),
}

impl std::fmt::Display for ActorStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActorStatus::Stopped => write!(f, "Stopped"),
            ActorStatus::Starting => write!(f, "Starting"),
            ActorStatus::Running => write!(f, "Running"),
            ActorStatus::Paused => write!(f, "Paused"),
            ActorStatus::Stopping => write!(f, "Stopping"),
            ActorStatus::Error(err) => write!(f, "Error: {}", err),
        }
    }
}

/// Actor health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub healthy: bool,
    pub message: String,
    pub details: HashMap<String, String>,
}

impl HealthStatus {
    pub fn healthy(message: impl Into<String>) -> Self {
        Self {
            healthy: true,
            message: message.into(),
            details: HashMap::new(),
        }
    }

    pub fn unhealthy(message: impl Into<String>) -> Self {
        Self {
            healthy: false,
            message: message.into(),
            details: HashMap::new(),
        }
    }

    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }
}

/// Actor information structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorInfo {
    pub name: String,
    pub version: String,
    pub uptime_seconds: Option<u64>,
    pub messages_processed: Option<u64>,
    pub metadata: HashMap<String, String>,
}

impl ActorInfo {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: "1.0.0".to_string(),
            uptime_seconds: None,
            messages_processed: None,
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// A generic actor message trait (for ergonomics/debugging)
pub trait Message: Send + Debug + 'static {}

impl<T: Send + Debug + 'static> Message for T {}

/// An actor trait. Accepts messages of type M.
#[async_trait]
pub trait Actor<M>
where
    M: Message,
{
    // Core actor lifecycle methods
    async fn handle(&mut self, msg: M) -> Result<()>;
    async fn shutdown(&mut self) -> Result<()>;
    async fn pause(&mut self) -> Result<()>;
    async fn resume(&mut self) -> Result<()>;
    async fn restart(&mut self) -> Result<()>;

    // Enhanced actor introspection methods
    async fn status(&self) -> ActorStatus;
    async fn health_check(&self) -> HealthStatus;
    async fn get_info(&self) -> ActorInfo;

    // Optional methods with default implementations
    async fn reset(&mut self) -> Result<()> {
        // Default implementation does nothing - actors can override if they support reset
        Ok(())
    }

    async fn validate(&self) -> Result<Vec<String>> {
        // Default implementation returns no validation errors
        // Actors can override to provide validation logic
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::ServiceType;

    // Test actor for demonstrating the enhanced trait
    struct TestActor {
        message_count: u64,
        is_paused: bool,
        should_error: bool,
    }

    #[derive(Debug)]
    enum TestMessage {
        Process,
        _Pause,
        _Resume,
        ToggleError,
    }

    impl TestActor {
        fn new() -> Self {
            Self {
                message_count: 0,
                is_paused: false,
                should_error: false,
            }
        }
    }

    #[async_trait]
    impl Actor<TestMessage> for TestActor {
        async fn handle(&mut self, msg: TestMessage) -> Result<()> {
            if self.is_paused && !matches!(msg, TestMessage::_Resume) {
                return Ok(());
            }

            match msg {
                TestMessage::Process => {
                    self.message_count += 1;
                    if self.should_error {
                        return Err(crate::error::OzymandiasError::InvalidServiceImage(
                            ServiceType::Custom("test".to_string()),
                        )
                        .into());
                    }
                }
                TestMessage::_Pause => {
                    self.is_paused = true;
                }
                TestMessage::_Resume => {
                    self.is_paused = false;
                }
                TestMessage::ToggleError => {
                    self.should_error = !self.should_error;
                }
            }
            Ok(())
        }

        async fn shutdown(&mut self) -> Result<()> {
            Ok(())
        }

        async fn pause(&mut self) -> Result<()> {
            self.is_paused = true;
            Ok(())
        }

        async fn resume(&mut self) -> Result<()> {
            self.is_paused = false;
            Ok(())
        }

        async fn restart(&mut self) -> Result<()> {
            self.message_count = 0;
            self.is_paused = false;
            self.should_error = false;
            Ok(())
        }

        async fn status(&self) -> ActorStatus {
            if self.should_error {
                ActorStatus::Error("Test error mode enabled".to_string())
            } else if self.is_paused {
                ActorStatus::Paused
            } else {
                ActorStatus::Running
            }
        }

        async fn health_check(&self) -> HealthStatus {
            if self.should_error {
                HealthStatus::unhealthy("Actor is in error mode")
                    .with_detail("should_error", "true")
            } else {
                HealthStatus::healthy("Test actor is operational")
                    .with_detail("message_count", self.message_count.to_string())
                    .with_detail("paused", self.is_paused.to_string())
            }
        }

        async fn get_info(&self) -> ActorInfo {
            ActorInfo::new("TestActor")
                .with_metadata("type", "test_actor")
                .with_metadata("message_count", self.message_count.to_string())
                .with_metadata("paused", self.is_paused.to_string())
                .with_metadata("should_error", self.should_error.to_string())
        }

        async fn validate(&self) -> Result<Vec<String>> {
            let mut errors = Vec::new();

            if self.message_count > 100 {
                errors.push("Message count is very high, consider resetting".to_string());
            }

            if self.should_error && !self.is_paused {
                errors.push(
                    "Actor is in error mode but not paused, this may cause issues".to_string(),
                );
            }

            Ok(errors)
        }

        async fn reset(&mut self) -> Result<()> {
            self.message_count = 0;
            self.should_error = false;
            // Don't reset pause state
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_enhanced_actor_trait() {
        let mut actor = TestActor::new();

        // Test initial status
        assert_eq!(actor.status().await, ActorStatus::Running);

        // Test health check
        let health = actor.health_check().await;
        assert!(health.healthy);
        assert_eq!(health.message, "Test actor is operational");

        // Test info
        let info = actor.get_info().await;
        assert_eq!(info.name, "TestActor");
        assert_eq!(info.metadata.get("message_count"), Some(&"0".to_string()));

        // Test validation
        let errors = actor.validate().await.unwrap();
        assert!(errors.is_empty());

        // Process some messages
        actor.handle(TestMessage::Process).await.unwrap();
        actor.handle(TestMessage::Process).await.unwrap();

        // Check updated info
        let info = actor.get_info().await;
        assert_eq!(info.metadata.get("message_count"), Some(&"2".to_string()));

        // Test pause functionality
        actor.pause().await.unwrap();
        assert_eq!(actor.status().await, ActorStatus::Paused);

        // Test health check when paused
        let health = actor.health_check().await;
        assert!(health.healthy);
        assert_eq!(health.details.get("paused"), Some(&"true".to_string()));

        // Resume
        actor.resume().await.unwrap();
        assert_eq!(actor.status().await, ActorStatus::Running);

        // Test error mode
        actor.handle(TestMessage::ToggleError).await.unwrap();
        assert_eq!(
            actor.status().await,
            ActorStatus::Error("Test error mode enabled".to_string())
        );

        // Test health check when in error
        let health = actor.health_check().await;
        assert!(!health.healthy);

        // Test validation with error mode
        let errors = actor.validate().await.unwrap();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("error mode"));

        // Test reset (should clear counters but keep pause state)
        actor.reset().await.unwrap();
        assert_eq!(actor.status().await, ActorStatus::Running); // Should not be paused anymore since error mode was cleared
        assert_eq!(
            actor.get_info().await.metadata.get("message_count"),
            Some(&"0".to_string())
        );

        // Pause it and test restart (full reset)
        actor.pause().await.unwrap();
        assert_eq!(actor.status().await, ActorStatus::Paused);

        actor.restart().await.unwrap();
        assert_eq!(actor.status().await, ActorStatus::Running);
    }

    #[tokio::test]
    async fn test_actor_status_display() {
        assert_eq!(format!("{}", ActorStatus::Stopped), "Stopped");
        assert_eq!(format!("{}", ActorStatus::Starting), "Starting");
        assert_eq!(format!("{}", ActorStatus::Running), "Running");
        assert_eq!(format!("{}", ActorStatus::Paused), "Paused");
        assert_eq!(format!("{}", ActorStatus::Stopping), "Stopping");
        assert_eq!(
            format!("{}", ActorStatus::Error("test error".to_string())),
            "Error: test error"
        );
    }

    #[tokio::test]
    async fn test_health_status_builders() {
        let healthy = HealthStatus::healthy("All good");
        assert!(healthy.healthy);
        assert_eq!(healthy.message, "All good");
        assert!(healthy.details.is_empty());

        let unhealthy = HealthStatus::unhealthy("Something wrong");
        assert!(!unhealthy.healthy);
        assert_eq!(unhealthy.message, "Something wrong");
        assert!(unhealthy.details.is_empty());

        let with_details = HealthStatus::healthy("Good")
            .with_detail("key1", "value1")
            .with_detail("key2", "value2");
        assert_eq!(
            with_details.details.get("key1"),
            Some(&"value1".to_string())
        );
        assert_eq!(
            with_details.details.get("key2"),
            Some(&"value2".to_string())
        );
    }

    #[tokio::test]
    async fn test_actor_info_builder() {
        let info = ActorInfo::new("TestActor");
        assert_eq!(info.name, "TestActor");
        assert_eq!(info.version, "1.0.0");
        assert!(info.uptime_seconds.is_none());
        assert!(info.messages_processed.is_none());
        assert!(info.metadata.is_empty());

        let info_with_metadata = ActorInfo::new("TestActor")
            .with_metadata("type", "test")
            .with_metadata("status", "active");
        assert_eq!(
            info_with_metadata.metadata.get("type"),
            Some(&"test".to_string())
        );
        assert_eq!(
            info_with_metadata.metadata.get("status"),
            Some(&"active".to_string())
        );
    }

    #[tokio::test]
    async fn test_actor_status_equality() {
        assert_eq!(ActorStatus::Running, ActorStatus::Running);
        assert_eq!(ActorStatus::Stopped, ActorStatus::Stopped);
        assert_eq!(
            ActorStatus::Error("test".to_string()),
            ActorStatus::Error("test".to_string())
        );

        assert_ne!(ActorStatus::Running, ActorStatus::Stopped);
        assert_ne!(
            ActorStatus::Error("test1".to_string()),
            ActorStatus::Error("test2".to_string())
        );
    }

    #[tokio::test]
    async fn test_message_trait_implementation() {
        // Test that our TestMessage implements Message trait
        let _msg: Box<dyn Message> = Box::new(TestMessage::Process);

        // Test debug formatting
        let msg = TestMessage::Process;
        let debug_str = format!("{:?}", msg);
        assert!(debug_str.contains("Process"));
    }
}
