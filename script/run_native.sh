#!/bin/sh

# If you are using wayland and the app doesnt start, try uncommenting this
# WAYLAND_DISPLAY="wayland"

# Possibly better performance (https://rust-lang.github.io/packed_simd/perf-guide/target-feature/rustflags.html)
RUSTFLAGS="-C target-cpu=native"

# Log errors
RUST_LOG="error"

# cargo r --release --no-default-features
cargo r --release --no-default-features --features rayon,save_system
