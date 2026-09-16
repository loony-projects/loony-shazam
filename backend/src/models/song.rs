use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;

/// Strongly-typed song primary key. Newtype over `i64` (the `BIGSERIAL`
/// column) so it can never be accidentally passed where a fingerprint hash
/// or offset is expected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct SongId(pub i64);

impl std::fmt::Display for SongId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct DurationMs(pub i32);

// release_date/created_at/updated_at are selected from the DB (FromRow
// needs the full row) but not yet surfaced through the public API — see
// SongResponse below, which is deliberately narrower.
#[allow(dead_code)]
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Song {
    pub id: SongId,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub duration_ms: DurationMs,
    pub release_date: Option<NaiveDate>,
    pub isrc: Option<String>,
    pub artwork_url: Option<String>,
    pub source: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Public API representation of a song — deliberately narrower than the DB
/// row (no internal timestamps/source bookkeeping leaked to clients).
#[derive(Debug, Clone, Serialize)]
pub struct SongResponse {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: i32,
    pub artwork_url: Option<String>,
}

impl From<Song> for SongResponse {
    fn from(song: Song) -> Self {
        Self {
            id: song.id.to_string(),
            title: song.title,
            artist: song.artist,
            album: song.album,
            duration_ms: song.duration_ms.0,
            artwork_url: song.artwork_url,
        }
    }
}
