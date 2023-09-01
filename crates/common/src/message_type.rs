use num_derive::{FromPrimitive, ToPrimitive};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display};

use crate::statistics::{ConnectionStatistics, TransferStatistics};

use self::msg_types::{ChatMessage, FileChunks, RequestFileChunks};

use super::{debug_message::DebugMessageType, encryption::NetworkedPublicKey};

pub type FileName = String;
pub type MessageId = String;

/// As the name implies, this facilitates the messaging between the threads.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum InterthreadMessage {
    SendChatMessage(NetworkedPublicKey, String, Option<Vec<FileName>>),
    /// - **From CM to UI:** Received a message either from another peer, or from themselves
    OnChatMessage(ChatMessage),
    /// - **From CM to UI:** Notify UI that a message has been received by the other peer. String is the id of the message.
    OnChatMessageReceived(String),
    AnnounceResponse(Vec<NetworkedPublicKey>),
    CallAccepted(NetworkedPublicKey),
    CallDenied(NetworkedPublicKey),
    PunchThroughSuccessfull(NetworkedPublicKey),
    Quit(),
    PeerDisconnected(NetworkedPublicKey),
    Call(NetworkedPublicKey),
    OpusPacketReady(Vec<u8>),
    AudioDataReadyToBeProcessed(Vec<f32>),
    DebugMessage(String, DebugMessageType),
    ConnectToServer(),
    /// - **From CM to UI:** Send connection statistics for each peer
    ConnectionStatistics(Vec<(NetworkedPublicKey, ConnectionStatistics)>),
    /// - **From CM to UI:** Send transfer statistics for each file
    TransferStatistics(HashMap<FileId, TransferStatistics>),
    // AUDIO
    AudioChangeInputDevice(String),
    AudioChangeOutputDevice(String),
    AudioNewInputDevices(Option<Vec<String>>),
    AudioNewOutputDevices(Option<Vec<String>>),
    AudioChangePreferredKbits(i32),
    AudioChangeMuteState(bool),
    AudioChangeDenoiserState(bool),
    // FILES
    /// - **From CM to FM:** Prepare the files for uploading
    SendFiles(NetworkedPublicKey, Vec<FileName>),
    /// - **From CM to FM:** Prepare for receiving the file
    ReceiveFiles(Vec<PreparedFile>, NetworkedPublicKey),
    /// - **From FM to CM:** Files ready for upload
    FilesReady(NetworkedPublicKey, Vec<PreparedFile>),
    /// - **From CM to FM:** Load the specified file data chunks and then notify CM to forward it to the peer
    GetFileChunks(RequestFileChunks, NetworkedPublicKey),
    /// - **From FM to CM:** Notify CM about loaded file data chunks
    FileChunksReady(Vec<FileDataChunk>, NetworkedPublicKey),
    /// - **From CM to FM:** Store the received file data chunks
    StoreFileChunk(FileChunks),
    /// - **From FM to CM:** Request the specified file chunks from the specified peers
    RequestFileChunks(HashMap<NetworkedPublicKey, Vec<FileChunk>>),
    WakeUp,
}

#[derive(Debug, ToPrimitive, FromPrimitive)]
pub enum MsgType {
    Announce = 0,
    AnnounceSecret = 8,
    Call = 1,
    CallResponse = 2,
    Disconnect = 3,
    KeepAlive = 4,
    ChatMessage = 5,
    ChatMessageReceived = 6,
    AnnounceRequest = 7,
    MessageConfirmation = 9,
    OpusPacket = 10,
    RequestFileChunks = 11,
    FileChunks = 12,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MsgEncryption {
    Unencrypted,
    PublicKey,
    SymmetricKey,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UdpPacket {
    pub data: Vec<u8>,
    pub reliable: bool,
    pub msg_id: u32,
    pub upgraded: MsgEncryption,
}

pub type FileId = String;

/// A file which has been split into transmittable chunks
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PreparedFile {
    /// This is a base64 value that is obtained by hashing the filename and file size
    pub file_id: FileId,
    pub file_name: String,
    pub file_extension: String,
    pub total_length: u64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FileChunk {
    pub file_id: FileId,
    pub index: usize,
}

impl Display for FileChunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}[{}]", &self.file_id[0..10], self.index)
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FileDataChunk {
    pub file_id: FileId,
    pub index: usize,
    pub data: Vec<u8>,
}

pub mod msg_types {
    use std::net::SocketAddr;

    use crate::encryption::NetworkedPublicKey;
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};

    use super::{FileChunk, FileDataChunk, PreparedFile};

    /// The server announced itself to the client, requesting an announcement.
    #[derive(Serialize, Deserialize)]
    pub struct AnnounceRequest {
        pub public_key: NetworkedPublicKey,
    }

    #[derive(Serialize, Deserialize)]
    pub struct AnnouncePublic {
        pub public_key: NetworkedPublicKey,
    }
    /// Client announces its secret to either the server, or another peer
    #[derive(Serialize, Deserialize)]
    pub struct AnnounceSecret {
        pub secret: Vec<u8>,
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq)]
    pub struct Call {
        pub callee: NetworkedPublicKey,
        pub caller: Option<NetworkedPublicKey>,
        /// This is either the callee's or caller's udp address or none, depending on who sent it, and who is the recipient
        pub udp_address: Option<SocketAddr>,
    }

    #[derive(Serialize, Deserialize)]
    pub struct CallResponse {
        pub call: Call,
        pub response: bool,
    }

    #[derive(Clone, Serialize, Deserialize, Debug)]
    pub struct ChatMessage {
        pub id: String,
        pub author: NetworkedPublicKey,
        pub recipient: NetworkedPublicKey,
        pub msg: String,
        pub attachments: Option<Vec<PreparedFile>>,
        pub dt: DateTime<Utc>,
    }

    #[derive(Serialize, Deserialize)]
    pub struct ChatMessageReceived {
        pub index: u32,
    }

    #[derive(Serialize, Deserialize)]
    pub struct Disconnect {
        pub public_key: NetworkedPublicKey,
    }

    #[derive(Serialize, Deserialize)]
    pub struct ReliableMessageReceived {
        pub id: u32,
    }

    #[derive(Clone, Serialize, Deserialize, Debug)]
    pub struct RequestFileChunks {
        pub chunks: Vec<FileChunk>,
    }

    #[derive(Clone, Serialize, Deserialize, Debug)]
    pub struct FileChunks {
        pub chunks: Vec<FileDataChunk>,
    }
}
