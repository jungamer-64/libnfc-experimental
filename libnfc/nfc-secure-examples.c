/**
 * @file nfc-secure-examples.c
 * @brief Practical usage examples for nfc-secure library
 * 
 * This file demonstrates best practices and common patterns for
 * using the nfc-secure memory operations library.
 */

#include "nfc-secure.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 * Example 1: Basic Array Copy (Compile-Time Checked)
 * ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */
void example_basic_array_copy(void)
{
    printf("\n=== Example 1: Basic Array Copy ===\n");
    
    uint8_t nfc_uid[10] = {0x04, 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0x80, 0x00, 0x00};
    uint8_t uid_backup[10];
    
    /* ✅ GOOD: Using macro with fixed-size array */
    int result = NFC_SAFE_MEMCPY(uid_backup, nfc_uid, sizeof(nfc_uid));
    
    if (result == NFC_SECURE_SUCCESS) {
        printf("✓ UID copied successfully\n");
        printf("  UID: ");
        for (size_t i = 0; i < sizeof(nfc_uid); i++) {
            printf("%02X ", uid_backup[i]);
        }
        printf("\n");
    } else {
        fprintf(stderr, "✗ Copy failed: %s\n", nfc_secure_strerror(result));
    }
}

/* ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 * Example 2: Dynamic Memory (Runtime Size)
 * ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */
void example_dynamic_memory(size_t buffer_size)
{
    printf("\n=== Example 2: Dynamic Memory (size=%zu) ===\n", buffer_size);
    
    uint8_t *buffer = malloc(buffer_size);
    if (!buffer) {
        fprintf(stderr, "✗ malloc failed\n");
        return;
    }
    
    uint8_t data[16] = "Hello, NFC!";
    
    /* ✅ GOOD: Explicit size for dynamic memory */
    int result = nfc_safe_memcpy(buffer, buffer_size, data, sizeof(data));
    
    if (result == NFC_SECURE_SUCCESS) {
        printf("✓ Data copied to dynamic buffer: %s\n", buffer);
    } else {
        fprintf(stderr, "✗ Copy failed: %s\n", nfc_secure_strerror(result));
    }
    
    /* ❌ WRONG: This would cause a bug!
     * NFC_SAFE_MEMCPY(buffer, data, sizeof(data));
     * → sizeof(buffer) == pointer size (4 or 8 bytes), not buffer_size!
     */
    
    free(buffer);
}

/* ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 * Example 3: Secure Key Erasure (Crypto)
 * ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */
void example_secure_key_erasure(void)
{
    printf("\n=== Example 3: Secure Key Erasure ===\n");
    
    /* Simulated MIFARE Classic key */
    uint8_t mifare_key[6] = {0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF};
    
    printf("Before: Key = ");
    for (size_t i = 0; i < sizeof(mifare_key); i++) {
        printf("%02X ", mifare_key[i]);
    }
    printf("\n");
    
    /* ... use key for authentication ... */
    
    /* ✅ GOOD: Securely erase key after use */
    int result = NFC_SECURE_MEMSET(mifare_key, 0x00);
    
    if (result == NFC_SECURE_SUCCESS) {
        printf("✓ Key securely erased (compiler cannot optimize away)\n");
        printf("After:  Key = ");
        for (size_t i = 0; i < sizeof(mifare_key); i++) {
            printf("%02X ", mifare_key[i]);
        }
        printf("\n");
    } else {
        fprintf(stderr, "✗ Secure erase failed: %s\n", nfc_secure_strerror(result));
    }
}

/* ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 * Example 4: Overlapping Buffers (memmove required)
 * ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */
void example_overlapping_buffers(void)
{
    printf("\n=== Example 4: Overlapping Buffers ===\n");
    
    uint8_t buffer[20] = "ABCDEFGHIJ";
    printf("Before: %s\n", buffer);
    
    /* ✅ GOOD: Use memmove for overlapping regions */
    /* Move "ABCDE" to position 5, creating "ABCDEABCDE" */
    int result = nfc_safe_memmove(buffer + 5, 15, buffer, 5);
    
    if (result == NFC_SECURE_SUCCESS) {
        printf("After:  %s\n", buffer);
        printf("✓ Overlapping move succeeded\n");
    } else {
        fprintf(stderr, "✗ Move failed: %s\n", nfc_secure_strerror(result));
    }
    
    /* ⚠️  WARNING: Using memcpy with overlap is undefined behavior!
     * nfc_safe_memcpy(buffer + 5, 15, buffer, 5);  // ❌ WRONG!
     */
}

/* ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 * Example 5: Buffer Overflow Prevention
 * ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */
void example_overflow_prevention(void)
{
    printf("\n=== Example 5: Buffer Overflow Prevention ===\n");
    
    uint8_t small_buffer[5];
    uint8_t large_data[10] = {1, 2, 3, 4, 5, 6, 7, 8, 9, 10};
    
    /* This will fail safely instead of overflowing */
    int result = NFC_SAFE_MEMCPY(small_buffer, large_data, sizeof(large_data));
    
    if (result == NFC_SECURE_ERROR_OVERFLOW) {
        printf("✓ Buffer overflow prevented!\n");
        printf("  Attempted: %zu bytes → %zu byte buffer\n", 
               sizeof(large_data), sizeof(small_buffer));
        printf("  Error: %s\n", nfc_secure_strerror(result));
    } else {
        fprintf(stderr, "✗ Unexpected result: %d\n", result);
    }
    
    /* ✅ GOOD: Copy only what fits */
    result = nfc_safe_memcpy(small_buffer, sizeof(small_buffer), 
                              large_data, sizeof(small_buffer));
    if (result == NFC_SECURE_SUCCESS) {
        printf("✓ Partial copy succeeded (first %zu bytes)\n", sizeof(small_buffer));
    }
}

/* ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 * Example 6: Zero-Size Detection
 * ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */
void example_zero_size_detection(void)
{
    printf("\n=== Example 6: Zero-Size Detection ===\n");
    
    uint8_t buffer[10];
    uint8_t data[10];
    
    /* This triggers a warning (suspicious usage) */
    int result = nfc_safe_memcpy(buffer, sizeof(buffer), data, 0);
    
    if (result == NFC_SECURE_ERROR_ZERO_SIZE) {
        printf("✓ Zero-size operation detected (likely a bug)\n");
        printf("  Error: %s\n", nfc_secure_strerror(result));
        printf("  This may indicate incorrect sizeof() or length calculation\n");
    }
}

/* ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 * Example 7: Error Handling Best Practices
 * ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */
int example_error_handling(const uint8_t *nfc_data, size_t nfc_len)
{
    printf("\n=== Example 7: Error Handling Best Practices ===\n");
    
    uint8_t buffer[256];
    
    /* ✅ GOOD: Comprehensive error handling */
    int result = nfc_safe_memcpy(buffer, sizeof(buffer), nfc_data, nfc_len);
    
    switch (result) {
        case NFC_SECURE_SUCCESS:
            printf("✓ Copy succeeded\n");
            return 0;
            
        case NFC_SECURE_ERROR_INVALID:
            fprintf(stderr, "✗ Invalid parameter (NULL pointer)\n");
            return -1;
            
        case NFC_SECURE_ERROR_OVERFLOW:
            fprintf(stderr, "✗ Buffer too small (%zu bytes needed, %zu available)\n",
                    nfc_len, sizeof(buffer));
            return -1;
            
        case NFC_SECURE_ERROR_RANGE:
            fprintf(stderr, "✗ Size exceeds maximum allowed\n");
            return -1;
            
        case NFC_SECURE_ERROR_ZERO_SIZE:
            fprintf(stderr, "⚠  Warning: Zero-size copy (possible bug)\n");
            return 0;  /* Treat as success but log warning */
            
        default:
            fprintf(stderr, "✗ Unknown error code: %d\n", result);
            return -1;
    }
}

/* ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 * Example 8: Performance-Conscious Usage
 * ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */
void example_performance_conscious(void)
{
    printf("\n=== Example 8: Performance-Conscious Usage ===\n");
    
    /* Small sensitive data: Use secure memset */
    uint8_t aes_key[32] = {0};
    printf("Clearing AES-256 key (32 bytes): Use nfc_secure_memset()\n");
    nfc_secure_memset(aes_key, 0, sizeof(aes_key));
    printf("  ✓ Secure clear (optimized for small buffers)\n");
    
    /* Large non-sensitive buffer: Consider standard memset */
    uint8_t *large_buffer = malloc(10000);
    if (large_buffer) {
        printf("Clearing large buffer (10KB): Consider standard memset()\n");
        
        /* For truly sensitive data, use nfc_secure_memset despite cost */
        nfc_secure_memset(large_buffer, 0, 10000);
        printf("  ✓ Secure clear (uses memset+barrier for large size)\n");
        printf("  ⚠  ~10-30%% slower than standard memset\n");
        
        free(large_buffer);
    }
    
    printf("\n💡 RECOMMENDATION:\n");
    printf("  - Crypto keys (<100 bytes): Always use nfc_secure_memset()\n");
    printf("  - Large buffers (>1KB): Evaluate if data is truly sensitive\n");
    printf("  - Non-sensitive data: Use standard memset() for performance\n");
}

/* ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 * Main: Run All Examples
 * ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */
int main(void)
{
    printf("\n╔════════════════════════════════════════════════════════╗\n");
    printf("║   NFC-SECURE Library - Practical Usage Examples       ║\n");
    printf("╚════════════════════════════════════════════════════════╝\n");
    
    example_basic_array_copy();
    example_dynamic_memory(128);
    example_secure_key_erasure();
    example_overlapping_buffers();
    example_overflow_prevention();
    example_zero_size_detection();
    
    /* Error handling example */
    uint8_t test_data[100] = {0};
    example_error_handling(test_data, sizeof(test_data));
    
    example_performance_conscious();
    
    printf("\n╔════════════════════════════════════════════════════════╗\n");
    printf("║   All examples completed successfully!                 ║\n");
    printf("╚════════════════════════════════════════════════════════╝\n\n");
    
    return 0;
}
