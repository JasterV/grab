use echo_service::{EchoServiceServer, FILE_DESCRIPTOR_SET};
use echo_service_impl::EchoServiceImpl;
use granc_core::reflection::client::{ReflectionClient, ReflectionResolveError};
use prost_reflect::DescriptorPool;
use tonic::Code;
use tonic_reflection::server::v1::ServerReflectionServer;

mod echo_service_impl;

fn setup_reflection_client()
-> ReflectionClient<ServerReflectionServer<impl tonic_reflection::server::v1::ServerReflection>> {
    // Configure the Reflection Service using the descriptor set from echo-service
    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()
        .expect("Failed to setup Reflection Service");

    ReflectionClient::new(reflection_service)
}

#[tokio::test]
async fn test_reflection_client_fetches_service_file_descriptor() {
    let mut client = setup_reflection_client();

    let fd_set = client
        .file_descriptor_set_by_symbol("echo.EchoService")
        .await
        .expect("Failed to fetch file descriptor set by symbol");

    let pool =
        DescriptorPool::from_file_descriptor_set(fd_set).expect("Failed to build descriptor pool");

    let service = pool
        .get_service_by_name("echo.EchoService")
        .expect("Failed to find service in file descriptor");

    assert!(service.methods().all(|f| f.input().name() == "EchoRequest"));
    assert!(
        service
            .methods()
            .all(|f| f.output().name() == "EchoResponse")
    );

    let unary_method = service.methods().find(|m| m.name() == "UnaryEcho").unwrap();

    let client_streaming_method = service
        .methods()
        .find(|m| m.name() == "ClientStreamingEcho")
        .unwrap();

    let server_streaming_method = service
        .methods()
        .find(|m| m.name() == "ServerStreamingEcho")
        .unwrap();

    let bidirectional_method = service
        .methods()
        .find(|m| m.name() == "BidirectionalEcho")
        .unwrap();

    assert!(
        !unary_method.is_client_streaming(),
        "Unary should not be client streaming"
    );
    assert!(
        !unary_method.is_server_streaming(),
        "Unary should not be server streaming"
    );

    // Assert Streaming Properties (Client Streaming only)
    assert!(
        client_streaming_method.is_client_streaming(),
        "ClientStreaming MUST be client streaming"
    );
    assert!(
        !client_streaming_method.is_server_streaming(),
        "ClientStreaming should not be server streaming"
    );

    assert!(
        !server_streaming_method.is_client_streaming(),
        "ServerStreaming should not be client streaming"
    );
    assert!(
        server_streaming_method.is_server_streaming(),
        "ServerStreaming MUST be server streaming"
    );

    assert!(
        bidirectional_method.is_client_streaming(),
        "Bidirectional MUST be client streaming"
    );

    assert!(
        bidirectional_method.is_server_streaming(),
        "Bidirectional MUST be server streaming"
    );
}

#[tokio::test]
async fn test_reflection_service_not_found_error() {
    let mut client = setup_reflection_client();

    let result: Result<_, _> = client
        .file_descriptor_set_by_symbol("non.existent.Service")
        .await;

    assert!(matches!(
        result,
        Err(ReflectionResolveError::ServerStreamFailure(status)) if status.code() == Code::NotFound
    ));
}

#[tokio::test]
async fn test_server_does_not_support_reflection() {
    // Create a server that ONLY hosts the EchoService.
    // This server does NOT have the Reflection service registered.
    let server = EchoServiceServer::new(EchoServiceImpl);
    let mut client = ReflectionClient::new(server);

    // The client will attempt to call `/grpc.reflection.v1.ServerReflection/ServerReflectionInfo` on this service.
    let result = client
        .file_descriptor_set_by_symbol("echo.EchoService")
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
