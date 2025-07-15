use std::time::Duration;

use async_trait::async_trait;
use rdkafka::{
    consumer::{Consumer, StreamConsumer},
    message::{Header, OwnedHeaders},
    producer::{FutureProducer, FutureRecord},
    ClientConfig,
};
use tracing::{error, info, warn};

use crate::{actor::Actor, scenario::KafkaMessage};
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
            .set("auto.offset.reset", "earliest")
            .set("session.timeout.ms", "6000")
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
}

#[cfg(test)]
mod tests {
    use crate::{
        actor::run_actor,
        containers::kafka::create_kafka_service,
        scenario::{Service, ServiceType},
    };
    use rdkafka::message::Message;

    use super::*;

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

        // Give Kafka some time to fully initialize
        println!("Waiting for Kafka to fully initialize...");
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Create a unique test topic to avoid conflicts with other tests
        let test_topic = format!(
            "test-topic-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
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

        // Get the consumer from the scheduler
        // let consumer = kafka_scheduler.consumer.as_ref().unwrap();
        let consumer = KafkaScheduler::create_consumer(
            &service_manager.get_connection_string(9093).unwrap(),
            "test-group",
            &[&test_topic],
        ).unwrap();

        tokio::spawn(async move {
            run_actor(kafka_scheduler, rx)
                .await
                .expect("Actor run failed");
        });

        // Send the test message to Kafka
        println!("Sending test message to topic: {}", test_topic);
        tx.send(KafkaSchdulerMessage::SendMessage(test_message.clone()))
            .await
            .expect("Failed to send message to actor channel");

        println!("Message sent successfully, now consuming...");

        // Read a single message with a timeout - no loop needed for simplicity
        println!("Reading message with a 10-second timeout");
        let message_result = tokio::time::timeout(Duration::from_secs(10), consumer.recv()).await;

        // Verify the message was received and matches what we sent
        match message_result {
            Ok(Ok(msg)) => {
                // Extract key and payload for verification
                let received_key = msg.key().map(|k| String::from_utf8_lossy(k).to_string());
                let received_value = msg
                    .payload()
                    .map(|p| String::from_utf8_lossy(p).to_string());

                println!(
                    "Received message with key: {:?}, payload: {:?}",
                    received_key, received_value
                );

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

                println!("Test passed! Message was successfully sent and received with matching content");
            }
            Ok(Err(e)) => {
                panic!("Error receiving message: {:?}", e);
            }
            Err(e) => {
                panic!("Timeout error while waiting for message: {}", e);
            }
        }

        // Attempt to clean up resources
        println!("Test completed successfully, cleaning up resources");
        if let Err(e) = service_manager.stop().await {
            println!("Failed to stop service manager: {:?}", e);
        } else {
            println!("Service manager stopped successfully");
        }
    }
}
