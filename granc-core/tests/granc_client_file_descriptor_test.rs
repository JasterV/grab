use echo_service::{EchoServiceServer, FILE_DESCRIPTOR_SET};
use echo_service_impl::EchoServiceImpl;
use granc_core::client::{
    Descriptor, DynamicRequest, DynamicResponse, GrancClient, with_file_descriptor,
};
use tonic::Code;

mod echo_service_impl;

fn setup_client() -> GrancClient<
    granc_core::client::with_file_descriptor::WithFileDescriptor<
        EchoServiceServer<EchoServiceImpl>,
    >,
> {
    let service = EchoServiceServer::new(EchoServiceImpl);
    let client_reflection = GrancClient::from_service(service);

    // Transition to File Descriptor state using the embedded set
    client_reflection
        .with_file_descriptor(FILE_DESCRIPTOR_SET.to_vec())
        .expect("Failed to load file descriptor set")
}

#[tokio::test]
async fn test_list_services() {
    let mut client = setup_client();
    let services = client.list_services();

    // The file descriptor set should contain the EchoService
    assert!(services.contains(&"echo.EchoService".to_string()));
}

#[tokio::test]
async fn test_describe_descriptors() {
    let mut client = setup_client();

    // 1. Describe Service
    let desc = client
        .get_descriptor_by_symbol("echo.EchoService")
        .expect("Service not found");
    if let Descriptor::ServiceDescriptor(s) = desc {
        assert_eq!(s.name(), "EchoService");
    } else {
        panic!("Expected ServiceDescriptor");
    }

    // 2. Describe Message
    let desc = client
        .get_descriptor_by_symbol("echo.EchoRequest")
        .expect("Message not found");
    if let Descriptor::MessageDescriptor(m) = desc {
        assert_eq!(m.name(), "EchoRequest");
    } else {
        panic!("Expected MessageDescriptor");
    }

    // 3. Error Case: Returns None
    let desc = client.get_descriptor_by_symbol("echo.Ghost");
    assert!(desc.is_none());
}

#[tokio::test]
async fn test_dynamic_calls() {
    let mut client = setup_client();

    // 1. Unary Call
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "UnaryEcho".to_string(),
        body: serde_json::json!({ "message": "hello-fd" }),
        headers: vec![],
    };
    let res = client.dynamic(req).await.unwrap();
    if let DynamicResponse::Unary(Ok(val)) = res {
        assert_eq!(val["message"], "hello-fd");
    } else {
        panic!("Unexpected response type for Unary");
    }

    // 2. Server Streaming
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "ServerStreamingEcho".to_string(),
        body: serde_json::json!({ "message": "stream-fd" }),
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
            { "message": "X" },
            { "message": "Y" }
        ]),
        headers: vec![],
    };
    let res = client.dynamic(req).await.unwrap();
    if let DynamicResponse::Unary(Ok(val)) = res {
        assert_eq!(val["message"], "XY");
    } else {
        panic!("Unexpected response type for Client Streaming");
    }

    // 4. Bidirectional Streaming
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "BidirectionalEcho".to_string(),
        body: serde_json::json!([
            { "message": "PingFD" }
        ]),
        headers: vec![],
    };
    let res = client.dynamic(req).await.unwrap();
    if let DynamicResponse::Streaming(Ok(stream)) = res {
        assert_eq!(stream.len(), 1);
        assert_eq!(stream[0].as_ref().unwrap()["message"], "echo: PingFD");
    } else {
        panic!("Unexpected response type for Bidi Streaming");
    }
}

#[tokio::test]
async fn test_error_cases() {
    let mut client = setup_client();

    // 1. Service Not Found (in local FD)
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

    // 2. Method Not Found
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

    // 3. Invalid JSON Structure (Streaming requires Array)
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

    // 4. Schema Mismatch (Unary)
    // Field mismatch causes encoding error -> Status::InvalidArgument
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "UnaryEcho".to_string(),
        body: serde_json::json!({ "unknown_field": 123 }),
        headers: vec![],
    };
    let result = client.dynamic(req).await;
    println!("{result:?}");

    if let Ok(DynamicResponse::Unary(Err(status))) = result {
        assert_eq!(status.code(), Code::Internal);
        // Verify Tonic is wrapping our error message
        assert!(status.message().contains("JSON structure does not match"));
    } else {
        panic!("Expected Unary(Err(Internal)), got: {:?}", result);
    }
}
