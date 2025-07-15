use serde::Deserialize;
use std::{collections::HashMap, fmt::Display};

#[derive(Debug, Deserialize)]
pub struct Scenario {
    pub meta: Meta,
    #[serde(default = "Vec::new")]
    pub services: Vec<Service>,
    pub mocks: Option<MockServer>,
    pub events: Option<Events>,
    pub failures: Option<Vec<Failure>>,
    pub assertions: Option<Assertions>,
    pub hooks: Option<Hooks>,
}

#[derive(Debug, Deserialize)]
pub struct Meta {
    pub name: String,
    pub description: String,
    pub timeout_seconds: Option<u64>,
    pub labels: Option<Vec<String>>,
}

// ========================
// Services
// ========================
#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub enum ServiceType {
    #[serde(rename = "redis_cluster")]
    RedisCluster,
    #[serde(rename = "kafka")]
    Kafka,
    Custom(String),
}

impl Display for ServiceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceType::RedisCluster => write!(f, "redis_cluster"),
            ServiceType::Kafka => write!(f, "kafka"),
            ServiceType::Custom(name) => write!(f, "{}", name),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Service {
    pub service_type: ServiceType,
    pub image: String,
    pub tag: Option<String>,
    pub container_name: Option<String>,
    pub ports: Vec<u16>,
    pub wait_for_log: Option<String>,
    pub alias: Option<String>,
    pub env: Vec<(String, String)>,
    pub retry_config: Option<RetryConfig>,
}

#[derive(Debug, Deserialize)]
pub struct MockServer {
    pub port: u16,
    pub delay_startup: Option<u64>,
    pub routes: Vec<MockRoute>,
    // pub exposed_env_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MockRoute {
    pub method: String,
    pub path: String,
    pub delay_ms: Option<u64>,
    pub response: HttpResponse,
}

#[derive(Debug, Deserialize)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,

    #[serde(default = "Vec::new")]
    pub headers: Vec<(String, String)>,
    pub mime_type: Option<String>,
}

// ========================
// Kafka Events
// ========================

#[derive(Debug, Deserialize)]
pub struct Events {
    pub kafka: Option<Vec<KafkaEvent>>,
}

#[derive(Debug, Deserialize)]
pub struct KafkaEvent {
    pub send_after_seconds: u64,
    pub messages: Vec<KafkaMessage>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct KafkaMessage {
    pub topic: String,
    pub key: String,
    pub value: String,
    #[serde(default = "Vec::new")]
    pub headers: Vec<(String, String)>,
}

// ========================
// Failures
// ========================

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum Failure {
    #[serde(rename = "container_down")]
    ContainerDown {
        target: String,
        at_seconds: u64,
        restart_after_seconds: Option<u64>,
    },
    #[serde(rename = "delay_mock")]
    DelayMock {
        target: String,
        endpoint: String,
        delay_ms: u64,
    },
    #[serde(rename = "network_glitch")]
    NetworkGlitch {
        target: String,
        at_seconds: u64,
        duration_seconds: u64,
    },
}

// ========================
// Assertions
// ========================

#[derive(Debug, Deserialize)]
pub struct Assertions {
    pub redis_keys: Option<RedisKeyAssertions>,
    pub ccs_call_count: Option<HashMap<String, u32>>,
    pub kafka: Option<KafkaAssertions>,
}

#[derive(Debug, Deserialize)]
pub struct RedisKeyAssertions {
    pub expected_keys: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct KafkaAssertions {
    pub topic_state: Option<KafkaTopicState>,
}

#[derive(Debug, Deserialize)]
pub struct KafkaTopicState {
    pub topic: String,
    pub expected_message_count: u32,
}

// ========================
// Hooks
// ========================

#[derive(Debug, Deserialize)]
pub struct Hooks {
    pub before_start: Option<HookCommands>,
    pub after_finish: Option<HookCommands>,
}

#[derive(Debug, Deserialize)]
pub struct HookCommands {
    pub commands: Vec<String>,
}
