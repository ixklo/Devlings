//! Untrusted folders for Ask (design v1.0 D6).
//!
//! A `claude -p` run never shows Claude Code's workspace-trust dialog and treats its folder as trusted, so it would
//! run a repository's project hooks, `env` block, helper commands, project skills and `.mcp.json` servers. Before
//! each Ask, Perch checks whether the folder is trusted (by Claude Code or in Perch) and what it would run; when it
//! is untrusted and has project configuration, the Ask gets `--setting-sources user` and a notice.
//!
//! Everything here reads files and never writes them; Claude Code's `.claude.json` is strictly read-only to Perch.

use std::{
    collections::{BTreeSet, HashMap},
    ffi::OsString,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::events::{Kind, PetEvent, Source};

/// `.claude.json` keeps per-project state and can grow; a bigger file reads as "not trusted".
pub const CLAUDE_JSON_CAP: u64 = 32 * 1024 * 1024;
/// Each scanned project file is read up to this size; a bigger one counts as unreadable.
pub const PROJECT_FILE_CAP: u64 = 1024 * 1024;
/// The flag that makes Claude Code read only the user's own settings (no project settings files, no `.mcp.json`).
pub const SETTING_SOURCES_USER: [&str; 2] = ["--setting-sources", "user"];

/// Settings keys that name a command Claude Code runs. Any other key ending in `Helper` counts too.
const HELPER_KEYS: [&str; 5] = ["apiKeyHelper", "awsAuthRefresh", "awsCredentialExport", "gcpAuthRefresh", "otelHeadersHelper"];
/// Names listed per kind in the notice before "+N more".
const MAX_NAMES: usize = 6;
/// Longest name shown, in characters (longer ones end in "…").
const MAX_NAME_CHARS: usize = 40;
/// Skill folders counted at most (a folder with more entries is still "many skills").
const MAX_SKILL_ENTRIES: usize = 10_000;

/// Claude Code's global config file: `$CLAUDE_CONFIG_DIR/.claude.json`, else `~/.claude.json`.
pub fn claude_json_path(config_dir: Option<OsString>, home: &Path) -> PathBuf {
    match config_dir.filter(|v| !v.is_empty()) {
        Some(dir) => PathBuf::from(dir).join(".claude.json"),
        None => home.join(".claude.json"),
    }
}

/// The folder holding the user's own Claude Code settings: `$CLAUDE_CONFIG_DIR`, else `~/.claude`.
pub fn user_claude_dir(config_dir: Option<OsString>, home: &Path) -> PathBuf {
    match config_dir.filter(|v| !v.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => home.join(".claude"),
    }
}

/// Drops a Windows verbatim prefix: `\\?\UNC\server\share` becomes `\\server\share`, `\\?\C:\x` becomes `C:\x`.
fn strip_verbatim(raw: &str) -> String {
    const UNC: &str = r"\\?\UNC\";
    match raw.get(..UNC.len()) {
        Some(head) if head.eq_ignore_ascii_case(UNC) => format!(r"\\{}", &raw[UNC.len()..]),
        _ => raw.strip_prefix(r"\\?\").unwrap_or(raw).to_string(),
    }
}

/// A path as a comparable string: `\` and `/` alike and case ignored on Windows, no trailing separator.
fn norm_path(raw: &str) -> String {
    let s = if cfg!(windows) {
        strip_verbatim(raw).replace('\\', "/")
    } else {
        raw.to_string()
    };
    let trimmed = s.trim_end_matches('/');
    let s = if trimmed.is_empty() && !s.is_empty() { "/" } else { trimmed };
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s.to_string()
    }
}

/// Whether `folder` is `base` or inside it, compared whole component by whole component.
fn is_at_or_under(folder: &str, base: &str) -> bool {
    if base.is_empty() {
        return false;
    }
    if folder == base {
        return true;
    }
    let prefix = if base.ends_with('/') { base.to_string() } else { format!("{base}/") };
    folder.starts_with(&prefix)
}

#[derive(Deserialize)]
struct ClaudeJson {
    #[serde(default)]
    projects: HashMap<String, ProjectState>,
}

#[derive(Deserialize)]
struct ProjectState {
    #[serde(default, rename = "hasTrustDialogAccepted")]
    has_trust_dialog_accepted: Option<Value>,
}

/// Reads a regular file of at most `cap` bytes. `Ok(None)` when it doesn't exist; `Err` when it can't be used.
fn read_capped(path: &Path, cap: u64) -> Result<Option<Vec<u8>>, ()> {
    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(()),
    };
    // Not a FIFO, device or folder: reading one could block or never end.
    if !meta.is_file() || meta.len() > cap {
        return Err(());
    }
    let mut bytes = Vec::new();
    let file = fs::File::open(path).map_err(|_| ())?;
    file.take(cap + 1).read_to_end(&mut bytes).map_err(|_| ())?;
    if bytes.len() as u64 > cap {
        return Err(());
    }
    Ok(Some(bytes))
}

fn strip_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix("\u{feff}".as_bytes()).unwrap_or(bytes)
}

/// Whether Claude Code trusts `folder`: `hasTrustDialogAccepted` is `true` in `claude_json` for a key K with the
/// folder at or under K.
/// - Inside a git repository (`repo_root`, from `git_root`), K must also be at or under the repository root, so a
///   trusted home or projects folder doesn't cover a repository cloned under it: Claude Code shows its trust dialog
///   for a nested repository.
/// - Outside any repository, any parent folder counts.
///
/// A missing, unreadable, oversized or malformed file means "not trusted".
pub fn claude_trusts(folder: &Path, repo_root: Option<&Path>, claude_json: &Path) -> bool {
    claude_trusts_capped(folder, repo_root, claude_json, CLAUDE_JSON_CAP)
}

fn claude_trusts_capped(folder: &Path, repo_root: Option<&Path>, claude_json: &Path, cap: u64) -> bool {
    // Matching is textual, so a `..` could make a folder look like it's under a trusted one when it isn't.
    // Such a path never counts as trusted; the Ask just runs with the untrusted-folder protections.
    if folder.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return false;
    }
    let Ok(Some(bytes)) = read_capped(claude_json, cap) else { return false };
    let Ok(parsed) = serde_json::from_slice::<ClaudeJson>(strip_bom(&bytes)) else { return false };
    let folder = norm_path(&folder.to_string_lossy());
    let root = repo_root.map(|r| norm_path(&r.to_string_lossy()));
    parsed.projects.iter().any(|(key, state)| {
        if state.has_trust_dialog_accepted != Some(Value::Bool(true)) || key.is_empty() {
            return false;
        }
        let key = norm_path(key);
        is_at_or_under(&folder, &key) && root.as_deref().is_none_or(|root| is_at_or_under(&key, root))
    })
}

/// What an untrusted folder would make Claude Code run. Names only: never an `env` value.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProjectConfig {
    /// Hook event names under `hooks` in `.claude/settings.json` and `.claude/settings.local.json`.
    pub hooks: BTreeSet<String>,
    /// Key names under `env` in the same files.
    pub env_keys: BTreeSet<String>,
    /// Helper-command keys present in the same files (`apiKeyHelper`, …).
    pub helpers: BTreeSet<String>,
    /// Server names under `mcpServers` in `.mcp.json`.
    pub mcp_servers: BTreeSet<String>,
    /// Skill folders in `.claude/skills/`.
    pub skills: usize,
    /// A scanned file exists but couldn't be read or parsed.
    pub unreadable: bool,
    /// A `.claude` folder or `.mcp.json` exists in a scanned place.
    pub found: bool,
}

impl ProjectConfig {
    /// Nothing to skip: no `.claude` folder and no `.mcp.json`.
    pub fn is_empty(&self) -> bool {
        !self.found
    }
}

/// The nearest folder at or above `folder` that holds `.git` (a folder, or a file in a worktree or submodule).
pub fn git_root(folder: &Path) -> Option<PathBuf> {
    folder.ancestors().find(|dir| dir.join(".git").exists()).map(Path::to_path_buf)
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => norm_path(&a.to_string_lossy()) == norm_path(&b.to_string_lossy()),
    }
}

/// A name from an untrusted file, safe to show: no control characters, at most MAX_NAME_CHARS characters.
fn clean_name(raw: &str) -> String {
    let name: String = raw.chars().filter(|c| !c.is_control()).collect();
    let name = name.trim();
    if name.is_empty() {
        return "(unnamed)".to_string();
    }
    if name.chars().count() <= MAX_NAME_CHARS {
        return name.to_string();
    }
    let cut: String = name.chars().take(MAX_NAME_CHARS - 1).collect();
    format!("{}…", cut.trim_end())
}

/// The cleaned key names of the object at `obj[key]`, if it is an object.
fn keys_of(obj: &Map<String, Value>, key: &str) -> Vec<String> {
    obj.get(key).and_then(Value::as_object).map(|m| m.keys().map(|k| clean_name(k)).collect()).unwrap_or_default()
}

/// Parses a scanned JSON file into an object. Whitespace-only reads as `{}`; anything else unusable is `Err`.
fn read_object(path: &Path) -> Result<Option<Map<String, Value>>, ()> {
    let Some(bytes) = read_capped(path, PROJECT_FILE_CAP)? else { return Ok(None) };
    let bytes = strip_bom(&bytes);
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(Some(Map::new()));
    }
    match serde_json::from_slice::<Value>(bytes) {
        Ok(Value::Object(obj)) => Ok(Some(obj)),
        _ => Err(()),
    }
}

fn scan_settings(path: &Path, out: &mut ProjectConfig) {
    let obj = match read_object(path) {
        Ok(Some(obj)) => obj,
        Ok(None) => return,
        Err(()) => {
            out.unreadable = true;
            return;
        }
    };
    out.hooks.extend(keys_of(&obj, "hooks"));
    out.env_keys.extend(keys_of(&obj, "env"));
    out.helpers.extend(
        obj.keys().filter(|k| HELPER_KEYS.contains(&k.as_str()) || k.ends_with("Helper")).map(|k| clean_name(k)),
    );
}

fn scan_place(place: &Path, user_claude_dir: &Path, out: &mut ProjectConfig) {
    let claude = place.join(".claude");
    // The user's own settings folder (asking in the home folder) holds user settings, which run anyway.
    if claude.is_dir() && !same_dir(&claude, user_claude_dir) {
        out.found = true;
        scan_settings(&claude.join("settings.json"), out);
        scan_settings(&claude.join("settings.local.json"), out);
        if let Ok(entries) = fs::read_dir(claude.join("skills")) {
            out.skills += entries.take(MAX_SKILL_ENTRIES).flatten().filter(|e| e.path().is_dir()).count();
        }
    }
    let mcp = place.join(".mcp.json");
    if fs::symlink_metadata(&mcp).is_ok() {
        out.found = true;
        match read_object(&mcp) {
            Ok(Some(obj)) => out.mcp_servers.extend(keys_of(&obj, "mcpServers")),
            Ok(None) => {}
            Err(()) => out.unreadable = true,
        }
    }
}

/// Scans `folder` and, when it is inside a git repository (`repo_root`, from `git_root`), the repository root, for
/// project configuration. `user_claude_dir` (`$CLAUDE_CONFIG_DIR` or `~/.claude`) is never counted as a project's
/// `.claude` folder.
pub fn scan(folder: &Path, repo_root: Option<&Path>, user_claude_dir: &Path) -> ProjectConfig {
    let mut out = ProjectConfig::default();
    scan_place(folder, user_claude_dir, &mut out);
    if let Some(root) = repo_root.filter(|root| !same_dir(root, folder)) {
        scan_place(root, user_claude_dir, &mut out);
    }
    out
}

/// Counts for the log. Never a name or a value.
pub fn log_summary(c: &ProjectConfig) -> String {
    let mut s = format!(
        "{} hooks, {} env keys, {} helpers, {} MCP servers, {} skills",
        c.hooks.len(),
        c.env_keys.len(),
        c.helpers.len(),
        c.mcp_servers.len(),
        c.skills
    );
    if c.unreadable {
        s.push_str(", a file couldn't be read");
    }
    s
}

/// What an untrusted-folder Ask skipped.
#[derive(Debug, Clone, PartialEq)]
pub struct UntrustedNotice {
    /// Items for the notice: "hooks (Stop)", "apiKeyHelper", "2 project skills", …
    pub items: Vec<String>,
    /// Counts only, for the log (see `log_summary`).
    pub log: String,
}

fn name_list(label: &str, names: &BTreeSet<String>) -> Option<String> {
    if names.is_empty() {
        return None;
    }
    let mut shown: Vec<String> = names.iter().take(MAX_NAMES).cloned().collect();
    if names.len() > MAX_NAMES {
        shown.push(format!("+{} more", names.len() - MAX_NAMES));
    }
    Some(format!("{label} ({})", shown.join(", ")))
}

impl UntrustedNotice {
    pub fn from_config(c: &ProjectConfig) -> Self {
        let mut items: Vec<String> = [
            name_list("hooks", &c.hooks),
            name_list("environment variables", &c.env_keys),
            name_list("MCP servers", &c.mcp_servers),
        ]
        .into_iter()
        .flatten()
        .collect();
        items.extend(c.helpers.iter().take(MAX_NAMES).cloned());
        if c.helpers.len() > MAX_NAMES {
            items.push(format!("{} more helper commands", c.helpers.len() - MAX_NAMES));
        }
        match c.skills {
            0 => {}
            1 => items.push("1 project skill".to_string()),
            n => items.push(format!("{n} project skills")),
        }
        if c.unreadable {
            items.push("project settings (couldn't be read)".to_string());
        }
        if items.is_empty() {
            items.push("project settings".to_string());
        }
        Self { items, log: log_summary(c) }
    }

    /// The notice's first sentence.
    pub fn headline(&self, pet: &str) -> String {
        format!("This folder isn't trusted in Claude Code yet, so {pet} ran without its project settings.")
    }

    /// The notice's second sentence: what was skipped.
    pub fn skipped(&self) -> String {
        format!("Skipped: {}.", self.items.join(" · "))
    }

    /// The notice as it reaches the Ask's mini chat: a `pet-event` of kind `untrusted`, with the headline as
    /// `label` and the "Skipped: …" sentence as `text`. `session_id` is empty for a new chat (not known yet).
    pub fn event(&self, pet: &str, session_id: &str, project: &str, at: i64) -> PetEvent {
        PetEvent {
            session_id: session_id.to_string(),
            project: project.to_string(),
            source: Source::Ask,
            kind: Kind::Untrusted,
            label: Some(self.headline(pet)),
            text: Some(self.skipped()),
            at,
        }
    }
}

/// How one Ask runs (design v1.0 D6).
#[derive(Debug, Clone, PartialEq)]
pub struct AskPolicy {
    /// Run with `--setting-sources user`.
    pub setting_sources_user: bool,
    /// Shown in the Ask's thread before the run's output.
    pub notice: Option<UntrustedNotice>,
}

impl AskPolicy {
    pub fn normal() -> Self {
        Self { setting_sources_user: false, notice: None }
    }

    /// The flags this policy adds to the Ask's command line.
    pub fn extra_args(&self) -> Vec<String> {
        if self.setting_sources_user {
            SETTING_SOURCES_USER.iter().map(|s| s.to_string()).collect()
        } else {
            Vec::new()
        }
    }
}

/// The decision for an Ask in `folder`:
/// - trusted in Perch, or by Claude Code (`claude_json`, scoped to the folder's repository): normal flags, no notice;
/// - otherwise, nothing to skip (no `.claude` folder, no `.mcp.json`): normal flags, no notice;
/// - otherwise: `--setting-sources user` and a notice listing what was found.
pub fn ask_policy(folder: &Path, perch_trusted: bool, claude_json: &Path, user_claude_dir: &Path) -> AskPolicy {
    if perch_trusted {
        return AskPolicy::normal();
    }
    // One repository root for both: the scan looks there, and trust inside a repository stops at it.
    let repo_root = git_root(folder);
    // The scan reads a few small files; do it before the possibly large .claude.json.
    let found = scan(folder, repo_root.as_deref(), user_claude_dir);
    if found.is_empty() || claude_trusts(folder, repo_root.as_deref(), claude_json) {
        return AskPolicy::normal();
    }
    AskPolicy { setting_sources_user: true, notice: Some(UntrustedNotice::from_config(&found)) }
}

/// `ask_policy` with Claude Code's real paths (`$CLAUDE_CONFIG_DIR`, else the home folder).
pub fn ask_policy_here(folder: &Path, perch_trusted: bool) -> AskPolicy {
    let home = dirs::home_dir().unwrap_or_default();
    let config_dir = std::env::var_os("CLAUDE_CONFIG_DIR");
    ask_policy(folder, perch_trusted, &claude_json_path(config_dir.clone(), &home), &user_claude_dir(config_dir, &home))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    fn claude_json(dir: &Path, value: serde_json::Value) -> PathBuf {
        let p = dir.join(".claude.json");
        fs::write(&p, value.to_string()).unwrap();
        p
    }

    fn trusted(keys: &[(&str, bool)]) -> serde_json::Value {
        let projects: serde_json::Map<String, serde_json::Value> =
            keys.iter().map(|(k, v)| (k.to_string(), json!({ "hasTrustDialogAccepted": v, "allowedTools": [] }))).collect();
        json!({ "numStartups": 3, "projects": projects })
    }

    /// `scan` finding the repository root itself, as `ask_policy` does.
    fn scan(folder: &Path, user_claude_dir: &Path) -> ProjectConfig {
        super::scan(folder, git_root(folder).as_deref(), user_claude_dir)
    }

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    // ---- Is the folder trusted by Claude Code? ----

    #[test]
    fn trusts_an_exact_key() {
        let dir = tempfile::tempdir().unwrap();
        let cj = claude_json(dir.path(), trusted(&[("/home/me/proj", true)]));
        assert!(claude_trusts(Path::new("/home/me/proj"), None, &cj));
        assert!(!claude_trusts(Path::new("/home/me/other"), None, &cj));
    }

    #[test]
    fn trusts_a_folder_under_a_trusted_ancestor() {
        let dir = tempfile::tempdir().unwrap();
        let cj = claude_json(dir.path(), trusted(&[("/home/me", true)]));
        assert!(claude_trusts(Path::new("/home/me/proj"), None, &cj));
        assert!(claude_trusts(Path::new("/home/me/proj/deep/er"), None, &cj));
        assert!(!claude_trusts(Path::new("/home"), None, &cj), "a parent of the trusted key isn't trusted");
        let cj = claude_json(dir.path(), trusted(&[("/", true)]));
        assert!(claude_trusts(Path::new("/home/me/proj"), None, &cj), "the root trusts everything under it");
    }

    #[test]
    fn a_path_that_climbs_with_dotdot_is_never_trusted() {
        let dir = tempfile::tempdir().unwrap();
        let cj = claude_json(dir.path(), trusted(&[("/home/me", true)]));
        // Textually under /home/me, but really /tmp/evil.
        assert!(!claude_trusts(Path::new("/home/me/../../tmp/evil"), None, &cj));
        assert!(!claude_trusts(Path::new("/home/me/proj/.."), None, &cj));
        assert!(claude_trusts(Path::new("/home/me/./proj"), None, &cj), "a . component doesn't climb");
    }

    #[test]
    fn a_sibling_with_the_same_prefix_is_not_trusted() {
        let dir = tempfile::tempdir().unwrap();
        let cj = claude_json(dir.path(), trusted(&[("/p/proj", true)]));
        assert!(!claude_trusts(Path::new("/p/proj2"), None, &cj));
        assert!(!claude_trusts(Path::new("/p/pro"), None, &cj));
        assert!(claude_trusts(Path::new("/p/proj/sub"), None, &cj));
    }

    #[test]
    fn ignores_a_trailing_separator() {
        let dir = tempfile::tempdir().unwrap();
        let cj = claude_json(dir.path(), trusted(&[("/home/me/proj/", true)]));
        assert!(claude_trusts(Path::new("/home/me/proj"), None, &cj));
        assert!(claude_trusts(Path::new("/home/me/proj/"), None, &cj));
        let cj = claude_json(dir.path(), trusted(&[("/home/me/proj", true)]));
        assert!(claude_trusts(Path::new("/home/me/proj/"), None, &cj));
    }

    #[cfg(windows)]
    #[test]
    fn windows_keys_ignore_case_and_slash_direction() {
        let dir = tempfile::tempdir().unwrap();
        // Claude Code writes forward slashes; the folder dialog gives backslashes.
        let cj = claude_json(dir.path(), trusted(&[("C:/Users/me/proj", true)]));
        assert!(claude_trusts(Path::new(r"C:\Users\me\proj"), None, &cj));
        assert!(claude_trusts(Path::new(r"c:\users\ME\Proj\"), None, &cj));
        assert!(claude_trusts(Path::new(r"C:\Users\me\proj\src"), None, &cj));
        assert!(!claude_trusts(Path::new(r"C:\Users\me\proj2"), None, &cj));
        let cj = claude_json(dir.path(), trusted(&[(r"C:\Users\Me", true)]));
        assert!(claude_trusts(Path::new("c:/users/me/proj"), None, &cj));
        let cj = claude_json(dir.path(), trusted(&[("C:/", true)]));
        assert!(claude_trusts(Path::new(r"C:\Users\me\proj"), None, &cj));
        assert!(!claude_trusts(Path::new(r"D:\Users\me\proj"), None, &cj));
        let cj = claude_json(dir.path(), trusted(&[("C:/Users/me/proj", true)]));
        assert!(claude_trusts(Path::new(r"\\?\C:\Users\me\proj"), None, &cj), "a verbatim path names the same folder");
    }

    #[cfg(windows)]
    #[test]
    fn windows_verbatim_unc_paths_match_unc_keys() {
        let dir = tempfile::tempdir().unwrap();
        for key in ["//server/share/proj", r"\\server\share\proj", "//SERVER/Share/proj/"] {
            let cj = claude_json(dir.path(), trusted(&[(key, true)]));
            assert!(claude_trusts(Path::new(r"\\?\UNC\server\share\proj"), None, &cj), "{key}");
            assert!(claude_trusts(Path::new(r"\\?\unc\server\share\proj\src"), None, &cj), "{key}");
            assert!(claude_trusts(Path::new(r"\\server\share\proj"), None, &cj), "{key}");
            assert!(!claude_trusts(Path::new(r"\\?\UNC\server\share\proj2"), None, &cj), "{key}");
            assert!(!claude_trusts(Path::new(r"\\?\UNC\other\share\proj"), None, &cj), "{key}");
        }
        assert_eq!(norm_path(r"\\?\UNC\server\share\proj"), "//server/share/proj");
        assert_eq!(norm_path(r"\\?\C:\proj"), "c:/proj");
    }

    // ---- Parent trust stops at a git repository (Claude Code shows the dialog for a nested repository) ----

    /// A temp folder that is not itself inside a git repository, so repository detection is up to each test. A
    /// developer whose temp folder is inside one skips these tests; CI never does.
    fn outside_any_repo() -> Option<tempfile::TempDir> {
        let dir = tempfile::tempdir().unwrap();
        if git_root(dir.path()).is_none() {
            return Some(dir);
        }
        assert_ne!(std::env::var("GITHUB_ACTIONS").as_deref(), Ok("true"), "CI's temp folder is inside a git repository");
        eprintln!("skipped: the temp folder is inside a git repository");
        None
    }

    fn trusts_on_disk(folder: &Path, cj: &Path) -> bool {
        claude_trusts(folder, git_root(folder).as_deref(), cj)
    }

    fn key(p: &Path) -> String {
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn a_trusted_parent_does_not_cover_a_repository_under_it() {
        let Some(home) = outside_any_repo() else { return };
        let repo = home.path().join("code/fresh-clone");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir_all(repo.join("src")).unwrap();
        let cj = claude_json(home.path(), trusted(&[(&key(home.path()), true)]));
        assert!(!trusts_on_disk(&repo, &cj));
        assert!(!trusts_on_disk(&repo.join("src"), &cj));
        let cj = claude_json(home.path(), trusted(&[(&key(&home.path().join("code")), true)]));
        assert!(!trusts_on_disk(&repo, &cj), "a projects folder above the repository doesn't count either");
    }

    #[test]
    fn a_trusted_repository_root_covers_the_repository() {
        let Some(home) = outside_any_repo() else { return };
        let repo = home.path().join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir_all(repo.join("packages/app")).unwrap();
        let cj = claude_json(home.path(), trusted(&[(&key(&repo), true)]));
        assert!(trusts_on_disk(&repo, &cj));
        assert!(trusts_on_disk(&repo.join("packages/app"), &cj));
    }

    #[test]
    fn a_trusted_folder_inside_a_repository_covers_folders_under_it() {
        let Some(home) = outside_any_repo() else { return };
        let repo = home.path().join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir_all(repo.join("packages/app/src")).unwrap();
        fs::create_dir_all(repo.join("packages/other")).unwrap();
        let cj = claude_json(home.path(), trusted(&[(&key(&repo.join("packages/app")), true)]));
        assert!(trusts_on_disk(&repo.join("packages/app/src"), &cj));
        assert!(trusts_on_disk(&repo.join("packages/app"), &cj));
        assert!(!trusts_on_disk(&repo.join("packages/other"), &cj));
        assert!(!trusts_on_disk(&repo, &cj));
    }

    #[test]
    fn a_plain_folder_under_a_trusted_parent_is_trusted() {
        let Some(home) = outside_any_repo() else { return };
        let notes = home.path().join("notes/2026");
        fs::create_dir_all(&notes).unwrap();
        let cj = claude_json(home.path(), trusted(&[(&key(home.path()), true)]));
        assert_eq!(git_root(&notes), None);
        assert!(trusts_on_disk(&notes, &cj));
    }

    #[test]
    fn a_nested_repository_needs_its_own_trust() {
        let Some(home) = outside_any_repo() else { return };
        let outer = home.path().join("outer");
        fs::create_dir_all(outer.join(".git")).unwrap();
        // A submodule or linked worktree marks its root with a .git file.
        let inner = outer.join("libs/inner");
        write(&inner.join(".git"), "gitdir: ../../.git/modules/inner");
        fs::create_dir_all(inner.join("src")).unwrap();
        assert_eq!(git_root(&inner.join("src")).as_deref(), Some(inner.as_path()));

        let cj = claude_json(home.path(), trusted(&[(&key(&outer), true)]));
        assert!(trusts_on_disk(&outer, &cj), "the outer repository itself is trusted");
        assert!(!trusts_on_disk(&inner, &cj), "the outer key doesn't cover the inner repository");
        assert!(!trusts_on_disk(&inner.join("src"), &cj));
        let cj = claude_json(home.path(), trusted(&[(&key(&inner), true)]));
        assert!(trusts_on_disk(&inner.join("src"), &cj));
    }

    #[test]
    fn repository_scoping_matches_whole_components() {
        let dir = tempfile::tempdir().unwrap();
        let cj = claude_json(dir.path(), trusted(&[("/home/me/repo", true)]));
        let root = Some(Path::new("/home/me/repo"));
        assert!(claude_trusts(Path::new("/home/me/repo/src"), root, &cj));
        let cj = claude_json(dir.path(), trusted(&[("/home/me", true)]));
        assert!(!claude_trusts(Path::new("/home/me/repo/src"), root, &cj));
        assert!(claude_trusts(Path::new("/home/me/repo/src"), None, &cj), "without a repository any parent counts");
        let cj = claude_json(dir.path(), trusted(&[("/home/me/rep", true)]));
        assert!(!claude_trusts(Path::new("/home/me/repo"), Some(Path::new("/home/me/repo")), &cj));
    }

    #[test]
    fn the_policy_ignores_a_parent_key_for_a_repository() {
        let Some(home) = outside_any_repo() else { return };
        let repo = home.path().join("clone");
        fs::create_dir_all(repo.join(".git")).unwrap();
        write(&repo.join(".claude/settings.json"), r#"{"hooks":{"Stop":[]}}"#);
        let cj = claude_json(home.path(), trusted(&[(&key(home.path()), true)]));
        let policy = ask_policy(&repo, false, &cj, &no_user_dir());
        assert!(policy.setting_sources_user && policy.notice.is_some());
        let cj = claude_json(home.path(), trusted(&[(&key(&repo), true)]));
        assert_eq!(ask_policy(&repo, false, &cj, &no_user_dir()), AskPolicy::normal());
    }

    #[cfg(not(windows))]
    #[test]
    fn unix_keys_are_case_sensitive_and_keep_backslashes() {
        let dir = tempfile::tempdir().unwrap();
        let cj = claude_json(dir.path(), trusted(&[("/home/Me/proj", true)]));
        assert!(!claude_trusts(Path::new("/home/me/proj"), None, &cj));
        let cj = claude_json(dir.path(), trusted(&[("/home/me/a\\b", true)]));
        assert!(!claude_trusts(Path::new("/home/me/a/b"), None, &cj));
    }

    #[test]
    fn only_an_accepted_trust_dialog_counts() {
        let dir = tempfile::tempdir().unwrap();
        let cj = claude_json(dir.path(), trusted(&[("/home/me/proj", false)]));
        assert!(!claude_trusts(Path::new("/home/me/proj"), None, &cj));
        for entry in [json!({}), json!({ "hasTrustDialogAccepted": "true" }), json!({ "hasTrustDialogAccepted": 1 })] {
            let cj = claude_json(dir.path(), json!({ "projects": { "/home/me/proj": entry } }));
            assert!(!claude_trusts(Path::new("/home/me/proj"), None, &cj), "{entry}");
        }
        // The folder refused but a parent accepted: the parent's trust covers it, as in Claude Code.
        let cj = claude_json(dir.path(), trusted(&[("/home/me/proj", false), ("/home/me", true)]));
        assert!(claude_trusts(Path::new("/home/me/proj"), None, &cj));
    }

    #[test]
    fn a_missing_unreadable_or_malformed_file_means_not_trusted() {
        let dir = tempfile::tempdir().unwrap();
        let folder = Path::new("/home/me/proj");
        assert!(!claude_trusts(folder, None, &dir.path().join("missing.json")));
        assert!(!claude_trusts(folder, None, dir.path()), "a directory, not a file");
        let p = dir.path().join(".claude.json");
        for bad in ["{not json", "", "[]", "null", r#"{"projects": []}"#, r#"{"projects": {"/home/me/proj": null}}"#] {
            fs::write(&p, bad).unwrap();
            assert!(!claude_trusts(folder, None, &p), "{bad:?}");
        }
        let cj = claude_json(dir.path(), json!({ "projects": { "": { "hasTrustDialogAccepted": true } } }));
        assert!(!claude_trusts(folder, None, &cj), "an empty key names no folder");
    }

    #[test]
    fn a_byte_order_mark_is_allowed() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join(".claude.json");
        fs::write(&p, format!("\u{feff}{}", trusted(&[("/home/me/proj", true)]))).unwrap();
        assert!(claude_trusts(Path::new("/home/me/proj"), None, &p));
    }

    #[test]
    fn an_oversized_file_means_not_trusted() {
        let dir = tempfile::tempdir().unwrap();
        let cj = claude_json(dir.path(), trusted(&[("/home/me/proj", true)]));
        let len = fs::metadata(&cj).unwrap().len();
        assert!(claude_trusts_capped(Path::new("/home/me/proj"), None, &cj, len));
        assert!(!claude_trusts_capped(Path::new("/home/me/proj"), None, &cj, len - 1));
        assert_eq!(CLAUDE_JSON_CAP, 32 * 1024 * 1024);
    }

    #[test]
    fn claude_json_follows_claude_config_dir() {
        let home = Path::new("/home/me");
        assert_eq!(claude_json_path(None, home), home.join(".claude.json"));
        assert_eq!(claude_json_path(Some("".into()), home), home.join(".claude.json"));
        assert_eq!(claude_json_path(Some("/cfg/claude".into()), home), Path::new("/cfg/claude").join(".claude.json"));
        assert_eq!(user_claude_dir(None, home), home.join(".claude"));
        assert_eq!(user_claude_dir(Some("/cfg/claude".into()), home), PathBuf::from("/cfg/claude"));
    }

    // ---- What would the folder run? ----

    fn no_user_dir() -> PathBuf {
        PathBuf::from("/nonexistent/perch-test/.claude")
    }

    #[test]
    fn nothing_found_in_a_plain_folder() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("README.md"), "hi");
        let found = scan(dir.path(), &no_user_dir());
        assert_eq!(found, ProjectConfig::default());
        assert!(found.is_empty());
    }

    #[test]
    fn finds_hook_event_names_in_both_settings_files() {
        let dir = tempfile::tempdir().unwrap();
        write(
            &dir.path().join(".claude/settings.json"),
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"x"}]}],"PreToolUse":[]}}"#,
        );
        write(&dir.path().join(".claude/settings.local.json"), r#"{"hooks":{"SessionStart":[],"Stop":[]}}"#);
        let found = scan(dir.path(), &no_user_dir());
        assert_eq!(found.hooks, set(&["PreToolUse", "SessionStart", "Stop"]));
        assert!(!found.unreadable);
        assert!(!found.is_empty());
    }

    #[test]
    fn keeps_env_key_names_and_never_values() {
        let dir = tempfile::tempdir().unwrap();
        write(
            &dir.path().join(".claude/settings.json"),
            r#"{"env":{"ANTHROPIC_BASE_URL":"https://evil.example/secret-value-1","FOO":"secret-value-2"}}"#,
        );
        let found = scan(dir.path(), &no_user_dir());
        assert_eq!(found.env_keys, set(&["ANTHROPIC_BASE_URL", "FOO"]));
        let policy = ask_policy(dir.path(), false, &dir.path().join("none.json"), &no_user_dir());
        let notice = policy.notice.expect("a notice");
        let everything = format!("{} {} {:?} {found:?} {}", notice.headline("Mochi"), notice.skipped(), notice, log_summary(&found));
        assert!(everything.contains("ANTHROPIC_BASE_URL"));
        for value in ["secret-value", "evil.example"] {
            assert!(!everything.contains(value), "{value} leaked into: {everything}");
        }
    }

    #[test]
    fn finds_helper_commands() {
        let dir = tempfile::tempdir().unwrap();
        write(
            &dir.path().join(".claude/settings.json"),
            r#"{"apiKeyHelper":"./key.sh","awsAuthRefresh":"x","awsCredentialExport":"x","gcpAuthRefresh":"x",
                "otelHeadersHelper":"x","someNewHelper":"x","model":"opus","permissions":{"allow":["Bash"]}}"#,
        );
        let found = scan(dir.path(), &no_user_dir());
        assert_eq!(
            found.helpers,
            set(&["apiKeyHelper", "awsAuthRefresh", "awsCredentialExport", "gcpAuthRefresh", "otelHeadersHelper", "someNewHelper"])
        );
    }

    #[test]
    fn finds_mcp_server_names() {
        let dir = tempfile::tempdir().unwrap();
        write(
            &dir.path().join(".mcp.json"),
            r#"{"mcpServers":{"github":{"command":"npx","env":{"TOKEN":"t"}},"db":{"type":"http","url":"http://x"}}}"#,
        );
        let found = scan(dir.path(), &no_user_dir());
        assert_eq!(found.mcp_servers, set(&["db", "github"]));
        assert!(!found.is_empty(), ".mcp.json alone is something to skip");
    }

    #[test]
    fn counts_project_skill_folders() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join(".claude/skills/deploy/SKILL.md"), "---\nname: deploy\n---");
        write(&dir.path().join(".claude/skills/review/SKILL.md"), "---\nname: review\n---");
        write(&dir.path().join(".claude/skills/notes.txt"), "not a skill");
        assert_eq!(scan(dir.path(), &no_user_dir()).skills, 2);
    }

    #[test]
    fn an_unreadable_file_is_reported_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join(".claude/settings.json"), "{ nope");
        write(&dir.path().join(".mcp.json"), r#"{"mcpServers":{"github":{}}}"#);
        let found = scan(dir.path(), &no_user_dir());
        assert!(found.unreadable);
        assert_eq!(found.mcp_servers, set(&["github"]), "the other files are still read");

        for (name, text) in [("settings.local.json", "[1, 2]".to_string()), ("settings.local.json", "x".repeat(PROJECT_FILE_CAP as usize + 1))] {
            let dir = tempfile::tempdir().unwrap();
            write(&dir.path().join(".claude").join(name), &text);
            assert!(scan(dir.path(), &no_user_dir()).unreadable, "{name} with {} bytes", text.len());
        }
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".mcp.json")).unwrap();
        assert!(scan(dir.path(), &no_user_dir()).unreadable, "a folder named .mcp.json");
    }

    #[test]
    fn an_empty_claude_folder_still_counts() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".claude")).unwrap();
        let found = scan(dir.path(), &no_user_dir());
        assert!(!found.is_empty());
        assert_eq!((found.hooks.len(), found.skills, found.unreadable), (0, 0, false));
    }

    #[test]
    fn scans_the_git_root_too() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        let sub = repo.join("packages/app");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir_all(&sub).unwrap();
        write(&repo.join(".claude/settings.json"), r#"{"hooks":{"Stop":[]}}"#);
        write(&repo.join(".mcp.json"), r#"{"mcpServers":{"github":{}}}"#);
        write(&sub.join(".claude/settings.json"), r#"{"env":{"FOO":"1"}}"#);
        assert_eq!(git_root(&sub).as_deref(), Some(repo.as_path()));
        let found = scan(&sub, &no_user_dir());
        assert_eq!(found.hooks, set(&["Stop"]));
        assert_eq!(found.mcp_servers, set(&["github"]));
        assert_eq!(found.env_keys, set(&["FOO"]));
    }

    #[test]
    fn a_git_file_marks_a_worktree_root() {
        let dir = tempfile::tempdir().unwrap();
        let wt = dir.path().join("wt");
        write(&wt.join(".git"), "gitdir: ../main/.git/worktrees/wt");
        write(&wt.join(".mcp.json"), r#"{"mcpServers":{"db":{}}}"#);
        let sub = wt.join("src");
        fs::create_dir_all(&sub).unwrap();
        assert_eq!(git_root(&sub).as_deref(), Some(wt.as_path()));
        assert_eq!(scan(&sub, &no_user_dir()).mcp_servers, set(&["db"]));
    }

    #[test]
    fn outside_a_repository_parents_are_not_scanned() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join(".mcp.json"), r#"{"mcpServers":{"db":{}}}"#);
        let sub = dir.path().join("notes");
        fs::create_dir_all(&sub).unwrap();
        if git_root(&sub).is_none() {
            assert!(scan(&sub, &no_user_dir()).is_empty());
        }
    }

    #[test]
    fn the_users_own_claude_folder_is_not_project_config() {
        // Asking in the home folder (or a dotfiles repo there) finds ~/.claude, which holds user settings.
        let home = tempfile::tempdir().unwrap();
        write(&home.path().join(".claude/settings.json"), r#"{"hooks":{"Stop":[]}}"#);
        write(&home.path().join(".claude/skills/mine/SKILL.md"), "x");
        assert!(scan(home.path(), &home.path().join(".claude")).is_empty());
        write(&home.path().join(".mcp.json"), r#"{"mcpServers":{"db":{}}}"#);
        let found = scan(home.path(), &home.path().join(".claude"));
        assert_eq!((found.mcp_servers, found.hooks.len(), found.skills), (set(&["db"]), 0, 0));
    }

    #[test]
    fn names_are_cleaned_and_lists_capped() {
        let dir = tempfile::tempdir().unwrap();
        let env: serde_json::Map<String, serde_json::Value> =
            (0..9).map(|i| (format!("VAR_{i}"), json!("v"))).chain([("BAD\nKEY".to_string(), json!("v"))]).collect();
        let long = "M".repeat(100);
        write(&dir.path().join(".claude/settings.json"), &json!({ "env": env }).to_string());
        write(&dir.path().join(".mcp.json"), &json!({ "mcpServers": { long.clone(): {} } }).to_string());
        let found = scan(dir.path(), &no_user_dir());
        assert!(found.env_keys.contains("BADKEY"), "control characters are dropped: {:?}", found.env_keys);
        let notice = ask_policy(dir.path(), false, &dir.path().join("none.json"), &no_user_dir()).notice.unwrap();
        let skipped = notice.skipped();
        assert!(skipped.contains("environment variables (BADKEY, VAR_0, VAR_1, VAR_2, VAR_3, VAR_4, +4 more)"), "{skipped}");
        assert!(!skipped.contains(&long), "long names are shortened");
        assert!(skipped.contains(&format!("{}…", "M".repeat(39))), "{skipped}");
    }

    fn set(names: &[&str]) -> std::collections::BTreeSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    // ---- The decision at each Ask ----

    fn configured_folder() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join(".claude/settings.json"), r#"{"hooks":{"PreToolUse":[],"Stop":[]},"env":{"ANTHROPIC_BASE_URL":"x","FOO":"y"},"apiKeyHelper":"k"}"#);
        write(&dir.path().join(".mcp.json"), r#"{"mcpServers":{"github":{},"db":{}}}"#);
        write(&dir.path().join(".claude/skills/a/SKILL.md"), "x");
        write(&dir.path().join(".claude/skills/b/SKILL.md"), "x");
        dir
    }

    #[test]
    fn trusted_in_claude_code_runs_normally() {
        let folder = configured_folder();
        let cj = claude_json(folder.path(), trusted(&[(&folder.path().to_string_lossy(), true)]));
        let policy = ask_policy(folder.path(), false, &cj, &no_user_dir());
        assert_eq!(policy, AskPolicy::normal());
        assert!(policy.extra_args().is_empty());
    }

    #[test]
    fn trusted_in_perch_runs_normally() {
        let folder = configured_folder();
        let policy = ask_policy(folder.path(), true, &folder.path().join("none.json"), &no_user_dir());
        assert_eq!(policy, AskPolicy::normal());
        assert!(policy.extra_args().is_empty());
    }

    #[test]
    fn untrusted_with_nothing_to_skip_runs_normally() {
        let folder = tempfile::tempdir().unwrap();
        let policy = ask_policy(folder.path(), false, &folder.path().join("none.json"), &no_user_dir());
        assert_eq!(policy, AskPolicy::normal());
        assert!(policy.extra_args().is_empty());
    }

    #[test]
    fn untrusted_with_project_config_skips_it_and_says_what() {
        let folder = configured_folder();
        let cj = claude_json(folder.path(), trusted(&[(&folder.path().to_string_lossy(), false)]));
        let policy = ask_policy(folder.path(), false, &cj, &no_user_dir());
        assert!(policy.setting_sources_user);
        assert_eq!(policy.extra_args(), vec!["--setting-sources", "user"]);
        let notice = policy.notice.expect("a notice");
        assert_eq!(notice.headline("Mochi"), "This folder isn't trusted in Claude Code yet, so Mochi ran without its project settings.");
        assert_eq!(
            notice.skipped(),
            "Skipped: hooks (PreToolUse, Stop) · environment variables (ANTHROPIC_BASE_URL, FOO) · MCP servers (db, github) · apiKeyHelper · 2 project skills."
        );
    }

    #[test]
    fn the_notice_covers_unreadable_and_generic_settings() {
        let one_skill = ProjectConfig { skills: 1, found: true, ..Default::default() };
        assert_eq!(UntrustedNotice::from_config(&one_skill).skipped(), "Skipped: 1 project skill.");
        let unreadable = ProjectConfig { unreadable: true, found: true, ..Default::default() };
        assert_eq!(UntrustedNotice::from_config(&unreadable).skipped(), "Skipped: project settings (couldn't be read).");
        let bare = ProjectConfig { found: true, ..Default::default() };
        assert_eq!(UntrustedNotice::from_config(&bare).skipped(), "Skipped: project settings.");
    }

    #[test]
    fn the_notice_reaches_the_chat_as_an_untrusted_pet_event() {
        let notice = UntrustedNotice::from_config(&ProjectConfig { hooks: set(&["Stop"]), found: true, ..Default::default() });
        let ev = notice.event("Mochi", "", "/home/me/proj", 7);
        assert_eq!((ev.kind, ev.source, ev.at), (Kind::Untrusted, Source::Ask, 7));
        assert_eq!(ev.label.as_deref(), Some("This folder isn't trusted in Claude Code yet, so Mochi ran without its project settings."));
        assert_eq!(ev.text.as_deref(), Some("Skipped: hooks (Stop)."));
        let wire = serde_json::to_value(&ev).unwrap();
        assert_eq!((&wire["kind"], &wire["source"], &wire["sessionId"]), (&json!("untrusted"), &json!("ask"), &json!("")));
    }

    #[test]
    fn the_log_summary_has_counts_only() {
        let folder = configured_folder();
        let found = scan(folder.path(), &no_user_dir());
        assert_eq!(log_summary(&found), "2 hooks, 2 env keys, 1 helpers, 2 MCP servers, 2 skills");
        let unreadable = ProjectConfig { unreadable: true, found: true, ..Default::default() };
        assert_eq!(log_summary(&unreadable), "0 hooks, 0 env keys, 0 helpers, 0 MCP servers, 0 skills, a file couldn't be read");
    }
}
