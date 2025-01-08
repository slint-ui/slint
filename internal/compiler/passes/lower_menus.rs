// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Passe lower the `MenuBar` and `ContextMenu` as well as all their contents
//!
//! Must be done before inlining and many other passes because the lowered code must
//! be further inlined as it may expends to native widget that needs inlining

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::expression_tree::{BuiltinFunction, Expression, NamedReference};
use crate::langtype::{ElementType, Type};
use crate::object_tree::*;
use core::cell::RefCell;
use smol_str::{format_smolstr, SmolStr};

struct UsefulMenuComponents {
    menubar_impl: ElementType,
    vertical_layout: ElementType,
    empty: ElementType,
}

pub async fn lower_menus(
    doc: &mut Document,
    type_loader: &mut crate::typeloader::TypeLoader,
    diag: &mut BuildDiagnostics,
) {
    // Ignore import errors
    let mut build_diags_to_ignore = BuildDiagnostics::default();

    let menubar_impl = type_loader
        .import_component("std-widgets.slint", "MenuBarImpl", &mut build_diags_to_ignore)
        .await
        .expect("MenuBarImpl should be in std-widgets.slint");
    let useful_menu_component = UsefulMenuComponents {
        menubar_impl: menubar_impl.clone().into(),
        vertical_layout: type_loader
            .global_type_registry
            .borrow()
            .lookup_builtin_element("VerticalLayout")
            .expect("VerticalLayout is a builtin type"),
        empty: type_loader.global_type_registry.borrow().empty_type(),
    };

    let mut has_menu = false;

    doc.visit_all_used_components(|component| {
        recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
            if matches!(&elem.borrow().builtin_type(), Some(b) if b.name == "Window") {
                has_menu |= process_window(elem, &useful_menu_component, diag);
            }
            if matches!(&elem.borrow().builtin_type(), Some(b) if b.name == "ContextMenu") {
                has_menu |= process_context_menu(elem, &useful_menu_component, diag);
            }
        })
    });

    if has_menu {
        let popup_menu_impl = type_loader
            .import_component("std-widgets.slint", "PopupMenuImpl", &mut build_diags_to_ignore)
            .await
            .expect("PopupMenuImpl should be in std-widgets.slint");
        {
            let mut root = popup_menu_impl.root_element.borrow_mut();

            for prop in ["entries", "sub-menu", "activated"] {
                match root.property_declarations.get_mut(prop) {
                    Some(d) => d.expose_in_public_api = true,
                    None => diag.push_error(format!("PopupMenuImpl doesn't have {prop}"), &*root),
                }
            }
            root.property_analysis.borrow_mut().entry("entries".into()).or_default().is_set = true;
        }

        recurse_elem_including_sub_components_no_borrow(&popup_menu_impl, &(), &mut |elem, _| {
            if matches!(&elem.borrow().builtin_type(), Some(b) if b.name == "ContextMenu") {
                process_context_menu(elem, &useful_menu_component, diag);
            }
        });
        recurse_elem_including_sub_components_no_borrow(&menubar_impl, &(), &mut |elem, _| {
            if matches!(&elem.borrow().builtin_type(), Some(b) if b.name == "ContextMenu") {
                process_context_menu(elem, &useful_menu_component, diag);
            }
        });
        doc.popup_menu_impl = popup_menu_impl.into();
    }
}

fn process_context_menu(
    _context_menu_elem: &ElementRc,
    _useful_menu_components: &UsefulMenuComponents,
    _diag: &mut BuildDiagnostics,
) -> bool {
    // TODO:
    true
}

fn process_window(
    win: &ElementRc,
    components: &UsefulMenuComponents,
    diag: &mut BuildDiagnostics,
) -> bool {
    /*  if matches!(&elem.borrow_mut().base_type, ElementType::Builtin(_)) {
        // That's the TabWidget re-exported from the style, it doesn't need to be processed
        return;
    }*/

    let mut window = win.borrow_mut();
    let mut menu_bar = None;
    window.children.retain(|x| {
        if matches!(&x.borrow().base_type, ElementType::Builtin(b) if b.name == "MenuBar") {
            if menu_bar.is_some() {
                diag.push_error("Only one MenuBar is allowed in a Window".into(), &*x.borrow());
            } else {
                menu_bar = Some(x.clone());
            }
            false
        } else {
            true
        }
    });

    let Some(menu_bar) = menu_bar else {
        return false;
    };
    if menu_bar.borrow().repeated.is_some() {
        diag.push_error(
            "MenuBar cannot be in a conditional or repeated element".into(),
            &*menu_bar.borrow(),
        );
    }
    assert!(
        menu_bar.borrow().children.is_empty() || diag.has_errors(),
        "MenuBar element can't have children"
    );

    let menubar_impl = Element {
        id: format_smolstr!("{}-menulayout", window.id),
        base_type: components.menubar_impl.clone(),
        enclosing_component: window.enclosing_component.clone(),
        repeated: Some(crate::object_tree::RepeatedElementInfo {
            model: Expression::UnaryOp {
                op: '!',
                sub: Expression::FunctionCall {
                    function: Expression::BuiltinFunctionReference(
                        BuiltinFunction::SupportsNativeMenuBar,
                        None,
                    )
                    .into(),
                    arguments: vec![],
                    source_location: None,
                }
                .into(),
            },
            model_data_id: SmolStr::default(),
            index_id: SmolStr::default(),
            is_conditional_element: true,
            is_listview: None,
        }),
        ..Default::default()
    }
    .make_rc();

    // Create a child that contains all the children of the window but the menubar
    let child = Element {
        id: format_smolstr!("{}-child", window.id),
        base_type: components.empty.clone(),
        enclosing_component: window.enclosing_component.clone(),
        children: std::mem::take(&mut window.children),
        ..Default::default()
    }
    .make_rc();

    const HEIGHT: &str = "height";
    let child_height = NamedReference::new(&child, SmolStr::new_static(HEIGHT));

    const ENTRIES: &str = "entries";
    const SUB_MENU: &str = "sub-menu";
    const ACTIVATE: &str = "activated";

    let source_location = Some(menu_bar.borrow().to_source_location());

    for prop in [ENTRIES, SUB_MENU, ACTIVATE] {
        // materialize the properties and callbacks
        let ty = components.menubar_impl.lookup_property(prop).property_type;
        assert_ne!(ty, Type::Invalid, "Can't lookup type for {prop}");
        let nr = NamedReference::new(&menu_bar, SmolStr::new_static(prop));
        let forward_expr = if let Type::Callback(cb) = &ty {
            Expression::FunctionCall {
                function: Expression::CallbackReference(nr, None).into(),
                arguments: cb
                    .args
                    .iter()
                    .enumerate()
                    .map(|(index, ty)| Expression::FunctionParameterReference {
                        index,
                        ty: ty.clone(),
                    })
                    .collect(),
                source_location: source_location.clone(),
            }
        } else {
            Expression::PropertyReference(nr)
        };
        menubar_impl.borrow_mut().bindings.insert(prop.into(), RefCell::new(forward_expr.into()));
        let old = menu_bar
            .borrow_mut()
            .property_declarations
            .insert(prop.into(), PropertyDeclaration { property_type: ty, ..Default::default() });
        assert!(old.is_none(), "{prop} already exists");
    }

    // Transform the MenuBar in a layout
    menu_bar.borrow_mut().base_type = components.vertical_layout.clone();
    menu_bar.borrow_mut().children = vec![menubar_impl, child];

    let setup_menubar = Expression::FunctionCall {
        function: Expression::BuiltinFunctionReference(
            BuiltinFunction::SetupNativeMenuBar,
            source_location.clone(),
        )
        .into(),
        arguments: vec![
            Expression::PropertyReference(NamedReference::new(
                &menu_bar,
                SmolStr::new_static(ENTRIES),
            )),
            Expression::CallbackReference(
                NamedReference::new(&menu_bar, SmolStr::new_static(SUB_MENU)),
                None,
            ),
            Expression::CallbackReference(
                NamedReference::new(&menu_bar, SmolStr::new_static(ACTIVATE)),
                None,
            ),
        ],
        source_location: source_location.clone(),
    };

    window.children.push(menu_bar);
    let component = window.enclosing_component.upgrade().unwrap();
    drop(window);

    // Rename every access to `root.height` into `child.height`
    let win_height = NamedReference::new(win, SmolStr::new_static(HEIGHT));
    crate::object_tree::visit_all_named_references(&component, &mut |nr| {
        if nr == &win_height {
            *nr = child_height.clone()
        }
    });
    // except for the actual geometry
    win.borrow_mut().geometry_props.as_mut().unwrap().height = win_height;

    component.init_code.borrow_mut().constructor_code.push(setup_menubar.into());

    true
}
