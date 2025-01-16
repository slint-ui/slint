// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Passe lower the `MenuBar` and `ContextMenu` as well as all their contents
//!
//! We can't have properties of type model because that is not binary compatible with C++,
//! so all the code that handle model of MenuEntry need to be handle by code in the generated code
//! and transformed into a `SharedVector<MenuEntry>` that is passed to Slint runtime.
//!
//! ## MenuBar
//!
//! ```slint
//! Window {
//!      menu-bar := MenuBar {
//!           entries: [...]
//!           sub-menu => ...
//!           activated => ...
//!      }
//!      content := ...
//! }
//! ```
//! Is transformed to
//! ```slint
//! Window {
//!     menu-bar := VerticalLayout {
//!        property <[MenuEntry]> entries : ...
//!        callback sub-menu => { ... }
//!        callback activated => { ... }
//!        if !Builtin.supports_native_menu_bar() : MenuBarImpl {
//!           entries: parent.entries
//!           sub-menu(..) => { parent.sub-menu(..) }
//!           activated(..) => { parent.activated(..) }
//!        }
//!        Empty {
//!           content := ...
//!        }
//!    }
//!    init => {
//!        // ... rest of init ...
//!        Builtin.setup_native_menu_bar(menu-bar.entries, menu-bar.sub-menu, menu-bar.activated)
//!    }
//! }
//! ```
//!
//! ## ContextMenu
//!
//! ```slint
//! menu := ContextMenu {
//!     entries: [...]
//!     sub-menu => ...
//!     activated => ...
//! }
//! Button { clicked => {menu.show({x: 0, y: 0;})} }
//! ```
//! Is transformed to
//!
//! ```slint
//! menu := ContextMenu {
//!    property <[MenuEntry]> entries : ...
//!    sub-menu => { ... }
//!    activated => { ... }
//!
//!    // show is actually a callback called by the native code when right clicking
//!    callback show(point) => { Builtin.show_context_menu(entries, sub-menu, activated, point) }
//! }
//!
//!
//!

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::expression_tree::{BuiltinFunction, Expression, NamedReference};
use crate::langtype::{ElementType, Type};
use crate::object_tree::*;
use core::cell::RefCell;
use smol_str::{format_smolstr, SmolStr};
use std::rc::Rc;

const HEIGHT: &str = "height";
const ENTRIES: &str = "entries";
const SUB_MENU: &str = "sub-menu";
const ACTIVATE: &str = "activated";
const SHOW: &str = "show";

struct UsefulMenuComponents {
    menubar_impl: ElementType,
    vertical_layout: ElementType,
    empty: ElementType,
    menu_entry: Type,
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
        menu_entry: type_loader.global_type_registry.borrow().lookup("MenuEntry"),
    };
    assert!(matches!(&useful_menu_component.menu_entry, Type::Struct(..)));

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
        doc.popup_menu_impl = popup_menu_impl.into();
    }
}

fn process_context_menu(
    context_menu_elem: &ElementRc,
    components: &UsefulMenuComponents,
    diag: &mut BuildDiagnostics,
) -> bool {
    // Materialize the entries property
    context_menu_elem.borrow_mut().property_declarations.insert(
        SmolStr::new_static(ENTRIES),
        Type::Array(components.menu_entry.clone().into()).into(),
    );

    // generate the show callback
    let source_location = Some(context_menu_elem.borrow().to_source_location());
    let expr = Expression::FunctionCall {
        function: Expression::BuiltinFunctionReference(
            BuiltinFunction::ShowPopupMenu,
            source_location.clone(),
        )
        .into(),
        arguments: vec![
            Expression::ElementReference(Rc::downgrade(context_menu_elem)),
            Expression::PropertyReference(NamedReference::new(
                &context_menu_elem,
                SmolStr::new_static(ENTRIES),
            )),
            Expression::FunctionParameterReference {
                index: 0,
                ty: crate::typeregister::logical_point_type(),
            },
        ],
        source_location,
    };
    let old = context_menu_elem
        .borrow_mut()
        .bindings
        .insert(SmolStr::new_static(SHOW), RefCell::new(expr.into()));
    if let Some(old) = old {
        diag.push_error("'show' is not a callback in ContextMenu".into(), &old.borrow().span);
    }

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

    let child_height = NamedReference::new(&child, SmolStr::new_static(HEIGHT));

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
