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
VERSION="v0.2.0"
# Get the folder where this script is, then go up one level to the project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRC_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
info()    { echo -e "${GREEN}[+]${NC} $*"; }
warn()    { echo -e "${YELLOW}[!]${NC} $*"; }
error()   { echo -e "${RED}[✗]${NC} $*"; exit 1; }
success() { echo -e "${GREEN}[✓]${NC} $*"; }

# ── 0. Uninstall mode ───────────────────────────────────────
if [[ "${1:-}" == "--uninstall" ]]; then
    info "Uninstalling $APP_NAME..."
    systemctl --user disable --now "${APP_NAME}-restore.service" 2>/dev/null || true
    systemctl --user disable --now "${APP_NAME}-save.service"    2>/dev/null || true
    rm -f "$SERVICE_DIR/${APP_NAME}-restore.service"
    rm -f "$SERVICE_DIR/${APP_NAME}-save.service"
    systemctl --user daemon-reload 2>/dev/null || true
    sudo rm -f "$INSTALL_DIR/$APP_NAME"
    success "Uninstalled. Sessions file kept at $SESSION_DIR/sessions.json"
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

# Check systemd user sessions are working
systemctl --user status &>/dev/null || {
    warn "systemd user session not fully running yet (this is OK during install)."
}

# ── 2. Download Prebuilt Binary from GitHub ──────────────────────────────
info "Downloading prebuilt binary from GitHub Releases..."

LATEST_URL="https://github.com/imrany/session-restore/releases/download/$VERSION/session-restore"
TMP_BIN="$(mktemp)"

if [[ -f "$SRC_DIR/src/main.rs" ]]; then
    # checks if cargo exists
    command -v cargo &>/dev/null || error "Rust/cargo not found. Install via: curl https://sh.rustup.rs -sSf | sh"
    # Ensure src/main.rs exists (Cargo standard layout)
    if [[ ! -d "$SRC_DIR/src" ]]; then
        mkdir -p "$SRC_DIR/src"
    fi
    if [[ -f "$SRC_DIR/main.rs" ]] && [[ ! -f "$SRC_DIR/src/main.rs" ]]; then
        info "Moving main.rs → src/main.rs (standard Cargo layout)..."
        cp "$SRC_DIR/main.rs" "$SRC_DIR/src/main.rs"
    fi
    [[ -f "$SRC_DIR/src/main.rs" ]] || error "src/main.rs not found. Place main.rs next to install.sh and re-run."

    # Only create Cargo.toml if one doesn't already exist
    if [[ ! -f "$SRC_DIR/Cargo.toml" ]]; then
        info "No Cargo.toml found — generating one..."
        cat > "$SRC_DIR/Cargo.toml" << 'TOML'
[package]
name = "session-restore"
version = "0.1.0"
edition = "2024"

[dependencies]
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.149"
sysinfo = "0.38.4"
TOML
        success "Cargo.toml created."
    else
        info "Existing Cargo.toml found — skipping generation."
    fi

    # Detect binary name from Cargo.toml (handles custom package names)
    CARGO_BIN_NAME=$(grep -Po '(?<=^name = ")[^"]+' "$SRC_DIR/Cargo.toml" | head -1)
    APP_NAME="${CARGO_BIN_NAME:-session-restore}"
    info "Binary name: $APP_NAME"

    (cd "$SRC_DIR" && cargo build --release 2>&1) || error "Build failed."

    BINARY="$SRC_DIR/target/release/$APP_NAME"
    [[ -f "$BINARY" ]] || error "Binary not found at $BINARY — check [package] name in Cargo.toml"
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
sudo chmod 1777 "$SESSION_DIR"   # sticky bit — each user owns their own files
success "Sessions directory ready."

# ── 5. Install systemd user services ────────────────────────
info "Installing systemd user services..."
mkdir -p "$SERVICE_DIR"

# ── 5a. Save-on-shutdown service ────────────────────────────
cat > "$SERVICE_DIR/${APP_NAME}-save.service" << EOF
[Unit]
Description=Save open applications before shutdown
DefaultDependencies=no
Before=shutdown.target reboot.target halt.target

[Service]
Type=oneshot
ExecStart=$INSTALL_DIR/$APP_NAME save
RemainAfterExit=yes
TimeoutStopSec=10

[Install]
WantedBy=shutdown.target
EOF

# ── 5b. Restore-on-login service ────────────────────────────
cat > "$SERVICE_DIR/${APP_NAME}-restore.service" << EOF
[Unit]
Description=Restore open applications on login
After=graphical-session.target
Requires=graphical-session.target

[Service]
Type=oneshot
ExecStart=$INSTALL_DIR/$APP_NAME restore
Environment=DISPLAY=:0
Environment=DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/%U/bus
RemainAfterExit=no

[Install]
WantedBy=graphical-session.target
EOF

# ── 5c. Enable both services ────────────────────────────────
systemctl --user daemon-reload
systemctl --user enable "${APP_NAME}-save.service"
systemctl --user enable "${APP_NAME}-restore.service"
success "systemd user services enabled."

# ── 6. Enable lingering (so user services survive after logout) ──
info "Enabling systemd lingering for $USER..."
sudo loginctl enable-linger "$USER"
success "Lingering enabled."

# ── 7. Smoke test ───────────────────────────────────────────
info "Running a quick smoke test (save + list)..."
"$INSTALL_DIR/$APP_NAME" save  && \
"$INSTALL_DIR/$APP_NAME" list  && \
success "Smoke test passed."

# ── 8. Done ─────────────────────────────────────────────────
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
echo "  To uninstall: curl -fsSL https://raw.githubusercontent.com/imrany/session-restore/main/scripts/install.sh | bash -s -- --uninstall"
echo ""
