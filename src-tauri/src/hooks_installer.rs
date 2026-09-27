//! Devlings' entries in Claude Code's user `settings.json` (design v1.0 §3).

use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use serde::Serialize;
use serde_json::{json, Value};

/// Frequent events: HTTP, silent and cheap when Devlings isn't running.
pub const HTTP_EVENTS: [&str; 7] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "Notification",
    "PermissionDenied",
];
/// Answered synchronously; Devlings always answers before this long hold ends (its own holds are at most 240 s).
pub const PERMISSION_EVENT: &str = "PermissionRequest";
/// HTTP versions of these show error noise when Devlings is quit, so they go through the async relay.
pub const RELAY_EVENTS: [&str; 3] = ["Stop", "StopFailure", "SessionEnd"];
pub const HTTP_TIMEOUT_SECS: u64 = 2;
pub const PERMISSION_TIMEOUT_SECS: u64 = 300;
pub const RELAY_FLAG: &str = "--hook-relay";
pub const MAX_BACKUPS: usize = 5;
const BACKUP_PREFIX: &str = "settings.json.devlings-backup-";
/// Backups made before the app was renamed (v1.1). Never written; they still count, so they're pruned in turn.
const LEGACY_BACKUP_PREFIX: &str = "settings.json.perch-backup-";
const WRITE_ATTEMPTS: usize = 3;

/// Where Devlings' hooks point: its server's port and token, and the executable that relays async events.
#[derive(Debug, Clone, Copy)]
pub struct HookTarget<'a> {
    pub port: u16,
    pub token: &'a str,
    /// None when this executable can't safely be written into a hook command; the relay events then use HTTP.
    pub exe: Option<&'a Path>,
}

/// This executable, judged as a relay command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelayExe {
    Usable(PathBuf),
    /// Gone after this run: macOS App Translocation, or running straight from a mounted disk image.
    Ephemeral(PathBuf),
    /// Its path has characters that bash and cmd expand differently inside double quotes.
    Unsafe(PathBuf),
}

impl RelayExe {
    pub fn classify(path: PathBuf) -> Self {
        if is_ephemeral(&path) {
            RelayExe::Ephemeral(path)
        } else if !is_relay_safe(&path) {
            RelayExe::Unsafe(path)
        } else {
            RelayExe::Usable(path)
        }
    }

    /// What the user can do about a relay Devlings can't use, for the setup status.
    pub fn setup_hint(&self) -> Option<String> {
        matches!(self, RelayExe::Ephemeral(_)).then(|| {
            "Move Devlings to Applications and open it from there. Until then, Claude Code may show hook errors \
             while Devlings is closed."
                .to_string()
        })
    }

    pub fn usable(&self) -> Option<&Path> {
        match self {
            RelayExe::Usable(p) => Some(p),
            _ => None,
        }
    }
}

/// A path that disappears after this run.
pub fn is_ephemeral(path: &Path) -> bool {
    let p = path.to_string_lossy();
    p.contains("/AppTranslocation/") || p.starts_with("/Volumes/")
}

/// A path that can go inside double quotes in a hook command and mean the same to bash and cmd.
pub fn is_relay_safe(path: &Path) -> bool {
    let Some(p) = path.to_str() else { return false };
    // `$`, backticks and `"` are live inside double quotes in sh; `%` and `!` in cmd. Backslashes are
    // Windows separators, but escapes to sh elsewhere.
    let special = |c: char| matches!(c, '$' | '`' | '"' | '%' | '!') || c.is_control() || (!cfg!(windows) && c == '\\');
    !p.is_empty() && !p.chars().any(special)
}

/// How the hooks in settings.json compare with what this build of Devlings would install.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HookStatus {
    /// No Devlings entries at all.
    NotInstalled,
    /// Exactly the expected v1 entries.
    Current,
    /// Devlings entries that differ in set or shape (an older version, another port, a moved executable).
    Outdated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Migration {
    NotInstalled,
    UpToDate,
    Migrated,
}

fn is_token(t: &str) -> bool {
    t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn is_perch_url(u: &str) -> bool {
    let Some(rest) = u.strip_prefix("http://127.0.0.1:") else { return false };
    let Some((port, tail)) = rest.split_once('/') else { return false };
    let Some(token) = tail.strip_prefix("hook/") else { return false };
    port.parse::<u16>().is_ok() && is_token(token)
}

/// A hook command that runs Devlings' relay: it contains `--hook-relay <port> <64 hex>`.
pub fn is_perch_command(c: &str) -> bool {
    c.split(RELAY_FLAG).skip(1).any(|rest| {
        let mut words = rest.split_whitespace();
        rest.starts_with(char::is_whitespace)
            && words.next().is_some_and(|p| p.parse::<u16>().is_ok())
            && words.next().is_some_and(is_token)
    })
}

fn is_perch_hook(h: &Value) -> bool {
    h.get("url").and_then(Value::as_str).is_some_and(is_perch_url)
        || h.get("command").and_then(Value::as_str).is_some_and(is_perch_command)
}

pub fn hook_url(port: u16, token: &str) -> String {
    format!("http://127.0.0.1:{port}/hook/{token}")
}

/// `"<exe>" --hook-relay <port> <token>`. Only for paths that pass `is_relay_safe`, which need no escaping.
pub fn relay_command(exe: &Path, port: u16, token: &str) -> String {
    format!("\"{}\" {RELAY_FLAG} {port} {token}", exe.display())
}

/// Every hook object Devlings installs, with its event.
pub fn expected(t: HookTarget) -> Vec<(&'static str, Value)> {
    let url = hook_url(t.port, t.token);
    let http = |timeout: u64| json!({ "type": "http", "url": url, "timeout": timeout });
    // Without a usable executable, the relay events fall back to HTTP (noisy while Devlings is quit, but they work).
    let relay = match t.exe {
        Some(exe) => json!({ "type": "command", "command": relay_command(exe, t.port, t.token), "async": true }),
        None => http(HTTP_TIMEOUT_SECS),
    };
    let mut out: Vec<(&'static str, Value)> = HTTP_EVENTS.iter().map(|ev| (*ev, http(HTTP_TIMEOUT_SECS))).collect();
    out.push((PERMISSION_EVENT, http(PERMISSION_TIMEOUT_SECS)));
    out.extend(RELAY_EVENTS.iter().map(|ev| (*ev, relay.clone())));
    out
}

/// Every Devlings hook object found in `settings`, with its event.
fn perch_hooks(settings: &Value) -> Vec<(String, Value)> {
    let Some(hooks) = settings.get("hooks").and_then(Value::as_object) else { return Vec::new() };
    let mut out = Vec::new();
    for (event, groups) in hooks {
        for group in groups.as_array().into_iter().flatten() {
            for h in group.get("hooks").and_then(Value::as_array).into_iter().flatten() {
                if is_perch_hook(h) {
                    out.push((event.clone(), h.clone()));
                }
            }
        }
    }
    out
}

/// What a hook entry means to Claude Code: event, type, URL or command, timeout (in ms, so 2 and 2.0 match)
/// and async. Key order and fields Devlings doesn't set don't count.
type Meaning = (String, Option<String>, Option<String>, Option<String>, Option<i64>, bool);

fn meaning(event: &str, h: &Value) -> Meaning {
    let text = |k: &str| h.get(k).and_then(Value::as_str).map(str::to_string);
    let timeout_ms = h.get("timeout").and_then(Value::as_f64).map(|t| (t * 1000.0).round() as i64);
    let is_async = h.get("async").and_then(Value::as_bool).unwrap_or(false);
    (event.to_string(), text("type"), text("url"), text("command"), timeout_ms, is_async)
}

pub fn status(settings: &Value, t: HookTarget) -> HookStatus {
    let mut found: Vec<Meaning> = perch_hooks(settings).iter().map(|(ev, h)| meaning(ev, h)).collect();
    if found.is_empty() {
        return HookStatus::NotInstalled;
    }
    let mut wanted: Vec<Meaning> = expected(t).iter().map(|(ev, h)| meaning(ev, h)).collect();
    found.sort();
    wanted.sort();
    if found == wanted { HookStatus::Current } else { HookStatus::Outdated }
}

pub fn uninstall(mut settings: Value) -> Value {
    let mut touched_any = false;
    let mut hooks_now_empty = false;
    if let Some(hooks) = settings.get_mut("hooks").and_then(Value::as_object_mut) {
        let mut touched_keys: Vec<String> = Vec::new();
        for (key, list) in hooks.iter_mut() {
            let Some(groups) = list.as_array_mut() else { continue };
            let mut kept = Vec::with_capacity(groups.len());
            let mut touched = false;
            for mut g in groups.drain(..) {
                let mut removed = false;
                if let Some(hs) = g.get_mut("hooks").and_then(Value::as_array_mut) {
                    let before = hs.len();
                    hs.retain(|h| !is_perch_hook(h));
                    removed = hs.len() != before;
                }
                let empty = g.get("hooks").and_then(Value::as_array).is_some_and(|a| a.is_empty());
                touched |= removed;
                if !(removed && empty) {
                    kept.push(g);
                }
            }
            *groups = kept;
            if touched {
                touched_keys.push(key.clone());
            }
        }
        touched_any = !touched_keys.is_empty();
        hooks.retain(|k, v| !(touched_keys.contains(k) && v.as_array().is_some_and(|a| a.is_empty())));
        hooks_now_empty = hooks.is_empty();
    }
    if touched_any && hooks_now_empty {
        if let Some(obj) = settings.as_object_mut() {
            obj.retain(|k, _| k != "hooks");
        }
    }
    settings
}

pub fn install(settings: Value, t: HookTarget) -> Result<Value, String> {
    let mut settings = if settings.is_null() { json!({}) } else { uninstall(settings) };
    let obj = settings.as_object_mut().ok_or("settings.json must contain a JSON object.")?;
    let hooks = obj
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("\"hooks\" in settings.json must be an object.")?;
    for (event, hook) in expected(t) {
        let list = hooks
            .entry(event)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| format!("\"hooks.{event}\" in settings.json must be an array."))?;
        list.push(json!({ "hooks": [hook] }));
    }
    Ok(settings)
}

/// The program the relay hooks run: this executable, or the AppImage file itself (its mount path changes every launch).
/// Found once per run: the path doesn't change while Devlings runs, and the setup status asks every second.
pub fn relay_exe() -> Result<RelayExe, String> {
    static RELAY: std::sync::OnceLock<Result<RelayExe, String>> = std::sync::OnceLock::new();
    RELAY
        .get_or_init(|| {
            #[cfg(target_os = "linux")]
            if let Some(appimage) = std::env::var_os("APPIMAGE").filter(|p| !p.is_empty()) {
                return Ok(RelayExe::classify(PathBuf::from(appimage)));
            }
            std::env::current_exe()
                .map(RelayExe::classify)
                .map_err(|e| format!("Couldn't find the Devlings program file: {e}"))
        })
        .clone()
}

pub fn settings_path() -> PathBuf {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".claude"))
        .join("settings.json")
}

fn parse_settings(path: &Path, raw: Option<&[u8]>) -> Result<Value, String> {
    let Some(raw) = raw else { return Ok(json!({})) };
    // Never "repair" bytes: a rewrite would silently change them.
    let text = std::str::from_utf8(raw)
        .map_err(|_| format!("{} isn't valid UTF-8 text. Fix it, then try again.", path.display()))?;
    let text = text.trim_start_matches('\u{feff}');
    if text.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(text).map_err(|e| format!("{} isn't valid JSON ({e}). Fix it, then try again.", path.display()))
}

fn read_raw(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::read(path) {
        Ok(b) => Ok(Some(b)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("Couldn't read {}: {e}", path.display())),
    }
}

pub fn read_settings(path: &Path) -> Result<Value, String> {
    parse_settings(path, read_raw(path)?.as_deref())
}

/// Orders backup file names oldest first: `settings.json.devlings-backup-<stamp>[-<n>]` (or the pre-rename
/// `settings.json.perch-backup-…`). None for other names.
fn backup_key(name: &str) -> Option<(i64, u32)> {
    let rest = name.strip_prefix(BACKUP_PREFIX).or_else(|| name.strip_prefix(LEGACY_BACKUP_PREFIX))?;
    let (stamp, n) = match rest.split_once('-') {
        Some((stamp, n)) => (stamp, n.parse().ok()?),
        None => (rest, 0),
    };
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    digits(stamp).then(|| stamp.parse().ok()).flatten().map(|stamp| (stamp, n))
}

/// A backup path for `stamp` that doesn't exist yet: `-1`, `-2`… are added for writes within the same second.
fn backup_path(path: &Path, stamp: i64) -> PathBuf {
    let first = path.with_file_name(format!("{BACKUP_PREFIX}{stamp}"));
    if !first.exists() {
        return first;
    }
    (1u32..)
        .map(|n| path.with_file_name(format!("{BACKUP_PREFIX}{stamp}-{n}")))
        .find(|p| !p.exists())
        .unwrap_or(first)
}

/// Deletes all but the newest `keep` Devlings backups next to `path`.
fn prune_backups(path: &Path, keep: usize) {
    let Some(dir) = path.parent() else { return };
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut backups: Vec<((i64, u32), PathBuf)> = entries
        .flatten()
        .filter_map(|e| backup_key(&e.file_name().to_string_lossy()).map(|k| (k, e.path())))
        .collect();
    backups.sort();
    let excess = backups.len().saturating_sub(keep);
    for (_, old) in backups.into_iter().take(excess) {
        if let Err(e) = fs::remove_file(&old) {
            log::warn!("Couldn't remove old backup {}: {e}", old.display());
        }
    }
}

/// Gives `to` the permissions of `from` on Unix, so a private settings.json doesn't become world-readable.
fn keep_permissions(from: &Path, to: &Path) {
    #[cfg(unix)]
    if let Ok(meta) = fs::metadata(from) {
        let _ = fs::set_permissions(to, meta.permissions());
    }
    #[cfg(not(unix))]
    let _ = (from, to);
}

/// Reads settings.json, applies `change`, and writes the result atomically with a backup.
/// `change` returns None when nothing needs to change. If the file changes between the read and the write,
/// starts over, up to three times. `before_commit` runs just before that check (tests use it to race).
fn update_file_with(
    path: &Path,
    stamp: i64,
    mut change: impl FnMut(Value) -> Result<Option<Value>, String>,
    mut before_commit: impl FnMut(),
    rename: impl Fn(&Path, &Path) -> std::io::Result<()>,
) -> Result<bool, String> {
    let io = |e: std::io::Error| format!("Couldn't write {}: {e}", path.display());
    // A symlinked settings.json (dotfiles) stays a symlink: the file it points to is replaced instead.
    let target = match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => fs::canonicalize(path).map_err(io)?,
        _ => path.to_path_buf(),
    };
    let tmp = target.with_file_name("settings.json.devlings-tmp");
    for _ in 0..WRITE_ATTEMPTS {
        let raw = read_raw(path)?;
        let current = parse_settings(path, raw.as_deref())?;
        let Some(next) = change(current)? else { return Ok(false) };
        if let Some(dir) = target.parent() {
            fs::create_dir_all(dir).map_err(io)?;
        }
        let text = serde_json::to_string_pretty(&next).map_err(|e| e.to_string())? + "\n";
        fs::write(&tmp, text).map_err(io)?;
        keep_permissions(&target, &tmp);
        before_commit();
        // Someone else (usually Claude Code) wrote the file since it was read: start over from their version.
        if read_raw(path)? != raw {
            let _ = fs::remove_file(&tmp);
            continue;
        }
        let backup = match &raw {
            Some(original) => {
                let backup = backup_path(path, stamp);
                if let Err(e) = fs::write(&backup, original) {
                    let _ = fs::remove_file(&tmp);
                    return Err(io(e));
                }
                keep_permissions(&target, &backup);
                Some(backup)
            }
            None => None,
        };
        if let Err(e) = rename(&tmp, &target) {
            // Nothing changed, so leave the folder as it was: no temp file, no extra backup, no pruning.
            let _ = fs::remove_file(&tmp);
            if let Some(b) = backup {
                let _ = fs::remove_file(b);
            }
            return Err(io(e));
        }
        // Only once the new file is in place do the old backups go.
        prune_backups(path, MAX_BACKUPS);
        return Ok(true);
    }
    Err(format!("{} kept changing while Devlings was updating it. Try again in a moment.", path.display()))
}

fn update_file(path: &Path, stamp: i64, change: impl FnMut(Value) -> Result<Option<Value>, String>) -> Result<bool, String> {
    update_file_with(path, stamp, change, || {}, |from, to| fs::rename(from, to))
}

pub fn install_file(path: &Path, t: HookTarget, stamp: i64) -> Result<(), String> {
    update_file(path, stamp, |v| install(v, t).map(Some)).map(|_| ())
}

/// Removes Devlings' entries. Returns whether the file changed.
pub fn uninstall_file(path: &Path, stamp: i64) -> Result<bool, String> {
    if !path.exists() {
        return Ok(false);
    }
    update_file(path, stamp, |current| {
        let next = uninstall(current.clone());
        Ok((next != current).then_some(next))
    })
}

/// Reinstalls once when Devlings' entries exist but differ from this build's; never touches a current or hook-free file.
pub fn migrate_file(path: &Path, t: HookTarget, stamp: i64) -> Result<Migration, String> {
    let mut found = Migration::NotInstalled;
    let changed = update_file(path, stamp, |current| match status(&current, t) {
        HookStatus::NotInstalled => Ok(None),
        HookStatus::Current => {
            found = Migration::UpToDate;
            Ok(None)
        }
        HookStatus::Outdated => install(current, t).map(Some),
    })?;
    Ok(if changed { Migration::Migrated } else { found })
}

pub fn free_port() -> std::io::Result<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

pub fn new_token() -> String {
    let bytes: [u8; 32] = rand::random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const OTHER: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

    fn exe() -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(r"C:\Users\me\AppData\Local\Devlings\devlings.exe")
        } else {
            PathBuf::from("/opt/Devlings/devlings")
        }
    }

    fn is_installed(settings: &Value, t: HookTarget) -> bool {
        status(settings, t) == HookStatus::Current
    }

    fn target(port: u16, exe: &Path) -> HookTarget<'_> {
        HookTarget { port, token: TOKEN, exe: Some(exe) }
    }

    fn foreign() -> Value {
        json!({
            "model": "opus",
            "hooks": {
                "PreToolUse": [
                    { "matcher": "Bash", "hooks": [ { "type": "command", "command": "echo hi" } ] }
                ],
                "Stop": [
                    { "hooks": [ { "type": "command", "command": "notify-send done", "async": true } ] }
                ]
            },
            "permissions": { "allow": ["Bash(git status *)"] }
        })
    }

    /// What v0.2 installed: seven HTTP entries with a 2 s timeout.
    fn v02(port: u16) -> Value {
        let url = hook_url(port, TOKEN);
        let mut hooks = serde_json::Map::new();
        for ev in ["SessionStart", "UserPromptSubmit", "PreToolUse", "Notification", "Stop", "StopFailure", "SessionEnd"] {
            hooks.insert(ev.into(), json!([{ "hooks": [ { "type": "http", "url": url, "timeout": 2 } ] }]));
        }
        json!({ "hooks": hooks })
    }

    #[test]
    fn perch_url_shape() {
        assert!(is_perch_url(&format!("http://127.0.0.1:4545/hook/{TOKEN}")));
        assert!(!is_perch_url("http://127.0.0.1:4545/hook/short"));
        assert!(!is_perch_url(&format!("http://localhost:4545/hook/{TOKEN}")));
        assert!(!is_perch_url(&format!("http://127.0.0.1:notaport/hook/{TOKEN}")));
    }

    #[test]
    fn perch_command_shape() {
        assert!(is_perch_command(&format!("\"C:\\Program Files\\Devlings\\devlings.exe\" --hook-relay 4545 {TOKEN}")));
        assert!(is_perch_command(&format!("/usr/bin/perch --hook-relay 1 {TOKEN}")));
        assert!(is_perch_command(&format!("perch  --hook-relay\t4545  {TOKEN} ")));
        assert!(!is_perch_command(&format!("perch --hook-relay 4545 {}", &TOKEN[1..])));
        assert!(!is_perch_command(&format!("perch --hook-relay 99999 {TOKEN}")));
        assert!(!is_perch_command(&format!("perch --hook-relay {TOKEN}")));
        assert!(!is_perch_command(&format!("perch --hook-relayx 4545 {TOKEN}")));
        assert!(!is_perch_command(&format!("perch --hook-relay 4545 {TOKEN}zz")));
        assert!(!is_perch_command("echo hi"));
    }

    #[test]
    fn relay_command_quotes_the_path() {
        let cmd = relay_command(&exe(), 4545, TOKEN);
        assert_eq!(cmd, format!("\"{}\" --hook-relay 4545 {TOKEN}", exe().display()));
        assert!(is_perch_command(&cmd));
        #[cfg(windows)]
        assert_eq!(
            relay_command(Path::new(r"C:\Program Files\Devlings App\devlings.exe"), 1, TOKEN),
            format!("\"C:\\Program Files\\Devlings App\\devlings.exe\" --hook-relay 1 {TOKEN}")
        );
    }

    #[test]
    fn relay_paths_with_shell_characters_are_not_used() {
        for bad in ["/opt/a$b/perch", "/opt/a`b`/perch", "/opt/a\"b/perch", "/opt/100%/perch", "/opt/hi!/perch", "/opt/a\nb/perch"] {
            assert!(!is_relay_safe(Path::new(bad)), "{bad}");
            assert_eq!(RelayExe::classify(PathBuf::from(bad)), RelayExe::Unsafe(PathBuf::from(bad)));
        }
        for good in ["/opt/Devlings App/devlings", "/home/jane/.local/bin/perch", "/opt/(x) [y] & 'z'/perch"] {
            assert!(is_relay_safe(Path::new(good)), "{good}");
        }
        assert!(is_relay_safe(Path::new(r"C:\Users\Jane Doe\AppData\Local\Devlings\devlings.exe")) == cfg!(windows));
        let usable = RelayExe::classify(exe());
        assert_eq!(usable.usable(), Some(exe().as_path()));
    }

    #[test]
    fn ephemeral_paths_are_not_used() {
        for gone in [
            "/private/var/folders/xy/T/AppTranslocation/0A1B/d/Devlings.app/Contents/MacOS/devlings",
            "/Volumes/Devlings 1.0.0/Devlings.app/Contents/MacOS/devlings",
        ] {
            assert!(is_ephemeral(Path::new(gone)), "{gone}");
            assert_eq!(RelayExe::classify(PathBuf::from(gone)).usable(), None);
            assert!(matches!(RelayExe::classify(PathBuf::from(gone)), RelayExe::Ephemeral(_)));
        }
        for stays in ["/Applications/Devlings.app/Contents/MacOS/devlings", "/Users/jane/Applications/Devlings.app/Contents/MacOS/devlings", "/opt/Volumes/perch"] {
            assert!(!is_ephemeral(Path::new(stays)), "{stays}");
        }
    }

    #[test]
    fn only_an_ephemeral_path_asks_the_user_to_move_perch() {
        let hint = RelayExe::Ephemeral(PathBuf::from("/Volumes/Devlings/Devlings.app/Contents/MacOS/devlings")).setup_hint().unwrap();
        assert!(hint.contains("Move Devlings to Applications"), "{hint}");
        assert_eq!(RelayExe::Usable(exe()).setup_hint(), None);
        assert_eq!(RelayExe::Unsafe(PathBuf::from("/opt/a$b/perch")).setup_hint(), None);
    }

    #[test]
    fn without_a_usable_relay_the_async_events_use_http() {
        let exe = exe();
        let t = HookTarget { port: 4545, token: TOKEN, exe: None };
        let v = install(json!({}), t).unwrap();
        let url = hook_url(4545, TOKEN);
        for ev in RELAY_EVENTS {
            assert_eq!(v["hooks"][ev], json!([{ "hooks": [ { "type": "http", "url": url, "timeout": 2 } ] }]), "{ev}");
        }
        assert_eq!(v["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"], 300);
        assert!(!v.to_string().contains("--hook-relay"));
        assert_eq!(status(&v, t), HookStatus::Current);
        // Once a usable path exists (Devlings moved to Applications), the HTTP fallback is outdated, and back.
        assert_eq!(status(&v, target(4545, &exe)), HookStatus::Outdated);
        assert_eq!(status(&install(json!({}), target(4545, &exe)).unwrap(), t), HookStatus::Outdated);
        assert_eq!(uninstall(v), json!({}));
    }

    #[test]
    fn status_compares_entries_by_meaning() {
        let exe = exe();
        let t = target(4545, &exe);
        let url = hook_url(4545, TOKEN);
        let cmd = relay_command(&exe, 4545, TOKEN);
        let mut hooks = serde_json::Map::new();
        for ev in HTTP_EVENTS {
            // Other key order, 2.0 instead of 2, an explicit "async": false and an extra field.
            hooks.insert(ev.into(), json!([{ "hooks": [ { "timeout": 2.0, "url": url, "async": false, "statusMessage": "x", "type": "http" } ] }]));
        }
        hooks.insert("PermissionRequest".into(), json!([{ "hooks": [ { "url": url, "type": "http", "timeout": 300 } ] }]));
        for ev in RELAY_EVENTS {
            hooks.insert(ev.into(), json!([{ "hooks": [ { "async": true, "command": cmd, "type": "command", "extra": [1] } ] }]));
        }
        let v = json!({ "hooks": hooks });
        assert_eq!(status(&v, t), HookStatus::Current);

        let mut not_async = v.clone();
        not_async["hooks"]["Stop"][0]["hooks"][0].as_object_mut().unwrap().remove("async");
        assert_eq!(status(&not_async, t), HookStatus::Outdated);
        let mut other_timeout = v.clone();
        other_timeout["hooks"]["SessionStart"][0]["hooks"][0]["timeout"] = json!(2.5);
        assert_eq!(status(&other_timeout, t), HookStatus::Outdated);
        let mut missing_timeout = v.clone();
        missing_timeout["hooks"]["SessionStart"][0]["hooks"][0].as_object_mut().unwrap().remove("timeout");
        assert_eq!(status(&missing_timeout, t), HookStatus::Outdated);
        let mut wrong_type = v;
        wrong_type["hooks"]["SessionStart"][0]["hooks"][0]["type"] = json!("command");
        assert_eq!(status(&wrong_type, t), HookStatus::Outdated);
    }

    #[test]
    fn install_into_empty_matches_the_v1_table() {
        let exe = exe();
        let v = install(json!({}), target(4545, &exe)).unwrap();
        let url = hook_url(4545, TOKEN);
        for ev in HTTP_EVENTS {
            assert_eq!(v["hooks"][ev], json!([{ "hooks": [ { "type": "http", "url": url, "timeout": 2 } ] }]), "{ev}");
        }
        assert_eq!(
            v["hooks"]["PermissionRequest"],
            json!([{ "hooks": [ { "type": "http", "url": url, "timeout": 300 } ] }])
        );
        let cmd = relay_command(&exe, 4545, TOKEN);
        for ev in RELAY_EVENTS {
            assert_eq!(v["hooks"][ev], json!([{ "hooks": [ { "type": "command", "command": cmd, "async": true } ] }]), "{ev}");
        }
        assert_eq!(v["hooks"].as_object().unwrap().len(), 11);
        assert_eq!(expected(target(4545, &exe)).len(), 11);
        assert_eq!(status(&v, target(4545, &exe)), HookStatus::Current);
        assert!(is_installed(&v, target(4545, &exe)));
        assert_eq!(status(&v, target(4546, &exe)), HookStatus::Outdated);
    }

    #[test]
    fn status_detects_every_difference() {
        let exe = exe();
        let t = target(4545, &exe);
        assert_eq!(status(&json!({}), t), HookStatus::NotInstalled);
        assert_eq!(status(&foreign(), t), HookStatus::NotInstalled);
        assert_eq!(status(&json!({"hooks": []}), t), HookStatus::NotInstalled);
        assert_eq!(status(&v02(4545), t), HookStatus::Outdated);

        let current = install(foreign(), t).unwrap();
        assert_eq!(status(&current, t), HookStatus::Current);

        let moved = Path::new("/elsewhere/perch");
        assert_eq!(status(&current, HookTarget { exe: Some(moved), ..t }), HookStatus::Outdated);
        assert_eq!(status(&current, HookTarget { token: OTHER, ..t }), HookStatus::Outdated);

        let mut missing_one = current.clone();
        missing_one["hooks"].as_object_mut().unwrap().remove("PostToolUseFailure");
        assert_eq!(status(&missing_one, t), HookStatus::Outdated);

        let mut duplicated = current.clone();
        duplicated["hooks"]["Stop"].as_array_mut().unwrap().push(json!({ "hooks": [ { "type": "command", "command": relay_command(&exe, 4545, TOKEN), "async": true } ] }));
        assert_eq!(status(&duplicated, t), HookStatus::Outdated);

        let mut wrong_timeout = current.clone();
        wrong_timeout["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"] = json!(2);
        assert_eq!(status(&wrong_timeout, t), HookStatus::Outdated);

        let mut stray = current;
        stray["hooks"]["SubagentStop"] = json!([{ "hooks": [ { "type": "http", "url": hook_url(4545, TOKEN), "timeout": 2 } ] }]);
        assert_eq!(status(&stray, t), HookStatus::Outdated);
    }

    #[test]
    fn install_keeps_foreign_config() {
        let exe = exe();
        let v = install(foreign(), target(4545, &exe)).unwrap();
        assert_eq!(v["model"], "opus");
        assert_eq!(v["permissions"], foreign()["permissions"]);
        let pre = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 2);
        assert_eq!(pre[0], foreign()["hooks"]["PreToolUse"][0]);
        let stop = v["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 2);
        assert_eq!(stop[0], foreign()["hooks"]["Stop"][0]);
    }

    #[test]
    fn reinstall_replaces_old_entries_including_v02() {
        let exe = exe();
        let v = install(install(json!({}), target(5000, &exe)).unwrap(), target(6000, &exe)).unwrap();
        assert!(!v.to_string().contains("127.0.0.1:5000"));
        assert!(!v.to_string().contains("--hook-relay 5000"));
        assert_eq!(status(&v, target(6000, &exe)), HookStatus::Current);
        for (ev, _) in expected(target(6000, &exe)) {
            assert_eq!(v["hooks"][ev].as_array().unwrap().len(), 1, "{ev}");
        }
        let migrated = install(v02(4545), target(4545, &exe)).unwrap();
        assert_eq!(status(&migrated, target(4545, &exe)), HookStatus::Current);
        assert_eq!(migrated, install(json!({}), target(4545, &exe)).unwrap());
    }

    #[test]
    fn uninstall_restores_original_exactly() {
        let exe = exe();
        let original = foreign();
        let round = uninstall(install(original.clone(), target(4545, &exe)).unwrap());
        assert_eq!(round, original);
        assert_eq!(round.to_string(), original.to_string());
        assert_eq!(uninstall(original.clone()), original);
        assert_eq!(uninstall(install(json!({}), target(4545, &exe)).unwrap()), json!({}));
        assert_eq!(uninstall(v02(4545)), json!({}));
    }

    #[test]
    fn uninstall_removes_relay_entries_for_any_port_and_exe() {
        let mut v = foreign();
        v["hooks"]["SessionEnd"] = json!([{ "hooks": [ { "type": "command", "command": format!("/old/place/perch --hook-relay 1234 {OTHER}"), "async": true } ] }]);
        assert_eq!(uninstall(v), foreign());
    }

    #[test]
    fn uninstall_keeps_foreign_hooks_that_share_a_group() {
        let exe = exe();
        let mut v = foreign();
        v["hooks"]["Stop"][0]["hooks"].as_array_mut().unwrap().push(json!({ "type": "command", "command": relay_command(&exe, 4545, TOKEN), "async": true }));
        assert_eq!(uninstall(v), foreign());
    }

    #[test]
    fn uninstall_leaves_untouched_empty_lists_alone() {
        let v = json!({ "hooks": { "Stop": [] } });
        assert_eq!(uninstall(v.clone()), v);
    }

    #[test]
    fn rejects_non_object_settings() {
        let exe = exe();
        assert!(install(json!([1, 2]), target(4545, &exe)).is_err());
        assert!(install(json!({"hooks": []}), target(4545, &exe)).is_err());
        assert!(install(json!({"hooks": {"Stop": {}}}), target(4545, &exe)).is_err());
    }

    #[test]
    fn file_install_backs_up_and_refuses_bad_json() {
        let exe = exe();
        let t = target(4545, &exe);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, foreign().to_string()).unwrap();
        install_file(&path, t, 1700000000).unwrap();
        let backup = dir.path().join("settings.json.devlings-backup-1700000000");
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), foreign().to_string());
        assert!(is_installed(&read_settings(&path).unwrap(), t));
        assert!(uninstall_file(&path, 1700000001).unwrap());
        assert_eq!(read_settings(&path).unwrap(), foreign());
        assert!(!uninstall_file(&path, 1700000002).unwrap());
        assert!(!dir.path().join("settings.json.devlings-backup-1700000002").exists());
        assert!(!dir.path().join("settings.json.devlings-tmp").exists());

        std::fs::write(&path, "{ nope").unwrap();
        let err = install_file(&path, t, 1).unwrap_err();
        assert!(err.contains("isn't valid JSON"), "{err}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ nope");
        assert!(uninstall_file(&path, 1).unwrap_err().contains("isn't valid JSON"));
        assert!(migrate_file(&path, t, 1).unwrap_err().contains("isn't valid JSON"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ nope");

        let missing = dir.path().join("new").join("settings.json");
        install_file(&missing, t, 2).unwrap();
        assert!(is_installed(&read_settings(&missing).unwrap(), t));
        assert!(!uninstall_file(&dir.path().join("absent.json"), 3).unwrap());
    }

    #[test]
    fn backups_in_the_same_second_do_not_overwrite_each_other() {
        let exe = exe();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, foreign().to_string()).unwrap();
        install_file(&path, target(4545, &exe), 50).unwrap();
        install_file(&path, target(4546, &exe), 50).unwrap();
        assert_eq!(std::fs::read_to_string(dir.path().join("settings.json.devlings-backup-50")).unwrap(), foreign().to_string());
        assert!(dir.path().join("settings.json.devlings-backup-50-1").exists());
    }

    #[test]
    fn keeps_only_the_newest_five_backups() {
        let exe = exe();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{}").unwrap();
        for name in ["100", "900", "20", "300", "300-1", "7"] {
            std::fs::write(dir.path().join(format!("{BACKUP_PREFIX}{name}")), "{}").unwrap();
        }
        std::fs::write(dir.path().join(format!("{BACKUP_PREFIX}notes")), "mine").unwrap();
        install_file(&path, target(4545, &exe), 1000).unwrap();
        let mut left: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .filter(|n| n.starts_with(BACKUP_PREFIX))
            .map(|n| n[BACKUP_PREFIX.len()..].to_string())
            .collect();
        left.sort();
        // 7 and 20 were the oldest; files that aren't Devlings backups by name are left alone.
        assert_eq!(left, vec!["100", "1000", "300", "300-1", "900", "notes"]);
    }

    #[test]
    fn backup_names_order_by_stamp_then_counter() {
        assert_eq!(backup_key("settings.json.devlings-backup-1700000000"), Some((1700000000, 0)));
        assert_eq!(backup_key("settings.json.devlings-backup-12-3"), Some((12, 3)));
        assert_eq!(backup_key("settings.json.devlings-backup-x"), None);
        assert_eq!(backup_key("settings.json.devlings-backup-"), None);
        assert_eq!(backup_key("other.json.perch-backup-1"), None);
        assert!(backup_key("settings.json.devlings-backup-9").unwrap() < backup_key("settings.json.devlings-backup-10").unwrap());
        // Backups made before the rename still count, so they're pruned in turn.
        assert_eq!(backup_key("settings.json.perch-backup-1700000000"), Some((1700000000, 0)));
        assert_eq!(backup_key("settings.json.perch-backup-12-3"), Some((12, 3)));
        assert_eq!(backup_key("settings.json.perch-backup-x"), None);
        assert!(backup_key("settings.json.perch-backup-9").unwrap() < backup_key("settings.json.devlings-backup-10").unwrap());
    }

    #[test]
    fn backups_from_before_the_rename_count_toward_the_five() {
        let exe = exe();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{}").unwrap();
        for stamp in 1..=5 {
            std::fs::write(dir.path().join(format!("settings.json.perch-backup-{stamp}")), "{}").unwrap();
        }
        install_file(&path, target(4545, &exe), 1000).unwrap();
        let mut left: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .filter(|n| n.contains("-backup-"))
            .collect();
        left.sort();
        assert_eq!(
            left,
            vec![
                "settings.json.devlings-backup-1000",
                "settings.json.perch-backup-2",
                "settings.json.perch-backup-3",
                "settings.json.perch-backup-4",
                "settings.json.perch-backup-5"
            ]
        );
    }

    #[test]
    fn a_failed_commit_leaves_the_folder_as_it_was() {
        let exe = exe();
        let t = target(4545, &exe);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"model":"opus"}"#).unwrap();
        let old: Vec<String> = (1..=7).map(|n| format!("{BACKUP_PREFIX}{n}")).collect();
        for name in &old {
            std::fs::write(dir.path().join(name), "{}").unwrap();
        }
        let failing_rename = |_: &Path, _: &Path| Err(std::io::Error::other("disk full"));
        let err = update_file_with(&path, 100, |v| install(v, t).map(Some), || {}, failing_rename).unwrap_err();
        assert!(err.contains("disk full"), "{err}");
        let mut names: Vec<String> =
            std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
        names.sort();
        let mut want = old.clone();
        want.push("settings.json".into());
        want.sort();
        assert_eq!(names, want, "no pruning, no new backup, no temp file");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), r#"{"model":"opus"}"#);
    }

    #[test]
    fn retries_when_the_file_changes_underneath() {
        let exe = exe();
        let t = target(4545, &exe);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"model":"opus"}"#).unwrap();
        let mut races = 0;
        let changed = update_file_with(
            &path,
            5,
            |v| install(v, t).map(Some),
            || {
                if races == 0 {
                    std::fs::write(&path, r#"{"model":"sonnet"}"#).unwrap();
                }
                races += 1;
            },
            |a, b| std::fs::rename(a, b),
        )
        .unwrap();
        assert!(changed);
        assert_eq!(races, 2);
        let v = read_settings(&path).unwrap();
        assert_eq!(v["model"], "sonnet");
        assert!(is_installed(&v, t));
        // The backup holds what was replaced: the other writer's version.
        assert_eq!(std::fs::read_to_string(dir.path().join("settings.json.devlings-backup-5")).unwrap(), r#"{"model":"sonnet"}"#);

        let mut n = 0;
        let err = update_file_with(
            &path,
            6,
            |v| install(v, t).map(Some),
            || {
                n += 1;
                std::fs::write(&path, format!(r#"{{"model":"m{n}"}}"#)).unwrap();
            },
            |a, b| std::fs::rename(a, b),
        )
        .unwrap_err();
        assert_eq!(n, 3);
        assert!(err.contains("kept changing"), "{err}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), r#"{"model":"m3"}"#);
        assert!(!dir.path().join("settings.json.devlings-tmp").exists());
    }

    #[test]
    fn migrates_once_and_only_when_needed() {
        let exe = exe();
        let t = target(4545, &exe);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        assert_eq!(migrate_file(&path, t, 1).unwrap(), Migration::NotInstalled);
        assert!(!path.exists());
        std::fs::write(&path, foreign().to_string()).unwrap();
        assert_eq!(migrate_file(&path, t, 1).unwrap(), Migration::NotInstalled);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), foreign().to_string());

        let mut old = v02(4545);
        old["model"] = json!("opus");
        std::fs::write(&path, old.to_string()).unwrap();
        assert_eq!(migrate_file(&path, t, 2).unwrap(), Migration::Migrated);
        assert!(dir.path().join("settings.json.devlings-backup-2").exists());
        let v = read_settings(&path).unwrap();
        assert_eq!(status(&v, t), HookStatus::Current);
        assert_eq!(v["model"], "opus");

        let before = std::fs::read(&path).unwrap();
        let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(migrate_file(&path, t, 3).unwrap(), Migration::UpToDate);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), modified);
        assert!(!dir.path().join("settings.json.devlings-backup-3").exists());

        // A moved executable counts as outdated.
        let moved = Path::new("/new/home/perch");
        assert_eq!(migrate_file(&path, HookTarget { exe: Some(moved), ..t }, 4).unwrap(), Migration::Migrated);
        assert!(read_settings(&path).unwrap().to_string().contains("/new/home/perch"));
    }

    /// v1.1 renamed the app. Perch 1.0's relay entries run perch.exe from Perch's folder, which the Windows installer
    /// removes; the port and token live in the shared app-data folder, so only the executable differs. On first launch
    /// that counts as outdated, and the entries are rewritten once, with a backup, to run devlings.exe.
    #[test]
    fn hooks_from_perch_migrate_to_the_devlings_exe() {
        let perch = Path::new(r"C:\Users\me\AppData\Local\Perch\perch.exe");
        let devlings = Path::new(r"C:\Users\me\AppData\Local\Devlings\devlings.exe");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let before = install(foreign(), target(4545, perch)).unwrap();
        std::fs::write(&path, before.to_string()).unwrap();

        let t = target(4545, devlings);
        assert_eq!(status(&before, t), HookStatus::Outdated);
        assert_eq!(migrate_file(&path, t, 10).unwrap(), Migration::Migrated);

        let after = read_settings(&path).unwrap();
        assert_eq!(status(&after, t), HookStatus::Current);
        for event in ["Stop", "StopFailure", "SessionEnd"] {
            let commands: Vec<&str> = after["hooks"][event]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|g| g["hooks"].as_array().unwrap())
                .filter_map(|h| h["command"].as_str())
                .filter(|c| is_perch_command(c))
                .collect();
            assert_eq!(commands, vec![relay_command(devlings, 4545, TOKEN)], "{event}");
        }
        assert!(!after.to_string().contains("perch.exe"), "{after}");
        // Everything that isn't ours is untouched, and the file as Perch left it is backed up.
        assert_eq!(uninstall(after.clone()), foreign());
        let backup = dir.path().join("settings.json.devlings-backup-10");
        assert_eq!(read_settings(&backup).unwrap(), before);
        // Once.
        assert_eq!(migrate_file(&path, t, 11).unwrap(), Migration::UpToDate);
        assert!(!dir.path().join("settings.json.devlings-backup-11").exists());
    }

    /// Design D2: the PermissionRequest entry went from 75 s to 300 s; earlier v1 installs migrate once.
    #[test]
    fn a_75_second_permission_entry_migrates_to_300() {
        let exe = exe();
        let t = target(4545, &exe);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut old = install(foreign(), t).unwrap();
        old["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"] = json!(75);
        assert_eq!(status(&old, t), HookStatus::Outdated);
        std::fs::write(&path, old.to_string()).unwrap();
        assert_eq!(migrate_file(&path, t, 7).unwrap(), Migration::Migrated);
        assert!(dir.path().join("settings.json.devlings-backup-7").exists());
        let v = read_settings(&path).unwrap();
        assert_eq!(v["hooks"]["PermissionRequest"], json!([{ "hooks": [ { "type": "http", "url": hook_url(4545, TOKEN), "timeout": 300 } ] }]));
        assert_eq!(status(&v, t), HookStatus::Current);
        assert_eq!(v["model"], "opus");
        assert_eq!(migrate_file(&path, t, 8).unwrap(), Migration::UpToDate);
    }

    #[test]
    fn refuses_to_rewrite_invalid_utf8() {
        let exe = exe();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, b"{\"model\":\"op\xffus\"}").unwrap();
        let err = install_file(&path, target(4545, &exe), 1).unwrap_err();
        assert!(err.contains("UTF-8"), "{err}");
        assert_eq!(std::fs::read(&path).unwrap(), b"{\"model\":\"op\xffus\"}");
    }

    #[cfg(unix)]
    #[test]
    fn keeps_a_symlinked_settings_file_a_symlink_and_private() {
        use std::os::unix::fs::PermissionsExt;
        let exe = exe();
        let t = target(4545, &exe);
        let dir = tempfile::tempdir().unwrap();
        let dotfiles = dir.path().join("dotfiles");
        std::fs::create_dir_all(&dotfiles).unwrap();
        let real = dotfiles.join("claude-settings.json");
        std::fs::write(&real, foreign().to_string()).unwrap();
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o600)).unwrap();
        let link = dir.path().join("settings.json");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        install_file(&link, t, 9).unwrap();
        assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
        assert!(is_installed(&read_settings(&real).unwrap(), t));
        assert_eq!(std::fs::metadata(&real).unwrap().permissions().mode() & 0o777, 0o600);
        let backup = dir.path().join("settings.json.devlings-backup-9");
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), foreign().to_string());
        assert_eq!(std::fs::metadata(&backup).unwrap().permissions().mode() & 0o777, 0o600);
    }

    #[test]
    fn reads_settings_with_byte_order_mark() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "\u{feff}{\"model\":\"opus\"}").unwrap();
        assert_eq!(read_settings(&path).unwrap(), json!({"model": "opus"}));
    }

    #[test]
    fn token_and_port() {
        let t = new_token();
        assert_eq!(t.len(), 64);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(free_port().unwrap() > 0);
    }
}
