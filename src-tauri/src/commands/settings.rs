use tauri::State;

use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub async fn get_server_url(state: State<'_, AppState>) -> Result<String, AppError> {
    Ok(state.api.read().await.base_url().to_string())
}

#[tauri::command]
pub async fn set_server_url(state: State<'_, AppState>, url: String) -> Result<(), AppError> {
    state.api.write().await.set_base_url(url);
    Ok(())
}
