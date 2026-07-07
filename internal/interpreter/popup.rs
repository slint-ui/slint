// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Runtime side of the `ShowPopupWindow` / `ClosePopupWindow` /
//! `SetupMenuBar` / `ShowPopupMenu` builtins: build the popup `Instance`,
//! register it with the window, and wire the menu callbacks.

use crate::Value;
use crate::eval::{
    EvalContext, eval_expression, find_window_adapter, resolve_item_rc_from_ref, store_property,
    walk_to,
};
use crate::instance::SubComponentInstance;
use i_slint_compiler::llr::{Expression, LocalMemberIndex, MemberReference};
use i_slint_core::model::Model;
use std::pin::Pin;
use std::rc::Rc;

pub(crate) fn show_popup_window(ctx: &mut EvalContext, arguments: &[Expression]) -> Value {
    // Expected args (from `BuiltinFunction::ShowPopupWindow`):
    //   0: NumberLiteral(popup_index) into the declaring sub-component's popup_windows
    //   1: close_policy expression
    //   2: PropertyReference to the declaring component's root (`owner_ref`): its
    //      `popup_id` and the popup's scope live here. Resolved through the full
    //      reference (parent level *and* sub-component path) so a popup shown from
    //      a nested sub-component or an inlined function resolves correctly.
    //   3: PropertyReference to the parent item used for positioning (`anchor_ref`),
    //      which may be a different (nested) item than the declaring component.
    //   4 (optional): PropertyReference to the synthesized `is-open` property,
    //      resolving in this call's own frame (see lower_show_popup_window)
    let [
        Expression::NumberLiteral(popup_index),
        close_policy_expr,
        Expression::PropertyReference(owner_ref),
        Expression::PropertyReference(anchor_ref),
        is_open_args @ ..,
    ] = arguments
    else {
        return Value::Void;
    };
    let MemberReference::Relative { parent_level, local_reference } = owner_ref else {
        return Value::Void;
    };

    let current = match ctx.current.as_ref() {
        Some(c) => c.clone(),
        None => return Value::Void,
    };
    let owner = walk_to(ctx, *parent_level, &local_reference.sub_component_path);
    let cu = owner.compilation_unit.clone();
    let sc = &cu.sub_components[owner.sub_component_idx];
    let popup = match sc.popup_windows.get(*popup_index as usize) {
        Some(p) => p,
        None => return Value::Void,
    };

    let popup_vrc = new_popup_for(&owner, &popup.item_tree);

    let close_policy: i_slint_core::items::PopupClosePolicy =
        eval_expression(ctx, close_policy_expr).try_into().unwrap_or_default();

    let Some((parent_instance, parent_flat)) = resolve_item_rc_from_ref(ctx, anchor_ref) else {
        return Value::Void;
    };
    let parent_item_rc = i_slint_core::items::ItemRc::new(
        vtable::VRc::into_dyn(parent_instance.clone()),
        parent_flat as u32,
    );
    // The owner may itself be a popup without its own window adapter; walk
    // the parent chain so a popup-in-popup registers with the root window's
    // adapter instead of creating a fresh, headless one.
    let Some(adapter) = find_window_adapter(ctx) else {
        return Value::Void;
    };

    let window_kind = || {
        if popup.is_tooltip {
            i_slint_core::window::WindowKind::ToolTip
        } else {
            i_slint_core::window::WindowKind::Popup
        }
    };
    // Give the popup its own window adapter when the backend can create
    // one — `WindowInner::show_popup` then shows it as a native top-level
    // popup instead of a child window rendered into the parent surface.
    if let Some(child_adapter) = i_slint_core::window::WindowInner::from_pub(adapter.window())
        .create_child_window_adapter(window_kind())
    {
        let _ = popup_vrc.window_adapter.set(child_adapter);
    }

    // Install bindings now but defer `init_code` until after `show_popup`,
    // so `forward-focus` calls reach a popup the window adapter already
    // considers active. The rust codegen splits these the same way.
    crate::instance::install_bindings_for_repeated_row(&popup_vrc);

    // The position expression is evaluated lazily so the window can
    // re-query it after measuring the popup.
    let access_position: Box<dyn Fn() -> i_slint_core::api::LogicalPosition> = {
        let pos_expr = popup.position.borrow().clone();
        let popup_root = popup_vrc.root_sub_component.clone();
        Box::new(move || {
            let mut popup_ctx = EvalContext::new(popup_root.clone());
            eval_expression(&mut popup_ctx, &pos_expr).try_into().unwrap_or_default()
        })
    };
    // Keeps the caller's synthesized `is-open` property in sync:
    // `show_popup` invokes this setter with `true` on show and `false` from
    // every close path. The reference resolves in the show call's own frame.
    let is_open_setter: Box<dyn Fn(bool)> = match is_open_args.first() {
        Some(Expression::PropertyReference(is_open_ref)) => {
            let is_open_ref = is_open_ref.clone();
            let current_weak = std::rc::Rc::downgrade(&Pin::into_inner(current.clone()));
            Box::new(move |value: bool| {
                if let Some(current) = current_weak.upgrade() {
                    let ctx = EvalContext::new(Pin::new(current));
                    store_property(&ctx, &is_open_ref, Value::Bool(value));
                }
            })
        }
        _ => Box::new(|_| {}),
    };
    let popup_dyn = vtable::VRc::into_dyn(popup_vrc.clone());
    let popup_id = i_slint_core::window::WindowInner::from_pub(adapter.window()).show_popup(
        &popup_dyn,
        access_position,
        close_policy,
        &parent_item_rc,
        window_kind(),
        is_open_setter,
    );
    // Remember the id so `close_popup_window` can find it by popup index.
    {
        let mut ids = owner.popup_ids.borrow_mut();
        if (*popup_index as usize) < ids.len() {
            ids[*popup_index as usize] = Some(popup_id);
        }
    }
    // `init_code` runs now that the popup is registered, so `forward-focus`
    // calls can target items in the popup.
    crate::instance::finalize_instance(&popup_vrc);
    Value::Void
}

pub(crate) fn close_popup_window(ctx: &mut EvalContext, arguments: &[Expression]) -> Value {
    // Expected args (from `BuiltinFunction::ClosePopupWindow`):
    //   0: NumberLiteral(popup_index) into the declaring sub-component's popup_windows
    //   1: PropertyReference to the declaring component's root, where `popup_id`
    //      lives — resolved through the full reference like `show_popup_window`.
    let [Expression::NumberLiteral(popup_index), Expression::PropertyReference(parent_ref)] =
        arguments
    else {
        return Value::Void;
    };
    let MemberReference::Relative { parent_level, local_reference } = parent_ref else {
        return Value::Void;
    };
    let Some(_current) = ctx.current.as_ref() else { return Value::Void };
    // Resolve the declaring component through the full parent item reference,
    // matching `show_popup_window` so the popup id stored there is found again.
    let owner = walk_to(ctx, *parent_level, &local_reference.sub_component_path);
    let id = {
        let mut ids = owner.popup_ids.borrow_mut();
        ids.get_mut(*popup_index as usize).and_then(|slot| slot.take())
    };
    if let Some(id) = id
        && let Some(root_inst) = owner.root.get().and_then(|w| w.upgrade())
        && let Some(adapter) = root_inst.window_adapter_or_default()
    {
        i_slint_core::window::WindowInner::from_pub(adapter.window()).close_popup(id);
    }
    Value::Void
}

pub(crate) fn setup_menubar(ctx: &mut EvalContext, arguments: &[Expression]) -> Value {
    let [
        Expression::PropertyReference(entries_ref),
        Expression::PropertyReference(sub_menu_ref),
        Expression::PropertyReference(activated_ref),
        Expression::NumberLiteral(tree_index),
        Expression::BoolLiteral(no_native),
        condition,
        visible,
        ..,
    ] = arguments
    else {
        return Value::Void;
    };
    let (tree_index, no_native) = (*tree_index as usize, *no_native);

    let current = match ctx.current.as_ref() {
        Some(c) => c.clone(),
        None => return Value::Void,
    };
    let cu = current.compilation_unit.clone();
    let sc = &cu.sub_components[current.sub_component_idx];
    let Some(menu_tree) = sc.menu_item_trees.get(tree_index) else {
        return Value::Void;
    };

    let menu_vrc = new_popup_for(&current, menu_tree);
    crate::instance::finalize_instance(&menu_vrc);

    let menu_dyn = vtable::VRc::into_dyn(menu_vrc);
    let menu_item_tree =
        vtable::VRc::new(i_slint_core::menus::MenuFromItemTree::new_with_condition_and_visible(
            menu_dyn,
            bool_binding(&current, condition),
            bool_binding(&current, visible),
        ));

    let Some(adapter) = find_window_adapter(ctx) else { return Value::Void };
    let window_inner = i_slint_core::window::WindowInner::from_pub(adapter.window());
    let menubar = vtable::VRc::into_dyn(vtable::VRc::clone(&menu_item_tree));
    window_inner.setup_menubar_shortcuts(vtable::VRc::clone(&menubar));

    if !no_native && window_inner.supports_native_menu_bar() {
        window_inner.setup_menubar(menubar);
        return Value::Void;
    }

    // Keep the menubar alive on the owning sub-component.
    *current.menubar.borrow_mut() = Some(menubar);

    // Wire up entries/sub_menu/activated for the fallback menu bar widget.
    wire_menu_from_item_tree(ctx, entries_ref, sub_menu_ref, activated_ref, menu_item_tree);

    Value::Void
}

pub(crate) fn setup_system_tray_icon(ctx: &mut EvalContext, arguments: &[Expression]) -> Value {
    // Expected args (from `lower_menus`):
    //   0: PropertyReference to the native `SystemTrayIcon` item
    //   1: NumberLiteral(tree_index) into the owning sub-component's menu_item_trees
    //   2 (optional): condition expression from `if cond : Menu { ... }`
    let [
        Expression::PropertyReference(system_tray_ref),
        Expression::NumberLiteral(tree_index),
        rest @ ..,
    ] = arguments
    else {
        return Value::Void;
    };

    let current = match ctx.current.as_ref() {
        Some(c) => c.clone(),
        None => return Value::Void,
    };
    let cu = current.compilation_unit.clone();
    let sc = &cu.sub_components[current.sub_component_idx];
    let Some(menu_tree) = sc.menu_item_trees.get(*tree_index as usize) else {
        return Value::Void;
    };

    let menu_vrc = new_popup_for(&current, menu_tree);
    crate::instance::finalize_instance(&menu_vrc);

    // `if cond : Menu { ... }` lowers the condition into a closure that
    // gates the menu's shadow tree.
    let condition: Box<dyn Fn() -> bool> = match rest.first() {
        Some(expr) => Box::new(bool_binding(&current, expr)),
        None => Box::new(|| true),
    };
    let menu_item_tree =
        vtable::VRc::new(i_slint_core::menus::MenuFromItemTree::new_with_condition_and_visible(
            vtable::VRc::into_dyn(menu_vrc),
            condition,
            || true,
        ));

    let Some((parent_inst, flat_idx)) = resolve_item_rc_from_ref(ctx, system_tray_ref) else {
        return Value::Void;
    };
    let item_rc =
        i_slint_core::items::ItemRc::new(vtable::VRc::into_dyn(parent_inst), flat_idx as u32);
    if let Some(tray) = item_rc.downcast::<i_slint_core::items::SystemTrayIcon>() {
        tray.as_pin_ref().set_menu(&item_rc, vtable::VRc::into_dyn(menu_item_tree));
    }
    Value::Void
}

pub(crate) fn show_popup_menu(ctx: &mut EvalContext, arguments: &[Expression]) -> Value {
    let [Expression::PropertyReference(context_menu_ref), entries_expr, position_expr] = arguments
    else {
        return Value::Void;
    };

    let position: i_slint_core::api::LogicalPosition =
        eval_expression(ctx, position_expr).try_into().unwrap_or_default();

    let Some((parent_inst, context_flat_idx)) = resolve_item_rc_from_ref(ctx, context_menu_ref)
    else {
        return Value::Void;
    };
    let context_item_rc = i_slint_core::items::ItemRc::new(
        vtable::VRc::into_dyn(parent_inst.clone()),
        context_flat_idx as u32,
    );
    let context_menu_item_weak = context_item_rc.downgrade();
    let Some(adapter) = find_window_adapter(ctx) else {
        return Value::Void;
    };

    let cu = ctx.current.as_ref().map(|c| c.compilation_unit.clone()).unwrap();
    let Some(popup_menu) = cu.popup_menu.as_ref() else {
        return Value::Void;
    };

    let current = ctx.current.as_ref().unwrap();
    let popup_vrc = new_popup_for(current, &popup_menu.item_tree);
    // Install bindings now but defer `init_code` until after `show_popup`,
    // so `forward-focus` calls reach a popup the window adapter already
    // considers active. The rust codegen splits these the same way.
    crate::instance::install_bindings_for_repeated_row(&popup_vrc);

    // Wire entries/sub_menu/activated on the popup. Two flavors:
    //
    //   - `ShowPopupMenu` (regular `ContextMenuArea`): the entries come
    //     from a `MenuItem` tree we walk via `MenuFromItemTree`.
    //   - `ShowPopupMenuInternal` (`ContextMenuInternal`): the entries
    //     come from an array property on the user's `ContextMenu` item,
    //     and `sub_menu` / `activated` forward back to the user's
    //     callbacks rather than a shadow tree.
    let popup_ctx = crate::eval::EvalContext::new(popup_vrc.root_sub_component.clone());

    if let Expression::NumberLiteral(tree_index) = entries_expr {
        let sc = &cu.sub_components[current.sub_component_idx];
        let Some(menu_tree) = sc.menu_item_trees.get(*tree_index as usize) else {
            return Value::Void;
        };
        let menu_vrc = crate::instance::Instance::new_popup(
            cu.clone(),
            menu_tree,
            std::rc::Rc::downgrade(&Pin::into_inner(current.clone())),
            popup_vrc.globals.clone(),
        );
        crate::instance::finalize_instance(&menu_vrc);
        let menu_item_tree = vtable::VRc::new(i_slint_core::menus::MenuFromItemTree::new(
            vtable::VRc::into_dyn(menu_vrc),
        ));

        // Prefer the platform's native context menu; fall back to the
        // Slint-rendered popup below when the backend doesn't provide one.
        if i_slint_core::window::WindowInner::from_pub(adapter.window()).show_native_popup_menu(
            vtable::VRc::into_dyn(vtable::VRc::clone(&menu_item_tree)),
            position,
            &context_item_rc,
        ) {
            return Value::Void;
        }

        wire_menu_from_item_tree(
            &popup_ctx,
            &popup_menu.entries,
            &popup_menu.sub_menu,
            &popup_menu.activated,
            menu_item_tree,
        );
    } else {
        // ShowPopupMenuInternal: entries are an array property of the
        // `ContextMenuInternal` item; sub_menu/activated forward to the
        // user-defined callbacks on the same item.
        let entries_value = eval_expression(ctx, entries_expr);
        wire_popup_menu_prop(&popup_ctx, &popup_menu.entries, move || entries_value.clone());

        if let MemberReference::Relative { parent_level, local_reference } = context_menu_ref {
            let sub_menu_owner = walk_to(ctx, *parent_level, &local_reference.sub_component_path);
            let LocalMemberIndex::Native { item_index, .. } = &local_reference.reference else {
                return Value::Void;
            };
            let item_index_for_sub = *item_index;
            let sub_menu_owner_weak =
                std::rc::Rc::downgrade(&Pin::into_inner(sub_menu_owner.clone()));
            wire_popup_menu_cb(&popup_ctx, &popup_menu.sub_menu, move |args| {
                let Some(owner) = sub_menu_owner_weak.upgrade() else { return Value::Void };
                let item = Pin::as_ref(&owner.items[item_index_for_sub]);
                let entry: i_slint_core::items::MenuEntry =
                    args.first().cloned().unwrap_or_default().try_into().unwrap_or_default();
                let raw = item.as_item_ref();
                use i_slint_core::items::ContextMenu;
                let Some(cm) = vtable::VRef::downcast_pin::<ContextMenu>(raw) else {
                    return Value::Void;
                };
                let mut out = i_slint_core::SharedVector::default();
                cm.sub_menu.call(&(entry,)).iter().for_each(|e| out.push(e));
                Value::Model(i_slint_core::model::ModelRc::new(
                    i_slint_core::model::VecModel::from(
                        out.into_iter().map(Value::from).collect::<Vec<_>>(),
                    ),
                ))
            });

            let activated_owner_weak =
                std::rc::Rc::downgrade(&Pin::into_inner(sub_menu_owner.clone()));
            let item_index_for_activated = *item_index;
            wire_popup_menu_cb(&popup_ctx, &popup_menu.activated, move |args| {
                let Some(owner) = activated_owner_weak.upgrade() else { return Value::Void };
                let item = Pin::as_ref(&owner.items[item_index_for_activated]);
                let entry: i_slint_core::items::MenuEntry =
                    args.first().cloned().unwrap_or_default().try_into().unwrap_or_default();
                let raw = item.as_item_ref();
                use i_slint_core::items::ContextMenu;
                let Some(cm) = vtable::VRef::downcast_pin::<ContextMenu>(raw) else {
                    return Value::Void;
                };
                cm.activated.call(&(entry,));
                Value::Void
            });
        }
    }

    // Wire the popup menu's `close` callback so a menu item that calls
    // `root.close()` dismisses the popup. The shared `Cell` bridges the
    // popup id (only known after `show_popup`) into the close handler
    // installed before the show.
    let popup_id_cell: std::rc::Rc<std::cell::Cell<Option<core::num::NonZeroU32>>> =
        std::rc::Rc::new(std::cell::Cell::new(None));
    let id_cell_for_close = popup_id_cell.clone();
    let adapter_for_close = adapter.clone();
    wire_popup_menu_cb(&popup_ctx, &popup_menu.close, move |_args| {
        if let Some(id) = id_cell_for_close.take() {
            i_slint_core::window::WindowInner::from_pub(adapter_for_close.window()).close_popup(id);
        }
        Value::Void
    });

    let popup_dyn = vtable::VRc::into_dyn(popup_vrc.clone());
    let window_inner = i_slint_core::window::WindowInner::from_pub(adapter.window());
    let popup_id = window_inner.show_popup(
        &popup_dyn,
        Box::new(move || position),
        i_slint_core::items::PopupClosePolicy::CloseOnClickOutside,
        &context_item_rc,
        i_slint_core::window::WindowKind::Menu,
        Box::new(|_| {}),
    );
    popup_id_cell.set(Some(popup_id));
    // Store the popup id on the native `ContextMenu` item, mirroring the
    // rust codegen, so the generated `is-open()` and `close()` member
    // functions see this popup as the active one.
    if let Some(item_rc) = context_menu_item_weak.upgrade()
        && let Some(cm) = item_rc.downcast::<i_slint_core::items::ContextMenu>()
    {
        cm.as_pin_ref().popup_id.set(Some(popup_id));
    }

    // `init_code` runs now that the popup is registered, so `forward-focus`
    // calls can target items in the popup.
    crate::instance::finalize_instance(&popup_vrc);

    Value::Void
}

/// Build a popup / menu `Instance` owned by `owner`, sharing the root
/// instance's global storage (or fresh storage when the owner is already
/// detached from a root).
fn new_popup_for(
    owner: &Pin<Rc<SubComponentInstance>>,
    item_tree: &i_slint_compiler::llr::ItemTree,
) -> vtable::VRc<i_slint_core::item_tree::ItemTreeVTable, crate::instance::Instance> {
    let cu = owner.compilation_unit.clone();
    let parent_weak = std::rc::Rc::downgrade(&Pin::into_inner(owner.clone()));
    let globals = owner
        .root
        .get()
        .and_then(|w| w.upgrade())
        .map(|inst| inst.globals.clone())
        .unwrap_or_else(|| std::rc::Rc::new(crate::globals::GlobalStorage::new(&cu)));
    crate::instance::Instance::new_popup(cu, item_tree, parent_weak, globals)
}

/// A closure that evaluates `expr` in `owner`'s scope on each call;
/// `false` once the owner is gone.
fn bool_binding(
    owner: &Pin<Rc<SubComponentInstance>>,
    expr: &Expression,
) -> impl Fn() -> bool + 'static {
    let expr = expr.clone();
    let weak = std::rc::Rc::downgrade(&Pin::into_inner(owner.clone()));
    move || {
        let Some(owner) = weak.upgrade() else { return false };
        let mut ctx = EvalContext::new(Pin::new(owner));
        matches!(eval_expression(&mut ctx, &expr), Value::Bool(true))
    }
}

/// Wire the `entries` property and the `sub-menu` / `activated` callbacks
/// to a `MenuFromItemTree`. Shared by the menu bar fallback widget and the
/// Slint-rendered context menu popup.
fn wire_menu_from_item_tree(
    ctx: &EvalContext,
    entries: &MemberReference,
    sub_menu: &MemberReference,
    activated: &MemberReference,
    menu_item_tree: vtable::VRc<
        i_slint_core::menus::MenuVTable,
        i_slint_core::menus::MenuFromItemTree,
    >,
) {
    fn entries_model(
        mt: &i_slint_core::menus::MenuFromItemTree,
        parent: Option<&i_slint_core::items::MenuEntry>,
    ) -> Value {
        let mut entries = i_slint_core::SharedVector::default();
        i_slint_core::menus::Menu::sub_menu(mt, parent, &mut entries);
        Value::Model(i_slint_core::model::ModelRc::new(i_slint_core::model::VecModel::from(
            entries.into_iter().map(Value::from).collect::<Vec<_>>(),
        )))
    }
    let mt = vtable::VRc::clone(&menu_item_tree);
    wire_popup_menu_prop(ctx, entries, move || entries_model(&mt, None));
    let mt = vtable::VRc::clone(&menu_item_tree);
    wire_popup_menu_cb(ctx, sub_menu, move |args| {
        let entry = args.first().cloned().unwrap_or_default().try_into().unwrap_or_default();
        entries_model(&mt, Some(&entry))
    });
    wire_popup_menu_cb(ctx, activated, move |args| {
        let entry = args.first().cloned().unwrap_or_default().try_into().unwrap_or_default();
        i_slint_core::menus::Menu::activate(&*menu_item_tree, &entry);
        Value::Void
    });
}

fn wire_popup_menu_prop(
    ctx: &EvalContext,
    mr: &MemberReference,
    binding: impl Fn() -> Value + 'static,
) {
    if let MemberReference::Relative { parent_level, local_reference } = mr {
        let owner = walk_to(ctx, *parent_level, &local_reference.sub_component_path);
        if let LocalMemberIndex::Property(idx) = &local_reference.reference {
            Pin::as_ref(&owner.properties[*idx]).set_binding(binding);
        }
    }
}

fn wire_popup_menu_cb(
    ctx: &EvalContext,
    mr: &MemberReference,
    handler: impl Fn(&[Value]) -> Value + 'static,
) {
    if let MemberReference::Relative { parent_level, local_reference } = mr {
        let owner = walk_to(ctx, *parent_level, &local_reference.sub_component_path);
        if let LocalMemberIndex::Callback(idx) = &local_reference.reference {
            Pin::as_ref(&owner.callbacks[*idx]).set_handler(move |args: &[Value]| handler(args));
        }
    }
}
