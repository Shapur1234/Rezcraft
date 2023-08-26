#!/bin/sh

mv .cargo/_config.toml .cargo/config.toml
wasm-pack build --release --target web --features wasm_thread/es_modules
mv .cargo/config.toml .cargo/_config.toml

cargo build --release --target x86_64-unknown-linux-gnu
cargo build --release --target x86_64-pc-windows-gnu
