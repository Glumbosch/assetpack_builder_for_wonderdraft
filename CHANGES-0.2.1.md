# GUI interaction update 0.2.1

- Fixed source-image and extracted-sprite wheel zoom at zoom factors below 1×. Zoom remains anchored to the pointer anywhere inside the viewer.
- Disabled viewer zoom while the Settings window is open.
- Prevented a hovered slider from scrolling its surrounding settings panel; wheel and arrow-key slider control remain available.
- Replaced the sprite transparency tool text buttons with theme-aware icons and explanatory tooltips.
- Added the **Erase picked color** brush, which only removes pixels matching the picked color within the configured tolerance.
- Made the picked-color swatch open a full visual color picker with numeric RGB entry.
- Added a checkerboard behind transparent parts of source images.
- Added contextual tooltips to the primary controls throughout the app.
- Added build version and executable details to Settings and retained wheel-routing diagnostics in terminal output.
