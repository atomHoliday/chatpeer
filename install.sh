#!/usr/bin/env bash
set -euo pipefail

REPO="chatpeer/chatpeer"
VERSION="${1:-latest}"

DEST_BIN="${HOME}/.local/bin"
DEST_SERVICE="${HOME}/.config/systemd/user"
DEST_EXTENSION="${HOME}/.local/share/gnome-shell/extensions/chatpeer@chatpeer.local"

# Detect if running from a local checkout (build from source)
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
if [ -f "$SCRIPT_DIR/Cargo.toml" ] && [ -d "$SCRIPT_DIR/daemon" ]; then
  echo "==> Local checkout detected — building from source..."
  cd "$SCRIPT_DIR"
  if ! command -v cargo &>/dev/null; then
    echo "Error: cargo not found. Install Rust: https://rustup.rs" >&2
    exit 1
  fi
  cargo build --release
  BIN="$SCRIPT_DIR/target/release/chatpeer-daemon"
  EXT_DIR="$SCRIPT_DIR/extension"
  SVC_FILE="$SCRIPT_DIR/daemon/chatpeer.service"
else
  # Download pre-built release from GitHub
  echo "==> Downloading ChatPeer release..."
  if [ "$VERSION" = "latest" ]; then
    DOWNLOAD_URL="https://github.com/$REPO/releases/latest/download/chatpeer-x86_64-linux.tar.gz"
  else
    DOWNLOAD_URL="https://github.com/$REPO/releases/download/$VERSION/chatpeer-x86_64-linux.tar.gz"
  fi

  TMPDIR="$(mktemp -d)"
  trap 'rm -rf "$TMPDIR"' EXIT

  if command -v curl &>/dev/null; then
    curl -sSfL "$DOWNLOAD_URL" -o "$TMPDIR/release.tar.gz"
  elif command -v wget &>/dev/null; then
    wget -q "$DOWNLOAD_URL" -O "$TMPDIR/release.tar.gz"
  else
    echo "Error: need curl or wget" >&2
    exit 1
  fi

  tar xzf "$TMPDIR/release.tar.gz" -C "$TMPDIR"
  EXTRACTED="$TMPDIR/chatpeer-x86_64-linux"
  BIN="$EXTRACTED/chatpeer-daemon"
  EXT_DIR="$EXTRACTED/extension"
  SVC_FILE="$EXTRACTED/chatpeer.service"
fi

echo "==> Installing daemon binary..."
mkdir -p "$DEST_BIN"
cp "$BIN" "$DEST_BIN/chatpeer-daemon"
chmod +x "$DEST_BIN/chatpeer-daemon"

echo "==> Installing systemd user service..."
mkdir -p "$DEST_SERVICE"
cp "$SVC_FILE" "$DEST_SERVICE/chatpeer.service"
systemctl --user daemon-reload
systemctl --user enable chatpeer.service
systemctl --user start chatpeer.service

echo "==> Installing GNOME Shell extension..."
mkdir -p "$DEST_EXTENSION"
cp "$EXT_DIR"/*.js "$DEST_EXTENSION/"
cp "$EXT_DIR/metadata.json" "$DEST_EXTENSION/"
cp "$EXT_DIR/stylesheet.css" "$DEST_EXTENSION/"

echo "==> Registering extension for next GNOME Shell restart..."
CURRENT_EXTENSIONS="$(gsettings get org.gnome.shell enabled-extensions 2>/dev/null || echo "[]")"
if ! echo "$CURRENT_EXTENSIONS" | grep -q "chatpeer@chatpeer.local"; then
  NEW_EXTENSIONS="$(echo "$CURRENT_EXTENSIONS" | sed "s/\]$/,\"chatpeer@chatpeer.local\"\]/")"
  gsettings set org.gnome.shell enabled-extensions "$NEW_EXTENSIONS" 2>/dev/null || true
fi

echo ""
echo "  ChatPeer installed!"
echo ""
echo "  Restart GNOME Shell (Alt+F2, type 'r', Enter) or log out and back in."
echo "  The extension will then appear in your top bar."
echo ""
echo "  To check the daemon: systemctl --user status chatpeer.service"
echo "  To view logs:        journalctl --user -u chatpeer.service -f"
