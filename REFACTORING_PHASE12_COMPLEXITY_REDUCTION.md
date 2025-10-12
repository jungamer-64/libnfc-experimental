# Phase 12: Complexity Reduction Refactoring

**Date**: 2025-10-12
**Goal**: Reduce cyclomatic complexity and improve code maintainability
**Codacy Current Grade**: B (73点)
**Target Grade**: A (85点以上)

## Executive Summary

Codacyの分析により460件の問題が検出されました:

- **循環的複雑度が高い関数**: 48件 (CCN > 8)
- **長大な関数**: 35件 (50行超過)
- **コード重複**: 29%
- **Shell脆弱性**: 10件 (変数展開の引用符なし)

## Priority 1: Ultra-High Complexity Functions (CCN > 40)

### 1. `snprint_nfc_iso14443a_info()` - CCN: 86 ⚠️ CRITICAL

- **File**: `libnfc/target-subr.c:126`
- **Lines**: ~275行
- **Issue**: 複雑なATS/ATQAデコード処理が単一関数に集約

**Refactoring Strategy**:

```
snprint_nfc_iso14443a_info()
├── snprint_atqa_section()           // CCN: ~8
├── snprint_uid_section()            // CCN: ~4
├── snprint_sak_section()            // CCN: ~6
├── snprint_ats_section()            // CCN: ~12
│   ├── snprint_ats_max_frame()
│   ├── snprint_ats_bitrate()        // CCN: ~9
│   ├── snprint_ats_timing()         // CCN: ~4
│   ├── snprint_ats_node_cid()       // CCN: ~3
│   └── snprint_ats_historical()     // CCN: ~10
│       ├── snprint_mifare_tk()
│       └── snprint_compact_tlv()
└── snprint_fingerprint_section()    // CCN: ~15 (next phase)
```

**Expected CCN after refactoring**: 12

### 2. `utils/nfc-list main()` - CCN: 76

- **File**: `utils/nfc-list.c:83`
- **Lines**: Not specified
- **Issue**: メイン関数にすべてのロジックが集約

**Refactoring Strategy**:

- `parse_command_line()` - 引数解析を分離
- `list_devices()` - デバイス一覧処理
- `list_targets()` - ターゲット一覧処理
- `print_target_details()` - 詳細出力

**Expected CCN**: 10

### 3. `utils/nfc-mfclassic main()` - CCN: 65

- **File**: `utils/nfc-mfclassic.c:643`
- **Action**: Similar strategy to nfc-list

### 4. `nfcforum_tag4_io()` - CCN: 43

- **File**: `utils/nfc-emulate-forum-tag4.c:136`
- **Lines**: 126行
- **Issue**: ISO7816 APDUハンドリングがすべて一つの関数に

**Refactoring Strategy**:

- `handle_select_apdu()`
- `handle_read_binary_apdu()`
- `handle_update_binary_apdu()`
- `handle_unknown_apdu()`

### 5. `nfc-st25tb main()` - CCN: 41

### 6. `nfc-anticol main()` - CCN: 40

### 7. `write_card()` - CCN: 37

## Priority 2: High Complexity Functions (CCN 20-40)

- `pn532_spi_receive()` - CCN: 25
- `pcsc_get_information_about()` - CCN: 24
- `pn532_uart_receive()` - CCN: 22
- `parse_line()` (conf.c) - CCN: 22
- `pcsc_initiator_transceive_bytes()` - CCN: 21
- `arygon_tama_receive()` - CCN: 20
- `acr122_pcsc_open()` - CCN: 20
- `pn53x_usb_set_property_bool()` - CCN: 20
- `read_card()` - CCN: 20

## Priority 3: Large Functions (Lines > 100)

- `nfc-mfultralight main()` - 246行 → 関数分割
- `nfc-mfsetuid main()` - 229行 → 関数分割
- `nfc-anticol main()` - 190行 → 関数分割
- `nfc-relay main()` - 145行 → 関数分割
- `nfc-st25tb main()` - 137行 → 関数分割
- `nfcforum_tag4_io()` - 126行 → 関数分割
- `pn532_spi_receive()` - 109行 → 関数分割
- `pn53x_usb_open()` - 108行 → 関数分割

## Priority 4: Shell Script Vulnerabilities (HIGH)

**Semgrep検出 - 変数展開の引用符なし** (10箇所):

1. `mingw-cross-compile.sh:2` - `$(dirname $0)`
2. `mingw-cross-compile.sh:14` - `$LIBUSB_WIN32_BIN_URL`
3. `mingw-cross-compile.sh:15` - `$LIBUSB_WIN32_BIN_ARCHIVE`
4. `mingw-cross-compile.sh:27` - `$PROJECT_DIR`
5. `make_release.sh:41` - `$LIBNFC_DOC_DIR`
6. `make_release.sh:48` - `$LIBNFC_DOC_DIR`
7. `make_release.sh:49` - `$LIBNFC_DOC_ARCHIVE`
8. `test/run-test.sh:3` - `$0`
9. `examples/pn53x-tamashell-scripts/ReadMobib.sh:7` - `$DEBUG`

**Fix**: すべての変数展開を二重引用符で囲む

## Priority 5: Code Duplication (29%)

**Target**: 10%未満に削減

**Strategy**:

1. Codacyの重複検出機能を使用
2. 共通パターンをヘルパー関数に抽出
3. 特にdriver scanコード、error handlingが重複している可能性

## Priority 6: Documentation Linting

**markdownlint warnings** (8箇所):

- `PHASE11_WEEK2_REFACTORING_PROGRESS.md` - MD024: Multiple headings with same content (5箇所)
- `PN53X_ROBUSTNESS_IMPROVEMENTS.md` - MD024: Multiple headings with same content (1箇所)

**Fix**: 見出しをユニークにする

## Implementation Plan

### Week 1: Ultra-High Complexity (Days 1-3)

- [ ] Day 1: `snprint_nfc_iso14443a_info()` refactoring (CCN 86→12)
  - Create helper functions for ATQA, UID, SAK
  - Extract ATS decoding to separate module
  - Add unit tests
- [ ] Day 2: `nfc-list main()` refactoring (CCN 76→10)
- [ ] Day 3: `nfc-mfclassic main()` refactoring (CCN 65→12)

### Week 1: High Complexity & Shell (Days 4-5)

- [ ] Day 4: Driver receive functions (CCN 20-25 → <15)
  - `pn532_spi_receive()`
  - `pn532_uart_receive()`
  - `arygon_tama_receive()`
- [ ] Day 5: Shell script vulnerability fixes (all 10 locations)

### Week 2: Large Functions (Days 6-10)

- [ ] Day 6-7: Example programs refactoring
  - nfc-mfultralight, nfc-mfsetuid, nfc-anticol
- [ ] Day 8-9: Driver open functions
  - pn53x_usb_open, acr122_usb_open
- [ ] Day 10: Documentation fixes + Code duplication reduction

## Success Metrics

| Metric | Before | Target | How to Measure |
|--------|--------|--------|----------------|
| Codacy Grade | B (73) | A (85+) | Codacy dashboard |
| Total Issues | 460 | <200 | Codacy issues count |
| High CCN Functions (>20) | 13 | 0 | Lizard analysis |
| Large Functions (>100 lines) | 8 | 0 | Manual review |
| Code Duplication | 29% | <10% | Codacy duplication metric |
| Shell Vulnerabilities | 10 | 0 | Semgrep security scan |

## Testing Strategy

1. **Unit Tests**: 各リファクタリング関数に対するテストを追加
2. **Integration Tests**: 既存のtest/を実行して互換性確認
3. **Regression Tests**: examples/を実行して動作確認
4. **Static Analysis**: Codacy, Lizard, Semgrep再実行
5. **Manual Review**: 重要な関数は手動で動作確認

## Rollback Plan

- 各リファクタリングは個別のコミット
- 問題が発生した場合は`git revert`でロールバック
- ビルドが失敗したら即座に前のコミットに戻す

## Notes

- リファクタリング中は新機能追加を凍結
- ABI互換性を維持（既存の関数シグネチャは変更しない）
- パフォーマンス劣化がないか確認（特にホットパスの関数）
- ドキュメントもリファクタリングに合わせて更新

---

## Progress Tracking

Use the following format to track progress:

```
✅ Completed
🚧 In Progress
⏸️ Paused
❌ Failed
⏭️ Skipped
```

| Task | Status | CCN Before | CCN After | Notes |
|------|--------|------------|-----------|-------|
| snprint_nfc_iso14443a_info | 🚧 | 86 | - | Starting now |
| nfc-list main | ⏸️ | 76 | - | - |
| nfc-mfclassic main | ⏸️ | 65 | - | - |
