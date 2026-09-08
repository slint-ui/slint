# Authored regression fixtures

Fixtures describe small, authored examples, not captured third-party designs.
Conversion consumes versioned JSON; Figma API mocks belong only at the capture boundary.

- Root snapshot JSON and matching Slint files: `converter.unit.spec.ts` checks
  fixed frames, flex sizing, text overrides, nested instances, SVG and image fills.
- `errors/`: `capture.unit.spec.ts` checks malformed and unsupported selections.
- `source/`: normalization and transport tests use small source-format cases.
  Each case is named for the behavior it exercises; its matching unit test or
  `source.unit.spec.ts`, `asset-transport.unit.spec.ts`, `convert-capture.unit.spec.ts`
  consumes it. Component cases are authored galleries used by the component-generator
  tests. Large trees are generated in tests.
- `regressions/crlf.snapshot.json`: `converter.unit.spec.ts` checks newline handling.
- `authored/{square,asymmetric,odd-size}.png`: programmatically authored solid
  geometry with transparent padding, used by capture, binary transport, and PNG
  dimension/validation tests. These are inputs, not Figma visual references.
- `authored/raster/`: independent Figma references and production-capture JSON
  for the raster browser suite. Three authored 32×24 frames at 1×, 1.25× and 2×:
  translucent asymmetric vector geometry, an outside vector stroke, and a rounded alpha
  mask with a shadow. Created and exported in Figma Desktop on September 8, 2026.
  References use PNG, sRGB, contentsOnly and useAbsoluteBounds. Never regenerate
  these references from Slint output. Tests read the committed files offline;
  no original Figma document or account is required. See the maintenance recipe below.

## Maintaining raster references

Create a new blank Figma design in any account. Use transparent 32×24 frames with
no auto-layout and clipping disabled. The committed `*-1x.json` captures record
node dimensions, positions, paints, masks and effects; node IDs and page positions
are incidental and may change when recapturing.

- `asymmetric`: a translucent asymmetric vector at (4, 4), sized 24×12.
- `stroke`: a 24×16 vector at (4, 4), no fill, with a 4 px outside stroke
  using RGB (0.125, 0.5, 0.75).
- `mask-shadow`: a 12×12 alpha-mask rectangle at (4, 4), corner radius 4,
  followed by a 24×16 filled rectangle at (0, 0). Both use RGB (0.125, 0.5, 0.75).
  The frame has a black 40% drop shadow, offset (2, 2), blur 2 and spread 0.

The captures contain raster exports rather than editable vector paths, so they
are not a lossless Figma document backup. When replacing a vector case, author
geometry that exercises the same behavior and review it as a fixture change;
do not expect an independently drawn vector to reproduce the old PNG exactly.

For each changed frame, obtain fresh production-capture JSON with PNG capture
enabled at export scales 1, 1.25 and 2. Independently export the same frame from
Figma as PNG at each scale, with sRGB, `contentsOnly: true` and
`useAbsoluteBounds: true`. Replace both JSON and PNG in
`authored/raster/<name>-<scale>x.{json,png}` together. Run `pnpm test:browser`
and inspect the references before accepting the change. Never use the plugin's
Slint preview as the reference image.

Do not add archive corpora or fonts. Keep combined fixture data and test images
under 1 MB, with no more than 12 PNGs. Explain each fixture's behavioral purpose
and consumer here when adding one.

## Source cases and consumers

| Source JSON | Regression / consumer |
| --- | --- |
| `asymmetric-rounded-stroke.json` | `appearance-normalization.unit.spec.ts` |
| `baseline-alignment.json` | `appearance-normalization.unit.spec.ts` |
| `basic.json` | `recovery.unit.spec.ts`, `source.unit.spec.ts` |
| `component-intrinsic.json` | `source.unit.spec.ts` |
| `component-raster-icon.json` | `component-generator.unit.spec.ts` |
| `component-tokens.json` | `component-generator.unit.spec.ts` |
| `component-variants.json` | `component-generator.unit.spec.ts`, `images.unit.spec.ts` |
| `desktop-grid.json` | `layout-normalization.unit.spec.ts` |
| `empty-flex-spacers.json` | `source.unit.spec.ts` |
| `export-fonts.json` | `images.unit.spec.ts`, `export.unit.spec.ts` |
| `geometry-roundoff.json` | `layout-normalization.unit.spec.ts` |
| `group-positioning.json` | `layout-normalization.unit.spec.ts` |
| `image-crop.json` | `image-normalization.unit.spec.ts` |
| `image-size-unavailable.json` | `image-normalization.unit.spec.ts` |
| `inner-shadow.json` | `appearance-normalization.unit.spec.ts` |
| `multiple-failures.json` | `recovery.unit.spec.ts` |
| `native-overlap.json` | `source.unit.spec.ts` |
| `negative-gap.json` | `layout-normalization.unit.spec.ts`, `capture.unit.spec.ts` |
| `nested-container.json` | `recovery.unit.spec.ts` |
| `painted-bounds.json` | `image-normalization.unit.spec.ts` |
| `png-first.json` | `source.unit.spec.ts`, `asset-transport.unit.spec.ts` |
| `rectangular-mask.json` | `image-normalization.unit.spec.ts`, `source.unit.spec.ts` |
| `reverse-paint-order.json` | `layout-normalization.unit.spec.ts` |
| `scaled-instance.json` | `layout-normalization.unit.spec.ts` |
| `stroke-alignment.json` | `appearance-normalization.unit.spec.ts` |
| `translucent-styled-text.json` | `converter.unit.spec.ts` |
