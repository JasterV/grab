use echo_service::EchoServiceServer;
use echo_service::FILE_DESCRIPTOR_SET;
use echo_service_impl::EchoServiceImpl;
use granc_core::client::{DynamicRequest, DynamicResponse, GrancClient};

mod echo_service_impl;

#[tokio::test]
async fn test_unary() {
    let payload = serde_json::json!({ "message": "hello" });

    let request = DynamicRequest {
        file_descriptor_set: Some(FILE_DESCRIPTOR_SET.to_vec()),
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
