// use testcontainers::{
//     core::{IntoContainerPort, WaitFor},
//     runners::AsyncRunner,
//     GenericImage,
// };

// #[tokio::test]
// async fn test_redis() {
//     let _container = GenericImage::new("redis", "7.2.4")
//         .with_exposed_port(6379.tcp())
//         .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
//         .start()
//         .await
//         .unwrap();
// }
