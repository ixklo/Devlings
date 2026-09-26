fn main() {
    // Pet art ships as a bundle resource. An empty folder keeps checkouts without art building.
    let _ = std::fs::create_dir_all("pets");
    println!("cargo:rerun-if-changed=pets");
    tauri_build::build()
}
