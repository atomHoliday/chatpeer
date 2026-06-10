use crate::crypto::ChatCrypto;
use crate::db::MessageStore;
use crate::protocol;
use libp2p::PeerId;
use std::collections::HashMap;

#[derive(Debug)]
pub enum Command {
    SendMessage { peer: PeerId, content: String },
    SetStatus { status: protocol::OnlineStatus },
}

pub struct AppState {
    pub crypto: ChatCrypto,
    pub username: String,
    pub my_peer_id: PeerId,
    pub peer_pubkeys: HashMap<PeerId, [u8; 32]>,
    pub peer_usernames: HashMap<PeerId, String>,
    pub msg_counter: u64,
    pub message_store: MessageStore,
}

impl AppState {
    pub fn next_msg_id(&mut self) -> String {
        self.msg_counter += 1;
        format!("msg_{}", self.msg_counter)
    }
}
