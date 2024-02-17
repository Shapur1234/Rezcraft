#! /bin/sh

rm -rf ./docs
mkdir ./docs

nix build .#rezcraft-wasm
cp -a ./result/. ./docs/ --no-preserve=mode,ownership
