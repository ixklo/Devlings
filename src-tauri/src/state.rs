use std::{
    collections::{HashMap, HashSet},
    io::{BufReader, Read},
    path::{Path, PathBuf},
    process::Child,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::{
    events::{Kind, PetEvent, Source},
    hook_server::{HookServer, EMPTY_ANSWER},
    hooks_installer::{self, HookStatus, HookTarget, Migration, RelayExe},
    locator::{self, Located},
    locks::lock,
    money_guard::AuthVerdict,
    normalize::{self, StreamItem},
    overlay::HitRect,
    pets::{self, Pet},
    runner::{self, KillReason},
    shell,
    store::{self, Config, ProjectEntry, Projects},
    threads::{self, PetState, ThreadInfo, ThreadStatus, Threads},
};

/// What the bubbles show; the ticker compares it to catch changes caused only by time passing.
type ViewKey = (PetState, Vec<String>);

fn temp_dir() -> String {
    std::env::temp_dir().to_string_lossy().to_string()
}

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

pub struct AppState {
    pub data_dir: PathBuf,
    pub config: Mutex<Config>,
    pub projects: Mutex<Projects>,
    pub threads: Mutex<Threads>,
    pub focused_thread: Mutex<Option<String>>,
    pub claude: Mutex<Result<Located, String>>,
    pub auth: Mutex<Option<AuthVerdict>>,
    pub hooks_installed: AtomicBool,
    pub hook_server: Mutex<Option<HookServer>>,
    pub hook_server_error: Mutex<Option<String>>,
    pub running: Mutex<HashMap<String, u32>>,
    pub stop_requested: Mutex<HashSet<String>>,
    pub ask_sids: Mutex<HashMap<String, String>>,
    pub last_view: Mutex<Option<ViewKey>>,
    pub pets: Mutex<Vec<Pet>>,
    /// Interactive rects of the pet window; None until the frontend first reports them.
    pub hit_regions: Mutex<Option<Vec<HitRect>>>,
    pub pet_visibility_gen: AtomicU64,
}

impl AppState {
    pub fn load(data_dir: PathBuf) -> std::io::Result<Self> {
        std::fs::create_dir_all(&data_dir)?;
        let mut projects: Projects = store::load(&data_dir.join("projects.json"));
        projects.retain_outside(&temp_dir());
        Ok(Self {
            config: Mutex::new(store::load::<Config>(&data_dir.join("config.json")).normalized()),
            projects: Mutex::new(projects),
            data_dir,
            threads: Mutex::new(Threads::default()),
            focused_thread: Mutex::new(None),
            claude: Mutex::new(Err("Checking for Claude Code…".to_string())),
            auth: Mutex::new(None),
            hooks_installed: AtomicBool::new(false),
            hook_server: Mutex::new(None),
            hook_server_error: Mutex::new(None),
            running: Mutex::new(HashMap::new()),
            stop_requested: Mutex::new(HashSet::new()),
            ask_sids: Mutex::new(HashMap::new()),
            last_view: Mutex::new(None),
            pets: Mutex::new(Vec::new()),
            hit_regions: Mutex::new(None),
            pet_visibility_gen: AtomicU64::new(0),
        })
    }

    pub fn save_config(&self) {
        let _ = store::save(&self.data_dir.join("config.json"), &*lock(&self.config));
    }

    pub fn save_projects(&self) {
        let _ = store::save(&self.data_dir.join("projects.json"), &*lock(&self.projects));
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
    /// Something the user should do that doesn't block setup, e.g. "Move Perch to Applications…".
    pub setup_hint: Option<String>,
    pub needs_setup: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub config: Config,
    pub pet_state: PetState,
    pub threads: Vec<ThreadInfo>,
    pub projects: Vec<ProjectEntry>,
    pub running: Vec<String>,
    pub setup: SetupStatus,
    pub update: crate::updater::UpdateStatus,
}

fn setup_status(s: &AppState) -> SetupStatus {
    let claude = lock(&s.claude).clone();
    let hooks_declined = lock(&s.config).hooks_declined;
    let auth = lock(&s.auth).clone();
    let hooks_installed = s.hooks_installed.load(Ordering::SeqCst);
    let hook_server_error = lock(&s.hook_server_error).clone();
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
        setup_hint: hooks_installer::relay_exe().ok().and_then(|r| r.setup_hint()),
        needs_setup,
    }
}

fn view_key(threads: &[ThreadInfo], pet_state: PetState) -> ViewKey {
    (pet_state, threads.iter().map(|t| t.session_id.clone()).collect())
}

pub fn snapshot(app: &AppHandle) -> Snapshot {
    let s = app.state::<AppState>();
    let setup = setup_status(&s);
    let threads = lock(&s.threads).visible(now_ms());
    let pet_state = threads::pet_state(&threads, setup.needs_setup);
    let projects = lock(&s.projects).sorted();
    let running = lock(&s.running).keys().cloned().collect();
    let mut config = lock(&s.config).clone();
    // Report the pet that is actually shown, so a removed pet falls back to the default everywhere.
    if let Some(pet) = pets::resolve(&lock(&s.pets), &config.pet_id) {
        config.pet_id = pet.info.id.clone();
    }
    Snapshot { config, pet_state, threads, projects, running, setup, update: crate::updater::status(app) }
}

/// Rescans every pet folder and caches the result. Also returns whether the shown pet changed.
fn scan_pets(app: &AppHandle) -> (Vec<Pet>, bool) {
    let s = app.state::<AppState>();
    let home = dirs::home_dir().unwrap_or_default();
    let roots = pets::roots(
        app.path().resource_dir().ok(),
        &s.data_dir,
        &pets::codex_home(std::env::var_os("CODEX_HOME"), &home),
    );
    let found = pets::discover(&roots);
    let wanted = lock(&s.config).pet_id.clone();
    let shown = |list: &[Pet]| pets::resolve(list, &wanted).map(|p| p.info.id.clone());
    let old = std::mem::replace(&mut *lock(&s.pets), found.clone());
    let changed = shown(&old) != shown(&found);
    (found, changed)
}

/// Rescans pets, and publishes a snapshot if that changed which pet is shown (e.g. its folder was deleted).
pub fn refresh_pets(app: &AppHandle) -> Vec<Pet> {
    let (found, changed) = scan_pets(app);
    if changed {
        emit_snapshot(app);
    }
    found
}

pub fn emit_snapshot(app: &AppHandle) {
    let snap = snapshot(app);
    *lock(&app.state::<AppState>().last_view) = Some(view_key(&snap.threads, snap.pet_state));
    let _ = app.emit("snapshot", &snap);
    shell::update_tray_tooltip(app, &snap);
}

pub fn handle_event(app: &AppHandle, ev: PetEvent) {
    let s = app.state::<AppState>();
    let applied = lock(&s.threads).apply(&ev);
    let _ = app.emit("pet-event", &ev);
    if let Some(status) = applied.alert {
        maybe_notify(app, &ev, status);
    }
    // Reply deltas arrive many times a second; only the first of a reply changes what the bubbles show.
    if ev.kind != Kind::ReplyDelta || applied.changed {
        emit_snapshot(app);
    }
}

fn maybe_notify(app: &AppHandle, ev: &PetEvent, status: ThreadStatus) {
    let s = app.state::<AppState>();
    let (enabled, pet) = {
        let c = lock(&s.config);
        (c.notifications, c.pet_name.clone())
    };
    let focused = lock(&s.focused_thread).as_deref() == Some(ev.session_id.as_str());
    if !enabled || focused {
        return;
    }
    let short = |t: &str| threads::excerpt(t).map(|x| x.chars().take(120).collect::<String>());
    let body = match status {
        ThreadStatus::NeedsInput => "Needs your approval".to_string(),
        ThreadStatus::Ready => ev.text.as_deref().and_then(short).unwrap_or_else(|| "Done".to_string()),
        ThreadStatus::Blocked => ev
            .text
            .as_deref()
            .and_then(short)
            .or_else(|| ev.label.clone())
            .unwrap_or_else(|| "Something went wrong".to_string()),
        ThreadStatus::Running | ThreadStatus::Idle => return,
    };
    let project = lock(&s.threads)
        .get(&ev.session_id)
        .map(|t| t.project_name.clone())
        .unwrap_or_else(|| store::project_name(&ev.project));
    let _ = app.notification().builder().title(format!("{pet} · {project}")).body(body).show();
}

pub fn on_hook_body(app: &AppHandle, body: Value) {
    let s = app.state::<AppState>();
    let sid = body.get("session_id").and_then(Value::as_str).unwrap_or("").to_string();
    let ask_project = lock(&s.ask_sids).get(&sid).cloned();
    if let Some(project) = ask_project {
        if let Some(tp) = body.get("transcript_path").and_then(Value::as_str) {
            let changed = lock(&s.projects).set_transcript_path(&project, tp);
            if changed {
                s.save_projects();
            }
        }
        return;
    }
    let Some(ev) = normalize::from_hook(&body, now_ms()) else { return };
    // Sessions in scratch folders (e.g. other agents' temp dirs) still show in Activity but don't become projects.
    let is_project = !ev.project.is_empty() && !store::is_under(&ev.project, &temp_dir());
    if is_project && lock(&s.projects).touch(&ev.project, ev.at) {
        s.save_projects();
    }
    handle_event(app, ev);
}

pub fn start_hook_server(app: &AppHandle) {
    let s = app.state::<AppState>();
    if let Some(old) = lock(&s.hook_server).take() {
        old.stop();
    }
    let (port, token) = {
        let c = lock(&s.config);
        (c.hook_port, c.hook_token.clone())
    };
    let (Some(port), Some(token)) = (port, token) else { return };
    let handle = app.clone();
    // Already answered; runs on the hook server's single processing thread, in arrival order.
    let on_event = move |body: Value| on_hook_body(&handle, body);
    // Runs on the request's worker and its answer is the response. No decision yet: the approvals feature holds here.
    let on_permission = |_body: Value| EMPTY_ANSWER.to_string();
    match HookServer::start(port, token, on_event, on_permission) {
        Ok(server) => {
            let bound_port = server.port;
            log::info!("Hook server listening on 127.0.0.1:{bound_port}");
            *lock(&s.hook_server) = Some(server);
            *lock(&s.hook_server_error) = None;
            if lock(&s.config).hook_port != Some(bound_port) {
                lock(&s.config).hook_port = Some(bound_port);
                s.save_config();
            }
        }
        Err(e) => {
            log::error!("Hook server didn't start: {e}");
            *lock(&s.hook_server_error) = Some(format!("{e}. Use \"Move to a new port\" in Settings."));
        }
    }
}

/// The installed hooks compared with this build's, or None when Perch has no port and token yet.
pub fn hook_status(app: &AppHandle) -> Option<Result<HookStatus, String>> {
    let s = app.state::<AppState>();
    let (port, token) = {
        let c = lock(&s.config);
        (c.hook_port, c.hook_token.clone())
    };
    let (port, token) = (port?, token?);
    Some(hooks_installer::relay_exe().and_then(|relay| {
        let settings = hooks_installer::read_settings(&hooks_installer::settings_path())?;
        Ok(hooks_installer::status(&settings, HookTarget { port, token: &token, exe: relay.usable() }))
    }))
}

/// Brings installed hooks up to date with this build, once, when they differ (design v1.0 §3).
fn migrate_hooks(app: &AppHandle) {
    let s = app.state::<AppState>();
    let (port, token) = {
        let c = lock(&s.config);
        (c.hook_port, c.hook_token.clone())
    };
    let (Some(port), Some(token)) = (port, token) else { return };
    let relay = match hooks_installer::relay_exe() {
        Ok(RelayExe::Ephemeral(exe)) => {
            // Writing this path would leave hooks pointing at a program that is gone after this run.
            log::warn!("Hook migration skipped: Perch is running from a temporary location ({})", exe.display());
            return;
        }
        Ok(relay) => relay,
        Err(e) => {
            log::error!("Hook migration skipped: {e}");
            return;
        }
    };
    if let RelayExe::Unsafe(exe) = &relay {
        log::warn!("Perch's path has shell characters, so Stop, StopFailure and SessionEnd use HTTP ({})", exe.display());
    }
    let path = hooks_installer::settings_path();
    let target = HookTarget { port, token: &token, exe: relay.usable() };
    match hooks_installer::migrate_file(&path, target, now_ms() / 1000) {
        Ok(Migration::Migrated) => log::info!("Hooks updated to this version's entries in {}", path.display()),
        Ok(Migration::UpToDate) => log::info!("Hooks are up to date"),
        Ok(Migration::NotInstalled) => log::info!("Hooks aren't installed"),
        Err(e) => log::error!("Hook migration failed: {e}"),
    }
}

pub fn refresh_hooks_installed(app: &AppHandle) {
    let installed = matches!(hook_status(app), Some(Ok(HookStatus::Current)));
    app.state::<AppState>().hooks_installed.store(installed, Ordering::SeqCst);
}

pub fn recheck_setup(app: &AppHandle) {
    let s = app.state::<AppState>();
    let override_path = lock(&s.config).claude_path.clone();
    let home = dirs::home_dir().unwrap_or_default();
    let path_env = std::env::var_os("PATH");
    let located = locator::relocate(override_path.as_deref().map(Path::new), path_env.as_deref(), &home, runner::version_of);
    match &located {
        Ok(l) => {
            let via = locator::describe_source(&l.path, override_path.as_deref().map(Path::new), path_env.as_deref(), &home);
            log::info!("Claude Code {} at {} (found via {via})", l.version, l.path.display());
        }
        Err(e) => log::warn!("Claude Code not usable: {e}"),
    }
    let auth = located.as_ref().ok().map(|l| runner::auth_status(&l.path));
    match &auth {
        Some(AuthVerdict::Allowed { subscription }) => log::info!("Claude Code login: {subscription} subscription"),
        Some(AuthVerdict::Refused { reason }) => log::warn!("Claude Code login refused: {reason}"),
        None => {}
    }
    *lock(&s.claude) = located;
    *lock(&s.auth) = auth;
    refresh_hooks_installed(app);
}

/// Re-locates Claude Code if the remembered path no longer runs (design gap G3.5): checked before
/// every Ask spawn, and on a plain setup recheck. A stale user-chosen path is skipped like any
/// missing candidate (see `locator::relocate`), so this falls back to auto-detection on its own.
/// Returns the binary to run, refreshing `s.claude`/`s.auth` first if it had to re-locate.
pub fn ensure_claude_located(app: &AppHandle) -> Result<PathBuf, String> {
    let located = lock(&app.state::<AppState>().claude).clone();
    let stale = match &located {
        Ok(l) => locator::is_stale(&l.path, runner::version_of),
        Err(_) => true,
    };
    if !stale {
        return located.map(|l| l.path);
    }
    log::info!("Claude Code's remembered path is stale; re-locating");
    recheck_setup(app);
    lock(&app.state::<AppState>().claude).clone().map(|l| l.path)
}

pub fn boot(app: AppHandle) {
    std::thread::spawn(move || {
        scan_pets(&app);
        migrate_hooks(&app);
        recheck_setup(&app);
        start_hook_server(&app);
        emit_snapshot(&app);
        let onboarded = lock(&app.state::<AppState>().config).onboarded;
        if !onboarded {
            // Window calls from this thread are queued; on the main thread they run in order, so open_settings can see and undo a minimized start.
            let handle = app.clone();
            let _ = app.run_on_main_thread(move || {
                let _ = shell::open_settings(&handle, shell::SettingsView::Onboarding);
            });
        }
        loop {
            std::thread::sleep(Duration::from_secs(1));
            tick(&app);
        }
    });
}

fn tick(app: &AppHandle) {
    let s = app.state::<AppState>();
    let now = now_ms();
    let threads = {
        let mut t = lock(&s.threads);
        t.prune(now);
        t.visible(now)
    };
    let view = view_key(&threads, threads::pet_state(&threads, setup_status(&s).needs_setup));
    let changed = lock(&s.last_view).as_ref() != Some(&view);
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
    let mut set = lock(&s.stop_requested);
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
    let Some(stdout) = child.stdout.take() else {
        log::error!("Ask in {project}: Claude Code's output wasn't captured");
        runner::kill_tree(pid);
        let _ = child.wait();
        lock(&s.running).retain(|p, _| !store::same_path(p, &project));
        emit_snapshot(&app);
        return;
    };
    let mut session_id = format!("ask:{project}");
    let mut finished = false;

    let kill = runner::pump(BufReader::new(stdout), &project, now_ms, |item| match item {
        StreamItem::Init { session_id: sid, .. } => {
            session_id = sid.clone();
            lock(&s.ask_sids).insert(sid.clone(), project.clone());
            if let Some(p) = lock(&s.projects).get_mut(&project) {
                p.ask_session_id = Some(sid);
            }
            s.save_projects();
        }
        StreamItem::Pet(ev) => {
            if matches!(ev.kind, Kind::Done | Kind::Failed) {
                finished = true;
            }
            let started = ev.kind == Kind::Started;
            let sid = ev.session_id.clone();
            handle_event(&app, ev);
            if started {
                // A headless run reports nothing until its first reply token or tool call; show the thread as working now.
                handle_event(&app, ask_event(&sid, &project, Kind::Prompt, "Thinking…", None));
            }
        }
        StreamItem::LimitRejected { resets_at } => {
            finished = true;
            let text = normalize::resets_text(resets_at, now_ms());
            handle_event(&app, ask_event(&session_id, &project, Kind::Failed, "Plan limit reached", Some(text)));
        }
        StreamItem::Overage { .. } => {}
    });

    if let Some(reason) = &kill {
        log::warn!("Ask in {project} stopped by the money guard: {reason:?}");
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
    match &status {
        Ok(st) => log::info!("Ask in {project} exited ({st})"),
        Err(e) => log::warn!("Ask in {project}: couldn't wait for Claude Code: {e}"),
    }
    if take_stop_request(&s, &project) {
        log::info!("Ask in {project} stopped by the user");
        handle_event(&app, ask_event(&session_id, &project, Kind::Ended, "Stopped", None));
    } else if !finished {
        let code = status.ok().and_then(|st| st.code()).map(|c| c.to_string()).unwrap_or_else(|| "?".into());
        let err = stderr_text.trim();
        let text = if err.is_empty() {
            format!("Claude Code exited unexpectedly (code {code}).")
        } else {
            err.chars().take(400).collect()
        };
        log::warn!("Ask in {project} ended without a result: {text}");
        handle_event(&app, ask_event(&session_id, &project, Kind::Failed, "Something went wrong", Some(text)));
    }
    lock(&s.running).retain(|p, _| !store::same_path(p, &project));
    emit_snapshot(&app);
}
