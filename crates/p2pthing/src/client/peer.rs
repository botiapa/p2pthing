use std::{net::SocketAddr, ops::{DerefMut, Deref}, slice::Iter};

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


#[derive(Debug)]
pub struct PeerList {
    pub inner: Vec<Peer>
}

/*impl Deref for PeerList {
    type Target = Vec<Peer>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for PeerList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    } 
}*/

impl Default for PeerList {
    fn default() -> Self {
        Self { inner: Default::default() }
    }
}

impl PeerList {
    pub fn push(&mut self, p: Peer){
        if self.inner.iter().any(|_p| *_p == p) {
            // We should never enter this if
            panic!("Tried to push a peer that is already present");
        }
        self.inner.push(p);
    }

    pub fn remove(&mut self, public_key: &NetworkedPublicKey) {
        self.inner.iter_mut()
        .position(|p| &p.public_key == public_key)
        .map(|i| self.inner.swap_remove(i));
    }

    pub fn peer(&self, public_key: &NetworkedPublicKey) -> Option<&Peer> {
        self.inner.iter().find(|p| &p.public_key == public_key)
    }

    pub fn peer_mut(&mut self, public_key: &NetworkedPublicKey) -> Option<&mut Peer> {
        self.inner.iter_mut().find(|p| &p.public_key == public_key)
    }

    pub fn peer_by_addr(&self, addr: &SocketAddr) -> Option<&Peer> {
        self.inner.iter().filter(|p| p.udp_conn.is_some()).find(|p| &p.udp_conn.as_ref().unwrap().address == addr)
    }
    
    pub fn peer_by_addr_mut(&mut self, addr: &SocketAddr) -> Option<&mut Peer> {
        self.inner.iter_mut().filter(|p| p.udp_conn.is_some()).find(|p| &p.udp_conn.as_ref().unwrap().address == addr)
    }

    pub fn conn(&mut self, addr: &SocketAddr) -> Option<&UdpConnection> {
        self.inner.iter().filter_map(|p| p.udp_conn.as_ref()).find(|c| &c.address == addr)
    }

    pub fn conn_mut(&mut self, addr: &SocketAddr) -> Option<&mut UdpConnection> {
        self.inner.iter_mut().filter_map(|p| p.udp_conn.as_mut()).find(|c| &c.address == addr)
    }

    pub fn connections(&self) -> impl Iterator<Item=&UdpConnection>{
        self.inner.iter().filter_map(|p| p.udp_conn.as_ref())
    }

    pub fn connections_mut(&mut self) -> impl Iterator<Item=&mut UdpConnection>{
        self.inner.iter_mut().filter_map(|p| p.udp_conn.as_mut())
    }
}