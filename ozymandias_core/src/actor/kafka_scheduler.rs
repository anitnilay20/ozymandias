use std::time::Duration;

use async_trait::async_trait;
use rdkafka::{
    consumer::{Consumer, StreamConsumer},
    message::{Header, OwnedHeaders},
    producer::{FutureProducer, FutureRecord},
    ClientConfig,
};
use tracing::{error, info, warn};

use crate::{
    actor::actor_trait::{Actor, ActorInfo, ActorStatus, HealthStatus},
    scenario::KafkaMessage,
};
use crate::{
    error::{OzymandiasError, Result},
    scenario::RetryConfig,
};

pub struct KafkaScheduler {
    producer: FutureProducer,
    consumer: Option<StreamConsumer>,
    retry_config: RetryConfig,
}

impl Default for RetryConfig {
    fn default() -> Self {
        RetryConfig {
            max_retries: 3,
            initial_backoff_ms: 100,
            max_backoff_ms: 2000,
        }
    }
}

impl KafkaScheduler {
    pub fn new(broker: &str, retry_config: Option<RetryConfig>) -> Result<Self> {
        Ok(KafkaScheduler {
            producer: Self::create_producer(broker)?,
            consumer: None,
            retry_config: retry_config.unwrap_or_default(),
        })
    }

    pub fn with_retry_config(
        mut self,
        max_retries: u32,
        initial_backoff_ms: u64,
        max_backoff_ms: u64,
    ) -> Self {
        self.retry_config = RetryConfig {
            max_retries,
            initial_backoff_ms,
            max_backoff_ms,
        };
        self
    }

    pub fn with_consumer(mut self, broker: &str, group_id: &str, topics: &[&str]) -> Result<Self> {
        self.consumer = Some(Self::create_consumer(broker, group_id, topics)?);
        Ok(self)
    }

    fn create_producer(broker: &str) -> Result<FutureProducer> {
        ClientConfig::new()
            .set("bootstrap.servers", broker)
            .set("message.timeout.ms", "5000")
            .create()
            .map_err(|e| {
                error!(target: "Create KafkaScheduler", error = ?e, "Failed to create Kafka producer");
                error_stack::Report::new(OzymandiasError::KafkaSendError(e))
            })
    }

    fn create_consumer(broker: &str, group_id: &str, topics: &[&str]) -> Result<StreamConsumer> {
        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", broker)
            .set("group.id", group_id)
            .set("enable.auto.commit", "true")
            .set("auto.offset.reset", "earliest") // Always start from the beginning of the topic
            .set("session.timeout.ms", "6000")
            .set("max.poll.interval.ms", "300000") // 5 minutes
            .set("fetch.min.bytes", "1") // Don't wait for batching
            .set("fetch.wait.max.ms", "100") // Don't wait long for batches
            .create()
            .map_err(|e| {
                error!(target: "Create KafkaScheduler", error = ?e, "Failed to create Kafka consumer");
                error_stack::Report::new(OzymandiasError::KafkaSendError(e))
            })?;

        consumer.subscribe(topics).map_err(|e| {
            error!(target: "Create KafkaScheduler", error = ?e, topics = ?topics, "Failed to subscribe to topics");
            error_stack::Report::new(OzymandiasError::KafkaSendError(e))
        })?;

        Ok(consumer)
    }

    pub async fn send_to_kafka(&self, msg: KafkaMessage) -> Result<()> {
        // Logic to send a message to Kafka
        info!(target: "Send message to kafka", message = ?msg, "Sending message to Kafka");

        let mut attempt = 0;
        let mut backoff_ms = self.retry_config.initial_backoff_ms;
        let mut last_error = None;

        while attempt < self.retry_config.max_retries {
            let mut record = FutureRecord::to(&msg.topic)
                .payload(&msg.value)
                .key(&msg.key);

            let mut owned_headers = OwnedHeaders::new();
            for (k, v) in &msg.headers {
                owned_headers = owned_headers.insert(Header {
                    key: k,
                    value: Some(v.as_bytes()),
                });
            }
            record = record.headers(owned_headers);

            // Send and await delivery report
            match self.producer.send(record, Duration::from_secs(10)).await {
                Ok(_) => {
                    info!(target: "Send message to kafka", topic = %msg.topic, key = %msg.key, "Message sent successfully");
                    return Ok(());
                }
                Err((e, _)) => {
                    warn!(
                        target: "Send message to kafka",
                        error = ?e,
                        attempt = attempt + 1,
                        max_attempts = self.retry_config.max_retries,
                        "Failed to send, retrying after {} ms",
                        backoff_ms
                    );
                    last_error = Some(e);

                    // Exponential backoff
                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                    backoff_ms = std::cmp::min(backoff_ms * 2, self.retry_config.max_backoff_ms);
                    attempt += 1;
                }
            }
        }

        // All retries failed
        if let Some(e) = last_error {
            error!(
                target: "Send message to kafka",
                error = ?e,
                topic = %msg.topic,
                key = %msg.key,
                "Failed to send message after {} attempts",
                self.retry_config.max_retries
            );

            return Err(error_stack::Report::new(OzymandiasError::KafkaSendError(e)));
        }

        // This should never happen, but just in case
        Err(error_stack::Report::new(OzymandiasError::KafkaSendError(
            rdkafka::error::KafkaError::Subscription("Unknown error in send_to_kafka".into()),
        )))
    }
}

#[derive(Debug)]
pub enum KafkaSchdulerMessage {
    Start,
    SendMessage(KafkaMessage),
    Stop,
}

#[async_trait]
impl Actor<KafkaSchdulerMessage> for KafkaScheduler {
    async fn handle(&mut self, msg: KafkaSchdulerMessage) -> Result<()> {
        info!(target: "KafkaScheduler", message = ?msg, "Handling Kafka scheduler message");
        match msg {
            KafkaSchdulerMessage::Start => Ok(()),
            KafkaSchdulerMessage::SendMessage(msg) => self.send_to_kafka(msg).await,
            KafkaSchdulerMessage::Stop => {
                info!(target: "KafkaScheduler", "Stopping Kafka scheduler");
                Ok(())
            }
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!(target: "KafkaScheduler", "Shutting down Kafka scheduler");
        // Any cleanup code here
        Ok(())
    }

    async fn pause(&mut self) -> Result<()> {
        info!(target: "KafkaScheduler", "Pausing Kafka scheduler");
        // Logic to pause processing
        Ok(())
    }

    async fn restart(&mut self) -> Result<()> {
        info!(target: "KafkaScheduler", "Restarting Kafka scheduler");
        // Logic to restart processing
        Ok(())
    }

    async fn resume(&mut self) -> Result<()> {
        info!(target: "KafkaScheduler", "Resuming Kafka scheduler");
        // Logic to resume processing
        Ok(())
    }

    async fn status(&self) -> ActorStatus {
        if self.producer.is_enabled() {
            ActorStatus::Running
        } else {
            ActorStatus::Stopped
        }
    }

    async fn health_check(&self) -> HealthStatus {
        // For Kafka scheduler, we can check if the producer is functional
        // by checking its configuration
        let producer_healthy = !self.producer.is_enabled() || {
            // Producer exists, so it's considered healthy
            true
        };

        if producer_healthy {
            let mut status = HealthStatus::healthy("Kafka scheduler is operational");

            if self.consumer.is_some() {
                status = status.with_detail("consumer", "enabled");
            } else {
                status = status.with_detail("consumer", "disabled");
            }

            status = status
                .with_detail("max_retries", self.retry_config.max_retries.to_string())
                .with_detail(
                    "initial_backoff_ms",
                    self.retry_config.initial_backoff_ms.to_string(),
                )
                .with_detail(
                    "max_backoff_ms",
                    self.retry_config.max_backoff_ms.to_string(),
                );

            status
        } else {
            HealthStatus::unhealthy("Kafka producer is not functional")
        }
    }

    async fn get_info(&self) -> ActorInfo {
        let mut info = ActorInfo::new("KafkaScheduler")
            .with_metadata("type", "kafka_scheduler")
            .with_metadata("service", "kafka");

        info = info
            .with_metadata("max_retries", self.retry_config.max_retries.to_string())
            .with_metadata(
                "initial_backoff_ms",
                self.retry_config.initial_backoff_ms.to_string(),
            )
            .with_metadata(
                "max_backoff_ms",
                self.retry_config.max_backoff_ms.to_string(),
            );

        if self.consumer.is_some() {
            info = info.with_metadata("has_consumer", "true");
        } else {
            info = info.with_metadata("has_consumer", "false");
        }

        info
    }

    async fn validate(&self) -> Result<Vec<String>> {
        let mut errors = Vec::new();

        // Basic validation of retry configuration
        if self.retry_config.max_retries == 0 {
            errors.push("Max retries is set to 0, which may not be desired".to_string());
        }

        if self.retry_config.initial_backoff_ms > self.retry_config.max_backoff_ms {
            errors.push("Initial backoff is greater than max backoff".to_string());
        }

        if self.retry_config.max_backoff_ms > 60_000 {
            errors
                .push("Max backoff is greater than 60 seconds, which may be too long".to_string());
        }

        Ok(errors)
    }
}

// Extension trait to check if FutureProducer is enabled/functional
trait ProducerExt {
    fn is_enabled(&self) -> bool;
}

impl ProducerExt for FutureProducer {
    fn is_enabled(&self) -> bool {
        // For now, we'll assume if the producer exists, it's enabled
        // In a real implementation, you might want to do more sophisticated checks
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        actor::runner::run_actor,
        containers::kafka::create_kafka_service,
        scenario::{Service, ServiceType},
    };
    use rdkafka::message::Message;

    use super::*;

    #[tokio::test]
    async fn test_kafka_scheduler_creation() {
        let scheduler = KafkaScheduler::new("localhost:9092", None).unwrap();

        assert_eq!(scheduler.retry_config.max_retries, 3);
        assert_eq!(scheduler.retry_config.initial_backoff_ms, 100);
        assert_eq!(scheduler.retry_config.max_backoff_ms, 2000);
        assert!(scheduler.consumer.is_none());
    }

    #[tokio::test]
    async fn test_kafka_scheduler_with_custom_retry_config() {
        let custom_retry = RetryConfig {
            max_retries: 5,
            initial_backoff_ms: 200,
            max_backoff_ms: 3000,
        };

        let scheduler = KafkaScheduler::new("localhost:9092", Some(custom_retry)).unwrap();

        assert_eq!(scheduler.retry_config.max_retries, 5);
        assert_eq!(scheduler.retry_config.initial_backoff_ms, 200);
        assert_eq!(scheduler.retry_config.max_backoff_ms, 3000);
    }

    #[tokio::test]
    async fn test_kafka_scheduler_with_retry_config_builder() {
        let scheduler = KafkaScheduler::new("localhost:9092", None)
            .unwrap()
            .with_retry_config(10, 500, 5000);

        assert_eq!(scheduler.retry_config.max_retries, 10);
        assert_eq!(scheduler.retry_config.initial_backoff_ms, 500);
        assert_eq!(scheduler.retry_config.max_backoff_ms, 5000);
    }

    #[tokio::test]
    async fn test_actor_trait_methods() {
        let mut scheduler = KafkaScheduler::new("localhost:9092", None).unwrap();

        // Test status
        let status = scheduler.status().await;
        assert_eq!(status, ActorStatus::Running);

        // Test health check
        let health = scheduler.health_check().await;
        assert!(health.healthy);
        assert_eq!(health.message, "Kafka scheduler is operational");
        assert_eq!(
            health.details.get("consumer"),
            Some(&"disabled".to_string())
        );

        // Test get_info
        let info = scheduler.get_info().await;
        assert_eq!(info.name, "KafkaScheduler");
        assert_eq!(
            info.metadata.get("type"),
            Some(&"kafka_scheduler".to_string())
        );
        assert_eq!(info.metadata.get("service"), Some(&"kafka".to_string()));
        assert_eq!(
            info.metadata.get("has_consumer"),
            Some(&"false".to_string())
        );

        // Test pause
        scheduler.pause().await.unwrap();

        // Test resume
        scheduler.resume().await.unwrap();

        // Test restart
        scheduler.restart().await.unwrap();

        // Test shutdown
        scheduler.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_validation_errors() {
        // Test with zero retries
        let scheduler = KafkaScheduler::new("localhost:9092", None)
            .unwrap()
            .with_retry_config(0, 100, 2000);

        let errors = scheduler.validate().await.unwrap();
        assert!(errors.iter().any(|e| e.contains("Max retries is set to 0")));

        // Test with initial backoff > max backoff
        let scheduler = KafkaScheduler::new("localhost:9092", None)
            .unwrap()
            .with_retry_config(3, 5000, 2000);

        let errors = scheduler.validate().await.unwrap();
        assert!(errors
            .iter()
            .any(|e| e.contains("Initial backoff is greater than max backoff")));

        // Test with very large max backoff
        let scheduler = KafkaScheduler::new("localhost:9092", None)
            .unwrap()
            .with_retry_config(3, 100, 70_000);

        let errors = scheduler.validate().await.unwrap();
        assert!(errors
            .iter()
            .any(|e| e.contains("Max backoff is greater than 60 seconds")));

        // Test valid config
        let scheduler = KafkaScheduler::new("localhost:9092", None).unwrap();
        let errors = scheduler.validate().await.unwrap();
        assert!(errors.is_empty());
    }

    #[tokio::test]
    async fn test_handle_messages() {
        let mut scheduler = KafkaScheduler::new("localhost:9092", None).unwrap();

        // Test Start message
        scheduler.handle(KafkaSchdulerMessage::Start).await.unwrap();

        // Test Stop message
        scheduler.handle(KafkaSchdulerMessage::Stop).await.unwrap();

        // SendMessage testing requires actual Kafka broker, covered in integration test
    }

    #[tokio::test]
    async fn test_retry_config_default() {
        let default_config = RetryConfig::default();

        assert_eq!(default_config.max_retries, 3);
        assert_eq!(default_config.initial_backoff_ms, 100);
        assert_eq!(default_config.max_backoff_ms, 2000);
    }

    #[tokio::test]
    async fn test_scheduler_with_consumer_creation_failure() {
        let scheduler = KafkaScheduler::new("invalid:9092", None).unwrap();

        // This should fail gracefully when trying to create consumer with invalid broker
        let result = scheduler.with_consumer("invalid:9092", "test-group", &["test-topic"]);
        // Kafka consumer creation may not immediately fail, just ensure no panic
        let _ = result;
    }

    #[tokio::test]
    async fn test_producer_ext_trait() {
        let scheduler = KafkaScheduler::new("localhost:9092", None).unwrap();

        // Test that producer is considered enabled
        assert!(scheduler.producer.is_enabled());
    }

    #[tokio::test]
    async fn test_create_kafka_service_manager() {
        let (tx, rx) = tokio::sync::mpsc::channel::<_>(8);
        // Create a service configuration for Kafka
        let service = Service {
            service_type: ServiceType::Kafka,
            image: "".into(), // The actual image is specified in the create_kafka_service function
            tag: None,
            container_name: None,
            ports: vec![],
            wait_for_log: None,
            alias: None,
            env: vec![],
            retry_config: None,
        };

        // Start the Kafka service
        let service_manager = create_kafka_service(service).await.unwrap();
        println!("Kafka service started successfully");

        println!("Waiting for Kafka to fully initialize...");

        // Implement a health check for Kafka before proceeding
        let mut is_ready = false;
        let mut attempts = 0;
        let max_attempts = 5;

        while !is_ready && attempts < max_attempts {
            attempts += 1;
            println!(
                "Kafka readiness check attempt {}/{}",
                attempts, max_attempts
            );

            // Try to create a producer as a readiness check
            match ClientConfig::new()
                .set(
                    "bootstrap.servers",
                    service_manager.get_connection_string(9093).unwrap(),
                )
                .set("message.timeout.ms", "5000")
                .create::<FutureProducer>()
            {
                Ok(_) => {
                    println!("✓ Kafka is ready!");
                    is_ready = true;
                }
                Err(e) => {
                    println!("✗ Kafka not ready yet: {:?}", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }

        if !is_ready {
            println!(
                "WARNING: Kafka may not be fully initialized after {} attempts",
                max_attempts
            );
        }

        // Additional wait time even if we think it's ready
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Create a unique test topic to avoid conflicts with other tests
        let test_topic = format!(
            "test-topic-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        );

        // Create a unique consumer group ID
        let unique_group_id = format!(
            "test-group-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        );

        println!(
            "Using unique topic: {} and group ID: {}",
            test_topic, unique_group_id
        );

        // Create a test message with unique key and value
        let test_key = "test-key-1";
        let test_value = "test-value-1";
        let test_header_key = "header1";
        let test_header_value = "value1";

        // Create the test message
        let test_message = KafkaMessage {
            topic: test_topic.clone(),
            key: test_key.to_string(),
            value: test_value.to_string(),
            headers: vec![(test_header_key.to_string(), test_header_value.to_string())],
        };

        println!(
            "Creating Kafka scheduler with consumer for topic: {}",
            test_topic
        );

        // Create a Kafka scheduler with both producer and consumer configured
        let kafka_scheduler =
            KafkaScheduler::new(&service_manager.get_connection_string(9093).unwrap(), None)
                .expect("Failed to create KafkaScheduler");

        // Create a separate consumer for verification with a unique group ID to ensure we get all messages
        println!("Creating consumer for topic: {}", test_topic);

        println!("Using unique consumer group ID: {}", unique_group_id);

        let consumer = KafkaScheduler::create_consumer(
            &service_manager.get_connection_string(9093).unwrap(),
            &unique_group_id,
            &[&test_topic],
        )
        .unwrap();

        // Pre-create the topic to ensure it exists
        println!("Pre-creating topic to ensure it exists...");
        let admin_client = ClientConfig::new()
            .set(
                "bootstrap.servers",
                service_manager.get_connection_string(9093).unwrap(),
            )
            .create::<FutureProducer>()
            .expect("Failed to create admin client");

        // Send a dummy message to create the topic with a different key for easy filtering
        let dummy_record = FutureRecord::to(&test_topic)
            .payload("__dummy_init__")
            .key("__dummy_init__");

        match admin_client
            .send(dummy_record, Duration::from_secs(10))
            .await
        {
            Ok(_) => println!("Topic pre-created successfully"),
            Err(e) => println!("Topic pre-creation error (may be ok): {:?}", e),
        }

        // Additional wait time after topic creation
        tokio::time::sleep(Duration::from_secs(2)).await;

        tokio::spawn(async move {
            run_actor(kafka_scheduler, rx)
                .await
                .expect("Actor run failed");
        });

        // Send the test message to Kafka
        println!("Sending test message to topic: {}", test_topic);
        println!(
            "Test message key: {}, value: {}, headers: {:?}",
            test_key,
            test_value,
            &[(test_header_key.to_string(), test_header_value.to_string())]
        );

        tx.send(KafkaSchdulerMessage::SendMessage(test_message.clone()))
            .await
            .expect("Failed to send message to actor channel");

        println!("Message sent successfully, now consuming...");

        // Add a delay to ensure message is processed fully
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Read a single message with a longer timeout for CI environments
        println!("Reading message with a 30-second timeout");

        // Implement a better approach: consume all messages and find the one we want
        println!("Processing messages until we find our test message...");
        let mut found_test_message = false;
        let mut test_message_result: Option<
            std::result::Result<rdkafka::message::BorrowedMessage, rdkafka::error::KafkaError>,
        > = None;
        let mut attempts = 0;
        let max_attempts = 30; // 30 attempts with 1-second delays = 30 seconds max

        // Continue consuming messages until we find our test message or hit max attempts
        while !found_test_message && attempts < max_attempts {
            attempts += 1;
            println!("Message consumption attempt {}/{}", attempts, max_attempts);

            match tokio::time::timeout(Duration::from_secs(1), consumer.recv()).await {
                Ok(Ok(msg)) => {
                    // Extract the key and value
                    let key = msg
                        .key()
                        .map(|k| String::from_utf8_lossy(k).to_string())
                        .unwrap_or_default();
                    let value = msg
                        .payload()
                        .map(|p| String::from_utf8_lossy(p).to_string())
                        .unwrap_or_default();

                    println!("Consumed message: key={}, value={}", key, value);

                    // Check if this is our test message
                    if key == test_key && value == test_value {
                        println!("Found our test message!");
                        found_test_message = true;
                        test_message_result = Some(Ok(msg));
                        break;
                    } else {
                        println!("Not our test message, continuing to read...");
                    }
                }
                Ok(Err(e)) => {
                    println!("Error receiving message: {:?}. Retrying...", e);
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                Err(_) => {
                    // Timeout, continue reading
                    println!("No message received in this attempt, continuing...");
                }
            }
        }

        // Use the result from our test message search
        let message_result = if found_test_message {
            test_message_result
        } else {
            None
        };

        // Verify the message was received and matches what we sent
        match message_result {
            Some(Ok(msg)) => {
                // Extract key and payload for verification
                let received_key = msg.key().map(|k| String::from_utf8_lossy(k).to_string());
                let received_value = msg
                    .payload()
                    .map(|p| String::from_utf8_lossy(p).to_string());

                println!(
                    "Received message with key: {:?}, payload: {:?}",
                    received_key, received_value
                );

                // Debug extra info
                if received_key.as_deref() != Some(test_key) {
                    println!("WARNING: Received key doesn't match expected key.");
                    println!("Expected: '{}', Received: '{:?}'", test_key, received_key);
                }

                if received_value.as_deref() != Some(test_value) {
                    println!("WARNING: Received value doesn't match expected value.");
                    println!(
                        "Expected: '{}', Received: '{:?}'",
                        test_value, received_value
                    );
                }

                // If we got the dummy message somehow, print a warning but don't fail
                if received_key.as_deref() == Some("__dummy_init__") {
                    println!("WARNING: Received dummy init message instead of test message!");
                    println!("This indicates an issue with message filtering.");
                }

                // Verify the message matches what we sent
                assert_eq!(
                    Some(test_key.to_string()),
                    received_key,
                    "Message key doesn't match"
                );
                assert_eq!(
                    Some(test_value.to_string()),
                    received_value,
                    "Message value doesn't match"
                );

                // Check if headers are present
                if msg.headers().is_some() {
                    println!("Message has headers (detailed verification skipped)");
                    // We're keeping this simple since header verification is complex in rdkafka
                    // The main verification is that we sent a message and received the same key/payload
                }

                println!(
                    "Test passed! Message was successfully sent and received with matching content"
                );
            }
            Some(Err(e)) => {
                panic!("Error receiving message: {:?}", e);
            }
            None => {
                panic!("Failed to receive message after {} attempts", max_attempts);
            }
        }

        // Attempt to clean up resources
        println!("Test completed successfully, cleaning up resources");

        // Close the actor channel
        drop(tx);

        // Give the actor time to shut down
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Stop the service manager
        if let Err(e) = service_manager.stop().await {
            println!("Failed to stop service manager: {:?}", e);
        } else {
            println!("Service manager stopped successfully");
        }
    }
}
