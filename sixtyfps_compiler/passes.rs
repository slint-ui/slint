/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

mod apply_default_properties_from_style;
mod binding_analysis;
mod check_expressions;
mod check_public_api;
mod clip;
mod collect_custom_fonts;
mod collect_globals;
mod collect_structs;
mod collect_subcomponents;
mod compile_paths;
mod const_propagation;
mod deduplicate_property_read;
mod default_geometry;
mod embed_images;
mod ensure_window;
mod flickable;
mod focus_item;
mod generate_item_indices;
mod infer_aliases_types;
mod inlining;
mod lower_layout;
mod lower_popups;
mod lower_shadows;
mod lower_states;
mod lower_tabwidget;
mod materialize_fake_properties;
mod move_declarations;
mod optimize_useless_rectangles;
mod remove_aliases;
mod remove_unused_properties;
mod repeater_component;
mod resolve_native_classes;
mod resolving;
mod transform_and_opacity;
mod unique_id;
mod visible;
mod z_order;

use crate::langtype::Type;

pub async fn run_passes(
    doc: &crate::object_tree::Document,
    diag: &mut crate::diagnostics::BuildDiagnostics,
    mut type_loader: &mut crate::typeloader::TypeLoader<'_>,
    compiler_config: &crate::CompilerConfiguration,
) {
    if matches!(doc.root_component.root_element.borrow().base_type, Type::Invalid | Type::Void) {
        // If there isn't a root component, we shouldn't do any of these passes
        return;
    }

    let global_type_registry = type_loader.global_type_registry.clone();
    let root_component = &doc.root_component;
    infer_aliases_types::resolve_aliases(doc, diag);
    resolving::resolve_expressions(doc, type_loader, diag);
    check_expressions::check_expressions(doc, diag);
    unique_id::check_unique_id(doc, diag);
    check_public_api::check_public_api(doc, diag);

    collect_subcomponents::collect_subcomponents(root_component);
    for component in (root_component.used_types.borrow().sub_components.iter())
        .chain(std::iter::once(root_component))
    {
        compile_paths::compile_paths(component, &doc.local_registry, diag);
        lower_tabwidget::lower_tabwidget(component, &mut type_loader, diag).await;
    }

    inlining::inline(doc, inlining::InlineSelection::InlineOnlyRequiredComponents);
    collect_subcomponents::collect_subcomponents(root_component);

    embed_images::embed_images(root_component, compiler_config.embed_resources, diag);

    for component in (root_component.used_types.borrow().sub_components.iter())
        .chain(std::iter::once(root_component))
    {
        focus_item::resolve_element_reference_in_set_focus_calls(component, diag);
        focus_item::determine_initial_focus_item(component, diag);
        focus_item::erase_forward_focus_properties(component);
    }

    ensure_window::ensure_window(root_component, &doc.local_registry);

    for component in (root_component.used_types.borrow().sub_components.iter())
        .chain(std::iter::once(root_component))
    {
        apply_default_properties_from_style::apply_default_properties_from_style(
            component,
            &mut type_loader,
            diag,
        )
        .await;
        flickable::handle_flickable(component, &global_type_registry.borrow());
        lower_states::lower_states(component, &doc.local_registry, diag);
        repeater_component::process_repeater_components(component);
        lower_popups::lower_popups(component, &doc.local_registry, diag);
        lower_layout::lower_layouts(component, &mut type_loader, diag).await;
        z_order::reorder_by_z_order(component, diag);
        lower_shadows::lower_shadow_properties(component, &doc.local_registry, diag);
        clip::handle_clip(component, &global_type_registry.borrow(), diag);
        transform_and_opacity::handle_transform_and_opacity(
            component,
            &global_type_registry.borrow(),
            diag,
        );
        default_geometry::default_geometry(component, diag);
        visible::handle_visible(component, &global_type_registry.borrow());
        materialize_fake_properties::materialize_fake_properties(component);
    }
    collect_globals::collect_globals(&doc, diag);

    if compiler_config.inline_all_elements {
        inlining::inline(doc, inlining::InlineSelection::InlineAllComponents);
        root_component.used_types.borrow_mut().sub_components.clear();
    }

    binding_analysis::binding_analysis(doc, diag);
    unique_id::assign_unique_id(doc);

    for component in (root_component.used_types.borrow().sub_components.iter())
        .chain(std::iter::once(root_component))
    {
        deduplicate_property_read::deduplicate_property_read(component);
        optimize_useless_rectangles::optimize_useless_rectangles(component);
        move_declarations::move_declarations(component, diag);
        remove_aliases::remove_aliases(component, diag);
        if !diag.has_error() {
            // binding loop causes panics in const_propagation
            const_propagation::const_propagation(component);
        }
    }

    for component in (root_component.used_types.borrow().sub_components.iter())
        .chain(std::iter::once(root_component))
    {
        resolve_native_classes::resolve_native_classes(component);
        remove_unused_properties::remove_unused_properties(component);
    }

    collect_structs::collect_structs(&doc);

    for component in (root_component.used_types.borrow().sub_components.iter())
        .chain(std::iter::once(root_component))
    {
        generate_item_indices::generate_item_indices(component);
    }

    collect_custom_fonts::collect_custom_fonts(
        root_component,
        std::iter::once(&*doc).chain(type_loader.all_documents()),
        compiler_config.embed_resources,
    );
    root_component.is_root_component.set(true);
}

/// Run the passes on imported documents
pub fn run_import_passes(
    doc: &crate::object_tree::Document,
    type_loader: &crate::typeloader::TypeLoader<'_>,
    diag: &mut crate::diagnostics::BuildDiagnostics,
) {
    infer_aliases_types::resolve_aliases(doc, diag);
    resolving::resolve_expressions(doc, type_loader, diag);
    check_expressions::check_expressions(doc, diag);
    unique_id::check_unique_id(doc, diag);
}
