// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::Context;
use crate::language::completion;
use crate::util;

use i_slint_compiler::langtype::Type;
use i_slint_compiler::object_tree::QualifiedTypeName;
use i_slint_compiler::parser::{SyntaxKind, syntax_nodes};
use i_slint_compiler::typeregister::TypeRegister;
use lsp_types::{CodeActionKind, CodeActionOrCommand};

pub(super) fn add(ctx: &Context, result: &mut Vec<CodeActionOrCommand>) {
    if ctx.token.kind() != SyntaxKind::Identifier || ctx.node.kind() != SyntaxKind::QualifiedName {
        return;
    }
    let normalized_name = i_slint_compiler::parser::normalize_identifier(ctx.token.text());
    match ctx.node.parent().map(|n| n.kind()) {
        Some(SyntaxKind::Element) => add_component_import(ctx, &normalized_name, result),
        Some(SyntaxKind::Type) => add_type_import(ctx, &normalized_name, result),
        _ => {}
    }
}

fn add_component_import(
    ctx: &Context,
    normalized_name: &str,
    result: &mut Vec<CodeActionOrCommand>,
) {
    if with_type_register(ctx, |type_register| {
        util::lookup_current_element_type(ctx.node.clone(), type_register).is_some()
    }) {
        return;
    }
    completion::build_component_import_statements_edits(
        &ctx.token,
        ctx.document_cache,
        &mut |component_info| {
            !component_info.is_global
                && component_info.is_exported
                && component_info.name == normalized_name
        },
        &mut |name, file, edit| {
            ctx.push_edit(
                result,
                format!("import {{ {name} }} from \"{file}\""),
                Some(CodeActionKind::QUICKFIX),
                vec![edit],
            )
        },
    );
}

fn add_type_import(ctx: &Context, normalized_name: &str, result: &mut Vec<CodeActionOrCommand>) {
    let Some(qualified_name) = syntax_nodes::QualifiedName::new(ctx.node.clone()) else {
        return;
    };
    let members = QualifiedTypeName::from_node(qualified_name).members;
    if !with_type_register(ctx, |type_register| {
        matches!(type_register.lookup_qualified(&members), Type::Invalid)
    }) {
        return;
    }
    completion::build_type_import_statements_edits(
        &ctx.token,
        ctx.document_cache,
        &mut |type_info| type_info.name == normalized_name,
        &mut |_type_info, name, file, edit| {
            ctx.push_edit(
                result,
                format!("import {{ {name} }} from \"{file}\""),
                Some(CodeActionKind::QUICKFIX),
                vec![edit],
            )
        },
    );
}

fn with_type_register<R>(ctx: &Context, lookup: impl FnOnce(&TypeRegister) -> R) -> R {
    let global_type_register = ctx.document_cache.global_type_registry();
    lookup(
        ctx.document_cache
            .get_document_for_source_file(&ctx.token.source_file)
            .map(|doc| &doc.local_registry)
            .unwrap_or(&global_type_register),
    )
}

#[cfg(test)]
mod tests {
    use crate::language::code_actions::test::CodeActionRequest;

    #[test]
    fn test_code_action_import_unresolved_component() {
        let actions = CodeActionRequest::at(
            r#"import { Button } from "std-widgets.slint";

export component Test {
    input := 🔺LineEdit { }
}
"#,
        )
        .load();
        assert_eq!(actions.titles(), ["import { LineEdit } from \"std-widgets.slint\""]);
        actions
            .assert(
                "import { LineEdit } from \"std-widgets.slint\"",
                Some(lsp_types::CodeActionKind::QUICKFIX),
                r#"import { Button, LineEdit } from "std-widgets.slint";

export component Test {
    input := LineEdit { }
}
"#,
            )
            .compiles();
    }

    const MY_POINT_TYPES: &str = "export struct MyPoint { x: int, y: int }\n";

    #[test]
    fn test_add_import_for_type_annotation() {
        CodeActionRequest::at(
            r#"export component TestWindow inherits Window {
    property <🔺MyPoint> position;
}
"#,
        )
        .with_file("types.slint", MY_POINT_TYPES)
        .load()
        .assert(
            "import { MyPoint } from \"types.slint\"",
            Some(lsp_types::CodeActionKind::QUICKFIX),
            r#"import { MyPoint } from "types.slint";
export component TestWindow inherits Window {
    property <MyPoint> position;
}
"#,
        )
        .compiles();
    }

    #[test]
    fn test_no_import_for_imported_type_annotation() {
        CodeActionRequest::at(
            r#"import { MyPoint } from "types.slint";

export component TestWindow inherits Window {
    property <🔺MyPoint> position;
}
"#,
        )
        .with_file("types.slint", MY_POINT_TYPES)
        .load()
        .assert_empty();
    }

    #[test]
    fn test_add_import_for_name_with_a_separator() {
        const TYPES: &str =
            "export struct My_Struct { x: int }\nexport component My_Component { }\n";
        for (name, main) in [
            (
                "My-Struct",
                "export component TestWindow inherits Window {\n    property <My_Struct> position;\n}\n",
            ),
            (
                "My-Component",
                "export component TestWindow inherits Window {\n    My_Component { }\n}\n",
            ),
        ] {
            let title = format!("import {{ {name} }} from \"types.slint\"");
            let actions = CodeActionRequest::at_offset(main, main.find("My_").unwrap())
                .with_file("types.slint", TYPES)
                .load();
            assert_eq!(actions.titles(), [title.as_str()]);
            actions
                .assert(
                    &title,
                    Some(lsp_types::CodeActionKind::QUICKFIX),
                    &format!("import {{ {name} }} from \"types.slint\";\n{main}"),
                )
                .compiles();
        }
    }
}
