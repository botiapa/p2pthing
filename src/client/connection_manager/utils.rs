use std::{io::{self, Write}};

use serde::Serialize;

use crate::{client::tui::Tui, common::{debug_message::DebugMessageType, encryption::NetworkedPublicKey, message_type::MsgType}};

use super::ConnectionManager;

impl ConnectionManager {
    pub fn send_tcp_message<T: ?Sized>(&mut self, t:MsgType, msg: &T) -> io::Result<()> where T: Serialize {
        let t: u8 = num::ToPrimitive::to_u8(&t).unwrap();
        let msg = &bincode::serialize(msg).unwrap()[..];

        let conn = self.udp_connections.iter()
        .find(|x| x.address == self.rendezvous_ip).unwrap();

        let encrypted = &conn.symmetric_key.as_ref().unwrap().encrypt(&[&[t], msg].concat()[..])[..];

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

    /// Send a UDP packet which optionally can be reliable
    pub fn send_udp_message<T: ?Sized>(&mut self, public_key: Option<NetworkedPublicKey>, t: MsgType, msg: &T, reliable: bool, custom_id: Option<u32>) -> Result<(), &'static str> where T: Serialize  {
        let rendezvous_ip = self.rendezvous_ip.clone();
        let conn = match public_key {
            Some(public_key) => {
                match self.udp_connections.iter_mut()
                .find(|c| 
                    if c.associated_peer.is_some() {c.associated_peer.as_ref().unwrap() == &public_key} else {false}
                ) {
                    Some(conn) => conn,
                    None => {
                        Tui::debug_message(&format!("Cannot find udp connection with public key: ({})", public_key), DebugMessageType::Error, &self.ui_s);
                        return Err("Cannot find udp connection");
                    }
                }
            }
            None => self.udp_connections.iter_mut().find(|c| c.address == rendezvous_ip).unwrap()
        };
        conn.send_udp_message(t, msg, reliable, custom_id);
        Ok(())
    }
}