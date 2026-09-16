//! Matching-engine latency benchmark at various catalog sizes. Seeds
//! synthetic fingerprint data directly (bypassing DSP — this benchmarks
//! the lookup/voting/scoring path, not the Python extraction pipeline,
//! which is benchmarked separately in `processor/benchmarks`) via `COPY`,
//! then times `MatchingEngine::recognize` over repeated synthetic queries.
//!
//! Run with: `DATABASE_URL=... cargo run --release --example bench_matching`

use std::sync::Arc;
use std::time::Instant;

use loony_shazam_backend::config::MatchingConfig;
use loony_shazam_backend::matching::{MatchingEngine, PostgresFingerprintIndex, QueryFingerprint};
use loony_shazam_backend::models::{AlgorithmVersion, FingerprintHash, OffsetMs};
use loony_shazam_backend::repositories::{FingerprintRepository, NewSong, SongRepository};
use loony_shazam_backend::storage;

const FINGERPRINTS_PER_SONG: usize = 4000; // representative of a ~3-4 minute track
const QUERY_FINGERPRINT_COUNT: usize = 300; // representative of an ~8s query clip
const QUERIES_PER_CATALOG_SIZE: usize = 30;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:devpass@localhost:55432/loony_shazam_bench".to_string()
    });

    // Fresh database so repeated runs don't accumulate stale rows.
    let admin_url = database_url
        .rsplit_once('/')
        .map(|(base, _)| format!("{base}/postgres"));
    if let Some(admin_url) = admin_url {
        if let Ok(admin_pool) = sqlx::PgPool::connect(&admin_url).await {
            let db_name = database_url
                .rsplit('/')
                .next()
                .unwrap_or("loony_shazam_bench");
            let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{db_name}\""))
                .execute(&admin_pool)
                .await;
            sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
                .execute(&admin_pool)
                .await?;
        }
    }

    let pool = storage::create_pool(&database_url).await?;
    storage::run_migrations(&pool).await?;

    let songs = SongRepository::new(pool.clone());
    let fingerprints = FingerprintRepository::new(pool.clone());

    println!("catalog_songs,seed_time_s,p50_ms,p95_ms,p99_ms,recognized_count");

    // One continuing stream for the whole run (not reset per tier!) — every
    // song across every tier gets a distinct hash sequence. Resetting this
    // per tier was an earlier bug: it made tier N's first song byte-
    // identical to tier N-1's first song, and since the database
    // accumulates catalog across tiers (see below), that produced a
    // genuine duplicate-song ambiguity (AmbiguousMargin), not a benchmark
    // artifact — a good reminder that "random-looking" test data still
    // needs the same rigor as production data.
    let mut rng_state: u64 = 0x9E3779B97F4A7C15;

    let tiers: Vec<usize> = std::env::var("BENCH_TIERS")
        .ok()
        .map(|s| s.split(',').map(|t| t.parse().unwrap()).collect())
        .unwrap_or_else(|| vec![10, 100, 1_000, 10_000]);
    for &catalog_size in &tiers {
        let seed_start = Instant::now();
        let mut known_song_hashes: Vec<(i64, Vec<i64>)> = Vec::new();

        for i in 0..catalog_size {
            let song = songs
                .create(NewSong {
                    title: format!("Bench Song {i}"),
                    artist: "Bench Artist".to_string(),
                    album: None,
                    album_artist: None,
                    duration_ms: 200_000,
                    isrc: None,
                    artwork_url: None,
                    source: "bench".to_string(),
                })
                .await?;

            let mut hashes = Vec::with_capacity(FINGERPRINTS_PER_SONG);
            let pairs: Vec<(FingerprintHash, OffsetMs)> = (0..FINGERPRINTS_PER_SONG)
                .map(|j| {
                    rng_state ^= rng_state << 13;
                    rng_state ^= rng_state >> 7;
                    rng_state ^= rng_state << 17;
                    // Xorshift64's low-order bits have known statistical
                    // weaknesses (short cycles/correlation) — take the
                    // high bits instead, which are much closer to uniform,
                    // to avoid artificially inflated hash collisions
                    // between unrelated synthetic "songs" in this benchmark.
                    let hash = ((rng_state >> 36) & 0x0FFF_FFFF) as i64;
                    hashes.push(hash);
                    (FingerprintHash(hash), OffsetMs((j as i32) * 50))
                })
                .collect();

            fingerprints
                .bulk_insert(song.id, &pairs, AlgorithmVersion(1))
                .await?;
            if i < 20 {
                known_song_hashes.push((song.id.0, hashes));
            }
        }
        let seed_time_s = seed_start.elapsed().as_secs_f64();

        let index = Arc::new(PostgresFingerprintIndex::new(pool.clone()));
        let engine = MatchingEngine::new(index, songs.clone(), MatchingConfig::default());

        let mut latencies_ms = Vec::with_capacity(QUERIES_PER_CATALOG_SIZE);
        let mut recognized_count = 0u32;

        for q in 0..QUERIES_PER_CATALOG_SIZE {
            let (song_id, hashes) = &known_song_hashes[q % known_song_hashes.len()];
            let query: Vec<QueryFingerprint> = hashes
                .iter()
                .take(QUERY_FINGERPRINT_COUNT)
                .enumerate()
                .map(|(i, &h)| QueryFingerprint {
                    hash: FingerprintHash(h),
                    offset_ms: OffsetMs((i as i32) * 50),
                })
                .collect();

            let started = Instant::now();
            let outcome = engine
                .recognize(query, AlgorithmVersion(1), Instant::now())
                .await?;
            latencies_ms.push(started.elapsed().as_secs_f64() * 1000.0);
            if outcome.recognized() && outcome.song.as_ref().map(|s| s.id.0) == Some(*song_id) {
                recognized_count += 1;
            } else if std::env::var("BENCH_DEBUG").is_ok() {
                eprintln!(
                    "MISS expected_song={} recognized={} got_song={:?} reason={:?} score={} latency_ms={} matched_fp={} query_fp={}",
                    song_id,
                    outcome.recognized(),
                    outcome.song.as_ref().map(|s| s.id.0),
                    outcome.reason,
                    outcome.score,
                    outcome.latency_ms,
                    outcome.matched_fingerprints,
                    outcome.query_fingerprints,
                );
            }
        }

        latencies_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p50 = percentile(&latencies_ms, 0.50);
        let p95 = percentile(&latencies_ms, 0.95);
        let p99 = percentile(&latencies_ms, 0.99);

        println!(
            "{catalog_size},{seed_time_s:.2},{p50:.2},{p95:.2},{p99:.2},{recognized_count}/{QUERIES_PER_CATALOG_SIZE}"
        );
    }

    Ok(())
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}
