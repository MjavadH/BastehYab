use serde::Serialize;

pub mod packages;
pub mod recommendations;
pub mod refresh;
pub mod status;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppHealth {
    pub status: &'static str,
    pub app_name: &'static str,
}

#[tauri::command]
pub fn app_health() -> AppHealth {
    AppHealth {
        status: "ok",
        app_name: "BastehYab",
    }
}
