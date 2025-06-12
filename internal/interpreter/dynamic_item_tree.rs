// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::api::{CompilationResult, ComponentDefinition, Value};
use crate::global_component::CompiledGlobalCollection;
use crate::{dynamic_type, eval};
use core::ptr::NonNull;
use dynamic_type::{Instance, InstanceBox};
use i_slint_compiler::expression_tree::{Expression, NamedReference};
use i_slint_compiler::langtype::Type;
use i_slint_compiler::object_tree::{ElementRc, TransitionDirection};
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
    /// Whether this is a `if` or a `for`
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

    /// Instantiate a runtime ItemTree from this ItemTreeDescription
    pub fn create(
        self: Rc<Self>,
        options: WindowOptions,
    ) -> Result<DynamicComponentVRc, PlatformError> {
        i_slint_backend_selector::with_platform(|_b| {
            // Nothing to do, just make sure a backend was created
            Ok(())
        })?;

        let instance = instantiate(self, None, None, Some(&options), Default::default());
        if let WindowOptions::UseExistingWindow(existing_adapter) = options {
            WindowInner::from_pub(existing_adapter.window())
                .set_component(&vtable::VRc::into_dyn(instance.clone()));
        }
        instance.run_setup_code();
        Ok(instance)
    }

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

/// Create a ItemTreeDescription from a source.
/// The path corresponding to the source need to be passed as well (path is used for diagnostics
/// and loading relative assets)
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
    #[cfg(feature = "internal-highlight")]
    let (path, mut diag, loader, raw_type_loader) =
        i_slint_compiler::load_root_file_with_raw_type_loader(
            &path,
            &path,
            source,
            diag,
            compiler_config,
        )
        .await;
    #[cfg(not(feature = "internal-highlight"))]
    let (path, mut diag, loader) =
        i_slint_compiler::load_root_file(&path, &path, source, diag, compiler_config).await;
    if diag.has_errors() {
        return CompilationResult {
            components: HashMap::new(),
            diagnostics: diag.into_iter().collect(),
            #[cfg(feature = "internal")]
            structs_and_enums: Vec::new(),
            #[cfg(feature = "internal")]
            named_exports: Vec::new(),
        };
    }

    #[cfg(feature = "internal-highlight")]
    let loader = Rc::new(loader);
    #[cfg(feature = "internal-highlight")]
    let raw_type_loader = raw_type_loader.map(Rc::new);

    let doc = loader.get_document(&path).unwrap();

    let compiled_globals = Rc::new(CompiledGlobalCollection::compile(doc));
    let mut components = HashMap::new();

    let popup_menu_description = if let Some(popup_menu_impl) = &doc.popup_menu_impl {
        PopupMenuDescription::Rc(Rc::new_cyclic(|weak| {
            generativity::make_guard!(guard);
            ErasedItemTreeDescription::from(generate_item_tree(
                popup_menu_impl,
                Some(compiled_globals.clone()),
                PopupMenuDescription::Weak(weak.clone()),
                true,
                guard,
            ))
        }))
    } else {
        PopupMenuDescription::Weak(Default::default())
    };

    for c in doc.exported_roots() {
        generativity::make_guard!(guard);
        #[allow(unused_mut)]
        let mut it = generate_item_tree(
            &c,
            Some(compiled_globals.clone()),
            popup_menu_description.clone(),
            false,
            guard,
        );
        #[cfg(feature = "internal-highlight")]
        {
            let _ = it.type_loader.set(loader.clone());
            let _ = it.raw_type_loader.set(raw_type_loader.clone());
        }
        components.insert(c.id.to_string(), ComponentDefinition { inner: it.into() });
    }

    if components.is_empty() {
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
        components,
        #[cfg(feature = "internal")]
        structs_and_enums,
        #[cfg(feature = "internal")]
        named_exports,
    }
}

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
            rtti_for::<Rotate>(),
            rtti_for::<Opacity>(),
            rtti_for::<Layer>(),
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
                if s.name.as_ref().is_some_and(|name| name.ends_with("::StateInfo")) =>
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
            | Type::ElementReference => panic!("bad type {ty:?}"),
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
        let Some((prop, type_info)) = property_info_for_type(&decl.property_type) else { continue };
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
                let (prop, type_info) = property_info_for_type(&model_ty).unwrap();
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
    self_weak: &ErasedItemTreeBoxWeak,
) -> impl Fn(&[Value]) -> Value {
    let self_weak = self_weak.clone();
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
    self_weak: &ErasedItemTreeBoxWeak,
) -> impl Fn() -> Value {
    let self_weak = self_weak.clone();
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
                        callback.set_handler(make_callback_eval_closure(expr, &self_weak));
                    } else {
                        let item_within_component = &description.items[&elem.id];
                        let item = item_within_component.item_from_item_tree(instance_ref.as_ptr());
                        if let Some(callback) =
                            item_within_component.rtti.callbacks.get(prop_name.as_str())
                        {
                            callback.set_handler(
                                item,
                                Box::new(make_callback_eval_closure(expr, &self_weak)),
                            );
                        } else {
                            panic!("unknown callback {prop_name}")
                        }
                    }
                }
            } else if let Some(PropertiesWithinComponent { offset, prop: prop_info, .. }) =
                description.custom_properties.get(prop_name).filter(|_| is_root)
            {
                let is_state_info = matches!(property_type, Type::Struct (s) if s.name.as_ref().is_some_and(|name| name.ends_with("::StateInfo")));
                if is_state_info {
                    let prop = Pin::new_unchecked(
                        &*(instance_ref.as_ptr().add(*offset)
                            as *const Property<i_slint_core::properties::StateInfo>),
                    );
                    let e = binding.expression.clone();
                    let state_binding = make_binding_eval_closure(e, &self_weak);
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
                                Box::new(make_binding_eval_closure(e, &self_weak)),
                                maybe_animation,
                            )
                            .unwrap();
                    }
                }
                for nr in &binding.two_way_bindings {
                    // Safety: The compiler must have ensured that the properties exist and are of the same type
                    prop_info.link_two_ways(item, get_property_ptr(nr, instance_ref));
                }
            } else {
                let item_within_component = &description.items[&elem.id];
                let item = item_within_component.item_from_item_tree(instance_ref.as_ptr());
                if let Some(prop_rtti) =
                    item_within_component.rtti.properties.get(prop_name.as_str())
                {
                    let maybe_animation = animation_for_property(instance_ref, &binding.animation);
                    for nr in &binding.two_way_bindings {
                        // Safety: The compiler must have ensured that the properties exist and are of the same type
                        prop_rtti.link_two_ways(item, get_property_ptr(nr, instance_ref));
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
                                Box::new(make_binding_eval_closure(e, &self_weak)),
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
        let model_binding_closure = make_binding_eval_closure(expr, &self_weak);
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
                    m.clone()
                } else {
                    ModelRc::new(crate::value_model::ValueModel::new(m))
                }
            });
        }
    }
    self_rc
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
    // FIXME: we should compile once and keep the cached compiled component
    let compiled = generate_item_tree(
        &popup.component,
        None,
        parent_comp.upgrade().unwrap().0.description().popup_menu_description.clone(),
        false,
        guard,
    );
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
) -> MenuFromItemTree {
    generativity::make_guard!(guard);
    let mit_compiled = generate_item_tree(
        menu_item_tree,
        None,
        enclosing_component.description.popup_menu_description.clone(),
        false,
        guard,
    );
    let mit_inst = instantiate(
        mit_compiled.clone(),
        Some(enclosing_component.self_weak().get().unwrap().clone()),
        None,
        None,
        Default::default(),
    );
    mit_inst.run_setup_code();
    MenuFromItemTree::new(vtable::VRc::into_dyn(mit_inst))
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
