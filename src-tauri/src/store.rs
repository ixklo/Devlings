use std::{fs, io, path::Path, str::FromStr};

use log::LevelFilter;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub const DEFAULT_PET_NAME: &str = "Perch";
pub const DEFAULT_PET_ID: &str = "perch";
pub const DEFAULT_PET_SCALE: f64 = 0.6;
pub const MIN_PET_SCALE: f64 = 0.4;
pub const MAX_PET_SCALE: f64 = 1.0;
pub const DEFAULT_DIAGNOSTICS_LEVEL: &str = "info";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub pet_name: String,
    pub onboarded: bool,
    pub credits_notice_seen: bool,
    pub hooks_declined: bool,
    pub hook_port: Option<u16>,
    pub hook_token: Option<String>,
    pub claude_path: Option<String>,
    pub pet_position: Option<(i32, i32)>,
    /// Whether `pet_position` is already in the sprite-anchor format (design D9). False for a
    /// v0.2 config file, so `overlay::place_pet` migrates it once, then sets this and re-saves.
    pub pet_position_migrated: bool,
    pub notifications: bool,
    pub launch_at_login: bool,
    pub pet_id: String,
    pub pet_scale: f64,
    pub threads_collapsed: bool,
    /// Log level: "off", "error", "warn", "info", "debug" or "trace".
    pub diagnostics_level: String,
    /// Check for updates at launch and daily (M1).
    pub auto_update: bool,
    /// Epoch ms of the last update check.
    pub last_update_check: Option<i64>,
}

impl Config {
    /// Repairs values a hand-edited or older config file may carry.
    pub fn normalized(mut self) -> Self {
        self.pet_scale = clamp_pet_scale(self.pet_scale);
        if self.pet_id.trim().is_empty() {
            self.pet_id = DEFAULT_PET_ID.to_string();
        }
        self.diagnostics_level = normalize_diagnostics_level(&self.diagnostics_level);
        self
    }
}

/// The log level a `diagnosticsLevel` value names. The one place levels are parsed; unknown values read as info.
pub fn diagnostics_level_filter(level: &str) -> LevelFilter {
    LevelFilter::from_str(level.trim()).unwrap_or(LevelFilter::Info)
}

/// The canonical lower-case spelling of a `diagnosticsLevel` value.
pub fn normalize_diagnostics_level(level: &str) -> String {
    diagnostics_level_filter(level).as_str().to_ascii_lowercase()
}

pub fn clamp_pet_scale(scale: f64) -> f64 {
    if scale.is_finite() { scale.clamp(MIN_PET_SCALE, MAX_PET_SCALE) } else { DEFAULT_PET_SCALE }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            pet_name: DEFAULT_PET_NAME.to_string(),
            onboarded: false,
            credits_notice_seen: false,
            hooks_declined: false,
            hook_port: None,
            hook_token: None,
            claude_path: None,
            pet_position: None,
            pet_position_migrated: false,
            notifications: true,
            launch_at_login: false,
            pet_id: DEFAULT_PET_ID.to_string(),
            pet_scale: DEFAULT_PET_SCALE,
            threads_collapsed: false,
            diagnostics_level: DEFAULT_DIAGNOSTICS_LEVEL.to_string(),
            auto_update: true,
            last_update_check: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    ReadOnly,
    #[default]
    EditFiles,
    Auto,
}

impl PermissionMode {
    pub fn flag(self) -> &'static str {
        match self {
            PermissionMode::ReadOnly => "dontAsk",
            PermissionMode::EditFiles => "acceptEdits",
            PermissionMode::Auto => "auto",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEntry {
    pub path: String,
    pub name: String,
    pub last_seen: i64,
    #[serde(default)]
    pub permission_mode: PermissionMode,
    #[serde(default)]
    pub ask_session_id: Option<String>,
    #[serde(default)]
    pub transcript_path: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Projects {
    pub list: Vec<ProjectEntry>,
}

impl Projects {
    pub fn touch(&mut self, path: &str, now: i64) -> bool {
        if let Some(p) = self.get_mut(path) {
            p.last_seen = p.last_seen.max(now);
            return false;
        }
        self.list.push(ProjectEntry {
            path: path.trim_end_matches(['/', '\\']).to_string(),
            name: project_name(path),
            last_seen: now,
            permission_mode: PermissionMode::default(),
            ask_session_id: None,
            transcript_path: None,
        });
        true
    }

    pub fn get(&self, path: &str) -> Option<&ProjectEntry> {
        self.list.iter().find(|p| same_path(&p.path, path))
    }

    pub fn get_mut(&mut self, path: &str) -> Option<&mut ProjectEntry> {
        self.list.iter_mut().find(|p| same_path(&p.path, path))
    }

    /// Records an Ask conversation's transcript. Returns whether it changed, so callers save only then.
    pub fn set_transcript_path(&mut self, project: &str, transcript: &str) -> bool {
        match self.get_mut(project) {
            Some(p) if p.transcript_path.as_deref() != Some(transcript) => {
                p.transcript_path = Some(transcript.to_string());
                true
            }
            _ => false,
        }
    }

    pub fn retain_outside(&mut self, base: &str) {
        self.list.retain(|p| !is_under(&p.path, base));
    }

    pub fn remove(&mut self, path: &str) {
        self.list.retain(|p| !same_path(&p.path, path));
    }

    pub fn sorted(&self) -> Vec<ProjectEntry> {
        let mut v = self.list.clone();
        v.sort_by_key(|a| std::cmp::Reverse(a.last_seen));
        v
    }
}

pub fn same_path(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.trim_end_matches(['/', '\\']).replace('\\', "/");
    if cfg!(windows) {
        norm(a).eq_ignore_ascii_case(&norm(b))
    } else {
        norm(a) == norm(b)
    }
}

pub fn is_under(path: &str, base: &str) -> bool {
    let norm = |s: &str| {
        let n = s.trim_end_matches(['/', '\\']).replace('\\', "/");
        if cfg!(windows) { n.to_ascii_lowercase() } else { n }
    };
    let (p, b) = (norm(path), norm(base));
    !b.is_empty() && (p == b || p.starts_with(&format!("{b}/")))
}

pub fn project_name(path: &str) -> String {
    let trimmed = path.trim_end_matches(['/', '\\']);
    trimmed.rsplit(['/', '\\']).next().unwrap_or(trimmed).to_string()
}

pub fn validate_pet_name(raw: &str) -> Result<String, String> {
    let name = raw.trim();
    match name.chars().count() {
        0 => Err("Give your pet a name.".to_string()),
        1..=24 => Ok(name.to_string()),
        _ => Err("Pet names can be up to 24 characters.".to_string()),
    }
}

pub fn load<T: DeserializeOwned + Default>(path: &Path) -> T {
    fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(t.trim_start_matches('\u{feff}')).ok())
        .unwrap_or_default()
}

pub fn save<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, serde_json::to_string_pretty(value).map_err(io::Error::other)?)?;
    fs::rename(tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn config_defaults_and_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        let mut c: Config = load(&p);
        assert_eq!(c.pet_name, "Perch");
        assert!(c.notifications);
        assert!(!c.onboarded);
        c.pet_name = "Mochi".into();
        c.pet_position = Some((10, -20));
        save(&p, &c).unwrap();
        assert_eq!(load::<Config>(&p), c);
    }

    #[test]
    fn v02_config_fields() {
        let c = Config::default();
        assert_eq!((c.pet_id.as_str(), c.pet_scale, c.threads_collapsed), ("perch", 0.6, false));
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!((&v["petId"], &v["petScale"], &v["threadsCollapsed"]), (&json!("perch"), &json!(0.6), &json!(false)));
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        std::fs::write(&p, r#"{"petName":"Bo","petScale":3.0,"petId":" "}"#).unwrap();
        let c = load::<Config>(&p).normalized();
        assert_eq!((c.pet_scale, c.pet_id.as_str(), c.pet_name.as_str()), (1.0, "perch", "Bo"));
    }

    #[test]
    fn diagnostics_level_defaults_and_repairs() {
        let c = Config::default();
        assert_eq!(c.diagnostics_level, "info");
        assert_eq!(serde_json::to_value(&c).unwrap()["diagnosticsLevel"], json!("info"));
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        std::fs::write(&p, r#"{"diagnosticsLevel":" DEBUG "}"#).unwrap();
        assert_eq!(load::<Config>(&p).normalized().diagnostics_level, "debug");
        std::fs::write(&p, r#"{"diagnosticsLevel":"loud"}"#).unwrap();
        assert_eq!(load::<Config>(&p).normalized().diagnostics_level, "info");
        std::fs::write(&p, r#"{"petName":"Bo"}"#).unwrap();
        assert_eq!(load::<Config>(&p).normalized().diagnostics_level, "info");
        for level in ["off", "error", "warn", "info", "debug", "trace"] {
            assert_eq!(normalize_diagnostics_level(level), level);
        }
    }

    #[test]
    fn diagnostics_levels_parse_in_one_place() {
        assert_eq!(diagnostics_level_filter("info"), LevelFilter::Info);
        assert_eq!(diagnostics_level_filter(" DEBUG "), LevelFilter::Debug);
        assert_eq!(diagnostics_level_filter("trace"), LevelFilter::Trace);
        assert_eq!(diagnostics_level_filter("Warn"), LevelFilter::Warn);
        assert_eq!(diagnostics_level_filter("error"), LevelFilter::Error);
        assert_eq!(diagnostics_level_filter("off"), LevelFilter::Off);
        assert_eq!(diagnostics_level_filter("chatty"), LevelFilter::Info);
        assert_eq!(diagnostics_level_filter(""), LevelFilter::Info);
    }

    #[test]
    fn update_config_keys_default_for_old_files() {
        let c = Config::default();
        assert_eq!((c.auto_update, c.last_update_check), (true, None));
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!((&v["autoUpdate"], &v["lastUpdateCheck"]), (&json!(true), &json!(null)));
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        // A v0.2 config file has neither key.
        std::fs::write(&p, r#"{"petName":"Bo","petId":"ember","threadsCollapsed":true}"#).unwrap();
        let c = load::<Config>(&p);
        assert_eq!((c.auto_update, c.last_update_check, c.pet_name.as_str()), (true, None, "Bo"));
        std::fs::write(&p, r#"{"autoUpdate":false,"lastUpdateCheck":1700000000000}"#).unwrap();
        let c = load::<Config>(&p);
        assert_eq!((c.auto_update, c.last_update_check), (false, Some(1_700_000_000_000)));
    }

    #[test]
    fn pet_scale_is_clamped() {
        assert_eq!(clamp_pet_scale(0.1), 0.4);
        assert_eq!(clamp_pet_scale(0.45), 0.45);
        assert_eq!(clamp_pet_scale(2.0), 1.0);
        assert_eq!(clamp_pet_scale(f64::NAN), 0.6);
    }

    #[test]
    fn loads_json_with_byte_order_mark() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        std::fs::write(&p, "\u{feff}{\"petName\":\"Bo\"}").unwrap();
        assert_eq!(load::<Config>(&p).pet_name, "Bo");
    }

    #[test]
    fn bad_or_partial_json() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        std::fs::write(&p, "{nope").unwrap();
        assert_eq!(load::<Config>(&p), Config::default());
        std::fs::write(&p, r#"{"petName":"Bo"}"#).unwrap();
        let c: Config = load(&p);
        assert_eq!((c.pet_name.as_str(), c.notifications), ("Bo", true));
    }

    #[test]
    fn pet_names() {
        assert_eq!(validate_pet_name("  Mochi "), Ok("Mochi".to_string()));
        assert!(validate_pet_name("").is_err());
        assert!(validate_pet_name("   ").is_err());
        assert!(validate_pet_name(&"x".repeat(25)).is_err());
        assert_eq!(validate_pet_name(&"x".repeat(24)), Ok("x".repeat(24)));
        assert_eq!(validate_pet_name("Ünï"), Ok("Ünï".to_string()));
    }

    #[test]
    fn detects_paths_inside_a_base_folder() {
        assert!(is_under("/tmp/claude/x/tc", "/tmp"));
        assert!(is_under("/tmp", "/tmp/"));
        assert!(!is_under("/tmpfoo/x", "/tmp"));
        assert!(!is_under("/home/u/proj", "/tmp"));
    }

    #[cfg(windows)]
    #[test]
    fn detects_windows_temp_paths_ignoring_case() {
        assert!(is_under("C:\\Users\\Me\\AppData\\Local\\Temp\\claude\\tc", "c:\\users\\me\\appdata\\local\\temp\\"));
    }

    #[test]
    fn transcript_path_changes_are_reported() {
        let mut p = Projects::default();
        assert!(!p.set_transcript_path("/home/u/proj", "/t/a.jsonl"), "unknown project");
        p.touch("/home/u/proj", 1);
        assert!(p.set_transcript_path("/home/u/proj", "/t/a.jsonl"));
        assert!(!p.set_transcript_path("/home/u/proj/", "/t/a.jsonl"));
        assert!(p.set_transcript_path("/home/u/proj", "/t/b.jsonl"));
        assert_eq!(p.get("/home/u/proj").unwrap().transcript_path.as_deref(), Some("/t/b.jsonl"));
    }

    #[test]
    fn drops_temp_projects() {
        let mut p = Projects::default();
        p.touch("/tmp/scratch/tc", 1);
        p.touch("/home/u/proj", 2);
        p.retain_outside("/tmp");
        assert_eq!(p.sorted().len(), 1);
        assert_eq!(p.sorted()[0].name, "proj");
    }

    #[test]
    fn projects_touch_and_sort() {
        let mut p = Projects::default();
        assert!(p.touch("C:\\a\\proj", 1));
        assert!(!p.touch("C:\\a\\proj\\", 5));
        assert!(p.touch("C:\\b\\other", 3));
        let sorted = p.sorted();
        assert_eq!(sorted.len(), 2);
        assert_eq!((sorted[0].name.as_str(), sorted[0].last_seen), ("proj", 5));
        assert_eq!(sorted[1].name, "other");
        p.remove("C:\\b\\other");
        assert!(p.get("C:\\b\\other").is_none());
    }

    #[cfg(windows)]
    #[test]
    fn windows_paths_ignore_case_and_slashes() {
        assert!(same_path("C:\\A\\Proj", "c:/a/proj/"));
    }

    #[test]
    fn permission_mode() {
        assert_eq!(PermissionMode::default(), PermissionMode::EditFiles);
        assert_eq!(PermissionMode::ReadOnly.flag(), "dontAsk");
        assert_eq!(PermissionMode::EditFiles.flag(), "acceptEdits");
        assert_eq!(PermissionMode::Auto.flag(), "auto");
        assert_eq!(serde_json::to_string(&PermissionMode::ReadOnly).unwrap(), "\"read_only\"");
    }
}
