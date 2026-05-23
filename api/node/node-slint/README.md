# node-slint

A Node.js runner that statically embeds the Slint NAPI module and wires
winit's event loop directly into libuv. Plain `node` works fine with
Slint on Linux and macOS thanks to the `uv_prepare`-based integration in
the `slint-ui` package, but Windows falls back to 16 ms polling.
`node-slint` runs winit's `CustomApplicationHandler` instead, ticking
libuv from `about_to_wait` for 0-latency UI events on Windows too.

It is also a single self-contained binary — no separate `.node` addon
ships beside it.

## Build

`node-slint` needs libnode (Node's C++ embed library) to link against.
Node.js does not distribute one prebuilt; either install
`libnode-dev` (Debian) or build from source via the helper script:

```sh
./build-libnode.sh --prefix ./libnode-install
export NODE_DIR=$PWD/libnode-install
cargo build -p node-slint
```

`build.rs` searches for libnode in this order:

1. `NODE_DIR` env var → `<dir>/include/node/node.h` + `<dir>/lib/`.
2. `pkg-config libnode`.

There is no auto-download — libnode compiles for ~15 minutes the first
time so we ask for the path explicitly.

## Run

```sh
LD_LIBRARY_PATH=$NODE_DIR/lib ./target/debug/node-slint path/to/app.mjs
```

The `LD_LIBRARY_PATH` is only needed for a shared-library `libnode.so`;
a static build (`*.a` archives in `$NODE_DIR/lib/`) yields a fully
self-contained binary.

## How it works

```
node-slint script.js
│
├─ Rust main (src/main.rs)
│   │  builds argv, prepares bootstrap JS, calls into the C++ embed shim
│   ▼
├─ C++ shim (src/embed.cpp)
│   │  InitializeOncePerProcess → CommonEnvironmentSetup → V8 context
│   │  ── before_user_script(uv_loop_ptr) ──► Rust callback registers
│   │  │                                       winit CustomApplicationHandler
│   │  │                                       via slint-node::
│   │  │                                       register_winit_libuv_handler
│   │  ▼
│   │  LoadEnvironment(bootstrap_js) runs the bootstrap, which:
│   │   • patches Module._extensions['.node'] so any require of a
│   │     `rust-module.*` redirects to process.dlopen(mod,
│   │     process.execPath) — Node loads *this binary* as a NAPI addon
│   │     and finds the statically linked napi_register_module_v1.
│   │   • dynamic-imports the user script.
│   │
│   ▼ SpinEventLoop → Stop → TearDownOncePerProcess
```

Two consequences of the in-process load:

- `require('slint-ui')` resolves to the slint-node code linked into
  `node-slint`. There is no separate `.node` file searched on disk.
- The bootstrap is invisible to user code — no `__slint_*` globals to
  inspect or special init steps.

## Event loop integration

- **Linux/macOS:** the existing `uv_prepare` path in `slint-ui`
  (`api/node/rust/uv_event_loop.rs`) handles backend integration. It is
  backend-agnostic — works with winit, Qt, software, or linuxkms.
  Zero idle CPU, zero UI/JS latency.
- **Windows + winit:** the runner registers a winit
  `CustomApplicationHandler` (`api/node/rust/winit_libuv_handler.rs`)
  before the user script runs. Winit's `run_event_loop` drives, and
  `about_to_wait` ticks libuv with `uv_run(NOWAIT)`. The TypeScript
  `runEventLoop` calls into the blocking `runEventLoopBlocking` napi
  entry point when `hasWinitLibuvIntegration()` is true.
- **Windows + non-winit backend (e.g., Qt):** falls back to JS-side
  `setInterval(pump, 16)`, same as plain `node` on Windows.
