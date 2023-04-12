// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

mod apply_default_properties_from_style;
mod binding_analysis;
mod check_expressions;
mod check_public_api;
mod check_rotation;
mod clip;
mod collect_custom_fonts;
mod collect_globals;
mod collect_init_code;
mod collect_structs;
mod collect_subcomponents;
mod compile_paths;
mod const_propagation;
mod deduplicate_property_read;
mod default_geometry;
#[cfg(feature = "software-renderer")]
mod embed_glyphs;
mod embed_images;
mod ensure_window;
mod flickable;
mod focus_item;
pub mod generate_item_indices;
pub mod infer_aliases_types;
mod inlining;
mod lower_accessibility;
mod lower_layout;
mod lower_popups;
mod lower_property_to_element;
mod lower_shadows;
mod lower_states;
mod lower_tabwidget;
mod lower_text_input_interface;
mod materialize_fake_properties;
mod move_declarations;
mod optimize_useless_rectangles;
mod purity_check;
mod remove_aliases;
mod remove_unused_properties;
mod repeater_component;
pub mod resolve_native_classes;
pub mod resolving;
mod unique_id;
mod visible;
mod z_order;

use crate::expression_tree::Expression;
use crate::langtype::ElementType;
use crate::namedreference::NamedReference;
use std::rc::Rc;

pub async fn run_passes(
    doc: &crate::object_tree::Document,
    diag: &mut crate::diagnostics::BuildDiagnostics,
    type_loader: &mut crate::typeloader::TypeLoader,
    compiler_config: &crate::CompilerConfiguration,
) {
    if matches!(
        doc.root_component.root_element.borrow().base_type,
        ElementType::Error | ElementType::Global
    ) {
        // If there isn't a root component, we shouldn't do any of these passes
        return;
    }

    let style_metrics = {
        // Ignore import errors
        let mut build_diags_to_ignore = crate::diagnostics::BuildDiagnostics::default();
        type_loader
            .import_component("std-widgets.slint", "StyleMetrics", &mut build_diags_to_ignore)
            .await
            .unwrap_or_else(|| panic!("can't load style metrics"))
    };

    let global_type_registry = type_loader.global_type_registry.clone();
    let root_component = &doc.root_component;
    run_import_passes(doc, type_loader, diag);
    check_public_api::check_public_api(doc, diag);

    collect_subcomponents::collect_subcomponents(root_component);
    for component in (root_component.used_types.borrow().sub_components.iter())
        .chain(std::iter::once(root_component))
    {
        compile_paths::compile_paths(component, &doc.local_registry, diag);
        lower_tabwidget::lower_tabwidget(component, type_loader, diag).await;
        apply_default_properties_from_style::apply_default_properties_from_style(
            component,
            &style_metrics,
            diag,
        );
        lower_states::lower_states(component, &doc.local_registry, diag);
        lower_text_input_interface::lower_text_input_interface(component);
    }

    inlining::inline(doc, inlining::InlineSelection::InlineOnlyRequiredComponents);
    collect_subcomponents::collect_subcomponents(root_component);

    for component in (root_component.used_types.borrow().sub_components.iter())
        .chain(std::iter::once(root_component))
    {
        focus_item::resolve_element_reference_in_set_focus_calls(component, diag);
        if Rc::ptr_eq(component, root_component) {
            focus_item::determine_initial_focus_item(component, diag);
        }
        focus_item::erase_forward_focus_properties(component);
    }

    ensure_window::ensure_window(root_component, &doc.local_registry, &style_metrics);

    for component in (root_component.used_types.borrow().sub_components.iter())
        .chain(std::iter::once(root_component))
    {
        flickable::handle_flickable(component, &global_type_registry.borrow());
        repeater_component::process_repeater_components(component);
        lower_popups::lower_popups(component, &doc.local_registry, diag);
        lower_layout::lower_layouts(component, type_loader, diag).await;
        default_geometry::default_geometry(component, diag);
        z_order::reorder_by_z_order(component, diag);
        lower_property_to_element::lower_property_to_element(
            component,
            "opacity",
            core::iter::empty(),
            None,
            "Opacity",
            &global_type_registry.borrow(),
            diag,
        );
        lower_property_to_element::lower_property_to_element(
            component,
            "cache-rendering-hint",
            core::iter::empty(),
            None,
            "Layer",
            &global_type_registry.borrow(),
            diag,
        );
        visible::handle_visible(component, &global_type_registry.borrow(), diag);
        lower_shadows::lower_shadow_properties(component, &doc.local_registry, diag);
        lower_property_to_element::lower_property_to_element(
            component,
            crate::typeregister::RESERVED_ROTATION_PROPERTIES[0].0,
            crate::typeregister::RESERVED_ROTATION_PROPERTIES[1..]
                .iter()
                .map(|(prop_name, _)| *prop_name),
            Some(&|e, prop| Expression::BinaryExpression {
                lhs: Expression::PropertyReference(NamedReference::new(
                    e,
                    match prop {
                        "rotation-origin-x" => "width",
                        "rotation-origin-y" => "height",
                        "rotation-angle" => return Expression::Invalid,
                        _ => unreachable!(),
                    },
                ))
                .into(),
                op: '/',
                rhs: Expression::NumberLiteral(2., Default::default()).into(),
            }),
            "Rotate",
            &global_type_registry.borrow(),
            diag,
        );
        clip::handle_clip(component, &global_type_registry.borrow(), diag);
        if compiler_config.accessibility {
            lower_accessibility::lower_accessibility_properties(component, diag);
        }
        collect_init_code::collect_init_code(component);
        materialize_fake_properties::materialize_fake_properties(component);
    }
    collect_globals::collect_globals(doc, diag);

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
        move_declarations::move_declarations(component);
    }

    remove_aliases::remove_aliases(doc, diag);

    for component in (root_component.used_types.borrow().sub_components.iter())
        .chain(std::iter::once(root_component))
    {
        if !diag.has_error() {
            // binding loop causes panics in const_propagation
            const_propagation::const_propagation(component);
        }
        resolve_native_classes::resolve_native_classes(component);
        remove_unused_properties::remove_unused_properties(component);
    }

    collect_structs::collect_structs(doc);

    for component in (root_component.used_types.borrow().sub_components.iter())
        .chain(std::iter::once(root_component))
    {
        generate_item_indices::generate_item_indices(component);
    }

    // collect globals once more: After optimizations we might have less globals
    collect_globals::collect_globals(doc, diag);

    embed_images::embed_images(
        root_component,
        compiler_config.embed_resources,
        compiler_config.scale_factor,
        diag,
    );

    match compiler_config.embed_resources {
        #[cfg(feature = "software-renderer")]
        crate::EmbedResourcesKind::EmbedTextures => {
            let mut characters_seen = std::collections::HashSet::new();

            // Include at least the default font sizes used in the MCU backend
            let mut font_pixel_sizes = vec![(12. * compiler_config.scale_factor) as i16];
            for component in (root_component.used_types.borrow().sub_components.iter())
                .chain(std::iter::once(root_component))
            {
                embed_glyphs::collect_font_sizes_used(
                    component,
                    compiler_config.scale_factor,
                    &mut font_pixel_sizes,
                );
                embed_glyphs::scan_string_literals(component, &mut characters_seen);
            }

            embed_glyphs::embed_glyphs(
                root_component,
                compiler_config.scale_factor,
                font_pixel_sizes,
                characters_seen,
                std::iter::once(&*doc).chain(type_loader.all_documents()),
                diag,
            );
        }
        _ => {
            // Create font registration calls for custom fonts, unless we're embedding pre-rendered glyphs
            collect_custom_fonts::collect_custom_fonts(
                root_component,
                std::iter::once(&*doc).chain(type_loader.all_documents()),
                compiler_config.embed_resources == crate::EmbedResourcesKind::EmbedAllResources,
            );
        }
    }

    root_component.is_root_component.set(true);
}

/// Run the passes on imported documents
pub fn run_import_passes(
    doc: &crate::object_tree::Document,
    type_loader: &crate::typeloader::TypeLoader,
    diag: &mut crate::diagnostics::BuildDiagnostics,
) {
    infer_aliases_types::resolve_aliases(doc, diag);
    resolving::resolve_expressions(doc, type_loader, diag);
    check_expressions::check_expressions(doc, diag);
    purity_check::purity_check(doc, diag);
    check_rotation::check_rotation(doc, diag);
    unique_id::check_unique_id(doc, diag);
}
