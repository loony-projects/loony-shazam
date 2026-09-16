# Ingestion scripts

The actual ingestion pipeline lives in the Python package
(`processor/src/music_fingerprint/ingestion.py` and `cli.py`), invoked as
`music-fingerprint ingest <dir>` or `make ingest MUSIC_DIR=<dir>`, rather
than as standalone shell scripts here — see
[docs/ingestion.md](../../docs/ingestion.md). This directory is reserved
for future operational wrappers around that CLI (e.g. a scheduled
re-ingestion job, a batch import from a specific source layout) as they
become needed.
