use crate::app::*;
use tauri::State;

#[tauri::command]
pub fn apply_package_filters(
    query: PackageQueryDto,
    app: State<ApplicationServices>,
) -> Result<Vec<PackageDto>, AppErrorDto> {
    Ok(app.filter_service().apply(query))
}
#[tauri::command]
pub fn search_packages(
    text: String,
    app: State<ApplicationServices>,
) -> Result<Vec<PackageDto>, AppErrorDto> {
    if text.trim().is_empty() {
        return Err(AppErrorDto {
            kind: AppErrorKind::InvalidRequest,
            message: "search text is required".into(),
        });
    }
    Ok(app.filter_service().search(text))
}
#[tauri::command]
pub fn sort_packages(
    sort: PackageSortDto,
    app: State<ApplicationServices>,
) -> Result<Vec<PackageDto>, AppErrorDto> {
    Ok(app.filter_service().sort(sort))
}
#[tauri::command]
pub fn get_cache_status(app: State<ApplicationServices>) -> Result<CacheStatusDto, AppErrorDto> {
    Ok(cache_status(&app.state))
}
