/**
 * @file nfc-secure.h
 * @brief Secure memory operation wrappers for libnfc
 *
 * This header provides safe wrappers around standard C memory functions
 * (memcpy, memset) to prevent buffer overflow vulnerabilities.
 *
 * Design follows common secure coding standards (CERT C, ISO/IEC TR 24772)
 * and general industry memory safety practices.
 *
 * C23 Optimizations:
 * - constexpr for compile-time constants
 * - nullptr for type-safe null pointer
 * - static_assert for compile-time checks
 * - [[nodiscard]] for mandatory error checking
 * - Improved type generic macros with typeof
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
 * NFC_SAFE_MEMCPY(buffer, data, sizeof(data));
 *
 * // Pointers/Dynamic memory (runtime size)
 * uint8_t *buf = malloc(64);
 * nfc_safe_memcpy(buf, 64, data, sizeof(data));
 *
 * // Secure erase (won't be optimized away)
 * uint8_t password[256];
 * NFC_SECURE_MEMSET(password, 0x00);
 * ```
 */

#ifndef NFC_SECURE_H
#define NFC_SECURE_H

#include <stddef.h>
#include <stdint.h>
#include <string.h>

/* ============================================================================
 * CONFIGURATION MACROS
 * ========================================================================== */

/**
 * Auto-enable overlap checking in debug builds (unless explicitly disabled)
 */
#if !defined(NFC_SECURE_CHECK_OVERLAP) && !defined(NDEBUG)
#define NFC_SECURE_CHECK_OVERLAP 1
#endif

/**
 * Performance optimization threshold for secure memset (bytes)
 */
#ifndef NFC_SECURE_MEMSET_THRESHOLD
#define NFC_SECURE_MEMSET_THRESHOLD 256
#endif

/* ============================================================================
 * C STANDARD DETECTION AND COMPATIBILITY
 * ========================================================================== */

/**
 * INTERNAL USE ONLY - DO NOT RELY ON THESE MACROS IN EXTERNAL CODE
 *
 * The NFC_HAVE_* macros below are for internal implementation only.
 * External code should check __STDC_VERSION__ directly if needed.
 * These macros may change or be removed in future versions without notice.
 */

/**
 * C23 nullptr support for better type safety
 *
 * C23 introduces nullptr as a distinct null pointer constant with type nullptr_t.
 * For older standards, we continue using NULL for compatibility.
 */
#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 202311L
#define NFC_NULL nullptr
#define NFC_HAVE_C23 1
#else
#define NFC_NULL NULL
#define NFC_HAVE_C23 0
#endif

/**
 * C11 detection for _Static_assert and other features
 */
#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
#define NFC_HAVE_C11 1
#else
#define NFC_HAVE_C11 0
#endif

/**
 * Compiler extension detection
 */
#if defined(__GNUC__) || defined(__clang__)
#define NFC_HAVE_GNU_EXTENSIONS 1
#else
#define NFC_HAVE_GNU_EXTENSIONS 0
#endif

/* ============================================================================
 * C23 ATTRIBUTE SUPPORT
 * ========================================================================== */

/**
 * [[nodiscard]] attribute for mandatory error checking (C23/C++17)
 */
#if NFC_HAVE_C23 && defined(__has_c_attribute)
#if __has_c_attribute(nodiscard)
#define NFC_NODISCARD [[nodiscard]]
#else
#define NFC_NODISCARD
#endif
#elif defined(__cplusplus) && __cplusplus >= 201703L
#define NFC_NODISCARD [[nodiscard]]
#elif NFC_HAVE_GNU_EXTENSIONS
#define NFC_NODISCARD __attribute__((warn_unused_result))
#else
#define NFC_NODISCARD
#endif

/**
 * [[deprecated]] attribute for obsolete functions (C23/C++14)
 */
#if NFC_HAVE_C23 && defined(__has_c_attribute)
#if __has_c_attribute(deprecated)
#define NFC_DEPRECATED(msg) [[deprecated(msg)]]
#else
#define NFC_DEPRECATED(msg)
#endif
#elif defined(__cplusplus) && __cplusplus >= 201402L
#define NFC_DEPRECATED(msg) [[deprecated(msg)]]
#elif NFC_HAVE_GNU_EXTENSIONS
#define NFC_DEPRECATED(msg) __attribute__((deprecated(msg)))
#else
#define NFC_DEPRECATED(msg)
#endif

/* ============================================================================
 * ERROR CODES AND DIAGNOSTICS
 * ========================================================================== */

#ifdef __cplusplus
extern "C"
{
#endif

  /**
   * @brief NFC Secure library error codes
   *
   * Platform-independent error codes for secure memory operations.
   * These are negative values to distinguish from success (0).
   */
  enum nfc_secure_error
  {
    NFC_SECURE_SUCCESS = 0,         /**< Operation succeeded */
    NFC_SECURE_ERROR_INVALID = -1,  /**< Invalid parameter (NULL pointer, etc.) */
    NFC_SECURE_ERROR_OVERFLOW = -2, /**< Buffer overflow would occur */
    NFC_SECURE_ERROR_RANGE = -3,    /**< Size parameter out of valid range */
    NFC_SECURE_ERROR_ZERO_SIZE = -4 /**< Zero-size operation (deprecated) */
  };

  /**
   * @brief Get human-readable error message
   *
   * This is an informational function - ignoring the return value is harmless.
   * No NFC_NODISCARD annotation needed (unlike security-critical functions).
   *
   * @param error_code Error code from nfc_secure_error enum
   * @return String describing the error, or "Unknown error" for invalid codes
   */
  const char *nfc_secure_strerror(int error_code);

  /* ============================================================================
   * CORE SECURE MEMORY FUNCTIONS
   * ========================================================================== */

  /**
   * @brief Safe memory copy with buffer size validation
   *
   * This function provides a secure alternative to memcpy() by validating
   * that the destination buffer has sufficient space before copying.
   *
   * @param[out] dst Destination buffer (must be non-NULL)
   * @param[in] dst_size Size of destination buffer in bytes
   * @param[in] src Source buffer (must be non-NULL)
   * @param[in] src_size Number of bytes to copy from source
   *
   * @return NFC_SECURE_SUCCESS (0) on success
   * @return NFC_SECURE_ERROR_INVALID if dst or src is NULL
   * @return NFC_SECURE_ERROR_OVERFLOW if dst_size < src_size
   * @return NFC_SECURE_ERROR_RANGE if size exceeds SIZE_MAX / 2
   *
   * @note For fixed-size arrays, use NFC_SAFE_MEMCPY() macro instead
   * @note For dynamic memory, ALWAYS use this function with explicit sizes
   *
   * Example:
   * ```c
   * uint8_t *buffer = malloc(100);
   * uint8_t data[50];
   * int result = nfc_safe_memcpy(buffer, 100, data, sizeof(data));
   * if (result != NFC_SECURE_SUCCESS) {
   *     fprintf(stderr, "Copy failed: %s\n", nfc_secure_strerror(result));
   * }
   * free(buffer);
   * ```
   */
  NFC_NODISCARD
  int nfc_safe_memcpy(void *dst, size_t dst_size, const void *src, size_t src_size);

  /**
   * @brief Safe memory move with buffer size validation
   *
   * This function provides a secure alternative to memmove() by validating
   * that the destination buffer has sufficient space before copying.
   * Unlike nfc_safe_memcpy(), this function correctly handles overlapping buffers.
   *
   * @param[out] dst Destination buffer (must be non-NULL)
   * @param[in] dst_size Size of destination buffer in bytes
   * @param[in] src Source buffer (must be non-NULL)
   * @param[in] src_size Number of bytes to move from source
   *
   * @return NFC_SECURE_SUCCESS (0) on success
   * @return NFC_SECURE_ERROR_INVALID if dst or src is NULL
   * @return NFC_SECURE_ERROR_OVERFLOW if dst_size < src_size
   * @return NFC_SECURE_ERROR_RANGE if size exceeds SIZE_MAX / 2
   *
   * Example (overlapping buffers):
   * ```c
   * uint8_t buffer[20] = "Hello, World!";
   * nfc_safe_memmove(buffer + 7, 13, buffer, 5);
   * // Result: "Hello, Hello!"
   * ```
   */
  NFC_NODISCARD
  int nfc_safe_memmove(void *dst, size_t dst_size, const void *src, size_t src_size);

  /**
   * @brief Secure memset for sensitive data
   *
   * This function ensures that memory is securely erased and cannot be
   * optimized away by the compiler (unlike standard memset).
   *
   * Platform-specific implementations:
   * - C23: memset_explicit (when available)
   * - C11: memset_s from Annex K (optional)
   * - POSIX/BSD: explicit_bzero
   * - Windows: SecureZeroMemory
   * - Fallback: volatile pointer + memory barriers
   *
   * @param[out] ptr Pointer to memory to clear (must be non-NULL)
   * @param[in] val Value to set (typically 0x00)
   * @param[in] size Number of bytes to set
   *
   * @return NFC_SECURE_SUCCESS (0) on success
   * @return NFC_SECURE_ERROR_INVALID if ptr is NULL
   * @return NFC_SECURE_ERROR_RANGE if size exceeds SIZE_MAX / 2
   *
   * Performance characteristics:
   * - Small buffers (≤256 bytes): Optimized volatile loop (~1-5 μs)
   * - Large buffers (>256 bytes): memset + memory barrier (~10-30% overhead)
   * - Platform functions: Near-native performance
   *
   * @warning Use ONLY for sensitive data (keys, passwords, crypto material)
   * @warning For non-sensitive data, use standard memset() for better performance
   *
   * Example:
   * ```c
   * uint8_t aes_key[32];
   * // ... use key for encryption ...
   * nfc_secure_memset(aes_key, 0x00, sizeof(aes_key));
   * // Compiler cannot optimize away this erasure
   * ```
   */
  NFC_NODISCARD
  int nfc_secure_memset(void *ptr, int val, size_t size);

#ifdef __cplusplus
}
#endif

/* ============================================================================
 * COMPILE-TIME TYPE CHECKING
 * ========================================================================== */

/**
 * Array vs pointer detection for compile-time safety
 *
 * This macro distinguishes between arrays and pointers to prevent
 * common sizeof() misuse bugs.
 *
 * Supported: C23/C11 with GCC/Clang
 * Fallback: Always returns true (no compile-time check)
 */
#if NFC_HAVE_C23 && NFC_HAVE_GNU_EXTENSIONS
/* C23: Use standardized typeof + builtin */
#define NFC_IS_ARRAY(x) \
  (!__builtin_types_compatible_p(typeof(x), typeof(&(x)[0])))
#elif NFC_HAVE_C11 && NFC_HAVE_GNU_EXTENSIONS
/* C11: Use __typeof__ + builtin */
#define NFC_IS_ARRAY(x) \
  (!__builtin_types_compatible_p(__typeof__(x), __typeof__(&(x)[0])))
#else
/* Fallback: No compile-time check */
#define NFC_IS_ARRAY(x) (1)
#endif

/* ============================================================================
 * CONVENIENCE MACROS
 * ========================================================================== */

/**
 * @brief Helper macro for safe memcpy with automatic sizeof() calculation
 *
 * Automatically calculates destination size using sizeof(), preventing
 * manual size calculation errors.
 *
 * @param dst Destination buffer (MUST be array, not pointer)
 * @param src Source buffer
 * @param src_size Number of bytes to copy
 *
 * @warning dst must be an array. For pointers, use nfc_safe_memcpy() directly
 *
 * Example (correct):
 * ```c
 * uint8_t buffer[10];
 * uint8_t data[5];
 * NFC_SAFE_MEMCPY(buffer, data, sizeof(data)); // ✅ Correct
 * ```
 *
 * Example (incorrect - will fail at compile time on C11+):
 * ```c
 * uint8_t *buffer = malloc(10);
 * uint8_t data[5];
 * NFC_SAFE_MEMCPY(buffer, data, sizeof(data)); // ❌ Compile error
 * // Use: nfc_safe_memcpy(buffer, 10, data, sizeof(data));
 * ```
 */
#if NFC_HAVE_C11 && NFC_HAVE_GNU_EXTENSIONS
/* C11+: Compile-time array type check */
#define NFC_SAFE_MEMCPY(dst, src, src_size)                                 \
  (__extension__({                                                          \
    _Static_assert(NFC_IS_ARRAY(dst),                                       \
                   "NFC_SAFE_MEMCPY: dst must be an array, not a pointer. " \
                   "For pointers, use nfc_safe_memcpy() directly.");        \
    nfc_safe_memcpy((dst), sizeof(dst), (src), (src_size));                 \
  }))
#else
/* Older compilers: No compile-time check */
#define NFC_SAFE_MEMCPY(dst, src, src_size) \
  nfc_safe_memcpy((dst), sizeof(dst), (src), (src_size))
#endif

/**
 * @brief Helper macro for safe memmove with automatic sizeof() calculation
 *
 * Like NFC_SAFE_MEMCPY but handles overlapping buffers correctly.
 *
 * @param dst Destination buffer (MUST be array, not pointer)
 * @param src Source buffer
 * @param src_size Number of bytes to move
 *
 * @note For pointer arithmetic (e.g., buffer + 7), use nfc_safe_memmove()
 */
#if NFC_HAVE_C11 && NFC_HAVE_GNU_EXTENSIONS
/* C11+: Compile-time array type check */
#define NFC_SAFE_MEMMOVE(dst, src, src_size)                                 \
  (__extension__({                                                           \
    _Static_assert(NFC_IS_ARRAY(dst),                                        \
                   "NFC_SAFE_MEMMOVE: dst must be an array, not a pointer. " \
                   "For pointers, use nfc_safe_memmove() directly.");        \
    nfc_safe_memmove((dst), sizeof(dst), (src), (src_size));                 \
  }))
#else
/* Older compilers: No compile-time check */
#define NFC_SAFE_MEMMOVE(dst, src, src_size) \
  nfc_safe_memmove((dst), sizeof(dst), (src), (src_size))
#endif

/**
 * @brief Helper macro for secure memset with automatic sizeof() calculation
 *
 * Automatically calculates buffer size using sizeof(), preventing
 * manual size calculation errors.
 *
 * @param ptr Pointer to memory (MUST be array, not pointer)
 * @param val Value to set
 *
 * Example:
 * ```c
 * uint8_t key[16];
 * NFC_SECURE_MEMSET(key, 0x00); // ✅ Correct
 * ```
 */
#if NFC_HAVE_C11 && NFC_HAVE_GNU_EXTENSIONS
/* C11+: Compile-time array type check */
#define NFC_SECURE_MEMSET(ptr, val)                                           \
  (__extension__({                                                            \
    _Static_assert(NFC_IS_ARRAY(ptr),                                         \
                   "NFC_SECURE_MEMSET: ptr must be an array, not a pointer. " \
                   "For pointers, use nfc_secure_memset() directly.");        \
    nfc_secure_memset((ptr), (val), sizeof(ptr));                             \
  }))
#else
/* Older compilers: No compile-time check */
#define NFC_SECURE_MEMSET(ptr, val) \
  nfc_secure_memset((ptr), (val), sizeof(ptr))
#endif

/* ============================================================================
 * SAFE STRING LENGTH FUNCTIONS (CWE-126 Prevention)
 * ========================================================================== */

/**
 * @brief Safely compute string length with maximum bounds check
 *
 * This function prevents buffer over-read vulnerabilities (CWE-126) by
 * limiting the search for the null terminator to a specified maximum length.
 *
 * @param[in] str      Pointer to the string (may not be null-terminated)
 * @param[in] maxlen   Maximum number of bytes to examine
 * @return Length of the string if null terminator found within maxlen,
 *         or maxlen if no null terminator found
 *
 * @note This is similar to POSIX strnlen() but with guaranteed availability
 * @note If str is NULL, returns 0
 *
 * @security Prevents buffer over-read by limiting memory scan
 *
 * Example:
 * ```c
 * char buffer[64];
 * // Unsafe: strlen(buffer) - may read beyond buffer if not null-terminated
 * // Safe:
 * size_t len = nfc_safe_strlen(buffer, sizeof(buffer));
 * ```
 */
// Implementation in nfc-secure.c (not inline due to export requirements)
size_t nfc_safe_strlen(const char *str, size_t maxlen);

/**
 * @brief Validate that a buffer contains a null-terminated string
 *
 * This function checks whether a buffer contains a properly null-terminated
 * string within the specified length.
 *
 * @param[in] buf      Pointer to the buffer to check
 * @param[in] bufsize  Size of the buffer in bytes
 * @return 1 if null terminator found within bufsize, 0 otherwise
 *
 * @note If buf is NULL, returns 0
 *
 * Example:
 * ```c
 * char user_input[256];
 * if (!nfc_is_null_terminated(user_input, sizeof(user_input))) {
 *     fprintf(stderr, "Error: Input not properly terminated\n");
 *     return NFC_EINVARG;
 * }
 * size_t len = strlen(user_input); // Now safe to use strlen
 * ```
 */
static inline int
nfc_is_null_terminated(const char *buf, size_t bufsize)
{
  if (buf == NFC_NULL || bufsize == 0)
  {
    return 0;
  }

  for (size_t i = 0; i < bufsize; i++)
  {
    if (buf[i] == '\0')
    {
      return 1;
    }
  }
  return 0;
}

/**
 * @brief Ensure a buffer is null-terminated by adding terminator if needed
 *
 * This function guarantees that a buffer is null-terminated by adding a
 * null terminator at the last position if none exists within the buffer.
 *
 * @param[inout] buf      Pointer to the buffer to null-terminate
 * @param[in]    bufsize  Size of the buffer in bytes (must be > 0)
 *
 * @note This will overwrite the last byte with '\0' if needed
 * @note If buf is NULL or bufsize is 0, does nothing
 *
 * @warning This may truncate data if the buffer is full without null terminator
 *
 * Example:
 * ```c
 * char buffer[256];
 * strncpy(buffer, user_input, sizeof(buffer)); // May not be null-terminated
 * nfc_ensure_null_terminated(buffer, sizeof(buffer)); // Now guaranteed safe
 * size_t len = strlen(buffer); // Safe
 * ```
 */
static inline void
nfc_ensure_null_terminated(char *buf, size_t bufsize)
{
  if (buf == NFC_NULL || bufsize == 0)
  {
    return;
  }

  /* Check if already null-terminated */
  int found_null = 0;
  for (size_t i = 0; i < bufsize; i++)
  {
    if (buf[i] == '\0')
    {
      found_null = 1;
      break;
    }
  }

  /* If not null-terminated, add terminator at the end */
  if (!found_null)
  {
    buf[bufsize - 1] = '\0';
  }
}

/* ============================================================================
 * THREAD SAFETY
 * ========================================================================== */

/**
 * @thread_safety All functions are thread-safe. They access only the
 * caller-provided buffers and do not touch global mutable state.
 */

/* ============================================================================
 * BEST PRACTICES SUMMARY
 * ========================================================================== */

/*
 * 💡 QUICK REFERENCE:
 *
 * 1. FIXED-SIZE ARRAYS → Use MACROS
 *    uint8_t buf[16];
 *    NFC_SAFE_MEMCPY(buf, src, len);
 *
 * 2. DYNAMIC MEMORY → Use FUNCTIONS
 *    uint8_t *buf = malloc(16);
 *    nfc_safe_memcpy(buf, 16, src, len);
 *
 * 3. OVERLAPPING BUFFERS → Use memmove variants
 *    nfc_safe_memmove(buf+5, 10, buf, 5);
 *
 * 4. SENSITIVE DATA → Always use secure memset
 *    NFC_SECURE_MEMSET(key, 0x00);
 *
 * 5. ERROR HANDLING → Always check return values
 *    if (result != NFC_SECURE_SUCCESS) {
 *        handle_error(nfc_secure_strerror(result));
 *    }
 */

#endif /* NFC_SECURE_H */
