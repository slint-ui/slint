// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![allow(uncommon_codepoints)]

#[cfg(test)]
fn do_test(snippet: &str, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let must_wrap = !snippet.contains("component ") && !snippet.contains("global ");

    let code = if must_wrap {
        format!(
            "import {{
                Button, CheckBox, ComboBox, DatePickerPopup, LineEdit, ProgressIndicator, ScrollView,
                Slider, SpinBox, Spinner, StandardButton, StandardListView, StandardTableView,
                Switch, TabWidget, TextEdit, TimePickerPopup}} from\"std-widgets.slint\";
            component Example {{\n{snippet}\n}}"
        )
    } else {
        snippet.into()
    };

    let compiler = slint_interpreter::Compiler::default();
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
