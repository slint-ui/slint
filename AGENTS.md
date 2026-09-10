# Repository Instructions

- Follow [the writing style guide](docs/internal/writing-style-guide.md) for new comments, documentation, Markdown, and commit messages.
  Don't reformat unrelated prose.
- Don't edit `CHANGELOG.md`; use a `ChangeLog:` commit trailer for noteworthy changes.
- The default branch is `master`.
  During review, add follow-up commits; squash or rebase when review is complete.
- For new visual or layout syntax, prefer CSS naming unless Slint's types or existing names require divergence; document the reason.

## Testing

- `examples/`, `demos/`, `tests/`, and `ui-libraries/material/` are separate Cargo workspaces.
  Use `--manifest-path <dir>/Cargo.toml` to target them.
- Run `cargo test` directly; no preceding `cargo build` is needed.
  Use release builds for performance measurements.
- Filter `.slint` cases with `SLINT_TEST_FILTER=<substring>` to avoid compiling every case.
  Example: `SLINT_TEST_FILTER=layout cargo test --manifest-path tests/Cargo.toml -p test-driver-interpreter`.
- In `tests/cases/*.slint`, declare the `test` property `out` or `in-out`; otherwise tests can pass without checking it.
- For `api/slint-sc`, every line, function, and region requires coverage, with no exclusions.
  Each requirement anchor `{#sls.…}` inside an `<SC>` block on an `SC: true` page needs a matching `//#sls.…` test in the same change.
  Run `scripts/slint_sc_test_suite.sh target/slint-sc-coverage` and `scripts/build_safety_manual_coverage.sh`.

## References

- [Development workflow](docs/development.md): setup, checks, and commit conventions.
- [Build prerequisites](docs/building.md).
- Read the relevant guide in `docs/development/` when working on its subsystem.
