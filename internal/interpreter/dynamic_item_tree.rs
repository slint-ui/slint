// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::{CompilationResult, Value};
use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::{llr, CompilerConfiguration};
use i_slint_core::item_tree::ItemTreeWeak;
use i_slint_core::window::WindowAdapterRc;
use std::rc::Rc;
use i_slint_core::Property;
use core::pin::Pin;
use i_slint_core::items::ItemVTable;
use typed_index_collections::TiVec;

struct DynamicItemTree {
    llr: Rc<llr::CompilationUnit>,
    public_component_index: usize,
}

type DynamicItemTreeRc = vtable::VRc<i_slint_core::item_tree::ItemTreeVTable, DynamicItemTree>;

struct SubComponentInstance {
    properties: Pin<Box<[Property<Value>]>>,
    items: TiVec<llr::ItemInstanceIdx, Pin<Box<dyn ItemInstance>>>,
    sub_components: TiVec<llr::SubComponentInstanceIdx, SubComponentInstance>
}

#[derive(Default)]
pub enum WindowOptions {
    #[default]
    CreateNewWindow,
    UseExistingWindow(WindowAdapterRc),
    Embed {
        parent_item_tree: ItemTreeWeak,
        parent_item_tree_index: u32,
    },
}

/// Compile a file to LLR
pub async fn load(
    source: String,
    path: std::path::PathBuf,
    mut compiler_config: CompilerConfiguration,
) -> CompilationResult {
    // If the native style should be Qt, resolve it here as we know that we have it
    let is_native = match &compiler_config.style {
        Some(s) => s == "native",
        None => std::env::var("SLINT_STYLE").map_or(true, |s| s == "native"),
    };
    if is_native {
        // On wasm, look at the browser user agent
        #[cfg(target_arch = "wasm32")]
        let target = web_sys::window()
            .and_then(|window| window.navigator().platform().ok())
            .map_or("wasm", |platform| {
                let platform = platform.to_ascii_lowercase();
                if platform.contains("mac")
                    || platform.contains("iphone")
                    || platform.contains("ipad")
                {
                    "apple"
                } else if platform.contains("android") {
                    "android"
                } else if platform.contains("win") {
                    "windows"
                } else if platform.contains("linux") {
                    "linux"
                } else {
                    "wasm"
                }
            });
        #[cfg(not(target_arch = "wasm32"))]
        let target = "";
        compiler_config.style = Some(
            i_slint_common::get_native_style(i_slint_backend_selector::HAS_NATIVE_STYLE, target)
                .to_string(),
        );
    }

    let diag = BuildDiagnostics::default();
    /*#[cfg(feature = "internal-highlight")]
    let (path, mut diag, loader, raw_type_loader) =
        i_slint_compiler::load_root_file_with_raw_type_loader(
            &path,
            &path,
            source,
            diag,
            compiler_config,
        )
        .await;
    #[cfg(not(feature = "internal-highlight"))]*/
    let (path, mut diag, loader) =
        i_slint_compiler::load_root_file(&path, &path, source, diag, compiler_config).await;
    if diag.has_errors() {
        return CompilationResult {
            llr: None,
            diagnostics: diag.into_iter().collect(),
            #[cfg(feature = "internal")]
            structs_and_enums: Vec::new(),
            #[cfg(feature = "internal")]
            named_exports: Vec::new(),
        };
    }

    let doc = loader.get_document(&path).unwrap();
    let llr = llr::lower_to_item_tree::lower_to_item_tree(doc, &loader.compiler_config);

    if llr.public_components.is_empty() {
        diag.push_error_with_span("No component found".into(), Default::default());
    };

    #[cfg(feature = "internal")]
    let structs_and_enums = doc.used_types.borrow().structs_and_enums.clone();

    #[cfg(feature = "internal")]
    let named_exports = doc
        .exports
        .iter()
        .filter_map(|export| match &export.1 {
            Either::Left(component) if !component.is_global() => {
                Some((&export.0.name, &component.id))
            }
            Either::Right(ty) => match &ty {
                Type::Struct(s) if s.name.is_some() && s.node.is_some() => {
                    Some((&export.0.name, s.name.as_ref().unwrap()))
                }
                Type::Enumeration(en) => Some((&export.0.name, &en.name)),
                _ => None,
            },
            _ => None,
        })
        .filter(|(export_name, type_name)| *export_name != *type_name)
        .map(|(export_name, type_name)| (type_name.to_string(), export_name.to_string()))
        .collect::<Vec<_>>();

    CompilationResult {
        diagnostics: diag.into_iter().collect(),
        llr: Some(llr.into()),
        #[cfg(feature = "internal")]
        structs_and_enums,
        #[cfg(feature = "internal")]
        named_exports,
    }
}

/// Generate the rust code for the given component.
fn instantiate_sub_component(
    component_idx: llr::SubComponentIdx,
    root: &llr::CompilationUnit,
    parent_ctx: Option<ParentCtx>,
    index_property: Option<llr::PropertyIdx>,
    pinned_drop: bool,
) -> SubComponentInstance {
    let component = &root.sub_components[component_idx];

    let ctx = EvaluationContext::new_sub_component(
        root,
        component_idx,
        InterpreterContext::default(),
        parent_ctx,
    );


    for property in component.properties.iter().filter(|p| p.use_count.get() > 0) {
        let prop_ident = ident(&property.name);
        if let Type::Callback(callback) = &property.ty {
            let callback_args =
                callback.args.iter().map(|a| rust_primitive_type(a).unwrap()).collect::<Vec<_>>();
            let return_type = rust_primitive_type(&callback.return_type).unwrap();
            declared_callbacks.push(prop_ident.clone());
            declared_callbacks_types.push(callback_args);
            declared_callbacks_ret.push(return_type);
        } else {
            let rust_property_type = rust_property_type(&property.ty).unwrap();
            declared_property_vars.push(prop_ident.clone());
            declared_property_types.push(rust_property_type.clone());
        }
    }

    let change_tracker_names = component
        .change_callbacks
        .iter()
        .enumerate()
        .map(|(idx, _)| format_ident!("change_tracker{idx}"));

    let declared_functions = generate_functions(component.functions.as_ref(), &ctx);

    let mut init = vec![];
    let mut item_names = vec![];
    let mut item_types = vec![];

    #[cfg(slint_debug_property)]
    init.push(quote!(
        #(self_rc.#declared_property_vars.debug_name.replace(
            concat!(stringify!(#inner_component_id), ".", stringify!(#declared_property_vars)).into());)*
    ));

    for item in &component.items {
        item_names.push(ident(&item.name));
        item_types.push(ident(&item.ty.class_name));
        #[cfg(slint_debug_property)]
        {
            let mut it = Some(&item.ty);
            let elem_name = ident(&item.name);
            while let Some(ty) = it {
                for (prop, info) in &ty.properties {
                    if info.ty.is_property_type() && prop != "commands" {
                        let name = format!("{}::{}.{}", component.name, item.name, prop);
                        let prop = ident(&prop);
                        init.push(
                            quote!(self_rc.#elem_name.#prop.debug_name.replace(#name.into());),
                        );
                    }
                }
                it = ty.parent.as_ref();
            }
        }
    }

    let mut repeated_visit_branch: Vec<TokenStream> = vec![];
    let mut repeated_element_components: Vec<TokenStream> = vec![];
    let mut repeated_subtree_ranges: Vec<TokenStream> = vec![];
    let mut repeated_subtree_components: Vec<TokenStream> = vec![];

    for (idx, repeated) in component.repeated.iter_enumerated() {
        extra_components.push(generate_repeated_component(
            repeated,
            root,
            ParentCtx::new(&ctx, Some(idx)),
        ));

        let idx = usize::from(idx) as u32;

        if let Some(item_index) = repeated.container_item_index {
            let embed_item = access_member(
                &llr::PropertyReference::InNativeItem {
                    sub_component_path: vec![],
                    item_index,
                    prop_name: String::new(),
                },
                &ctx,
            )
            .unwrap();

            let ensure_updated = {
                quote! {
                    #embed_item.ensure_updated();
                }
            };

            repeated_visit_branch.push(quote!(
                #idx => {
                    #ensure_updated
                    #embed_item.visit_children_item(-1, order, visitor)
                }
            ));
            repeated_subtree_ranges.push(quote!(
                #idx => {
                    #ensure_updated
                    #embed_item.subtree_range()
                }
            ));
            repeated_subtree_components.push(quote!(
                #idx => {
                    #ensure_updated
                    if subtree_index == 0 {
                        *result = #embed_item.subtree_component()
                    }
                }
            ));
        } else {
            let repeater_id = format_ident!("repeater{}", idx);
            let rep_inner_component_id =
                self::inner_component_id(&root.sub_components[repeated.sub_tree.root]);

            let model = compile_expression(&repeated.model.borrow(), &ctx);
            init.push(quote! {
                _self.#repeater_id.set_model_binding({
                    let self_weak = sp::VRcMapped::downgrade(&self_rc);
                    move || {
                        let self_rc = self_weak.upgrade().unwrap();
                        let _self = self_rc.as_pin_ref();
                        (#model) as _
                    }
                });
            });
            let ensure_updated = if let Some(listview) = &repeated.listview {
                let vp_y = access_member(&listview.viewport_y, &ctx).unwrap();
                let vp_h = access_member(&listview.viewport_height, &ctx).unwrap();
                let lv_h = access_member(&listview.listview_height, &ctx).unwrap();
                let vp_w = access_member(&listview.viewport_width, &ctx).unwrap();
                let lv_w = access_member(&listview.listview_width, &ctx).unwrap();

                quote! {
                    #inner_component_id::FIELD_OFFSETS.#repeater_id.apply_pin(_self).ensure_updated_listview(
                        || { #rep_inner_component_id::new(_self.self_weak.get().unwrap().clone()).unwrap().into() },
                        #vp_w, #vp_h, #vp_y, #lv_w.get(), #lv_h
                    );
                }
            } else {
                quote! {
                    #inner_component_id::FIELD_OFFSETS.#repeater_id.apply_pin(_self).ensure_updated(
                        || #rep_inner_component_id::new(_self.self_weak.get().unwrap().clone()).unwrap().into()
                    );
                }
            };
            repeated_visit_branch.push(quote!(
                #idx => {
                    #ensure_updated
                    _self.#repeater_id.visit(order, visitor)
                }
            ));
            repeated_subtree_ranges.push(quote!(
                #idx => {
                    #ensure_updated
                    sp::IndexRange::from(_self.#repeater_id.range())
                }
            ));
            repeated_subtree_components.push(quote!(
                #idx => {
                    #ensure_updated
                    if let Some(instance) = _self.#repeater_id.instance_at(subtree_index) {
                        *result = sp::VRc::downgrade(&sp::VRc::into_dyn(instance));
                    }
                }
            ));
            repeated_element_components.push(if repeated.index_prop.is_some() {
                quote!(#repeater_id: sp::Repeater<#rep_inner_component_id>)
            } else {
                quote!(#repeater_id: sp::Conditional<#rep_inner_component_id>)
            });
        }
    }

    let mut accessible_role_branch = vec![];
    let mut accessible_string_property_branch = vec![];
    let mut accessibility_action_branch = vec![];
    let mut supported_accessibility_actions = BTreeMap::<u32, BTreeSet<_>>::new();
    for ((index, what), expr) in &component.accessible_prop {
        let e = compile_expression(&expr.borrow(), &ctx);
        if what == "Role" {
            accessible_role_branch.push(quote!(#index => #e,));
        } else if let Some(what) = what.strip_prefix("Action") {
            let what = ident(what);
            let has_args = matches!(&*expr.borrow(), Expression::CallBackCall { arguments, .. } if !arguments.is_empty());
            accessibility_action_branch.push(if has_args {
                quote!((#index, sp::AccessibilityAction::#what(args)) => { let args = (args,); #e })
            } else {
                quote!((#index, sp::AccessibilityAction::#what) => { #e })
            });
            supported_accessibility_actions.entry(*index).or_default().insert(what);
        } else {
            let what = ident(what);
            accessible_string_property_branch
                .push(quote!((#index, sp::AccessibleStringProperty::#what) => sp::Some(#e),));
        }
    }
    let mut supported_accessibility_actions_branch = supported_accessibility_actions
        .into_iter()
        .map(|(index, values)| quote!(#index => #(sp::SupportedAccessibilityAction::#values)|*,))
        .collect::<Vec<_>>();

    let mut item_geometry_branch = component
        .geometries
        .iter()
        .enumerate()
        .filter_map(|(i, x)| x.as_ref().map(|x| (i, x)))
        .map(|(index, expr)| {
            let expr = compile_expression(&expr.borrow(), &ctx);
            let index = index as u32;
            quote!(#index => #expr,)
        })
        .collect::<Vec<_>>();

    let mut item_element_infos_branch = component
        .element_infos
        .iter()
        .map(|(item_index, ids)| quote!(#item_index => { return sp::Some(#ids.into()); }))
        .collect::<Vec<_>>();

    let mut user_init_code: Vec<TokenStream> = Vec::new();

    let mut sub_component_names: Vec<Ident> = vec![];
    let mut sub_component_types: Vec<Ident> = vec![];

    for sub in &component.sub_components {
        let field_name = ident(&sub.name);
        let sc = &root.sub_components[sub.ty];
        let sub_component_id = self::inner_component_id(sc);
        let local_tree_index: u32 = sub.index_in_tree as _;
        let local_index_of_first_child: u32 = sub.index_of_first_child_in_tree as _;
        let global_access = &ctx.generator_state.global_access;

        // For children of sub-components, the item index generated by the generate_item_indices pass
        // starts at 1 (0 is the root element).
        let global_index = if local_tree_index == 0 {
            quote!(tree_index)
        } else {
            quote!(tree_index_of_first_child + #local_tree_index - 1)
        };
        let global_children = if local_index_of_first_child == 0 {
            quote!(0)
        } else {
            quote!(tree_index_of_first_child + #local_index_of_first_child - 1)
        };

        let sub_compo_field = access_component_field_offset(&format_ident!("Self"), &field_name);

        init.push(quote!(#sub_component_id::init(
            sp::VRcMapped::map(self_rc.clone(), |x| #sub_compo_field.apply_pin(x)),
            #global_access.clone(), #global_index, #global_children
        );));
        user_init_code.push(quote!(#sub_component_id::user_init(
            sp::VRcMapped::map(self_rc.clone(), |x| #sub_compo_field.apply_pin(x)),
        );));

        let sub_component_repeater_count = sc.repeater_count(root);
        if sub_component_repeater_count > 0 {
            let repeater_offset = sub.repeater_offset;
            let last_repeater = repeater_offset + sub_component_repeater_count - 1;
            repeated_visit_branch.push(quote!(
                #repeater_offset..=#last_repeater => {
                    #sub_compo_field.apply_pin(_self).visit_dynamic_children(dyn_index - #repeater_offset, order, visitor)
                }
            ));
            repeated_subtree_ranges.push(quote!(
                #repeater_offset..=#last_repeater => {
                    #sub_compo_field.apply_pin(_self).subtree_range(dyn_index - #repeater_offset)
                }
            ));
            repeated_subtree_components.push(quote!(
                #repeater_offset..=#last_repeater => {
                    #sub_compo_field.apply_pin(_self).subtree_component(dyn_index - #repeater_offset, subtree_index, result)
                }
            ));
        }

        let sub_items_count = sc.child_item_count(root);
        accessible_role_branch.push(quote!(
            #local_tree_index => #sub_compo_field.apply_pin(_self).accessible_role(0),
        ));
        accessible_string_property_branch.push(quote!(
            (#local_tree_index, _) => #sub_compo_field.apply_pin(_self).accessible_string_property(0, what),
        ));
        accessibility_action_branch.push(quote!(
            (#local_tree_index, _) => #sub_compo_field.apply_pin(_self).accessibility_action(0, action),
        ));
        supported_accessibility_actions_branch.push(quote!(
            #local_tree_index => #sub_compo_field.apply_pin(_self).supported_accessibility_actions(0),
        ));
        if sub_items_count > 1 {
            let range_begin = local_index_of_first_child;
            let range_end = range_begin + sub_items_count - 2 + sc.repeater_count(root);
            accessible_role_branch.push(quote!(
                #range_begin..=#range_end => #sub_compo_field.apply_pin(_self).accessible_role(index - #range_begin + 1),
            ));
            accessible_string_property_branch.push(quote!(
                (#range_begin..=#range_end, _) => #sub_compo_field.apply_pin(_self).accessible_string_property(index - #range_begin + 1, what),
            ));
            item_geometry_branch.push(quote!(
                #range_begin..=#range_end => return #sub_compo_field.apply_pin(_self).item_geometry(index - #range_begin + 1),
            ));
            accessibility_action_branch.push(quote!(
                (#range_begin..=#range_end, _) => #sub_compo_field.apply_pin(_self).accessibility_action(index - #range_begin + 1, action),
            ));
            supported_accessibility_actions_branch.push(quote!(
                #range_begin..=#range_end => #sub_compo_field.apply_pin(_self).supported_accessibility_actions(index - #range_begin + 1),
            ));
            item_element_infos_branch.push(quote!(
                #range_begin..=#range_end => #sub_compo_field.apply_pin(_self).item_element_infos(index - #range_begin + 1),
            ));
        }

        sub_component_names.push(field_name);
        sub_component_types.push(sub_component_id);
    }

    let popup_id_names =
        component.popup_windows.iter().enumerate().map(|(i, _)| internal_popup_id(i));

    for (prop1, prop2, fields) in &component.two_way_bindings {
        let p1 = access_member(prop1, &ctx);
        let p2 = access_member(prop2, &ctx);
        let r = p1.then(|p1| {
            p2.then(|p2| {
                if fields.is_empty() {
                    quote!(sp::Property::link_two_way(#p1, #p2))
                } else {
                    let mut access = quote!();
                    let mut ty = ctx.property_ty(prop2);
                    for f in fields {
                        let Type::Struct (s) = &ty else { panic!("Field of two way binding on a non-struct type") };
                        let a = struct_field_access(s, f);
                        access.extend(quote!(.#a));
                        ty = s.fields.get(f).unwrap();
                    }
                    quote!(sp::Property::link_two_way_with_map(#p2, #p1, |s| s #access .clone(), |s, v| s #access = v.clone()))
                }
            })
        });
        init.push(quote!(#r;))
    }

    for (prop, expression) in &component.property_init {
        if expression.use_count.get() > 0 && component.prop_used(prop, root) {
            handle_property_init(prop, expression, &mut init, &ctx)
        }
    }
    for prop in &component.const_properties {
        if component.prop_used(prop, root) {
            let rust_property = access_member(prop, &ctx).unwrap();
            init.push(quote!(#rust_property.set_constant();))
        }
    }

    let parent_component_type = parent_ctx.iter().map(|parent| {
        let parent_component_id =
            self::inner_component_id(parent.ctx.current_sub_component().unwrap());
        quote!(sp::VWeakMapped::<sp::ItemTreeVTable, #parent_component_id>)
    });

    user_init_code.extend(component.init_code.iter().map(|e| {
        let code = compile_expression(&e.borrow(), &ctx);
        quote!(#code;)
    }));

    user_init_code.extend(component.change_callbacks.iter().enumerate().map(|(idx, (p, e))| {
        let code = compile_expression(&e.borrow(), &ctx);
        let prop = compile_expression(&Expression::PropertyReference(p.clone()), &ctx);
        let change_tracker = format_ident!("change_tracker{idx}");
        quote! {
            let self_weak = sp::VRcMapped::downgrade(&self_rc);
            #[allow(dead_code, unused)]
            _self.#change_tracker.init(
                self_weak,
                move |self_weak| {
                    let self_rc = self_weak.upgrade().unwrap();
                    let _self = self_rc.as_pin_ref();
                    #prop
                },
                move |self_weak, _| {
                    let self_rc = self_weak.upgrade().unwrap();
                    let _self = self_rc.as_pin_ref();
                    #code;
                }
            );
        }
    }));

    let layout_info_h = compile_expression_no_parenthesis(&component.layout_info_h.borrow(), &ctx);
    let layout_info_v = compile_expression_no_parenthesis(&component.layout_info_v.borrow(), &ctx);

    // FIXME! this is only public because of the ComponentHandle::WeakInner. we should find another way
    let visibility = parent_ctx.is_none().then(|| quote!(pub));

    let subtree_index_function = if let Some(property_index) = index_property {
        let prop = access_member(
            &llr::PropertyReference::Local { sub_component_path: vec![], property_index },
            &ctx,
        )
        .unwrap();
        quote!(#prop.get() as usize)
    } else {
        quote!(usize::MAX)
    };

    let timer_names =
        component.timers.iter().enumerate().map(|(idx, _)| format_ident!("timer{idx}"));
    let update_timers = (!component.timers.is_empty()).then(|| {
        let updt = component.timers.iter().enumerate().map(|(idx, tmr)| {
            let ident = format_ident!("timer{idx}");
            let interval = compile_expression(&tmr.interval.borrow(), &ctx);
            let running = compile_expression(&tmr.running.borrow(), &ctx);
            let callback = compile_expression(&tmr.triggered.borrow(), &ctx);
            quote!(
                if #running {
                    let interval = ::core::time::Duration::from_millis(#interval as u64);
                    if !self.#ident.running() || interval != self.#ident.interval() {
                        let self_weak = self.self_weak.get().unwrap().clone();
                        self.#ident.start(sp::TimerMode::Repeated, interval, move || {
                            if let Some(self_rc) = self_weak.upgrade() {
                                let _self = self_rc.as_pin_ref();
                                #callback
                            }
                        });
                    }
                } else {
                    self.#ident.stop();
                }
            )
        });
        user_init_code.push(quote!(_self.update_timers();));
        quote!(
            fn update_timers(self: ::core::pin::Pin<&Self>) {
                let _self = self;
                #(#updt)*
            }
        )
    });

    let pin_macro = if pinned_drop { quote!(#[pin_drop]) } else { quote!(#[pin]) };

    quote!(
        #[derive(sp::FieldOffsets, Default)]
        #[const_field_offset(sp::const_field_offset)]
        #[repr(C)]
        #pin_macro
        #visibility
        struct #inner_component_id {
            #(#item_names : sp::#item_types,)*
            #(#sub_component_names : #sub_component_types,)*
            #(#popup_id_names : ::core::cell::Cell<sp::Option<::core::num::NonZeroU32>>,)*
            #(#declared_property_vars : sp::Property<#declared_property_types>,)*
            #(#declared_callbacks : sp::Callback<(#(#declared_callbacks_types,)*), #declared_callbacks_ret>,)*
            #(#repeated_element_components,)*
            #(#change_tracker_names : sp::ChangeTracker,)*
            #(#timer_names : sp::Timer,)*
            self_weak : sp::OnceCell<sp::VWeakMapped<sp::ItemTreeVTable, #inner_component_id>>,
            #(parent : #parent_component_type,)*
            globals: sp::OnceCell<sp::Rc<SharedGlobals>>,
            tree_index: ::core::cell::Cell<u32>,
            tree_index_of_first_child: ::core::cell::Cell<u32>,
        }

        impl #inner_component_id {
            fn init(self_rc: sp::VRcMapped<sp::ItemTreeVTable, Self>,
                    globals : sp::Rc<SharedGlobals>,
                    tree_index: u32, tree_index_of_first_child: u32) {
                #![allow(unused)]
                let _self = self_rc.as_pin_ref();
                let _ = _self.self_weak.set(sp::VRcMapped::downgrade(&self_rc));
                let _ = _self.globals.set(globals);
                _self.tree_index.set(tree_index);
                _self.tree_index_of_first_child.set(tree_index_of_first_child);
                #(#init)*
            }

            fn user_init(self_rc: sp::VRcMapped<sp::ItemTreeVTable, Self>) {
                #![allow(unused)]
                let _self = self_rc.as_pin_ref();
                #(#user_init_code)*
            }

            fn visit_dynamic_children(
                self: ::core::pin::Pin<&Self>,
                dyn_index: u32,
                order: sp::TraversalOrder,
                visitor: sp::ItemVisitorRefMut<'_>
            ) -> sp::VisitChildrenResult {
                #![allow(unused)]
                let _self = self;
                match dyn_index {
                    #(#repeated_visit_branch)*
                    _ => panic!("invalid dyn_index {}", dyn_index),
                }
            }

            fn layout_info(self: ::core::pin::Pin<&Self>, orientation: sp::Orientation) -> sp::LayoutInfo {
                #![allow(unused)]
                let _self = self;
                match orientation {
                    sp::Orientation::Horizontal => #layout_info_h,
                    sp::Orientation::Vertical => #layout_info_v,
                }
            }

            fn subtree_range(self: ::core::pin::Pin<&Self>, dyn_index: u32) -> sp::IndexRange {
                #![allow(unused)]
                let _self = self;
                match dyn_index {
                    #(#repeated_subtree_ranges)*
                    _ => panic!("invalid dyn_index {}", dyn_index),
                }
            }

            fn subtree_component(self: ::core::pin::Pin<&Self>, dyn_index: u32, subtree_index: usize, result: &mut sp::ItemTreeWeak) {
                #![allow(unused)]
                let _self = self;
                match dyn_index {
                    #(#repeated_subtree_components)*
                    _ => panic!("invalid dyn_index {}", dyn_index),
                };
            }

            fn index_property(self: ::core::pin::Pin<&Self>) -> usize {
                #![allow(unused)]
                let _self = self;
                #subtree_index_function
            }

            fn item_geometry(self: ::core::pin::Pin<&Self>, index: u32) -> sp::LogicalRect {
                #![allow(unused)]
                let _self = self;
                // The result of the expression is an anonymous struct, `{height: length, width: length, x: length, y: length}`
                // fields are in alphabetical order
                let (h, w, x, y) = match index {
                    #(#item_geometry_branch)*
                    _ => return ::core::default::Default::default()
                };
                sp::euclid::rect(x, y, w, h)
            }

            fn accessible_role(self: ::core::pin::Pin<&Self>, index: u32) -> sp::AccessibleRole {
                #![allow(unused)]
                let _self = self;
                match index {
                    #(#accessible_role_branch)*
                    //#(#forward_sub_ranges => #forward_sub_field.apply_pin(_self).accessible_role())*
                    _ => sp::AccessibleRole::default(),
                }
            }

            fn accessible_string_property(
                self: ::core::pin::Pin<&Self>,
                index: u32,
                what: sp::AccessibleStringProperty,
            ) -> sp::Option<sp::SharedString> {
                #![allow(unused)]
                let _self = self;
                match (index, what) {
                    #(#accessible_string_property_branch)*
                    _ => sp::None,
                }
            }

            fn accessibility_action(self: ::core::pin::Pin<&Self>, index: u32, action: &sp::AccessibilityAction) {
                #![allow(unused)]
                let _self = self;
                match (index, action) {
                    #(#accessibility_action_branch)*
                    _ => (),
                }
            }

            fn supported_accessibility_actions(self: ::core::pin::Pin<&Self>, index: u32) -> sp::SupportedAccessibilityAction {
                #![allow(unused)]
                let _self = self;
                match index {
                    #(#supported_accessibility_actions_branch)*
                    _ => ::core::default::Default::default(),
                }
            }

            fn item_element_infos(self: ::core::pin::Pin<&Self>, index: u32) -> sp::Option<sp::SharedString> {
                #![allow(unused)]
                let _self = self;
                match index {
                    #(#item_element_infos_branch)*
                    _ => { ::core::default::Default::default() }
                }
            }

            #update_timers

            #(#declared_functions)*
        }

        #(#extra_components)*
    )
}

fn instantiate_item_tree(
    sub_tree: &llr::ItemTree,
    root: &llr::CompilationUnit,
    parent_ctx: Option<ParentCtx>,
    index_property: Option<llr::PropertyIdx>,
    is_popup_menu: bool,
) -> DynamicItemTreeRc {
    let sub_comp = instentiate_sub_component(sub_tree.root, root, parent_ctx, index_property, true);
    let inner_component_id = self::inner_component_id(&root.sub_components[sub_tree.root]);
    let parent_component_type = parent_ctx
        .iter()
        .map(|parent| {
            let parent_component_id =
                self::inner_component_id(parent.ctx.current_sub_component().unwrap());
            quote!(sp::VWeakMapped::<sp::ItemTreeVTable, #parent_component_id>)
        })
        .collect::<Vec<_>>();

    let globals = if is_popup_menu {
        quote!(globals)
    } else if parent_ctx.is_some() {
        quote!(parent.upgrade().unwrap().globals.get().unwrap().clone())
    } else {
        quote!(SharedGlobals::new(sp::VRc::downgrade(&self_dyn_rc)))
    };
    let globals_arg = is_popup_menu.then(|| quote!(globals: sp::Rc<SharedGlobals>));

    let embedding_function = if parent_ctx.is_some() {
        quote!(todo!("Components written in Rust can not get embedded yet."))
    } else {
        quote!(false)
    };

    let parent_item_expression = parent_ctx.map(|parent| parent.repeater_index.map_or_else(|| {
            // No repeater index, this could be a PopupWindow
            quote!(if let Some(parent_rc) = self.parent.clone().upgrade() {
                       let parent_origin = sp::VRcMapped::origin(&parent_rc);
                       // TODO: store popup index in ctx and set it here instead of 0?
                       *_result = sp::ItemRc::new(parent_origin, 0).downgrade();
                   })
        }, |idx| {
            let current_sub_component = parent.ctx.current_sub_component().unwrap();
            let sub_component_offset = current_sub_component.repeated[idx].index_in_tree;

            quote!(if let Some((parent_component, parent_index)) = self
                .parent
                .clone()
                .upgrade()
                .map(|sc| (sp::VRcMapped::origin(&sc), sc.tree_index_of_first_child.get()))
            {
                *_result = sp::ItemRc::new(parent_component, parent_index + #sub_component_offset - 1)
                    .downgrade();
            })
        }));
    let mut item_tree_array = vec![];
    let mut item_array = vec![];
    sub_tree.tree.visit_in_array(&mut |node, children_offset, parent_index| {
        let parent_index = parent_index as u32;
        let (path, component) =
            follow_sub_component_path(root, sub_tree.root, &node.sub_component_path);
        match node.item_index {
            Either::Right(mut repeater_index) => {
                assert_eq!(node.children.len(), 0);
                let mut sub_component = &root.sub_components[sub_tree.root];
                for i in &node.sub_component_path {
                    repeater_index += sub_component.sub_components[*i].repeater_offset;
                    sub_component = &root.sub_components[sub_component.sub_components[*i].ty];
                }
                item_tree_array.push(quote!(
                    sp::ItemTreeNode::DynamicTree {
                        index: #repeater_index,
                        parent_index: #parent_index,
                    }
                ));
            }
            Either::Left(item_index) => {
                let item = &component.items[item_index];
                let field = access_component_field_offset(
                    &self::inner_component_id(component),
                    &ident(&item.name),
                );

                let children_count = node.children.len() as u32;
                let children_index = children_offset as u32;
                let item_array_len = item_array.len() as u32;
                let is_accessible = node.is_accessible;
                item_tree_array.push(quote!(
                    sp::ItemTreeNode::Item {
                        is_accessible: #is_accessible,
                        children_count: #children_count,
                        children_index: #children_index,
                        parent_index: #parent_index,
                        item_array_index: #item_array_len,
                    }
                ));
                item_array.push(quote!(sp::VOffset::new(#path #field)));
            }
        }
    });

    let item_tree_array_len = item_tree_array.len();
    let item_array_len = item_array.len();

    let element_info_body = if root.has_debug_info {
        quote!(
            *_result = self.item_element_infos(_index).unwrap_or_default();
            true
        )
    } else {
        quote!(false)
    };

    quote!(
        #sub_comp

        impl #inner_component_id {
            fn new(#(parent: #parent_component_type,)* #globals_arg) -> ::core::result::Result<sp::VRc<sp::ItemTreeVTable, Self>, slint::PlatformError> {
                #![allow(unused)]
                slint::private_unstable_api::ensure_backend()?;
                let mut _self = Self::default();
                #(_self.parent = parent.clone() as #parent_component_type;)*
                let self_rc = sp::VRc::new(_self);
                let self_dyn_rc = sp::VRc::into_dyn(self_rc.clone());
                let globals = #globals;
                sp::register_item_tree(&self_dyn_rc, globals.maybe_window_adapter_impl());
                Self::init(sp::VRc::map(self_rc.clone(), |x| x), globals, 0, 1);
                ::core::result::Result::Ok(self_rc)
            }

            fn item_tree() -> &'static [sp::ItemTreeNode] {
                const ITEM_TREE : [sp::ItemTreeNode; #item_tree_array_len] = [#(#item_tree_array),*];
                &ITEM_TREE
            }

            fn item_array() -> &'static [sp::VOffset<Self, sp::ItemVTable, sp::AllowPin>] {
                // FIXME: ideally this should be a const, but we can't because of the pointer to the vtable
                static ITEM_ARRAY : sp::OnceBox<
                    [sp::VOffset<#inner_component_id, sp::ItemVTable, sp::AllowPin>; #item_array_len]
                > = sp::OnceBox::new();
                &*ITEM_ARRAY.get_or_init(|| sp::vec![#(#item_array),*].into_boxed_slice().try_into().unwrap())
            }
        }

        const _ : () = {
            use slint::private_unstable_api::re_exports::*;
            ItemTreeVTable_static!(static VT for self::#inner_component_id);
        };

        impl sp::PinnedDrop for #inner_component_id {
            fn drop(self: ::core::pin::Pin<&mut #inner_component_id>) {
                sp::vtable::new_vref!(let vref : VRef<sp::ItemTreeVTable> for sp::ItemTree = self.as_ref().get_ref());
                if let Some(wa) = self.globals.get().unwrap().maybe_window_adapter_impl() {
                    sp::unregister_item_tree(self.as_ref(), vref, Self::item_array(), &wa);
                }
            }
        }

        impl sp::ItemTree for #inner_component_id {
            fn visit_children_item(self: ::core::pin::Pin<&Self>, index: isize, order: sp::TraversalOrder, visitor: sp::ItemVisitorRefMut<'_>)
                -> sp::VisitChildrenResult
            {
                return sp::visit_item_tree(self, &sp::VRcMapped::origin(&self.as_ref().self_weak.get().unwrap().upgrade().unwrap()), self.get_item_tree().as_slice(), index, order, visitor, visit_dynamic);
                #[allow(unused)]
                fn visit_dynamic(_self: ::core::pin::Pin<&#inner_component_id>, order: sp::TraversalOrder, visitor: sp::ItemVisitorRefMut<'_>, dyn_index: u32) -> sp::VisitChildrenResult  {
                    _self.visit_dynamic_children(dyn_index, order, visitor)
                }
            }

            fn get_item_ref(self: ::core::pin::Pin<&Self>, index: u32) -> ::core::pin::Pin<sp::ItemRef<'_>> {
                match &self.get_item_tree().as_slice()[index as usize] {
                    sp::ItemTreeNode::Item { item_array_index, .. } => {
                        Self::item_array()[*item_array_index as usize].apply_pin(self)
                    }
                    sp::ItemTreeNode::DynamicTree { .. } => panic!("get_item_ref called on dynamic tree"),

                }
            }

            fn get_item_tree(
                self: ::core::pin::Pin<&Self>) -> sp::Slice<'_, sp::ItemTreeNode>
            {
                Self::item_tree().into()
            }

            fn get_subtree_range(
                self: ::core::pin::Pin<&Self>, index: u32) -> sp::IndexRange
            {
                self.subtree_range(index)
            }

            fn get_subtree(
                self: ::core::pin::Pin<&Self>, index: u32, subtree_index: usize, result: &mut sp::ItemTreeWeak)
            {
                self.subtree_component(index, subtree_index, result);
            }

            fn subtree_index(
                self: ::core::pin::Pin<&Self>) -> usize
            {
                self.index_property()
            }

            fn parent_node(self: ::core::pin::Pin<&Self>, _result: &mut sp::ItemWeak) {
                #parent_item_expression
            }

            fn embed_component(self: ::core::pin::Pin<&Self>, _parent_component: &sp::ItemTreeWeak, _item_tree_index: u32) -> bool {
                #embedding_function
            }

            fn layout_info(self: ::core::pin::Pin<&Self>, orientation: sp::Orientation) -> sp::LayoutInfo {
                self.layout_info(orientation)
            }

            fn item_geometry(self: ::core::pin::Pin<&Self>, index: u32) -> sp::LogicalRect {
                self.item_geometry(index)
            }

            fn accessible_role(self: ::core::pin::Pin<&Self>, index: u32) -> sp::AccessibleRole {
                self.accessible_role(index)
            }

            fn accessible_string_property(
                self: ::core::pin::Pin<&Self>,
                index: u32,
                what: sp::AccessibleStringProperty,
                result: &mut sp::SharedString,
            ) -> bool {
                if let Some(r) = self.accessible_string_property(index, what) {
                    *result = r;
                    true
                } else {
                    false
                }
            }

            fn accessibility_action(self: ::core::pin::Pin<&Self>, index: u32, action: &sp::AccessibilityAction) {
                self.accessibility_action(index, action);
            }

            fn supported_accessibility_actions(self: ::core::pin::Pin<&Self>, index: u32) -> sp::SupportedAccessibilityAction {
                self.supported_accessibility_actions(index)
            }

            fn item_element_infos(
                self: ::core::pin::Pin<&Self>,
                _index: u32,
                _result: &mut sp::SharedString,
            ) -> bool {
                #element_info_body
            }

            fn window_adapter(
                self: ::core::pin::Pin<&Self>,
                do_create: bool,
                result: &mut sp::Option<sp::Rc<dyn sp::WindowAdapter>>,
            ) {
                if do_create {
                    *result = sp::Some(self.globals.get().unwrap().window_adapter_impl());
                } else {
                    *result = self.globals.get().unwrap().maybe_window_adapter_impl();
                }
            }
        }


    )
}

/*
use crate::api::{CompilationResult, ComponentDefinition, Value};
use crate::global_component::CompiledGlobalCollection;
use crate::{dynamic_type, eval};
use core::ptr::NonNull;
use dynamic_type::{Instance, InstanceBox};
use i_slint_compiler::expression_tree::{Expression, NamedReference};
use i_slint_compiler::langtype::{BuiltinPrivateStruct, StructName, Type};
use i_slint_compiler::object_tree::{ElementRc, ElementWeak, TransitionDirection};
use i_slint_compiler::{diagnostics::BuildDiagnostics, object_tree::PropertyDeclaration};
use i_slint_compiler::{generator, object_tree, parser, CompilerConfiguration};
use i_slint_core::accessibility::{
    AccessibilityAction, AccessibleStringProperty, SupportedAccessibilityAction,
};
use i_slint_core::api::LogicalPosition;
use i_slint_core::component_factory::ComponentFactory;
use i_slint_core::item_tree::{
    IndexRange, ItemRc, ItemTree, ItemTreeNode, ItemTreeRef, ItemTreeRefPin, ItemTreeVTable,
    ItemTreeWeak, ItemVisitorRefMut, ItemVisitorVTable, ItemWeak, TraversalOrder,
    VisitChildrenResult,
};
use i_slint_core::items::{
    AccessibleRole, ItemRef, ItemVTable, PopupClosePolicy, PropertyAnimation,
};
use i_slint_core::layout::{BoxLayoutCellData, LayoutInfo, Orientation};
use i_slint_core::lengths::{LogicalLength, LogicalRect};
use i_slint_core::menus::MenuFromItemTree;
use i_slint_core::model::{ModelRc, RepeatedItemTree, Repeater};
use i_slint_core::platform::PlatformError;
use i_slint_core::properties::{ChangeTracker, InterpolatedPropertyValue};
use i_slint_core::rtti::{self, AnimatedBindingKind, FieldOffset, PropertyInfo};
use i_slint_core::slice::Slice;
use i_slint_core::timers::Timer;
use i_slint_core::window::{WindowAdapterRc, WindowInner};
use i_slint_core::{Brush, Color, Property, SharedString, SharedVector};
#[cfg(feature = "internal")]
use itertools::Either;
use once_cell::unsync::{Lazy, OnceCell};
use smol_str::{SmolStr, ToSmolStr};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::rc::Weak;
use std::{pin::Pin, rc::Rc};

pub const SPECIAL_PROPERTY_INDEX: &str = "$index";
pub const SPECIAL_PROPERTY_MODEL_DATA: &str = "$model_data";

pub(crate) type CallbackHandler = Box<dyn Fn(&[Value]) -> Value>;

pub struct ItemTreeBox<'id> {
    instance: InstanceBox<'id>,
    description: Rc<ItemTreeDescription<'id>>,
}

impl<'id> ItemTreeBox<'id> {
    /// Borrow this instance as a `Pin<ItemTreeRef>`
    pub fn borrow(&self) -> ItemTreeRefPin<'_> {
        self.borrow_instance().borrow()
    }

    /// Safety: the lifetime is not unique
    pub fn description(&self) -> Rc<ItemTreeDescription<'id>> {
        self.description.clone()
    }

    pub fn borrow_instance<'a>(&'a self) -> InstanceRef<'a, 'id> {
        InstanceRef { instance: self.instance.as_pin_ref(), description: &self.description }
    }

    pub fn window_adapter_ref(&self) -> Result<&WindowAdapterRc, PlatformError> {
        let root_weak = vtable::VWeak::into_dyn(self.borrow_instance().root_weak().clone());
        InstanceRef::get_or_init_window_adapter_ref(
            &self.description,
            root_weak,
            true,
            self.instance.as_pin_ref().get_ref(),
        )
    }
}

pub(crate) type ErasedItemTreeBoxWeak = vtable::VWeak<ItemTreeVTable, ErasedItemTreeBox>;

pub(crate) struct ItemWithinItemTree {
    offset: usize,
    pub(crate) rtti: Rc<ItemRTTI>,
    elem: ElementRc,
}

impl ItemWithinItemTree {
    /// Safety: the pointer must be a dynamic item tree which is coming from the same description as Self
    pub(crate) unsafe fn item_from_item_tree(
        &self,
        mem: *const u8,
    ) -> Pin<vtable::VRef<'_, ItemVTable>> {
        Pin::new_unchecked(vtable::VRef::from_raw(
            NonNull::from(self.rtti.vtable),
            NonNull::new(mem.add(self.offset) as _).unwrap(),
        ))
    }

    pub(crate) fn item_index(&self) -> u32 {
        *self.elem.borrow().item_index.get().unwrap()
    }
}

pub(crate) struct PropertiesWithinComponent {
    pub(crate) offset: usize,
    pub(crate) prop: Box<dyn PropertyInfo<u8, Value>>,
}

pub(crate) struct RepeaterWithinItemTree<'par_id, 'sub_id> {
    /// The description of the items to repeat
    pub(crate) item_tree_to_repeat: Rc<ItemTreeDescription<'sub_id>>,
    /// The model
    pub(crate) model: Expression,
    /// Offset of the `Repeater`
    offset: FieldOffset<Instance<'par_id>, Repeater<ErasedItemTreeBox>>,
    /// When true, it is representing a `if`, instead of a `for`.
    /// Based on [`i_slint_compiler::object_tree::RepeatedElementInfo::is_conditional_element`]
    is_conditional: bool,
}

impl RepeatedItemTree for ErasedItemTreeBox {
    type Data = Value;

    fn update(&self, index: usize, data: Self::Data) {
        generativity::make_guard!(guard);
        let s = self.unerase(guard);
        let is_repeated = s.description.original.parent_element.upgrade().is_some_and(|p| {
            p.borrow().repeated.as_ref().is_some_and(|r| !r.is_conditional_element)
        });
        if is_repeated {
            s.description.set_property(s.borrow(), SPECIAL_PROPERTY_INDEX, index.into()).unwrap();
            s.description.set_property(s.borrow(), SPECIAL_PROPERTY_MODEL_DATA, data).unwrap();
        }
    }

    fn init(&self) {
        self.run_setup_code();
    }

    fn listview_layout(self: Pin<&Self>, offset_y: &mut LogicalLength) -> LogicalLength {
        generativity::make_guard!(guard);
        let s = self.unerase(guard);

        let geom = s.description.original.root_element.borrow().geometry_props.clone().unwrap();

        crate::eval::store_property(
            s.borrow_instance(),
            &geom.y.element(),
            geom.y.name(),
            Value::Number(offset_y.get() as f64),
        )
        .expect("cannot set y");

        let h: LogicalLength = crate::eval::load_property(
            s.borrow_instance(),
            &geom.height.element(),
            geom.height.name(),
        )
        .expect("missing height")
        .try_into()
        .expect("height not the right type");

        *offset_y += h;
        LogicalLength::new(self.borrow().as_ref().layout_info(Orientation::Horizontal).min)
    }

    fn box_layout_data(self: Pin<&Self>, o: Orientation) -> BoxLayoutCellData {
        BoxLayoutCellData { constraint: self.borrow().as_ref().layout_info(o) }
    }
}

impl ItemTree for ErasedItemTreeBox {
    fn visit_children_item(
        self: Pin<&Self>,
        index: isize,
        order: TraversalOrder,
        visitor: ItemVisitorRefMut,
    ) -> VisitChildrenResult {
        self.borrow().as_ref().visit_children_item(index, order, visitor)
    }

    fn layout_info(self: Pin<&Self>, orientation: Orientation) -> i_slint_core::layout::LayoutInfo {
        self.borrow().as_ref().layout_info(orientation)
    }

    fn get_item_tree(self: Pin<&Self>) -> Slice<'_, ItemTreeNode> {
        get_item_tree(self.get_ref().borrow())
    }

    fn get_item_ref(self: Pin<&Self>, index: u32) -> Pin<ItemRef<'_>> {
        // We're having difficulties transferring the lifetime to a pinned reference
        // to the other ItemTreeVTable with the same life time. So skip the vtable
        // indirection and call our implementation directly.
        unsafe { get_item_ref(self.get_ref().borrow(), index) }
    }

    fn get_subtree_range(self: Pin<&Self>, index: u32) -> IndexRange {
        self.borrow().as_ref().get_subtree_range(index)
    }

    fn get_subtree(self: Pin<&Self>, index: u32, subindex: usize, result: &mut ItemTreeWeak) {
        self.borrow().as_ref().get_subtree(index, subindex, result);
    }

    fn parent_node(self: Pin<&Self>, result: &mut ItemWeak) {
        self.borrow().as_ref().parent_node(result)
    }

    fn embed_component(
        self: core::pin::Pin<&Self>,
        parent_component: &ItemTreeWeak,
        item_tree_index: u32,
    ) -> bool {
        self.borrow().as_ref().embed_component(parent_component, item_tree_index)
    }

    fn subtree_index(self: Pin<&Self>) -> usize {
        self.borrow().as_ref().subtree_index()
    }

    fn item_geometry(self: Pin<&Self>, item_index: u32) -> i_slint_core::lengths::LogicalRect {
        self.borrow().as_ref().item_geometry(item_index)
    }

    fn accessible_role(self: Pin<&Self>, index: u32) -> AccessibleRole {
        self.borrow().as_ref().accessible_role(index)
    }

    fn accessible_string_property(
        self: Pin<&Self>,
        index: u32,
        what: AccessibleStringProperty,
        result: &mut SharedString,
    ) -> bool {
        self.borrow().as_ref().accessible_string_property(index, what, result)
    }

    fn window_adapter(self: Pin<&Self>, do_create: bool, result: &mut Option<WindowAdapterRc>) {
        self.borrow().as_ref().window_adapter(do_create, result);
    }

    fn accessibility_action(self: core::pin::Pin<&Self>, index: u32, action: &AccessibilityAction) {
        self.borrow().as_ref().accessibility_action(index, action)
    }

    fn supported_accessibility_actions(
        self: core::pin::Pin<&Self>,
        index: u32,
    ) -> SupportedAccessibilityAction {
        self.borrow().as_ref().supported_accessibility_actions(index)
    }

    fn item_element_infos(
        self: core::pin::Pin<&Self>,
        index: u32,
        result: &mut SharedString,
    ) -> bool {
        self.borrow().as_ref().item_element_infos(index, result)
    }
}

i_slint_core::ItemTreeVTable_static!(static COMPONENT_BOX_VT for ErasedItemTreeBox);

impl Drop for ErasedItemTreeBox {
    fn drop(&mut self) {
        generativity::make_guard!(guard);
        let unerase = self.unerase(guard);
        let instance_ref = unerase.borrow_instance();
        // Do not walk out of our ItemTree here:
        if let Some(window_adapter) = instance_ref.maybe_window_adapter() {
            i_slint_core::item_tree::unregister_item_tree(
                instance_ref.instance,
                vtable::VRef::new(self),
                instance_ref.description.item_array.as_slice(),
                &window_adapter,
            );
        }
    }
}

pub type DynamicComponentVRc = vtable::VRc<ItemTreeVTable, ErasedItemTreeBox>;

#[derive(Default)]
pub(crate) struct ComponentExtraData {
    pub(crate) globals: OnceCell<crate::global_component::GlobalStorage>,
    pub(crate) self_weak: OnceCell<ErasedItemTreeBoxWeak>,
    pub(crate) embedding_position: OnceCell<(ItemTreeWeak, u32)>,
}

struct ErasedRepeaterWithinComponent<'id>(RepeaterWithinItemTree<'id, 'static>);
impl<'id, 'sub_id> From<RepeaterWithinItemTree<'id, 'sub_id>>
    for ErasedRepeaterWithinComponent<'id>
{
    fn from(from: RepeaterWithinItemTree<'id, 'sub_id>) -> Self {
        // Safety: this is safe as we erase the sub_id lifetime.
        // As long as when we get it back we get an unique lifetime with ErasedRepeaterWithinComponent::unerase
        Self(unsafe {
            core::mem::transmute::<
                RepeaterWithinItemTree<'id, 'sub_id>,
                RepeaterWithinItemTree<'id, 'static>,
            >(from)
        })
    }
}
impl<'id> ErasedRepeaterWithinComponent<'id> {
    pub fn unerase<'a, 'sub_id>(
        &'a self,
        _guard: generativity::Guard<'sub_id>,
    ) -> &'a RepeaterWithinItemTree<'id, 'sub_id> {
        // Safety: we just go from 'static to an unique lifetime
        unsafe {
            core::mem::transmute::<
                &'a RepeaterWithinItemTree<'id, 'static>,
                &'a RepeaterWithinItemTree<'id, 'sub_id>,
            >(&self.0)
        }
    }

    /// Return a repeater with a ItemTree with a 'static lifetime
    ///
    /// Safety: one should ensure that the inner ItemTree is not mixed with other inner ItemTree
    unsafe fn get_untagged(&self) -> &RepeaterWithinItemTree<'id, 'static> {
        &self.0
    }
}

type Callback = i_slint_core::Callback<[Value], Value>;

#[derive(Clone)]
pub struct ErasedItemTreeDescription(Rc<ItemTreeDescription<'static>>);
impl ErasedItemTreeDescription {
    pub fn unerase<'a, 'id>(
        &'a self,
        _guard: generativity::Guard<'id>,
    ) -> &'a Rc<ItemTreeDescription<'id>> {
        // Safety: we just go from 'static to an unique lifetime
        unsafe {
            core::mem::transmute::<
                &'a Rc<ItemTreeDescription<'static>>,
                &'a Rc<ItemTreeDescription<'id>>,
            >(&self.0)
        }
    }
}
impl<'id> From<Rc<ItemTreeDescription<'id>>> for ErasedItemTreeDescription {
    fn from(from: Rc<ItemTreeDescription<'id>>) -> Self {
        // Safety: We never access the ItemTreeDescription with the static lifetime, only after we unerase it
        Self(unsafe {
            core::mem::transmute::<Rc<ItemTreeDescription<'id>>, Rc<ItemTreeDescription<'static>>>(
                from,
            )
        })
    }
}

/// ItemTreeDescription is a representation of a ItemTree suitable for interpretation
///
/// It contains information about how to create and destroy the Component.
/// Its first member is the ItemTreeVTable for generated instance, since it is a `#[repr(C)]`
/// structure, it is valid to cast a pointer to the ItemTreeVTable back to a
/// ItemTreeDescription to access the extra field that are needed at runtime
#[repr(C)]
pub struct ItemTreeDescription<'id> {
    pub(crate) ct: ItemTreeVTable,
    /// INVARIANT: both dynamic_type and item_tree have the same lifetime id. Here it is erased to 'static
    dynamic_type: Rc<dynamic_type::TypeInfo<'id>>,
    item_tree: Vec<ItemTreeNode>,
    item_array:
        Vec<vtable::VOffset<crate::dynamic_type::Instance<'id>, ItemVTable, vtable::AllowPin>>,
    pub(crate) items: HashMap<SmolStr, ItemWithinItemTree>,
    pub(crate) custom_properties: HashMap<SmolStr, PropertiesWithinComponent>,
    pub(crate) custom_callbacks: HashMap<SmolStr, FieldOffset<Instance<'id>, Callback>>,
    repeater: Vec<ErasedRepeaterWithinComponent<'id>>,
    /// Map the Element::id of the repeater to the index in the `repeater` vec
    pub repeater_names: HashMap<SmolStr, usize>,
    /// Offset to a Option<ComponentPinRef>
    pub(crate) parent_item_tree_offset:
        Option<FieldOffset<Instance<'id>, OnceCell<ErasedItemTreeBoxWeak>>>,
    pub(crate) root_offset: FieldOffset<Instance<'id>, OnceCell<ErasedItemTreeBoxWeak>>,
    /// Offset to the window reference
    pub(crate) window_adapter_offset: FieldOffset<Instance<'id>, OnceCell<WindowAdapterRc>>,
    /// Offset of a ComponentExtraData
    pub(crate) extra_data_offset: FieldOffset<Instance<'id>, ComponentExtraData>,
    /// Keep the Rc alive
    pub(crate) original: Rc<object_tree::Component>,
    /// Maps from an item_id to the original element it came from
    pub(crate) original_elements: Vec<ElementRc>,
    /// Copy of original.root_element.property_declarations, without a guarded refcell
    public_properties: BTreeMap<SmolStr, PropertyDeclaration>,
    change_trackers: Option<(
        FieldOffset<Instance<'id>, OnceCell<Vec<ChangeTracker>>>,
        Vec<(NamedReference, Expression)>,
    )>,
    timers: Vec<FieldOffset<Instance<'id>, Timer>>,
    /// Map of element IDs to their active popup's ID
    popup_ids: std::cell::RefCell<HashMap<SmolStr, NonZeroU32>>,

    pub(crate) popup_menu_description: PopupMenuDescription,

    /// The collection of compiled globals
    compiled_globals: Option<Rc<CompiledGlobalCollection>>,

    /// The type loader, which will be available only on the top-most `ItemTreeDescription`.
    /// All other `ItemTreeDescription`s have `None` here.
    #[cfg(feature = "internal-highlight")]
    pub(crate) type_loader:
        std::cell::OnceCell<std::rc::Rc<i_slint_compiler::typeloader::TypeLoader>>,
    /// The type loader, which will be available only on the top-most `ItemTreeDescription`.
    /// All other `ItemTreeDescription`s have `None` here.
    #[cfg(feature = "internal-highlight")]
    pub(crate) raw_type_loader:
        std::cell::OnceCell<Option<std::rc::Rc<i_slint_compiler::typeloader::TypeLoader>>>,

    pub(crate) debug_handler: std::cell::RefCell<
        Rc<dyn Fn(Option<&i_slint_compiler::diagnostics::SourceLocation>, &str)>,
    >,
}

#[derive(Clone, derive_more::From)]
pub(crate) enum PopupMenuDescription {
    Rc(Rc<ErasedItemTreeDescription>),
    Weak(Weak<ErasedItemTreeDescription>),
}
impl PopupMenuDescription {
    pub fn unerase<'id>(&self, guard: generativity::Guard<'id>) -> Rc<ItemTreeDescription<'id>> {
        match self {
            PopupMenuDescription::Rc(rc) => rc.unerase(guard).clone(),
            PopupMenuDescription::Weak(weak) => weak.upgrade().unwrap().unerase(guard).clone(),
        }
    }
}

fn internal_properties_to_public<'a>(
    prop_iter: impl Iterator<Item = (&'a SmolStr, &'a PropertyDeclaration)> + 'a,
) -> impl Iterator<
    Item = (
        SmolStr,
        i_slint_compiler::langtype::Type,
        i_slint_compiler::object_tree::PropertyVisibility,
    ),
> + 'a {
    prop_iter.filter(|(_, v)| v.expose_in_public_api).map(|(s, v)| {
        let name = v
            .node
            .as_ref()
            .and_then(|n| {
                n.child_node(parser::SyntaxKind::DeclaredIdentifier)
                    .and_then(|n| n.child_token(parser::SyntaxKind::Identifier))
            })
            .map(|n| n.to_smolstr())
            .unwrap_or_else(|| s.to_smolstr());
        (name, v.property_type.clone(), v.visibility)
    })
}

<removed WindowOption>

impl ItemTreeDescription<'_> {
    /// The name of this Component as written in the .slint file
    pub fn id(&self) -> &str {
        self.original.id.as_str()
    }

    /// List of publicly declared properties or callbacks
    ///
    /// We try to preserve the dashes and underscore as written in the property declaration
    pub fn properties(
        &self,
    ) -> impl Iterator<
        Item = (
            SmolStr,
            i_slint_compiler::langtype::Type,
            i_slint_compiler::object_tree::PropertyVisibility,
        ),
    > + '_ {
        internal_properties_to_public(self.public_properties.iter())
    }

    /// List names of exported global singletons
    pub fn global_names(&self) -> impl Iterator<Item = SmolStr> + '_ {
        self.compiled_globals
            .as_ref()
            .expect("Root component should have globals")
            .compiled_globals
            .iter()
            .filter(|g| g.visible_in_public_api())
            .flat_map(|g| g.names().into_iter())
    }

    pub fn global_properties(
        &self,
        name: &str,
    ) -> Option<
        impl Iterator<
                Item = (
                    SmolStr,
                    i_slint_compiler::langtype::Type,
                    i_slint_compiler::object_tree::PropertyVisibility,
                ),
            > + '_,
    > {
        let g = self.compiled_globals.as_ref().expect("Root component should have globals");
        g.exported_globals_by_name
            .get(&crate::normalize_identifier(name))
            .and_then(|global_idx| g.compiled_globals.get(*global_idx))
            .map(|global| internal_properties_to_public(global.public_properties()))
    }

    <removed fn create>

    /// Set a value to property.
    ///
    /// Return an error if the property with this name does not exist,
    /// or if the value is the wrong type.
    /// Panics if the component is not an instance corresponding to this ItemTreeDescription,
    pub fn set_property(
        &self,
        component: ItemTreeRefPin,
        name: &str,
        value: Value,
    ) -> Result<(), crate::api::SetPropertyError> {
        if !core::ptr::eq((&self.ct) as *const _, component.get_vtable() as *const _) {
            panic!("mismatch instance and vtable");
        }
        generativity::make_guard!(guard);
        let c = unsafe { InstanceRef::from_pin_ref(component, guard) };
        if let Some(alias) = self
            .original
            .root_element
            .borrow()
            .property_declarations
            .get(name)
            .and_then(|d| d.is_alias.as_ref())
        {
            eval::store_property(c, &alias.element(), alias.name(), value)
        } else {
            eval::store_property(c, &self.original.root_element, name, value)
        }
    }

    /// Set a binding to a property
    ///
    /// Returns an error if the instance does not corresponds to this ItemTreeDescription,
    /// or if the property with this name does not exist in this component
    pub fn set_binding(
        &self,
        component: ItemTreeRefPin,
        name: &str,
        binding: Box<dyn Fn() -> Value>,
    ) -> Result<(), ()> {
        if !core::ptr::eq((&self.ct) as *const _, component.get_vtable() as *const _) {
            return Err(());
        }
        let x = self.custom_properties.get(name).ok_or(())?;
        unsafe {
            x.prop
                .set_binding(
                    Pin::new_unchecked(&*component.as_ptr().add(x.offset)),
                    binding,
                    i_slint_core::rtti::AnimatedBindingKind::NotAnimated,
                )
                .unwrap()
        };
        Ok(())
    }

    /// Return the value of a property
    ///
    /// Returns an error if the component is not an instance corresponding to this ItemTreeDescription,
    /// or if a callback with this name does not exist
    pub fn get_property(&self, component: ItemTreeRefPin, name: &str) -> Result<Value, ()> {
        if !core::ptr::eq((&self.ct) as *const _, component.get_vtable() as *const _) {
            return Err(());
        }
        generativity::make_guard!(guard);
        // Safety: we just verified that the component has the right vtable
        let c = unsafe { InstanceRef::from_pin_ref(component, guard) };
        if let Some(alias) = self
            .original
            .root_element
            .borrow()
            .property_declarations
            .get(name)
            .and_then(|d| d.is_alias.as_ref())
        {
            eval::load_property(c, &alias.element(), alias.name())
        } else {
            eval::load_property(c, &self.original.root_element, name)
        }
    }

    /// Sets an handler for a callback
    ///
    /// Returns an error if the component is not an instance corresponding to this ItemTreeDescription,
    /// or if the property with this name does not exist
    pub fn set_callback_handler(
        &self,
        component: Pin<ItemTreeRef>,
        name: &str,
        handler: CallbackHandler,
    ) -> Result<(), ()> {
        if !core::ptr::eq((&self.ct) as *const _, component.get_vtable() as *const _) {
            return Err(());
        }
        if let Some(alias) = self
            .original
            .root_element
            .borrow()
            .property_declarations
            .get(name)
            .and_then(|d| d.is_alias.as_ref())
        {
            generativity::make_guard!(guard);
            // Safety: we just verified that the component has the right vtable
            let c = unsafe { InstanceRef::from_pin_ref(component, guard) };
            let inst = eval::ComponentInstance::InstanceRef(c);
            eval::set_callback_handler(&inst, &alias.element(), alias.name(), handler)?
        } else {
            let x = self.custom_callbacks.get(name).ok_or(())?;
            let sig = x.apply(unsafe { &*(component.as_ptr() as *const dynamic_type::Instance) });
            sig.set_handler(handler);
        }
        Ok(())
    }

    /// Invoke the specified callback or function
    ///
    /// Returns an error if the component is not an instance corresponding to this ItemTreeDescription,
    /// or if the callback with this name does not exist in this component
    pub fn invoke(
        &self,
        component: ItemTreeRefPin,
        name: &SmolStr,
        args: &[Value],
    ) -> Result<Value, ()> {
        if !core::ptr::eq((&self.ct) as *const _, component.get_vtable() as *const _) {
            return Err(());
        }
        generativity::make_guard!(guard);
        // Safety: we just verified that the component has the right vtable
        let c = unsafe { InstanceRef::from_pin_ref(component, guard) };
        let borrow = self.original.root_element.borrow();
        let decl = borrow.property_declarations.get(name).ok_or(())?;

        let (elem, name) = if let Some(alias) = &decl.is_alias {
            (alias.element(), alias.name())
        } else {
            (self.original.root_element.clone(), name)
        };

        let inst = eval::ComponentInstance::InstanceRef(c);

        if matches!(&decl.property_type, Type::Function { .. }) {
            eval::call_function(&inst, &elem, name, args.to_vec()).ok_or(())
        } else {
            eval::invoke_callback(&inst, &elem, name, args).ok_or(())
        }
    }

    // Return the global with the given name
    pub fn get_global(
        &self,
        component: ItemTreeRefPin,
        global_name: &str,
    ) -> Result<Pin<Rc<dyn crate::global_component::GlobalComponent>>, ()> {
        if !core::ptr::eq((&self.ct) as *const _, component.get_vtable() as *const _) {
            return Err(());
        }
        generativity::make_guard!(guard);
        // Safety: we just verified that the component has the right vtable
        let c = unsafe { InstanceRef::from_pin_ref(component, guard) };
        let extra_data = c.description.extra_data_offset.apply(c.instance.get_ref());
        let g = extra_data.globals.get().unwrap().get(global_name).clone();
        g.ok_or(())
    }

    pub fn recursively_set_debug_handler(
        &self,
        handler: Rc<dyn Fn(Option<&i_slint_compiler::diagnostics::SourceLocation>, &str)>,
    ) {
        *self.debug_handler.borrow_mut() = handler.clone();

        for r in &self.repeater {
            generativity::make_guard!(guard);
            r.unerase(guard).item_tree_to_repeat.recursively_set_debug_handler(handler.clone());
        }
    }
}

#[cfg_attr(not(feature = "ffi"), i_slint_core_macros::remove_extern)]
extern "C" fn visit_children_item(
    component: ItemTreeRefPin,
    index: isize,
    order: TraversalOrder,
    v: ItemVisitorRefMut,
) -> VisitChildrenResult {
    generativity::make_guard!(guard);
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };
    let comp_rc = instance_ref.self_weak().get().unwrap().upgrade().unwrap();
    i_slint_core::item_tree::visit_item_tree(
        instance_ref.instance,
        &vtable::VRc::into_dyn(comp_rc),
        get_item_tree(component).as_slice(),
        index,
        order,
        v,
        |_, order, visitor, index| {
            if index as usize >= instance_ref.description.repeater.len() {
                // Do nothing: We are ComponentContainer and Our parent already did all the work!
                VisitChildrenResult::CONTINUE
            } else {
                // `ensure_updated` needs a 'static lifetime so we must call get_untagged.
                // Safety: we do not mix the component with other component id in this function
                let rep_in_comp =
                    unsafe { instance_ref.description.repeater[index as usize].get_untagged() };
                ensure_repeater_updated(instance_ref, rep_in_comp);
                let repeater = rep_in_comp.offset.apply_pin(instance_ref.instance);
                repeater.visit(order, visitor)
            }
        },
    )
}

/// Make sure that the repeater is updated
fn ensure_repeater_updated<'id>(
    instance_ref: InstanceRef<'_, 'id>,
    rep_in_comp: &RepeaterWithinItemTree<'id, '_>,
) {
    let repeater = rep_in_comp.offset.apply_pin(instance_ref.instance);
    let init = || {
        let instance = instantiate(
            rep_in_comp.item_tree_to_repeat.clone(),
            instance_ref.self_weak().get().cloned(),
            None,
            None,
            Default::default(),
        );
        instance
    };
    if let Some(lv) = &rep_in_comp
        .item_tree_to_repeat
        .original
        .parent_element
        .upgrade()
        .unwrap()
        .borrow()
        .repeated
        .as_ref()
        .unwrap()
        .is_listview
    {
        let assume_property_logical_length =
            |prop| unsafe { Pin::new_unchecked(&*(prop as *const Property<LogicalLength>)) };
        let get_prop = |nr: &NamedReference| -> LogicalLength {
            eval::load_property(instance_ref, &nr.element(), nr.name()).unwrap().try_into().unwrap()
        };
        repeater.ensure_updated_listview(
            init,
            assume_property_logical_length(get_property_ptr(&lv.viewport_width, instance_ref)),
            assume_property_logical_length(get_property_ptr(&lv.viewport_height, instance_ref)),
            assume_property_logical_length(get_property_ptr(&lv.viewport_y, instance_ref)),
            get_prop(&lv.listview_width),
            assume_property_logical_length(get_property_ptr(&lv.listview_height, instance_ref)),
        );
    } else {
        repeater.ensure_updated(init);
    }
}

/// Information attached to a builtin item
pub(crate) struct ItemRTTI {
    vtable: &'static ItemVTable,
    type_info: dynamic_type::StaticTypeInfo,
    pub(crate) properties: HashMap<&'static str, Box<dyn eval::ErasedPropertyInfo>>,
    pub(crate) callbacks: HashMap<&'static str, Box<dyn eval::ErasedCallbackInfo>>,
}

fn rtti_for<T: 'static + Default + rtti::BuiltinItem + vtable::HasStaticVTable<ItemVTable>>(
) -> (&'static str, Rc<ItemRTTI>) {
    let rtti = ItemRTTI {
        vtable: T::static_vtable(),
        type_info: dynamic_type::StaticTypeInfo::new::<T>(),
        properties: T::properties()
            .into_iter()
            .map(|(k, v)| (k, Box::new(v) as Box<dyn eval::ErasedPropertyInfo>))
            .collect(),
        callbacks: T::callbacks()
            .into_iter()
            .map(|(k, v)| (k, Box::new(v) as Box<dyn eval::ErasedCallbackInfo>))
            .collect(),
    };
    (T::name(), Rc::new(rtti))
}

<removed fn load>

fn generate_rtti() -> HashMap<&'static str, Rc<ItemRTTI>> {
    let mut rtti = HashMap::new();
    use i_slint_core::items::*;
    rtti.extend(
        [
            rtti_for::<ComponentContainer>(),
            rtti_for::<Empty>(),
            rtti_for::<ImageItem>(),
            rtti_for::<ClippedImage>(),
            rtti_for::<ComplexText>(),
            rtti_for::<MarkdownText>(),
            rtti_for::<SimpleText>(),
            rtti_for::<Rectangle>(),
            rtti_for::<BasicBorderRectangle>(),
            rtti_for::<BorderRectangle>(),
            rtti_for::<TouchArea>(),
            rtti_for::<FocusScope>(),
            rtti_for::<SwipeGestureHandler>(),
            rtti_for::<Path>(),
            rtti_for::<Flickable>(),
            rtti_for::<WindowItem>(),
            rtti_for::<TextInput>(),
            rtti_for::<Clip>(),
            rtti_for::<BoxShadow>(),
            rtti_for::<Transform>(),
            rtti_for::<Opacity>(),
            rtti_for::<Layer>(),
            rtti_for::<DragArea>(),
            rtti_for::<DropArea>(),
            rtti_for::<ContextMenu>(),
            rtti_for::<MenuItem>(),
        ]
        .iter()
        .cloned(),
    );

    trait NativeHelper {
        fn push(rtti: &mut HashMap<&str, Rc<ItemRTTI>>);
    }
    impl NativeHelper for () {
        fn push(_rtti: &mut HashMap<&str, Rc<ItemRTTI>>) {}
    }
    impl<
            T: 'static + Default + rtti::BuiltinItem + vtable::HasStaticVTable<ItemVTable>,
            Next: NativeHelper,
        > NativeHelper for (T, Next)
    {
        fn push(rtti: &mut HashMap<&str, Rc<ItemRTTI>>) {
            let info = rtti_for::<T>();
            rtti.insert(info.0, info.1);
            Next::push(rtti);
        }
    }
    i_slint_backend_selector::NativeWidgets::push(&mut rtti);

    rtti
}

pub(crate) fn generate_item_tree<'id>(
    component: &Rc<object_tree::Component>,
    compiled_globals: Option<Rc<CompiledGlobalCollection>>,
    popup_menu_description: PopupMenuDescription,
    is_popup_menu_impl: bool,
    guard: generativity::Guard<'id>,
) -> Rc<ItemTreeDescription<'id>> {
    //dbg!(&*component.root_element.borrow());

    thread_local! {
        static RTTI: Lazy<HashMap<&'static str, Rc<ItemRTTI>>> = Lazy::new(generate_rtti);
    }

    struct TreeBuilder<'id> {
        tree_array: Vec<ItemTreeNode>,
        item_array:
            Vec<vtable::VOffset<crate::dynamic_type::Instance<'id>, ItemVTable, vtable::AllowPin>>,
        original_elements: Vec<ElementRc>,
        items_types: HashMap<SmolStr, ItemWithinItemTree>,
        type_builder: dynamic_type::TypeBuilder<'id>,
        repeater: Vec<ErasedRepeaterWithinComponent<'id>>,
        repeater_names: HashMap<SmolStr, usize>,
        change_callbacks: Vec<(NamedReference, Expression)>,
        popup_menu_description: PopupMenuDescription,
    }
    impl generator::ItemTreeBuilder for TreeBuilder<'_> {
        type SubComponentState = ();

        fn push_repeated_item(
            &mut self,
            item_rc: &ElementRc,
            repeater_count: u32,
            parent_index: u32,
            _component_state: &Self::SubComponentState,
        ) {
            self.tree_array.push(ItemTreeNode::DynamicTree { index: repeater_count, parent_index });
            self.original_elements.push(item_rc.clone());
            let item = item_rc.borrow();
            let base_component = item.base_type.as_component();
            self.repeater_names.insert(item.id.clone(), self.repeater.len());
            generativity::make_guard!(guard);
            let repeated_element_info = item.repeated.as_ref().unwrap();
            self.repeater.push(
                RepeaterWithinItemTree {
                    item_tree_to_repeat: generate_item_tree(
                        base_component,
                        None,
                        self.popup_menu_description.clone(),
                        false,
                        guard,
                    ),
                    offset: self.type_builder.add_field_type::<Repeater<ErasedItemTreeBox>>(),
                    model: repeated_element_info.model.clone(),
                    is_conditional: repeated_element_info.is_conditional_element,
                }
                .into(),
            );
        }

        fn push_native_item(
            &mut self,
            rc_item: &ElementRc,
            child_offset: u32,
            parent_index: u32,
            _component_state: &Self::SubComponentState,
        ) {
            let item = rc_item.borrow();
            let rt = RTTI.with(|rtti| {
                rtti.get(&*item.base_type.as_native().class_name)
                    .unwrap_or_else(|| {
                        panic!(
                            "Native type not registered: {}",
                            item.base_type.as_native().class_name
                        )
                    })
                    .clone()
            });

            let offset = self.type_builder.add_field(rt.type_info);

            self.tree_array.push(ItemTreeNode::Item {
                is_accessible: !item.accessibility_props.0.is_empty(),
                children_index: child_offset,
                children_count: item.children.len() as u32,
                parent_index,
                item_array_index: self.item_array.len() as u32,
            });
            self.item_array.push(unsafe { vtable::VOffset::from_raw(rt.vtable, offset) });
            self.original_elements.push(rc_item.clone());
            debug_assert_eq!(self.original_elements.len(), self.tree_array.len());
            self.items_types.insert(
                item.id.clone(),
                ItemWithinItemTree { offset, rtti: rt, elem: rc_item.clone() },
            );
            for (prop, expr) in &item.change_callbacks {
                self.change_callbacks.push((
                    NamedReference::new(rc_item, prop.clone()),
                    Expression::CodeBlock(expr.borrow().clone()),
                ));
            }
        }

        fn enter_component(
            &mut self,
            _item: &ElementRc,
            _sub_component: &Rc<object_tree::Component>,
            _children_offset: u32,
            _component_state: &Self::SubComponentState,
        ) -> Self::SubComponentState {
            /* nothing to do */
        }

        fn enter_component_children(
            &mut self,
            _item: &ElementRc,
            _repeater_count: u32,
            _component_state: &Self::SubComponentState,
            _sub_component_state: &Self::SubComponentState,
        ) {
            todo!()
        }
    }

    let mut builder = TreeBuilder {
        tree_array: vec![],
        item_array: vec![],
        original_elements: vec![],
        items_types: HashMap::new(),
        type_builder: dynamic_type::TypeBuilder::new(guard),
        repeater: vec![],
        repeater_names: HashMap::new(),
        change_callbacks: vec![],
        popup_menu_description,
    };

    if !component.is_global() {
        generator::build_item_tree(component, &(), &mut builder);
    } else {
        for (prop, expr) in component.root_element.borrow().change_callbacks.iter() {
            builder.change_callbacks.push((
                NamedReference::new(&component.root_element, prop.clone()),
                Expression::CodeBlock(expr.borrow().clone()),
            ));
        }
    }

    let mut custom_properties = HashMap::new();
    let mut custom_callbacks = HashMap::new();
    fn property_info<T>() -> (Box<dyn PropertyInfo<u8, Value>>, dynamic_type::StaticTypeInfo)
    where
        T: PartialEq + Clone + Default + std::convert::TryInto<Value> + 'static,
        Value: std::convert::TryInto<T>,
    {
        // Fixme: using u8 in PropertyInfo<> is not sound, we would need to materialize a type for out component
        (
            Box::new(unsafe {
                vtable::FieldOffset::<u8, Property<T>, _>::new_from_offset_pinned(0)
            }),
            dynamic_type::StaticTypeInfo::new::<Property<T>>(),
        )
    }
    fn animated_property_info<T>(
    ) -> (Box<dyn PropertyInfo<u8, Value>>, dynamic_type::StaticTypeInfo)
    where
        T: Clone + Default + InterpolatedPropertyValue + std::convert::TryInto<Value> + 'static,
        Value: std::convert::TryInto<T>,
    {
        // Fixme: using u8 in PropertyInfo<> is not sound, we would need to materialize a type for out component
        (
            Box::new(unsafe {
                rtti::MaybeAnimatedPropertyInfoWrapper(
                    vtable::FieldOffset::<u8, Property<T>, _>::new_from_offset_pinned(0),
                )
            }),
            dynamic_type::StaticTypeInfo::new::<Property<T>>(),
        )
    }

    fn property_info_for_type(
        ty: &Type,
        name: &str,
    ) -> Option<(Box<dyn PropertyInfo<u8, Value>>, dynamic_type::StaticTypeInfo)> {
        Some(match ty {
            Type::Float32 => animated_property_info::<f32>(),
            Type::Int32 => animated_property_info::<i32>(),
            Type::String => property_info::<SharedString>(),
            Type::Color => animated_property_info::<Color>(),
            Type::Brush => animated_property_info::<Brush>(),
            Type::Duration => animated_property_info::<i64>(),
            Type::Angle => animated_property_info::<f32>(),
            Type::PhysicalLength => animated_property_info::<f32>(),
            Type::LogicalLength => animated_property_info::<f32>(),
            Type::Rem => animated_property_info::<f32>(),
            Type::Image => property_info::<i_slint_core::graphics::Image>(),
            Type::Bool => property_info::<bool>(),
            Type::ComponentFactory => property_info::<ComponentFactory>(),
            Type::Struct(s)
                if matches!(
                    s.name,
                    StructName::BuiltinPrivate(BuiltinPrivateStruct::StateInfo)
                ) =>
            {
                property_info::<i_slint_core::properties::StateInfo>()
            }
            Type::Struct(_) => property_info::<Value>(),
            Type::Array(_) => property_info::<Value>(),
            Type::Easing => property_info::<i_slint_core::animations::EasingCurve>(),
            Type::Percent => animated_property_info::<f32>(),
            Type::Enumeration(e) => {
                macro_rules! match_enum_type {
                    ($( $(#[$enum_doc:meta])* enum $Name:ident { $($body:tt)* })*) => {
                        match e.name.as_str() {
                            $(
                                stringify!($Name) => property_info::<i_slint_core::items::$Name>(),
                            )*
                            x => unreachable!("Unknown non-builtin enum {x}"),
                        }
                    }
                }
                if e.node.is_some() {
                    property_info::<Value>()
                } else {
                    i_slint_common::for_each_enums!(match_enum_type)
                }
            }
            Type::LayoutCache => property_info::<SharedVector<f32>>(),
            Type::Function { .. } | Type::Callback { .. } => return None,

            // These can't be used in properties
            Type::Invalid
            | Type::Void
            | Type::InferredProperty
            | Type::InferredCallback
            | Type::Model
            | Type::PathData
            | Type::UnitProduct(_)
            | Type::ElementReference => panic!("bad type {ty:?} for property {name}"),
        })
    }

    for (name, decl) in &component.root_element.borrow().property_declarations {
        if decl.is_alias.is_some() {
            continue;
        }
        if matches!(&decl.property_type, Type::Callback { .. }) {
            custom_callbacks
                .insert(name.clone(), builder.type_builder.add_field_type::<Callback>());
            continue;
        }
        let Some((prop, type_info)) = property_info_for_type(&decl.property_type, &name) else {
            continue;
        };
        custom_properties.insert(
            name.clone(),
            PropertiesWithinComponent { offset: builder.type_builder.add_field(type_info), prop },
        );
    }
    if let Some(parent_element) = component.parent_element.upgrade() {
        if let Some(r) = &parent_element.borrow().repeated {
            if !r.is_conditional_element {
                let (prop, type_info) = property_info::<u32>();
                custom_properties.insert(
                    SPECIAL_PROPERTY_INDEX.into(),
                    PropertiesWithinComponent {
                        offset: builder.type_builder.add_field(type_info),
                        prop,
                    },
                );

                let model_ty = Expression::RepeaterModelReference {
                    element: component.parent_element.clone(),
                }
                .ty();
                let (prop, type_info) =
                    property_info_for_type(&model_ty, &SPECIAL_PROPERTY_MODEL_DATA).unwrap();
                custom_properties.insert(
                    SPECIAL_PROPERTY_MODEL_DATA.into(),
                    PropertiesWithinComponent {
                        offset: builder.type_builder.add_field(type_info),
                        prop,
                    },
                );
            }
        }
    }

    let parent_item_tree_offset =
        if component.parent_element.upgrade().is_some() || is_popup_menu_impl {
            Some(builder.type_builder.add_field_type::<OnceCell<ErasedItemTreeBoxWeak>>())
        } else {
            None
        };

    let root_offset = builder.type_builder.add_field_type::<OnceCell<ErasedItemTreeBoxWeak>>();

    let window_adapter_offset = builder.type_builder.add_field_type::<OnceCell<WindowAdapterRc>>();

    let extra_data_offset = builder.type_builder.add_field_type::<ComponentExtraData>();

    let change_trackers = (!builder.change_callbacks.is_empty()).then(|| {
        (
            builder.type_builder.add_field_type::<OnceCell<Vec<ChangeTracker>>>(),
            builder.change_callbacks,
        )
    });
    let timers = component
        .timers
        .borrow()
        .iter()
        .map(|_| builder.type_builder.add_field_type::<Timer>())
        .collect();

    // only the public exported component needs the public property list
    let public_properties = if component.parent_element.upgrade().is_none() {
        component.root_element.borrow().property_declarations.clone()
    } else {
        Default::default()
    };

    let t = ItemTreeVTable {
        visit_children_item,
        layout_info,
        get_item_ref,
        get_item_tree,
        get_subtree_range,
        get_subtree,
        parent_node,
        embed_component,
        subtree_index,
        item_geometry,
        accessible_role,
        accessible_string_property,
        accessibility_action,
        supported_accessibility_actions,
        item_element_infos,
        window_adapter,
        drop_in_place,
        dealloc,
    };
    let t = ItemTreeDescription {
        ct: t,
        dynamic_type: builder.type_builder.build(),
        item_tree: builder.tree_array,
        item_array: builder.item_array,
        items: builder.items_types,
        custom_properties,
        custom_callbacks,
        original: component.clone(),
        original_elements: builder.original_elements,
        repeater: builder.repeater,
        repeater_names: builder.repeater_names,
        parent_item_tree_offset,
        root_offset,
        window_adapter_offset,
        extra_data_offset,
        public_properties,
        compiled_globals,
        change_trackers,
        timers,
        popup_ids: std::cell::RefCell::new(HashMap::new()),
        popup_menu_description: builder.popup_menu_description,
        #[cfg(feature = "internal-highlight")]
        type_loader: std::cell::OnceCell::new(),
        #[cfg(feature = "internal-highlight")]
        raw_type_loader: std::cell::OnceCell::new(),
        debug_handler: std::cell::RefCell::new(Rc::new(|_, text| {
            i_slint_core::debug_log!("{text}")
        })),
    };

    Rc::new(t)
}

pub fn animation_for_property(
    component: InstanceRef,
    animation: &Option<i_slint_compiler::object_tree::PropertyAnimation>,
) -> AnimatedBindingKind {
    match animation {
        Some(i_slint_compiler::object_tree::PropertyAnimation::Static(anim_elem)) => {
            AnimatedBindingKind::Animation(eval::new_struct_with_bindings(
                &anim_elem.borrow().bindings,
                &mut eval::EvalLocalContext::from_component_instance(component),
            ))
        }
        Some(i_slint_compiler::object_tree::PropertyAnimation::Transition {
            animations,
            state_ref,
        }) => {
            let component_ptr = component.as_ptr();
            let vtable = NonNull::from(&component.description.ct).cast();
            let animations = animations.clone();
            let state_ref = state_ref.clone();
            AnimatedBindingKind::Transition(Box::new(
                move || -> (PropertyAnimation, i_slint_core::animations::Instant) {
                    generativity::make_guard!(guard);
                    let component = unsafe {
                        InstanceRef::from_pin_ref(
                            Pin::new_unchecked(vtable::VRef::from_raw(
                                vtable,
                                NonNull::new_unchecked(component_ptr as *mut u8),
                            )),
                            guard,
                        )
                    };

                    let mut context = eval::EvalLocalContext::from_component_instance(component);
                    let state = eval::eval_expression(&state_ref, &mut context);
                    let state_info: i_slint_core::properties::StateInfo = state.try_into().unwrap();
                    for a in &animations {
                        let is_previous_state = a.state_id == state_info.previous_state;
                        let is_current_state = a.state_id == state_info.current_state;
                        match (a.direction, is_previous_state, is_current_state) {
                            (TransitionDirection::In, false, true)
                            | (TransitionDirection::Out, true, false)
                            | (TransitionDirection::InOut, false, true)
                            | (TransitionDirection::InOut, true, false) => {
                                return (
                                    eval::new_struct_with_bindings(
                                        &a.animation.borrow().bindings,
                                        &mut context,
                                    ),
                                    state_info.change_time,
                                );
                            }
                            _ => {}
                        }
                    }
                    Default::default()
                },
            ))
        }
        None => AnimatedBindingKind::NotAnimated,
    }
}

fn make_callback_eval_closure(
    expr: Expression,
    self_weak: ErasedItemTreeBoxWeak,
) -> impl Fn(&[Value]) -> Value {
    move |args| {
        let self_rc = self_weak.upgrade().unwrap();
        generativity::make_guard!(guard);
        let self_ = self_rc.unerase(guard);
        let instance_ref = self_.borrow_instance();
        let mut local_context =
            eval::EvalLocalContext::from_function_arguments(instance_ref, args.to_vec());
        eval::eval_expression(&expr, &mut local_context)
    }
}

fn make_binding_eval_closure(
    expr: Expression,
    self_weak: ErasedItemTreeBoxWeak,
) -> impl Fn() -> Value {
    move || {
        let self_rc = self_weak.upgrade().unwrap();
        generativity::make_guard!(guard);
        let self_ = self_rc.unerase(guard);
        let instance_ref = self_.borrow_instance();
        eval::eval_expression(
            &expr,
            &mut eval::EvalLocalContext::from_component_instance(instance_ref),
        )
    }
}

pub fn instantiate(
    description: Rc<ItemTreeDescription>,
    parent_ctx: Option<ErasedItemTreeBoxWeak>,
    root: Option<ErasedItemTreeBoxWeak>,
    window_options: Option<&WindowOptions>,
    mut globals: crate::global_component::GlobalStorage,
) -> DynamicComponentVRc {
    let instance = description.dynamic_type.clone().create_instance();

    let component_box = ItemTreeBox { instance, description: description.clone() };

    let self_rc = vtable::VRc::new(ErasedItemTreeBox::from(component_box));
    let self_weak = vtable::VRc::downgrade(&self_rc);

    generativity::make_guard!(guard);
    let comp = self_rc.unerase(guard);
    let instance_ref = comp.borrow_instance();
    instance_ref.self_weak().set(self_weak.clone()).ok();
    let description = comp.description();

    if let Some(parent) = parent_ctx {
        description
            .parent_item_tree_offset
            .unwrap()
            .apply(instance_ref.as_ref())
            .set(parent)
            .ok()
            .unwrap();
    } else {
        if let Some(g) = description.compiled_globals.as_ref() {
            for g in g.compiled_globals.iter() {
                crate::global_component::instantiate(g, &mut globals, self_weak.clone());
            }
        }
        let extra_data = description.extra_data_offset.apply(instance_ref.as_ref());
        extra_data.globals.set(globals).ok().unwrap();
    }

    if let Some(WindowOptions::Embed { parent_item_tree, parent_item_tree_index }) = window_options
    {
        vtable::VRc::borrow_pin(&self_rc)
            .as_ref()
            .embed_component(parent_item_tree, *parent_item_tree_index);
        description.root_offset.apply(instance_ref.as_ref()).set(self_weak.clone()).ok().unwrap();
    } else {
        generativity::make_guard!(guard);
        let root = root
            .or_else(|| {
                instance_ref.parent_instance(guard).map(|parent| parent.root_weak().clone())
            })
            .unwrap_or_else(|| self_weak.clone());
        description.root_offset.apply(instance_ref.as_ref()).set(root).ok().unwrap();
    }

    if !description.original.is_global() {
        let maybe_window_adapter =
            if let Some(WindowOptions::UseExistingWindow(adapter)) = window_options.as_ref() {
                Some(adapter.clone())
            } else {
                instance_ref.maybe_window_adapter()
            };

        let component_rc = vtable::VRc::into_dyn(self_rc.clone());
        i_slint_core::item_tree::register_item_tree(&component_rc, maybe_window_adapter);
    }

    if let Some(WindowOptions::UseExistingWindow(existing_adapter)) = &window_options {
        description
            .window_adapter_offset
            .apply(instance_ref.as_ref())
            .set(existing_adapter.clone())
            .ok()
            .unwrap();
    }

    // Some properties are generated as Value, but for which the default constructed Value must be initialized
    for (prop_name, decl) in &description.original.root_element.borrow().property_declarations {
        if !matches!(
            decl.property_type,
            Type::Struct { .. } | Type::Array(_) | Type::Enumeration(_)
        ) || decl.is_alias.is_some()
        {
            continue;
        }
        if let Some(b) = description.original.root_element.borrow().bindings.get(prop_name) {
            if b.borrow().two_way_bindings.is_empty() {
                continue;
            }
        }
        let p = description.custom_properties.get(prop_name).unwrap();
        unsafe {
            let item = Pin::new_unchecked(&*instance_ref.as_ptr().add(p.offset));
            p.prop.set(item, eval::default_value_for_type(&decl.property_type), None).unwrap();
        }
    }

    generator::handle_property_bindings_init(
        &description.original,
        |elem, prop_name, binding| unsafe {
            let is_root = Rc::ptr_eq(
                elem,
                &elem.borrow().enclosing_component.upgrade().unwrap().root_element,
            );
            let elem = elem.borrow();
            let is_const = binding.analysis.as_ref().is_some_and(|a| a.is_const);

            let property_type = elem.lookup_property(prop_name).property_type;
            if let Type::Function { .. } = property_type {
                // function don't need initialization
            } else if let Type::Callback { .. } = property_type {
                if !matches!(binding.expression, Expression::Invalid) {
                    let expr = binding.expression.clone();
                    let description = description.clone();
                    if let Some(callback_offset) =
                        description.custom_callbacks.get(prop_name).filter(|_| is_root)
                    {
                        let callback = callback_offset.apply(instance_ref.as_ref());
                        callback.set_handler(make_callback_eval_closure(expr, self_weak.clone()));
                    } else {
                        let item_within_component = &description.items[&elem.id];
                        let item = item_within_component.item_from_item_tree(instance_ref.as_ptr());
                        if let Some(callback) =
                            item_within_component.rtti.callbacks.get(prop_name.as_str())
                        {
                            callback.set_handler(
                                item,
                                Box::new(make_callback_eval_closure(expr, self_weak.clone())),
                            );
                        } else {
                            panic!("unknown callback {prop_name}")
                        }
                    }
                }
            } else if let Some(PropertiesWithinComponent { offset, prop: prop_info, .. }) =
                description.custom_properties.get(prop_name).filter(|_| is_root)
            {
                let is_state_info = matches!(&property_type, Type::Struct (s) if matches!(s.name, StructName::BuiltinPrivate(BuiltinPrivateStruct::StateInfo)));
                if is_state_info {
                    let prop = Pin::new_unchecked(
                        &*(instance_ref.as_ptr().add(*offset)
                            as *const Property<i_slint_core::properties::StateInfo>),
                    );
                    let e = binding.expression.clone();
                    let state_binding = make_binding_eval_closure(e, self_weak.clone());
                    i_slint_core::properties::set_state_binding(prop, move || {
                        state_binding().try_into().unwrap()
                    });
                    return;
                }

                let maybe_animation = animation_for_property(instance_ref, &binding.animation);
                let item = Pin::new_unchecked(&*instance_ref.as_ptr().add(*offset));

                if !matches!(binding.expression, Expression::Invalid) {
                    if is_const {
                        let v = eval::eval_expression(
                            &binding.expression,
                            &mut eval::EvalLocalContext::from_component_instance(instance_ref),
                        );
                        prop_info.set(item, v, None).unwrap();
                    } else {
                        let e = binding.expression.clone();
                        prop_info
                            .set_binding(
                                item,
                                Box::new(make_binding_eval_closure(e, self_weak.clone())),
                                maybe_animation,
                            )
                            .unwrap();
                    }
                }
                for twb in &binding.two_way_bindings {
                    if twb.field_access.is_empty()
                        && !matches!(&property_type, Type::Struct(..) | Type::Array(..))
                    {
                        // Safety: The compiler must have ensured that the properties exist and are of the same type
                        // (Except for struct and array, which may map to a Value)
                        prop_info
                            .link_two_ways(item, get_property_ptr(&twb.property, instance_ref));
                    } else {
                        let (common, map) = prepare_for_two_way_binding(instance_ref, twb);
                        prop_info.link_two_way_with_map(item, common, map);
                    }
                }
            } else {
                let item_within_component = &description.items[&elem.id];
                let item = item_within_component.item_from_item_tree(instance_ref.as_ptr());
                if let Some(prop_rtti) =
                    item_within_component.rtti.properties.get(prop_name.as_str())
                {
                    let maybe_animation = animation_for_property(instance_ref, &binding.animation);

                    for twb in &binding.two_way_bindings {
                        if twb.field_access.is_empty()
                            && !matches!(&property_type, Type::Struct(..) | Type::Array(..))
                        {
                            // Safety: The compiler must have ensured that the properties exist and are of the same type
                            prop_rtti
                                .link_two_ways(item, get_property_ptr(&twb.property, instance_ref));
                        } else {
                            let (common, map) = prepare_for_two_way_binding(instance_ref, twb);
                            prop_rtti.link_two_way_with_map(item, common, map);
                        }
                    }
                    if !matches!(binding.expression, Expression::Invalid) {
                        if is_const {
                            prop_rtti
                                .set(
                                    item,
                                    eval::eval_expression(
                                        &binding.expression,
                                        &mut eval::EvalLocalContext::from_component_instance(
                                            instance_ref,
                                        ),
                                    ),
                                    maybe_animation.as_animation(),
                                )
                                .unwrap();
                        } else {
                            let e = binding.expression.clone();
                            prop_rtti.set_binding(
                                item,
                                Box::new(make_binding_eval_closure(e, self_weak.clone())),
                                maybe_animation,
                            );
                        }
                    }
                } else {
                    panic!("unknown property {} in {}", prop_name, elem.id);
                }
            }
        },
    );

    for rep_in_comp in &description.repeater {
        generativity::make_guard!(guard);
        let rep_in_comp = rep_in_comp.unerase(guard);

        let repeater = rep_in_comp.offset.apply_pin(instance_ref.instance);
        let expr = rep_in_comp.model.clone();
        let model_binding_closure = make_binding_eval_closure(expr, self_weak.clone());
        if rep_in_comp.is_conditional {
            let bool_model = Rc::new(crate::value_model::BoolModel::default());
            repeater.set_model_binding(move || {
                let v = model_binding_closure();
                bool_model.set_value(v.try_into().expect("condition model is bool"));
                ModelRc::from(bool_model.clone())
            });
        } else {
            repeater.set_model_binding(move || {
                let m = model_binding_closure();
                if let Value::Model(m) = m {
                    m
                } else {
                    ModelRc::new(crate::value_model::ValueModel::new(m))
                }
            });
        }
    }
    self_rc
}

fn prepare_for_two_way_binding(
    instance_ref: InstanceRef,
    twb: &i_slint_compiler::expression_tree::TwoWayBinding,
) -> (Pin<Rc<Property<Value>>>, Option<Rc<dyn rtti::TwoWayBindingMapping<Value>>>) {
    let element = twb.property.element();
    let name = twb.property.name();
    generativity::make_guard!(guard);
    let enclosing_component = eval::enclosing_component_instance_for_element(
        &element,
        &eval::ComponentInstance::InstanceRef(instance_ref),
        guard,
    );
    let map: Option<Rc<dyn rtti::TwoWayBindingMapping<Value>>> = if twb.field_access.is_empty() {
        None
    } else {
        struct FieldAccess(Vec<SmolStr>);
        impl rtti::TwoWayBindingMapping<Value> for FieldAccess {
            fn map_to(&self, value: &Value) -> Value {
                let mut value = value.clone();
                for f in &self.0 {
                    match value {
                        Value::Struct(o) => value = o.get_field(f).cloned().unwrap_or_default(),
                        Value::Void => return Value::Void,
                        _ => panic!("Cannot map to a field of a non-struct {value:?}  - {f}"),
                    }
                }
                value
            }
            fn map_from(&self, mut value: &mut Value, from: &Value) {
                for f in &self.0 {
                    match value {
                        Value::Struct(o) => {
                            value = o.0.get_mut(f).expect("field not found while mapping")
                        }
                        _ => panic!("Cannot map to a field of a non-struct {value:?}"),
                    }
                }
                *value = from.clone();
            }
        }
        Some(Rc::new(FieldAccess(twb.field_access.clone())))
    };
    let common = match enclosing_component {
        eval::ComponentInstance::InstanceRef(enclosing_component) => {
            let element = element.borrow();
            if element.id == element.enclosing_component.upgrade().unwrap().root_element.borrow().id
            {
                if let Some(x) = enclosing_component.description.custom_properties.get(name) {
                    let item = unsafe { Pin::new_unchecked(&*instance_ref.as_ptr().add(x.offset)) };
                    let common = x.prop.prepare_for_two_way_binding(item);
                    return (common, map);
                };
            };
            let item_info = enclosing_component
                .description
                .items
                .get(element.id.as_str())
                .unwrap_or_else(|| panic!("Unknown element for {}.{}", element.id, name));
            let prop_info = item_info
                .rtti
                .properties
                .get(name.as_str())
                .unwrap_or_else(|| panic!("Property {} not in {}", name, element.id));
            core::mem::drop(element);
            let item = unsafe { item_info.item_from_item_tree(enclosing_component.as_ptr()) };
            prop_info.prepare_for_two_way_binding(item)
        }
        eval::ComponentInstance::GlobalComponent(glob) => {
            glob.as_ref().prepare_for_two_way_binding(name).unwrap()
        }
    };
    (common, map)
}

pub(crate) fn get_property_ptr(nr: &NamedReference, instance: InstanceRef) -> *const () {
    let element = nr.element();
    generativity::make_guard!(guard);
    let enclosing_component = eval::enclosing_component_instance_for_element(
        &element,
        &eval::ComponentInstance::InstanceRef(instance),
        guard,
    );
    match enclosing_component {
        eval::ComponentInstance::InstanceRef(enclosing_component) => {
            let element = element.borrow();
            if element.id == element.enclosing_component.upgrade().unwrap().root_element.borrow().id
            {
                if let Some(x) = enclosing_component.description.custom_properties.get(nr.name()) {
                    return unsafe { enclosing_component.as_ptr().add(x.offset).cast() };
                };
            };
            let item_info = enclosing_component
                .description
                .items
                .get(element.id.as_str())
                .unwrap_or_else(|| panic!("Unknown element for {}.{}", element.id, nr.name()));
            let prop_info = item_info
                .rtti
                .properties
                .get(nr.name().as_str())
                .unwrap_or_else(|| panic!("Property {} not in {}", nr.name(), element.id));
            core::mem::drop(element);
            let item = unsafe { item_info.item_from_item_tree(enclosing_component.as_ptr()) };
            unsafe { item.as_ptr().add(prop_info.offset()).cast() }
        }
        eval::ComponentInstance::GlobalComponent(glob) => glob.as_ref().get_property_ptr(nr.name()),
    }
}

pub struct ErasedItemTreeBox(ItemTreeBox<'static>);
impl ErasedItemTreeBox {
    pub fn unerase<'a, 'id>(
        &'a self,
        _guard: generativity::Guard<'id>,
    ) -> Pin<&'a ItemTreeBox<'id>> {
        Pin::new(
            //Safety: 'id is unique because of `_guard`
            unsafe { core::mem::transmute::<&ItemTreeBox<'static>, &ItemTreeBox<'id>>(&self.0) },
        )
    }

    pub fn borrow(&self) -> ItemTreeRefPin<'_> {
        // Safety: it is safe to access self.0 here because the 'id lifetime does not leak
        self.0.borrow()
    }

    pub fn window_adapter_ref(&self) -> Result<&WindowAdapterRc, PlatformError> {
        self.0.window_adapter_ref()
    }

    pub fn run_setup_code(&self) {
        generativity::make_guard!(guard);
        let compo_box = self.unerase(guard);
        let instance_ref = compo_box.borrow_instance();
        for extra_init_code in self.0.description.original.init_code.borrow().iter() {
            eval::eval_expression(
                extra_init_code,
                &mut eval::EvalLocalContext::from_component_instance(instance_ref),
            );
        }
        if let Some(cts) = instance_ref.description.change_trackers.as_ref() {
            let self_weak = instance_ref.self_weak().get().unwrap();
            let v = cts
                .1
                .iter()
                .enumerate()
                .map(|(idx, _)| {
                    let ct = ChangeTracker::default();
                    ct.init(
                        self_weak.clone(),
                        move |self_weak| {
                            let s = self_weak.upgrade().unwrap();
                            generativity::make_guard!(guard);
                            let compo_box = s.unerase(guard);
                            let instance_ref = compo_box.borrow_instance();
                            let nr = &s.0.description.change_trackers.as_ref().unwrap().1[idx].0;
                            eval::load_property(instance_ref, &nr.element(), nr.name()).unwrap()
                        },
                        move |self_weak, _| {
                            let s = self_weak.upgrade().unwrap();
                            generativity::make_guard!(guard);
                            let compo_box = s.unerase(guard);
                            let instance_ref = compo_box.borrow_instance();
                            let e = &s.0.description.change_trackers.as_ref().unwrap().1[idx].1;
                            eval::eval_expression(
                                e,
                                &mut eval::EvalLocalContext::from_component_instance(instance_ref),
                            );
                        },
                    );
                    ct
                })
                .collect::<Vec<_>>();
            cts.0
                .apply_pin(instance_ref.instance)
                .set(v)
                .unwrap_or_else(|_| panic!("run_setup_code called twice?"));
        }
        update_timers(instance_ref);
    }
}
impl<'id> From<ItemTreeBox<'id>> for ErasedItemTreeBox {
    fn from(inner: ItemTreeBox<'id>) -> Self {
        // Safety: Nothing access the component directly, we only access it through unerased where
        // the lifetime is unique again
        unsafe {
            ErasedItemTreeBox(core::mem::transmute::<ItemTreeBox<'id>, ItemTreeBox<'static>>(inner))
        }
    }
}

pub fn get_repeater_by_name<'a, 'id>(
    instance_ref: InstanceRef<'a, '_>,
    name: &str,
    guard: generativity::Guard<'id>,
) -> (std::pin::Pin<&'a Repeater<ErasedItemTreeBox>>, Rc<ItemTreeDescription<'id>>) {
    let rep_index = instance_ref.description.repeater_names[name];
    let rep_in_comp = instance_ref.description.repeater[rep_index].unerase(guard);
    (rep_in_comp.offset.apply_pin(instance_ref.instance), rep_in_comp.item_tree_to_repeat.clone())
}

#[cfg_attr(not(feature = "ffi"), i_slint_core_macros::remove_extern)]
extern "C" fn layout_info(component: ItemTreeRefPin, orientation: Orientation) -> LayoutInfo {
    generativity::make_guard!(guard);
    // This is fine since we can only be called with a component that with our vtable which is a ItemTreeDescription
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };
    let orientation = crate::eval_layout::from_runtime(orientation);

    let mut result = crate::eval_layout::get_layout_info(
        &instance_ref.description.original.root_element,
        instance_ref,
        &instance_ref.window_adapter(),
        orientation,
    );

    let constraints = instance_ref.description.original.root_constraints.borrow();
    if constraints.has_explicit_restrictions(orientation) {
        crate::eval_layout::fill_layout_info_constraints(
            &mut result,
            &constraints,
            orientation,
            &|nr: &NamedReference| {
                eval::load_property(instance_ref, &nr.element(), nr.name())
                    .unwrap()
                    .try_into()
                    .unwrap()
            },
        );
    }
    result
}

#[cfg_attr(not(feature = "ffi"), i_slint_core_macros::remove_extern)]
unsafe extern "C" fn get_item_ref(component: ItemTreeRefPin, index: u32) -> Pin<ItemRef> {
    let tree = get_item_tree(component);
    match &tree[index as usize] {
        ItemTreeNode::Item { item_array_index, .. } => {
            generativity::make_guard!(guard);
            let instance_ref = InstanceRef::from_pin_ref(component, guard);
            core::mem::transmute::<Pin<ItemRef>, Pin<ItemRef>>(
                instance_ref.description.item_array[*item_array_index as usize]
                    .apply_pin(instance_ref.instance),
            )
        }
        ItemTreeNode::DynamicTree { .. } => panic!("get_item_ref called on dynamic tree"),
    }
}

#[cfg_attr(not(feature = "ffi"), i_slint_core_macros::remove_extern)]
extern "C" fn get_subtree_range(component: ItemTreeRefPin, index: u32) -> IndexRange {
    generativity::make_guard!(guard);
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };
    if index as usize >= instance_ref.description.repeater.len() {
        let container_index = {
            let tree_node = &component.as_ref().get_item_tree()[index as usize];
            if let ItemTreeNode::DynamicTree { parent_index, .. } = tree_node {
                *parent_index
            } else {
                u32::MAX
            }
        };
        let container = component.as_ref().get_item_ref(container_index);
        let container = i_slint_core::items::ItemRef::downcast_pin::<
            i_slint_core::items::ComponentContainer,
        >(container)
        .unwrap();
        container.ensure_updated();
        container.subtree_range()
    } else {
        let rep_in_comp =
            unsafe { instance_ref.description.repeater[index as usize].get_untagged() };
        ensure_repeater_updated(instance_ref, rep_in_comp);

        let repeater = rep_in_comp.offset.apply(&instance_ref.instance);
        repeater.range().into()
    }
}

#[cfg_attr(not(feature = "ffi"), i_slint_core_macros::remove_extern)]
extern "C" fn get_subtree(
    component: ItemTreeRefPin,
    index: u32,
    subtree_index: usize,
    result: &mut ItemTreeWeak,
) {
    generativity::make_guard!(guard);
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };
    if index as usize >= instance_ref.description.repeater.len() {
        let container_index = {
            let tree_node = &component.as_ref().get_item_tree()[index as usize];
            if let ItemTreeNode::DynamicTree { parent_index, .. } = tree_node {
                *parent_index
            } else {
                u32::MAX
            }
        };
        let container = component.as_ref().get_item_ref(container_index);
        let container = i_slint_core::items::ItemRef::downcast_pin::<
            i_slint_core::items::ComponentContainer,
        >(container)
        .unwrap();
        container.ensure_updated();
        if subtree_index == 0 {
            *result = container.subtree_component();
        }
    } else {
        let rep_in_comp =
            unsafe { instance_ref.description.repeater[index as usize].get_untagged() };
        ensure_repeater_updated(instance_ref, rep_in_comp);

        let repeater = rep_in_comp.offset.apply(&instance_ref.instance);
        if let Some(instance_at) = repeater.instance_at(subtree_index) {
            *result = vtable::VRc::downgrade(&vtable::VRc::into_dyn(instance_at))
        }
    }
}

#[cfg_attr(not(feature = "ffi"), i_slint_core_macros::remove_extern)]
extern "C" fn get_item_tree(component: ItemTreeRefPin) -> Slice<ItemTreeNode> {
    generativity::make_guard!(guard);
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };
    let tree = instance_ref.description.item_tree.as_slice();
    unsafe { core::mem::transmute::<&[ItemTreeNode], &[ItemTreeNode]>(tree) }.into()
}

#[cfg_attr(not(feature = "ffi"), i_slint_core_macros::remove_extern)]
extern "C" fn subtree_index(component: ItemTreeRefPin) -> usize {
    generativity::make_guard!(guard);
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };
    if let Ok(value) = instance_ref.description.get_property(component, SPECIAL_PROPERTY_INDEX) {
        value.try_into().unwrap()
    } else {
        usize::MAX
    }
}

#[cfg_attr(not(feature = "ffi"), i_slint_core_macros::remove_extern)]
unsafe extern "C" fn parent_node(component: ItemTreeRefPin, result: &mut ItemWeak) {
    generativity::make_guard!(guard);
    let instance_ref = InstanceRef::from_pin_ref(component, guard);

    let component_and_index = {
        // Normal inner-compilation unit case:
        if let Some(parent_offset) = instance_ref.description.parent_item_tree_offset {
            let parent_item_index = instance_ref
                .description
                .original
                .parent_element
                .upgrade()
                .and_then(|e| e.borrow().item_index.get().cloned())
                .unwrap_or(u32::MAX);
            let parent_component = parent_offset
                .apply(instance_ref.as_ref())
                .get()
                .and_then(|p| p.upgrade())
                .map(vtable::VRc::into_dyn);

            (parent_component, parent_item_index)
        } else if let Some((parent_component, parent_index)) = instance_ref
            .description
            .extra_data_offset
            .apply(instance_ref.as_ref())
            .embedding_position
            .get()
        {
            (parent_component.upgrade(), *parent_index)
        } else {
            (None, u32::MAX)
        }
    };

    if let (Some(component), index) = component_and_index {
        *result = ItemRc::new(component, index).downgrade();
    }
}

#[cfg_attr(not(feature = "ffi"), i_slint_core_macros::remove_extern)]
unsafe extern "C" fn embed_component(
    component: ItemTreeRefPin,
    parent_component: &ItemTreeWeak,
    parent_item_tree_index: u32,
) -> bool {
    generativity::make_guard!(guard);
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };

    if instance_ref.description.parent_item_tree_offset.is_some() {
        // We are not the root of the compilation unit tree... Can not embed this!
        return false;
    }

    {
        // sanity check parent:
        let prc = parent_component.upgrade().unwrap();
        let pref = vtable::VRc::borrow_pin(&prc);
        let it = pref.as_ref().get_item_tree();
        if !matches!(
            it.get(parent_item_tree_index as usize),
            Some(ItemTreeNode::DynamicTree { .. })
        ) {
            panic!("Trying to embed into a non-dynamic index in the parents item tree")
        }
    }

    let extra_data = instance_ref.description.extra_data_offset.apply(instance_ref.as_ref());
    extra_data.embedding_position.set((parent_component.clone(), parent_item_tree_index)).is_ok()
}

#[cfg_attr(not(feature = "ffi"), i_slint_core_macros::remove_extern)]
extern "C" fn item_geometry(component: ItemTreeRefPin, item_index: u32) -> LogicalRect {
    generativity::make_guard!(guard);
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };

    let e = instance_ref.description.original_elements[item_index as usize].borrow();
    let g = e.geometry_props.as_ref().unwrap();

    let load_f32 = |nr: &NamedReference| -> f32 {
        crate::eval::load_property(instance_ref, &nr.element(), nr.name())
            .unwrap()
            .try_into()
            .unwrap()
    };

    LogicalRect {
        origin: (load_f32(&g.x), load_f32(&g.y)).into(),
        size: (load_f32(&g.width), load_f32(&g.height)).into(),
    }
}

// silence the warning despite `AccessibleRole` is a `#[non_exhaustive]` enum from another crate.
#[allow(improper_ctypes_definitions)]
#[cfg_attr(not(feature = "ffi"), i_slint_core_macros::remove_extern)]
extern "C" fn accessible_role(component: ItemTreeRefPin, item_index: u32) -> AccessibleRole {
    generativity::make_guard!(guard);
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };
    let nr = instance_ref.description.original_elements[item_index as usize]
        .borrow()
        .accessibility_props
        .0
        .get("accessible-role")
        .cloned();
    match nr {
        Some(nr) => crate::eval::load_property(instance_ref, &nr.element(), nr.name())
            .unwrap()
            .try_into()
            .unwrap(),
        None => AccessibleRole::default(),
    }
}

#[cfg_attr(not(feature = "ffi"), i_slint_core_macros::remove_extern)]
extern "C" fn accessible_string_property(
    component: ItemTreeRefPin,
    item_index: u32,
    what: AccessibleStringProperty,
    result: &mut SharedString,
) -> bool {
    generativity::make_guard!(guard);
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };
    let prop_name = format!("accessible-{what}");
    let nr = instance_ref.description.original_elements[item_index as usize]
        .borrow()
        .accessibility_props
        .0
        .get(&prop_name)
        .cloned();
    if let Some(nr) = nr {
        let value = crate::eval::load_property(instance_ref, &nr.element(), nr.name()).unwrap();
        match value {
            Value::String(s) => *result = s,
            Value::Bool(b) => *result = if b { "true" } else { "false" }.into(),
            Value::Number(x) => *result = x.to_string().into(),
            _ => unimplemented!("invalid type for accessible_string_property"),
        };
        true
    } else {
        false
    }
}

#[cfg_attr(not(feature = "ffi"), i_slint_core_macros::remove_extern)]
extern "C" fn accessibility_action(
    component: ItemTreeRefPin,
    item_index: u32,
    action: &AccessibilityAction,
) {
    let perform = |prop_name, args: &[Value]| {
        generativity::make_guard!(guard);
        let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };
        let nr = instance_ref.description.original_elements[item_index as usize]
            .borrow()
            .accessibility_props
            .0
            .get(prop_name)
            .cloned();
        if let Some(nr) = nr {
            let instance_ref = eval::ComponentInstance::InstanceRef(instance_ref);
            crate::eval::invoke_callback(&instance_ref, &nr.element(), nr.name(), args).unwrap();
        }
    };

    match action {
        AccessibilityAction::Default => perform("accessible-action-default", &[]),
        AccessibilityAction::Decrement => perform("accessible-action-decrement", &[]),
        AccessibilityAction::Increment => perform("accessible-action-increment", &[]),
        AccessibilityAction::Expand => perform("accessible-action-expand", &[]),
        AccessibilityAction::ReplaceSelectedText(_a) => {
            //perform("accessible-action-replace-selected-text", &[Value::String(a.clone())])
            i_slint_core::debug_log!("AccessibilityAction::ReplaceSelectedText not implemented in interpreter's accessibility_action");
        }
        AccessibilityAction::SetValue(a) => {
            perform("accessible-action-set-value", &[Value::String(a.clone())])
        }
    };
}

#[cfg_attr(not(feature = "ffi"), i_slint_core_macros::remove_extern)]
extern "C" fn supported_accessibility_actions(
    component: ItemTreeRefPin,
    item_index: u32,
) -> SupportedAccessibilityAction {
    generativity::make_guard!(guard);
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };
    let val = instance_ref.description.original_elements[item_index as usize]
        .borrow()
        .accessibility_props
        .0
        .keys()
        .filter_map(|x| x.strip_prefix("accessible-action-"))
        .fold(SupportedAccessibilityAction::default(), |acc, value| {
            SupportedAccessibilityAction::from_name(&i_slint_compiler::generator::to_pascal_case(
                value,
            ))
            .unwrap_or_else(|| panic!("Not an accessible action: {value:?}"))
                | acc
        });
    val
}

#[cfg_attr(not(feature = "ffi"), i_slint_core_macros::remove_extern)]
extern "C" fn item_element_infos(
    component: ItemTreeRefPin,
    item_index: u32,
    result: &mut SharedString,
) -> bool {
    generativity::make_guard!(guard);
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };
    *result = instance_ref.description.original_elements[item_index as usize]
        .borrow()
        .element_infos()
        .into();
    true
}

#[cfg_attr(not(feature = "ffi"), i_slint_core_macros::remove_extern)]
extern "C" fn window_adapter(
    component: ItemTreeRefPin,
    do_create: bool,
    result: &mut Option<WindowAdapterRc>,
) {
    generativity::make_guard!(guard);
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };
    if do_create {
        *result = Some(instance_ref.window_adapter());
    } else {
        *result = instance_ref.maybe_window_adapter();
    }
}

#[cfg_attr(not(feature = "ffi"), i_slint_core_macros::remove_extern)]
unsafe extern "C" fn drop_in_place(component: vtable::VRefMut<ItemTreeVTable>) -> vtable::Layout {
    let instance_ptr = component.as_ptr() as *mut Instance<'static>;
    let layout = (*instance_ptr).type_info().layout();
    dynamic_type::TypeInfo::drop_in_place(instance_ptr);
    layout.into()
}

#[cfg_attr(not(feature = "ffi"), i_slint_core_macros::remove_extern)]
unsafe extern "C" fn dealloc(_vtable: &ItemTreeVTable, ptr: *mut u8, layout: vtable::Layout) {
    std::alloc::dealloc(ptr, layout.try_into().unwrap());
}

#[derive(Copy, Clone)]
pub struct InstanceRef<'a, 'id> {
    pub instance: Pin<&'a Instance<'id>>,
    pub description: &'a ItemTreeDescription<'id>,
}

impl<'a, 'id> InstanceRef<'a, 'id> {
    pub unsafe fn from_pin_ref(
        component: ItemTreeRefPin<'a>,
        _guard: generativity::Guard<'id>,
    ) -> Self {
        Self {
            instance: Pin::new_unchecked(&*(component.as_ref().as_ptr() as *const Instance<'id>)),
            description: &*(Pin::into_inner_unchecked(component).get_vtable()
                as *const ItemTreeVTable
                as *const ItemTreeDescription<'id>),
        }
    }

    pub fn as_ptr(&self) -> *const u8 {
        (&*self.instance.as_ref()) as *const Instance as *const u8
    }

    pub fn as_ref(&self) -> &Instance<'id> {
        &self.instance
    }

    /// Borrow this component as a `Pin<ItemTreeRef>`
    pub fn borrow(self) -> ItemTreeRefPin<'a> {
        unsafe {
            Pin::new_unchecked(vtable::VRef::from_raw(
                NonNull::from(&self.description.ct).cast(),
                NonNull::from(self.instance.get_ref()).cast(),
            ))
        }
    }

    pub fn self_weak(&self) -> &OnceCell<ErasedItemTreeBoxWeak> {
        let extra_data = self.description.extra_data_offset.apply(self.as_ref());
        &extra_data.self_weak
    }

    pub fn root_weak(&self) -> &ErasedItemTreeBoxWeak {
        self.description.root_offset.apply(self.as_ref()).get().unwrap()
    }

    pub fn window_adapter(&self) -> WindowAdapterRc {
        let root_weak = vtable::VWeak::into_dyn(self.root_weak().clone());
        let root = self.root_weak().upgrade().unwrap();
        generativity::make_guard!(guard);
        let comp = root.unerase(guard);
        Self::get_or_init_window_adapter_ref(
            &comp.description,
            root_weak,
            true,
            comp.instance.as_pin_ref().get_ref(),
        )
        .unwrap()
        .clone()
    }

    pub fn get_or_init_window_adapter_ref<'b, 'id2>(
        description: &'b ItemTreeDescription<'id2>,
        root_weak: ItemTreeWeak,
        do_create: bool,
        instance: &'b Instance<'id2>,
    ) -> Result<&'b WindowAdapterRc, PlatformError> {
        // We are the actual root: Generate and store a window_adapter if necessary
        description.window_adapter_offset.apply(instance).get_or_try_init(|| {
            let mut parent_node = ItemWeak::default();
            if let Some(rc) = vtable::VWeak::upgrade(&root_weak) {
                vtable::VRc::borrow_pin(&rc).as_ref().parent_node(&mut parent_node);
            }

            if let Some(parent) = parent_node.upgrade() {
                // We are embedded: Get window adapter from our parent
                let mut result = None;
                vtable::VRc::borrow_pin(parent.item_tree())
                    .as_ref()
                    .window_adapter(do_create, &mut result);
                result.ok_or(PlatformError::NoPlatform)
            } else if do_create {
                let extra_data = description.extra_data_offset.apply(instance);
                let window_adapter = // We are the root: Create a window adapter
                    i_slint_backend_selector::with_platform(|_b| {
                        return _b.create_window_adapter();
                    })?;

                let comp_rc = extra_data.self_weak.get().unwrap().upgrade().unwrap();
                WindowInner::from_pub(window_adapter.window())
                    .set_component(&vtable::VRc::into_dyn(comp_rc));
                Ok(window_adapter)
            } else {
                Err(PlatformError::NoPlatform)
            }
        })
    }

    pub fn maybe_window_adapter(&self) -> Option<WindowAdapterRc> {
        let root_weak = vtable::VWeak::into_dyn(self.root_weak().clone());
        let root = self.root_weak().upgrade()?;
        generativity::make_guard!(guard);
        let comp = root.unerase(guard);
        Self::get_or_init_window_adapter_ref(
            &comp.description,
            root_weak,
            false,
            comp.instance.as_pin_ref().get_ref(),
        )
        .ok()
        .cloned()
    }

    pub fn access_window<R>(
        self,
        callback: impl FnOnce(&'_ i_slint_core::window::WindowInner) -> R,
    ) -> R {
        callback(WindowInner::from_pub(self.window_adapter().window()))
    }

    pub fn parent_instance<'id2>(
        &self,
        _guard: generativity::Guard<'id2>,
    ) -> Option<InstanceRef<'a, 'id2>> {
        // we need a 'static guard in order to be able to re-borrow with lifetime 'a.
        // Safety: This is the only 'static Id in scope.
        if let Some(parent_offset) = self.description.parent_item_tree_offset {
            if let Some(parent) =
                parent_offset.apply(self.as_ref()).get().and_then(vtable::VWeak::upgrade)
            {
                let parent_instance = parent.unerase(_guard);
                // And also assume that the parent lives for at least 'a.  FIXME: this may not be sound
                let parent_instance = unsafe {
                    std::mem::transmute::<InstanceRef<'_, 'id2>, InstanceRef<'a, 'id2>>(
                        parent_instance.borrow_instance(),
                    )
                };
                return Some(parent_instance);
            };
        }
        None
    }

    pub fn toplevel_instance<'id2>(
        &self,
        _guard: generativity::Guard<'id2>,
    ) -> InstanceRef<'a, 'id2> {
        generativity::make_guard!(guard2);
        if let Some(parent) = self.parent_instance(guard2) {
            let tl = parent.toplevel_instance(_guard);
            // assuming that the parent lives at least for lifetime 'a.
            // FIXME: this may not be sound
            unsafe { std::mem::transmute::<InstanceRef<'_, 'id2>, InstanceRef<'a, 'id2>>(tl) }
        } else {
            // Safety: casting from an id to a new id is valid
            unsafe { std::mem::transmute::<InstanceRef<'a, 'id>, InstanceRef<'a, 'id2>>(*self) }
        }
    }
}

/// Show the popup at the given location
pub fn show_popup(
    element: ElementRc,
    instance: InstanceRef,
    popup: &object_tree::PopupWindow,
    pos_getter: impl FnOnce(InstanceRef<'_, '_>) -> LogicalPosition,
    close_policy: PopupClosePolicy,
    parent_comp: ErasedItemTreeBoxWeak,
    parent_window_adapter: WindowAdapterRc,
    parent_item: &ItemRc,
) {
    generativity::make_guard!(guard);
    let debug_handler = instance.description.debug_handler.borrow().clone();

    // FIXME: we should compile once and keep the cached compiled component
    let compiled = generate_item_tree(
        &popup.component,
        None,
        parent_comp.upgrade().unwrap().0.description().popup_menu_description.clone(),
        false,
        guard,
    );
    compiled.recursively_set_debug_handler(debug_handler);

    let inst = instantiate(
        compiled,
        Some(parent_comp),
        None,
        Some(&WindowOptions::UseExistingWindow(parent_window_adapter.clone())),
        Default::default(),
    );
    let pos = {
        generativity::make_guard!(guard);
        let compo_box = inst.unerase(guard);
        let instance_ref = compo_box.borrow_instance();
        pos_getter(instance_ref)
    };
    close_popup(element.clone(), instance, parent_window_adapter.clone());
    instance.description.popup_ids.borrow_mut().insert(
        element.borrow().id.clone(),
        WindowInner::from_pub(parent_window_adapter.window()).show_popup(
            &vtable::VRc::into_dyn(inst.clone()),
            pos,
            close_policy,
            parent_item,
            false,
        ),
    );
    inst.run_setup_code();
}

pub fn close_popup(
    element: ElementRc,
    instance: InstanceRef,
    parent_window_adapter: WindowAdapterRc,
) {
    if let Some(current_id) =
        instance.description.popup_ids.borrow_mut().remove(&element.borrow().id)
    {
        WindowInner::from_pub(parent_window_adapter.window()).close_popup(current_id);
    }
}

pub fn make_menu_item_tree(
    menu_item_tree: &Rc<object_tree::Component>,
    enclosing_component: &InstanceRef,
    condition: Option<&Expression>,
) -> vtable::VRc<i_slint_core::menus::MenuVTable, MenuFromItemTree> {
    generativity::make_guard!(guard);
    let mit_compiled = generate_item_tree(
        menu_item_tree,
        None,
        enclosing_component.description.popup_menu_description.clone(),
        false,
        guard,
    );
    let enclosing_component_weak = enclosing_component.self_weak().get().unwrap();
    let mit_inst = instantiate(
        mit_compiled.clone(),
        Some(enclosing_component_weak.clone()),
        None,
        None,
        Default::default(),
    );
    mit_inst.run_setup_code();
    let item_tree = vtable::VRc::into_dyn(mit_inst);
    let menu = match condition {
        Some(condition) => {
            let binding =
                make_binding_eval_closure(condition.clone(), enclosing_component_weak.clone());
            MenuFromItemTree::new_with_condition(item_tree, move || binding().try_into().unwrap())
        }
        None => MenuFromItemTree::new(item_tree),
    };
    vtable::VRc::new(menu)
}

pub fn update_timers(instance: InstanceRef) {
    let ts = instance.description.original.timers.borrow();
    for (desc, offset) in ts.iter().zip(&instance.description.timers) {
        let timer = offset.apply(instance.as_ref());
        let running =
            eval::load_property(instance, &desc.running.element(), desc.running.name()).unwrap();
        if matches!(running, Value::Bool(true)) {
            let millis: i64 =
                eval::load_property(instance, &desc.interval.element(), desc.interval.name())
                    .unwrap()
                    .try_into()
                    .expect("interval must be a duration");
            if millis < 0 {
                timer.stop();
                continue;
            }
            let interval = core::time::Duration::from_millis(millis as _);
            if !timer.running() || interval != timer.interval() {
                let callback = desc.triggered.clone();
                let self_weak = instance.self_weak().get().unwrap().clone();
                timer.start(i_slint_core::timers::TimerMode::Repeated, interval, move || {
                    if let Some(instance) = self_weak.upgrade() {
                        generativity::make_guard!(guard);
                        let c = instance.unerase(guard);
                        let c = c.borrow_instance();
                        let inst = eval::ComponentInstance::InstanceRef(c);
                        eval::invoke_callback(&inst, &callback.element(), callback.name(), &[])
                            .unwrap();
                    }
                });
            }
        } else {
            timer.stop();
        }
    }
}

pub fn restart_timer(element: ElementWeak, instance: InstanceRef) {
    let timers = instance.description.original.timers.borrow();
    if let Some((_, offset)) = timers
        .iter()
        .zip(&instance.description.timers)
        .find(|(desc, _)| Weak::ptr_eq(&desc.element, &element))
    {
        let timer = offset.apply(instance.as_ref());
        timer.restart();
    }
}
*/



#[derive(Clone)]
struct InterpreterContext {

}

type EvaluationContext<'a> = llr::EvaluationContext<'a, InterpreterContext>;
type ParentCtx<'a> = llr::ParentCtx<'a, InterpreterContext>;
