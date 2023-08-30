#!/bin/sh

cargo clean

rm -f -rf ./pkg
rm -f -rf ./cargo

rm -f ./perf.data
rm -f ./flamegraph.svg
rm -f ./Cargo.lock
rm -f ./resource/.xdp-icon.ico-5bKR00
