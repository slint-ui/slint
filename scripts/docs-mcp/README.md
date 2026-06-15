<!-- Copyright © SixtyFPS GmbH <info@slint.dev> -->
<!-- SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

# docs-mcp content pipeline

Feeds the **Slint documentation MCP server** (`docs-mcp.slint.dev`, a separate
private repo) with content. After the docs build, this collects each Starlight
site's **per-page Markdown** (one `.md` per page) and syncs it to the R2 bucket
that Cloudflare AI Search indexes.

- [`collect-and-upload.mjs`](collect-and-upload.mjs) — the collector/uploader.
- [`../../.github/workflows/docs-mcp-upload.yaml`](../../.github/workflows/docs-mcp-upload.yaml) — reusable CI workflow that runs it after `build_docs.yaml`.

## R2 layout produced

One object per docs page:

```
<version>/slint/<slug>.md     # docs/astro   (the .slint language docs)
<version>/cpp/<slug>.md        # docs/cpp
<version>/node/<slug>.md       # docs/nodejs  (JavaScript/TypeScript)
<version>/python/<slug>.md     # docs/python
<version>/safety/<slug>.md     # docs/safety

e.g. master/slint/guide/language/coding/properties.md
```

`<version>` is `master` for snapshots, or the Cargo workspace version on release
builds (mirrors `build_docs.yaml`). The keys match the public docs URL layout
`/<version>/docs/<segment>/<slug>.md`.

## What is indexed

The docs sites emit one clean `.md` per page via the per-page Markdown endpoint
(`docs/common/src/utils/markdown-endpoint.ts` + `[...slug].md.ts`). The collector
globs every `*.md` under each site's build output and preserves its slug, so AI
Search gets clean per-page chunks and the MCP server returns real slugs + exact
reads. Only the per-page `*.md` files are uploaded; the site-wide aggregate
`.txt` outputs are skipped.

## Upload

`rclone sync` over R2's S3 API — idempotent (only changed pages move; pages
removed from a version are pruned). It syncs only the `<version>/` prefix, so
other versions are left untouched.

## Local dry run

No credentials needed — stage locally and inspect the keys:

```sh
# --docs-root points at a tree of <segment>/…/<slug>.md files
node scripts/docs-mcp/collect-and-upload.mjs --docs-root artifact/docs --dry-run
```

## Provisioning (owner)

See the docs-mcp project README. CI needs these on **slint-ui/slint**:

- secret `CLOUDFLARE_ACCOUNT_ID`
- secrets `R2_ACCESS_KEY_ID` + `R2_SECRET_ACCESS_KEY` — an **R2 API token**
  (dashboard: R2 → Manage R2 API Tokens), **not** the Workers API token
- the bucket `slint-docs-mcp`, passed as the `r2-bucket` input by the caller
  (`nightly_snapshot.yaml`)

The workflow installs `rclone` on the runner.
