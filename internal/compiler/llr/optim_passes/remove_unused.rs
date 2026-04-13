// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::llr::*;
use typed_index_collections::TiVec;

struct Mapping {
    prop_mapping: TiVec<PropertyIdx, Option<PropertyIdx>>,
    callback_mapping: TiVec<CallbackIdx, Option<CallbackIdx>>,
    function_mapping: TiVec<FunctionIdx, Option<FunctionIdx>>,
}

impl Mapping {
    fn keep(&self, member: &LocalMemberIndex) -> bool {
        match member {
            LocalMemberIndex::Property(p) => self.prop_mapping[*p].is_some(),
            LocalMemberIndex::Callback(c) => self.callback_mapping[*c].is_some(),
            LocalMemberIndex::Function(f) => self.function_mapping[*f].is_some(),
            LocalMemberIndex::Native { .. } => true,
        }
    }
}

type ScMappings = TiVec<SubComponentIdx, Mapping>;
type GlobMappings = TiVec<GlobalIdx, Mapping>;

pub fn remove_unused(root: &mut CompilationUnit) {
    struct RemoveUnusedMappings {
        sc_mappings: ScMappings,
        glob_mappings: GlobMappings,
    }
    let mappings = RemoveUnusedMappings {
        sc_mappings: root
            .sub_components
            .iter_mut()
            .map(|sc| create_mapping(&mut sc.properties, &mut sc.callbacks, &mut sc.functions))
            .collect(),
        glob_mappings: root
            .globals
            .iter_mut()
            .map(|g| {
                clean_vec(&mut g.const_properties, &g.properties);
                clean_vec(&mut g.prop_analysis, &g.properties);
                create_mapping(&mut g.properties, &mut g.callbacks, &mut g.functions)
            })
            .collect(),
    };

    let state = visitor::VisitorState::new(root);

    for (idx, sc) in root.sub_components.iter_mut_enumerated() {
        let keep = |refer: &MemberReference| match refer {
            MemberReference::Relative { parent_level, local_reference } => {
                assert_eq!(*parent_level, 0);
                let idx = state.follow_sub_components(idx, &local_reference.sub_component_path);
                mappings.sc_mappings[idx].keep(&local_reference.reference)
            }
            MemberReference::Global { global_index, member } => {
                mappings.glob_mappings[*global_index].keep(member)
            }
        };

        let mut property_init_mapping = Vec::new();
        let mut i = 0;
        sc.property_init.retain(|(x, v)| {
            if keep(x) && v.use_count.get() > 0 {
                property_init_mapping.push(Some(i));
                i += 1;
                true
            } else {
                property_init_mapping.push(None);
                false
            }
        });
        sc.change_callbacks.retain(|(x, _)| keep(x));
        sc.const_properties.retain(|x| {
            let idx = state.follow_sub_components(idx, &x.sub_component_path);
            mappings.sc_mappings[idx].keep(&x.reference)
        });
        sc.prop_analysis.retain(|x, v| {
            v.property_init = v.property_init.and_then(|x| property_init_mapping[x]);
            keep(x)
        });
        sc.animations.retain(|x, _| keep(&x.clone().into()));
    }
    for (idx, g) in root.globals.iter_mut_enumerated() {
        g.init_values.retain(|x, _| mappings.glob_mappings[idx].keep(x));
        g.animations.retain(|x, _| mappings.glob_mappings[idx].keep(x));
    }

    macro_rules! remap_index {
        ($method:ident, $idx:ty, $field:ident) => {
            fn $method(
                &mut self,
                p: &mut $idx,
                scope: &EvaluationScope,
                _state: &visitor::VisitorState,
            ) {
                *p = match scope {
                    EvaluationScope::SubComponent(sub_component_idx, _) => {
                        self.sc_mappings[*sub_component_idx].$field[*p]
                    }
                    EvaluationScope::Global(global_idx) => {
                        self.glob_mappings[*global_idx].$field[*p]
                    }
                    EvaluationScope::Const => {
                        panic!("member reference in a constant expression")
                    }
                }
                .unwrap();
            }
        };
    }
    impl visitor::Visitor for &RemoveUnusedMappings {
        // All three remap an index through the mapping of the enclosing scope. If one of the
        // unwraps fails, count_property_use() forgot to visit something, so a member that is
        // still referenced was removed.
        remap_index!(visit_property_idx, PropertyIdx, prop_mapping);
        remap_index!(visit_callback_idx, CallbackIdx, callback_mapping);
        remap_index!(visit_function_idx, FunctionIdx, function_mapping);
    }
    let mut visitor = &mappings;
    visitor::visit_compilation_unit(root, &state, &mut visitor);
}

fn create_mapping(
    properties: &mut TiVec<PropertyIdx, Property>,
    callbacks: &mut TiVec<CallbackIdx, Callback>,
    functions: &mut TiVec<FunctionIdx, Function>,
) -> Mapping {
    Mapping {
        prop_mapping: create_vec_mapping(properties, |p| p.use_count.get() > 0),
        callback_mapping: create_vec_mapping(callbacks, |c| c.use_count.get() > 0),
        function_mapping: create_vec_mapping(functions, |f| f.use_count.get() > 0),
    }
}

fn create_vec_mapping<Idx: From<usize>, T>(
    vec: &mut TiVec<Idx, T>,
    mut retain: impl FnMut(&T) -> bool,
) -> TiVec<Idx, Option<Idx>> {
    let mut map = TiVec::with_capacity(vec.len());
    let mut i = 0;
    vec.retain(|t| {
        if retain(t) {
            map.push(Some(Idx::from(i)));
            i += 1;
            true
        } else {
            map.push(None);
            false
        }
    });
    map
}

fn clean_vec<T>(vec: &mut TiVec<PropertyIdx, T>, properties: &TiVec<PropertyIdx, Property>) {
    let mut idx = 0;
    vec.retain(|_| {
        idx += 1;
        properties[PropertyIdx::from(idx - 1)].use_count.get() >= 1
    });
}

mod visitor {

    use super::*;

    pub trait Visitor {
        fn visit_property_idx(
            &mut self,
            _p: &mut PropertyIdx,
            _scope: &EvaluationScope,
            _state: &VisitorState,
        ) {
        }
        fn visit_function_idx(
            &mut self,
            _p: &mut FunctionIdx,
            _scope: &EvaluationScope,
            _state: &VisitorState,
        ) {
        }

        fn visit_callback_idx(
            &mut self,
            _p: &mut CallbackIdx,
            _scope: &EvaluationScope,
            _state: &VisitorState,
        ) {
        }
    }

    pub struct VisitorState {
        /// Copy of SubComponent::sub_components::ty
        sub_component_maps: TiVec<SubComponentIdx, TiVec<SubComponentInstanceIdx, SubComponentIdx>>,
        /// parent mapping
        parent_mapping: TiVec<SubComponentIdx, Option<SubComponentIdx>>,
    }

    impl VisitorState {
        pub fn new(cu: &CompilationUnit) -> Self {
            let mut parent_mapping = TiVec::new();
            parent_mapping.resize(cu.sub_components.len(), None);
            for (idx, sc) in cu.sub_components.iter_enumerated() {
                for r in &sc.repeated {
                    parent_mapping[r.sub_tree.root] = Some(idx);
                }
                for p in &sc.popup_windows {
                    parent_mapping[p.item_tree.root] = Some(idx);
                }
                for m in &sc.menu_item_trees {
                    parent_mapping[m.root] = Some(idx);
                }
            }
            Self {
                sub_component_maps: cu
                    .sub_components
                    .iter()
                    .map(|sc| sc.sub_components.iter().map(|x| x.ty).collect())
                    .collect(),
                parent_mapping,
            }
        }

        pub fn follow_sub_components(
            &self,
            mut sc: SubComponentIdx,
            sub_component_path: &[SubComponentInstanceIdx],
        ) -> SubComponentIdx {
            for i in sub_component_path {
                sc = self.sub_component_maps[sc][*i];
            }
            sc
        }
    }

    pub fn visit_compilation_unit(
        CompilationUnit {
            public_components,
            sub_components,
            used_sub_components: _,
            globals,
            popup_menu,
            has_debug_info: _,
            #[cfg(feature = "bundle-translations")]
                translations: _,
        }: &mut crate::llr::CompilationUnit,
        state: &VisitorState,
        visitor: &mut (impl Visitor + ?Sized),
    ) {
        for c in public_components {
            visit_public_component(c, state, visitor);
        }
        for (idx, sc) in sub_components.iter_mut_enumerated() {
            visit_sub_component(idx, sc, state, visitor);
        }
        for (idx, g) in globals.iter_mut_enumerated() {
            visit_global(idx, g, state, visitor);
        }
        if let Some(p) = popup_menu {
            visit_popup_menu(p, state, visitor);
        }
    }

    pub fn visit_public_component(
        PublicComponent {
            public_properties,
            private_properties: _,
            item_tree,
            name: _,
            top_level_type: _,
        }: &mut PublicComponent,
        state: &VisitorState,
        visitor: &mut (impl Visitor + ?Sized),
    ) {
        let scope = EvaluationScope::SubComponent(item_tree.root, None);
        for p in public_properties {
            visit_public_property(p, &scope, state, visitor);
        }
    }

    pub fn visit_sub_component(
        idx: SubComponentIdx,
        SubComponent {
            name: _,
            properties: _,
            callbacks: _,
            functions,
            items: _,
            repeated,
            component_containers: _,
            popup_windows,
            menu_item_trees: _,
            timers,
            sub_components: _,
            property_init,
            change_callbacks,
            animations,
            two_way_bindings,
            const_properties,
            pre_init_code,
            init_code,
            geometries,
            layout_info_h,
            layout_info_v,
            child_of_layout: _,
            grid_layout_input_for_repeated,
            flexbox_layout_item_info_for_repeated,
            layout_info_v_constrained_for_repeated,
            layout_info_v_at_cross_width_for_repeated,
            is_repeated_row: _,
            grid_layout_children,
            accessible_prop,
            element_infos: _,
            row_child_templates: _,
            prop_analysis,
            debug_info: _,
        }: &mut SubComponent,
        state: &VisitorState,
        visitor: &mut (impl Visitor + ?Sized),
    ) {
        let scope = EvaluationScope::SubComponent(idx, None);
        for f in functions {
            visit_function(f, &scope, state, visitor);
        }
        for RepeatedElement {
            model,
            index_prop,
            data_prop,
            sub_tree,
            index_in_tree: _,
            listview,
            container_item_index: _,
        } in repeated
        {
            visit_expression(model.get_mut(), &scope, state, visitor);
            let inner_scope = EvaluationScope::SubComponent(sub_tree.root, None);
            if let Some(index_prop) = index_prop {
                visitor.visit_property_idx(index_prop, &inner_scope, state);
            }
            if let Some(data_prop) = data_prop {
                visitor.visit_property_idx(data_prop, &inner_scope, state);
            }

            if let Some(listview) = listview {
                visit_member_reference(&mut listview.viewport_y, &scope, state, visitor);
                visit_member_reference(&mut listview.viewport_height, &scope, state, visitor);
                visit_member_reference(&mut listview.viewport_width, &scope, state, visitor);
                visit_member_reference(&mut listview.listview_width, &scope, state, visitor);
                visit_member_reference(&mut listview.listview_height, &scope, state, visitor);

                visit_member_reference(&mut listview.prop_y, &inner_scope, state, visitor);
                visit_member_reference(&mut listview.prop_height, &inner_scope, state, visitor);
            }
        }

        for p in popup_windows {
            let popup_scope = EvaluationScope::SubComponent(p.item_tree.root, None);
            visit_expression(p.position.get_mut(), &popup_scope, state, visitor);
        }
        for t in timers {
            visit_expression(t.interval.get_mut(), &scope, state, visitor);
            visit_expression(t.triggered.get_mut(), &scope, state, visitor);
            visit_expression(t.running.get_mut(), &scope, state, visitor);
        }
        for (idx, init) in property_init {
            visit_member_reference(idx, &scope, state, visitor);
            visit_binding_expression(init, &scope, state, visitor);
        }
        for (idx, e) in change_callbacks {
            visit_member_reference(idx, &scope, state, visitor);
            visit_expression(e.get_mut(), &scope, state, visitor);
        }
        *animations = std::mem::take(animations)
            .into_iter()
            .map(|(mut k, mut v)| {
                visit_local_member_reference(&mut k, &scope, state, visitor);
                visit_expression(&mut v, &scope, state, visitor);
                (k, v)
            })
            .collect();

        for twb in two_way_bindings {
            visit_local_member_reference(&mut twb.prop1, &scope, state, visitor);
            visit_member_reference(&mut twb.prop2, &scope, state, visitor);
        }
        for c in const_properties {
            visit_local_member_reference(c, &scope, state, visitor);
        }
        for i in pre_init_code.iter_mut().chain(init_code) {
            visit_expression(i.get_mut(), &scope, state, visitor);
        }
        for g in geometries.iter_mut().flatten() {
            visit_expression(g.get_mut(), &scope, state, visitor);
        }
        visit_expression(layout_info_h.get_mut(), &scope, state, visitor);
        visit_expression(layout_info_v.get_mut(), &scope, state, visitor);
        if let Some(e) = grid_layout_input_for_repeated {
            visit_expression(e.get_mut(), &scope, state, visitor);
        }
        if let Some(e) = flexbox_layout_item_info_for_repeated {
            visit_expression(e.get_mut(), &scope, state, visitor);
        }
        if let Some(e) = layout_info_v_constrained_for_repeated {
            visit_expression(e.get_mut(), &scope, state, visitor);
        }
        if let Some(e) = layout_info_v_at_cross_width_for_repeated {
            visit_expression(e.get_mut(), &scope, state, visitor);
        }
        for child in grid_layout_children {
            visit_expression(child.layout_info_h.get_mut(), &scope, state, visitor);
            visit_expression(child.layout_info_v.get_mut(), &scope, state, visitor);
        }

        for a in accessible_prop.values_mut() {
            visit_expression(a.get_mut(), &scope, state, visitor);
        }

        *prop_analysis = std::mem::take(prop_analysis)
            .into_iter()
            .map(|(mut k, v)| {
                visit_member_reference(&mut k, &scope, state, visitor);
                (k, v)
            })
            .collect();
    }

    fn visit_global(
        global_idx: GlobalIdx,
        GlobalComponent {
            name: _,
            properties: _,
            callbacks: _,
            functions,
            init_values,
            animations,
            change_callbacks,
            const_properties: _,
            public_properties,
            private_properties: _,
            exported: _,
            aliases: _,
            is_builtin: _,
            from_library: _,
            prop_analysis: _,
        }: &mut GlobalComponent,
        state: &VisitorState,
        visitor: &mut (impl Visitor + ?Sized),
    ) {
        let scope = EvaluationScope::Global(global_idx);
        for f in functions {
            visit_function(f, &scope, state, visitor);
        }

        *init_values = std::mem::take(init_values)
            .into_iter()
            .map(|(mut k, mut v)| {
                visit_member_index(&mut k, &scope, state, visitor);
                visit_binding_expression(&mut v, &scope, state, visitor);
                (k, v)
            })
            .collect();

        *animations = std::mem::take(animations)
            .into_iter()
            .map(|(mut k, mut v)| {
                visit_member_index(&mut k, &scope, state, visitor);
                visit_expression(&mut v, &scope, state, visitor);
                (k, v)
            })
            .collect();

        *change_callbacks = std::mem::take(change_callbacks)
            .into_iter()
            .map(|(mut k, mut v)| {
                visitor.visit_property_idx(&mut k, &scope, state);
                visit_expression(v.get_mut(), &scope, state, visitor);
                (k, v)
            })
            .collect();

        for p in public_properties {
            visit_public_property(p, &scope, state, visitor);
        }
    }

    pub fn visit_popup_menu(
        PopupMenu { item_tree, sub_menu, activated, close, entries }: &mut PopupMenu,
        state: &VisitorState,
        visitor: &mut (impl Visitor + ?Sized),
    ) {
        let scope = EvaluationScope::SubComponent(item_tree.root, None);
        visit_member_reference(sub_menu, &scope, state, visitor);
        visit_member_reference(activated, &scope, state, visitor);
        visit_member_reference(close, &scope, state, visitor);
        visit_member_reference(entries, &scope, state, visitor);
    }

    pub fn visit_public_property(
        PublicProperty { name: _, ty: _, prop, read_only: _ }: &mut PublicProperty,
        scope: &EvaluationScope,
        state: &VisitorState,
        visitor: &mut (impl Visitor + ?Sized),
    ) {
        visit_member_reference(prop, scope, state, visitor);
    }

    pub fn visit_function(
        Function { name: _, ret_ty: _, args: _, code, use_count: _ }: &mut Function,
        scope: &EvaluationScope,
        state: &VisitorState,
        visitor: &mut (impl Visitor + ?Sized),
    ) {
        visit_expression(code.get_mut(), scope, state, visitor);
    }

    pub fn visit_expression(
        expr: &mut Expression,
        scope: &EvaluationScope,
        state: &VisitorState,
        visitor: &mut (impl Visitor + ?Sized),
    ) {
        expr.visit_recursive_mut(&mut |expr| {
            let p = match expr {
                Expression::PropertyReference(p) => p,
                Expression::CallBackCall { callback, .. } => callback,
                Expression::FunctionCall { function, .. } => function,
                Expression::PropertyAssignment { property, .. } => property,
                Expression::LayoutCacheAccess { layout_cache_prop, .. } => layout_cache_prop,
                Expression::GridRepeaterCacheAccess { layout_cache_prop, .. } => layout_cache_prop,
                _ => return,
            };
            visit_member_reference(p, scope, state, visitor);
        });
    }

    pub fn visit_binding_expression(
        BindingExpression { expression, animation, kind: _, use_count: _ }: &mut BindingExpression,
        scope: &EvaluationScope,
        state: &VisitorState,
        visitor: &mut (impl Visitor + ?Sized),
    ) {
        visit_expression(expression.get_mut(), scope, state, visitor);
        match animation {
            Some(Animation::Static(anim) | Animation::Transition(anim)) => {
                visit_expression(anim, scope, state, visitor)
            }
            None => (),
        }
    }

    pub fn visit_member_reference(
        member: &mut MemberReference,
        scope: &EvaluationScope,
        state: &VisitorState,
        visitor: &mut (impl Visitor + ?Sized),
    ) {
        match member {
            MemberReference::Relative { parent_level, local_reference } => {
                let &EvaluationScope::SubComponent(mut sc, _) = scope else { unreachable!() };
                for _ in 0..*parent_level {
                    sc = state.parent_mapping[sc].unwrap();
                }
                let scope = EvaluationScope::SubComponent(sc, None);
                visit_local_member_reference(local_reference, &scope, state, visitor);
            }
            MemberReference::Global { global_index, member } => {
                let scope = EvaluationScope::Global(*global_index);
                visit_member_index(member, &scope, state, visitor);
            }
        }
    }

    pub fn visit_local_member_reference(
        local_reference: &mut LocalMemberReference,
        scope: &EvaluationScope,
        state: &VisitorState,
        visitor: &mut (impl Visitor + ?Sized),
    ) {
        let scope = match scope {
            EvaluationScope::SubComponent(sub_component_idx, _) => EvaluationScope::SubComponent(
                state
                    .follow_sub_components(*sub_component_idx, &local_reference.sub_component_path),
                None,
            ),
            scope => *scope,
        };
        visit_member_index(&mut local_reference.reference, &scope, state, visitor);
    }

    pub fn visit_member_index(
        member: &mut LocalMemberIndex,
        scope: &EvaluationScope,
        state: &VisitorState,
        visitor: &mut (impl Visitor + ?Sized),
    ) {
        match member {
            LocalMemberIndex::Property(p) => {
                visitor.visit_property_idx(p, scope, state);
            }
            LocalMemberIndex::Function(f) => {
                visitor.visit_function_idx(f, scope, state);
            }
            LocalMemberIndex::Callback(c) => {
                visitor.visit_callback_idx(c, scope, state);
            }
            LocalMemberIndex::Native { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    /// Compile `source`, lower it to the LLR (which runs the optimization passes
    /// including [`remove_unused`]), and return every declared property name
    /// across all sub-components. The names are prefixed by the element path, so
    /// callers match with `contains`.
    fn lowered_property_names(source: &str) -> std::collections::HashSet<String> {
        let mut config =
            crate::CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
        config.style = Some("fluent".into());
        let mut diags = crate::diagnostics::BuildDiagnostics::default();
        let doc_node =
            crate::parser::parse(source.into(), Some(std::path::Path::new("t.slint")), &mut diags);
        let (doc, diag, _) =
            spin_on::spin_on(crate::compile_syntax_node(doc_node, diags, config.clone()));
        assert!(!diag.has_errors(), "compile error: {:#?}", diag.to_string_vec());
        let unit = crate::llr::lower_to_item_tree::lower_to_item_tree(&doc, &config);
        unit.sub_components
            .iter()
            .flat_map(|sc| sc.properties.iter().map(|p| p.name.to_string()))
            .collect()
    }

    /// The property values below all depend on the `ext` input so they are not
    /// constant-folded away by the object-tree passes, and therefore actually
    /// exercise the LLR optimization passes.
    const SOURCE: &str = r#"
export component Foo inherits Window {
    in property <int> ext: 1;
    in property <int> ext2: 2;

    // KEPT: exposed in the public API.
    out property <int> kept_public: ext + 1;

    // REMOVED: read only from one other binding, so it is inlined into it and
    // the reader is itself unused.
    property <int> gone_inlined: ext * 2;
    property <int> gone_reader: gone_inlined + 3;

    // REMOVED: cheap enough to inline into each of its two (unused) readers.
    property <int> gone_shared: ext * 3;
    property <int> gone_shared_a: gone_shared + 1;
    property <int> gone_shared_b: gone_shared + 2;

    // KEPT: binding too expensive to inline, and read from two places.
    property <int> kept_expensive: ext * ext2 + ext2 * ext + ext * ext2;
    out property <int> exp_a: kept_expensive + 1;
    out property <int> exp_b: kept_expensive + 2;

    // REMOVED: only read from a timer's interval, which is inlined.
    property <duration> gone_timer: ext * 1ms;

    // REMOVED: read from a function that is called exactly once. The function is
    // inlined into its caller, and the property is then inlined and removed.
    property <int> gone_single_call: ext * 5;
    pure function called_once() -> int { gone_single_call }
    out property <int> single_reader: called_once();

    // REMOVED: read only from a function's body. The function itself is not
    // inlined (it is called from two places), but the property is inlined into
    // the body, so it becomes unused.
    property <int> gone_multi_call: ext * 6;
    pure function called_twice() -> int { gone_multi_call }
    out property <int> reader_a: called_twice();
    out property <int> reader_b: called_twice();

    // KEPT: has a change callback.
    property <int> kept_with_change_callback: ext * 7;
    changed kept_with_change_callback => {}

    // REMOVED: read only from a callback handler, which is inlined into like a
    // binding.
    property <int> gone_callback: ext * 8;
    callback do_it(int);
    do_it(v) => { debug(gone_callback + v); }

    Timer { interval: gone_timer; running: true; triggered => { do_it(1); } }
}
"#;

    #[test]
    fn unused_properties_are_removed_and_used_ones_kept() {
        let names = lowered_property_names(SOURCE);
        for gone in [
            "gone-inlined",
            "gone-reader",
            "gone-shared",
            "gone-timer",
            "gone-single-call",
            "gone-multi-call",
            "gone-callback",
        ] {
            assert!(
                !names.iter().any(|n| n.contains(gone)),
                "property {gone} should have been removed, got {names:?}"
            );
        }
        for kept in ["kept-public", "kept-expensive", "kept-with-change-callback"] {
            assert!(
                names.iter().any(|n| n.contains(kept)),
                "property {kept} should have been kept, got {names:?}"
            );
        }
    }
}
