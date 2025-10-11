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

#include "nfc-secure.h"
#include "log-internal.h"

#include <limits.h>
#include <stdbool.h>

/* Platform-specific headers for secure memset implementations */
#if defined(_WIN32) || defined(_WIN64)
#include <windows.h>  /* For SecureZeroMemory */
#endif

/* explicit_bzero declaration for systems that have it but don't expose it in headers */
#if (defined(__GLIBC__) && __GLIBC__ >= 2 && __GLIBC_MINOR__ >= 25) || \
    defined(__OpenBSD__) || defined(__FreeBSD__)
#ifndef _DEFAULT_SOURCE
extern void explicit_bzero(void *s, size_t n);
#endif
#endif

/* Maximum reasonable buffer size: half of SIZE_MAX to prevent integer overflow */
#define MAX_BUFFER_SIZE (SIZE_MAX / 2)

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
    /* Validation 1: NULL pointer checks */
    if (dst == NULL)
    {
#ifdef LOG
        log_put_internal("nfc_safe_memcpy: dst is NULL");
#endif
        return -EINVAL;
    }

    if (src == NULL)
    {
#ifdef LOG
        log_put_internal("nfc_safe_memcpy: src is NULL");
#endif
        return -EINVAL;
    }

    /* Validation 2: Size range checks (prevent integer overflow) */
    if (src_size == 0)
    {
        /* Zero-size copy is technically valid but suspicious */
        return 0;
    }

    if (src_size > MAX_BUFFER_SIZE)
    {
#ifdef LOG
        log_put_internal("nfc_safe_memcpy: src_size exceeds MAX_BUFFER_SIZE");
#endif
        return -ERANGE;
    }

    if (dst_size > MAX_BUFFER_SIZE)
    {
#ifdef LOG
        log_put_internal("nfc_safe_memcpy: dst_size exceeds MAX_BUFFER_SIZE");
#endif
        return -ERANGE;
    }

    /* Validation 3: CRITICAL BUFFER OVERFLOW CHECK */
    /* This check prevents buffer overflow by ensuring destination has sufficient space */
    if (dst_size < src_size)
    {
#ifdef LOG
        log_put_internal("nfc_safe_memcpy: BUFFER OVERFLOW PREVENTED");
#endif
        return -EOVERFLOW;
    }

#ifdef NFC_SECURE_CHECK_OVERLAP
    /* Validation 4: Buffer overlap check (debug builds only) */
    /* memcpy() has undefined behavior with overlapping buffers */
    /* For production code with possible overlap, use memmove() instead */
    if (buffers_overlap(dst, dst_size, src, src_size))
    {
#ifdef LOG
        log_put_internal("nfc_safe_memcpy: BUFFER OVERLAP DETECTED - use memmove() instead");
#endif
        return -EINVAL;
    }
#endif

    /* All checks passed - safe to copy */
    /* This memcpy is safe because dst_size >= src_size is validated above */
    memcpy(dst, src, src_size);

    return 0;
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
    /* Validation 1: NULL pointer check */
    if (ptr == NULL)
    {
#ifdef LOG
        log_put_internal("nfc_secure_memset: ptr is NULL");
#endif
        return -EINVAL;
    }

    /* Validation 2: Size range check */
    if (size == 0)
    {
        /* Zero-size memset is valid, no-op */
        return 0;
    }

    if (size > MAX_BUFFER_SIZE)
    {
#ifdef LOG
        log_put_internal("nfc_secure_memset: size exceeds MAX_BUFFER_SIZE");
#endif
        return -ERANGE;
    }

    /* Use platform-specific secure memset implementations when available */
    bool use_volatile_fallback = false;

#if defined(__STDC_LIB_EXT1__) && defined(__STDC_WANT_LIB_EXT1__)
    /* C11 Annex K: memset_s - safest and most portable when available */
    errno_t result = memset_s(ptr, size, val, size);
    if (result != 0)
    {
#ifdef LOG
        log_put_internal("nfc_secure_memset: memset_s failed");
#endif
        return -EINVAL;
    }
#elif defined(__unix__) || defined(__linux__) || defined(__APPLE__)
    /* BSD/Linux: explicit_bzero - guaranteed not to be optimized away */
#if defined(__GLIBC__) && __GLIBC__ >= 2 && __GLIBC_MINOR__ >= 25
    /* glibc 2.25+ provides explicit_bzero */
    explicit_bzero(ptr, size);
#elif defined(__OpenBSD__) || defined(__FreeBSD__)
    /* BSD systems have explicit_bzero */
    explicit_bzero(ptr, size);
#else
    /* Fallback for older glibc or other Unix systems */
    use_volatile_fallback = true;
#endif
#elif defined(_WIN32) || defined(_WIN64)
    /* Windows: SecureZeroMemory */
    SecureZeroMemory(ptr, size);
#else
    /* No platform-specific function available, use volatile fallback */
    use_volatile_fallback = true;
#endif

    if (use_volatile_fallback)
    {
        /* Secure memset implementation using volatile pointer */
        /* CRITICAL: volatile prevents compiler optimization */
        volatile uint8_t *volatile_ptr = (volatile uint8_t *)ptr;
        uint8_t byte_value = (uint8_t)val;

        /* Explicit loop to ensure every byte is written */
        /* Compiler cannot optimize away writes to volatile pointer */
        for (size_t i = 0; i < size; i++)
        {
            volatile_ptr[i] = byte_value;
        }
    }

    return 0;
}

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
