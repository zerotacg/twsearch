# Rust WASM

.PHONY: build-rust-wasm
build-rust-wasm:
	rm -rf "./.temp/rust-wasm"
	cargo tool-run-bin wasm-pack build --release --target web --out-dir "../../.temp/rust-wasm" src/rs-wasm
	bun run script/node-esm-compat.ts
	bun run "./script/build-wasm-package.ts"

.PHONY: test-rust-wasm
test-rust-wasm: build-rust-wasm
	bun run "script/test-dist-wasm.ts"
