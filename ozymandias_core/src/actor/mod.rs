pub mod kafka_scheduler;

use crate::error::Result;
use async_trait::async_trait;
use std::fmt::Debug;
use tokio::sync::mpsc::Receiver;

/// A generic actor message trait (for ergonomics/debugging)
pub trait Message: Send + Debug + 'static {}

impl<T: Send + Debug + 'static> Message for T {}

/// An actor trait. Accepts messages of type M.
#[async_trait]
pub trait Actor<M>
where
    M: Message,
{
    async fn handle(&mut self, msg: M) -> Result<()>;
}

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
