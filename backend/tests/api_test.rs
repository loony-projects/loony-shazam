//! HTTP-layer integration tests: request validation, routing, auth, and
//! response shape — via `tower::ServiceExt::oneshot` against the real
//! Axum router, no network socket needed. Requires Postgres reachable at
//! `TEST_DATABASE_URL` (see tests/common/mod.rs).

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use loony_shazam_backend::config::MatchingConfig;
use loony_shazam_backend::repositories::{FingerprintRepository, NewSong, SongRepository};
use tower::ServiceExt;

async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn health_endpoint_always_ok() {
    let pool = common::setup_test_db().await;
    let state = common::test_app_state(pool, MatchingConfig::default()).await;
    let app = loony_shazam_backend::routes::build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn ready_endpoint_reports_unreachable_processor() {
    let pool = common::setup_test_db().await;
    // test_app_state points at an unreachable processor URL on purpose.
    let state = common::test_app_state(pool, MatchingConfig::default()).await;
    let app = loony_shazam_backend::routes::build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let json = body_json(response).await;
    assert_eq!(json["database"], "ok");
    assert_eq!(json["processor"], "unavailable");
}

#[tokio::test]
async fn get_unknown_song_returns_404_with_no_internal_detail() {
    let pool = common::setup_test_db().await;
    let state = common::test_app_state(pool, MatchingConfig::default()).await;
    let app = loony_shazam_backend::routes::build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/songs/999999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = body_json(response).await;
    assert_eq!(json["error"], "SONG_NOT_FOUND");
    assert!(
        !json.to_string().contains("sql"),
        "error body must not leak SQL detail"
    );
}

#[tokio::test]
async fn get_song_returns_metadata_after_seeding() {
    let pool = common::setup_test_db().await;
    let songs = SongRepository::new(pool.clone());
    let song = songs
        .create(NewSong {
            title: "API Test Song".to_string(),
            artist: "API Test Artist".to_string(),
            album: None,
            album_artist: None,
            duration_ms: 12345,
            isrc: None,
            artwork_url: None,
            source: "test".to_string(),
        })
        .await
        .unwrap();

    let state = common::test_app_state(pool, MatchingConfig::default()).await;
    let app = loony_shazam_backend::routes::build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/songs/{}", song.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["title"], "API Test Song");
    assert_eq!(json["artist"], "API Test Artist");
    assert_eq!(json["duration_ms"], 12345);
}

#[tokio::test]
async fn invalid_song_id_is_bad_request() {
    let pool = common::setup_test_db().await;
    let state = common::test_app_state(pool, MatchingConfig::default()).await;
    let app = loony_shazam_backend::routes::build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/songs/-5")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn recognize_fingerprints_json_recognizes_seeded_song() {
    let pool = common::setup_test_db().await;
    let songs = SongRepository::new(pool.clone());
    let fingerprints = FingerprintRepository::new(pool.clone());

    let song = songs
        .create(NewSong {
            title: "JSON Path Song".to_string(),
            artist: "Artist".to_string(),
            album: None,
            album_artist: None,
            duration_ms: 20000,
            isrc: None,
            artwork_url: None,
            source: "test".to_string(),
        })
        .await
        .unwrap();
    let pairs: Vec<_> = (0..10)
        .map(|i| {
            (
                loony_shazam_backend::models::FingerprintHash(6000 + i),
                loony_shazam_backend::models::OffsetMs(i as i32 * 100),
            )
        })
        .collect();
    fingerprints
        .bulk_insert(
            song.id,
            &pairs,
            loony_shazam_backend::models::AlgorithmVersion(1),
        )
        .await
        .unwrap();

    let state = common::test_app_state(pool, MatchingConfig::default()).await;
    let app = loony_shazam_backend::routes::build_router(state);

    let body = serde_json::json!({
        "algorithm_version": 1,
        "fingerprints": (0..10).map(|i| serde_json::json!({"hash": 6000 + i, "offset_ms": i * 100})).collect::<Vec<_>>()
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/recognitions")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["recognized"], true);
    assert_eq!(json["song"]["title"], "JSON Path Song");
    assert_eq!(json["match"]["algorithm_version"], 1);
}

#[tokio::test]
async fn recognize_fingerprints_unknown_returns_no_match_reason() {
    let pool = common::setup_test_db().await;
    let state = common::test_app_state(pool, MatchingConfig::default()).await;
    let app = loony_shazam_backend::routes::build_router(state);

    let body = serde_json::json!({
        "algorithm_version": 1,
        "fingerprints": (0..10).map(|i| serde_json::json!({"hash": 777000 + i, "offset_ms": i * 100})).collect::<Vec<_>>()
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/recognitions")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["recognized"], false);
    assert_eq!(json["reason"], "NO_MATCH");
}

#[tokio::test]
async fn recognize_fingerprints_response_includes_recognition_id_retrievable_via_get() {
    let pool = common::setup_test_db().await;
    let songs = SongRepository::new(pool.clone());
    let fingerprints = FingerprintRepository::new(pool.clone());

    let song = songs
        .create(NewSong {
            title: "Retrievable Song".to_string(),
            artist: "Artist".to_string(),
            album: None,
            album_artist: None,
            duration_ms: 20000,
            isrc: None,
            artwork_url: None,
            source: "test".to_string(),
        })
        .await
        .unwrap();
    let pairs: Vec<_> = (0..10)
        .map(|i| {
            (
                loony_shazam_backend::models::FingerprintHash(9000 + i),
                loony_shazam_backend::models::OffsetMs(i as i32 * 100),
            )
        })
        .collect();
    fingerprints
        .bulk_insert(
            song.id,
            &pairs,
            loony_shazam_backend::models::AlgorithmVersion(1),
        )
        .await
        .unwrap();

    let state = common::test_app_state(pool, MatchingConfig::default()).await;
    let app = loony_shazam_backend::routes::build_router(state);

    let body = serde_json::json!({
        "algorithm_version": 1,
        "fingerprints": (0..10).map(|i| serde_json::json!({"hash": 9000 + i, "offset_ms": i * 100})).collect::<Vec<_>>()
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/recognitions")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    let recognition_id = json["recognition_id"]
        .as_str()
        .expect("recognition_id present")
        .to_string();

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/recognitions/{recognition_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let record = body_json(response).await;
    assert_eq!(record["id"], recognition_id);
    assert_eq!(record["recognized"], true);
}

#[tokio::test]
async fn recognize_audio_route_is_not_shadowed_by_recognitions_id_route() {
    let pool = common::setup_test_db().await;
    let state = common::test_app_state(pool, MatchingConfig::default()).await;
    let app = loony_shazam_backend::routes::build_router(state);

    // POSTing to /api/v1/recognitions/audio must hit the audio handler
    // (MISSING_AUDIO for an empty body), not be swallowed by the
    // /api/v1/recognitions/:id GET route.
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/recognitions/audio")
                .header("content-type", "multipart/form-data; boundary=X")
                .body(Body::from("--X--\r\n"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = body_json(response).await;
    assert_eq!(json["error"], "MISSING_AUDIO");
}

#[tokio::test]
async fn get_unknown_recognition_returns_404() {
    let pool = common::setup_test_db().await;
    let state = common::test_app_state(pool, MatchingConfig::default()).await;
    let app = loony_shazam_backend::routes::build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/recognitions/999999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn artwork_route_serves_files_from_the_configured_directory() {
    let pool = common::setup_test_db().await;
    let state = common::test_app_state(pool, MatchingConfig::default()).await;
    let artwork_dir = state.config.artwork_dir.clone();
    std::fs::write(
        artwork_dir.join("test-cover.jpg"),
        b"not a real jpeg, just bytes",
    )
    .unwrap();

    let app = loony_shazam_backend::routes::build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/artwork/test-cover.jpg")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&bytes[..], b"not a real jpeg, just bytes");
}

#[tokio::test]
async fn artwork_route_404s_for_unknown_file() {
    let pool = common::setup_test_db().await;
    let state = common::test_app_state(pool, MatchingConfig::default()).await;
    let app = loony_shazam_backend::routes::build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/artwork/does-not-exist.jpg")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn malformed_json_body_is_rejected() {
    let pool = common::setup_test_db().await;
    let state = common::test_app_state(pool, MatchingConfig::default()).await;
    let app = loony_shazam_backend::routes::build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/recognitions")
                .header("content-type", "application/json")
                .body(Body::from("{not valid json"))
                .unwrap(),
        )
        .await
        .unwrap();

    // Axum's `Json` extractor returns 400 for a JSON *syntax* error (vs.
    // 422 for well-formed JSON that fails to deserialize into the expected
    // shape) — this exercises the same "malformed request" validation path.
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn admin_endpoint_requires_bearer_token() {
    let pool = common::setup_test_db().await;
    let state = common::test_app_state(pool, MatchingConfig::default()).await;
    let app = loony_shazam_backend::routes::build_router(state);

    let body = serde_json::json!({
        "title": "Admin Song", "artist": "Admin Artist", "duration_ms": 1000,
        "algorithm_version": 1, "fingerprints": []
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/ingest")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_endpoint_succeeds_with_correct_token() {
    let pool = common::setup_test_db().await;
    let state = common::test_app_state(pool, MatchingConfig::default()).await;
    let app = loony_shazam_backend::routes::build_router(state);

    let body = serde_json::json!({
        "title": "Admin Song", "artist": "Admin Artist", "duration_ms": 1000,
        "algorithm_version": 1,
        "fingerprints": [{"hash": 8000, "offset_ms": 0}, {"hash": 8001, "offset_ms": 100}]
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/ingest")
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-admin-key")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["fingerprints_inserted"], 2);
}

#[tokio::test]
async fn recognize_audio_without_audio_field_is_rejected() {
    let pool = common::setup_test_db().await;
    let state = common::test_app_state(pool, MatchingConfig::default()).await;
    let app = loony_shazam_backend::routes::build_router(state);

    let boundary = "X-BOUNDARY";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"not_audio\"\r\n\r\nhello\r\n--{boundary}--\r\n"
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/recognitions/audio")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = body_json(response).await;
    assert_eq!(json["error"], "MISSING_AUDIO");
}
