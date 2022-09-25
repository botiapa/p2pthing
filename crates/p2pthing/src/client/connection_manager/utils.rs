use std::io::{self, Write};

use p2pthing_common::num;
use p2pthing_common::{encryption::NetworkedPublicKey, message_type::MsgType, ui::UIConn};
use p2pthing_common::serde::Serialize;

use crate::client::udp_connection::UdpConnection;

use super::ConnectionManager;

impl ConnectionManager {
    pub fn get_conn(&self, public_key: &NetworkedPublicKey) -> Option<&UdpConnection> {
        self.peers.iter().find(|p| &p.public_key == public_key).map_or(None, |p| p.udp_conn.as_ref())
    }

    pub fn send_tcp_message<T: ?Sized>(&mut self, t: MsgType, msg: &T) -> io::Result<()> where T: Serialize {
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
    pub fn send_udp_message<T: ?Sized>(&mut self, public_key: Option<NetworkedPublicKey>, t: MsgType, msg: &T, reliable: bool, custom_id: bool) -> Result<(), String> where T: Serialize  {
        let rendezvous_ip = self.rendezvous_ip.clone();
        let conn = match public_key {
            Some(public_key) => {
                match self.udp_connections.iter_mut()
                .find(|c| 
                    if c.associated_peer.is_some() {c.associated_peer.as_ref().unwrap() == &public_key} else {false}
                ) {
                    Some(conn) => conn,
                    None => {
                        self.ui_s.log_error(&format!("Cannot find udp connection with public key: ({})", public_key));
                        return Err("Cannot find udp connection".into());
                    }
                }
            }
            None => self.udp_connections.iter_mut().find(|c| c.address == rendezvous_ip).unwrap()
        };
        match custom_id {
            true => {
                conn.send_udp_message(t, msg, reliable, Some(self.next_custom_id))?;
                self.next_custom_id += 1;
            },
            false => conn.send_udp_message(t, msg, reliable, None)?,
        }
        
        Ok(())
    }
}