use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum DebugMessageType {
    Info = 0,
    Warning = 1,
    Error = 2,
}
pub struct DebugMessage {
    pub message: String,
    pub time: DateTime<Utc>,
    pub msg_type: DebugMessageType,
}

impl Display for DebugMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let time = self.time.format("%H:%M:%S");
        write!(f, "{}: {}", time, self.message)
    }
}
