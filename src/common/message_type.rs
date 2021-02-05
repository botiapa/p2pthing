use std::net::SocketAddr;
use num_derive::{FromPrimitive, ToPrimitive};
use serde::{Serialize, Deserialize};

use super::encryption::NetworkedPublicKey;

#[derive(FromPrimitive)] #[derive(ToPrimitive)]
pub enum MsgType {
    Announce=0,
    Call=1,
    CallResponse=2,
    Disconnect=3,
    KeepAlive=4,
    ChatMessage=5
}
#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Peer {
    pub addr: Option<SocketAddr>,
    pub udp_addr: Option<SocketAddr>,
    pub public_key: NetworkedPublicKey
}

pub mod MsgTypes {
    use serde::{Serialize, Deserialize};
    use crate::common::encryption::NetworkedPublicKey;

    use super::Peer;
   
    #[derive(Serialize, Deserialize)]
    pub struct Announce {
        pub public_key: NetworkedPublicKey
    }
    
    #[derive(Serialize, Deserialize, Clone)]
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
    pub struct Disconnect {
        pub public_key: NetworkedPublicKey
    }
}