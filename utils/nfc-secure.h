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
 */

#ifndef NFC_SECURE_H
#define NFC_SECURE_H

#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include <errno.h> /* for symbolic error codes (EINVAL, EOVERFLOW, ERANGE) */

#ifdef __cplusplus
extern "C"
{
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
 * @return  0       on success
 * @return -EINVAL  if dst or src is NULL
 * @return -EOVERFLOW if dst_size < src_size (buffer overflow prevented)
 * @return -ERANGE  if src_size or dst_size exceeds SIZE_MAX / 2
 *
 * @note This function mimics the behavior of `memcpy_s()` defined in C11
 *       Annex K but does not require optional Annex K support from the C
 *       runtime. It performs explicit size validation on the destination
 *       buffer before copying to avoid buffer overflows.
 *
 * Example usage:
 * ```c
 * uint8_t buffer[10];
 * uint8_t data[5] = {1, 2, 3, 4, 5};
 *
 * // Safe copy - will succeed
 * int result = nfc_safe_memcpy(buffer, sizeof(buffer), data, sizeof(data));
 * if (result < 0) {
 *     // Handle error
 * }
 *
 * // Unsafe copy - will fail with -EOVERFLOW
 * uint8_t small_buffer[3];
 * result = nfc_safe_memcpy(small_buffer, sizeof(small_buffer), data, sizeof(data));
 * // result == -EOVERFLOW, buffer overflow prevented
 * ```
 */
int nfc_safe_memcpy(void *dst, size_t dst_size, const void *src, size_t src_size);

/**
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
 * @return  0       on success
 * @return -EINVAL  if ptr is NULL
 * @return -ERANGE  if size exceeds SIZE_MAX / 2
 *
 * @note This function is explicitly designed to prevent the compiler from
 *       optimizing away the memory write (for example, via dead-store
 *       elimination). Use this function only for sensitive data such as
 *       cryptographic keys, passwords, or authentication tokens. For
 *       non-sensitive data, prefer the standard memset() for better
 *       performance.
 *
 * Example usage:
 * ```c
 * uint8_t key[16];
 * // ... use key for crypto operations ...
 *
 * // Securely erase key from memory
 * nfc_secure_memset(key, 0x00, sizeof(key));
 * // Compiler cannot optimize away this erasure
 * ```
 */
int nfc_secure_memset(void *ptr, int val, size_t size);

/**
 * @brief Secure zeroing helper
 *
 * Use this API to explicitly zero secrets. This function is the
 * preferred method to erase sensitive data because it is implemented to
 * prefer platform "zero-only" primitives when available (for example
 * explicit_bzero or SecureZeroMemory).
 *
 * @param[out] ptr Pointer to memory to clear (must be non-NULL)
 * @param[in] size Number of bytes to set to zero
 *
 * @return  0       on success
 * @return -EINVAL  if ptr is NULL
 * @return -ERANGE  if size exceeds SIZE_MAX / 2
 */
int nfc_secure_zero(void *ptr, size_t size);

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
 * @thread_safety These functions are thread-safe. They access only the
 * caller-provided buffers and do not touch global mutable state.
 */

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
 * NFC_SAFE_MEMCPY(buffer, data, sizeof(data)); // ❌ Dangerous: sizeof(buffer) == sizeof(void*) (e.g., 8 on x86_64)
 * ```
 */
#define NFC_SAFE_MEMCPY(dst, src, src_size) \
  nfc_safe_memcpy((dst), sizeof(dst), (src), (src_size))

/**
 * @brief Helper macro for secure memset with sizeof() validation
 *
 * Automatically calculates buffer size using sizeof(), preventing
 * manual size calculation errors.
 *
 * @param ptr Pointer to memory (must be array, not pointer)
 * @param val Value to set
 *
 * Example:
 * ```c
 * uint8_t key[16];
 * NFC_SECURE_MEMSET(key, 0x00); // Zero out securely
 * NFC_SECURE_MEMSET(key, 0xFF); // Fill with 0xFF
 * ```
 */
#define NFC_SECURE_MEMSET(ptr, val) \
  nfc_secure_memset((ptr), (val), sizeof(ptr))

/**
 * @brief Convenience macro to zero an array-sized buffer
 *
 * This macro behaves similarly to NFC_SECURE_MEMSET but is intended for
 * the zeroing use-case and expands to a call to nfc_secure_zero().
 */
#define NFC_SECURE_ZERO(ptr) \
  nfc_secure_zero((ptr), sizeof(ptr))

/**
 * @brief Safe string length calculation with maximum bound
 *
 * Calculates the length of a null-terminated string, but never scans more than
 * maxlen bytes. This prevents reading beyond buffer boundaries when strings
 * are not properly null-terminated.
 *
 * @param[in] str String to measure (can be NULL)
 * @param[in] maxlen Maximum number of bytes to scan
 * @return Length of string (excluding null terminator), or 0 if str is NULL
 * @note Returns maxlen if no null terminator found within maxlen bytes
 */
size_t nfc_safe_strlen(const char *str, size_t maxlen);

#ifdef __cplusplus
}
#endif

#endif /* NFC_SECURE_H */
