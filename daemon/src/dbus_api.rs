use crate::state::{AppState, Command};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use zbus::{interface, Connection};

pub struct ChatDbus {
    cmd_tx: mpsc::Sender<Command>,
    state: Arc<Mutex<AppState>>,
}

#[interface(name = "com.chatpeer.Daemon")]
impl ChatDbus {
    async fn send_message(&self, peer_id: &str, content: &str) -> zbus::fdo::Result<String> {
        let peer: libp2p::PeerId = peer_id
            .parse()
            .map_err(|e| zbus::fdo::Error::InvalidArgs(format!("invalid peer id: {e}")))?;
        let msg_id = self.state.lock().await.next_msg_id();
        let _ = self
            .cmd_tx
            .send(Command::SendMessage {
                peer,
                content: content.to_string(),
            })
            .await;
        Ok(msg_id)
    }

    async fn get_online_peers(&self) -> Vec<(String, String, String)> {
        let state = self.state.lock().await;
        state
            .peer_usernames
            .iter()
            .map(|(id, name)| (name.clone(), id.to_string(), "Online".into()))
            .collect()
    }

    async fn get_my_peer_id(&self) -> String {
        self.state.lock().await.my_peer_id.to_string()
    }

    async fn get_conversation(
        &self,
        peer_id: &str,
        limit: u32,
    ) -> Vec<(String, String, bool, String, u64, bool)> {
        let state = self.state.lock().await;
        match state.message_store.get_conversation(peer_id, limit) {
            Ok(messages) => messages
                .into_iter()
                .map(|m| {
                    (
                        m.id,
                        m.peer_id,
                        m.is_outgoing,
                        m.content,
                        m.timestamp,
                        m.delivered,
                    )
                })
                .collect(),
            Err(e) => {
                tracing::error!("get_conversation: {e}");
                vec![]
            }
        }
    }

    async fn list_conversations(&self) -> Vec<String> {
        let state = self.state.lock().await;
        state
            .message_store
            .all_conversations()
            .unwrap_or_default()
    }
}

pub async fn run_dbus_server(
    cmd_tx: mpsc::Sender<Command>,
    state: Arc<Mutex<AppState>>,
) -> Result<Connection> {
    let dbus = ChatDbus { cmd_tx, state };
    let conn = Connection::session()
        .await
        .map_err(|e| anyhow::anyhow!("D-Bus session connection: {e}"))?;
    conn.request_name("com.chatpeer.Daemon")
        .await
        .map_err(|e| anyhow::anyhow!("D-Bus name request: {e}"))?;
    let _ = conn
        .object_server()
        .at("/com/chatpeer/Daemon", dbus)
        .await;
    Ok(conn)
}
