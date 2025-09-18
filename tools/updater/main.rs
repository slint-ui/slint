// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//!
//! Tool to change the syntax or reformat a .slint file

use clap::Parser;
use experiments::lookup_changes::LookupChangeState;
use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::object_tree::{self, Component, Document, ElementRc};
use i_slint_compiler::parser::{syntax_nodes, NodeOrToken, SyntaxKind, SyntaxNode};
use i_slint_compiler::typeloader::TypeLoader;
use smol_str::SmolStr;
use std::cell::RefCell;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::rc::Rc;

mod experiments {
    pub(super) mod geometry_changes;
    pub(super) mod input_output_properties;
    pub(super) mod lookup_changes;
    pub(super) mod new_component_declaration;
    pub(super) mod purity;
    pub(super) mod transitions;
}

mod transforms {
    pub(super) mod renames;
}

#[derive(clap::Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[arg(name = "path to .slint file(s)", action)]
    paths: Vec<std::path::PathBuf>,

    /// Modify the file inline instead of printing to updated contents to stdout
    #[arg(short, long, action)]
    inline: bool,

    /// Move all properties declarations to root of each component
    #[arg(long, action)]
    move_declarations: bool,
}

fn main() -> std::io::Result<()> {
    let args = Cli::parse();

    for path in &args.paths {
        let source = std::fs::read_to_string(path)?;

        if args.inline {
            let file = BufWriter::new(std::fs::File::create(path)?);
            process_file(source, path, file, &args)?
        } else {
            process_file(source, path, std::io::stdout(), &args)?
        }
    }
    Ok(())
}

fn process_rust_file(source: String, mut file: impl Write, args: &Cli) -> std::io::Result<()> {
    let mut last = 0;
    for range in i_slint_compiler::lexer::locate_slint_macro(&source) {
        file.write_all(&source.as_bytes()[last..=range.start])?;
        last = range.end;
        let code = &source[range];

        let mut diag = BuildDiagnostics::default();
        let syntax_node = i_slint_compiler::parser::parse(code.to_owned(), None, &mut diag);
        let len = syntax_node.text_range().end().into();
        let mut state = init_state(&syntax_node, &mut diag);
        visit_node(syntax_node, &mut file, &mut state, args)?;
        if diag.has_errors() {
            file.write_all(&code.as_bytes()[len..])?;
            diag.print();
        }
    }
    file.write_all(&source.as_bytes()[last..])
}

fn process_markdown_file(source: String, mut file: impl Write, args: &Cli) -> std::io::Result<()> {
    let mut source_slice = &source[..];
    const CODE_FENCE_START: &str = "```slint";
    const CODE_FENCE_END: &str = "```\n";
    'l: while let Some(code_start) = source_slice
        .find(CODE_FENCE_START)
        .map(|idx| idx + CODE_FENCE_START.len())
        .and_then(|x| source_slice[x..].find('\n').map(|idx| idx + x))
    {
        let code_end = if let Some(code_end) = source_slice[code_start..].find(CODE_FENCE_END) {
            code_end
        } else {
            break 'l;
        };
        file.write_all(&source_slice.as_bytes()[..=code_start - 1])?;
        source_slice = &source_slice[code_start..];
        let code = &source_slice[..code_end];
        source_slice = &source_slice[code_end..];

        let mut diag = BuildDiagnostics::default();
        let syntax_node = i_slint_compiler::parser::parse(code.to_owned(), None, &mut diag);
        let len = syntax_node.text_range().end().into();
        let mut state = init_state(&syntax_node, &mut diag);
        visit_node(syntax_node, &mut file, &mut state, args)?;
        if diag.has_errors() {
            file.write_all(&code.as_bytes()[len..])?;
            diag.print();
        }
    }
    file.write_all(source_slice.as_bytes())
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
    let mut state = init_state(&syntax_node, &mut diag);
    visit_node(syntax_node, &mut file, &mut state, args)?;
    if diag.has_errors() {
        file.write_all(&source.as_bytes()[len..])?;
        diag.print();
    }
    file.flush()?;
    Ok(())
}

fn init_state(syntax_node: &SyntaxNode, diag: &mut BuildDiagnostics) -> State {
    let mut state = State::default();
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
    let (foreign_imports, reexports) = spin_on::spin_on(type_loader.load_dependencies_recursively(
        &doc,
        diag,
        &dependency_registry,
    ));
    let current_doc = crate::object_tree::Document::from_node(
        doc,
        foreign_imports,
        reexports,
        diag,
        &dependency_registry,
    );
    i_slint_compiler::passes::infer_aliases_types::resolve_aliases(&current_doc, diag);
    state.current_doc = Rc::new(current_doc).into();
    state
}

#[derive(Default, Clone)]
struct State {
    /// When visiting a binding, this is the name of the current property
    property_name: Option<SmolStr>,

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
                let component_name = i_slint_compiler::parser::normalize_identifier(
                    &syntax_nodes::Component::from(node.clone())
                        .DeclaredIdentifier()
                        .child_text(SyntaxKind::Identifier)
                        .unwrap_or(SmolStr::default()),
                );

                state.current_component =
                    doc.inner_components.iter().find(|c| c.id == component_name).cloned();
                if args.move_declarations {
                    experiments::lookup_changes::collect_movable_properties(&mut state);
                }
            }
        }
        SyntaxKind::RepeatedElement | SyntaxKind::ConditionalElement => {
            if args.move_declarations {
                experiments::lookup_changes::collect_movable_properties(&mut state);
            }
        }
        SyntaxKind::Element => {
            if let Some(parent_el) = state.current_elem.take() {
                state.current_elem = parent_el
                    .borrow()
                    .children
                    .iter()
                    .find(|c| c.borrow().debug.first().is_some_and(|n| n.node.node == node.node))
                    .cloned()
            } else if let Some(parent_co) = &state.current_component {
                if node.parent().is_some_and(|n| n.kind() == SyntaxKind::Component) {
                    state.current_elem = Some(parent_co.root_element.clone())
                }
            }

            experiments::lookup_changes::enter_element(&mut state);
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
    if experiments::input_output_properties::fold_node(node, file, state, args)? {
        return Ok(true);
    }
    if experiments::new_component_declaration::fold_node(node, file, state, args)? {
        return Ok(true);
    }
    if experiments::lookup_changes::fold_node(node, file, state, args)? {
        return Ok(true);
    }
    if experiments::geometry_changes::fold_node(node, file, state, args)? {
        return Ok(true);
    }
    if experiments::transitions::fold_node(node, file, state, args)? {
        return Ok(true);
    }
    if experiments::purity::fold_node(node, file, state, args)? {
        return Ok(true);
    }
    if transforms::renames::fold_node(node, file, state, args)? {
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
