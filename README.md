# Wonderdraft Asset Studio

A small native desktop application for preparing Wonderdraft asset packs from ordinary images and sprite sheets.

It is written in Rust with `egui/eframe`. It does not use Electron, npm, Java, Python, or an embedded browser. Release builds are distributed as one executable per operating system.

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
10. Add or edit themes in the **Themes** tab, then click **Export Wonderdraft pack**.

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

## Automatic Windows and Linux builds

The included GitHub Actions workflow builds and tests both operating-system versions. Push the project to GitHub, open the **Actions** tab, run **Build release binaries**, and download the two artifacts.

Creating a Git tag beginning with `v`, such as `v0.2.0`, also runs the workflow.

## Algorithm correspondence

The JavaScript operations supplied for the project were ported as follows:

- `smartEdgeRemove`: corner-color sampling, `tolerance × 1.73`, border-seeded four-neighbour flood fill, then alpha set to zero.
- `removePickedColor`: Euclidean RGB distance against the selected color, using the same tolerance multiplier.
- `softenAlpha`: box blur of only the alpha channel. The Rust implementation uses a summed-area table, so larger radii remain fast.
- `thresholdAlpha`: alpha below the selected threshold becomes 0; all other alpha becomes 255.

## Current scope

This version intentionally leaves out AI background removal. It also does not import font files; fonts can be copied manually into the exported pack. The theme editor exposes the complete format as JSON so unknown or future Wonderdraft properties are not discarded.
