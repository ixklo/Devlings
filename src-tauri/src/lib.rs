mod cli;
mod commands;
mod diagnostics;
mod events;
mod hook_server;
mod hooks_installer;
mod locator;
mod locks;
mod money_guard;
mod normalize;
mod notify;
mod overlay;
mod pets;
mod runner;
mod shell;
mod state;
mod store;
mod threads;
mod transcript;
mod trust;
mod updater;

pub fn run() {
    // Headless modes (the hook relay and the uninstaller's cleanup) come before anything else, so they stay fast
    // and never reach a running Perch through the single-instance plugin.
    if let Some(code) = cli::run_headless() {
        std::process::exit(code);
    }
    diagnostics::install_panic_hook();
    let context = tauri::generate_context!();
    // Managed before the builder creates the config windows: their webviews can invoke commands before setup() runs.
    // dirs::data_dir()/<identifier> is the same folder Tauri's app_data_dir() resolves to on every desktop OS.
    let data_dir = dirs::data_dir()
        .expect("this OS provides a per-user data directory")
        .join(&context.config().identifier);
    let app_state = state::AppState::load(data_dir).expect("Perch's data directory must be writable");
    let log_level = store::diagnostics_level_filter(&locks::lock(&app_state.config).diagnostics_level);

    tauri::Builder::default()
        .manage(app_state)
        .manage(updater::Updates::default())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| shell::show_pet(app)))
        .plugin(diagnostics::log_plugin(log_level))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let info = app.package_info();
            log::info!("{} {} starting on {} {}", info.name, info.version, std::env::consts::OS, std::env::consts::ARCH);
            let handle = app.handle().clone();
            shell::setup_tray(&handle)?;
            shell::register_shortcut(&handle);
            overlay::place_pet(&handle);
            overlay::start_click_through(handle.clone());
            updater::start(handle.clone());
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
            commands::trust_project,
            commands::untrust_project,
            commands::mark_credits_notice_seen,
            commands::finish_onboarding,
            commands::set_notifications,
            commands::set_launch_at_login,
            updater::check_for_update,
            updater::install_update,
            updater::set_auto_update,
            commands::mark_viewed,
            commands::set_focused_thread,
            commands::set_threads_collapsed,
            commands::set_pet_scale,
            commands::list_pets,
            commands::get_pet_sprite,
            commands::set_pet,
            commands::set_hit_regions,
            commands::open_settings,
            commands::close_settings,
            commands::open_project,
            commands::open_pets_folder,
            commands::reset_pet_position,
            commands::move_pet_by,
            commands::drag_pet_by,
            commands::show_pet_menu,
            commands::save_pet_position,
            commands::get_diagnostics,
            commands::open_log_folder,
        ])
        .run(context)
        .expect("error while running Perch");
}
