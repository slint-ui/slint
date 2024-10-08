// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Passe lower the `MenuBar` and `ContextMenu` as well as all their contents
//!
//! Must be done before inlining and many other passes because the lowered code must
//! be further inlined as it may expends to native widget that needs inlining

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::NamedReference;
use crate::langtype::ElementType;
use crate::object_tree::*;
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
    menu_bar.borrow_mut().base_type = components.menubar_impl.clone();

    // Create a child that contains all the child but the menubar
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

    // Create a layout
    let layout = Element {
        id: format_smolstr!("{}-menulayout", window.id),
        base_type: components.vertical_layout.clone(),
        enclosing_component: window.enclosing_component.clone(),
        children: vec![menu_bar, child],
        ..Default::default()
    }
    .make_rc();

    window.children.push(layout);
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
    true
}
