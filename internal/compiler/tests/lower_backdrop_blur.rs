// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::expression_tree::Expression;
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::object_tree::{ElementRc, recurse_elem};
use i_slint_compiler::parser::parse;
use i_slint_compiler::{CompilerConfiguration, compile_syntax_node};
use smol_str::ToSmolStr;

fn compile(source: &str) -> ElementRc {
    let mut diagnostics = BuildDiagnostics::default();
    let syntax_node = parse(source.into(), None, &mut diagnostics);
    let compiler_config = CompilerConfiguration::new(OutputFormat::Interpreter);
    let (doc, diagnostics, _) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diagnostics, compiler_config));
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.to_string_vec());
    doc.last_exported_component().unwrap().root_element.clone()
}

fn find_parent_of(root: &ElementRc, base_type: &str) -> ElementRc {
    let mut result = None;
    recurse_elem(root, &(), &mut |element, _| {
        if element.borrow().children.iter().any(|c| c.borrow().base_type.to_smolstr() == base_type)
        {
            result = Some(element.clone());
        }
    });
    result.unwrap_or_else(|| panic!("{base_type} element should be generated"))
}

fn child_types(element: &ElementRc) -> Vec<String> {
    element.borrow().children.iter().map(|c| c.borrow().base_type.to_string()).collect()
}

#[test]
fn backdrop_blur_is_drawn_beneath_the_drop_shadow() {
    let root = compile(
        r#"
export component TestCase inherits Window {
    in-out property <length> radius: 24px;
    Rectangle {
        background: #fff4;
        border-radius: root.radius;
        backdrop-blur: 8px;
        drop-shadow-blur: 8px;
        drop-shadow-color: black;
    }
}
"#,
    );

    let parent = find_parent_of(&root, "BackdropBlur");
    assert_eq!(child_types(&parent), ["BackdropBlur", "BoxShadow", "Rectangle"]);

    let blur = parent.borrow().children[0].clone();
    for property_name in [
        "blur",
        "border-top-left-radius",
        "border-top-right-radius",
        "border-bottom-right-radius",
        "border-bottom-left-radius",
    ] {
        assert!(blur.borrow().binding(property_name).is_some(), "{property_name} binding missing");
    }
    let blur_ref = blur.borrow();
    let radius = blur_ref.binding("border-top-left-radius").unwrap();
    assert!(matches!(radius.expression.ignore_debug_hooks(), Expression::PropertyReference(_)));
}

#[test]
fn backdrop_blur_is_inside_the_opacity_element() {
    let root = compile(
        r#"
export component TestCase inherits Window {
    Rectangle {
        background: #fff4;
        backdrop-blur: 8px;
        opacity: 0.5;
    }
}
"#,
    );

    let parent = find_parent_of(&root, "BackdropBlur");
    assert_eq!(parent.borrow().base_type.to_smolstr(), "Opacity");
    assert_eq!(child_types(&parent), ["BackdropBlur", "Rectangle"]);
    assert!(opacity_wraps_backdrop_blur(&root));
}

#[test]
fn backdrop_blur_on_a_repeated_rectangle() {
    let root = compile(
        r#"
export component TestCase inherits Window {
    for i in 3: Rectangle {
        y: i * 20px;
        background: #fff4;
        backdrop-blur: 8px;
        drop-shadow-blur: 8px;
        drop-shadow-color: black;
    }
}
"#,
    );

    let mut repeated_root = None;
    recurse_elem(&root, &(), &mut |element, _| {
        if element.borrow().repeated.is_some() {
            repeated_root = Some(element.borrow().base_type.as_component().root_element.clone());
        }
    });
    let repeated_root = repeated_root.expect("repeater should exist");
    assert_eq!(repeated_root.borrow().base_type.to_smolstr(), "BackdropBlur");
    assert_eq!(child_types(&repeated_root), ["BoxShadow"]);
    assert_eq!(child_types(&repeated_root.borrow().children[0]), ["Rectangle"]);
}

#[test]
fn backdrop_blur_on_a_component_root_is_inlined() {
    let root = compile(
        r#"
component Glass inherits Rectangle {
    background: #fff4;
    backdrop-blur: 8px;
}
export component TestCase inherits Window {
    Glass { }
}
"#,
    );

    let parent = find_parent_of(&root, "BackdropBlur");
    assert_eq!(child_types(&parent), ["BackdropBlur", "Rectangle"]);
}

fn opacity_wraps_backdrop_blur(root: &ElementRc) -> bool {
    wraps_backdrop_blur(root, "Opacity")
}

fn wraps_backdrop_blur(root: &ElementRc, base_type: &str) -> bool {
    let mut result = None;
    recurse_elem(root, &(), &mut |element, _| {
        if element.borrow().base_type.to_smolstr() == base_type {
            result = Some(element.borrow().is_binding_set("wraps-backdrop-blur", false));
        }
    });
    result.unwrap_or_else(|| panic!("{base_type} element should be generated"))
}

#[test]
fn own_cache_rendering_hint_is_marked() {
    let root = compile(
        r#"
export component TestCase inherits Window {
    Rectangle {
        background: #fff4;
        opacity: 0.5;
        cache-rendering-hint: true;
        backdrop-blur: 8px;
    }
}
"#,
    );
    assert!(wraps_backdrop_blur(&root, "Layer"));
    assert!(opacity_wraps_backdrop_blur(&root));
}

#[test]
fn ancestor_cache_rendering_hint_is_not_marked() {
    let root = compile(
        r#"
export component TestCase inherits Window {
    Rectangle {
        cache-rendering-hint: true;
        Rectangle { background: #fff4; backdrop-blur: 8px; }
    }
}
"#,
    );
    assert!(!wraps_backdrop_blur(&root, "Layer"));
}

#[test]
fn backdrop_blur_assigned_in_a_callback() {
    let root = compile(
        r#"
export component TestCase inherits Window {
    glass := Rectangle {
        background: #fff4;
        opacity: 0.5;
    }
    TouchArea {
        clicked => { glass.backdrop-blur = 8px; }
    }
}
"#,
    );

    let parent = find_parent_of(&root, "BackdropBlur");
    assert_eq!(child_types(&parent), ["BackdropBlur", "Rectangle"]);
    assert!(opacity_wraps_backdrop_blur(&root));
    assert!(parent.borrow().children[0].borrow().binding("blur").is_some());
}

#[test]
fn own_opacity_of_a_repeated_rectangle_is_marked() {
    let root = compile(
        r#"
export component TestCase inherits Window {
    for i in 2: Rectangle {
        background: #fff4;
        opacity: 0.5;
        transform-rotation: 10deg;
        backdrop-blur: 8px;
    }
}
"#,
    );
    let mut repeated_root = None;
    recurse_elem(&root, &(), &mut |element, _| {
        if element.borrow().repeated.is_some() {
            repeated_root = Some(element.borrow().base_type.as_component().root_element.clone());
        }
    });
    assert!(opacity_wraps_backdrop_blur(&repeated_root.expect("repeater should exist")));
}

// Like CSS, an ancestor's opacity group is a backdrop root.
#[test]
fn ancestor_opacity_is_not_marked() {
    for source in [
        r#"
export component TestCase inherits Window {
    Rectangle {
        opacity: 0.5;
        Rectangle { background: #fff4; backdrop-blur: 8px; }
    }
}
"#,
        r#"
component Panel {
    Rectangle { background: #fff4; backdrop-blur: 8px; }
}
export component TestCase inherits Window {
    Rectangle {
        opacity: 0.5;
        Panel { }
    }
}
"#,
        r#"
export component TestCase inherits Window {
    in property <bool> show;
    Rectangle {
        opacity: 0.5;
        if root.show: Rectangle { background: #fff4; backdrop-blur: 8px; }
    }
}
"#,
    ] {
        assert!(!opacity_wraps_backdrop_blur(&compile(source)), "{source}");
    }
}

#[test]
fn opacity_without_backdrop_blur_is_not_marked() {
    let root = compile(
        r#"
export component TestCase inherits Window {
    Rectangle {
        opacity: 0.5;
        Rectangle { background: red; }
    }
}
"#,
    );
    assert!(!opacity_wraps_backdrop_blur(&root));
}
