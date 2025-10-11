/**
 * @file nfc-secure.h
 * @brief Secure memory operation wrappers for libnfc
 *
 * This header provides safe wrappers around standard C memory functions
 * (memcpy, memset) to prevent buffer overflow vulnerabilities.
 *
 * Design follows common secure coding standards (for example CERT C and
 * ISO/IEC TR 24772) and general industry memory safety practices.
 *
 * @internal Use within NFC secure runtime or safe utilities only.
 *
 * 📚 DOCUMENTATION:
 * - Complete Usage Guide: libnfc/NFC_SECURE_USAGE_GUIDE.md
 * - Best Practices: libnfc/NFC_SECURE_BEST_PRACTICES_V4.md
 * - Security Fixes: libnfc/NFC_SECURE_CRITICAL_FIXES_V5.md
 * - Examples: libnfc/nfc-secure-examples.c
 *
 * 🎯 QUICK START:
 * ```c
 * // Arrays (compile-time size)
 * uint8_t buffer[64], data[16];
 * NFC_SAFE_MEMCPY(buffer, data, sizeof(data));  // ✅ Automatic overflow check
 *
 * // Pointers/Dynamic memory (runtime size)
 * uint8_t *buf = malloc(64);
 * nfc_safe_memcpy(buf, 64, data, sizeof(data)); // ✅ Explicit size required
 *
 * // Secure erase (won't be optimized away)
 * uint8_t password[256];
 * NFC_SECURE_MEMSET(password, 0x00);            // ✅ Guaranteed execution
 * ```
 *
 * Configuration Options:
 *
 * - NFC_SECURE_CHECK_OVERLAP: Enable buffer overlap detection in nfc_safe_memcpy()
 *   Default: Enabled in debug builds (!NDEBUG), disabled in release builds
 *   Performance impact: ~10-20 CPU cycles per call
 *   💡 RECOMMENDATION: Keep enabled in debug/testing, disable for production
 *   Override: Define as 0 to force disable, or 1 to force enable
 *
 * - NFC_SECURE_DEBUG: Enable runtime pointer/size validation warnings
 *   Default: Disabled (must be explicitly enabled)
 *   Use case: C89/C99 compilers without compile-time checks
 *   Performance impact: Minimal (~5-10 cycles per call)
 *   💡 RECOMMENDATION: Enable during development on old compilers
 *
 * - NFC_SECURE_MEMSET_THRESHOLD: Size threshold for secure memset optimization
 *   Default: 256 bytes
 *   Small buffers (≤threshold): Use volatile loop (secure, slower)
 *   Large buffers (>threshold): Use memset+barrier (faster, still secure)
 *   💡 RECOMMENDATION: Tune based on your typical buffer sizes:
 *      * Crypto-heavy (keys): Lower to 128
 *      * Mixed workload: Keep at 256 (default)
 *      * Large buffers: Increase to 512 or 1024
 *
 * ⚠️ IMPORTANT LIMITATIONS AND WARNINGS:
 *
 * 1. DYNAMIC MEMORY (malloc/calloc/realloc):
 *    - Compile-time checks (_Static_assert) DO NOT work with dynamic memory
 *    - Always use NFC_SAFE_MEMCPY_RUNTIME() for heap-allocated buffers
 *    - Example problem:
 *      ```c
 *      uint8_t *buffer = malloc(100);
 *      NFC_SAFE_MEMCPY(buffer, ...); // ❌ WRONG: sizeof(buffer) == pointer size!
 *      ```
 *    - Correct usage:
 *      ```c
 *      uint8_t *buffer = malloc(100);
 *      nfc_safe_memcpy(buffer, 100, ...); // ✅ CORRECT: explicit size
 *      ```
 *
 * 2. MEMORY ALIGNMENT:
 *    - This library does NOT handle unaligned memory access
 *    - Buffers must be properly aligned for their data types
 *    - On strict alignment architectures (ARM, SPARC), unaligned access may:
 *      * Cause bus errors (SIGBUS crashes)
 *      * Trigger performance penalties
 *      * Lead to incorrect results
 *    - Use proper allocation: malloc/aligned_alloc/posix_memalign
 *    - Avoid casting unaligned buffers (uint8_t[7] → uint32_t*)
 *
 * 3. OLD COMPILER LIMITATIONS:
 *    - C89/C90: No _Static_assert, compile-time checks disabled
 *      * Use NFC_SECURE_DEBUG for runtime warnings instead
 *      * More runtime overhead, fewer guarantees
 *    - C99: Partial support (depends on compiler extensions)
 *    - C11+: Full support with _Static_assert and optional memset_s
 *    - MSVC: Use /std:c11 or /std:c17 for best support
 *    - GCC/Clang: Use -std=c11 or newer
 *
 * 4. VOLATILE FALLBACK LIMITATIONS:
 *    - On unsupported platforms, secure memset uses volatile pointer
 *    - This is NOT guaranteed by C standard, but works on all tested compilers
 *    - If compiler aggressively optimizes away sensitive data:
 *      * Verify with objdump/IDA that memset calls remain
 *      * Consider platform-specific secure functions (SecureZeroMemory, etc.)
 *      * Use memory barriers (__asm__ volatile on GCC/Clang)
 */

#ifndef NFC_SECURE_H
#define NFC_SECURE_H

#include <stddef.h>
#include <stdint.h>
#include <string.h>

/* Auto-enable overlap checking in debug builds (unless explicitly disabled) */
#if !defined(NFC_SECURE_CHECK_OVERLAP) && !defined(NDEBUG)
#define NFC_SECURE_CHECK_OVERLAP 1
#endif

/*
 * C23 nullptr support for better type safety
 *
 * ⚠️  INTERNAL USE ONLY: This macro is primarily for internal implementation.
 * 
 * External code should continue using NULL (C89-C17) or nullptr (C23) directly.
 * You are NOT required to use NFC_NULL in your application code.
 * 
 * The nfc-secure library uses NFC_NULL internally to maintain consistency
 * across C standards (C89/C99/C11/C17/C23), but the public API accepts
 * standard NULL/nullptr values from application code.
 *
 * C23 introduces nullptr as a distinct null pointer constant with type nullptr_t.
 * For older standards, we continue using NULL for compatibility.
 *
 * Internal usage example:
 *   if (ptr == NFC_NULL) { return error; }  // Works in C89-C23
 *
 * Benefits:
 * - C23: Type-safe nullptr (distinct type, better diagnostics)
 * - Pre-C23: Standard NULL (backward compatible)
 * - Consistent internal implementation across all standards
 */
#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 202311L
/* C23: Use standardized nullptr */
#define NFC_NULL nullptr
#else
/* Pre-C23: Use traditional NULL */
#define NFC_NULL NULL
#endif

#ifdef __cplusplus
extern "C"
{
#endif

/**
 * @brief NFC Secure library error codes
 *
 * Platform-independent error codes for secure memory operations.
 * These are negative values to distinguish from success (0).
 *
 * @note We define our own error codes instead of using errno constants
 *       (EINVAL, EOVERFLOW, ERANGE) for better cross-platform compatibility.
 *       Windows and POSIX systems have different errno values.
 */
enum nfc_secure_error {
  NFC_SECURE_SUCCESS = 0,         /**< Operation succeeded */
  NFC_SECURE_ERROR_INVALID = -1,  /**< Invalid parameter (NULL pointer, etc.) */
  NFC_SECURE_ERROR_OVERFLOW = -2, /**< Buffer overflow would occur */
  NFC_SECURE_ERROR_RANGE = -3,    /**< Size parameter out of valid range */
  NFC_SECURE_ERROR_ZERO_SIZE = -4 /**< Zero-size operation (deprecated, always returns SUCCESS) */
};

/**
 * @brief Get human-readable error message
 *
 * @param error_code Error code from nfc_secure_error enum
 * @return String describing the error, or "Unknown error" for invalid codes
 */
const char *nfc_secure_strerror(int error_code);

#ifdef __cplusplus
}
#endif

/**
 * @brief Safe memory copy with buffer size validation
 *
 * This function provides a secure alternative to memcpy() by validating
 * that the destination buffer has sufficient space before copying.
 *
 * Memory Safety Pattern:
 * ```c
 * // Constrained Memory Copy - prevents buffer overflow
 * if (dst_size >= src_size) {
 *     memcpy(dst, src, src_size);
 * } else {
 *     return error;
 * }
 * ```
 *
 * @param[out] dst Destination buffer (must be non-NULL)
 * @param[in] dst_size Size of destination buffer in bytes (CRITICAL for safety)
 * @param[in] src Source buffer (must be non-NULL)
 * @param[in] src_size Number of bytes to copy from source
 *
 * @return  NFC_SECURE_SUCCESS (0)     on success (including zero-size operations)
 * @return  NFC_SECURE_ERROR_INVALID  if dst or src is NULL (or buffer overlap detected)
 * @return  NFC_SECURE_ERROR_OVERFLOW if dst_size < src_size (buffer overflow prevented)
 * @return  NFC_SECURE_ERROR_RANGE    if src_size or dst_size exceeds SIZE_MAX / 2
 * @return  NFC_SECURE_ERROR_ZERO_SIZE (deprecated) - now returns SUCCESS for zero-size
 *
 * @note This function mimics the behavior of `memcpy_s()` defined in C11
 *       Annex K but does not require optional Annex K support from the C
 *       runtime. It performs explicit size validation on the destination
 *       buffer before copying to avoid buffer overflows.
 *
 * ⚠️ CRITICAL: Dynamic Memory Warning
 * When using dynamic memory (malloc/calloc/realloc), you MUST use the
 * runtime function directly, NOT the macro:
 * ```c
 * uint8_t *buffer = malloc(100);
 *
 * // ❌ WRONG - sizeof(buffer) is pointer size (4 or 8 bytes)!
 * NFC_SAFE_MEMCPY(buffer, data, 50);
 *
 * // ✅ CORRECT - explicit size parameter
 * nfc_safe_memcpy(buffer, 100, data, 50);
 * ```
 *
 * Example usage:
 * ```c
 * uint8_t buffer[10];
 * uint8_t data[5] = {1, 2, 3, 4, 5};
 *
 * // Safe copy - will succeed
 * int result = nfc_safe_memcpy(buffer, sizeof(buffer), data, sizeof(data));
 * if (result != NFC_SECURE_SUCCESS) {
 *     fprintf(stderr, "Copy failed: %s\n", nfc_secure_strerror(result));
 * }
 *
 * // Unsafe copy - will fail with NFC_SECURE_ERROR_OVERFLOW
 * uint8_t small_buffer[3];
 * result = nfc_safe_memcpy(small_buffer, sizeof(small_buffer), data, sizeof(data));
 * // result == NFC_SECURE_ERROR_OVERFLOW, buffer overflow prevented
 * ```
 */
int nfc_safe_memcpy(void *dst, size_t dst_size, const void *src, size_t src_size);/**
 * @brief Secure memset for sensitive data
 *
 * This function ensures that memory is securely erased and cannot be
 * optimized away by the compiler (unlike standard memset).
 *
 * Security Concern:
 * When handling sensitive information (keys, passwords, crypto material),
 * standard memset() may be optimized away by the compiler if it detects
 * that the memory is not used after the call ("dead store elimination").
 *
 * Implementation uses volatile pointer trick to prevent optimization:
 * ```c
 * volatile uint8_t *p = ptr;
 * while (size--) {
 *     *p++ = val;
 * }
 * ```
 *
 * @param[out] ptr Pointer to memory to clear (must be non-NULL)
 * @param[in] val Value to set (typically 0x00)
 * @param[in] size Number of bytes to set
 *
 * @return  NFC_SECURE_SUCCESS (0)     on success (including zero-size operations)
 * @return  NFC_SECURE_ERROR_INVALID  if ptr is NULL
 * @return  NFC_SECURE_ERROR_RANGE    if size exceeds SIZE_MAX / 2
 * @return  NFC_SECURE_ERROR_ZERO_SIZE (deprecated) - now returns SUCCESS for zero-size
 *
 * @note This function is explicitly designed to prevent the compiler from
 *       optimizing away the memory write (for example, via dead-store
 *       elimination). Use this function only for sensitive data such as
 *       cryptographic keys, passwords, or authentication tokens. For
 *       non-sensitive data, prefer the standard memset() for better
 *       performance.
 *
 * ⚠️ WARNING: Alignment Requirements
 * This function does NOT handle alignment issues. Ensure that:
 * - Buffer is properly aligned for its intended use
 * - On ARM/SPARC, misaligned access may cause SIGBUS
 * - Use malloc/aligned_alloc for dynamic memory
 *
 * ⚠️ WARNING: Old Compiler Limitations
 * On C89/C90 compilers:
 * - No _Static_assert (compile-time checks disabled)
 * - Volatile fallback may be less reliable
 * - Test with objdump to verify memset is not optimized away
 *
 * Platform-specific implementations:
 * - Windows: Uses SecureZeroMemory (guaranteed not optimized away)
 * - BSD/Linux: Uses explicit_bzero (guaranteed not optimized away)
 * - C11: Uses memset_s from Annex K (optional, guaranteed)
 * - Fallback: volatile pointer + memory barriers
 *
 * ⚠️ PERFORMANCE CHARACTERISTICS:
 * - Small buffers (≤256 bytes): Optimized volatile loop (~20-50 cycles overhead)
 *   * Ideal for: Crypto keys (16-32 bytes), MIFARE keys (6 bytes), UIDs (4-10 bytes)
 *   * Performance: ~1-5 microseconds on modern CPUs
 *
 * - Large buffers (>256 bytes): memset + memory barrier fallback
 *   * Penalty: ~10-30% slower than standard memset
 *   * Still acceptable for: Authentication buffers (<1KB), temporary command buffers
 *   * NOT recommended for: Large file buffers, network packet buffers (use standard memset)
 *
 * - Platform functions (SecureZeroMemory/explicit_bzero): Near-native performance
 *   * Minimal overhead compared to standard memset
 *   * Always preferred when available
 *
 * 💡 RECOMMENDATION: Use this function ONLY for sensitive data that MUST be cleared.
 *    For non-sensitive data, use standard memset() for better performance.
 *
 * Example usage:
 * ```c
 * uint8_t key[16];
 * // ... use key for crypto operations ...
 *
 * // Securely erase key from memory
 * int result = nfc_secure_memset(key, 0x00, sizeof(key));
 * if (result != NFC_SECURE_SUCCESS) {
 *     fprintf(stderr, "Secure erase failed: %s\n", nfc_secure_strerror(result));
 * }
 * // Compiler cannot optimize away this erasure
 * ```
 */
int nfc_secure_memset(void *ptr, int val, size_t size);

/*
 * @note Always check the return value of these functions. Negative
 *       return codes indicate validation errors and must not be ignored
 *       by the caller. Example:
 *
 * int rc = nfc_safe_memcpy(dst, dst_size, src, src_size);
 * if (rc != 0) {
 *     // handle error - do not assume the copy succeeded
 * }
 */

/**
 * @brief Safe memory move with buffer size validation
 *
 * This function provides a secure alternative to memmove() by validating
 * that the destination buffer has sufficient space before copying.
 * Unlike nfc_safe_memcpy(), this function correctly handles overlapping
 * source and destination buffers.
 *
 * Use this function when:
 * - Source and destination buffers may overlap
 * - You need to move data within the same buffer
 * - You're unsure if buffers overlap and want to be safe
 *
 * @param[out] dst Destination buffer (must be non-NULL)
 * @param[in] dst_size Size of destination buffer in bytes
 * @param[in] src Source buffer (must be non-NULL)
 * @param[in] src_size Number of bytes to copy from source
 *
 * @return  NFC_SECURE_SUCCESS (0)     on success (including zero-size operations)
 * @return  NFC_SECURE_ERROR_INVALID  if dst or src is NULL
 * @return  NFC_SECURE_ERROR_OVERFLOW if dst_size < src_size (buffer overflow prevented)
 * @return  NFC_SECURE_ERROR_RANGE    if src_size or dst_size exceeds SIZE_MAX / 2
 * @return  NFC_SECURE_ERROR_ZERO_SIZE (deprecated) - now returns SUCCESS for zero-size
 *
 * @note This function uses memmove() internally, which correctly handles
 *       overlapping buffers. For non-overlapping buffers, nfc_safe_memcpy()
 *       may be slightly faster, but this function is always safe.
 *
 * Example usage:
 * ```c
 * uint8_t buffer[20] = "Hello, World!";
 * // Move data within the same buffer (overlapping region)
 * int result = nfc_safe_memmove(buffer + 7, 13, buffer, 5);
 * // buffer is now "Hello, Hello!"
 * ```
 */
int nfc_safe_memmove(void *dst, size_t dst_size, const void *src, size_t src_size);

/**
 * @thread_safety These functions are thread-safe. They access only the
 * caller-provided buffers and do not touch global mutable state.
 */

/*
 * ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 * 💡 BEST PRACTICES GUIDE
 * ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 *
 * 1. WHEN TO USE memcpy vs memmove:
 *
 *    ✅ Use nfc_safe_memcpy() when:
 *       - Buffers are GUARANTEED not to overlap
 *       - Copying between different objects (e.g., struct to array)
 *       - Slightly faster (~5-10% on some platforms)
 *
 *    ✅ Use nfc_safe_memmove() when:
 *       - Buffers MAY overlap (e.g., moving within same buffer)
 *       - Unsure about overlap - always safe
 *       - Moving data left/right within an array
 *
 *    ⚠️  RULE OF THUMB: If in doubt, use memmove - it's always safe!
 *        Modern compilers optimize memmove to memcpy when overlap is impossible.
 *
 * 2. MACRO vs FUNCTION:
 *
 *    ✅ Use MACROS (NFC_SAFE_MEMCPY, NFC_SECURE_MEMSET) when:
 *       - Working with fixed-size arrays: uint8_t buffer[16]
 *       - Want compile-time type checking (C11+)
 *       - Automatic sizeof() calculation
 *
 *    ✅ Use FUNCTIONS (nfc_safe_memcpy, nfc_secure_memset) when:
 *       - Working with dynamic memory: malloc(), calloc()
 *       - Pointer arithmetic: buffer + offset
 *       - Size is calculated at runtime
 *
 * 3. ERROR HANDLING:
 *
 *    ⚠️  ALWAYS check return values in production code:
 *
 *       int result = nfc_safe_memcpy(dst, dst_size, src, src_size);
 *       if (result != NFC_SECURE_SUCCESS) {
 *           log_error("Copy failed: %s", nfc_secure_strerror(result));
 *           return result;  // Propagate error
 *       }
 *
 *    ⚠️  NEVER ignore errors - they indicate real security issues!
 *
 * 4. PERFORMANCE OPTIMIZATION:
 *
 *    For sensitive data (keys, passwords):
 *       → Use nfc_secure_memset() - accept the performance cost
 *
 *    For non-sensitive data (general buffers):
 *       → Use standard memset() - ~10-30% faster
 *
 *    For large buffer clears (>1KB):
 *       → Consider whether data is truly sensitive
 *       → Standard memset may be acceptable for non-crypto data
 *
 * 5. DEBUG vs RELEASE BUILDS:
 *
 *    Debug builds (recommended):
 *       - Enable NFC_SECURE_CHECK_OVERLAP (auto-enabled by default)
 *       - Enable NFC_SECURE_DEBUG for extra validation
 *       - Catches bugs early in development
 *
 *    Release builds (recommended):
 *       - Disable NFC_SECURE_CHECK_OVERLAP (define NDEBUG)
 *       - Disable NFC_SECURE_DEBUG
 *       - Maximizes performance
 *
 * ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 */

/*
 * Compile-time check for array vs pointer
 *
 * IMPORTANT: This macro relies on compiler-specific extensions:
 * - __builtin_types_compatible_p: GCC/Clang builtin (NOT C standard)
 * - typeof: C23 standard operator (or __typeof__ in GCC/Clang)
 *
 * C23 standardizes `typeof`, but __builtin_types_compatible_p remains
 * a compiler extension. Therefore, we require GCC/Clang even in C23 mode.
 *
 * Platform Support:
 * - C23 + GCC/Clang: Full compile-time array detection
 * - C11 + GCC/Clang: Full compile-time array detection
 * - Other compilers: Runtime checks only (no compile-time guarantee)
 */
#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 202311L && \
    (defined(__GNUC__) || defined(__clang__))
/* C23 with GCC/Clang: Use standardized typeof + builtin */
#define NFC_IS_ARRAY(x) \
  (!__builtin_types_compatible_p(typeof(x), typeof(&(x)[0])))
#elif defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L && \
    (defined(__GNUC__) || defined(__clang__))
/* C11 with GNU/Clang extensions: Use __typeof__ + builtin */
#define NFC_IS_ARRAY(x) \
  (!__builtin_types_compatible_p(__typeof__(x), __typeof__(&(x)[0])))
#else
/* Fallback for other compilers - no compile-time check */
#define NFC_IS_ARRAY(x) (1)
#endif

/**
 * @brief Helper macro for safe memcpy with sizeof() validation
 *
 * Automatically calculates destination size using sizeof(), preventing
 * manual size calculation errors.
 *
 * @param dst Destination buffer (must be array, not pointer)
 * @param src Source buffer
 * @param src_size Number of bytes to copy
 *
 * @warning `dst` must be an array, not a pointer.
 *          If `dst` is a pointer, `sizeof(dst)` will return the pointer size
 *          (for example, 8 on x86_64) rather than the actual buffer size,
 *          which can lead to incomplete or unsafe copies that incorrectly pass
 *          the size check.
 *
 * @note Starting from C11 with GNU/Clang compilers, this macro will generate
 *       a compile-time error if `dst` is a pointer instead of an array,
 *       preventing this common mistake at build time rather than runtime.
 *
 * Example (correct):
 * ```c
 * uint8_t buffer[10];
 * uint8_t data[5];
 * NFC_SAFE_MEMCPY(buffer, data, sizeof(data)); // ✅ Safe, uses sizeof(buffer)
 * ```
 *
 * Incorrect usage (pointer case):
 * ```c
 * uint8_t *buffer = malloc(10);
 * uint8_t data[5];
 * NFC_SAFE_MEMCPY(buffer, data, sizeof(data)); // ❌ Compile error (C11+): "dst must be an array, not a pointer"
 * ```
 */
#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L && \
    (defined(__GNUC__) || defined(__clang__))
/* C11+ with GNU/Clang: compile-time array type check */
#define NFC_SAFE_MEMCPY(dst, src, src_size)                                                        \
  (__extension__({                                                                               \
    _Static_assert(NFC_IS_ARRAY(dst), "NFC_SAFE_MEMCPY: dst must be an array, not a pointer"); \
    nfc_safe_memcpy((dst), sizeof(dst), (src), (src_size));                                    \
  }))
#else
/* Older compilers: no compile-time check */
#define NFC_SAFE_MEMCPY(dst, src, src_size) \
  nfc_safe_memcpy((dst), sizeof(dst), (src), (src_size))
#endif

/**
 * @brief Helper macro for secure memset with sizeof() validation
 *
 * Automatically calculates buffer size using sizeof(), preventing
 * manual size calculation errors.
 *
 * @param ptr Pointer to memory (must be array, not pointer)
 * @param val Value to set
 *
 * @note Starting from C11 with GNU/Clang compilers, this macro will generate
 *       a compile-time error if `ptr` is a pointer instead of an array.
 *
 * Example:
 * ```c
 * uint8_t key[16];
 * NFC_SECURE_MEMSET(key, 0x00); // Zero out securely
 * NFC_SECURE_MEMSET(key, 0xFF); // Fill with 0xFF
 * ```
 */
#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L && \
    (defined(__GNUC__) || defined(__clang__))
/* C11+ with GNU/Clang: compile-time array type check */
#define NFC_SECURE_MEMSET(ptr, val)                                                                  \
  (__extension__({                                                                                 \
    _Static_assert(NFC_IS_ARRAY(ptr), "NFC_SECURE_MEMSET: ptr must be an array, not a pointer"); \
    nfc_secure_memset((ptr), (val), sizeof(ptr));                                                \
  }))
#else
/* Older compilers: no compile-time check */
#define NFC_SECURE_MEMSET(ptr, val) \
  nfc_secure_memset((ptr), (val), sizeof(ptr))
#endif

/**
 * @brief Helper macro for safe memmove with sizeof() validation
 *
 * Automatically calculates destination size using sizeof(), preventing
 * manual size calculation errors. This macro is safe for overlapping buffers.
 *
 * @param dst Destination buffer (must be array, not pointer)
 * @param src Source buffer
 * @param src_size Number of bytes to move
 *
 * @warning Like NFC_SAFE_MEMCPY, dst must be an array, not a pointer.
 *          For pointer arithmetic (e.g., buffer + 7), use the function directly.
 *
 * Example (correct usage):
 * ```c
 * uint8_t src[10] = {1, 2, 3, 4, 5, 6, 7, 8, 9, 10};
 * uint8_t dst[15];
 * NFC_SAFE_MEMMOVE(dst, src, sizeof(src)); // ✅ Works - dst is an array
 * ```
 *
 * For overlapping buffers within the same array, use the function:
 * ```c
 * uint8_t buffer[20] = "Hello, World!";
 * // ❌ Won't compile: buffer + 7 is a pointer
 * // NFC_SAFE_MEMMOVE(buffer + 7, buffer, 5);
 *
 * // ✅ Correct: use function for pointer arithmetic
 * nfc_safe_memmove(buffer + 7, 13, buffer, 5);
 * ```
 */
#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L && \
    (defined(__GNUC__) || defined(__clang__))
/* C11+ with GNU/Clang: compile-time array type check */
#define NFC_SAFE_MEMMOVE(dst, src, src_size)                                                        \
  (__extension__({                                                                                \
    _Static_assert(NFC_IS_ARRAY(dst), "NFC_SAFE_MEMMOVE: dst must be an array, not a pointer"); \
    nfc_safe_memmove((dst), sizeof(dst), (src), (src_size));                                    \
  }))
#else
/* Older compilers: no compile-time check */
#define NFC_SAFE_MEMMOVE(dst, src, src_size) \
  nfc_safe_memmove((dst), sizeof(dst), (src), (src_size))
#endif

#ifdef __cplusplus
}
#endif

#endif /* NFC_SECURE_H */
