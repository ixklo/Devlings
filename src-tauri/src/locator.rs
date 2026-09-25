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
    found.sort_by(|a, b| b.0.cmp(&a.0));
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
            "Claude Code {}.{}.{} at {} is too old. Perch needs 2.1.259 or newer; run `claude update` or update the VS Code extension.",
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

        let err = locate(&[old.clone()], version_of).unwrap_err();
        assert!(err.contains("too old"), "{err}");
        let err = locate(&[missing], version_of).unwrap_err();
        assert!(err.contains("wasn't found"), "{err}");
    }
}
