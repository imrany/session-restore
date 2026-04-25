# session-restore — Developer Guide

> Architecture, internals, and contribution guide. v0.2.0 — Rust — Linux (systemd)


## Overview

`session-restore` is a Rust CLI tool that snapshots running GUI applications before shutdown and relaunches them on login via systemd user services. It uses the `sysinfo` crate to enumerate processes, filters them down to real user-facing apps, and persists the result as a JSON file.

| | |
|---|---|
| **Language** | Rust (edition 2021) |
| **Dependencies** | `sysinfo 0.38`, `serde 1.0`, `serde_json 1.0` |
| **Install path** | `/usr/local/bin/session-restore` |
| **Session file** | `/var/lib/session-restore/<user>/sessions.json` |
| **Services** | `~/.config/systemd/user/session-restore-{save,restore}.service` |


## Repository Layout

```
session-restore/
├── src/
│   └── main.rs          # entire CLI implementation
├── scripts/
│   └── install.sh       # installer + systemd service generator
├── Cargo.toml
├── Cargo.lock
└── README.md
```

All application logic lives in a single file (`src/main.rs`) intentionally — the tool is small enough that splitting into modules would add navigation overhead without benefit.


## Architecture

### CLI entry point

`main()` reads the first argument and dispatches to one of four functions. Unknown or missing arguments print usage and exit with code 1.

```
save    → save_session()
restore → restore_session()
list    → list_session()
info    → get_app_info()
```


### `save_session()`

The core snapshot function. It:

1. Initialises `sysinfo::System` and refreshes all processes
2. Identifies the current user's UID from the self process
3. Iterates all processes, applying four filters:
   - `user_id == my_uid` — only this user's processes
   - `in_allowed_path` — exe is under `/opt`, `/usr/bin`, `/usr/local/bin`, `/snap`, `/flatpak`, `/home`
   - `!is_helper` — name/path does not match any blacklisted keyword
   - `run_time() >= 30` — process has been alive for at least 30 seconds
4. Calls `resolve_launch_command()` to remap internal binaries to their proper wrapper scripts
5. Serialises the deduplicated set to JSON and writes it to the per-user sessions file


### `restore_session()`

Reads the sessions JSON, waits 8 seconds for the desktop to settle, then for each saved app:

- Skips if the binary path no longer exists on disk
- Checks `sysinfo` for a running process with a matching exe path — skips if already running
- Spawns the process with the full display environment (`DISPLAY`, `WAYLAND_DISPLAY`, `DBUS_SESSION_BUS_ADDRESS`, `XDG_RUNTIME_DIR`, `XDG_SESSION_TYPE`, `XDG_CURRENT_DESKTOP`)
- Redirects `stdin`/`stdout`/`stderr` to null so apps don't pollute the terminal or journal


### `resolve_launch_command()`

Some apps use an internal binary that cannot be launched directly — they rely on a wrapper script to set up library paths and environment. This function maps known internal paths to their public launchers:

| Internal binary | Launcher used |
|---|---|
| `/opt/google/chrome/chrome` | `/usr/bin/google-chrome` |
| `/opt/google/chrome-beta/chrome` | `/usr/bin/google-chrome-beta` |
| `/opt/brave.com/brave/brave` | `/usr/bin/brave-browser` |
| `/opt/vivaldi/vivaldi-bin` | `/usr/bin/vivaldi` |
| `/opt/opera/opera` | `/usr/bin/opera` |
| `/opt/slack/slack` | `/usr/bin/slack` |
| `/opt/discord/Discord` | `/usr/bin/discord` |

If no mapping exists, the function falls back to using the exe path directly — but only if the file actually exists on disk.


### `sessions_file()`

Returns a per-user path at runtime. In debug builds (`cargo run`) it returns `./sessions.json` in the working directory. In release builds it returns `/var/lib/session-restore/$USER/sessions.json`.

> **Important:** This must be a function, not a `const`. The username is resolved at runtime from the `USER` or `LOGNAME` environment variable — it cannot be known at compile time.


## Systemd Services

### `session-restore-save.service`

Installed to `~/.config/systemd/user/`. Configured to start with `graphical-session.target` and conflict with `graphical-session-pre.target` — this causes it to trigger when GNOME begins tearing down the session, before apps are killed.

```ini
[Unit]
After=graphical-session.target
Before=graphical-session-pre.target shutdown.target
Conflicts=graphical-session-pre.target

[Service]
Type=oneshot
ExecStart=/usr/local/bin/session-restore save
RemainAfterExit=yes
TimeoutStopSec=10

[Install]
WantedBy=graphical-session.target
```

### `session-restore-restore.service`

Waits for `graphical-session.target`, runs an 8-second pre-sleep, then launches restore. Uses `PassEnvironment` to forward the full Wayland/X11 session environment from systemd into the restore process.

```ini
[Unit]
After=graphical-session.target
Requires=graphical-session.target

[Service]
Type=oneshot
ExecStartPre=/bin/sleep 8
ExecStart=/usr/local/bin/session-restore restore
PassEnvironment=DISPLAY WAYLAND_DISPLAY DBUS_SESSION_BUS_ADDRESS XDG_RUNTIME_DIR XDG_SESSION_TYPE XDG_CURRENT_DESKTOP

[Install]
WantedBy=graphical-session.target
```


## Building from Source

### Prerequisites

- `rustup` / Cargo (stable toolchain)
- Linux with systemd
- `libssl-dev` (for some sysinfo builds — usually pre-installed)

### Development build

```bash
git clone https://github.com/imrany/session-restore
cd session-restore
cargo build
cargo run save          # saves to ./sessions.json (debug path)
cargo run list
cargo run restore
```

### Release build and install

```bash
cargo build --release
sudo install -m 755 target/release/session-restore /usr/local/bin/session-restore
```

### Run the full installer from source

```bash
bash scripts/install.sh
```

The installer detects `src/main.rs` and builds from source automatically instead of downloading a prebuilt binary.


## Process Filter Design

Getting the right set of processes to save is the hardest part of this tool. Three layers of filtering are applied:

### Layer 1 — Path allowlist

Only processes whose executable path starts with a known user-app prefix are considered. This immediately eliminates kernel threads, most system daemons, and anything in `/usr/lib/`.

### Layer 2 — Keyword blacklist

Process names and exe paths are checked against a keyword list. This catches background components of GUI apps (Chrome renderers, extension helpers) that live in allowed paths but should not be restored as top-level apps.

Current blacklisted keywords:

```
helper, crashpad, srv, analyzer, extension, worker, handler, renderer,
plugin, daemon, agent, service, dbus, systemd, session-restore,
gnome-shell, gnome-session, gjs, xwayland, pipewire, wireplumber,
pulseaudio, update-notifier, snapd, bash, sh, zsh, fish,
cat, grep, sed, awk, less, more, at-spi, ibus, fcitx,
zeitgeist, tracker, bwrap, mpris-proxy, ubuntu-report, python, perl, ruby, snap
```

### Layer 3 — Run-time threshold

Processes with `run_time() < 30` seconds are skipped. This filters out transient CLI tools that happen to be alive because the user ran `session-restore save` in a terminal.

> **Known limitation:** This approach saves exe paths, not window state. Apps are relaunched from scratch — open files, tabs, and scroll positions are not restored. Per-app state would require D-Bus/AT-SPI integration which is out of scope.


## Adding New App Mappings

If an app fails to restore with a `No such file or directory` error, its internal binary path needs a mapping in `resolve_launch_command()`.

**Step 1** — Find the wrapper script:
```bash
which <appname>
# e.g. which google-chrome  →  /usr/bin/google-chrome
```

**Step 2** — Find the internal binary being reported by sysinfo:
```bash
session-restore save && cat /var/lib/session-restore/$USER/sessions.json
```

**Step 3** — Add an entry to the `known_mappings` array in `resolve_launch_command()`:
```rust
("/opt/myapp/myapp-bin", "/usr/bin/myapp"),
```


## Known Issues & Limitations

| Issue | Notes |
|---|---|
| Save fires after apps are killed | On some GNOME versions, `graphical-session-pre.target` fires too late. Workaround: run `session-restore save` manually before shutdown. |
| Snap apps may not restore | Snap processes run under versioned paths that change between updates. |
| No window geometry restore | Apps launch at their default size/position. Geometry restore would need compositor integration. |
| AnyDesk always skipped | AnyDesk runs as a background service, so it is always detected as already running. |
| Chrome requires `.deb` install | If Chrome is installed as a Snap, `/usr/bin/google-chrome` does not exist and restore fails. |


## Contributing

Contributions are welcome. For bug fixes, open a PR directly. For new features, open an issue first to discuss scope.

### Adding an app to the blacklist

Add the keyword (lowercase) to the `blacklisted_keywords` array in `save_session()`. The keyword is matched as a substring against both the process name and exe path.

### Adding a new subcommand

Add a branch in the `match cmd { ... }` block in `main()` and implement the function. Update the usage string in the `_` arm.

### Adding a new app mapping

See [Adding New App Mappings](#adding-new-app-mappings) above.


*Source: [github.com/imrany/session-restore](https://github.com/imrany/session-restore)*
