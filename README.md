# Wonderdraft Asset Studio

[![CI](https://github.com/Glumbosch/assetpack_builder_for_wonderdraft/actions/workflows/build.yml/badge.svg)](https://github.com/Glumbosch/assetpack_builder_for_wonderdraft/actions/workflows/build.yml)
[![Latest release](https://img.shields.io/github/v/release/Glumbosch/assetpack_builder_for_wonderdraft)](https://github.com/Glumbosch/assetpack_builder_for_wonderdraft/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A small native desktop application for preparing Wonderdraft asset packs from ordinary images and sprite sheets.

It is written in Rust with `egui/eframe`. It does not use Electron, npm, Java, Python, or an embedded browser. Release builds are distributed as one executable per operating system.

> This is an experimental, unofficial tool. Keep backups of source images and
> project files, and inspect installed packs in Wonderdraft before relying on
> them.

This project is not affiliated with or endorsed by Wonderdraft or Megasploot.
Wonderdraft and its bundled assets are not distributed with this project.

## AI generation disclosure

The current project-authored source code was generated with AI assistance. See
the full [AI generation disclosure](AI_DISCLOSURE.md). Project-authored
material is distributed under the [MIT License](LICENSE); third-party
dependencies, Wonderdraft, and user-provided assets retain their own terms.

## Download

Prebuilt Linux (x86_64), Windows (x86_64), and macOS (Intel) executables are
attached to each [GitHub release](https://github.com/Glumbosch/assetpack_builder_for_wonderdraft/releases/latest).
The macOS binary is unsigned, so macOS may require explicit approval in
Privacy & Security on first launch.

## Implemented features

- Import one or many PNG, JPEG, WebP, BMP, or TIFF images.
- Drag image files anywhere in the application window to import them, with a full-window drop overlay.
- Draw any number of crop rectangles on one source image.
- Select, move, and resize crop rectangles using eight draggable edge/corner handles.
- Edit exact crop X, Y, width, and height values.
- Copy crop regions and extract one crop or every unextracted crop as independent sprites.
- Variable-size erase brush that sets alpha to zero.
- Variable-size restore brush that copies the original RGBA pixel values back.
- Pick a color directly from a sprite.
- Remove every pixel matching the picked color within a configurable tolerance.
- Smart edge removal using the colors in the four corners and a flood fill from the image border.
- Soften alpha edges with an adjustable box-blur radius.
- Convert alpha to fully transparent/opaque with an adjustable threshold.
- Undo and redo for destructive sprite operations and brush strokes.
- Checkerboard transparency preview.
- Sprite canvas zoom slider, mouse-wheel zoom, middle-button panning, and Fit reset.
- Keyboard undo with Ctrl+Z and redo with Ctrl+Shift+Z.
- Per-sprite Wonderdraft settings:
  - display name
  - PNG filename and metadata key
  - asset type
  - category or nested category path
  - `draw_mode`
  - `radius`
  - `offset_x`
  - `offset_y`
- Visual pivot point and collision-radius preview, both directly draggable on the sprite canvas.
- Asset types for symbols, mountains, trees, ground textures, water textures, and brushes.
- Automatic `.wonderdraft_symbols` generation in every exported sprite folder.
- Theme creation and editing using validated JSON.
- Multiple `.wonderdraft_theme` files per project.
- Project saving, including edited/original sprite images in a sidecar data folder.
- Wonderdraft directory export with duplicate-name and invalid-theme checks.
- Automatic Wonderdraft installation-folder discovery from `config.ini` on Linux, Windows, and macOS.
- One-click **Install asset pack**, honoring Wonderdraft's `custom_assets_directory` override and offering a folder picker when automatic discovery is unavailable.

## Typical workflow

1. Click **Import images**, or drag files anywhere into the window.
2. Select a source image. Imported images start with no crop regions.
3. Choose **Draw crop** and drag around every sprite contained in the source image.
4. Use **Select / move / resize** to move a crop or drag one of its eight handles.
5. Use **Copy crop** for repeated layouts, then extract one crop or click **Extract all crops as sprites**.
6. Switch among **Erase**, **Restore**, and **Pick color** in the sprite editor.
7. Zoom with the wheel or zoom slider, pan with the middle mouse button, and reset with **Fit**.
8. Drag the red pivot marker to set offsets, or drag the orange circle to set the radius.
9. Assign the asset type, category, name, draw mode, radius, and offsets.
10. Add or edit themes in the **Themes** tab.
11. Click **Install asset pack** to copy it directly into Wonderdraft, or **Export Wonderdraft pack** to choose another destination.

## Install directly into Wonderdraft

The **Install asset pack** button looks for the same Wonderdraft user-data locations used by Miracle Draft Map Helper:

- Windows: `%APPDATA%\Wonderdraft\config.ini`
- macOS: `~/Library/Application Support/Wonderdraft/config.ini`
- Linux: `~/.local/share/Wonderdraft/config.ini`

If `config.ini` contains `custom_assets_directory`, that directory takes precedence. Otherwise the standard Wonderdraft user-data directory is used. If the folder cannot be detected, the app asks you to select either the folder containing `config.ini` or the custom content root. Selecting an `assets` folder also works; the app uses its parent so exported paths remain `assets/<PackName>/...` rather than `assets/assets/<PackName>/...`.

## Exported structure

For a pack called `MyFantasyPack`, the selected export folder receives paths such as:

```text
assets/
└── MyFantasyPack/
    ├── sprites/
    │   ├── symbols/
    │   │   └── Cities/
    │   │       ├── city_01.png
    │   │       └── .wonderdraft_symbols
    │   ├── mountains/
    │   └── trees/
    └── textures/
        ├── ground/
        └── water/
brushes/
└── MyBrushes/
themes/
└── Adventure.wonderdraft_theme
```

A category may contain `/`, for example `Cities/Capital`. Each path component is sanitized before export.

## Project files

Saving `gareth.wdassetproj` also creates:

```text
gareth.wdassetproj_data/
└── sprites/
    ├── 17_original.png
    ├── 17_working.png
    └── ...
```

Keep the project file and its data folder together. Source image paths remain references to the original files. Extracted sprites can still be reopened if a source file is missing, but its crop canvas will not be available.

## Build on Linux

Install Rust using rustup, then install common native build dependencies.

Ubuntu/Debian example:

```bash
sudo apt update
sudo apt install -y \
  build-essential pkg-config libgl1-mesa-dev libx11-dev libxi-dev \
  libxcursor-dev libxrandr-dev libxinerama-dev libxkbcommon-dev \
  libwayland-dev libdbus-1-dev

cargo test
cargo build --release
```

The executable is:

```text
target/release/wonderdraft-asset-studio
```

You may copy that one file anywhere. The Linux binary uses the normal desktop graphics/window libraries already present on mainstream distributions.

A convenience script is included:

```bash
./build-linux.sh
```

### Linux application launcher

After building the release executable, add Wonderdraft Asset Studio to your
desktop application menu without administrator access:

```bash
./install-linux-launcher.sh
```

The installer copies the executable below `XDG_DATA_HOME` (normally
`~/.local/share`), installs `assetpack_builder_for_wonderdraft.png` as the
fallback icon and `assetpack_builder_for_wonderdraft.svg` as the scalable icon,
and creates
`~/.local/share/applications/wonderdraft-asset-studio.desktop`. The repository
also contains `wonderdraft-asset-studio.desktop`, the portable launcher
template used by the installer.

## Build on Windows

Install:

- Rust through rustup
- Microsoft Visual Studio Build Tools with **Desktop development with C++**

Then run in PowerShell:

```powershell
cargo test
cargo build --release
```

The executable is:

```text
target\release\wonderdraft-asset-studio.exe
```

A convenience script is included:

```powershell
.\build-windows.ps1
```

For development, `start_wonderdraft_asset_studio.bat` builds and runs the app
from the repository.

## Build on macOS

Install Rust using rustup, then run:

```bash
cargo test
cargo build --release
```

The executable is:

```text
target/release/wonderdraft-asset-studio
```

## Automatic Linux, Windows, and macOS builds

The included GitHub Actions workflow builds and tests Linux, Windows, and macOS versions. Push the project to GitHub, open the **Actions** tab, run **Build release binaries**, and download the three artifacts.

Creating a Git tag beginning with `v`, such as `v0.2.0`, also creates a GitHub Release and attaches all three binaries.

## Development and validation

```bash
cargo fmt --all -- --check
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the contribution workflow,
[AI_DISCLOSURE.md](AI_DISCLOSURE.md) for the AI-use disclosure, and
[LICENSE](LICENSE) for licensing terms.

## Algorithm correspondence

The JavaScript operations supplied for the project were ported as follows:

- `smartEdgeRemove`: corner-color sampling, `tolerance × 1.73`, border-seeded four-neighbour flood fill, then alpha set to zero.
- `removePickedColor`: Euclidean RGB distance against the selected color, using the same tolerance multiplier.
- `softenAlpha`: box blur of only the alpha channel. The Rust implementation uses a summed-area table, so larger radii remain fast.
- `thresholdAlpha`: alpha below the selected threshold becomes 0; all other alpha becomes 255.

## Current scope

This version intentionally leaves out AI background removal. It also does not import font files; fonts can be copied manually into the exported pack. The theme editor exposes the complete format as JSON so unknown or future Wonderdraft properties are not discarded.
