// src/nfc_secure.rs

//! Secure memory and string helpers exposed to C via a stable FFI.
//!
//! This module implements safe, well-documented replacements for a set of
//! libnfc C helpers. The functions below perform explicit parameter
//! validation and return stable integer error codes so they can be called
//! safely from C code. When available the implementation prefers platform
//! secure-zeroing APIs (explicit_bzero / memset_s / SecureZeroMemory) for
//! zeroing sensitive memory.
//!
//! Safety: All exported functions are `unsafe extern "C"` and their
//! safety requirements are documented on each function.
use libc::{c_char, c_int, size_t};
use std::ptr;
// Import compiler_fence/Ordering when the fallback path that uses
// libc::memset+compiler_fence may be compiled. This happens when no
// `memset`-style primitives are available (i.e. we must rely on the
// libc fallback to prevent the compiler optimizing away the write).
#[cfg(not(any(have_memset_explicit, have_memset_s)))]
use std::sync::atomic::{compiler_fence, Ordering};

#[cfg(feature = "nfc_secure_debug")]
use std::ffi::CStr;

pub const NFC_SECURE_SUCCESS: c_int = 0;
pub const NFC_SECURE_ERROR_INVALID: c_int = -1;
pub const NFC_SECURE_ERROR_OVERFLOW: c_int = -2;
pub const NFC_SECURE_ERROR_RANGE: c_int = -3;
pub const NFC_SECURE_ERROR_ZERO_SIZE: c_int = -4;
pub const NFC_SECURE_ERROR_INTERNAL: c_int = -5; // Internal sentinel returned when a panic occurs inside a secure helper.

// A conservative, explicit upper bound used to detect clearly-invalid
// size arguments. We prefer a named constant over `size_t::MAX / 2` so
// the intent is obvious when reviewers inspect the check.
const NFC_SECURE_MAX_REASONABLE_SIZE_64: usize = 1usize << 47; // 128 TiB

// Threshold used to decide when to switch from volatile-byte writes to
// a libc::memset+compiler_fence path for efficiency.
const NFC_SECURE_MEMSET_THRESHOLD: usize = 256;

// Helper: architecture-aware maximum acceptable size for secure
// operations. Centralizes the 64-bit vs smaller-platform logic so all
// callers use the same threshold.
fn secure_max_size() -> size_t {
    if std::mem::size_of::<size_t>() >= 8 {
        NFC_SECURE_MAX_REASONABLE_SIZE_64 as size_t
    } else {
        size_t::MAX / 4
    }
}

#[cfg(not(any(have_memset_explicit, have_memset_s)))]
#[test]
fn volatile_memset_aligned_and_unaligned() {
    unsafe {
        let usize_bytes = std::mem::size_of::<usize>();
        let small_len = std::cmp::min(usize_bytes * 3, NFC_SECURE_MEMSET_THRESHOLD);

        // Aligned buffer: vector allocations are aligned for usize.
        let mut aligned = vec![0u8; small_len];
        let p_aligned = aligned.as_mut_ptr() as *mut libc::c_void;
        let rc = nfc_secure_memset(p_aligned, 0x5A, small_len);
        assert_eq!(rc, NFC_SECURE_SUCCESS);
        for &b in &aligned {
            assert_eq!(b, 0x5A);
        }

        // Unaligned: offset the pointer by one to force byte-wise path.
        let mut unaligned = vec![0u8; small_len + 1];
        let p_unaligned = unaligned.as_mut_ptr().add(1) as *mut libc::c_void;
        let rc2 = nfc_secure_memset(p_unaligned, 0xA5, small_len);
        assert_eq!(rc2, NFC_SECURE_SUCCESS);
        for i in 0..small_len {
            assert_eq!(unaligned[i + 1], 0xA5);
        }
    }
}

#[cfg(not(any(have_memset_explicit, have_memset_s)))]
#[test]
fn repeated_byte_pattern_covers_all_bytes() {
    assert_eq!(repeated_byte_pattern(0x00), 0x0000_0000_0000_0000usize);
    assert_eq!(repeated_byte_pattern(0xFF), usize::MAX);
    assert_eq!(repeated_byte_pattern(0x5A), {
        let mut expected: usize = 0;
        let width = std::mem::size_of::<usize>();
        for shift in 0..width {
            expected |= (0x5Ausize) << (shift * 8);
        }
        expected
    });
}

#[cfg(not(any(have_memset_explicit, have_memset_s)))]
#[test]
fn large_memset_uses_memset_and_fence() {
    unsafe {
        let len = NFC_SECURE_MEMSET_THRESHOLD + 64;
        let mut buf = vec![0u8; len];
        let p = buf.as_mut_ptr() as *mut libc::c_void;
        let rc = nfc_secure_memset(p, 0x7E, len);
        assert_eq!(rc, NFC_SECURE_SUCCESS);
        for &b in &buf {
            assert_eq!(b, 0x7E);
        }
    }
}

#[cfg(not(any(have_memset_explicit, have_memset_s)))]
#[test]
fn volatile_memset_word_boundary_aligned() {
    unsafe {
        let usize_bytes = std::mem::size_of::<usize>();
        let l1 = if usize_bytes > 1 { usize_bytes - 1 } else { 1 };
        let lens = [
            l1,
            usize_bytes,
            usize_bytes + 1,
            usize_bytes * 2 - 1,
            usize_bytes * 2,
        ];
        for &len in &lens {
            if len == 0 {
                continue;
            }
            let mut buf = vec![0u8; len];
            let val = (0xA0u8).wrapping_add((len & 0xff) as u8);
            let rc = nfc_secure_memset(
                buf.as_mut_ptr() as *mut libc::c_void,
                val as libc::c_int,
                len,
            );
            assert_eq!(rc, NFC_SECURE_SUCCESS);
            for &b in &buf {
                assert_eq!(b, val);
            }
        }
    }
}

#[cfg(not(any(have_memset_explicit, have_memset_s)))]
#[test]
fn volatile_memset_word_boundary_unaligned() {
    unsafe {
        let usize_bytes = std::mem::size_of::<usize>();
        let l1 = if usize_bytes > 1 { usize_bytes - 1 } else { 1 };
        let lens = [
            l1,
            usize_bytes,
            usize_bytes + 1,
            usize_bytes * 2 - 1,
            usize_bytes * 2,
        ];
        for &len in &lens {
            if len == 0 {
                continue;
            }
            let mut buf = vec![0u8; len + 1];
            let ptr = buf.as_mut_ptr().add(1) as *mut libc::c_void;
            let val = (0x5Bu8).wrapping_add((len & 0xff) as u8);
            let rc = nfc_secure_memset(ptr, val as libc::c_int, len);
            assert_eq!(rc, NFC_SECURE_SUCCESS);
            for i in 0..len {
                assert_eq!(buf[i + 1], val);
            }
        }
    }
}

#[cfg(not(any(have_memset_explicit, have_memset_s)))]
#[test]
fn volatile_memset_threshold_edges() {
    unsafe {
        let t = NFC_SECURE_MEMSET_THRESHOLD;
        let lens = [t - 1, t, t + 1];
        for &len in &lens {
            let mut buf = vec![0u8; len];
            let val = (0x7Fu8).wrapping_add((len & 0xff) as u8);
            let rc = nfc_secure_memset(
                buf.as_mut_ptr() as *mut libc::c_void,
                val as libc::c_int,
                len,
            );
            assert_eq!(rc, NFC_SECURE_SUCCESS);
            for &b in &buf {
                assert_eq!(b, val);
            }
        }
    }
}

#[cfg(not(any(have_memset_explicit, have_memset_s)))]
#[test]
fn memset_multi_chunk_patterns() {
    unsafe {
        let usize_bytes = std::mem::size_of::<usize>();
        // Patterned sizes to exercise full-word chunk writes and tails.
        let sizes = [
            usize_bytes,                     // exactly one word
            usize_bytes + 1,                 // word plus tail
            usize_bytes * 3 + 2,             // multiple words plus small tail
            NFC_SECURE_MEMSET_THRESHOLD + 3, // just above the small-buffer threshold
            NFC_SECURE_MEMSET_THRESHOLD * 4 + 7,
        ];

        for &len in &sizes {
            let mut buf = vec![0u8; len];
            let p = buf.as_mut_ptr() as *mut libc::c_void;
            let val = ((len as u8).wrapping_mul(13)).wrapping_add(0x21);
            let rc = nfc_secure_memset(p, val as libc::c_int, len);
            assert_eq!(rc, NFC_SECURE_SUCCESS);
            for &b in &buf {
                assert_eq!(b, val);
            }
        }
    }
}

#[cfg(not(any(have_memset_explicit, have_memset_s)))]
#[test]
fn memset_fuzz_random_lengths() {
    unsafe {
        const ITER: usize = 256;
        const MAX_LEN: usize = 4096;
        // Simple deterministic xorshift32 RNG for reproducible fuzz
        let mut seed: u32 = 0xDEADBEEF;
        for _ in 0..ITER {
            // xorshift32
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let len = (seed as usize % MAX_LEN) + 1; // [1, MAX_LEN]
                                                     // allocate a slightly larger buffer so we can test unaligned offsets
            let mut buf = vec![0u8; len + 3];
            let offset = (seed as usize) % 3; // 0,1,2 to vary alignment
            let p = buf.as_mut_ptr().add(offset) as *mut libc::c_void;
            let val = ((seed >> 8) as u8).wrapping_add((len & 0xff) as u8);
            let rc = nfc_secure_memset(p, val as libc::c_int, len);
            assert_eq!(rc, NFC_SECURE_SUCCESS);
            for i in 0..len {
                assert_eq!(buf[i + offset], val);
            }
        }
    }
}

// Helper: perform small-buffer volatile writes. Tries to write machine-word
// sized chunks when the destination is suitably aligned for better
// throughput, falling back to byte-wise volatile stores otherwise.
//
// This helper is only compiled when libc/setmem-style primitives are
// unavailable so it mirrors the cfg used by the fallback paths.
#[cfg(not(any(have_memset_explicit, have_memset_s)))]
#[inline]
fn repeated_byte_pattern(byte: u8) -> usize {
    let mut pattern: usize = 0;
    let width = std::mem::size_of::<usize>();
    for shift in 0..width {
        pattern |= (byte as usize) << (shift * 8);
    }
    pattern
}

#[cfg(not(any(have_memset_explicit, have_memset_s)))]
#[inline]
unsafe fn volatile_memset(dst: *mut u8, byte: u8, len: usize) {
    let usize_bytes = std::mem::size_of::<usize>();
    if usize_bytes > 1 && (dst as usize) % usize_bytes == 0 && len >= usize_bytes {
        // Build a repeated-byte pattern for word writes using a shift-based
        // approach so every byte lane receives the exact value without
        // depending on multiplication edge cases (e.g. byte == 0xFF).
        let pattern_usize = repeated_byte_pattern(byte);
        let dst_usize = dst as *mut usize;
        let chunks = len / usize_bytes;
        for i in 0..chunks {
            unsafe { ptr::write_volatile(dst_usize.add(i), pattern_usize) };
        }
        let tail = len % usize_bytes;
        let tail_off = chunks * usize_bytes;
        for i in 0..tail {
            unsafe { ptr::write_volatile(dst.add(tail_off + i), byte) };
        }
    } else {
        for i in 0..len {
            unsafe { ptr::write_volatile(dst.add(i), byte) };
        }
    }
}

// Helper: perform libc::memset followed by a compiler fence to ensure the
// stores are not optimized away. Used for large buffers when platform
// secure primitives are unavailable.
#[cfg(not(any(have_memset_explicit, have_memset_s)))]
#[inline]
unsafe fn memset_and_fence(ptr: *mut libc::c_void, c: libc::c_int, len: usize) {
    unsafe { libc::memset(ptr, c, len as libc::size_t) };
    compiler_fence(Ordering::SeqCst);
}

fn validate_params(
    dst: *mut u8,
    dst_size: size_t,
    src: *const u8,
    src_size: size_t,
    func_name: *const c_char,
) -> c_int {
    if dst.is_null() || src.is_null() {
        return NFC_SECURE_ERROR_INVALID;
    }
    if src_size == 0 {
        return NFC_SECURE_SUCCESS;
    }
    // Defense-in-depth validation order:
    // 1. Apply individual range caps so obviously-invalid arguments (e.g.
    //    SIZE_MAX or crafted MAX/2 pairs) are rejected early.
    // 2. Guard against addition overflow for any future code that combines
    //    sizes (even though the current implementation does not) so wrapped
    //    arithmetic never proceeds silently.
    // 3. Ensure the destination buffer can actually hold the requested bytes.
    // Keeping the checks in this order prevents adversaries from crafting
    // degenerate inputs that would otherwise slip through.
    // Choose a reasonable, architecture-aware upper bound for sizes so we
    // reject obviously-invalid inputs (malicious or accidental). On 64-bit
    // platforms use the large explicit constant above; on smaller platforms
    // fall back to a fraction of the platform's max size to avoid overflow.
    let max: size_t = secure_max_size();
    if src_size > max || dst_size > max {
        return NFC_SECURE_ERROR_RANGE;
    }
    // Defend against future code paths that may add sizes together
    // (for example a naive dst_size + src_size check). Ensure the
    // arguments do not cause integer overflow when summed — treat an
    // overflow as an invalid/range error rather than relying on
    // wrapping arithmetic later in the call chain.
    if dst_size.checked_add(src_size).is_none() {
        return NFC_SECURE_ERROR_RANGE;
    }
    if dst_size < src_size {
        return NFC_SECURE_ERROR_OVERFLOW;
    }
    // When debug helpers are enabled, exercise the suspicious size
    // heuristic here so callers do not need to invoke it manually.
    #[cfg(feature = "nfc_secure_debug")]
    {
        // Safe to call the debug-only extern; the call itself is
        // performed inside an unsafe block because the FFI function is
        // declared unsafe.
        unsafe {
            // func_name may be null; the helper handles that case.
            nfc_check_suspicious_size(dst_size as size_t, func_name);
        }
    }
    NFC_SECURE_SUCCESS
}

/// Copy `src_size` bytes from `src` to `dst` after validating the
/// provided buffer sizes and pointers.
///
/// Returns one of the libnfc secure error codes:
/// - `NFC_SECURE_SUCCESS` (0) on success
/// - `NFC_SECURE_ERROR_INVALID` when `dst` or `src` is NULL
/// - `NFC_SECURE_ERROR_OVERFLOW` when `dst_size < src_size`
/// - `NFC_SECURE_ERROR_RANGE` when a supplied size is unreasonably large
///
/// # Safety
/// Both `dst` and `src` must point to valid memory regions for
/// `src_size` bytes. The regions must not overlap; use
/// `nfc_safe_memmove` when overlapping copies are required.
///
/// # Example (Rust, no_run)
/// ```no_run
/// use libnfc_rs::nfc_safe_memcpy;
/// let mut dst = [0u8; 16];
/// let src = [1u8, 2, 3, 4];
/// let rc = unsafe {
///     nfc_safe_memcpy(
///         dst.as_mut_ptr() as *mut _,
///         dst.len(),
///         src.as_ptr() as *const _,
///         src.len(),
///     )
/// };
/// assert_eq!(rc, 0);
/// ```
///
/// # C Example
/// ```c
/// #include <libnfc_rs.h>
/// #include <stdio.h>
///
/// int example_memcpy(void) {
///     char dst[16];
///     const char src[] = "hello";
///     int rc = nfc_safe_memcpy(dst, sizeof(dst), src, sizeof(src) - 1);
///     if (rc != NFC_SECURE_SUCCESS) {
///         fprintf(stderr, "memcpy failed: %s\n", nfc_secure_strerror(rc));
///         return rc;
///     }
///     dst[sizeof(src)-1] = '\0';
///     printf("copied: %s\n", dst);
///     return NFC_SECURE_SUCCESS;
/// }
/// ```
#[must_use = "Return value must be checked for errors"]
#[no_mangle]
pub unsafe extern "C" fn nfc_safe_memcpy(
    dst: *mut libc::c_void,
    dst_size: size_t,
    src: *const libc::c_void,
    src_size: size_t,
) -> c_int {
    crate::ffi_catch_unwind_int("nfc_safe_memcpy", NFC_SECURE_ERROR_INTERNAL, || {
        let res = validate_params(
            dst as *mut u8,
            dst_size,
            src as *const u8,
            src_size,
            b"nfc_safe_memcpy\0".as_ptr() as *const c_char,
        );
        if res != NFC_SECURE_SUCCESS {
            return res;
        }
        if src_size == 0 {
            return NFC_SECURE_SUCCESS;
        }
        // Debug-only alignment heuristic: warn in debug builds when the
        // destination pointer is not aligned to machine word. This does
        // not change semantics but helps developers spot accidental
        // unaligned writes which can be inefficient or cause SIGBUS on
        // some architectures.
        #[cfg(debug_assertions)]
        {
            if (dst as usize) % std::mem::align_of::<usize>() != 0 {
                crate::log_debug("nfc_safe_memcpy: destination pointer is unaligned");
            }
        }
        ptr::copy_nonoverlapping(src as *const u8, dst as *mut u8, src_size as usize);
        NFC_SECURE_SUCCESS
    })
}

/// Like `nfc_safe_memcpy` but safe for overlapping source and
/// destination ranges (semantics equivalent to `memmove`).
///
/// Returns the same set of `NFC_SECURE_*` error codes used by the
/// memcpy variant.
///
/// # Safety
/// Both `dst` and `src` must point to valid memory regions for
/// `src_size` bytes. The function allows overlap between the regions.
///
/// # Example (Rust, no_run)
/// ```no_run
/// use libnfc_rs::nfc_safe_memmove;
/// let mut buf = [0u8; 32];
/// // Move 8 bytes forward within same buffer
/// let rc = unsafe { nfc_safe_memmove(buf.as_mut_ptr() as *mut _, 8, buf.as_ptr() as *const _, 8) };
/// assert_eq!(rc, 0);
/// ```
///
/// # C Example
/// ```c
/// #include <libnfc_rs.h>
///
/// void example_memmove(void) {
///     char buf[64];
///     /* prepare buf */
///     nfc_safe_memmove(buf + 2, 32, buf, 32);
/// }
/// ```
#[must_use = "Return value must be checked for errors"]
#[no_mangle]
pub unsafe extern "C" fn nfc_safe_memmove(
    dst: *mut libc::c_void,
    dst_size: size_t,
    src: *const libc::c_void,
    src_size: size_t,
) -> c_int {
    crate::ffi_catch_unwind_int("nfc_safe_memmove", NFC_SECURE_ERROR_INTERNAL, || {
        let res = validate_params(
            dst as *mut u8,
            dst_size,
            src as *const u8,
            src_size,
            b"nfc_safe_memmove\0".as_ptr() as *const c_char,
        );
        if res != NFC_SECURE_SUCCESS {
            return res;
        }
        if src_size == 0 {
            return NFC_SECURE_SUCCESS;
        }
        ptr::copy(src as *const u8, dst as *mut u8, src_size as usize);
        NFC_SECURE_SUCCESS
    })
}

/// Securely set `size` bytes at `ptr` to the byte value `val`.
///
/// When available this function uses platform-provided secure-zeroing
/// or secure-memset routines to avoid having the compiler optimize the
/// write away. Note that some platform primitives (for example
/// `explicit_bzero` or Windows `SecureZeroMemory`) only support
/// zeroing; if the caller requests a non-zero value and the platform
/// exposes no primitive that accepts an arbitrary fill value, the
/// implementation will fall back to a standard `memset` + compiler
/// fence or a volatile-write loop. In that case the call will still
/// perform the requested fill, but it may not benefit from the same
/// platform-provided guarantees as a true secure-zero primitive.
///
/// Return codes mirror the memcpy/memmove helpers:
/// - `NFC_SECURE_SUCCESS` on success
/// - `NFC_SECURE_ERROR_INVALID` when `ptr` is NULL or a platform
///   secure API reports failure
/// - `NFC_SECURE_ERROR_RANGE` when `size` is out of acceptable range
///
/// # Safety
/// The caller must ensure `ptr` is valid for `size` bytes and writable.
///
/// # Notes
/// When available this function prefers platform primitives that the
/// system guarantees will not be optimized away (C23 `memset_explicit`,
/// `memset_s`, `explicit_bzero`, or `SecureZeroMemory`). If none are
/// present the implementation falls back to a volatile write loop for
/// small buffers and `memset` + compiler fence for larger ones.
///
/// # Security Notes
/// - Prefer `nfc_secure_zero()` when the goal is strictly zeroing
///   secret material; it prioritizes zero-only primitives when present.
/// - Validation checks are performed prior to any memory writes and
///   extremely large sizes are rejected. The implementation avoids
///   secret-dependent early returns so that control flow does not
///   depend on secret contents.
/// - This function does not provide cryptographic constant-time
///   guarantees for arbitrary operations; if constant-time behaviour
///   is required use dedicated constant-time primitives for that
///   specific purpose.
///
/// # Example (Rust, no_run)
/// ```no_run
/// use libnfc_rs::nfc_secure_memset;
/// let mut secret = [0xFFu8; 64];
/// let rc = unsafe { nfc_secure_memset(secret.as_mut_ptr() as *mut _, 0, secret.len()) };
/// assert_eq!(rc, 0);
/// ```
///
/// # C Example
/// ```c
/// #include <libnfc_rs.h>
///
/// void scrub_secret(void *ptr, size_t len) {
///     if (nfc_secure_memset(ptr, 0, len) != NFC_SECURE_SUCCESS) {
///         /* handle error */
///     }
/// }
/// ```
#[must_use = "Return value must be checked for errors"]
#[no_mangle]
pub unsafe extern "C" fn nfc_secure_memset(
    ptr: *mut libc::c_void,
    val: libc::c_int,
    size: size_t,
) -> c_int {
    crate::ffi_catch_unwind_int("nfc_secure_memset", NFC_SECURE_ERROR_INTERNAL, || {
        if ptr.is_null() {
            return NFC_SECURE_ERROR_INVALID;
        }
        if size == 0 {
            return NFC_SECURE_SUCCESS;
        }
        // Use the same reasonable-size check as validate_params so all
        // secure helpers reject obviously-invalid large sizes.
        let max: size_t = secure_max_size();
        if size > max {
            return NFC_SECURE_ERROR_RANGE;
        }
        // Normalize the value we'll write so all branches can reference
        // a single variable name (`_val`). Some platform-specific
        // primitives rely on `val` directly while the fallback paths
        // use a byte-sized form; keep `_val` as the raw c_int and cast
        // where needed.
        let _val: libc::c_int = val;

        // Prefer primitives that accept an arbitrary fill value (these
        // also cover the zeroing case):
        #[cfg(have_memset_explicit)]
        {
            extern "C" {
                fn memset_explicit(s: *mut libc::c_void, c: libc::c_int, n: libc::size_t);
            }
            unsafe { memset_explicit(ptr as *mut _, _val as libc::c_int, size as libc::size_t) };
            return NFC_SECURE_SUCCESS;
        }

        #[cfg(have_memset_s)]
        {
            extern "C" {
                fn memset_s(
                    dest: *mut libc::c_void,
                    destsz: libc::size_t,
                    ch: libc::c_int,
                    count: libc::size_t,
                ) -> libc::c_int;
            }
            let res = unsafe {
                memset_s(
                    ptr as *mut _,
                    size as libc::size_t,
                    _val as libc::c_int,
                    size as libc::size_t,
                )
            };
            if res != 0 {
                crate::log_error("nfc_secure_memset: memset_s failed");
                return NFC_SECURE_ERROR_INVALID;
            }
            return NFC_SECURE_SUCCESS;
        }

        // If the caller requested a non-zero fill and the platform does
        // not provide a primitive that accepts arbitrary values, avoid
        // calling zero-only primitives (explicit_bzero /
        // SecureZeroMemory). Instead, fall back to a best-effort
        // non-zero fill (volatile writes or libc::memset + fence).
        if _val != 0 {
            // Fallback for non-zero fills: compile this path only when
            // no memset-style primitives are available so we avoid
            // unreachable-code warnings on platforms that do provide
            // those primitives.
            #[cfg(not(any(have_memset_explicit, have_memset_s)))]
            {
                let len = size as usize;
                let dst = ptr as *mut u8;
                let byte = _val as u8;

                if len <= NFC_SECURE_MEMSET_THRESHOLD {
                    // Small buffers: use the shared volatile write helper
                    // which performs alignment-aware word writes when
                    // possible and byte-wise volatile stores otherwise.
                    volatile_memset(dst, byte, len);
                    return NFC_SECURE_SUCCESS;
                }

                // Large buffers: use libc::memset for speed and fence the
                // write so the store cannot be optimized away.
                // Large buffers: use libc::memset for speed and ensure
                // the write is not optimized away.
                memset_and_fence(ptr, _val as libc::c_int, len);
                return NFC_SECURE_SUCCESS;
            }

            // If we reach here the build provided a memset-style primitive
            // but it was handled above; falling through is intentional so
            // the zero-path below can take care of zero-only primitives.
        }

        // Zeroing path: prefer zero-only primitives if available. We only
        // reach this block either because _val == 0 or because the
        // non-zero path above was not taken/available.
        #[cfg(have_explicit_bzero)]
        {
            // Consume `val` to silence unused-variable warnings when
            // explicit_bzero is the only available primitive.
            let _ = val;
            extern "C" {
                fn explicit_bzero(s: *mut libc::c_void, n: libc::size_t);
            }
            unsafe { explicit_bzero(ptr as *mut _, size as libc::size_t) };
            return NFC_SECURE_SUCCESS;
        }

        #[cfg(have_secure_zero_memory)]
        {
            // Consume `val` to silence unused-variable warnings when
            // SecureZeroMemory is the chosen primitive.
            let _ = val;
            extern "system" {
                fn SecureZeroMemory(ptr: *mut libc::c_void, cnt: libc::size_t);
            }
            unsafe { SecureZeroMemory(ptr as *mut _, size as libc::size_t) };
            return NFC_SECURE_SUCCESS;
        }

        // Fallback for the zeroing case: only compiled when none of the
        // platform primitives were available so we avoid unreachable
        // code warnings in other builds.
        #[cfg(not(any(
            have_memset_explicit,
            have_memset_s,
            have_explicit_bzero,
            have_secure_zero_memory
        )))]
        {
            let len = size as usize;
            let dst = ptr as *mut u8;

            if len <= NFC_SECURE_MEMSET_THRESHOLD {
                // Small buffers: shared volatile write helper.
                volatile_memset(dst, _val as u8, len);
                return NFC_SECURE_SUCCESS;
            }

            // Large buffers: use libc::memset for speed, then ensure the write is not optimized away
            unsafe {
                libc::memset(ptr, _val as libc::c_int, len as libc::size_t);
            }
            compiler_fence(Ordering::SeqCst);
            return NFC_SECURE_SUCCESS;
        }
        // If any platform primitive was present the function already
        // returned; reaching here means there's nothing left to do.
    })
}

/// Securely zero `size` bytes at `ptr`.
///
/// This API is explicitly for zeroing secrets. It prefers platform
/// zeroing primitives (C23 `memset_explicit`, `memset_s`,
/// `explicit_bzero`, `SecureZeroMemory`) and will never attempt to fill
/// with non-zero bytes. Use `nfc_secure_memset` when you need to fill
/// with an arbitrary byte value.
///
/// # Security Notes
/// - This function is the recommended API for erasing secret material;
///   it prioritizes zero-only primitives that some platforms provide.
/// - All input sizes are validated prior to performing any writes and
///   unreasonable sizes are rejected with `NFC_SECURE_ERROR_RANGE`.
/// - Callers MUST check return values. This function does not attempt
///   to provide cryptographic constant-time semantics beyond avoiding
///   secret-dependent control flow during validation.
#[must_use = "Return value must be checked for errors"]
#[no_mangle]
pub unsafe extern "C" fn nfc_secure_zero(ptr: *mut libc::c_void, size: size_t) -> c_int {
    crate::ffi_catch_unwind_int("nfc_secure_zero", NFC_SECURE_ERROR_INTERNAL, || {
        if ptr.is_null() {
            return NFC_SECURE_ERROR_INVALID;
        }
        if size == 0 {
            return NFC_SECURE_SUCCESS;
        }
        let max: size_t = secure_max_size();
        if size > max {
            return NFC_SECURE_ERROR_RANGE;
        }

        #[cfg(have_memset_explicit)]
        {
            extern "C" {
                fn memset_explicit(s: *mut libc::c_void, c: libc::c_int, n: libc::size_t);
            }
            unsafe { memset_explicit(ptr as *mut _, 0 as libc::c_int, size as libc::size_t) };
            return NFC_SECURE_SUCCESS;
        }

        #[cfg(have_memset_s)]
        {
            extern "C" {
                fn memset_s(
                    dest: *mut libc::c_void,
                    destsz: libc::size_t,
                    ch: libc::c_int,
                    count: libc::size_t,
                ) -> libc::c_int;
            }
            let res =
                unsafe { memset_s(ptr as *mut _, size as libc::size_t, 0, size as libc::size_t) };
            if res != 0 {
                crate::log_error("nfc_secure_zero: memset_s failed");
                return NFC_SECURE_ERROR_INVALID;
            }
            return NFC_SECURE_SUCCESS;
        }

        #[cfg(have_explicit_bzero)]
        {
            extern "C" {
                fn explicit_bzero(s: *mut libc::c_void, n: libc::size_t);
            }
            unsafe { explicit_bzero(ptr as *mut _, size as libc::size_t) };
            return NFC_SECURE_SUCCESS;
        }

        #[cfg(have_secure_zero_memory)]
        {
            extern "system" {
                fn SecureZeroMemory(ptr: *mut libc::c_void, cnt: libc::size_t);
            }
            unsafe { SecureZeroMemory(ptr as *mut _, size as libc::size_t) };
            return NFC_SECURE_SUCCESS;
        }

        #[cfg(not(any(
            have_memset_explicit,
            have_memset_s,
            have_explicit_bzero,
            have_secure_zero_memory
        )))]
        {
            let len = size as usize;
            let dst = ptr as *mut u8;
            if len <= NFC_SECURE_MEMSET_THRESHOLD {
                for i in 0..len {
                    unsafe { ptr::write_volatile(dst.add(i), 0u8) };
                }
                return NFC_SECURE_SUCCESS;
            }
            // Large buffers: use libc::memset for speed and ensure the
            // write is not optimized away.
            memset_and_fence(ptr, 0 as libc::c_int, len);
            return NFC_SECURE_SUCCESS;
        }
    })
}

/// Return a static NUL-terminated message describing `code`.
///
/// The returned pointer references a static string owned by the
/// library and MUST NOT be freed by the caller.
///
/// # Example (Rust, no_run)
/// ```no_run
/// use libnfc_rs::nfc_secure_strerror;
/// let msg = unsafe { nfc_secure_strerror(0) };
/// // msg points to a static C string; don't free it from Rust
/// ```
///
/// # C Example
/// ```c
/// #include <libnfc_rs.h>
/// #include <stdio.h>
///
/// void show_error(int code) {
///     printf("error: %s\n", nfc_secure_strerror(code));
/// }
/// ```
#[no_mangle]
pub extern "C" fn nfc_secure_strerror(code: c_int) -> *const c_char {
    match code {
        NFC_SECURE_SUCCESS => b"Success\0".as_ptr() as *const c_char,
        NFC_SECURE_ERROR_INVALID => {
            b"Invalid parameter (NULL pointer or invalid input)\0".as_ptr() as *const c_char
        }
        NFC_SECURE_ERROR_OVERFLOW => {
            b"Buffer overflow prevented (destination too small)\0".as_ptr() as *const c_char
        }
        NFC_SECURE_ERROR_RANGE => b"Size parameter out of valid range\0".as_ptr() as *const c_char,
        NFC_SECURE_ERROR_ZERO_SIZE => {
            b"Zero-size operation (deprecated, now treated as success)\0".as_ptr() as *const c_char
        }
        _ => b"Unknown error code\0".as_ptr() as *const c_char,
    }
}

/// Compute the length of a NUL-terminated C string but never read
/// past `maxlen` bytes.
///
/// Returns the number of bytes before the first NUL or `0` when
/// `str` is NULL. The return value is bounded by `maxlen`.
///
/// # Example (Rust, no_run)
/// ```no_run
/// use libnfc_rs::nfc_safe_strlen;
/// let s = std::ffi::CString::new("hello").unwrap();
/// let len = unsafe { nfc_safe_strlen(s.as_ptr(), 100) };
/// assert_eq!(len as usize, 5);
/// ```
///
/// # C Example
/// ```c
/// #include <libnfc_rs.h>
/// #include <stdio.h>
///
/// void example_strlen(const char *s) {
///     size_t l = nfc_safe_strlen(s, 100);
///     printf("len=%zu\n", l);
/// }
/// ```
#[no_mangle]
pub unsafe extern "C" fn nfc_safe_strlen(str: *const c_char, maxlen: size_t) -> size_t {
    if str.is_null() {
        return 0;
    }
    let mut len: usize = 0;
    while len < (maxlen as usize) {
        let b = *(str.add(len) as *const u8);
        if b == 0 {
            break;
        }
        len += 1;
    }
    len as size_t
}

/// Inspect `buf` up to `bufsize` bytes and return `1` if a NUL
/// terminator is found, otherwise return `0`.
///
/// `buf` may be NULL; a NULL pointer yields `0`.
///
/// Note: this helper operates on raw bytes and does not validate
/// UTF-8 or any multibyte encoding; it simply searches for the NUL
/// byte (0x00) inside the provided byte range.
///
/// # Example (Rust, no_run)
/// ```no_run
/// use libnfc_rs::nfc_is_null_terminated;
/// let buf = ['A' as i8, 0, 'B' as i8];
/// let ok = unsafe { nfc_is_null_terminated(buf.as_ptr() as *const _, 3) };
/// assert_eq!(ok, 1);
/// ```
///
/// # C Example
/// ```c
/// #include <libnfc_rs.h>
///
/// int check_buffer(const char *buf, size_t size) {
///     return nfc_is_null_terminated(buf, size);
/// }
/// ```
#[no_mangle]
pub unsafe extern "C" fn nfc_is_null_terminated(buf: *const c_char, bufsize: size_t) -> c_int {
    if buf.is_null() || bufsize == 0 {
        return 0;
    }
    let mut i: usize = 0;
    while i < (bufsize as usize) {
        if *buf.add(i) as u8 == 0 {
            return 1;
        }
        i += 1;
    }
    0
}

/// Ensure a buffer of size `bufsize` contains a terminating NUL.
///
/// If no NUL is found within the first `bufsize` bytes the last
/// byte (`buf[bufsize-1]`) is set to `0`. If `buf` is NULL or
/// `bufsize` is zero the function returns immediately.
///
/// Note: this helper only ensures a NUL byte exists inside the
/// provided range; it does not perform any UTF-8 validation.
///
/// # Example (Rust, no_run)
/// ```no_run
/// use libnfc_rs::nfc_ensure_null_terminated;
/// let mut buf = [b'A' as i8; 4];
/// unsafe { nfc_ensure_null_terminated(buf.as_mut_ptr() as *mut _, 4) };
/// ```
///
/// # C Example
/// ```c
/// #include <libnfc_rs.h>
///
/// void ensure_buf(char *buf, size_t size) {
///     nfc_ensure_null_terminated(buf, size);
/// }
/// ```
#[no_mangle]
pub unsafe extern "C" fn nfc_ensure_null_terminated(buf: *mut c_char, bufsize: size_t) {
    if buf.is_null() || bufsize == 0 {
        return;
    }
    let mut found_null = false;
    let mut i: usize = 0;
    while i < (bufsize as usize) {
        if *buf.add(i) as u8 == 0 {
            found_null = true;
            break;
        }
        i += 1;
    }
    if !found_null {
        // Overwrite last byte with NUL
        *buf.add(bufsize as usize - 1) = 0;
    }
}

/// Debug helper (enabled with `nfc_secure_debug`) that detects
/// whether two memory ranges overlap. Returns `1` on overlap and
/// `0` otherwise.
///
/// # Safety
/// Pointers must be valid for the provided sizes or NULL.
#[cfg(feature = "nfc_secure_debug")]
#[no_mangle]
pub unsafe extern "C" fn nfc_buffers_overlap(
    dst: *const libc::c_void,
    dst_size: size_t,
    src: *const libc::c_void,
    src_size: size_t,
) -> c_int {
    if dst.is_null() || src.is_null() {
        return 0;
    }
    let dst_ptr = dst as usize;
    let src_ptr = src as usize;
    let dst_len = dst_size as usize;
    let src_len = src_size as usize;
    // Use checked_add to avoid overflow when computing range ends.
    if dst_ptr >= src_ptr {
        let src_end = src_ptr.checked_add(src_len);
        if src_end.map_or(false, |end| dst_ptr < end) {
            return 1;
        }
    }
    if src_ptr >= dst_ptr {
        let dst_end = dst_ptr.checked_add(dst_len);
        if dst_end.map_or(false, |end| src_ptr < end) {
            return 1;
        }
    }
    0
}

// Test-only helper that performs the same overlap computation using
// usize values instead of raw pointers. This is useful for tests that
// want to model extreme address values without creating potentially
// invalid pointer values. The logic mirrors `nfc_buffers_overlap` and
// returns 1 for overlap, 0 otherwise.
#[cfg(any(test, feature = "test_helpers"))]
pub fn nfc_buffers_overlap_usize(
    dst_addr: usize,
    dst_size: usize,
    src_addr: usize,
    src_size: usize,
) -> c_int {
    // If either address is zero, consider it non-overlapping (matches
    // the behavior of the pointer-based implementation which returns
    // 0 for NULL inputs).
    if dst_addr == 0 || src_addr == 0 {
        return 0;
    }
    if dst_addr >= src_addr {
        let src_end = src_addr.checked_add(src_size);
        if src_end.map_or(false, |end| dst_addr < end) {
            return 1;
        }
    }
    if src_addr >= dst_addr {
        let dst_end = dst_addr.checked_add(dst_size);
        if dst_end.map_or(false, |end| src_addr < end) {
            return 1;
        }
    }
    0
}

// Test helpers: expose small utilities for integration tests when the
// `test_helpers` feature is enabled. These are intentionally minimal
// and mirror internal constants/behaviour so tests can assert on
// boundary conditions without reaching into private internals.
#[cfg(any(test, feature = "test_helpers"))]
pub fn nfc_secure_memset_threshold() -> usize {
    NFC_SECURE_MEMSET_THRESHOLD
}

#[cfg(any(test, feature = "test_helpers"))]
pub fn nfc_secure_max_reasonable_size() -> usize {
    NFC_SECURE_MAX_REASONABLE_SIZE_64
}

#[cfg(any(test, feature = "test_helpers"))]
pub fn nfc_secure_max_size_usize() -> usize {
    secure_max_size() as usize
}

// Re-export small volatile helpers only when the build actually
// compiles the volatile fallback path.
#[cfg(all(
    any(test, feature = "test_helpers"),
    not(any(have_memset_explicit, have_memset_s))
))]
#[inline]
pub unsafe fn nfc_volatile_memset(dst: *mut u8, byte: u8, len: usize) {
    volatile_memset(dst, byte, len)
}

#[cfg(all(
    any(test, feature = "test_helpers"),
    not(any(have_memset_explicit, have_memset_s))
))]
#[inline]
pub unsafe fn nfc_memset_and_fence(ptr: *mut libc::c_void, c: libc::c_int, len: usize) {
    memset_and_fence(ptr, c, len)
}

/// Debug-only heuristic that logs a warning when `dst_size` looks
/// suspicious (for example, equals pointer-size or is a small power
/// of two). This helps detect accidental misuse where a pointer or
/// a byte count was passed instead of an array size.
///
/// Enabled only when the crate is compiled with
/// `--features nfc_secure_debug`.
#[cfg(feature = "nfc_secure_debug")]
#[no_mangle]
pub unsafe extern "C" fn nfc_check_suspicious_size(dst_size: size_t, func_name: *const c_char) {
    // Helper: small utility to detect power-of-two sizes
    fn is_power_of_2(n: usize) -> bool {
        n != 0 && (n & (n - 1)) == 0
    }
    // Heuristic: if dst_size equals pointer size and is small (<=16), warn
    let ptr_size = std::mem::size_of::<*const libc::c_void>();
    let sz = dst_size as usize;
    if (sz == ptr_size && sz <= 16) || (is_power_of_2(sz) && sz <= 16) {
        let func = if func_name.is_null() {
            "<unknown>"
        } else {
            match CStr::from_ptr(func_name).to_str() {
                Ok(s) => s,
                Err(_) => "<non-utf8>",
            }
        };
        let msg = format!(
            "{}: WARNING - dst_size={} matches pointer size ({} bytes). Did you pass a pointer instead of an array?",
            func, sz, ptr_size
        );
        // Use the crate-level logging helper
        crate::log_error(&msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn memcpy_success() {
        unsafe {
            let mut dst = [0u8; 8];
            let src = [1u8, 2, 3, 4];
            let rc = nfc_safe_memcpy(
                dst.as_mut_ptr() as *mut _,
                dst.len(),
                src.as_ptr() as *const _,
                src.len(),
            );
            assert_eq!(rc, NFC_SECURE_SUCCESS);
            assert_eq!(&dst[..4], &src);
        }
    }

    #[test]
    fn memcpy_overflow() {
        unsafe {
            let mut dst = [0u8; 2];
            let src = [1u8, 2, 3, 4];
            let rc = nfc_safe_memcpy(
                dst.as_mut_ptr() as *mut _,
                dst.len(),
                src.as_ptr() as *const _,
                src.len(),
            );
            assert_eq!(rc, NFC_SECURE_ERROR_OVERFLOW);
        }
    }

    #[test]
    fn memset_zero() {
        unsafe {
            let mut buf = [0xFFu8; 4];
            let rc = nfc_secure_memset(buf.as_mut_ptr() as *mut _, 0, buf.len());
            assert_eq!(rc, NFC_SECURE_SUCCESS);
            assert_eq!(buf, [0u8; 4]);
        }
    }

    #[test]
    fn strlen_null_and_bounds() {
        unsafe {
            // NULL pointer returns 0
            assert_eq!(nfc_safe_strlen(std::ptr::null(), 10), 0);

            let s = CString::new("hello").unwrap();
            // normal case
            assert_eq!(nfc_safe_strlen(s.as_ptr(), 100) as usize, 5);
            // maxlen smaller than actual length
            assert_eq!(nfc_safe_strlen(s.as_ptr(), 3) as usize, 3);

            // buffer without NUL in the first N bytes
            let v = vec![b'A'; 6];
            let p = v.as_ptr() as *const c_char;
            assert_eq!(nfc_safe_strlen(p, 6) as usize, 6);
        }
    }

    #[test]
    fn null_terminated_helpers() {
        unsafe {
            // is_null_terminated: NULL -> 0
            assert_eq!(nfc_is_null_terminated(std::ptr::null(), 10), 0);

            // buffer with NUL in range (create bytes with interior NUL)
            let inner = vec![b'a', b'b', 0u8, b'c', b'd'];
            let p_inner = inner.as_ptr() as *const c_char;
            assert_eq!(nfc_is_null_terminated(p_inner, 5), 1);

            // buffer without NUL in first N
            let mut v = vec![b'X'; 4];
            let p = v.as_mut_ptr() as *mut c_char;
            assert_eq!(nfc_is_null_terminated(p as *const c_char, 4), 0);

            // ensure_null_terminated modifies last byte
            nfc_ensure_null_terminated(p, 4);
            assert_eq!(*p.add(3) as u8, 0);

            // already terminated case: should leave existing terminator
            let mut buf = [b'A', b'\0', b'B'];
            let pb = buf.as_mut_ptr() as *mut c_char;
            nfc_ensure_null_terminated(pb, 3);
            assert_eq!(buf[1], 0);
        }
    }

    #[cfg(feature = "nfc_secure_debug")]
    #[test]
    fn buffers_overlap_detects_overlap() {
        unsafe {
            let mut a = [0u8; 8];
            let pa = a.as_mut_ptr() as *mut libc::c_void;
            // overlapping: dst starts at a[2], src at a[0]
            let dst = pa.add(2) as *const libc::c_void;
            let src = pa as *const libc::c_void;
            assert_eq!(nfc_buffers_overlap(dst, 4, src, 4), 1);

            // non-overlap
            let mut b = [0u8; 8];
            let pb = b.as_mut_ptr() as *const libc::c_void;
            assert_eq!(nfc_buffers_overlap(pb, 4, pb.add(4), 4), 0);
        }
    }

    #[cfg(feature = "nfc_secure_debug")]
    #[test]
    fn suspicious_size_logs_warning() {
        unsafe {
            crate::test_clear_last_log();
            let psz = std::mem::size_of::<*const libc::c_void>();
            let name = CString::new("check_test").unwrap();
            nfc_check_suspicious_size(psz as size_t, name.as_ptr());
            let logged = crate::test_get_last_log();
            assert!(logged.is_some());
            assert!(logged.unwrap().contains("WARNING - dst_size="));
        }
    }

    // end suspicious_size_logs_warning

    #[cfg(feature = "nfc_secure_debug")]
    #[test]
    fn memcpy_triggers_suspicious_size_warning() {
        unsafe {
            crate::test_clear_last_log();
            let psz = std::mem::size_of::<*const libc::c_void>();
            let mut dst = vec![0u8; psz];
            let src = vec![1u8; psz];
            // call memcpy with dst_size equal to pointer size to trigger heuristic
            let rc = nfc_safe_memcpy(
                dst.as_mut_ptr() as *mut _,
                psz as size_t,
                src.as_ptr() as *const _,
                1,
            );
            assert_eq!(rc, NFC_SECURE_SUCCESS);
            let logged = crate::test_get_last_log();
            assert!(logged.is_some());
            assert!(logged.unwrap().contains("WARNING - dst_size="));
        }
    }

    #[cfg(feature = "nfc_secure_debug")]
    #[test]
    fn memmove_triggers_suspicious_size_warning() {
        unsafe {
            crate::test_clear_last_log();
            let psz = std::mem::size_of::<*const libc::c_void>();
            let mut dst = vec![0u8; psz];
            let src = vec![1u8; psz];
            // call memmove with dst_size equal to pointer size to trigger heuristic
            let rc = nfc_safe_memmove(
                dst.as_mut_ptr() as *mut _,
                psz as size_t,
                src.as_ptr() as *const _,
                1,
            );
            assert_eq!(rc, NFC_SECURE_SUCCESS);
            let logged = crate::test_get_last_log();
            assert!(logged.is_some());
            assert!(logged.unwrap().contains("WARNING - dst_size="));
        }
    }

    #[test]
    fn memset_large_zeroes_buffer() {
        unsafe {
            let mut buf = vec![0xFFu8; 512];
            let p = buf.as_mut_ptr() as *mut libc::c_void;
            let rc = nfc_secure_memset(p, 0, buf.len());
            assert_eq!(rc, NFC_SECURE_SUCCESS);
            for &b in &buf {
                assert_eq!(b, 0);
            }
        }
    }

    #[test]
    fn memset_null_ptr_returns_invalid() {
        unsafe {
            let rc = nfc_secure_memset(std::ptr::null_mut(), 0, 10);
            assert_eq!(rc, NFC_SECURE_ERROR_INVALID);
        }
    }

    #[test]
    fn memset_size_range_checks() {
        unsafe {
            // Very large size should be rejected
            let mut buf = vec![0u8; 8];
            // Use a size greater than SIZE_MAX/2 simulated by using a huge usize (truncate on 64-bit)
            let large = (usize::MAX / 2) + 100usize;
            let rc = nfc_secure_memset(buf.as_mut_ptr() as *mut _, 0, large);
            assert_eq!(rc, NFC_SECURE_ERROR_RANGE);
        }
    }

    #[test]
    fn memset_nonzero_sets_value() {
        unsafe {
            let mut buf = vec![0u8; 64];
            let p = buf.as_mut_ptr() as *mut libc::c_void;
            let rc = nfc_secure_memset(p, 0x5A, buf.len());
            assert_eq!(rc, NFC_SECURE_SUCCESS);
            for &b in &buf {
                assert_eq!(b, 0x5A);
            }
        }
    }

    #[test]
    fn secure_zero_zeros_buffer() {
        unsafe {
            let mut buf = vec![0xFFu8; 64];
            let p = buf.as_mut_ptr() as *mut libc::c_void;
            let rc = nfc_secure_zero(p, buf.len());
            assert_eq!(rc, NFC_SECURE_SUCCESS);
            for &b in &buf {
                assert_eq!(b, 0u8);
            }
        }
    }

    #[test]
    fn buffers_overlap_handles_overflow_values() {
        unsafe {
            // Use extreme addresses simulated as usize values that would
            // cause an addition overflow if naively added. To be explicit
            // and avoid inline integer->pointer casts we keep the usize
            // representations and then cast to pointers for the call.
            // We do not dereference these pointers; they are only used for
            // arithmetic checks inside `nfc_buffers_overlap`.
            let large_addr = usize::MAX - 1usize;
            let small_addr = 8usize;
            // Use the usize-based overlap helper to avoid creating
            // potentially invalid pointer values from arbitrary usize
            // values. This computes overlap purely on arithmetic.
            assert_eq!(nfc_buffers_overlap_usize(large_addr, 16, small_addr, 4), 0);
        }
    }

    #[test]
    fn memset_rejects_unreasonable_size_constant() {
        unsafe {
            let mut buf = vec![0u8; 8];
            let large = (NFC_SECURE_MAX_REASONABLE_SIZE_64 as usize) + 1usize;
            let rc = nfc_secure_memset(buf.as_mut_ptr() as *mut _, 0, large as size_t);
            assert_eq!(rc, NFC_SECURE_ERROR_RANGE);
        }
    }
}

#[cfg(all(test, feature = "asan_tests"))]
mod asan_tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn asan_buffer_overflow_detected() {
        const TEST_NAME: &str = "asan_tests::asan_buffer_overflow_detected";

        if std::env::var("ASAN_TEST_CHILD").is_ok() {
            let mut buf = vec![0u8; 8];
            unsafe {
                ptr::write_volatile(buf.as_mut_ptr().add(16), 0u8);
            }
            // Reaching this exit means ASan did not abort; return success so
            // the parent can detect the lack of sanitizer intervention.
            std::process::exit(0);
        }

        let exe = std::env::current_exe().expect("locate test binary");
        let status = Command::new(&exe)
            .arg("--exact")
            .arg(TEST_NAME)
            .env("ASAN_TEST_CHILD", "1")
            .status()
            .expect("spawn ASan test child");

        assert!(
            !status.success(),
            "ASan should abort the child process when buffer overflow occurs"
        );
    }
}
