
# Rust FFI

.PHONY: build-rust-ffi
build-rust-ffi:
	cargo build --release --package twsearch-ffi
	mkdir -p "./.temp"
	cargo bin cbindgen --crate twsearch-ffi --lang c --cpp-compat --output "./.temp/twsearch-ffi.h" # TODO: install `cbindgen`
	cat "./.temp/twsearch-ffi.h" | sed "s#\[..\];#;#g" | sed "s#\[.\];#;#g" | sed "s#const uint8_t#const char#g" > "./target/release/libtwsearch_ffi.h"

.PHONY: test-rust-ffi # TODO: non-PHONY?
test-rust-ffi: test-rust-ffi-rs test-rust-ffi-js test-rust-ffi-c

.PHONY: test-rust-ffi-rs
test-rust-ffi-rs: build-rust-ffi
	cargo test --package twsearch-ffi

.PHONY: test-rust-ffi-js
test-rust-ffi-js: setup-js build-rust-ffi
	bun run "src/rs-ffi/test/js_test.ts"

.PHONY: test-rust-ffi-c
test-rust-ffi-c: build-rust-ffi
	gcc -o src/rs-ffi/test/c_test.bin -L./target/release src/rs-ffi/test/c_test.c -ltwsearch_ffi
	env LD_LIBRARY_PATH=./target/release src/rs-ffi/test/c_test.bin

.PHONY: publish-rust-ffi
publish-rust-ffi:
	@echo "WARNING: will fall back to `--no-verify` due to https://github.com/rust-lang/cargo/issues/8407" # TODO
	cargo publish --package twsearch-ffi || cargo publish --package twsearch-ffi --no-verify
