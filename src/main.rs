use tokio::net::UdpSocket;
use tokio::prelude::*;
use std::net;
extern crate clap;
use clap::{Arg, App, SubCommand};
use std::collections::HashSet;



struct PioNode {
    port: u16,
    sock: UdpSocket,
    peers: HashSet<net::SocketAddr>,
}

impl PioNode {
    pub async fn new(port: u16) -> Self {
        println!("Listening at 127.0.0.1:{}", port);
        let sock = UdpSocket::bind((net::Ipv4Addr::new(127, 0, 0, 1), port)).await.unwrap();
        Self {
            port, sock, peers: Default::default()
        }
    }


    pub async fn start(&mut self) {
        let mut buf = [0u8; 0xFFFF]; // maximum udp dgram size

        loop {
            let (len, peer) = self.sock.recv_from(&mut buf).await.unwrap();
            if (!self.peers.contains(&peer)) {
                println!("Not in the peer set!\n");
                continue;
            }

            println!("you are in the peer set!\n");
            dbg!(len, &buf[0..len], peer);

        }
    }

    pub fn add_peer(&mut self, peer: net::SocketAddr) {
        self.peers.insert(peer);
    }
}


#[tokio::main]
async fn main()  {

     let matches = App::new("Power over Ethernet Node")
                          .version("1.0")
                          .author("Just a bunch of dumbies")
                          .about("Mesh communication and packet bouncing")
                          .arg(Arg::with_name("port")
                               .short("p")
                               .long("port")
                               .help("Sets a port to listen from")
                               .required(true)
                               .takes_value(true))
                          .arg(Arg::with_name("peers")
                               .short("c")
                               .long("peers")
                               .help("A CSV list of topologically adjacent servers")
                               .required(true)
                               .takes_value(true))
                          .get_matches();


    let port = matches.value_of("port").unwrap().parse::<u16>().expect("Port is not an integer");

    let mut node = PioNode::new(port).await;


    let peer_strings = matches.value_of("peers").unwrap().split(',').collect::<Vec<&str>>();

    for s in peer_strings {
        let addr: net::SocketAddr = s.parse().unwrap();
        node.add_peer(addr);
    }

    node.start().await;

}
