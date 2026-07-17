# GUI interaction update 0.2.1

- Fixed source-image and extracted-sprite wheel zoom at zoom factors below 1×. Below 1×, changing zoom no longer pans or moves the image.
- Reserved Ctrl+wheel in the sprite viewer for the current brush diameter or picked-color tolerance, without applying zoom.
- Made the crop viewer a source-image drop target and its empty state a clickable import target. The selected-sprite viewer accepts complete images as directly imported sprites.
- Disabled viewer zoom while the Settings window is open.
- Prevented a hovered slider from scrolling its surrounding settings panel; wheel and arrow-key slider control remain available.
- Replaced the sprite transparency tool text buttons with theme-aware icons and explanatory tooltips.
- Added the **Erase picked color** brush, which only removes pixels matching the picked color within the configured tolerance.
- Made the picked-color swatch open a full visual color picker with numeric RGB entry.
- Added a checkerboard behind transparent parts of source images.
- Added contextual tooltips to the primary controls throughout the app.
- Added build version and executable details to Settings and retained wheel-routing diagnostics in terminal output.
