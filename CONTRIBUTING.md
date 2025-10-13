# Contributing to libnfc

Thank you for your interest in contributing to libnfc!

## Code Quality Standards

All contributions should meet the following requirements:

* **Test Coverage**: 60%+ for new code (when test infrastructure is available)
* **Duplication**: <30%
* **Build**: All targets compile without errors

### Continuous Integration

Every commit is automatically verified:

1. **Static Analysis**: Code analysis and security scanning
2. **Build & Test**: Compilation and unit tests (when available)
3. **Quality Gate**: Verification of standards

View the CI/CD pipeline: [.github/workflows/code-quality.yml](.github/workflows/code-quality.yml)

## Code Standards

### Memory Safety

All new code **must** use the `nfc-secure` memory safety layer:

```c
#include <nfc/nfc-secure.h>

// ✅ Good: Safe memory operations
NFC_SAFE_MEMCPY(dest, src, size);
NFC_SECURE_MEMSET(password, 0x00);

// ❌ Bad: Direct memory operations
memcpy(dest, src, size);
memset(password, 0, sizeof(password));
```

See [libnfc/NFC_SECURE_USAGE_GUIDE.md](libnfc/NFC_SECURE_USAGE_GUIDE.md) for complete usage guide.

### Error Handling

Use the unified error handling infrastructure from `nfc-common.h`:

```c
#include "nfc-common.h"

// ✅ Good: Structured error handling
NFC_LOG_ERROR("Failed to open device: %s", error);
return nfc_error_set(NFC_EIO, "Device initialization failed");

// ❌ Bad: Direct perror() calls
perror("open failed");
return -1;
```

### Frame Processing

Use the frame processing utilities from `nfc-frame.h`:

```c
#include "nfc-frame.h"

// ✅ Good: Centralized frame handling
if (!nfc_frame_validate_header(frame, len)) {
    return NFC_EINVARG;
}

// ❌ Bad: Duplicate frame validation logic
if (frame[0] != 0x00 || frame[1] != 0x00 || frame[2] != 0xff) {
    return -1;
}
```

## Testing

### Coverage Requirements

* **New Functions**: 80%+ line coverage
* **Modified Functions**: Maintain or improve existing coverage
* **Critical Paths**: 100% coverage for security-sensitive code

### Test Organization

```
test/
├── unit/           # Unit tests for individual functions
├── integration/    # Integration tests for driver interaction
└── fixtures/       # Test data and mock devices
```

### Running Tests

```bash
# Build with coverage
./configure CFLAGS="--coverage -g -O0" LDFLAGS="--coverage"
make -j$(nproc)

# Run tests
make check

# Generate coverage report
lcov --capture --directory . --output-file coverage.info
lcov --remove coverage.info '/usr/*' '*/test/*' --output-file coverage_filtered.info
genhtml coverage_filtered.info --output-directory coverage_html
```

## Development Workflow

### 1. Create Feature Branch

```bash
git checkout -b feature/your-feature-name
```

### 2. Make Changes

* Follow code standards above
* Add tests for new functionality (when test infrastructure is available)
* Update documentation if needed

### 3. Verify Locally

```bash
# Build
autoreconf -vis
./configure
make -j$(nproc)

# Run tests (when available)
make check

    # Verify no new issues
    # Check: code quality dashboard (internal)
```

### 4. Commit Changes

Use conventional commit messages:

```bash
git commit -m "feat: Add new driver support for XYZ"
git commit -m "fix: Resolve buffer overflow in acr122_usb_receive"
git commit -m "docs: Update installation instructions"
git commit -m "test: Add unit tests for nfc_initiator_init"
```

Commit types:

* `feat`: New feature
* `fix`: Bug fix
* `docs`: Documentation only
* `test`: Adding or modifying tests
* `refactor`: Code refactoring without behavior change
* `perf`: Performance improvement
* `style`: Code style/formatting
* `chore`: Build system, dependencies, etc.

### 5. Push and Create Pull Request

```bash
git push origin feature/your-feature-name
```

Then create a Pull Request on GitHub. The CI/CD pipeline will automatically verify:

* Build succeeds
* Tests pass (when available)
* Code quality standards met
* Coverage targets achieved (when test infrastructure is available)

## FFI Change Checklist

When your change touches the FFI boundary (Rust ⇄ C), include the following in your PR description and ensure CI checks pass:

1. Regenerate and commit the cbindgen header (if any symbol/signature/layout changed):

        ```sh
        cbindgen --config rust/libnfc-rs/cbindgen.toml --crate libnfc-rs --output rust/libnfc-rs/include/libnfc_rs.h
        ```

2. Include a short ownership table: who allocates / who frees any returned buffers (use `nfc_free_*` wrappers for CallerFree cases).
3. Demonstrate `nfc_set_last_error()` usage for error paths where appropriate and list the error codes added.
4. Add or update unit tests (Rust) and C integration tests that exercise success and failure paths. `ffi-sanity` integration must pass.
5. Ensure `scripts/check-cbindgen.sh` and `scripts/check_callerfree_usage.sh` pass locally and in CI.
6. Coverage thresholds: changed modules should maintain >= 80% line coverage; critical FFI paths require 100% tests where feasible.
7. If the change is ABI-breaking, include an RFC and obtain maintainers' approval before merging.

CI will gate FFI PRs on the above checks — failing the header check or ffi-sanity will block merges.

## Refactoring Guidelines

### Complexity Reduction

Target: Reduce cyclomatic complexity (CC) of complex functions

**High Priority Functions**:

* `acr122_usb_receive` (CC: 26 → 12)
* `pn53x_usb_open` (CC: 25 → 10)
* `pn532_uart_receive` (CC: 22 → 10)
* `pn53x_usb_set_property_bool` (CC: 20 → 8)
* `arygon_tama_receive` (CC: 20 → 10)

**Strategy**: Extract Method refactoring

```c
// Before: Complex function (CC: 26)
int acr122_usb_receive(nfc_device *pnd, ...) {
    // 26 decision points
    // Frame validation
    // Timeout handling
    // Error codes
}

// After: Helper functions (CC: 12)
static int acr122_validate_frame_header(...);
static int acr122_handle_timeout(...);
static int acr122_process_response(...);

int acr122_usb_receive(nfc_device *pnd, ...) {
    // 12 decision points
    if (!acr122_validate_frame_header(...)) return NFC_EINVARG;
    if (acr122_handle_timeout(...) < 0) return NFC_ETIMEOUT;
    return acr122_process_response(...);
}
```

### Duplication Reduction

Target: Reduce duplication from 30% to 20%

**Common Patterns**:

* Frame preamble: `0x00 0x00 0xff`
* Checksum calculation (LCS, DCS)
* Length encoding
* Error frame detection

**Solution**: Centralized `nfc-frame.h/c` utilities

## Development Goals

### Code Quality Enhancement

**Completed**:

* Memory safety layer (nfc-secure)
* Unified error handling (nfc-common)
* Driver refactoring (4 drivers)
* CI/CD pipeline

**In Progress**:

* High-complexity function refactoring
* Frame processing utilities
* Code duplication reduction

**Future Work**:

* Test suite development
* Coverage targets: 60%+ overall, 80%+ for new code
* Unit tests for core functions
* Integration tests for drivers

## Resources

* **Security**: [SECURITY.md](SECURITY.md)
* **Memory Safety**: [libnfc/NFC_SECURE_USAGE_GUIDE.md](libnfc/NFC_SECURE_USAGE_GUIDE.md)
* **CI/CD Pipeline**: [GitHub Actions](https://github.com/jungamer-64/libnfc/actions)

## Questions?

* [Open an issue](https://github.com/jungamer-64/libnfc/issues)
* [Check existing discussions](https://github.com/jungamer-64/libnfc/discussions)

Thank you for contributing to libnfc!
