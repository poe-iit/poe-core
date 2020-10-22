use std::{
    collections::{HashMap, HashSet},
    marker::PhantomData,
    net::{Ipv4Addr, SocketAddr},
};

use crate::{
    peer::Peer,
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
    port: u16,
    peers: HashMap<SocketAddr, Peer<M>>,
    pub(super) known_peers: HashSet<SocketAddr>,
    inbound_packets: mpsc::Receiver<Packet<M>>,
    tx: mpsc::Sender<Packet<M>>,
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
            port,
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
            let peer = Peer::new(stream, self.tx.clone());
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
                    if self.seen_msgs.contains(&pkt.id) {
                        continue;
                    }
                    self.seen_msgs.put(pkt.id, ());
                    match pkt.payload {
                        Payload::Message(m) => {
                            // what kind of message operation was it?
                            match pkt.op {
                                #[allow(unreachable_code)]
                                Operation::Broadcast { mut seen, hops } => {
                                    // TODO: get a real address and determine it at a higher scope
                                    let my_addr = SocketAddr::from(([127, 0, 0, 1], self.port));
                                    // if I have already seen this message, skip it
                                    if seen.contains(&my_addr) {
                                        continue;
                                    }
                                    println!("[{}] got msg {} '{:?}' {} hops", self.port, pkt.id, m, hops);
                                    seen.insert(my_addr);
                                    let new_pkt = Packet {
                                        id: pkt.id,
                                        op: Operation::Broadcast { seen, hops: hops + 1 },
                                        payload: Payload::Message(m)
                                    };
                                    self.broadcast(new_pkt).await;
                                },
                                Operation::Directed { .. } => {
                                    todo!("directed message!");
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn start(self) -> RunningNode<M> {
        let (metatx, metarx) = mpsc::channel(META_CHAN_CAPACITY);
        let handle = tokio::spawn(self.run(metarx));
        RunningNode { handle, tx: metatx }
    }

    /// Send a msg to each node in the peer set and returns a set of
    pub async fn broadcast(&mut self, payload: Packet<M>) -> HashMap<SocketAddr, tokio::io::Error> {
        let mut errs = HashMap::new();
        for (addr, peer) in &mut self.peers {
            if let Err(e) = peer.send_packet(&payload).await {
                errs.insert(*addr, e);
            }
        }
        errs
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
