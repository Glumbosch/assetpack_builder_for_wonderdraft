# GUI interaction update 0.2.0

## Applied changes

- File drops are consumed from egui's window-global dropped-file queue, so images can be dropped anywhere in the application window.
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
- Mouse editing actions are restricted to the primary button so panning does not erase pixels.
- Added crop-resize unit tests.

## Behavioral note

A crop linked to an already extracted sprite cannot be moved or resized. Delete the extracted sprite first, adjust the crop, and extract it again. This prevents the crop geometry and existing sprite pixels from silently diverging.
