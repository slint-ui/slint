# Repository Instructions

- Follow [the writing style guide](docs/internal/writing-style-guide.md) for new comments, documentation, Markdown, and commit messages.
- The default branch is `master`.
  During review, add follow-up commits; squash or rebase when review is complete.
- For new API, look at how other toolkits solve the problem.
  For `.slint` syntax, follow CSS unless it's overly complicated or a Slint concept fits better.

## Testing

- `examples/`, `demos/`, `tests/`, and `ui-libraries/material/` are separate Cargo workspaces.
  Use `--manifest-path <dir>/Cargo.toml` to target them.
- Run `cargo test` directly; no preceding `cargo build` is needed.
  Use release builds for performance measurements.
- Filter `.slint` cases with `SLINT_TEST_FILTER=<substring>` to avoid compiling every case.
  Example: `SLINT_TEST_FILTER=layout cargo test --manifest-path tests/Cargo.toml -p test-driver-interpreter`.
- CI runs `test-driver-rust` with `--all-features`, which enables `build-time` mode.
  To reproduce a CI-only failure or a lint in generated code, add `--features build-time`.
  The live-preview job also sets `SLINT_LIVE_PREVIEW=1` and enables `slint/live-preview`.
- Where it fits, prefer `tests/cases`, which checks every code generator, over a Rust `#[test]`.
  Extend an existing case when it makes sense, since each one adds compile time.
  A case can hold several `rust`, `cpp`, `js`, and `python` blocks.
  Declare the `test` property `out`, or the drivers won't check it.
- Keep screenshot tests small in pixels, and run `oxipng -o max --strip all` on new reference PNGs.
- For `api/slint-sc`, every line, function, code region, and branch outcome requires coverage, with no exclusions.
  Each requirement anchor `{#sls.…}` inside an `<SC>` block on an `SC: true` page needs a matching `//#sls.…` test in the same change.
  Run `scripts/slint_sc_test_suite.sh target/slint-sc-coverage` and `scripts/build_safety_manual_coverage.sh`.

## References

- [Development workflow](docs/development.md): setup, checks, and commit conventions.
- [Build prerequisites](docs/building.md).
- [Testing](docs/testing.md): test drivers, syntax tests, screenshots.
- [Public API](docs/development/public-api.md).
- Read the relevant guide in `docs/development/` when working on its subsystem.
