/// structures and methods for communication over a Peer connection
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashSet;
use std::net;
use uuid::Uuid;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum Operation {
    Broadcast(
        /* blacklist */ HashSet<net::SocketAddr>,
        /* hops */ u16,
    ),
    Targetted(/* Target */ net::SocketAddr),
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum Payload<T> {
    Heartbeat,
    Message(T),
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Packet<T> {
    pub id: Uuid,
    pub op: Operation,
    pub payload: Payload<T>,
}

impl<T> Packet<T> {
    pub fn new(op: Operation, payload: Payload<T>) -> Self {
        Self {
            id: Uuid::new_v4(),
            op,
            payload,
        }
    }
}

pub trait SanePayload = Send + Sync + Serialize + DeserializeOwned + std::fmt::Debug + 'static;
