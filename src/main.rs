use std::{
    collections::{HashMap, HashSet},
    io::{self, Write},
    marker::PhantomData,
    net,
    time::Duration,
};

use lru::LruCache;
use tokio::{io::AsyncBufReadExt, net::TcpListener, sync::mpsc, time};
use uuid::Uuid;

mod cli;
mod peer;
mod proto;

struct Node<M> {
    listener: TcpListener,
    peers: HashMap<net::SocketAddr, peer::Peer<M>>,
    known_peers: HashSet<net::SocketAddr>,
    port: u16,

    phantom: PhantomData<M>,
}

impl<M: proto::SanePayload> Node<M> {
    pub async fn new(port: u16) -> Self {
        println!("Listening at 127.0.0.1:{}", port);
        let listener = TcpListener::bind((net::Ipv4Addr::new(127, 0, 0, 1), port))
            .await
            .unwrap();
        Self {
            listener,
            peers: Default::default(),
            known_peers: Default::default(),
            port,
            phantom: PhantomData,
        }
    }

    /*
    pub async fn recv_packet(&mut self) -> (proto::Packet<M>, net::SocketAddr) {
        loop {
            let mut buf = [0u8; 0xFFFF]; // maximum udp dgram size
            let (len, peer) = self.sock.recv_from(&mut buf).await.unwrap();
            if !self.peers.contains_key(&peer) {
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
    */

    async fn recv_from_socket(
        addr: net::SocketAddr,
        peer: &peer::Peer<M>,
    ) -> io::Result<(net::SocketAddr, proto::Payload<M>)> {
        let msg = peer.recv_packet().await?;
        Ok((addr, msg))
    }

    pub fn start(mut self) -> RunningNode<M> {
        let (metatx, mut metarx) = mpsc::channel::<MetaCommand<M>>(100);

        let handle = tokio::spawn(async move {
            // 100 messages
            let _seen_msg_ids = LruCache::<Uuid, ()>::new(100);

            // send a heartbeat every second
            let mut heartbeat = time::interval(Duration::from_millis(1000));
            let _my_addr =
                net::SocketAddr::new(net::IpAddr::V4(net::Ipv4Addr::new(127, 0, 0, 1)), self.port);

            let mut active = true;
            while active {
                let mut new_peers = Vec::<(net::SocketAddr, peer::Peer<M>)>::new();

                {
                    let recvs = self
                        .peers
                        .iter()
                        .map(|(addr, peer)| Box::pin(Self::recv_from_socket(addr.clone(), peer)))
                        .collect::<Vec<_>>();

                    tokio::select! {
                        _ = heartbeat.tick() => {
                            self.broadcast(proto::Payload::Heartbeat).await;
                        },
                        Ok((stream, addr)) = self.listener.accept() => {
                            if self.known_peers.contains(&addr) {
                                new_peers.push((addr, peer::Peer::new(stream)));
                            }
                        }

                        (Ok((_addr, _payload)), _, _) = futures::future::select_all(recvs) => {

                        }
                        /*
                        (packet, _peer) = self.recv_packet() => {

                            if seen_msg_ids.contains(&packet.id) {
                                continue;
                            }
                            seen_msg_ids.put(packet.id, ());

                            // switch over the payload data
                            match packet.payload {
                                proto::Payload::Heartbeat => {
                                },
                                proto::Payload::Message(m) => {
                                    // what kind of message operation was it?
                                    match packet.op {
                                        proto::Operation::Broadcast(mut seen, hops) => {
                                            // if I have already seen this message, skip it
                                            if seen.contains(&my_addr) {
                                                continue;
                                            }
                                            println!("[{}] got msg {} '{:?}' {} hops", self.port, packet.id, m, hops);
                                            // insert myself into the
                                            seen.insert(my_addr.clone());
                                            let pkt = proto::Packet {
                                                id: packet.id,
                                                op: proto::Operation::Broadcast(seen.clone(), hops + 1),
                                                payload: proto::Payload::Message(m)
                                            };
                                            let encoded = bincode::serialize(&pkt).unwrap();
                                            for peer in self.peers.keys() {
                                                if seen.contains(peer) {
                                                    continue;
                                                }
                                                self.sock.send_to(&encoded, peer).await.unwrap();
                                            }
                                        },
                                        proto::Operation::Targetted(_) => {
                                            println!("Targetted message!");
                                        }
                                    }
                                },
                            }

                        },
                        */
                        meta = metarx.recv() => {

                            match meta.unwrap() {
                                MetaCommand::Die => {
                                    active = false;
                                    println!("Node Terminating");
                                },
                                MetaCommand::Broadcast(msg) => {
                                    // println!("Told to broadcast '{:?}'", msg);
                                    self.broadcast(proto::Payload::Message(msg)).await;
                                }
                            }
                        },
                    }
                }
                for (addr, peer) in new_peers {
                    self.peers.insert(addr, peer);
                }
            }
        });

        return RunningNode { handle, tx: metatx };
    }

    /// Send a msg to each node in the peer set and returns a set of
    pub async fn broadcast(&self, payload: proto::Payload<M>) -> Vec<(net::SocketAddr, io::Error)> {
        let mut errs = vec![];

        for (addr, peer) in &self.peers {
            if let Err(e) = peer.send_packet(&payload).await {
                errs.push((*addr, e));
            }
        }

        return errs;
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

impl<M: proto::SanePayload> RunningNode<M> {
    pub async fn wait(self) {
        self.handle.await.unwrap();
    }

    pub async fn terminate(mut self) {
        let _ = self.tx.send(MetaCommand::<M>::Die).await;
        self.wait().await;
    }

    pub async fn broadcast(&mut self, msg: M) {
        let _ = self.tx.send(MetaCommand::<M>::Broadcast(msg)).await;
    }
}

#[tokio::main]
async fn main() {
    let matches = cli::build_cli().get_matches();

    let port = matches
        .value_of("port")
        .unwrap()
        .parse::<u16>()
        .expect("Port is not an integer");

    let mut node: Node<String> = Node::new(port).await;

    let peer_strings = matches.value_of("peers").unwrap().split(',');

    for s in peer_strings {
        let addr: net::SocketAddr = s.parse().unwrap();
        node.known_peers.insert(addr);
    }

    let mut node = node.start();

    // run the interactive shell if we need to

    if matches.is_present("shell") {
        let mut reader = tokio::io::BufReader::new(tokio::io::stdin());
        loop {
            print!(">>> ");
            std::io::stdout().flush().unwrap(); // ugh
            let mut buffer = String::new();
            if let Ok(_) = reader.read_line(&mut buffer).await {
                let line = buffer.trim();
                let parts = line.split_ascii_whitespace().collect::<Vec<&str>>();
                if parts.len() == 0 {
                    continue;
                }

                match parts[0] {
                    "exit" => {
                        break;
                    }
                    "b" => {
                        let msg = parts[1..].join(" ");
                        node.broadcast(msg).await;
                    }
                    _ => println!("Unknown '{}'", line),
                };
            } else {
                break;
            }
        }
        node.terminate().await;
    } else {
        // otherwise wait for the node to finish
        node.wait().await;
    }
}
