use sqlx::PgPool;

use crate::error::AppError;
use crate::models::{Song, SongId};

#[derive(Debug, Clone)]
pub struct NewSong {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub duration_ms: i32,
    pub isrc: Option<String>,
    pub artwork_url: Option<String>,
    pub source: String,
}

#[derive(Clone)]
pub struct SongRepository {
    pool: PgPool,
}

impl SongRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_by_id(&self, id: SongId) -> Result<Option<Song>, AppError> {
        let started = std::time::Instant::now();
        let song = sqlx::query_as::<_, Song>(
            r#"
            SELECT id, title, artist, album, album_artist, duration_ms,
                   release_date, isrc, artwork_url, source, created_at, updated_at
            FROM songs
            WHERE id = $1
            "#,
        )
        .bind(id.0)
        .fetch_optional(&self.pool)
        .await?;
        metrics::histogram!(crate::telemetry::metric_names::DB_QUERY_LATENCY, "query" => "songs.find_by_id")
            .record(started.elapsed().as_millis() as f64);
        Ok(song)
    }

    pub async fn create(&self, new_song: NewSong) -> Result<Song, AppError> {
        let song = sqlx::query_as::<_, Song>(
            r#"
            INSERT INTO songs (title, artist, album, album_artist, duration_ms, isrc, artwork_url, source)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, title, artist, album, album_artist, duration_ms,
                      release_date, isrc, artwork_url, source, created_at, updated_at
            "#,
        )
        .bind(new_song.title)
        .bind(new_song.artist)
        .bind(new_song.album)
        .bind(new_song.album_artist)
        .bind(new_song.duration_ms)
        .bind(new_song.isrc)
        .bind(new_song.artwork_url)
        .bind(new_song.source)
        .fetch_one(&self.pool)
        .await?;
        Ok(song)
    }
}
