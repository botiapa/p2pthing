use mio_misc::{NotificationId, channel::channel, queue::NotificationQueue};
use msg_types::{Call};
use mio::{Interest, Poll, Waker, net::{TcpStream, UdpSocket}};
use std::{net::SocketAddr, ops::Add, rc::Rc, str::FromStr, sync::{Arc}, thread::{self, JoinHandle}, time::{Duration, Instant}};
use mio_misc::channel::Sender;

use mio::Token;

use crate::common::{encryption::{AsymmetricEncryption, NetworkedPublicKey, SymmetricEncryption}, message_type::{InterthreadMessage, MsgEncryption, MsgType, Peer, UdpPacket, msg_types}};

use super::{audio::Audio, udp_connection::{UdpConnection, UdpConnectionState}};

mod event_loop;
mod tcp_messages;
mod udp_messages;
mod utils;

const RENDEZVOUS: Token = Token(0);
const WAKER: Token = Token(1);
const UDP_SOCKET: Token = Token(2);

/// Call decay defined in seconds
const CALL_DECAY: Duration = Duration::from_secs(10); 
/// Keep alive delay between messages
pub const KEEP_ALIVE_DELAY: Duration = Duration::from_secs(10); 
/// Message sending interval when mid-call
pub const KEEP_ALIVE_DELAY_MIDCALL: Duration = Duration::from_secs(1); 
/// Message sending interval when announcing
pub const ANNOUNCE_DELAY: Duration = Duration::from_secs(1); 
/// Delay between rendezvous server reconnect tries
const RECONNECT_DELAY: Duration = Duration::from_secs(5);
/// Delay between retrying to send a reliable message
pub const RELIABLE_MESSAGE_DELAY: Duration = Duration::from_secs(2);

pub struct ConnectionManager {
    rendezvous_socket: TcpStream,
    rendezvous_ip: SocketAddr,
    rendezvous_public_key: Option<NetworkedPublicKey>,
    udp_socket: Rc<UdpSocket>,
    peers: Vec<Peer>,
    udp_connections: Vec<UdpConnection>,
    poll: Poll,
    ui_s: Sender<InterthreadMessage>,
    cm_s: Sender<InterthreadMessage>,
    encryption: Rc<AsymmetricEncryption>,
    /// Instant is when the call was sent
    calls_in_progress: Vec<(Call, Instant)>,
    audio: Audio
}

pub struct UdpHolder {
    pub packet: UdpPacket,
    pub last_send: Instant,
    pub sock: Rc<UdpSocket>,
    pub address: SocketAddr,
    pub msg_type: MsgType,
    /// Custom identifier used when trying to identify a confirmed message
    pub custom_id: Option<u32>
}

impl UdpHolder{
    pub fn resend(&mut self) {
        let packet_data = &bincode::serialize(&self.packet).unwrap()[..];
        self.sock.send_to(packet_data, self.address).unwrap();
        self.last_send = Instant::now();
    }
}

impl ConnectionManager {
    pub fn new(encryption:AsymmetricEncryption, rend_ip: String, poll: Poll, ui_s: Sender<InterthreadMessage>, cm_s: Sender<InterthreadMessage>) -> ConnectionManager {
        let rend_ip = SocketAddr::from_str(&rend_ip).unwrap();

        let mut rendezvous_socket = TcpStream::connect(rend_ip).unwrap();
        poll.registry().register(&mut rendezvous_socket, RENDEZVOUS, Interest::READABLE).unwrap();

        let mut udp_socket = UdpSocket::bind(SocketAddr::from_str("0.0.0.0:0").unwrap()).unwrap();
        poll.registry().register(&mut udp_socket, UDP_SOCKET, Interest::READABLE).unwrap();
        let mut udp_connections = Vec::new();

        let audio = Audio::new(ui_s.clone(), cm_s.clone());

        let udp_socket = Rc::new(udp_socket);
        let encryption = Rc::new(encryption);
        udp_connections.push(UdpConnection::new(
            UdpConnectionState::Unannounced, 
            rend_ip, 
            udp_socket.clone(), 
            Some(SymmetricEncryption::new()),
            encryption.clone()
        ));

        ConnectionManager {
            rendezvous_socket,
            rendezvous_ip: rend_ip,
            rendezvous_public_key: None,
            udp_socket: udp_socket.clone(),
            peers: Vec::new(),
            udp_connections,
            poll,
            audio,
            ui_s,
            cm_s: cm_s.clone(),
            encryption,
            calls_in_progress: Vec::new(),
        }
    }

    pub fn start(rend_ip: String, ui_s: Sender<InterthreadMessage>) -> (mio_misc::channel::Sender<InterthreadMessage>, JoinHandle<()>, NetworkedPublicKey) {
        let poll = Poll::new().unwrap();
        let waker = Arc::new(Waker::new(poll.registry(), WAKER).unwrap());
        let queue = Arc::new(NotificationQueue::new(waker.clone()));
        let (cm_s, mut cm_r) = channel(queue, NotificationId::gen_next());

        let encryption = AsymmetricEncryption::new();
        let key = encryption.get_public_key();

        let cm_s1 = cm_s.clone();
        let thr = thread::spawn(move || {
            let mut mgr = ConnectionManager::new(encryption, rend_ip, poll, ui_s, cm_s1);
            mgr.event_loop(&mut cm_r);
        });
        
        (cm_s, thr, key)
    }

    pub fn call(s: &Sender<InterthreadMessage>, public_key: NetworkedPublicKey) {
        s.send(InterthreadMessage::Call(public_key)).unwrap();
    }

    pub fn send_chat_message(s: &Sender<InterthreadMessage>, public_key: NetworkedPublicKey, msg: String, custom_id: u32) {
        s.send(InterthreadMessage::SendChatMessage(public_key, msg, custom_id)).unwrap();
    }

    pub fn quit(s: &Sender<InterthreadMessage>) {
        s.send(InterthreadMessage::Quit()).unwrap();
    }
}