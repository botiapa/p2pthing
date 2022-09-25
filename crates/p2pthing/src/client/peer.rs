use std::net::SocketAddr;

use p2pthing_common::{encryption::{NetworkedPublicKey, SymmetricEncryption}, enumset::{EnumSetType, EnumSet}};
use serde::{Serialize, Deserialize};

use super::udp_connection::UdpConnection;

#[derive(EnumSetType, Serialize, Deserialize, Debug)]
pub enum PeerSource {
    Multicast,
    Rendezvous
}

#[derive(Debug)]
pub struct Peer {
    pub addr: Option<SocketAddr>,
    pub udp_addr: Option<SocketAddr>,
    pub udp_conn: Option<UdpConnection>,
    pub public_key: NetworkedPublicKey,
    pub source: EnumSet<PeerSource>,
    pub sym_key: Option<SymmetricEncryption>
}

impl PartialEq for Peer {
    fn eq(&self, other: &Self) -> bool {
        self.public_key == other.public_key
    }
}