# Slint SC

Slint SC is the safety-critical subset of [Slint](https://slint.dev),
designed to be certifiable under functional safety standards such as
ISO 26262 (automotive), IEC 61508 (industrial), and related norms.

It provides a drastically reduced feature set compared to the full
Slint framework, with an emphasis on auditability, bounded resource
usage, and deterministic behavior.

This crate (`slint-sc`) is the runtime library.
It's `no_std`, has zero external dependencies, and doesn't use
dynamic memory allocation.

## Documentation

The Slint SC Safety Manual, Qualification Plan, and usage guide live in
[`docs/safety/`](https://github.com/slint-ui/slint/tree/master/docs/safety)
in the Slint repository.

## Status

Slint SC is an early prototype.
The compiler mode exists but all language features are currently disabled
and will be unlocked incrementally.
