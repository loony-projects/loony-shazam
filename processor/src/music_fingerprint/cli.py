"""CLI entry point: `python -m music_fingerprint <command>` /
`music-fingerprint <command>`."""

from __future__ import annotations

import logging
import os
from pathlib import Path

import click

from music_fingerprint.config import DEFAULT_CONFIG
from music_fingerprint.extraction import extract_from_file
from music_fingerprint.ingestion import Ingestor

logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(name)s %(message)s")
log = logging.getLogger("music_fingerprint.cli")


@click.group()
def main() -> None:
    """Loony Shazam audio fingerprinting tools."""


@main.command()
@click.argument("music_dir", type=click.Path(exists=True, file_okay=False, path_type=Path))
@click.option(
    "--database-url",
    envvar="DATABASE_URL",
    required=True,
    help="Postgres connection string (or set DATABASE_URL).",
)
@click.option(
    "--artwork-dir",
    envvar="ARTWORK_DIR",
    default=None,
    help="Directory to write extracted cover art to (default: ./data/artwork). "
    "Must be the same directory the backend serves at GET /artwork/*.",
)
def ingest(music_dir: Path, database_url: str, artwork_dir: str | None) -> None:
    """Ingest a directory of licensed/local audio into the catalog.

    Idempotent: re-running over the same directory skips files already
    recorded in `ingested_sources` (identified by SHA-256). Embedded cover
    art (ID3 APIC / MP4 covr / FLAC pictures) is extracted automatically
    where present.
    """
    with Ingestor(db_url=database_url, artwork_dir=artwork_dir) as ingestor:
        outcomes = ingestor.ingest_directory(music_dir)

    ingested = [o for o in outcomes if o.status == "ingested"]
    skipped = [o for o in outcomes if o.status == "skipped_duplicate"]
    failed = [o for o in outcomes if o.status == "failed"]

    total_fingerprints = sum(o.fingerprint_count for o in ingested)
    click.echo(f"Ingested:  {len(ingested)} files, {total_fingerprints} fingerprints")
    click.echo(f"Skipped:   {len(skipped)} files (already ingested)")
    click.echo(f"Failed:    {len(failed)} files")
    for o in failed:
        click.echo(f"  FAILED {o.path}: {o.error}")

    if failed:
        raise SystemExit(1)


@main.command("backfill-artwork")
@click.argument("music_dir", type=click.Path(exists=True, file_okay=False, path_type=Path))
@click.option("--database-url", envvar="DATABASE_URL", required=True)
@click.option("--artwork-dir", envvar="ARTWORK_DIR", default=None)
def backfill_artwork(music_dir: Path, database_url: str, artwork_dir: str | None) -> None:
    """Fill in artwork_url for songs ingested before artwork extraction
    existed, without touching their fingerprints. Matches files to
    already-ingested songs purely by content hash (same identity used by
    `ingest`), so it's safe to point this at the same directory (or a
    superset of it) repeatedly.
    """
    with Ingestor(db_url=database_url, artwork_dir=artwork_dir) as ingestor:
        counts = ingestor.backfill_artwork_directory(music_dir)

    click.echo(f"Updated:                 {counts.get('updated', 0)}")
    click.echo(f"Already had artwork:     {counts.get('already_has_artwork', 0)}")
    click.echo(f"No embedded artwork:     {counts.get('no_artwork', 0)}")
    click.echo(f"Not a previously-ingested file: {counts.get('not_previously_ingested', 0)}")


@main.command()
@click.argument("audio_file", type=click.Path(exists=True, dir_okay=False, path_type=Path))
def extract(audio_file: Path) -> None:
    """Extract and print fingerprints for a single audio file (debugging)."""
    result = extract_from_file(audio_file, config=DEFAULT_CONFIG)
    click.echo(
        f"algorithm_version={result.algorithm_version} "
        f"sample_rate={result.sample_rate} duration_ms={result.duration_ms} "
        f"fingerprints={len(result.fingerprints)}"
    )


@main.command()
@click.option("--host", default=lambda: os.environ.get("PROCESSOR_HOST", "0.0.0.0"))
@click.option("--port", default=lambda: int(os.environ.get("PROCESSOR_PORT", "8001")), type=int)
def serve(host: str, port: int) -> None:
    """Run the persistent fingerprint extraction HTTP service."""
    import uvicorn

    uvicorn.run("music_fingerprint.service:app", host=host, port=port, log_level="info")


if __name__ == "__main__":
    main()
