#!/bin/sh

mv .cargo/_config.toml .cargo/config.toml
mv _rust-toolchain.toml rust-toolchain.toml

wasm-pack build --release --no-default-features --features portable --target web --features wasm_thread/es_modules

mv .cargo/config.toml .cargo/_config.toml
mv rust-toolchain.toml _rust-toolchain.toml

if ! [ -x "$(command -v sfz)" ]; then
	echo 'Error: sfz (https://crates.io/crates/sfz/) is not installed.' >&2
	exit 1
fi

sfz -r --coi
