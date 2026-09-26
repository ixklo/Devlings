use std::path::Path;

use tauri::{AppHandle, Manager, Window};
use tauri_plugin_autostart::ManagerExt;

use crate::{
    hooks_installer,
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
    s.config.lock().unwrap().pet_name = name;
    s.save_config();
    Ok(publish(&app))
}

fn install_on_port(app: &AppHandle, new_port: bool) -> CmdResult<Snapshot> {
    let s = app.state::<AppState>();
    let (port, token) = {
        let mut c = s.config.lock().unwrap();
        if new_port || c.hook_port.is_none() {
            c.hook_port = Some(hooks_installer::free_port().map_err(|e| e.to_string())?);
        }
        if c.hook_token.is_none() {
            c.hook_token = Some(hooks_installer::new_token());
        }
        c.hooks_declined = false;
        (c.hook_port.unwrap(), c.hook_token.clone().unwrap())
    };
    s.save_config();
    hooks_installer::install_file(&hooks_installer::settings_path(), port, &token, now_ms() / 1000)?;
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
    hooks_installer::uninstall_file(&hooks_installer::settings_path(), now_ms() / 1000)?;
    let s = app.state::<AppState>();
    if let Some(server) = s.hook_server.lock().unwrap().take() {
        server.stop();
    }
    s.config.lock().unwrap().hooks_declined = true;
    s.save_config();
    state::refresh_hooks_installed(&app);
    Ok(publish(&app))
}

#[tauri::command]
pub fn decline_hooks(app: AppHandle) -> Snapshot {
    let s = app.state::<AppState>();
    s.config.lock().unwrap().hooks_declined = true;
    s.save_config();
    publish(&app)
}

#[tauri::command]
pub async fn set_claude_path(app: AppHandle, path: Option<String>) -> Snapshot {
    {
        let s = app.state::<AppState>();
        s.config.lock().unwrap().claude_path = path.filter(|p| !p.trim().is_empty());
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
    s.projects.lock().unwrap().touch(&path, now_ms());
    s.save_projects();
    Ok(publish(&app))
}

#[tauri::command]
pub fn set_permission_mode(app: AppHandle, project: String, mode: PermissionMode) -> CmdResult<Snapshot> {
    let s = app.state::<AppState>();
    s.projects
        .lock()
        .unwrap()
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
        let mut projects = s.projects.lock().unwrap();
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
    let path = s.projects.lock().unwrap().get(&project).and_then(|p| p.transcript_path.clone());
    path.map(|p| transcript::load(Path::new(&p))).unwrap_or_default()
}

#[tauri::command]
pub async fn ask(app: AppHandle, project: String, prompt: String) -> CmdResult<()> {
    let s = app.state::<AppState>();
    if prompt.trim().is_empty() {
        return Err("Type something first.".into());
    }
    if !s.config.lock().unwrap().credits_notice_seen {
        return Err("credits_notice".into());
    }
    if s.running.lock().unwrap().keys().any(|p| store::same_path(p, &project)) {
        return Err("Already working on this project.".into());
    }
    if !Path::new(&project).is_dir() {
        s.projects.lock().unwrap().remove(&project);
        s.save_projects();
        state::emit_snapshot(&app);
        return Err("That folder no longer exists, so it was removed from your projects.".into());
    }
    let bin = s.claude.lock().unwrap().as_ref().map(|l| l.path.clone()).map_err(|e| e.clone())?;
    let verdict = runner::auth_status(&bin);
    *s.auth.lock().unwrap() = Some(verdict.clone());
    if let AuthVerdict::Refused { reason } = verdict {
        state::emit_snapshot(&app);
        return Err(reason);
    }
    let (mode, resume) = {
        let mut projects = s.projects.lock().unwrap();
        projects.touch(&project, now_ms());
        let p = projects.get(&project).expect("project was just touched");
        (p.permission_mode, p.ask_session_id.clone())
    };
    s.save_projects();
    let req = AskRequest { bin, project: project.clone(), prompt, mode_flag: mode.flag(), resume };
    let child = runner::spawn(&req).map_err(|e| format!("Couldn't start Claude Code: {e}"))?;
    s.running.lock().unwrap().insert(project.clone(), child.id());
    state::emit_snapshot(&app);
    let handle = app.clone();
    std::thread::spawn(move || state::run_ask(handle, project, child));
    Ok(())
}

#[tauri::command]
pub fn stop_ask(app: AppHandle, project: String) {
    let s = app.state::<AppState>();
    let pid = s
        .running
        .lock()
        .unwrap()
        .iter()
        .find(|(p, _)| store::same_path(p, &project))
        .map(|(_, pid)| *pid);
    if let Some(pid) = pid {
        s.stop_requested.lock().unwrap().insert(project);
        runner::kill_tree(pid);
    }
}

#[tauri::command]
pub fn mark_credits_notice_seen(app: AppHandle) -> Snapshot {
    let s = app.state::<AppState>();
    s.config.lock().unwrap().credits_notice_seen = true;
    s.save_config();
    publish(&app)
}

#[tauri::command]
pub fn finish_onboarding(app: AppHandle) -> Snapshot {
    let s = app.state::<AppState>();
    s.config.lock().unwrap().onboarded = true;
    s.save_config();
    publish(&app)
}

#[tauri::command]
pub fn set_notifications(app: AppHandle, enabled: bool) -> Snapshot {
    let s = app.state::<AppState>();
    s.config.lock().unwrap().notifications = enabled;
    s.save_config();
    publish(&app)
}

#[tauri::command]
pub fn set_launch_at_login(app: AppHandle, enabled: bool) -> CmdResult<Snapshot> {
    let launcher = app.autolaunch();
    let result = if enabled { launcher.enable() } else { launcher.disable() };
    result.map_err(|e| e.to_string())?;
    let s = app.state::<AppState>();
    s.config.lock().unwrap().launch_at_login = enabled;
    s.save_config();
    Ok(publish(&app))
}

#[tauri::command]
pub fn mark_viewed(app: AppHandle, session_id: String) -> Snapshot {
    app.state::<AppState>().threads.lock().unwrap().mark_viewed(&session_id);
    publish(&app)
}

#[tauri::command]
pub fn set_threads_collapsed(app: AppHandle, collapsed: bool) -> Snapshot {
    let s = app.state::<AppState>();
    s.config.lock().unwrap().threads_collapsed = collapsed;
    s.save_config();
    publish(&app)
}

#[tauri::command]
pub fn set_pet_scale(app: AppHandle, scale: f64) -> Snapshot {
    let s = app.state::<AppState>();
    s.config.lock().unwrap().pet_scale = store::clamp_pet_scale(scale);
    s.save_config();
    publish(&app)
}

#[tauri::command]
pub async fn list_pets(app: AppHandle) -> Vec<PetInfo> {
    state::refresh_pets(&app).into_iter().map(|p| p.info).collect()
}

#[tauri::command]
pub async fn get_pet_sprite(app: AppHandle, id: String) -> CmdResult<String> {
    let cached = app.state::<AppState>().pets.lock().unwrap().iter().find(|p| p.info.id == id).cloned();
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
    s.config.lock().unwrap().pet_id = id.to_string();
    s.save_config();
    Ok(())
}

#[tauri::command]
pub fn set_focused_thread(app: AppHandle, session_id: Option<String>) {
    *app.state::<AppState>().focused_thread.lock().unwrap() = session_id;
}

#[tauri::command]
pub fn set_hit_regions(app: AppHandle, regions: Vec<HitRect>) {
    *app.state::<AppState>().hit_regions.lock().unwrap() = Some(regions);
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

#[tauri::command]
pub fn save_pet_position(app: AppHandle, x: i32, y: i32) {
    let s = app.state::<AppState>();
    s.config.lock().unwrap().pet_position = Some((x, y));
    s.save_config();
}
