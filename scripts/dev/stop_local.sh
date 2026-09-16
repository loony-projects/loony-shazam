#!/usr/bin/env bash
# Stops processes started by scripts/dev/run_local.sh.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUN_DIR="$ROOT_DIR/.run"

stop_one() {
  local name="$1"
  local pid_file="$RUN_DIR/$name.pid"
  if [ -f "$pid_file" ]; then
    local pid
    pid="$(cat "$pid_file")"
    if kill -0 "$pid" 2>/dev/null; then
      echo "Stopping $name (pid $pid)"
      kill "$pid"
      for _ in $(seq 1 20); do
        kill -0 "$pid" 2>/dev/null || break
        sleep 0.5
      done
      kill -0 "$pid" 2>/dev/null && kill -9 "$pid" 2>/dev/null || true
    else
      echo "$name not running (stale pid file)"
    fi
    rm -f "$pid_file"
  else
    echo "$name not running (no pid file)"
  fi
}

stop_one backend
stop_one processor
