RUST_TARGET_DIR = $(abs_top_builddir)/rust/target
RUST_RELEASE_LIB = $(RUST_TARGET_DIR)/release/liblibnfc_rs.a

RUST_SOURCES = \
	libnfc-rs/src/lib.rs

EXTRA_DIST = \
	libnfc-rs/Cargo.toml \
	$(RUST_SOURCES)

all-local: $(RUST_RELEASE_LIB)

$(RUST_RELEASE_LIB): libnfc-rs/Cargo.toml $(RUST_SOURCES)
	$(AM_V_GEN)cd $(top_srcdir)/rust/libnfc-rs && \
		CARGO_TARGET_DIR=$(RUST_TARGET_DIR) $(CARGO) build --release $(RUST_CARGO_FEATURES)

if USE_RUST_NFC_SECURE
RUST_CARGO_FEATURES = --features nfc_secure
else
RUST_CARGO_FEATURES =
endif

clean-local:
	-rm -rf $(RUST_TARGET_DIR)
