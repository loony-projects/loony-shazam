#!/usr/bin/env bash
# End-to-end test: brings up the full stack via Docker Compose, ingests a
# synthetic test catalog, sends a recognizable query and an unrelated
# query, and asserts both outcomes. Run with `make e2e` from the repo root.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

CATALOG_DIR="$(mktemp -d)"
trap 'rm -rf "$CATALOG_DIR"; docker compose down' EXIT

ENV_FILE=.env
if [ ! -f "$ENV_FILE" ]; then
  echo "No .env found, using .env.example defaults for this run"
  ENV_FILE=.env.example
fi
set -a
# shellcheck disable=SC1090
source <(grep -v '^#' "$ENV_FILE" | grep -v '^\s*$')
set +a

echo "==> Starting stack (postgres, redis, processor, backend)"
docker compose up -d --build --wait postgres redis processor backend

BACKEND_URL="http://localhost:${API_PORT:-8080}"
PROCESSOR_URL_LOCAL="http://localhost:${PROCESSOR_PORT:-8001}"

echo "==> Waiting for backend readiness"
for i in $(seq 1 30); do
  if curl -sf "$BACKEND_URL/ready" >/dev/null; then break; fi
  sleep 2
done
curl -sf "$BACKEND_URL/ready" || { echo "backend never became ready"; exit 1; }

echo "==> Generating synthetic test catalog"
python3 -m venv "$CATALOG_DIR/venv" >/dev/null
source "$CATALOG_DIR/venv/bin/activate"
pip install --quiet numpy soundfile
python3 scripts/dev/generate_test_catalog.py "$CATALOG_DIR/audio" --num-songs 3 --duration 20
deactivate

echo "==> Ingesting test catalog"
(
  cd processor
  source .venv/bin/activate
  DATABASE_URL="postgresql://${POSTGRES_USER:-postgres}:${POSTGRES_PASSWORD:-devpass}@localhost:${POSTGRES_PORT:-5432}/${POSTGRES_DB:-loony_shazam}" \
    music-fingerprint ingest "$CATALOG_DIR/audio"
)

echo "==> Building a query clip (trimmed excerpt of Synthetic Alpha)"
(
  cd processor
  source .venv/bin/activate
  python3 -c "
import soundfile as sf
data, sr = sf.read('$CATALOG_DIR/audio/Test Artist - Synthetic Alpha.wav')
clip = data[sr*5:sr*13]
sf.write('$CATALOG_DIR/query_known.wav', clip, sr)
"
)

echo "==> Recognizing known clip"
RESPONSE=$(curl -sf -X POST "$BACKEND_URL/api/v1/recognitions/audio" \
  -F "audio=@$CATALOG_DIR/query_known.wav;type=audio/wav")
echo "$RESPONSE" | python3 -m json.tool
RECOGNIZED=$(echo "$RESPONSE" | python3 -c "import json,sys; print(json.load(sys.stdin)['recognized'])")
if [ "$RECOGNIZED" != "True" ]; then
  echo "FAIL: expected known clip to be recognized"
  exit 1
fi
TITLE=$(echo "$RESPONSE" | python3 -c "import json,sys; print(json.load(sys.stdin)['song']['title'])")
if [ "$TITLE" != "Synthetic Alpha" ]; then
  echo "FAIL: expected 'Synthetic Alpha', got '$TITLE'"
  exit 1
fi
echo "PASS: known clip recognized as '$TITLE'"

echo "==> Building an unrelated (noise) query clip"
(
  cd processor
  source .venv/bin/activate
  python3 -c "
import numpy as np
import soundfile as sf
rng = np.random.default_rng(123)
noise = rng.normal(0, 0.3, 44100 * 8).astype(np.float32)
sf.write('$CATALOG_DIR/query_unknown.wav', noise, 44100)
"
)

echo "==> Recognizing unrelated clip (expect NOT_RECOGNIZED)"
RESPONSE=$(curl -sf -X POST "$BACKEND_URL/api/v1/recognitions/audio" \
  -F "audio=@$CATALOG_DIR/query_unknown.wav;type=audio/wav")
echo "$RESPONSE" | python3 -m json.tool
RECOGNIZED=$(echo "$RESPONSE" | python3 -c "import json,sys; print(json.load(sys.stdin)['recognized'])")
if [ "$RECOGNIZED" != "False" ]; then
  echo "FAIL: expected unrelated clip to be NOT_RECOGNIZED"
  exit 1
fi
echo "PASS: unrelated clip correctly not recognized"

echo "==> E2E TEST PASSED"
