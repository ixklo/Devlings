mod commands;
mod events;
mod hook_server;
mod hooks_installer;
mod locator;
mod money_guard;
mod normalize;
mod runner;
mod sessions;
mod shell;
mod state;
mod store;
mod transcript;

use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| shell::show_pet(app)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .setup(|app| {
            let handle = app.handle().clone();
            app.manage(state::AppState::load(&handle)?);
            shell::setup_tray(&handle)?;
            shell::register_shortcut(&handle);
            shell::place_pet(&handle);
            state::boot(handle);
            Ok(())
        })
        .on_menu_event(shell::on_menu_event)
        .on_window_event(shell::on_window_event)
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::recheck_setup,
            commands::set_pet_name,
            commands::install_hooks,
            commands::move_hooks_port,
            commands::uninstall_hooks,
            commands::decline_hooks,
            commands::set_claude_path,
            commands::add_project,
            commands::set_permission_mode,
            commands::new_conversation,
            commands::load_conversation,
            commands::ask,
            commands::stop_ask,
            commands::mark_credits_notice_seen,
            commands::finish_onboarding,
            commands::set_notifications,
            commands::set_launch_at_login,
            commands::toggle_panel,
            commands::close_panel,
            commands::show_pet_menu,
            commands::save_pet_position,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Perch");
}
