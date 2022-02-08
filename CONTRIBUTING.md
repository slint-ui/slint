# Contributing

We warmly welcome contributions to the project. Let's discuss ideas or questions in [Github discussions](https://github.com/slint-ui/slint/discussions).
Please feel welcome to open github issues, pull requests or comment for example on [RFC tagged](https://github.com/slint-ui/slint/labels/rfc) issues.

## Coding Style

For the Rust portion of the code base, we enforce the coding style via rustfmt. The [`rustfmt.toml`](/rustfmt.toml) in the root direction is used to configure the style.

For the C++ portion of the code base, we enforce the coding style via `clang-format`, via the [`.clang-format`](/.clang-format) file in the root directory.

## Tooling

`pre-commit` is set up and ready to be used in this repository. Simply install the pre-commit python script using:

```bash
> pip install pre-commit
> pre-commit install
```

and the pre-commit git hooks will be active and do basic sanity checks on your code whenever you commit something.

## Testing

All changes submitted to the repository are automatically built and tested via Github Actions and the corresponding workflow defined in the [`rust.yaml`](/.github/workflows/rust.yaml) file.

## Contributor License Agreement

When opening a pull request, you will be asked to sign a [Contributor License Agreement (CLA)](https://cla-assistant.io/sixtyfpsui/sixtyfps) which
will allow us to relicense your contribution.
Please contact us for any concerns.
