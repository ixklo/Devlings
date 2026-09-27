use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Stdio,
    sync::atomic::Ordering,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, WebviewWindow, Window, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_opener::OpenerExt;

use crate::{
    commands,
    locks::lock,
    overlay, pets, runner,
    state::{self, AppState, Snapshot},
    threads::PetState,
};

const TRAY_ID: &str = "perch-tray";
pub const SETTINGS: &str = "settings";
const PET_ITEM_PREFIX: &str = "pet:";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingsView {
    Settings,
    Onboarding,
}

fn window(app: &AppHandle, label: &str) -> WebviewWindow {
    app.get_webview_window(label)
        .unwrap_or_else(|| panic!("window '{label}' is declared in tauri.conf.json"))
}

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "toggle-pet", "Show/Hide pet", true, None::<&str>)?,
            &MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?,
        ],
    )?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID).tooltip("Perch").menu(&menu);
    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    Ok(())
}

fn state_words(s: PetState) -> &'static str {
    match s {
        PetState::Setup => "needs setup",
        PetState::NeedsInput => "needs you",
        PetState::Blocked => "hit a problem",
        PetState::Ready => "has a reply",
        PetState::Running => "is working",
        PetState::Idle => "is idle",
    }
}

pub fn update_tray_tooltip(app: &AppHandle, snap: &Snapshot) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(format!("{} {}", snap.config.pet_name, state_words(snap.pet_state))));
    }
}

pub fn register_shortcut(app: &AppHandle) {
    let result = app.global_shortcut().on_shortcut("CommandOrControl+Alt+P", |app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            toggle_pet(app, true);
        }
    });
    if let Err(e) = result {
        log::warn!("Couldn't register Ctrl+Alt+P: {e}");
    }
}

/// Every show or hide bumps the generation, so a pending "hide for 1 hour" timer knows it was overridden.
fn bump_visibility(app: &AppHandle) -> u64 {
    app.state::<AppState>().pet_visibility_gen.fetch_add(1, Ordering::SeqCst) + 1
}

pub fn show_pet(app: &AppHandle) {
    bump_visibility(app);
    let _ = window(app, overlay::PET).show();
}

fn hide_pet(app: &AppHandle) -> u64 {
    let generation = bump_visibility(app);
    let _ = window(app, overlay::PET).hide();
    generation
}

/// Shows or hides the pet. When `compose` is set, showing it also focuses it and opens the composer.
pub fn toggle_pet(app: &AppHandle, compose: bool) {
    let pet = window(app, overlay::PET);
    if pet.is_visible().unwrap_or(true) {
        hide_pet(app);
        return;
    }
    show_pet(app);
    if compose {
        let _ = pet.set_focus();
        let _ = app.emit_to(overlay::PET, "pet-open", json!({ "view": "compose" }));
    }
}

fn hide_pet_for_hour(app: &AppHandle) {
    let generation = hide_pet(app);
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(3600));
        if handle.state::<AppState>().pet_visibility_gen.load(Ordering::SeqCst) == generation {
            show_pet(&handle);
        }
    });
}

pub fn open_settings(app: &AppHandle, view: SettingsView) -> tauri::Result<()> {
    let settings = window(app, SETTINGS);
    if !settings.is_visible()? {
        settings.show()?;
    }
    if settings.is_minimized()? {
        settings.unminimize()?;
    }
    settings.set_focus()?;
    app.emit_to(SETTINGS, "settings-view", view)
}

pub fn close_settings(app: &AppHandle) -> tauri::Result<()> {
    window(app, SETTINGS).hide()
}

pub fn show_pet_menu(win: &Window) -> tauri::Result<()> {
    let app = win.app_handle();
    let all = state::refresh_pets(app);
    let wanted = lock(&app.state::<AppState>().config).pet_id.clone();
    let current = pets::resolve(&all, &wanted).map(|p| p.info.id.clone());
    let change = Submenu::with_id(app, "change-pet", "Change pet", true)?;
    if all.is_empty() {
        change.append(&MenuItem::with_id(app, "no-pets", "No pets found", false, None::<&str>)?)?;
    }
    for p in &all {
        let checked = current.as_deref() == Some(p.info.id.as_str());
        let id = format!("{PET_ITEM_PREFIX}{}", p.info.id);
        change.append(&CheckMenuItem::with_id(app, id, &p.info.display_name, true, checked, None::<&str>)?)?;
    }
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?,
            &change,
            &MenuItem::with_id(app, "hide-hour", "Hide for 1 hour", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?,
        ],
    )?;
    win.popup_menu(&menu)
}

pub fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    let id = event.id().as_ref();
    if let Some(pet_id) = id.strip_prefix(PET_ITEM_PREFIX) {
        if commands::choose_pet(app, pet_id).is_ok() {
            state::emit_snapshot(app);
        }
        return;
    }
    match id {
        "toggle-pet" => toggle_pet(app, false),
        "settings" => {
            let _ = open_settings(app, SettingsView::Settings);
        }
        "hide-hour" => hide_pet_for_hour(app),
        "quit" => app.exit(0),
        _ => {}
    }
}

pub fn on_window_event(win: &Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        if win.label() == overlay::PET {
            hide_pet(win.app_handle());
        } else {
            let _ = win.hide();
        }
    }
}

fn vscode_cli_name() -> &'static str {
    if cfg!(windows) { "code.cmd" } else { "code" }
}

/// VS Code's `code` launcher on PATH, if any.
pub fn find_on_path(name: &str, path_env: Option<&OsStr>) -> Option<PathBuf> {
    std::env::split_paths(path_env?).map(|dir| dir.join(name)).find(|p| p.is_file())
}

/// Windows: VS Code's CLI in the two common per-user/per-machine install locations. Pure
/// (injected env values) so it's testable on every OS, even though only Windows calls it for real.
pub fn windows_known_locations(local_appdata: Option<&str>, program_files: Option<&str>, program_files_x86: Option<&str>) -> Vec<PathBuf> {
    [local_appdata.map(|d| Path::new(d).join("Programs")), program_files.map(PathBuf::from), program_files_x86.map(PathBuf::from)]
        .into_iter()
        .flatten()
        .map(|dir| dir.join("Microsoft VS Code").join("bin").join("code.cmd"))
        .collect()
}

/// macOS: the CLI inside the app bundle, system-wide and per-user.
pub fn macos_known_locations(home: &Path) -> Vec<PathBuf> {
    const SUFFIX: &str = "Visual Studio Code.app/Contents/Resources/app/bin/code";
    vec![PathBuf::from("/Applications").join(SUFFIX), home.join("Applications").join(SUFFIX)]
}

/// Linux: common CLI locations across distro packages, Snap and Flatpak.
pub fn linux_known_locations() -> Vec<PathBuf> {
    ["/usr/bin/code", "/usr/share/code/bin/code", "/snap/bin/code", "/var/lib/flatpak/exports/bin/com.visualstudio.code"]
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

/// Where a `code-url-handler` registration would live on Linux, so the `vscode://` URL is only
/// tried when something has actually registered it (otherwise a plain xdg-open would show a
/// "choose an app" prompt instead of opening VS Code).
pub fn linux_url_handler_locations(home: &Path) -> Vec<PathBuf> {
    ["/usr/share/applications", "/usr/local/share/applications"]
        .into_iter()
        .map(PathBuf::from)
        .chain(std::iter::once(home.join(".local/share/applications")))
        .map(|dir| dir.join("code-url-handler.desktop"))
        .collect()
}

/// VS Code CLI candidates in order: PATH, then this OS's common install locations (step 1-2).
pub fn vscode_cli_candidates(
    path_env: Option<&OsStr>,
    local_appdata: Option<&str>,
    program_files: Option<&str>,
    program_files_x86: Option<&str>,
    home: &Path,
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    out.extend(find_on_path(vscode_cli_name(), path_env));
    if cfg!(windows) {
        out.extend(windows_known_locations(local_appdata, program_files, program_files_x86));
    } else if cfg!(target_os = "macos") {
        out.extend(macos_known_locations(home));
    } else {
        out.extend(linux_known_locations());
    }
    out
}

/// Builds a `vscode://file/<path>/` URL (step 3): forward slashes, a trailing slash so VS Code
/// treats it as a folder, and percent-encoding for the characters that would otherwise break the
/// URL (spaces, `#`, `%`, and anything non-ASCII). Everything else, including a drive letter's
/// `:` or a UNC path's doubled leading slash, is passed through unescaped.
pub fn vscode_url(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let with_leading_slash = if normalized.starts_with('/') { normalized } else { format!("/{normalized}") };
    let mut url = String::from("vscode://file");
    for ch in with_leading_slash.chars() {
        if ch == ' ' || ch == '#' || ch == '%' || !ch.is_ascii() {
            let mut buf = [0u8; 4];
            for byte in ch.encode_utf8(&mut buf).as_bytes() {
                url.push('%');
                url.push_str(&format!("{byte:02X}"));
            }
        } else {
            url.push(ch);
        }
    }
    if !url.ends_with('/') {
        url.push('/');
    }
    url
}

/// Whether Windows has something registered for the `vscode:` URL scheme, so opening one never
/// shows "How do you want to open this?". Shells out to `reg query` rather than adding a registry
/// crate dependency for a single read-only lookup.
#[cfg(windows)]
fn windows_vscode_scheme_registered() -> bool {
    ["HKCU\\Software\\Classes\\vscode", "HKCR\\vscode"].iter().any(|key| {
        runner::background_command(Path::new("reg"))
            .args(["query", key])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}

#[cfg(not(windows))]
fn windows_vscode_scheme_registered() -> bool {
    false
}

/// Whether the `vscode://` URL is safe to try (step 3's gate): registered on Windows; on macOS or
/// Linux, only when VS Code's app or its URL handler was actually found in step 2's locations.
fn vscode_url_usable(home: &Path) -> bool {
    if cfg!(windows) {
        windows_vscode_scheme_registered()
    } else if cfg!(target_os = "macos") {
        macos_known_locations(home).iter().any(|p| p.is_file())
    } else {
        linux_url_handler_locations(home).iter().any(|p| p.is_file())
    }
}

/// Opens a folder in VS Code (PATH, then known install locations, then the `vscode://` URL if the
/// scheme is safe to use), falling back to the file manager.
pub fn open_project(app: &AppHandle, dir: &Path) -> Result<(), String> {
    let home = dirs::home_dir().unwrap_or_default();
    let candidates = vscode_cli_candidates(
        std::env::var_os("PATH").as_deref(),
        std::env::var("LOCALAPPDATA").ok().as_deref(),
        std::env::var("ProgramFiles").ok().as_deref(),
        std::env::var("ProgramFiles(x86)").ok().as_deref(),
        &home,
    );
    if let Some(code) = candidates.into_iter().find(|p| p.is_file()) {
        // Rust runs a .cmd through cmd.exe itself and escapes the arguments for it.
        let spawned = runner::background_command(&code)
            .arg(dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        if spawned.is_ok() {
            return Ok(());
        }
    }
    if vscode_url_usable(&home) && app.opener().open_url(vscode_url(dir), None::<&str>).is_ok() {
        return Ok(());
    }
    open_folder(app, dir)
}

pub fn open_folder(app: &AppHandle, dir: &Path) -> Result<(), String> {
    match app.opener().open_path(dir.to_string_lossy(), None::<&str>) {
        Ok(()) => Ok(()),
        Err(e) => {
            #[cfg(windows)]
            if runner::background_command(Path::new("explorer")).arg(dir).spawn().is_ok() {
                return Ok(());
            }
            Err(format!("Couldn't open {}: {e}", dir.display()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_first_match_on_path() {
        let t = tempfile::tempdir().unwrap();
        let (a, b) = (t.path().join("a"), t.path().join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(b.join("code.cmd")).unwrap();
        std::fs::write(a.join("code.cmd"), "").unwrap();
        let path = std::env::join_paths([b.clone(), a.clone()]).unwrap();
        assert_eq!(find_on_path("code.cmd", Some(&path)), Some(a.join("code.cmd")));
        assert_eq!(find_on_path("missing", Some(&path)), None);
        assert_eq!(find_on_path("code.cmd", None), None);
    }

    #[test]
    fn windows_known_locations_use_each_env_var_when_present() {
        assert_eq!(windows_known_locations(None, None, None), Vec::<PathBuf>::new());
        assert_eq!(
            windows_known_locations(Some("C:\\Users\\me\\AppData\\Local"), None, None),
            vec![PathBuf::from("C:\\Users\\me\\AppData\\Local\\Programs\\Microsoft VS Code\\bin\\code.cmd")]
        );
        assert_eq!(
            windows_known_locations(Some("L"), Some("C:\\Program Files"), Some("C:\\Program Files (x86)")),
            vec![
                PathBuf::from("L\\Programs\\Microsoft VS Code\\bin\\code.cmd"),
                PathBuf::from("C:\\Program Files\\Microsoft VS Code\\bin\\code.cmd"),
                PathBuf::from("C:\\Program Files (x86)\\Microsoft VS Code\\bin\\code.cmd"),
            ]
        );
    }

    #[test]
    fn macos_known_locations_cover_system_and_user_applications() {
        let home = Path::new("/Users/me");
        let found = macos_known_locations(home);
        assert_eq!(found[0], PathBuf::from("/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code"));
        assert_eq!(found[1], home.join("Applications/Visual Studio Code.app/Contents/Resources/app/bin/code"));
    }

    #[test]
    fn linux_known_locations_cover_distro_snap_and_flatpak() {
        let found = linux_known_locations();
        assert!(found.contains(&PathBuf::from("/usr/bin/code")));
        assert!(found.contains(&PathBuf::from("/snap/bin/code")));
        assert!(found.contains(&PathBuf::from("/var/lib/flatpak/exports/bin/com.visualstudio.code")));
    }

    #[test]
    fn linux_url_handler_locations_include_the_user_and_system_dirs() {
        let home = Path::new("/home/me");
        let found = linux_url_handler_locations(home);
        assert!(found.contains(&PathBuf::from("/usr/share/applications/code-url-handler.desktop")));
        assert!(found.contains(&home.join(".local/share/applications/code-url-handler.desktop")));
    }

    #[test]
    fn cli_candidates_try_path_before_known_locations() {
        let t = tempfile::tempdir().unwrap();
        let on_path_dir = t.path().join("on-path");
        std::fs::create_dir_all(&on_path_dir).unwrap();
        std::fs::write(on_path_dir.join(vscode_cli_name()), "").unwrap();
        let path_env = std::env::join_paths([&on_path_dir]).unwrap();
        // Every platform's known-location env vars are supplied, so the list is non-empty on any OS.
        let candidates = vscode_cli_candidates(Some(&path_env), Some("L"), Some("P"), Some("P86"), Path::new("/h"));
        assert_eq!(candidates[0], on_path_dir.join(vscode_cli_name()));
        assert!(candidates.len() > 1, "known locations should follow the PATH match");
        // PATH missing entirely: falls straight to the platform's known locations.
        let candidates = vscode_cli_candidates(None, Some("L"), Some("P"), Some("P86"), Path::new("/h"));
        assert!(!candidates.is_empty());
    }

    #[test]
    fn vscode_url_builder_handles_a_windows_drive_a_unc_and_a_unix_path() {
        assert_eq!(vscode_url(Path::new(r"C:\Users\me\My Proj")), "vscode://file/C:/Users/me/My%20Proj/");
        assert_eq!(vscode_url(Path::new(r"\\srv\share\proj #1")), "vscode://file//srv/share/proj%20%231/");
        assert_eq!(vscode_url(Path::new("/home/me/café")), "vscode://file/home/me/caf%C3%A9/");
        // Already has a trailing slash: not doubled.
        assert_eq!(vscode_url(Path::new("/already/there/")), "vscode://file/already/there/");
        // A literal percent must be escaped so it isn't read as the start of another escape.
        assert_eq!(vscode_url(Path::new("/proj/100%")), "vscode://file/proj/100%25/");
    }

    #[test]
    fn settings_views_use_the_contract_strings() {
        assert_eq!(serde_json::to_value(SettingsView::Onboarding).unwrap(), "onboarding");
        assert_eq!(serde_json::from_str::<SettingsView>("\"settings\"").unwrap(), SettingsView::Settings);
        assert!(serde_json::from_str::<SettingsView>("\"main\"").is_err());
    }

    /// A folder name with shell metacharacters reaches a .cmd launcher as one literal argument.
    #[cfg(windows)]
    #[test]
    fn cmd_launchers_get_paths_verbatim() {
        let t = tempfile::tempdir().unwrap();
        let out = t.path().join("out.txt");
        let script = t.path().join("fake code.cmd");
        std::fs::write(&script, format!("@echo off\r\n>\"{}\" echo(%1\r\n", out.display())).unwrap();
        let dir = t.path().join("R&D (x) 100%");
        std::fs::create_dir_all(&dir).unwrap();
        let status = runner::background_command(&script).arg(&dir).status().unwrap();
        assert!(status.success());
        assert_eq!(std::fs::read_to_string(&out).unwrap().trim().trim_matches('"'), dir.display().to_string());
    }
}
