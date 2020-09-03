use tokio::net::UdpSocket;
use std::net;
extern crate clap;
use clap::{Arg, App};
use std::collections::HashSet;
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use tokio::time;
use std::time::Duration;
use std::sync::{ Mutex};



#[derive(Serialize, Deserialize, PartialEq, Debug)]
enum Operation {
    Broadcast(/* blacklist */ HashSet<net::SocketAddr>),
    Targetted(/* Target */net::SocketAddr),
}


#[derive(Serialize, Deserialize, PartialEq, Debug)]
enum Payload<T> {
    Heartbeat,
    Message(T)
}



#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Packet<T>(Operation, Payload<T>);




struct Node<M> {
    sock: UdpSocket,
    peers: HashSet<net::SocketAddr>,

    phantom: std::marker::PhantomData<M>,
}

impl<M: std::marker::Sync + std::marker::Send + Serialize + DeserializeOwned + std::fmt::Debug> Node<M> {
    pub async fn new(port: u16) -> Self {
        println!("Listening at 127.0.0.1:{}", port);
        let sock = UdpSocket::bind((net::Ipv4Addr::new(127, 0, 0, 1), port)).await.unwrap();
        Self {
            sock, peers: Default::default(),
            phantom: std::marker::PhantomData,
        }
    }


    pub async fn recv_packet(&mut self) -> (Packet<M>, net::SocketAddr) {
        loop {
            let mut buf = [0u8; 0xFFFF]; // maximum udp dgram size
            let (len, peer) = self.sock.recv_from(&mut buf).await.unwrap();
            if !self.peers.contains(&peer) {
                println!("Not in the peer set!\n");
                continue;
            }

            match bincode::deserialize(&buf[0..len]) {
                Ok(pkt) => {
                    return (pkt, peer);
                }
                Err(_) => {
                    continue;
                }
            }
        }
    }


    pub async fn start(&mut self) {
        let mut heartbeat = time::interval(Duration::from_millis(100));

        for i in 0.. {
            tokio::select! {
                _ = heartbeat.tick() => {
                    self.broadcast(Payload::Heartbeat).await;
                },
                (packet, peer) = self.recv_packet() => {
                    println!("{} from {}: {:?}", i, peer, packet);
                }
            }
        }

    }

    /// Send a msg to each node in the peer set
    pub async fn broadcast(&mut self, payload: Payload<M>) {
        let pkt = Packet(Operation::Broadcast(self.peers.clone()), payload);
        let encoded = bincode::serialize(&pkt).unwrap();
        // TODO: do this all async like ;^}
        for peer in &self.peers {
            self.sock.send_to(&encoded, peer).await.unwrap();
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

    let mut node: Node<String> = Node::new(port).await;


    let peer_strings = matches.value_of("peers").unwrap().split(',');

    for s in peer_strings {
        let addr: net::SocketAddr = s.parse().unwrap();
        node.add_peer(addr);
    }

    node.start().await;

}
