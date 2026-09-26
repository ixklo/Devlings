use std::{
    collections::{HashMap, HashSet},
    io::{BufReader, Read},
    path::{Path, PathBuf},
    process::Child,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::{
    events::{Kind, Mood, PetEvent, Source},
    hook_server::HookServer,
    hooks_installer,
    locator::{self, Located},
    money_guard::AuthVerdict,
    normalize::{self, StreamItem},
    runner::{self, KillReason},
    sessions::{SessionInfo, Sessions},
    shell,
    store::{self, Config, ProjectEntry, Projects},
};

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

pub struct AppState {
    pub data_dir: PathBuf,
    pub config: Mutex<Config>,
    pub projects: Mutex<Projects>,
    pub sessions: Mutex<Sessions>,
    pub claude: Mutex<Result<Located, String>>,
    pub auth: Mutex<Option<AuthVerdict>>,
    pub hooks_installed: AtomicBool,
    pub hook_server: Mutex<Option<HookServer>>,
    pub hook_server_error: Mutex<Option<String>>,
    pub running: Mutex<HashMap<String, u32>>,
    pub stop_requested: Mutex<HashSet<String>>,
    pub ask_sids: Mutex<HashMap<String, String>>,
    pub panel_open: AtomicBool,
    pub last_mood: Mutex<Option<Mood>>,
}

impl AppState {
    pub fn load(app: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let data_dir = app.path().app_data_dir()?;
        std::fs::create_dir_all(&data_dir)?;
        Ok(Self {
            config: Mutex::new(store::load(&data_dir.join("config.json"))),
            projects: Mutex::new(store::load(&data_dir.join("projects.json"))),
            data_dir,
            sessions: Mutex::new(Sessions::new(now_ms())),
            claude: Mutex::new(Err("Checking for Claude Code…".to_string())),
            auth: Mutex::new(None),
            hooks_installed: AtomicBool::new(false),
            hook_server: Mutex::new(None),
            hook_server_error: Mutex::new(None),
            running: Mutex::new(HashMap::new()),
            stop_requested: Mutex::new(HashSet::new()),
            ask_sids: Mutex::new(HashMap::new()),
            panel_open: AtomicBool::new(false),
            last_mood: Mutex::new(None),
        })
    }

    pub fn save_config(&self) {
        let _ = store::save(&self.data_dir.join("config.json"), &*self.config.lock().unwrap());
    }

    pub fn save_projects(&self) {
        let _ = store::save(&self.data_dir.join("projects.json"), &*self.projects.lock().unwrap());
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetupStatus {
    pub claude_path: Option<String>,
    pub claude_version: Option<String>,
    pub claude_error: Option<String>,
    pub hooks_installed: bool,
    pub auth: Option<AuthVerdict>,
    pub hook_server_error: Option<String>,
    pub needs_setup: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub config: Config,
    pub mood: Mood,
    pub sessions: Vec<SessionInfo>,
    pub projects: Vec<ProjectEntry>,
    pub running: Vec<String>,
    pub setup: SetupStatus,
}

fn setup_status(s: &AppState) -> SetupStatus {
    let claude = s.claude.lock().unwrap().clone();
    let hooks_declined = s.config.lock().unwrap().hooks_declined;
    let auth = s.auth.lock().unwrap().clone();
    let hooks_installed = s.hooks_installed.load(Ordering::SeqCst);
    let hook_server_error = s.hook_server_error.lock().unwrap().clone();
    let auth_refused = matches!(auth, Some(AuthVerdict::Refused { .. }));
    let needs_setup = claude.is_err()
        || hook_server_error.is_some()
        || auth_refused
        || (!hooks_installed && !hooks_declined);
    SetupStatus {
        claude_path: claude.as_ref().ok().map(|l| l.path.display().to_string()),
        claude_version: claude.as_ref().ok().map(|l| l.version.clone()),
        claude_error: claude.err(),
        hooks_installed,
        auth,
        hook_server_error,
        needs_setup,
    }
}

fn mood_with(s: &AppState, setup: &SetupStatus) -> Mood {
    s.sessions
        .lock()
        .unwrap()
        .mood(now_ms(), s.panel_open.load(Ordering::SeqCst), setup.needs_setup)
}

pub fn snapshot(app: &AppHandle) -> Snapshot {
    let s = app.state::<AppState>();
    let setup = setup_status(&s);
    let mood = mood_with(&s, &setup);
    let sessions = s.sessions.lock().unwrap().list();
    let projects = s.projects.lock().unwrap().sorted();
    let running = s.running.lock().unwrap().keys().cloned().collect();
    let config = s.config.lock().unwrap().clone();
    Snapshot { config, mood, sessions, projects, running, setup }
}

pub fn emit_snapshot(app: &AppHandle) {
    let snap = snapshot(app);
    *app.state::<AppState>().last_mood.lock().unwrap() = Some(snap.mood);
    let _ = app.emit("snapshot", &snap);
    shell::update_tray_tooltip(app, &snap);
}

pub fn handle_event(app: &AppHandle, ev: PetEvent) {
    let s = app.state::<AppState>();
    s.sessions.lock().unwrap().apply(&ev);
    let _ = app.emit("pet-event", &ev);
    if ev.kind != Kind::ReplyDelta {
        if let Some(label) = &ev.label {
            let _ = app.emit("pet-bubble", label);
        }
        maybe_notify(app, &ev);
        emit_snapshot(app);
    }
}

fn maybe_notify(app: &AppHandle, ev: &PetEvent) {
    if !matches!(ev.kind, Kind::Done | Kind::NeedsYou) {
        return;
    }
    let s = app.state::<AppState>();
    if s.panel_open.load(Ordering::SeqCst) {
        return;
    }
    let (enabled, pet) = {
        let c = s.config.lock().unwrap();
        (c.notifications, c.pet_name.clone())
    };
    if !enabled {
        return;
    }
    let body = match ev.kind {
        Kind::NeedsYou => "Needs your approval".to_string(),
        _ => ev
            .text
            .as_deref()
            .map(|t| t.chars().take(120).collect())
            .unwrap_or_else(|| "Done".to_string()),
    };
    let _ = app
        .notification()
        .builder()
        .title(format!("{pet} · {}", store::project_name(&ev.project)))
        .body(body)
        .show();
}

pub fn on_hook_body(app: &AppHandle, body: Value) {
    let s = app.state::<AppState>();
    let sid = body.get("session_id").and_then(Value::as_str).unwrap_or("").to_string();
    let ask_project = s.ask_sids.lock().unwrap().get(&sid).cloned();
    if let Some(project) = ask_project {
        if let Some(tp) = body.get("transcript_path").and_then(Value::as_str) {
            if let Some(p) = s.projects.lock().unwrap().get_mut(&project) {
                p.transcript_path = Some(tp.to_string());
            }
            s.save_projects();
        }
        return;
    }
    let Some(ev) = normalize::from_hook(&body, now_ms()) else { return };
    if !ev.project.is_empty() && s.projects.lock().unwrap().touch(&ev.project, ev.at) {
        s.save_projects();
    }
    handle_event(app, ev);
}

pub fn start_hook_server(app: &AppHandle) {
    let s = app.state::<AppState>();
    if let Some(old) = s.hook_server.lock().unwrap().take() {
        old.stop();
    }
    let (port, token) = {
        let c = s.config.lock().unwrap();
        (c.hook_port, c.hook_token.clone())
    };
    let (Some(port), Some(token)) = (port, token) else { return };
    let handle = app.clone();
    match HookServer::start(port, token, move |body| on_hook_body(&handle, body)) {
        Ok(server) => {
            let bound_port = server.port;
            *s.hook_server.lock().unwrap() = Some(server);
            *s.hook_server_error.lock().unwrap() = None;
            if s.config.lock().unwrap().hook_port != Some(bound_port) {
                s.config.lock().unwrap().hook_port = Some(bound_port);
                s.save_config();
            }
        }
        Err(e) => {
            *s.hook_server_error.lock().unwrap() = Some(format!("{e}. Use \"Move to a new port\" in Settings."));
        }
    }
}

pub fn refresh_hooks_installed(app: &AppHandle) {
    let s = app.state::<AppState>();
    let (port, token) = {
        let c = s.config.lock().unwrap();
        (c.hook_port, c.hook_token.clone())
    };
    let installed = match (port, token) {
        (Some(p), Some(t)) => hooks_installer::read_settings(&hooks_installer::settings_path())
            .map(|v| hooks_installer::is_installed(&v, p, &t))
            .unwrap_or(false),
        _ => false,
    };
    s.hooks_installed.store(installed, Ordering::SeqCst);
}

pub fn recheck_setup(app: &AppHandle) {
    let s = app.state::<AppState>();
    let override_path = s.config.lock().unwrap().claude_path.clone();
    let home = dirs::home_dir().unwrap_or_default();
    let path_env = std::env::var_os("PATH");
    let cands = locator::candidates(override_path.as_deref().map(Path::new), path_env.as_deref(), &home);
    let located = locator::locate(&cands, runner::version_of);
    let auth = located.as_ref().ok().map(|l| runner::auth_status(&l.path));
    *s.claude.lock().unwrap() = located;
    *s.auth.lock().unwrap() = auth;
    refresh_hooks_installed(app);
}

pub fn boot(app: AppHandle) {
    std::thread::spawn(move || {
        recheck_setup(&app);
        start_hook_server(&app);
        emit_snapshot(&app);
        let onboarded = app.state::<AppState>().config.lock().unwrap().onboarded;
        if !onboarded {
            let _ = shell::open_panel(&app, "onboarding");
        }
        loop {
            std::thread::sleep(Duration::from_secs(1));
            tick(&app);
        }
    });
}

fn tick(app: &AppHandle) {
    let s = app.state::<AppState>();
    s.sessions.lock().unwrap().prune(now_ms());
    let setup = setup_status(&s);
    let mood = mood_with(&s, &setup);
    let changed = *s.last_mood.lock().unwrap() != Some(mood);
    if changed {
        emit_snapshot(app);
    }
}

fn ask_event(sid: &str, project: &str, kind: Kind, label: &str, text: Option<String>) -> PetEvent {
    PetEvent {
        session_id: sid.to_string(),
        project: project.to_string(),
        source: Source::Ask,
        kind,
        label: Some(label.to_string()),
        text,
        at: now_ms(),
    }
}

fn take_stop_request(s: &AppState, project: &str) -> bool {
    let mut set = s.stop_requested.lock().unwrap();
    let hit = set.iter().find(|p| store::same_path(p, project)).cloned();
    if let Some(h) = &hit {
        set.remove(h);
    }
    hit.is_some()
}

pub fn run_ask(app: AppHandle, project: String, mut child: Child) {
    let s = app.state::<AppState>();
    let pid = child.id();
    let stderr_thread = child.stderr.take().map(|mut err| {
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = err.read_to_string(&mut text);
            text
        })
    });
    let stdout = child.stdout.take().expect("stdout is piped");
    let mut session_id = format!("ask:{project}");
    let mut finished = false;

    let kill = runner::pump(BufReader::new(stdout), &project, now_ms, |item| match item {
        StreamItem::Init { session_id: sid, .. } => {
            session_id = sid.clone();
            s.ask_sids.lock().unwrap().insert(sid.clone(), project.clone());
            if let Some(p) = s.projects.lock().unwrap().get_mut(&project) {
                p.ask_session_id = Some(sid);
            }
            s.save_projects();
        }
        StreamItem::Pet(ev) => {
            if matches!(ev.kind, Kind::Done | Kind::Failed) {
                finished = true;
            }
            handle_event(&app, ev);
        }
        StreamItem::LimitRejected { resets_at } => {
            finished = true;
            let text = normalize::resets_text(resets_at, now_ms());
            handle_event(&app, ask_event(&session_id, &project, Kind::Failed, "Plan limit reached", Some(text)));
        }
        StreamItem::Overage { .. } => {}
    });

    if let Some(reason) = &kill {
        runner::kill_tree(pid);
        finished = true;
        let (label, text) = match reason {
            KillReason::NotSubscription(msg) => ("Stopped: not using your subscription", msg.clone()),
            KillReason::Overage { resets_at } => (
                "Stopped before using paid credits",
                format!(
                    "Your plan limit is used up and this would bill usage credits. {}",
                    normalize::resets_text(*resets_at, now_ms())
                ),
            ),
        };
        handle_event(&app, ask_event(&session_id, &project, Kind::Failed, label, Some(text)));
    }

    let status = child.wait();
    let stderr_text = stderr_thread.and_then(|h| h.join().ok()).unwrap_or_default();
    if take_stop_request(&s, &project) {
        handle_event(&app, ask_event(&session_id, &project, Kind::Ended, "Stopped", None));
    } else if !finished {
        let code = status.ok().and_then(|st| st.code()).map(|c| c.to_string()).unwrap_or_else(|| "?".into());
        let err = stderr_text.trim();
        let text = if err.is_empty() {
            format!("Claude Code exited unexpectedly (code {code}).")
        } else {
            err.chars().take(400).collect()
        };
        handle_event(&app, ask_event(&session_id, &project, Kind::Failed, "Something went wrong", Some(text)));
    }
    s.running.lock().unwrap().retain(|p, _| !store::same_path(p, &project));
    emit_snapshot(&app);
}
