# LSP Server Architecture

> Note for AI coding assistants (agents):
> **When to load this document:** Working on `tools/lsp/`, language server features,
> code completion, hover, go-to-definition, semantic tokens, live preview integration,
> or IDE tooling.
> For general build commands and project structure, see `/AGENTS.md`.

## Overview

The Slint LSP (Language Server Protocol) server provides IDE features for `.slint` files:

- **Code completion** - Property, element, type suggestions
- **Hover** - Type information and documentation
- **Go-to-definition** - Navigate to declarations
- **Semantic tokens** - Syntax highlighting
- **Document symbols** - Outline view
- **Rename** - Refactoring support
- **Formatting** - Code formatting
- **Live preview** - Real-time UI preview with hot reload

## Key Files

| File | Purpose |
|------|---------|
| `tools/lsp/main.rs` | Native entry point, CLI parsing, message loop |
| `tools/lsp/wasm_main.rs` | WASM entry point for web-based editors |
| `tools/lsp/language.rs` | LSP request handlers, server capabilities |
| `tools/lsp/language/completion.rs` | Code completion logic |
| `tools/lsp/language/goto.rs` | Go-to-definition |
| `tools/lsp/language/hover.rs` | Hover information |
| `tools/lsp/language/semantic_tokens.rs` | Syntax highlighting |
| `tools/lsp/language/signature_help.rs` | Function/callback signatures |
| `internal/editor-preview/editor_session.rs` | Editing session state (e.g. compiled documents, communication with preview) |
| `internal/editor-preview/document_cache.rs` | Document caching and compilation |
| `internal/editor-preview/editing/rename_component.rs` | Rename of components, structs, enums, properties, callbacks, functions |
| `tools/lsp/host_language_search.rs` | Cross-language rename: walks workspace files to replace matching Rust/C++ accessor identifiers |
| `internal/compiler/generator/accessor_names.rs` | Shared name mapping for Rust/C++ property/callback/function accessors (used by both codegen and the LSP scanner) |
| `tools/lsp/preview.rs` | Live preview engine |
| `tools/lsp/fmt/` | Code formatter |

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         IDE / Editor                            │
│                  (VS Code, vim, etc.)                           │
└───────────────────────────┬─────────────────────────────────────┘
                            │ LSP Protocol (JSON-RPC)
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│                      ServerNotifier                             │
│              (sends notifications/requests to client)           │
├─────────────────────────────────────────────────────────────────┤
│                        Context                                  │
│  ┌─────────────────┐  ┌─────────────────┐  ┌──────────────────┐ │
│  │ EditorSession   │  │ PreviewConfig   │  │ InitializeParams │ │
│  │ (DocumentCache) │  │                 │  │ (client caps)    │ │
│  └─────────────────┘  └─────────────────┘  └──────────────────┘ │
├─────────────────────────────────────────────────────────────────┤
│                    RequestHandler                               │
│  ┌───────────┐ ┌───────────┐ ┌───────────┐ ┌───────────┐        │
│  │Completion │ │ Hover     │ │ GotoDef   │ │ Rename    │ ...    │
│  └───────────┘ └───────────┘ └───────────┘ └───────────┘        │
├─────────────────────────────────────────────────────────────────┤
│                    Live Preview                                 │
│  ┌─────────────────┐  ┌─────────────────┐                       │
│  │ PreviewState    │  │ ComponentInst   │                       │
│  │ (UI, selection) │  │ (interpreter)   │                       │
│  └─────────────────┘  └─────────────────┘                       │
└─────────────────────────────────────────────────────────────────┘
```

## Core Types

### `Context`

`Context` holds the LSP-specific state shared by request handlers, including the editor session,
client capabilities, and the connection back to the editor.
Location: [`tools/lsp/language.rs`](../../tools/lsp/language.rs).

### `EditorSession`

`EditorSession` owns the document cache and the state needed to keep previews synchronized with edited files.
Shared between the LSP and Visual Editor.

Location: [`internal/editor-preview/editor_session.rs`](../../internal/editor-preview/editor_session.rs).

### `DocumentCache`

`DocumentCache` holds the core state of the edit session.
It loads and compiles documents, and tracks dependencies.
It is implemented as a wrapper over Slint's `TypeLoader`.

Location: [`internal/editor-preview/document_cache.rs`](../../internal/editor-preview/document_cache.rs).

### `RequestHandler`

`RequestHandler` maps LSP request names to typed handlers that operate on `Context`.
Location: [`tools/lsp/language.rs`](../../tools/lsp/language.rs).

## Server Capabilities

The LSP server advertises its capabilities via the `ServerCapabilities` type in `server_initialize_result` (`tools/lsp/language.rs`).

## Code Completion

### Completion Contexts

The completion system provides different `CompletionItems` based on the context:

```rust
pub(crate) fn completion_at(
    document_cache: &mut DocumentCache,
    token: SyntaxToken,
    offset: TextSize,
    client_caps: Option<&CompletionClientCapabilities>,
) -> Option<Vec<CompletionItem>>;
```

**Contexts handled:**
- **String literals**: Path completion for imports and `@image-url`
- **Element scope**: Child elements, properties, callbacks, keywords
- **Binding expressions**: Variables, properties, functions
- **Type annotations**: Type names from registry
- **Callback declarations**: Parameter types

### Element Scope Completion

```rust
fn resolve_element_scope(
    element: syntax_nodes::Element,
    document_cache: &DocumentCache,
    with_snippets: bool,
) -> Option<Vec<CompletionItem>>;
```

Suggests:
- Available child element types
- Properties from element type
- Callbacks from element type
- Keywords (`property`, `callback`, `animate`, `states`, etc.)
- Components available for import

### Expression Scope Completion

```rust
fn resolve_expression_scope(
    lookup_ctx: &LookupCtx,
    document_cache: &DocumentCache,
    snippet_support: bool,
) -> Option<Vec<CompletionItem>>;
```

Suggests:
- Local variables
- Properties from scope
- Built-in functions (`Math.*`, `Colors.*`)
- Enumeration values

## Semantic Tokens

The LSP provides syntax highlighting data via the SemanticTokens request.
In `tools/lsp/language/semantic_tokens.rs`, each syntax kind is assigned to a SemanticTokenType.
The semantic token types are lookup indices into a legend of `LEGEND_TYPES` and `LEGEND_MODS`.

## Go-to-Definition

Navigates to declarations:

```rust
pub fn goto_definition(
    document_cache: &mut DocumentCache,
    token: SyntaxToken,
) -> Option<GotoDefinitionResponse>;
```

**Handles:**
- Element IDs → Element definition
- Property names → Property declaration
- Type names → Struct/component definition
- Import paths → Imported file
- Qualified names → Resolved definition

## Rename

Rename support lives in `internal/editor-preview/editing/rename_component.rs` and is
dispatched from the `textDocument/rename` handler in `language.rs`. It
handles components, structs, enums, internal/export names, properties,
callbacks, and functions through a single `DeclarationNode::rename`
entry point that returns a `WorkspaceEdit` covering the `.slint`
sources.

### Cross-language rename

Renaming a public property, callback, or function can also search and replace
its generated Rust/C++ accessors in workspace files.
See `internal/editor-preview/editing/rename_component.rs` for the rename flow and
`tools/lsp/host_language_search.rs` for the workspace search.

## Live Preview

The Live Preview is architecturally separated from the LSP with its own state.
This allows running the Live Preview either embedded, or in a separate process.
Running in a separate process prevents the LSP from crashing if the live preview
crashes and is necessary on macOS, where only the main thread of a process can show UI.

### LSP ↔ Preview Communication

The LSP communicates with the preview via a protocol defined in the `i-slint-live-preview` crate
(`internal/live-preview/protocol/`), which implements the `LspToPreviewMessage` and
`PreviewToLspMessage` enums used for message traffic.


### Preview State

```rust
pub struct PreviewState {
    pub app_window: Option<ui::AppWindow>,
    pub api: slint::Weak<ui::Api<'static>>,
    handle: Rc<RefCell<Option<ComponentInstance>>>,
    document_cache: Rc<RefCell<Option<Rc<DocumentCache>>>>,
    selected: Option<ElementSelection>,

    source_code: SourceCodeCache,
    pub config: PreviewConfig,
    current_previewed_component: Option<PreviewComponent>,
    loading_state: PreviewFutureState,

    pub to_lsp: RefCell<Option<Rc<dyn PreviewToLsp>>>,
    // ... undo/redo, live-data, and dependency-tracking fields
}
```

### Preview Loading States

```
                              ┌─────────────┐
                           ┌──│ NeedsReload │◄─┐
                           │  └─────────────┘  │
                           ▼                   │
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│ Pending     │────►│ PreLoading  │────►│ Loading     │
└─────────────┘     └─────────────┘     └─────────────┘
       ▲                                       │
       │                                       │
       └───────────────────────────────────────┘
```


## Document Synchronization

### Open/Change/Close Flow

```
Editor                    LSP Server
   │                          │
   │──didOpen(uri, text)─────►│ Compile document
   │                          │ Cache in DocumentCache
   │                          │
   │──didChange(uri, text)───►│ Re-compile document
   │                          │ Publish diagnostics
   │                          │ Notify preview
   │                          │
   │◄──publishDiagnostics─────│
   │                          │
   │──didClose(uri)──────────►│ Remove from open set
   │                          │ Drop document, queue
   │                          │ dependent recompilations
```

### File Watching

The server registers for file change notifications via the editor:

```rust
let file_system_watcher = DidChangeWatchedFilesRegistrationOptions {
    watchers: vec![FileSystemWatcher {
        glob_pattern: GlobPattern::String("**/*".to_string()),
        kind: Some(WatchKind::Change | WatchKind::Delete | WatchKind::Create),
    }],
};
```

When a file changes on disk:
1. If the file is not open in the editor, drop it from the cache
2. Queue any open dependent documents for recompilation via `pending_recompile`
3. After a debounce delay, recompile all pending documents
4. If a resource file changes, the live preview is reloaded

## Commands

### Show Preview

```rust
pub const SHOW_PREVIEW_COMMAND: &str = "slint/showPreview";

// Arguments: [file_uri, component_name]
let title = format!("{}Show Preview", if pretty { &"▶ " } else { &"" });
Command::new(
    title,
    SHOW_PREVIEW_COMMAND.into(),
    Some(vec![file.as_str().into(), component_name.into()]),
)
```

### Populate (Insert Text)

```rust
const POPULATE_COMMAND: &str = "slint/populate";

let text_document = OptionalVersionedTextDocumentIdentifier { uri: document_uri, version };
Command::new(
    title,
    POPULATE_COMMAND.into(),
    Some(vec![serde_json::to_value(text_document).unwrap(), text.into()]),
)
```

## Common Patterns

### Finding Token at Position

```rust
let (document, offset) = document_cache.get_document_and_offset(&document_uri, &position)?;
let token = token_at_offset(document.node.as_ref()?, offset)?;
```

### Using Lookup Context

```rust
fn with_lookup_ctx<R>(
    document_cache: &DocumentCache,
    node: SyntaxNode,
    offset: Option<TextSize>,
    callback: impl FnOnce(&mut LookupCtx) -> R,
) -> Option<R>;

// Example usage
with_lookup_ctx(document_cache, node, Some(offset), |lookup_context| {
    resolve_expression_scope(lookup_context, document_cache, snippet_support)
})?
```

### Finding Element at Position

`element_at_position` is a method on `DocumentCache`
(`internal/editor-preview/document_cache.rs`), not a free function:

```rust
impl DocumentCache {
    pub fn element_at_position(
        &self,
        text_document_uri: &Url,
        position: &Position,
    ) -> Option<ElementRcNode>;
}
```

### Publishing Diagnostics

```rust
crate::lsp_to_editor::publish_diagnostics(&context.server_notifier, diagnostics);
```

## Testing

### Running LSP Tests

```sh
# Run all LSP tests
cargo test -p slint-lsp

# Run specific module tests
cargo test -p slint-lsp language::test
cargo test -p slint-lsp completion

# Run with logging
RUST_LOG=debug cargo test -p slint-lsp
```

### Test Utilities

```rust
// In language/test.rs
pub use i_slint_editor_preview::test::{
    complex_document_cache, empty_document_cache, empty_document_cache_with_experimental, load,
    loaded_document_cache, loaded_document_cache_with_experimental,
    loaded_document_cache_with_file_name,
};
```

## Debugging Tips

### Common Issues

| Issue | Cause | Solution |
|-------|-------|----------|
| No completions | Token not found | Check offset calculation, byte format |
| Wrong definitions | Stale cache | Trigger recompile via didChange |
| Preview not updating | Message not sent | Check to_preview channel |
| Semantic tokens wrong | Token classification | Check SyntaxKind → token type mapping |

### Logging

The LSP server uses the `tracing` crate for structured logging:
Set `RUST_LOG` when starting the editor or the lsp itself.
e.g. in VS Code, the log can be observed via the "Output" panel.

```sh
# Enable debug logging
RUST_LOG=slint_lsp=debug code

# Enable trace logging for more detail
RUST_LOG=slint_lsp=trace code
```

### Inspecting Document State

```rust
// List all cached documents
for (url, doc) in document_cache.all_url_documents() {
    tracing::trace!("Cached: {}", url);
}

// Check document version
let version = document_cache.document_version(&uri);
```

## Building

```sh
# Build LSP server
cargo build -p slint-lsp

# Build for WASM (VS Code web)
pnpm --dir editors/vscode run build:wasm_lsp
```
