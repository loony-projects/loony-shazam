# Seed data

No SQL seed data ships here on purpose: this system's meaningful "seed
data" is a fingerprinted audio catalog, which can only come from real
audio files (see the copyright policy in
[docs/ingestion.md](../../docs/ingestion.md)) — it can't be represented as
a static SQL fixture.

For local development, use the ingestion CLI against a synthetic catalog:
```bash
python3 scripts/dev/generate_test_catalog.py /tmp/test_catalog --num-songs 5
DATABASE_URL=postgresql://postgres:devpass@localhost:5432/loony_shazam \
  music-fingerprint ingest /tmp/test_catalog
```
or against your own licensed music directory. See
[docs/ingestion.md](../../docs/ingestion.md).
