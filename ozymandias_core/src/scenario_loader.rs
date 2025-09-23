use crate::error::{OzymandiasError, Result};
use crate::scenario::Scenario;
use tokio::fs;

pub async fn load_scenario_from_toml(toml: &str) -> Result<Scenario> {
    let scenario: Scenario = toml::de::from_str(toml)
        .map_err(|e| OzymandiasError::TomlDeserializationError(e.to_string()))?;
    Ok(scenario)
}

pub async fn load_scenario_from_toml_file(file_path: &str) -> Result<Scenario> {
    let toml_content = fs::read_to_string(file_path)
        .await
        .map_err(OzymandiasError::FileError)?;
    let scenario: Scenario = load_scenario_from_toml(&toml_content).await?;
    Ok(scenario)
}

#[cfg(test)]
mod test {
    #[tokio::test]
    async fn test_load_toml() {
        let toml = r#"
            [meta]
            name = "redis_cluster_ccs_kafka"
            description = "Test config-poller with all major integrations and simulate failures"
            timeout_seconds = 60
            labels = ["ci", "redis", "kafka", "ccs", "resilience"]

            [[services]]
            service_type = "redis_cluster"
            image = "grokzen/redis-cluster:6.0.7"
            ports = [7000, 7001, 7002, 7003, 7004, 7005]
            wait_for_log = "Ready to accept connections"
            alias = "redis"
            env = []
        "#;

        let result = super::load_scenario_from_toml(toml).await;
        println!("{:?}", result);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_load_toml_file() {
        // Create a temporary file with valid TOML content
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_scenario.toml");

        let toml_content = r#"
[meta]
name = "redis_cluster_ccs_kafka"
description = "Test config-poller with all major integrations and simulate failures"
timeout_seconds = 60
labels = ["ci", "redis", "kafka", "ccs", "resilience"]

[[services]]
service_type = "redis_cluster"
image = "grokzen/redis-cluster:6.0.7"
ports = [7000, 7001, 7002, 7003, 7004, 7005]
wait_for_log = "Ready to accept connections"
alias = "redis"
env = []
        "#;

        std::fs::write(&temp_file, toml_content).expect("Failed to write test file");

        let result = super::load_scenario_from_toml_file(temp_file.to_str().unwrap()).await;
        println!("{:?}", result);
        assert!(result.is_ok());

        // Clean up
        let _ = std::fs::remove_file(temp_file);
    }
}
