// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Integration tests for the classification and search portions of the
// cross-language rename pipeline:
// `find_declaration_node` -> `rename()` -> `host_language_classification()`
// -> `search_replace_host_language_accessors()`.
// Tests for the interactive LSP requests live in `language.rs`.

use std::collections::HashMap;
use std::path::PathBuf;

use i_slint_compiler::diagnostics::{BuildDiagnostics, ByteFormat};
use i_slint_compiler::parser::{SyntaxKind, SyntaxToken};
use i_slint_editor_preview as editor_preview;
use i_slint_editor_preview::editing::rename_component::find_declaration_node;
use lsp_types::{Url, WorkspaceEdit, WorkspaceFolder};

use super::{ScanBounds, search_replace_host_language_accessors};

/// Throwaway tempdir; cleans itself up on drop.
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn tempdir() -> TempDir {
    use std::sync::atomic::{AtomicUsize, Ordering};
    // Per-test serial -- bare timestamps collide when many tests start
    // in the same nanosecond under cargo's parallel runner.
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let serial = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut path = std::env::temp_dir();
    path.push(format!(
        "slint-lsp-rename-roundtrip-{}-{}-{serial}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    ));
    std::fs::create_dir(&path).unwrap();
    TempDir { path }
}

/// Build a workspace under `tempdir`: writes the given `.slint` files and
/// `.rs` files to disk, then loads the slint files into a `DocumentCache`.
/// The first slint file in `slint_files` is treated as the "main" one
/// and its URL is returned.
///
/// The second arg to load_url is the source version; we use 1 so that
/// the produced WorkspaceEdit has a non-None document version on the
/// slint side -- which the host scanner deliberately leaves as None for
/// `.rs`/`.cpp` files (those aren't in the cache).
fn setup(
    tmp: &TempDir,
    slint_files: &[(&str, &str)],
    host_files: &[(&str, &str)],
) -> (editor_preview::DocumentCache, Url, Vec<WorkspaceFolder>) {
    // Write all host-language files to disk. The scanner is language-
    // agnostic at this layer (same accessor strings emitted for Rust and
    // C++), so `host_files` may contain `.rs`, `.cpp`, `.h`, etc.
    for (rel_path, contents) in host_files {
        let full = tmp.path().join(rel_path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&full, contents).unwrap();
    }

    // Build the DocumentCache.
    let config = editor_preview::document_cache::CompilerConfiguration {
        style: Some("fluent".into()),
        ..Default::default()
    };
    let mut cache = editor_preview::DocumentCache::new(config);
    spin_on::spin_on(cache.preload_builtins());

    // Write each slint file and load it into the cache.
    let mut main_url: Option<Url> = None;
    for (i, (rel_path, contents)) in slint_files.iter().enumerate() {
        let full = tmp.path().join(rel_path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&full, contents).unwrap();
        let url = Url::from_file_path(&full).unwrap();
        let mut diag = BuildDiagnostics::default();
        spin_on::spin_on(cache.load_url(&url, Some(1), contents.to_string(), &mut diag)).unwrap();
        assert!(
            !diag.has_errors(),
            "slint diagnostics in {rel_path}: {:?}",
            diag.iter().map(|d| d.message().to_string()).collect::<Vec<_>>(),
        );
        if i == 0 {
            main_url = Some(url);
        }
    }

    let folders =
        vec![WorkspaceFolder { uri: Url::from_file_path(tmp.path()).unwrap(), name: "ws".into() }];
    (cache, main_url.unwrap(), folders)
}

/// Locate the token immediately before a `/* <- TEST_ME<suffix> */` marker
/// in the document at `url`.
#[track_caller]
fn find_token_in_url(
    document_cache: &editor_preview::DocumentCache,
    url: &Url,
    suffix: &str,
) -> SyntaxToken {
    let document = document_cache.get_document(url).unwrap();
    let document = document.node.as_ref().unwrap();
    let offset = document.text().to_string().find(&format!("<- TEST_ME{suffix} ")).unwrap() as u32;
    let comment = document.token_at_offset(offset.into()).next().unwrap();
    assert_eq!(comment.kind(), SyntaxKind::Comment);
    let mut token = comment.prev_token();
    while let Some(t) = &token {
        if ![SyntaxKind::Comment, SyntaxKind::Eof, SyntaxKind::Whitespace].contains(&t.kind()) {
            break;
        }
        token = t.prev_token();
    }
    token.unwrap()
}

/// Compose Slint and host-language edits for concise assertions.
/// Production sends these as two independent workspace edits.
fn perform_rename(
    document_cache: &editor_preview::DocumentCache,
    folders: &[WorkspaceFolder],
    slint_url: &Url,
    token_suffix: &str,
    new_name: &str,
) -> WorkspaceEdit {
    let token = find_token_in_url(document_cache, slint_url, token_suffix);
    let decl =
        find_declaration_node(document_cache, &token).expect("declaration node must be found");
    let mut workspace_edit = decl.rename(document_cache, new_name).expect("rename");

    if let Some(info) = decl.host_language_classification(document_cache)
        && i_slint_compiler::parser::normalize_identifier(new_name) != info.old_name
    {
        let host_edits = search_replace_host_language_accessors(
            folders,
            info.kind,
            &info.old_name,
            new_name,
            ByteFormat::Utf16,
            ScanBounds::DEFAULT,
        )
        .expect("scan");
        if !host_edits.is_empty() {
            let host_we =
                editor_preview::editing::create_workspace_edit_from_single_text_edits(host_edits);
            editor_preview::editing::merge_workspace_edits(&mut workspace_edit, host_we);
        }
    }
    workspace_edit
}

/// Collect the (URL -> [new_text]) map from a WorkspaceEdit's
/// `document_changes::Edits` form, for assertion ergonomics.
fn edits_by_url(edit: &WorkspaceEdit) -> HashMap<Url, Vec<String>> {
    let mut out: HashMap<Url, Vec<String>> = HashMap::new();
    if let Some(lsp_types::DocumentChanges::Edits(v)) = edit.document_changes.as_ref() {
        for tde in v {
            let url = tde.text_document.uri.clone();
            for e in &tde.edits {
                if let lsp_types::OneOf::Left(te) = e {
                    out.entry(url.clone()).or_default().push(te.new_text.clone());
                }
            }
        }
    }
    out
}

fn rs_url(tmp: &TempDir, rel: &str) -> Url {
    Url::from_file_path(tmp.path().join(rel)).unwrap()
}

fn slint_url(tmp: &TempDir, rel: &str) -> Url {
    Url::from_file_path(tmp.path().join(rel)).unwrap()
}

/// Property rename: `count` -> `total` in .slint should rewrite the
/// `.slint` declaration AND the `get_count`/`set_count` accessors in
/// the .rs file.
#[test]
fn property_rename_rewrites_slint_and_rust() {
    let tmp = tempdir();
    let (cache, url, folders) = setup(
        &tmp,
        &[(
            "ui/app.slint",
            r#"
export component App inherits Window {
in property <int> count /* <- TEST_ME */;
}
            "#,
        )],
        &[("src/main.rs", "fn main() { let v = obj.get_count(); obj.set_count(v + 1); }\n")],
    );

    let edit = perform_rename(&cache, &folders, &url, "", "total");
    let by_url = edits_by_url(&edit);
    let rs = rs_url(&tmp, "src/main.rs");
    let slint = slint_url(&tmp, "ui/app.slint");
    assert!(by_url.contains_key(&slint), "missing .slint edits");
    let rust_edits = by_url.get(&rs).expect("missing .rs edits");
    assert!(rust_edits.contains(&"get_total".to_string()), "got: {rust_edits:?}");
    assert!(rust_edits.contains(&"set_total".to_string()), "got: {rust_edits:?}");
}

/// Callback rename: `clicked` -> `pressed` should rewrite `invoke_clicked`
/// and `on_clicked` in the .rs file.
#[test]
fn callback_rename_rewrites_invoke_and_on() {
    let tmp = tempdir();
    let (cache, url, folders) = setup(
        &tmp,
        &[(
            "ui/app.slint",
            r#"
export component App inherits Window {
callback clicked /* <- TEST_ME */();
}
            "#,
        )],
        &[("src/main.rs", "fn main() { obj.invoke_clicked(); obj.on_clicked(|| {}); }\n")],
    );

    let edit = perform_rename(&cache, &folders, &url, "", "pressed");
    let by_url = edits_by_url(&edit);
    let rust_edits = by_url.get(&rs_url(&tmp, "src/main.rs")).expect("missing .rs edits");
    assert!(rust_edits.contains(&"invoke_pressed".to_string()), "{rust_edits:?}");
    assert!(rust_edits.contains(&"on_pressed".to_string()), "{rust_edits:?}");
}

/// Function rename: `bump` -> `add` should rewrite `invoke_bump` (no
/// `on_` for functions).
#[test]
fn function_rename_rewrites_invoke_only() {
    let tmp = tempdir();
    let (cache, url, folders) = setup(
        &tmp,
        &[(
            "ui/app.slint",
            r#"
export component App inherits Window {
public function bump /* <- TEST_ME */(by: int) -> int { by + 1 }
}
            "#,
        )],
        &[("src/main.rs", "fn main() { let _ = obj.invoke_bump(2); }\n")],
    );

    let edit = perform_rename(&cache, &folders, &url, "", "add");
    let by_url = edits_by_url(&edit);
    let rust_edits = by_url.get(&rs_url(&tmp, "src/main.rs")).expect("missing .rs edits");
    assert_eq!(rust_edits, &vec!["invoke_add".to_string()]);
}

/// Private property: classifier returns None, scanner is skipped, only
/// the .slint edit is produced.
#[test]
fn private_property_does_not_touch_host_files() {
    let tmp = tempdir();
    let (cache, url, folders) = setup(
        &tmp,
        &[(
            "ui/app.slint",
            r#"
export component App inherits Window {
private property <int> secret /* <- TEST_ME */;
}
            "#,
        )],
        &[(
            "src/main.rs",
            // Even though the text 'get_secret' appears, it shouldn't
            // be rewritten because the slint property is private.
            "fn main() { let _ = obj.get_secret(); }\n",
        )],
    );

    let edit = perform_rename(&cache, &folders, &url, "", "hidden");
    let by_url = edits_by_url(&edit);
    assert!(
        !by_url.contains_key(&rs_url(&tmp, "src/main.rs")),
        "private property must not produce .rs edits"
    );
}

/// Kebab/snake normalization no-op (`my-count` -> `my_count` both
/// produce `get_my_count`): the scanner must NOT run, otherwise the
/// preview would show every .rs file as "modified" with identical text.
#[test]
fn noop_normalization_skips_scanner() {
    let tmp = tempdir();
    let (cache, url, folders) = setup(
        &tmp,
        &[(
            "ui/app.slint",
            r#"
export component App inherits Window {
in property <int> my-count /* <- TEST_ME */;
}
            "#,
        )],
        &[("src/main.rs", "fn main() { let _ = obj.get_my_count(); }\n")],
    );

    let edit = perform_rename(&cache, &folders, &url, "", "my_count");
    let by_url = edits_by_url(&edit);
    assert!(
        !by_url.contains_key(&rs_url(&tmp, "src/main.rs")),
        "no-op normalization must skip host-language scanning"
    );
}

/// Cross-file inheritance: property declared in a non-exported base in
/// `base.slint`, inherited by an exported component in `app.slint`. The
/// .rs file calls accessors on the derived component. Renaming the base
/// declaration must update both .slint files AND the .rs file.
#[test]
fn cross_file_inheritance_rewrites_all_three_files() {
    let tmp = tempdir();
    // base.slint must be loaded into the cache BEFORE app.slint, since
    // app.slint imports it. setup() loads files in array order.
    let (cache, _, folders) = setup(
        &tmp,
        &[
            (
                "ui/base.slint",
                r#"
export component Base {
in property <int> shared /* <- TEST_ME */;
}
                "#,
            ),
            (
                "ui/app.slint",
                r#"
import { Base } from "base.slint";
export component App inherits Base { }
                "#,
            ),
        ],
        &[("src/main.rs", "fn main() { obj.set_shared(42); }\n")],
    );

    // The renamed declaration is in base.slint, so we drive the rename
    // from base.slint's URL.
    let base_url = slint_url(&tmp, "ui/base.slint");
    let edit = perform_rename(&cache, &folders, &base_url, "", "common");
    let by_url = edits_by_url(&edit);
    let rust_edits = by_url.get(&rs_url(&tmp, "src/main.rs")).expect("missing .rs edits");
    assert_eq!(rust_edits, &vec!["set_common".to_string()]);
}

/// Globals: `export global Settings { in property <int> volume; }` plus
/// `Settings.volume` accessed via `global<Settings>().get_volume()` on
/// the Rust side. Renaming `volume` must rewrite the .rs call site.
#[test]
fn global_rename_rewrites_rust_accessors() {
    let tmp = tempdir();
    let (cache, url, folders) = setup(
        &tmp,
        &[(
            "ui/app.slint",
            r#"
export global Settings {
in-out property <int> volume /* <- TEST_ME */;
}
export component App inherits Window { }
            "#,
        )],
        &[("src/main.rs", "fn main() { app.global::<Settings>().set_volume(5); }\n")],
    );

    let edit = perform_rename(&cache, &folders, &url, "", "level");
    let by_url = edits_by_url(&edit);
    let rust_edits = by_url.get(&rs_url(&tmp, "src/main.rs")).expect("missing .rs edits");
    assert!(rust_edits.contains(&"set_level".to_string()), "{rust_edits:?}");
}

/// Property rename should rewrite the same `get_<n>`/`set_<n>` accessors
/// in a `.cpp` source file -- the scanner is language-agnostic at the
/// byte level and both backends emit identical accessor names.
#[test]
fn cpp_property_rename_rewrites_slint_and_cpp() {
    let tmp = tempdir();
    let (cache, url, folders) = setup(
        &tmp,
        &[(
            "ui/app.slint",
            r#"
export component App inherits Window {
in property <int> count /* <- TEST_ME */;
}
            "#,
        )],
        &[("src/main.cpp", "int main() { auto v = app->get_count(); app->set_count(v + 1); }\n")],
    );

    let edit = perform_rename(&cache, &folders, &url, "", "total");
    let by_url = edits_by_url(&edit);
    let cpp = Url::from_file_path(tmp.path().join("src/main.cpp")).unwrap();
    let cpp_edits = by_url.get(&cpp).expect("missing .cpp edits");
    assert!(cpp_edits.contains(&"get_total".to_string()), "{cpp_edits:?}");
    assert!(cpp_edits.contains(&"set_total".to_string()), "{cpp_edits:?}");
}

/// Header files (`.h`, `.hpp`) are also walked. A caller that holds a
/// reference to the generated component in a header method must be
/// rewritten too.
#[test]
fn cpp_header_call_site_is_rewritten() {
    let tmp = tempdir();
    let (cache, url, folders) = setup(
        &tmp,
        &[(
            "ui/app.slint",
            r#"
export component App inherits Window {
callback clicked /* <- TEST_ME */();
}
            "#,
        )],
        &[("include/binding.hpp", "inline void wire(App* app) { app->on_clicked([](){}); }\n")],
    );

    let edit = perform_rename(&cache, &folders, &url, "", "pressed");
    let by_url = edits_by_url(&edit);
    let hpp = Url::from_file_path(tmp.path().join("include/binding.hpp")).unwrap();
    let hpp_edits = by_url.get(&hpp).expect("missing .hpp edits");
    assert_eq!(hpp_edits, &vec!["on_pressed".to_string()]);
}

/// Mixed-language workspace: a single rename must rewrite call sites in
/// both `.rs` and `.cpp` files in a single `WorkspaceEdit`.
#[test]
fn mixed_rust_and_cpp_workspace_both_rewritten() {
    let tmp = tempdir();
    let (cache, url, folders) = setup(
        &tmp,
        &[(
            "ui/app.slint",
            r#"
export component App inherits Window {
in property <int> count /* <- TEST_ME */;
}
            "#,
        )],
        &[
            ("rust/src/main.rs", "fn main() { obj.get_count(); }\n"),
            ("cpp/src/main.cpp", "int main() { app->set_count(1); }\n"),
        ],
    );

    let edit = perform_rename(&cache, &folders, &url, "", "total");
    let by_url = edits_by_url(&edit);
    let rs = Url::from_file_path(tmp.path().join("rust/src/main.rs")).unwrap();
    let cpp = Url::from_file_path(tmp.path().join("cpp/src/main.cpp")).unwrap();
    assert_eq!(by_url.get(&rs).map(Vec::as_slice), Some(&["get_total".to_string()][..]));
    assert_eq!(by_url.get(&cpp).map(Vec::as_slice), Some(&["set_total".to_string()][..]));
}
