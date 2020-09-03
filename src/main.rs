use tokio::net::UdpSocket;
use tokio::prelude::*;


#[tokio::main]
async fn main()  {

    let mut socket = UdpSocket::bind("127.0.0.1:42069").await.unwrap();

    let mut buf = vec![0u8; 4096];


    loop {
        let (len, peer) = socket.recv_from(&mut buf).await.unwrap();
        dbg!(len, &buf[0..len], peer);

    }
}
