use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::models::SongId;
use crate::services::{AdminService, IngestSongRequest};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct AdminIngestSongRequest {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub duration_ms: i32,
    pub isrc: Option<String>,
    pub artwork_url: Option<String>,
    #[serde(default = "default_source")]
    pub source: String,
    pub algorithm_version: i16,
    #[serde(default)]
    pub fingerprints: Vec<AdminFingerprint>,
}

#[derive(Deserialize)]
pub struct AdminFingerprint {
    pub hash: i64,
    pub offset_ms: i32,
}

fn default_source() -> String {
    "admin_api".to_string()
}

#[derive(Serialize)]
pub struct AdminIngestResponse {
    pub song_id: String,
    pub fingerprints_inserted: u64,
}

/// `POST /api/v1/admin/ingest` — small/API-driven ingest of a song plus its
/// pre-extracted fingerprints. See `services::admin_service` for why this
/// is not the path used for bulk catalog ingestion.
pub async fn admin_ingest(
    State(state): State<AppState>,
    Json(req): Json<AdminIngestSongRequest>,
) -> AppResult<Json<AdminIngestResponse>> {
    if req.fingerprints.len() > 200_000 {
        return Err(AppError::BadRequest(
            "too many fingerprints for the API ingest path; use the ingestion CLI for bulk catalog loads"
                .into(),
        ));
    }

    let service = AdminService::new(state.songs.clone(), state.fingerprints.clone());
    let (song, inserted) = service
        .ingest_song(IngestSongRequest {
            title: req.title,
            artist: req.artist,
            album: req.album,
            album_artist: req.album_artist,
            duration_ms: req.duration_ms,
            isrc: req.isrc,
            artwork_url: req.artwork_url,
            source: req.source,
            algorithm_version: req.algorithm_version,
            fingerprints: req
                .fingerprints
                .into_iter()
                .map(|f| (f.hash, f.offset_ms))
                .collect(),
        })
        .await?;

    Ok(Json(AdminIngestResponse {
        song_id: song.id.to_string(),
        fingerprints_inserted: inserted,
    }))
}

#[derive(Serialize)]
pub struct AdminDeleteFingerprintsResponse {
    pub song_id: String,
    pub fingerprints_deleted: u64,
}

/// `DELETE /api/v1/admin/songs/{id}/fingerprints` — drops all fingerprints
/// for a song without deleting the song row itself, e.g. to re-ingest it
/// under a new algorithm version.
pub async fn admin_delete_song_fingerprints(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<AdminDeleteFingerprintsResponse>> {
    if id <= 0 {
        return Err(AppError::BadRequest("invalid song id".into()));
    }
    let service = AdminService::new(state.songs.clone(), state.fingerprints.clone());
    let deleted = service.delete_song_fingerprints(SongId(id)).await?;
    Ok(Json(AdminDeleteFingerprintsResponse {
        song_id: id.to_string(),
        fingerprints_deleted: deleted,
    }))
}
