use echo_service::{EchoServiceServer, FILE_DESCRIPTOR_SET};
use echo_service_impl::EchoServiceImpl;
use granc_core::client::{
    Descriptor, DynamicRequest, DynamicResponse, GrancClient, with_file_descriptor,
    with_server_reflection,
};
use granc_core::reflection::client::ReflectionResolveError;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{Code, transport::Server};

mod echo_service_impl;

async fn spawn_server() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        let reflection_service = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
            .build_v1()
            .unwrap();

        let echo_service = EchoServiceServer::new(EchoServiceImpl);

        Server::builder()
            .add_service(reflection_service)
            .add_service(echo_service)
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await
            .unwrap();
    });

    addr
}

#[tokio::test]
async fn test_list_services() {
    let addr = spawn_server().await;
    let url = format!("http://{}", addr);
    let mut client = GrancClient::connect(&url).await.unwrap();

    let services = client.list_services().await.unwrap();
    assert!(services.contains(&"echo.EchoService".to_string()));
    assert!(services.contains(&"grpc.reflection.v1.ServerReflection".to_string()));
}

#[tokio::test]
async fn test_describe_descriptors() {
    let addr = spawn_server().await;
    let url = format!("http://{}", addr);
    let mut client = GrancClient::connect(&url).await.unwrap();

    // 1. Describe Service
    let desc = client
        .get_descriptor_by_symbol("echo.EchoService")
        .await
        .unwrap();
    if let Descriptor::ServiceDescriptor(s) = desc {
        assert_eq!(s.name(), "EchoService");
        assert!(s.methods().any(|m| m.name() == "UnaryEcho"));
    } else {
        panic!("Expected ServiceDescriptor");
    }

    // 2. Describe Message
    let desc = client
        .get_descriptor_by_symbol("echo.EchoRequest")
        .await
        .unwrap();
    if let Descriptor::MessageDescriptor(m) = desc {
        assert_eq!(m.name(), "EchoRequest");
        assert!(m.fields().any(|f| f.name() == "message"));
    } else {
        panic!("Expected MessageDescriptor");
    }
}

#[tokio::test]
async fn test_describe_error() {
    let addr = spawn_server().await;
    let url = format!("http://{}", addr);
    let mut client = GrancClient::connect(&url).await.unwrap();

    // Error Case: Non-existent symbol
    let result = client.get_descriptor_by_symbol("echo.Ghost").await;

    assert!(matches!(
        result,
        Err(with_server_reflection::GetDescriptorError::NotFound(name)) if name == "echo.Ghost"
    ));
}

#[tokio::test]
async fn test_dynamic_calls() {
    let addr = spawn_server().await;
    let url = format!("http://{}", addr);
    let mut client = GrancClient::connect(&url).await.unwrap();

    // 1. Unary Call
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "UnaryEcho".to_string(),
        body: serde_json::json!({ "message": "hello" }),
        headers: vec![],
    };
    let res = client.dynamic(req).await.unwrap();
    if let DynamicResponse::Unary(Ok(val)) = res {
        assert_eq!(val["message"], "hello");
    } else {
        panic!("Unexpected response type for Unary");
    }

    // 2. Server Streaming
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "ServerStreamingEcho".to_string(),
        body: serde_json::json!({ "message": "stream" }),
        headers: vec![],
    };
    let res = client.dynamic(req).await.unwrap();
    if let DynamicResponse::Streaming(Ok(stream)) = res {
        assert_eq!(stream.len(), 3);
    } else {
        panic!("Unexpected response type for Server Streaming");
    }

    // 3. Client Streaming
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
    if let DynamicResponse::Unary(Ok(val)) = res {
        assert_eq!(val["message"], "AB");
    } else {
        panic!("Unexpected response type for Client Streaming");
    }

    // 4. Bidirectional Streaming
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "BidirectionalEcho".to_string(),
        body: serde_json::json!([
            { "message": "Ping" }
        ]),
        headers: vec![],
    };
    let res = client.dynamic(req).await.unwrap();
    if let DynamicResponse::Streaming(Ok(stream)) = res {
        assert_eq!(stream.len(), 1);
        assert_eq!(stream[0].as_ref().unwrap()["message"], "echo: Ping");
    } else {
        panic!("Unexpected response type for Bidi Streaming");
    }
}

#[tokio::test]
async fn test_dynamic_error_cases() {
    let addr = spawn_server().await;
    let url = format!("http://{}", addr);
    let mut client = GrancClient::connect(&url).await.unwrap();

    // 1. Invalid Service Name
    // The reflection client will try to fetch the schema for "echo.GhostService" and fail.
    let req = DynamicRequest {
        service: "echo.GhostService".to_string(),
        method: "UnaryEcho".to_string(),
        body: serde_json::json!({}),
        headers: vec![],
    };
    let result = client.dynamic(req).await;

    // We expect a ReflectionResolve error wrapping a ServerStreamFailure with Code::NotFound
    assert!(matches!(
        result,
        Err(with_server_reflection::DynamicCallError::ReflectionResolve(
            ReflectionResolveError::ServerStreamFailure(status)
        )) if status.code() == Code::NotFound
    ));

    // 2. Invalid Method Name
    // The reflection will succeed for the Service, but then the method lookup will fail locally.
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "GhostMethod".to_string(),
        body: serde_json::json!({}),
        headers: vec![],
    };
    let result = client.dynamic(req).await;

    // We expect the error to bubble up from the underlying WithFileDescriptor client
    assert!(matches!(
        result,
        Err(with_server_reflection::DynamicCallError::DynamicCallError(
            with_file_descriptor::DynamicCallError::MethodNotFound(name)
        )) if name == "GhostMethod"
    ));

    // 3. Invalid JSON Structure (Streaming requires Array, Object provided)
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

    // 4. Schema Mismatch (Unary)
    // Passing a field that doesn't exist. This fails at encoding time inside the Codec.
    // Tonic captures this and returns it as a Status::InvalidArgument.
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "UnaryEcho".to_string(),
        body: serde_json::json!({ "non_existent_field": "oops" }),
        headers: vec![],
    };
    let result = client.dynamic(req).await;

    if let Ok(DynamicResponse::Unary(Err(status))) = result {
        assert_eq!(status.code(), Code::Internal);
        // Note: For network/transport errors (h2 protocol error), specific message matching is fragile.
    } else {
        panic!("Expected Unary(Err(Internal)), got: {:?}", result);
    }
}
