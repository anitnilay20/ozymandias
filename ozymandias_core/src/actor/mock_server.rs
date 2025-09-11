use async_trait::async_trait;
use tracing::{error, info, warn};
use wiremock::MockServer as WireMockServer;

use super::actor_trait::{Actor, ActorInfo, ActorStatus, HealthStatus};
use crate::{error::Result, mock_server::start_mock_server, scenario::MockServer};

#[derive(Debug)]
pub enum MockServerMessage {
    Start(Box<MockServer>),
    Stop,
    GetUri,
    Pause,
    Resume,
    Status,
    Reset,
}

pub struct MockServerActor {
    mock_server: Option<WireMockServer>,
    is_paused: bool,
    original_config: Option<MockServer>,
}

impl MockServerActor {
    pub fn new() -> Self {
        Self {
            mock_server: None,
            is_paused: false,
            original_config: None,
        }
    }

    fn get_server_status(&self) -> String {
        match (&self.mock_server, self.is_paused) {
            (Some(_), false) => "Running".to_string(),
            (Some(_), true) => "Paused".to_string(),
            (None, _) => "Stopped".to_string(),
        }
    }
}

#[async_trait]
impl Actor<MockServerMessage> for MockServerActor {
    async fn handle(&mut self, msg: MockServerMessage) -> Result<()> {
        if self.is_paused {
            match msg {
                MockServerMessage::Stop | MockServerMessage::Resume | MockServerMessage::Status => {
                    // Allow these messages even when paused
                }
                _ => {
                    info!("Mock server actor is paused, ignoring message: {:?}", msg);
                    return Ok(());
                }
            }
        }

        match msg {
            MockServerMessage::Start(mock_server_config) => {
                info!("Starting mock server on port {}", mock_server_config.port);
                let config_clone = mock_server_config.as_ref().clone();
                let mock_server = start_mock_server(*mock_server_config).await;
                info!("Mock server started at: {}", mock_server.uri());
                self.mock_server = Some(mock_server);
                self.original_config = Some(config_clone);
                Ok(())
            }
            MockServerMessage::Stop => {
                info!("Stopping mock server");
                if let Some(_mock_server) = self.mock_server.take() {
                    // WireMock server is automatically dropped and stopped
                    info!("Mock server stopped successfully");
                } else {
                    info!("No mock server to stop");
                }
                self.is_paused = false; // Reset pause state when stopped
                self.original_config = None; // Clear original config
                Ok(())
            }
            MockServerMessage::GetUri => {
                if let Some(ref mock_server) = self.mock_server {
                    info!("Mock server URI: {}", mock_server.uri());
                } else {
                    error!("No mock server running");
                }
                Ok(())
            }
            MockServerMessage::Pause => {
                info!("Received pause message for mock server");
                self.pause().await
            }
            MockServerMessage::Resume => {
                info!("Received resume message for mock server");
                self.resume().await
            }
            MockServerMessage::Status => {
                let status = self.get_server_status();
                let uri = self
                    .mock_server
                    .as_ref()
                    .map(|s| s.uri())
                    .unwrap_or_else(|| "N/A".to_string());
                info!("Mock server status: {}, URI: {}", status, uri);
                Ok(())
            }
            MockServerMessage::Reset => {
                info!("Resetting mock server");
                if let Some(ref mock_server) = self.mock_server {
                    // Reset all mocks on the server
                    mock_server.reset().await;
                    info!("Mock server reset successfully");
                } else {
                    warn!("No mock server running to reset");
                }
                Ok(())
            }
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down mock server actor");
        if let Some(_mock_server) = self.mock_server.take() {
            // WireMock server is automatically dropped and stopped
            info!("Mock server shut down");
        }
        Ok(())
    }

    async fn pause(&mut self) -> Result<()> {
        info!("Pausing mock server");

        if let Some(ref mock_server) = self.mock_server {
            // For WireMock, we simulate pause by resetting all mocks
            // This effectively makes the server respond with 404s to all requests
            mock_server.reset().await;
            info!("Mock server paused (all mocks cleared)");
            self.is_paused = true;
        } else {
            warn!("No mock server running to pause");
            self.is_paused = true;
        }

        Ok(())
    }

    async fn restart(&mut self) -> Result<()> {
        info!("Restarting mock server actor");
        Ok(())
    }

    async fn resume(&mut self) -> Result<()> {
        info!("Resuming mock server");

        if let Some(ref mock_server) = self.mock_server {
            if let Some(ref config) = self.original_config {
                // Re-create all the original mocks
                let routes = &config.routes;
                crate::mock_server::generate_routes(routes, mock_server).await;
                info!("Mock server resumed (original mocks restored)");
                self.is_paused = false;
            } else {
                warn!("No original configuration available to restore mocks");
                self.is_paused = false;
            }
        } else {
            warn!("No mock server running to resume");
            self.is_paused = false;
        }

        Ok(())
    }

    async fn status(&self) -> ActorStatus {
        match (&self.mock_server, self.is_paused) {
            (Some(_), false) => ActorStatus::Running,
            (Some(_), true) => ActorStatus::Paused,
            (None, _) => ActorStatus::Stopped,
        }
    }

    async fn health_check(&self) -> HealthStatus {
        if let Some(ref mock_server) = self.mock_server {
            // Try to make a simple request to verify the server is responsive
            let uri = mock_server.uri();
            match reqwest::get(&format!("{}/health-check-non-existent-endpoint", uri)).await {
                Ok(response) => {
                    // We expect a 404 for a non-existent endpoint, which means the server is responding
                    if response.status().is_client_error() {
                        HealthStatus::healthy("Mock server is responding to requests")
                            .with_detail("uri", &uri)
                            .with_detail("paused", self.is_paused.to_string())
                    } else {
                        HealthStatus::healthy("Mock server is responding")
                            .with_detail("uri", &uri)
                            .with_detail("paused", self.is_paused.to_string())
                            .with_detail("unexpected_response", response.status().to_string())
                    }
                }
                Err(e) => HealthStatus::unhealthy(format!("Mock server not responding: {}", e))
                    .with_detail("uri", &uri),
            }
        } else {
            HealthStatus::unhealthy("No mock server running")
        }
    }

    async fn get_info(&self) -> ActorInfo {
        let mut info = ActorInfo::new("MockServerActor")
            .with_metadata("type", "mock_server_actor")
            .with_metadata("service", "wiremock");

        if let Some(ref mock_server) = self.mock_server {
            info = info.with_metadata("uri", mock_server.uri());
        }

        if let Some(ref config) = self.original_config {
            info = info
                .with_metadata("port", config.port.to_string())
                .with_metadata("routes_count", config.routes.len().to_string());

            if let Some(delay) = config.delay_startup {
                info = info.with_metadata("delay_startup", delay.to_string());
            }
        }

        info
    }

    async fn validate(&self) -> Result<Vec<String>> {
        let mut errors = Vec::new();

        // If we have a server, validate it's accessible
        if let Some(ref mock_server) = self.mock_server {
            let uri = mock_server.uri();
            match reqwest::Client::new()
                .get(format!("{}/validation-test", uri))
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await
            {
                Ok(_) => {
                    // Any response (including 404) means the server is accessible
                }
                Err(e) => {
                    errors.push(format!("Mock server at {} is not accessible: {}", uri, e));
                }
            }
        }

        // Validate configuration consistency
        if self.is_paused && self.original_config.is_none() {
            errors.push("Actor is paused but no original configuration is stored".to_string());
        }

        Ok(errors)
    }

    // Override reset to provide mock server specific reset functionality
    async fn reset(&mut self) -> Result<()> {
        if let Some(ref mock_server) = self.mock_server {
            mock_server.reset().await;
            info!("Mock server reset - all mocks cleared");
            // Don't change pause state during reset
        } else {
            warn!("No mock server to reset");
        }
        Ok(())
    }
}

impl Default for MockServerActor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use super::*;
    use crate::{
        actor::runner::run_actor,
        scenario::{HttpResponse, MockRoute},
    };

    #[tokio::test]
    async fn test_mock_server_actor() {
        let (tx, rx) = mpsc::channel::<MockServerMessage>(10);
        let actor = MockServerActor::new();

        let handle = tokio::spawn(async move { run_actor(actor, rx).await });

        let mock_server_config = MockServer {
            port: 3001,
            delay_startup: None,
            routes: vec![MockRoute {
                method: "GET".to_string(),
                path: "/test".to_string(),
                delay_ms: Some(0),
                response: HttpResponse {
                    status: 200,
                    body: r#"{"message": "Hello from actor!"}"#.to_string(),
                    headers: vec![],
                    mime_type: Some("application/json".to_string()),
                },
            }],
        };

        tx.send(MockServerMessage::Start(Box::new(mock_server_config)))
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        tx.send(MockServerMessage::GetUri).await.unwrap();

        // Test the mock server endpoint
        let response = reqwest::get("http://127.0.0.1:3001/test").await.unwrap();
        assert_eq!(response.status().as_u16(), 200);
        let body = response.text().await.unwrap();
        assert!(body.contains("Hello from actor!"));

        tx.send(MockServerMessage::Stop).await.unwrap();

        drop(tx);

        handle.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn test_mock_server_actor_enhanced_operations() {
        let (tx, rx) = mpsc::channel::<MockServerMessage>(10);
        let actor = MockServerActor::new();

        let handle = tokio::spawn(async move { run_actor(actor, rx).await });

        let mock_server_config = MockServer {
            port: 3004,
            delay_startup: None,
            routes: vec![
                MockRoute {
                    method: "GET".to_string(),
                    path: "/test".to_string(),
                    delay_ms: Some(0),
                    response: HttpResponse {
                        status: 200,
                        body: r#"{"message": "Hello from enhanced test!"}"#.to_string(),
                        headers: vec![],
                        mime_type: Some("application/json".to_string()),
                    },
                },
                MockRoute {
                    method: "POST".to_string(),
                    path: "/data".to_string(),
                    delay_ms: Some(0),
                    response: HttpResponse {
                        status: 201,
                        body: r#"{"created": true}"#.to_string(),
                        headers: vec![],
                        mime_type: Some("application/json".to_string()),
                    },
                },
            ],
        };

        // Start the server
        tx.send(MockServerMessage::Start(Box::new(mock_server_config)))
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // Test initial status
        tx.send(MockServerMessage::Status).await.unwrap();

        // Test the endpoints work
        let response = reqwest::get("http://127.0.0.1:3004/test").await.unwrap();
        assert_eq!(response.status().as_u16(), 200);

        let client = reqwest::Client::new();
        let response = client
            .post("http://127.0.0.1:3004/data")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 201);

        // Pause the server (clears all mocks)
        tx.send(MockServerMessage::Pause).await.unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // Test status shows paused
        tx.send(MockServerMessage::Status).await.unwrap();

        // Test endpoints should now return 404
        let response = reqwest::get("http://127.0.0.1:3004/test").await.unwrap();
        assert_eq!(response.status().as_u16(), 404);

        // Resume the server (restores all mocks)
        tx.send(MockServerMessage::Resume).await.unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // Test status shows running
        tx.send(MockServerMessage::Status).await.unwrap();

        // Test endpoints work again
        let response = reqwest::get("http://127.0.0.1:3004/test").await.unwrap();
        assert_eq!(response.status().as_u16(), 200);

        // Test reset functionality
        tx.send(MockServerMessage::Reset).await.unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // After reset, endpoints should return 404 again
        let response = reqwest::get("http://127.0.0.1:3004/test").await.unwrap();
        assert_eq!(response.status().as_u16(), 404);

        tx.send(MockServerMessage::Stop).await.unwrap();

        drop(tx);

        handle.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn test_mock_server_actor_pause_resume() {
        let mut actor = MockServerActor::new();

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

        let mock_server_config = MockServer {
            port: 3002,
            delay_startup: None,
            routes: vec![],
        };

        // This should be ignored due to pause
        let result = actor
            .handle(MockServerMessage::Start(Box::new(mock_server_config)))
            .await;
        assert!(result.is_ok());
        assert!(actor.mock_server.is_none()); // Should not have started

        // Resume and try again
        actor.resume().await.unwrap();
        let mock_server_config = MockServer {
            port: 3003,
            delay_startup: None,
            routes: vec![],
        };

        // This should work after resume
        let result = actor
            .handle(MockServerMessage::Start(Box::new(mock_server_config)))
            .await;
        assert!(result.is_ok());
        assert!(actor.mock_server.is_some()); // Should have started

        // Cleanup
        actor.handle(MockServerMessage::Stop).await.unwrap();
    }
}
