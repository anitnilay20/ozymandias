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
        .map_err(|e| OzymandiasError::FileError(e))?;
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

            [services.redis_cluster]
            image = "grokzen/redis-cluster:6.0.7"
            ports = [7000, 7001, 7002, 7003, 7004, 7005]
            wait_for_log = "Ready to accept connections"
            alias = "redis"
        "#;

        let result = super::load_scenario_from_toml(toml).await;
        println!("{:?}", result);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_load_toml_file() {
        let result = super::load_scenario_from_toml_file("../scenarios/1.toml").await;
        println!("{:?}", result);
        assert!(result.is_ok());
    }
}
