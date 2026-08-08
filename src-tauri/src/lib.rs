pub mod app;
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
        .setup(|app| {
            let cache = cache::CacheStore::for_app(app.handle())?;
            app.manage(app::ApplicationServices::initialize(cache));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_health,
            commands::packages::get_packages,
            commands::packages::get_packages_by_operator,
            commands::packages::get_package_details,
            commands::refresh::refresh_operator,
            commands::refresh::refresh_all_operators,
            commands::recommendations::get_recommendations,
            commands::recommendations::get_recommendations_by_strategy,
            commands::status::apply_package_filters,
            commands::status::search_packages,
            commands::status::sort_packages,
            commands::status::get_cache_status
        ])
        .run(tauri::generate_context!())
        .expect("failed to run BastehYab desktop application");
}
