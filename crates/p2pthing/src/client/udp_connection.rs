use std::{io, net::SocketAddr, rc::Rc, time::{Duration, Instant}};

use mio::net::UdpSocket;
use p2pthing_common::{encryption::{AsymmetricEncryption, NetworkedPublicKey, SymmetricEncryption}, message_type::{MsgEncryption, MsgType, UdpPacket}, statistics::ConnectionStatistics, num, serde::Serialize};

use super::connection_manager::{RELIABLE_MESSAGE_DELAY, KEEP_ALIVE_DELAY_MIDCALL, ANNOUNCE_DELAY, KEEP_ALIVE_DELAY, UdpHolder};

#[derive(Debug, PartialEq)]
pub enum UdpConnectionState {
    /// We haven't contacted the peer yet, and no connection has been started by either side
    Unknown=0,
    /// The punch through is currently being done
    MidCall=1, 
    /// The socket is 'connected' so only keep alive packets need to be sent
    Connected=2,
    /// The socket is waiting for the server to accept the announce
    Unannounced=3,
    /// We are waiting for the ui to decide whether we should start connecting
    Pending=4
}

#[derive(Debug)]
pub struct UdpConnection {
    pub associated_peer: Option<NetworkedPublicKey>,
    pub address: SocketAddr,
    pub last_message_sent: Option<Instant>,
    pub last_announce: Option<Instant>,
    pub state: UdpConnectionState,
    pub next_msg_id: u32,
    /// Messages waiting to be confirmed that they arrived
    pub sent_messages: Vec<UdpHolder>,
    /// List of message ids, so duplicate messages can be thrown away
    pub received_messages: Vec<u32>,
    pub sock: Rc<UdpSocket>,
    pub symmetric_key: Option<SymmetricEncryption>,
    /// Has a symmetrically encrypted tunnel been created?
    pub upgraded: bool,
    pub encryption: Rc<AsymmetricEncryption>,
    pub statistics: ConnectionStatistics
}

impl UdpConnection{
    pub fn new(state: UdpConnectionState, address: SocketAddr, sock: Rc<UdpSocket>, symmetric_key: Option<SymmetricEncryption>, encryption: Rc<AsymmetricEncryption>) -> UdpConnection {
        UdpConnection{
            associated_peer: None,
            address,
            last_message_sent: None,
            last_announce: None,
            state,
            next_msg_id: 0,
            sent_messages: vec![],
            sock: sock.clone(),
            symmetric_key,
            received_messages: vec![],
            upgraded: false,
            encryption,
            statistics: ConnectionStatistics::new()
        }
    }

    pub fn send_udp_message<T: ?Sized>(&mut self, t: MsgType, msg: &T, reliable: bool, custom_id: Option<u32>) -> Result<(), String> where T: Serialize {
        match self.upgraded {
            true => self.send_udp_message_with_asymmetric_key(t, msg, reliable, custom_id),
            false => self.send_udp_message_with_public_key(t, msg, reliable, custom_id)
        }
    }

    /// Send a UDP packet encrypted with the symmetric key, which optionally can be reliable
    pub fn send_udp_message_with_asymmetric_key<T: ?Sized>(&mut self, msg_type: MsgType, msg: &T, reliable: bool, custom_id: Option<u32>) -> Result<(), String> where T: Serialize  {
        let t: u8 = num::ToPrimitive::to_u8(&msg_type).unwrap();
        let msg = &bincode::serialize(msg).unwrap()[..];
        let chained: &[u8] = &[&[t], msg].concat()[..];

        let symmetric_key = match &self.symmetric_key{
            Some(p) => p,
            None => {
                return Err("Cannot find symmetric key".into());
            }
        };
        let encrypted = symmetric_key.encrypt(&chained[..]);

        let wrapped = UdpPacket {
            data: encrypted,
            reliable,
            msg_id: self.next_msg_id,
            upgraded: MsgEncryption::SymmetricKey
        };

        if let Err(e) =  self.send_udp_packet(msg_type, wrapped, reliable, custom_id) {
            return Err(format!("Error while sending udp packet: {}", e));
        }

        Ok(())
    }

    /// Send a UDP packet encrypted with the public key, which optionally can be reliable
    pub fn send_udp_message_with_public_key<T: ?Sized>(&mut self, msg_type: MsgType, msg: &T, reliable: bool, custom_id: Option<u32>) -> Result<(), String> where T: Serialize  {
        let t: u8 = num::ToPrimitive::to_u8(&msg_type).unwrap();
        let msg = &bincode::serialize(msg).unwrap()[..];
        let chained: &[u8] = &[&[t], msg].concat()[..];

        let public_key = match &self.associated_peer{
            Some(p) => p,
            None => {
                return Err("Cannot find udp connection".into());
            }
        };
        let encrypted = public_key.encrypt(chained);
        let wrapped = UdpPacket {
            data: encrypted,
            reliable,
            msg_id: self.next_msg_id,
            upgraded: MsgEncryption::PublicKey
        };
        if let Err(e) = self.send_udp_packet(msg_type, wrapped, reliable, custom_id){
            return Err(format!("Error while sending udp packet: {}", e));
        }
        
        Ok(())
    }

    /// Send message unencrypted
    pub fn send_raw_message<T: ?Sized>(&mut self, msg_type: MsgType, msg: &T, reliable: bool, custom_id: Option<u32>) -> io::Result<()> where T: Serialize {
        let t: u8 = num::ToPrimitive::to_u8(&msg_type).unwrap();
        let msg = &bincode::serialize(msg).unwrap()[..];
        let chained: &[u8] = &[&[t], msg].concat()[..];

        let packet = UdpPacket {
            data: chained.to_vec(),
            reliable,
            msg_id: self.next_msg_id,
            upgraded: MsgEncryption::Unencrypted
        };
        self.send_udp_packet(msg_type, packet, reliable, custom_id)
    }

    pub fn send_udp_packet(&mut self, msg_type: MsgType, packet: UdpPacket, reliable: bool, custom_id: Option<u32>) -> io::Result<()> {
        self.next_msg_id += 1;
        let wrapped_data = &bincode::serialize(&packet).unwrap()[..];
        if reliable {
            self.sent_messages.push(UdpHolder{
                packet,
                last_send: Instant::now(),
                sent: Instant::now(),
                sock: self.sock.clone(),
                address: self.address,
                msg_type,
                custom_id
            });
        }

        let sent = self.sock.send_to(wrapped_data, self.address)?;
        self.statistics.sent_bytes(sent as u64);
        self.last_message_sent = Some(Instant::now());
        Ok(())
    }

    pub fn next_keep_alive(&mut self) -> Duration {
        let delay = match self.state {
            UdpConnectionState::Unknown => Duration::new(u64::MAX, 0),
            UdpConnectionState::MidCall => KEEP_ALIVE_DELAY_MIDCALL,
            UdpConnectionState::Connected => KEEP_ALIVE_DELAY,
            UdpConnectionState::Unannounced => ANNOUNCE_DELAY,
            UdpConnectionState::Pending => Duration::new(u64::MAX, 0)
        };
        match self.last_message_sent {
            Some(last_message_sent) => {
                (last_message_sent + delay).checked_duration_since(last_message_sent).unwrap_or(Duration::from_secs(0))
            } 
            None => Duration::from_secs(0)
        }
        
    }

    pub fn next_resendable(&mut self) -> Option<Duration> {
        self.sent_messages.sort_by(|a, b| a.last_send.cmp(&b.last_send));
        match self.sent_messages.get(0) {
            Some(msg) => {
                let duration_since = (msg.last_send + RELIABLE_MESSAGE_DELAY).checked_duration_since(msg.last_send);
                match duration_since {
                    Some(duration) => Some(duration),
                    None => Some(Duration::from_secs(0))
                }
            },
            None => None
        }
    }

    pub fn resend_reliable_messages(&mut self) {
        for packet in self.sent_messages.iter_mut() {
            if packet.last_send.elapsed() > RELIABLE_MESSAGE_DELAY {
                packet.resend();
                self.last_message_sent = Some(Instant::now());
            }
        }
    }

    pub fn send_confirmation(&mut self, id: u32) {
        self.send_udp_message(MsgType::MessageConfirmation, &id, false, None).unwrap();
    }

    pub fn decrypt(&self, packet: UdpPacket) -> Result<Vec<u8>, ()> {
        match packet.upgraded {
            MsgEncryption::SymmetricKey => {
                match &self.symmetric_key {
                    Some(key) => Ok(key.decrypt(&packet.data[..])),
                    None => {
                        return Err(())
                    }
                }
            },
            MsgEncryption::PublicKey => Ok(self.encryption.decrypt(&packet.data[..])),
            MsgEncryption::Unencrypted => Ok(packet.data)
        }
    }
}
