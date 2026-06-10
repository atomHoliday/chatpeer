mod config;
mod crypto;
mod db;
mod dbus_api;
mod protocol;
mod state;

use anyhow::Result;
use async_trait::async_trait;
use crypto::ChatCrypto;
use futures::prelude::*;
use futures::StreamExt;
use libp2p::{
    gossipsub, identify, identity::Keypair, kad, mdns, noise, ping,
    request_response::{self, Codec, ProtocolSupport},
    swarm::{NetworkBehaviour, StreamProtocol, SwarmEvent},
    yamux, Multiaddr, PeerId, SwarmBuilder,
};
use protocol::{OnlineStatus, PresenceMessage, PRESENCE_TOPIC};
use serde::{Deserialize, Serialize};
use state::{AppState, Command};
use std::{
    collections::HashMap,
    io,
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{mpsc, Mutex};
use tracing_subscriber::EnvFilter;

const KEYPAIR_PATH: &str = "keypair";

fn load_or_generate_keypair(data_dir: &PathBuf) -> Result<Keypair> {
    let key_path = data_dir.join(KEYPAIR_PATH);
    if key_path.exists() {
        let bytes = std::fs::read(&key_path)?;
        let keypair = Keypair::from_protobuf_encoding(&bytes)?;
        tracing::info!("loaded existing keypair");
        Ok(keypair)
    } else {
        let keypair = Keypair::generate_ed25519();
        let bytes = keypair.to_protobuf_encoding()?;
        std::fs::create_dir_all(data_dir)?;
        std::fs::write(&key_path, bytes)?;
        tracing::info!("generated new keypair");
        Ok(keypair)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatEnvelope {
    sender_pub: [u8; 32],
    encrypted: Vec<u8>,
    timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Ack {
    msg_id: String,
}

#[derive(Default, Clone)]
struct JsonCodec;

#[async_trait]
impl Codec for JsonCodec {
    type Protocol = StreamProtocol;
    type Request = Vec<u8>;
    type Response = Vec<u8>;

    async fn read_request<T>(&mut self, _protocol: &Self::Protocol, io: &mut T) -> io::Result<Vec<u8>>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut buf = Vec::new();
        io.take(1024 * 1024).read_to_end(&mut buf).await?;
        Ok(buf)
    }

    async fn read_response<T>(&mut self, _protocol: &Self::Protocol, io: &mut T) -> io::Result<Vec<u8>>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut buf = Vec::new();
        io.take(10 * 1024 * 1024).read_to_end(&mut buf).await?;
        Ok(buf)
    }

    async fn write_request<T>(&mut self, _protocol: &Self::Protocol, io: &mut T, req: Vec<u8>) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        io.write_all(&req).await?;
        Ok(())
    }

    async fn write_response<T>(&mut self, _protocol: &Self::Protocol, io: &mut T, res: Vec<u8>) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        io.write_all(&res).await?;
        Ok(())
    }
}

#[derive(NetworkBehaviour)]
struct ChatBehaviour {
    mdns: mdns::tokio::Behaviour,
    kad: kad::Behaviour<kad::store::MemoryStore>,
    gossipsub: gossipsub::Behaviour,
    chat: request_response::Behaviour<JsonCodec>,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = config::Config::load_or_default()?;
    tracing::info!(
        "starting chatpeer daemon for user: {}",
        config.identity.username
    );

    let data_dir = config.data_dir();
    std::fs::create_dir_all(&data_dir)?;
    let keypair = load_or_generate_keypair(&data_dir)?;
    let crypto = ChatCrypto::load_or_generate(&data_dir)?;
    let my_peer_id = PeerId::from(keypair.public());
    tracing::info!("peer id: {}", my_peer_id);

    let message_store = db::MessageStore::open(&config.db.path)?;

    let (cmd_tx, mut cmd_rx) = mpsc::channel::<Command>(64);

    let state = Arc::new(Mutex::new(AppState {
        crypto,
        username: config.identity.username.clone(),
        my_peer_id,
        peer_pubkeys: HashMap::new(),
        peer_usernames: HashMap::new(),
        msg_counter: 0,
        message_store,
    }));

    let mut swarm = SwarmBuilder::with_existing_identity(keypair.clone())
        .with_tokio()
        .with_tcp(
            libp2p::tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(|key| {
            let local_peer_id = PeerId::from(key.public());

            let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)?;

            let kad_store = kad::store::MemoryStore::new(local_peer_id);
            let kad = kad::Behaviour::new(local_peer_id, kad_store);

            let gossipsub_config = gossipsub::ConfigBuilder::default()
                .message_id_fn(|msg: &gossipsub::Message| {
                    gossipsub::MessageId::from(&msg.data[..msg.data.len().min(20)])
                })
                .build()
                .map_err(|e| anyhow::anyhow!("gossipsub config: {e}"))?;
            let message_auth = gossipsub::MessageAuthenticity::Signed(key.clone());
            let gossipsub = gossipsub::Behaviour::new(message_auth, gossipsub_config)?;

            let chat = request_response::Behaviour::with_codec(
                JsonCodec,
                vec![(
                    StreamProtocol::new("/chatpeer/chat/1.0.0"),
                    ProtocolSupport::Full,
                )],
                request_response::Config::default(),
            );

            let identify = identify::Behaviour::new(identify::Config::new(
                "/chatpeer/1.0.0".to_string(),
                key.public(),
            ));

            let ping = ping::Behaviour::new(ping::Config::new());

            Ok(ChatBehaviour {
                mdns,
                kad,
                gossipsub,
                chat,
                identify,
                ping,
            })
        })?
        .build();

    let topic = gossipsub::IdentTopic::new(PRESENCE_TOPIC);
    swarm.behaviour_mut().gossipsub.subscribe(&topic)?;

    // publish initial presence
    {
        let state_guard = state.lock().await;
        if let Ok(data) = serde_json::to_vec(&PresenceMessage {
            username: state_guard.username.clone(),
            status: OnlineStatus::Online,
            public_key: state_guard.crypto.public_key_bytes(),
        }) {
            let _ = swarm.behaviour_mut().gossipsub.publish(topic.clone(), data);
        }
    }

    // periodic presence republish
    let tx_periodic = cmd_tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            if tx_periodic
                .send(Command::SetStatus {
                    status: OnlineStatus::Online,
                })
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // Start D-Bus server
    let dbus_conn = dbus_api::run_dbus_server(cmd_tx.clone(), state.clone()).await?;

    // Bootstrap Kademlia after a short delay
    let bootstrap_after = tokio::time::sleep(Duration::from_secs(5));
    tokio::pin!(bootstrap_after);

    for addr in &config.network.listen_on {
        let multiaddr: Multiaddr = addr.parse()?;
        swarm.listen_on(multiaddr)?;
    }

    // main event loop
    loop {
        tokio::select! {
            biased;
            _ = &mut bootstrap_after => {
                let _ = swarm.behaviour_mut().kad.bootstrap();
            }
            event = swarm.select_next_some() => {
                handle_swarm_event(&mut swarm, &state, &dbus_conn, event).await;
            }
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(cmd) => handle_command(&mut swarm, &state, cmd).await,
                    None => return Ok(()),
                }
            }
        }
    }
}


async fn handle_command(
    swarm: &mut libp2p::swarm::Swarm<ChatBehaviour>,
    state: &Arc<Mutex<AppState>>,
    cmd: Command,
) {
    match cmd {
        Command::SetStatus { status } => {
            let state_guard = state.lock().await;
            if let Ok(data) = serde_json::to_vec(&PresenceMessage {
                username: state_guard.username.clone(),
                status,
                public_key: state_guard.crypto.public_key_bytes(),
            }) {
                let topic = gossipsub::IdentTopic::new(PRESENCE_TOPIC);
                let _ = swarm.behaviour_mut().gossipsub.publish(topic, data);
            }
        }
        Command::SendMessage { peer, content } => {
            let mut state_guard = state.lock().await;
            if let Some(pubkey) = state_guard.peer_pubkeys.get(&peer) {
                match state_guard.crypto.encrypt(pubkey, content.as_bytes()) {
                    Ok(encrypted) => {
                        let timestamp = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        if let Ok(data) = serde_json::to_vec(&ChatEnvelope {
                            sender_pub: state_guard.crypto.public_key_bytes(),
                            encrypted,
                            timestamp,
                        }) {
                            let msg_id = state_guard.next_msg_id();
                            let peer_str = peer.to_string();
                            if let Err(e) = state_guard.message_store.store_message(
                                &msg_id,
                                &peer_str,
                                true,
                                &content,
                                timestamp,
                            ) {
                                tracing::error!("store outgoing msg: {e}");
                            }
                            drop(state_guard);
                            swarm.behaviour_mut().chat.send_request(&peer, data);
                        }
                    }
                    Err(e) => tracing::error!("encrypt failed: {e}"),
                }
            } else {
                tracing::warn!("no public key for peer {peer}");
            }
        }
    }
}

async fn handle_swarm_event(
    swarm: &mut libp2p::swarm::Swarm<ChatBehaviour>,
    state: &Arc<Mutex<AppState>>,
    dbus_conn: &zbus::Connection,
    event: libp2p::swarm::SwarmEvent<<ChatBehaviour as NetworkBehaviour>::ToSwarm>,
) {
    match event {
        SwarmEvent::Behaviour(ChatBehaviourEvent::Mdns(event)) => match event {
            mdns::Event::Discovered(list) => {
                for (peer_id, addr) in list {
                    tracing::info!("mDNS discovered: {peer_id} at {addr}");
                    if peer_id != *swarm.local_peer_id() {
                        if let Err(e) = swarm.dial(addr.clone()) {
                            tracing::warn!("dial {peer_id} failed: {e}");
                        }
                    }
                }
            }
            mdns::Event::Expired(list) => {
                for (peer_id, _addr) in list {
                    tracing::info!("mDNS expired: {peer_id}");
                }
            }
        },
        SwarmEvent::Behaviour(ChatBehaviourEvent::Kad(event)) => {
            tracing::debug!("Kademlia event: {event:?}");
        }
        SwarmEvent::Behaviour(ChatBehaviourEvent::Gossipsub(event)) => {
            if let gossipsub::Event::Message {
                propagation_source,
                message,
                ..
            } = event
            {
                if propagation_source == *swarm.local_peer_id() {
                    return;
                }
                if let Ok(presence) =
                    serde_json::from_slice::<PresenceMessage>(&message.data)
                {
                    let is_new = {
                        let mut state_guard = state.lock().await;
                        let is_new = !state_guard.peer_usernames.contains_key(&propagation_source);
                        state_guard
                            .peer_pubkeys
                            .insert(propagation_source, presence.public_key);
                        state_guard
                            .peer_usernames
                            .insert(propagation_source, presence.username.clone());
                        is_new
                    };
                    if is_new {
                        let _ = dbus_conn
                            .emit_signal(
                                None::<&str>,
                                "/com/chatpeer/Daemon",
                                "com.chatpeer.Daemon",
                                "PeerOnline",
                                &(
                                    propagation_source.to_string(),
                                    presence.username.clone(),
                                ),
                            )
                            .await;
                    }
                }
            }
        }
        SwarmEvent::Behaviour(ChatBehaviourEvent::Chat(event)) => match event {
            request_response::Event::Message {
                peer,
                message:
                    request_response::Message::Request {
                        request,
                        channel,
                        ..
                    },
                ..
            } => {
                let mut state_guard = state.lock().await;
                if let Ok(envelope) = serde_json::from_slice::<ChatEnvelope>(&request) {
                    let key_ok = state_guard
                        .peer_pubkeys
                        .get(&peer)
                        .map_or(false, |stored| *stored == envelope.sender_pub);
                    if key_ok {
                        if let Ok(plaintext) = state_guard
                            .crypto
                            .decrypt(&envelope.sender_pub, &envelope.encrypted)
                        {
                            let msg_text = String::from_utf8_lossy(&plaintext).to_string();
                            tracing::info!("decrypted msg from {peer}: {msg_text}");
                            let username = state_guard
                                .peer_usernames
                                .get(&peer)
                                .cloned()
                                .unwrap_or_default();
                            let msg_id = state_guard.next_msg_id();
                            let peer_str = peer.to_string();
                            if let Err(e) = state_guard.message_store.store_message(
                                &msg_id,
                                &peer_str,
                                false,
                                &msg_text,
                                envelope.timestamp,
                            ) {
                                tracing::error!("store incoming msg: {e}");
                            }
                            drop(state_guard);
                            let _ = dbus_conn
                                .emit_signal(
                                    None::<&str>,
                                    "/com/chatpeer/Daemon",
                                    "com.chatpeer.Daemon",
                                    "MessageReceived",
                                    &(peer.to_string(), username, msg_text, msg_id.clone()),
                                )
                                .await;
                            if let Ok(ack) = serde_json::to_vec(&Ack { msg_id }) {
                                let _ = swarm.behaviour_mut().chat.send_response(channel, ack);
                            }
                        } else {
                            tracing::warn!("decrypt failed for {peer}");
                        }
                    }
                }
            }
            request_response::Event::Message {
                peer,
                message: request_response::Message::Response { response, .. },
                ..
            } => {
                if let Ok(ack) = serde_json::from_slice::<Ack>(&response) {
                    tracing::info!("chat ack from {peer} for msg {}", ack.msg_id);
                }
            }
            request_response::Event::OutboundFailure {
                peer, error, ..
            } => {
                tracing::warn!("outbound to {peer} failed: {error:?}");
            }
            request_response::Event::InboundFailure {
                peer, error, ..
            } => {
                tracing::warn!("inbound from {peer} failed: {error:?}");
            }
            _ => {}
        },
        SwarmEvent::Behaviour(ChatBehaviourEvent::Identify(event)) => {
            if let identify::Event::Received { peer_id, .. } = event {
                tracing::info!("identified {peer_id}");
            }
        }
        SwarmEvent::Behaviour(ChatBehaviourEvent::Ping(event)) => match event.result {
            Ok(rtt) => {
                tracing::debug!("ping to {}: {}ms", event.peer, rtt.as_millis());
            }
            Err(_) => {
                tracing::warn!("ping to {} failed", event.peer);
            }
        },
        SwarmEvent::IncomingConnection { .. } => {}
        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
            tracing::info!("connected to {peer_id}");
        }
        SwarmEvent::ConnectionClosed { peer_id, .. } => {
            tracing::info!("disconnected from {peer_id}");
        }
        _ => {}
    }
}
