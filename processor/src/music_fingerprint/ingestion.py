"""Idempotent, batched catalog ingestion: walk a directory of licensed/local
audio, extract fingerprints, and bulk-load them into PostgreSQL.

Idempotency: identity is the SHA-256 of the file's bytes (not filename or
mtime, which are unreliable), tracked in `ingested_sources`. Re-running
ingestion over the same directory never re-inserts a file it has already
processed successfully.

Bulk insert: fingerprints are loaded with `COPY`, never row-by-row INSERT —
see docs/database.md for why this matters at catalog scale.
"""

from __future__ import annotations

import hashlib
import logging
from collections.abc import Iterable
from dataclasses import dataclass
from pathlib import Path

import psycopg
from mutagen import File as MutagenFile

from music_fingerprint.config import FINGERPRINT_ALGORITHM_VERSION, FingerprintConfig
from music_fingerprint.extraction import DEFAULT_MAX_INGEST_BYTES, extract_from_file
from music_fingerprint.validation import AudioValidationError

log = logging.getLogger(__name__)

SUPPORTED_EXTENSIONS = {".wav", ".flac", ".mp3", ".m4a", ".aac", ".ogg"}


@dataclass(frozen=True, slots=True)
class SongMetadata:
    title: str
    artist: str
    album: str | None = None
    album_artist: str | None = None
    isrc: str | None = None
    artwork_url: str | None = None
    source: str = "local"


@dataclass(frozen=True, slots=True)
class IngestOutcome:
    path: Path
    status: str  # "ingested" | "skipped_duplicate" | "failed"
    song_id: int | None = None
    fingerprint_count: int = 0
    error: str | None = None


def discover_audio_files(directory: Path) -> list[Path]:
    return sorted(
        p for p in directory.rglob("*") if p.is_file() and p.suffix.lower() in SUPPORTED_EXTENSIONS
    )


def compute_sha256(path: Path, chunk_size: int = 1024 * 1024) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as f:
        while chunk := f.read(chunk_size):
            digest.update(chunk)
    return digest.hexdigest()


def extract_metadata(path: Path) -> SongMetadata:
    """Best-effort tag extraction; falls back to filename parsing.

    Never trusts tags for anything security-relevant (they only ever
    populate display metadata columns via parameterized queries).
    """
    title, artist, album, album_artist = None, None, None, None
    try:
        tags = MutagenFile(path, easy=True)
        if tags is not None:
            title = _first(tags.get("title"))
            artist = _first(tags.get("artist"))
            album = _first(tags.get("album"))
            album_artist = _first(tags.get("albumartist"))
    except Exception as exc:  # noqa: BLE001 - tag parsing is best-effort only
        log.debug("tag_read_failed", extra={"path": str(path), "error": str(exc)})

    if not title or not artist:
        stem = path.stem
        if " - " in stem:
            parsed_artist, parsed_title = stem.split(" - ", 1)
            artist = artist or parsed_artist.strip()
            title = title or parsed_title.strip()
        else:
            title = title or stem
            artist = artist or "Unknown Artist"

    return SongMetadata(title=title, artist=artist, album=album, album_artist=album_artist)


def _first(values: list[str] | None) -> str | None:
    return values[0] if values else None


class Ingestor:
    """Owns a single DB connection/transaction scope for a batch ingestion
    run. Not thread-safe; run multiple workers with separate instances if
    parallelism is needed."""

    def __init__(
        self,
        db_url: str,
        config: FingerprintConfig | None = None,
        algorithm_version: int = FINGERPRINT_ALGORITHM_VERSION,
        max_bytes: int = DEFAULT_MAX_INGEST_BYTES,
    ) -> None:
        self.config = config or FingerprintConfig()
        self.algorithm_version = algorithm_version
        self.max_bytes = max_bytes
        # autocommit=True: reads (already_ingested) never leave a dangling
        # open transaction; the actual write group below opens its own
        # real transaction via `with self.conn.transaction():`.
        self.conn = psycopg.connect(db_url, autocommit=True)

    def close(self) -> None:
        self.conn.close()

    def __enter__(self) -> Ingestor:
        return self

    def __exit__(self, *exc_info: object) -> None:
        self.close()

    def already_ingested(self, sha256: str) -> bool:
        with self.conn.cursor() as cur:
            cur.execute("SELECT 1 FROM ingested_sources WHERE sha256 = %s", (sha256,))
            return cur.fetchone() is not None

    def _insert_song(self, metadata: SongMetadata, duration_ms: int) -> int:
        with self.conn.cursor() as cur:
            cur.execute(
                """
                INSERT INTO songs
                    (title, artist, album, album_artist, duration_ms, isrc, artwork_url, source)
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s)
                RETURNING id
                """,
                (
                    metadata.title,
                    metadata.artist,
                    metadata.album,
                    metadata.album_artist,
                    duration_ms,
                    metadata.isrc,
                    metadata.artwork_url,
                    metadata.source,
                ),
            )
            row = cur.fetchone()
            assert row is not None
            return row[0]

    def _bulk_insert_fingerprints(
        self, song_id: int, fingerprints: Iterable[tuple[int, int]]
    ) -> int:
        count = 0
        with (
            self.conn.cursor() as cur,
            cur.copy(
                "COPY fingerprints (hash, song_id, offset_ms, algorithm_version) FROM STDIN"
            ) as copy,
        ):
            for hash_value, offset_ms in fingerprints:
                copy.write_row((hash_value, song_id, offset_ms, self.algorithm_version))
                count += 1
        return count

    def _record_source(self, sha256: str, path: Path, song_id: int, fingerprint_count: int) -> None:
        with self.conn.cursor() as cur:
            cur.execute(
                """
                INSERT INTO ingested_sources
                    (sha256, file_path, song_id, algorithm_version, fingerprint_count)
                VALUES (%s, %s, %s, %s, %s)
                """,
                (sha256, str(path), song_id, self.algorithm_version, fingerprint_count),
            )

    def ingest_file(self, path: Path, metadata: SongMetadata | None = None) -> IngestOutcome:
        try:
            sha256 = compute_sha256(path)
            if self.already_ingested(sha256):
                return IngestOutcome(path=path, status="skipped_duplicate")

            result = extract_from_file(path, config=self.config, max_bytes=self.max_bytes)
            meta = metadata or extract_metadata(path)

            # One real transaction per file: song + fingerprints + source
            # ledger commit atomically together, so a crash partway through
            # a large ingestion run never leaves a song without its
            # fingerprints or an untracked partially-ingested file. Already
            # -committed files are durably recorded, so the next run
            # resumes cleanly via `already_ingested` instead of redoing them.
            with self.conn.transaction():
                song_id = self._insert_song(meta, result.duration_ms)
                count = self._bulk_insert_fingerprints(
                    song_id, ((f.hash, f.offset_ms) for f in result.fingerprints)
                )
                self._record_source(sha256, path, song_id, count)

            return IngestOutcome(
                path=path, status="ingested", song_id=song_id, fingerprint_count=count
            )
        except AudioValidationError as exc:
            return IngestOutcome(path=path, status="failed", error=str(exc))
        except Exception as exc:  # noqa: BLE001 - one bad file must not abort the whole run
            log.exception("ingest_failed", extra={"path": str(path)})
            return IngestOutcome(path=path, status="failed", error=str(exc))

    def ingest_directory(self, directory: Path, progress_every: int = 10) -> list[IngestOutcome]:
        files = discover_audio_files(directory)
        outcomes: list[IngestOutcome] = []
        for i, path in enumerate(files, start=1):
            outcome = self.ingest_file(path)
            outcomes.append(outcome)
            if i % progress_every == 0 or i == len(files):
                ingested = sum(1 for o in outcomes if o.status == "ingested")
                skipped = sum(1 for o in outcomes if o.status == "skipped_duplicate")
                failed = sum(1 for o in outcomes if o.status == "failed")
                log.info(
                    "ingestion_progress",
                    extra={
                        "processed": i,
                        "total": len(files),
                        "ingested": ingested,
                        "skipped_duplicate": skipped,
                        "failed": failed,
                    },
                )
        return outcomes
