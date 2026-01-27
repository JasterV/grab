use echo_service::{EchoServiceServer, FILE_DESCRIPTOR_SET};
use echo_service_impl::EchoServiceImpl;
use granc_core::client::{
    Descriptor, DynamicRequest, DynamicResponse, GrancClient, WithFileDescriptor,
    with_file_descriptor,
};
use tonic::Code;

mod echo_service_impl;

fn setup_client() -> GrancClient<WithFileDescriptor<EchoServiceServer<EchoServiceImpl>>> {
    let service = EchoServiceServer::new(EchoServiceImpl);
    let client_reflection = GrancClient::from_service(service);

    client_reflection
        .with_file_descriptor(FILE_DESCRIPTOR_SET.to_vec())
        .expect("Failed to load file descriptor set")
}

#[tokio::test]
async fn test_list_services() {
    let mut client = setup_client();

    let services = client.list_services();

    assert_eq!(services.as_slice(), ["echo.EchoService"]);
}

#[tokio::test]
async fn test_describe_descriptors() {
    let mut client = setup_client();

    // Describe Service
    let desc = client
        .get_descriptor_by_symbol("echo.EchoService")
        .expect("Service not found");

    assert!(matches!(
        desc,
        Descriptor::ServiceDescriptor(s) if s.name() == "EchoService"
    ));

    // Describe Message
    let desc = client
        .get_descriptor_by_symbol("echo.EchoRequest")
        .expect("Message not found");

    assert!(matches!(
        desc,
        Descriptor::MessageDescriptor(m) if m.name() == "EchoRequest"
    ));

    // Error Case: Returns None
    let desc = client.get_descriptor_by_symbol("echo.Ghost");

    assert!(desc.is_none());
}

#[tokio::test]
async fn test_dynamic_calls() {
    let mut client = setup_client();

    // Unary Call
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "UnaryEcho".to_string(),
        body: serde_json::json!({ "message": "hello" }),
        headers: vec![],
    };

    let res = client.dynamic(req).await.unwrap();

    assert!(matches!(res, DynamicResponse::Unary(Ok(val)) if val["message"] == "hello"));

    // Server Streaming
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "ServerStreamingEcho".to_string(),
        body: serde_json::json!({ "message": "stream" }),
        headers: vec![],
    };

    let res = client.dynamic(req).await.unwrap();

    assert!(matches!(res, DynamicResponse::Streaming(Ok(stream)) if stream.len() == 3));

    // Client Streaming
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "ClientStreamingEcho".to_string(),
        body: serde_json::json!([
            { "message": "A" },
            { "message": "B" }
        ]),
        headers: vec![],
    };

    let res = client.dynamic(req).await.unwrap();

    assert!(matches!(res, DynamicResponse::Unary(Ok(val)) if val["message"] == "AB"));

    // Bidirectional Streaming
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "BidirectionalEcho".to_string(),
        body: serde_json::json!([
            { "message": "Ping" }
        ]),
        headers: vec![],
    };
    let res = client.dynamic(req).await.unwrap();

    assert!(matches!(res,
        DynamicResponse::Streaming(Ok(stream))
            if stream.len() == 1
            && stream[0].as_ref().unwrap()["message"] == "echo: Ping"
    ));
}

#[tokio::test]
async fn test_error_cases() {
    let mut client = setup_client();

    // Service Not Found
    let req = DynamicRequest {
        service: "echo.GhostService".to_string(),
        method: "UnaryEcho".to_string(),
        body: serde_json::json!({}),
        headers: vec![],
    };

    let result = client.dynamic(req).await;

    assert!(matches!(
        result,
        Err(with_file_descriptor::DynamicCallError::ServiceNotFound(name)) if name == "echo.GhostService"
    ));

    // Method Not Found
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "GhostMethod".to_string(),
        body: serde_json::json!({}),
        headers: vec![],
    };

    let result = client.dynamic(req).await;

    assert!(matches!(
        result,
        Err(with_file_descriptor::DynamicCallError::MethodNotFound(name)) if name == "GhostMethod"
    ));

    // Invalid JSON Structure (Streaming requires Array)
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "ClientStreamingEcho".to_string(),
        body: serde_json::json!({ "message": "I should be an array" }),
        headers: vec![],
    };

    let result = client.dynamic(req).await;

    assert!(matches!(
        result,
        Err(with_file_descriptor::DynamicCallError::InvalidInput(_))
    ));

    // Schema Mismatch (Unary)
    // Field mismatch causes encoding error -> Status::InvalidArgument
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "UnaryEcho".to_string(),
        body: serde_json::json!({ "unknown_field": 123 }),
        headers: vec![],
    };

    let result = client.dynamic(req).await;

    assert!(matches!(
        result,
        Ok(DynamicResponse::Unary(Err(status)))
            if status.code() == Code::Internal
            && status.message().contains("JSON structure does not match")
    ));
}
