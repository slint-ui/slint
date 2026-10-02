# Springboard Architecture

Status: Design in progress.
The current direction is a protocol router with reusable embedded and standalone UI.

## Purpose

Springboard directs a preview protocol session to a selected preview target.
Targets include a local or remote Slint Viewer, the visual editor's built-in run preview,
and potentially a Slint application running in `LIVE_PREVIEW` mode.

The current scope is selecting and connecting to remote live preview sessions, with a local run preview as an alternative endpoint.
Only one endpoint receives the session at a time.
Selecting a remote viewer immediately closes the local run preview and starts connecting to that viewer.

The debugger remains a future requirement.
This design should allow adding it without deciding its protocol, runtime data model, or UI architecture yet.

## Requirements

The following requirements are copied verbatim.

### Springboard & Debugger

- Establishes and manages the selection of and connection to Slint Viewer instances (local or remote)
  - Future: Might support managing multiple connected viewers
- Allows switching to a different viewer even after initial connection
- Supports login to slint account for direct/easier connection
- When connected: Shows the **Debugger&#x20;**&#x76;iew (Gammaray for Slint)
  - *Read-only* introspection into the connected Viewer
  - Future: Can connect to a normal running slint app
  - Should read the runtime debug info out of the connected slint app
  - Features:
    - Inspect (puts Viewer into selection mode)
    - Console Output
    - Outline of the running item tree
    - Property Inspector for the properties as they are&#x20;

## Existing Architecture

- [`EditorSession`](editor_session.rs) owns edited documents, preview configuration, and a list of `PreviewConnection` entries.
  It broadcasts document changes and can send the current state to a particular connection.
- [`LspToPreviews`](lsp_to_previews.rs) routes a connection's messages to a selected local preview and an optional remote viewer.
  Remote viewing currently mirrors messages sent to the local preview.
- The [visual editor](../../tools/editor/main.rs) has separate connections for its editing preview and its run preview.
  The run preview launches a child process through [`ChildProcessLspToPreview`](child_process.rs).
- [`PreviewSession`](../live-preview/preview_sessions.rs) handles received files, dependencies, configuration, and compilation on the preview side.
- The [remote connector](../../tools/lsp/connector/remote.rs) currently lives in the LSP and handles remote connections, pairing, and reconnection.
- The [preview protocol](../live-preview/protocol/) carries document synchronization, preview requests, source highlighting, console messages, and connection control.
  It does not currently define runtime item-tree or property-value queries.

The existing protocol's `PreviewTarget` describes the local transport choice: child process, embedded WASM, or dummy.
Springboard needs a target concept that can also identify individual viewers and, eventually, running applications.

## Architecture Direction

### Protocol Router

Springboard implements `LspToPreview` and presents itself as a preview endpoint to the LSP or editor session.
The sender uses the existing protocol without knowing whether Springboard consumes a message or forwards it to another endpoint.
Springboard forwards preview messages to the selected endpoint and carries replies back through the existing preview-to-editor channel.

```text
LSP / EditorSession
        |
        | Existing preview protocol, in both directions
        v
Springboard router <----> Springboard UI
        |
        | One active endpoint
        v
Local run preview OR remote Viewer
```

Document ownership remains in `EditorSession`.
Springboard owns endpoint selection, connection management, and the lifetime of any local preview it starts.
The selected endpoint owns compilation and the running preview.

In the visual editor, Springboard replaces the current run-preview connection.
The editing preview remains a separate connection.
Springboard can start its own local preview when needed and switch the session to a remote viewer.
It does not mirror the session to both endpoints.

The routing role in `LspToPreviews` provides a starting point, but its local-plus-remote behavior differs from this design.
Remote connection management currently tied to the LSP must become reusable by Springboard.
The LSP should not need remote-target-specific routing or connection handling.

### Reusable Slint Library

Springboard lives in a submodule of `internal/editor-preview`, which is already shared by the LSP and visual editor.
It can move into a separate crate later if needed.

Springboard provides a Slint library with an exported global API and reusable UI components.
The global exposes lifecycle and connection state, endpoint selection, and connection prompts.
Rust code connects this UI API to the router and connection management.
Exact API names and types remain to be designed.

The reusable components must fit inside an existing Slint window, including a visual-editor tab or panel.
They must not require a standalone main window.
The host connects the global in its component to the corresponding Springboard router.

Keep connection logic independent of the window that presents it.
The shared implementation must work within an existing Slint application's backend and event loop.
The standalone host owns application startup and its main window.

### Embedded and Standalone Modes

Both modes use the same router, global API, and UI components.

| Mode | Host Responsibilities | Springboard Responsibilities |
| --- | --- | --- |
| Embedded | The visual editor places Springboard components in its window and connects its run-preview session. | Manage the selected endpoint and expose connection state and actions through the shared UI API. |
| Standalone | A host creates a main window around the shared components and connects the incoming preview protocol session. | Provide the same routing and connection behavior as embedded mode. |

The standalone host can add controls or behavior around the shared components.
The LSP launches Springboard as a subprocess through `ChildProcessLspToPreview`, eventually replacing its current live preview.
Springboard's standalone host receives the protocol session and presents its own main window.

The process boundaries differ between the two modes:

- In the visual editor, Springboard runs inside the editor process.
  It starts a subprocess only for its local run preview.
- In the LSP case, Springboard itself runs as the preview subprocess.
  Its local run preview could use another subprocess or a separate window in the Springboard process.
  This choice remains open and should not affect the shared Slint global API.

### Session and Endpoint Lifetimes

Springboard starts only when the user chooses Run in the visual editor or Show Preview in the LSP.
Embedding its UI does not start discovery, launch a preview, or connect to a viewer.
On start, restore the last selected endpoint through the existing settings storage mechanism and automatically launch or connect to it.
If there is no saved endpoint, or that endpoint is no longer available, show the target list and wait for selection.
Save endpoint selection so it can be restored on the next start.

Stopping Springboard closes its owned local preview, disconnects any remote viewer, and stops discovery and reconnect attempts.
There is no separate disconnect action that leaves Springboard running.
The saved endpoint selection survives stopping.

The upstream protocol session stays connected when the selected endpoint changes.
Springboard's UI also stays available when the local run preview closes or a remote viewer disconnects.
Closing an owned local preview and disconnecting an independently running remote viewer are separate operations.

Routing therefore includes handling connection and lifecycle messages, beyond forwarding preview content.
For example, an `Exited` message from the local preview being replaced must not report that Springboard itself has exited.
Replies from a replaced endpoint must not reach the current upstream session.
Pairing and connection-state handling belong inside Springboard and its transport support.

Springboard tries to maintain the selected remote connection and reconnects when it is lost.
Connection failure does not select another endpoint or start a local preview.
Selecting another target cancels connection attempts and reconnects for the previous target.
The exact behavior of upstream `Quit` and closing the standalone window remains to be specified.

### Switching and Synchronization

A newly connected preview endpoint automatically requests the current state through the existing `RequestState` exchange.
Springboard forwards that request upstream and routes the response messages to the selected endpoint.
Springboard does not originate the state request.
The editor session answers through its normal protocol behavior without knowing that a target switch occurred.

This avoids requiring Springboard to own another document model.
Message ordering during connection setup and switching still needs to be specified so the endpoint receives a consistent session state.

Springboard presents a list of connection targets that includes a local target.
Selecting any different target immediately starts switching to it.
Selecting a remote target closes the local run preview before attempting the remote connection.
The local preview does not remain running during connection setup or pairing.
Selecting local again starts the local preview again.

Preserve the latest source highlight across endpoint switches and reconnects, including an explicit cleared highlight.
Springboard retains this highlight and forwards it when the selected endpoint can apply it.
No other additional state needs to be retained across endpoint switches for now.

### Future Debugger

Keep the selected endpoint accessible to future debugger functionality in both UI modes.
Allow the shared UI to grow beyond connection management.
Runtime inspection messages, capabilities, and data ownership are deferred to a later design discussion.

### Account Connection

Account login is outside the current scope.
Account login should provide another way to find and connect to a target.
How account identity relates to existing viewer pairing, connection transport, and reconnection remains to be designed.

### Endpoint Sources

The current endpoint list covers local preview, discovered viewers, and manually entered remote addresses.
The Slint endpoint struct exposes remote IP addresses and a port for display.
Connection handling and discovery bookkeeping stay in Rust.

Future targets may include iOS and Android simulators.
Springboard could launch a simulator, open Slint Viewer inside it, and connect.
Simulator support is outside the current design scope.

## Slint API Draft

This draft defines one global, one struct, and three enums.
The declarations expose UI state and actions without implementing connection behavior.

```slint
export enum PreviewEndpointKind {
    Local,
    Remote,
}

export struct PreviewEndpoint {
    name: string,
    kind: PreviewEndpointKind,
    addresses: [string],
    port: int,
    unavailable-reason: string,
}

export enum SpringboardState {
    Stopped,
    Idle,
    EndpointSelected,
}

export enum ConnectionState {
    None,
    Connecting,
    PairingRequired,
    UnpairedConfirmationRequired,
    Connected,
    Reconnecting,
    Failed,
}

export global Springboard {
    in property <[PreviewEndpoint]> endpoints;
    in property <int> selected-endpoint-index: -1;
    out property <PreviewEndpoint> selected-endpoint: endpoints[selected-endpoint-index];
    in property <SpringboardState> state;
    in property <ConnectionState> connection-state;
    in property <string> error-message;

    callback close();
    callback select-endpoint(endpoint-index: int);
    callback select-remote-address(address: string);
    callback submit-pairing-code(code: string);
    callback accept-unpaired-connection();
}
```

Rust supplies the input properties; UI code invokes callbacks to request changes.
`selected-endpoint-index` determines selection, with `-1` meaning no selection.
`selected-endpoint` is an output binding to that entry, not independently assigned state.
Consumers check the index before using the selected endpoint.
Rust matches endpoints across discovery updates and keeps the selected index aligned when the list changes.
Endpoint matching and the connection information saved in settings are internal details, without an ID in the Slint API.
Retain the selected entry while reconnecting even if discovery no longer reports it.

`SpringboardState` describes the lifecycle: stopped, running without a selected endpoint, or running with an endpoint selected.
`Idle` covers both restoring settings and waiting for the user to select an endpoint.
`ConnectionState` describes only the remote connection.
It is `None` when Springboard is stopped, no endpoint is selected, or the local endpoint is selected.
When it is `Connected`, `selected-endpoint` identifies the connected remote viewer.
Selection remains unchanged during connection attempts, pairing, reconnects, and failures.

`addresses` contains the remote viewer's IP addresses as strings; `port` is its connection port.
For local preview, the address list is empty and the port is unused.
Rust retains the connection information needed for manually entered hostnames and resolves their IP addresses.

An empty `unavailable-reason` means the endpoint can be selected.
Otherwise, it explains why the endpoint is unavailable, including protocol incompatibility.

The host starts Springboard externally through Run or Show Preview; the global reports the resulting state.
`close()` requests that the host stop Springboard, including its preview and connection activity.
In embedded mode, this does not close the visual editor's main window.
Endpoint selection callbacks apply while Springboard is running and immediately switch the selected target.
`select-remote-address()` accepts a manually entered host and port, creates or resolves its endpoint, and selects it.
Address validation and connection errors appear in `error-message`.

The pairing code stays local to the input component until submitted.
The current pairing protocol uses exactly four numeric digits, including leading zeros.
There is no variable code-length property in this API.
`PairingRequired` asks the user to enter the code displayed by the viewer to establish an authenticated, encrypted connection.
`UnpairedConfirmationRequired` asks permission to connect to a viewer with pairing disabled, without authentication or encryption.
`accept-unpaired-connection()` confirms a connection to a viewer with pairing disabled.
The user can decline a connection prompt by stopping Springboard or selecting another endpoint.

The old [remote preview view](../../tools/lsp/ui/components/remote-preview-view.slint) was checked for required information and actions.
It establishes the need for manual addresses, compatibility feedback, pairing input, unpaired confirmation, and connection errors.
Its UI implementation and API organization are not carried forward.

## Deferred Implementation Decisions

- Whether the standalone Springboard's local preview uses another subprocess or another window in the same process.
- Shutdown behavior for upstream `Quit` and closing the standalone window.
- Ordering of state synchronization and highlight replay during endpoint changes.
- How long startup discovery waits before treating a saved endpoint as unavailable.
- Account connection details and debugger architecture.
