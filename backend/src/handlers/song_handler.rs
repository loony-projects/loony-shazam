use axum::extract::{Path, State};
use axum::Json;

use crate::error::{AppError, AppResult};
use crate::models::{SongId, SongResponse};
use crate::services::SongService;
use crate::state::AppState;

pub async fn get_song(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<SongResponse>> {
    if id <= 0 {
        return Err(AppError::BadRequest("invalid song id".into()));
    }
    let service = SongService::new(state.songs.clone(), state.redis.clone());
    let song = service.get(SongId(id)).await?;
    Ok(Json(SongResponse::from(song)))
}
