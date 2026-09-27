use std::path::Path;

use tauri::{AppHandle, Manager, Window};
use tauri_plugin_autostart::ManagerExt;

use crate::{
    diagnostics,
    hooks_installer::{self, HookTarget},
    locks::lock,
    money_guard::AuthVerdict,
    overlay::{self, HitRect},
    pets::{self, PetInfo},
    runner::{self, AskRequest},
    shell::{self, SettingsView},
    state::{self, now_ms, AppState, Snapshot},
    store::{self, PermissionMode},
    transcript::{self, ChatTurn},
};

type CmdResult<T> = Result<T, String>;

fn publish(app: &AppHandle) -> Snapshot {
    state::emit_snapshot(app);
    state::snapshot(app)
}

#[tauri::command]
pub fn get_snapshot(app: AppHandle) -> Snapshot {
    state::snapshot(&app)
}

#[tauri::command]
pub async fn recheck_setup(app: AppHandle) -> Snapshot {
    state::recheck_setup(&app);
    publish(&app)
}

#[tauri::command]
pub fn set_pet_name(app: AppHandle, name: String) -> CmdResult<Snapshot> {
    let name = store::validate_pet_name(&name)?;
    let s = app.state::<AppState>();
    lock(&s.config).pet_name = name;
    s.save_config();
    Ok(publish(&app))
}

fn install_on_port(app: &AppHandle, new_port: bool) -> CmdResult<Snapshot> {
    let s = app.state::<AppState>();
    let exe = hooks_installer::relay_exe()?;
    let (port, token) = {
        let mut c = lock(&s.config);
        let port = match c.hook_port {
            Some(p) if !new_port => p,
            _ => hooks_installer::free_port().map_err(|e| e.to_string())?,
        };
        let token = c.hook_token.clone().unwrap_or_else(hooks_installer::new_token);
        c.hook_port = Some(port);
        c.hook_token = Some(token.clone());
        c.hooks_declined = false;
        (port, token)
    };
    s.save_config();
    let path = hooks_installer::settings_path();
    if let Err(e) = hooks_installer::install_file(&path, HookTarget { port, token: &token, exe: &exe }, now_ms() / 1000) {
        log::error!("Installing hooks failed: {e}");
        return Err(e);
    }
    log::info!("Hooks installed in {} for port {port}", path.display());
    state::start_hook_server(app);
    state::refresh_hooks_installed(app);
    Ok(publish(app))
}

#[tauri::command]
pub async fn install_hooks(app: AppHandle) -> CmdResult<Snapshot> {
    install_on_port(&app, false)
}

#[tauri::command]
pub async fn move_hooks_port(app: AppHandle) -> CmdResult<Snapshot> {
    install_on_port(&app, true)
}

#[tauri::command]
pub fn uninstall_hooks(app: AppHandle) -> CmdResult<Snapshot> {
    match hooks_installer::uninstall_file(&hooks_installer::settings_path(), now_ms() / 1000) {
        Ok(changed) => log::info!("Hooks removed (settings.json changed: {changed})"),
        Err(e) => {
            log::error!("Removing hooks failed: {e}");
            return Err(e);
        }
    }
    let s = app.state::<AppState>();
    if let Some(server) = lock(&s.hook_server).take() {
        server.stop();
    }
    lock(&s.config).hooks_declined = true;
    s.save_config();
    state::refresh_hooks_installed(&app);
    Ok(publish(&app))
}

#[tauri::command]
pub fn decline_hooks(app: AppHandle) -> Snapshot {
    let s = app.state::<AppState>();
    lock(&s.config).hooks_declined = true;
    s.save_config();
    publish(&app)
}

#[tauri::command]
pub async fn set_claude_path(app: AppHandle, path: Option<String>) -> Snapshot {
    {
        let s = app.state::<AppState>();
        lock(&s.config).claude_path = path.filter(|p| !p.trim().is_empty());
        s.save_config();
    }
    state::recheck_setup(&app);
    publish(&app)
}

#[tauri::command]
pub fn add_project(app: AppHandle, path: String) -> CmdResult<Snapshot> {
    if !Path::new(&path).is_dir() {
        return Err("That folder doesn't exist.".into());
    }
    let s = app.state::<AppState>();
    lock(&s.projects).touch(&path, now_ms());
    s.save_projects();
    Ok(publish(&app))
}

#[tauri::command]
pub fn set_permission_mode(app: AppHandle, project: String, mode: PermissionMode) -> CmdResult<Snapshot> {
    let s = app.state::<AppState>();
    lock(&s.projects)
        .get_mut(&project)
        .ok_or("Unknown project.")?
        .permission_mode = mode;
    s.save_projects();
    Ok(publish(&app))
}

#[tauri::command]
pub fn new_conversation(app: AppHandle, project: String) -> CmdResult<Snapshot> {
    let s = app.state::<AppState>();
    {
        let mut projects = lock(&s.projects);
        let p = projects.get_mut(&project).ok_or("Unknown project.")?;
        p.ask_session_id = None;
        p.transcript_path = None;
    }
    s.save_projects();
    Ok(publish(&app))
}

#[tauri::command]
pub fn load_conversation(app: AppHandle, project: String) -> Vec<ChatTurn> {
    let s = app.state::<AppState>();
    let path = lock(&s.projects).get(&project).and_then(|p| p.transcript_path.clone());
    path.map(|p| transcript::load(Path::new(&p))).unwrap_or_default()
}

#[tauri::command]
pub async fn ask(app: AppHandle, project: String, prompt: String) -> CmdResult<()> {
    let s = app.state::<AppState>();
    if prompt.trim().is_empty() {
        return Err("Type something first.".into());
    }
    if !lock(&s.config).credits_notice_seen {
        return Err("credits_notice".into());
    }
    if lock(&s.running).keys().any(|p| store::same_path(p, &project)) {
        return Err("Already working on this project.".into());
    }
    if !Path::new(&project).is_dir() {
        lock(&s.projects).remove(&project);
        s.save_projects();
        state::emit_snapshot(&app);
        return Err("That folder no longer exists, so it was removed from your projects.".into());
    }
    let bin = lock(&s.claude).as_ref().map(|l| l.path.clone()).map_err(|e| e.clone())?;
    let verdict = runner::auth_status(&bin);
    *lock(&s.auth) = Some(verdict.clone());
    if let AuthVerdict::Refused { reason } = verdict {
        state::emit_snapshot(&app);
        return Err(reason);
    }
    let (mode, resume) = {
        let mut projects = lock(&s.projects);
        projects.touch(&project, now_ms());
        let p = projects.get(&project);
        (p.map(|p| p.permission_mode).unwrap_or_default(), p.and_then(|p| p.ask_session_id.clone()))
    };
    s.save_projects();
    log::info!("Ask in {project} (mode {}, {})", mode.flag(), if resume.is_some() { "follow-up" } else { "new chat" });
    let req = AskRequest { bin, project: project.clone(), prompt, mode_flag: mode.flag(), resume };
    let child = runner::spawn(&req).map_err(|e| {
        log::error!("Ask in {project} couldn't start Claude Code: {e}");
        format!("Couldn't start Claude Code: {e}")
    })?;
    lock(&s.running).insert(project.clone(), child.id());
    state::emit_snapshot(&app);
    let handle = app.clone();
    std::thread::spawn(move || state::run_ask(handle, project, child));
    Ok(())
}

#[tauri::command]
pub fn stop_ask(app: AppHandle, project: String) {
    let s = app.state::<AppState>();
    let pid = lock(&s.running)
        .iter()
        .find(|(p, _)| store::same_path(p, &project))
        .map(|(_, pid)| *pid);
    if let Some(pid) = pid {
        log::info!("Stop requested for the Ask in {project}");
        lock(&s.stop_requested).insert(project);
        runner::kill_tree(pid);
    }
}

#[tauri::command]
pub fn mark_credits_notice_seen(app: AppHandle) -> Snapshot {
    let s = app.state::<AppState>();
    lock(&s.config).credits_notice_seen = true;
    s.save_config();
    publish(&app)
}

#[tauri::command]
pub fn finish_onboarding(app: AppHandle) -> Snapshot {
    let s = app.state::<AppState>();
    lock(&s.config).onboarded = true;
    s.save_config();
    publish(&app)
}

#[tauri::command]
pub fn set_notifications(app: AppHandle, enabled: bool) -> Snapshot {
    let s = app.state::<AppState>();
    lock(&s.config).notifications = enabled;
    s.save_config();
    publish(&app)
}

#[tauri::command]
pub fn set_launch_at_login(app: AppHandle, enabled: bool) -> CmdResult<Snapshot> {
    let launcher = app.autolaunch();
    let result = if enabled { launcher.enable() } else { launcher.disable() };
    result.map_err(|e| e.to_string())?;
    let s = app.state::<AppState>();
    lock(&s.config).launch_at_login = enabled;
    s.save_config();
    Ok(publish(&app))
}

#[tauri::command]
pub fn mark_viewed(app: AppHandle, session_id: String) -> Snapshot {
    lock(&app.state::<AppState>().threads).mark_viewed(&session_id);
    publish(&app)
}

#[tauri::command]
pub fn set_threads_collapsed(app: AppHandle, collapsed: bool) -> Snapshot {
    let s = app.state::<AppState>();
    lock(&s.config).threads_collapsed = collapsed;
    s.save_config();
    publish(&app)
}

#[tauri::command]
pub fn set_pet_scale(app: AppHandle, scale: f64) -> Snapshot {
    let s = app.state::<AppState>();
    lock(&s.config).pet_scale = store::clamp_pet_scale(scale);
    s.save_config();
    publish(&app)
}

#[tauri::command]
pub async fn list_pets(app: AppHandle) -> Vec<PetInfo> {
    state::refresh_pets(&app).into_iter().map(|p| p.info).collect()
}

#[tauri::command]
pub async fn get_pet_sprite(app: AppHandle, id: String) -> CmdResult<String> {
    let cached = lock(&app.state::<AppState>().pets).iter().find(|p| p.info.id == id).cloned();
    let pet = match cached {
        Some(p) => p,
        None => pets::resolve(&state::refresh_pets(&app), &id).cloned().ok_or("No pets found.")?,
    };
    pets::sprite_data_url(&pet.sprite)
}

#[tauri::command]
pub async fn set_pet(app: AppHandle, id: String) -> CmdResult<Snapshot> {
    choose_pet(&app, &id)?;
    Ok(publish(&app))
}

pub fn choose_pet(app: &AppHandle, id: &str) -> CmdResult<()> {
    if !state::refresh_pets(app).iter().any(|p| p.info.id == id) {
        return Err("That pet wasn't found.".into());
    }
    let s = app.state::<AppState>();
    lock(&s.config).pet_id = id.to_string();
    s.save_config();
    Ok(())
}

#[tauri::command]
pub fn set_focused_thread(app: AppHandle, session_id: Option<String>) {
    *lock(&app.state::<AppState>().focused_thread) = session_id;
}

#[tauri::command]
pub fn set_hit_regions(app: AppHandle, regions: Vec<HitRect>) {
    *lock(&app.state::<AppState>().hit_regions) = Some(regions);
}

#[tauri::command]
pub fn open_settings(app: AppHandle, view: SettingsView) -> CmdResult<()> {
    shell::open_settings(&app, view).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn close_settings(app: AppHandle) -> CmdResult<()> {
    shell::close_settings(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_project(app: AppHandle, path: String) -> CmdResult<()> {
    let dir = Path::new(&path);
    if !dir.is_absolute() || !dir.is_dir() {
        return Err("That folder doesn't exist.".into());
    }
    shell::open_project(&app, dir)
}

#[tauri::command]
pub async fn open_pets_folder(app: AppHandle) -> CmdResult<()> {
    let dir = pets::user_pets_dir(&app.state::<AppState>().data_dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't create {}: {e}", dir.display()))?;
    shell::open_folder(&app, &dir)
}

#[tauri::command]
pub fn reset_pet_position(app: AppHandle) {
    overlay::reset_pet_position(&app);
}

#[tauri::command]
pub fn move_pet_by(app: AppHandle, dx: f64, dy: f64) {
    overlay::move_pet_by(&app, dx, dy);
}

#[tauri::command]
pub fn drag_pet_by(app: AppHandle, dx: f64, dy: f64) {
    overlay::drag_pet_by(&app, dx, dy);
}

#[tauri::command]
pub fn show_pet_menu(window: Window) -> CmdResult<()> {
    shell::show_pet_menu(&window).map_err(|e| e.to_string())
}

/// Redacted diagnostics for bug reports: versions, how Claude Code was found, hook status, and the log tail.
#[tauri::command]
pub async fn get_diagnostics(app: AppHandle) -> String {
    diagnostics::collect(&app)
}

#[tauri::command]
pub async fn open_log_folder(app: AppHandle) -> CmdResult<()> {
    let dir = diagnostics::log_dir(&app);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't create {}: {e}", dir.display()))?;
    shell::open_folder(&app, &dir)
}

#[tauri::command]
pub fn save_pet_position(app: AppHandle, x: i32, y: i32) {
    let s = app.state::<AppState>();
    lock(&s.config).pet_position = Some((x, y));
    s.save_config();
}
