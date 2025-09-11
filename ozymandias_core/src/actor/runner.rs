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
    use crate::actor::actor_trait::{ActorInfo, ActorStatus, HealthStatus};
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
}
