// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell:ignore punct

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]

extern crate proc_macro;

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::parser::SyntaxKind;
use i_slint_compiler::*;
use proc_macro::{Spacing, TokenStream, TokenTree};
use quote::quote;
use std::path::PathBuf;

mod expansion_cache;

/// Returns true if the two token are touching. For example the two token `foo`and `-` are touching if
/// it was written like so in the source code: `foo-` but not when written like so `foo -`
///
/// Returns None if we couldn't detect whether they are touching  (eg, our heuristics don't work with rust-analyzer)
fn are_token_touching(token1: proc_macro::Span, token2: proc_macro::Span) -> Option<bool> {
    let t1 = token1.end();
    let t2 = token2.start();
    let t1_column = t1.column();
    if t1_column == 1 && t1.line() == 1 && t2.end().line() == 1 && t2.end().column() == 1 {
        // If everything is 1, this means that Span::line and Span::column are not working properly
        // (eg, rust-analyzer)
        return None;
    }
    Some(t1.line() == t2.line() && t1_column == t2.column())
}

fn fill_token_vec(stream: impl Iterator<Item = TokenTree>, vec: &mut Vec<parser::Token>) {
    let mut prev_spacing = Spacing::Alone;
    let mut prev_span = proc_macro::Span::call_site();
    for t in stream {
        let span = t.span();
        match t {
            TokenTree::Ident(i) => {
                if let Some(last) = vec.last_mut()
                    && ((last.kind == SyntaxKind::ColorLiteral && last.text.len() == 1)
                        || (last.kind == SyntaxKind::Identifier
                            && are_token_touching(prev_span, span)
                                .unwrap_or_else(|| last.text.ends_with('-'))))
                {
                    last.text = format!("{}{}", last.text, i).into();
                    prev_span = span;
                    continue;
                }
                vec.push(parser::Token {
                    kind: SyntaxKind::Identifier,
                    text: i.to_string().into(),
                    span: Some(i.span()),
                    ..Default::default()
                });
            }
            TokenTree::Punct(p) => {
                let kind = match p.as_char() {
                    ':' => SyntaxKind::Colon,
                    '=' => {
                        if let Some(last) = vec.last_mut() {
                            let kt = match last.kind {
                                SyntaxKind::Star => Some((SyntaxKind::StarEqual, "*=")),
                                SyntaxKind::Colon => Some((SyntaxKind::ColonEqual, ":=")),
                                SyntaxKind::Plus => Some((SyntaxKind::PlusEqual, "+=")),
                                SyntaxKind::Minus => Some((SyntaxKind::MinusEqual, "-=")),
                                SyntaxKind::Div => Some((SyntaxKind::DivEqual, "/=")),
                                SyntaxKind::LAngle => Some((SyntaxKind::LessEqual, "<=")),
                                SyntaxKind::RAngle => Some((SyntaxKind::GreaterEqual, ">=")),
                                SyntaxKind::Equal => Some((SyntaxKind::EqualEqual, "==")),
                                SyntaxKind::Bang => Some((SyntaxKind::NotEqual, "!=")),
                                _ => None,
                            };
                            if let Some((k, t)) = kt
                                && prev_spacing == Spacing::Joint
                            {
                                last.kind = k;
                                last.text = t.into();
                                continue;
                            }
                        }
                        SyntaxKind::Equal
                    }
                    ';' => SyntaxKind::Semicolon,
                    '!' => SyntaxKind::Bang,
                    '.' => {
                        // `4..log` is lexed as `4 . . log` in rust, but should be `4. . log` in slint
                        if let Some(last) = vec.last_mut()
                            && last.kind == SyntaxKind::NumberLiteral
                            && are_token_touching(prev_span, p.span()).unwrap_or(false)
                            && !last.text.contains('.')
                            && !last.text.ends_with(char::is_alphabetic)
                        {
                            last.text = format!("{}.", last.text).into();
                            prev_span = span;
                            continue;
                        }
                        SyntaxKind::Dot
                    }
                    '+' => SyntaxKind::Plus,
                    '-' => {
                        if let Some(last) = vec.last_mut()
                            && last.kind == SyntaxKind::Identifier
                            && are_token_touching(prev_span, p.span()).unwrap_or(true)
                        {
                            last.text = format!("{}-", last.text).into();
                            prev_span = span;
                            continue;
                        }
                        SyntaxKind::Minus
                    }
                    '*' => SyntaxKind::Star,
                    '/' => SyntaxKind::Div,
                    '<' => {
                        if let Some(last) = vec.last_mut()
                            && last.kind == SyntaxKind::LAngle
                            && prev_spacing == Spacing::Joint
                        {
                            last.kind = SyntaxKind::DoubleLess;
                            last.text = "<<".into();
                            continue;
                        }
                        SyntaxKind::LAngle
                    }
                    '>' => {
                        if let Some(last) = vec.last_mut() {
                            if last.kind == SyntaxKind::LessEqual && prev_spacing == Spacing::Joint
                            {
                                last.kind = SyntaxKind::DoubleArrow;
                                last.text = "<=>".into();
                                continue;
                            } else if last.kind == SyntaxKind::Equal
                                && prev_spacing == Spacing::Joint
                            {
                                last.kind = SyntaxKind::FatArrow;
                                last.text = "=>".into();
                                continue;
                            } else if last.kind == SyntaxKind::Minus
                                && prev_spacing == Spacing::Joint
                            {
                                last.kind = SyntaxKind::Arrow;
                                last.text = "->".into();
                                continue;
                            }
                        }
                        SyntaxKind::RAngle
                    }
                    '#' => SyntaxKind::ColorLiteral,
                    '?' => SyntaxKind::Question,
                    ',' => SyntaxKind::Comma,
                    '&' => {
                        // Since the '&' alone does not exist or cannot be part of any other token that &&
                        // just consider it as '&&' and skip the joint ones.  FIXME. do that properly
                        if let Some(last) = vec.last_mut()
                            && last.kind == SyntaxKind::AndAnd
                            && prev_spacing == Spacing::Joint
                        {
                            continue;
                        }
                        SyntaxKind::AndAnd
                    }
                    '|' => {
                        // Since the '|' alone does not exist or cannot be part of any other token that ||
                        // just consider it as '||' and skip the joint ones.
                        if let Some(last) = vec.last_mut()
                            && last.kind == SyntaxKind::Pipe
                            && prev_spacing == Spacing::Joint
                        {
                            last.kind = SyntaxKind::OrOr;
                            continue;
                        }
                        SyntaxKind::Pipe
                    }
                    '%' => {
                        // handle % as a unit
                        if let Some(last) = vec.last_mut()
                            && last.kind == SyntaxKind::NumberLiteral
                        {
                            last.text = format!("{}%", last.text).into();
                            continue;
                        }
                        SyntaxKind::Percent
                    }
                    '$' => SyntaxKind::Dollar,
                    '@' => SyntaxKind::At,
                    _ => SyntaxKind::Error,
                };
                prev_spacing = p.spacing();
                vec.push(parser::Token {
                    kind,
                    text: p.to_string().into(),
                    span: Some(p.span()),
                    ..Default::default()
                });
            }
            TokenTree::Literal(l) => {
                let s = l.to_string();
                // Why can't the rust API give me the type of the literal
                let f = s.chars().next().unwrap();
                let kind = if f == '"' {
                    SyntaxKind::StringLiteral
                } else if f.is_ascii_digit() {
                    if let Some(last) = vec.last_mut()
                        && ((last.kind == SyntaxKind::ColorLiteral && last.text.len() == 1)
                            || (last.kind == SyntaxKind::Identifier
                                && are_token_touching(prev_span, span)
                                    .unwrap_or_else(|| last.text.ends_with('-'))))
                    {
                        last.text = format!("{}{}", last.text, s).into();
                        prev_span = span;
                        continue;
                    }
                    SyntaxKind::NumberLiteral
                } else {
                    SyntaxKind::Error
                };
                vec.push(parser::Token {
                    kind,
                    text: s.into(),
                    span: Some(l.span()),
                    ..Default::default()
                });
            }
            TokenTree::Group(g) => {
                use SyntaxKind::*;
                use proc_macro::Delimiter::*;
                let (l, r, sl, sr) = match g.delimiter() {
                    Parenthesis => (LParent, RParent, "(", ")"),
                    Brace => (LBrace, RBrace, "{", "}"),
                    Bracket => (LBracket, RBracket, "[", "]"),
                    None => todo!(),
                };
                vec.push(parser::Token {
                    kind: l,
                    text: sl.into(),
                    span: Some(g.span()), // span_open is not stable
                    ..Default::default()
                });
                fill_token_vec(g.stream().into_iter(), vec);
                vec.push(parser::Token {
                    kind: r,
                    text: sr.into(),
                    span: Some(g.span()), // span_clone is not stable
                    ..Default::default()
                });
            }
        }
        prev_span = span;
    }
}

fn extract_path(literal: proc_macro::Literal) -> std::path::PathBuf {
    let path_with_quotes = literal.to_string();
    let path_with_quotes_stripped = if let Some(p) = path_with_quotes.strip_prefix('r') {
        let hash_removed = p.trim_matches('#');
        hash_removed.strip_prefix('\"').unwrap().strip_suffix('\"').unwrap()
    } else {
        // FIXME: unescape
        path_with_quotes.trim_matches('\"')
    };
    path_with_quotes_stripped.into()
}

fn extract_compiler_config(
    mut stream: proc_macro::token_stream::IntoIter,
    compiler_config: &mut CompilerConfiguration,
) -> impl Iterator<Item = TokenTree> {
    let mut remaining_stream;
    loop {
        remaining_stream = stream.clone();
        match (stream.next(), stream.next()) {
            (Some(TokenTree::Punct(p)), Some(TokenTree::Group(group)))
                if p.as_char() == '#' && group.delimiter() == proc_macro::Delimiter::Bracket =>
            {
                let mut attr_stream = group.stream().into_iter();
                match attr_stream.next() {
                    Some(TokenTree::Ident(include_ident))
                        if include_ident.to_string() == "include_path" =>
                    {
                        match (attr_stream.next(), attr_stream.next()) {
                            (
                                Some(TokenTree::Punct(equal_punct)),
                                Some(TokenTree::Literal(path)),
                            ) if equal_punct.as_char() == '=' => {
                                compiler_config.include_paths.push(extract_path(path));
                            }
                            _ => break,
                        }
                    }
                    Some(TokenTree::Ident(library_ident))
                        if library_ident.to_string() == "library_path" =>
                    {
                        match (attr_stream.next(), attr_stream.next(), attr_stream.next()) {
                            (
                                Some(TokenTree::Group(group)),
                                Some(TokenTree::Punct(equal_punct)),
                                Some(TokenTree::Literal(path)),
                            ) if group.delimiter() == proc_macro::Delimiter::Parenthesis
                                && equal_punct.as_char() == '=' =>
                            {
                                let library_name = group.stream().into_iter().next().unwrap();
                                compiler_config
                                    .library_paths
                                    .insert(library_name.to_string(), extract_path(path));
                            }
                            _ => break,
                        }
                    }
                    Some(TokenTree::Ident(style_ident)) if style_ident.to_string() == "style" => {
                        match (attr_stream.next(), attr_stream.next()) {
                            (
                                Some(TokenTree::Punct(equal_punct)),
                                Some(TokenTree::Literal(requested_style)),
                            ) if equal_punct.as_char() == '=' => {
                                compiler_config.style = requested_style
                                    .to_string()
                                    .strip_prefix('\"')
                                    .unwrap()
                                    .strip_suffix('\"')
                                    .unwrap()
                                    .to_string()
                                    .into();
                            }
                            _ => break,
                        }
                    }
                    _ => break,
                }
            }
            _ => break,
        }
    }
    remaining_stream
}

/// The directory the search for `slint.project.json` starts in.
///
/// A macro body that does nothing but re-export from a single `.slint` file follows
/// that file, so it picks up the same project file as compiling the file directly.
/// Anything else is its own entry point and searches from the `.rs` file.
fn project_file_search_directory(
    document: &parser::syntax_nodes::Document,
    source_path: &std::path::Path,
) -> PathBuf {
    let source_directory = pathutils::dirname(source_path);

    let declares_anything = document.Component().next().is_some()
        || document.StructDeclaration().next().is_some()
        || document.EnumDeclaration().next().is_some()
        || document.ExportsList().any(|exports| {
            exports.Component().is_some()
                || exports.StructDeclaration().next().is_some()
                || exports.EnumDeclaration().next().is_some()
        });
    if declares_anything {
        return source_directory;
    }

    let mut imported_uris = document
        .ImportSpecifier()
        .filter_map(|import| import.child_token(SyntaxKind::StringLiteral))
        .chain(
            document
                .ExportsList()
                .filter_map(|exports| exports.ExportModule())
                .filter_map(|reexport| reexport.child_token(SyntaxKind::StringLiteral)),
        );

    let (Some(imported_uri), None) = (imported_uris.next(), imported_uris.next()) else {
        return source_directory;
    };

    // Same verbatim treatment as the type loader gives an import path.
    let imported_path = imported_uri.text().to_string();
    let imported_path = imported_path.trim_matches('"');

    // A library import needs the library paths the project file is meant to supply.
    if imported_path.is_empty() || imported_path.starts_with('@') {
        return source_directory;
    }

    pathutils::join(&source_directory, std::path::Path::new(imported_path))
        .filter(|path| path.exists())
        .map(|path| pathutils::dirname(&path))
        .unwrap_or(source_directory)
}

/// The external files whose changes should invalidate this expansion: the loaded
/// files that are absolute and not the `Cargo.toml`. This is the set that both the
/// `include_bytes!` recompile markers and the output cache key off of.
fn loaded_files(diag: &BuildDiagnostics) -> Vec<PathBuf> {
    diag.all_loaded_files
        .iter()
        .filter(|path| path.is_absolute() && !path.ends_with("Cargo.toml"))
        .cloned()
        .collect()
}

/// This macro allows you to use the Slint design markup language inline in Rust code. Within the braces of the macro
/// you can use place Slint code and the named exported components will be available for instantiation.
///
/// For the documentation about the syntax of the language, see
#[doc = concat!("[The Slint Language Documentation](https://slint.dev/releases/", env!("CARGO_PKG_VERSION"), "/docs/slint)")]
///
/// When Rust 1.88 or later is used, the paths for loading images with `@image-url` and importing `.slint` files
/// are relative to the `.rs` file that contains the macro.
/// For compatibility with older rust version, the files are also searched in the manifest directory that contains `Cargo.toml`.
///
/// ### Limitations
///
/// Because this macro receives its input through Rust's tokenizer, a few constructs that are
/// valid in standalone `.slint` files cannot be used here:
///
/// - **String interpolation with `\{...}`**: Rust parses the macro body as Rust string literals
///   first, and `\{...}` is not a valid Rust string escape.
///
/// - **Color literals that begin with `#0b`** (for example `#0bf707`): Rust's
///   lexer sees the `0b` as the start of a numeric literal with a
///   binary prefix, then rejects the remaining hex digits as invalid digits for that base.
///
/// - **Color literals matching `#<digits>e<non-digit-hex>…`** (for example `#10ea4c`):
///   Rust's lexer tries to read the payload as a float with scientific notation (`10e…`), and
///   rejects the non-digit characters that follow the `e`.
///
/// In all three cases the workarounds are to either rewrite the literal in a form Rust can
/// tokenize (e.g. `rgb(11, 247, 7)` in place of `#0bf707`), or to move the Slint code into a
/// `.slint` file and compile it via [`slint-build`](https://crates.io/crates/slint-build).
#[proc_macro]
pub fn slint(stream: TokenStream) -> TokenStream {
    let token_iter = stream.into_iter();

    let mut compiler_config =
        CompilerConfiguration::new(i_slint_compiler::generator::OutputFormat::Rust);

    let token_iter = extract_compiler_config(token_iter, &mut compiler_config);

    let mut tokens = Vec::new();
    fill_token_vec(token_iter, &mut tokens);
    // Position each token in the document the parser will see: the concatenation of the token
    // texts. A token can still grow while the vector is built, as later tokens are merged into
    // it, so this can only be done now.
    let mut offset = 0;
    for t in &mut tokens {
        t.offset = offset;
        offset += t.text.len();
    }

    fn local_file(tokens: &[parser::Token]) -> Option<PathBuf> {
        tokens.first()?.span?.local_file()
    }

    let source_path: PathBuf = if let Some(path) = local_file(&tokens) {
        path
    } else if let Some(cargo_manifest) = std::env::var_os("CARGO_MANIFEST_DIR") {
        let mut path: std::path::PathBuf = cargo_manifest.into();
        path.push("Cargo.toml");
        path
    } else {
        Default::default()
    };

    compiler_config.translation_domain = std::env::var("CARGO_PKG_NAME").ok();

    let source_file = diagnostics::SourceFileInner::from_path_only(source_path.clone());
    let mut diag = BuildDiagnostics::default();
    let syntax_node = parser::parse_tokens(tokens.clone(), source_file, &mut diag);
    if diag.has_errors() {
        return diag.report_macro_diagnostic(&tokens);
    }

    // Before the cache key is taken, since the project file changes the generated output.
    let document = parser::syntax_nodes::Document::from(syntax_node.clone());
    let search_directory = project_file_search_directory(&document, &source_path);
    match project_file::find_project_file_path(&search_directory) {
        Ok(Some(path)) => match project_file::ProjectFile::load(&path) {
            Ok(project_file) => project_file.apply_to(&mut compiler_config),
            Err(error) => {
                diag.push_error_with_span(
                    format!("Cannot load {}: {error}", path.display()),
                    Default::default(),
                );
                return diag.report_macro_diagnostic(&tokens);
            }
        },
        Ok(None) => {}
        Err(error) => {
            diag.push_error_with_span(
                format!("Cannot look for {}: {error}", project_file::FILE_NAME),
                Default::default(),
            );
            return diag.report_macro_diagnostic(&tokens);
        }
    }

    // Consult the output cache before doing any (expensive) compilation. The key
    // is computed from the macro body plus everything else that influences the
    // generated output; a hit just re-parses the cached output string. Only
    // active under rust-analyzer (see expansion_cache docs).
    let cache_key = expansion_cache::enabled()
        .then(|| expansion_cache::key_material(&tokens, &compiler_config, &source_path));
    if let Some(key) = &cache_key
        && let Some(output) = expansion_cache::lookup(key)
        && let Ok(stream) = output.parse::<TokenStream>()
    {
        return stream;
    }

    //println!("{syntax_node:#?}");
    let (root_component, diag, loader) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diag, compiler_config));
    //println!("{tree:#?}");
    if diag.has_errors() {
        return diag.report_macro_diagnostic(&tokens);
    }

    if expansion_cache::is_rust_analyzer() {
        // When running on rust-analyzer, only generate the API (using the live preview) to make rust-analyzer faster and use less memory
        // (This uses an unstable env variable, but it is just an optimization)
        let generated =
            generator::rust_live_preview::generate(&root_component, &loader.compiler_config)
                .unwrap_or_else(|e| {
                    let e_str = e.to_string();
                    quote!(compile_error!(#e_str))
                });
        // Populate the cache so the next identical expansion is a cheap re-parse.
        // The live-preview output is a pure function of the compiled component and
        // config (no diagnostic/span tokens), so it is safe to cache regardless of
        // warnings — which this path discards anyway.
        if let Some(key) = cache_key {
            expansion_cache::store(key, generated.to_string(), &loaded_files(&diag));
        }
        return generated.into();
    }

    let mut result = generator::rust::generate(&root_component, &loader.compiler_config)
        .unwrap_or_else(|e| {
            let e_str = e.to_string();
            quote!(compile_error!(#e_str))
        });

    // Make sure to recompile if any of the external files changes
    let loaded = loaded_files(&diag);
    let reload = loaded
        .iter()
        .filter_map(|p| p.to_str())
        .map(|p| quote! {const _ : &'static [u8] = ::core::include_bytes!(#p);});

    result.extend(reload);
    result.extend(quote! {const _ : ::core::option::Option<&'static str> = ::core::option_env!("SLINT_STYLE");});

    let mut result = TokenStream::from(result);
    if !diag.is_empty() {
        // Output carries span-bearing diagnostic tokens tied to this call site, so
        // it must not be cached.
        result.extend(diag.report_macro_diagnostic(&tokens));
    } else if let Some(key) = cache_key {
        // Clean expansion: cache the full output (including the reload markers) so
        // the next identical expansion is a cheap re-parse.
        expansion_cache::store(key, result.to_string(), &loaded);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::project_file_search_directory;
    use i_slint_compiler::diagnostics::BuildDiagnostics;
    use i_slint_compiler::parser;
    use std::path::{Path, PathBuf};

    fn search_directory_for(body: &str, source_path: &Path) -> PathBuf {
        let mut diag = BuildDiagnostics::default();
        let node = parser::parse(body.to_string(), Some(source_path), &mut diag);
        assert!(!diag.has_errors(), "{:?}", diag.to_string_vec());
        let document = parser::syntax_nodes::Document::from(node);
        project_file_search_directory(&document, source_path)
    }

    fn with_test_directory<R>(f: impl FnOnce(&Path) -> R) -> R {
        let directory = tempfile::TempDir::new().unwrap();
        let path = std::fs::canonicalize(directory.path()).unwrap();
        f(&path)
    }

    #[test]
    fn an_inline_component_searches_from_the_rust_file() {
        with_test_directory(|root| {
            let source_path = root.join("src/main.rs");
            assert_eq!(
                search_directory_for("export component App inherits Rectangle {}", &source_path),
                root.join("src")
            );
        });
    }

    #[test]
    fn a_lone_re_export_follows_the_slint_file() {
        with_test_directory(|root| {
            let ui_directory = root.join("ui");
            std::fs::create_dir_all(&ui_directory).unwrap();
            std::fs::write(ui_directory.join("main.slint"), "export component App {}").unwrap();
            let source_path = root.join("src/main.rs");
            std::fs::create_dir_all(source_path.parent().unwrap()).unwrap();

            assert_eq!(
                search_directory_for(r#"export { App } from "../ui/main.slint";"#, &source_path),
                ui_directory
            );
        });
    }

    #[test]
    fn a_lone_import_follows_the_slint_file() {
        with_test_directory(|root| {
            let ui_directory = root.join("ui");
            std::fs::create_dir_all(&ui_directory).unwrap();
            std::fs::write(ui_directory.join("main.slint"), "export component App {}").unwrap();
            let source_path = root.join("main.rs");

            assert_eq!(
                search_directory_for(
                    r#"import { App } from "ui/main.slint"; export { App }"#,
                    &source_path
                ),
                ui_directory
            );
        });
    }

    #[test]
    fn a_re_export_next_to_an_inline_component_searches_from_the_rust_file() {
        with_test_directory(|root| {
            let ui_directory = root.join("ui");
            std::fs::create_dir_all(&ui_directory).unwrap();
            std::fs::write(ui_directory.join("main.slint"), "export component App {}").unwrap();
            let source_path = root.join("main.rs");

            assert_eq!(
                search_directory_for(
                    r#"export { App } from "ui/main.slint"; export component Extra inherits Rectangle {}"#,
                    &source_path
                ),
                *root
            );
        });
    }

    #[test]
    fn two_imports_search_from_the_rust_file() {
        with_test_directory(|root| {
            let ui_directory = root.join("ui");
            std::fs::create_dir_all(&ui_directory).unwrap();
            std::fs::write(ui_directory.join("a.slint"), "export component A {}").unwrap();
            std::fs::write(ui_directory.join("b.slint"), "export component B {}").unwrap();
            let source_path = root.join("main.rs");

            assert_eq!(
                search_directory_for(
                    r#"import { A } from "ui/a.slint"; import { B } from "ui/b.slint"; export { A, B }"#,
                    &source_path
                ),
                *root
            );
        });
    }

    #[test]
    fn a_library_import_searches_from_the_rust_file() {
        with_test_directory(|root| {
            let source_path = root.join("main.rs");
            assert_eq!(
                search_directory_for(r#"export { App } from "@widgets/main.slint";"#, &source_path),
                *root
            );
        });
    }

    #[test]
    fn a_missing_slint_file_searches_from_the_rust_file() {
        with_test_directory(|root| {
            let source_path = root.join("main.rs");
            assert_eq!(
                search_directory_for(r#"export { App } from "ui/gone.slint";"#, &source_path),
                *root
            );
        });
    }
}
