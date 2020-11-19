use std::{collections::HashSet, fmt::Debug, net::SocketAddr};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum Operation {
    Broadcast {
        seen: HashSet<SocketAddr>,
        hops: u16,
    },
    Directed {
        target: SocketAddr,
    },
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum Payload<T> {
    Message(T),
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Packet<T> {
    pub id: Uuid,
    pub sender: SocketAddr,
    pub op: Operation,
    pub payload: Payload<T>,
}

impl<T> Packet<T> {
    #[allow(dead_code)]
    pub fn new(op: Operation, sender: SocketAddr, payload: Payload<T>) -> Self {
        Self {
            id: Uuid::new_v4(),
            sender,
            op,
            payload,
        }
    }
}

pub trait SanePayload:
    Clone + Send + Sync + Serialize + DeserializeOwned + Debug + 'static
{
}
impl<T: Clone + Send + Sync + Serialize + DeserializeOwned + Debug + 'static> SanePayload for T {}
