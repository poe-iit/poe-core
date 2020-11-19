use std::{
    collections::{HashMap, HashSet},
    marker::PhantomData,
    net::{IpAddr, Ipv4Addr, SocketAddr},
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
    // pub(super) known_peers: HashSet<SocketAddr>,
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

        /*
        // Load public certificate.
        let certs = load_certs("keys/key.cert").unwrap();
        // Load private key.
        let key = load_private_key("keys/key.pkey").unwrap();
        // Do not use client certificate authentication.
        let mut cfg = rustls::ServerConfig::new(rustls::NoClientAuth::new());
        // Select a certificate to use.
        cfg.set_single_cert(certs, key).unwrap();

        let acceptor = TlsAcceptor::from(Arc::new(cfg));
        */

        Self {
            listener,
            // acceptor,
            port,
            peers: Default::default(),
            // known_peers: Default::default(),
            inbound_packets: rx,
            seen_msgs: LruCache::new(SEEN_CACHE_CAPACITY),
            tx,
            phantom: PhantomData,
        }
    }

    fn add_peer(&mut self, stream: TcpStream, addr: SocketAddr) {
        let peer = Peer::new(stream, self.tx.clone());
        self.peers.insert(addr, peer);
    }

    async fn run(
        mut self,
        mut metarx: mpsc::Receiver<MetaCommand<M>>,
        datatx: mpsc::Sender<(M, SocketAddr)>,
    ) {
        loop {
            tokio::select! {
                new_peer = self.listener.accept() => {
                    match new_peer {
                        Ok((stream, addr)) => {
                            // println!("accept from {}!", addr);
                            self.add_peer(stream /* .into() */, addr);
                        },
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
                            let mut seen = HashSet::new();
                            seen.insert(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), self.port));

                            let op = Operation::Broadcast {
                                seen,
                                hops: 0, // ???
                            };


                            let my_addr = SocketAddr::from(([127, 0, 0, 1], self.port));
                            let packet = Packet::new(op, my_addr, payload);
                            self.broadcast(packet).await;
                        },
                        MetaCommand::AddPeer(stream, addr) => {
                            self.add_peer(stream, addr);
                        }
                    }
                }
                pkt = self.inbound_packets.recv() => {
                    let pkt = pkt.expect("no senders???");
                    self.handle_packet(pkt, &datatx).await;
                }
            }
        }
    }

    async fn handle_packet(&mut self, pkt: Packet<M>, datatx: &mpsc::Sender<(M, SocketAddr)>) {
        if self.seen_msgs.contains(&pkt.id) {
            return;
        } else {
            self.seen_msgs.put(pkt.id, ());
        }

        match pkt.payload {
            Payload::Message(m) => {
                // what kind of message operation was it?
                match pkt.op {
                    Operation::Broadcast { mut seen, hops } => {
                        // TODO: get a real address and determine it earlier
                        let my_addr = SocketAddr::from(([127, 0, 0, 1], self.port));
                        // if I have already seen this message, skip it
                        if seen.contains(&my_addr) {
                            return;
                        }
                        println!("[{}] got msg {} '{:?}' {} hops", self.port, pkt.id, m, hops);
                        seen.insert(my_addr);
                        let new_pkt = Packet {
                            id: pkt.id,
                            sender: pkt.sender.clone(),
                            op: Operation::Broadcast {
                                seen,
                                hops: hops + 1,
                            },
                            payload: Payload::Message(m.clone()),
                        };
                        self.broadcast(new_pkt).await;
                    }
                    Operation::Directed { .. } => {
                        todo!("directed message!");
                    }
                }

                datatx.send((m, pkt.sender)).await.expect("I am afraid");
            }
        }
    }

    pub fn start(self) -> RunningNode<M> {
        let (metatx, metarx) = mpsc::channel(META_CHAN_CAPACITY);
        let (datatx, datarx) = mpsc::channel(MSG_CHAN_CAPACITY);
        let handle = tokio::spawn(self.run(metarx, datatx));
        RunningNode {
            handle,
            tx: metatx,
            rx: datarx,
        }
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

pub enum MetaCommand<M> {
    Die,
    Broadcast(M),
    AddPeer(TcpStream, SocketAddr),
}

pub struct RunningNode<M> {
    handle: tokio::task::JoinHandle<()>,
    tx: mpsc::Sender<MetaCommand<M>>,
    rx: mpsc::Receiver<(M, SocketAddr)>,
}

impl<M: SanePayload> RunningNode<M> {
    pub async fn wait(self) {
        self.handle.await.unwrap();
    }

    pub async fn terminate(self) {
        let _ = self.tx.send(MetaCommand::Die).await;
        self.wait().await;
    }

    pub async fn broadcast(&mut self, msg: M) {
        let _ = self.tx.send(MetaCommand::Broadcast(msg)).await;
    }

    pub async fn send_cmd(&mut self, cmd: MetaCommand<M>) {
        let _ = self.tx.send(cmd).await;
    }

    pub async fn recv(&mut self) -> Option<(M, SocketAddr)> {
        self.rx.recv().await
    }
}

/*
// Load public certificate from file.
fn load_certs(filename: &str) -> io::Result<Vec<rustls::Certificate>> {
    // Open certificate file.
    let certfile = fs::File::open(filename)?;
    let mut reader = io::BufReader::new(certfile);

    // Load and return certificate.
    Ok(pemfile::certs(&mut reader).unwrap())
}

// Load private key from file.
fn load_private_key(filename: &str) -> io::Result<rustls::PrivateKey> {
    // Open keyfile.
    let keyfile = fs::File::open(filename)?;
    let mut reader = io::BufReader::new(keyfile);

    // Load and return a single private key.
    let keys = pemfile::rsa_private_keys(&mut reader).unwrap();
    println!("len = {}", keys.len());
    if keys.len() != 1 {
        return Err(io::Error::new(io::ErrorKind::Other, "I am afraid"));
    }
    Ok(keys[0].clone())
}
*/
