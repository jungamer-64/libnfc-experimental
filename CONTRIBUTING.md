# Contributing to libnfc

Thank you for your interest in contributing to libnfc!

## Code Quality Standards

This project maintains high code quality standards to ensure reliability and maintainability:

### Quality Gates

All contributions must meet the following minimum requirements:

* **Code Grade**: B (75%+) on Codacy
* **Test Coverage**: 60%+ for new code (Phase 12+)
* **Duplication**: <30% (target: <20%)
* **Complex Files**: <30% of total files
* **Build**: All targets compile without errors

### Continuous Integration

Every commit is automatically verified through GitHub Actions:

1. **Codacy Analysis**: Static code analysis and security scanning
2. **Build & Test**: Compilation and unit tests (when available)
3. **Code Quality Gate**: Verification of quality standards

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

Use the frame processing utilities from `nfc-frame.h` (Phase 11 Week 3+):

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

## Testing (Phase 12+)

### Coverage Requirements

* **New Functions**: 80%+ line coverage required
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
* Add tests for new functionality (Phase 12+)
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
# Check: https://app.codacy.com/gh/jungamer-64/libnfc/dashboard
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
* ✅ Build succeeds
* ✅ Tests pass (Phase 12+)
* ✅ Code quality standards met
* ✅ Coverage targets achieved (Phase 12+)

## Refactoring Guidelines (Phase 11)

### Complexity Reduction

Target: Reduce cyclomatic complexity (CC) of complex functions

**High Priority Functions** (Week 2):
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

Target: 30% → 20% duplication

**Common Patterns** (Week 3):
* Frame preamble: `0x00 0x00 0xff`
* Checksum calculation (LCS, DCS)
* Length encoding
* Error frame detection

**Solution**: Centralized `nfc-frame.h/c` utilities

## Project Phases

### Phase 11: Code Quality Enhancement (Current)

**Week 1-2**: ✅ Foundation
* Memory safety layer (nfc-secure)
* Unified error handling (nfc-common)
* Driver refactoring (4 drivers)
* CI/CD pipeline

**Week 2**: 🔄 High-Complexity Refactoring
* Target: 6 functions with CC > 20
* Goal: Reduce average complexity

**Week 3**: Frame Processing
* Create nfc-frame utilities
* Reduce code duplication
* Standardize frame handling

**Week 4**: CI/CD Enhancement
* Coverage integration ✅
* Quality gates
* Documentation updates

### Phase 12: Test Suite (Future)

**Goals**:
* 60%+ overall coverage
* 80%+ coverage for new code
* Unit tests for core functions
* Integration tests for drivers

## Resources

* **Security**: [SECURITY.md](SECURITY.md)
* **Memory Safety**: [libnfc/NFC_SECURE_USAGE_GUIDE.md](libnfc/NFC_SECURE_USAGE_GUIDE.md)
* **Code Quality**: https://app.codacy.com/gh/jungamer-64/libnfc/dashboard
* **CI/CD Pipeline**: https://github.com/jungamer-64/libnfc/actions

## Questions?

* Open an issue: https://github.com/jungamer-64/libnfc/issues
* Check existing discussions: https://github.com/jungamer-64/libnfc/discussions

Thank you for contributing to libnfc! 🚀
