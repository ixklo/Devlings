//! Logging, the panic hook, and the redacted diagnostics report behind "Copy diagnostics".

use std::{
    any::Any,
    fs,
    path::{Path, PathBuf},
};

use log::LevelFilter;
use tauri::{plugin::TauriPlugin, AppHandle, Manager, Runtime};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

use crate::{
    hooks_installer::{self, HookStatus, RelayExe},
    locator,
    locks::lock,
    money_guard::AuthVerdict,
    state::{self, AppState},
};

/// The log file is `<app log dir>/devlings.log`; rotated files are `devlings_<date>.log`. Logs written before the
/// rename (`perch.log`, `perch_<date>.log`) stay in the same folder and are left alone.
pub const LOG_FILE_STEM: &str = "devlings";
pub const MAX_LOG_BYTES: u128 = 1_000_000;
/// At most this many log files exist: the active one plus rotated ones.
pub const MAX_LOG_FILES: usize = 5;
pub const TAIL_LINES: usize = 200;
pub const TOKEN_MARK: &str = "<token>";

/// Rotating file log in the app log dir, plus stdout in debug builds.
pub fn log_plugin<R: Runtime>(level: LevelFilter) -> TauriPlugin<R> {
    let mut targets = vec![Target::new(TargetKind::LogDir { file_name: Some(LOG_FILE_STEM.into()) })];
    if cfg!(debug_assertions) {
        targets.push(Target::new(TargetKind::Stdout));
    }
    tauri_plugin_log::Builder::new()
        .clear_targets()
        .targets(targets)
        .level(level)
        .max_file_size(MAX_LOG_BYTES)
        .rotation_strategy(RotationStrategy::KeepSome(MAX_LOG_FILES - 1))
        .build()
}

/// "message at file:line:col" for a panic payload.
pub fn describe_panic(payload: &(dyn Any + Send), location: Option<(&str, u32, u32)>) -> String {
    let message = payload
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "(non-string panic payload)".to_string());
    match location {
        Some((file, line, col)) => format!("{message} at {file}:{line}:{col}"),
        None => message,
    }
}

/// Logs every panic (message, location, backtrace), then runs the default hook.
pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info.location().map(|l| (l.file(), l.line(), l.column()));
        let backtrace = std::backtrace::Backtrace::force_capture();
        log::error!("Panic: {}\n{backtrace}", describe_panic(info.payload(), location));
        log::logger().flush();
        default_hook(info);
    }));
}

/// Hides the hook token (and anything shaped like one) and the user's home folder.
pub fn redact(text: &str, token: Option<&str>, home: Option<&Path>) -> String {
    let mut out = match token {
        Some(t) if t.len() >= 16 => text.replace(t, TOKEN_MARK),
        _ => text.to_string(),
    };
    out = redact_token_shapes(&out);
    if let Some(home) = home.map(|h| h.display().to_string()).filter(|h| h.len() > 1) {
        let home = home.trim_end_matches(['/', '\\']);
        let encoded: String = home.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
        // JSON-escaped first, so the plain spelling can't match half of it.
        for spelling in [home.replace('\\', "\\\\"), home.to_string(), home.replace('\\', "/"), encoded] {
            out = replace_path_prefix(&out, &spelling);
        }
    }
    out
}

/// Replaces every run of exactly 64 hex digits, the shape of Devlings' hook token.
fn redact_token_shapes(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_hexdigit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_hexdigit() {
                i += 1;
            }
            out.push_str(if i - start == 64 { TOKEN_MARK } else { &text[start..i] });
        } else {
            let start = i;
            while i < bytes.len() && !bytes[i].is_ascii_hexdigit() {
                i += 1;
            }
            out.push_str(&text[start..i]);
        }
    }
    out
}

/// Replaces `prefix` with `~` wherever it isn't just the start of a longer name (`/home/jane` but not `/home/janet`).
/// Case-insensitive on Windows, where paths are.
fn replace_path_prefix(text: &str, prefix: &str) -> String {
    if prefix.is_empty() {
        return text.to_string();
    }
    let fold = |s: &str| if cfg!(windows) { s.to_ascii_lowercase() } else { s.to_string() };
    let (hay, needle) = (fold(text), fold(prefix));
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    let mut from = 0;
    while let Some(pos) = hay[from..].find(&needle).map(|p| p + from) {
        let end = pos + needle.len();
        let continues_name = text[end..].chars().next().is_some_and(|c| c.is_alphanumeric());
        if continues_name {
            from = end;
            continue;
        }
        out.push_str(&text[last..pos]);
        out.push('~');
        last = end;
        from = end;
    }
    out.push_str(&text[last..]);
    out
}

/// Log files oldest first; the active file is last.
pub fn log_files(dir: &Path) -> Vec<PathBuf> {
    let active = format!("{LOG_FILE_STEM}.log");
    let rotated_prefix = format!("{LOG_FILE_STEM}_");
    let mut rotated: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .is_some_and(|n| n.starts_with(&rotated_prefix) && n.ends_with(".log"))
        })
        .collect();
    // Rotated names carry a sortable timestamp.
    rotated.sort();
    let current = dir.join(active);
    if current.is_file() {
        rotated.push(current);
    }
    rotated
}

/// The last `n` lines across the log files.
pub fn tail(dir: &Path, n: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for file in log_files(dir).iter().rev() {
        if lines.len() >= n {
            break;
        }
        let Ok(bytes) = fs::read(file) else { continue };
        let text = String::from_utf8_lossy(&bytes);
        let want = n - lines.len();
        let mut newest: Vec<String> = text.lines().rev().take(want).map(str::to_string).collect();
        newest.reverse();
        newest.append(&mut lines);
        lines = newest;
    }
    lines
}

/// Everything "Copy diagnostics" reports, before redaction.
pub struct Report {
    pub app_version: String,
    pub os: String,
    pub arch: String,
    /// (version, path, how it was found), or why it wasn't.
    pub claude: Result<(String, String, String), String>,
    pub login: String,
    pub hooks: String,
    pub hook_server: String,
    pub settings_path: String,
    pub log_dir: String,
    pub log_tail: Vec<String>,
}

pub fn render(r: &Report) -> String {
    let claude = match &r.claude {
        Ok((version, path, via)) => format!("{version} at {path} (found via {via})"),
        Err(e) => format!("not found ({e})"),
    };
    let tail = if r.log_tail.is_empty() { "(empty)".to_string() } else { r.log_tail.join("\n") };
    format!(
        "Devlings diagnostics\n\
         App: Devlings {}\n\
         OS: {} {}\n\
         Claude Code: {claude}\n\
         Login: {}\n\
         Hooks: {}\n\
         Hook server: {}\n\
         Settings file: {}\n\
         Log folder: {}\n\
         \n\
         Last {} log lines:\n\
         {tail}\n",
        r.app_version, r.os, r.arch, r.login, r.hooks, r.hook_server, r.settings_path, r.log_dir, TAIL_LINES
    )
}

pub fn log_dir(app: &AppHandle) -> PathBuf {
    app.path().app_log_dir().unwrap_or_else(|_| app.state::<AppState>().data_dir.join("logs"))
}

fn hooks_text(app: &AppHandle, declined: bool) -> String {
    let status = match state::hook_status(app) {
        None => "not set up".to_string(),
        Some(Ok(HookStatus::Current)) => "installed".to_string(),
        Some(Ok(HookStatus::Outdated)) => "installed, but different from this version's entries".to_string(),
        Some(Ok(HookStatus::NotInstalled)) => "not installed".to_string(),
        Some(Err(e)) => format!("unknown ({e})"),
    };
    let relay = match hooks_installer::relay_exe() {
        Ok(RelayExe::Usable(_)) => "relay command",
        Ok(RelayExe::Ephemeral(_)) => "HTTP for Stop/StopFailure/SessionEnd (Devlings runs from a temporary location)",
        Ok(RelayExe::Unsafe(_)) => "HTTP for Stop/StopFailure/SessionEnd (the Devlings program path has shell characters)",
        Err(_) => "unknown",
    };
    let status = format!("{status}; async events via {relay}");
    if declined { format!("{status}; declined in Devlings") } else { status }
}

/// The redacted diagnostics text.
pub fn collect(app: &AppHandle) -> String {
    let s = app.state::<AppState>();
    let home = dirs::home_dir();
    let (token, declined, override_path) = {
        let c = lock(&s.config);
        (c.hook_token.clone(), c.hooks_declined, c.claude_path.clone())
    };
    let path_env = std::env::var_os("PATH");
    let claude = lock(&s.claude).clone().map(|l| {
        let via = locator::describe_source(
            &l.path,
            override_path.as_deref().map(Path::new),
            path_env.as_deref(),
            home.as_deref().unwrap_or(Path::new("")),
        );
        (l.version, l.path.display().to_string(), via)
    });
    let login = match lock(&s.auth).clone() {
        None => "not checked".to_string(),
        Some(AuthVerdict::Allowed { subscription }) => format!("subscription ({subscription})"),
        Some(AuthVerdict::Refused { reason }) => format!("refused: {reason}"),
    };
    let server_error = lock(&s.hook_server_error).clone();
    let hook_server = match (server_error, lock(&s.hook_server).as_ref().map(|srv| srv.port)) {
        (Some(e), _) => format!("error: {e}"),
        (None, Some(port)) => format!("listening on 127.0.0.1:{port}"),
        (None, None) => "not running".to_string(),
    };
    let dir = log_dir(app);
    let report = Report {
        app_version: app.package_info().version.to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        claude,
        login,
        hooks: hooks_text(app, declined),
        hook_server,
        settings_path: hooks_installer::settings_path().display().to_string(),
        log_dir: dir.display().to_string(),
        log_tail: tail(&dir, TAIL_LINES),
    };
    redact(&render(&report), token.as_deref(), home.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn panic_descriptions() {
        assert_eq!(describe_panic(&"boom", Some(("src/state.rs", 3, 9))), "boom at src/state.rs:3:9");
        assert_eq!(describe_panic(&String::from("bad"), None), "bad");
        assert_eq!(describe_panic(&42u8, None), "(non-string panic payload)");
    }

    #[test]
    fn redacts_the_token_and_token_shapes() {
        let other = "f".repeat(64);
        let text = format!("url http://127.0.0.1:1/hook/{TOKEN} relay --hook-relay 1 {other} sha {} long {}", "a".repeat(40), "b".repeat(65));
        let out = redact(&text, Some(TOKEN), None);
        assert!(!out.contains(TOKEN) && !out.contains(&other), "{out}");
        assert_eq!(out.matches(TOKEN_MARK).count(), 2, "{out}");
        assert!(out.contains(&"a".repeat(40)) && out.contains(&"b".repeat(65)), "{out}");
        // A short or missing token never blanks the text.
        assert_eq!(redact("abc", Some(""), None), "abc");
    }

    #[test]
    fn redacts_the_home_folder_in_every_spelling() {
        if cfg!(windows) {
            let home = Path::new(r"C:\Users\Jane");
            let text = r#"C:\Users\Jane\proj and c:/users/jane/x and "C:\\Users\\Jane\\y" and C--Users-Jane-proj and C:\Users\Janet\z"#;
            assert_eq!(
                redact(text, None, Some(home)),
                r#"~\proj and ~/x and "~\\y" and ~-proj and C:\Users\Janet\z"#
            );
        } else {
            let home = Path::new("/home/jane");
            let text = "/home/jane/proj and -home-jane-proj and /home/janet/x and /home/jane";
            assert_eq!(redact(text, None, Some(home)), "~/proj and ~-proj and /home/janet/x and ~");
        }
        assert_eq!(redact("x", None, Some(Path::new(""))), "x");
    }

    #[test]
    fn tails_across_rotated_files() {
        let dir = tempfile::tempdir().unwrap();
        assert!(tail(dir.path(), 5).is_empty());
        fs::write(dir.path().join("devlings_2026-09-01_10-00-00.log"), "a1\na2\n").unwrap();
        fs::write(dir.path().join("devlings_2026-09-02_10-00-00.log"), "b1\nb2\nb3\n").unwrap();
        fs::write(dir.path().join("devlings.log"), "c1\nc2\n").unwrap();
        fs::write(dir.path().join("other.log"), "zz\n").unwrap();
        // Logs from before the rename share the folder (the app identifier didn't change) but aren't this app's.
        fs::write(dir.path().join("perch.log"), "old\n").unwrap();
        fs::write(dir.path().join("perch_2026-08-01_10-00-00.log"), "older\n").unwrap();
        let names: Vec<String> = log_files(dir.path()).iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert_eq!(names, vec!["devlings_2026-09-01_10-00-00.log", "devlings_2026-09-02_10-00-00.log", "devlings.log"]);
        assert_eq!(tail(dir.path(), 3), vec!["b3", "c1", "c2"]);
        assert_eq!(tail(dir.path(), 100), vec!["a1", "a2", "b1", "b2", "b3", "c1", "c2"]);
        assert_eq!(tail(dir.path(), 0), Vec::<String>::new());
    }

    #[test]
    fn renders_a_report() {
        let r = Report {
            app_version: "1.0.0".into(),
            os: "windows".into(),
            arch: "x86_64".into(),
            claude: Ok(("2.1.282".into(), "C:\\cc\\claude.exe".into(), "PATH".into())),
            login: "subscription (pro)".into(),
            hooks: "current".into(),
            hook_server: "listening on 127.0.0.1:4545".into(),
            settings_path: "C:\\h\\.claude\\settings.json".into(),
            log_dir: "C:\\logs".into(),
            log_tail: vec!["line one".into(), "line two".into()],
        };
        let text = render(&r);
        for want in [
            "Devlings 1.0.0",
            "windows x86_64",
            "Claude Code: 2.1.282 at C:\\cc\\claude.exe (found via PATH)",
            "Login: subscription (pro)",
            "Hooks: current",
            "Hook server: listening on 127.0.0.1:4545",
            "C:\\h\\.claude\\settings.json",
            "C:\\logs",
            "line one\nline two",
        ] {
            assert!(text.contains(want), "missing {want:?} in:\n{text}");
        }
        let missing = Report { claude: Err("Claude Code wasn't found.".into()), log_tail: vec![], ..r };
        let text = render(&missing);
        assert!(text.contains("Claude Code: not found (Claude Code wasn't found.)"), "{text}");
        assert!(text.contains("(empty)"), "{text}");
    }
}
