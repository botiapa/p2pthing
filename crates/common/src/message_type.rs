use std::{collections::HashMap, net::SocketAddr, fmt::Display};
use num_derive::{FromPrimitive, ToPrimitive};
use serde::{Serialize, Deserialize};

use crate::statistics::Statistics;

use self::msg_types::{FileChunks, RequestFileChunks};

use super::{debug_message::DebugMessageType, encryption::{NetworkedPublicKey, SymmetricEncryption}};

#[derive(Serialize, Deserialize, Clone)]
pub enum InterthreadMessage {
    SendChatMessage(NetworkedPublicKey, String, u32),
    OnChatMessage(Peer, String),
    OnChatMessageReceived(u32), //u32 is the custom_id
    AnnounceResponse(Vec<Peer>),
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
    ConnectionStatistics(Vec<(NetworkedPublicKey, Statistics)>),
    // AUDIO
    AudioChangeInputDevice(String),
    AudioChangeOutputDevice(String),
    AudioNewInputDevices(Option<Vec<String>>),
    AudioNewOutputDevices(Option<Vec<String>>),
    AudioChangePreferredKbits(i32),
    AudioChangeMuteState(bool),
    AudioChangeDenoiserState(bool),
    // FILES
    /// - **From client to CM:** Start sending the specified files to a peer.
    /// - **From CM to FM:** Prepare the files for uploading
    SendFiles(NetworkedPublicKey, Vec<String>),
    /// - **From CM to FM:** Prepare for receiving the file
    ReceiveFiles(Vec<SplitFile>, NetworkedPublicKey),
    /// - **From FM to CM:** Files ready for upload
    FilesReady(NetworkedPublicKey, Vec<SplitFile>),
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

#[derive(ToPrimitive, FromPrimitive)]
pub enum MsgType {
    Announce=0,
    AnnounceSecret=8,
    Call=1,
    CallResponse=2,
    Disconnect=3,
    KeepAlive=4,
    ChatMessage=5,
    ChatMessageReceived=6,
    AnnounceRequest=7,
    MessageConfirmation=9,
    OpusPacket=10,
    SendFilesRequest=11,
    RequestFileChunks=12,
    FileChunks=13
}

#[derive(Serialize, Deserialize)]
pub struct Peer {
    pub addr: Option<SocketAddr>,
    pub udp_addr: Option<SocketAddr>,
    pub public_key: NetworkedPublicKey,
    #[serde(skip)]
    pub sym_key: Option<SymmetricEncryption>
}

impl Clone for Peer {
    fn clone(&self) -> Self {
        self.full_clone()
    }
}

impl Peer {
    
    /// This is an unsafe clone because it can potentially leak ip adresses
    fn full_clone(&self) -> Self {
        Self {
            public_key: self.public_key.clone(),
            addr: self.addr.clone(),
            udp_addr: self.udp_addr.clone(),
            sym_key: None,
        }
    }

    /// This clone only copies public information
    pub fn safe_clone(&self) -> Self{
        Self {
            public_key: self.public_key.clone(),
            addr: None,
            udp_addr: None,
            sym_key: None,
        }
    }
}

impl PartialEq for Peer {
    fn eq(&self, other: &Self) -> bool {
        self.public_key == other.public_key
    }
}
#[derive(Serialize, Deserialize, Clone)]
pub enum MsgEncryption {
    Unencrypted,
    PublicKey,
    SymmetricKey
}

#[derive(Serialize, Deserialize, Clone)]
pub struct UdpPacket {
    pub data: Vec<u8>,
    pub reliable: bool,
    pub msg_id: u32,
    pub upgraded: MsgEncryption
}

pub type FileId = String;

/// A file which has been split into transmittable chunks
#[derive(Clone, Serialize, Deserialize)]
pub struct SplitFile {
    /// This is a base64 value that is obtained by hashing the filename and file size
    pub file_id: FileId,
    pub file_name: String,
    pub total_length: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FileChunk {
    pub file_id: FileId, 
    pub index: usize
}

impl Display for FileChunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}[{}]", &self.file_id[0..10], self.index)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FileDataChunk {
    pub file_id: FileId, 
    pub index: usize,
    pub data: Vec<u8>
}

/// A single chunk of a file

pub mod msg_types {
    use std::net::SocketAddr;

    use serde::{Serialize, Deserialize};
    use crate::encryption::NetworkedPublicKey;

    use super::{FileChunk, FileDataChunk, SplitFile};
    
    /// The server announced itself to the client, requesting an announcement.
    #[derive(Serialize, Deserialize)]
    pub struct AnnounceRequest {
        pub public_key: NetworkedPublicKey
    }

    
    #[derive(Serialize, Deserialize)]
    pub struct AnnouncePublic {
        pub public_key: NetworkedPublicKey,
    }
    /// Client announces its secret to either the server, or another peer
    #[derive(Serialize, Deserialize)]
    pub struct AnnounceSecret {
        pub secret: Vec<u8>
    }

    
    #[derive(Serialize, Deserialize, Clone, PartialEq)]
    pub struct Call {
        pub callee: NetworkedPublicKey,
        pub caller: Option<NetworkedPublicKey>,
        /// This is either the callee's or caller's udp address or none, depending on who sent it, and who is the recipient
        pub udp_address: Option<SocketAddr>
    }

    #[derive(Serialize, Deserialize)]
    pub struct CallResponse {
        pub call: Call,
        pub response: bool
    }

    #[derive(Serialize, Deserialize)]
    pub struct ChatMessage {
        pub msg: String,
    }

    #[derive(Serialize, Deserialize)]
    pub struct ChatMessageReceived {
        pub index: u32
    }

    #[derive(Serialize, Deserialize)]
    pub struct Disconnect {
        pub public_key: NetworkedPublicKey
    }

    #[derive(Serialize, Deserialize)]
    pub struct ReliableMessageReceived {
        pub id: u32
    }

    #[derive(Serialize, Deserialize)]
    pub struct SendFilesRequest {
        pub files: Vec<SplitFile>,
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub struct RequestFileChunks {
        pub chunks: Vec<FileChunk>
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub struct FileChunks {
        pub chunks: Vec<FileDataChunk>
    }
    
}