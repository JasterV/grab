use echo_service::EchoServiceServer;
use echo_service::FILE_DESCRIPTOR_SET;
use echo_service_impl::EchoServiceImpl;
use granc_core::client::{DynamicRequest, DynamicResponse, GrancClient};
use tonic_reflection::server::v1::ServerReflectionServer;

mod echo_service_impl;

fn reflection_service()
-> ServerReflectionServer<impl tonic_reflection::server::v1::ServerReflection> {
    tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()
        .expect("Failed to setup Reflection Service")
}

#[tokio::test]
async fn test_unary() {
    let payload = serde_json::json!({ "message": "hello" });

    let request = DynamicRequest {
        body: payload.clone(),
        headers: vec![],
        service: "echo.EchoService".to_string(),
        method: "UnaryEcho".to_string(),
    };

    let mut client = GrancClient::new(EchoServiceServer::new(EchoServiceImpl));

    let res = client.dynamic(request).await.unwrap();

    match res {
        DynamicResponse::Unary(Ok(value)) => assert_eq!(value, payload),
        DynamicResponse::Unary(Err(_)) => {
            panic!("Received error status for valid unary request")
        }
        _ => panic!("Received stream response for unary request"),
    };
}

#[tokio::test]
async fn test_server_streaming() {
    let payload = serde_json::json!({ "message": "stream" });

    let request = DynamicRequest {
        file_descriptor_set: Some(FILE_DESCRIPTOR_SET.to_vec()),
        body: payload.clone(),
        headers: vec![],
        service: "echo.EchoService".to_string(),
        method: "ServerStreamingEcho".to_string(),
    };

    let mut client = GrancClient::new(EchoServiceServer::new(EchoServiceImpl));

    let res = client.dynamic(request).await.unwrap();

    match res {
        DynamicResponse::Streaming(Ok(elems)) => {
            let results: Vec<_> = elems.into_iter().map(|r| r.unwrap()).collect();

            assert_eq!(results.len(), 3);
            assert_eq!(results[0]["message"], "stream - seq 0");
            assert_eq!(results[1]["message"], "stream - seq 1");
            assert_eq!(results[2]["message"], "stream - seq 2");
        }
        DynamicResponse::Streaming(Err(_)) => {
            panic!("Received error status for valid server streaming request")
        }
        _ => panic!("Received unary response for server streaming request"),
    };
}

#[tokio::test]
async fn test_client_streaming() {
    let payload = serde_json::json!([
        { "message": "A" },
        { "message": "B" },
        { "message": "C" }
    ]);

    let request = DynamicRequest {
        file_descriptor_set: Some(FILE_DESCRIPTOR_SET.to_vec()),
        body: payload.clone(),
        headers: vec![],
        service: "echo.EchoService".to_string(),
        method: "ClientStreamingEcho".to_string(),
    };

    let mut client = GrancClient::new(EchoServiceServer::new(EchoServiceImpl));

    let res = client.dynamic(request).await.unwrap();

    match res {
        DynamicResponse::Unary(Ok(value)) => {
            assert_eq!(value, serde_json::json!({"message": "ABC"}))
        }
        DynamicResponse::Unary(Err(_)) => {
            panic!("Received error status for valid client stream request")
        }
        _ => panic!("Received stream response for client stream request"),
    };
}

#[tokio::test]
async fn test_bidirectional_streaming() {
    let payload = serde_json::json!([
        { "message": "Ping" },
        { "message": "Pong" }
    ]);

    let request = DynamicRequest {
        file_descriptor_set: Some(FILE_DESCRIPTOR_SET.to_vec()),
        body: payload.clone(),
        headers: vec![],
        service: "echo.EchoService".to_string(),
        method: "BidirectionalEcho".to_string(),
    };

    let mut client = GrancClient::new(EchoServiceServer::new(EchoServiceImpl));

    let res = client.dynamic(request).await.unwrap();

    match res {
        DynamicResponse::Streaming(Ok(elems)) => {
            let results: Vec<_> = elems.into_iter().map(|r| r.unwrap()).collect();

            assert_eq!(results.len(), 2);
            assert_eq!(results[0]["message"], "echo: Ping");
            assert_eq!(results[1]["message"], "echo: Pong");
        }
        DynamicResponse::Streaming(Err(_)) => {
            panic!("Received error status for valid bidirectional streaming request")
        }
        _ => panic!("Received unary response for bidirectional streaming request"),
    };
}

#[tokio::test]
async fn test_list_services_success() {
    let mut client = GrancClient::new(reflection_service());

    let services = client
        .list_services()
        .await
        .expect("Failed to list services");

    // We expect "echo.EchoService" because we registered it.
    // The list usually also includes the reflection service itself ("grpc.reflection.v1.ServerReflection").
    assert!(
        services.contains(&"echo.EchoService".to_string()),
        "Services list did not contain 'echo.EchoService'. Found: {:?}",
        services
    );
}

#[tokio::test]
async fn test_get_service_descriptor_success() {
    let mut client = GrancClient::new(reflection_service());

    let descriptor = client
        .get_descriptor_by_symbol("echo.EchoService")
        .await
        .expect("Failed to get service descriptor");

    let descriptor = descriptor.service_descriptor().unwrap();

    assert_eq!(descriptor.name(), "EchoService");
    assert_eq!(descriptor.full_name(), "echo.EchoService");

    // Verify methods are present
    let method_names: Vec<String> = descriptor.methods().map(|m| m.name().to_string()).collect();
    assert!(method_names.contains(&"UnaryEcho".to_string()));
    assert!(method_names.contains(&"ServerStreamingEcho".to_string()));
    assert!(method_names.contains(&"ClientStreamingEcho".to_string()));
    assert!(method_names.contains(&"BidirectionalEcho".to_string()));
}

#[tokio::test]
async fn test_get_message_descriptor_success() {
    let mut client = GrancClient::new(reflection_service());

    let desc = client
        .get_descriptor_by_symbol("echo.EchoRequest")
        .await
        .expect("Failed to get message descriptor");

    let desc = desc.message_descriptor().unwrap();

    assert_eq!(desc.name(), "EchoRequest");
    assert_eq!(desc.full_name(), "echo.EchoRequest");

    // Verify fields
    let fields: Vec<String> = desc.fields().map(|f| f.name().to_string()).collect();
    assert!(fields.contains(&"message".to_string()));
}

#[tokio::test]
async fn test_get_descriptor_not_found() {
    let mut client = GrancClient::new(reflection_service());

    // "echo.GhostService" does not exist in the registered descriptors.
    // The reflection client should fail to find the symbol, resulting in a ResolutionError.
    let err = client
        .get_descriptor_by_symbol("echo.GhostService")
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        granc_core::client::GetDescriptorError::NotFound(_)
    ));
}
