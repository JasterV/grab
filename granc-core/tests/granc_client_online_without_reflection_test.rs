use echo_service::{EchoServiceServer, FILE_DESCRIPTOR_SET};
use echo_service_impl::EchoServiceImpl;
use granc_core::client::{
    DynamicRequest, DynamicResponse, GrancClient, OnlineWithoutReflection,
    online_without_reflection,
};
use tonic::Code;

mod echo_service_impl;

fn setup_client() -> GrancClient<OnlineWithoutReflection<EchoServiceServer<EchoServiceImpl>>> {
    let service = EchoServiceServer::new(EchoServiceImpl);
    let client_reflection = GrancClient::from(service);

    client_reflection
        .with_file_descriptor(FILE_DESCRIPTOR_SET.to_vec())
        .expect("Failed to load file descriptor set")
}

#[tokio::test]
async fn test_dynamic_unary_success() {
    let mut client = setup_client();

    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "UnaryEcho".to_string(),
        body: serde_json::json!({ "message": "hello" }),
        headers: vec![],
    };

    let res = client.dynamic(req).await.unwrap();

    assert!(matches!(
        res,
        DynamicResponse::Unary(Ok(val)) if val["message"] == "hello"
    ));
}

#[tokio::test]
async fn test_dynamic_server_streaming_success() {
    let mut client = setup_client();

    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "ServerStreamingEcho".to_string(),
        body: serde_json::json!({ "message": "stream" }),
        headers: vec![],
    };

    let res = client.dynamic(req).await.unwrap();

    match res {
        DynamicResponse::Streaming(Ok(stream)) => {
            assert_eq!(stream.len(), 3);
            assert_eq!(stream[0].as_ref().unwrap()["message"], "stream - seq 0");
            assert_eq!(stream[1].as_ref().unwrap()["message"], "stream - seq 1");
            assert_eq!(stream[2].as_ref().unwrap()["message"], "stream - seq 2");
        }
        _ => panic!("Expected Streaming response"),
    }
}

#[tokio::test]
async fn test_dynamic_client_streaming_success() {
    let mut client = setup_client();

    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "ClientStreamingEcho".to_string(),
        // Client streaming requires a JSON Array
        body: serde_json::json!([
            { "message": "A" },
            { "message": "B" },
            { "message": "C" }
        ]),
        headers: vec![],
    };

    let res = client.dynamic(req).await.unwrap();

    assert!(matches!(
        res,
        DynamicResponse::Unary(Ok(val)) if val["message"] == "ABC"
    ));
}

#[tokio::test]
async fn test_dynamic_bidirectional_streaming_success() {
    let mut client = setup_client();

    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "BidirectionalEcho".to_string(),
        body: serde_json::json!([
            { "message": "Ping" },
            { "message": "Pong" }
        ]),
        headers: vec![],
    };

    let res = client.dynamic(req).await.unwrap();

    match res {
        DynamicResponse::Streaming(Ok(stream)) => {
            assert_eq!(stream.len(), 2);
            assert_eq!(stream[0].as_ref().unwrap()["message"], "echo: Ping");
            assert_eq!(stream[1].as_ref().unwrap()["message"], "echo: Pong");
        }
        _ => panic!("Expected Streaming response"),
    }
}

#[tokio::test]
async fn test_error_service_not_found() {
    let mut client = setup_client();

    let req = DynamicRequest {
        service: "echo.GhostService".to_string(),
        method: "UnaryEcho".to_string(),
        body: serde_json::json!({}),
        headers: vec![],
    };

    let result = client.dynamic(req).await;

    assert!(matches!(
        result,
        Err(online_without_reflection::DynamicCallError::ServiceNotFound(name)) if name == "echo.GhostService"
    ));
}

#[tokio::test]
async fn test_error_method_not_found() {
    let mut client = setup_client();

    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "GhostMethod".to_string(),
        body: serde_json::json!({}),
        headers: vec![],
    };

    let result = client.dynamic(req).await;

    assert!(matches!(
        result,
        Err(online_without_reflection::DynamicCallError::MethodNotFound(name)) if name == "GhostMethod"
    ));
}

#[tokio::test]
async fn test_error_invalid_input_structure() {
    let mut client = setup_client();

    // Client streaming requires an Array, passing an Object should fail
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "ClientStreamingEcho".to_string(),
        body: serde_json::json!({ "message": "I should be an array" }),
        headers: vec![],
    };

    let result = client.dynamic(req).await;

    assert!(matches!(
        result,
        Err(online_without_reflection::DynamicCallError::InvalidInput(_))
    ));
}

#[tokio::test]
async fn test_error_schema_mismatch() {
    let mut client = setup_client();

    // Passing a field ("unknown_field") that doesn't exist in the EchoRequest proto definition.
    // The JsonCodec (in granc-core/src/grpc/codec.rs) maps this to Status::InvalidArgument.
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "UnaryEcho".to_string(),
        body: serde_json::json!({ "unknown_field": 123 }),
        headers: vec![],
    };

    let result = client.dynamic(req).await;

    // This error happens during encoding inside the Tonic stack, so it returns
    // a successful Result<DynamicResponse> containing an Err(Status).
    assert!(matches!(
        result,
        Ok(DynamicResponse::Unary(Err(status)))
            if status.code() == Code::Internal
            && status.message().contains("JSON structure does not match Protobuf schema")
    ));
}
