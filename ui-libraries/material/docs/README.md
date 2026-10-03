<!-- Copyright © SixtyFPS GmbH <info@slint.dev> -->
<!-- SPDX-License-Identifier: MIT -->

# Slint Material Component Documentation

The site uses Astro 7.3 and Starlight, with the shared Slint documentation header, footer, styles, and content components.
The homepage uses a plain Astro page and local CSS to preserve its original design.
Site configuration is in `astro.config.ts` and the documentation collection is in `src/content.config.ts`.

Documentation pages live in `src/content/docs/`.
Images in `src/assets/` can be referenced with relative Markdown links and are optimized by Astro.
Static files such as favicons live in `public/`.
The APK, WebAssembly gallery, and ZIP downloads are supplied by the publishing workflow.

## Build

Install Rust, Node.js, and pnpm.
Run these commands from the repository root:

```sh
pnpm install --frozen-lockfile
cargo run -p slint-doc-generator -- screenshots --overwrite -Lmaterial=$PWD/ui-libraries/material/src/material.slint ui-libraries/material/docs/src/content/docs
cd ui-libraries/material/docs
pnpm run build
```

The screenshot generator writes images into `src/assets/generated/`.
The site build checks Astro types and internal documentation links, and writes the site to `dist/`.
Starlight supplies MDX support, search, and the sitemap.
Astro's default Sharp image service optimizes images.

## Develop and test

Run these commands from `ui-libraries/material/docs/`:

| Command | Purpose |
| --- | --- |
| `pnpm dev` | Start the development server at `http://localhost:4321/` |
| `pnpm build` | Check and build the site |
| `pnpm preview` | Serve the production build locally |
| `pnpm type-check` | Check Astro types |
| `pnpm lint` | Lint source files |
| `pnpm format:fix` | Format source files |
| `pnpm spellcheck` | Check documentation spelling |
| `pnpm test` | Test the built site in Chromium, Firefox, and WebKit |

Install Playwright's browsers with `pnpm exec playwright install chromium firefox webkit` before running the browser tests.
Build the site before running `pnpm test`.

## Deploy under a path prefix

The production site defaults to `https://material.slint.dev/`.
Set `MATERIAL_DOCS_BASE_PATH` when building, previewing, or testing a deployment under a path prefix.
Use a leading and trailing slash:

```sh
MATERIAL_DOCS_BASE_PATH=/material/ pnpm build
MATERIAL_DOCS_BASE_PATH=/material/ pnpm preview
MATERIAL_DOCS_BASE_PATH=/material/ pnpm test
```

The preview is available at `http://localhost:4321/material/`.
Homepage links, favicons, image metadata, robots, and sitemap URLs use the configured prefix.
