use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use serde_json::{json, Value};

pub const HOOK_EVENTS: [&str; 7] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "Notification",
    "Stop",
    "StopFailure",
    "SessionEnd",
];

pub fn is_perch_url(u: &str) -> bool {
    let Some(rest) = u.strip_prefix("http://127.0.0.1:") else { return false };
    let Some((port, tail)) = rest.split_once('/') else { return false };
    let Some(token) = tail.strip_prefix("hook/") else { return false };
    port.parse::<u16>().is_ok() && token.len() == 64 && token.chars().all(|c| c.is_ascii_hexdigit())
}

fn is_perch_hook(h: &Value) -> bool {
    h.get("url").and_then(Value::as_str).is_some_and(is_perch_url)
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

pub fn install(settings: Value, port: u16, token: &str) -> Result<Value, String> {
    let mut settings = if settings.is_null() { json!({}) } else { uninstall(settings) };
    let obj = settings
        .as_object_mut()
        .ok_or("settings.json must contain a JSON object.")?;
    let hooks = obj
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("\"hooks\" in settings.json must be an object.")?;
    let url = format!("http://127.0.0.1:{port}/hook/{token}");
    for event in HOOK_EVENTS {
        let list = hooks
            .entry(event)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| format!("\"hooks.{event}\" in settings.json must be an array."))?;
        list.push(json!({ "hooks": [ { "type": "http", "url": url, "timeout": 2 } ] }));
    }
    Ok(settings)
}

pub fn is_installed(settings: &Value, port: u16, token: &str) -> bool {
    let url = format!("http://127.0.0.1:{port}/hook/{token}");
    HOOK_EVENTS.iter().all(|ev| {
        settings
            .pointer(&format!("/hooks/{ev}"))
            .and_then(Value::as_array)
            .is_some_and(|groups| {
                groups.iter().any(|g| {
                    g.get("hooks").and_then(Value::as_array).is_some_and(|hs| {
                        hs.iter().any(|h| h.get("url").and_then(Value::as_str) == Some(url.as_str()))
                    })
                })
            })
    })
}

pub fn settings_path() -> PathBuf {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".claude"))
        .join("settings.json")
}

pub fn read_settings(path: &Path) -> Result<Value, String> {
    match fs::read_to_string(path) {
        Ok(t) if t.trim().is_empty() => Ok(json!({})),
        Ok(t) => serde_json::from_str(&t).map_err(|e| {
            format!("{} isn't valid JSON ({e}). Fix it, then try again.", path.display())
        }),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(format!("Couldn't read {}: {e}", path.display())),
    }
}

fn write_with_backup(path: &Path, value: &Value, stamp: i64) -> Result<(), String> {
    let io = |e: std::io::Error| format!("Couldn't write {}: {e}", path.display());
    if path.exists() {
        let backup = path.with_file_name(format!("settings.json.perch-backup-{stamp}"));
        fs::copy(path, backup).map_err(io)?;
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(io)?;
    }
    let tmp = path.with_extension("json.perch-tmp");
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())? + "\n";
    fs::write(&tmp, text).map_err(io)?;
    fs::rename(&tmp, path).map_err(io)
}

pub fn install_file(path: &Path, port: u16, token: &str, stamp: i64) -> Result<(), String> {
    let current = read_settings(path)?;
    let next = install(current, port, token)?;
    write_with_backup(path, &next, stamp)
}

pub fn uninstall_file(path: &Path, stamp: i64) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let current = read_settings(path)?;
    let next = uninstall(current.clone());
    if next == current {
        return Ok(());
    }
    write_with_backup(path, &next, stamp)
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

    fn foreign() -> Value {
        json!({
            "model": "opus",
            "hooks": {
                "PreToolUse": [
                    { "matcher": "Bash", "hooks": [ { "type": "command", "command": "echo hi" } ] }
                ]
            },
            "permissions": { "allow": ["Bash(git status *)"] }
        })
    }

    #[test]
    fn perch_url_shape() {
        assert!(is_perch_url(&format!("http://127.0.0.1:4545/hook/{TOKEN}")));
        assert!(!is_perch_url("http://127.0.0.1:4545/hook/short"));
        assert!(!is_perch_url(&format!("http://localhost:4545/hook/{TOKEN}")));
        assert!(!is_perch_url(&format!("http://127.0.0.1:notaport/hook/{TOKEN}")));
    }

    #[test]
    fn install_into_empty() {
        let v = install(json!({}), 4545, TOKEN).unwrap();
        for ev in HOOK_EVENTS {
            let groups = v["hooks"][ev].as_array().unwrap();
            assert_eq!(groups.len(), 1, "{ev}");
            assert_eq!(groups[0]["hooks"][0]["type"], "http");
            assert_eq!(groups[0]["hooks"][0]["timeout"], 2);
        }
        assert!(is_installed(&v, 4545, TOKEN));
        assert!(!is_installed(&v, 4546, TOKEN));
    }

    #[test]
    fn install_keeps_foreign_config() {
        let v = install(foreign(), 4545, TOKEN).unwrap();
        assert_eq!(v["model"], "opus");
        assert_eq!(v["permissions"], foreign()["permissions"]);
        let pre = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 2);
        assert_eq!(pre[0], foreign()["hooks"]["PreToolUse"][0]);
    }

    #[test]
    fn reinstall_on_new_port_replaces_old_entries() {
        let v = install(install(json!({}), 5000, TOKEN).unwrap(), 6000, TOKEN).unwrap();
        let text = v.to_string();
        assert!(!text.contains("127.0.0.1:5000"));
        for ev in HOOK_EVENTS {
            assert_eq!(v["hooks"][ev].as_array().unwrap().len(), 1);
        }
    }

    #[test]
    fn uninstall_restores_original_exactly() {
        let original = foreign();
        let round = uninstall(install(original.clone(), 4545, TOKEN).unwrap());
        assert_eq!(round, original);
        assert_eq!(uninstall(original.clone()), original);
        assert_eq!(uninstall(install(json!({}), 4545, TOKEN).unwrap()), json!({}));
    }

    #[test]
    fn uninstall_leaves_untouched_empty_lists_alone() {
        let v = json!({ "hooks": { "Stop": [] } });
        assert_eq!(uninstall(v.clone()), v);
    }

    #[test]
    fn rejects_non_object_settings() {
        assert!(install(json!([1, 2]), 4545, TOKEN).is_err());
        assert!(install(json!({"hooks": []}), 4545, TOKEN).is_err());
    }

    #[test]
    fn file_install_backs_up_and_refuses_bad_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, foreign().to_string()).unwrap();
        install_file(&path, 4545, TOKEN, 1700000000).unwrap();
        assert!(dir.path().join("settings.json.perch-backup-1700000000").exists());
        assert!(is_installed(&read_settings(&path).unwrap(), 4545, TOKEN));
        uninstall_file(&path, 1700000001).unwrap();
        assert_eq!(read_settings(&path).unwrap(), foreign());

        std::fs::write(&path, "{ nope").unwrap();
        let err = install_file(&path, 4545, TOKEN, 1).unwrap_err();
        assert!(err.contains("isn't valid JSON"), "{err}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ nope");

        let missing = dir.path().join("new").join("settings.json");
        install_file(&missing, 4545, TOKEN, 2).unwrap();
        assert!(is_installed(&read_settings(&missing).unwrap(), 4545, TOKEN));
    }

    #[test]
    fn token_and_port() {
        let t = new_token();
        assert_eq!(t.len(), 64);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(free_port().unwrap() > 0);
    }
}
