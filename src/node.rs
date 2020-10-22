use std::{
    collections::{HashMap, HashSet},
    marker::PhantomData,
    net::{Ipv4Addr, SocketAddr},
};

use crate::{
    peer,
    proto::{Operation, Packet, Payload, SanePayload},
};

use lru::LruCache;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
};
use uuid::Uuid;

const MSG_CHAN_CAPACITY: usize = 128;
const SEEN_CACHE_CAPACITY: usize = 128;
const META_CHAN_CAPACITY: usize = 16;

pub struct Node<M> {
    listener: TcpListener,
    peers: HashMap<SocketAddr, peer::Peer<M>>,
    pub(super) known_peers: HashSet<SocketAddr>,
    inbound_packets: mpsc::Receiver<Packet<M>>,
    tx: mpsc::Sender<Packet<M>>,
    #[allow(dead_code)]
    seen_msgs: LruCache<Uuid, ()>,
    phantom: PhantomData<M>,
}

impl<M: SanePayload> Node<M> {
    pub async fn new(port: u16) -> Self {
        println!("Listening at 127.0.0.1:{}", port);
        let listener = TcpListener::bind((Ipv4Addr::new(127, 0, 0, 1), port))
            .await
            .unwrap();
        let (tx, rx) = mpsc::channel(MSG_CHAN_CAPACITY);
        Self {
            listener,
            peers: Default::default(),
            known_peers: Default::default(),
            inbound_packets: rx,
            seen_msgs: LruCache::new(SEEN_CACHE_CAPACITY),
            tx,
            phantom: PhantomData,
        }
    }

    fn add_peer(&mut self, stream: TcpStream, addr: SocketAddr) {
        if self.known_peers.contains(&addr) {
            let peer = peer::Peer::new(stream, self.tx.clone());
            self.peers.insert(addr, peer);
        }
    }

    async fn run(mut self, mut metarx: mpsc::Receiver<MetaCommand<M>>) {
        loop {
            tokio::select! {
                new_peer = self.listener.accept() => {
                    match new_peer {
                        Ok((stream, addr)) => self.add_peer(stream, addr),
                        Err(e) => panic!("TcpListener::accept failed: {}", e),
                    }
                }
                meta = metarx.recv() => {
                    match meta.expect("meta channel closed") {
                        MetaCommand::Die => {
                            println!("Node Terminating");
                            break;
                        },
                        MetaCommand::Broadcast(msg) => {
                            println!("Told to broadcast '{:?}'", msg);
                            let payload = Payload::Message(msg);
                            let op = Operation::Broadcast {
                                seen: HashSet::new(),
                                hops: 0, // ???
                            };
                            let packet = Packet::new(op, payload);
                            self.broadcast(packet).await;
                        }
                    }
                }
                pkt = self.inbound_packets.recv() => {
                    let pkt = pkt.expect("no senders???");
                    todo!("do something with this packet: {:?}", pkt);
                }
            }
        }
    }

    pub fn start(self) -> RunningNode<M> {
        let (metatx, metarx) = mpsc::channel(META_CHAN_CAPACITY);
        let handle = tokio::spawn(self.run(metarx));
        RunningNode { handle, tx: metatx }
    }

    /*
        let _my_addr =
            net::SocketAddr::new(net::IpAddr::V4(net::Ipv4Addr::new(127, 0, 0, 1)), self.port);

            tokio::select! {
            (packet, _peer) = self.recv_packet() => {
                if seen_msg_ids.contains(&packet.id) {
                    continue;
                }
                seen_msg_ids.put(packet.id, ());
                // switch over the payload data
                match packet.payload {
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
    }
    */

    /// Send a msg to each node in the peer set and returns a set of
    pub async fn broadcast(&mut self, payload: Packet<M>) -> Vec<(SocketAddr, tokio::io::Error)> {
        let mut errs = vec![];

        for (addr, peer) in &mut self.peers {
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

pub struct RunningNode<M> {
    handle: tokio::task::JoinHandle<()>,
    tx: mpsc::Sender<MetaCommand<M>>,
}

impl<M: SanePayload> RunningNode<M> {
    pub async fn wait(self) {
        self.handle.await.unwrap();
    }

    pub async fn terminate(mut self) {
        let _ = self.tx.send(MetaCommand::Die).await;
        self.wait().await;
    }

    pub async fn broadcast(&mut self, msg: M) {
        let _ = self.tx.send(MetaCommand::Broadcast(msg)).await;
    }
}
