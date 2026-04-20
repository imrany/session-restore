use std::process;
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use sysinfo::System;

fn sessions_file() -> PathBuf {
    if cfg!(debug_assertions) {
        PathBuf::from("./sessions.json")
    } else {
        let user = std::env::var("USER")
            .or_else(|_| std::env::var("LOGNAME"))
            .unwrap_or_else(|_| "default".to_string());
        PathBuf::from(format!(
            "/var/lib/session-restore/{}/sessions.json",
            user
        ))
    }
}

fn save_session() {
    let mut sys = System::new_all();
    sys.refresh_all();

    let current_pid = sysinfo::get_current_pid().expect("Failed to get self PID");
    let my_uid = sys
        .process(current_pid)
        .and_then(|p| p.user_id())
        .expect("Failed to get current user UID");

    let mut apps: HashSet<String> = HashSet::new();
    let self_pid = process::id();

    let blacklisted_keywords = [
        "helper",
        "crashpad",
        "srv",
        "analyzer",
        "extension",
        "worker",
        "handler",
        "renderer",
        "plugin",
        "node",
        "daemon",
        "agent",
        "service",
        "dbus",
        "systemd",
        "session-restore",
        "gnome-shell",
        "gnome-session",
        "gjs",
        "xwayland",
        "pipewire",
        "wireplumber",
        "pulseaudio",
        "update-notifier",
        "snapd",
        "bash",
        "sh",
        "zsh",
        "fish",
        "cat",
        "grep",
        "sed",
        "awk",
        "less",
        "more",
        "at-spi",
        "ibus",
        "fcitx",
        "zeitgeist",
        "tracker",
    ];

    let allowed_prefixes = [
        "/opt/",
        "/usr/bin/",
        "/usr/local/bin/",
        "/snap/",
        "/var/lib/flatpak/",
        "/home/",
    ];

    for (pid, process) in sys.processes() {
        if let Some(exe_path) = process.exe() {
            let exe_str = exe_path.to_string_lossy();
            let name = process.name().to_string_lossy().to_lowercase();
            let exe_lower = exe_str.to_lowercase();

            let is_helper = blacklisted_keywords
                .iter()
                .any(|&k| exe_lower.contains(k) || name.contains(k));

            let in_allowed_path = allowed_prefixes
                .iter()
                .any(|&prefix| exe_str.starts_with(prefix));

            // Skip processes that haven't been running for at least 30 seconds
            // This filters out transient CLI tools (cat, bash, grep etc.)
            // that were only alive because the user ran session-restore save
            if process.run_time() < 30 {
                continue;
            }

            if process.user_id() == Some(my_uid)
                && pid.as_u32() != self_pid
                && in_allowed_path
                && !is_helper
            {
                apps.insert(exe_str.into_owned());
            }
        }
    }

    let apps_vec: Vec<String> = apps.into_iter().collect();
    let path = sessions_file();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("Failed to create sessions directory");
    }

    fs::write(&path, serde_json::to_string_pretty(&apps_vec).unwrap())
        .expect("Failed to write session file");

    eprintln!(
        "[session-restore] Saved {} apps to {}",
        apps_vec.len(),
        path.display()
    );
}

fn restore_session() {
    let path = sessions_file();

    let data = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(_) => {
            eprintln!(
                "[session-restore] No sessions file found at {}. Nothing to restore.",
                path.display()
            );
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).ok();
            }
            fs::write(&path, "[]").unwrap_or_else(|e| eprintln!("{}", e));
            return;
        }
    };

    let apps: Vec<String> = match serde_json::from_str(&data) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[session-restore] Failed to parse sessions file: {}", e);
            return;
        }
    };

    if apps.is_empty() {
        eprintln!("[session-restore] No apps to restore.");
        return;
    }

    eprintln!("[session-restore] Restoring {} apps...", apps.len());

    // Wait for desktop to fully settle before launching apps
    std::thread::sleep(std::time::Duration::from_secs(8));

    let display = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string());
    let xdg_runtime = std::env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| format!("/run/user/{}", get_uid()));

    for app in &apps {
        let mut cmd = Command::new(app);

        cmd.env("DISPLAY", &display)
           .env("XDG_RUNTIME_DIR", &xdg_runtime);

        if let Ok(w) = std::env::var("WAYLAND_DISPLAY") {
            cmd.env("WAYLAND_DISPLAY", w);
        }
        if let Ok(d) = std::env::var("DBUS_SESSION_BUS_ADDRESS") {
            cmd.env("DBUS_SESSION_BUS_ADDRESS", d);
        }
        if let Ok(t) = std::env::var("XDG_SESSION_TYPE") {
            cmd.env("XDG_SESSION_TYPE", t);
        }
        if let Ok(d) = std::env::var("XDG_CURRENT_DESKTOP") {
            cmd.env("XDG_CURRENT_DESKTOP", d);
        }

        match cmd.spawn() {
            Ok(_) => eprintln!("[session-restore] Launched: {}", app),
            Err(e) => eprintln!("[session-restore] Failed to restore '{}': {}", app, e),
        }
    }
}

fn get_uid() -> u32 {
    // Safe way to get UID without libc dependency
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Uid:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|uid| uid.parse().ok())
        })
        .unwrap_or(1000)
}

fn list_session() {
    let path = sessions_file();
    let data = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => {
            println!("No sessions file found at {}", path.display());
            return;
        }
    };
    let apps: Vec<String> = serde_json::from_str(&data).unwrap_or_default();
    if apps.is_empty() {
        println!("No saved apps.");
    } else {
        println!("Saved apps ({}):", apps.len());
        for app in apps {
            println!("  {}", app);
        }
    }
}

fn get_app_info() {
    let name = env!("CARGO_PKG_NAME");
    let version = env!("CARGO_PKG_VERSION");
    let authors = env!("CARGO_PKG_AUTHORS");
    println!("{} v{} by {}", name, version, authors);
    println!("Sessions file: {}", sessions_file().display());
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("_");

    match cmd {
        "save" => save_session(),
        "restore" => restore_session(),
        "list" => list_session(),
        "info" => get_app_info(),
        _ => {
            eprintln!("Usage: session-restore [save|restore|list|info]");
            eprintln!("  save    — snapshot currently running user apps");
            eprintln!("  restore — relaunch saved apps");
            eprintln!("  list    — show what's currently saved");
            eprintln!("  info    — show version and config info");
            process::exit(1);
        }
    }
}
