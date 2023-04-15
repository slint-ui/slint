# Contributing

We warmly welcome contributions to the project. Let's discuss ideas or questions in [Github discussions](https://github.com/slint-ui/slint/discussions).
Please feel welcome to open github issues, pull requests or comment for example on [RFC tagged](https://github.com/slint-ui/slint/labels/rfc) issues.

## Coding Style

For the Rust portion of the code base, we enforce the coding style via rustfmt. The [`rustfmt.toml`](/rustfmt.toml) in the root direction is used to configure the style.

For the C++ portion of the code base, we enforce the coding style via `clang-format`, via the [`.clang-format`](/.clang-format) file in the root directory.

## Tooling

Pre-commit is a tool intended to help you in your local development, it is
_not_ enforced on the repository side!

`pre-commit` is set up and ready to be used in this repository. Simply install
the pre-commit python script using:

```bash
> pip install pre-commit
> pre-commit install
```

and the pre-commit git hooks will be active and do basic sanity checks on your
code whenever you commit something.

Not all parts of the code base are pre-commit clean, so use it as an early
warning system: See what it reports, fix any issue you introduced (as those
make sense to fix), and then feel free to skip pre-commit module still failing
using

`SKIP="module1,module2" git commit`

Typical modules to skip include: `cspell`, `check-json` and `prettier`.

## Testing

All changes submitted to the repository are automatically built and tested via Github Actions and the corresponding workflow defined in the [`rust.yaml`](/.github/workflows/rust.yaml) file.

## Contributor License Agreement

When opening a pull request, you will be asked to sign a [Contributor License Agreement (CLA)](https://cla-assistant.io/slint-ui/slint) which
will allow us to relicense your contribution.
Please contact us for any concerns.
