# Zoom and image loading update 0.2.2

- Added editable `+` and `-` shortcuts for zooming in and out in both the Crop and Sprite settings workspaces.
- Prevented the zoom slider's editable value field from restoring stale zoom values on later UI passes. Slider values remain visible as read-only labels, while dragging, wheel input, and arrow keys continue to edit them.
- Expanded zoom diagnostics with UI pass numbers, state checkpoints, focused-widget details, fit/display sizes, and reason-tagged writes from sliders, shortcuts, Fit actions, selection changes, and imports.
- Added content-signature format detection as a fallback when an image cannot be decoded using its filename extension, including WebP images mislabeled as `.png`.
- Bumped the application version shown in Settings to 0.2.2.
