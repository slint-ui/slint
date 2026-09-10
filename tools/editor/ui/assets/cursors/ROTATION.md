# Rotation Cursor

The rotation SVG comes from Grida's `template_rotate` in
[templates.ts](https://github.com/gridaco/grida/blob/main/packages/grida-canvas-hud/cursors/templates.ts).
Grida distributes this source under Apache-2.0.
See [the license](../../../../../LICENSES/Apache-2.0.txt).
The angle placeholder is adapted for the editor's Rust callback.
The cursor retains the original 26×24 dimensions and (12, 12) hotspot.
Each orientation is rasterized before caching, matching the fixed-pixel canvas pointer.

The editor caches 360 orientations and selects the nearest degree.
Corner offsets follow Grida: top-left −45°, top-right 45°, bottom-right 135°, and bottom-left −135°.
