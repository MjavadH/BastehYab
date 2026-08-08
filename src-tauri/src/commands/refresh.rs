use crate::{app::*, domain::operator::Operator};
use tauri::State;

#[tauri::command]
pub fn refresh_operator(
    operator: Operator,
    app: State<ApplicationServices>,
) -> Result<RefreshResultDto, AppErrorDto> {
    Ok(app.refresh_service().refresh_operator(operator))
}
#[tauri::command]
pub fn refresh_all_operators(
    app: State<ApplicationServices>,
) -> Result<RefreshResultDto, AppErrorDto> {
    Ok(app.refresh_service().refresh_all())
}
