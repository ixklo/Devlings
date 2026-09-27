use std::{fs, io, path::Path, str::FromStr};

use log::LevelFilter;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub const DEFAULT_PET_NAME: &str = "Perch";
pub const DEFAULT_PET_ID: &str = "perch";
pub const DEFAULT_PET_SCALE: f64 = 0.6;
pub const MIN_PET_SCALE: f64 = 0.4;
pub const MAX_PET_SCALE: f64 = 1.0;
pub const DEFAULT_DIAGNOSTICS_LEVEL: &str = "info";
/// How long a watched permission request can be held for an answer in Devlings; the UI offers exactly these.
pub const APPROVAL_HOLDS: [u64; 4] = [30, 60, 120, 240];
pub const DEFAULT_APPROVAL_HOLD_SECS: u64 = 60;

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
    /// Answer permission requests of watched sessions from the pet (design v1.0 D3). Off by default.
    pub watch_approvals: bool,
    /// How long a watched request is held for an answer in Devlings: one of `APPROVAL_HOLDS`.
    pub approval_hold_secs: u64,
    /// The one-time "answer permission prompts" intro card was answered.
    pub approvals_intro_seen: bool,
}

impl Config {
    /// Repairs values a hand-edited or older config file may carry.
    pub fn normalized(mut self) -> Self {
        self.pet_scale = clamp_pet_scale(self.pet_scale);
        if self.pet_id.trim().is_empty() {
            self.pet_id = DEFAULT_PET_ID.to_string();
        }
        self.diagnostics_level = normalize_diagnostics_level(&self.diagnostics_level);
        self.approval_hold_secs = validate_approval_hold(self.approval_hold_secs).unwrap_or(DEFAULT_APPROVAL_HOLD_SECS);
        self
    }
}

impl Config {
    /// Turning watching on also answers the one-time intro card.
    pub fn set_watch_approvals(&mut self, enabled: bool) {
        self.watch_approvals = enabled;
        if enabled {
            self.approvals_intro_seen = true;
        }
    }

    /// Onboarding done. It explains permission prompts, so the one-time upgrade intro is answered too.
    pub fn finish_onboarding(&mut self) {
        self.onboarded = true;
        self.approvals_intro_seen = true;
    }
}

/// A hold the UI offers, or an error for anything else.
pub fn validate_approval_hold(secs: u64) -> Result<u64, String> {
    if APPROVAL_HOLDS.contains(&secs) {
        Ok(secs)
    } else {
        Err("Choose 30 seconds, 1 minute, 2 minutes or 4 minutes.".to_string())
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
            watch_approvals: false,
            approval_hold_secs: DEFAULT_APPROVAL_HOLD_SECS,
            approvals_intro_seen: false,
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
    /// Trusted in Devlings: Asks here use the folder's own Claude Code settings even if Claude Code hasn't trusted it
    /// (design v1.0 D6).
    #[serde(default)]
    pub trusted: bool,
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
            trusted: false,
        });
        true
    }

    /// Sets Devlings-level trust for a known project. Returns whether it changed, so callers save only then.
    pub fn set_trusted(&mut self, path: &str, trusted: bool) -> Result<bool, String> {
        let p = self.get_mut(path).ok_or("Unknown project.")?;
        let changed = p.trusted != trusted;
        p.trusted = trusted;
        Ok(changed)
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

/// The name to switch to when the user picks another pet, or `None` to keep the current one.
/// A name the user chose stays. A name that is still a default (the old pet's own name, or the
/// "Perch" every install starts with) becomes the new pet's name.
pub fn name_after_pet_change(current: &str, old_pet_name: Option<&str>, new_pet_name: &str) -> Option<String> {
    let is_default = current == DEFAULT_PET_NAME || old_pet_name == Some(current);
    let next = validate_pet_name(new_pet_name).ok()?;
    (is_default && next != current).then_some(next)
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
    fn approval_config_keys_default_for_old_files() {
        let c = Config::default();
        assert_eq!((c.watch_approvals, c.approval_hold_secs, c.approvals_intro_seen), (false, 60, false));
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!((&v["watchApprovals"], &v["approvalHoldSecs"], &v["approvalsIntroSeen"]), (&json!(false), &json!(60), &json!(false)));
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        // A v0.2 config file has none of them.
        std::fs::write(&p, r#"{"petName":"Bo","onboarded":true,"hookPort":4545}"#).unwrap();
        let c = load::<Config>(&p).normalized();
        assert_eq!((c.watch_approvals, c.approval_hold_secs, c.approvals_intro_seen, c.onboarded), (false, 60, false, true));
        std::fs::write(&p, r#"{"watchApprovals":true,"approvalHoldSecs":240,"approvalsIntroSeen":true}"#).unwrap();
        let c = load::<Config>(&p).normalized();
        assert_eq!((c.watch_approvals, c.approval_hold_secs, c.approvals_intro_seen), (true, 240, true));
        // A hold the UI doesn't offer reads as the default.
        for odd in ["15", "0", "999", "-5", "\"60\""] {
            std::fs::write(&p, format!(r#"{{"petName":"Bo","approvalHoldSecs":{odd}}}"#)).unwrap();
            let c = load::<Config>(&p).normalized();
            assert_eq!(c.approval_hold_secs, 60, "{odd}");
        }
    }

    #[test]
    fn turning_watch_approvals_on_answers_the_intro() {
        let mut c = Config::default();
        c.set_watch_approvals(false);
        assert_eq!((c.watch_approvals, c.approvals_intro_seen), (false, false));
        c.set_watch_approvals(true);
        assert_eq!((c.watch_approvals, c.approvals_intro_seen), (true, true));
        c.set_watch_approvals(false);
        assert_eq!((c.watch_approvals, c.approvals_intro_seen), (false, true));
    }

    #[test]
    fn finishing_onboarding_also_answers_the_approvals_intro() {
        let mut c = Config::default();
        assert_eq!((c.onboarded, c.approvals_intro_seen, c.watch_approvals), (false, false, false));
        c.finish_onboarding();
        // Onboarding explains permission prompts (step 3), so the upgrade intro card never shows,
        // and finishing leaves the switch as the user set it.
        assert_eq!((c.onboarded, c.approvals_intro_seen, c.watch_approvals), (true, true, false));
    }

    #[test]
    fn approval_holds() {
        for secs in [30, 60, 120, 240] {
            assert_eq!(validate_approval_hold(secs), Ok(secs));
        }
        for secs in [0, 15, 59, 300, 241] {
            assert!(validate_approval_hold(secs).is_err(), "{secs}");
        }
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
    fn a_default_name_follows_the_pet_and_a_chosen_one_stays() {
        // Still the old pet's own name: the new pet's name replaces it.
        assert_eq!(name_after_pet_change("Ember", Some("Ember"), "Pip"), Some("Pip".to_string()));
        // Every install starts out as "Perch", whichever pet was picked.
        assert_eq!(name_after_pet_change("Perch", Some("Plum"), "Miso"), Some("Miso".to_string()));
        assert_eq!(name_after_pet_change("Perch", None, "Miso"), Some("Miso".to_string()));
        // A name the user chose stays, even one that matches another pet.
        assert_eq!(name_after_pet_change("Mochi", Some("Perch"), "Pip"), None);
        assert_eq!(name_after_pet_change("Plum", Some("Pip"), "Wisp"), None);
        // Nothing to change, or a display name that isn't a valid pet name.
        assert_eq!(name_after_pet_change("Pip", Some("Pip"), "Pip"), None);
        assert_eq!(name_after_pet_change("Pip", Some("Pip"), &"x".repeat(25)), None);
        assert_eq!(name_after_pet_change("Pip", Some("Pip"), " Bolt "), Some("Bolt".to_string()));
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
    fn projects_are_untrusted_until_trusted_in_perch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("projects.json");
        // A v0.2 projects.json has no `trusted` key.
        std::fs::write(&path, r#"{"list":[{"path":"/home/u/app","name":"app","lastSeen":1}]}"#).unwrap();
        let mut p: Projects = load(&path);
        assert!(!p.get("/home/u/app").unwrap().trusted);
        assert!(p.touch("/home/u/new", 2));
        assert!(!p.get("/home/u/new").unwrap().trusted);

        assert_eq!(p.set_trusted("/home/u/app/", true), Ok(true));
        assert_eq!(p.set_trusted("/home/u/app", true), Ok(false), "already trusted");
        assert_eq!(p.set_trusted("/nowhere", true), Err("Unknown project.".to_string()));
        save(&path, &p).unwrap();
        let reloaded: Projects = load(&path);
        assert!(reloaded.get("/home/u/app").unwrap().trusted);
        assert!(!reloaded.get("/home/u/new").unwrap().trusted);
        assert_eq!(serde_json::to_value(&reloaded.list[0]).unwrap()["trusted"], json!(true));

        let mut p = reloaded;
        assert_eq!(p.set_trusted("/home/u/app", false), Ok(true));
        assert!(!p.get("/home/u/app").unwrap().trusted);
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
