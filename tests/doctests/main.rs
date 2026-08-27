// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![allow(uncommon_codepoints)]

/// Whether a snippet is a fragment that needs wrapping in a component,
/// as opposed to a complete file with top-level items.
#[cfg(test)]
fn must_wrap(snippet: &str) -> bool {
    !["component ", "global ", "struct ", "enum ", "import ", "export "]
        .iter()
        .any(|kw| snippet.contains(kw))
}

#[cfg(test)]
fn do_test(snippet: &str, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Strip rustdoc-style `# ` prefix from hidden lines
    let snippet: String = snippet
        .lines()
        .map(|line| {
            if let Some(rest) = line.strip_prefix("# ") {
                rest
            } else if line == "#" {
                ""
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let snippet = snippet.as_str();

    let code = if must_wrap(snippet) {
        format!(
            "import {{
                Button, CheckBox, ComboBox, DatePickerPopup, LineEdit, Palette, ProgressIndicator,
                ScrollView, Slider, SpinBox, Spinner, StandardButton, StandardListView,
                StandardTableView, RadioGroup,
                Switch, TabWidget, TextEdit, TimePickerPopup}} from\"std-widgets.slint\";
            component Example {{\n{snippet}\n}}"
        )
    } else {
        snippet.into()
    };

    let mut compiler = slint_interpreter::Compiler::default();
    compiler.set_library_paths(
        std::iter::once((
            "material".into(),
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../ui-libraries/material/src/material.slint"),
        ))
        .collect(),
    );
    let result = spin_on::spin_on(compiler.build_from_source(code, path.into()));

    let diagnostics = result
        .diagnostics()
        .filter(|d| {
            let msg = d.message();
            // It is ok if there is no components
            msg != "No component found"
                // Ignore warning about examples that don't inherit from Window or not exported
                && !msg.contains(" doesn't inherit Window.")
                && msg != "Component is implicitly marked for export. This is deprecated and it should be explicitly exported"

        })
        .collect::<Vec<_>>();
    slint_interpreter::print_diagnostics(&diagnostics);

    if result.has_errors() && !diagnostics.is_empty() {
        return Err(format!("Error when loading {snippet:?} in {path:?}: {diagnostics:?}").into());
    }
    Ok(())
}

/// Compile a Language Specification snippet in Slint SC mode.
/// Snippets that the SC subset doesn't accept yet carry a `no-sc-test`
/// fence marker and are skipped by the build script.
#[cfg(test)]
fn do_test_sc(snippet: &str, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let code = if must_wrap(snippet) {
        format!("export component Example inherits Window {{\n{snippet}\n}}")
    } else {
        snippet.into()
    };

    let mut diag = i_slint_compiler::diagnostics::BuildDiagnostics::default();
    let node = i_slint_compiler::parser::parse(code, Some(std::path::Path::new(path)), &mut diag);
    let config = i_slint_compiler::CompilerConfiguration::new(
        i_slint_compiler::generator::OutputFormat::SlintSc,
    );
    let (doc, diag, loader) =
        spin_on::spin_on(i_slint_compiler::compile_syntax_node(node, diag, config));
    if diag.has_errors() {
        diag.print();
        return Err(format!("Error when compiling {snippet:?} in {path:?} in Slint SC mode").into());
    }
    i_slint_compiler::generator::generate(
        i_slint_compiler::generator::OutputFormat::SlintSc,
        &mut std::io::sink(),
        None,
        &doc,
        &loader.compiler_config,
    )?;
    Ok(())
}

include!(env!("TEST_FUNCTIONS"));

fn main() {
    println!("Nothing to see here, please run me through cargo test :)");
}
