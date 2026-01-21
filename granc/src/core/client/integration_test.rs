use crate::core::client::GrpcClient;
use crate::core::reflection::DescriptorRegistry;
use echo_service::EchoServiceServer;
use echo_service::FILE_DESCRIPTOR_SET;
use echo_service_impl::EchoServiceImpl;
use tokio_stream::StreamExt;

mod echo_service_impl;

#[tokio::test]
async fn test_unary() {
    let registry = DescriptorRegistry::from_bytes(FILE_DESCRIPTOR_SET).unwrap();

    let method = registry
        .get_method_descriptor("echo.EchoService", "UnaryEcho")
        .unwrap();

    let client = GrpcClient {
        service: EchoServiceServer::new(EchoServiceImpl),
    };

    let payload = serde_json::json!({ "message": "hello" });

    let res = client
        .unary(method, payload, vec![])
        .await
        .unwrap()
        .unwrap();

    assert_eq!(res["message"], "hello");
}

#[tokio::test]
async fn test_server_streaming() {
    let registry = DescriptorRegistry::from_bytes(FILE_DESCRIPTOR_SET).unwrap();

    let method = registry
        .get_method_descriptor("echo.EchoService", "ServerStreamingEcho")
        .unwrap();

    let client = GrpcClient {
        service: EchoServiceServer::new(EchoServiceImpl),
    };

    let payload = serde_json::json!({ "message": "stream" });

    let stream = client
        .server_streaming(method, payload, vec![])
        .await
        .unwrap()
        .unwrap();

    let results: Vec<_> = stream.map(|r| r.unwrap()).collect().await;

    assert_eq!(results.len(), 3);
    assert_eq!(results[0]["message"], "stream - seq 0");
    assert_eq!(results[1]["message"], "stream - seq 1");
    assert_eq!(results[2]["message"], "stream - seq 2");
}

#[tokio::test]
async fn test_client_streaming() {
    let registry = DescriptorRegistry::from_bytes(FILE_DESCRIPTOR_SET).unwrap();
    let method = registry
        .get_method_descriptor("echo.EchoService", "ClientStreamingEcho")
        .unwrap();

    let client = GrpcClient {
        service: EchoServiceServer::new(EchoServiceImpl),
    };

    let payload = serde_json::json!([
        { "message": "A" },
        { "message": "B" },
        { "message": "C" }
    ]);

    let stream_source = tokio_stream::iter(payload.as_array().unwrap().clone());

    let res = client
        .client_streaming(method, stream_source, vec![])
        .await
        .unwrap()
        .unwrap();

    assert_eq!(res["message"], "ABC");
}

#[tokio::test]
async fn test_bidirectional_streaming() {
    let registry = DescriptorRegistry::from_bytes(FILE_DESCRIPTOR_SET).unwrap();
    let method = registry
        .get_method_descriptor("echo.EchoService", "BidirectionalEcho")
        .unwrap();

    let client = GrpcClient {
        service: EchoServiceServer::new(EchoServiceImpl),
    };

    let payload = serde_json::json!([
        { "message": "Ping" },
        { "message": "Pong" }
    ]);

    let stream_source = tokio_stream::iter(payload.as_array().unwrap().clone());

    let response_stream = client
        .bidirectional_streaming(method, stream_source, vec![])
        .await
        .unwrap()
        .unwrap();

    let results: Vec<_> = response_stream.map(|r| r.unwrap()).collect().await;

    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["message"], "echo: Ping");
    assert_eq!(results[1]["message"], "echo: Pong");
}
