use serde::{Deserialize, Serialize};
use std::process::{self, Command};
use std::{collections::HashSet, fs, path::PathBuf};
use sysinfo::System;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
struct AppConfig {
    exe: String,
    args: Vec<String>,
    cwd: String,
}

fn sessions_file() -> PathBuf {
    if cfg!(debug_assertions) {
        PathBuf::from("./sessions.json")
    } else {
        let user = std::env::var("USER").unwrap_or_else(|_| "default".to_string());
        PathBuf::from(format!("/var/lib/session-restore/{}/sessions.json", user))
    }
}

fn save_session() {
    let mut sys = System::new_all();
    sys.refresh_all();

    let my_pid = sysinfo::get_current_pid().expect("Failed to get self PID");
    let my_uid = sys
        .process(my_pid)
        .and_then(|p| p.user_id())
        .expect("Failed to get UID");

    let mut apps: HashSet<AppConfig> = HashSet::new();
    let mut seen_exes: HashSet<String> = HashSet::new();

    let blacklisted = [
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
        "gnome-",
        "gsd-",
        "gvfs",
        "at-spi",
        "ibus",
        "xdg-",
        "mutter",
        "pipewire",
        "wireplumber",
        "pulseaudio",
        "evolution-",
        "gjs",
        "session-restore",
        "Xwayland",
        "snapd-desktop-integration",
        "update-notifier",
        "cat",
        "bash",
        "sh",
    ];

    let home_dir = std::env::var("HOME").unwrap_or_default();
    let local_dir = format!("{}/.local/", home_dir);

    let allowed_prefixes = [
        "/usr/bin/",
        "/usr/local/bin/",
        "/opt/",
        "/snap/",
        &local_dir,
    ];

    for (pid, process) in sys.processes() {
        if let Some(exe_path) = process.exe() {
            let exe_str = exe_path.to_string_lossy().to_string();
            let name = process.name().to_string_lossy().to_lowercase();

            if seen_exes.contains(&exe_str) {
                continue;
            }

            if process.user_id() != Some(my_uid) || pid == &my_pid || process.run_time() < 30 {
                continue;
            }

            if blacklisted
                .iter()
                .any(|&k| name.contains(k) || exe_str.contains(k))
            {
                continue;
            }

            // The "Root Process" Logic for Chrome/Electron
            // Most browsers use a 'type=' argument for child processes (renderer, gpu-process, etc.)
            // We ONLY want the one that doesn't have a 'type' argument.
            let is_browser_child = process
                .cmd()
                .iter()
                .any(|arg| arg.to_string_lossy().contains("--type="));
            if (name.contains("chrome") || name.contains("brave") || name.contains("edge"))
                && is_browser_child
            {
                continue;
            }

            // Allowed path check
            let in_allowed_path = allowed_prefixes
                .iter()
                .any(|&prefix| exe_str.starts_with(prefix));

            if in_allowed_path && !exe_str.contains("/libexec/") {
                let config = AppConfig {
                    exe: exe_str.clone(),
                    args: process
                        .cmd()
                        .iter()
                        .map(|s| s.to_string_lossy().to_string())
                        .skip(1)
                        .collect(),
                    cwd: process
                        .cwd()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default(),
                };
                apps.insert(config);
                seen_exes.insert(exe_str);
            }
        }
    }

    let path = sessions_file();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(&path, serde_json::to_string_pretty(&apps).unwrap()).expect("Write failed");
    eprintln!(
        "[session-restore] Saved {} apps to {}",
        apps.len(),
        path.display()
    );
}

fn restore_session() {
    let path = sessions_file();
    let data = fs::read_to_string(&path).unwrap_or_else(|_| "[]".to_string());
    let apps: Vec<AppConfig> = serde_json::from_str(&data).unwrap_or_default();

    let mut sys = System::new_all();
    sys.refresh_all();

    // Get currently running exes to prevent duplicates
    let active_exes: HashSet<String> = sys
        .processes()
        .values()
        .filter_map(|p| p.exe().map(|e| e.to_string_lossy().to_string()))
        .collect();

    let service_flags = ["--gapplication-service", "--tray", "--background"];
    for mut app in apps {
        if active_exes.contains(&app.exe) {
            eprintln!("[session-restore] Skipping (already running): {}", app.exe);
            continue;
        }

        // remove the 'service' or 'tray' flag so a window actually opens
        app.args
            .retain(|arg| !service_flags.contains(&arg.as_str()));

        let mut cmd = Command::new(&app.exe);
        cmd.args(&app.args);
        if !app.cwd.is_empty() {
            cmd.current_dir(&app.cwd);
        }

        // Pass essential GUI env vars
        for var in [
            "DISPLAY",
            "WAYLAND_DISPLAY",
            "XDG_RUNTIME_DIR",
            "DBUS_SESSION_BUS_ADDRESS",
        ] {
            if let Ok(val) = std::env::var(var) {
                cmd.env(var, val);
            }
        }

        // Detach the process so it doesn't print logs to your terminal or die when you close it
        cmd.stdout(process::Stdio::null());
        cmd.stderr(process::Stdio::null());

        #[cfg(unix)]
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid(); // Create a new session so the child is a true orphan
                Ok(())
            });
        }

        match cmd.spawn() {
            Ok(_) => eprintln!("[session-restore] Relaunched: {}", app.exe),
            Err(e) => eprintln!("[session-restore] Error: {} -> {}", app.exe, e),
        }
    }
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
    let apps: Vec<AppConfig> = serde_json::from_str(&data).unwrap_or_default();
    if apps.is_empty() {
        println!("No saved apps.");
    } else {
        println!("Saved apps ({}):", apps.len());
        for app in apps {
            // Print the exe and the directory it will open in
            println!("  {} (in {})", app.exe, app.cwd);
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
