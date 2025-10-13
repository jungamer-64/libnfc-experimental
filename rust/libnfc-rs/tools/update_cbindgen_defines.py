#!/usr/bin/env python3
"""
Helper script: run cbindgen, collect missing `[defines]` warnings and
append corresponding entries to cbindgen.toml mapping them to
`NFC_SECURE` when the missing expression mentions the `nfc_secure`
feature. This is a convenience to iteratively add mappings discovered
by cbindgen.

Usage: python3 tools/update_cbindgen_defines.py

Note: This script appends keys to `cbindgen.toml` without attempting
full TOML parsing; it performs conservative checks to avoid duplicate
insertions.
"""

import argparse
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CDB_TOML = ROOT / "cbindgen.toml"

parser = argparse.ArgumentParser()
parser.add_argument("--features", required=False, help="Comma-separated features to pass to cbindgen")
args = parser.parse_args()

# Build cbindgen invocation; include features if requested so discovery
# matches the same feature-set as the generation step.
CMD = [
    "cbindgen",
    "--config",
    str(CDB_TOML),
    "--crate",
    "libnfc-rs",
    "--output",
    "/tmp/libnfc_rs.generated.h",
]
if args.features:
    CMD.extend(["--features", args.features])

pattern = re.compile(r"Missing \`\[defines\]\` entry for `(.+?)` in cbindgen config\.")

print("Running cbindgen to discover missing defines...")
proc = subprocess.run(CMD, capture_output=True, text=True)
stderr = proc.stderr

found = set()
for line in stderr.splitlines():
    m = pattern.search(line)
    if m:
        found.add(m.group(1))

if not found:
    print("No missing define expressions found in cbindgen output.")
    exit(0)

existing = CDB_TOML.read_text()
added = []

def derive_macro_for_expr(expr: str):
    # Prefer explicit known mappings
    if 'nfc_secure_debug' in expr:
        return 'NFC_SECURE_DEBUG'
    if 'nfc_secure' in expr:
        return 'NFC_SECURE'
    # feature = "..." pattern -> uppercased macro name
    feat = re.search(r'feature\s*=\s*"([^"]+)"', expr)
    if feat:
        name = feat.group(1)
        return name.upper().replace('-', '_')
    # have_... pattern -> HAVE_... uppercase
    have = re.search(r'(have_[A-Za-z0-9_]+)', expr)
    if have:
        return have.group(1).upper()
    # test harness expressions
    if expr.strip() == 'test' or 'cfg(test)' in expr:
        return 'RUST_TEST'
    return None

for expr in sorted(found):
    macro = derive_macro_for_expr(expr)
    if not macro:
        print(f"Skipping unknown expression: {expr}")
        continue
    # Escape double quotes and backslashes for TOML double-quoted key
    key = expr.replace('\\', '\\\\').replace('"', '\\"')
    entry = f'"{key}" = "{macro}"\n'
    # Don't add if some equivalent already exists (loose string match)
    # If the exact expression or key already exists, or if the macro
    # is already used as a mapping target, skip adding a duplicate.
    if expr in existing or key in existing or (f' = "{macro}"' in existing):
        print(f"Mapping for `{expr}` already present; skipping.")
        continue
    # Append the entry at the end of the [defines] section if present
    # Conservative approach: append near end of file.
    CDB_TOML.write_text(existing + "\n" + entry)
    existing = CDB_TOML.read_text()
    added.append((expr, macro))
    print(f"Added mapping for `{expr}` -> {macro}")

if added:
    print(f"Added {len(added)} entries to {CDB_TOML}")
else:
    print("No entries added; existing mappings covered the discovered expressions.")
