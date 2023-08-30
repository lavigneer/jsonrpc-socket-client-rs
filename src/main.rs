use std::time::Duration;

use codec::{RPCDelimiterCodec, RPCDelimiterCodecError};
use futures_util::sink::SinkExt;
use futures_util::stream::{SplitSink, SplitStream, StreamExt};
use futures_util::FutureExt;
use jsonrpsee_core::client::{Client, ClientBuilder};
use jsonrpsee_core::client::{ClientT, IdKind};
use jsonrpsee_core::params::ArrayParams;
use jsonrpsee_core::rpc_params;
use jsonrpsee_core::{
    async_trait,
    client::{ReceivedMessage, TransportReceiverT, TransportSenderT},
};
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio_util::codec::Framed;

mod codec;

pub struct Sender(SplitSink<Framed<TcpStream, RPCDelimiterCodec>, String>);
pub struct Receiver(SplitStream<Framed<TcpStream, RPCDelimiterCodec>>);

#[derive(Debug, thiserror::Error)]
pub enum SocketErr {
    /// Sender went away
    #[error("Sender went away couldn't receive the message")]
    SenderDisconnected,

    #[error("Could not connect to socket endpoint")]
    CannotConnect,

    #[error("Socket Error: {0:?}")]
    SendError(RPCDelimiterCodecError),

    #[error("Socket Error: {0:?}")]
    ReceiveError(RPCDelimiterCodecError),
}

#[async_trait()]
impl TransportSenderT for Sender {
    #[doc = " Error that may occur during sending a message."]
    type Error = RPCDelimiterCodecError;

    #[doc = " Send."]
    async fn send(&mut self, msg: String) -> Result<(), Self::Error> {
        self.0.send(msg).await?;
        Ok(())
    }
}

#[async_trait()]
impl TransportReceiverT for Receiver {
    #[doc = " Error that may occur during receiving a message."]
    type Error = SocketErr;

    #[doc = " Receive."]
    async fn receive(&mut self) -> Result<ReceivedMessage, Self::Error> {
        match self.0.next().await {
            Some(Ok(msg)) => Ok(ReceivedMessage::Bytes(msg.to_vec())),
            Some(Err(err)) => Err(SocketErr::ReceiveError(err)),
            None => Err(SocketErr::SenderDisconnected),
        }
    }
}

pub async fn connect(url: impl ToSocketAddrs) -> Result<(Sender, Receiver), SocketErr> {
    let stream = TcpStream::connect(url)
        .await
        .map_err(|_err| SocketErr::SenderDisconnected)?;
    let codec = RPCDelimiterCodec::default();
    let framed_sock = Framed::new(stream, codec);
    let (frame_sink, frame_stream) = framed_sock.split::<String>();
    Ok((Sender(frame_sink), Receiver(frame_stream)))
}

pub struct SocketClientBuilder {
    id_kind: IdKind,
    max_concurrent_requests: usize,
    max_buffer_capacity_per_subscription: usize,
    max_log_length: u32,
    request_timeout: Duration,
}

impl Default for SocketClientBuilder {
    fn default() -> Self {
        Self {
            id_kind: IdKind::Number,
            max_log_length: 4096,
            max_concurrent_requests: 256,
            max_buffer_capacity_per_subscription: 1024,
            request_timeout: Duration::from_secs(60),
        }
    }
}

impl SocketClientBuilder {
    /// Create a new WASM client builder.
    pub fn new() -> SocketClientBuilder {
        SocketClientBuilder::default()
    }

    /// See documentation [`ClientBuilder::request_timeout`] (default is 60 seconds).
    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// See documentation [`ClientBuilder::max_concurrent_requests`] (default is 256).
    pub fn max_concurrent_requests(mut self, max: usize) -> Self {
        self.max_concurrent_requests = max;
        self
    }

    /// See documentation [`ClientBuilder::max_buffer_capacity_per_subscription`] (default is 1024).
    pub fn max_buffer_capacity_per_subscription(mut self, max: usize) -> Self {
        self.max_buffer_capacity_per_subscription = max;
        self
    }

    /// See documentation for [`ClientBuilder::id_format`] (default is Number).
    pub fn id_format(mut self, kind: IdKind) -> Self {
        self.id_kind = kind;
        self
    }

    /// Set maximum length for logging calls and responses.
    ///
    /// Logs bigger than this limit will be truncated.
    pub fn set_max_logging_length(mut self, max: u32) -> Self {
        self.max_log_length = max;
        self
    }

    /// Build the client with specified URL to connect to.
    pub async fn build(self, url: impl ToSocketAddrs) -> Result<Client, jsonrpsee_core::Error> {
        let Self {
            max_log_length,
            id_kind,
            request_timeout,
            max_concurrent_requests,
            max_buffer_capacity_per_subscription,
        } = self;
        let (sender, receiver) = connect(url)
            .await
            .map_err(|e| jsonrpsee_core::Error::Transport(e.into()))?;

        let builder = ClientBuilder::default()
            .set_max_logging_length(max_log_length)
            .request_timeout(request_timeout)
            .id_format(id_kind)
            .max_buffer_capacity_per_subscription(max_buffer_capacity_per_subscription)
            .max_concurrent_requests(max_concurrent_requests);

        Ok(builder.build_with_tokio(sender, receiver))
    }
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = "192.168.2.213:12000";
    let builder = SocketClientBuilder::new();
    let client = builder.build(url).await?;

    // Throwaway call to force the request id to 1 for real calls
    client
        .request::<String, ArrayParams>("", rpc_params![])
        .now_or_never();

    let response = client
        .request::<Vec<String>, ArrayParams>("list.name.sets", rpc_params![])
        .await?;
    println!("{:?}", response);

    Ok(())
}
