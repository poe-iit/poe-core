#![feature(trait_alias)]

use std::net;
use tokio::net::UdpSocket;
extern crate clap;
use clap::{App, Arg};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
enum Operation {
    Broadcast(/* blacklist */ HashSet<net::SocketAddr>),
    Targetted(/* Target */ net::SocketAddr),
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
enum Payload<T> {
    Heartbeat,
    Message(T),
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Packet<T>(Operation, Payload<T>);

struct Node<M> {
    sock: UdpSocket,
    peers: HashSet<net::SocketAddr>,

    phantom: std::marker::PhantomData<M>,
}

trait SaneMessage = Send + Sync + Serialize + DeserializeOwned + std::fmt::Debug + 'static;

impl<M: SaneMessage> Node<M> {
    pub async fn new(port: u16) -> Self {
        println!("Listening at 127.0.0.1:{}", port);
        let sock = UdpSocket::bind((net::Ipv4Addr::new(127, 0, 0, 1), port))
            .await
            .unwrap();
        Self {
            sock,
            peers: Default::default(),
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

    pub async fn start(mut self) -> RunningNode<M> {
        let (tx, mut metarx) = mpsc::channel::<MetaCommand<M>>(100);

        let handle = tokio::spawn(async move {
            let mut heartbeat = time::interval(Duration::from_millis(100));

            let mut active = true;
            while active {
                tokio::select! {
                    _ = heartbeat.tick() => {
                        self.broadcast(Payload::Heartbeat).await;
                    },
                    (packet, peer) = self.recv_packet() => {
                        println!("msg from {}: {:?}", peer, packet);
                    },
                    meta = metarx.recv() => {

                        match meta.unwrap() {
                            MetaCommand::Die => {
                                active = false;
                                println!("Node Terminating");
                            },
                            MetaCommand::Broadcast(msg) => {
                                println!("Told to broadcast '{:?}'", msg);
                            }
                        }
                    },
                }
            }
        });

        return RunningNode { handle, tx };
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

enum MetaCommand<M> {
    Die,
    Broadcast(M),
}

struct RunningNode<M> {
    handle: tokio::task::JoinHandle<()>,
    tx: mpsc::Sender<MetaCommand<M>>,
}

impl<M: SaneMessage> RunningNode<M> {
    pub async fn wait(self) {
        self.handle.await.unwrap();
    }

    pub async fn terminate(mut self) {
        let _ = self.tx.send(MetaCommand::<M>::Die).await;
        self.wait().await;
    }
}

#[tokio::main]
async fn main() {
    let matches = App::new("Power over Ethernet Node")
        .version("1.0")
        .author("Just a bunch of dumbies")
        .about("Mesh communication and packet bouncing")
        .arg(
            Arg::with_name("port")
                .short("p")
                .long("port")
                .help("Sets a port to listen from")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("peers")
                .short("c")
                .long("peers")
                .help("A CSV list of topologically adjacent servers")
                .required(true)
                .takes_value(true),
        )
        .get_matches();

    let port = matches
        .value_of("port")
        .unwrap()
        .parse::<u16>()
        .expect("Port is not an integer");

    let mut node: Node<String> = Node::new(port).await;

    let peer_strings = matches.value_of("peers").unwrap().split(',');

    for s in peer_strings {
        let addr: net::SocketAddr = s.parse().unwrap();
        node.add_peer(addr);
    }

    let node = node.start().await;

    // TODO: actually do something

    node.terminate().await;
}
