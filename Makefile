# nes-rs Makefile — formatting, linting, building, packaging.
#
# Targets:
#   make check      — fmt check + clippy + tests (CI default)
#   make build      — debug build for the host
#   make release    — optimised release build for the host
#   make fmt        — apply rustfmt
#   make lint       — run clippy
#   make test       — run tests
#   make clean      — remove build artefacts
#   make package    — build + strip + tar/zip for the host target
#   make docs       — build mdBook documentation
#
# Cross-compilation (requires the target toolchain installed):
#   make release TARGET=x86_64-pc-windows-gnu
#   make package TARGET=x86_64-pc-windows-gnu

CARGO   := cargo
BINARY  := nes-rs
TARGET  ?=

# Derive cargo flags from TARGET.
ifdef TARGET
  CARGO_FLAGS := --target $(TARGET)
  TARGET_DIR  := target/$(TARGET)/release
else
  CARGO_FLAGS :=
  TARGET_DIR  := target/release
endif

# Detect binary extension for the target.
ifneq (,$(findstring windows,$(TARGET)))
  EXT := .exe
else ifneq (,$(findstring windows,$(shell rustc -vV 2>/dev/null | grep host)))
  ifndef TARGET
    EXT := .exe
  else
    EXT :=
  endif
else
  EXT :=
endif

# ── Quality ──────────────────────────────────────────────────────

.PHONY: fmt
fmt:
	$(CARGO) fmt
	taplo fmt

.PHONY: fmt-check
fmt-check:
	$(CARGO) fmt -- --check
	taplo fmt --check

.PHONY: toml-lint
toml-lint:
	taplo lint

.PHONY: lint
lint:
	$(CARGO) clippy $(CARGO_FLAGS) -- -D warnings

.PHONY: test
test:
	$(CARGO) test $(CARGO_FLAGS)

.PHONY: check
check: fmt-check toml-lint lint test

# ── Build ────────────────────────────────────────────────────────

.PHONY: build
build:
	$(CARGO) build $(CARGO_FLAGS)

.PHONY: release
release:
	$(CARGO) build --release $(CARGO_FLAGS)

.PHONY: clean
clean:
	$(CARGO) clean

# ── Package ──────────────────────────────────────────────────────

# Determine archive name components.
VERSION  := $(shell grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
ifdef TARGET
  ARCHIVE_STEM := $(BINARY)-$(VERSION)-$(TARGET)
else
  ARCHIVE_STEM := $(BINARY)-$(VERSION)-$(shell rustc -vV | grep host | awk '{print $$2}')
endif

# Extra files bundled into every release archive.
BUNDLE_FILES := README.md LICENSE

.PHONY: package
package: release
ifneq (,$(findstring windows,$(TARGET))$(findstring windows,$(shell rustc -vV 2>/dev/null | grep host)))
	@mkdir -p dist
	cd $(TARGET_DIR) && zip -j ../../../dist/$(ARCHIVE_STEM).zip $(BINARY)$(EXT)
	zip -j dist/$(ARCHIVE_STEM).zip $(BUNDLE_FILES)
	@echo "Packaged: dist/$(ARCHIVE_STEM).zip"
else
	@mkdir -p dist
	strip $(TARGET_DIR)/$(BINARY)$(EXT) 2>/dev/null || true
	tar -czf dist/$(ARCHIVE_STEM).tar.gz \
		-C $(TARGET_DIR) $(BINARY)$(EXT) \
		-C $(CURDIR) $(BUNDLE_FILES)
	@echo "Packaged: dist/$(ARCHIVE_STEM).tar.gz"
endif

# ── Docs ────────────────────────────────────────────────────────

.PHONY: docs
docs:
	mdbook-mermaid install docs
	mdbook build docs
