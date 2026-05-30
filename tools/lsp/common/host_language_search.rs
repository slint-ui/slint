// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore absolutized bget countñ xéget xget xñget xxget

//! Scan the workspace for Rust/C++ call sites of Slint-generated property,
//! callback, and function accessors so that a Slint rename can extend its
//! `WorkspaceEdit` with edits in host-language source files.
//!
//! The scanner is intentionally simple: byte-level word-boundary search for a
//! small set of accessor names derived from
//! [`i_slint_compiler::generator::accessor_names`]. It does not parse Rust or
//! C++. Cross-component accessor collisions are possible by construction; see
//! the design in the PR for #11841 for the mitigations (config opt-in,
//! preview-before-apply in v2).

#![cfg(not(target_arch = "wasm32"))]

use std::path::{Path, PathBuf};
use std::rc::Rc;

use i_slint_compiler::diagnostics::{ByteFormat, SourceFile, SourceFileInner};
use i_slint_compiler::generator::accessor_names::{self, DeclarationKind};
use lsp_types::{InitializeParams, TextEdit, Url, WorkspaceFolder};

use super::SingleTextEdit;

/// Host-language file extensions the scanner considers.
const HOST_FILE_EXTENSIONS: &[&str] = &["rs", "cpp", "cc", "cxx", "h", "hpp", "hh"];

/// Directory names skipped during the walk.
const SKIP_DIRS: &[&str] = &["target", "build", "out", "dist", "node_modules", ".git"];

/// User policy for extending a Slint rename with host-language accessor
/// edits. Mapped from the `slint.renameAccessorsInHostLanguages` workspace
/// configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RenameAccessorsPolicy {
    /// No host-language edits. Current behavior.
    #[default]
    Never,
    /// Extend every `textDocument/rename` `WorkspaceEdit` with the scanner's
    /// host-language edits.
    Always,
}

impl RenameAccessorsPolicy {
    /// Parse the string form used in the workspace config.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "never" => Some(Self::Never),
            "always" => Some(Self::Always),
            _ => None,
        }
    }
}

/// Hard limits on scan work. `textDocument/rename` is synchronous in the LSP;
/// an unbounded scan would stall the server.
#[derive(Debug, Clone, Copy)]
pub struct ScanBounds {
    /// Maximum number of host-language files inspected.
    pub max_files: usize,
    /// Maximum size in bytes of a single file. Larger files are skipped
    /// (not an error — likely generated code or vendored sources).
    pub max_file_bytes: u64,
}

impl Default for ScanBounds {
    fn default() -> Self {
        Self { max_files: 5_000, max_file_bytes: 1 << 20 /* 1 MiB */ }
    }
}

/// Failure modes for the scanner. The Rename handler in `language.rs`
/// soft-degrades these to a `tracing::warn!` and applies only the
/// `.slint`-side edits, so callers should treat them as advisory rather
/// than fatal. The error variants exist so the warning can be specific.
#[derive(Debug)]
pub enum HostLanguageScanError {
    /// The renamed `.slint` file is not under any configured workspace
    /// folder, so there is no well-defined scan root.
    OutsideWorkspace,
    /// The scan would inspect more than `bounds.max_files` files.
    TooManyFiles { limit: usize },
}

impl std::fmt::Display for HostLanguageScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutsideWorkspace => f.write_str(
                "Cannot scan host-language files: the renamed .slint file is \
                 not under any workspace folder",
            ),
            Self::TooManyFiles { limit } => write!(
                f,
                "Cannot scan host-language files: workspace exceeds {limit} \
                 files. Disable slint.renameAccessorsInHostLanguages or \
                 narrow the workspace folder.",
            ),
        }
    }
}

impl std::error::Error for HostLanguageScanError {}

/// Walk the workspace folder containing `renamed_source` for `.rs`/`.cpp`/...
/// files, replace any occurrence of the old accessor names derived from
/// `(kind, old_name)` with the corresponding new accessor names derived from
/// `(kind, new_name)`, and return the resulting per-file edits.
///
/// The returned edits can be merged with the `.slint`-only edits via
/// [`super::create_workspace_edit_from_single_text_edits`].
pub fn scan_host_language_accessors(
    workspace_folders: &[WorkspaceFolder],
    renamed_source: &Path,
    kind: DeclarationKind,
    old_name: &str,
    new_name: &str,
    format: ByteFormat,
    bounds: ScanBounds,
) -> Result<Vec<SingleTextEdit>, HostLanguageScanError> {
    let scan_root = workspace_folder_for(workspace_folders, renamed_source)
        .ok_or(HostLanguageScanError::OutsideWorkspace)?;

    let accessors = accessor_pairs(kind, old_name, new_name);

    let mut files = Vec::new();
    collect_files(&scan_root, &mut files, bounds.max_files)?;

    let mut edits = Vec::new();
    for path in files {
        let metadata = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if metadata.len() > bounds.max_file_bytes {
            continue;
        }
        let contents = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => continue, // non-UTF-8 or unreadable; skip
        };

        let file_edits = scan_file_contents(&contents, &accessors);
        if file_edits.is_empty() {
            continue;
        }

        let url = match Url::from_file_path(&path) {
            Ok(u) => u,
            Err(()) => continue,
        };
        // Synthesize a SourceFile for line/column conversion. Host-language
        // files are not in the LSP document cache, so they have no version.
        let source_file: SourceFile = Rc::new(SourceFileInner::new(path.clone(), contents));
        for (range, new_text) in file_edits {
            let lsp_range = byte_range_to_lsp_range(&source_file, range, format);
            edits.push(SingleTextEdit {
                url: url.clone(),
                version: None,
                edit: TextEdit { range: lsp_range, new_text },
            });
        }
    }
    Ok(edits)
}

/// Build the set of workspace folders the scanner should consider.
///
/// Prefers `init_param.workspace_folders` when the client sent any; falls
/// back to the (LSP-deprecated but still common) `root_uri` / `root_path` so
/// editors that initialize the server in single-folder mode (older VS Code,
/// many Neovim setups, Helix) still get host-language scanning.
///
/// Returns an empty Vec if no folder could be derived; the scanner will then
/// fail with `OutsideWorkspace` (which the handler soft-degrades to a
/// warning).
#[allow(deprecated)] // root_uri / root_path are deprecated but widely used
pub fn resolve_workspace_folders(init_param: &InitializeParams) -> Vec<WorkspaceFolder> {
    if let Some(folders) = init_param.workspace_folders.as_ref()
        && !folders.is_empty()
    {
        return folders.clone();
    }
    if let Some(uri) = init_param.root_uri.as_ref() {
        return vec![WorkspaceFolder { uri: uri.clone(), name: String::new() }];
    }
    if let Some(path) = init_param.root_path.as_ref()
        && let Ok(uri) = Url::from_file_path(path)
    {
        return vec![WorkspaceFolder { uri, name: String::new() }];
    }
    Vec::new()
}

/// Most-specific workspace folder whose URI's filesystem path is an ancestor
/// of `path`. Returns `None` if no folder qualifies.
///
/// Performs the prefix comparison on absolutized (but not canonicalized)
/// paths so that:
/// - unsaved/untitled documents whose path doesn't exist on disk still match
///   their containing workspace folder;
/// - a `.slint` reached via a symlink isn't excluded because canonicalize
///   resolves it outside the workspace.
///
/// The trade-off is that a symlink-based "outside-the-workspace" file path
/// can falsely match its symlink parent; that's a milder failure than
/// silently dropping every rename for symlinked files.
fn workspace_folder_for(folders: &[WorkspaceFolder], path: &Path) -> Option<PathBuf> {
    let target = absolute_path(path);
    folders
        .iter()
        .filter_map(|f| f.uri.to_file_path().ok())
        .map(|p| absolute_path(&p))
        .filter(|folder| target.starts_with(folder))
        .max_by_key(|folder| folder.components().count())
}

/// Make a path absolute without touching the filesystem. Falls back to the
/// path as-given if joining with the current dir would fail (which it
/// effectively can't for our inputs).
fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map(|cwd| cwd.join(path)).unwrap_or_else(|_| path.to_path_buf())
    }
}

/// Compute the `(old, new)` accessor name pairs for the rename. Rust and C++
/// emit identical accessor strings today, so a single pair set suffices for
/// both languages.
fn accessor_pairs(kind: DeclarationKind, old_name: &str, new_name: &str) -> Vec<(String, String)> {
    kind.accessor_kinds()
        .iter()
        .map(|&ak| {
            (
                accessor_names::rust_accessor_name(old_name, ak).to_string(),
                accessor_names::rust_accessor_name(new_name, ak).to_string(),
            )
        })
        .collect()
}

/// Recursive walk of `root`, appending host-language file paths into `out`.
/// Returns `TooManyFiles` if `max_files` is exceeded. Unreadable directories
/// are silently skipped (don't fail the whole rename on a permission error
/// in some sibling folder).
///
/// Does not follow symlinks: an `entry.file_type()` whose `is_symlink()` is
/// true is skipped entirely. This avoids unbounded recursion through symlink
/// cycles (vendored deps, sibling-crate links) at the cost of not scanning
/// symlinked source trees -- users who want those scanned should add the
/// real directory as a workspace folder.
fn collect_files(
    root: &Path,
    out: &mut Vec<PathBuf>,
    max_files: usize,
) -> Result<(), HostLanguageScanError> {
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(e) => {
            tracing::debug!("host-language scan: skipping unreadable dir {}: {e}", root.display());
            return Ok(());
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                tracing::debug!(
                    "host-language scan: skipping unreadable entry under {}: {e}",
                    root.display()
                );
                continue;
            }
        };
        let Ok(file_type) = entry.file_type() else { continue };
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && SKIP_DIRS.contains(&name)
            {
                continue;
            }
            collect_files(&path, out, max_files)?;
        } else if file_type.is_file() && has_host_extension(&path) {
            if out.len() >= max_files {
                return Err(HostLanguageScanError::TooManyFiles { limit: max_files });
            }
            out.push(path);
        }
    }
    Ok(())
}

/// Case-insensitive check against [`HOST_FILE_EXTENSIONS`].
fn has_host_extension(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else { return false };
    HOST_FILE_EXTENSIONS.iter().any(|known| known.eq_ignore_ascii_case(ext))
}

/// Find every word-boundary-aligned occurrence of any old accessor name in
/// `contents`, paired with the replacement text.
///
/// The walk maintains a small lexer state so matches inside line comments,
/// block comments, double-quoted string literals, and short char literals
/// (`'X'`, `'\\X'`) are skipped. The lexer is intentionally minimal -- raw
/// strings, byte strings, Rust lifetimes (`'a`), nested block comments,
/// and language-specific escapes aren't handled. False positives in those
/// contexts are an accepted v1 limitation; users opted in via
/// `slint.renameAccessorsInHostLanguages = "always"` and should review the
/// proposed edits before applying.
///
/// Word boundaries treat any non-ASCII byte as if it extended an identifier,
/// so a UTF-8 continuation byte adjacent to a match defeats the boundary --
/// `xñget_count` is correctly rejected.
///
/// Optional leading `r#` (raw identifier syntax in Rust) is included in the
/// matched range so that the replacement drops it; the accessor prefixes
/// (`get_`, `set_`, etc.) make the result never a Rust keyword.
fn scan_file_contents(
    contents: &str,
    accessors: &[(String, String)],
) -> Vec<(std::ops::Range<usize>, String)> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum LexState {
        Code,
        LineComment,
        BlockComment,
        String,
    }

    let bytes = contents.as_bytes();
    let mut matches = Vec::new();
    let mut state = LexState::Code;
    let mut i = 0;
    while i < bytes.len() {
        match state {
            LexState::Code => {
                if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'/') {
                    state = LexState::LineComment;
                    i += 2;
                    continue;
                }
                if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                    state = LexState::BlockComment;
                    i += 2;
                    continue;
                }
                if bytes[i] == b'"' {
                    state = LexState::String;
                    i += 1;
                    continue;
                }
                // Char literal: `'X'` or `'\\X'` (any single escape).
                // Lifetimes look like `'a` without a closing quote; we
                // peek ahead and only skip if a matching `'` is present
                // within a short distance, otherwise treat `'` as a
                // plain code byte and fall through.
                if bytes[i] == b'\''
                    && let Some(end) = try_consume_char_literal(bytes, i)
                {
                    i = end;
                    continue;
                }
                // Try matching each accessor needle at the current position.
                let mut matched_len = 0;
                for (old, new) in accessors {
                    let needle = old.as_bytes();
                    if bytes[i..].starts_with(needle) {
                        let end = i + needle.len();
                        let after_ok = end == bytes.len() || !extends_identifier(bytes[end]);
                        let (effective_start, before_ok) =
                            if i >= 2 && bytes[i - 2] == b'r' && bytes[i - 1] == b'#' {
                                let s = i - 2;
                                (s, s == 0 || !extends_identifier(bytes[s - 1]))
                            } else {
                                (i, i == 0 || !extends_identifier(bytes[i - 1]))
                            };
                        if before_ok && after_ok {
                            matches.push((effective_start..end, new.clone()));
                            matched_len = needle.len();
                            break;
                        }
                    }
                }
                if matched_len > 0 {
                    i += matched_len;
                } else {
                    i += utf8_char_len(bytes, i);
                }
            }
            LexState::LineComment => {
                if bytes[i] == b'\n' {
                    state = LexState::Code;
                }
                i += 1;
            }
            LexState::BlockComment => {
                if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    state = LexState::Code;
                    i += 2;
                    continue;
                }
                i += 1;
            }
            LexState::String => {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2; // skip escape sequence (\" included)
                    continue;
                }
                if bytes[i] == b'"' {
                    state = LexState::Code;
                }
                i += 1;
            }
        }
    }
    matches.sort_by_key(|(r, _)| r.start);
    matches
}

/// If `bytes[i]` starts a short char literal (`'X'` or `'\\X'`), return the
/// byte offset just past the closing quote; otherwise return `None`. The
/// caller should then treat `'` as a plain code byte (Rust lifetimes look
/// like `'a` without a closing quote).
fn try_consume_char_literal(bytes: &[u8], i: usize) -> Option<usize> {
    debug_assert_eq!(bytes[i], b'\'');
    // `'\X'` where X is any single byte (covers `'\n'`, `'\''`, `'\"'`, etc.)
    if bytes.get(i + 1) == Some(&b'\\') && i + 3 < bytes.len() && bytes[i + 3] == b'\'' {
        return Some(i + 4);
    }
    // `'X'` where X is one UTF-8 code point.
    if i + 1 < bytes.len() && bytes[i + 1] != b'\'' {
        let char_len = utf8_char_len(bytes, i + 1);
        let close = i + 1 + char_len;
        if bytes.get(close) == Some(&b'\'') {
            return Some(close + 1);
        }
    }
    None
}

/// Returns the length of the UTF-8 code point starting at `bytes[i]`,
/// or 1 if the byte is invalid UTF-8 (defensive — we already hold a `&str`).
fn utf8_char_len(bytes: &[u8], i: usize) -> usize {
    match bytes[i] {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => 1,
    }
}

/// True if a byte at a candidate word boundary would prevent the boundary
/// from being placed there -- i.e. it's an ASCII identifier byte or the
/// (continuation or leading) byte of a non-ASCII identifier character.
/// Treating every byte >= 0x80 as identifier-extending is conservative but
/// safe: it makes matches adjacent to any multi-byte UTF-8 character reject.
fn extends_identifier(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80
}

fn byte_range_to_lsp_range(
    source_file: &SourceFile,
    range: std::ops::Range<usize>,
    format: ByteFormat,
) -> lsp_types::Range {
    let (line_start, col_start) = source_file.line_column(range.start, format);
    let (line_end, col_end) = source_file.line_column(range.end, format);
    lsp_types::Range {
        start: lsp_types::Position::new(
            (line_start as u32).saturating_sub(1),
            (col_start as u32).saturating_sub(1),
        ),
        end: lsp_types::Position::new(
            (line_end as u32).saturating_sub(1),
            (col_end as u32).saturating_sub(1),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(kind: DeclarationKind, old: &str, new: &str) -> Vec<(String, String)> {
        accessor_pairs(kind, old, new)
    }

    #[test]
    fn property_matches_get_and_set() {
        let contents = "fn main() {\n    let v = obj.get_count();\n    obj.set_count(v + 1);\n}\n";
        let edits =
            scan_file_contents(contents, &pairs(DeclarationKind::Property, "count", "total"));
        assert_eq!(edits.len(), 2);
        let (range_get, new_get) = &edits[0];
        assert_eq!(&contents[range_get.clone()], "get_count");
        assert_eq!(new_get, "get_total");
        let (range_set, new_set) = &edits[1];
        assert_eq!(&contents[range_set.clone()], "set_count");
        assert_eq!(new_set, "set_total");
    }

    #[test]
    fn callback_matches_invoke_and_on() {
        let contents = "obj.invoke_clicked(); obj.on_clicked(|| {});";
        let edits =
            scan_file_contents(contents, &pairs(DeclarationKind::Callback, "clicked", "pressed"));
        assert_eq!(edits.len(), 2);
        let texts: Vec<_> = edits.iter().map(|(r, _)| &contents[r.clone()]).collect();
        assert!(texts.contains(&"invoke_clicked"));
        assert!(texts.contains(&"on_clicked"));
    }

    #[test]
    fn function_matches_invoke_only() {
        let contents = "obj.invoke_multiply(2);";
        let edits =
            scan_file_contents(contents, &pairs(DeclarationKind::Function, "multiply", "double"));
        assert_eq!(edits.len(), 1);
        assert_eq!(&contents[edits[0].0.clone()], "invoke_multiply");
    }

    #[test]
    fn skip_matches_inside_line_comments() {
        let contents = "let x = obj.get_count(); // get_count again here\n";
        let edits =
            scan_file_contents(contents, &pairs(DeclarationKind::Property, "count", "total"));
        assert_eq!(edits.len(), 1, "only the live call should match, got {edits:?}");
        assert_eq!(&contents[edits[0].0.clone()], "get_count");
    }

    #[test]
    fn skip_matches_inside_block_comments() {
        let contents = "/* old API used obj.get_count() */\nobj.get_count();";
        let edits =
            scan_file_contents(contents, &pairs(DeclarationKind::Property, "count", "total"));
        assert_eq!(edits.len(), 1, "match inside block comment must be skipped, got {edits:?}");
    }

    #[test]
    fn skip_matches_inside_string_literals() {
        let contents = r#"
            let msg = "see obj.get_count for details";
            obj.get_count();
        "#;
        let edits =
            scan_file_contents(contents, &pairs(DeclarationKind::Property, "count", "total"));
        assert_eq!(edits.len(), 1, "match inside string literal must be skipped, got {edits:?}");
    }

    #[test]
    fn string_escapes_do_not_close_string_prematurely() {
        // The escaped \" must not be interpreted as the end of the string.
        let contents = "let s = \"a\\\" get_count not here\"; obj.get_count();";
        let edits =
            scan_file_contents(contents, &pairs(DeclarationKind::Property, "count", "total"));
        assert_eq!(edits.len(), 1);
    }

    #[test]
    fn skip_matches_inside_char_literals() {
        // The `"` inside the char literal must not flip the lexer into
        // String state; otherwise the subsequent real call site is missed.
        let contents = "let c = '\"'; obj.get_count();";
        let edits =
            scan_file_contents(contents, &pairs(DeclarationKind::Property, "count", "total"));
        assert_eq!(edits.len(), 1);
        assert_eq!(&contents[edits[0].0.clone()], "get_count");
    }

    #[test]
    fn lifetime_is_not_consumed_as_char_literal() {
        // `'a` in `&'a T` is a lifetime, not a char literal. The lexer must
        // not enter Char state and swallow the rest of the file.
        let contents = "fn f<'a>(x: &'a u32) { obj.get_count(); }";
        let edits =
            scan_file_contents(contents, &pairs(DeclarationKind::Property, "count", "total"));
        assert_eq!(edits.len(), 1);
    }

    #[test]
    fn escaped_char_literal_is_consumed() {
        // `'\n'`, `'\''`, `'\"'` etc. should all be skipped wholesale.
        let contents = "let n = '\\n'; let q = '\\''; obj.get_count();";
        let edits =
            scan_file_contents(contents, &pairs(DeclarationKind::Property, "count", "total"));
        assert_eq!(edits.len(), 1);
    }

    #[test]
    fn non_ascii_char_after_match_defeats_word_boundary() {
        // Symmetric to non_ascii_char_before_match_defeats_word_boundary --
        // the byte AFTER the match must also be treated as identifier-
        // extending when non-ASCII, so `get_countñ` is rejected.
        let contents = "let xxget_countñ = 1; obj.get_count();";
        let edits =
            scan_file_contents(contents, &pairs(DeclarationKind::Property, "count", "total"));
        assert_eq!(edits.len(), 1, "only the call-site match is valid, got {edits:?}");
        assert_eq!(&contents[edits[0].0.clone()], "get_count");
    }

    #[test]
    fn non_ascii_char_before_match_defeats_word_boundary() {
        // `é` is two bytes (0xC3, 0xA9); the byte before `get_count` is a
        // continuation byte. Without the non-ASCII guard, the old logic
        // would accept the match and corrupt the unrelated identifier.
        let contents = "let xéget_count = 1; obj.get_count();";
        let edits =
            scan_file_contents(contents, &pairs(DeclarationKind::Property, "count", "total"));
        assert_eq!(edits.len(), 1, "only the call-site match is valid, got {edits:?}");
        assert_eq!(&contents[edits[0].0.clone()], "get_count");
    }

    #[test]
    fn raw_identifier_prefix_included_in_match() {
        let contents = "obj.r#get_type();";
        let edits = scan_file_contents(contents, &pairs(DeclarationKind::Property, "type", "kind"));
        assert_eq!(edits.len(), 1);
        // The match swallows `r#` so the rewrite is the bare accessor.
        assert_eq!(&contents[edits[0].0.clone()], "r#get_type");
        assert_eq!(edits[0].1, "get_kind");
    }

    #[test]
    fn word_boundary_rejects_substrings() {
        // `get_counter` and `bget_count` and `get_count_more` must NOT match the rename of `count`.
        let contents =
            "obj.get_counter(); xget_count(); obj.get_count_more(); let _ = my_get_count_;";
        let edits =
            scan_file_contents(contents, &pairs(DeclarationKind::Property, "count", "total"));
        assert!(edits.is_empty(), "matched substrings: {edits:?}");
    }

    #[test]
    fn snake_case_old_name() {
        // Slint source written as kebab-case `my-counter` produces accessor `get_my_counter`.
        let contents = "obj.get_my_counter()";
        let edits = scan_file_contents(
            contents,
            &pairs(DeclarationKind::Property, "my-counter", "my-total"),
        );
        assert_eq!(edits.len(), 1);
        assert_eq!(&contents[edits[0].0.clone()], "get_my_counter");
        assert_eq!(edits[0].1, "get_my_total");
    }

    #[test]
    fn multiple_call_sites() {
        let contents = "obj.get_x(); other.get_x(); a.get_x() + b.get_x();";
        let edits = scan_file_contents(contents, &pairs(DeclarationKind::Property, "x", "y"));
        assert_eq!(edits.len(), 4);
    }

    #[test]
    fn collect_files_skips_target_and_filters_by_extension() {
        let tmp = tempdir();
        std::fs::write(tmp.path().join("a.rs"), "").unwrap();
        std::fs::write(tmp.path().join("b.cpp"), "").unwrap();
        std::fs::write(tmp.path().join("ignored.txt"), "").unwrap();
        std::fs::create_dir(tmp.path().join("target")).unwrap();
        std::fs::write(tmp.path().join("target").join("c.rs"), "").unwrap();
        std::fs::create_dir(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src").join("d.rs"), "").unwrap();

        let mut out = Vec::new();
        collect_files(tmp.path(), &mut out, 100).unwrap();
        let names: Vec<_> =
            out.iter().map(|p| p.file_name().unwrap().to_str().unwrap().to_string()).collect();
        assert!(names.contains(&"a.rs".to_string()));
        assert!(names.contains(&"b.cpp".to_string()));
        assert!(names.contains(&"d.rs".to_string()));
        assert!(!names.contains(&"ignored.txt".to_string()));
        assert!(!names.contains(&"c.rs".to_string()), "target/ should be skipped");
    }

    #[test]
    fn end_to_end_scan_produces_workspace_edits() {
        let tmp = tempdir();
        // Use `tmp.path()` directly rather than canonicalize: on Windows
        // `canonicalize()` returns a `\\?\` UNC path which doesn't round-trip
        // through `Url::from_file_path` / `Url::to_file_path` (the latter
        // strips the prefix), breaking the workspace-folder prefix match.
        let root = tmp.path();
        std::fs::write(
            root.join("main.rs"),
            "fn main() { let v = obj.get_count(); obj.set_count(v); }\n",
        )
        .unwrap();
        std::fs::create_dir(root.join("ui")).unwrap();
        std::fs::write(root.join("ui").join("app.slint"), "// fake\n").unwrap();

        let folders =
            vec![WorkspaceFolder { uri: Url::from_file_path(root).unwrap(), name: "test".into() }];

        let edits = scan_host_language_accessors(
            &folders,
            &root.join("ui").join("app.slint"),
            DeclarationKind::Property,
            "count",
            "total",
            ByteFormat::Utf16,
            ScanBounds::default(),
        )
        .unwrap();
        assert_eq!(edits.len(), 2);
        assert!(edits.iter().all(|e| e.url.path().ends_with("main.rs")));
        let texts: Vec<_> = edits.iter().map(|e| e.edit.new_text.clone()).collect();
        assert!(texts.contains(&"get_total".to_string()));
        assert!(texts.contains(&"set_total".to_string()));
    }

    #[test]
    #[allow(deprecated)]
    fn resolve_workspace_folders_prefers_workspace_folders() {
        let tmp = tempdir();
        let mut init = InitializeParams::default();
        let folder =
            WorkspaceFolder { uri: Url::from_file_path(tmp.path()).unwrap(), name: "ws".into() };
        init.workspace_folders = Some(vec![folder.clone()]);
        init.root_uri = Some(Url::parse("file:///should/not/be/used").unwrap());
        let resolved = resolve_workspace_folders(&init);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].uri, folder.uri);
    }

    #[test]
    #[allow(deprecated)]
    fn resolve_workspace_folders_falls_back_to_root_uri() {
        let tmp = tempdir();
        let mut init = InitializeParams::default();
        let uri = Url::from_file_path(tmp.path()).unwrap();
        init.workspace_folders = None;
        init.root_uri = Some(uri.clone());
        let resolved = resolve_workspace_folders(&init);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].uri, uri);
    }

    #[test]
    #[allow(deprecated)]
    fn resolve_workspace_folders_falls_back_to_root_path() {
        let tmp = tempdir();
        let init = InitializeParams {
            workspace_folders: Some(Vec::new()), // empty, not None
            root_uri: None,
            root_path: Some(tmp.path().to_string_lossy().into_owned()),
            ..Default::default()
        };
        let resolved = resolve_workspace_folders(&init);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].uri.to_file_path().unwrap(), tmp.path());
    }

    #[test]
    fn unsaved_buffer_path_still_resolves_to_workspace_folder() {
        // Path doesn't exist on disk (simulating an untitled buffer or an
        // unsaved new file) but is still under a configured workspace folder.
        // Pre-fix this returned OutsideWorkspace via canonicalize().
        let tmp = tempdir();
        let folders = vec![WorkspaceFolder {
            uri: Url::from_file_path(tmp.path()).unwrap(),
            name: "ws".into(),
        }];
        let unsaved = tmp.path().join("never-saved.slint");
        assert!(!unsaved.exists());
        let folder = workspace_folder_for(&folders, &unsaved).expect("should match");
        assert_eq!(folder, tmp.path().to_path_buf());
    }

    #[test]
    fn outside_workspace_fails_closed() {
        let tmp_workspace = tempdir();
        let tmp_outside = tempdir();
        let folders = vec![WorkspaceFolder {
            uri: Url::from_file_path(tmp_workspace.path()).unwrap(),
            name: "test".into(),
        }];
        std::fs::write(tmp_outside.path().join("orphan.slint"), "// fake").unwrap();
        let result = scan_host_language_accessors(
            &folders,
            &tmp_outside.path().join("orphan.slint"),
            DeclarationKind::Property,
            "x",
            "y",
            ByteFormat::Utf16,
            ScanBounds::default(),
        );
        match result {
            Err(HostLanguageScanError::OutsideWorkspace) => {}
            Err(other) => panic!("expected OutsideWorkspace, got {other:?}"),
            Ok(_) => panic!("expected OutsideWorkspace error, got Ok"),
        }
    }

    #[test]
    fn collect_files_extension_match_is_case_insensitive() {
        let tmp = tempdir();
        std::fs::write(tmp.path().join("Main.RS"), "").unwrap();
        std::fs::write(tmp.path().join("Window.HPP"), "").unwrap();
        std::fs::write(tmp.path().join("README.md"), "").unwrap();
        let mut out = Vec::new();
        collect_files(tmp.path(), &mut out, 100).unwrap();
        let names: Vec<_> =
            out.iter().map(|p| p.file_name().unwrap().to_str().unwrap().to_string()).collect();
        assert!(names.contains(&"Main.RS".to_string()), "got {names:?}");
        assert!(names.contains(&"Window.HPP".to_string()), "got {names:?}");
        assert!(!names.contains(&"README.md".to_string()), "got {names:?}");
    }

    #[cfg(unix)]
    #[test]
    fn collect_files_does_not_follow_symlinks() {
        use std::os::unix::fs::symlink;
        let tmp = tempdir();
        // Create a real file and a symlinked directory cycle.
        std::fs::write(tmp.path().join("real.rs"), "").unwrap();
        std::fs::create_dir(tmp.path().join("sub")).unwrap();
        std::fs::write(tmp.path().join("sub").join("nested.rs"), "").unwrap();
        // Symlink cycle: sub/back -> ..
        symlink("..", tmp.path().join("sub").join("back")).unwrap();
        // Symlink to a file that would otherwise match.
        symlink(tmp.path().join("real.rs"), tmp.path().join("linked.rs")).unwrap();

        let mut out = Vec::new();
        collect_files(tmp.path(), &mut out, 100).unwrap();
        let names: Vec<_> =
            out.iter().map(|p| p.file_name().unwrap().to_str().unwrap().to_string()).collect();
        // The real files are present, the symlinked dir wasn't recursed
        // (no stack overflow), and the symlink to a file was skipped.
        assert!(names.contains(&"real.rs".to_string()));
        assert!(names.contains(&"nested.rs".to_string()));
        assert!(!names.contains(&"linked.rs".to_string()), "symlink should be skipped");
    }

    #[test]
    fn collect_files_respects_max_files() {
        let tmp = tempdir();
        for i in 0..10 {
            std::fs::write(tmp.path().join(format!("f{i}.rs")), "").unwrap();
        }
        let mut out = Vec::new();
        let err = collect_files(tmp.path(), &mut out, 3).unwrap_err();
        assert!(matches!(err, HostLanguageScanError::TooManyFiles { limit: 3 }));
    }

    /// A throwaway directory that cleans itself up on drop.
    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn tempdir() -> TempDir {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "slint-lsp-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir(&path).unwrap();
        TempDir { path }
    }
}
