# Zoom synchronization update 0.2.3

- Lowered the shared Crop and Sprite viewer minimum zoom from 0.1× to 0.01×.
- Fixed the right-panel logarithmic zoom slider rounding canvas wheel values back to a stale linear step during rendering.
- Isolated slider editing in a temporary value and committed it only after genuine slider interaction.
- Added regression coverage proving that rendering the slider preserves a canvas-written zoom value such as 0.766596.
- Retained high-precision zoom diagnostics for keyboard, slider, and canvas-wheel changes.
