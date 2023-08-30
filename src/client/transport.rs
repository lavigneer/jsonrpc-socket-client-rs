use futures_util::sink::SinkExt;
use futures_util::stream::{SplitSink, SplitStream, StreamExt};
use jsonrpsee_core::{
    async_trait,
    client::{ReceivedMessage, TransportReceiverT, TransportSenderT},
};
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio_util::codec::Framed;

use super::codec::{RPCDelimiterCodec, RPCDelimiterCodecError};

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
