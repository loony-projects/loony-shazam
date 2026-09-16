use crate::error::AppError;
use crate::models::{AlgorithmVersion, FingerprintHash, OffsetMs, Song, SongId};
use crate::repositories::{FingerprintRepository, NewSong, SongRepository};

/// Request body for the admin ingest endpoint — a small/API-driven path for
/// adding a handful of songs at a time (e.g. a demo catalog or a
/// single-song correction). Bulk catalog ingestion (thousands+ of songs)
/// goes through the Python CLI's direct-to-Postgres `COPY` path instead
/// (see docs/ingestion.md) — round-tripping millions of fingerprint rows
/// through JSON-over-HTTP would be needlessly slow and memory-heavy.
pub struct IngestSongRequest {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub duration_ms: i32,
    pub isrc: Option<String>,
    pub artwork_url: Option<String>,
    pub source: String,
    pub algorithm_version: i16,
    pub fingerprints: Vec<(i64, i32)>, // (hash, offset_ms)
}

pub struct AdminService {
    songs: SongRepository,
    fingerprints: FingerprintRepository,
}

impl AdminService {
    pub fn new(songs: SongRepository, fingerprints: FingerprintRepository) -> Self {
        Self {
            songs,
            fingerprints,
        }
    }

    pub async fn ingest_song(&self, req: IngestSongRequest) -> Result<(Song, u64), AppError> {
        if req.title.trim().is_empty() || req.artist.trim().is_empty() {
            return Err(AppError::BadRequest("title and artist are required".into()));
        }
        if req.duration_ms <= 0 {
            return Err(AppError::BadRequest("duration_ms must be positive".into()));
        }

        let song = self
            .songs
            .create(NewSong {
                title: req.title,
                artist: req.artist,
                album: req.album,
                album_artist: req.album_artist,
                duration_ms: req.duration_ms,
                isrc: req.isrc,
                artwork_url: req.artwork_url,
                source: req.source,
            })
            .await?;

        let pairs: Vec<(FingerprintHash, OffsetMs)> = req
            .fingerprints
            .into_iter()
            .map(|(h, o)| (FingerprintHash(h), OffsetMs(o)))
            .collect();

        let inserted = self
            .fingerprints
            .bulk_insert(song.id, &pairs, AlgorithmVersion(req.algorithm_version))
            .await?;

        Ok((song, inserted))
    }

    pub async fn delete_song_fingerprints(&self, song_id: SongId) -> Result<u64, AppError> {
        self.fingerprints.delete_for_song(song_id).await
    }
}
