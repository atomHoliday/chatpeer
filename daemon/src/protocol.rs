use serde::{Deserialize, Serialize};

pub const PRESENCE_TOPIC: &str = "/chatpeer/presence/1.0.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OnlineStatus {
    Online,
    Away,
    Busy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceMessage {
    pub username: String,
    pub status: OnlineStatus,
    pub public_key: [u8; 32],
}
