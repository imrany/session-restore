# session-restore

`session-restore` automatically saves your running applications before shutdown and restores them once you log in — resuming exactly where you left off.

## Features
- 🔄 Save your open apps automatically on shutdown
- 🚀 Restore them seamlessly on login
- 📋 Manual commands for snapshot, restore, and listing sessions
- ⚙️ Systemd user services for plug‑and‑play setup
- 🗂 Sessions stored in `/var/lib/session-restore/sessions.json`

## Installation

To install the latest release binary directly from GitHub:

```bash
curl -fsSL https://raw.githubusercontent.com/imrany/session-restore/main/scripts/install.sh | bash
```

This script will:
- Download the latest prebuilt binary from GitHub Releases if no source is present
- Or build from source with Cargo if `src/main.rs` exists locally
- Install the binary into `/usr/local/bin`
- Set up systemd user services for save/restore
- Run a quick smoke test to verify installation

## Usage

```bash
session-restore save     # snapshot running apps now
session-restore restore  # relaunch saved apps
session-restore list     # show saved apps
session-restore info     # show app information
```

## Uninstall

```bash
curl -fsSL https://raw.githubusercontent.com/imrany/session-restore/main/scripts/install.sh | bash install.sh --uninstall
```

This removes the binary and services but keeps your saved sessions file.
