//! Perch's entries in Claude Code's user `settings.json` (design v1.0 §3).

use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use serde::Serialize;
use serde_json::{json, Value};

/// Frequent events: HTTP, silent and cheap when Perch isn't running.
pub const HTTP_EVENTS: [&str; 7] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "Notification",
    "PermissionDenied",
];
/// Answered synchronously; Perch always answers before this long hold ends.
pub const PERMISSION_EVENT: &str = "PermissionRequest";
/// HTTP versions of these show error noise when Perch is quit, so they go through the async relay.
pub const RELAY_EVENTS: [&str; 3] = ["Stop", "StopFailure", "SessionEnd"];
pub const HTTP_TIMEOUT_SECS: u64 = 2;
pub const PERMISSION_TIMEOUT_SECS: u64 = 75;
pub const RELAY_FLAG: &str = "--hook-relay";
pub const MAX_BACKUPS: usize = 5;
const BACKUP_PREFIX: &str = "settings.json.perch-backup-";
const WRITE_ATTEMPTS: usize = 3;

/// Where Perch's hooks point: its server's port and token, and the executable that relays async events.
#[derive(Debug, Clone, Copy)]
pub struct HookTarget<'a> {
    pub port: u16,
    pub token: &'a str,
    pub exe: &'a Path,
}

/// How the hooks in settings.json compare with what this build of Perch would install.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HookStatus {
    /// No Perch entries at all.
    NotInstalled,
    /// Exactly the expected v1 entries.
    Current,
    /// Perch entries that differ in set or shape (an older version, another port, a moved executable).
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

/// A hook command that runs Perch's relay: it contains `--hook-relay <port> <64 hex>`.
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

/// `"<exe>" --hook-relay <port> <token>`, with the path quoted for the shell that runs hook commands.
pub fn relay_command(exe: &Path, port: u16, token: &str) -> String {
    let path = exe.display().to_string();
    // Windows paths can't contain quotes, and a verified hook command there keeps backslashes as they are.
    // Elsewhere the command runs through sh, where these four characters are special inside double quotes.
    let quoted = if cfg!(windows) {
        path
    } else {
        path.chars()
            .flat_map(|c| match c {
                '\\' | '"' | '$' | '`' => vec!['\\', c],
                c => vec![c],
            })
            .collect()
    };
    format!("\"{quoted}\" {RELAY_FLAG} {port} {token}")
}

/// Every hook object Perch installs, with its event.
pub fn expected(t: HookTarget) -> Vec<(&'static str, Value)> {
    let url = hook_url(t.port, t.token);
    let http = |timeout: u64| json!({ "type": "http", "url": url, "timeout": timeout });
    let relay = json!({ "type": "command", "command": relay_command(t.exe, t.port, t.token), "async": true });
    let mut out: Vec<(&'static str, Value)> = HTTP_EVENTS.iter().map(|ev| (*ev, http(HTTP_TIMEOUT_SECS))).collect();
    out.push((PERMISSION_EVENT, http(PERMISSION_TIMEOUT_SECS)));
    out.extend(RELAY_EVENTS.iter().map(|ev| (*ev, relay.clone())));
    out
}

/// Every Perch hook object found in `settings`, with its event.
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

pub fn status(settings: &Value, t: HookTarget) -> HookStatus {
    let key = |(ev, h): &(String, Value)| (ev.clone(), h.to_string());
    let mut found: Vec<(String, String)> = perch_hooks(settings).iter().map(key).collect();
    if found.is_empty() {
        return HookStatus::NotInstalled;
    }
    let mut wanted: Vec<(String, String)> = expected(t).into_iter().map(|(ev, h)| key(&(ev.to_string(), h))).collect();
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
pub fn relay_exe() -> Result<PathBuf, String> {
    #[cfg(target_os = "linux")]
    if let Some(appimage) = std::env::var_os("APPIMAGE").filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(appimage));
    }
    std::env::current_exe().map_err(|e| format!("Couldn't find Perch's own program file: {e}"))
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

/// Orders backup file names oldest first: `settings.json.perch-backup-<stamp>[-<n>]`. None for other names.
fn backup_key(name: &str) -> Option<(i64, u32)> {
    let rest = name.strip_prefix(BACKUP_PREFIX)?;
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

/// Deletes all but the newest `keep` Perch backups next to `path`.
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
) -> Result<bool, String> {
    let io = |e: std::io::Error| format!("Couldn't write {}: {e}", path.display());
    // A symlinked settings.json (dotfiles) stays a symlink: the file it points to is replaced instead.
    let target = match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => fs::canonicalize(path).map_err(io)?,
        _ => path.to_path_buf(),
    };
    let tmp = target.with_file_name("settings.json.perch-tmp");
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
        if let Some(original) = &raw {
            let backup = backup_path(path, stamp);
            if let Err(e) = fs::write(&backup, original) {
                let _ = fs::remove_file(&tmp);
                return Err(io(e));
            }
            keep_permissions(&target, &backup);
            prune_backups(path, MAX_BACKUPS);
        }
        if let Err(e) = fs::rename(&tmp, &target) {
            let _ = fs::remove_file(&tmp);
            return Err(io(e));
        }
        return Ok(true);
    }
    Err(format!("{} kept changing while Perch was updating it. Try again in a moment.", path.display()))
}

fn update_file(path: &Path, stamp: i64, change: impl FnMut(Value) -> Result<Option<Value>, String>) -> Result<bool, String> {
    update_file_with(path, stamp, change, || {})
}

pub fn install_file(path: &Path, t: HookTarget, stamp: i64) -> Result<(), String> {
    update_file(path, stamp, |v| install(v, t).map(Some)).map(|_| ())
}

/// Removes Perch's entries. Returns whether the file changed.
pub fn uninstall_file(path: &Path, stamp: i64) -> Result<bool, String> {
    if !path.exists() {
        return Ok(false);
    }
    update_file(path, stamp, |current| {
        let next = uninstall(current.clone());
        Ok((next != current).then_some(next))
    })
}

/// Reinstalls once when Perch's entries exist but differ from this build's; never touches a current or hook-free file.
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
            PathBuf::from(r"C:\Users\me\AppData\Local\Perch\perch.exe")
        } else {
            PathBuf::from("/opt/Perch/perch")
        }
    }

    fn is_installed(settings: &Value, t: HookTarget) -> bool {
        status(settings, t) == HookStatus::Current
    }

    fn target(port: u16, exe: &Path) -> HookTarget<'_> {
        HookTarget { port, token: TOKEN, exe }
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
        assert!(is_perch_command(&format!("\"C:\\Program Files\\Perch\\perch.exe\" --hook-relay 4545 {TOKEN}")));
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
            relay_command(Path::new(r"C:\Program Files\Perch App\perch.exe"), 1, TOKEN),
            format!("\"C:\\Program Files\\Perch App\\perch.exe\" --hook-relay 1 {TOKEN}")
        );
        #[cfg(not(windows))]
        assert_eq!(
            relay_command(Path::new("/home/a \"b\"/$x`y`\\z/perch"), 1, TOKEN),
            format!("\"/home/a \\\"b\\\"/\\$x\\`y\\`\\\\z/perch\" --hook-relay 1 {TOKEN}")
        );
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
            json!([{ "hooks": [ { "type": "http", "url": url, "timeout": 75 } ] }])
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
        assert_eq!(status(&current, HookTarget { exe: moved, ..t }), HookStatus::Outdated);
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
        let backup = dir.path().join("settings.json.perch-backup-1700000000");
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), foreign().to_string());
        assert!(is_installed(&read_settings(&path).unwrap(), t));
        assert!(uninstall_file(&path, 1700000001).unwrap());
        assert_eq!(read_settings(&path).unwrap(), foreign());
        assert!(!uninstall_file(&path, 1700000002).unwrap());
        assert!(!dir.path().join("settings.json.perch-backup-1700000002").exists());
        assert!(!dir.path().join("settings.json.perch-tmp").exists());

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
        assert_eq!(std::fs::read_to_string(dir.path().join("settings.json.perch-backup-50")).unwrap(), foreign().to_string());
        assert!(dir.path().join("settings.json.perch-backup-50-1").exists());
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
        // 7 and 20 were the oldest; files that aren't Perch backups by name are left alone.
        assert_eq!(left, vec!["100", "1000", "300", "300-1", "900", "notes"]);
    }

    #[test]
    fn backup_names_order_by_stamp_then_counter() {
        assert_eq!(backup_key("settings.json.perch-backup-1700000000"), Some((1700000000, 0)));
        assert_eq!(backup_key("settings.json.perch-backup-12-3"), Some((12, 3)));
        assert_eq!(backup_key("settings.json.perch-backup-x"), None);
        assert_eq!(backup_key("settings.json.perch-backup-"), None);
        assert_eq!(backup_key("other.json.perch-backup-1"), None);
        assert!(backup_key("settings.json.perch-backup-9").unwrap() < backup_key("settings.json.perch-backup-10").unwrap());
    }

    #[test]
    fn retries_when_the_file_changes_underneath() {
        let exe = exe();
        let t = target(4545, &exe);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"model":"opus"}"#).unwrap();
        let mut races = 0;
        let changed = update_file_with(&path, 5, |v| install(v, t).map(Some), || {
            if races == 0 {
                std::fs::write(&path, r#"{"model":"sonnet"}"#).unwrap();
            }
            races += 1;
        })
        .unwrap();
        assert!(changed);
        assert_eq!(races, 2);
        let v = read_settings(&path).unwrap();
        assert_eq!(v["model"], "sonnet");
        assert!(is_installed(&v, t));
        // The backup holds what was replaced: the other writer's version.
        assert_eq!(std::fs::read_to_string(dir.path().join("settings.json.perch-backup-5")).unwrap(), r#"{"model":"sonnet"}"#);

        let mut n = 0;
        let err = update_file_with(&path, 6, |v| install(v, t).map(Some), || {
            n += 1;
            std::fs::write(&path, format!(r#"{{"model":"m{n}"}}"#)).unwrap();
        })
        .unwrap_err();
        assert_eq!(n, 3);
        assert!(err.contains("kept changing"), "{err}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), r#"{"model":"m3"}"#);
        assert!(!dir.path().join("settings.json.perch-tmp").exists());
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
        assert!(dir.path().join("settings.json.perch-backup-2").exists());
        let v = read_settings(&path).unwrap();
        assert_eq!(status(&v, t), HookStatus::Current);
        assert_eq!(v["model"], "opus");

        let before = std::fs::read(&path).unwrap();
        let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(migrate_file(&path, t, 3).unwrap(), Migration::UpToDate);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), modified);
        assert!(!dir.path().join("settings.json.perch-backup-3").exists());

        // A moved executable counts as outdated.
        let moved = Path::new("/new/home/perch");
        assert_eq!(migrate_file(&path, HookTarget { exe: moved, ..t }, 4).unwrap(), Migration::Migrated);
        assert!(read_settings(&path).unwrap().to_string().contains("/new/home/perch"));
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
        let backup = dir.path().join("settings.json.perch-backup-9");
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
