#!/bin/sh

export RUSTUP_TOOLCHAIN="nightly"

RUSTFLAGS="-C target-feature=+atomics,+bulk-memory,+mutable-globals" \
	wasm-pack build --target web --features wasm_thread/es_modules

if ! [ -x "$(command -v sfz)" ]; then
	echo 'Error: sfz (https://crates.io/crates/sfz/) is not installed.' >&2
	exit 1
fi

sfz -r --coi
