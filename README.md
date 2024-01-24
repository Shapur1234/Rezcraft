# [Rezcraft](https://shapur1234.github.io/Rezcraft-Demo/ "Link to web version (mobile controls not supported)")

- Voxel engine written in rust using wgpu
- Supports both native targets and [wasm](https://en.wikipedia.org/wiki/WebAssembly)

## Screenshots

![Sunlight](./screenshot/2.png?raw=true "Sunlight")
![Lighting](./screenshot/3.png?raw=true "Lighting")
![UI](./screenshot/4.png?raw=true "UI")

## Features

- Parallelised world and mesh generation
- Efficient meshes using [greedy meshing](https://0fps.net/2012/06/30/meshing-in-a-minecraft-game/)
- Easily add custom textures and blocks, modify blocktypes at runtime
- Colored lighting system, sunlight
- Configurable through in-app settings
- Transparency (native only)
- Savegame system (native only)

## How to

### Installation

- Run the [web version](https://shapur1234.github.io/Rezcraft-Demo/ "Link to web version (mobile controls not supported)") without installing anything
- Precompiled binaries can be found under [releases](https://github.com/Shapur1234/Rezcraft/releases), these binaries have all assets baked into themselves, so you need no resource directory

- Alternatively, if you compile `rezcraft` without the `portable` feature enabled, setup your file structure like this:
  - Have the binary `rezcraft` (or `rezcraft.exe` on windows) and the `res` directory are in the same directory
  - Your directory tree should look like this:

```
├── res
│   ├── block
│   │   └── ...
│   ├── icon.png
│   ├── shader
│   │   └── ...
│   └── texture
│       └── ...
└── rezcraft (rezcraft.exe on windows)
```

- - The location of the save and resource directories (defaults are `./saves` and `./res`) can be change by setting the `SAVES_PATH` and `RESOURCE_PATH` enviromental variable

### Adding custom textures and blocks

#### Textures

- Add a `.png` image to `./res/texture/`, it will be loaded after programm restart
- All texture must be square and all textures must have the same resolution

#### Blocks

- Add a `.yaml` block describing file to `./res/block/`, use one of the exisitng files as a tempalte
- Blocks and their textures, light souces, and properties such as transparency and solidness can also be edited at runtime (`Edit block` menu while paused)

### Controls

| Key             | Action                           |
| --------------- | -------------------------------- |
| Mouse motion    | Rotate camera                    |
| W / ArrowUp     | Move forward                     |
| S / ArrowDown   | Move back                        |
| A / ArrowLeft   | Move left                        |
| D / ArrowRight  | Move right                       |
| Space / K       | Move up                          |
| LShift / J      | Move down                        |
| X / MouseRight  | Delete block                     |
| C / MouseLeft   | Place block                      |
| V / MouseMiddle | Pick block                       |
| M               | Reload chunk at players position |
| F5              | Save                             |
| F9              | Load                             |
| F11             | Toggle fullscreen                |
| F12             | Reload settings from config file |
| Tab             | Pause / Resume                   |
| Escape          | Exit                             |

## Building using cargo

- Have [rust](https://www.rust-lang.org/tools/install) installed, or optionally use the included dev shelle: `nix develop`
- Pick feautres

| Feature     | Description                                                                                     | Notes                                     |
| ----------- | ----------------------------------------------------------------------------------------------- | ----------------------------------------- |
| portable    | Doesn't read resources (textures, shaders...) from disk, but instead bakes them into the binary | Must be enabled when compiling for `wasm` |
| save_system | Allow for saving and olding of the world                                                        | Doesn't work with `wasm`                  |
| rayon       | Extra pararelism for loading the world and saving                                               | Doesn't work with `wasm`                  |

- Manually
  - To build - `cargo build --no-default-features --release --features "Feature1 Feature2"`
  - To run - `cargo run --no-default-features --release --features "Feature1 Feature2"`
  - To build for wasm - `wasm-pack build --release --no-default-features --features portable --target web --features wasm_thread/es_modules`
- Using a build script f
  - To build for native targets - [run_native.sh](./script/run_native.sh)
  - To build for wasm - [run_wasm.sh](./script/run_wasm.sh)

## Links

- Source repo - [https://github.com/Shapur1234/Rezcraft](https://github.com/Shapur1234/Rezcraft)
- Crate.io - [https://crates.io/crates/rezcraft](https://crates.io/crates/rezcraft)

## Possible plans for future updates

- Improved worldgen
- Editable controls
- Physics, improved collision detection
- Optionally bake assets into the binary
- Fancy shader effects
- Multiplayer
