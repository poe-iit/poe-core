use std::marker::PhantomData;
use tokio::sync::Mutex;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

pub struct Peer<M> {
    read_stream: Mutex<tokio::io::ReadHalf<TcpStream>>,
    write_stream: Mutex<tokio::io::WriteHalf<TcpStream>>,
    phantom: PhantomData<M>,
}

impl<M: super::proto::SanePayload> Peer<M> {
    pub fn new(stream: TcpStream) -> Self {
        let (read, write) = tokio::io::split(stream);
        Self {
            read_stream: Mutex::new(read),
            write_stream: Mutex::new(write),
            phantom: PhantomData,
        }
    }

    pub async fn recv_packet(&self) -> tokio::io::Result<super::proto::Payload<M>> {
        let mut stream = self.read_stream.lock().await;
        let msg_len = stream.read_u64().await?;
        let mut buf = vec![0u8; msg_len as usize];
        stream.read_exact(&mut buf[..]).await?;
        let pkt = bincode::deserialize(&buf[..]).expect("TODO: handle invalid packet data");
        Ok(pkt)
    }

    pub async fn send_packet(&self, packet: &super::proto::Payload<M>) -> tokio::io::Result<()> {
        let mut stream = self.write_stream.lock().await;

        let buf = bincode::serialize(&packet).expect("TODO: handle serialization failures");
        stream.write_u64(buf.len() as u64).await?;
        stream.write_all(&buf[..]).await?;
        stream.flush().await?;
        Ok(())
    }
}
