// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//!
//! Tool to change the syntax or reformat a .slint file
//!
//! As of know, it just rewrite the exact same as the input, but it can be changed
//!
//! This is how it can be used:
//!
//! ````shell
//! cargo run --bin syntax_updater -- --from 0.0.5 -i  **/*.60
//! cargo run --bin syntax_updater -- --from 0.0.5 -i  **/*.slint
//! cargo run --bin syntax_updater -- --from 0.0.5 -i  **/*.rs
//! cargo run --bin syntax_updater -- --from 0.0.5 -i  **/*.md
//! ````

use clap::Parser;
use experiments::lookup_changes::LookupChangeState;
use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::object_tree::{self, Component, Document, ElementRc};
use i_slint_compiler::parser::{syntax_nodes, NodeOrToken, SyntaxKind, SyntaxNode};
use i_slint_compiler::typeloader::TypeLoader;
use std::cell::RefCell;
use std::io::Write;
use std::path::Path;
use std::rc::Rc;

mod experiments {
    pub(super) mod lookup_changes;
}

#[derive(clap::Parser)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
    #[clap(name = "path to .slint file(s)", action)]
    paths: Vec<std::path::PathBuf>,

    /// modify the file inline instead of printing to stdout
    #[clap(short, long, action)]
    inline: bool,

    /// Do the lookup changes from issue #273
    #[clap(long, action)]
    experimental_lookup_changes: bool,

    /// Move all properties declaration to root
    #[clap(long, action)]
    experimental_move_declaration: bool,
}

fn main() -> std::io::Result<()> {
    let args = Cli::parse();

    for path in &args.paths {
        let source = std::fs::read_to_string(path)?;

        if args.inline {
            let file = std::fs::File::create(path)?;
            process_file(source, path, file, &args)?
        } else {
            process_file(source, path, std::io::stdout(), &args)?
        }
    }
    Ok(())
}

fn process_rust_file(source: String, mut file: impl Write, args: &Cli) -> std::io::Result<()> {
    let mut source_slice = &source[..];
    let slint_macro = format!("{}!", "slint"); // in a variable so it does not appear as is
    'l: while let Some(idx) = source_slice.find(&slint_macro) {
        // Note: this code ignore string literal and unbalanced comment, but that should be good enough
        let idx2 =
            if let Some(idx2) = source_slice[idx..].find(|c| c == '{' || c == '(' || c == '[') {
                idx2
            } else {
                break 'l;
            };
        let open = source_slice.as_bytes()[idx + idx2].into();
        let close = match open {
            '{' => '}',
            '(' => ')',
            '[' => ']',
            _ => panic!(),
        };
        file.write_all(source_slice[..=idx + idx2].as_bytes())?;
        source_slice = &source_slice[idx + idx2 + 1..];
        let mut idx = 0;
        let mut count = 1;
        while count > 0 {
            if let Some(idx2) = source_slice[idx..].find(|c| {
                if c == open {
                    count += 1;
                    true
                } else if c == close {
                    count -= 1;
                    true
                } else {
                    false
                }
            }) {
                idx += idx2 + 1;
            } else {
                break 'l;
            }
        }
        let code = &source_slice[..idx - 1];
        source_slice = &source_slice[idx - 1..];

        let mut diag = BuildDiagnostics::default();
        let syntax_node = i_slint_compiler::parser::parse(code.to_owned(), None, &mut diag);
        let len = syntax_node.text_range().end().into();
        visit_node(syntax_node, &mut file, &mut State::default(), args)?;
        if diag.has_error() {
            file.write_all(&code.as_bytes()[len..])?;
            diag.print();
        }
    }
    return file.write_all(source_slice.as_bytes());
}

fn process_markdown_file(source: String, mut file: impl Write, args: &Cli) -> std::io::Result<()> {
    let mut source_slice = &source[..];
    const CODE_FENCE_START: &str = "```slint\n";
    const CODE_FENCE_END: &str = "```\n";
    'l: while let Some(code_start) =
        source_slice.find(CODE_FENCE_START).map(|idx| idx + CODE_FENCE_START.len())
    {
        let code_end = if let Some(code_end) = source_slice[code_start..].find(CODE_FENCE_END) {
            code_end
        } else {
            break 'l;
        };
        file.write_all(source_slice[..=code_start - 1].as_bytes())?;
        source_slice = &source_slice[code_start..];
        let code = &source_slice[..code_end];
        source_slice = &source_slice[code_end..];

        let mut diag = BuildDiagnostics::default();
        let syntax_node = i_slint_compiler::parser::parse(code.to_owned(), None, &mut diag);
        let len = syntax_node.text_range().end().into();
        visit_node(syntax_node, &mut file, &mut State::default(), args)?;
        if diag.has_error() {
            file.write_all(&code.as_bytes()[len..])?;
            diag.print();
        }
    }
    return file.write_all(source_slice.as_bytes());
}

fn process_file(
    source: String,
    path: &Path,
    mut file: impl Write,
    args: &Cli,
) -> std::io::Result<()> {
    match path.extension() {
        Some(ext) if ext == "rs" => return process_rust_file(source, file, args),
        Some(ext) if ext == "md" => return process_markdown_file(source, file, args),
        _ => {}
    }

    let mut diag = BuildDiagnostics::default();
    let syntax_node = i_slint_compiler::parser::parse(source.clone(), Some(path), &mut diag);
    let len = syntax_node.node.text_range().end().into();
    let mut state = State::default();
    if args.experimental_lookup_changes {
        let doc = syntax_node.clone().into();
        let mut type_loader = TypeLoader::new(
            i_slint_compiler::typeregister::TypeRegister::builtin(),
            i_slint_compiler::CompilerConfiguration::new(
                i_slint_compiler::generator::OutputFormat::Llr,
            ),
            &mut BuildDiagnostics::default(),
        );
        let dependency_registry = Rc::new(RefCell::new(
            i_slint_compiler::typeregister::TypeRegister::new(&type_loader.global_type_registry),
        ));
        let foreign_imports = spin_on::spin_on(type_loader.load_dependencies_recursively(
            &doc,
            &mut diag,
            &dependency_registry,
        ));
        let current_doc = crate::object_tree::Document::from_node(
            doc,
            foreign_imports,
            &mut diag,
            &dependency_registry,
        );
        state.current_doc = Rc::new(current_doc).into()
    }
    visit_node(syntax_node, &mut file, &mut state, args)?;
    if diag.has_error() {
        file.write_all(&source.as_bytes()[len..])?;
        diag.print();
    }
    Ok(())
}

#[derive(Default, Clone)]
struct State {
    /// When visiting a binding, this is the name of the current property
    property_name: Option<String>,

    /// The Document being visited
    current_doc: Option<Rc<Document>>,
    /// The component in scope,
    current_component: Option<Rc<Component>>,
    /// The element in scope
    current_elem: Option<ElementRc>,

    lookup_change: LookupChangeState,
}

fn visit_node(
    node: SyntaxNode,
    file: &mut impl Write,
    state: &mut State,
    args: &Cli,
) -> std::io::Result<()> {
    let mut state = state.clone();
    match node.kind() {
        SyntaxKind::PropertyDeclaration => {
            state.property_name = node.child_text(SyntaxKind::DeclaredIdentifier)
        }
        SyntaxKind::Binding => state.property_name = node.child_text(SyntaxKind::Identifier),
        SyntaxKind::CallbackDeclaration => {
            state.property_name = node.child_text(SyntaxKind::Identifier)
        }
        SyntaxKind::Component => {
            if let Some(doc) = &state.current_doc {
                let component_name =
                    syntax_nodes::Component::from(node.clone()).DeclaredIdentifier().to_string();
                state.current_component =
                    doc.inner_components.iter().find(|c| c.id == component_name).cloned();
                if args.experimental_move_declaration {
                    experiments::lookup_changes::collect_movable_properties(&mut state);
                }
            }
        }
        SyntaxKind::RepeatedElement | SyntaxKind::ConditionalElement => {
            if args.experimental_move_declaration {
                experiments::lookup_changes::collect_movable_properties(&mut state);
            }
        }
        SyntaxKind::Element => {
            /*let element_node = syntax_nodes::Element::from(node.clone());
            state.element_name = element_node
                .QualifiedName()
                .map(|qn| object_tree::QualifiedTypeName::from_node(qn).to_string());*/
            if let Some(parent_el) = state.current_elem.take() {
                state.current_elem = parent_el
                    .borrow()
                    .children
                    .iter()
                    .find(|c| c.borrow().node.as_ref().map_or(false, |n| n.node == node.node))
                    .cloned()
            } else if let Some(parent_co) = &state.current_component {
                if node.parent().map_or(false, |n| n.kind() == SyntaxKind::Component) {
                    state.current_elem = Some(parent_co.root_element.clone())
                }
            }
        }
        _ => (),
    }

    if fold_node(&node, file, &mut state, args)? {
        return Ok(());
    }
    for n in node.children_with_tokens() {
        visit_node_or_token(n, file, &mut state, args)?;
    }
    Ok(())
}

pub(crate) fn visit_node_or_token(
    n: NodeOrToken,
    file: &mut impl Write,
    state: &mut State,
    args: &Cli,
) -> std::io::Result<()> {
    match n {
        NodeOrToken::Node(n) => visit_node(n, file, state, args)?,
        NodeOrToken::Token(t) => fold_token(t, file, state)?,
    };
    Ok(())
}

/// return false if one need to continue folding the children
fn fold_node(
    node: &SyntaxNode,
    file: &mut impl Write,
    state: &mut State,
    args: &Cli,
) -> std::io::Result<bool> {
    if (args.experimental_lookup_changes || args.experimental_move_declaration)
        && experiments::lookup_changes::fold_node(node, file, state, args)?
    {
        return Ok(true);
    }
    Ok(false)
}

fn fold_token(
    node: i_slint_compiler::parser::SyntaxToken,
    file: &mut impl Write,
    #[allow(unused)] state: &mut State,
) -> std::io::Result<()> {
    /* Example: this adds the "ms" prefix to the number within a "duration" binding
    if state.property_name == Some("duration".into()) && node.kind() == SyntaxKind::NumberLiteral {
        if !node.text().ends_with("s") {
            return write!(file, "{}ms", node.text());
        }
    }*/
    /* Example: replace _ by - in identifiers
    if node.kind() == SyntaxKind::Identifier
        && node.text().contains('_')
        && !node.text().starts_with("_")
    {
        return file.write_all(node.text().replace('_', "-").as_bytes())
    }*/
    file.write_all(node.text().as_bytes())
}
