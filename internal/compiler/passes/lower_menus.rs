// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Passe lower the `MenuBar` and `ContextMenuArea` as well as all their contents
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
//!        Menu {
//!           title: "File";
//!           MenuItem {
//!             title: "A";
//!             activated => { ... }
//!           }
//!           Menu {
//!               title: "B";
//!               MenuItem { title: "C"; }
//!           }
//!        }
//!      }
//!      content := ...
//! }
//! ```
//! Is transformed to
//! ```slint
//! Window {
//!     menu-bar := VerticalLayout {
//!        property <[MenuEntry]> entries : [ { id: "0", title: "File", has-sub-menu: true } ];
//!        callback sub-menu(entry: MenuEntry) => {
//!            if(entry.id == "0") { return [ { id: "1", title: "A" }, { id: "2", title: "B", has-sub-menu: true } ]; }
//!            else if(entry.id == "2") { return [ { id: "3", title: "C" } ]; } else { return []; }
//!        }
//!        callback activated() => { if (entry.id == "2") { ... } }
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
//! ## ContextMenuInternal
//!
//! ```slint
//! menu := ContextMenuInternal {
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
//! ## ContextMenuArea
//!
//! This is the same as ContextMenuInternal, but entries, sub-menu, and activated are generated
//! from the MenuItem similar to MenuBar

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::expression_tree::{BuiltinFunction, Callable, Expression, NamedReference};
use crate::langtype::{ElementType, Type};
use crate::object_tree::*;
use core::cell::RefCell;
use i_slint_common::MENU_SEPARATOR_PLACEHOLDER_TITLE;
use smol_str::{format_smolstr, SmolStr};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

const HEIGHT: &str = "height";
const ENTRIES: &str = "entries";
const SUB_MENU: &str = "sub-menu";
const ACTIVATED: &str = "activated";
const SHOW: &str = "show";

struct UsefulMenuComponents {
    menubar_impl: ElementType,
    vertical_layout: ElementType,
    context_menu_internal: ElementType,
    empty: ElementType,
    menu_entry: Type,
    menu_item_element: ElementType,
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

    let menu_item_element = type_loader
        .global_type_registry
        .borrow()
        .lookup_builtin_element("ContextMenuArea")
        .unwrap()
        .as_builtin()
        .additional_accepted_child_types
        .get("Menu")
        .expect("ContextMenuArea should accept Menu")
        .additional_accepted_child_types
        .get("MenuItem")
        .expect("Menu should accept MenuItem")
        .clone()
        .into();

    let useful_menu_component = UsefulMenuComponents {
        menubar_impl: menubar_impl.clone().into(),
        context_menu_internal: type_loader
            .global_type_registry
            .borrow()
            .lookup_builtin_element("ContextMenuInternal")
            .expect("ContextMenuInternal is a builtin type"),
        vertical_layout: type_loader
            .global_type_registry
            .borrow()
            .lookup_builtin_element("VerticalLayout")
            .expect("VerticalLayout is a builtin type"),
        empty: type_loader.global_type_registry.borrow().empty_type(),
        menu_entry: type_loader.global_type_registry.borrow().lookup("MenuEntry"),
        menu_item_element,
    };
    assert!(matches!(&useful_menu_component.menu_entry, Type::Struct(..)));

    let mut has_menu = false;
    let mut has_menubar = false;

    doc.visit_all_used_components(|component| {
        recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
            if matches!(&elem.borrow().builtin_type(), Some(b) if b.name == "Window") {
                has_menubar |= process_window(elem, &useful_menu_component, type_loader.compiler_config.no_native_menu, diag);
            }
            if matches!(&elem.borrow().builtin_type(), Some(b) if matches!(b.name.as_str(), "ContextMenuArea" | "ContextMenuInternal")) {
                has_menu |= process_context_menu(elem, &useful_menu_component, diag);
            }
        })
    });

    if has_menubar {
        recurse_elem_including_sub_components_no_borrow(&menubar_impl, &(), &mut |elem, _| {
            if matches!(&elem.borrow().builtin_type(), Some(b) if matches!(b.name.as_str(), "ContextMenuArea" | "ContextMenuInternal"))
            {
                has_menu |= process_context_menu(elem, &useful_menu_component, diag);
            }
        });
    }
    if has_menu {
        let popup_menu_impl = type_loader
            .import_component("std-widgets.slint", "PopupMenuImpl", &mut build_diags_to_ignore)
            .await
            .expect("PopupMenuImpl should be in std-widgets.slint");
        {
            let mut root = popup_menu_impl.root_element.borrow_mut();

            for prop in [ENTRIES, SUB_MENU, ACTIVATED] {
                match root.property_declarations.get_mut(prop) {
                    Some(d) => d.expose_in_public_api = true,
                    None => diag.push_error(format!("PopupMenuImpl doesn't have {prop}"), &*root),
                }
            }
            root.property_analysis
                .borrow_mut()
                .entry(SmolStr::new_static(ENTRIES))
                .or_default()
                .is_set = true;
        }

        recurse_elem_including_sub_components_no_borrow(&popup_menu_impl, &(), &mut |elem, _| {
            if matches!(&elem.borrow().builtin_type(), Some(b) if matches!(b.name.as_str(), "ContextMenuArea" | "ContextMenuInternal"))
            {
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
    let is_internal = matches!(&context_menu_elem.borrow().base_type, ElementType::Builtin(b) if b.name == "ContextMenuInternal");

    if is_internal && context_menu_elem.borrow().property_declarations.contains_key(ENTRIES) {
        // Already processed;
        return false;
    }

    let item_tree_root = if !is_internal {
        // Lower Menu into entries
        let menu_element_type = context_menu_elem
            .borrow()
            .base_type
            .as_builtin()
            .additional_accepted_child_types
            .get("Menu")
            .expect("ContextMenu should accept Menu")
            .clone()
            .into();

        context_menu_elem.borrow_mut().base_type = components.context_menu_internal.clone();

        let mut menu_elem = None;
        context_menu_elem.borrow_mut().children.retain(|x| {
            if x.borrow().base_type == menu_element_type {
                if menu_elem.is_some() {
                    diag.push_error(
                        "Only one Menu is allowed in a ContextMenu".into(),
                        &*x.borrow(),
                    );
                } else {
                    menu_elem = Some(x.clone());
                }
                false
            } else {
                true
            }
        });

        let item_tree_root = if let Some(menu_elem) = menu_elem {
            if menu_elem.borrow().repeated.is_some() {
                diag.push_error(
                    "ContextMenuArea's root Menu cannot be in a conditional or repeated element"
                        .into(),
                    &*menu_elem.borrow(),
                );
            }

            let children = std::mem::take(&mut menu_elem.borrow_mut().children);
            lower_menu_items(context_menu_elem, children, components)
                .map(|c| Expression::ElementReference(Rc::downgrade(&c.root_element)))
        } else {
            diag.push_error(
                "ContextMenuArea should have a Menu".into(),
                &*context_menu_elem.borrow(),
            );
            None
        };

        for (name, _) in &components.context_menu_internal.property_list() {
            if let Some(decl) = context_menu_elem.borrow().property_declarations.get(name) {
                diag.push_error(format!("Cannot re-define internal property '{name}'"), &decl.node);
            }
        }

        item_tree_root
    } else {
        None
    };

    let entries = if let Some(item_tree_root) = item_tree_root {
        item_tree_root
    } else {
        // Materialize the entries property
        context_menu_elem.borrow_mut().property_declarations.insert(
            SmolStr::new_static(ENTRIES),
            Type::Array(components.menu_entry.clone().into()).into(),
        );
        Expression::PropertyReference(NamedReference::new(
            context_menu_elem,
            SmolStr::new_static(ENTRIES),
        ))
    };

    // generate the show callback
    let source_location = Some(context_menu_elem.borrow().to_source_location());
    let expr = Expression::FunctionCall {
        function: BuiltinFunction::ShowPopupMenu.into(),
        arguments: vec![
            Expression::ElementReference(Rc::downgrade(context_menu_elem)),
            entries,
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
        diag.push_error("'show' is not a callback in ContextMenuArea".into(), &old.borrow().span);
    }

    true
}

fn process_window(
    win: &ElementRc,
    components: &UsefulMenuComponents,
    no_native_menu: bool,
    diag: &mut BuildDiagnostics,
) -> bool {
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

    // Lower MenuItem's into entries
    let children = std::mem::take(&mut menu_bar.borrow_mut().children);
    let item_tree_root = if !children.is_empty() {
        lower_menu_items(&menu_bar, children, components)
            .map(|c| Expression::ElementReference(Rc::downgrade(&c.root_element)))
    } else {
        None
    };

    let menubar_impl = Element {
        id: format_smolstr!("{}-menulayout", window.id),
        base_type: components.menubar_impl.clone(),
        enclosing_component: window.enclosing_component.clone(),
        repeated: (!no_native_menu).then(|| crate::object_tree::RepeatedElementInfo {
            model: Expression::UnaryOp {
                op: '!',
                sub: Expression::FunctionCall {
                    function: BuiltinFunction::SupportsNativeMenuBar.into(),
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

    for prop in [ENTRIES, SUB_MENU, ACTIVATED] {
        // materialize the properties and callbacks
        let ty = components.menubar_impl.lookup_property(prop).property_type;
        assert_ne!(ty, Type::Invalid, "Can't lookup type for {prop}");
        let nr = NamedReference::new(&menu_bar, SmolStr::new_static(prop));
        let forward_expr = if let Type::Callback(cb) = &ty {
            Expression::FunctionCall {
                function: Callable::Callback(nr),
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
        if let Some(old) = old {
            diag.push_error(format!("Cannot re-define internal property '{prop}'"), &old.node);
        }
    }

    // Transform the MenuBar in a layout
    menu_bar.borrow_mut().base_type = components.vertical_layout.clone();
    menu_bar.borrow_mut().children = vec![menubar_impl, child];

    for prop in [ENTRIES, SUB_MENU, ACTIVATED] {
        menu_bar
            .borrow()
            .property_analysis
            .borrow_mut()
            .entry(SmolStr::new_static(prop))
            .or_default()
            .is_set = true;
    }

    window.children.push(menu_bar.clone());
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

    if !no_native_menu || item_tree_root.is_some() {
        let mut arguments = vec![
            Expression::PropertyReference(NamedReference::new(
                &menu_bar,
                SmolStr::new_static(ENTRIES),
            )),
            Expression::PropertyReference(NamedReference::new(
                &menu_bar,
                SmolStr::new_static(SUB_MENU),
            )),
            Expression::PropertyReference(NamedReference::new(
                &menu_bar,
                SmolStr::new_static(ACTIVATED),
            )),
        ];

        if let Some(item_tree_root) = item_tree_root {
            arguments.push(item_tree_root.into());
            arguments.push(Expression::BoolLiteral(no_native_menu));
        }
        let setup_menubar = Expression::FunctionCall {
            function: BuiltinFunction::SetupNativeMenuBar.into(),
            arguments,
            source_location,
        };
        component.init_code.borrow_mut().constructor_code.push(setup_menubar.into());
    }
    true
}

/// Lower the MenuItem's and Menu's to either
///  - `entries` and `activated` and `sub-menu` properties/callback, in which cases it returns None
///  - or a Component which is a tree of MenuItem, in which case returns the component that is within the enclosing component's menu_item_trees
fn lower_menu_items(
    parent: &ElementRc,
    children: Vec<ElementRc>,
    components: &UsefulMenuComponents,
) -> Option<Rc<Component>> {
    let mut has_repeated = false;
    for i in &children {
        recurse_elem(i, &(), &mut |e, _| {
            if e.borrow().repeated.is_some() {
                has_repeated = true;
            }
        });
        if has_repeated {
            break;
        }
    }
    if !has_repeated {
        let menu_entry = &components.menu_entry;
        let mut state = GenMenuState {
            id: 0,
            menu_entry: menu_entry.clone(),
            activate: Vec::new(),
            sub_menu: Vec::new(),
        };
        let entries = generate_menu_entries(children.into_iter(), &mut state);
        parent.borrow_mut().bindings.insert(
            ENTRIES.into(),
            RefCell::new(
                Expression::Array { element_ty: menu_entry.clone(), values: entries }.into(),
            ),
        );
        let entry_id = Expression::StructFieldAccess {
            base: Expression::FunctionParameterReference { index: 0, ty: menu_entry.clone() }
                .into(),
            name: SmolStr::new_static("id"),
        };

        let sub_entries = build_cases_function(
            &entry_id,
            Expression::Array { element_ty: menu_entry.clone(), values: vec![] },
            state.sub_menu,
        );
        parent.borrow_mut().bindings.insert(SUB_MENU.into(), RefCell::new(sub_entries.into()));

        let activated =
            build_cases_function(&entry_id, Expression::CodeBlock(vec![]), state.activate);
        parent.borrow_mut().bindings.insert(ACTIVATED.into(), RefCell::new(activated.into()));
        None
    } else {
        let component = Rc::new_cyclic(|component_weak| {
            let root_element = Rc::new(RefCell::new(Element {
                base_type: components.empty.clone(),
                children,
                enclosing_component: component_weak.clone(),
                ..Default::default()
            }));
            recurse_elem(&root_element, &true, &mut |element: &ElementRc, is_root| {
                if !is_root {
                    debug_assert!(Weak::ptr_eq(
                        &element.borrow().enclosing_component,
                        &parent.borrow().enclosing_component
                    ));
                    element.borrow_mut().enclosing_component = component_weak.clone();
                    element.borrow_mut().geometry_props = None;
                    if element.borrow().base_type.type_name() == Some("MenuSeparator") {
                        element.borrow_mut().bindings.insert(
                            "title".into(),
                            RefCell::new(
                                Expression::StringLiteral(SmolStr::new_static(
                                    MENU_SEPARATOR_PLACEHOLDER_TITLE,
                                ))
                                .into(),
                            ),
                        );
                    }
                    // Menu/MenuSeparator -> MenuItem
                    element.borrow_mut().base_type = components.menu_item_element.clone();
                }
                false
            });
            Component {
                node: parent.borrow().debug.first().map(|n| n.node.clone().into()),
                id: SmolStr::default(),
                root_element,
                parent_element: Rc::downgrade(parent),
                ..Default::default()
            }
        });
        parent
            .borrow()
            .enclosing_component
            .upgrade()
            .unwrap()
            .menu_item_tree
            .borrow_mut()
            .push(component.clone());
        Some(component)
    }
}

fn build_cases_function(
    entry_id: &Expression,
    default_case: Expression,
    cases: Vec<(SmolStr, Expression)>,
) -> Expression {
    let mut result = default_case;
    for (id, expr) in cases.into_iter().rev() {
        result = Expression::Condition {
            condition: Expression::BinaryExpression {
                lhs: entry_id.clone().into(),
                rhs: Expression::StringLiteral(id).into(),
                op: '=',
            }
            .into(),
            true_expr: expr.into(),
            false_expr: result.into(),
        }
    }
    result
}

struct GenMenuState {
    id: usize,
    /// Maps `entry.id` to the callback
    activate: Vec<(SmolStr, Expression)>,
    /// Maps `entry.id` to the sub-menu entries
    sub_menu: Vec<(SmolStr, Expression)>,

    menu_entry: Type,
}

/// Recursively generate the menu entries for the given menu items
fn generate_menu_entries(
    menu_items: impl Iterator<Item = ElementRc>,
    state: &mut GenMenuState,
) -> Vec<Expression> {
    let mut entries = Vec::new();
    let mut last_is_separator = false;

    for item in menu_items {
        let mut borrow_mut = item.borrow_mut();
        let base_name = borrow_mut.base_type.type_name().unwrap();
        let is_sub_menu = base_name == "Menu";
        let is_separator = base_name == "MenuSeparator";
        if !is_sub_menu && !is_separator {
            assert_eq!(base_name, "MenuItem");
        }

        if is_separator && (last_is_separator || entries.is_empty()) {
            continue;
        }
        last_is_separator = is_separator;

        borrow_mut
            .enclosing_component
            .upgrade()
            .unwrap()
            .optimized_elements
            .borrow_mut()
            .push(item.clone());

        assert!(borrow_mut.repeated.is_none());

        let mut values = HashMap::<SmolStr, Expression>::new();
        state.id += 1;
        let id_str = format_smolstr!("{}", state.id);
        values.insert(SmolStr::new_static("id"), Expression::StringLiteral(id_str.clone()));

        if let Some(callback) = borrow_mut.bindings.remove(ACTIVATED) {
            state.activate.push((id_str.clone(), callback.into_inner().expression));
        }

        if is_sub_menu {
            let sub_entries =
                generate_menu_entries(std::mem::take(&mut borrow_mut.children).into_iter(), state);

            state.sub_menu.push((
                id_str,
                Expression::Array { element_ty: state.menu_entry.clone(), values: sub_entries },
            ));
            values
                .insert(SmolStr::new_static("has-sub-menu"), Expression::BoolLiteral(true).into());
        }

        drop(borrow_mut);
        if !is_separator {
            for prop in ["title", "enabled"] {
                if item.borrow().bindings.contains_key(prop) {
                    let n = SmolStr::new_static(prop);
                    values.insert(
                        n.clone(),
                        Expression::PropertyReference(NamedReference::new(&item, n)),
                    );
                }
            }
        } else {
            values.insert(SmolStr::new_static("is_separator"), Expression::BoolLiteral(true));
        }

        entries.push(mk_struct(state.menu_entry.clone(), values));
    }
    if last_is_separator {
        entries.pop();
    }

    entries
}

fn mk_struct(ty: Type, mut values: HashMap<SmolStr, Expression>) -> Expression {
    let Type::Struct(ty) = ty else { panic!("Not a struct") };
    for (k, v) in ty.fields.iter() {
        values.entry(k.clone()).or_insert_with(|| Expression::default_value_for_type(v));
    }
    Expression::Struct { ty, values }
}
