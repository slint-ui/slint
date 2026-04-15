// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Internal compiled-component / running-instance types used by
//! [`crate::api::ComponentDefinition`] and [`crate::api::ComponentInstance`].
//! The public API wraps these thin structs so downstream callers never
//! see the compilation-unit surface directly.

use crate::Value;
use crate::instance::Instance;
use crate::public_api;
use i_slint_compiler::langtype::Type as LangType;
use i_slint_compiler::llr::{CompilationUnit, GlobalComponent};
use i_slint_compiler::object_tree::PropertyVisibility;
use i_slint_compiler::parser::normalize_identifier;
use i_slint_core::item_tree::ItemTreeVTable;
use smol_str::SmolStr;
use std::rc::Rc;
use vtable::VRc;

/// Pair of `TypeLoader`s retained alongside a compiled component for
/// internal tooling (highlight, live preview, LSP).
///
/// `type_loader` is the post-pass state — the compiler's lowered object
/// tree, what `highlight.rs` walks to resolve elements to runtime items.
/// `raw_type_loader` is a snapshot taken *before* most passes ran — what
/// the LSP hands to `common::DocumentCache::new_from_raw_parts` so the
/// library / outline / properties / preview-data panels see the tree as
/// the user wrote it, not the compiler-transformed form. Neither can be
/// derived from the other; passes are destructive.
#[derive(Clone, Default)]
pub struct TypeLoaders {
    #[cfg_attr(not(any(feature = "internal", feature = "internal-highlight")), allow(dead_code))]
    pub type_loader: Option<std::rc::Rc<i_slint_compiler::typeloader::TypeLoader>>,
    #[cfg_attr(not(feature = "internal-highlight"), allow(dead_code))]
    pub raw_type_loader: Option<std::rc::Rc<i_slint_compiler::typeloader::TypeLoader>>,
}

/// Compiled component, one per exported public component in the
/// source file. Produced by [`build_from_source`] and held behind
/// [`crate::api::ComponentDefinition`].
#[derive(Clone)]
pub struct ComponentDefinitionInner {
    pub compilation_unit: Rc<CompilationUnit>,
    pub public_index: usize,
    /// `None` on both sides when the definition was reconstructed from a
    /// running instance that doesn't carry a `TypeLoader` reference back.
    pub type_loaders: TypeLoaders,
    /// Debug handler set by the LSP. Propagated to instances on creation.
    #[cfg_attr(not(feature = "internal"), allow(dead_code))]
    pub debug_handler: std::cell::RefCell<
        Option<Rc<dyn Fn(Option<&i_slint_compiler::diagnostics::SourceLocation>, &str)>>,
    >,
}

impl ComponentDefinitionInner {
    pub fn name(&self) -> &str {
        self.public().name.as_str()
    }

    /// Instantiate the component.
    pub fn create(&self) -> ComponentInstanceInner {
        let vrc = Instance::new_with_window(
            self.compilation_unit.clone(),
            self.public_index,
            None,
            self.type_loaders.clone(),
        );
        if let Some(handler) = self.debug_handler.borrow().clone() {
            *vrc.debug_handler.borrow_mut() = Some(handler);
        }
        ComponentInstanceInner(vrc)
    }

    /// Instantiate the component, reusing the given `WindowAdapter` instead
    /// of creating a fresh one via the backend selector.
    pub fn create_with_existing_window(
        &self,
        window_adapter: i_slint_core::window::WindowAdapterRc,
    ) -> ComponentInstanceInner {
        let vrc = Instance::new_with_window(
            self.compilation_unit.clone(),
            self.public_index,
            Some(window_adapter),
            self.type_loaders.clone(),
        );
        ComponentInstanceInner(vrc)
    }

    /// Instantiate the component and embed it at `parent_item_tree_index`
    /// in the given outer item tree. Used by rust-codegen's
    /// `ComponentFactory` path to embed an interpreter-built component
    /// inside a natively compiled one.
    pub fn create_embedded(
        &self,
        parent: vtable::VWeak<ItemTreeVTable>,
        parent_item_tree_index: u32,
    ) -> ComponentInstanceInner {
        let vrc = Instance::new_embedded(
            self.compilation_unit.clone(),
            self.public_index,
            self.type_loaders.clone(),
            parent,
            parent_item_tree_index,
        );
        ComponentInstanceInner(vrc)
    }

    fn public(&self) -> &i_slint_compiler::llr::PublicComponent {
        &self.compilation_unit.public_components[self.public_index]
    }

    fn properties_with_info(
        &self,
    ) -> impl Iterator<Item = (SmolStr, LangType, PropertyVisibility)> + '_ {
        public_properties_info(&self.public().public_properties)
    }

    /// Iterator of `(name, type, visibility)` for every property, callback and
    /// function declared on this component. Exposed through the `internal`
    /// feature; the public `ComponentDefinition::properties()` / `callbacks()`
    /// / `functions()` helpers filter on top of it.
    #[cfg_attr(not(feature = "internal"), allow(dead_code))]
    pub fn properties_and_callbacks(
        &self,
    ) -> impl Iterator<Item = (SmolStr, LangType, PropertyVisibility)> + '_ {
        self.properties_with_info()
    }

    /// Iterator of `(name, type)` limited to property-typed declarations
    /// (excludes callbacks and functions).
    pub fn properties(&self) -> impl Iterator<Item = (SmolStr, LangType)> + '_ {
        self.properties_with_info()
            .filter(|(_, ty, _)| ty.is_property_type())
            .map(|(n, ty, _)| (n, ty))
    }

    pub fn callbacks(&self) -> impl Iterator<Item = SmolStr> + '_ {
        self.properties_with_info()
            .filter(|(_, ty, _)| matches!(ty, LangType::Callback(_)))
            .map(|(n, _, _)| n)
    }

    pub fn functions(&self) -> impl Iterator<Item = SmolStr> + '_ {
        self.properties_with_info()
            .filter(|(_, ty, _)| matches!(ty, LangType::Function(_)))
            .map(|(n, _, _)| n)
    }

    /// Names of every exported global declared by the compilation unit.
    /// The interpreter surfaces aliases first and the
    /// canonical component name last; mirror that order so the public API
    /// observes the same listing.
    pub fn globals(&self) -> impl Iterator<Item = SmolStr> + '_ {
        self.compilation_unit
            .globals
            .iter()
            .filter(|g| visible_in_public_api(g))
            .flat_map(|g| g.aliases.iter().cloned().chain(std::iter::once(g.name.clone())))
    }

    fn global_by_name(&self, name: &str) -> Option<&GlobalComponent> {
        // Names on `GlobalComponent` preserve whatever form the compiler
        // stored (often source-form with dashes), so normalize both sides.
        let needle = normalize_identifier(name);
        self.compilation_unit.globals.iter().filter(|g| visible_in_public_api(g)).find(|g| {
            normalize_identifier(&g.name) == needle
                || g.aliases.iter().any(|a| normalize_identifier(a) == needle)
        })
    }

    pub fn global_properties_and_callbacks(
        &self,
        name: &str,
    ) -> Option<impl Iterator<Item = (SmolStr, LangType, PropertyVisibility)> + '_> {
        self.global_by_name(name).map(|g| public_properties_info(&g.public_properties))
    }

    pub fn global_properties(
        &self,
        name: &str,
    ) -> Option<impl Iterator<Item = (SmolStr, LangType)> + '_> {
        self.global_properties_and_callbacks(name)
            .map(|it| it.filter(|(_, ty, _)| ty.is_property_type()).map(|(n, ty, _)| (n, ty)))
    }

    pub fn global_callbacks(&self, name: &str) -> Option<impl Iterator<Item = SmolStr> + '_> {
        self.global_properties_and_callbacks(name).map(|it| {
            it.filter(|(_, ty, _)| matches!(ty, LangType::Callback(_))).map(|(n, _, _)| n)
        })
    }

    pub fn global_functions(&self, name: &str) -> Option<impl Iterator<Item = SmolStr> + '_> {
        self.global_properties_and_callbacks(name).map(|it| {
            it.filter(|(_, ty, _)| matches!(ty, LangType::Function(_))).map(|(n, _, _)| n)
        })
    }
}

fn public_properties_info<'a>(
    public_properties: &'a i_slint_compiler::llr::PublicProperties,
) -> impl Iterator<Item = (SmolStr, LangType, PropertyVisibility)> + 'a {
    // Return the source-form identifier (dashes preserved) so the
    // public API matches the names as written in the `.slint` file.
    public_properties.values().map(|p| (p.display_name.clone(), p.ty.clone(), p.visibility))
}

fn visible_in_public_api(g: &GlobalComponent) -> bool {
    // A builtin global has no public surface of its own in the API.
    g.exported && !g.is_builtin
}

/// Live instance of a compiled component.
pub struct ComponentInstanceInner(pub VRc<ItemTreeVTable, Instance>);

impl Clone for ComponentInstanceInner {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl ComponentInstanceInner {
    /// Access the underlying vtable VRc so host code can downgrade to a weak
    /// reference or forward it to the window adapter.
    pub fn vrc(&self) -> &VRc<ItemTreeVTable, Instance> {
        &self.0
    }

    pub fn get_property(&self, name: &str) -> Option<Value> {
        public_api::get(&self.0, name)
    }

    pub fn set_property(
        &self,
        name: &str,
        value: Value,
    ) -> Result<(), crate::api::SetPropertyError> {
        public_api::set(&self.0, name, value)
    }

    pub fn invoke(&self, name: &str, args: &[Value]) -> Option<Value> {
        public_api::invoke(&self.0, name, args)
    }

    pub fn set_callback(
        &self,
        name: &str,
        handler: impl Fn(&[Value]) -> Value + 'static,
    ) -> Result<(), ()> {
        public_api::set_callback(&self.0, name, Box::new(handler))
    }

    pub fn get_global_property(&self, global: &str, property: &str) -> Option<Value> {
        public_api::get_global(&self.0, global, property)
    }

    pub fn set_global_property(
        &self,
        global: &str,
        property: &str,
        value: Value,
    ) -> Result<(), crate::api::SetPropertyError> {
        public_api::set_global(&self.0, global, property, value)
    }

    pub fn set_global_callback(
        &self,
        global: &str,
        name: &str,
        handler: impl Fn(&[Value]) -> Value + 'static,
    ) -> Result<(), ()> {
        public_api::set_global_callback(&self.0, global, name, Box::new(handler))
    }

    pub fn invoke_global(&self, global: &str, name: &str, args: &[Value]) -> Option<Value> {
        public_api::invoke_global(&self.0, global, name, args)
    }

    /// Return a borrowed reference to the window adapter, creating one
    /// through the backend selector if necessary. The returned reference
    /// lives as long as the instance.
    pub fn window_adapter_ref(
        &self,
    ) -> Result<&i_slint_core::window::WindowAdapterRc, i_slint_core::api::PlatformError> {
        self.0.window_adapter_or_default().ok_or_else(|| {
            i_slint_core::api::PlatformError::Other(
                "Could not obtain a window adapter for the interpreter instance".into(),
            )
        })?;
        Ok(self.0.window_adapter.get().expect("window_adapter just initialized above"))
    }

    /// Definition this instance was created from.
    pub fn definition(&self) -> ComponentDefinitionInner {
        let public_index = self.0.public_component_index.unwrap_or(0);
        ComponentDefinitionInner {
            compilation_unit: self.0.root_sub_component.compilation_unit.clone(),
            public_index,
            type_loaders: self.0.type_loaders.clone(),
            debug_handler: std::cell::RefCell::new(self.0.debug_handler.borrow().clone()),
        }
    }
}

/// Lower a compiled `Document` to a `CompilationUnit` and wrap each public
/// component in a `ComponentDefinitionInner`.
pub fn build_from_document(
    document: &i_slint_compiler::object_tree::Document,
    compiler_config: &i_slint_compiler::CompilerConfiguration,
    type_loaders: TypeLoaders,
) -> Vec<ComponentDefinitionInner> {
    let unit = Rc::new(i_slint_compiler::llr::lower_to_item_tree::lower_to_item_tree(
        document,
        compiler_config,
    ));
    (0..unit.public_components.len())
        .map(|public_index| ComponentDefinitionInner {
            compilation_unit: unit.clone(),
            public_index,
            type_loaders: type_loaders.clone(),
            debug_handler: Default::default(),
        })
        .collect()
}

/// Compile a `.slint` source string.
///
/// Returns the diagnostics plus a map of public component name →
/// `ComponentDefinitionInner` for each exported root in the document.
pub async fn build_from_source(
    source_code: String,
    path: std::path::PathBuf,
    mut config: i_slint_compiler::CompilerConfiguration,
) -> (
    Vec<i_slint_compiler::diagnostics::Diagnostic>,
    std::collections::HashMap<String, ComponentDefinitionInner>,
) {
    // Element inlining is off by default: the interpreter wants to
    // preserve sub-components so that
    // `@children` and friends are resolved at runtime via the item-tree rather
    // than at compile time. Inlining is left on only if the caller explicitly
    // forced it via `SLINT_INLINING`.
    if std::env::var_os("SLINT_INLINING").is_none() {
        config.inline_all_elements = false;
    }
    // Populate the LLR debug-info side table so highlight/live-preview can
    // map source-level elements back to runtime items.
    config.debug_info = true;
    let diag = i_slint_compiler::diagnostics::BuildDiagnostics::default();
    let (path, mut diag, loader, raw_loader) =
        i_slint_compiler::load_root_file_with_raw_type_loader(
            &path,
            &path,
            source_code,
            diag,
            config.clone(),
        )
        .await;
    if diag.has_errors() {
        return (diag.into_iter().collect(), Default::default());
    }
    let type_loader = std::rc::Rc::new(loader);
    let type_loaders = TypeLoaders {
        type_loader: Some(type_loader.clone()),
        raw_type_loader: raw_loader.map(std::rc::Rc::new),
    };
    let doc = match type_loader.get_document(&path) {
        Some(doc) => doc,
        None => return (diag.into_iter().collect(), Default::default()),
    };
    let mut components = std::collections::HashMap::new();
    for def in build_from_document(doc, &config, type_loaders) {
        components.insert(def.name().to_string(), def);
    }
    if components.is_empty() {
        diag.push_error_with_span("No component found".into(), Default::default());
    }
    (diag.into_iter().collect(), components)
}
