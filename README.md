# session-restore 
> Automatically save and restore your desktop session on Linux.


## What is session-restore?

`session-restore` is a lightweight Linux utility that automatically saves your open applications before you shut down, then relaunches them when you log back in — picking up right where you left off.

No configuration needed. Install it once and forget about it.

**Works with:** Chrome, Firefox, VS Code, Zed, Evince, Nautilus, and most GUI apps  
**Supports:** Wayland and X11  
**Safe on shared machines:** sessions are stored per-user


## Requirements

- Ubuntu 20.04 or later (or any systemd-based Linux distro)
- GNOME desktop environment
- Internet connection (for the one-line installer)


## Installation

Run this single command in your terminal:

```bash
curl -fsSL https://raw.githubusercontent.com/imrany/session-restore/main/scripts/install.sh | bash
```

The installer will:

1. Download and install the `session-restore` binary to `/usr/local/bin`
2. Create the sessions directory at `/var/lib/session-restore`
3. Set up two systemd user services — one to save on shutdown, one to restore on login
4. Run a quick smoke test to confirm everything works

Installation takes under 30 seconds. You will be prompted for your `sudo` password once.


## How It Works

### On shutdown

`session-restore` scans your running applications and writes their paths to a session file at:

```
/var/lib/session-restore/<your-username>/sessions.json
```

Only real user-launched GUI apps are saved. System processes, audio servers, display managers, and background daemons are automatically excluded.

### On login

After you log in and your desktop has fully loaded (about 8–10 seconds), `session-restore` reads the session file and relaunches each saved app silently in the background.


## Commands

You can also control `session-restore` manually from the terminal:

| Command                     | What it does                                  |
|-----------------------------|-----------------------------------------------|
| `session-restore save`      | Snapshot your currently running apps right now |
| `session-restore restore`   | Relaunch all saved apps immediately            |
| `session-restore list`      | Show which apps are currently saved            |
| `session-restore info`      | Show version and session file location         |


## What Gets Saved

Apps are captured from these locations:

- `/opt/` — apps like Google Chrome, Slack, Discord
- `/usr/bin/` — system apps like Evince, Nautilus, GIMP
- `/usr/local/bin/` — manually installed apps
- `/snap/` — Snap-packaged apps
- `/var/lib/flatpak/` — Flatpak apps
- `/home/` — apps installed in your home directory (e.g. Zed)

The following are always excluded:

- System processes: `gnome-shell`, `pipewire`, `wireplumber`, `dbus`
- Background daemons and agents
- Command-line tools: `bash`, `python`, `cat`, `grep`
- Processes running for less than 30 seconds at save time


## Troubleshooting

### Apps are not being restored on login

Check if the restore service ran after your last login:

```bash
journalctl --user -u session-restore-restore.service --no-pager
```

Check what is currently saved:

```bash
session-restore list
```

### An app I use is not being saved

Run `session-restore save` while the app is open, then `session-restore list` to see if it appears. If it does not, the app may be installed in a non-standard location or its process name may match an exclusion keyword.

### Chrome is not launching on restore

Make sure Google Chrome is installed via the official `.deb` package (not as a Snap). The restore uses `/usr/bin/google-chrome` as the launch command.

### Checking service status

```bash
# Is the save service enabled?
systemctl --user is-enabled session-restore-save.service

# Is the restore service enabled?
systemctl --user is-enabled session-restore-restore.service

# Full logs for both
journalctl --user -u session-restore-save.service --no-pager
journalctl --user -u session-restore-restore.service --no-pager
```


## Uninstalling

To remove `session-restore` (your saved session file is kept):

```bash
curl -fsSL https://raw.githubusercontent.com/imrany/session-restore/main/scripts/install.sh | bash -s -- --uninstall
```

This removes the binary and both systemd services. Your sessions file at `/var/lib/session-restore` remains untouched.


## Privacy

`session-restore` stores only executable file paths — no window titles, file contents, browser history, or personal data. The session file is stored locally on your machine and is never transmitted anywhere.


*Source: [github.com/imrany/session-restore](https://github.com/imrany/session-restore)*
