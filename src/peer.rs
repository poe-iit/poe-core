use std::marker::PhantomData;

use crate::proto::{Payload, SanePayload};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf},
    net::TcpStream,
    sync::mpsc,
};

pub struct Peer<M> {
    stream: WriteHalf<TcpStream>,
    phantom: PhantomData<M>,
}

impl<M: SanePayload> Peer<M> {
    pub fn new(stream: TcpStream, tx: mpsc::Sender<Payload<M>>) -> Self {
        let (read, write) = tokio::io::split(stream);
        let rcvr = Receiver::new(read);
        tokio::spawn(rcvr.recv_into_chan(tx));
        Self {
            stream: write,
            phantom: PhantomData,
        }
    }

    pub async fn send_packet(&mut self, packet: &Payload<M>) -> tokio::io::Result<()> {
        let buf = bincode::serialize(&packet).expect("TODO: handle serialization failures");
        self.stream.write_u64(buf.len() as u64).await?;
        self.stream.write_all(&buf[..]).await?;
        self.stream.flush().await?;
        Ok(())
    }
}

struct Receiver<M> {
    stream: ReadHalf<TcpStream>,
    phantom: PhantomData<M>,
}

impl<M: SanePayload> Receiver<M> {
    fn new(stream: ReadHalf<TcpStream>) -> Self {
        Self {
            stream,
            phantom: PhantomData,
        }
    }

    async fn recv_packet(&mut self) -> tokio::io::Result<Payload<M>> {
        let msg_len = self.stream.read_u64().await?;
        let mut buf = vec![0u8; msg_len as usize];
        self.stream.read_exact(&mut buf[..]).await?;
        let pkt = bincode::deserialize(&buf[..]).expect("TODO: handle invalid packet data");
        Ok(pkt)
    }

    async fn recv_into_chan(mut self, mut tx: mpsc::Sender<Payload<M>>) -> tokio::io::Result<()> {
        loop {
            let pkt = self.recv_packet().await?;
            if tx.send(pkt).await.is_err() {
                break;
            }
        }
        Ok(())
    }
}
