pub mod cache;
pub mod collectors;
pub mod commands;
pub mod domain;
pub mod error;
pub mod filtering;
pub mod normalizers;
pub mod recommendations;
pub mod refresh;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::app_health])
        .run(tauri::generate_context!())
        .expect("failed to run BastehYab desktop application");
}
