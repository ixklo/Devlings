//! Runs the real `perch` binary in its headless modes. Neither mode may start the app or print anything.

use std::{
    io::{Read, Write},
    net::TcpListener,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const BIN: &str = env!("CARGO_BIN_EXE_perch");
const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// Accepts one connection, returns what was posted, and answers `{}`.
fn one_shot_server() -> (u16, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let (mut conn, _) = listener.accept().unwrap();
        conn.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut got = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = conn.read(&mut buf).unwrap();
            got.extend_from_slice(&buf[..n]);
            let text = String::from_utf8_lossy(&got).to_string();
            if let Some((head, body)) = text.split_once("\r\n\r\n") {
                let len = head
                    .lines()
                    .find_map(|l| l.strip_prefix("Content-Length: "))
                    .and_then(|v| v.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if body.len() >= len {
                    break;
                }
            }
            if n == 0 {
                break;
            }
        }
        conn.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}").unwrap();
        String::from_utf8(got).unwrap()
    });
    (port, handle)
}

fn run_relay(args: &[&str], stdin: &[u8]) -> (std::process::Output, Duration) {
    let t0 = Instant::now();
    let mut child = Command::new(BIN)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    let out = child.wait_with_output().unwrap();
    (out, t0.elapsed())
}

#[test]
fn hook_relay_posts_stdin_and_exits_quietly() {
    let (port, server) = one_shot_server();
    let body = r#"{"hook_event_name":"Stop","session_id":"s1","last_assistant_message":"done ✓"}"#;
    let (out, took) = run_relay(&["--hook-relay", &port.to_string(), TOKEN], body.as_bytes());
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty() && out.stderr.is_empty(), "{out:?}");
    assert!(took < Duration::from_secs(5), "{took:?}");
    let request = server.join().unwrap();
    assert!(request.starts_with(&format!("POST /hook/{TOKEN} HTTP/1.1\r\n")), "{request}");
    assert!(request.contains("Content-Type: application/json\r\n"), "{request}");
    assert!(request.ends_with(body), "{request}");
}

#[test]
fn hook_relay_exits_zero_when_perch_is_not_running() {
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let (out, took) = run_relay(&["--hook-relay", &port.to_string(), TOKEN], b"{}");
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty() && out.stderr.is_empty(), "{out:?}");
    assert!(took < Duration::from_secs(4), "{took:?}");
}

#[test]
fn hook_relay_with_bad_arguments_exits_zero_without_starting_the_app() {
    let (out, took) = run_relay(&["--hook-relay", "not-a-port"], b"{}");
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty() && out.stderr.is_empty(), "{out:?}");
    assert!(took < Duration::from_secs(4), "{took:?}");
}

/// Removes Perch's hooks from `$CLAUDE_CONFIG_DIR/settings.json`. It also deletes the start-at-login entry, which on
/// Windows is the real per-user registry value, so there it only runs on GitHub Actions.
#[test]
fn uninstall_hooks_cleans_settings() {
    if cfg!(windows) && std::env::var("GITHUB_ACTIONS").as_deref() != Ok("true") {
        eprintln!("skipped outside GitHub Actions on Windows: it would delete this machine's Perch start-at-login entry");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("claude");
    std::fs::create_dir_all(&config).unwrap();
    let settings = config.join("settings.json");
    let url = format!("http://127.0.0.1:4545/hook/{TOKEN}");
    let relay = format!("\"/opt/perch\" --hook-relay 4545 {TOKEN}");
    let original = serde_json::json!({
        "model": "opus",
        "hooks": {
            "PreToolUse": [
                { "matcher": "Bash", "hooks": [ { "type": "command", "command": "echo hi" } ] },
                { "hooks": [ { "type": "http", "url": url, "timeout": 2 } ] }
            ],
            "Stop": [ { "hooks": [ { "type": "command", "command": relay, "async": true } ] } ]
        }
    });
    std::fs::write(&settings, original.to_string()).unwrap();
    // A fake home keeps macOS and Linux autostart cleanup inside the temp folder.
    let autostart = if cfg!(target_os = "macos") {
        Some(dir.path().join("Library").join("LaunchAgents").join("Perch.plist"))
    } else if cfg!(windows) {
        None
    } else {
        Some(dir.path().join(".config").join("autostart").join("Perch.desktop"))
    };
    if let Some(file) = &autostart {
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, "x").unwrap();
    }
    let out = Command::new(BIN)
        .arg("--uninstall-hooks")
        .env("CLAUDE_CONFIG_DIR", &config)
        .env("HOME", dir.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let after: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
    assert_eq!(
        after,
        serde_json::json!({
            "model": "opus",
            "hooks": { "PreToolUse": [ { "matcher": "Bash", "hooks": [ { "type": "command", "command": "echo hi" } ] } ] }
        })
    );
    if let Some(file) = autostart {
        assert!(!file.exists());
    }
}
