use echo_service::{EchoServiceServer, FILE_DESCRIPTOR_SET};
use echo_service_impl::EchoServiceImpl;
use granc_core::client::{
    Descriptor, DynamicRequest, DynamicResponse, GrancClient, WithServerReflection,
    with_file_descriptor, with_server_reflection,
};
use granc_core::reflection::client::ReflectionResolveError;
use tonic::Code;
use tonic::service::Routes;

mod echo_service_impl;

async fn setup_client() -> GrancClient<WithServerReflection<Routes>> {
    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()
        .unwrap();

    let echo_service = EchoServiceServer::new(EchoServiceImpl);

    let service = Routes::new(reflection_service).add_service(echo_service);

    GrancClient::from_service(service)
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
async fn test_reflection_describe_descriptors() {
    let mut client = setup_client().await;

    let desc = client
        .get_descriptor_by_symbol("echo.EchoService")
        .await
        .unwrap();

    assert!(matches!(
        desc,
        Descriptor::ServiceDescriptor(s)
            if s.name() == "EchoService"
            && s.methods().any(|m| m.name() == "UnaryEcho")
    ));

    let desc = client
        .get_descriptor_by_symbol("echo.EchoRequest")
        .await
        .unwrap();

    assert!(matches!(
        desc,
        Descriptor::MessageDescriptor(m)
            if m.name() == "EchoRequest"
            && m.fields().any(|f| f.name() == "message")
    ));
}

#[tokio::test]
async fn test_reflection_describe_error() {
    let mut client = setup_client().await;

    let result = client.get_descriptor_by_symbol("echo.Ghost").await;

    assert!(matches!(
        result,
        Err(with_server_reflection::GetDescriptorError::NotFound(name)) if name == "echo.Ghost"
    ));
}

#[tokio::test]
async fn test_reflection_dynamic_calls() {
    let mut client = setup_client().await;

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
async fn test_reflection_dynamic_error_cases() {
    let mut client = setup_client().await;

    // Invalid Service Name
    let req = DynamicRequest {
        service: "echo.GhostService".to_string(),
        method: "UnaryEcho".to_string(),
        body: serde_json::json!({}),
        headers: vec![],
    };

    let result = client.dynamic(req).await;

    assert!(matches!(
        result,
        Err(with_server_reflection::DynamicCallError::ReflectionResolve(
            ReflectionResolveError::ServerStreamFailure(status)
        )) if status.code() == Code::NotFound
    ));

    // Invalid Method Name
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "GhostMethod".to_string(),
        body: serde_json::json!({}),
        headers: vec![],
    };

    let result = client.dynamic(req).await;

    assert!(matches!(
        result,
        Err(with_server_reflection::DynamicCallError::DynamicCallError(
            with_file_descriptor::DynamicCallError::MethodNotFound(name)
        )) if name == "GhostMethod"
    ));

    // Invalid JSON Structure (Streaming requires Array, Object provided)
    // This triggers `DynamicCallError::InvalidInput` before the request is sent.
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "ClientStreamingEcho".to_string(),
        body: serde_json::json!({ "message": "I should be an array" }),
        headers: vec![],
    };

    let result = client.dynamic(req).await;

    assert!(matches!(
        result,
        Err(with_server_reflection::DynamicCallError::DynamicCallError(
            with_file_descriptor::DynamicCallError::InvalidInput(_)
        ))
    ));

    // Schema Mismatch (Unary)
    // Passing a field that doesn't exist. This fails at encoding time inside the Codec.
    // Tonic wraps encoding errors as Code::Internal.
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "UnaryEcho".to_string(),
        body: serde_json::json!({ "non_existent_field": "oops" }),
        headers: vec![],
    };

    let result = client.dynamic(req).await;

    assert!(matches!(
        result,
        Ok(DynamicResponse::Unary(Err(status))) if status.code() == Code::Internal
    ));
}
