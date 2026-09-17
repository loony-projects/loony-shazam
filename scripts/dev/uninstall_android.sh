#!/usr/bin/env bash
# Uninstalls the app from a connected device/emulator.
set -euo pipefail

APP_ID="com.loonyshazam.app"

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

"$ADB" uninstall "$APP_ID"
