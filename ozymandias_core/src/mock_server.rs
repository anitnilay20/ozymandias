use std::time::Duration;
use tokio::time;
use tracing::{debug, info};
use wiremock::{
    matchers::{method, path},
    Mock, MockServer as WireMockServer, ResponseTemplate,
};

use crate::scenario::{MockRoute, MockServer};

pub async fn start_mock_server(mock_server: MockServer) -> WireMockServer {
    info!(target: "Starting mock server", port = ?mock_server.port);

    if let Some(delay) = mock_server.delay_startup {
        info!(target: "Starting mock server", delay = ?delay, "Delaying startup");
        time::sleep(Duration::from_secs(delay)).await;
    }

    let listener = std::net::TcpListener::bind(format!("127.0.0.1:{}", mock_server.port)).unwrap();
    let server_builder = WireMockServer::builder().listener(listener);

    let server = server_builder.start().await;

    generate_routes(&mock_server.routes, &server).await;

    info!(target: "Mock server started", uri = ?server.uri());
    debug!(target: "Mock server object", object = ?server, input = ?mock_server);

    server
}

pub async fn generate_routes(routes: &Vec<MockRoute>, server: &WireMockServer) {
    info!(target: "Generating mock routes", routes = ?routes);
    for route in routes {
        info!(target: "Generating mock route for", route = ?route);

        let mut response = ResponseTemplate::new(route.response.status)
            .set_delay(Duration::from_millis(route.delay_ms.unwrap_or(0)));

        if let Some(mime_type) = route.response.mime_type.as_ref() {
            response = response.set_body_raw(route.response.body.as_str(), mime_type);
        } else {
            response = response.set_body_raw(route.response.body.as_str(), "application/json");
        }

        let headers = route.response.headers.clone();

        response = response.append_headers(headers.clone());

        info!(target: "Generating mock routes", headers = ?headers);

        Mock::given(method(route.method.to_uppercase().as_str()))
            .and(path(route.path.as_str()))
            .respond_with(response)
            .mount(server)
            .await;

        debug!(target: "Generated mock routes", server = ?server, route = ?route);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::scenario::{HttpResponse, MockServer};

    #[tokio::test]
    async fn test_start_mock_server() {
        let mock_server = MockServer {
            port: 8080,
            delay_startup: Some(0), // 1 second delay
            routes: Vec::new(),
        };

        let server = start_mock_server(mock_server).await;
        assert!(server.uri() == "http://127.0.0.1:8080");
    }

    #[tokio::test]
    async fn test_generate_routes() {
        let mock_server = MockServer {
            port: 8081,
            delay_startup: None,
            routes: vec![MockRoute {
                method: "GET".to_string(),
                path: "/test".to_string(),
                delay_ms: Some(0),
                response: HttpResponse {
                    status: 200,
                    body: r#"{"message": "Hello, World!"}"#.to_string(),
                    headers: vec![],
                    mime_type: None,
                },
            }],
        };
        let server = start_mock_server(mock_server).await;

        let response = reqwest::get(format!("{}/test", server.uri()))
            .await
            .unwrap();

        assert_eq!(response.status().as_u16(), 200);

        assert_eq!(
            response.json::<HashMap<String, String>>().await.unwrap(),
            HashMap::from([("message".to_string(), "Hello, World!".to_string())])
        );
    }

    #[tokio::test]
    async fn test_generate_routes_with_mime_type() {
        let mock_server = MockServer {
            port: 8082,
            delay_startup: None,
            routes: vec![MockRoute {
                method: "GET".to_string(),
                path: "/test".to_string(),
                delay_ms: Some(0),
                response: HttpResponse {
                    status: 202,
                    body: r#"Hello world"#.to_string(),
                    headers: vec![("user-id".to_string(), "1".to_string())],
                    mime_type: Some("text/plain".to_string()),
                },
            }],
        };
        let server = start_mock_server(mock_server).await;

        let response = reqwest::get(format!("{}/test", server.uri()))
            .await
            .unwrap();

        assert_eq!(response.status().as_u16(), 202);

        assert_eq!(response.headers().get("user-id").unwrap(), "1");

        assert_eq!(response.text().await.unwrap(), "Hello world");
    }
}
