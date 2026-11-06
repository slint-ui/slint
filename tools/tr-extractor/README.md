
# slint-tr-extractor

A command-line tool for extracting translatable strings from `.slint` files and generating gettext-compatible translation files.

## Overview

`slint-tr-extractor` is part of the [Slint framework](https://slint.dev)'s internationalization (i18n) workflow.
It scans `.slint` files for strings marked with the `@tr()` macro and extracts them into gettext `.pot`
(Portable Object Template) files, which can then be translated into multiple languages.
Pass a [project file](https://docs.slint.dev/latest/docs/slint/guide/tooling/project-file/) or its directory to scan the main file and every file it imports.

See also the [Slint Translations Guide](https://docs.slint.dev/latest/docs/slint/guide/development/translations/) for more information.

## Prerequisites

* Install Rust by following the [Rust Getting Started Guide](https://www.rust-lang.org/learn/get-started).

## Installation

Install the tool using Cargo:

```bash
cargo install --locked slint-tr-extractor
```

## Usage

```
Usage: slint-tr-extractor [OPTIONS] <path to .slint file(s) or project>...

Arguments:
  <path to .slint file(s) or project>...
          Specify .slint files, project directories, or slint-project.json files to extract from.
          A project directory or slint-project.json file also selects every file that its main file imports

Options:
  -d, --default-domain <DOMAIN>
  -o <file>                                Write output to specified file (instead of messages.po).
      --package-name <PACKAGE_NAME>        Set the package name in the header of the output
      --package-version <PACKAGE_VERSION>  Set the package version in the header of the output
  -j, --join-existing                      Join messages with existing file
  -h, --help                               Print help
  -V, --version                            Print version
```
