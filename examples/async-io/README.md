<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Example demonstrating asynchronous API usage with Slint

This example demonstrates how to use asynchronous I/O, by means of issuing HTTP GET requests, within the Slint event loop.

The http GET requests fetch the closing prices of a few publicly traded stocks, and the result is show in the simple Slint UI.

# Rust

The Rust version is contained in [`main.rs`](./main.rs). It uses the `request` crate to establish a network connection and issue the HTTP get requests, using Rusts `async` functions. These are run inside a future run with `slint::spawn_local()`, where we can await for the result of the network request and update the UI directly - as we're being run in the UI thread.

Run the Rust version via `cargo run --manifest-path examples/async-io/Cargo.toml` from the root of the repository.

# Python

The Python version is contained in [`main.py`](./main.py). It uses the `aiohttp` library to establish a network connection and issue the HTTP get requests, using Python's `asyncio` library. The entire request is started from within the `refresh` function that's marked to be `async` and connected to the `refresh` callback in `stockticker.slint`. Slint detects that the callback is async in Python and runs it as a new task.

Run the Python version via `uv run main.py` in the `examples/async-io` directory.

# C#

The C# version is contained in [`dotnet/Program.cs`](./dotnet/Program.cs). It uses `HttpClient` to issue the HTTP GET requests, from an `async` handler of the `refresh` callback. While the event loop runs, each `await` continues on the UI thread, so the handler updates the model directly.

Run the C# version via `dotnet run --project examples/async-io/dotnet` from the root of the repository.
