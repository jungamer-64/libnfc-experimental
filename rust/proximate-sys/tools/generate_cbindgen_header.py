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
parser.add_argument(
    "--config",
    required=False,
    default=str(CDB_TOML),
    help="Path to the cbindgen config file to use",
)
parser.add_argument("--features", required=False, help="Comma-separated features to pass to cbindgen")
parser.add_argument("--auto-update", action="store_true", help="Auto-update cbindgen.toml for discovered defines")
parser.add_argument("--verbose", action="store_true", help="Print suppressed cbindgen warnings and details")
args = parser.parse_args()

generated = Path(args.output).resolve()
config_path = Path(args.config).resolve()
if not config_path.exists():
    print(f"cbindgen config not found at {config_path}", file=sys.stderr)
    sys.exit(2)

generated.parent.mkdir(parents=True, exist_ok=True)


def postprocess_generated_header(path: Path) -> None:
    text = path.read_text()
    text = text.replace(
        "typedef struct usb_dev_handle {\n"
        "  uint8_t _private[0];\n"
        "} usb_dev_handle;\n",
        "typedef struct usb_dev_handle usb_dev_handle;\n",
        1,
    )
    path.write_text(text)


def postprocess_private_header(path: Path) -> None:
    text = path.read_text()
    public_type_block = (
        "#pragma pack(push, 1)\n\n"
        "typedef struct nfc_barcode_info {\n"
        "  size_t szDataLen;\n"
        "  uint8_t abtData[32];\n"
        "} nfc_barcode_info;\n\n"
        "typedef struct nfc_dep_info {\n"
        "  uint8_t abtNFCID3[10];\n"
        "  uint8_t btDID;\n"
        "  uint8_t btBS;\n"
        "  uint8_t btBR;\n"
        "  uint8_t btTO;\n"
        "  uint8_t btPP;\n"
        "  uint8_t abtGB[48];\n"
        "  size_t szGB;\n"
        "  nfc_dep_mode ndm;\n"
        "} nfc_dep_info;\n\n"
        "typedef struct nfc_felica_info {\n"
        "  size_t szLen;\n"
        "  uint8_t btResCode;\n"
        "  uint8_t abtId[8];\n"
        "  uint8_t abtPad[8];\n"
        "  uint8_t abtSysCode[2];\n"
        "} nfc_felica_info;\n\n"
        "typedef struct nfc_iso14443a_info {\n"
        "  uint8_t abtAtqa[2];\n"
        "  uint8_t btSak;\n"
        "  size_t szUidLen;\n"
        "  uint8_t abtUid[10];\n"
        "  size_t szAtsLen;\n"
        "  uint8_t abtAts[254];\n"
        "} nfc_iso14443a_info;\n\n"
        "typedef struct nfc_iso14443b2ct_info {\n"
        "  uint8_t abtUID[4];\n"
        "  uint8_t btProdCode;\n"
        "  uint8_t btFabCode;\n"
        "} nfc_iso14443b2ct_info;\n\n"
        "typedef struct nfc_iso14443b2sr_info {\n"
        "  uint8_t abtUID[8];\n"
        "} nfc_iso14443b2sr_info;\n\n"
        "typedef struct nfc_iso14443b_info {\n"
        "  uint8_t abtPupi[4];\n"
        "  uint8_t abtApplicationData[4];\n"
        "  uint8_t abtProtocolInfo[3];\n"
        "  uint8_t ui8CardIdentifier;\n"
        "} nfc_iso14443b_info;\n\n"
        "typedef struct nfc_iso14443bi_info {\n"
        "  uint8_t abtDIV[4];\n"
        "  uint8_t btVerLog;\n"
        "  uint8_t btConfig;\n"
        "  size_t szAtrLen;\n"
        "  uint8_t abtAtr[33];\n"
        "} nfc_iso14443bi_info;\n\n"
        "typedef struct nfc_iso14443biclass_info {\n"
        "  uint8_t abtUID[8];\n"
        "} nfc_iso14443biclass_info;\n\n"
        "typedef struct nfc_jewel_info {\n"
        "  uint8_t btSensRes[2];\n"
        "  uint8_t btId[4];\n"
        "} nfc_jewel_info;\n\n"
        "typedef struct nfc_modulation {\n"
        "  nfc_modulation_type nmt;\n"
        "  nfc_baud_rate nbr;\n"
        "} nfc_modulation;\n\n"
        "typedef union nfc_target_info {\n"
        "  nfc_iso14443a_info nai;\n"
        "  nfc_felica_info nfi;\n"
        "  nfc_iso14443b_info nbi;\n"
        "  nfc_iso14443bi_info nii;\n"
        "  nfc_iso14443b2sr_info nsi;\n"
        "  nfc_iso14443b2ct_info nci;\n"
        "  nfc_jewel_info nji;\n"
        "  nfc_dep_info ndi;\n"
        "  nfc_barcode_info nti;\n"
        "  nfc_iso14443biclass_info nhi;\n"
        "} nfc_target_info;\n\n"
        "typedef struct nfc_target {\n"
        "  nfc_target_info nti;\n"
        "  nfc_modulation nm;\n"
        "} nfc_target;\n\n"
        "#pragma pack(pop)\n"
    )
    public_forward_decl_block = (
        "typedef struct nfc_barcode_info nfc_barcode_info;\n\n"
        "typedef struct nfc_dep_info nfc_dep_info;\n\n"
        "typedef struct nfc_felica_info nfc_felica_info;\n\n"
        "typedef struct nfc_iso14443a_info nfc_iso14443a_info;\n\n"
        "typedef struct nfc_iso14443b2ct_info nfc_iso14443b2ct_info;\n\n"
        "typedef struct nfc_iso14443b2sr_info nfc_iso14443b2sr_info;\n\n"
        "typedef struct nfc_iso14443b_info nfc_iso14443b_info;\n\n"
        "typedef struct nfc_iso14443bi_info nfc_iso14443bi_info;\n\n"
        "typedef struct nfc_iso14443biclass_info nfc_iso14443biclass_info;\n\n"
        "typedef struct nfc_jewel_info nfc_jewel_info;\n\n"
        "typedef struct nfc_modulation nfc_modulation;\n\n"
        "typedef struct nfc_target nfc_target;\n\n"
        "typedef struct nfc_target_info nfc_target_info;\n"
    )
    inline_secure_helpers = (
        "static inline int\n"
        "nfc_is_null_terminated(const char *buf, size_t bufsize)\n"
        "{\n"
        "  if (buf == NULL || bufsize == 0) {\n"
        "    return 0;\n"
        "  }\n\n"
        "  for (size_t i = 0; i < bufsize; i++) {\n"
        "    if (buf[i] == '\\0') {\n"
        "      return 1;\n"
        "    }\n"
        "  }\n\n"
        "  return 0;\n"
        "}\n\n"
        "static inline void\n"
        "nfc_ensure_null_terminated(char *buf, size_t bufsize)\n"
        "{\n"
        "  if (buf == NULL || bufsize == 0) {\n"
        "    return;\n"
        "  }\n\n"
        "  if (!nfc_is_null_terminated(buf, bufsize)) {\n"
        "    buf[bufsize - 1] = '\\0';\n"
        "  }\n"
        "}\n"
    )
    state_block = (
        "typedef struct nfc_emulation_state_machine {\n"
        "  nfc_emulation_io_fn io;\n"
        "  void *data;\n"
        "} nfc_emulation_state_machine;\n\n"
        "typedef struct nfc_emulator {\n"
        "  struct nfc_target *target;\n"
        "  struct nfc_emulation_state_machine *state_machine;\n"
        "  void *user_data;\n"
        "} nfc_emulator;\n"
    )
    fn_match = re.search(
        r"typedef int \(\*nfc_emulation_io_fn\)\(struct nfc_emulator \*emulator,\n"
        r"                                   const uint8_t \*data_in,\n"
        r"                                   size_t data_in_len,\n"
        r"                                   uint8_t \*data_out,\n"
        r"                                   size_t data_out_len\);\n",
        text,
    )

    if fn_match is None:
        if public_forward_decl_block in text:
            text = text.replace(public_forward_decl_block, public_type_block + "\n", 1)
        path.write_text(text)
        return

    fn_block = fn_match.group(0)
    state_index = text.find(state_block)
    fn_index = text.find(fn_block)
    if public_forward_decl_block in text:
        text = text.replace(public_forward_decl_block, public_type_block + "\n", 1)
        state_index = text.find(state_block)
        fn_index = text.find(fn_block)
    if state_index == -1 or fn_index == -1 or fn_index < state_index:
        if "struct nfc_emulator;\n\n" not in text:
            text = text.replace(fn_block, "struct nfc_emulator;\n\n" + fn_block, 1)
        path.write_text(text)
        return

    text = text.replace(fn_block, "", 1)
    text = text.replace(state_block, fn_block + "\n" + state_block, 1)
    if "struct nfc_emulator;\n\n" not in text:
        text = text.replace(fn_block + "\n", "struct nfc_emulator;\n\n" + fn_block + "\n", 1)
    text = text.replace(
        "int nfc_is_null_terminated(const char *buf, size_t bufsize);\n\n"
        "void nfc_ensure_null_terminated(char *buf, size_t bufsize);\n",
        inline_secure_helpers + "\n",
        1,
    )
    path.write_text(text)

cmd = [
    "cbindgen",
    "--config",
    str(config_path),
    "--crate",
    "proximate-sys",
]

# cbindgen generation itself stays stable-friendly and does not rely on
# passing Cargo features on the command line. The optional --features
# flag is kept only for compatibility with helper tooling such as the
# define auto-updater below.
cmd.extend(["--output", str(generated)])

print("Running cbindgen...")
proc = subprocess.run(cmd, capture_output=True, text=True, cwd=ROOT)
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
            print(f"Auto-updating {config_path.name} with discovered expressions...")
            updater_cmd = [sys.executable, str(updater), "--config", str(config_path)]
            if args.features:
                updater_cmd.extend(["--features", args.features])
            r = subprocess.run(updater_cmd, capture_output=True, text=True, cwd=ROOT)
            print(r.stdout)
            if r.stderr:
                print(r.stderr, file=sys.stderr)
        else:
            print("Updater script not found; skipping auto-update.")

if not proc.returncode == 0:
    print("cbindgen failed to generate header:", file=sys.stderr)
    print(stderr, file=sys.stderr)
    sys.exit(proc.returncode)

postprocess_generated_header(generated)
if config_path.name == "cbindgen.private.toml":
    postprocess_private_header(generated)

print(f"Header generated to {generated}")
sys.exit(0)
