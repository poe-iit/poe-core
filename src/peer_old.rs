use crate::proto;

use tokio::io::AsyncWriteExt;

/// A Peer is a connection to another node in the network. It is
/// able to send and recv packets asynchronously and all that cool jazz
pub struct Peer<T> {
    stream: tokio::net::TcpStream,

    phantom: std::marker::PhantomData<T>,
}

impl<T: proto::SaneMessage> Peer<T> {
    /// Open a tcp connection and return a peer if it connected.
    pub async fn connect(ip: std::net::SocketAddr) -> Option<Self> {
        return match tokio::net::TcpStream::connect(&ip).await {
            Err(_) => None,
            Ok(c) => Some(Self::create(c)),
        };
    }

    /// Create a peer connection from a tcp connection
    pub fn create(stream: tokio::net::TcpStream) -> Self {
        Self {
            stream,
            phantom: std::marker::PhantomData,
        }
    }

    /// encode and send a packet over the peer, returning if it succeeded or not
    pub async fn send_packet(&mut self, payload: proto::Payload<T>) -> bool {
        let encoded = bincode::serialize(&payload).unwrap();
        let res = self.stream.write(&encoded).await;
        return res.is_ok();
    }
}
