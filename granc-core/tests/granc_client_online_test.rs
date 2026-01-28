use echo_service::{EchoServiceServer, FILE_DESCRIPTOR_SET};
use echo_service_impl::EchoServiceImpl;
use granc_core::client::{DynamicRequest, DynamicResponse, GrancClient, Online, online};
use granc_core::reflection::client::ReflectionResolveError;
use tonic::Code;
use tonic::service::Routes;

mod echo_service_impl;

async fn setup_client() -> GrancClient<Online<Routes>> {
    // Enable Reflection
    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()
        .unwrap();

    let echo_service = EchoServiceServer::new(EchoServiceImpl);

    let service = Routes::new(reflection_service).add_service(echo_service);

    GrancClient::from(service)
}

#[tokio::test]
async fn test_reflection_list_services() {
    let mut client = setup_client().await;
    let mut services = client.list_services().await.unwrap();
    services.sort();

    assert_eq!(
        services.as_slice(),
        ["echo.EchoService", "grpc.reflection.v1.ServerReflection"]
    );
}

#[tokio::test]
async fn test_reflection_unary_success() {
    let mut client = setup_client().await;

    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "UnaryEcho".to_string(),
        body: serde_json::json!({ "message": "reflection" }),
        headers: vec![],
    };

    let res = client.dynamic(req).await.unwrap();
    assert!(matches!(res, DynamicResponse::Unary(Ok(val)) if val["message"] == "reflection"));
}

#[tokio::test]
async fn test_reflection_server_streaming_success() {
    let mut client = setup_client().await;

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
async fn test_reflection_client_streaming_success() {
    let mut client = setup_client().await;

    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "ClientStreamingEcho".to_string(),
        body: serde_json::json!([{ "message": "A" }, { "message": "B" }]),
        headers: vec![],
    };

    let res = client.dynamic(req).await.unwrap();
    assert!(matches!(res, DynamicResponse::Unary(Ok(val)) if val["message"] == "AB"));
}

#[tokio::test]
async fn test_reflection_service_not_found() {
    let mut client = setup_client().await;

    // Requesting a service that doesn't exist on the server.
    // This fails during the Reflection Lookup phase.
    let req = DynamicRequest {
        service: "echo.GhostService".to_string(),
        method: "UnaryEcho".to_string(),
        body: serde_json::json!({}),
        headers: vec![],
    };

    let result = client.dynamic(req).await;

    assert!(matches!(
        result,
        Err(online::DynamicCallError::ReflectionResolve(
            ReflectionResolveError::ServerStreamFailure(status)
        )) if status.code() == Code::NotFound
    ));
}

#[tokio::test]
async fn test_reflection_method_not_found() {
    let mut client = setup_client().await;

    // The service exists, so reflection succeeds in fetching the schema.
    // However, the schema does not contain "GhostMethod", so it fails locally before call.
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "GhostMethod".to_string(),
        body: serde_json::json!({}),
        headers: vec![],
    };

    let result = client.dynamic(req).await;

    assert!(matches!(
        result,
        Err(online::DynamicCallError::DynamicCallError(
            granc_core::client::online_without_reflection::DynamicCallError::MethodNotFound(name)
        )) if name == "GhostMethod"
    ));
}

#[tokio::test]
async fn test_reflection_invalid_input_structure() {
    let mut client = setup_client().await;

    // Client streaming requires Array.
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "ClientStreamingEcho".to_string(),
        body: serde_json::json!({ "msg": "not array" }),
        headers: vec![],
    };

    let result = client.dynamic(req).await;

    assert!(matches!(
        result,
        Err(online::DynamicCallError::DynamicCallError(
            granc_core::client::online_without_reflection::DynamicCallError::InvalidInput(_)
        ))
    ));
}

#[tokio::test]
async fn test_reflection_schema_mismatch() {
    let mut client = setup_client().await;

    // Field "wrong_field" does not exist in the protobuf definition.
    // Should fail with InvalidArgument during encoding.
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "UnaryEcho".to_string(),
        body: serde_json::json!({ "wrong_field": "val" }),
        headers: vec![],
    };

    let result = client.dynamic(req).await;

    assert!(matches!(
        result,
        Ok(DynamicResponse::Unary(Err(status))) if status.code() == Code::Internal
    ));
}
