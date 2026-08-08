use crate::{app::*, domain::operator::Operator};
use tauri::State;

#[tauri::command]
pub fn get_packages(app: State<ApplicationServices>) -> Result<Vec<PackageDto>, AppErrorDto> {
    Ok(app.package_service().all())
}
#[tauri::command]
pub fn get_packages_by_operator(
    operator: Operator,
    app: State<ApplicationServices>,
) -> Result<Vec<PackageDto>, AppErrorDto> {
    Ok(app.package_service().by_operator(operator))
}
#[tauri::command]
pub fn get_package_details(
    id: String,
    app: State<ApplicationServices>,
) -> Result<PackageDetailsDto, AppErrorDto> {
    if id.trim().is_empty() {
        return Err(AppErrorDto {
            kind: AppErrorKind::InvalidRequest,
            message: "package id is required".into(),
        });
    }
    app.package_service().details(id)
}
