//! Integration tests against a real, freshly-migrated Postgres database:
//! repository layer, fingerprint bulk insert + lookup, offset-histogram
//! voting, ranking, and unknown-result detection end-to-end through
//! `MatchingEngine`. Requires Postgres reachable at `TEST_DATABASE_URL`
//! (see tests/common/mod.rs for the default).

mod common;

use loony_shazam_backend::config::MatchingConfig;
use loony_shazam_backend::matching::QueryFingerprint;
use loony_shazam_backend::models::{AlgorithmVersion, FingerprintHash, OffsetMs};
use loony_shazam_backend::repositories::{FingerprintRepository, NewSong, SongRepository};

async fn seed_song(
    songs: &SongRepository,
    fingerprints: &FingerprintRepository,
    title: &str,
    pairs: &[(i64, i32)],
) -> loony_shazam_backend::models::SongId {
    let song = songs
        .create(NewSong {
            title: title.to_string(),
            artist: "Test Artist".to_string(),
            album: None,
            album_artist: None,
            duration_ms: 20_000,
            isrc: None,
            artwork_url: None,
            source: "test".to_string(),
        })
        .await
        .expect("create song");

    let typed: Vec<_> = pairs
        .iter()
        .map(|&(h, o)| (FingerprintHash(h), OffsetMs(o)))
        .collect();
    fingerprints
        .bulk_insert(song.id, &typed, AlgorithmVersion(1))
        .await
        .expect("bulk insert fingerprints");

    song.id
}

#[tokio::test]
async fn recognizes_song_with_consistent_offset() {
    let pool = common::setup_test_db().await;
    let songs = SongRepository::new(pool.clone());
    let fingerprints = FingerprintRepository::new(pool.clone());

    // Reference: 10 landmarks starting at t=5000ms, spaced 100ms apart.
    let reference: Vec<(i64, i32)> = (0..10).map(|i| (1000 + i, 5000 + i as i32 * 100)).collect();
    seed_song(&songs, &fingerprints, "Song A", &reference).await;

    let index = std::sync::Arc::new(
        loony_shazam_backend::matching::PostgresFingerprintIndex::new(pool.clone()),
    );
    let engine = loony_shazam_backend::matching::MatchingEngine::new(
        index,
        songs.clone(),
        MatchingConfig::default(),
    );

    // Query: same 10 hashes, but starting at query-offset 0 (i.e. the clip
    // was trimmed to start exactly at the reference's landmark region) —
    // every match should agree on offset = 5000ms.
    let query: Vec<QueryFingerprint> = (0..10)
        .map(|i| QueryFingerprint {
            hash: FingerprintHash(1000 + i),
            offset_ms: OffsetMs(i as i32 * 100),
        })
        .collect();

    let outcome = engine
        .recognize(query, AlgorithmVersion(1), std::time::Instant::now())
        .await
        .expect("recognize should not error");

    assert!(
        outcome.recognized(),
        "expected a match, got {:?}",
        outcome.reason
    );
    assert_eq!(outcome.song.unwrap().title, "Song A");
    assert_eq!(outcome.offset_ms, Some(5000));
    assert_eq!(outcome.matched_fingerprints, 10);
}

#[tokio::test]
async fn picks_correct_song_among_multiple_candidates() {
    let pool = common::setup_test_db().await;
    let songs = SongRepository::new(pool.clone());
    let fingerprints = FingerprintRepository::new(pool.clone());

    let song_a: Vec<(i64, i32)> = (0..8).map(|i| (2000 + i, i as i32 * 100)).collect();
    let song_b: Vec<(i64, i32)> = (0..3).map(|i| (2000 + i, i as i32 * 100)).collect(); // shares 3 hashes with A

    seed_song(&songs, &fingerprints, "Song A", &song_a).await;
    seed_song(&songs, &fingerprints, "Song B", &song_b).await;

    let index = std::sync::Arc::new(
        loony_shazam_backend::matching::PostgresFingerprintIndex::new(pool.clone()),
    );
    let engine = loony_shazam_backend::matching::MatchingEngine::new(
        index,
        songs.clone(),
        MatchingConfig::default(),
    );

    let query: Vec<QueryFingerprint> = (0..8)
        .map(|i| QueryFingerprint {
            hash: FingerprintHash(2000 + i),
            offset_ms: OffsetMs(i as i32 * 100),
        })
        .collect();

    let outcome = engine
        .recognize(query, AlgorithmVersion(1), std::time::Instant::now())
        .await
        .expect("recognize should not error");

    assert!(outcome.recognized());
    assert_eq!(outcome.song.unwrap().title, "Song A");
}

#[tokio::test]
async fn unrelated_query_returns_not_recognized() {
    let pool = common::setup_test_db().await;
    let songs = SongRepository::new(pool.clone());
    let fingerprints = FingerprintRepository::new(pool.clone());

    let reference: Vec<(i64, i32)> = (0..10).map(|i| (3000 + i, i as i32 * 100)).collect();
    seed_song(&songs, &fingerprints, "Song A", &reference).await;

    let index = std::sync::Arc::new(
        loony_shazam_backend::matching::PostgresFingerprintIndex::new(pool.clone()),
    );
    let engine = loony_shazam_backend::matching::MatchingEngine::new(
        index,
        songs.clone(),
        MatchingConfig::default(),
    );

    // Hashes that don't exist in the index at all.
    let query: Vec<QueryFingerprint> = (0..10)
        .map(|i| QueryFingerprint {
            hash: FingerprintHash(999_000 + i),
            offset_ms: OffsetMs(i as i32 * 100),
        })
        .collect();

    let outcome = engine
        .recognize(query, AlgorithmVersion(1), std::time::Instant::now())
        .await
        .expect("recognize should not error");

    assert!(!outcome.recognized());
    assert!(outcome.song.is_none());
}

#[tokio::test]
async fn empty_query_is_not_recognized_without_error() {
    let pool = common::setup_test_db().await;
    let songs = SongRepository::new(pool.clone());
    let index = std::sync::Arc::new(
        loony_shazam_backend::matching::PostgresFingerprintIndex::new(pool.clone()),
    );
    let engine = loony_shazam_backend::matching::MatchingEngine::new(
        index,
        songs,
        MatchingConfig::default(),
    );

    let outcome = engine
        .recognize(vec![], AlgorithmVersion(1), std::time::Instant::now())
        .await
        .expect("recognize should not error on empty query");

    assert!(!outcome.recognized());
    assert_eq!(outcome.query_fingerprints, 0);
}

#[tokio::test]
async fn below_minimum_votes_is_not_recognized() {
    let pool = common::setup_test_db().await;
    let songs = SongRepository::new(pool.clone());
    let fingerprints = FingerprintRepository::new(pool.clone());

    // Only 2 shared landmarks, well under the default min_dominant_votes (5).
    let reference: Vec<(i64, i32)> = vec![(4000, 0), (4001, 100)];
    seed_song(&songs, &fingerprints, "Song A", &reference).await;

    let index = std::sync::Arc::new(
        loony_shazam_backend::matching::PostgresFingerprintIndex::new(pool.clone()),
    );
    let engine = loony_shazam_backend::matching::MatchingEngine::new(
        index,
        songs.clone(),
        MatchingConfig::default(),
    );

    let query = vec![
        QueryFingerprint {
            hash: FingerprintHash(4000),
            offset_ms: OffsetMs(0),
        },
        QueryFingerprint {
            hash: FingerprintHash(4001),
            offset_ms: OffsetMs(100),
        },
    ];

    let outcome = engine
        .recognize(query, AlgorithmVersion(1), std::time::Instant::now())
        .await
        .expect("recognize should not error");

    assert!(!outcome.recognized());
}

#[tokio::test]
async fn algorithm_version_mismatch_does_not_match_across_versions() {
    let pool = common::setup_test_db().await;
    let songs = SongRepository::new(pool.clone());
    let fingerprints = FingerprintRepository::new(pool.clone());

    // Same hash values, but stored under algorithm_version=2.
    let reference: Vec<(i64, i32)> = (0..10).map(|i| (5000 + i, i as i32 * 100)).collect();
    let song = songs
        .create(NewSong {
            title: "Versioned Song".to_string(),
            artist: "Test".to_string(),
            album: None,
            album_artist: None,
            duration_ms: 20_000,
            isrc: None,
            artwork_url: None,
            source: "test".to_string(),
        })
        .await
        .unwrap();
    let typed: Vec<_> = reference
        .iter()
        .map(|&(h, o)| (FingerprintHash(h), OffsetMs(o)))
        .collect();
    fingerprints
        .bulk_insert(song.id, &typed, AlgorithmVersion(2))
        .await
        .unwrap();

    let index = std::sync::Arc::new(
        loony_shazam_backend::matching::PostgresFingerprintIndex::new(pool.clone()),
    );
    let engine = loony_shazam_backend::matching::MatchingEngine::new(
        index,
        songs.clone(),
        MatchingConfig::default(),
    );

    // Query under algorithm_version=1 (a mismatch).
    let query: Vec<QueryFingerprint> = (0..10)
        .map(|i| QueryFingerprint {
            hash: FingerprintHash(5000 + i),
            offset_ms: OffsetMs(i as i32 * 100),
        })
        .collect();

    let outcome = engine
        .recognize(query, AlgorithmVersion(1), std::time::Instant::now())
        .await
        .expect("recognize should not error");

    assert!(
        !outcome.recognized(),
        "fingerprints under a different algorithm_version must not match"
    );
}
