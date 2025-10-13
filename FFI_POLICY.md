# FFI_POLICY for libnfc — Rust/C migration

この文書は libnfc の Rust への段階的移行における FFI（C ⇄ Rust）ルールと運用方針を定めます。
目的は ABI/メモリ所有権/エンコーディング/エラー伝搬等の曖昧さを排除し、安全に移行を進めることです。

## 適用範囲

この方針はリポジトリ内のすべての Rust/FFI 境界に適用します。既存 C API を Rust 実装で置き換える場合も、本方針に従ってください。

---

## 絶対禁止事項（FFI 移行の三種の神器）

1. **panic を FFI 境界の外へ漏らさないこと** — すべての公開 `#[no_mangle] extern "C"` 関数は `ffi_catch_unwind`（または同等のラッパー）でトップレベルを防御し、パニックを NULL もしくは規定の errno に正規化する。`unwrap()` や panic を起こし得る操作をラッパー外で実行することを禁止する。
2. **CallerFree ポインタを生の `free()` で解放しないこと** — Rust 側で確保して `CString::into_raw` 等で C に渡したメモリは、対応する `*_free` ラッパーのみで解放する。C テスト・サンプルを含むどのコードでも `free(ptr)` は使用禁止。
3. **cbindgen で再生成したヘッダをコミットせずに ABI を変更しないこと** — FFI のシグネチャや `#[repr(C)]` 構造体に変更がある場合、必ず `cbindgen --config rust/libnfc-rs/cbindgen.toml ...` を実行し、`rust/libnfc-rs/include/libnfc_rs.h` の差分を PR に含める。レビューチェックリスト（§7）に従い ABI 互換性の証跡を添付すること。

これらのルール違反はただちにマージ不可とし、既存ブランチでも修正されるまで作業を停止すること。

## 1) 基本原則（契約）

- C 側と Rust 側は明確な契約（入力: 型と所有権、出力: 型と所有権、エラー/副作用）を持つこと。
- すべての公開 FFI 関数は `extern "C"` + `#[no_mangle]` を使い、ドキュメントで所有権ルールを明記する。
- C 側に渡すポインタは NULL チェックを行うこと。NULL を許す場合はその意味を明示する。

## 2) 文字列（重要）

- すべての public C API で用いられるテキストは UTF-8 を推奨する（入出力ともに）。古い API の互換で他エンコーディングを受け付ける場合は明示する。
- Rust 側では外部から受け取る `const char *` を `CStr` で受け取り、UTF-8 ではない場合はエラーを返す（エンコーディングの選択は API ドキュメントで明記）。
- 文字列を C に返す場合は、次の所有権ポリシーのどれかを採用する（API ごとに明記）:
  - CallerFree: Rust が `malloc` 互換の関数で確保し、C 呼び出し元が `free()` で解放する（互換性のため、alloc は libc の malloc を使うラッパを用意）
  - CalleeFree: 呼び出し元が渡したバッファに書く（バッファ長を受け取る）
  - ThreadLastError: エラー文字列はスレッドローカルに格納し、`nfc_get_last_error()` で取り出す
- NUL 終端は常に保証すること。バイト列が NUL を含む場合の扱いも API ドキュメントで定義する。
- Rust から C に返す文字列は `CString::into_raw` でポインタ化し、対応する `*_free` 関数で `CString::from_raw` を用いて解放する。

## 2.5) メモリアロケータの一貫性

- Rust 側で `libc::malloc` / `libc::free` を呼び出す際は、常にホスト側と同一の C ランタイム（libc 実装）と連携することを明確にする。
- 異なる libc 実装（glibc vs musl）や異なるアロケータ間での混在（例: glibc の malloc で確保した領域を musl の free で解放する）は禁止する。クロスビルドや staticlink 環境でビルドする場合はリンク先の libc を明示し、テストで検証すること。
- `staticlib` として Rust をビルドする際は、`#[global_allocator]` に `std::alloc::System` を使う（ホストのシステムアロケータを利用）ことを推奨する。これにより、C と Rust が同じアロケータを共有しやすくなる。

### CallerFree の実装と `*_free` ラッパーの義務化

- CallerFree を採用する API では、必ず対応する `nfc_free_XXX_string(const char *ptr)` のような専用の解放関数を公開すること。C 側の利用者は **直接** `free()` を呼んではならず、常にこの解放ラッパーを使用することを義務付ける。
- ラッパー実装は内部で確実に Rust が利用するアロケータ/ランタイムと同一の解放方法を用いる（例: `CString::from_raw` を使うか、libc の `free` をラップする）。
- CI では CallerFree を採用する API の一覧を生成し、テスト/サンプルコードで直接 `free()` を呼んでいないかを静的にチェックするルールを導入することを推奨する（例: `scripts/check_callerfree_usage.sh` を作成して grep/AST 解析で検出）。

### ビルド時アロケータ情報の記録とランタイムチェック

- `build.rs` は、ビルド対象のターゲットやリンクされる libc の識別（glibc/musl/その他）を検出し、`cargo:rustc-env=LIBC_IMPLEMENTATION=...` のように出力してビルド成果物に記録することを推奨する。
- `ffi-test` 起動時にこの環境変数を読み、実行時に簡易なアロケータ一致チェック（例えば、Rust 側で malloc したポインタを専用の free ラッパーで解放して成功するかの確認）を行い、一致しない場合はテストを失敗させる仕組みを推奨する。

## 3) メモリと所有権

- FFI 境界を越えるポインタは、「誰が割当て」「誰が解放するか」を必ずコメント/ドキュメント化すること。
- Rust 側で確保したメモリを C 側で解放させる必要がある場合、libc の `malloc`/`free` を用いるか、明示的な `nfc_free()` を提供する。直接 `Box::into_raw` を渡す場合は、対応する `Box::from_raw` を必ず用意する wrapper を公開する。
- 不透明ポインタ（opaque pointer）を原則とする。C 側には内部構造を公開せず、操作は関数経由に限定する（例: `struct nfc_context;` を外部公開し、`nfc_context_new()` / `nfc_context_free()` を用意）。これにより内部実装の自由度が高まる。

## 4) エラーハンドリング

- Rust 側の `Result<T, E>` は C では整数コード（enum）で返す。ただし詳細な文字列情報は別途取り出せるようにする（`nfc_get_last_error()` など）。
- `nfc_get_last_error()` のようなスレッドローカルな文字列アクセサを用意し、直近の失敗理由を C 呼び出し元が取得できるようにする。
- エラーコード表（enum）は `libnfc_error_t` を定義し、拡張する際は互換性を配慮する。
- 可能であれば、`errno` は使わず独自のスレッドローカル `last_error` を用いる。`errno` を利用する既存コードが多い場合はマッピングを明記する。

## 4.5) Error Mapping Layer

- すべての Rust 側エラー型（例: `enum NfcError`）は FFI 境界での整数表現を明示するために `#[repr(i32)]`（またはプロジェクトで合意した整数型）を付与すること。
- すべてのエラー型には `fn as_errno(&self) -> i32` のような変換関数を実装し、FFI での返却は必ずこの関数を介して行う（標準化された変換レイヤ）。

例:

```rust
#[repr(i32)]
pub enum NfcError {
  Success = 0,
  Io = -5,
  Timeout = -110,
  InvalidArg = -22,
}

impl NfcError {
  pub fn as_errno(&self) -> i32 { *self as i32 }
}

// Result<T, NfcError> を FFI に変換する際は、Err(e) => e.as_errno() を常に使う
```

- この一貫したマッピングにより、C 側の `errno` や既存のエラーコード体系との整合性が保たれ、保守性が向上する。

### ffi_catch_unwind の必須化と静的チェック

- すべての `#[no_mangle] extern "C"` 公開関数は、エントリポイントで `ffi_catch_unwind`（またはプロジェクト標準の等価ラッパ）を用いて panic を吸収し、必ず整数のエラーコードで戻ることを義務付ける。
- CI では簡易的な静的チェックを導入し、`#[no_mangle] extern "C"` 関数が `ffi_catch_unwind` を呼んでいるかを検査するルールを設ける（不適合は PR チェックで失敗させる）。

## 5) 不変条件と unsafe の扱い

- unsafe ブロックを使用する場合、PR に次を含めることを必須とする：
  - この unsafe が必要な理由
  - 安全性を保証する不変条件（invariants）の明示
  - 試験方法（ユニット/統合/FFI テスト）
- unsafe の使用は最小限に留め、可能なら safe wrapper を外部に公開する。

## 6) 同期（Concurrency）方針

- FFI 境界を越える API はできるだけロックを隠蔽し、単純な操作（トランザクション単位）を提供する。複数 API を跨いでロックを保持させる設計は避ける。
- コールバックを実装する場合、C から Rust へコールバックするときの再入やデッドロックを避けるため、ドキュメントで再入禁止か、`reentrant` を許す設計か明示する。

## 7) コールバックの設計

- コールバックを渡す API は、コールバックが呼ばれるスレッドコンテキスト（呼び出し元スレッド、内部ワーカースレッド等）を明記する。
- コールバックが FFI をまたいで Rust の所有するデータ構造にアクセスする場合、ミューテックスや参照カウントの取り扱いを明確にする。

### user_data パターンの標準化

- C 側に登録するコールバック関数は、原則として `void *user_data` 引数を受け取るシグネチャを持つこと。
- Rust 側はこの `user_data` を使い、`Arc<T>` や `Box<T>` へのポインタを渡すことで、コールバック内で安全に Rust 側の状態にアクセスする。これにより、グローバルな状態変数への依存を避ける。

例:

```rust
// Rust 側でコールバックとデータを登録する例
let my_state = std::sync::Arc::new(std::sync::Mutex::new(MyState::new()));
let ptr = std::sync::Arc::into_raw(my_state.clone()) as *mut std::ffi::c_void;
unsafe {
  // C の関数を呼び出し、コールバックとポインタを渡す
  register_callback(my_c_callback, ptr);
}

// C から呼ばれるコールバック関数（Rust 側実装）
#[no_mangle]
extern "C" fn my_c_callback(event_data: i32, user_data: *mut std::ffi::c_void) {
  // user_data を Arc に戻して使用し、所有権を戻すために forget する
  let state_arc = unsafe { std::sync::Arc::<std::sync::Mutex<MyState>>::from_raw(user_data as *const _ ) };
  {
    let mut state = state_arc.lock().unwrap();
    // ... state を安全に使う ...
  }
  // 所有権を戻す（ポインタのライフタイムを維持するため）
  std::mem::forget(state_arc);
}
```

- このパターンを標準化することで、コールバックの実装が統一され、レビューが容易になります。

### コールバックの終了と user_data の解放責任

- `register_callback` に対しては必ず対応する `unregister_callback`（または `nfc_context_free` 等の終了関数）を実装し、そこで `user_data` に対して一度だけ `Arc::from_raw` を呼び出し、適切に drop（所有権の解放）することを義務化する。
- コールバック内で一時的に `Arc::from_raw` を使って所有権を取得するパターンを許容するが、最終的な解放責任は必ず unregister 側で担うこと。複数回の unregister 呼び出しに対して安全（idempotent）であることを API 設計で考慮する。
- unregister の呼び出し時期と責任範囲をドキュメント化し、ffi-test に cleanup の検証ケースを追加することを推奨する。

## 8) ABI 検証と自動テスト

- `cbindgen` の出力を CI で生成し、差分チェックを行うこと。
- 可能なら `bindgen` を用いて C ヘッダから Rust 側の型を自動生成し、既存の `#[repr(C)]` 構造体と比較するテスト（サイズ/オフセットの検証）を追加する。これらの検証は CI の自動ジョブとして常時実行し、ABI 崩壊をビルド時に検出する。
- nightly で ASan/TSan による検査ジョブと、cargo-fuzz を用いた Fuzz テストを導入することを推奨する（Phase 1 以降の Nightly ジョブ）。

## 8.5) FFI Test Crate Policy

- FFI 境界の統合テストはメインの crate から分離し、専用の `ffi-test` crate（`rust/libnfc-ffi-test` 等）として配置することを推奨する。
- `ffi-test` crate の目的は、Rust 側テストと C 側の小さなテストバイナリを同一の `cargo test` 流れでビルド/実行し、リンクやランタイムの不整合（アロケータ、シンボル、ABI）を検出することにある。
- 実装指針:
  - `ffi-test/build.rs` で `cc` crate を使い、必要な C テストコードをビルドしてリンクする。
  - `ffi-test` のテストは `cargo test -p ffi-test` で実行できること。
  - CI では `cargo test -p ffi-test -- --nocapture` を Nightly + ASan 環境で実行し、メモリ/UB 不整合を検出する。

これにより、C と Rust が混在する移行期間において、テストが分離され保守が容易になります。

## 9) ドライバ層（ハードウェアアクセス）固有の注意点

- ドライバ層の移行は段階的に行う。まずは高レベル API を Rust にし、低レベル IO は既存の C ドライバを呼ぶラッパー方式で段階的に移行する。
- ハードウェア I/O 周りでは、unsafe を用いたメモリ/ポインタ演算や register マッピングが発生するため、PR レビューでの重点チェック項目とする。

## 10) バージョニングとリリース方針

- ABI 互換性に破壊的変更がある場合はメジャーバージョンを上げる（セマンティックバージョニング）。移行中の暫定ビルドでは `-rc` や `-canary` を用いること。

## 11) ドキュメントと PR 要件

- 各 FFI を変更する PR は次を満たすこと：
  - 所有権ポリシーの明記
  - unsafe の根拠と不変条件
  - ABI 互換性のエビデンス（cbindgen 出力や readelf/nm の抜粋）
  - 対応する単体テスト / FFI サニティテスト

## 12) 付録: 推奨ツール一覧

- cbindgen (ヘッダ生成)
- bindgen (C ヘッダ → Rust 型検証)
- cargo-fuzz (ファジング)
- cargo-asm / readelf / nm（シンボル確認）

## 13) 不透明ポインタと API 設計

- `nfc_context` や `nfc_device` など状態を持つ構造体は C には不透明ポインタとして公開し、ヘッダでは `struct nfc_context;` のように前方宣言だけを行う。
- 利用者には `nfc_context_new()` / `nfc_context_free()` などのコンストラクタ/デストラクタ関数を提供し、メモリ生存期間と所有権を明確にする。
- `#[repr(C)]` でレイアウトを共有するのは、フィールドを直接公開する必要がある単純なデータ構造に限定する。その場合はパディングや順序を含む不変条件をドキュメント化する。
- 将来的にフィールドを追加・変更する際は、このセクションのルールに照らして ABI 安全性をレビューし、必要であればバージョンを更新する。

---

このファイルは移行計画の living document として更新してください。重大な方針変更がある場合は、RFC として議論・承認を得た上で更新します。

## 14) 定数とEnumの管理

- FFI 境界を越えて共有される enum や定数は、Rust 側をソース・オブ・トゥルース（source-of-truth）とし、`cbindgen` を使って C ヘッダへ自動生成すること。
- 既存の C 側にしかない `#define` 定数を Rust 側で使う必要がある場合は、`bindgen` を `build.rs` に組み込み、ビルド時に取り込むか、手動で Rust 側にミラーリングして同期を保つこと。
- Rust の enum を C で使う場合は、必ず `#[repr(C)]` や具体的な整数表現（例: `#[repr(u32)]`）を付与して ABI 互換性を保証すること。
- 値の変更は破壊的な互換性を生むため、変更時はリリースノートに明確に記載し、必要であればメジャーバージョンを上げる。

## 最終微調整（運用向け）

### panic → error 正規化ラッパー

FFI 境界において Rust の panic が呼び出し元 C に伝播することを防ぐため、共通ラッパーを利用して panic をキャッチし、統一されたエラーコードに変換することを推奨します。例:

```rust
#[inline]
pub fn ffi_catch_unwind<F, T>(f: F) -> i32
where
  F: FnOnce() -> Result<T, NfcError>,
{
  match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
    Ok(Ok(_)) => NfcError::Success.as_errno(),
    Ok(Err(e)) => e.as_errno(),
    Err(_) => NfcError::Panic.as_errno(),
  }
}
```

このパターンを FFI 関数の先頭で使うことで、すべての実行パスを `i32` に正規化できます。

### CI Failure Policy（日本語）

- `ffi-test` crate のテストが失敗した場合、その変更はリリースブランチへマージ禁止とする。
- 既知の未移行箇所については `#[ignore]` を使い、ドキュメント化されたテストのみを一時的に除外できる（ただしマージ時には解消を要求する）。
- FFI テスト失敗時は、CI ログを `--show-output` で再実行し、`readelf -Ws` / `nm` によるシンボル確認を行うことを必須手順とする。

### Governance（運用ルール）

- 本方針の変更は `RFC: FFI Policy Update` として PR で提案し、FFI Maintainer または Rust Maintainer のレビュー承認を必須とする。
- 承認者は `CODEOWNERS` ファイルに `rust/`, `include/`, `ffi/` を含む行で明示すること。

---

## English: Final adjustments (operational)

### panic → error normalization wrapper

To prevent Rust panics from unwinding across the FFI boundary, use a shared wrapper that catches panics and normalizes outcomes to an integer error code. Example:

```rust
#[inline]
pub fn ffi_catch_unwind<F, T>(f: F) -> i32
where
  F: FnOnce() -> Result<T, NfcError>,
{
  match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
    Ok(Ok(_)) => NfcError::Success.as_errno(),
    Ok(Err(e)) => e.as_errno(),
    Err(_) => NfcError::Panic.as_errno(),
  }
}
```

Apply this wrapper at the entrypoint of every FFI-exposed function to ensure all control-flow paths normalize to an `i32` return value.

### CI Failure Policy (English)

- Tests in the `ffi-test` crate must pass before a change can be merged into a release branch.
- Tests annotated with `#[ignore]` (documented, known-to-be-broken cases) may be temporarily skipped, but merging requires a plan and eventual resolution.
- Upon `ffi-test` failures, CI must re-run failing cases with `--show-output` and the reviewers must perform symbol checks with `readelf -Ws` / `nm` to rule out ABI or symbol issues.

### Governance

- Changes to this policy must be proposed as `RFC: FFI Policy Update` via a pull request, and must be approved by the FFI Maintainer or Rust Maintainer.
- Add the maintainers to `CODEOWNERS` covering `rust/`, `include/`, and `ffi/` paths to make approvals explicit.

## English FFI Policy

This document defines the ground rules for interactions between the legacy C
API and the ongoing Rust implementation. Every pull request that touches the
FFI boundary must link back to the relevant items below.

## 1. ABI and Symbol Conventions

- All Rust functions exported to C **must** use `#[no_mangle]` and
  `extern "C"`.
- Exports are placed in the `libnfc_rs` crate. The crate `lib` stanza remains
  `crate-type = ["staticlib"]` until the final deprecation of the pure C
  implementation.
- Stable symbol names follow the existing C naming scheme. When a Rust function
  replaces a former C implementation, the symbol name is kept identical so that
  existing binaries continue to link.
- Structures intentionally shared across the boundary must be annotated with
  `#[repr(C)]`, mirrored in the C headers, and accompanied by documented layout
  invariants. Layout changes require a SONAME bump.

## 2. Ownership and Memory Management

- Callers retain ownership of inputs unless explicitly documented otherwise.
- Strings crossing the boundary must be NUL-terminated UTF-8. Rust validates
  inputs with `CStr::from_ptr`; APIs that accept a different encoding must
  convert at the boundary and document the behaviour.
- When returning strings to C, convert with `CString::into_raw` and expose a
  matching `*_free` helper that reclaims the allocation with
  `CString::from_raw`.
- Any Rust function that allocates memory and returns it to C **must** expose a
  matching `*_free` routine, or return buffers whose ownership is clearly
  documented (e.g., borrowed slices that stay valid only during the call).
- All allocations performed inside Rust use `libc::malloc`/`free` equivalents
  when the lifetime crosses the boundary. Purely internal Rust data may use the
  standard allocator as long as no raw pointer escapes to C.

## 3. Error Handling and Logging

- Rust functions return `int` error codes that mirror the existing C contract.
  Negative values use the `-errno` scheme already present in libnfc. Success is
  `0` unless a legacy API specifies a different sentinel.
- Rich error context stays in Rust logs. Short messages are emitted via the C
  helper `log_put_message()` so existing log filters keep working.
- Rust code must not panic across the FFI boundary. Any panic must be caught
  and translated into `NFC_COMMON_ERROR` (or a more specific code). Use
  `std::panic::catch_unwind` when delegating to code that might panic.
- Provide a thread-local `nfc_get_last_error()`-style accessor that returns the
  most recent descriptive error string so C callers can branch on structured
  failure details.

## 4. Opaque Pointers and API Design

- Stateful objects (e.g., `nfc_context`, `nfc_device`) are presented to C as
  opaque handles. C headers forward-declare `struct nfc_context;` and expose
  constructor/destructor-style functions such as `nfc_context_new()` and
  `nfc_context_free()`.
- Rust owns the underlying structure; internal fields may change without
  breaking the ABI as long as the pointer size and lifetime contract stay
  stable.
- Only simple data-transfer structs that require direct field access should use
  `#[repr(C)]`. Any such structs must have their layout locked down and be
  mirrored in the public C headers.
- Document invariants for any intentionally shared struct so future reviewers
  can evaluate ABI safety when fields are amended.

## 5. Header Generation (cbindgen)

- The canonical C declarations for Rust functions are generated by
  `cbindgen`. Manual header edits are forbidden; regenerate via the provided CI
  job or `make ffi-headers` (to be introduced in Phase 0).
- The generated header lives in `rust/libnfc-rs/include/libnfc_rs.h` and is
  versioned in Git to keep the build reproducible.

## 6. Testing and Compatibility

- Each FFI-facing function must have:
  - A Rust unit test that exercises success and failure paths.
  - A C integration test (or reuse of an existing one) that validates the
    exported ABI under `-fsanitize=address` in nightly CI.
- Behavioural differences between the legacy and Rust implementations must be
  documented in the release notes before merging the change that introduces
  them.
- CI must include an automated ABI layout check (e.g., `bindgen`-generated
  mirrors compared against `#[repr(C)]` types or compile-time assertions) to
  catch size/alignment regressions at build time.

## 7. Review Checklist

PR descriptions that modify FFI code must answer the following:

1. Which symbols were added/changed/removed?
2. Who is responsible for freeing data returned from Rust?
3. How are errors mapped to the public API?
4. Has `cbindgen` output been regenerated and included?
5. Which tests (Rust + C) were executed locally and in CI?

Keeping this checklist in review templates ensures that no hidden ABI change
slips into a release.
