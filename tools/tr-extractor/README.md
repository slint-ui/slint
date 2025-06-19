<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

# slint-tr-extractor

A command-line tool for extracting translatable strings from `.slint` files and generating gettext-compatible translation files.

## Overview

`slint-tr-extractor` is part of the [Slint framework](https://slint.dev)'s internationalization (i18n) workflow.
It scans `.slint` files for strings marked with the `@tr()` macro and extracts them into gettext `.pot`
(Portable Object Template) files, which can then be translated into multiple languages.

See also the [Slint Translations Guide](https://docs.slint.dev/latest/docs/slint/guide/development/translations/) for more information.

## Prerequisites

* Install Rust by following the [Rust Getting Started Guide](https://www.rust-lang.org/learn/get-started).

## Installation

Install the tool using Cargo:

```bash
cargo install slint-tr-extractor
```

## Usage

```
Usage: slint-tr-extractor [OPTIONS] [path to .slint file(s)]...

Arguments:
  [path to .slint file(s)]...

Options:
  -d, --default-domain <DOMAIN>
  -o <file>                                Write output to specified file (instead of messages.po).
      --package-name <PACKAGE_NAME>        Set the package name in the header of the output
      --package-version <PACKAGE_VERSION>  Set the package version in the header of the output
  -j, --join-existing                      Join messages with existing file
  -h, --help                               Print help
  -V, --version                            Print version
```
