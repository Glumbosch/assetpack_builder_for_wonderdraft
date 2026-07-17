#!/usr/bin/env sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
binary=${WONDERDRAFT_ASSET_STUDIO_BINARY:-"$project_dir/wonderdraft-asset-studio"}

if [ ! -x "$binary" ]; then
    binary="$project_dir/dist/wonderdraft-asset-studio"
fi

if [ ! -x "$binary" ]; then
    binary="$project_dir/target/release/wonderdraft-asset-studio"
fi

if [ ! -x "$binary" ]; then
    echo "Release executable not found: $binary" >&2
    echo "Build it first with: cargo build --release, or run this script beside an extracted Linux release binary." >&2
    exit 1
fi

data_home=${XDG_DATA_HOME:-"$HOME/.local/share"}
app_dir="$data_home/wonderdraft-asset-studio"
applications_dir="$data_home/applications"
pixmaps_dir="$data_home/pixmaps"
scalable_icons_dir="$data_home/icons/hicolor/scalable/apps"
desktop_file="$applications_dir/wonderdraft-asset-studio.desktop"

install -d "$app_dir/bin" "$applications_dir" "$pixmaps_dir" "$scalable_icons_dir"
install -m 755 "$binary" "$app_dir/bin/wonderdraft-asset-studio"
install -m 644 "$project_dir/assetpack_builder_for_wonderdraft.png" \
    "$pixmaps_dir/wonderdraft-asset-studio.png"
install -m 644 "$project_dir/assetpack_builder_for_wonderdraft.svg" \
    "$scalable_icons_dir/wonderdraft-asset-studio.svg"

escaped_exec=$(printf '%s' "$app_dir/bin/wonderdraft-asset-studio" | sed 's/[&|]/\\&/g')
escaped_path=$(printf '%s' "$app_dir" | sed 's/[&|]/\\&/g')
sed \
    -e "s|^Exec=.*|Exec=\"$escaped_exec\"|" \
    -e "s|^Path=.*|Path=$escaped_path|" \
    -e 's|^Icon=.*|Icon=wonderdraft-asset-studio|' \
    "$project_dir/wonderdraft-asset-studio.desktop" >"$desktop_file"
chmod 644 "$desktop_file"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$applications_dir" >/dev/null 2>&1 || true
fi

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$data_home/icons/hicolor" >/dev/null 2>&1 || true
fi

echo "Installed Wonderdraft Asset Studio launcher:"
echo "  $desktop_file"
