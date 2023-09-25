use std::net::SocketAddr;

use p2pthing_common::{
    message_type::{
        self,
        msg_types::{self, FileChunks},
        MsgType,
    },
    ui::UIConn,
};

use crate::client::connection_manager::ConnectionManager;

use super::FileHandler;

impl FileHandler for ConnectionManager {
    /// The client received a request for file chunks
    fn on_request_file_chunks(&mut self, addr: SocketAddr, data: &[u8]) {
        let data: msg_types::RequestFileChunks = bincode::deserialize(data).unwrap();
        let p = self.peers.peer_by_addr(&addr).unwrap();
        let public_key = p.public_key.as_ref().unwrap().clone();

        //TODO: Ability to accept or deny file download
        match self.file_manager.get_file_chunks(data) {
            Ok(chunks) => {
                for ch in chunks {
                    if let Err(e) =
                        self.send_udp_message(Some(public_key.clone()), MsgType::FileChunk, &ch, false, false)
                    {
                        self.ui_s
                            .log_error(&format!("Error while trying to send a file chunk request: {}", e.to_string()));
                    }
                }
            }
            Err(e) => self.ui_s.log_error(&format!("Failed reading file chunks: {}", &e)),
        }
    }

    /// The client received file chunks
    fn on_file_chunks(&mut self, addr: SocketAddr, data: &[u8]) {
        let data: message_type::FileDataChunk = bincode::deserialize(data).unwrap();

        //TODO: Ability to accept or deny file download
        //TODO: Rename store_file_chunks and only store one chunk at a time
        if let Err(e) = self.file_manager.store_file_chunks(FileChunks { chunks: vec![data] }) {
            self.ui_s.log_error(&format!("Error while trying to save a file chunk: {}", e));
        }
    }
}
