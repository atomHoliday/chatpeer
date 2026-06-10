# ChatPeer

E2EE encrypted P2P chat in your GNOME top bar.

Discover other Linux users on your network and chat with them — no server, no accounts, no setup.

## Install

```bash
bash <(curl -sSf https://raw.githubusercontent.com/chatpeer/chatpeer/main/install.sh)
```

Or download the latest release from the [releases page](https://github.com/chatpeer/chatpeer/releases).

After installing, restart GNOME Shell (Alt+F2, type `r`, Enter). The chat icon will appear in your top bar.

## How it works

- **Peer discovery** — finds other ChatPeer users on your LAN via mDNS; over the internet via Kademlia DHT
- **Chat** — end-to-end encrypted with X25519 key exchange + XSalsa20-Poly1305 (NaCl box)
- **Online presence** — shows who's online with status (Online / Away / Busy)
- **No accounts** — each machine generates a unique identity on first run; your peer ID is your address
- **No server** — libp2p handles NAT traversal, peer routing, and message relay

## Architecture

```
┌─────────────────────────────────┐
│  GNOME Shell Extension (JS)     │
│  - top bar button               │
│  - peer list / chat UI          │
└──────────┬──────────────────────┘
           │ D-Bus (session bus)
┌──────────▼──────────────────────┐
│  chatpeer-daemon (Rust)         │
│  - libp2p swarm                 │
│  - mDNS + Kademlia              │
│  - gossipsub (presence)         │
│  - request-response (E2EE chat) │
│  - SQLite message store         │
└─────────────────────────────────┘
```

## Usage

1. Click the ChatPeer icon (user-available) in the top bar
2. Online peers appear in the menu — click one to start a chat
3. Type a message and press Enter — it's encrypted before it leaves your machine

## Build from source

```bash
git clone https://github.com/chatpeer/chatpeer
cd chatpeer
cargo build --release
bash install.sh
```

Requires Rust 1.75+ and the GNOME Shell development headers.

## License

MIT
