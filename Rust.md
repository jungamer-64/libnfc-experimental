# Rust移行計画（libnfc） — 段階的ロードマップ

**開始日:** 2025-10-13
**目的:** プロジェクトを段階的にRustへ移行し、常にビルド可能・テスト可能な状態を維持する。C互換の薄いラッパーを残しつつモジュール単位で再実装する。

---

## 早見ガント（フェーズ別）

| フェーズ                   |                  期間（開始 — 終了） | 主要成果物 / マイルストーン                                     |
| ---------------------- | ---------------------------: | --------------------------------------------------- |
| Phase 0 — 準備と境界定義      | 2025-10-13 — 2025-10-27 (2週) | FFI方針文書、cbindgen初期設定、CIにRustビルド追加                   |
| Phase 1 — FFI基盤安定化     | 2025-10-28 — 2025-11-24 (4週) | log_put_messageの双方向テスト、`#[repr(C)]`構造体検証、FFIテストスイート |
| Phase 2 — 独立ユーティリティ移植  | 2025-11-25 — 2026-01-12 (7週) | `nfc-secure`等のRust移植、ユニット＋C統合テスト、リリース候補             |
| Phase 3 — 共通ロジック移行     | 2026-01-13 — 2026-03-08 (8週) | connstring 等のコアヘルパ完全移設、ドキュメント更新                     |
| Phase 4 — 状態管理・ドライバ層移行 | 2026-03-09 — 2026-05-03 (8週) | device lifecycleのRust実装、feature-flag分離、ドライバごとのテスト   |
| Phase 5 — 凍結・後始末       | 2026-05-04 — 2026-05-31 (4週) | Cレガシー凍結、統合テスト完了、ドキュメント最終化、削除準備                      |

> 合計期間（目安）: 約7〜8ヶ月（状況次第で前後）

---

## フェーズ別詳細（タスク・出力・受入基準・リスク）

### Phase 0 — 準備と境界定義（2週）

**目的:** 作業ルールとビルド境界を定め、CIにRustを組み込む。

**主要タスク**

* FFI方針文書を作成（呼び出し方向、エラーコードマッピング、型規約）。
* `Cargo.toml` の `crate-type = ["staticlib","cdylib"]` を確定。
* `cbindgen.toml` のテンプレ作成、ヘッダ出力パスを `rust/libnfc-rs/include/` に設定。
* CIにRustビルドを追加（CMake/Autotoolsと統合）。

**成果物（Deliverables）**

* `FFI_POLICY.md`（簡潔に守るべきルールを列挙）
* `cbindgen.toml` + CIジョブ
* 最低1回の成功したCMake連携ビルドログ

**受入基準**

* CIでCMake全テストが通ること（Rustコードはリンク済み）

**主要リスクと対策**

* *リスク:* シンボル名の不一致、ABIのズレ。
  *対策:* `readelf -Ws` と `nm` でシンボルチェック、簡易FFIサニティテストを作成。

---

### Phase 1 — FFI基盤安定化（4週）

**目的:** Rust⇄Cの往復が安全で安定していることを確認する。

**主要タスク**

* `log_put_message`を使った双方向テストケース作成（Rust→C→Rust）。
* `#[repr(C)]` で構造体を宣言、`static_assert`相当のABI検証を追加。
* CIに`ffi-sanity`パイプラインを作る（`cargo test` + Cの小テストを連続実行）。

**成果物**

* `ffi-sanity` テストジョブ（成功ログ）
* ABI互換のチェックスクリプト（プロジェクトルート）

**受入基準**

* 既存のC APIを呼んで戻ってくる基本ケースが全て通る。

**主要リスクと対策**

* *リスク:* ライフタイム/所有権の渡し方でダングリング発生。
  *対策:* すべてのFFI公開関数は「所有権ポリシー」を明示（誰がfreeするか）。

---

### Phase 2 — 独立ユーティリティ移植（7週）

**目的:** 安全なユーティリティをRustで実装し、並行運用で挙動一致を確認する。

**対象モジュール（候補）**

* `nfc-secure`（memcpy/memset/strlenラッパ） — 優先
* バイト列変換、ログラッピング、汎用ユーティリティ

**主要タスク**

* Rust実装を書く（`nfc_secure_rs.rs` 等）。
* `#[no_mangle] extern "C"` 関数を用意してCから利用可能にする。
* `cbindgen`でヘッダを生成、C側でインクルードして並列ビルド。
* ユニットテスト（Rust）＋既存Cの単体テストを両方実行し比較。自動差分チェックを整備。

**成果物**

* `libnfc_rs` のリリース候補アーティファクト（静的/共有ライブラリ）
* `nfc-secure`のRust実装 + C互換ヘッダ

**受入基準**

* 既存C実装と**振る舞いが完全一致**すること（境界ケース含む）。
* CIで両実装のテストが通過し、性能がベースラインを下回らない（許容差は設定）。

**主要リスクと対策**

* *リスク:* Rust実装でパフォーマンス低下。
  *対策:* ベンチマークを必須にし、threshold超過でPR拒否。

---

### Phase 3 — 共通ロジック移行（8週）

**目的:** `connstring`, `device_list`, `context` などの中核ヘルパをRustへ移す。

**主要タスク**

* 各構造体を `#[repr(C)]` でミラーリングしつつ、内部実装は安全型で書く。
* CのAPIは薄いラッパ関数で保持（`nfc_context_new()` 等はRust内部実装の薄ラッパ）。
* Cargoテスト（境界条件・エラーパス）を拡充。

**成果物**

* `connstring` と `device_list` のRust実装 + cbindgen出力
* テストレポート（差分・パフォーマンス・メモリ）

**受入基準**

* API互換性テスト通過
* 既存の上位テスト（integration）で問題なし

**主要リスクと対策**

* *リスク:* エラーハンドリングの意味合いが変わる（`errno` vs `Result`）。
  *対策:* エラーマッピング仕様を明文化（FFI_POLICYに追加）。

---

### Phase 4 — 状態管理・ドライバ層移行（8週）

**目的:** 長期的に最も価値がある部分（状態管理・I/O層）をRustで書き換える。

**主要タスク**

* `nfc_device` の所有権管理をRustで行う（`Box`, `Arc<Mutex<...>>`等の利用は注意）。
* 各ドライバはfeature-flagで切替可能にする（`--features pn532` 等）。
* ドライバごとの統合テスト、実機テストを計画して実行（必要ならハードウェアラボで回す）。

**成果物**

* ドライバのRust実装（段階的リリース）
* feature-flagに基づくビルドmatrix

**受入基準**

* 各ドライバの機能検証が実機で成功
* race-conditionやデッドロックがないことの確認（負荷テスト）

**主要リスクと対策**

* *リスク:* 同期・共有資源の扱いを誤るとデバイスがクラッシュ。
  *対策:* 動的解析（sanitizer相当）, ログ強化, ストレステスト

---

### Phase 5 — 凍結・後始末（4週）

**目的:** 移行完了後にCコードを凍結・整理し、最終ドキュメント化。

**主要タスク**

* C実装を `deprecated/` に移動し、READMEで非推奨を明記。
* `cbindgen` のヘッダを安定化、ドキュメントをRust-centricに更新。
* 最終統合テスト（例: `examples/` 全実行）をCIで実施。

**成果物**

* 最終リリースタグ（`vX.Y.Z-rust`）
* 移行報告書（変更点、互換性注意点、残課題）

**受入基準**

* 全テスト（unit/integration/examples）通過
* ダウングレード（C-only）パスがCIから消えている

---

## CI / テスト戦略（必須）

1. **パイプライン分岐:** `ci/rust-sanity`, `ci/ffi-sanity`, `ci/full` の3段構成。
2. **ビルド順序:** Rustライブラリビルド → cbindgenヘッダ出力 → Cビルド → 統合テスト。
3. **自動ABIチェック:** `cbindgen`出力と既存ヘッダの差分をチェックするジョブ。
4. **性能ゲート:** 重要ユニットにはbenchを追加。性能劣化が `>10%` なら要レビュー。
5. **メモリ保護:** sanitizer（ASan/TSan）をNightlyで回すジョブを用意。

---

## PR / レビュー / リリース運用

* **小さく短いPR** を徹底（1PRで1モジュール）。大きい差分はレビュー死する。
* **PRテンプレート** を用意（FFIの所有者/解放ポリシー、テスト手順、互換性チェックリストを含む）。
* **ステージングリリース**: `vX.Y.Z-rcN` を段階リリースしてユーザにテストしてもらう。

---

## ロールバックと緊急対応

* 各フェーズで「ゴー/ノーゴー判定」を設ける。失敗時は直前の安定Tagに即ロールバック。
* 重大なABI破壊やデバイス破壊の恐れがある変更は`canary` featureで限定公開。

---

## 参考コマンド（実務でよく使う）

* cbindgenヘッダ生成:

```bash
cbindgen --config rust/libnfc-rs/cbindgen.toml --crate libnfc_rs --output rust/libnfc-rs/include/libnfc_rs.h
```

* Cargoを静的ライブラリでビルド:

```toml
# Cargo.toml
[lib]
crate-type = ["staticlib", "cdylib"]
```

* CMakeでRustライブラリを組み込む（例）:

```cmake
add_custom_command(OUTPUT ${CMAKE_BINARY_DIR}/rust/liblibnfc_rs.a
  COMMAND cargo build --manifest-path=${CMAKE_SOURCE_DIR}/rust/libnfc-rs/Cargo.toml --release --target-dir ${CMAKE_BINARY_DIR}/rust
  WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}/rust/libnfc-rs
  COMMENT "Building libnfc_rs"
)
add_library(libnfc_rs STATIC IMPORTED GLOBAL)
set_target_properties(libnfc_rs PROPERTIES IMPORTED_LOCATION ${CMAKE_BINARY_DIR}/rust/release/liblibnfc_rs.a)
add_dependencies(libnfc_rs ${CMAKE_BINARY_DIR}/rust/liblibnfc_rs.a)
target_link_libraries(nfc PRIVATE libnfc_rs)
```

---

## 最後に（率直に）

* この計画は**堅実で現実的**だが、確実に時間はかかる。短期で劇的な効果を期待すると失敗する。
* 一番の失敗パターンは「大きすぎる単一PR」と「FFIポリシーが曖昧なまま突っ込むこと」。避けろ。

---

**次にやるべきこと（今すぐ取り掛かれる）**

1. `FFI_POLICY.md` をこのリポジトリのルートに追加する（Phase 0）
2. `cbindgen.toml` のテンプレを作る（出力パス・コメント規約）
3. CIに `ffi-sanity` ジョブを追加
4. フェーズ間依存マップ（本ドキュメント内）を作成してPRテンプレに組み込む
5. ベンチマークハーネス（`bench/`、`BENCH.md`）を追加し、ベースライン計測手順を確定する
6. `docs/hardware_matrix.md`（ドライバ×ハードウェア×テスト種別）を作り、Phase 4開始前に整備する
7. PRテンプレートを更新し、PRサイズ上限・必須CIチェックリストを明記する（小さなPR方針）
8. `FFI_POLICY.md` にエラーコードマッピングの具体例を追加する（errno ⇄ Result 等）

補足：レビューで指摘された改善点（簡潔）

* フェーズ間依存の明示
  * 本文書にフェーズ依存マップを追加しました（下記参照）。主要な依存例:
    * Phase 0 (FFI_POLICY, cbindgen) → Phase 1,2
    * Phase 1 → Phase 2 (FFI サニティが通ること)
    * Phase 2 (ユーティリティ) → Phase 3 (コア移行)
    * Phase 3 (コア) → Phase 4 (ドライバ)
  * 目的: PR設計時に「どのフェーズが安定している必要があるか」を明確化する。

* 性能ゲートラインの具体化（測定方法と閾値）
  * 測定手順（簡潔）: リリースビルド／CPUガバナを固定／ウォームアップ実行→N回（推奨10〜20）計測。計測項目はスループット（ops/s）、レイテンシ p50/p95/p99、ピークRSS。
  * 比較方法: 相対差 (%) を採用。ベースラインは既存のC実装（CIで定期収集）。
  * 閾値（提案）: p50/p95の悪化 >10% は要レビュー、>20% は gate fail。スループット低下 >10% は gate fail。メモリ増加 >20% は gate fail。
  * ツール: Rust 側は Criterion（cargo bench）推奨、C側は自動スクリプトで同一負荷を再現。

* FFI のエラーコードマッピング例
  * 例（簡潔）:
    * C: errno EINVAL → `NFC_ERR_INVALID_ARG`
    * C: errno ENOMEM → `NFC_ERR_NO_MEM`
    * C: errno EIO → `NFC_ERR_IO`
    * 成功: 0 → `NFC_ERR_OK`
  * 実装方針: `FFI_POLICY.md` に "errno を直接返すのではなく、必ず enum へ変換して返す" を明記。Rust 側は `#[repr(C)]` の error enum を公開し、Cからは整数で扱えるようにする。

* ドライバごとの feature-flag とハードウェアマトリクス
  * `docs/hardware_matrix.md` を用意し、各ドライバについて「モデル名 / ファームウェア範囲 / OS / テストレベル(manual/ci/emu) / 担当」を管理する。
  * CI では可能な限りエミュレータ／合成テストを用意し、実機テストはラボ回帰として計画する。

* 大規模PR対策（具体）
  * ガイドライン案: 1 PR は原則 500 行未満、かつ 1 モジュールまで。超過時は "large-change" ラベルとメインテイナー承認を必須にする。
  * 必須 CI チェック: コンパイル (C/Rust), unit tests (Rust/C), ffi-sanity, cbindgen-diff, linters/static-analysis。性能を変更するPRはベンチ結果添付を必須にする。

* Canary module の導入（早期検証）
  * 候補: `nfc-secure`（Phase 2）を canary として早期に Rust 化。目的は所有権／バッファハンドリングの典型パターン検証。
  * 受入基準: ユニットテスト＋ffi-sanity＋ASan/TSanでのクリーン、cbindgen ヘッダー差分なし、ベンチ閾値の合格。

短評: 指摘された点を本文に追記し、Phase 0〜2 での早期の小さな検証（canary）、メトリクスの定量化、ハードウェアマトリクス整備、PRサイズ運用の導入で運用上のリスクはさらに低下します。
