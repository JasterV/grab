use echo_service::EchoService;
use echo_service::pb::{EchoRequest, EchoResponse};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};

// A minimal service that satisfies the EchoService trait.
// We don't need real logic here, just enough to compile.
pub struct DummyEchoService;

#[tonic::async_trait]
impl EchoService for DummyEchoService {
    type ServerStreamingEchoStream = ReceiverStream<Result<EchoResponse, Status>>;
    type BidirectionalEchoStream = ReceiverStream<Result<EchoResponse, Status>>;

    async fn unary_echo(
        &self,
        _req: Request<EchoRequest>,
    ) -> Result<Response<EchoResponse>, Status> {
        unimplemented!("This will never be used")
    }

    async fn server_streaming_echo(
        &self,
        _req: Request<EchoRequest>,
    ) -> Result<Response<Self::ServerStreamingEchoStream>, Status> {
        unimplemented!("This will never be used")
    }

    async fn client_streaming_echo(
        &self,
        _req: Request<Streaming<EchoRequest>>,
    ) -> Result<Response<EchoResponse>, Status> {
        unimplemented!("This will never be used")
    }

    async fn bidirectional_echo(
        &self,
        _req: Request<Streaming<EchoRequest>>,
    ) -> Result<Response<Self::BidirectionalEchoStream>, Status> {
        unimplemented!("This will never be used")
    }
}
