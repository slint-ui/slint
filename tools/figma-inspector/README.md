# Figma to Slint Property Inspector

A Figma plugin that displays Slint code snippets in Figma's Dev mode inspector. When you select a design element, the plugin shows the equivalent Slint markup instead of CSS properties.

## Features

- Converts Figma elements to Slint component snippets
- Supports Figma variables (references them as Slint property paths)
- Works in Figma Desktop and VS Code extension

### Supported Elements

| Figma Node Type | Slint Element |
|-----------------|---------------|
| Frame, Rectangle, Group | `Rectangle { }` |
| Component, Instance | `Rectangle { }` |
| Text | `Text { }` |
| Vector | `Path { }` |

### Converted Properties

**Layout:** `x`, `y`, `width`, `height`

**Appearance:**
- `background` / `fill` (solid colors, linear and radial gradients)
- `opacity`
- `border-radius` (uniform or per-corner)
- `border-width`, `border-color`

**Text:**
- `text`, `color`
- `font-family`, `font-size`, `font-weight`
- `horizontal-alignment`

**Path:** `commands` (extracted from SVG), `stroke`, `stroke-width`

### Figma Variables

When enabled, the plugin references Figma variables as Slint property paths:

```slint
// Without variables
background: #3b82f6;

// With variables enabled
background: Colors.current.primary;
```

## Installation

### From Figma Community (Recommended)

Install directly from [Figma Community](https://www.figma.com/community/plugin/1474418299182276871/figma-to-slint) or search for "Figma To Slint" in the Figma plugin browser.

### From Nightly Build

1. Download [figma-plugin.zip](https://github.com/slint-ui/slint/releases/download/nightly/figma-plugin.zip)
2. Extract the archive
3. In Figma: right-click → `Plugins` → `Development` → `Import Plugin From Manifest...`
4. Select the `manifest.json` from the extracted folder

### Requirements

- Figma Desktop App or Figma VS Code extension
- Figma subscription with Dev mode access (Team Professional or higher)

## Development

### Prerequisites

- [Node.js](https://nodejs.org/) v20 or newer
- [pnpm](https://pnpm.io/)
- Figma Desktop App or VS Code extension

### Build

```sh
pnpm install    # Install dependencies (first time only)
pnpm build      # Build the plugin
```

Import the plugin in Figma: right-click → `Plugins` → `Development` → `Import Plugin From Manifest...` → select `dist/manifest.json`

### Development Mode

```sh
pnpm dev
```

Enable hot reload in Figma: `Plugins` → `Development` → `Hot Reload Plugin`

Changes are automatically recompiled and reloaded.

### Testing

Unit tests use Vitest with exported Figma JSON fixtures.

```sh
pnpm test       # Run tests in watch mode
```

#### Updating Test Fixtures

1. Generate a Figma access token:
   - Figma home → click username → `Settings` → `Security` → `Generate new token`

2. Get the file ID from the Figma URL:
   ```
   https://www.figma.com/design/njC6jSUbrYpqLRJ2dyV6NT/...
                               └─────────────────────┘
                                      File ID
   ```

3. Download the file as JSON:
   ```sh
   curl -H 'X-Figma-Token: <TOKEN>' \
        'https://api.figma.com/v1/files/<FILE_ID>' \
        -o tests/figma_output.json
   ```
