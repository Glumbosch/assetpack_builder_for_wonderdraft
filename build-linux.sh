#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

echo "Building Linux X11/XWayland version..."
cargo clean
cargo test
cargo build --release

mkdir -p dist
cp target/release/wonderdraft-asset-studio \
   dist/wonderdraft-asset-studio

echo
echo "Built:"
echo "  dist/wonderdraft-asset-studio"
echo
echo "This Linux build uses X11/XWayland so file drag-and-drop works."
