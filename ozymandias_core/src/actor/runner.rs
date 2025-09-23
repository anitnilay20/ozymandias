use crate::error::Result;
use tokio::sync::mpsc::Receiver;
use tracing;

use super::actor_trait::{Actor, Message};

/// A runner that receives messages and invokes the actor
pub async fn run_actor<A, M>(mut actor: A, mut rx: Receiver<M>) -> Result<()>
where
    A: Actor<M> + Send + 'static,
    M: Message,
{
    while let Some(msg) = rx.recv().await {
        actor.handle(msg).await?;
    }
    Ok(())
}

/// Enhanced actor runner with lifecycle management
pub struct ActorRunner<A, M>
where
    A: Actor<M> + Send + Sync + 'static,
    M: Message,
{
    actor: A,
    rx: Receiver<M>,
}

impl<A, M> ActorRunner<A, M>
where
    A: Actor<M> + Send + Sync + 'static,
    M: Message,
{
    pub fn new(actor: A, rx: Receiver<M>) -> Self {
        Self { actor, rx }
    }

    /// Run the actor with enhanced lifecycle management
    pub async fn run(mut self) -> Result<()> {
        // Log initial actor info
        let info = self.actor.get_info().await;
        tracing::info!("Starting actor: {} v{}", info.name, info.version);

        // Validate actor before starting
        match self.actor.validate().await {
            Ok(warnings) => {
                if !warnings.is_empty() {
                    for warning in warnings {
                        tracing::warn!("Actor validation warning: {}", warning);
                    }
                }
            }
            Err(e) => {
                tracing::error!("Actor validation failed: {}", e);
                return Err(e);
            }
        }

        // Check initial health
        let health = self.actor.health_check().await;
        if !health.healthy {
            tracing::warn!("Actor starting with unhealthy status: {}", health.message);
        }

        // Main message processing loop
        let mut message_count = 0u64;
        while let Some(msg) = self.rx.recv().await {
            message_count += 1;

            if let Err(e) = self.actor.handle(msg).await {
                tracing::error!(
                    "Actor message handling failed (message #{}): {}",
                    message_count,
                    e
                );
                return Err(e);
            }

            // Periodic health checks (every 100 messages)
            if message_count % 100 == 0 {
                let health = self.actor.health_check().await;
                if !health.healthy {
                    tracing::warn!(
                        "Actor health degraded after {} messages: {}",
                        message_count,
                        health.message
                    );
                }
            }
        }

        // Graceful shutdown
        tracing::info!("Actor shutting down gracefully");
        self.actor.shutdown().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        actor::actor_trait::{ActorInfo, ActorStatus, HealthStatus},
        error::OzymandiasError,
        scenario::ServiceType,
    };
    use tokio::sync::mpsc;

    struct SimpleTestActor {
        counter: u32,
    }

    #[derive(Debug)]
    enum SimpleMessage {
        Increment,
        _Stop,
    }

    impl SimpleTestActor {
        fn new() -> Self {
            Self { counter: 0 }
        }
    }

    #[async_trait::async_trait]
    impl Actor<SimpleMessage> for SimpleTestActor {
        async fn handle(&mut self, msg: SimpleMessage) -> Result<()> {
            match msg {
                SimpleMessage::Increment => {
                    self.counter += 1;
                }
                SimpleMessage::_Stop => {
                    // Stop is handled by dropping the sender
                }
            }
            Ok(())
        }

        async fn shutdown(&mut self) -> Result<()> {
            Ok(())
        }

        async fn pause(&mut self) -> Result<()> {
            Ok(())
        }

        async fn resume(&mut self) -> Result<()> {
            Ok(())
        }

        async fn restart(&mut self) -> Result<()> {
            self.counter = 0;
            Ok(())
        }

        async fn status(&self) -> ActorStatus {
            ActorStatus::Running
        }

        async fn health_check(&self) -> HealthStatus {
            HealthStatus::healthy("Simple test actor is running")
                .with_detail("counter", self.counter.to_string())
        }

        async fn get_info(&self) -> ActorInfo {
            ActorInfo::new("SimpleTestActor").with_metadata("counter", self.counter.to_string())
        }
    }

    #[tokio::test]
    async fn test_simple_run_actor() {
        let (tx, rx) = mpsc::channel(10);
        let actor = SimpleTestActor::new();

        let handle = tokio::spawn(async move { run_actor(actor, rx).await });

        // Send some messages
        tx.send(SimpleMessage::Increment).await.unwrap();
        tx.send(SimpleMessage::Increment).await.unwrap();
        tx.send(SimpleMessage::Increment).await.unwrap();

        // Close channel to stop actor
        drop(tx);

        // Wait for actor to finish
        handle.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn test_enhanced_actor_runner() {
        let (tx, rx) = mpsc::channel(10);
        let actor = SimpleTestActor::new();

        let runner = ActorRunner::new(actor, rx);
        let handle = tokio::spawn(async move { runner.run().await });

        // Send some messages
        tx.send(SimpleMessage::Increment).await.unwrap();
        tx.send(SimpleMessage::Increment).await.unwrap();

        // Close channel to stop actor
        drop(tx);

        // Wait for actor to finish
        handle.await.unwrap().unwrap();
    }

    // Additional test actor that can simulate various conditions
    struct TestActorWithConditions {
        counter: u32,
        should_fail: bool,
        validation_errors: Vec<String>,
        is_healthy: bool,
    }

    #[derive(Debug)]
    enum TestMessage {
        Increment,
        ToggleHealth,
        ToggleFailure,
        AddValidationError(String),
        Reset,
    }

    impl TestActorWithConditions {
        fn new() -> Self {
            Self {
                counter: 0,
                should_fail: false,
                validation_errors: Vec::new(),
                is_healthy: true,
            }
        }

        fn with_validation_errors(mut self, errors: Vec<String>) -> Self {
            self.validation_errors = errors;
            self
        }

        fn unhealthy(mut self) -> Self {
            self.is_healthy = false;
            self
        }
    }

    #[async_trait::async_trait]
    impl Actor<TestMessage> for TestActorWithConditions {
        async fn handle(&mut self, msg: TestMessage) -> Result<()> {
            if self.should_fail {
                return Err(error_stack::Report::new(
                    OzymandiasError::InvalidServiceImage(ServiceType::Custom(
                        "test_failure".to_string(),
                    )),
                ));
            }

            match msg {
                TestMessage::Increment => {
                    self.counter += 1;
                }
                TestMessage::ToggleHealth => {
                    self.is_healthy = !self.is_healthy;
                }
                TestMessage::ToggleFailure => {
                    self.should_fail = !self.should_fail;
                }
                TestMessage::AddValidationError(error) => {
                    self.validation_errors.push(error);
                }
                TestMessage::Reset => {
                    self.counter = 0;
                    self.should_fail = false;
                    self.validation_errors.clear();
                    self.is_healthy = true;
                }
            }
            Ok(())
        }

        async fn shutdown(&mut self) -> Result<()> {
            Ok(())
        }

        async fn pause(&mut self) -> Result<()> {
            Ok(())
        }

        async fn resume(&mut self) -> Result<()> {
            Ok(())
        }

        async fn restart(&mut self) -> Result<()> {
            self.counter = 0;
            Ok(())
        }

        async fn status(&self) -> ActorStatus {
            if self.should_fail {
                ActorStatus::Error("Actor configured to fail".to_string())
            } else {
                ActorStatus::Running
            }
        }

        async fn health_check(&self) -> HealthStatus {
            if self.is_healthy {
                HealthStatus::healthy("Test actor is healthy")
                    .with_detail("counter", self.counter.to_string())
            } else {
                HealthStatus::unhealthy("Test actor is unhealthy")
                    .with_detail("counter", self.counter.to_string())
            }
        }

        async fn get_info(&self) -> ActorInfo {
            ActorInfo::new("TestActorWithConditions")
                .with_metadata("counter", self.counter.to_string())
                .with_metadata("should_fail", self.should_fail.to_string())
                .with_metadata("is_healthy", self.is_healthy.to_string())
        }

        async fn validate(&self) -> Result<Vec<String>> {
            Ok(self.validation_errors.clone())
        }
    }

    #[tokio::test]
    async fn test_actor_runner_with_validation_warnings() {
        let (tx, rx) = mpsc::channel(10);
        let actor = TestActorWithConditions::new()
            .with_validation_errors(vec!["Warning 1".to_string(), "Warning 2".to_string()]);

        let runner = ActorRunner::new(actor, rx);
        let handle = tokio::spawn(async move { runner.run().await });

        // Send a message
        tx.send(TestMessage::Increment).await.unwrap();

        // Close channel to stop actor
        drop(tx);

        // Should complete successfully despite warnings
        handle.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn test_actor_runner_with_initial_unhealthy_status() {
        let (tx, rx) = mpsc::channel(10);
        let actor = TestActorWithConditions::new().unhealthy();

        let runner = ActorRunner::new(actor, rx);
        let handle = tokio::spawn(async move { runner.run().await });

        // Send a message
        tx.send(TestMessage::Increment).await.unwrap();

        // Close channel to stop actor
        drop(tx);

        // Should complete successfully despite being initially unhealthy
        handle.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn test_actor_runner_message_handling_failure() {
        let (tx, rx) = mpsc::channel(10);
        let actor = TestActorWithConditions::new();

        let runner = ActorRunner::new(actor, rx);
        let handle = tokio::spawn(async move { runner.run().await });

        // Send a message to toggle failure, then try to increment
        tx.send(TestMessage::ToggleFailure).await.unwrap();
        tx.send(TestMessage::Increment).await.unwrap(); // This should fail

        drop(tx);

        // Should return an error
        let result = handle.await.unwrap();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_actor_runner_periodic_health_checks() {
        let (tx, rx) = mpsc::channel(200); // Larger buffer for 100+ messages
        let actor = TestActorWithConditions::new();

        let runner = ActorRunner::new(actor, rx);
        let handle = tokio::spawn(async move { runner.run().await });

        // Send 101 messages to trigger periodic health check
        for _ in 0..50 {
            tx.send(TestMessage::Increment).await.unwrap();
        }

        // Toggle health to unhealthy
        tx.send(TestMessage::ToggleHealth).await.unwrap();

        // Send 50 more messages to trigger health check with unhealthy status
        for _ in 0..50 {
            tx.send(TestMessage::Increment).await.unwrap();
        }

        drop(tx);

        // Should complete successfully
        handle.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn test_actor_runner_new() {
        let (tx, rx) = mpsc::channel(10);
        let actor = SimpleTestActor::new();

        let runner = ActorRunner::new(actor, rx);

        // Just test that we can create the runner
        drop(tx);
        runner.run().await.unwrap();
    }

    #[tokio::test]
    async fn test_run_actor_with_empty_channel() {
        let (tx, rx) = mpsc::channel(10);
        let actor = SimpleTestActor::new();

        drop(tx); // Close channel immediately

        let result = run_actor(actor, rx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_enhanced_runner_with_empty_channel() {
        let (tx, rx) = mpsc::channel(10);
        let actor = TestActorWithConditions::new();

        drop(tx); // Close channel immediately

        let runner = ActorRunner::new(actor, rx);
        let result = runner.run().await;
        assert!(result.is_ok());
    }
}
