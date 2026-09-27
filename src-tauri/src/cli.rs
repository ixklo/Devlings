//! Headless command-line modes, handled before any Tauri or single-instance setup so a running Perch never sees them.
//!
//! - `perch --uninstall-hooks`: removes Perch's hooks from Claude Code's settings.json and the start-at-login entry.
//!   Run by the Windows uninstaller. Exits 0 on success, 1 if settings.json couldn't be updated.
//! - `perch --hook-relay <port> <token>`: posts the hook body on stdin to the running Perch and always exits 0,
//!   silently and within about a second. Claude Code runs it as an async command hook for Stop, StopFailure and
//!   SessionEnd, whose HTTP versions show errors while Perch is quit.

use std::{
    ffi::OsString,
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpStream},
    path::{Path, PathBuf},
    time::Duration,
};

use crate::hooks_installer::{self, RELAY_FLAG};

pub const UNINSTALL_FLAG: &str = "--uninstall-hooks";
/// Same cap as the hook server: a bigger body would be ignored there anyway.
pub const MAX_RELAY_BODY: u64 = crate::hook_server::MAX_BODY_BYTES as u64;
const RELAY_STEP_TIMEOUT: Duration = Duration::from_secs(1);
/// The relay exits by this deadline even if stdin never closes.
const RELAY_DEADLINE: Duration = Duration::from_secs(5);
/// The app name the autostart plugin registers under (the product name).
const AUTOSTART_NAME: &str = "Perch";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    App,
    UninstallHooks,
    /// `None` when the arguments are malformed; the relay then exits quietly without sending anything.
    HookRelay(Option<(u16, String)>),
}

/// Reads the mode from the full argument list (program name first).
pub fn parse(args: &[OsString]) -> Mode {
    let args: Vec<String> = args.iter().skip(1).map(|a| a.to_string_lossy().to_string()).collect();
    if let Some(i) = args.iter().position(|a| a == RELAY_FLAG) {
        let port = args.get(i + 1).and_then(|p| p.parse::<u16>().ok());
        // The token goes into a request line, so only plain characters pass.
        let token = args.get(i + 2).filter(|t| !t.is_empty() && t.chars().all(|c| c.is_ascii_alphanumeric()));
        return Mode::HookRelay(port.zip(token.cloned()));
    }
    if args.iter().any(|a| a == UNINSTALL_FLAG) {
        Mode::UninstallHooks
    } else {
        Mode::App
    }
}

/// The raw HTTP request the relay sends.
pub fn relay_request(port: u16, token: &str, body: &[u8]) -> Vec<u8> {
    let mut req = format!(
        "POST /hook/{token} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    req.extend_from_slice(body);
    req
}

/// Posts `body` to Perch's hook server with short timeouts and waits briefly for the answer.
pub fn post_hook(port: u16, token: &str, body: &[u8]) -> std::io::Result<()> {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut stream = TcpStream::connect_timeout(&addr, RELAY_STEP_TIMEOUT)?;
    stream.set_write_timeout(Some(RELAY_STEP_TIMEOUT))?;
    stream.set_read_timeout(Some(RELAY_STEP_TIMEOUT))?;
    stream.write_all(&relay_request(port, token, body))?;
    stream.flush()?;
    // The server answers after handling the body, so the first bytes of the answer mean it arrived.
    let mut answer = [0u8; 256];
    stream.read(&mut answer).map(|_| ())
}

fn relay(target: Option<(u16, String)>) {
    // Whatever happens (stdin never closing, a stuck server), the relay is gone by the deadline.
    std::thread::spawn(|| {
        std::thread::sleep(RELAY_DEADLINE);
        std::process::exit(0);
    });
    let mut body = Vec::new();
    let read = std::io::stdin().lock().take(MAX_RELAY_BODY + 1).read_to_end(&mut body);
    // A body over the cap would arrive cut off and be ignored anyway.
    if read.is_err() || body.is_empty() || body.len() as u64 > MAX_RELAY_BODY {
        return;
    }
    if let Some((port, token)) = target {
        let _ = post_hook(port, &token, &body);
    }
}

/// Start-at-login files the autostart plugin creates on macOS and Linux.
pub fn autostart_files(home: &Path) -> Vec<PathBuf> {
    if cfg!(target_os = "macos") {
        vec![home.join("Library").join("LaunchAgents").join(format!("{AUTOSTART_NAME}.plist"))]
    } else if cfg!(windows) {
        Vec::new()
    } else {
        vec![home.join(".config").join("autostart").join(format!("{AUTOSTART_NAME}.desktop"))]
    }
}

/// Removes the start-at-login entry the autostart plugin made, ignoring every error (it may not exist).
fn remove_autostart() {
    #[cfg(windows)]
    {
        use std::process::Stdio;
        let reg = std::env::var_os("SystemRoot")
            .map(|root| PathBuf::from(root).join("System32").join("reg.exe"))
            .unwrap_or_else(|| PathBuf::from("reg.exe"));
        for key in [
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run",
        ] {
            let _ = crate::runner::background_command(&reg)
                .args(["delete", key, "/v", AUTOSTART_NAME, "/f"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
    if let Some(home) = dirs::home_dir() {
        for file in autostart_files(&home) {
            let _ = std::fs::remove_file(file);
        }
    }
}

/// Removes Perch's hooks from `settings`, then the start-at-login entry. The entry goes even if the hooks couldn't.
pub fn uninstall(settings: &Path, stamp: i64, remove_autostart: impl FnOnce()) -> Result<bool, String> {
    let result = hooks_installer::uninstall_file(settings, stamp);
    remove_autostart();
    result
}

/// Runs a headless mode if the arguments ask for one and returns its exit code; None means start the app.
pub fn run_headless() -> Option<i32> {
    let args: Vec<OsString> = std::env::args_os().collect();
    match parse(&args) {
        Mode::App => None,
        Mode::HookRelay(target) => {
            relay(target);
            Some(0)
        }
        Mode::UninstallHooks => {
            let result = uninstall(&hooks_installer::settings_path(), crate::state::now_ms() / 1000, remove_autostart);
            Some(match result {
                Ok(_) => 0,
                Err(e) => {
                    let _ = writeln!(std::io::stderr(), "Perch: {e}");
                    1
                }
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hook_server::{HookServer, MAX_BODY_BYTES};
    use serde_json::{json, Value};
    use std::{sync::mpsc, time::Instant};

    const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn args(list: &[&str]) -> Vec<OsString> {
        std::iter::once("perch").chain(list.iter().copied()).map(OsString::from).collect()
    }

    #[test]
    fn parses_modes() {
        assert_eq!(parse(&args(&[])), Mode::App);
        assert_eq!(parse(&args(&["--something-else"])), Mode::App);
        assert_eq!(parse(&args(&["--uninstall-hooks"])), Mode::UninstallHooks);
        assert_eq!(parse(&args(&["--hook-relay", "4545", TOKEN])), Mode::HookRelay(Some((4545, TOKEN.into()))));
        assert_eq!(parse(&args(&["--hook-relay", "4545"])), Mode::HookRelay(None));
        assert_eq!(parse(&args(&["--hook-relay", "port", TOKEN])), Mode::HookRelay(None));
        assert_eq!(parse(&args(&["--hook-relay", "4545", "bad token"])), Mode::HookRelay(None));
        assert_eq!(parse(&args(&["--hook-relay", "4545", "a/b"])), Mode::HookRelay(None));
        assert_eq!(parse(&args(&["--hook-relay"])), Mode::HookRelay(None));
        // The relay wins over everything else: it must never start a second app.
        assert_eq!(parse(&args(&["--uninstall-hooks", "--hook-relay", "1", TOKEN])), Mode::HookRelay(Some((1, TOKEN.into()))));
    }

    #[test]
    fn relay_and_server_share_the_body_cap() {
        assert_eq!(MAX_RELAY_BODY, MAX_BODY_BYTES as u64);
    }

    #[test]
    fn builds_a_plain_http_post() {
        let req = String::from_utf8(relay_request(4545, "tok", br#"{"a":1}"#)).unwrap();
        assert_eq!(
            req,
            "POST /hook/tok HTTP/1.1\r\nHost: 127.0.0.1:4545\r\nContent-Type: application/json\r\nContent-Length: 7\r\nConnection: close\r\n\r\n{\"a\":1}"
        );
    }

    #[test]
    fn relays_a_body_to_the_hook_server() {
        let (tx, rx) = mpsc::channel();
        let server = HookServer::start(0, TOKEN.into(), move |v| tx.send(v).unwrap(), |_| String::new()).unwrap();
        let body = json!({"hook_event_name": "Stop", "session_id": "s1", "last_assistant_message": "héllo"}).to_string();
        post_hook(server.port, TOKEN, body.as_bytes()).unwrap();
        let got: Value = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(got["last_assistant_message"], "héllo");
        server.stop();
    }

    #[test]
    fn relay_fails_fast_when_perch_is_not_running() {
        let port = hooks_installer::free_port().unwrap();
        let t0 = Instant::now();
        assert!(post_hook(port, TOKEN, b"{}").is_err());
        assert!(t0.elapsed() < Duration::from_secs(3), "{:?}", t0.elapsed());
    }

    #[test]
    fn autostart_locations() {
        let home = Path::new("/home/u");
        let files = autostart_files(home);
        if cfg!(target_os = "macos") {
            assert_eq!(files, vec![home.join("Library").join("LaunchAgents").join("Perch.plist")]);
        } else if cfg!(windows) {
            assert!(files.is_empty());
        } else {
            assert_eq!(files, vec![home.join(".config").join("autostart").join("Perch.desktop")]);
        }
    }

    #[test]
    fn uninstall_removes_hooks_then_autostart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let exe = Path::new("/opt/perch");
        let t = hooks_installer::HookTarget { port: 4545, token: TOKEN, exe };
        std::fs::write(&path, r#"{"model":"opus"}"#).unwrap();
        hooks_installer::install_file(&path, t, 1).unwrap();
        let mut removed = false;
        assert!(uninstall(&path, 2, || removed = true).unwrap());
        assert!(removed);
        assert_eq!(hooks_installer::read_settings(&path).unwrap(), json!({"model": "opus"}));

        let mut removed_again = false;
        assert!(!uninstall(&path, 3, || removed_again = true).unwrap());
        assert!(removed_again);

        std::fs::write(&path, "{ broken").unwrap();
        let mut removed_anyway = false;
        assert!(uninstall(&path, 4, || removed_anyway = true).is_err());
        assert!(removed_anyway);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ broken");
    }
}
