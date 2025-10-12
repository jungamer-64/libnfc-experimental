/**
 * @file nfc-secure.c
 * @brief Secure memory operation implementations for libnfc
 *
 * Implements safe wrappers around standard C memory functions to prevent
 * buffer overflow vulnerabilities.
 *
 * C23 Optimizations:
 * - constexpr for compile-time constants
 * - static_assert for compile-time validation
 * - Improved platform detection for memset_explicit
 * - Better code organization and readability
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
#include <stdio.h>

/* Platform-specific headers */
#if defined(_WIN32) || defined(_WIN64)
#include <windows.h>
#endif

/* ============================================================================
 * COMPILE-TIME CONSTANTS AND LIMITS
 * ========================================================================== */

/**
 * Maximum reasonable buffer size: half of SIZE_MAX to prevent integer overflow
 *
 * Rationale:
 * - Prevents dst_size + src_size overflow when checking buffer operations
 * - Leaves room for internal calculations without wraparound
 * - Any buffer > SIZE_MAX/2 is likely a bug
 *
 * IMPORTANT: C23 constexpr for variables is still under discussion.
 * Current C23 drafts only support constexpr for functions, not variables.
 * Using static const instead for maximum compatibility.
 *
 * C23: Use static const with compile-time assertions
 * Pre-C23: Use macro (most portable)
 */
#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 202311L
/* C23: Use static const with compile-time assertions */
static const size_t MAX_BUFFER_SIZE = SIZE_MAX / 2;

/* Compile-time validation (C23 static_assert) */
static_assert(SIZE_MAX / 2 > 0,
              "MAX_BUFFER_SIZE calculation must be positive");
static_assert(SIZE_MAX / 2 < SIZE_MAX,
              "MAX_BUFFER_SIZE calculation must be less than SIZE_MAX");
#else
/* Pre-C23: Use macro (most portable) */
#define MAX_BUFFER_SIZE (SIZE_MAX / 2)
#endif

/* ============================================================================
 * PLATFORM DETECTION FOR SECURE MEMSET
 * ========================================================================== */

/**
 * C23 memset_explicit detection (DISABLED as of 2025)
 *
 * STATUS: No mainstream compiler implements memset_explicit yet.
 * ESTIMATED: 2026-2027+ (GCC 15+, Clang 20+, MSVC 2025+)
 *
 * Before enabling:
 * 1. Verify with: echo 'int main(){void *p=0; memset_explicit(p,0,0);}' | cc -x c -std=c23 -
 * 2. Check compiler release notes
 * 3. Test on all target platforms
 */
#if 0 /* DISABLED: Enable when compilers mature */
#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 202311L
#if (defined(__GNUC__) && __GNUC__ >= 15) ||         \
    (defined(__clang__) && __clang_major__ >= 20) || \
    (defined(_MSC_VER) && _MSC_VER >= 1950)
#define HAVE_MEMSET_EXPLICIT 1
#endif
#endif
#endif

/**
 * C11 Annex K memset_s support
 */
#if defined(__STDC_LIB_EXT1__) && defined(__STDC_WANT_LIB_EXT1__)
#include <errno.h>
#define HAVE_MEMSET_S 1
#endif

/**
 * POSIX/BSD explicit_bzero detection
 *
 * Requirements:
 * - Linux glibc 2.25+ (released 2017)
 * - BSD systems (OpenBSD, FreeBSD)
 *
 * Declared in <string.h>, NOT <strings.h>
 */
#if ((defined(__GLIBC__) &&                                              \
      ((__GLIBC__ > 2) || (__GLIBC__ == 2 && __GLIBC_MINOR__ >= 25))) || \
     defined(__OpenBSD__) || defined(__FreeBSD__))
#if defined(__has_include)
#if __has_include(<string.h>)
#define HAVE_EXPLICIT_BZERO 1
#endif
#else
#define HAVE_EXPLICIT_BZERO 1
#endif
#endif

/* ============================================================================
 * DEBUG AND VALIDATION HELPERS
 * ========================================================================== */

/**
 * Runtime size validation (debug mode only)
 *
 * Detects suspicious buffer sizes that are likely to be pointer sizes
 * rather than array sizes.
 */
#ifdef NFC_SECURE_DEBUG
static inline void
check_suspicious_size(size_t dst_size, const char *func_name)
{
  /* Check if size matches pointer size AND is suspiciously small */
  if (dst_size == sizeof(void *) && dst_size <= 16) {
    /* Additional check: is it a power of 2? (pointers are always power of 2) */
    const bool is_power_of_2 = (dst_size & (dst_size - 1)) == 0;

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
 * Buffer overlap detection (debug mode only)
 */
#ifdef NFC_SECURE_CHECK_OVERLAP
static inline bool
buffers_overlap(const void *dst, size_t dst_size,
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
#endif

/* ============================================================================
 * VALIDATION HELPERS
 * ========================================================================== */

/**
 * Common parameter validation (60+ lines - too large for inline)
 *
 * This function performs comprehensive validation but is intentionally NOT inlined.
 * The function is large (60+ lines) and compilers typically won't inline it anyway.
 * Let the compiler decide based on optimization level (-O2, -O3, etc.).
 *
 * Returns: NFC_SECURE_SUCCESS if valid, error code otherwise
 */
static int
validate_params(const void *dst, size_t dst_size,
                const void *src, size_t src_size,
                const char *func_name)
{
  /* NULL pointer checks */
  if (dst == NFC_NULL) {
#ifdef LOG
    char msg[128]; /* Increased buffer size for safety (func_name can be long) */
    snprintf(msg, sizeof(msg), "%s: dst is NULL", func_name);
    log_put_internal(msg);
#endif
    return NFC_SECURE_ERROR_INVALID;
  }

  if (src == NFC_NULL) {
#ifdef LOG
    char msg[128]; /* Increased buffer size for safety (func_name can be long) */
    snprintf(msg, sizeof(msg), "%s: src is NULL", func_name);
    log_put_internal(msg);
#endif
    return NFC_SECURE_ERROR_INVALID;
  }

  /* Zero-size is technically valid but may indicate a bug */
  if (src_size == 0) {
#if defined(NFC_SECURE_DEBUG) && defined(LOG)
    char msg[80];
    snprintf(msg, sizeof(msg),
             "%s: INFO - zero-size operation (may indicate logic error)",
             func_name);
    log_put_internal(msg);
#endif
    return NFC_SECURE_SUCCESS; /* Not an error, just a no-op */
  }

  /* Range checks */
  if (src_size > MAX_BUFFER_SIZE) {
#ifdef LOG
    char msg[128]; /* Increased buffer size for safety (func_name can be long) */
    snprintf(msg, sizeof(msg), "%s: src_size exceeds MAX_BUFFER_SIZE", func_name);
    log_put_internal(msg);
#endif
    return NFC_SECURE_ERROR_RANGE;
  }

  if (dst_size > MAX_BUFFER_SIZE) {
#ifdef LOG
    char msg[128]; /* Increased buffer size for safety (func_name can be long) */
    snprintf(msg, sizeof(msg), "%s: dst_size exceeds MAX_BUFFER_SIZE", func_name);
    log_put_internal(msg);
#endif
    return NFC_SECURE_ERROR_RANGE;
  }

  /* Buffer overflow check */
  if (dst_size < src_size) {
#ifdef LOG
    char msg[128]; /* Increased buffer size for safety (func_name can be long) */
    snprintf(msg, sizeof(msg), "%s: BUFFER OVERFLOW PREVENTED", func_name);
    log_put_internal(msg);
#endif
    return NFC_SECURE_ERROR_OVERFLOW;
  }

  /* Debug checks */
  check_suspicious_size(dst_size, func_name);

  return NFC_SECURE_SUCCESS;
}

/* ============================================================================
 * PUBLIC API IMPLEMENTATION
 * ========================================================================== */

/**
 * @brief Get human-readable error message
 */
const char *
nfc_secure_strerror(int error_code)
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

/**
 * @brief Safe memory copy with buffer size validation
 */
int nfc_safe_memcpy(void *dst, size_t dst_size, const void *src, size_t src_size)
{
  /* Validate parameters */
  const int validation_result = validate_params(dst, dst_size, src, src_size,
                                                "nfc_safe_memcpy");
  if (validation_result != NFC_SECURE_SUCCESS) {
    return validation_result;
  }

  /* Check for zero-size (already validated, but early return) */
  if (src_size == 0) {
    return NFC_SECURE_SUCCESS;
  }

#ifdef NFC_SECURE_CHECK_OVERLAP
  /* Buffer overlap check (memcpy has undefined behavior with overlap) */
  if (buffers_overlap(dst, dst_size, src, src_size)) {
#ifdef LOG
    log_put_internal("nfc_safe_memcpy: BUFFER OVERLAP DETECTED - use memmove() instead");
#endif
    return NFC_SECURE_ERROR_INVALID;
  }
#endif

  /* All checks passed - safe to copy */
  memcpy(dst, src, src_size);
  return NFC_SECURE_SUCCESS;
}

/**
 * @brief Safe memory move with buffer size validation
 */
int nfc_safe_memmove(void *dst, size_t dst_size, const void *src, size_t src_size)
{
  /* Validate parameters */
  const int validation_result = validate_params(dst, dst_size, src, src_size,
                                                "nfc_safe_memmove");
  if (validation_result != NFC_SECURE_SUCCESS) {
    return validation_result;
  }

  /* Check for zero-size (already validated, but early return) */
  if (src_size == 0) {
    return NFC_SECURE_SUCCESS;
  }

  /* All checks passed - safe to move (handles overlapping buffers) */
  memmove(dst, src, src_size);
  return NFC_SECURE_SUCCESS;
}

/* ============================================================================
 * SECURE MEMSET IMPLEMENTATION
 * ========================================================================== */

/**
 * Secure memset using volatile pointer trick
 *
 * This implementation prevents compiler optimization using volatile pointer.
 * Used as fallback when platform-specific secure functions are unavailable.
 *
 * This function is small (10 lines) and critical - force inlining.
 */
#if defined(__GNUC__) || defined(__clang__)
__attribute__((always_inline))
#endif
static inline void
secure_memset_volatile(void *ptr, int val, size_t size)
{
  volatile uint8_t *volatile_ptr = (volatile uint8_t *)ptr;
  const uint8_t byte_value = (uint8_t)val;

  for (size_t i = 0; i < size; i++) {
    volatile_ptr[i] = byte_value;
  }
}

/**
 * Secure memset using memset + memory barrier
 *
 * This implementation is faster for large buffers but still prevents
 * compiler optimization through memory barriers.
 *
 * This function is small (15 lines) and critical - force inlining.
 */
#if defined(__GNUC__) || defined(__clang__)
__attribute__((always_inline))
#endif
static inline void
secure_memset_barrier(void *ptr, int val, size_t size)
{
  memset(ptr, val, size);

  /* Memory barrier prevents compiler from optimizing away memset */
#if defined(__GNUC__) || defined(__clang__)
  __asm__ __volatile__("" ::: "memory");
#elif defined(_MSC_VER)
  _ReadWriteBarrier();
#else
  /* Fallback: volatile write to force completion */
  volatile uint8_t *vptr = (volatile uint8_t *)ptr;
  volatile uint8_t tmp = *vptr;
  *vptr = tmp;
#endif
}

/**
 * @brief Secure memset for sensitive data
 */
int nfc_secure_memset(void *ptr, int val, size_t size)
{
  /* Validation: NULL pointer check */
  if (ptr == NFC_NULL) {
#ifdef LOG
    log_put_internal("nfc_secure_memset: ptr is NULL");
#endif
    return NFC_SECURE_ERROR_INVALID;
  }

  /* Validation: Zero-size is technically valid */
  if (size == 0) {
#if defined(NFC_SECURE_DEBUG) && defined(LOG)
    log_put_internal("nfc_secure_memset: INFO - zero-size memset (may indicate logic error)");
#endif
    return NFC_SECURE_SUCCESS;
  }

  /* Validation: Range check */
  if (size > MAX_BUFFER_SIZE) {
#ifdef LOG
    log_put_internal("nfc_secure_memset: size exceeds MAX_BUFFER_SIZE");
#endif
    return NFC_SECURE_ERROR_RANGE;
  }

  /*
   * Platform-specific secure memset implementations (priority order):
   * 1. C23 memset_explicit (future-proof, standardized)
   * 2. C11 Annex K memset_s (portable but rarely implemented)
   * 3. POSIX/BSD explicit_bzero (widely available on Unix)
   * 4. Windows SecureZeroMemory (Windows-specific)
   * 5. Volatile fallback or memset+barrier (universal)
   */

#if defined(HAVE_MEMSET_EXPLICIT)
  /* C23: memset_explicit - standardized secure memset */
  memset_explicit(ptr, val, size);

#elif defined(HAVE_MEMSET_S)
  /* C11 Annex K: memset_s - portable when available */
  const errno_t result = memset_s(ptr, size, val, size);
  if (result != 0) {
#ifdef LOG
    log_put_internal("nfc_secure_memset: memset_s failed");
#endif
    return NFC_SECURE_ERROR_INVALID;
  }

#elif defined(HAVE_EXPLICIT_BZERO)
  /* POSIX/BSD: explicit_bzero - guaranteed not to be optimized away */
  explicit_bzero(ptr, size);

#elif defined(_WIN32) || defined(_WIN64)
  /* Windows: SecureZeroMemory */
  SecureZeroMemory(ptr, size);

#else
  /* Fallback: Choose implementation based on buffer size */
  if (size <= NFC_SECURE_MEMSET_THRESHOLD) {
    /* Small buffers: Use volatile loop (most secure) */
    secure_memset_volatile(ptr, val, size);
  } else {
    /* Large buffers: Use memset + barrier (faster) */
    secure_memset_barrier(ptr, val, size);
  }
#endif

  return NFC_SECURE_SUCCESS;
}
