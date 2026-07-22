// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore bget countñ xéget xget xñget xxget

//! Scan the workspace for identifiers that match Slint-generated Rust/C++
//! property, callback, and function accessors.
//!
//! The scanner tokenizes identifiers using Unicode XID_Start / XID_Continue
//! (via [`icu_properties`]) and matches whole identifiers against a small
//! lookup table derived from [`i_slint_compiler::generator::accessor_names`].
//! It does not parse Rust or C++. Cross-component accessor collisions are
//! possible by construction. The client may apply the `workspace/applyEdit`
//! request immediately, so users should inspect the resulting changes with
//! source control.

#![cfg(not(target_arch = "wasm32"))]

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use i_slint_compiler::diagnostics::{ByteFormat, SourceFile, SourceFileInner};
use i_slint_compiler::generator::accessor_names::{self, DeclarationKind};
use lsp_types::{InitializeParams, TextEdit, Url, WorkspaceFolder};

use super::SingleTextEdit;

/// Host-language file extensions the scanner considers.
const HOST_FILE_EXTENSIONS: &[&str] = &["rs", "cpp", "cc", "cxx", "h", "hpp", "hh"];

// TODO: `.gitignore` semantics would be the right way to pick which files to
// scan -- it already encodes what is and isn't source. The Language Server
// Protocol has no server-side query for the client's set of "active" or
// "ignored" files; the hard-coded skip list below is the closest
// approximation we can ship today. Replace this when an LSP extension or
// client capability for that exists, or switch to the `ignore` crate if we
// accept its filesystem-walk cost.
const SKIP_DIRS: &[&str] = &["target", "build", "out", "dist", "node_modules", ".git"];

/// Hard limits on scan work. The host-language follow-up runs in a
/// spawned task, but we still bound it so a misconfigured workspace can't
/// hold the LSP open indefinitely.
#[derive(Debug, Clone, Copy)]
pub struct ScanBounds {
    /// Maximum number of host-language files inspected.
    pub max_files: usize,
    /// Maximum size in bytes of a single file. Larger files are skipped
    /// (not an error — likely generated code or vendored sources).
    pub max_file_bytes: u64,
}

impl ScanBounds {
    pub const DEFAULT: Self = Self { max_files: 5_000, max_file_bytes: 1 << 20 /* 1 MiB */ };
}

/// Failure modes for the scanner. The Rename follow-up in `language.rs`
/// soft-degrades these to a `tracing::warn!` and a `window/showMessage`,
/// so callers should treat them as advisory rather than fatal.
#[derive(Debug)]
pub enum HostLanguageScanError {
    /// The client did not report any open workspace folders, so the
    /// scanner has nowhere to walk.
    NoWorkspaceFolders,
    /// The scan would inspect more than `bounds.max_files` files across all
    /// configured workspace folders combined.
    TooManyFiles { limit: usize },
}

impl std::fmt::Display for HostLanguageScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoWorkspaceFolders => f.write_str(
                "Cannot scan host-language files: the client did not report \
                 any open workspace folders",
            ),
            Self::TooManyFiles { limit } => write!(
                f,
                "Cannot scan host-language files: workspace exceeds {limit} files. \
                 Narrow the workspace folders.",
            ),
        }
    }
}

impl std::error::Error for HostLanguageScanError {}

/// Walk every configured workspace folder for `.rs`/`.cpp`/... files, replace
/// any occurrence of the old accessor names derived from `(kind, old_name)`
/// with the corresponding new accessor names derived from `(kind, new_name)`,
/// and return the resulting per-file edits.
///
/// Each [`WorkspaceFolder`] is walked independently; per the LSP spec, an
/// editor can attach unrelated directories as separate workspace folders to
/// the same server, and matching identifiers may live in any folder.
pub fn search_replace_host_language_accessors(
    workspace_folders: &[WorkspaceFolder],
    kind: DeclarationKind,
    old_name: &str,
    new_name: &str,
    format: ByteFormat,
    bounds: ScanBounds,
) -> Result<Vec<SingleTextEdit>, HostLanguageScanError> {
    if workspace_folders.is_empty() {
        return Err(HostLanguageScanError::NoWorkspaceFolders);
    }

    let pairs = accessor_pairs(kind, old_name, new_name);
    let accessors: HashMap<&str, &str> =
        pairs.iter().map(|(o, n)| (o.as_str(), n.as_str())).collect();

    let mut files = HashSet::new();
    for folder in workspace_folders {
        let Ok(folder_path) = folder.uri.to_file_path() else { continue };
        collect_files(&folder_path, &mut files, bounds.max_files)?;
    }
    let mut files: Vec<_> = files.into_iter().collect();
    files.sort();

    let mut edits = Vec::new();
    for path in files {
        let metadata = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if metadata.len() > bounds.max_file_bytes {
            continue;
        }
        // TODO: Reading the file directly here may cause issues with files that are open and edited in the
        // editor.
        // Unfortunately, the LSP spec does not easily allow us to request the contents of a certain
        // file. So we would have to keep track of all the workspace edits, even if the files are
        // not relevant for the LSP otherwise.
        // For now, we'll just not do this and hope that the files on disk are reasonably up to
        // date.
        //
        // If this causes issues for our users, we should add another document cache that stores the
        // contents of all open files, even if they're not Slint files and then read from that.
        let contents = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => continue, // non-UTF-8 or unreadable; skip
        };

        let file_edits = search_replace_file_contents(&contents, &accessors);
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

/// Build the set of workspace folders the scanner should consider, falling
/// back through the LSP-deprecated-but-widely-used `root_uri` / `root_path`
/// so editors that initialize the server in single-folder mode still get
/// host-language scanning.
///
/// Returns an empty Vec if no folder could be derived; the scanner will then
/// fail with `NoWorkspaceFolders`.
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

/// Ask the client for its current workspace folders via the
/// `workspace/workspaceFolders` request, falling back to the cached
/// [`InitializeParams`] when the client doesn't advertise support.
///
/// Folders can change after initialization (the user opens or closes folders
/// in their editor); using the cached `init_param.workspace_folders` would
/// scan a stale set and either miss files or scan files no longer in the
/// workspace. Clients that don't advertise the capability never send change
/// notifications either, so the InitializeParams snapshot is the best we can
/// do for them.
pub async fn current_workspace_folders(
    server_notifier: &crate::ServerNotifier,
    init_param: &InitializeParams,
) -> Vec<WorkspaceFolder> {
    let supports_query = init_param
        .capabilities
        .workspace
        .as_ref()
        .and_then(|w| w.workspace_folders)
        .unwrap_or(false);
    if supports_query
        && let Ok(fut) =
            server_notifier.send_request::<lsp_types::request::WorkspaceFoldersRequest>(())
        && let Ok(Some(folders)) = fut.await
        && !folders.is_empty()
    {
        return folders;
    }
    resolve_workspace_folders(init_param)
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

/// Iterative breadth-first walk of `root`, appending host-language file paths
/// into `out`. Returns `TooManyFiles` if `max_files` is exceeded.
///
/// Iterative rather than recursive so we don't burn stack on arbitrarily deep
/// directory trees, and so a future change can fan the queue out to multiple
/// worker tasks. Unreadable directories are silently skipped (don't fail the
/// whole rename on a permission error in some sibling folder).
///
/// Does not follow symlinks: an `entry.file_type()` whose `is_symlink()` is
/// true is skipped entirely. This avoids unbounded traversal through symlink
/// cycles (vendored deps, sibling-crate links) at the cost of not scanning
/// symlinked source trees -- users who want those scanned should add the
/// real directory as a workspace folder.
fn collect_files(
    root: &Path,
    out: &mut HashSet<PathBuf>,
    max_files: usize,
) -> Result<(), HostLanguageScanError> {
    let mut queue: VecDeque<PathBuf> = VecDeque::new();
    queue.push_back(root.to_path_buf());
    while let Some(dir) = queue.pop_front() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) => {
                tracing::debug!(
                    "host-language scan: skipping unreadable dir {}: {e}",
                    dir.display()
                );
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    tracing::debug!(
                        "host-language scan: skipping unreadable entry under {}: {e}",
                        dir.display()
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
                queue.push_back(path);
            } else if file_type.is_file() && has_host_extension(&path) {
                if out.contains(&path) {
                    continue;
                }
                if out.len() >= max_files {
                    return Err(HostLanguageScanError::TooManyFiles { limit: max_files });
                }
                out.insert(path);
            }
        }
    }
    Ok(())
}

fn has_host_extension(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else { return false };
    // Case-insensitive: Windows filesystems are case-insensitive by default
    // and projects in the wild ship files like `Window.HPP`.
    HOST_FILE_EXTENSIONS.iter().any(|known| known.eq_ignore_ascii_case(ext))
}

/// Tokenize `contents` into Unicode identifiers and emit a replacement for
/// every identifier whose text matches a key in `accessors`.
///
/// This is a flat textual scan with no awareness of comments, string
/// literals, raw strings, lifetimes, or any other language construct. The
/// rename is *textual by design*. The client may apply the
/// `workspace/applyEdit` request without showing a preview.
/// Selectively skipping some contexts (comments, strings) while still
/// rewriting unrelated types that happen to share the accessor name was
/// inconsistent enough to mislead users about what the tool actually does;
/// the simpler flat scan is honest about the trade-off.
///
/// Identifiers are recognized via Unicode `XID_Start` / `XID_Continue`
/// (matching Rust's identifier rules and the wider modern-language family),
/// so `xñget_count` becomes a single identifier and equality-rejects against
/// `get_count` rather than producing a partial match.
fn search_replace_file_contents(
    contents: &str,
    accessors: &HashMap<&str, &str>,
) -> Vec<(std::ops::Range<usize>, String)> {
    let xid_start = icu_properties::CodePointSetData::new::<icu_properties::props::XidStart>();
    let xid_continue =
        icu_properties::CodePointSetData::new::<icu_properties::props::XidContinue>();

    let mut matches = Vec::new();
    let mut chars = contents.char_indices().peekable();
    while let Some(&(start, ch)) = chars.peek() {
        if !is_identifier_start(ch, xid_start) {
            chars.next();
            continue;
        }
        // Consume one identifier.
        let mut end = start + ch.len_utf8();
        chars.next();
        while let Some(&(idx, c)) = chars.peek() {
            if is_identifier_continue(c, xid_continue) {
                end = idx + c.len_utf8();
                chars.next();
            } else {
                break;
            }
        }
        let Some(&new) = accessors.get(&contents[start..end]) else { continue };

        matches.push((start..end, new.to_string()));
    }
    matches
}

fn is_identifier_start(ch: char, xid_start: icu_properties::CodePointSetDataBorrowed<'_>) -> bool {
    ch == '_' || xid_start.contains(ch)
}

fn is_identifier_continue(
    ch: char,
    xid_continue: icu_properties::CodePointSetDataBorrowed<'_>,
) -> bool {
    xid_continue.contains(ch)
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

    /// Build the owned accessor pairs and emit edits in one call -- the
    /// scanner takes a `HashMap<&str, &str>` borrowing into those pairs.
    fn scan(
        contents: &str,
        kind: DeclarationKind,
        old: &str,
        new: &str,
    ) -> Vec<(std::ops::Range<usize>, String)> {
        let pairs = accessor_pairs(kind, old, new);
        let map: HashMap<&str, &str> =
            pairs.iter().map(|(o, n)| (o.as_str(), n.as_str())).collect();
        search_replace_file_contents(contents, &map)
    }

    #[test]
    fn property_matches_get_and_set() {
        let contents = "fn main() {\n    let v = obj.get_count();\n    obj.set_count(v + 1);\n}\n";
        let edits = scan(contents, DeclarationKind::Property, "count", "total");
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
        let edits = scan(contents, DeclarationKind::Callback, "clicked", "pressed");
        assert_eq!(edits.len(), 2);
        let texts: Vec<_> = edits.iter().map(|(r, _)| &contents[r.clone()]).collect();
        assert!(texts.contains(&"invoke_clicked"));
        assert!(texts.contains(&"on_clicked"));
    }

    #[test]
    fn function_matches_invoke_only() {
        let contents = "obj.invoke_multiply(2);";
        let edits = scan(contents, DeclarationKind::Function, "multiply", "double");
        assert_eq!(edits.len(), 1);
        assert_eq!(&contents[edits[0].0.clone()], "invoke_multiply");
    }

    #[test]
    fn textual_scan_does_match_inside_comments_and_strings() {
        // The scanner is flat textual and deliberately doesn't skip comments
        // or string literals.
        let contents = r#"
            // see obj.get_count for details
            let msg = "obj.get_count is great";
            obj.get_count();
        "#;
        let edits = scan(contents, DeclarationKind::Property, "count", "total");
        assert_eq!(edits.len(), 3, "all three textual matches should fire, got {edits:?}");
    }

    #[test]
    fn non_ascii_char_after_match_defeats_word_boundary() {
        // The XID tokenizer treats `ñ` as identifier-continue, so
        // `get_countñ` is one identifier and equality-rejects against
        // `get_count`.
        let contents = "let xxget_countñ = 1; obj.get_count();";
        let edits = scan(contents, DeclarationKind::Property, "count", "total");
        assert_eq!(edits.len(), 1, "only the standalone match is valid, got {edits:?}");
        assert_eq!(&contents[edits[0].0.clone()], "get_count");
    }

    #[test]
    fn non_ascii_char_before_match_defeats_word_boundary() {
        // `é` is XID_Continue, so `xéget_count` is one identifier and the
        // accessor lookup rejects it cleanly.
        let contents = "let xéget_count = 1; obj.get_count();";
        let edits = scan(contents, DeclarationKind::Property, "count", "total");
        assert_eq!(edits.len(), 1, "only the standalone match is valid, got {edits:?}");
        assert_eq!(&contents[edits[0].0.clone()], "get_count");
    }

    #[test]
    fn raw_identifier_prefix_is_preserved() {
        let contents = "obj.r#get_type();";
        let edits = scan(contents, DeclarationKind::Property, "type", "kind");
        assert_eq!(edits.len(), 1);
        assert_eq!(&contents[edits[0].0.clone()], "get_type");
        assert_eq!(edits[0].1, "get_kind");
    }

    #[test]
    fn word_boundary_rejects_substrings() {
        // `get_counter` and `bget_count` and `get_count_more` must NOT match the rename of `count`.
        let contents =
            "obj.get_counter(); xget_count(); obj.get_count_more(); let _ = my_get_count_;";
        let edits = scan(contents, DeclarationKind::Property, "count", "total");
        assert!(edits.is_empty(), "matched substrings: {edits:?}");
    }

    #[test]
    fn snake_case_old_name() {
        // Slint source written as kebab-case `my-counter` produces accessor `get_my_counter`.
        let contents = "obj.get_my_counter()";
        let edits = scan(contents, DeclarationKind::Property, "my-counter", "my-total");
        assert_eq!(edits.len(), 1);
        assert_eq!(&contents[edits[0].0.clone()], "get_my_counter");
        assert_eq!(edits[0].1, "get_my_total");
    }

    #[test]
    fn multiple_call_sites() {
        let contents = "obj.get_x(); other.get_x(); a.get_x() + b.get_x();";
        let edits = scan(contents, DeclarationKind::Property, "x", "y");
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

        let mut out = HashSet::new();
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
            "// 😀 obj.get_count();\nfn main() { obj.set_count(1); }\n",
        )
        .unwrap();
        std::fs::create_dir(root.join("ui")).unwrap();
        std::fs::write(root.join("ui").join("app.slint"), "// fake\n").unwrap();

        let folders =
            vec![WorkspaceFolder { uri: Url::from_file_path(root).unwrap(), name: "test".into() }];

        let edits = search_replace_host_language_accessors(
            &folders,
            DeclarationKind::Property,
            "count",
            "total",
            ByteFormat::Utf16,
            ScanBounds::DEFAULT,
        )
        .unwrap();
        assert_eq!(edits.len(), 2);
        assert!(edits.iter().all(|e| e.url.path().ends_with("main.rs")));
        let texts: Vec<_> = edits.iter().map(|e| e.edit.new_text.clone()).collect();
        assert!(texts.contains(&"get_total".to_string()));
        assert!(texts.contains(&"set_total".to_string()));
        let getter = edits.iter().find(|e| e.edit.new_text == "get_total").unwrap();
        assert_eq!(getter.edit.range.start, lsp_types::Position::new(0, 10));
        assert_eq!(getter.edit.range.end, lsp_types::Position::new(0, 19));
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
    fn empty_workspace_fails_closed() {
        let result = search_replace_host_language_accessors(
            &[],
            DeclarationKind::Property,
            "x",
            "y",
            ByteFormat::Utf16,
            ScanBounds::DEFAULT,
        );
        match result {
            Err(HostLanguageScanError::NoWorkspaceFolders) => {}
            Err(other) => panic!("expected NoWorkspaceFolders, got {other:?}"),
            Ok(_) => panic!("expected NoWorkspaceFolders error, got Ok"),
        }
    }

    #[test]
    fn multiple_workspace_folders_scan_independently() {
        let folder_a = tempdir();
        let folder_b = tempdir();
        std::fs::write(folder_a.path().join("a.rs"), "fn f() { x.get_count(); }\n").unwrap();
        std::fs::write(folder_b.path().join("b.rs"), "fn g() { y.set_count(0); }\n").unwrap();
        let folders = vec![
            WorkspaceFolder {
                uri: Url::from_file_path(folder_a.path()).unwrap(),
                name: "a".into(),
            },
            WorkspaceFolder {
                uri: Url::from_file_path(folder_b.path()).unwrap(),
                name: "b".into(),
            },
        ];

        let edits = search_replace_host_language_accessors(
            &folders,
            DeclarationKind::Property,
            "count",
            "total",
            ByteFormat::Utf16,
            ScanBounds::DEFAULT,
        )
        .unwrap();

        // Both folders contributed one edit.
        let urls: std::collections::HashSet<_> = edits.iter().map(|e| e.url.clone()).collect();
        assert_eq!(urls.len(), 2);
    }

    #[test]
    fn overlapping_workspace_folders_count_unique_files() {
        let root = tempdir();
        let nested = root.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(root.path().join("root.rs"), "obj.get_count();").unwrap();
        std::fs::write(nested.join("nested.rs"), "obj.set_count(1);").unwrap();
        let folders = vec![
            WorkspaceFolder { uri: Url::from_file_path(root.path()).unwrap(), name: "root".into() },
            WorkspaceFolder { uri: Url::from_file_path(&nested).unwrap(), name: "nested".into() },
        ];

        let edits = search_replace_host_language_accessors(
            &folders,
            DeclarationKind::Property,
            "count",
            "total",
            ByteFormat::Utf16,
            ScanBounds { max_files: 2, max_file_bytes: 1024 },
        )
        .unwrap();

        assert_eq!(edits.len(), 2);
        let urls: HashSet<_> = edits.iter().map(|e| &e.url).collect();
        assert_eq!(urls.len(), 2);
    }

    #[test]
    fn files_larger_than_the_size_bound_are_skipped() {
        let root = tempdir();
        std::fs::write(root.path().join("main.rs"), "obj.get_count();").unwrap();
        let folders = vec![WorkspaceFolder {
            uri: Url::from_file_path(root.path()).unwrap(),
            name: "root".into(),
        }];

        let edits = search_replace_host_language_accessors(
            &folders,
            DeclarationKind::Property,
            "count",
            "total",
            ByteFormat::Utf16,
            ScanBounds { max_files: 1, max_file_bytes: 4 },
        )
        .unwrap();

        assert!(edits.is_empty());
    }

    #[test]
    fn collect_files_extension_match_is_case_insensitive() {
        let tmp = tempdir();
        std::fs::write(tmp.path().join("Main.RS"), "").unwrap();
        std::fs::write(tmp.path().join("Window.HPP"), "").unwrap();
        std::fs::write(tmp.path().join("README.md"), "").unwrap();
        let mut out = HashSet::new();
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

        let mut out = HashSet::new();
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
        let mut out = HashSet::new();
        let err = collect_files(tmp.path(), &mut out, 3).unwrap_err();
        assert!(matches!(err, HostLanguageScanError::TooManyFiles { limit: 3 }));
    }

    fn tempdir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }
}
