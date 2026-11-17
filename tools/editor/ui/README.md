<!-- Copyright © SixtyFPS GmbH <info@slint.dev> -->
<!-- SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

# Visual Editor Assets

Import assets through the entry point for their category.
Keep literal `@image-url` references in these definition files.

| Category | Entry Point | Contents |
| --- | --- | --- |
| Icons | [icons/index.slint](icons/index.slint) | Action, object, and status symbols; element and file icon mappings; configurable path glyphs |
| Cursors | [cursors/index.slint](cursors/index.slint) | Pointer images, anchor points, and cursor image callbacks |
| Images | [images/index.slint](images/index.slint) | Branding, backgrounds, and transparency textures |

Keep icon colors, sizes, rotations, and control interaction in the consuming component.
Fixed glyphs use SVG assets.
Glyphs with adjustable stroke widths are components in [icons/stroked-icons.slint](icons/stroked-icons.slint).
Canvas guides, handles, and other interactive geometry stay with their controls.

Cursor SVG templates live in `cursors/`.
[preview/ui.rs](../preview/ui.rs) substitutes rotation angles, rasterizes the cursor images, and caches them.
The corner-radius SVG and the larger default pointer remain available as source artwork and an alternate resolution.

Shared branding images use symbolic links to their original files.
Fonts remain in `assets/`.
Project image previews load document content at runtime.
Application icons and installer artwork remain in `../packaging/`.

Preserve asset attribution when moving or adding files.
Update [REUSE.toml](../../../REUSE.toml) and the [Lucide provenance notes](icons/lucide/README.md) where applicable.
