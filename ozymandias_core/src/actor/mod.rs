/*!
# Actor System

A robust, production-ready actor system for managing containerized services and messaging.

## Core Components

### [`actor_trait`] - Core Actor Trait and Types
- `Actor<M>` - Main actor trait with lifecycle and introspection methods
- `ActorStatus` - Status enumeration (Running, Paused, Stopped, Error, etc.)
- `HealthStatus` - Rich health check results with details
- `ActorInfo` - Actor metadata and configuration information
- `Message` - Generic message trait for type safety

### [`runner`] - Actor Execution
- `run_actor()` - Simple actor runner function
- `ActorRunner` - Enhanced runner with lifecycle management, logging, and health monitoring

## Actor Implementations

- [`kafka`] - Kafka container actor with Docker pause/unpause support
- [`kafka_scheduler`] - Kafka message scheduling and retry logic
- [`redis`] - Redis cluster container actor with Docker control
- [`mock_server`] - WireMock HTTP server actor with mock management
- [`service`] - Generic container service actor

## Usage Example

```rust,no_run
use ozymandias_core::actor::{Actor, ActorRunner, run_actor};
use ozymandias_core::actor::kafka::KafkaContainerActor;

# #[tokio::main]
# async fn main() -> ozymandias_core::error::Result<()> {
// Simple usage
let (tx, rx) = tokio::sync::mpsc::channel(10);
let actor = KafkaContainerActor::new();
tokio::spawn(run_actor(actor, rx));

// Enhanced usage with lifecycle management
let (tx2, rx2) = tokio::sync::mpsc::channel(10);
let actor2 = KafkaContainerActor::new();
let runner = ActorRunner::new(actor2, rx2);
tokio::spawn(runner.run());

// Actor introspection
let actor3 = KafkaContainerActor::new();
let status = actor3.status().await;
let health = actor3.health_check().await;
let info = actor3.get_info().await;
let warnings = actor3.validate().await?;
# Ok(())
# }
```
*/

// Actor trait and related types
pub mod actor_trait;
pub mod runner;

// Actor implementations
pub mod kafka;
pub mod kafka_scheduler;
pub mod mock_server;
pub mod redis;
pub mod service;

// Re-export commonly used types for convenience
pub use actor_trait::{Actor, ActorInfo, ActorStatus, HealthStatus, Message};
pub use runner::{run_actor, ActorRunner};
