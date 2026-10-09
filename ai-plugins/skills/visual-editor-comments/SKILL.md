---
name: visual-editor-comments
description: Connect the current Codex chat to a running Slint Visual Editor when the user explicitly requests it, and handle annotations sent to an already connected chat.
---

# Visual Editor Chat Registration

## Connect This Chat

Register only when the user explicitly asks to connect this chat to the Visual Editor.
Invoking the Slint plugin or editing Slint source does not request registration.
The Visual Editor is installed separately; this plugin provides its MCP bridge.
If the editor tools are unavailable, report that the plugin runtime must be built or a platform package installed.

Call `discover_visual_editors` with the current chat's exact absolute working directory as `workingDirectory`.
Discovery includes editors with no annotations and returns their `instanceId` and `projectRoot`.
Only use editors whose canonical project root equals or is beneath that working directory.
If no editor matches, report that result and keep the working directory unchanged.
If several editors match, ask the user to choose an instance.

Call `register_visual_editor_chat` with the same `workingDirectory`, the selected `instanceId`, and `provider: "codex"`.
Supply this chat's own `threadId`, a recognizable `displayName`, and the absolute Codex CLI path as `cliPath`.
Use `CODEX_THREAD_ID` for the current thread ID when available in a local Codex task.
Resolve the CLI path with `command -v codex` and ensure it is absolute.
If the current chat ID is unavailable, ask for it rather than guessing or registering another chat.
Registration replaces the project's previous destination chat.

## Handle Annotation Feedback

Registration lets the user send annotations from the editor to this chat through the Codex CLI queue.
Treat received annotations as user feedback on the identified file, element, and source range.
Keep the editor's project and instance as the scope for subsequent tool calls.
If the instance is unknown, discover matching editors and ask the user to choose when several match.
Receiving annotation feedback does not require registering the chat again.

Use `pendingMessageIds` in received feedback to identify new user messages.
The chronological `conversation` provides user and Codex replies; the original annotation's `id` and source context identify the issue.
Read and edit the identified source files, preserving the user's requested scope.
Call `screenshot_visual_editor_canvas` with the same `workingDirectory` and selected `instanceId` to inspect the current canvas.
It waits for source compilation and includes zoom, pan, selection, and annotation popovers.
Compilation failures return diagnostics; do not treat them as a current screenshot.
Use this canvas for visual verification of editor feedback instead of opening another Wasm preview.
Inspect the returned image before claiming visual verification.

Call `reply_visual_editor_annotation` with that scope, the original `id` as `annotationId`, `provider: "codex"`, and your reply as `text`.
Use a reply to explain changes or ask an outstanding question; it appears in the existing canvas thread.
Call `resolve_visual_editor_annotation` with that scope and `annotationId` only once the issue is definitively addressed and no open questions remain.
Otherwise, reply or ask and leave the thread open.
Resolving permanently removes the original annotation and all replies; there's no history or reopening.
