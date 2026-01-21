use crate::core::reflection::{
    client::{
        ReflectionClient, ReflectionResolveError, integration_test::dummy_service::DummyEchoService,
    },
    generated::reflection_v1::server_reflection_client::ServerReflectionClient,
};
use echo_service::{EchoServiceServer, FILE_DESCRIPTOR_SET};
use tonic::Code;
use tonic_reflection::server::v1::ServerReflectionServer;

mod dummy_service;

fn setup_reflection_client()
-> ReflectionClient<ServerReflectionServer<impl tonic_reflection::server::v1::ServerReflection>> {
    // Configure the Reflection Service using the descriptor set from echo-service
    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()
        .expect("Failed to setup Reflection Service");

    ReflectionClient {
        client: ServerReflectionClient::new(reflection_service),
        base_url: "http://localhost".to_string(),
    }
}

#[tokio::test]
async fn test_reflection_client_fetches_unary_echo() {
    let mut client = setup_reflection_client();

    let registry = client
        .resolve_service_descriptor_registry("echo.EchoService")
        .await
        .expect("Failed to resolve service descriptor registry");

    let method = registry
        .get_method_descriptor("echo.EchoService", "UnaryEcho")
        .expect("Method UnaryEcho not found");

    // Assert Types
    assert_eq!(method.input().name(), "EchoRequest");
    assert_eq!(method.output().name(), "EchoResponse");

    // Assert Streaming Properties (Unary = No Streaming)
    assert!(
        !method.is_client_streaming(),
        "Unary should not be client streaming"
    );
    assert!(
        !method.is_server_streaming(),
        "Unary should not be server streaming"
    );
}

#[tokio::test]
async fn test_reflection_client_fetches_server_streaming_echo() {
    let mut client = setup_reflection_client();

    let registry = client
        .resolve_service_descriptor_registry("echo.EchoService")
        .await
        .expect("Failed to resolve service descriptor registry");

    let method = registry
        .get_method_descriptor("echo.EchoService", "ServerStreamingEcho")
        .expect("Method ServerStreamingEcho not found");

    // Assert Types
    assert_eq!(method.input().name(), "EchoRequest");
    assert_eq!(method.output().name(), "EchoResponse");

    // Assert Streaming Properties (Server Streaming only)
    assert!(
        !method.is_client_streaming(),
        "ServerStreaming should not be client streaming"
    );
    assert!(
        method.is_server_streaming(),
        "ServerStreaming MUST be server streaming"
    );
}

#[tokio::test]
async fn test_reflection_client_fetches_client_streaming_echo() {
    let mut client = setup_reflection_client();

    let registry = client
        .resolve_service_descriptor_registry("echo.EchoService")
        .await
        .expect("Failed to resolve service descriptor registry");

    let method = registry
        .get_method_descriptor("echo.EchoService", "ClientStreamingEcho")
        .expect("Method ClientStreamingEcho not found");

    // Assert Types
    assert_eq!(method.input().name(), "EchoRequest");
    assert_eq!(method.output().name(), "EchoResponse");

    // Assert Streaming Properties (Client Streaming only)
    assert!(
        method.is_client_streaming(),
        "ClientStreaming MUST be client streaming"
    );
    assert!(
        !method.is_server_streaming(),
        "ClientStreaming should not be server streaming"
    );
}

#[tokio::test]
async fn test_reflection_client_fetches_bidirectional_echo() {
    let mut client = setup_reflection_client();

    let registry = client
        .resolve_service_descriptor_registry("echo.EchoService")
        .await
        .expect("Failed to resolve service descriptor registry");

    let method = registry
        .get_method_descriptor("echo.EchoService", "BidirectionalEcho")
        .expect("Method BidirectionalEcho not found");

    assert_eq!(method.input().name(), "EchoRequest");
    assert_eq!(method.output().name(), "EchoResponse");

    assert!(
        method.is_client_streaming(),
        "Bidirectional MUST be client streaming"
    );
    assert!(
        method.is_server_streaming(),
        "Bidirectional MUST be server streaming"
    );
}

#[tokio::test]
async fn test_reflection_service_not_found_error() {
    let mut client = setup_reflection_client();

    let result: Result<_, _> = client
        .resolve_service_descriptor_registry("non.existent.Service")
        .await;

    assert!(matches!(
        result,
        Err(crate::core::reflection::client::ReflectionResolveError::ServerStreamFailure(status)) if status.code() == Code::NotFound
    ));
}

#[tokio::test]
async fn test_server_does_not_support_reflection() {
    // Create a server that ONLY hosts the EchoService.
    // This server does NOT have the Reflection service registered.
    let server = EchoServiceServer::new(DummyEchoService);

    let mut client = ReflectionClient {
        client: ServerReflectionClient::new(server),
        base_url: "http://localhost".to_string(),
    };

    // The client will attempt to call `/grpc.reflection.v1.ServerReflection/ServerReflectionInfo` on this service.
    let result = client
        .resolve_service_descriptor_registry("echo.EchoService")
        .await;

    match result {
        Err(ReflectionResolveError::ServerStreamInitFailed(status)) => {
            assert_eq!(
                status.code(),
                tonic::Code::Unimplemented,
                "Expected UNIMPLEMENTED status (service not found), but got: {:?}",
                status
            );
        }
        Err(e) => panic!("Expected StreamInitFailed(Unimplemented), got: {:?}", e),
        Ok(_) => panic!("Expected error, but got successful registry"),
    }
}
