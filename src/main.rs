use std::process;
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use sysinfo::System;

// Automatically switches based on build profile
const SESSIONS_FILE_PATH: &str = if cfg!(debug_assertions) {
    "./sessions.json"
} else {
    "/var/lib/session-restore/sessions.json"
};

fn sessions_file() -> PathBuf {
    PathBuf::from(SESSIONS_FILE_PATH)
}

fn save_session() {
    let mut sys = System::new_all();
    sys.refresh_all();

    // Get the UID of the current user running this script
    let current_pid = sysinfo::get_current_pid().expect("Failed to get self PID");
    let my_uid = sys
        .process(current_pid)
        .and_then(|p| p.user_id())
        .expect("Failed to get current user UID");

    let mut apps: HashSet<String> = HashSet::new();
    let self_pid = process::id();

    // Keywords that identify background helpers or sub-processes we don't want to save
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
        "session-restore", // don't save ourselves
    ];

    // Paths where user-facing GUI apps typically live
    let allowed_prefixes = [
        "/home/",
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

            if process.user_id() == Some(my_uid)
                && pid.as_u32() != self_pid
                && in_allowed_path
                && !is_helper
            {
                // Deduplicate: only store the base executable path (not per-window instances)
                apps.insert(exe_str.into_owned());
            }
        }
    }

    let apps_vec: Vec<String> = apps.into_iter().collect();
    let path = sessions_file();

    // Ensure parent directory exists
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
            // Create an empty sessions file so save works next time
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

    // Small delay to let the desktop environment finish loading
    std::thread::sleep(std::time::Duration::from_secs(3));

    for app in &apps {
        match Command::new(app).spawn() {
            Ok(_) => eprintln!("[session-restore] Launched: {}", app),
            Err(e) => eprintln!("[session-restore] Failed to restore '{}': {}", app, e),
        }
    }
}

fn list_session() {
    let path = sessions_file();
    let data = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => {
            println!("No sessions file found.");
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
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("restore");

    match cmd {
        "save" => save_session(),
        "restore" => restore_session(),
        "list" => list_session(),
        "info" => get_app_info(),
        _ => {
            eprintln!("Usage: session-restore [save|restore|list|info]");
            eprintln!("  save    — snapshot currently running user apps");
            eprintln!("  restore — relaunch saved apps (default)");
            eprintln!("  list    — show what's currently saved");
            process::exit(1);
        }
    }
}
