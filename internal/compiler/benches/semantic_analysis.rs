// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Benchmarks for the compiler's semantic analysis phase.
//!
//! These benchmarks measure the performance of various compilation stages,
//! focusing on areas identified as allocation hotspots.
//!
//! Run with: cargo bench -p i-slint-compiler --features rust
//!
//! To run a specific benchmark:
//!   cargo bench -p i-slint-compiler --features rust -- full_compilation
//!
//! To run proc-macro simulation (measures full slint! macro overhead):
//!   cargo bench -p i-slint-compiler --features rust -- proc_macro_simulation
//!
//! To get allocation statistics, set DIVAN_BYTES=1

use i_slint_compiler::CompilerConfiguration;
use i_slint_compiler::diagnostics::{BuildDiagnostics, SourceFile, SourceFileInner};
use i_slint_compiler::object_tree::Document;
use i_slint_compiler::parser;
use std::path::PathBuf;
use std::rc::Rc;

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

/// Minimal valid Slint document - baseline for proc-macro overhead
const EMPTY_COMPONENT: &str = "export component Empty {}";

/// Simple component for baseline measurements
const SIMPLE_COMPONENT: &str = r#"
export component Simple inherits Rectangle {
    width: 100px;
    height: 100px;
    background: blue;

    Text {
        text: "Hello";
        color: white;
    }
}
"#;

/// Component with many children to stress children Vec allocation
fn generate_many_children(count: usize) -> String {
    let mut s = String::from("export component ManyChildren inherits Rectangle {\n");
    for i in 0..count {
        s.push_str(&format!(
            "    rect{i}: Rectangle {{ x: {x}px; y: {y}px; width: 10px; height: 10px; }}\n",
            x = (i % 10) * 15,
            y = (i / 10) * 15
        ));
    }
    s.push_str("}\n");
    s
}

/// Component with many properties to stress property declaration allocations
fn generate_many_properties(count: usize) -> String {
    let mut s = String::from("export component ManyProps inherits Rectangle {\n");
    for i in 0..count {
        s.push_str(&format!("    in-out property <int> prop{i}: {i};\n"));
    }
    s.push_str("    width: 100px;\n");
    s.push_str("    height: 100px;\n");
    s.push_str("}\n");
    s
}

/// Component with deep expression trees to stress Box<Expression> allocations
fn generate_deep_expressions(depth: usize) -> String {
    let mut expr = String::from("1");
    for i in 2..=depth {
        expr = format!("({expr} + {i})");
    }
    format!(
        r#"
export component DeepExpr inherits Rectangle {{
    property <int> result: {expr};
    width: 100px;
    height: 100px;
}}
"#
    )
}

/// Component with many states to stress states Vec allocation
fn generate_many_states(count: usize) -> String {
    let mut s = String::from(
        r#"
export component ManyStates inherits Rectangle {
    in-out property <int> current-state: 0;
    width: 100px;
    height: 100px;
    background: gray;
"#,
    );
    for i in 0..count {
        s.push_str(&format!(
            r#"
    states [
        state{i} when current-state == {i}: {{
            background: rgb({r}, {g}, {b});
        }}
    ]
"#,
            r = (i * 7) % 256,
            g = (i * 13) % 256,
            b = (i * 23) % 256
        ));
    }
    s.push_str("}\n");
    s
}

/// Component with nested sub-components to stress inlining
fn generate_nested_components(depth: usize) -> String {
    let mut s = String::new();
    for i in (0..depth).rev() {
        if i == depth - 1 {
            s.push_str(&format!(
                r#"
component Level{i} inherits Rectangle {{
    width: 10px;
    height: 10px;
    background: red;
}}
"#
            ));
        } else {
            s.push_str(&format!(
                r#"
component Level{i} inherits Rectangle {{
    Level{next} {{ }}
    width: parent.width + 10px;
    height: parent.height + 10px;
}}
"#,
                next = i + 1
            ));
        }
    }
    s.push_str(
        r#"
export component NestedComponents inherits Rectangle {
    Level0 { }
    width: 200px;
    height: 200px;
}
"#,
    );
    s
}

/// Component with many exports to stress export sorting
fn generate_many_exports(count: usize) -> String {
    let mut s = String::new();
    // Generate components in non-alphabetical order to stress sorting
    let mut indices: Vec<usize> = (0..count).collect();
    indices.sort_by(|a, b| ((b * 17) % count).cmp(&((a * 17) % count)));

    for i in indices {
        s.push_str(&format!(
            r#"
export component Export{i:04} inherits Rectangle {{
    width: 10px;
    height: 10px;
}}
"#
        ));
    }
    s
}

/// Component with bindings that have dependencies (for binding analysis)
fn generate_binding_chain(length: usize) -> String {
    let mut s = String::from(
        r#"
export component BindingChain inherits Rectangle {
    property <int> start: 1;
"#,
    );
    for i in 0..length {
        s.push_str(&format!("    property <int> step{i}: start + {i};\n"));
    }
    s.push_str(&format!("    property <int> end: step{};\n", length.saturating_sub(1)));
    s.push_str("    width: 100px;\n");
    s.push_str("    height: 100px;\n");
    s.push_str("}\n");
    s
}

/// Parse source code into a syntax node
fn parse_source(source: &str) -> parser::SyntaxNode {
    let mut diagnostics = BuildDiagnostics::default();
    let tokens = i_slint_compiler::lexer::lex(source);
    let source_file: SourceFile =
        Rc::new(SourceFileInner::new(PathBuf::from("bench.slint"), source.to_string()));
    parser::parse_tokens(tokens, source_file, &mut diagnostics)
}

/// Full compilation including all passes
fn compile_full(source: &str) -> (Document, BuildDiagnostics) {
    let diagnostics = BuildDiagnostics::default();
    let node = parse_source(source);

    let config = CompilerConfiguration::new(i_slint_compiler::generator::OutputFormat::Interpreter);

    let (doc, diag, _loader) =
        spin_on::spin_on(i_slint_compiler::compile_syntax_node(node, diagnostics, config));
    (doc, diag)
}

// ============================================================================
// Benchmarks
// ============================================================================

mod parsing {
    use super::*;

    #[divan::bench]
    fn simple_component() {
        divan::black_box(parse_source(SIMPLE_COMPONENT));
    }

    #[divan::bench(args = [10, 50, 100, 200])]
    fn many_children(n: usize) {
        let source = generate_many_children(n);
        divan::black_box(parse_source(&source));
    }

    #[divan::bench(args = [10, 50, 100])]
    fn many_properties(n: usize) {
        let source = generate_many_properties(n);
        divan::black_box(parse_source(&source));
    }
}

mod lexing {
    use super::*;

    #[divan::bench]
    fn simple_component() {
        divan::black_box(i_slint_compiler::lexer::lex(SIMPLE_COMPONENT));
    }

    #[divan::bench(args = [10, 50, 100, 200])]
    fn many_children(n: usize) {
        let source = generate_many_children(n);
        divan::black_box(i_slint_compiler::lexer::lex(&source));
    }

    #[divan::bench(args = [10, 50, 100])]
    fn many_properties(n: usize) {
        let source = generate_many_properties(n);
        divan::black_box(i_slint_compiler::lexer::lex(&source));
    }
}

mod full_compilation {
    use super::*;

    #[divan::bench]
    fn simple_component() {
        divan::black_box(compile_full(SIMPLE_COMPONENT));
    }

    #[divan::bench(args = [10, 50, 100])]
    fn many_children(n: usize) {
        let source = generate_many_children(n);
        divan::black_box(compile_full(&source));
    }

    #[divan::bench(args = [10, 50, 100])]
    fn many_properties(n: usize) {
        let source = generate_many_properties(n);
        divan::black_box(compile_full(&source));
    }

    #[divan::bench(args = [5, 10, 20])]
    fn deep_expressions(depth: usize) {
        let source = generate_deep_expressions(depth);
        divan::black_box(compile_full(&source));
    }

    #[divan::bench(args = [5, 10, 15])]
    fn nested_components(depth: usize) {
        let source = generate_nested_components(depth);
        divan::black_box(compile_full(&source));
    }

    #[divan::bench(args = [10, 50, 100])]
    fn binding_chain(length: usize) {
        let source = generate_binding_chain(length);
        divan::black_box(compile_full(&source));
    }

    /// Realistic export counts: typical app (5), std-widgets (20), material library (60)
    #[divan::bench(args = [5, 20, 60])]
    fn many_exports(n: usize) {
        let source = generate_many_exports(n);
        divan::black_box(compile_full(&source));
    }

    #[divan::bench(args = [5, 10, 20])]
    fn many_states(n: usize) {
        let source = generate_many_states(n);
        divan::black_box(compile_full(&source));
    }
}

mod expression_complexity {
    use super::*;

    /// Stress test for binary expression allocation
    #[divan::bench(args = [10, 25, 50])]
    fn binary_expression_chain(n: usize) {
        let source = generate_deep_expressions(n);
        divan::black_box(compile_full(&source));
    }

    /// Stress test for struct field access chains
    #[divan::bench(args = [3, 5, 8])]
    fn struct_field_access_chain(depth: usize) {
        let mut struct_def = String::from("export struct Level0 { value: int }\n");
        for i in 1..depth {
            struct_def.push_str(&format!(
                "export struct Level{i} {{ inner: Level{prev} }}\n",
                prev = i - 1
            ));
        }
        let access = (0..depth - 1).fold(String::from("data"), |acc, _| format!("{acc}.inner"));
        let source = format!(
            r#"
{struct_def}
export component FieldAccess inherits Rectangle {{
property <Level{last}> data;
property <int> result: {access}.value;
width: 100px;
height: 100px;
}}
"#,
            last = depth - 1
        );
        divan::black_box(compile_full(&source));
    }
}

/// Benchmark simulating the proc-macro pipeline (compile + Rust code generation).
///
/// This module measures the full cost of processing a slint! macro invocation,
/// providing a baseline for tracking down slow proc-macro expansion in rust-analyzer.
mod proc_macro_simulation {
    use super::*;

    /// Compile and generate Rust code (simulates what the slint! proc-macro does)
    fn compile_to_rust(source: &str) -> (proc_macro2::TokenStream, BuildDiagnostics) {
        let diagnostics = BuildDiagnostics::default();
        let node = parse_source(source);

        let config = CompilerConfiguration::new(i_slint_compiler::generator::OutputFormat::Rust);

        let (doc, diag, loader) =
            spin_on::spin_on(i_slint_compiler::compile_syntax_node(node, diagnostics, config));

        let rust_code = i_slint_compiler::generator::rust::generate(&doc, &loader.compiler_config)
            .expect("Rust code generation failed");

        (rust_code, diag)
    }

    /// Baseline benchmark: empty component through full proc-macro pipeline.
    /// This measures the minimum overhead of proc-macro expansion.
    #[divan::bench]
    fn empty_component() {
        divan::black_box(compile_to_rust(EMPTY_COMPONENT));
    }
}

/// Detailed phase benchmarks to identify hotspots in compilation.
mod phase_breakdown {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// Phase 1: Just parsing (lexing + parsing)
    #[divan::bench]
    fn phase1_parsing() {
        divan::black_box(parse_source(EMPTY_COMPONENT));
    }

    /// Phase 2: Create TypeLoader (includes style resolution)
    #[divan::bench]
    fn phase2_prepare_compile() {
        let mut diag = BuildDiagnostics::default();
        let config = CompilerConfiguration::new(i_slint_compiler::generator::OutputFormat::Rust);
        divan::black_box(i_slint_compiler::typeloader::TypeLoader::new(config, &mut diag));
    }

    /// Phase 3: Load dependencies (slint-widgets.slint and its deps)
    #[divan::bench]
    fn phase3_load_dependencies() {
        let mut diag = BuildDiagnostics::default();
        let config = CompilerConfiguration::new(i_slint_compiler::generator::OutputFormat::Rust);
        let mut loader = i_slint_compiler::typeloader::TypeLoader::new(config, &mut diag);
        let doc_node: i_slint_compiler::parser::syntax_nodes::Document =
            parse_source(EMPTY_COMPONENT).into();
        let type_registry = Rc::new(RefCell::new(
            i_slint_compiler::typeregister::TypeRegister::new(&loader.global_type_registry),
        ));
        divan::black_box(spin_on::spin_on(loader.load_dependencies_recursively(
            &doc_node,
            &mut diag,
            &type_registry,
        )));
    }

    /// Phase 4: Full compile_syntax_node (parsing + loading + passes)
    #[divan::bench]
    fn phase4_compile_syntax_node() {
        let diagnostics = BuildDiagnostics::default();
        let node = parse_source(EMPTY_COMPONENT);
        let config = CompilerConfiguration::new(i_slint_compiler::generator::OutputFormat::Rust);
        divan::black_box(spin_on::spin_on(i_slint_compiler::compile_syntax_node(
            node,
            diagnostics,
            config,
        )));
    }

    /// Phase 5: Just Rust code generation (given already compiled doc)
    #[divan::bench]
    fn phase5_rust_codegen() {
        // First compile to get the document
        let diagnostics = BuildDiagnostics::default();
        let node = parse_source(EMPTY_COMPONENT);
        let config = CompilerConfiguration::new(i_slint_compiler::generator::OutputFormat::Rust);
        let (doc, _diag, loader) =
            spin_on::spin_on(i_slint_compiler::compile_syntax_node(node, diagnostics, config));

        // Now benchmark just the code generation
        divan::black_box(
            i_slint_compiler::generator::rust::generate(&doc, &loader.compiler_config).unwrap(),
        );
    }

    /// Phase 4a: Document::from_node (creates object tree from syntax)
    #[divan::bench]
    fn phase4a_document_from_node() {
        let mut diag = BuildDiagnostics::default();
        let config = CompilerConfiguration::new(i_slint_compiler::generator::OutputFormat::Rust);
        let mut loader = i_slint_compiler::typeloader::TypeLoader::new(config, &mut diag);
        let doc_node: i_slint_compiler::parser::syntax_nodes::Document =
            parse_source(EMPTY_COMPONENT).into();
        let type_registry = Rc::new(RefCell::new(
            i_slint_compiler::typeregister::TypeRegister::new(&loader.global_type_registry),
        ));
        let (foreign_imports, reexports) = spin_on::spin_on(loader.load_dependencies_recursively(
            &doc_node,
            &mut diag,
            &type_registry,
        ));

        // Benchmark just Document::from_node
        divan::black_box(i_slint_compiler::object_tree::Document::from_node(
            doc_node,
            foreign_imports,
            reexports,
            &mut diag,
            &type_registry,
        ));
    }

    /// Phase 4b: run_passes (all compiler passes)
    #[divan::bench]
    fn phase4b_run_passes() {
        let mut diag = BuildDiagnostics::default();
        let config = CompilerConfiguration::new(i_slint_compiler::generator::OutputFormat::Rust);
        let mut loader = i_slint_compiler::typeloader::TypeLoader::new(config, &mut diag);
        let doc_node: i_slint_compiler::parser::syntax_nodes::Document =
            parse_source(EMPTY_COMPONENT).into();
        let type_registry = Rc::new(RefCell::new(
            i_slint_compiler::typeregister::TypeRegister::new(&loader.global_type_registry),
        ));
        let (foreign_imports, reexports) = spin_on::spin_on(loader.load_dependencies_recursively(
            &doc_node,
            &mut diag,
            &type_registry,
        ));
        let mut doc = i_slint_compiler::object_tree::Document::from_node(
            doc_node,
            foreign_imports,
            reexports,
            &mut diag,
            &type_registry,
        );

        // Benchmark just run_passes
        divan::black_box(spin_on::spin_on(i_slint_compiler::passes::run_passes(
            &mut doc,
            &mut loader,
            false,
            &mut diag,
        )));
    }

    /// Phase 4b1: Just import StyleMetrics and Palette (start of run_passes)
    #[divan::bench]
    fn phase4b1_import_style_components() {
        let mut diag = BuildDiagnostics::default();
        let config = CompilerConfiguration::new(i_slint_compiler::generator::OutputFormat::Rust);
        let mut loader = i_slint_compiler::typeloader::TypeLoader::new(config, &mut diag);

        // Benchmark just the import_component calls
        let mut build_diags_to_ignore = BuildDiagnostics::default();
        let _style_metrics = spin_on::spin_on(loader.import_component(
            "slint-widgets.slint",
            "StyleMetrics",
            &mut build_diags_to_ignore,
        ));
        let _palette = spin_on::spin_on(loader.import_component(
            "slint-widgets.slint",
            "Palette",
            &mut build_diags_to_ignore,
        ));

        // avoid the unused variables being optimized away
        divan::black_box((_style_metrics, _palette));
    }
}

fn main() {
    divan::main();
}
