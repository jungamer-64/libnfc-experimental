#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PATTERN='^\s*#include\s+[<"].*(nfc-internal\.h|rust_bridge\.h|rust_usb_bridge\.h|nfc-common\.h|nfc-secure\.h|chips/pn53x\.h|libnfc/chips/pn53x\.h|libnfc_rs_private\.h)[>"]'
TEST_ONLY_PATTERN='^\s*#include\s+[<"].*(test/rust_core_test\.h|libnfc/log\.h|libnfc/pn53x_extras\.h)[>"]'

mapfile -t tracked_files < <(
  git -C "$ROOT_DIR" ls-files --cached --modified --others --exclude-standard '*.c' '*.h'
)

scan_matches() {
  local pattern="$1"
  shift

  if [[ "$#" -eq 0 ]]; then
    return 0
  fi

  printf '%s\0' "$@" | xargs -0 -r rg -n --no-heading "$pattern" || true
}

all_files=()
test_files=()
for path in "${tracked_files[@]}"; do
  [[ -f "$ROOT_DIR/$path" ]] || continue
  all_files+=("$ROOT_DIR/$path")
  if [[ "$path" == test/* ]]; then
    test_files+=("$ROOT_DIR/$path")
  fi
done

matches="$(scan_matches "$PATTERN" "${all_files[@]}")"
test_matches="$(scan_matches "$TEST_ONLY_PATTERN" "${test_files[@]}")"

if [[ -n "$matches" ]]; then
  echo "[check-no-retired-private-headers] Found retired private header includes:" >&2
  echo "$matches" >&2
  exit 1
fi

if [[ -n "$test_matches" ]]; then
  echo "[check-no-retired-private-headers] Found non-public test header includes:" >&2
  echo "$test_matches" >&2
  exit 1
fi

echo "[check-no-retired-private-headers] OK"
