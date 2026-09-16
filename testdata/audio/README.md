# Test audio

Empty on purpose — no copyrighted or even realistic music is committed to
this repository (see [docs/ingestion.md](../../docs/ingestion.md)). Tests
generate synthetic, deterministic tone-sequence audio in-process (see
`processor/tests/conftest.py`'s `synth_track`, reused by
`scripts/dev/generate_test_catalog.py`) rather than reading fixture files
from this directory. If you need on-disk audio for manual testing, generate
it:
```bash
python3 scripts/dev/generate_test_catalog.py testdata/audio --num-songs 3
```
(that output is itself gitignored — see the root `.gitignore` — so it
never gets committed even if you generate it here).
