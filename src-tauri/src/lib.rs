mod events;
mod hooks_installer;
mod locator;
mod money_guard;
mod normalize;
mod sessions;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running Perch");
}
