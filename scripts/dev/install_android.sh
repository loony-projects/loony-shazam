#!/usr/bin/env bash
# Builds and installs the Android app on a connected device/emulator, and
# wires up its connection to a locally-running backend.
#
# Usage:
#   scripts/dev/install_android.sh              # build+install debug, launch it
#   scripts/dev/install_android.sh --no-launch  # install but don't open the app
#   scripts/dev/install_android.sh --no-build   # install the APK already on disk
#   scripts/dev/install_android.sh --device X   # target a specific adb device
#
# Requires: Android SDK platform-tools (adb) on PATH or under $ANDROID_HOME,
# a connected device or running emulator, and (for a real recognition to
# work) the backend already running — see `make dev` / `make dev-local`.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ANDROID_DIR="$ROOT_DIR/android"
APP_ID="com.loonyshazam.app"
BACKEND_PORT="${API_PORT:-8080}"

DO_BUILD=1
DO_LAUNCH=1
DEVICE_ARG=()

while [ $# -gt 0 ]; do
  case "$1" in
    --no-build) DO_BUILD=0; shift ;;
    --no-launch) DO_LAUNCH=0; shift ;;
    --device) DEVICE_ARG=(-s "$2"); shift 2 ;;
    -h|--help)
      sed -n '2,14p' "$0"
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

find_adb() {
  if command -v adb >/dev/null 2>&1; then
    command -v adb
    return
  fi
  for candidate in "${ANDROID_HOME:-}/platform-tools/adb" "$HOME/Android/Sdk/platform-tools/adb"; do
    if [ -x "$candidate" ]; then
      echo "$candidate"
      return
    fi
  done
  echo "FAILED: adb not found. Install Android SDK platform-tools or set ANDROID_HOME." >&2
  exit 1
}
ADB="$(find_adb)"

echo "==> Checking for a connected device"
DEVICE_LIST="$("$ADB" devices | awk 'NR>1 && $2=="device" {print $1}')"
if [ -z "$DEVICE_LIST" ]; then
  echo "FAILED: no device/emulator found in 'adb devices'. Connect a device (USB debugging"
  echo "enabled) or start an emulator, then retry."
  "$ADB" devices
  exit 1
fi
echo "$DEVICE_LIST" | sed 's/^/    /'
if [ "$(echo "$DEVICE_LIST" | wc -l)" -gt 1 ] && [ ${#DEVICE_ARG[@]} -eq 0 ]; then
  echo "FAILED: multiple devices connected — pass --device <id> to pick one."
  exit 1
fi

if [ "$DO_BUILD" -eq 1 ]; then
  echo "==> Building debug APK"
  (cd "$ANDROID_DIR" && ./gradlew assembleDebug --console=plain -q)
fi

APK_PATH="$ANDROID_DIR/app/build/outputs/apk/debug/app-debug.apk"
if [ ! -f "$APK_PATH" ]; then
  echo "FAILED: $APK_PATH not found. Run without --no-build, or build it first."
  exit 1
fi

echo "==> adb reverse tcp:$BACKEND_PORT tcp:$BACKEND_PORT"
# Forwards the device's own localhost:$BACKEND_PORT to this machine's
# localhost:$BACKEND_PORT over the adb/USB (or emulator) connection. Works
# for a real device and an emulator alike — the debug build's BASE_URL is
# "http://localhost:$BACKEND_PORT/" precisely so this is the only networking
# trick needed (see app/build.gradle.kts and docs/android.md). Must be
# re-run after every reboot/reconnect (adb reverse rules don't persist).
"$ADB" "${DEVICE_ARG[@]}" reverse "tcp:$BACKEND_PORT" "tcp:$BACKEND_PORT"

if ! curl -sf "http://localhost:$BACKEND_PORT/health" >/dev/null 2>&1; then
  echo "WARNING: backend not responding at http://localhost:$BACKEND_PORT/health —"
  echo "         the app will install fine but recognition calls will fail until it's"
  echo "         running (\`make dev\` or \`make dev-local\`)."
fi

echo "==> Installing $APK_PATH"
"$ADB" "${DEVICE_ARG[@]}" install -r "$APK_PATH"

if [ "$DO_LAUNCH" -eq 1 ]; then
  echo "==> Launching $APP_ID"
  "$ADB" "${DEVICE_ARG[@]}" shell am start -n "$APP_ID/.MainActivity" >/dev/null
fi

echo "==> Done"
