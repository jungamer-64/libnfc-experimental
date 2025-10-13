libnfc-rs: C FFI helpers for libnfc

This crate exposes a set of small utility functions intended to replace
and harden existing C implementations in libnfc. The functions are
exported with a stable C ABI and are safe to call from C code when the
caller follows the documented safety preconditions.

Examples (C)

- Secure memcpy

```c
#include <libnfc_rs.h>

int example_memcpy(void) {
    char dst[16];
    const char src[] = "hello";
    int rc = nfc_safe_memcpy(dst, sizeof(dst), src, sizeof(src) - 1);
    if (rc != NFC_SECURE_SUCCESS) {
        // handle error
        return rc;
    }
    dst[sizeof(src)-1] = '\0';
    return NFC_SECURE_SUCCESS;
}
```

- Secure memset (zeroing sensitive buffers)

```c
#include <libnfc_rs.h>

void scrub_secret(void *ptr, size_t len) {
    if (nfc_secure_memset(ptr, 0, len) != NFC_SECURE_SUCCESS) {
        // handle error
    }
}
```

- String helpers

```c
#include <libnfc_rs.h>
#include <stdio.h>

void example_strlen(const char *s) {
    size_t l = nfc_safe_strlen(s, 256);
    printf("len=%zu\n", l);
}
```
