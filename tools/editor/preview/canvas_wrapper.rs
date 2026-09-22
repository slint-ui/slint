// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::{
    langtype::{ElementType, Type},
    object_tree::{Document, PropertyVisibility},
};
use std::rc::Rc;

pub(super) struct Wrapper {
    pub source: String,
    pub name: String,
    pub component_name: String,
}

pub(super) fn wrap(document: &Document, source: &str, selected: Option<&str>) -> Option<Wrapper> {
    let component = if let Some(name) = selected {
        document
            .exports
            .iter()
            .find_map(|(export, item)| {
                let c = item.as_ref().left()?;
                (export.name == name || c.id == name).then(|| c.clone())
            })
            .or_else(|| match document.local_registry.lookup_element(name).ok()? {
                ElementType::Component(c) => Some(c),
                _ => None,
            })?
    } else {
        document.last_exported_component()?
    };
    let base =
        document.local_registry.all_elements().into_iter().find_map(
            |(name, element)| match element {
                ElementType::Component(c) if Rc::ptr_eq(&c, &component) => Some(name),
                _ => None,
            },
        )?;
    let mut name = "SlintEditorCanvasPreview".to_string();
    while source.contains(&name) {
        name.push('X');
    }
    let content = format!("{name}-content");
    let mut aliases = String::new();
    for (property, declaration) in &component.root_element.borrow().property_declarations {
        if declaration.visibility == PropertyVisibility::Private
            || !declaration.property_type.ok_for_public_api()
        {
            continue;
        }
        if let Type::Function(function_type) = &declaration.property_type {
            let function =
                i_slint_compiler::parser::syntax_nodes::Function::new(declaration.node.clone()?)?;
            let block = function.CodeBlock()?;
            let offset = usize::from(block.text_range().start() - function.text_range().start());
            let text = function.text().to_string();
            let arguments = function
                .ArgumentDeclaration()
                .map(|arg| arg.DeclaredIdentifier().text().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let returns = if function_type.return_type == Type::Void { "" } else { "return " };
            aliases.push_str(&format!(
                "{}{{ {returns}{content}.{property}({arguments}); }}\n",
                &text[..offset]
            ));
        } else if matches!(declaration.property_type, Type::Callback { .. }) {
            let pure = if declaration.pure == Some(true) { "pure " } else { "" };
            aliases.push_str(&format!("{pure}callback {property} <=> {content}.{property};\n"));
        } else {
            aliases.push_str(&format!(
                "{} property {property} <=> {content}.{property};\n",
                declaration.visibility
            ));
        }
    }
    let ignore = i_slint_editor_preview::NODE_IGNORE_COMMENT;
    Some(Wrapper {
        source: format!(
            "{source}\nexport component {name} inherits Window {{\n{aliases}\n{content} := {base} {{\nx: 0px; y: 0px; width: 100%; height: 100%;\n/* {ignore} */\n}}\n/* {ignore} */\n}}\nexport {{ {base} as {name}Source }}\n"
        ),
        name,
        component_name: component.id.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use slint_interpreter::Value;

    #[test]
    fn source_root_keeps_canvas_geometry_through_wrappers() {
        use slint_interpreter::ComponentHandle;
        let source = "export component Main inherits Window { width: 100px; height: 80px; Rectangle { width: parent.width; height: parent.height; } }";
        let instance = crate::preview::test::interpret_test("fluent", source);
        instance.window().set_size(slint::LogicalSize::new(390., 720.));
        let offset = source.find("Window").unwrap() as u32;
        let root = crate::preview::element_selection::root_element(&instance);
        assert!(root.borrow().debug.iter().any(|d| {
            d.node.QualifiedName().is_some_and(|n| u32::from(n.text_range().start()) == offset)
        }));
        let positions = instance.element_positions(&root);
        assert_eq!(positions.len(), 1);
        assert_eq!((positions[0].rect.width(), positions[0].rect.height()), (390., 720.));
    }

    #[test]
    fn preserves_preview_properties_callbacks_and_functions() {
        let instance = crate::preview::test::interpret_test(
            "fluent",
            r#"
            export component Main inherits Window {
                in-out property <int> count: 4;
                out property <int> doubled: count * 2;
                callback add(int) -> int;
                add(value) => { count += value; return count; }
                public pure function multiply(value: int) -> int { return count * value; }
            }
        "#,
        );
        assert_eq!(instance.get_property("count"), Ok(Value::Number(4.)));
        instance.set_property("count", Value::Number(7.)).unwrap();
        assert_eq!(instance.get_property("doubled"), Ok(Value::Number(14.)));
        assert_eq!(instance.invoke("multiply", &[Value::Number(3.)]), Ok(Value::Number(21.)));
        assert_eq!(instance.invoke("add", &[Value::Number(2.)]), Ok(Value::Number(9.)));
    }
}
