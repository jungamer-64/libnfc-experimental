#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TRACKED_HEADER="$ROOT_DIR/rust/proximate-sys/include/libnfc_rs.h"
TMP_HEADER="$(mktemp /tmp/proximate_sys.h.XXXXXX)"

cleanup() {
  rm -f "$TMP_HEADER"
}
trap cleanup EXIT

echo "[check-cbindgen] Generating headers for comparison..."

if ! command -v cbindgen >/dev/null 2>&1; then
  echo "[check-cbindgen] cbindgen not found. Skipping header check." >&2
  exit 0
fi

# Prefer Python wrapper if present (keeps command-line flags centralized)
PY_WRAPPER="$ROOT_DIR/rust/proximate-sys/tools/generate_cbindgen_header.py"
if command -v python3 >/dev/null 2>&1 && [ -f "$PY_WRAPPER" ]; then
  echo "[check-cbindgen] Using python wrapper: $PY_WRAPPER"
  python3 "$PY_WRAPPER" --output "$TMP_HEADER"
else
  echo "[check-cbindgen] python3 and the generation wrapper are required for header checks." >&2
  exit 1
fi

if [ ! -f "$TRACKED_HEADER" ]; then
  echo "[check-cbindgen] Tracked header not found at: $TRACKED_HEADER" >&2
  exit 1
fi

if ! cmp -s "$TRACKED_HEADER" "$TMP_HEADER"; then
  echo "[check-cbindgen] Tracked public header snapshot is out-of-sync. Refresh with:" >&2
  echo "  python3 rust/proximate-sys/tools/generate_cbindgen_header.py --output rust/proximate-sys/include/libnfc_rs.h" >&2
  echo "" >&2
  echo "--- BEGIN DIFF ---" >&2
  diff -u "$TRACKED_HEADER" "$TMP_HEADER" || true
  echo "--- END DIFF ---" >&2
  exit 2
fi

echo "[check-cbindgen] Tracked header is up-to-date"
exit 0
