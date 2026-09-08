# AGENTS.md

## Project

Offline Figma-to-Slint plugin in the Slint monorepo. Target Slint 1.18+; no backwards compatibility.

## Sources of truth

- Resolve Slint from `SLINT_REPO`, defaulting to the separately prepared `.generated/slint-source`; require the unmodified revision in `runtime-pin.json`.
- Treat that checkout as read-only unless explicitly asked to edit it.
- Syntax asset checks use the inspector sources in the separately pinned runtime checkout.
- Use the root pnpm workspace, dependency catalog and lockfile. Never implicitly use the enclosing monorepo as runtime source.
- Never edit generated files in `.generated/` or `dist/`.

## Architecture

- Only `src/plugin/capture.ts` may read Figma node APIs.
- Capture once into versioned JSON; keep JSON-to-Slint conversion pure and deterministic.
- Test conversion with JSON fixtures, not repeated Figma API mocks.
- Use Slint `FlexboxLayout` for supported Figma auto-layout.
- Keep sandbox/UI communication typed and validated.

## Invariants

- Empty selection clears the preview; it is never an error.
- New selections immediately blank the preview and clear source/export; errors stop progress, keep output blank, and open Diagnostics.
- Stale revisions never replace newer work; unchanged source skips recompilation.
- Pin state is memory-only and resets on restart, page/file change, or root deletion.
- Never add artificial capture or rendering delays.
- Keep performance traces revision-correlated and totals reconciled.
- The bundle remains self-contained with `allowedDomains: ["none"]`.
- Copy the raw generated Slint, never highlighted HTML.
- Do not change unrelated UI or existing status/diagnostic colors.

## Working rules

- Check `git status` first and preserve unrelated changes.
- Inspect existing Slint implementations before designing new ones.
- Make small logical commits and stage only relevant files.
- Run `pnpm verify` before completion.
- Visually inspect every UI change in the browser and, when Figma-specific, in Figma Desktop.
