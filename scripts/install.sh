#!/usr/bin/env bash
# ============================================================
#  session-restore — plug-and-play installer
#  Saves open apps on shutdown, restores them on login
# ============================================================
set -euo pipefail

APP_NAME="session-restore"
INSTALL_DIR="/usr/local/bin"
SESSION_DIR="/var/lib/session-restore"
SERVICE_DIR="$HOME/.config/systemd/user"
VERSION="v0.3.0"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRC_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'
info() { echo -e "${GREEN}[+]${NC} $*"; }
warn() { echo -e "${YELLOW}[!]${NC} $*"; }
error() {
  echo -e "${RED}[✗]${NC} $*"
  exit 1
}
success() { echo -e "${GREEN}[✓]${NC} $*"; }

# ── 0. Uninstall mode ───────────────────────────────────────
if [[ "${1:-}" == "--uninstall" ]]; then
  info "Uninstalling $APP_NAME..."
  systemctl --user disable --now "${APP_NAME}-restore.service" 2>/dev/null || true
  systemctl --user disable --now "${APP_NAME}-save.service" 2>/dev/null || true
  rm -f "$SERVICE_DIR/${APP_NAME}-restore.service"
  rm -f "$SERVICE_DIR/${APP_NAME}-save.service"
  systemctl --user daemon-reload 2>/dev/null || true
  sudo rm -f "$INSTALL_DIR/$APP_NAME"
  success "Uninstalled. Sessions file kept at $SESSION_DIR/$USER/sessions.json"
  exit 0
fi

echo ""
echo "  ╔══════════════════════════════════════╗"
echo "  ║     session-restore  installer       ║"
echo "  ╚══════════════════════════════════════╝"
echo ""

# ── 1. Prerequisites ────────────────────────────────────────
info "Checking prerequisites..."

command -v systemctl &>/dev/null || error "systemd not found. This installer requires a systemd-based distro."

systemctl --user status &>/dev/null || {
  warn "systemd user session not fully running yet (this is OK during install)."
}

# ── 2. Build or download binary ─────────────────────────────
info "Checking for source or prebuilt binary..."

LATEST_URL="https://github.com/imrany/session-restore/releases/download/$VERSION/session-restore"
TMP_BIN="$(mktemp)"

if [[ -f "$SRC_DIR/src/main.rs" ]]; then
  command -v cargo &>/dev/null || error "Rust/cargo not found. Install via: curl https://sh.rustup.rs -sSf | sh"

  if [[ ! -d "$SRC_DIR/src" ]]; then
    mkdir -p "$SRC_DIR/src"
  fi
  if [[ -f "$SRC_DIR/main.rs" ]] && [[ ! -f "$SRC_DIR/src/main.rs" ]]; then
    info "Moving main.rs → src/main.rs (standard Cargo layout)..."
    cp "$SRC_DIR/main.rs" "$SRC_DIR/src/main.rs"
  fi
  [[ -f "$SRC_DIR/src/main.rs" ]] || error "src/main.rs not found."

  if [[ ! -f "$SRC_DIR/Cargo.toml" ]]; then
    info "No Cargo.toml found — generating one..."
    cat >"$SRC_DIR/Cargo.toml" <<'TOML'
[package]
name = "session-restore"
version = "0.2.0"
edition = "2021"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
sysinfo = "0.38"
TOML
    success "Cargo.toml created."
  else
    info "Existing Cargo.toml found — skipping generation."
  fi

  CARGO_BIN_NAME=$(grep -Po '(?<=^name = ")[^"]+' "$SRC_DIR/Cargo.toml" | head -1)
  APP_NAME="${CARGO_BIN_NAME:-session-restore}"
  info "Binary name: $APP_NAME"

  (cd "$SRC_DIR" && cargo build --release 2>&1) || error "Build failed."

  BINARY="$SRC_DIR/target/release/$APP_NAME"
  [[ -f "$BINARY" ]] || error "Binary not found at $BINARY"
  success "Build complete."
else
  info "No source found — downloading prebuilt binary..."
  if curl -fsSL "$LATEST_URL" -o "$TMP_BIN"; then
    BINARY="$TMP_BIN"
    success "Downloaded prebuilt binary."
  else
    error "Failed to download prebuilt binary from $LATEST_URL"
  fi
fi

# ── 3. Install binary ───────────────────────────────────────
info "Installing binary to $INSTALL_DIR/$APP_NAME ..."
sudo install -m 755 "$BINARY" "$INSTALL_DIR/$APP_NAME"
success "Binary installed."

# ── 4. Create sessions directory ────────────────────────────
info "Creating sessions directory $SESSION_DIR ..."
sudo mkdir -p "$SESSION_DIR"
sudo chmod 1777 "$SESSION_DIR"
success "Sessions directory ready."

# ── 5. Install systemd user services ────────────────────────
info "Installing systemd user services..."
mkdir -p "$SERVICE_DIR"

# ── 5a. Save-on-shutdown service ────────────────────────────
# Uses Conflicts= with graphical-session-pre.target so it fires
# when GNOME begins tearing down the session — before apps are killed
cat >"$SERVICE_DIR/${APP_NAME}-save.service" <<EOF
[Unit]
Description=Save open applications before shutdown
After=graphical-session.target
Before=graphical-session-pre.target shutdown.target
Conflicts=graphical-session-pre.target

[Service]
Type=oneshot
ExecStart=$INSTALL_DIR/$APP_NAME save
RemainAfterExit=yes
TimeoutStopSec=10

[Install]
WantedBy=graphical-session.target
EOF

# ── 5b. Restore-on-login service ────────────────────────────
# PassEnvironment ensures Wayland/X11/DBus vars are available
# ExecStartPre sleep gives the desktop time to fully settle
cat >"$SERVICE_DIR/${APP_NAME}-restore.service" <<EOF
[Unit]
Description=Restore open applications on login
After=graphical-session.target
Requires=graphical-session.target

[Service]
Type=oneshot
ExecStartPre=/bin/sleep 8
ExecStart=$INSTALL_DIR/$APP_NAME restore
PassEnvironment=DISPLAY WAYLAND_DISPLAY DBUS_SESSION_BUS_ADDRESS XDG_RUNTIME_DIR XDG_SESSION_TYPE XDG_CURRENT_DESKTOP

[Install]
WantedBy=graphical-session.target
EOF

# ── 5c. Enable both services ────────────────────────────────
systemctl --user daemon-reload
systemctl --user enable "${APP_NAME}-save.service"
systemctl --user enable "${APP_NAME}-restore.service"
success "systemd user services enabled."

# ── 6. Enable lingering ─────────────────────────────────────
info "Enabling systemd lingering for $USER..."
sudo loginctl enable-linger "$USER"
success "Lingering enabled."

# ── 7. Verify graphical-session.target is active ────────────
info "Checking graphical-session.target..."
if systemctl --user is-active graphical-session.target &>/dev/null; then
  success "graphical-session.target is active — services will hook correctly."
else
  warn "graphical-session.target is NOT active on this system."
  warn "The save/restore services may not fire automatically."
  warn "Run: systemctl --user status graphical-session.target"
  warn "for more details."
fi

# ── 8. Smoke test ───────────────────────────────────────────
info "Running a quick smoke test (save + list)..."
"$INSTALL_DIR/$APP_NAME" save &&
  "$INSTALL_DIR/$APP_NAME" list &&
  success "Smoke test passed."

# ── 9. Done ─────────────────────────────────────────────────
echo ""
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}  ✓  session-restore is installed!       ${NC}"
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo "  What happens now:"
echo "    • On shutdown  → your open apps are saved automatically"
echo "    • On login     → your saved apps are relaunched automatically"
echo ""
echo "  Manual commands:"
echo "    session-restore save     # snapshot right now"
echo "    session-restore restore  # relaunch saved apps"
echo "    session-restore list     # see what's saved"
echo "    session-restore info     # see app information"
echo ""
echo "  Saved session file: $SESSION_DIR/$USER/sessions.json"
echo ""
echo "  To uninstall:"
echo "    curl -fsSL https://raw.githubusercontent.com/imrany/session-restore/main/scripts/install.sh | bash -s -- --uninstall"
echo ""
