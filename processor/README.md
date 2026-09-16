# music-fingerprint

Reference Shazam-style audio fingerprinting DSP pipeline and extraction
service for Loony Shazam. See [../docs/fingerprinting.md](../docs/fingerprinting.md)
for the algorithm and [../docs/ingestion.md](../docs/ingestion.md) for the
ingestion CLI.

## Quick start

```bash
python3 -m venv .venv && source .venv/bin/activate
pip install -e ".[dev]"

# Run tests
pytest

# Run the extraction service (used by the Rust backend)
music-fingerprint serve --port 8001

# Ingest a directory of licensed/local audio
DATABASE_URL=postgresql://postgres:devpass@localhost:5432/loony_shazam \
    music-fingerprint ingest ./path/to/music
```
