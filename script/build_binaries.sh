#! /bin/sh

function get_binary_path() {
  echo | grep -o '"[^"]\+"' $1 | tr -d '"' | grep -o '.*-wrapped'
}

rm -rf ./bin
mkdir ./bin

nix build .#rezcraft-native
cp $(get_binary_path ./result/bin/rezcraft-native) ./bin/rezcraft-linux_amd64 --no-preserve=mode,ownership
chmod +x ./bin/rezcraft-linux_amd64

nix build .#rezcraft-win
cp ./result/bin/rezcraft-native.exe ./bin/rezcraft-win_amd64.exe --no-preserve=mode,ownership
chmod +x ./bin/rezcraft-win_amd64.exe
