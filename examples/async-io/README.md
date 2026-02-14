<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Example demonstrating asynchronous API usage with Slint

This example demonstrates how to use asynchronous I/O, by means of issuing HTTP GET requests, within the Slint event loop.

The http GET requests fetch the closing prices of a few publicly traded stocks, and the result is show in the simple Slint UI.

# Rust

The Rust version is contained in [`main.rs`](./main.rs). It uses the `rewquest` crate to establish a network connection and issue the HTTP get requests, using Rusts `async` functions. These are run inside a future run with `slint::spawn_local()`, where we can await for the result of the network request and update the UI directly - as we're being run in the UI thread.

Run the Rust version via `cargo run -p stockticker`.

# Python

The Python version is contained in [`main.py`](./main.py). It uses the `aiohttp` library to establish a network connection and issue the HTTP get requests, using Python's `asyncio` library. The entire request is started from within the `refresh` function that's marked to be `async` and connected to the `refresh` callback in `stockticker.slint`. Slint detects that the callback is async in Python and runs it as a new task.

Run the Python version via `uv run main.py` in the `examples/async-io` directory.
