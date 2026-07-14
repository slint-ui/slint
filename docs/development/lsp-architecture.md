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
| `tools/lsp/common/document_cache.rs` | Document caching and compilation |
| `tools/lsp/common/rename_component.rs` | Rename of components, structs, enums, properties, callbacks, functions |
| `tools/lsp/common/host_language_search.rs` | Cross-language rename: walks workspace files to replace matching Rust/C++ accessor identifiers |
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
│  │ DocumentCache   │  │ PreviewConfig   │  │ InitializeParams │ │
│  │ (TypeLoader)    │  │                 │  │ (client caps)    │ │
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

### Context

Main server state shared across all request handlers:

```rust
pub struct Context {
    /// Cached compiled documents
    pub document_cache: RefCell<DocumentCache>,

    /// Preview configuration (style, backend)
    pub preview_config: RefCell<PreviewConfig>,

    /// For sending messages to client
    pub server_notifier: ServerNotifier,

    /// Client capabilities from initialization
    pub init_param: InitializeParams,

    /// Currently open files in editor
    pub open_urls: RefCell<HashSet<Url>>,

    /// Channel to preview process
    pub to_preview: Rc<dyn LspToPreview>,

    /// Files to recompile after all other operations are done
    /// (recompilations triggered by updates to unopened files)
    pub pending_recompile: RefCell<HashSet<Url>>,
}
```

### DocumentCache

Manages compiled documents using the compiler's TypeLoader:

```rust
pub struct DocumentCache {
    type_loader: TypeLoader,
    open_import_callback: Option<OpenImportCallback>,
    source_file_versions: Rc<RefCell<SourceFileVersionMap>>,
    pub format: ByteFormat,  // UTF-8 or UTF-16
}

impl DocumentCache {
    /// Get compiled document by URL
    pub fn get_document(&self, url: &Url) -> Option<&Document>;

    /// Get document and text offset for position
    pub fn get_document_and_offset(
        &self,
        uri: &Url,
        pos: &Position,
    ) -> Option<(&Document, TextSize)>;

    /// Iterate all documents
    pub fn all_url_documents(&self) -> impl Iterator<Item = (Url, &syntax_nodes::Document)>;

    /// Reconfigure compiler settings
    pub async fn reconfigure(
        &mut self,
        style: Option<String>,
        include_paths: Option<Vec<PathBuf>>,
        library_paths: Option<HashMap<String, PathBuf>>,
    ) -> Result<CompilerConfiguration>;

    /// Create snapshot for preview
    pub fn snapshot(&self) -> Option<Self>;

    /// Drop document and reload from disk. Returns invalidated dependencies.
    pub fn drop_document(&mut self, url: &Url) -> Result<HashSet<Url>>;

    /// Invalidate document but keep CST in cache (only re-analyze).
    pub fn invalidate_url(&mut self, url: &Url) -> HashSet<Url>;
}
```

### RequestHandler

Dispatches LSP requests to handlers:

```rust
pub struct RequestHandler(
    HashMap<
        &'static str,
        Box<dyn Fn(Value, Rc<Context>) -> Pin<Box<dyn Future<Output = Result<Value, LspError>>>>>,
    >,
);

impl RequestHandler {
    pub fn register<R: Request, Fut>(
        &mut self,
        handler: fn(R::Params, Rc<Context>) -> Fut,
    );
}

// Registration example
pub fn register_request_handlers(rh: &mut RequestHandler) {
    rh.register::<GotoDefinition, _>(goto_definition_handler);
    rh.register::<Completion, _>(completion_handler);
    rh.register::<HoverRequest, _>(hover_handler);
    // ...
}
```

## Server Capabilities

The LSP server advertises these capabilities:

```rust
ServerCapabilities {
    hover_provider: true,
    signature_help_provider: SignatureHelpOptions {
        trigger_characters: ["(", ","],
    },
    completion_provider: CompletionOptions {
        trigger_characters: ["."],
    },
    definition_provider: true,
    text_document_sync: TextDocumentSyncKind::FULL,
    code_action_provider: true,
    execute_command_provider: ["slint/populate", "slint/showPreview"],
    document_symbol_provider: true,
    color_provider: true,
    code_lens_provider: true,
    semantic_tokens_provider: SemanticTokensOptions { ... },
    document_highlight_provider: true,
    rename_provider: RenameOptions { prepare_provider: true },
    document_formatting_provider: true,
}
```

## Code Completion

### Completion Contexts

The completion system handles different contexts:

```rust
pub fn completion_at(
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

Provides syntax highlighting data:

```rust
// Token types
pub const LEGEND_TYPES: &[SemanticTokenType] = &[
    TYPE, PARAMETER, VARIABLE, PROPERTY, FUNCTION,
    MACRO, KEYWORD, COMMENT, STRING, NUMBER, OPERATOR,
    ENUM, ENUM_MEMBER,
];

// Token modifiers
pub const LEGEND_MODS: &[SemanticTokenModifier] = &[
    DEFINITION, DECLARATION,
];
```

### Token Assignment

| Syntax Kind | Token Type | Notes |
|-------------|------------|-------|
| `Comment` | COMMENT | |
| `StringLiteral` | STRING | |
| `NumberLiteral` | NUMBER | |
| `ColorLiteral` | NUMBER | |
| Component name | TYPE | With DEFINITION modifier |
| Element ID | VARIABLE | With DEFINITION modifier |
| Property binding | PROPERTY | |
| Callback name | FUNCTION | |
| `@children` | MACRO | |

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

Rename support lives in `tools/lsp/common/rename_component.rs` and is
dispatched from the `textDocument/rename` handler in `language.rs`. It
handles components, structs, enums, internal/export names, properties,
callbacks, and functions through a single `DeclarationNode::rename`
entry point that returns a `WorkspaceEdit` covering the `.slint`
sources.

### Cross-language rename

Renaming a public property, callback, or function can also search and replace
its generated Rust/C++ accessors in workspace files.
See `tools/lsp/common/rename_component.rs` for the rename flow and
`tools/lsp/common/host_language_search.rs` for the workspace search.

## Live Preview

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
    // ... plus undo/redo, live-data, and dependency-tracking fields
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

### LSP ↔ Preview Communication

Both enums are defined in the `i-slint-live-preview` crate
(`internal/live-preview/protocol/`), not in `tools/lsp/` itself.

```rust
// LSP to Preview (internal/live-preview/protocol/lsp_to_preview.rs)
pub enum LspToPreviewMessage {
    InvalidateContents { url: Url },
    ForgetFile { url: Url },
    SetContents { url: VersionedUrl, contents: Vec<u8> },
    SetConfiguration { config: PreviewConfig },
    ShowPreview(PreviewComponent),
    HighlightFromEditor { url: Option<Url>, offset: u32 },
    // ... plus remote-preview WebSocket state messages
}

// Preview to LSP (internal/live-preview/protocol/preview_to_lsp.rs)
pub enum PreviewToLspMessage {
    Diagnostics { uri: Url, version: SourceFileVersion, diagnostics: Vec<Diagnostic> },
    ShowDocument { file: Url, selection: Range, take_focus: bool },
    PreviewTypeChanged { target: PreviewTarget },
    RequestState { files: Vec<Url> },
    SendWorkspaceEdit { label: Option<String>, edit: WorkspaceEdit },
    SendShowMessage { message: ShowMessageParams },
    TelemetryEvent(Map<String, Value>),
    DebugMessage { location: Option<(PathBuf, usize, usize)>, message: String },
    ConnectRemote { addresses: Vec<String>, port: u16 },
    DisconnectRemote,
    Pong,
}
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

The server registers for file change notifications:

```rust
let fs_watcher = DidChangeWatchedFilesRegistrationOptions {
    watchers: vec![FileSystemWatcher {
        glob_pattern: "**/*".to_string(),
        kind: WatchKind::Change | WatchKind::Delete,
    }],
};
```

When a file changes on disk:
1. If the file is not open in the editor, drop it from the cache
2. Queue any open dependent documents for recompilation via `pending_recompile`
3. After a 50ms debounce delay, recompile all pending documents
4. If a resource file changes, the live preview is reloaded

## Commands

### Show Preview

```rust
const SHOW_PREVIEW_COMMAND: &str = "slint/showPreview";

// Arguments: [file_uri, component_name]
Command::new(
    "Show Preview",
    SHOW_PREVIEW_COMMAND,
    Some(vec![file.as_str().into(), component_name.into()]),
)
```

### Populate (Insert Text)

```rust
const POPULATE_COMMAND: &str = "slint/populate";

// Used for auto-inserting property templates
Command::new(
    title,
    POPULATE_COMMAND,
    Some(vec![text_document.into(), text.into()]),
)
```

## Common Patterns

### Finding Token at Position

```rust
let (doc, offset) = document_cache.get_document_and_offset(&uri, &position)?;
let token = doc.node.as_ref()?.token_at_offset(offset).right_biased()?;
```

### Using Lookup Context

```rust
fn with_lookup_ctx<R>(
    document_cache: &DocumentCache,
    node: SyntaxNode,
    offset: Option<TextSize>,
    f: impl FnOnce(&LookupCtx) -> R,
) -> Option<R>;

// Example usage
with_lookup_ctx(document_cache, node, Some(offset), |ctx| {
    resolve_expression_scope(ctx, document_cache, snippet_support)
})?
```

### Finding Element at Position

`element_at_position` is a method on `DocumentCache`
(`tools/lsp/common/document_cache.rs`), not a free function:

```rust
impl DocumentCache {
    pub fn element_at_position(
        &self,
        uri: &Url,
        position: &Position,
    ) -> Option<ElementRcNode>;
}
```

### Publishing Diagnostics

```rust
ctx.server_notifier.send_notification::<PublishDiagnostics>(
    PublishDiagnosticsParams {
        uri: file_to_uri(&path)?,
        diagnostics: diags,
        version: document_cache.document_version(&uri),
    },
)?;
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
pub fn compile_test_source(source: &str) -> (DocumentCache, Url);

// Test completion
#[test]
fn test_element_completion() {
    let (mut dc, url) = compile_test_source("component Foo { }");
    let completions = completion_at(&mut dc, token, offset, None);
    assert!(completions.iter().any(|c| c.label == "Rectangle"));
}
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

```sh
# Enable debug logging
RUST_LOG=slint_lsp=debug slint-lsp

# Enable trace logging for more detail
RUST_LOG=slint_lsp=trace slint-lsp
```

Key events are logged at appropriate levels:
- `trace`: Document loading, diagnostics sending, file imports
- `debug`: Document open/close/change, file watcher events, preview diagnostics

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

# Build with preview
cargo build -p slint-lsp --features preview-engine

# Build for WASM (VS Code web)
cargo build -p slint-lsp --target wasm32-unknown-unknown
```
