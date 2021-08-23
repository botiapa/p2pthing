use mio_misc::channel::Sender;

use crate::{debug_message::DebugMessageType, encryption::NetworkedPublicKey, message_type::InterthreadMessage};

pub const CHOOSABLE_KBITS: [i32; 7] = [2, 8, 16, 32, 64, 128, 256];

/// A trait which all user interfaces need to implement
pub trait UI {
    /// Return the channel which can be used to message the UI
    fn get_notifier(&self) -> Sender<InterthreadMessage>;
    /// This function blocks the thread, display and handles the UI.
    fn main_loop(&mut self, cm_s: Sender<InterthreadMessage>, own_public_key: NetworkedPublicKey);
}

pub enum UIType {
    TUI,
    GUI
}

pub enum CallStatus {
    PunchThroughSuccessfull,
    PunchThroughInProgress,
    SentRequest,
    RequestFailed
}

pub struct CallStatusHolder {
    pub status: CallStatus,
    pub public_key: NetworkedPublicKey
}

/// Helper trait for logging messages
pub trait UIConn {
    fn log_message(&self, msg: &str, msg_type: DebugMessageType);
    fn log_info(&self, msg: &str);
    fn log_warning(&self, msg: &str);
    fn log_error(&self, msg: &str);
}

impl UIConn for Sender<InterthreadMessage> {
    fn log_message(&self, msg: &str, msg_type: DebugMessageType) {
        self.send(InterthreadMessage::DebugMessage(msg.into(), msg_type)).unwrap();
    }

    fn log_info(&self, msg: &str) {
        self.send(InterthreadMessage::DebugMessage(msg.into(), DebugMessageType::Info)).unwrap();
    }

    fn log_warning(&self, msg: &str) {
        self.send(InterthreadMessage::DebugMessage(msg.into(), DebugMessageType::Warning)).unwrap();
    }

    fn log_error(&self, msg: &str) {
        self.send(InterthreadMessage::DebugMessage(msg.into(), DebugMessageType::Error)).unwrap();
    }
}