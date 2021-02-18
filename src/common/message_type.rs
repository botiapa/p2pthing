use std::net::SocketAddr;
use num_derive::{FromPrimitive, ToPrimitive};
use serde::{Serialize, Deserialize};

use super::{debug_message::DebugMessageType, encryption::{NetworkedPublicKey, SymmetricEncryption}};
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
    DebugMessage(String, DebugMessageType),
    ConnectToServer(),
    AudioChangeInputDevice(String),
    AudioChangeOutputDevice(String),
    AudioNewInputDevices(Option<Vec<String>>),
    AudioNewOutputDevices(Option<Vec<String>>),
    AudioChangePreferredKbits(i32),
    AudioChangeMuteState(bool),
    WakeUp,
}

#[derive(FromPrimitive)] #[derive(ToPrimitive)]
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
    OpusPacket=10
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

pub mod msg_types {
    use serde::{Serialize, Deserialize};
    use crate::common::encryption::NetworkedPublicKey;

    use super::Peer;
    
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
        pub callee: Peer,
        pub caller: Option<Peer>
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
}