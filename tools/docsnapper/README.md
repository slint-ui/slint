# Docsnapper

Automated screenshot generator for Slint documentation examples. Scans markdown files for Slint code snippets and renders them to images.

## How It Works

1. Walks through `.md` and `.mdx` files in a documentation folder
2. Finds `<CodeSnippetMD>` tags containing ` ```slint ` code blocks
3. Compiles each snippet using the Slint interpreter
4. Renders the UI headlessly with the Skia renderer
5. Saves screenshots to the paths specified in the tags

## Markdown Tag Format

```markdown
<CodeSnippetMD imagePath="/src/assets/example.png" imageWidth="200" imageHeight="100">
```slint
Button { text: "Click me"; }
```
</CodeSnippetMD>
```

### Tag Attributes

| Attribute | Description |
|-----------|-------------|
| `imagePath` | Output path for the screenshot (relative to doc file or absolute from project root if starting with `/`) |
| `imageWidth` | Width in pixels |
| `imageHeight` | Height in pixels |
| `scale` | Scale factor (default: 1.0) |
| `noScreenShot` | Skip this snippet (no value needed) |

If the code block doesn't contain a `component` declaration, the snippet is automatically wrapped in a window component.

## Usage

```sh
slint-docsnapper <docs-folder> [options]
```

### Options

| Option | Description |
|--------|-------------|
| `-I <path>` | Include path for `.slint` files or images |
| `-L <library=path>` | Library location (e.g., `-L std=/path/to/std`) |
| `--style <name>` | Style name (`native`, `fluent`, etc.) |
| `--overwrite` | Overwrite existing screenshot files |
| `--component <name>` | Specific component to render |

### Example

```sh
# Generate screenshots for all docs, overwriting existing ones
slint-docsnapper docs/astro/src/content/docs --style fluent --overwrite

# With include paths
slint-docsnapper docs/astro/src/content/docs -I ../examples -I ../ui
```

## Build

```sh
cargo build -p slint-docsnapper --release
```

## Notes

- Requires a display server or headless environment (uses Skia with Wayland support)
- Primarily used in CI to generate/update documentation screenshots
- The tool finds the project root by looking for `astro.config.mjs` or `astro.config.ts`
