use crate::app::*;
use tauri::State;

#[tauri::command]
pub fn get_recommendations(
    context: RecommendationContextDto,
    app: State<ApplicationServices>,
) -> Result<Vec<RecommendationSetDto>, AppErrorDto> {
    Ok(app.recommendation_service().all(context))
}
#[tauri::command]
pub fn get_recommendations_by_strategy(
    strategy: RecommendationStrategyDto,
    context: RecommendationContextDto,
    app: State<ApplicationServices>,
) -> Result<RecommendationSetDto, AppErrorDto> {
    Ok(app.recommendation_service().by_strategy(strategy, context))
}
