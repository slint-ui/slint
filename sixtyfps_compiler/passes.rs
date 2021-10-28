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

    let components = root_component
        .used_types
        .borrow()
        .sub_components
        .iter()
        .cloned()
        .chain(std::iter::once(root_component.clone()))
        .collect::<Vec<_>>();

    for component in &components {
        compile_paths::compile_paths(component, &doc.local_registry, diag);
        lower_tabwidget::lower_tabwidget(component, &mut type_loader, diag).await;
    }

    embed_images::embed_images(root_component, compiler_config.embed_resources, diag);

    inlining::inline(doc, inlining::InlineSelection::InlineOnlyRequiredComponents);

    for component in &components {
        focus_item::resolve_element_reference_in_set_focus_calls(component, diag);
        focus_item::determine_initial_focus_item(component, diag);
        focus_item::erase_forward_focus_properties(component);
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
        materialize_fake_properties::materialize_fake_properties(component);
        apply_default_properties_from_style::apply_default_properties_from_style(
            component,
            &mut type_loader,
            diag,
        )
        .await;
        ensure_window::ensure_window(component, &doc.local_registry);
        unique_id::assign_unique_id(component);
        collect_globals::collect_globals(&doc, diag);

        binding_analysis::binding_analysis(component, diag);
    }

    deduplicate_property_read::deduplicate_property_read(root_component);
    optimize_useless_rectangles::optimize_useless_rectangles(root_component);
    move_declarations::move_declarations(root_component, diag);
    remove_aliases::remove_aliases(root_component, diag);
    resolve_native_classes::resolve_native_classes(root_component);
    remove_unused_properties::remove_unused_properties(root_component);

    collect_structs::collect_structs(root_component, diag);
    generate_item_indices::generate_item_indices(root_component);
    collect_custom_fonts::collect_custom_fonts(
        root_component,
        std::iter::once(&*doc).chain(type_loader.all_documents()),
        compiler_config.embed_resources,
    );
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
