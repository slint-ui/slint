# Visual Editor Chat Registration

Build the MCP server with `cargo build -p slint-editor-mcp` and launch `target/debug/slint-editor-mcp` as a stdio MCP server.
The editor publishes only connection metadata for discovery.
Annotations stay in the editor until the user sends them to a registered chat.

Call `discover_visual_editors` with the current chat's absolute `workingDirectory`:

```json
{
  "workingDirectory": "/home/leon/Documents/projects/slint"
}
```

Discovery includes running editors with no annotations.
It returns each editor's `instanceId` and `projectRoot`.
All tools restrict the editor project root to the canonical working directory or one of its descendants.
Keep the current working directory when no editor matches.

Call `register_visual_editor_chat` using the current chat's own thread ID and display name, and an absolute Codex CLI path:

```json
{
  "workingDirectory": "/home/leon/Documents/projects/slint",
  "instanceId": "instance-from-discovery",
  "provider": "codex",
  "threadId": "current-chat-thread-id",
  "displayName": "Visual Editor Feedback",
  "cliPath": "/absolute/path/to/codex"
}
```

When exactly one editor matches, `instanceId` is optional.
When several editors match, choose an instance explicitly.
An explicit instance ID does not bypass the working directory restriction.
Registration fails if the editor changes projects before it receives the request.

The prototype supports Codex destinations.
Sending an annotation invokes the registered CLI from the editor and queues feedback to the selected chat.
Call `screenshot_visual_editor_canvas` with `workingDirectory` and an optional `instanceId` to capture the current canvas viewport.
The PNG includes the current zoom, pan, selection, and annotation popovers.
It excludes the editor top bar and sidebars.
The tool waits for current source files and imports to finish compiling and installing in the canvas.
Compilation failures return the latest compiler diagnostics instead of a stale screenshot.
A project or preview target change cancels the request.
The readiness wait times out after 20 seconds.
The MCP server has no annotation snapshots, resources, mention search, or annotation-reading tools.
