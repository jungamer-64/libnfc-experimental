/**
 * @file nfc-secure.c
 * @brief Secure memory operation implementations for libnfc
 *
 * Implements safe wrappers around standard C memory functions to prevent
 * buffer overflow vulnerabilities.
 *
 * This implementation provides:
 * - Safe memcpy with destination buffer size validation
 * - Secure memset that prevents compiler optimization
 */

/* Feature test macros for explicit_bzero on GNU systems */
#if defined(__linux__) || defined(__GLIBC__)
#ifndef _DEFAULT_SOURCE
#define _DEFAULT_SOURCE
#endif
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#endif

#include "nfc-secure.h"
#include "log-internal.h"

#include <limits.h>
#include <stdbool.h>
#include <stdio.h>  /* For snprintf in debug mode */

/**
 * @brief Get human-readable error message for NFC Secure error codes
 */
const char *nfc_secure_strerror(int error_code)
{
  switch (error_code) {
    case NFC_SECURE_SUCCESS:
      return "Success";
    case NFC_SECURE_ERROR_INVALID:
      return "Invalid parameter (NULL pointer or invalid input)";
    case NFC_SECURE_ERROR_OVERFLOW:
      return "Buffer overflow prevented (destination too small)";
    case NFC_SECURE_ERROR_RANGE:
      return "Size parameter out of valid range";
    case NFC_SECURE_ERROR_ZERO_SIZE:
      return "Zero-size operation (deprecated, now treated as success)";
    default:
      return "Unknown error code";
  }
}

/* Platform-specific headers for secure memset implementations */
#if defined(_WIN32) || defined(_WIN64)
#include <windows.h>  /* For SecureZeroMemory */
#endif

/*
 * C23 memset_explicit detection (DISABLED as of 2025)
 *
 * CRITICAL ISSUE FIXED: __builtin_memset_explicit does NOT exist.
 * memset_explicit is a regular function declared in <string.h>, not a builtin.
 *
 * STATUS (2025-10-12):
 * - GCC 14.x: Partial C23 support, memset_explicit NOT YET implemented
 * - Clang 18.x: Partial C23 support, memset_explicit NOT YET implemented
 * - MSVC 2024: No C23 support announced yet
 *
 * FUTURE ENABLEMENT:
 * When compilers mature (estimated 2026-2027), uncomment and test:
 *
 * #if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 202311L
 *   #if defined(__GNUC__) && __GNUC__ >= 15  // Estimated
 *     #define HAVE_MEMSET_EXPLICIT 1
 *   #elif defined(__clang__) && __clang_major__ >= 20  // Estimated
 *     #define HAVE_MEMSET_EXPLICIT 1
 *   #elif defined(_MSC_VER) && _MSC_VER >= 1950  // Estimated
 *     #define HAVE_MEMSET_EXPLICIT 1
 *   #endif
 * #endif
 *
 * For now, we rely on existing secure implementations:
 * - C11 Annex K: memset_s (Windows/MSVC)
 * - POSIX/BSD: explicit_bzero (glibc 2.25+, *BSD)
 * - Windows: SecureZeroMemory
 * - Universal: volatile fallback
 */
#if 0  /* DISABLED: Enable when C23 compilers mature (2026+) */
#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 202311L
/* Check for actual implementation, not just standard version */
#if defined(__GNUC__) && __GNUC__ >= 15
#define HAVE_MEMSET_EXPLICIT 1
#elif defined(__clang__) && __clang_major__ >= 20
#define HAVE_MEMSET_EXPLICIT 1
#elif defined(_MSC_VER) && _MSC_VER >= 1950
#define HAVE_MEMSET_EXPLICIT 1
#endif
#endif
#endif  /* End of disabled C23 memset_explicit detection */

/* C11 Annex K memset_s support (requires <errno.h> for errno_t) */
#if defined(__STDC_LIB_EXT1__) && defined(__STDC_WANT_LIB_EXT1__)
#include <errno.h>  /* For errno_t */
#define HAVE_MEMSET_S 1
#endif

/*
 * explicit_bzero detection with robust header checking
 *
 * Platform requirements:
 * - Linux glibc: Requires glibc 2.25+ (explicit_bzero added in 2.25)
 * - BSD systems: Available in all modern versions
 *
 * Correct version logic:
 *   (__GLIBC__ > 2) OR (__GLIBC__ == 2 AND __GLIBC_MINOR__ >= 25)
 * This handles glibc 3.x correctly (where __GLIBC_MINOR__ is irrelevant).
 *
 * Safety Enhancement:
 * - Verify <string.h> availability before enabling
 * - Prevents link errors if headers are missing
 * - Graceful degradation to fallback implementation
 *
 * Header Declaration Location:
 * - Linux/glibc: <string.h> (glibc 2.25+)
 * - BSD (OpenBSD/FreeBSD): <string.h> (all modern versions)
 * 
 * IMPORTANT: Do NOT confuse with <strings.h>:
 * - <strings.h> is for legacy BSD functions (bcopy, bzero, etc.)
 * - explicit_bzero() is declared in <string.h>, NOT <strings.h>
 */
#if ((defined(__GLIBC__) && \
      ((__GLIBC__ > 2) || (__GLIBC__ == 2 && __GLIBC_MINOR__ >= 25))) || \
     defined(__OpenBSD__) || defined(__FreeBSD__))
/* Verify header availability (most systems have string.h) */
#if defined(__has_include)
#if __has_include(<string.h>) || __has_include(<strings.h>)
#define HAVE_EXPLICIT_BZERO 1
#endif
#else
/* Assume header is available if __has_include not supported */
#define HAVE_EXPLICIT_BZERO 1
#endif
#endif

/**
 * Maximum reasonable buffer size: half of SIZE_MAX to prevent integer overflow
 *
 * Rationale:
 * - Prevents dst_size + src_size overflow when checking buffer operations
 * - Leaves room for internal calculations without wraparound
 * - Any buffer > SIZE_MAX/2 is likely a bug (e.g., negative value cast to size_t)
 *
 * Example vulnerability without this limit:
 *   size_t dst_size = SIZE_MAX;
 *   size_t src_size = 100;
 *   if (dst_size >= src_size) { // ✓ passes
 *       if (dst_size + 100 < dst_size) { // ✗ overflow! wraps to 99
 *
 * With SIZE_MAX/2 limit, such overflow scenarios are prevented.
 *
 * NOTE: Using static const instead of constexpr for better compatibility.
 *       C23 constexpr support is still immature in most compilers (2025).
 */
#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 202311L
/* C23: Use static const (constexpr support still limited in compilers) */
static const size_t MAX_BUFFER_SIZE = SIZE_MAX / 2;
#else
#define MAX_BUFFER_SIZE (SIZE_MAX / 2)
#endif

/*
 * NFC_NULL is now defined in nfc-secure.h for project-wide visibility.
 * See header file for C23 nullptr support details.
 */

/**
 * @brief Runtime size validation for macro usage (debug mode only)
 *
 * This function helps detect pointer misuse in older compilers that don't
 * support compile-time checks. It flags suspicious buffer sizes that are
 * likely to be pointer sizes rather than array sizes.
 *
 * Common pointer sizes:
 * - 4 bytes (32-bit systems)
 * - 8 bytes (64-bit systems)
 *
 * To reduce false positives, we only warn if:
 * 1. Size exactly matches sizeof(void*)
 * 2. AND size is suspiciously small (≤16 bytes)
 * 3. AND size is a power of 2 (4, 8, 16)
 *
 * This reduces warnings for legitimate small arrays like uint8_t[8].
 */
#ifdef NFC_SECURE_DEBUG
static inline void check_suspicious_size(size_t dst_size, const char *func_name)
{
  /* Check if size matches pointer size AND is suspiciously small */
  if (dst_size == sizeof(void *) && dst_size <= 16) {
    /* Additional check: is it a power of 2? (pointers are always power of 2) */
    bool is_power_of_2 = (dst_size & (dst_size - 1)) == 0;

    if (is_power_of_2) {
#ifdef LOG
      char msg[128];
      snprintf(msg, sizeof(msg),
               "%s: WARNING - dst_size=%zu matches pointer size (%zu bytes). "
               "Did you pass a pointer instead of an array?",
               func_name, dst_size, sizeof(void *));
      log_put_internal(msg);
#endif
    }
  }
}
#else
#define check_suspicious_size(dst_size, func_name) ((void)0)
#endif

/**
 * Performance optimization threshold for secure memset
 *
 * For buffers larger than this size, we use a hybrid approach:
 * 1. Use platform-specific secure functions if available (fastest)
 * 2. Use memset() followed by memory barrier (faster than volatile loop)
 * 3. Fall back to volatile loop for very small buffers (most secure)
 *
 * Typical cryptographic keys are small (6-32 bytes), so volatile loop
 * overhead is negligible. For larger buffers (>256 bytes), performance
 * becomes more important.
 *
 * This threshold can be tuned based on profiling. Set to 0 to always use
 * volatile loop (maximum security, slower for large buffers).
 */
#ifndef NFC_SECURE_MEMSET_THRESHOLD
#define NFC_SECURE_MEMSET_THRESHOLD 256
#endif

/**
 * @brief Internal helper: Check if buffers overlap
 *
 * Note: Standard memcpy() has undefined behavior if source and destination overlap.
 * This check is enabled in debug builds (NFC_SECURE_CHECK_OVERLAP) to detect
 * programming errors. For production, prefer using memmove() when overlap is possible.
 *
 * @param dst Destination pointer
 * @param dst_size Destination size
 * @param src Source pointer
 * @param src_size Source size
 * @return true if buffers overlap
 */
#ifdef NFC_SECURE_CHECK_OVERLAP
static bool buffers_overlap(const void *dst, size_t dst_size,
                            const void *src, size_t src_size)
{
  const uint8_t *dst_ptr = (const uint8_t *)dst;
  const uint8_t *src_ptr = (const uint8_t *)src;

  /* Check if dst overlaps with src */
  if (dst_ptr >= src_ptr && dst_ptr < (src_ptr + src_size)) {
    return true;
  }

  /* Check if src overlaps with dst */
  if (src_ptr >= dst_ptr && src_ptr < (dst_ptr + dst_size)) {
    return true;
  }

  return false;
}
#endif /* NFC_SECURE_CHECK_OVERLAP */

/**
 * @brief Safe memory copy with buffer size validation
 *
 * Implementation follows memory safety best practices:
 * 1. Validate input parameters (NULL checks)
 * 2. Check buffer size constraints (dst_size >= src_size)
 * 3. Validate size ranges (prevent integer overflow)
 * 4. Perform copy only if all checks pass
 * 5. Return specific error codes for debugging
 */
int nfc_safe_memcpy(void *dst, size_t dst_size, const void *src, size_t src_size)
{
  /* Validation 1: NULL pointer checks (C23: uses nullptr, pre-C23: uses NULL) */
  if (dst == NFC_NULL) {
#ifdef LOG
    log_put_internal("nfc_safe_memcpy: dst is NULL");
#endif
    return NFC_SECURE_ERROR_INVALID;
  }

  if (src == NFC_NULL) {
#ifdef LOG
    log_put_internal("nfc_safe_memcpy: src is NULL");
#endif
    return NFC_SECURE_ERROR_INVALID;
  }

  /* Validation 2: Size range checks (prevent integer overflow) */
  if (src_size == 0) {
    /*
     * Zero-size copy is technically valid (memcpy(dst, src, 0) is safe)
     * but may indicate a logic error in caller code.
     *
     * We return success but log a warning in debug builds to help
     * developers catch potential bugs (e.g., sizeof() misuse).
     */
#if defined(NFC_SECURE_DEBUG) && defined(LOG)
    log_put_internal("nfc_safe_memcpy: INFO - zero-size copy (may indicate logic error)");
#endif
    return NFC_SECURE_SUCCESS;  /* Not an error, just a no-op */
  }

  if (src_size > MAX_BUFFER_SIZE) {
#ifdef LOG
    log_put_internal("nfc_safe_memcpy: src_size exceeds MAX_BUFFER_SIZE");
#endif
    return NFC_SECURE_ERROR_RANGE;
  }

  if (dst_size > MAX_BUFFER_SIZE) {
#ifdef LOG
    log_put_internal("nfc_safe_memcpy: dst_size exceeds MAX_BUFFER_SIZE");
#endif
    return NFC_SECURE_ERROR_RANGE;
  }

  /* Debug: Check for suspicious buffer sizes (potential pointer misuse) */
  check_suspicious_size(dst_size, "nfc_safe_memcpy");

  /* Validation 3: CRITICAL BUFFER OVERFLOW CHECK */
  /* This check prevents buffer overflow by ensuring destination has sufficient space */
  if (dst_size < src_size) {
#ifdef LOG
    log_put_internal("nfc_safe_memcpy: BUFFER OVERFLOW PREVENTED");
#endif
    return NFC_SECURE_ERROR_OVERFLOW;
  }

#ifdef NFC_SECURE_CHECK_OVERLAP
  /* Validation 4: Buffer overlap check (debug builds only) */
  /* memcpy() has undefined behavior with overlapping buffers */
  /* For production code with possible overlap, use memmove() instead */
  if (buffers_overlap(dst, dst_size, src, src_size)) {
#ifdef LOG
    log_put_internal("nfc_safe_memcpy: BUFFER OVERLAP DETECTED - use memmove() instead");
#endif
    return NFC_SECURE_ERROR_INVALID;
  }
#endif

  /* All checks passed - safe to copy */
  /* This memcpy is safe because dst_size >= src_size is validated above */
  memcpy(dst, src, src_size);

  return NFC_SECURE_SUCCESS;
}

/**
 * @brief Safe memory move with buffer size validation
 *
 * This function is identical to nfc_safe_memcpy() but uses memmove() internally,
 * which correctly handles overlapping buffers.
 *
 * @see nfc_safe_memcpy() for parameter documentation
 */
int nfc_safe_memmove(void *dst, size_t dst_size, const void *src, size_t src_size)
{
  /* Validation 1: NULL pointer checks (C23: uses nullptr, pre-C23: uses NULL) */
  if (dst == NFC_NULL) {
#ifdef LOG
    log_put_internal("nfc_safe_memmove: dst is NULL");
#endif
    return NFC_SECURE_ERROR_INVALID;
  }

  if (src == NFC_NULL) {
#ifdef LOG
    log_put_internal("nfc_safe_memmove: src is NULL");
#endif
    return NFC_SECURE_ERROR_INVALID;
  }

  /* Validation 2: Size range checks (prevent integer overflow) */
  if (src_size == 0) {
    /* Zero-size move is technically valid (memmove(dst, src, 0) is safe) */
#if defined(NFC_SECURE_DEBUG) && defined(LOG)
    log_put_internal("nfc_safe_memmove: INFO - zero-size move (may indicate logic error)");
#endif
    return NFC_SECURE_SUCCESS;  /* Not an error, just a no-op */
  }

  if (src_size > MAX_BUFFER_SIZE) {
#ifdef LOG
    log_put_internal("nfc_safe_memmove: src_size exceeds MAX_BUFFER_SIZE");
#endif
    return NFC_SECURE_ERROR_RANGE;
  }

  if (dst_size > MAX_BUFFER_SIZE) {
#ifdef LOG
    log_put_internal("nfc_safe_memmove: dst_size exceeds MAX_BUFFER_SIZE");
#endif
    return NFC_SECURE_ERROR_RANGE;
  }

  /* Validation 3: CRITICAL BUFFER OVERFLOW CHECK */
  if (dst_size < src_size) {
#ifdef LOG
    log_put_internal("nfc_safe_memmove: BUFFER OVERFLOW PREVENTED");
#endif
    return NFC_SECURE_ERROR_OVERFLOW;
  }

  /* All checks passed - safe to move */
  /* memmove() correctly handles overlapping buffers */
  memmove(dst, src, src_size);

  return NFC_SECURE_SUCCESS;
}

/**
 * @brief Secure memset for sensitive data
 *
 * Implementation prevents compiler optimization using volatile pointer trick.
 *
 * Standard memset() can be optimized away by compiler if:
 * - Memory is not used after memset (dead store elimination)
 * - Memory is freed immediately after memset
 * - Compiler determines memset has no observable effect
 *
 * Example scenario:
 * ```c
 * uint8_t key[16];
 * // ... use key ...
 * memset(key, 0, sizeof(key));  // MAY BE OPTIMIZED AWAY!
 * free(key);                     // Compiler sees key not used after memset
 * ```
 *
 * This implementation uses platform-specific secure functions when available:
 * 1. C11 memset_s (if __STDC_LIB_EXT1__ is defined)
 * 2. explicit_bzero (BSD/Linux)
 * 3. SecureZeroMemory (Windows)
 * 4. Fallback to volatile pointer trick
 *
 * Typical use cases:
 * - MIFARE keys (6 bytes)
 * - NFCID3 (10 bytes)
 * - ATR buffers (up to 254 bytes)
 * - Temporary command buffers with authentication data
 */
int nfc_secure_memset(void *ptr, int val, size_t size)
{
  /* Validation 1: NULL pointer check (C23: uses nullptr, pre-C23: uses NULL) */
  if (ptr == NFC_NULL) {
#ifdef LOG
    log_put_internal("nfc_secure_memset: ptr is NULL");
#endif
    return NFC_SECURE_ERROR_INVALID;
  }

  /* Validation 2: Size range check */
  if (size == 0) {
    /* Zero-size memset is technically valid (memset(ptr, val, 0) is safe) */
#if defined(NFC_SECURE_DEBUG) && defined(LOG)
    log_put_internal("nfc_secure_memset: INFO - zero-size memset (may indicate logic error)");
#endif
    return NFC_SECURE_SUCCESS;  /* Not an error, just a no-op */
  }

  if (size > MAX_BUFFER_SIZE) {
#ifdef LOG
    log_put_internal("nfc_secure_memset: size exceeds MAX_BUFFER_SIZE");
#endif
    return NFC_SECURE_ERROR_RANGE;
  }

  /*
   * Use platform-specific secure memset implementations in priority order:
   * 1. C23 memset_explicit (future-proof, standardized)
   * 2. C11 Annex K memset_s (portable but rarely implemented)
   * 3. POSIX/BSD explicit_bzero (widely available on Unix)
   * 4. Windows SecureZeroMemory (Windows-specific)
   * 5. Volatile fallback (universal but slowest)
   */
  bool use_volatile_fallback = false;

#if defined(HAVE_MEMSET_EXPLICIT)
  /* C23: memset_explicit - standardized secure memset */
  memset_explicit(ptr, val, size);

#elif defined(HAVE_MEMSET_S)
  /*
   * C11 Annex K: memset_s - portable when available (rare)
   *
   * Signature: errno_t memset_s(void *s, rsize_t smax, int c, rsize_t n)
   *   s    - pointer to destination
   *   smax - maximum size of destination buffer
   *   c    - value to set (converted to unsigned char)
   *   n    - number of bytes to set
   *
   * NOTE: We pass 'size' for both smax and n since we trust the caller
   *       to provide the correct buffer size. This is safe because we
   *       already validated size <= MAX_BUFFER_SIZE above.
   */
  errno_t result = memset_s(ptr, size, val, size);
  if (result != 0) {
#ifdef LOG
    log_put_internal("nfc_secure_memset: memset_s failed");
#endif
    return NFC_SECURE_ERROR_INVALID;
  }

#elif defined(HAVE_EXPLICIT_BZERO)
  /*
   * POSIX/BSD: explicit_bzero - guaranteed not to be optimized away
   *
   * NOTE: Uses HAVE_EXPLICIT_BZERO macro defined at compile time.
   *       This avoids duplicate platform detection logic.
   */
  explicit_bzero(ptr, size);

#elif defined(_WIN32) || defined(_WIN64)
  /* Windows: SecureZeroMemory */
  SecureZeroMemory(ptr, size);

#else
  /* No platform-specific function available, use volatile fallback */
  use_volatile_fallback = true;
#endif

  if (use_volatile_fallback) {
    /* For small buffers, use volatile loop (most secure) */
    /* For large buffers, use memset + barrier (faster) */
    if (size <= NFC_SECURE_MEMSET_THRESHOLD) {
      /* Secure memset implementation using volatile pointer */
      /* CRITICAL: volatile prevents compiler optimization */
      volatile uint8_t *volatile_ptr = (volatile uint8_t *)ptr;
      uint8_t byte_value = (uint8_t)val;

      /* Explicit loop to ensure every byte is written */
      /* Compiler cannot optimize away writes to volatile pointer */
      for (size_t i = 0; i < size; i++) {
        volatile_ptr[i] = byte_value;
      }
    } else {
      /* For large buffers, use standard memset with memory barrier */
      /* This is faster than volatile loop but still prevents optimization */
      memset(ptr, val, size);

      /* Memory barrier prevents compiler from optimizing away memset */
      /* This forces the compiler to treat the memory write as observable */
#if defined(__GNUC__) || defined(__clang__)
      __asm__ __volatile__("" ::: "memory");
#elif defined(_MSC_VER)
      _ReadWriteBarrier();
#else
      /*
       * Fallback for unknown compilers: volatile write to force completion
       *
       * We write a volatile value back to the first byte of the buffer.
       * This creates an observable side effect that cannot be optimized away.
       *
       * Note: A simple volatile read like *(volatile char*)ptr might be
       * optimized away by aggressive compilers. A volatile write is more
       * reliable as it has a clear side effect.
       */
      {
        volatile uint8_t *vptr = (volatile uint8_t *)ptr;
        volatile uint8_t tmp = *vptr;  /* Read current value */
        *vptr = tmp;                    /* Write it back (observable side effect) */
      }
#endif
    }
  }

  return NFC_SECURE_SUCCESS;
}
