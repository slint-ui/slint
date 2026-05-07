<!-- Copyright © SixtyFPS GmbH <info@slint.dev>
  SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

# Slint Remote Viewer

> **Experimental.**
> This tool is under active development.
> It'll eventually be merged into the [Slint Viewer](../viewer/).

The remote viewer runs on a target device (phone, embedded board, desktop, etc.)
and renders Slint components sent from the editor over WebSocket.
This lets you preview your UI on real hardware while you edit `.slint` files in VS Code.

## How It Works

1. Start the remote viewer on the target device.
2. It announces itself on the local network via mDNS.
3. The VS Code extension discovers it and connects automatically.
4. As you edit, the LSP server sends `.slint` source files to the viewer over WebSocket,
   which compiles and renders them in real time.

## Building

```sh
cargo build -p remote-viewer
```

## Running

```sh
cargo run -p remote-viewer
```

The viewer picks a random port and announces itself via mDNS by default.
Run `--help` to see available options:

```
--port <NUM>           Listen on a specific port.
--listen <IP:PORT>     Listen on a specific address and port.
--disable-mdns         Don't announce on the local network.
```

## Connecting from VS Code

The [Slint VS Code extension](../../editors/vscode/) can discover and connect
to remote viewers automatically.

1. Make sure the remote viewer and VS Code are on the same local network.
2. Start the remote viewer on the target device.
   If mDNS is enabled (the default), VS Code discovers it automatically.
3. Click the **Slint Remote Preview** status bar item (bottom right) to open the
   connection picker.
4. Select a discovered viewer from the list,
   or type an address manually (e.g. `192.168.1.42:3000` or `[::1]:3000`).
5. Open a `.slint` file and trigger a preview — the UI renders on the remote device.

Click the status bar item again to disconnect.

If the target isn't on the same network or mDNS isn't available,
start the viewer with `--disable-mdns` and connect manually using the address.

## Protocol

Messages between the editor and the remote viewer are serialized with
[postcard](https://crates.io/crates/postcard) over WebSocket.
The protocol is defined in `internal/preview-protocol/src/`.
