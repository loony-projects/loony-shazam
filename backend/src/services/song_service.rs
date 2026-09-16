use crate::error::AppError;
use crate::models::{Song, SongId};
use crate::repositories::SongRepository;
use crate::storage::RedisCache;

pub struct SongService {
    songs: SongRepository,
    cache: RedisCache,
}

impl SongService {
    pub fn new(songs: SongRepository, cache: RedisCache) -> Self {
        Self { songs, cache }
    }

    pub async fn get(&self, id: SongId) -> Result<Song, AppError> {
        let cache_key = format!("song:{}", id.0);
        // Metadata cache only — never consulted for fingerprint lookup
        // (see docs/database.md "Redis usage"). A cache miss or a disabled
        // cache both fall through to Postgres transparently.
        if let Some(cached) = self.cache.get_json::<CachedSong>(&cache_key).await {
            return Ok(cached.into());
        }

        let song = self
            .songs
            .find_by_id(id)
            .await?
            .ok_or(AppError::SongNotFound)?;
        self.cache
            .set_json(&cache_key, &CachedSong::from(song.clone()), 300)
            .await;
        Ok(song)
    }
}

/// Cache representation kept separate from the DB row type so cache-schema
/// changes never require a migration and vice versa.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct CachedSong {
    id: i64,
    title: String,
    artist: String,
    album: Option<String>,
    album_artist: Option<String>,
    duration_ms: i32,
    isrc: Option<String>,
    artwork_url: Option<String>,
    source: String,
}

impl From<Song> for CachedSong {
    fn from(s: Song) -> Self {
        Self {
            id: s.id.0,
            title: s.title,
            artist: s.artist,
            album: s.album,
            album_artist: s.album_artist,
            duration_ms: s.duration_ms.0,
            isrc: s.isrc,
            artwork_url: s.artwork_url,
            source: s.source,
        }
    }
}

impl From<CachedSong> for Song {
    fn from(c: CachedSong) -> Self {
        Self {
            id: SongId(c.id),
            title: c.title,
            artist: c.artist,
            album: c.album,
            album_artist: c.album_artist,
            duration_ms: crate::models::DurationMs(c.duration_ms),
            release_date: None,
            isrc: c.isrc,
            artwork_url: c.artwork_url,
            source: c.source,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }
}
