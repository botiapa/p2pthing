use core::panic;
use std::{io::{self, Write}, net::SocketAddr};

use mio::net::UdpSocket;
use serde::Serialize;

use crate::{client::tui::Tui, common::{debug_message::DebugMessageType, encryption::NetworkedPublicKey, message_type::MsgType}};

use super::ConnectionManager;

impl ConnectionManager {
    pub fn send_tcp_message<T: ?Sized>(&mut self, t:MsgType, msg: &T) -> io::Result<()> where T: Serialize {
        let t: u8 = num::ToPrimitive::to_u8(&t).unwrap();
        let msg = &bincode::serialize(msg).unwrap()[..];

        let encrypted = &self.rendezvous_syn_key.encrypt(&[&[t], msg].concat()[..]);

        let msg_size = bincode::serialize(&encrypted.len()).unwrap();
        let chained: &[u8] = &[&msg_size[..], encrypted].concat()[..];

        self.rendezvous_socket.write_all(&chained[..])?;
        Ok(())
    }

    pub fn send_tcp_message_public_key<T: ?Sized>(&mut self, t:MsgType, msg: &T) -> io::Result<()> where T: Serialize {
        let t: u8 = num::ToPrimitive::to_u8(&t).unwrap();
        let msg = &bincode::serialize(msg).unwrap()[..];

        let key = self.rendezvous_public_key.as_ref().unwrap();
        let encrypted = &key.encrypt(&[&[t], msg].concat()[..]);

        let msg_size = bincode::serialize(&encrypted.len()).unwrap();
        let chained: &[u8] = &[&msg_size[..], encrypted].concat()[..];

        self.rendezvous_socket.write_all(&chained[..])?;
        Ok(())
    }

    pub fn send_udp_message<T: ?Sized>(&self, public_key: Option<NetworkedPublicKey>, t: MsgType, msg: &T) -> io::Result<()> where T: Serialize  {
        let addr = match public_key {
            Some(public_key) => match self.udp_connections.iter().find(|&c| if let Some(p) = c.associated_peer.as_ref() {*p == public_key} else {false}) {
                Some(c) => c.address,
                None => {
                    Tui::debug_message(&format!("Tried sending chat message to ({}) with no udp address associated", public_key), DebugMessageType::Warning, &self.ui_s);
                    return Ok(()) // FIXME: Introduce error?
                }
            },
            None => self.rendezvous_ip
        };
        let t: u8 = num::ToPrimitive::to_u8(&t).unwrap();
        let msg = &bincode::serialize(msg).unwrap()[..];
        let chained: &[u8] = &[&[t], msg].concat()[..];

        self.udp_socket.send_to(chained, addr)?;
        Ok(())
    }

    pub fn send_udp_message_to<T: ?Sized>(sock: &UdpSocket, addr: SocketAddr, t: MsgType, msg: &T) -> io::Result<()> where T: Serialize  {
        let t: u8 = num::ToPrimitive::to_u8(&t).unwrap();
        let msg = &bincode::serialize(msg).unwrap()[..];
        let chained: &[u8] = &[&[t], msg].concat()[..];

        sock.send_to(chained, addr)?;
        Ok(())
    }
}