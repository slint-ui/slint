// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore rfind

use super::Context;

use lsp_types::CodeActionOrCommand;

pub(super) fn add(ctx: &Context, result: &mut Vec<CodeActionOrCommand>) {
    let Some(component) = &ctx.component else {
        return;
    };
    let Some(component_name) =
        i_slint_compiler::parser::identifier_text(&component.DeclaredIdentifier())
    else {
        return;
    };
    result.push(CodeActionOrCommand::Command(crate::language::create_show_preview_command(
        false,
        &ctx.uri,
        &component_name,
    )));
}

#[cfg(test)]
mod tests {
    use crate::language::code_actions::test::CodeActionRequest;

    #[test]
    fn test_code_action_show_preview() {
        const SOURCE: &str = "export component TestWindow inherits Window { }\n";
        let url = crate::editor_preview::test::main_test_file_name().to_url().unwrap();
        for offset in [
            SOURCE.find("export").unwrap(),
            SOURCE.find("component").unwrap(),
            SOURCE.find("TestWindow").unwrap(),
            SOURCE.rfind("Window").unwrap(),
        ] {
            let actions = CodeActionRequest::at_offset(SOURCE, offset).load();
            assert_eq!(actions.titles(), ["Show Preview"], "offset {offset}");
            assert_eq!(
                actions.command("Show Preview"),
                &lsp_types::Command::new(
                    "Show Preview".into(),
                    crate::language::SHOW_PREVIEW_COMMAND.into(),
                    Some(vec![url.as_str().into(), "TestWindow".into()]),
                )
            );
        }
    }
}
