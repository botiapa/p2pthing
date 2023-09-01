use p2pthing_common::{
    encryption::NetworkedPublicKey,
    message_type::{msg_types::ChatMessage, PreparedFile},
};

use super::chat_input::ChatInput;

pub struct ChatMessageUI {
    pub author: NetworkedPublicKey,
    pub recipient: NetworkedPublicKey,
    pub msg: String,
    pub id: String,
    pub attachments: Option<Vec<PreparedFile>>,
    pub received: Option<bool>,
    pub own: bool,
}

impl ChatMessageUI {
    pub fn from_chat_message(msg: ChatMessage, received: Option<bool>, own: bool) -> ChatMessageUI {
        ChatMessageUI {
            author: msg.author,
            recipient: msg.recipient,
            msg: msg.msg,
            id: msg.id,
            attachments: msg.attachments,
            received,
            own,
        }
    }
}

pub struct UIPeer {
    inner: NetworkedPublicKey,
    pub chat_input: ChatInput,
    pub chat_messages: Vec<ChatMessageUI>,
}

impl UIPeer {
    pub fn from(p: &NetworkedPublicKey) -> UIPeer {
        UIPeer { inner: p.clone(), chat_input: ChatInput::new(), chat_messages: vec![] }
    }

    pub fn get_public_key(&self) -> &NetworkedPublicKey {
        &self.inner
    }
}

impl PartialEq for UIPeer {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}
