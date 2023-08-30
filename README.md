# [Rezcraft](https://shapur1234.github.io/Rezcraft-Demo/ "Link to web version (mobile controls not supported)")

* Minecraft like game written in rust using wgpu
* Supports both native targets and [wasm](https://en.wikipedia.org/wiki/WebAssembly)

## Screenshots

![Sunlight](/screenshot/2.png?raw=true "Sunlight")
![Lighting](/screenshot/3.png?raw=true "Lighting")
![UI](/screenshot/4.png?raw=true "UI")

## Features

* Highly configurable through in-game settings
* Parallelised world generation
* Efficient meshes using [greedy meshing](https://0fps.net/2012/06/30/meshing-in-a-minecraft-game/)
* Colored lighting system, sunlight
* Transparency (native only)
* Savegame system (native only)
* Easily add custom textures and blocks, modify blocktypes at runtime

## How to

### Installation: 

* Run the [web version](https://shapur1234.github.io/Rezcraft-Demo/ "Link to web version (mobile controls not supported)") without installing anything
* Precompiled binaries can be found under [releases](https://github.com/Shapur1234/Rezcraft/releases)
  * Download a binary for your system and `resource.zip`
  * Extract both archives in the same directory, so `rezcraft` (or `rezcraft.exe` on windows) and the `resouce` directory are in the same directory

* Your directory should look like this: 
```
├── resource
│   ├── block
│   │   ├── ...
│   ├── icon.png
│   ├── shader
│   │   └── ...
│   └── texture
│       └── ...
└── rezcraft (rezcraft.exe on windows)
```
![Directory](/screenshot/directory_strucutre.png?raw=true "Directory")


### Controls:

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

## How to run and build locally

* To build for native, use cargo normally (`cargo build --release`) or use the [run_native.sh](/script/run_native.sh) script
* To build to wasm, use the [run_wasm.sh](/script/run_wasm.sh) script

## Possible plans for future updates

* Improved worldgen
* Editable controls
* Physics, improved collision detection
* Optionally bake assets into the binary
* Fancy shader effects
* Multiplayer
