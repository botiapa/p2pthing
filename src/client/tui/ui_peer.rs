use crate::common::{encryption::NetworkedPublicKey, message_type::Peer};

use super::chat_input::ChatInput;

pub struct ChatMessage {
    pub author: Peer,
    pub msg: String
}

pub struct UIPeer {
    inner: Peer,
    pub chat_input: ChatInput,
    pub chat_messages: Vec<ChatMessage>
}

impl UIPeer {
    pub fn from(p: &Peer) -> UIPeer {
        UIPeer {
            inner: p.clone(),
            chat_input: ChatInput::new(),
            chat_messages: vec![]
        }
    }

    pub fn get_public_key(&self) -> &NetworkedPublicKey {
        &self.inner.public_key
    }
}

impl PartialEq for UIPeer {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}