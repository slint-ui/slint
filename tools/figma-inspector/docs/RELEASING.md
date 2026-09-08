# Release checklist

This plugin is not commercially releasable until official Slint 1.18.0 exists.
The development pin uses unmodified upstream code. Never patch Slint or generated
bindings to satisfy the plugin. `pnpm prepare:slint` creates a separate checkout;
`SLINT_REPO` overrides are checked read-only and must match runtime-pin.json.

## Build and verification

1. Replace the development pin with the official v1.18.0 commit and set
   `development` to false. Verify the tag against the upstream repository.
2. Install locked dependencies with `pnpm install --frozen-lockfile` and install
   the WASM target and the pinned wasm-pack version. Use a trusted Rust toolchain
   on PATH; do not modify the source checkout to work around tool configuration.
3. Run `pnpm verify` from a clean plugin checkout. It builds both production and
   development artifacts. `pnpm dev` writes dist-dev; normal builds write dist.
4. Run the authored regression suite and inspect its independent Figma reference
   cases. Do not replace references with Slint output to make tests pass.
5. Complete the platform matrix below and inspect resource usage across repeated
   replacements. Record findings, exact versions, checksums and baseline results.
6. Build with `pnpm zip`. Packaging refuses a
   development Slint pin and dummy IDs. Package tests use .package-test instead.
7. Inspect notices, dependency inventory and provenance. Confirm font and fixture
   redistribution rights and that every shipped dependency's required notices are
   present. Plugin source follows the monorepo inspector's per-file licensing.

Release packaging checks the local tag and queries the official GitHub tag before
building. The remote lookup is noninteractive and has a 30-second timeout. If it
times out or fails, check Git/network access to github.com and retry `pnpm zip`.
A missing or mismatched tag requires correcting the official checkout/pin first.
Ordinary development builds do not perform this release-tag lookup; the shipped
plugin remains offline.

## Platform acceptance

Record pass/fail and OS/app version for Figma Desktop on macOS and Windows and
Figma web in Chrome and Edge. Include standard and high-density displays.
Verify selection immediately blanks output and starts progress; success shows
current output; errors stop progress and open Diagnostics; empty selection is not
an error. Exercise rapid selection changes, pinning, copying, ZIP export, resize,
image-heavy and large selections, and recovery after a failed selection.
Production must have no Performance UI. Development must retain usable timings.
A repeated replacement run must not exhibit sustained resource growth.

## Community publication

Use the SixtyFPS GmbH publisher and info@slint.dev support contact, following the
monorepo inspector's publishing convention. Import the verified production
manifest in Figma Desktop and submit that exact build through Manage Plugins.
Review listing details and current Figma publishing requirements. Publication is
a separate explicit action; build/verification commands never publish anything.

## Private distribution and updates

Distribute the verified ZIP, checksums and matching source revision. Users extract
it and choose Plugins > Development > Import plugin from manifest in Figma
Desktop. To update, replace the extracted files and restart the plugin. This is a
development-plugin installation path, not an automatic Community installation.

Keep prior verified source pins and archives. Roll back by rebuilding the prior
verified version and distributing it, or submitting it as a new Community update.
Do not mutate an already published archive.

## Privacy and support

All capture, conversion and preview processing stays in the plugin. The manifest
allows no network domains. There is no telemetry, account or payment integration.
Exported files contain the selected design; users control where they save/share
those files. Pin state is session-only. Support: info@slint.dev. Users may share a
minimal reproduction voluntarily; never require entire confidential design files.

## Compatibility claims

Slint 1.18+ is required. Diagnostics distinguish native support, approximations
and image fallbacks. Raster preview fidelity does not guarantee identical native
text export. Platform and authored regression checks are mandatory release evidence,
not claims inferred from unit tests or a successful process launch.

## Nightly snapshots

Run `pnpm build:slint` followed by `pnpm zip:nightly`.
This explicit mode permits the exact clean development runtime pin and creates `zip/figma-plugin.zip`.
It retains artifact validation, the assigned plugin identity, notices and checksums.
The packaged provenance records `channel: nightly` and the pinned runtime revision.
Never submit a nightly archive to the Community listing.
The default `pnpm zip` still verifies the official release tag and never falls back to nightly mode.
