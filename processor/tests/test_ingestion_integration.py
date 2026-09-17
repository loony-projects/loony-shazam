"""Integration tests against a real Postgres: ingestion idempotency and
the artwork extraction/backfill flow end to end. Requires
TEST_DATABASE_URL (or the default below) to be reachable.
"""

from __future__ import annotations

import os
import uuid

import numpy as np
import psycopg
import pytest
import soundfile as sf
from mutagen.flac import FLAC, Picture

from music_fingerprint.ingestion import Ingestor

DATABASE_URL = os.environ.get(
    "TEST_DATABASE_URL", "postgresql://postgres:devpass@localhost:55432/loony_shazam"
)


def _flac_with_artwork(path, song_index: int, sample_rate: int = 11025) -> None:
    # A random nonce (not just song_index) keeps the file's SHA-256 unique
    # across repeated test runs against a persistent (non-per-test-isolated)
    # database — otherwise a re-run collides with the previous run's
    # "already ingested" ledger entry and every assertion about a *fresh*
    # ingest silently turns into a "skipped_duplicate" instead.
    nonce = uuid.uuid4().hex
    t = np.arange(sample_rate * 3) / sample_rate
    samples = (0.3 * np.sin(2 * np.pi * (300 + song_index * 50) * t)).astype(np.float32)
    sf.write(path, samples, sample_rate, format="FLAC")

    audio = FLAC(path)
    audio["title"] = f"Integration Test Song {song_index} {nonce}"
    audio["artist"] = "Integration Test Artist"
    picture = Picture()
    picture.type = 3
    picture.mime = "image/png"
    picture.data = bytes([137, 80, 78, 71]) + bytes(song_index)  # distinct per song, not a real PNG
    audio.add_picture(picture)
    audio.save()


@pytest.fixture
def db_available():
    try:
        conn = psycopg.connect(DATABASE_URL, connect_timeout=3)
        conn.close()
    except psycopg.OperationalError:
        pytest.skip(f"Postgres not reachable at {DATABASE_URL}")


def test_ingest_extracts_and_stores_artwork(db_available, tmp_path):
    flac_path = tmp_path / "song.flac"
    _flac_with_artwork(flac_path, song_index=1)
    artwork_dir = tmp_path / "artwork"

    with Ingestor(db_url=DATABASE_URL, artwork_dir=artwork_dir) as ingestor:
        outcome = ingestor.ingest_file(flac_path)

    assert outcome.status == "ingested"

    with psycopg.connect(DATABASE_URL, autocommit=True) as conn, conn.cursor() as cur:
        cur.execute("SELECT artwork_url FROM songs WHERE id = %s", (outcome.song_id,))
        (artwork_url,) = cur.fetchone()

    assert artwork_url is not None
    assert artwork_url.startswith("/artwork/")
    saved_file = artwork_dir / artwork_url.removeprefix("/artwork/")
    assert saved_file.exists()
    assert saved_file.read_bytes() == bytes([137, 80, 78, 71]) + bytes(1)


def test_backfill_artwork_updates_existing_song_without_touching_fingerprints(
    db_available, tmp_path
):
    flac_path = tmp_path / "song2.flac"
    _flac_with_artwork(flac_path, song_index=2)
    artwork_dir_1 = tmp_path / "artwork1"
    artwork_dir_2 = tmp_path / "artwork2"

    # Simulate "ingested before artwork extraction existed": ingest with an
    # artwork dir that never gets consulted for backfill, verifying the
    # song currently has no artwork_url.
    with Ingestor(db_url=DATABASE_URL, artwork_dir=artwork_dir_1) as ingestor:
        outcome = ingestor.ingest_file(flac_path)
    assert outcome.status == "ingested"
    original_fingerprint_count = outcome.fingerprint_count

    # Manually clear artwork_url to simulate a pre-artwork-feature row.
    with psycopg.connect(DATABASE_URL, autocommit=True) as conn, conn.cursor() as cur:
        cur.execute("UPDATE songs SET artwork_url = NULL WHERE id = %s", (outcome.song_id,))

    with Ingestor(db_url=DATABASE_URL, artwork_dir=artwork_dir_2) as ingestor:
        status = ingestor.backfill_artwork(flac_path)

    assert status == "updated"

    with psycopg.connect(DATABASE_URL, autocommit=True) as conn, conn.cursor() as cur:
        cur.execute("SELECT artwork_url FROM songs WHERE id = %s", (outcome.song_id,))
        (artwork_url,) = cur.fetchone()
        cur.execute("SELECT count(*) FROM fingerprints WHERE song_id = %s", (outcome.song_id,))
        (fingerprint_count,) = cur.fetchone()

    assert artwork_url is not None
    assert (artwork_dir_2 / artwork_url.removeprefix("/artwork/")).exists()
    # Backfill must never touch fingerprints — only the metadata column.
    assert fingerprint_count == original_fingerprint_count


def test_backfill_artwork_is_a_noop_for_unknown_file(db_available, tmp_path):
    flac_path = tmp_path / "never_ingested.flac"
    _flac_with_artwork(flac_path, song_index=99)

    with Ingestor(db_url=DATABASE_URL, artwork_dir=tmp_path / "artwork") as ingestor:
        status = ingestor.backfill_artwork(flac_path)

    assert status == "not_previously_ingested"
