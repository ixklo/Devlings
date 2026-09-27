use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use serde::Serialize;

pub const MIN_VERSION: (u32, u32, u32) = (2, 1, 259);

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Located {
    pub path: PathBuf,
    pub version: String,
}

pub fn exe_name() -> &'static str {
    if cfg!(windows) { "claude.exe" } else { "claude" }
}

pub fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let token = s.split_whitespace().next()?;
    let mut parts = token.split('.').map(|p| p.parse::<u32>().ok());
    Some((parts.next()??, parts.next()??, parts.next()??))
}

pub fn candidates(override_path: Option<&Path>, path_env: Option<&OsStr>, home: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(p) = override_path {
        out.push(p.to_path_buf());
    }
    if let Some(pe) = path_env {
        out.extend(std::env::split_paths(pe).map(|dir| dir.join(exe_name())));
    }
    out.push(home.join(".local").join("bin").join(exe_name()));
    out.push(home.join(".claude").join("local").join(exe_name()));
    out.extend(extension_binaries(home));
    out
}

pub fn extension_binaries(home: &Path) -> Vec<PathBuf> {
    let mut found: Vec<((u32, u32, u32), PathBuf)> = Vec::new();
    for root in [".vscode", ".vscode-insiders", ".cursor", ".windsurf"] {
        let Ok(entries) = std::fs::read_dir(home.join(root).join("extensions")) else { continue };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let Some(rest) = name.strip_prefix("anthropic.claude-code-") else { continue };
            let Some(version) = parse_version(&rest.replace('-', " ")) else { continue };
            if let Some(bin) = find_file(&entry.path(), exe_name(), 3) {
                found.push((version, bin));
            }
        }
    }
    found.sort_by_key(|a| std::cmp::Reverse(a.0));
    found.into_iter().map(|(_, p)| p).collect()
}

fn find_file(dir: &Path, name: &str, depth: u32) -> Option<PathBuf> {
    let direct = dir.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    if depth == 0 {
        return None;
    }
    let mut subdirs: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    subdirs.sort();
    subdirs.iter().find_map(|d| find_file(d, name, depth - 1))
}

/// How a located binary was found, in words for logs and diagnostics.
pub fn describe_source(path: &Path, override_path: Option<&Path>, path_env: Option<&OsStr>, home: &Path) -> String {
    if override_path == Some(path) {
        return "chosen in Settings".to_string();
    }
    let dir = path.parent();
    if let (Some(pe), Some(dir)) = (path_env, dir) {
        if std::env::split_paths(pe).any(|p| p == dir) {
            return "PATH".to_string();
        }
    }
    if dir == Some(home.join(".local").join("bin").as_path()) {
        return "~/.local/bin".to_string();
    }
    if dir == Some(home.join(".claude").join("local").as_path()) {
        return "~/.claude/local".to_string();
    }
    for (root, editor) in [(".vscode", "VS Code"), (".vscode-insiders", "VS Code Insiders"), (".cursor", "Cursor"), (".windsurf", "Windsurf")] {
        if let Ok(rest) = path.strip_prefix(home.join(root).join("extensions")) {
            let ext = rest.components().next().map(|c| c.as_os_str().to_string_lossy().to_string()).unwrap_or_default();
            return format!("{editor} extension ({ext})");
        }
    }
    "other location".to_string()
}

/// Finds Claude Code again: candidates in order (an override path, then PATH, then the usual
/// fallbacks and editor extensions), filtered to the first that runs `--version` at or above
/// `MIN_VERSION`. A missing or too-old `override_path` (e.g. a user-chosen path that vanished) is
/// skipped like any other bad candidate, so this naturally falls back to auto-detection.
pub fn relocate(
    override_path: Option<&Path>,
    path_env: Option<&OsStr>,
    home: &Path,
    version_of: impl Fn(&Path) -> Option<String>,
) -> Result<Located, String> {
    locate(&candidates(override_path, path_env, home), version_of)
}

/// Whether a remembered binary path should be re-located (design D-gap G3.5): it no longer exists,
/// or no longer runs `--version` successfully (moved by an extension update, deleted, or replaced).
pub fn is_stale(path: &Path, version_of: impl Fn(&Path) -> Option<String>) -> bool {
    !path.is_file() || version_of(path).and_then(|v| parse_version(&v)).is_none()
}

pub fn locate(cands: &[PathBuf], version_of: impl Fn(&Path) -> Option<String>) -> Result<Located, String> {
    let mut too_old: Option<String> = None;
    for c in cands {
        if !c.is_file() {
            continue;
        }
        let Some(v) = version_of(c).as_deref().and_then(parse_version) else { continue };
        if v >= MIN_VERSION {
            return Ok(Located { path: c.clone(), version: format!("{}.{}.{}", v.0, v.1, v.2) });
        }
        too_old.get_or_insert(format!(
            "Claude Code {}.{}.{} at {} is too old. Devlings needs 2.1.259 or newer; run `claude update` or update the VS Code extension.",
            v.0, v.1, v.2, c.display()
        ));
    }
    Err(too_old.unwrap_or_else(|| {
        "Claude Code wasn't found. Install it from https://code.claude.com, or choose its file in Settings.".to_string()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(p: &Path) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"").unwrap();
    }

    #[test]
    fn parses_versions() {
        assert_eq!(parse_version("2.1.282 (Claude Code)\n"), Some((2, 1, 282)));
        assert_eq!(parse_version("garbage"), None);
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn finds_newest_extension_binary() {
        let home = tempfile::tempdir().unwrap();
        let ext = home.path().join(".vscode").join("extensions");
        let old = ext.join("anthropic.claude-code-2.1.200-win32-x64").join("resources").join("native-binary").join(exe_name());
        let new = ext.join("anthropic.claude-code-2.1.282-win32-x64").join("resources").join("native-binary").join(exe_name());
        touch(&old);
        touch(&new);
        touch(&ext.join("someone.else-1.0.0").join(exe_name()));
        assert_eq!(extension_binaries(home.path()), vec![new, old]);
    }

    #[test]
    fn candidate_order() {
        let home = Path::new("/h");
        let c = candidates(Some(Path::new("/custom/claude")), None, home);
        assert_eq!(c[0], PathBuf::from("/custom/claude"));
        assert!(c.contains(&home.join(".local").join("bin").join(exe_name())));
    }

    #[test]
    fn describes_where_a_binary_came_from() {
        let home = Path::new("/h");
        let custom = Path::new("/custom/claude");
        assert_eq!(describe_source(custom, Some(custom), None, home), "chosen in Settings");
        let on_path = std::env::join_paths([PathBuf::from("/usr/bin"), PathBuf::from("/opt/cc")]).unwrap();
        let bin = Path::new("/opt/cc").join(exe_name());
        assert_eq!(describe_source(&bin, None, Some(&on_path), home), "PATH");
        let local = home.join(".local").join("bin").join(exe_name());
        assert_eq!(describe_source(&local, None, None, home), "~/.local/bin");
        let claude_local = home.join(".claude").join("local").join(exe_name());
        assert_eq!(describe_source(&claude_local, None, None, home), "~/.claude/local");
        let ext = home.join(".vscode").join("extensions").join("anthropic.claude-code-2.1.282-win32-x64").join("resources").join(exe_name());
        assert_eq!(describe_source(&ext, None, None, home), "VS Code extension (anthropic.claude-code-2.1.282-win32-x64)");
        let cursor = home.join(".cursor").join("extensions").join("anthropic.claude-code-2.1.1").join(exe_name());
        assert_eq!(describe_source(&cursor, None, None, home), "Cursor extension (anthropic.claude-code-2.1.1)");
        assert_eq!(describe_source(Path::new("/somewhere/claude"), None, None, home), "other location");
    }

    #[test]
    fn stale_path_is_missing_or_fails_version() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("gone").join(exe_name());
        let present = dir.path().join(exe_name());
        touch(&present);
        let version_of = |p: &Path| (p == present.as_path()).then(|| "2.1.282 (Claude Code)".to_string());
        assert!(is_stale(&missing, version_of), "a path that no longer exists is stale");
        assert!(!is_stale(&present, version_of), "a path that runs --version is not stale");
        assert!(is_stale(&present, |_: &Path| None), "a path whose --version now fails is stale");
    }

    #[test]
    fn relocate_finds_the_new_path_after_an_extension_update() {
        // The remembered path pointed at the old version's folder, which the update removed.
        let home = tempfile::tempdir().unwrap();
        let ext = home.path().join(".vscode").join("extensions");
        let stale = ext.join("anthropic.claude-code-2.1.200-win32-x64").join("resources").join("native-binary").join(exe_name());
        let new = ext.join("anthropic.claude-code-2.1.282-win32-x64").join("resources").join("native-binary").join(exe_name());
        touch(&new);
        let version_of = |p: &Path| (p == new.as_path()).then(|| "2.1.282 (Claude Code)".to_string());
        assert!(is_stale(&stale, version_of));
        let found = relocate(None, None, home.path(), version_of).unwrap();
        assert_eq!(found.path, new);
    }

    #[test]
    fn relocate_falls_back_to_auto_detect_when_the_chosen_path_vanished() {
        let home = tempfile::tempdir().unwrap();
        let chosen = home.path().join("custom").join(exe_name()); // configured in Settings, now missing
        let local = home.path().join(".local").join("bin").join(exe_name());
        touch(&local);
        let version_of = |p: &Path| (p == local.as_path()).then(|| "2.1.282 (Claude Code)".to_string());
        assert!(is_stale(&chosen, version_of));
        let found = relocate(Some(&chosen), None, home.path(), version_of).unwrap();
        assert_eq!(found.path, local);
    }

    #[test]
    fn relocate_reports_a_clear_error_when_nothing_is_found() {
        let home = tempfile::tempdir().unwrap();
        let err = relocate(None, None, home.path(), |_: &Path| None).unwrap_err();
        assert!(err.contains("wasn't found"), "{err}");
    }

    #[test]
    fn locate_skips_missing_and_too_old() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing").join(exe_name());
        let old = dir.path().join("old").join(exe_name());
        let good = dir.path().join("good").join(exe_name());
        touch(&old);
        touch(&good);
        let version_of = |p: &Path| {
            if p == old.as_path() { Some("2.1.100 (Claude Code)".to_string()) } else { Some("2.1.282 (Claude Code)".to_string()) }
        };
        let found = locate(&[missing.clone(), old.clone(), good.clone()], version_of).unwrap();
        assert_eq!(found, Located { path: good, version: "2.1.282".into() });

        let err = locate(std::slice::from_ref(&old), version_of).unwrap_err();
        assert!(err.contains("too old"), "{err}");
        let err = locate(&[missing], version_of).unwrap_err();
        assert!(err.contains("wasn't found"), "{err}");
    }
}
