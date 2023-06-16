# Slint Updater

This program is a tool to upgrade `.slint` files from the [Slint Project](https://slint.dev) to the latest syntax.

The Slint Design Language evolves, with new features being added and old ones marked for deprecation. Use this tool to
automatically upgrade your `.slint` files to the latest syntax.

## Installation

The updater can be installed from crates.io:

```bash
cargo install slint-updater
```

### Usage:

```
slint-updater -i /path/to/my/app/ui/**/*.slint
```

