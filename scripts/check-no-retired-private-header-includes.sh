#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PATTERN='^\s*#include\s+[<"].*(nfc-internal\.h|rust_bridge\.h|rust_usb_bridge\.h|nfc-common\.h|nfc-secure\.h|chips/pn53x\.h|libnfc/chips/pn53x\.h)[>"]'

matches="$(
  git -C "$ROOT_DIR" ls-files --cached --modified --others --exclude-standard '*.c' '*.h' \
    | while IFS= read -r path; do
        [[ -f "$ROOT_DIR/$path" ]] && printf '%s\0' "$ROOT_DIR/$path"
      done \
    | xargs -0 -r rg -n --no-heading "$PATTERN" \
    || true
)"

if [[ -n "$matches" ]]; then
  echo "[check-no-retired-private-headers] Found retired private header includes:" >&2
  echo "$matches" >&2
  exit 1
fi

echo "[check-no-retired-private-headers] OK"
