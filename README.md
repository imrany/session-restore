# session-restore

`session-restore` automatically saves your running applications before shutdown and restores them once you log in — resuming exactly where you left off.

### Features
- 🔄 Save your open apps automatically on shutdown
- 🚀 Restore them seamlessly on login
- 📋 Manual commands for snapshot, restore, and listing sessions
- ⚙️ Systemd user services for plug‑and‑play setup
- 🗂 Sessions stored in `/var/lib/session-restore/sessions.json`

### 🚀 Installation

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

### Usage

```bash
session-restore save     # snapshot running apps now
session-restore restore  # relaunch saved apps
session-restore list     # show saved apps
session-restore info     # show app information
```

### Uninstall

```bash
curl -fsSL https://raw.githubusercontent.com/imrany/session-restore/main/scripts/install.sh | bash -s -- --uninstall
```

This removes the binary and services but keeps your saved sessions file.

### 🛠 Core Idea
- It’s a small CLI tool that can **snapshot your running applications** before shutdown and then **relaunch them automatically** when you log back in.
- It integrates with **systemd user services** so the save/restore happens automatically at shutdown and login.

### ⚙️ Workflow
1. **On shutdown**  
   - The systemd *save service* runs `session-restore save`.  
   - This command inspects your current user processes (using `sysinfo` in Rust).  
   - It records the list of running apps into a JSON file at `/var/lib/session-restore/sessions.json`.

2. **On login**  
   - The systemd *restore service* runs `session-restore restore`.  
   - This command reads the JSON file and relaunches each saved app, so your desktop session resumes where you left off.

3. **Manual commands**  
   - `session-restore save` → snapshot apps immediately.  
   - `session-restore restore` → relaunch saved apps.  
   - `session-restore list` → show what’s currently saved in the JSON file.  
   - `session-restore info` → display app metadata (version, config paths, etc.).


### 🔄 How the CLI behaves
- If you run `session-restore` with **no arguments**, it defaults to `_`.  
- If you run `cargo run` (no args) during development, it also defaults to `_` — unless you change the default to `_` (usage/help).  
- If you pass a subcommand (`save`, `list`, `info`, `restore`), it executes that branch.

### 📂 Data storage
- Sessions are stored in `/var/lib/session-restore/sessions.json`.  
- The file contains a serialized list of apps and their launch commands.  
- Each user’s files are protected by sticky‑bit permissions so users only manage their own sessions.


So in practice:  
- End‑users just run the one‑liner installer.  
- After that, they don’t need to think about it — their apps will be saved on shutdown and restored on login.  
- Developers can still build from source with Cargo if they want to hack on the code.
