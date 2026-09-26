mod commands;
mod events;
mod hook_server;
mod hooks_installer;
mod locator;
mod money_guard;
mod normalize;
mod pets;
mod runner;
mod shell;
mod state;
mod store;
mod threads;
mod transcript;

pub fn run() {
    let context = tauri::generate_context!();
    // Managed before the builder creates the config windows: their webviews can invoke commands before setup() runs.
    // dirs::data_dir()/<identifier> is the same folder Tauri's app_data_dir() resolves to on every desktop OS.
    let data_dir = dirs::data_dir()
        .expect("this OS provides a per-user data directory")
        .join(&context.config().identifier);
    let app_state = state::AppState::load(data_dir).expect("Perch's data directory must be writable");

    tauri::Builder::default()
        .manage(app_state)
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| shell::show_pet(app)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .setup(|app| {
            let handle = app.handle().clone();
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
            commands::mark_viewed,
            commands::set_focused_thread,
            commands::set_threads_collapsed,
            commands::set_pet_scale,
            commands::list_pets,
            commands::get_pet_sprite,
            commands::set_pet,
            commands::toggle_panel,
            commands::close_panel,
            commands::show_pet_menu,
            commands::save_pet_position,
        ])
        .run(context)
        .expect("error while running Perch");
}
