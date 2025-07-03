use testcontainers::{ContainerAsync, GenericImage};

use super::service::start_service;
use crate::error::Result;
use crate::scenario::Service;

pub async fn create_redis_cluster_container(
    service: Service,
) -> Result<ContainerAsync<GenericImage>> {
    start_service(service).await
}

#[cfg(test)]
mod tests {
    use testcontainers::Image;

    use crate::scenario::ServiceType;

    use super::*;

    #[tokio::test]
    async fn test_create_redis_cluster_container() {
        let service = Service {
            service_type: ServiceType::RedisCluster,
            image: "grokzen/redis-cluster".to_string(),
            tag: Some("6.0.7".to_string()),
            container_name: None,
            ports: vec![7000, 7001, 7002, 7003, 7004, 7005],
            wait_for_log: Some("Ready to accept connections".to_string()),
            alias: None,
            env: None,
        };
        let container = create_redis_cluster_container(service).await;
        assert!(
            container.is_ok(),
            "Failed to create Redis cluster container: {:?}",
            container
        );
        let container = container.unwrap();
        // Since ContainerAsync does not have is_running, we just check that the container exists.
        assert!(
            !container.id().is_empty(),
            "Redis cluster container has an empty ID, likely not running"
        );
        assert_eq!(
            container.image().name(),
            "grokzen/redis-cluster",
            "Unexpected image name"
        );
        assert_eq!(container.image().tag(), "6.0.7", "Unexpected image tag");
    }
}
