use mio_misc::{NotificationId, channel::channel, queue::NotificationQueue};
use msg_types::{Call};
use mio::{Interest, Poll, Waker, net::{TcpStream, UdpSocket}};
use std::{net::{ SocketAddr}, str::FromStr, sync::{Arc, mpsc}, thread::{self, JoinHandle}, time::Instant};
use mio_misc::channel::Sender;

use mio::Token;

use crate::common::{encryption::{AsymmetricEncryption, NetworkedPublicKey, SymmetricEncryption}, message_type::{InterthreadMessage,  msg_types, Peer}};

mod event_loop;
mod waker_thread;
mod tcp_messages;
mod udp_messages;
mod utils;

const RENDEZVOUS: Token = Token(0);
const WAKER: Token = Token(1);
const UDP_SOCKET: Token = Token(2);

/// Call decay defined in seconds
const CALL_DECAY: u64 = 60; 
/// Keep alive delay between messages
const KEEP_ALIVE_DELAY: u64 = 10; 
/// Message sending interval when mid-call
const KEEP_ALIVE_DELAY_MIDCALL: u64 = 1; 
/// Message sending interval when announcing
const ANNOUNCE_DELAY: u64 = 1; 
/// Delay between rendezvous server reconnect tries
const RECONNECT_DELAY: u64 = 5;

pub struct ConnectionManager {
    rendezvous_socket: TcpStream,
    rendezvous_ip: SocketAddr,
    rendezvous_public_key: Option<NetworkedPublicKey>,
    rendezvous_syn_key: SymmetricEncryption,
    udp_socket: UdpSocket,
    peers: Vec<Peer>,
    udp_connections: Vec<UdpConnection>,
    poll: Poll,
    ui_s: Sender<InterthreadMessage>,
    cm_s: Sender<InterthreadMessage>,
    encryption: AsymmetricEncryption,
    /// Instant is when the call was sent
    calls_in_progress: Vec<(Call, Instant)>, 
    waker_thread: mpsc::Sender<InterthreadMessage>
}

struct UdpConnection {
    associated_peer: Option<NetworkedPublicKey>,
    address: SocketAddr,
    last_keep_alive: Option<Instant>,
    last_announce: Option<Instant>,
    state: UdpConnectionState
}

enum UdpConnectionState {
    /// The punch through is currently being done
    MidCall=0, 
    /// The socket is 'connected' so only keep alive packets need to be sent
    Connected=1,
    /// The socket is waiting for the server to accept the announce
    Unannounced=2
}

impl ConnectionManager {
    pub fn start(rend_ip: &str, ui_s: Sender<InterthreadMessage>) -> (mio_misc::channel::Sender<InterthreadMessage>, JoinHandle<()>, Arc<Waker>, NetworkedPublicKey) {
        let mut udp_connections = Vec::new();
        let poll = Poll::new().unwrap();

        let waker = Arc::new(Waker::new(poll.registry(), WAKER).unwrap());
        let queue = Arc::new(NotificationQueue::new(waker.clone()));
        let (cm_s, cm_r) = channel(queue, NotificationId::gen_next());
        
        let rend_ip = SocketAddr::from_str(rend_ip).unwrap();

        let mut rendezvous_socket = TcpStream::connect(rend_ip).unwrap();
        poll.registry().register(&mut rendezvous_socket, RENDEZVOUS, Interest::READABLE).unwrap();

        let mut udp_socket = UdpSocket::bind(SocketAddr::from_str("0.0.0.0:0").unwrap()).unwrap();
        udp_connections.push(UdpConnection{
            associated_peer: None,
            address: rend_ip,
            last_keep_alive: None,
            last_announce: None,
            state: UdpConnectionState::Unannounced,
        });
        poll.registry().register(&mut udp_socket, UDP_SOCKET, Interest::READABLE).unwrap();

        let waker_thread = ConnectionManager::set_up_waker_thread(waker.clone());
        waker_thread.send(InterthreadMessage::SetWakepDelay(ANNOUNCE_DELAY)).unwrap();

        let encryption = AsymmetricEncryption::new();
        let key = encryption.get_public_key();

        let mut mgr = ConnectionManager {
            rendezvous_socket,
            rendezvous_ip: rend_ip,
            rendezvous_public_key: None,
            rendezvous_syn_key: SymmetricEncryption::new(),
            udp_socket,
            peers: Vec::new(),
            udp_connections,
            poll,
            ui_s,
            cm_s: cm_s.clone(),
            encryption,
            calls_in_progress: Vec::new(),
            waker_thread,
        };
        let thr = thread::spawn(move || {
            mgr.event_loop(cm_r);
        });
        (cm_s, thr, waker, key)
    }

    pub fn call(s: &Sender<InterthreadMessage>, public_key: NetworkedPublicKey) {
        s.send(InterthreadMessage::Call(public_key)).unwrap();
    }

    pub fn send_chat_message(s: &Sender<InterthreadMessage>, public_key: NetworkedPublicKey, msg: String) {
        s.send(InterthreadMessage::SendChatMessage(public_key, msg)).unwrap();
    }

    pub fn quit(s: &Sender<InterthreadMessage>, waker: &Waker) {
        s.send(InterthreadMessage::Quit());
    }
}