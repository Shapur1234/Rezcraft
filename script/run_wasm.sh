#! /bin/sh

mkdir .cargo

echo "[target.wasm32-unknown-unknown]
rustflags = [\"-C\", \"target-feature=+atomics,+bulk-memory,+mutable-globals\"]

[unstable]
build-std = [\"std,panic_abort\"]" >.cargo/config.toml

trunk serve --release
rm .cargo/config.toml
rmdir .cargo
