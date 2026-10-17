// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! A document whose formatted text can be selected with the mouse and copied.
//! The search box selects the first match with `set-selection-offsets`.

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = MainWindow::new()?;

    let weak = ui.as_weak();
    ui.on_search(move |query| {
        let Some(ui) = weak.upgrade() else {
            return;
        };

        let query = query.to_string();
        if query.is_empty() {
            ui.invoke_clear_selection();
            ui.set_status("Select text with the mouse, or search above.".into());
            return;
        }

        // The selection's byte offsets index the text without markup, so the match's
        // position in `document-text` is exactly the range to select.
        match ui.get_document_text().find(query.as_str()) {
            Some(start) => {
                let end = start + query.len();
                ui.invoke_select_range(start as i32, end as i32);
                ui.set_status(format!("Selected the first match of \"{query}\"").into());
            }
            None => {
                ui.invoke_clear_selection();
                ui.set_status(format!("No match for \"{query}\"").into());
            }
        }
    });

    ui.run()
}
