#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# Only scan Rust sources for direct free() usage — C examples and tests often use free() legitimately.
SEARCH_DIRS=("$ROOT_DIR/rust")

echo "[check_callerfree_usage] Scanning for direct free() usage in Rust code and FFI tests..."

FOUND=0
for d in "${SEARCH_DIRS[@]}"; do
  if [ -d "$d" ]; then
    # Look for the literal token free( in source files under these directories
    # Limit to Rust sources to avoid reporting C examples that legitimately call free().
    matches=$(grep -R --line-number --binary-files=without-match -E "\bfree\s*\(" --exclude-dir=target --exclude-dir=build --include "*.rs" "$d" || true)
    if [ -n "$matches" ]; then
      echo "[check_callerfree_usage] Found potential free() uses in $d:" >&2
      echo "$matches" >&2
      FOUND=1
    fi
  fi
done

if [ "$FOUND" -eq 1 ]; then
  echo "[check_callerfree_usage] ERROR: Found direct free() calls in Rust/FFI test areas. Review and ensure *_free wrappers are used instead." >&2
  exit 2
fi

echo "[check_callerfree_usage] No suspicious free() usages detected in scanned paths."
exit 0
