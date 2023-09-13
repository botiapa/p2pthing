use std::net::SocketAddr;

use p2pthing_common::{
    message_type::{msg_types, InterthreadMessage},
    ui::UIConn,
};

use crate::client::connection_manager::ConnectionManager;

use super::ChatHandler;

impl ChatHandler for ConnectionManager {
    fn on_chat_message(&mut self, addr: SocketAddr, data: &[u8]) {
        let msg: msg_types::ChatMessage = bincode::deserialize(data).unwrap();
        let p = self.peers.peer_by_addr(&addr).unwrap();
        self.ui_s.send(InterthreadMessage::OnChatMessage(msg.clone())).unwrap();

        //TODO: Ability to accept or deny file download
        if let Some(files) = msg.attachments {
            for file in files {
                if let Err(e) =
                    self.file_manager.start_receiving_file(file.clone(), p.public_key.as_ref().unwrap().clone())
                {
                    self.ui_s.log_error(&format!("Failed preparing to receive file: {}", e));
                }
            }
        }
    }
}
