#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/**
 * Collect the per-page Markdown the docs build emits (one `.md` per page, via
 * docs/common/src/utils/markdown-endpoint.ts) and upload it to the R2 bucket
 * that Cloudflare AI Search indexes for the docs MCP server (the separate
 * `docs-mcp` project / private repo).
 *
 * R2 layout produced here (one object per docs page):
 *
 *     <version>/<segment>/<slug>.md
 *     e.g. master/slint/guide/language/coding/properties.md
 *
 * where <segment> is one of: slint, cpp, node, python, safety, matching the
 * public docs URL layout `/<version>/docs/<segment>/<slug>.md`.
 *
 * Usage:
 *     node scripts/docs-mcp/collect-and-upload.mjs \
 *         --docs-root artifact/docs \
 *         [--version master | --release] \
 *         [--bucket slint-docs-mcp] \
 *         [--staging r2-staging] \
 *         [--dry-run]
 *
 * Environment (used as fallbacks; see .github/workflows/docs-mcp-upload.yaml):
 *     DOCS_VERSION, DOCS_MCP_R2_BUCKET, RELEASE_INPUT, DRY_RUN
 *
 * Upload uses `rclone sync` against R2's S3 API (idempotent; only changed files
 * move, pages deleted from a version are pruned). It reads R2 credentials from
 * the environment — nothing is embedded here:
 *     CLOUDFLARE_ACCOUNT_ID, R2_ACCESS_KEY_ID, R2_SECRET_ACCESS_KEY
 * (create an R2 API token in the dashboard: R2 -> Manage R2 API Tokens.)
 */

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, copyFileSync } from "node:fs";
import { dirname, join, relative, sep } from "node:path";
import { parseArgs } from "node:util";

// docs build output subdir -> AI Search corpus segment.
// (The `rust` subdir is rustdoc HTML with no per-page .md, so it is omitted.)
const SEGMENTS = ["slint", "cpp", "node", "python", "safety"];

function main() {
  const { values } = parseArgs({
    options: {
      "docs-root": { type: "string", default: process.env.DOCS_ROOT ?? "artifact/docs" },
      version: { type: "string", default: process.env.DOCS_VERSION },
      release: { type: "boolean", default: process.env.RELEASE_INPUT === "true" },
      bucket: { type: "string", default: process.env.DOCS_MCP_R2_BUCKET },
      staging: { type: "string", default: "r2-staging" },
      "dry-run": { type: "boolean", default: process.env.DRY_RUN === "1" },
    },
  });

  const docsRoot = values["docs-root"];
  const version = resolveVersion(values.version, values.release);
  const staging = values.staging;
  const dryRun = values["dry-run"];
  const bucket = values.bucket;

  if (!dryRun && !bucket) {
    fail("No R2 bucket given. Pass --bucket or set DOCS_MCP_R2_BUCKET (or use --dry-run).");
  }

  console.log(`docs-mcp: collecting from "${docsRoot}" as version "${version}"`);

  // Start from a clean staging tree so `rclone sync` prunes pages that were
  // removed from this version on a re-index.
  rmSync(staging, { recursive: true, force: true });

  let total = 0;
  for (const segment of SEGMENTS) {
    const siteDir = join(docsRoot, segment);
    if (!existsSync(siteDir)) {
      console.warn(`  ! ${segment}: site dir "${siteDir}" not found — skipping`);
      continue;
    }
    const mdFiles = findMarkdownFiles(siteDir);
    if (mdFiles.length === 0) {
      console.warn(`  ! ${segment}: no .md pages under "${siteDir}" — did the per-page .md route build?`);
      continue;
    }
    // The deploy serves files under `/<version>/docs/<segment>/`. Depending on
    // whether the build nests dist under the base path, the .md path may or may
    // not contain a `docs/<segment>/` prefix; take everything after the last one
    // (or the whole site-relative path if absent) as the page's `<slug>.md`.
    const marker = `docs/${segment}/`;
    for (const file of mdFiles) {
      const rel = relative(siteDir, file).split(sep).join("/");
      const idx = rel.lastIndexOf(marker);
      const slugMd = idx >= 0 ? rel.slice(idx + marker.length) : rel;
      const key = `${version}/${segment}/${slugMd}`;
      const dest = join(staging, key);
      mkdirSync(dirname(dest), { recursive: true });
      copyFileSync(file, dest);
    }
    console.log(`  ✓ ${segment}: ${mdFiles.length} page(s)`);
    total += mdFiles.length;
  }

  if (total === 0) {
    fail("No .md pages were collected. Did the docs build emit the per-page markdown endpoints?");
  }

  if (dryRun) {
    console.log(`docs-mcp: dry run — staged ${total} page(s) under "${staging}/${version}/", not uploading.`);
    return;
  }

  console.log(`docs-mcp: syncing ${total} page(s) to R2 bucket "${bucket}" (prefix "${version}/")…`);
  rcloneSync(join(staging, version), `${bucket}/${version}`);
  console.log("docs-mcp: upload complete.");
}

/** Release builds publish under the version number; otherwise under "master". */
function resolveVersion(explicit, release) {
  if (explicit) return sanitize(explicit);
  const cargoVersion = readWorkspaceVersion();
  return release ? sanitize(cargoVersion) : "master";
}

function sanitize(v) {
  const cleaned = String(v).trim().replace(/[^\w.\-]/g, "");
  if (!cleaned) fail(`Invalid version: "${v}"`);
  return cleaned;
}

/** Read `version` from the `[workspace.package]` table of the root Cargo.toml. */
function readWorkspaceVersion() {
  const toml = readFileSync("Cargo.toml", "utf8");
  let inSection = false;
  for (const line of toml.split("\n")) {
    const trimmed = line.trim();
    if (trimmed.startsWith("[")) inSection = trimmed === "[workspace.package]";
    else if (inSection) {
      const m = /^version\s*=\s*"([^"]+)"/.exec(trimmed);
      if (m) return m[1];
    }
  }
  fail("Could not read version from [workspace.package] in Cargo.toml.");
}

/** Recursively collect every `.md` file under `root`. */
function findMarkdownFiles(root) {
  const out = [];
  const stack = [root];
  while (stack.length > 0) {
    const dir = stack.pop();
    let entries;
    try {
      entries = readdirSync(dir, { withFileTypes: true });
    } catch {
      continue;
    }
    for (const entry of entries) {
      const p = join(dir, entry.name);
      if (entry.isDirectory()) stack.push(p);
      else if (entry.isFile() && entry.name.endsWith(".md")) out.push(p);
    }
  }
  return out;
}

/** Sync a local dir to `<bucket>/<prefix>` via rclone over R2's S3 API. */
function rcloneSync(localDir, dest) {
  const accountId = process.env.CLOUDFLARE_ACCOUNT_ID ?? process.env.R2_ACCOUNT_ID;
  const accessKey = process.env.R2_ACCESS_KEY_ID;
  const secretKey = process.env.R2_SECRET_ACCESS_KEY;
  if (!accountId || !accessKey || !secretKey) {
    fail(
      "Missing R2 credentials. Set CLOUDFLARE_ACCOUNT_ID, R2_ACCESS_KEY_ID and " +
        "R2_SECRET_ACCESS_KEY (R2 -> Manage R2 API Tokens), or use --dry-run.",
    );
  }
  const env = {
    ...process.env,
    RCLONE_S3_PROVIDER: "Cloudflare",
    RCLONE_S3_ACCESS_KEY_ID: accessKey,
    RCLONE_S3_SECRET_ACCESS_KEY: secretKey,
    RCLONE_S3_ENDPOINT: `https://${accountId}.r2.cloudflarestorage.com`,
    RCLONE_S3_NO_CHECK_BUCKET: "true",
  };
  try {
    execFileSync(
      "rclone",
      ["sync", localDir, `:s3:${dest}`, "--checksum", "--transfers", "16", "--stats-one-line"],
      { env, stdio: "inherit" },
    );
  } catch (err) {
    fail(`rclone sync failed (is rclone installed?): ${err.message}`);
  }
}

function fail(message) {
  console.error(`docs-mcp: ERROR: ${message}`);
  process.exit(1);
}

main();
