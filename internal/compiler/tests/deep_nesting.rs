// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Code nested as deep as the parser allows compiles within a 900 KB stack
//! in an unoptimized build, for the interpreter and with the code generators.

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::generator::{self, OutputFormat};
use i_slint_compiler::parser::parse;
use i_slint_compiler::{CompilerConfiguration, compile_syntax_node};

/// The source that `make` generates for the deepest nesting the parser accepts.
fn deepest(make: impl Fn(usize) -> String) -> String {
    let parses = |n| {
        let mut diagnostics = BuildDiagnostics::default();
        parse(make(n), None, &mut diagnostics);
        !diagnostics.has_errors()
    };
    let mut n = 1;
    while parses(n + 1) {
        n += 1;
    }
    make(n)
}

fn compile_deepest_on_small_stack(make: impl Fn(usize) -> String) {
    compile_on_small_stack(deepest(make));
}

fn compile_on_small_stack(source: String) {
    let mut formats = vec![OutputFormat::Interpreter];
    #[cfg(feature = "cpp")]
    formats.push(OutputFormat::Cpp(Default::default()));
    #[cfg(feature = "rust")]
    formats.push(OutputFormat::Rust);
    for format in formats {
        let source = source.clone();
        let format_name = format!("{format:?}");
        std::thread::Builder::new()
            .stack_size(900 * 1024)
            .spawn(move || {
                let mut diagnostics = BuildDiagnostics::default();
                let syntax_node = parse(source, None, &mut diagnostics);
                let config = CompilerConfiguration::new(format.clone());
                let (doc, diagnostics, loader) =
                    spin_on::spin_on(compile_syntax_node(syntax_node, diagnostics, config));
                assert!(!diagnostics.has_errors(), "{:?}", diagnostics.to_string_vec());
                if format == OutputFormat::Interpreter {
                    return;
                }
                generator::generate(
                    format,
                    &mut std::io::sink(),
                    None,
                    &doc,
                    &loader.compiler_config,
                )
                .unwrap();
            })
            .unwrap()
            .join()
            .unwrap_or_else(|_| panic!("compiling to {format_name} failed"));
    }
}

fn component(body: &str) -> String {
    format!("export component Test inherits Window {{ in property <int> v; {body} }}")
}

#[test]
fn conditional_expression_chain() {
    compile_deepest_on_small_stack(|n| {
        let chain: String = (0..n).map(|i| format!("v == {i} ? {i} : ")).collect();
        component(&format!("out property <int> o: {chain} -1;"))
    });
}

#[test]
fn if_statement_chain() {
    compile_deepest_on_small_stack(|n| {
        let mut code = "1".to_string();
        for i in 0..n {
            code = format!("if v == {i} {{ {i} }} else {{ {code} }}");
        }
        component(&format!("out property <int> o: {{ {code} }}"))
    });
}

#[test]
fn nested_function_calls() {
    compile_deepest_on_small_stack(|n| {
        let code = format!("{}v{}", "Math.abs(".repeat(n), ")".repeat(n));
        component(&format!("out property <int> o: {code};"))
    });
}

#[test]
fn nested_arrays() {
    compile_deepest_on_small_stack(|n| {
        let code = format!("{}1{}{}", "[".repeat(n), "]".repeat(n), "[0]".repeat(n));
        component(&format!("out property <int> o: {code};"))
    });
}

#[test]
fn nested_elements() {
    compile_deepest_on_small_stack(|n| {
        component(&format!("{}{}", "Rectangle { ".repeat(n), "}".repeat(n)))
    });
}

#[test]
fn nested_conditional_elements() {
    compile_deepest_on_small_stack(|n| {
        component(&format!("{}{}", "if v > 0 : Rectangle { ".repeat(n), "}".repeat(n)))
    });
}

#[test]
fn nested_repeated_elements() {
    compile_deepest_on_small_stack(|n| {
        component(&format!("{}{}", "for i in 2 : Rectangle { ".repeat(n), "}".repeat(n)))
    });
}

// The parser doesn't count the nesting below, so these test a depth that fits today.

#[test]
fn binary_operator_chain() {
    let chain = vec!["v"; 120].join(" + ");
    compile_on_small_stack(component(&format!("out property <int> o: {chain};")));
}

#[test]
fn method_call_chain() {
    let chain = ".abs()".repeat(45);
    compile_on_small_stack(component(&format!("out property <float> o: v{chain};")));
}

#[test]
fn nested_popup_windows() {
    let n = 40;
    compile_on_small_stack(component(&format!("{}{}", "PopupWindow { ".repeat(n), "}".repeat(n))));
}

#[test]
fn nested_layouts() {
    let n = 50;
    compile_on_small_stack(component(&format!(
        "{}{}",
        "HorizontalLayout { ".repeat(n),
        "}".repeat(n)
    )));
}
