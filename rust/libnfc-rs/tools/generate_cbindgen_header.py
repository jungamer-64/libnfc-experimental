#!/usr/bin/env python3
"""
Generate a C header from the Rust crate with cbindgen while
silencing a class of benign warnings (`Missing [defines] entry`).

This script runs cbindgen with the crate's `cbindgen.toml` and writes
the header to the requested output path. Any "Missing `[defines]`"
warnings are suppressed and collected; optionally the helper
`update_cbindgen_defines.py` can be invoked to append missing
mappings to the `cbindgen.toml` automatically (pass --auto-update).

Usage:
  python3 tools/generate_cbindgen_header.py --output /path/to/out.h

Exit codes:
 - 0 on success
 - non-zero when cbindgen fails
"""

import argparse
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CDB_TOML = ROOT / "cbindgen.toml"

parser = argparse.ArgumentParser()
parser.add_argument("--output", required=True, help="Path to write the generated header")
parser.add_argument("--features", required=False, help="Comma-separated features to pass to cbindgen")
parser.add_argument("--auto-update", action="store_true", help="Auto-update cbindgen.toml for discovered defines")
parser.add_argument("--verbose", action="store_true", help="Print suppressed cbindgen warnings and details")
args = parser.parse_args()

generated = Path(args.output)
if not CDB_TOML.exists():
    print(f"cbindgen config not found at {CDB_TOML}", file=sys.stderr)
    sys.exit(2)

cmd = [
    "cbindgen",
    "--config",
    str(CDB_TOML),
    "--crate",
    "libnfc-rs",
]

# Optionally pass explicit feature list through to cbindgen so callers
# can request a feature-gated header (for example, to include
# debug-only helpers). cbindgen expects a comma-separated list of
# feature names, e.g. --features nfc_secure_debug
if args.features:
    cmd.extend(["--features", args.features])

cmd.extend(["--output", str(generated)])

print("Running cbindgen...")
proc = subprocess.run(cmd, capture_output=True, text=True)
stderr = proc.stderr
stdout = proc.stdout

# Collect missing defines
pattern = re.compile(r"Missing `\[defines\]` entry for `(.+?)` in cbindgen config\.")
missing = []
other_warnings = []
for line in stderr.splitlines():
    if pattern.search(line):
        missing.append(pattern.search(line).group(1))
    elif line.strip():
        other_warnings.append(line)

# Print other warnings unchanged
for w in other_warnings:
    print(w, file=sys.stderr)

if missing and args.verbose:
    unique = sorted(set(missing))
    print(f"Suppressed {len(missing)} cbindgen `[defines]` warnings (unique: {len(unique)})")
    for expr in unique:
        print(f"  * {expr}")

    if args.auto_update:
        updater = ROOT / "tools" / "update_cbindgen_defines.py"
        if updater.exists():
            print("Auto-updating cbindgen.toml with discovered expressions...")
            updater_cmd = [sys.executable, str(updater)]
            if args.features:
                updater_cmd.extend(["--features", args.features])
            r = subprocess.run(updater_cmd, capture_output=True, text=True)
            print(r.stdout)
            if r.stderr:
                print(r.stderr, file=sys.stderr)
        else:
            print("Updater script not found; skipping auto-update.")

if not proc.returncode == 0:
    print("cbindgen failed to generate header:", file=sys.stderr)
    print(stderr, file=sys.stderr)
    sys.exit(proc.returncode)

print(f"Header generated to {generated}")
sys.exit(0)
