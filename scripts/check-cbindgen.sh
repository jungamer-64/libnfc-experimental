#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TRACKED_HEADER="$ROOT_DIR/rust/libnfc-rs/include/libnfc_rs.h"
TMP_HEADER="$(mktemp /tmp/libnfc_rs.h.XXXXXX)"

cleanup() {
  rm -f "$TMP_HEADER"
}
trap cleanup EXIT

echo "[check-cbindgen] Generating cbindgen header for comparison..."

# Prefer Python wrapper if present (keeps command-line flags centralized)
PY_WRAPPER="$ROOT_DIR/rust/libnfc-rs/tools/generate_cbindgen_header.py"
if command -v python3 >/dev/null 2>&1 && [ -x "$PY_WRAPPER" ]; then
  echo "[check-cbindgen] Using python wrapper: $PY_WRAPPER"
  python3 "$PY_WRAPPER" --output "$TMP_HEADER" || true
fi

if [ ! -s "$TMP_HEADER" ]; then
  if command -v cbindgen >/dev/null 2>&1; then
    echo "[check-cbindgen] Falling back to cbindgen binary"
    cbindgen --config "$ROOT_DIR/rust/libnfc-rs/cbindgen.toml" --crate libnfc-rs --output "$TMP_HEADER"
  else
    echo "[check-cbindgen] cbindgen not found and python wrapper not available. Skipping header check." >&2
    exit 0
  fi
fi

if [ ! -f "$TRACKED_HEADER" ]; then
  echo "[check-cbindgen] Tracked header not found at: $TRACKED_HEADER" >&2
  exit 1
fi

if ! cmp -s "$TRACKED_HEADER" "$TMP_HEADER"; then
  echo "[check-cbindgen] Tracked cbindgen header is out-of-date. Regenerate with:" >&2
  echo "  cbindgen --config rust/libnfc-rs/cbindgen.toml --crate libnfc-rs --output rust/libnfc-rs/include/libnfc_rs.h" >&2
  echo "" >&2
  echo "--- BEGIN DIFF ---" >&2
  diff -u "$TRACKED_HEADER" "$TMP_HEADER" || true
  echo "--- END DIFF ---" >&2
  exit 2
fi

echo "[check-cbindgen] Tracked cbindgen header is up-to-date"
exit 0
