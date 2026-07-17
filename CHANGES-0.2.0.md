# GUI interaction update 0.2.0

## Applied changes

- File drops use separate Source images and Extracted sprites targets; dropping elsewhere does not import the files into the wrong list.
- Added draw-mode symbols to sprite rows, photo-share markers to extracted crop rows, and persistent Dark/Light appearance with text-colored SVG symbols.
- Added wheel-event routing diagnostics and Settings build information. Canvas wheel zoom no longer depends on egui's response-hover flag and also accepts native zoom gestures.
- A full-window overlay is shown while files are hovering over the application.
- New imports begin without an automatic full-image crop.
- Selected crop regions have eight draggable edge and corner handles.
- Crop regions can still be moved by dragging inside them.
- Added **Copy crop** before **Delete crop**.
- Added **Extract all crops as sprites** for all crops without an existing sprite.
- Radius, Offset X, and Offset Y each have a slider and exact numeric drag field.
- The Wonderdraft pivot can be dragged directly on the sprite canvas.
- The collision-radius circle can be dragged to change the radius.
- Added Ctrl+Z for undo and Ctrl+Shift+Z for redo in the sprite workspace.
- Added zoom slider, mouse-wheel zoom, middle-button panning, and a **Fit** reset.
- Added separate erase and restore brush diameters, editable Ctrl+wheel bindings for active-tool diameter/tolerance changes, and wheel plus arrow-key control for every slider.
- Double-clicking a crop now extracts or opens its sprite. Alt-cropping a sprite can shrink or enlarge it against its source and keeps the Crop workspace region synchronized.
- Crop, pivot, and radius dragging now use move or direction-matched resize cursors immediately, including newly drawn crop regions.
- Mouse editing actions are restricted to the primary button so panning does not erase pixels.
- Added crop-resize unit tests.

## Behavioral note

A crop linked to an already extracted sprite cannot be moved or resized. Delete the extracted sprite first, adjust the crop, and extract it again. This prevents the crop geometry and existing sprite pixels from silently diverging.
