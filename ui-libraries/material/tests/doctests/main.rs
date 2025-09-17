// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#![allow(uncommon_codepoints)]

#[cfg(test)]
fn do_test(snippet: &str, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let must_wrap = !snippet.contains("component ") && !snippet.contains("global ");

    let code = if must_wrap {
        format!(
            "import {{
                CheckBox, CheckState, Switch, TimePickerPopup, NavigationBar}} from\"material.slint\";
            component Example {{\n{snippet}\n}}"
        )
    } else {
        snippet.into()
    };

    let mut compiler = slint_interpreter::Compiler::default();

    compiler.set_include_paths(vec![std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../"
    ))]);

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

include!(env!("TEST_FUNCTIONS"));

fn main() {
    println!("Nothing to see here, please run me through cargo test :)");
}
