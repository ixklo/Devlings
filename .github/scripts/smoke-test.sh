#!/usr/bin/env bash
# Shared by ci.yml's smoke job on both macOS and Linux: launches the debug
# build produced by `npm run tauri build -- --debug --no-bundle`, waits 60s,
# confirms it's still running with no panic, and takes a best-effort
# screenshot. Linux additionally needs a virtual display (Xvfb), since
# there's no real one to launch a GUI app against.
set -e

BIN=src-tauri/target/debug/perch
chmod +x "$BIN"

OS="$(uname -s)"

if [ "$OS" = "Linux" ]; then
  Xvfb :99 -screen 0 1920x1080x24 &
  XVFB_PID=$!
  sleep 2
  export DISPLAY=:99
fi

"$BIN" > smoke.log 2>&1 &
APP_PID=$!
sleep 60

if ! kill -0 "$APP_PID" 2>/dev/null; then
  echo "::error::app exited before the 60s smoke window ended"
  cat smoke.log
  exit 1
fi

if grep -iq "panic" smoke.log; then
  echo "::error::panic found in app output"
  cat smoke.log
  exit 1
fi

if [ "$OS" = "Linux" ]; then
  import -window root smoke-screenshot.png
else
  # Best-effort: GitHub's macOS runners can refuse screencapture over a TCC
  # (Screen Recording) permission prompt that CI can't answer itself. That
  # doesn't affect the pass/fail result above, which is what gate G4.1
  # actually requires; a missing screenshot here is a warning, not a
  # failure.
  if ! screencapture -x smoke-screenshot.png; then
    echo "::warning::screencapture failed (likely a CI Screen Recording permission gap); app itself ran fine for 60s with no panic"
  fi
fi

kill "$APP_PID" 2>/dev/null || true
if [ "$OS" = "Linux" ]; then
  kill "$XVFB_PID" 2>/dev/null || true
fi
