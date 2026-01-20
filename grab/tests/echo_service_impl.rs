use echo_service::EchoService;
use echo_service::pb::{EchoRequest, EchoResponse};

use futures_util::Stream;
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use tonic::{Request, Response, Status, Streaming};

pub struct EchoServiceImpl;

#[tonic::async_trait]
impl EchoService for EchoServiceImpl {
    type BidirectionalEchoStream = Pin<Box<dyn Stream<Item = Result<EchoResponse, Status>> + Send>>;
    type ServerStreamingEchoStream = ReceiverStream<Result<EchoResponse, Status>>;

    async fn unary_echo(
        &self,
        request: Request<EchoRequest>,
    ) -> Result<Response<EchoResponse>, Status> {
        Ok(Response::new(EchoResponse {
            message: request.into_inner().message,
        }))
    }

    async fn server_streaming_echo(
        &self,
        request: Request<EchoRequest>,
    ) -> Result<Response<Self::ServerStreamingEchoStream>, Status> {
        let msg = request.into_inner().message;
        let (tx, rx) = mpsc::channel(4);

        tokio::spawn(async move {
            for i in 0..3 {
                let response = EchoResponse {
                    message: format!("{} - seq {}", msg, i),
                };
                tx.send(Ok(response)).await.ok();
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn client_streaming_echo(
        &self,
        request: Request<Streaming<EchoRequest>>,
    ) -> Result<Response<EchoResponse>, Status> {
        let mut stream = request.into_inner();
        let mut full_msg = String::new();

        while let Some(req) = stream.next().await {
            let req = req?;
            full_msg.push_str(&req.message);
        }

        Ok(Response::new(EchoResponse { message: full_msg }))
    }

    async fn bidirectional_echo(
        &self,
        request: Request<Streaming<EchoRequest>>,
    ) -> Result<Response<Self::BidirectionalEchoStream>, Status> {
        let mut in_stream = request.into_inner();
        let (tx, rx) = mpsc::channel(128);

        tokio::spawn(async move {
            while let Some(result) = in_stream.next().await {
                match result {
                    Ok(req) => {
                        let resp = EchoResponse {
                            message: format!("echo: {}", req.message),
                        };
                        if tx.send(Ok(resp)).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e)).await;
                        break;
                    }
                }
            }
        });

        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }
}
