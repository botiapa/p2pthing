use std::{net::SocketAddr, ops::{DerefMut, Deref}, slice::Iter};

use mio::net::TcpStream;
use p2pthing_common::{encryption::{NetworkedPublicKey, SymmetricEncryption}, enumset::{EnumSetType, EnumSet}};
use serde::{Serialize, Deserialize};

use super::udp_connection::UdpConnection;

#[derive(EnumSetType, Serialize, Deserialize, Debug)]
pub enum PeerSource {
    /// The peer was discovered using multicast broadcasts
    Multicast,
    /// The peer was received from a rendezvous server
    Rendezvous,
    /// The peer was manually added by the user
    Manual,
}


#[derive(EnumSetType, Serialize, Deserialize, Debug)]
pub enum PeerType {
    RendezvousServer,
    ClientPeer
}

#[derive(Debug)]
pub struct Peer {
    /// The address of the peer
    pub addr: Option<SocketAddr>,
    /// The TCP stream of the peer. Only servers have this
    pub tcp_conn: Option<TcpStream>,
    /// The UDP connection of the peer
    pub udp_conn: Option<UdpConnection>,
    /// The symmetric encryption keys of the peer
    pub sym_key: Option<SymmetricEncryption>,
    /// The public key of the peer, this might be unknown at first if the peer was added manually
    pub public_key: Option<NetworkedPublicKey>,
    /// The source where the peer was found
    pub source: EnumSet<PeerSource>,
    /// The type of the peer
    pub peer_type: PeerType,
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
    /// Adds the given peer to the list
    pub fn push(&mut self, p: Peer){
        if self.inner.iter().any(|_p| *_p == p) {
            // We should never enter this if
            panic!("Tried to push a peer that is already present");
        }
        self.inner.push(p);
    }

    /// Removes a peer by the given public key
    pub fn remove(&mut self, public_key: &NetworkedPublicKey) {
        self.inner.iter_mut()
        .position(|p| p.public_key.is_some() && &p.public_key.as_ref().unwrap() == &public_key)
        .map(|i| self.inner.swap_remove(i));
    }

    /// Searches for a peer by the given public key
    pub fn peer(&self, public_key: &NetworkedPublicKey) -> Option<&Peer> {
        self.inner.iter().find(|p| p.public_key.is_some() && &p.public_key.as_ref().unwrap() == &public_key)
    }

    /// Searches for a peer by the given public key, returns it as mutable
    pub fn peer_mut(&mut self, public_key: &NetworkedPublicKey) -> Option<&mut Peer> {
        self.inner.iter_mut().find(|p| p.public_key.is_some() && &p.public_key.as_ref().unwrap() == &public_key)
    }

    /// Searches for a peer by the given udp socket address
    pub fn peer_by_addr(&self, addr: &SocketAddr) -> Option<&Peer> {
        self.inner.iter().filter(|p| p.udp_conn.is_some()).find(|p| &p.udp_conn.as_ref().unwrap().address == addr)
    }
    
    /// Searches for a peer by the given udp socket address, returns it as mutable
    pub fn peer_by_addr_mut(&mut self, addr: &SocketAddr) -> Option<&mut Peer> {
        self.inner.iter_mut().filter(|p| p.udp_conn.is_some()).find(|p| &p.udp_conn.as_ref().unwrap().address == addr)
    }

    /// Returns a peer's udp connection for the given socket address
    pub fn conn(&mut self, addr: &SocketAddr) -> Option<&UdpConnection> {
        self.inner.iter().filter_map(|p| p.udp_conn.as_ref()).find(|c| &c.address == addr)
    }

    /// Returns a peer's mutable udp connection for the given socket address
    pub fn conn_mut(&mut self, addr: &SocketAddr) -> Option<&mut UdpConnection> {
        self.inner.iter_mut().filter_map(|p| p.udp_conn.as_mut()).find(|c| &c.address == addr)
    }

    /// Returns all peer connections
    pub fn connections(&self) -> impl Iterator<Item=&UdpConnection>{
        self.inner.iter().filter_map(|p| p.udp_conn.as_ref())
    }

    /// Returns all peer connections as mutable
    pub fn connections_mut(&mut self) -> impl Iterator<Item=&mut UdpConnection>{
        self.inner.iter_mut().filter_map(|p| p.udp_conn.as_mut())
    }

    /// Returns all rendezvous servers
    pub fn rendezvous_servers(&self) -> impl Iterator<Item=&Peer>{
        self.inner.iter().filter(|p| p.peer_type == PeerType::RendezvousServer)
    }

    pub fn rendezvous_servers_mut(&mut self) -> impl Iterator<Item=&mut Peer>{
        self.inner.iter_mut().filter(|p| p.peer_type == PeerType::RendezvousServer)
    }
}