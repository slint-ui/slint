// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

mod apply_default_properties_from_style;
mod binding_analysis;
mod border_radius;
mod check_expressions;
mod check_public_api;
mod check_rotation;
mod clip;
mod collect_custom_fonts;
mod collect_globals;
mod collect_init_code;
mod collect_structs_and_enums;
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
mod focus_handling;
pub mod generate_item_indices;
pub mod infer_aliases_types;
mod inject_debug_hooks;
mod inlining;
mod lower_absolute_coordinates;
mod lower_accessibility;
mod lower_component_container;
mod lower_layout;
mod lower_menus;
mod lower_platform;
mod lower_popups;
mod lower_property_to_element;
mod lower_shadows;
mod lower_states;
mod lower_tabwidget;
mod lower_text_input_interface;
mod lower_timers;
pub mod materialize_fake_properties;
pub mod move_declarations;
mod optimize_useless_rectangles;
mod purity_check;
mod remove_aliases;
mod remove_return;
mod remove_unused_properties;
mod repeater_component;
pub mod resolve_native_classes;
pub mod resolving;
mod unique_id;
mod visible;
mod z_order;

use crate::expression_tree::Expression;
use crate::namedreference::NamedReference;
use smol_str::SmolStr;

pub fn ignore_debug_hooks(expr: &Expression) -> &Expression {
    let mut expr = expr;
    loop {
        match expr {
            Expression::DebugHook { expression, .. } => expr = expression.as_ref(),
            _ => return expr,
        }
    }
}

pub async fn run_passes(
    doc: &mut crate::object_tree::Document,
    type_loader: &mut crate::typeloader::TypeLoader,
    keep_raw: bool,
    diag: &mut crate::diagnostics::BuildDiagnostics,
) -> Option<crate::typeloader::TypeLoader> {
    let style_metrics = {
        // Ignore import errors
        let mut build_diags_to_ignore = crate::diagnostics::BuildDiagnostics::default();
        type_loader
            .import_component("std-widgets.slint", "StyleMetrics", &mut build_diags_to_ignore)
            .await
            .unwrap_or_else(|| panic!("can't load style metrics"))
    };

    let palette = {
        // Ignore import errors
        let mut build_diags_to_ignore = crate::diagnostics::BuildDiagnostics::default();
        type_loader
            .import_component("std-widgets.slint", "Palette", &mut build_diags_to_ignore)
            .await
            .unwrap_or_else(|| panic!("can't load palette"))
    };

    let global_type_registry = type_loader.global_type_registry.clone();

    run_import_passes(doc, type_loader, diag);
    check_public_api::check_public_api(doc, &type_loader.compiler_config, diag);

    let raw_type_loader =
        keep_raw.then(|| crate::typeloader::snapshot_with_extra_doc(type_loader, doc).unwrap());

    collect_subcomponents::collect_subcomponents(doc);
    lower_tabwidget::lower_tabwidget(doc, type_loader, diag).await;
    lower_menus::lower_menus(doc, type_loader, diag).await;
    lower_component_container::lower_component_container(doc, type_loader, diag);
    collect_subcomponents::collect_subcomponents(doc);

    doc.visit_all_used_components(|component| {
        apply_default_properties_from_style::apply_default_properties_from_style(
            component,
            &style_metrics,
            &palette,
            diag,
        );
        lower_states::lower_states(component, &doc.local_registry, diag);
        lower_text_input_interface::lower_text_input_interface(component);
        compile_paths::compile_paths(
            component,
            &doc.local_registry,
            type_loader.compiler_config.embed_resources,
            diag,
        );
        repeater_component::process_repeater_components(component);
        lower_popups::lower_popups(component, &doc.local_registry, diag);
        collect_init_code::collect_init_code(component);
        lower_timers::lower_timers(component, diag);
    });

    inlining::inline(doc, inlining::InlineSelection::InlineOnlyRequiredComponents, diag);
    collect_subcomponents::collect_subcomponents(doc);

    for root_component in doc.exported_roots() {
        focus_handling::call_focus_on_init(&root_component);
        ensure_window::ensure_window(&root_component, &doc.local_registry, &style_metrics, diag);
    }
    if let Some(popup_menu_impl) = &doc.popup_menu_impl {
        focus_handling::call_focus_on_init(popup_menu_impl);
    }

    doc.visit_all_used_components(|component| {
        border_radius::handle_border_radius(component, diag);
        flickable::handle_flickable(component, &global_type_registry.borrow());
        lower_layout::lower_layouts(component, type_loader, &style_metrics, diag);
        default_geometry::default_geometry(component, diag);
        lower_absolute_coordinates::lower_absolute_coordinates(component);
        z_order::reorder_by_z_order(component, diag);
        lower_property_to_element::lower_property_to_element(
            component,
            "opacity",
            core::iter::empty(),
            None,
            &SmolStr::new_static("Opacity"),
            &global_type_registry.borrow(),
            diag,
        );
        lower_property_to_element::lower_property_to_element(
            component,
            "cache-rendering-hint",
            core::iter::empty(),
            None,
            &SmolStr::new_static("Layer"),
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
                        "rotation-origin-x" => SmolStr::new_static("width"),
                        "rotation-origin-y" => SmolStr::new_static("height"),
                        "rotation-angle" => return Expression::Invalid,
                        _ => unreachable!(),
                    },
                ))
                .into(),
                op: '/',
                rhs: Expression::NumberLiteral(2., Default::default()).into(),
            }),
            &SmolStr::new_static("Rotate"),
            &global_type_registry.borrow(),
            diag,
        );
        clip::handle_clip(component, &global_type_registry.borrow(), diag);
        if type_loader.compiler_config.accessibility {
            lower_accessibility::lower_accessibility_properties(component, diag);
        }
        materialize_fake_properties::materialize_fake_properties(component);
    });
    for root_component in doc.exported_roots() {
        lower_layout::check_window_layout(&root_component);
    }
    collect_globals::collect_globals(doc, diag);

    if type_loader.compiler_config.inline_all_elements {
        inlining::inline(doc, inlining::InlineSelection::InlineAllComponents, diag);
        doc.used_types.borrow_mut().sub_components.clear();
    }

    binding_analysis::binding_analysis(doc, &type_loader.compiler_config, diag);
    unique_id::assign_unique_id(doc);

    doc.visit_all_used_components(|component| {
        lower_platform::lower_platform(component, type_loader);

        // Don't perform the empty rectangle removal when debug info is requested, because the resulting
        // item tree ends up with a hierarchy where certain items have children that aren't child elements
        // but siblings or sibling children. We need a new data structure to perform a correct element tree
        // traversal.
        if !type_loader.compiler_config.debug_info {
            optimize_useless_rectangles::optimize_useless_rectangles(component);
        }
        move_declarations::move_declarations(component);
    });

    remove_aliases::remove_aliases(doc, diag);
    remove_return::remove_return(doc);

    doc.visit_all_used_components(|component| {
        if !diag.has_errors() {
            // binding loop causes panics in const_propagation
            const_propagation::const_propagation(component);
        }
        deduplicate_property_read::deduplicate_property_read(component);
        if !component.is_global() {
            resolve_native_classes::resolve_native_classes(component);
        }
    });

    remove_unused_properties::remove_unused_properties(doc);
    // collect globals once more: After optimizations we might have less globals
    collect_globals::collect_globals(doc, diag);
    collect_structs_and_enums::collect_structs_and_enums(doc);

    doc.visit_all_used_components(|component| {
        if !component.is_global() {
            generate_item_indices::generate_item_indices(component);
        }
    });

    embed_images::embed_images(
        doc,
        type_loader.compiler_config.embed_resources,
        type_loader.compiler_config.const_scale_factor,
        &type_loader.compiler_config.resource_url_mapper,
        diag,
    )
    .await;

    #[cfg(feature = "bundle-translations")]
    if let Some(path) = &type_loader.compiler_config.translation_path_bundle {
        match crate::translations::TranslationsBuilder::load_translations(
            path,
            type_loader.compiler_config.translation_domain.as_deref().unwrap_or(""),
        ) {
            Ok(builder) => {
                doc.translation_builder = Some(builder);
            }
            Err(err) => {
                diag.push_error(
                    format!("Cannot load bundled translation: {err}"),
                    doc.node.as_ref().expect("Unexpected empty document"),
                );
            }
        }
    }

    match type_loader.compiler_config.embed_resources {
        #[cfg(feature = "software-renderer")]
        crate::EmbedResourcesKind::EmbedTextures => {
            let mut characters_seen = std::collections::HashSet::new();

            // Include at least the default font sizes used in the MCU backend
            let mut font_pixel_sizes =
                vec![(12. * type_loader.compiler_config.const_scale_factor) as i16];
            doc.visit_all_used_components(|component| {
                embed_glyphs::collect_font_sizes_used(
                    component,
                    type_loader.compiler_config.const_scale_factor,
                    &mut font_pixel_sizes,
                );
                embed_glyphs::scan_string_literals(component, &mut characters_seen);
            });

            // This is not perfect, as this includes translations that may not be used.
            #[cfg(feature = "bundle-translations")]
            if let Some(translation_builder) = doc.translation_builder.as_ref() {
                translation_builder.collect_characters_seen(&mut characters_seen);
            }

            embed_glyphs::embed_glyphs(
                doc,
                &type_loader.compiler_config,
                font_pixel_sizes,
                characters_seen,
                std::iter::once(&*doc).chain(type_loader.all_documents()),
                diag,
            );
        }
        _ => {
            // Create font registration calls for custom fonts, unless we're embedding pre-rendered glyphs
            collect_custom_fonts::collect_custom_fonts(
                doc,
                std::iter::once(&*doc).chain(type_loader.all_documents()),
                type_loader.compiler_config.embed_resources
                    == crate::EmbedResourcesKind::EmbedAllResources,
            );
        }
    };

    raw_type_loader
}

/// Run the passes on imported documents
pub fn run_import_passes(
    doc: &crate::object_tree::Document,
    type_loader: &crate::typeloader::TypeLoader,
    diag: &mut crate::diagnostics::BuildDiagnostics,
) {
    inject_debug_hooks::inject_debug_hooks(doc, type_loader);
    infer_aliases_types::resolve_aliases(doc, diag);
    resolving::resolve_expressions(doc, type_loader, diag);
    purity_check::purity_check(doc, diag);
    focus_handling::replace_forward_focus_bindings_with_focus_functions(doc, diag);
    check_expressions::check_expressions(doc, diag);
    check_rotation::check_rotation(doc, diag);
    unique_id::check_unique_id(doc, diag);
}
