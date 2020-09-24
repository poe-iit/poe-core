use std::marker::PhantomData;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

pub struct Peer<M> {
    stream: TcpStream,
    phantom: PhantomData<M>,
}

impl<M: super::proto::SaneMessage> Peer<M> {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            phantom: PhantomData,
        }
    }

    pub async fn recv_packet(&mut self) -> tokio::io::Result<M> {
        let msg_len = self.stream.read_u64().await?;
        let mut buf = vec![0u8; msg_len as usize];
        self.stream.read_exact(&mut buf[..]).await?;
        let pkt = bincode::deserialize(&buf[..]).expect("TODO: handle invalid packet data");
        Ok(pkt)
    }

    pub async fn send_packet(&mut self, packet: M) -> tokio::io::Result<()> {
        let buf = bincode::serialize(&packet).expect("TODO: handle serialization failures");
        self.stream.write_u64(buf.len() as u64).await?;
        self.stream.write_all(&buf[..]).await?;
        self.stream.flush().await?;
        Ok(())
    }
}
