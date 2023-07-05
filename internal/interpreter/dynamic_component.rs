// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// cSpell: ignore unerase

use crate::{api::Value, dynamic_type, eval};

use core::convert::TryInto;
use core::ptr::NonNull;
use dynamic_type::{Instance, InstanceBox};
use i_slint_compiler::expression_tree::{Expression, NamedReference};
use i_slint_compiler::langtype::{ElementType, Type};
use i_slint_compiler::object_tree::ElementRc;
use i_slint_compiler::*;
use i_slint_compiler::{diagnostics::BuildDiagnostics, object_tree::PropertyDeclaration};
use i_slint_core::accessibility::AccessibleStringProperty;
use i_slint_core::component::{
    Component, ComponentRef, ComponentRefPin, ComponentVTable, ComponentWeak, IndexRange,
};
use i_slint_core::component_factory::ComponentFactory;
use i_slint_core::item_tree::{
    ItemRc, ItemTreeNode, ItemVisitorRefMut, ItemVisitorVTable, ItemWeak, TraversalOrder,
    VisitChildrenResult,
};
use i_slint_core::items::{AccessibleRole, Flickable, ItemRef, ItemVTable, PropertyAnimation};
use i_slint_core::layout::{BoxLayoutCellData, LayoutInfo, Orientation};
use i_slint_core::lengths::LogicalLength;
use i_slint_core::model::RepeatedComponent;
use i_slint_core::model::Repeater;
use i_slint_core::platform::PlatformError;
use i_slint_core::properties::InterpolatedPropertyValue;
use i_slint_core::rtti::{self, AnimatedBindingKind, FieldOffset, PropertyInfo};
use i_slint_core::slice::Slice;
use i_slint_core::window::{WindowAdapter, WindowInner};
use i_slint_core::{Brush, Color, Property, SharedString, SharedVector};
use once_cell::unsync::OnceCell;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::{pin::Pin, rc::Rc};

pub struct ComponentBox<'id> {
    instance: InstanceBox<'id>,
    component_type: Rc<ComponentDescription<'id>>,
}

impl<'id> ComponentBox<'id> {
    /// Borrow this component as a `Pin<ComponentRef>`
    pub fn borrow(&self) -> ComponentRefPin {
        self.borrow_instance().borrow()
    }

    /// Safety: the lifetime is not unique
    pub fn description(&self) -> Rc<ComponentDescription<'id>> {
        self.component_type.clone()
    }

    pub fn borrow_instance<'a>(&'a self) -> InstanceRef<'a, 'id> {
        InstanceRef { instance: self.instance.as_pin_ref(), component_type: &self.component_type }
    }

    pub fn window_adapter(&self) -> Result<&Rc<dyn WindowAdapter>, PlatformError> {
        InstanceRef::get_or_init_window_adapter_ref(
            &self.component_type,
            self.instance.as_pin_ref().get_ref(),
        )
    }
}

pub(crate) struct ItemWithinComponent {
    offset: usize,
    pub(crate) rtti: Rc<ItemRTTI>,
    elem: ElementRc,
}

impl ItemWithinComponent {
    pub(crate) unsafe fn item_from_component(
        &self,
        mem: *const u8,
    ) -> Pin<vtable::VRef<ItemVTable>> {
        Pin::new_unchecked(vtable::VRef::from_raw(
            NonNull::from(self.rtti.vtable),
            NonNull::new(mem.add(self.offset) as _).unwrap(),
        ))
    }

    pub(crate) fn item_index(&self) -> usize {
        *self.elem.borrow().item_index.get().unwrap()
    }
}

pub(crate) struct PropertiesWithinComponent {
    pub(crate) offset: usize,
    pub(crate) prop: Box<dyn PropertyInfo<u8, Value>>,
}

pub(crate) struct RepeaterWithinComponent<'par_id, 'sub_id> {
    /// The component description of the items to repeat
    pub(crate) component_to_repeat: Rc<ComponentDescription<'sub_id>>,
    /// The model
    pub(crate) model: Expression,
    /// Offset of the `Repeater`
    offset: FieldOffset<Instance<'par_id>, Repeater<ErasedComponentBox>>,
}

impl RepeatedComponent for ErasedComponentBox {
    type Data = Value;

    fn update(&self, index: usize, data: Self::Data) {
        generativity::make_guard!(guard);
        let s = self.unerase(guard);
        s.component_type.set_property(s.borrow(), "index", index.try_into().unwrap()).unwrap();
        s.component_type.set_property(s.borrow(), "model_data", data).unwrap();
    }

    fn init(&self) {
        self.run_setup_code();
    }

    fn listview_layout(
        self: Pin<&Self>,
        offset_y: &mut LogicalLength,
        viewport_width: Pin<&Property<LogicalLength>>,
    ) {
        generativity::make_guard!(guard);
        let s = self.unerase(guard);

        s.component_type
            .set_property(s.borrow(), "y", Value::Number(offset_y.get() as f64))
            .expect("cannot set y");
        let h: f32 = s
            .component_type
            .get_property(s.borrow(), "height")
            .expect("missing height")
            .try_into()
            .expect("height not the right type");
        let w: f32 = s
            .component_type
            .get_property(s.borrow(), "width")
            .expect("missing width")
            .try_into()
            .expect("width not the right type");
        let h = LogicalLength::new(h);
        let w = LogicalLength::new(w);
        *offset_y += h;
        let vp_w = viewport_width.get();
        if vp_w < w {
            viewport_width.set(w);
        }
    }

    fn box_layout_data(self: Pin<&Self>, o: Orientation) -> BoxLayoutCellData {
        BoxLayoutCellData { constraint: self.borrow().as_ref().layout_info(o) }
    }
}

impl Component for ErasedComponentBox {
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

    fn get_item_tree(self: Pin<&Self>) -> Slice<ItemTreeNode> {
        get_item_tree(self.get_ref().borrow())
    }

    fn get_item_ref(self: Pin<&Self>, index: usize) -> Pin<ItemRef> {
        // We're having difficulties transferring the lifetime to a pinned reference
        // to the other ComponentVTable with the same life time. So skip the vtable
        // indirection and call our implementation directly.
        unsafe { get_item_ref(self.get_ref().borrow(), index) }
    }

    fn get_subtree_range(self: Pin<&Self>, index: usize) -> IndexRange {
        self.borrow().as_ref().get_subtree_range(index)
    }

    fn get_subtree_component(
        self: Pin<&Self>,
        index: usize,
        subindex: usize,
        result: &mut ComponentWeak,
    ) {
        self.borrow().as_ref().get_subtree_component(index, subindex, result);
    }

    fn parent_node(self: Pin<&Self>, result: &mut ItemWeak) {
        self.borrow().as_ref().parent_node(result)
    }

    fn embed_component(
        self: core::pin::Pin<&Self>,
        _parent_component: &ComponentWeak,
        _item_tree_index: usize,
    ) -> bool {
        false
    }

    fn subtree_index(self: Pin<&Self>) -> usize {
        self.borrow().as_ref().subtree_index()
    }

    fn accessible_role(self: Pin<&Self>, index: usize) -> AccessibleRole {
        self.borrow().as_ref().accessible_role(index)
    }

    fn accessible_string_property(
        self: Pin<&Self>,
        index: usize,
        what: AccessibleStringProperty,
        result: &mut SharedString,
    ) {
        self.borrow().as_ref().accessible_string_property(index, what, result)
    }
}

i_slint_core::ComponentVTable_static!(static COMPONENT_BOX_VT for ErasedComponentBox);

impl<'id> Drop for ErasedComponentBox {
    fn drop(&mut self) {
        generativity::make_guard!(guard);
        let unerase = self.unerase(guard);
        let instance_ref = unerase.borrow_instance();
        if let Some(window_adapter) = instance_ref.maybe_window_adapter() {
            i_slint_core::component::unregister_component(
                instance_ref.instance,
                vtable::VRef::new(self),
                instance_ref.component_type.item_array.as_slice(),
                &window_adapter,
            );
        }
    }
}

pub type DynamicComponentVRc = vtable::VRc<ComponentVTable, ErasedComponentBox>;

#[derive(Default)]
pub(crate) struct ComponentExtraData {
    pub(crate) globals: OnceCell<crate::global_component::GlobalStorage>,
    pub(crate) self_weak: OnceCell<vtable::VWeak<ComponentVTable, ErasedComponentBox>>,
    pub(crate) embedding_position: OnceCell<(ComponentWeak, usize)>,
    // resource id -> file path
    pub(crate) embedded_file_resources: OnceCell<HashMap<usize, String>>,
    #[cfg(target_arch = "wasm32")]
    pub(crate) canvas_id: OnceCell<String>,
}

struct ErasedRepeaterWithinComponent<'id>(RepeaterWithinComponent<'id, 'static>);
impl<'id, 'sub_id> From<RepeaterWithinComponent<'id, 'sub_id>>
    for ErasedRepeaterWithinComponent<'id>
{
    fn from(from: RepeaterWithinComponent<'id, 'sub_id>) -> Self {
        // Safety: this is safe as we erase the sub_id lifetime.
        // As long as when we get it back we get an unique lifetime with ErasedRepeaterWithinComponent::unerase
        Self(unsafe {
            core::mem::transmute::<
                RepeaterWithinComponent<'id, 'sub_id>,
                RepeaterWithinComponent<'id, 'static>,
            >(from)
        })
    }
}
impl<'id> ErasedRepeaterWithinComponent<'id> {
    pub fn unerase<'a, 'sub_id>(
        &'a self,
        _guard: generativity::Guard<'sub_id>,
    ) -> &'a RepeaterWithinComponent<'id, 'sub_id> {
        // Safety: we just go from 'static to an unique lifetime
        unsafe {
            core::mem::transmute::<
                &'a RepeaterWithinComponent<'id, 'static>,
                &'a RepeaterWithinComponent<'id, 'sub_id>,
            >(&self.0)
        }
    }

    /// Return a repeater with a component with a 'static lifetime
    ///
    /// Safety: one should ensure that the inner component is not mixed with other inner component
    unsafe fn get_untagged(&self) -> &RepeaterWithinComponent<'id, 'static> {
        &self.0
    }
}

type Callback = i_slint_core::Callback<[Value], Value>;

#[derive(Clone)]
pub struct ErasedComponentDescription(Rc<ComponentDescription<'static>>);
impl ErasedComponentDescription {
    pub fn unerase<'a, 'id>(
        &'a self,
        _guard: generativity::Guard<'id>,
    ) -> &'a Rc<ComponentDescription<'id>> {
        // Safety: we just go from 'static to an unique lifetime
        unsafe {
            core::mem::transmute::<
                &'a Rc<ComponentDescription<'static>>,
                &'a Rc<ComponentDescription<'id>>,
            >(&self.0)
        }
    }
}
impl<'id> From<Rc<ComponentDescription<'id>>> for ErasedComponentDescription {
    fn from(from: Rc<ComponentDescription<'id>>) -> Self {
        // Safety: We never access the ComponentDescription with the static lifetime, only after we unerase it
        Self(unsafe {
            core::mem::transmute::<Rc<ComponentDescription<'id>>, Rc<ComponentDescription<'static>>>(
                from,
            )
        })
    }
}

/// ComponentDescription is a representation of a component suitable for interpretation
///
/// It contains information about how to create and destroy the Component.
/// Its first member is the ComponentVTable for this component, since it is a `#[repr(C)]`
/// structure, it is valid to cast a pointer to the ComponentVTable back to a
/// ComponentDescription to access the extra field that are needed at runtime
#[repr(C)]
pub struct ComponentDescription<'id> {
    pub(crate) ct: ComponentVTable,
    /// INVARIANT: both dynamic_type and item_tree have the same lifetime id. Here it is erased to 'static
    dynamic_type: Rc<dynamic_type::TypeInfo<'id>>,
    item_tree: Vec<ItemTreeNode>,
    item_array:
        Vec<vtable::VOffset<crate::dynamic_type::Instance<'id>, ItemVTable, vtable::AllowPin>>,
    pub(crate) items: HashMap<String, ItemWithinComponent>,
    pub(crate) custom_properties: HashMap<String, PropertiesWithinComponent>,
    pub(crate) custom_callbacks: HashMap<String, FieldOffset<Instance<'id>, Callback>>,
    repeater: Vec<ErasedRepeaterWithinComponent<'id>>,
    /// Map the Element::id of the repeater to the index in the `repeater` vec
    pub repeater_names: HashMap<String, usize>,
    /// Offset to a Option<ComponentPinRef>
    pub(crate) parent_component_offset:
        Option<FieldOffset<Instance<'id>, OnceCell<ComponentRefPin<'id>>>>,
    pub(crate) root_offset:
        FieldOffset<Instance<'id>, OnceCell<vtable::VWeak<ComponentVTable, ErasedComponentBox>>>,
    /// Offset to the window reference
    pub(crate) window_adapter_offset: FieldOffset<Instance<'id>, OnceCell<Rc<dyn WindowAdapter>>>,
    /// Offset of a ComponentExtraData
    pub(crate) extra_data_offset: FieldOffset<Instance<'id>, ComponentExtraData>,
    /// Keep the Rc alive
    pub(crate) original: Rc<object_tree::Component>,
    /// Maps from an item_id to the original element it came from
    pub(crate) original_elements: Vec<ElementRc>,
    /// Copy of original.root_element.property_declarations, without a guarded refcell
    public_properties: BTreeMap<String, PropertyDeclaration>,

    /// compiled globals
    compiled_globals: Vec<crate::global_component::CompiledGlobal>,
    /// Map of all exported global singletons and their index in the compiled_globals vector. The key
    /// is the normalized name of the global.
    exported_globals_by_name: BTreeMap<String, usize>,
}

fn internal_properties_to_public<'a>(
    prop_iter: impl Iterator<Item = (&'a String, &'a PropertyDeclaration)> + 'a,
) -> impl Iterator<Item = (String, i_slint_compiler::langtype::Type)> + 'a {
    prop_iter.filter(|(_, v)| v.expose_in_public_api).map(|(s, v)| {
        let name = v
            .node
            .as_ref()
            .and_then(|n| {
                n.child_node(parser::SyntaxKind::DeclaredIdentifier)
                    .and_then(|n| n.child_token(parser::SyntaxKind::Identifier))
            })
            .map(|n| n.to_string())
            .unwrap_or_else(|| s.clone());
        (name, v.property_type.clone())
    })
}

#[derive(Default)]
pub enum WindowOptions {
    #[default]
    CreateNewWindow,
    UseExistingWindow(Rc<dyn WindowAdapter>),
    #[cfg(target_arch = "wasm32")]
    CreateWithCanvasId(String),
}

impl<'id> ComponentDescription<'id> {
    /// The name of this Component as written in the .slint file
    pub fn id(&self) -> &str {
        self.original.id.as_str()
    }

    /// List of publicly declared properties or callbacks
    ///
    /// We try to preserve the dashes and underscore as written in the property declaration
    pub fn properties(
        &self,
    ) -> impl Iterator<Item = (String, i_slint_compiler::langtype::Type)> + '_ {
        internal_properties_to_public(self.public_properties.iter())
    }

    /// List names of exported global singletons
    pub fn global_names(&self) -> impl Iterator<Item = String> + '_ {
        self.compiled_globals
            .iter()
            .filter(|g| g.visible_in_public_api())
            .flat_map(|g| g.names().into_iter())
    }

    pub fn global_properties(
        &self,
        name: &str,
    ) -> Option<impl Iterator<Item = (String, i_slint_compiler::langtype::Type)> + '_> {
        self.exported_globals_by_name
            .get(crate::normalize_identifier(name).as_ref())
            .and_then(|global_idx| self.compiled_globals.get(*global_idx))
            .map(|global| internal_properties_to_public(global.public_properties()))
    }

    /// Instantiate a runtime component from this ComponentDescription
    pub fn create(
        self: Rc<Self>,
        options: WindowOptions,
    ) -> Result<DynamicComponentVRc, PlatformError> {
        i_slint_backend_selector::with_platform(|_b| {
            // Nothing to do, just make sure a backend was created
            Ok(())
        })?;

        let component_ref = instantiate(self, None, None, Some(&options), Default::default());
        if let WindowOptions::UseExistingWindow(existing_adapter) = options {
            WindowInner::from_pub(existing_adapter.window())
                .set_component(&vtable::VRc::into_dyn(component_ref.clone()));
        }
        component_ref.run_setup_code();
        Ok(component_ref)
    }

    /// Set a value to property.
    ///
    /// Return an error if the property with this name does not exist in this component,
    /// or if the value is the wrong type.
    /// Panics if the component is not an instance corresponding to this ComponentDescription,
    pub fn set_property(
        &self,
        component: ComponentRefPin,
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
    /// Returns an error if the component is not an instance corresponding to this ComponentDescription,
    /// or if the property with this name does not exist in this component
    #[allow(unused)]
    pub fn set_binding(
        &self,
        component: ComponentRefPin,
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
    /// Returns an error if the component is not an instance corresponding to this ComponentDescription,
    /// or if a callback with this name does not exist in this component
    pub fn get_property(&self, component: ComponentRefPin, name: &str) -> Result<Value, ()> {
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
    /// Returns an error if the component is not an instance corresponding to this ComponentDescription,
    /// or if the property with this name does not exist in this component
    pub fn set_callback_handler(
        &self,
        component: Pin<ComponentRef>,
        name: &str,
        handler: Box<dyn Fn(&[Value]) -> Value>,
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
            generativity::make_guard!(guard);
            let element = alias.element();
            match eval::enclosing_component_instance_for_element(
                &element,
                eval::ComponentInstance::InstanceRef(c),
                guard,
            ) {
                eval::ComponentInstance::InstanceRef(enclosing_component) => {
                    let component_type = enclosing_component.component_type;
                    let item_info = &component_type.items[element.borrow().id.as_str()];
                    let item =
                        unsafe { item_info.item_from_component(enclosing_component.as_ptr()) };
                    if let Some(callback) = item_info.rtti.callbacks.get(alias.name()) {
                        callback.set_handler(item, handler)
                    } else if let Some(callback_offset) =
                        component_type.custom_callbacks.get(alias.name())
                    {
                        let callback = callback_offset.apply(&*enclosing_component.instance);
                        callback.set_handler(handler)
                    } else {
                        return Err(());
                    }
                }
                eval::ComponentInstance::GlobalComponent(glob) => {
                    return glob.as_ref().set_callback_handler(alias.name(), handler);
                }
            }
        } else {
            let x = self.custom_callbacks.get(name).ok_or(())?;
            let sig = x.apply(unsafe { &*(component.as_ptr() as *const dynamic_type::Instance) });
            sig.set_handler(handler);
        }
        Ok(())
    }

    /// Invoke the specified callback or function
    ///
    /// Returns an error if the component is not an instance corresponding to this ComponentDescription,
    /// or if the callback with this name does not exist in this component
    pub fn invoke(
        &self,
        component: ComponentRefPin,
        name: &str,
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
            eval::call_function(inst, &elem, name, args.to_vec()).ok_or(())
        } else {
            eval::invoke_callback(inst, &elem, name, args).ok_or(())
        }
    }

    // Return the global with the given name
    pub fn get_global(
        &self,
        component: ComponentRefPin,
        global_name: &str,
    ) -> Result<Pin<Rc<dyn crate::global_component::GlobalComponent>>, ()> {
        if !core::ptr::eq((&self.ct) as *const _, component.get_vtable() as *const _) {
            return Err(());
        }
        generativity::make_guard!(guard);
        // Safety: we just verified that the component has the right vtable
        let c = unsafe { InstanceRef::from_pin_ref(component, guard) };
        let extra_data = c.component_type.extra_data_offset.apply(c.instance.get_ref());
        extra_data.globals.get().unwrap().get(global_name).cloned().ok_or(())
    }
}

extern "C" fn visit_children_item(
    component: ComponentRefPin,
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
            // `ensure_updated` needs a 'static lifetime so we must call get_untagged.
            // Safety: we do not mix the component with other component id in this function
            let rep_in_comp = unsafe { instance_ref.component_type.repeater[index].get_untagged() };
            ensure_repeater_updated(instance_ref, rep_in_comp);
            let repeater = rep_in_comp.offset.apply_pin(instance_ref.instance);
            repeater.visit(order, visitor)
        },
    )
}

/// Make sure that the repeater is updated
fn ensure_repeater_updated<'id>(
    instance_ref: InstanceRef<'_, 'id>,
    rep_in_comp: &RepeaterWithinComponent<'id, '_>,
) {
    let repeater = rep_in_comp.offset.apply_pin(instance_ref.instance);
    let init = || {
        let instance = instantiate(
            rep_in_comp.component_to_repeat.clone(),
            Some(instance_ref.borrow()),
            None,
            None,
            Default::default(),
        );
        instance
    };
    if let Some(lv) = &rep_in_comp
        .component_to_repeat
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

/// Create a ComponentDescription from a source.
/// The path corresponding to the source need to be passed as well (path is used for diagnostics
/// and loading relative assets)
pub async fn load(
    source: String,
    path: std::path::PathBuf,
    mut compiler_config: CompilerConfiguration,
    guard: generativity::Guard<'_>,
) -> (Result<Rc<ComponentDescription<'_>>, ()>, i_slint_compiler::diagnostics::BuildDiagnostics) {
    if compiler_config.style.is_none() && std::env::var("SLINT_STYLE").is_err() {
        // Defaults to native if it exists:
        compiler_config.style = Some(if i_slint_backend_selector::HAS_NATIVE_STYLE {
            "native".to_owned()
        } else {
            "fluent".to_owned()
        });
    }

    let mut diag = BuildDiagnostics::default();
    let syntax_node = parser::parse(source, Some(path.as_path()), &mut diag);
    if diag.has_error() {
        return (Err(()), diag);
    }
    let (doc, mut diag) = compile_syntax_node(syntax_node, diag, compiler_config).await;
    if diag.has_error() {
        return (Err(()), diag);
    }
    if matches!(
        doc.root_component.root_element.borrow().base_type,
        ElementType::Global | ElementType::Error
    ) {
        diag.push_error_with_span("No component found".into(), Default::default());
        return (Err(()), diag);
    }

    #[cfg(feature = "highlight")]
    crate::highlight::add_highlighting(&doc);

    (Ok(generate_component(&doc.root_component, guard)), diag)
}

pub(crate) fn generate_component<'id>(
    component: &Rc<object_tree::Component>,
    guard: generativity::Guard<'id>,
) -> Rc<ComponentDescription<'id>> {
    //dbg!(&*component.root_element.borrow());
    let mut rtti = HashMap::new();
    {
        use i_slint_core::items::*;
        rtti.extend(
            [
                rtti_for::<ComponentContainer>(),
                rtti_for::<Empty>(),
                rtti_for::<ImageItem>(),
                rtti_for::<ClippedImage>(),
                rtti_for::<Text>(),
                rtti_for::<Rectangle>(),
                rtti_for::<BorderRectangle>(),
                rtti_for::<TouchArea>(),
                rtti_for::<FocusScope>(),
                rtti_for::<Path>(),
                rtti_for::<Flickable>(),
                rtti_for::<WindowItem>(),
                rtti_for::<TextInput>(),
                rtti_for::<Clip>(),
                rtti_for::<BoxShadow>(),
                rtti_for::<Rotate>(),
                rtti_for::<Opacity>(),
                rtti_for::<Layer>(),
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
    }

    struct TreeBuilder<'id> {
        tree_array: Vec<ItemTreeNode>,
        item_array:
            Vec<vtable::VOffset<crate::dynamic_type::Instance<'id>, ItemVTable, vtable::AllowPin>>,
        original_elements: Vec<ElementRc>,
        items_types: HashMap<String, ItemWithinComponent>,
        type_builder: dynamic_type::TypeBuilder<'id>,
        repeater: Vec<ErasedRepeaterWithinComponent<'id>>,
        repeater_names: HashMap<String, usize>,
        rtti: Rc<HashMap<&'static str, Rc<ItemRTTI>>>,
    }
    impl<'id> generator::ItemTreeBuilder for TreeBuilder<'id> {
        type SubComponentState = ();

        fn push_repeated_item(
            &mut self,
            item_rc: &ElementRc,
            repeater_count: u32,
            parent_index: u32,
            _component_state: &Self::SubComponentState,
        ) {
            self.tree_array
                .push(ItemTreeNode::DynamicTree { index: repeater_count as usize, parent_index });
            self.original_elements.push(item_rc.clone());
            let item = item_rc.borrow();
            let base_component = item.base_type.as_component();
            self.repeater_names.insert(item.id.clone(), self.repeater.len());
            generativity::make_guard!(guard);
            self.repeater.push(
                RepeaterWithinComponent {
                    component_to_repeat: generate_component(base_component, guard),
                    offset: self.type_builder.add_field_type::<Repeater<ErasedComponentBox>>(),
                    model: item.repeated.as_ref().unwrap().model.clone(),
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
            let rt = self.rtti.get(&*item.base_type.as_native().class_name).unwrap_or_else(|| {
                panic!("Native type not registered: {}", item.base_type.as_native().class_name)
            });

            let offset = if item.is_flickable_viewport {
                let parent = &self.items_types
                    [&object_tree::find_parent_element(rc_item).unwrap().borrow().id];
                assert_eq!(
                    parent.elem.borrow().base_type.as_native().class_name.as_str(),
                    "Flickable"
                );
                parent.offset + Flickable::FIELD_OFFSETS.viewport.get_byte_offset()
            } else {
                self.type_builder.add_field(rt.type_info)
            };
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
                ItemWithinComponent { offset, rtti: rt.clone(), elem: rc_item.clone() },
            );
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
        rtti: Rc::new(rtti),
    };

    if !component.is_global() {
        generator::build_item_tree(component, &(), &mut builder);
    }

    let mut custom_properties = HashMap::new();
    let mut custom_callbacks = HashMap::new();
    fn property_info<T: PartialEq + Clone + Default + 'static>(
    ) -> (Box<dyn PropertyInfo<u8, Value>>, dynamic_type::StaticTypeInfo)
    where
        T: std::convert::TryInto<Value>,
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
    fn animated_property_info<T: Clone + Default + InterpolatedPropertyValue + 'static>(
    ) -> (Box<dyn PropertyInfo<u8, Value>>, dynamic_type::StaticTypeInfo)
    where
        T: std::convert::TryInto<Value>,
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

    for (name, decl) in &component.root_element.borrow().property_declarations {
        if decl.is_alias.is_some() {
            continue;
        }
        let (prop, type_info) = match &decl.property_type {
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
            Type::Callback { .. } => {
                custom_callbacks
                    .insert(name.clone(), builder.type_builder.add_field_type::<Callback>());
                continue;
            }
            Type::ComponentFactory => property_info::<ComponentFactory>(),
            Type::Struct { name: Some(name), .. } if name.ends_with("::StateInfo") => {
                property_info::<i_slint_core::properties::StateInfo>()
            }
            Type::Struct { .. } => property_info::<Value>(),
            Type::Array(_) => property_info::<Value>(),
            Type::Easing => property_info::<i_slint_core::animations::EasingCurve>(),
            Type::Percent => property_info::<f32>(),
            Type::Enumeration(e) => {
                macro_rules! match_enum_type {
                    ($( $(#[$enum_doc:meta])* enum $Name:ident { $($body:tt)* })*) => {
                        match e.name.as_str() {
                            $(
                                stringify!($Name) => property_info::<i_slint_core::items::$Name>(),
                            )*
                            _ => property_info::<Value>(),
                        }
                    }
                }
                i_slint_common::for_each_enums!(match_enum_type)
            }
            Type::LayoutCache => property_info::<SharedVector<f32>>(),
            Type::Function { .. } => continue,

            // These can't be used in properties
            Type::Invalid
            | Type::Void
            | Type::InferredProperty
            | Type::InferredCallback
            | Type::Model
            | Type::PathData
            | Type::UnitProduct(_)
            | Type::ElementReference => panic!("bad type {:?}", &decl.property_type),
        };
        custom_properties.insert(
            name.clone(),
            PropertiesWithinComponent { offset: builder.type_builder.add_field(type_info), prop },
        );
    }
    if component.parent_element.upgrade().is_some() {
        let (prop, type_info) = property_info::<u32>();
        custom_properties.insert(
            "index".into(),
            PropertiesWithinComponent { offset: builder.type_builder.add_field(type_info), prop },
        );
        // FIXME: make it a property for the correct type instead of being generic
        let (prop, type_info) = property_info::<Value>();
        custom_properties.insert(
            "model_data".into(),
            PropertiesWithinComponent { offset: builder.type_builder.add_field(type_info), prop },
        );
    } else {
        let (prop, type_info) = property_info::<f32>();
        custom_properties.insert(
            "scale_factor".into(),
            PropertiesWithinComponent { offset: builder.type_builder.add_field(type_info), prop },
        );
    }

    let parent_component_offset = if component.parent_element.upgrade().is_some() {
        Some(builder.type_builder.add_field_type::<OnceCell<ComponentRefPin>>())
    } else {
        None
    };

    let root_offset = builder
        .type_builder
        .add_field_type::<OnceCell<vtable::VWeak<ComponentVTable, ErasedComponentBox>>>();

    let window_adapter_offset =
        builder.type_builder.add_field_type::<OnceCell<Rc<dyn WindowAdapter>>>();

    let extra_data_offset = builder.type_builder.add_field_type::<ComponentExtraData>();

    let public_properties = component.root_element.borrow().property_declarations.clone();

    let mut exported_globals_by_name: BTreeMap<String, usize> = Default::default();

    let compiled_globals = component
        .used_types
        .borrow()
        .globals
        .iter()
        .enumerate()
        .map(|(index, component)| {
            let mut global = crate::global_component::generate(component);

            if component.visible_in_public_api() {
                global.extend_public_properties(
                    component.root_element.borrow().property_declarations.clone().into_iter(),
                );

                exported_globals_by_name.extend(
                    component
                        .exported_global_names
                        .borrow()
                        .iter()
                        .map(|exported_name| (exported_name.name.clone(), index)),
                )
            }

            global
        })
        .collect();

    let t = ComponentVTable {
        visit_children_item,
        layout_info,
        get_item_ref,
        get_item_tree,
        get_subtree_range,
        get_subtree_component,
        parent_node,
        embed_component,
        subtree_index,
        accessible_role,
        accessible_string_property,
        drop_in_place,
        dealloc,
    };
    let t = ComponentDescription {
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
        parent_component_offset,
        root_offset,
        window_adapter_offset,
        extra_data_offset,
        public_properties,
        compiled_globals,
        exported_globals_by_name,
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
            let vtable = NonNull::from(&component.component_type.ct).cast();
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
                        if (a.is_out && a.state_id == state_info.previous_state)
                            || (!a.is_out && a.state_id == state_info.current_state)
                        {
                            return (
                                eval::new_struct_with_bindings(
                                    &a.animation.borrow().bindings,
                                    &mut context,
                                ),
                                state_info.change_time,
                            );
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
    self_weak: &vtable::VWeak<ComponentVTable, ErasedComponentBox>,
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
    self_weak: &vtable::VWeak<ComponentVTable, ErasedComponentBox>,
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
    component_type: Rc<ComponentDescription>,
    parent_ctx: Option<ComponentRefPin>,
    root: Option<vtable::VWeak<ComponentVTable, ErasedComponentBox>>,
    window_options: Option<&WindowOptions>,
    mut globals: crate::global_component::GlobalStorage,
) -> DynamicComponentVRc {
    let instance = component_type.dynamic_type.clone().create_instance();

    let component_box = ComponentBox { instance, component_type: component_type.clone() };

    let self_rc = vtable::VRc::new(ErasedComponentBox::from(component_box));
    let self_weak = vtable::VRc::downgrade(&self_rc);

    generativity::make_guard!(guard);
    let comp = self_rc.unerase(guard);
    let instance_ref = comp.borrow_instance();
    instance_ref.self_weak().set(self_weak.clone()).ok();
    let component_type = comp.description();

    if !component_type.original.is_global() {
        let maybe_window_adapter =
            if let Some(WindowOptions::UseExistingWindow(adapter)) = window_options.as_ref() {
                Some(adapter.clone())
            } else {
                root.as_ref().and_then(|root| root.upgrade()).and_then(|root| {
                    generativity::make_guard!(guard);
                    let comp = root.unerase(guard);
                    let instance = comp.borrow_instance();
                    instance.maybe_window_adapter()
                })
            };

        let component_rc = vtable::VRc::into_dyn(self_rc.clone());
        i_slint_core::component::register_component(&component_rc, maybe_window_adapter);
    }

    if let Some(parent) = parent_ctx {
        component_type
            .parent_component_offset
            .unwrap()
            .apply(instance_ref.as_ref())
            .set(parent)
            .ok()
            .unwrap();
    } else {
        for g in &component_type.compiled_globals {
            crate::global_component::instantiate(g, &mut globals, self_weak.clone());
        }
        let extra_data = component_type.extra_data_offset.apply(instance_ref.as_ref());
        extra_data.globals.set(globals).ok().unwrap();

        extra_data
            .embedded_file_resources
            .set(
                component_type
                    .original
                    .embedded_file_resources
                    .borrow()
                    .iter()
                    .map(|(path, er)| (er.id, path.clone()))
                    .collect(),
            )
            .ok()
            .unwrap();

        #[cfg(target_arch = "wasm32")]
        if let Some(WindowOptions::CreateWithCanvasId(canvas_id)) = window_options {
            extra_data.canvas_id.set(canvas_id.clone()).unwrap();
        }
    }

    let root = root
        .or_else(|| instance_ref.parent_instance().map(|parent| parent.root_weak().clone()))
        .unwrap_or_else(|| self_weak.clone());
    component_type.root_offset.apply(instance_ref.as_ref()).set(root).ok().unwrap();

    if let Some(WindowOptions::UseExistingWindow(window_adapter)) = window_options {
        component_type
            .window_adapter_offset
            .apply(instance_ref.as_ref())
            .set(window_adapter.clone())
            .ok()
            .unwrap();
    }

    // Some properties are generated as Value, but for which the default constructed Value must be initialized
    for (prop_name, decl) in &component_type.original.root_element.borrow().property_declarations {
        if !matches!(
            decl.property_type,
            Type::Struct { .. } | Type::Array(_) | Type::Enumeration(_)
        ) || decl.is_alias.is_some()
        {
            continue;
        }
        if let Some(b) = component_type.original.root_element.borrow().bindings.get(prop_name) {
            if b.borrow().two_way_bindings.is_empty() {
                continue;
            }
        }
        let p = component_type.custom_properties.get(prop_name).unwrap();
        unsafe {
            let item = Pin::new_unchecked(&*instance_ref.as_ptr().add(p.offset));
            p.prop.set(item, eval::default_value_for_type(&decl.property_type), None).unwrap();
        }
    }

    generator::handle_property_bindings_init(
        &component_type.original,
        |elem, prop_name, binding| unsafe {
            let is_root = Rc::ptr_eq(
                elem,
                &elem.borrow().enclosing_component.upgrade().unwrap().root_element,
            );
            let elem = elem.borrow();
            let is_const = binding.analysis.as_ref().map_or(false, |a| a.is_const);

            let property_type = elem.lookup_property(prop_name).property_type;
            if let Type::Function { .. } = property_type {
                // function don't need initialization
            } else if let Type::Callback { .. } = property_type {
                if !matches!(binding.expression, Expression::Invalid) {
                    let expr = binding.expression.clone();
                    let component_type = component_type.clone();
                    if let Some(callback_offset) =
                        component_type.custom_callbacks.get(prop_name).filter(|_| is_root)
                    {
                        let callback = callback_offset.apply(instance_ref.as_ref());
                        callback.set_handler(make_callback_eval_closure(expr, &self_weak));
                    } else {
                        let item_within_component = &component_type.items[&elem.id];
                        let item = item_within_component.item_from_component(instance_ref.as_ptr());
                        if let Some(callback) = item_within_component.rtti.callbacks.get(prop_name)
                        {
                            callback.set_handler(
                                item,
                                Box::new(make_callback_eval_closure(expr, &self_weak)),
                            );
                        } else {
                            panic!("unknown callback {}", prop_name)
                        }
                    }
                }
            } else if let Some(PropertiesWithinComponent { offset, prop: prop_info, .. }) =
                component_type.custom_properties.get(prop_name).filter(|_| is_root)
            {
                let is_state_info = matches!(property_type, Type::Struct { name: Some(name), .. } if name.ends_with("::StateInfo"));
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
                let item_within_component = &component_type.items[&elem.id];
                let item = item_within_component.item_from_component(instance_ref.as_ptr());
                if let Some(prop_rtti) = item_within_component.rtti.properties.get(prop_name) {
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
                    panic!("unknown property {}", prop_name);
                }
            }
        },
    );

    for rep_in_comp in &component_type.repeater {
        generativity::make_guard!(guard);
        let rep_in_comp = rep_in_comp.unerase(guard);

        let repeater = rep_in_comp.offset.apply_pin(instance_ref.instance);
        let expr = rep_in_comp.model.clone();
        let model_binding_closure = make_binding_eval_closure(expr, &self_weak);
        repeater.set_model_binding(move || {
            let m = model_binding_closure();
            i_slint_core::model::ModelRc::new(crate::value_model::ValueModel::new(m))
        });
    }

    self_rc
}

pub(crate) fn get_property_ptr(nr: &NamedReference, instance: InstanceRef) -> *const () {
    let element = nr.element();
    generativity::make_guard!(guard);
    let enclosing_component = eval::enclosing_component_instance_for_element(
        &element,
        eval::ComponentInstance::InstanceRef(instance),
        guard,
    );
    match enclosing_component {
        eval::ComponentInstance::InstanceRef(enclosing_component) => {
            let element = element.borrow();
            if element.id == element.enclosing_component.upgrade().unwrap().root_element.borrow().id
            {
                if let Some(x) = enclosing_component.component_type.custom_properties.get(nr.name())
                {
                    return unsafe { enclosing_component.as_ptr().add(x.offset).cast() };
                };
            };
            let item_info = enclosing_component
                .component_type
                .items
                .get(element.id.as_str())
                .unwrap_or_else(|| panic!("Unknown element for {}.{}", element.id, nr.name()));
            let prop_info = item_info
                .rtti
                .properties
                .get(nr.name())
                .unwrap_or_else(|| panic!("Property {} not in {}", nr.name(), element.id));
            core::mem::drop(element);
            let item = unsafe { item_info.item_from_component(enclosing_component.as_ptr()) };
            unsafe { item.as_ptr().add(prop_info.offset()).cast() }
        }
        eval::ComponentInstance::GlobalComponent(glob) => glob.as_ref().get_property_ptr(nr.name()),
    }
}

pub struct ErasedComponentBox(ComponentBox<'static>);
impl ErasedComponentBox {
    pub fn unerase<'a, 'id>(
        &'a self,
        _guard: generativity::Guard<'id>,
    ) -> Pin<&'a ComponentBox<'id>> {
        Pin::new(
            //Safety: 'id is unique because of `_guard`
            unsafe { core::mem::transmute::<&ComponentBox<'static>, &ComponentBox<'id>>(&self.0) },
        )
    }

    pub fn borrow(&self) -> ComponentRefPin {
        // Safety: it is safe to access self.0 here because the 'id lifetime does not leak
        self.0.borrow()
    }

    pub fn window_adapter(&self) -> Result<&Rc<dyn WindowAdapter>, PlatformError> {
        self.0.window_adapter()
    }

    pub fn maybe_window_adapter(&self) -> Option<Rc<dyn WindowAdapter>> {
        self.0.borrow_instance().maybe_window_adapter()
    }

    pub fn run_setup_code(&self) {
        generativity::make_guard!(guard);
        let compo_box = self.unerase(guard);
        let instance_ref = compo_box.borrow_instance();
        for extra_init_code in self.0.component_type.original.init_code.borrow().iter() {
            eval::eval_expression(
                extra_init_code,
                &mut eval::EvalLocalContext::from_component_instance(instance_ref),
            );
        }
    }
}
impl<'id> From<ComponentBox<'id>> for ErasedComponentBox {
    fn from(inner: ComponentBox<'id>) -> Self {
        // Safety: Nothing access the component directly, we only access it through unerased where
        // the lifetime is unique again
        unsafe {
            ErasedComponentBox(core::mem::transmute::<ComponentBox<'id>, ComponentBox<'static>>(
                inner,
            ))
        }
    }
}

pub fn get_repeater_by_name<'a, 'id>(
    instance_ref: InstanceRef<'a, '_>,
    name: &str,
    guard: generativity::Guard<'id>,
) -> (std::pin::Pin<&'a Repeater<ErasedComponentBox>>, Rc<ComponentDescription<'id>>) {
    let rep_index = instance_ref.component_type.repeater_names[name];
    let rep_in_comp = instance_ref.component_type.repeater[rep_index].unerase(guard);
    (rep_in_comp.offset.apply_pin(instance_ref.instance), rep_in_comp.component_to_repeat.clone())
}

extern "C" fn layout_info(component: ComponentRefPin, orientation: Orientation) -> LayoutInfo {
    generativity::make_guard!(guard);
    // This is fine since we can only be called with a component that with our vtable which is a ComponentDescription
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };
    let orientation = crate::eval_layout::from_runtime(orientation);

    let mut result = crate::eval_layout::get_layout_info(
        &instance_ref.component_type.original.root_element,
        instance_ref,
        &instance_ref.window_adapter(),
        orientation,
    );

    let constraints = instance_ref.component_type.original.root_constraints.borrow();
    if constraints.has_explicit_restrictions() {
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

unsafe extern "C" fn get_item_ref(component: ComponentRefPin, index: usize) -> Pin<ItemRef> {
    let tree = get_item_tree(component);
    match &tree[index] {
        ItemTreeNode::Item { item_array_index, .. } => {
            generativity::make_guard!(guard);
            let instance_ref = InstanceRef::from_pin_ref(component, guard);
            core::mem::transmute::<Pin<ItemRef>, Pin<ItemRef>>(
                instance_ref.component_type.item_array[*item_array_index as usize]
                    .apply_pin(instance_ref.instance),
            )
        }
        ItemTreeNode::DynamicTree { .. } => panic!("get_item_ref called on dynamic tree"),
    }
}

extern "C" fn get_subtree_range(component: ComponentRefPin, index: usize) -> IndexRange {
    generativity::make_guard!(guard);
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };
    let rep_in_comp = unsafe { instance_ref.component_type.repeater[index].get_untagged() };
    ensure_repeater_updated(instance_ref, rep_in_comp);

    let repeater = rep_in_comp.offset.apply(&instance_ref.instance);
    repeater.range().into()
}

extern "C" fn get_subtree_component(
    component: ComponentRefPin,
    index: usize,
    subtree_index: usize,
    result: &mut ComponentWeak,
) {
    generativity::make_guard!(guard);
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };
    let rep_in_comp = unsafe { instance_ref.component_type.repeater[index].get_untagged() };
    ensure_repeater_updated(instance_ref, rep_in_comp);

    let repeater = rep_in_comp.offset.apply(&instance_ref.instance);
    *result = vtable::VRc::downgrade(&vtable::VRc::into_dyn(
        repeater.component_at(subtree_index).unwrap(),
    ))
}

extern "C" fn get_item_tree(component: ComponentRefPin) -> Slice<ItemTreeNode> {
    generativity::make_guard!(guard);
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };
    let tree = instance_ref.component_type.item_tree.as_slice();
    unsafe { core::mem::transmute::<&[ItemTreeNode], &[ItemTreeNode]>(tree) }.into()
}

extern "C" fn subtree_index(component: ComponentRefPin) -> usize {
    generativity::make_guard!(guard);
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };
    if let Ok(value) = instance_ref.component_type.get_property(component, "index") {
        value.try_into().unwrap()
    } else {
        core::usize::MAX
    }
}

unsafe extern "C" fn parent_node(component: ComponentRefPin, result: &mut ItemWeak) {
    generativity::make_guard!(guard);
    let instance_ref = InstanceRef::from_pin_ref(component, guard);

    let component_and_index = {
        // Normal inner-compilation unit case:
        if let Some(parent_offset) = instance_ref.component_type.parent_component_offset {
            let parent_item_index = instance_ref
                .component_type
                .original
                .parent_element
                .upgrade()
                .and_then(|e| e.borrow().item_index.get().cloned())
                .unwrap_or(usize::MAX);
            let parent_component = parent_offset.apply(instance_ref.as_ref()).get().map(|prp| {
                generativity::make_guard!(new_guard);
                let instance = InstanceRef::from_pin_ref(*prp, new_guard);
                instance.self_weak().get().unwrap().clone().into_dyn().upgrade().unwrap()
            });
            (parent_component, parent_item_index)
        } else if let Some((parent_component, parent_index)) = instance_ref
            .component_type
            .extra_data_offset
            .apply(instance_ref.as_ref())
            .embedding_position
            .get()
        {
            (parent_component.upgrade(), *parent_index)
        } else {
            (None, usize::MAX)
        }
    };

    if let (Some(component), index) = component_and_index {
        *result = ItemRc::new(component, index).downgrade();
    }
}

unsafe extern "C" fn embed_component(
    component: ComponentRefPin,
    parent_component: &ComponentWeak,
    parent_item_tree_index: usize,
) -> bool {
    generativity::make_guard!(guard);
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };

    if instance_ref.component_type.parent_component_offset.is_some() {
        // We are not the root of the compilation unit tree... Can not embed this!
        return false;
    }

    {
        // sanity check parent:
        let prc = parent_component.upgrade().unwrap();
        let pref = vtable::VRc::borrow_pin(&prc);
        let it = pref.as_ref().get_item_tree();
        if !matches!(it.get(parent_item_tree_index), Some(ItemTreeNode::DynamicTree { .. })) {
            panic!("Trying to embed into a non-dynamic index in the parents item tree")
        }
    }

    let extra_data = instance_ref.component_type.extra_data_offset.apply(instance_ref.as_ref());
    extra_data.embedding_position.set((parent_component.clone(), parent_item_tree_index)).is_ok()
}

extern "C" fn accessible_role(component: ComponentRefPin, item_index: usize) -> AccessibleRole {
    generativity::make_guard!(guard);
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };
    let nr = instance_ref.component_type.original_elements[item_index]
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
extern "C" fn accessible_string_property(
    component: ComponentRefPin,
    item_index: usize,
    what: AccessibleStringProperty,
    result: &mut SharedString,
) {
    generativity::make_guard!(guard);
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };
    let prop_name = format!("accessible-{}", what);
    let nr = instance_ref.component_type.original_elements[item_index]
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
    }
}

unsafe extern "C" fn drop_in_place(component: vtable::VRefMut<ComponentVTable>) -> vtable::Layout {
    let instance_ptr = component.as_ptr() as *mut Instance<'static>;
    let layout = (*instance_ptr).type_info().layout();
    dynamic_type::TypeInfo::drop_in_place(instance_ptr);
    layout.into()
}

unsafe extern "C" fn dealloc(_vtable: &ComponentVTable, ptr: *mut u8, layout: vtable::Layout) {
    std::alloc::dealloc(ptr, layout.try_into().unwrap());
}

#[derive(Copy, Clone)]
pub struct InstanceRef<'a, 'id> {
    pub instance: Pin<&'a Instance<'id>>,
    pub component_type: &'a ComponentDescription<'id>,
}

impl<'a, 'id> InstanceRef<'a, 'id> {
    pub unsafe fn from_pin_ref(
        component: ComponentRefPin<'a>,
        _guard: generativity::Guard<'id>,
    ) -> Self {
        Self {
            instance: Pin::new_unchecked(&*(component.as_ref().as_ptr() as *const Instance<'id>)),
            component_type: &*(Pin::into_inner_unchecked(component).get_vtable()
                as *const ComponentVTable
                as *const ComponentDescription<'id>),
        }
    }

    pub fn as_ptr(&self) -> *const u8 {
        (&*self.instance.as_ref()) as *const Instance as *const u8
    }

    pub fn as_ref(&self) -> &Instance<'id> {
        &self.instance
    }

    /// Borrow this component as a `Pin<ComponentRef>`
    pub fn borrow(self) -> ComponentRefPin<'a> {
        unsafe {
            Pin::new_unchecked(vtable::VRef::from_raw(
                NonNull::from(&self.component_type.ct).cast(),
                NonNull::from(self.instance.get_ref()).cast(),
            ))
        }
    }

    pub fn self_weak(&self) -> &OnceCell<vtable::VWeak<ComponentVTable, ErasedComponentBox>> {
        let extra_data = self.component_type.extra_data_offset.apply(self.as_ref());
        &extra_data.self_weak
    }

    pub fn root_weak(&self) -> &vtable::VWeak<ComponentVTable, ErasedComponentBox> {
        self.component_type.root_offset.apply(self.as_ref()).get().unwrap()
    }

    pub fn window_adapter(&self) -> Rc<dyn WindowAdapter> {
        let root = self.root_weak().upgrade().unwrap();
        generativity::make_guard!(guard);
        let comp = root.unerase(guard);
        Self::get_or_init_window_adapter_ref(
            &comp.component_type,
            comp.instance.as_pin_ref().get_ref(),
        )
        .unwrap()
        .clone()
    }

    // Call this only on root components!
    pub fn get_or_init_window_adapter_ref<'b, 'id2>(
        component_type: &'b ComponentDescription<'id2>,
        instance: &'b Instance<'id2>,
    ) -> Result<&'b Rc<dyn WindowAdapter>, PlatformError> {
        component_type.window_adapter_offset.apply(instance).get_or_try_init(|| {
            let extra_data = component_type.extra_data_offset.apply(instance);
            let window_adapter = i_slint_backend_selector::with_platform(|_b| {
                #[cfg(not(target_arch = "wasm32"))]
                return _b.create_window_adapter();
                #[cfg(target_arch = "wasm32")]
                i_slint_backend_winit::create_gl_window_with_canvas_id(
                    extra_data.canvas_id.get().map_or("canvas", |s| s.as_str()),
                )
            })?;
            let comp_rc = extra_data.self_weak.get().unwrap().upgrade().unwrap();
            WindowInner::from_pub(window_adapter.window())
                .set_component(&vtable::VRc::into_dyn(comp_rc));
            Ok(window_adapter)
        })
    }

    pub fn maybe_window_adapter(&self) -> Option<Rc<dyn WindowAdapter>> {
        let root = self.root_weak().upgrade()?;
        generativity::make_guard!(guard);
        let comp = root.unerase(guard);
        let instance = comp.borrow_instance();
        instance.component_type.window_adapter_offset.apply(instance.as_ref()).get().cloned()
    }

    pub fn access_window<R>(
        self,
        callback: impl FnOnce(&'_ i_slint_core::window::WindowInner) -> R,
    ) -> R {
        callback(WindowInner::from_pub(self.window_adapter().window()))
    }

    pub fn parent_instance(&self) -> Option<InstanceRef<'a, 'id>> {
        if let Some(parent_offset) = self.component_type.parent_component_offset {
            if let Some(parent) = parent_offset.apply(self.as_ref()).get() {
                let parent_instance = unsafe {
                    Self {
                        instance: Pin::new_unchecked(
                            &*(parent.as_ref().as_ptr() as *const Instance<'id>),
                        ),
                        component_type: &*(Pin::into_inner_unchecked(*parent).get_vtable()
                            as *const ComponentVTable
                            as *const ComponentDescription<'id>),
                    }
                };
                return Some(parent_instance);
            };
        }
        None
    }

    pub fn toplevel_instance(&self) -> InstanceRef<'a, 'id> {
        if let Some(parent) = self.parent_instance() {
            parent.toplevel_instance()
        } else {
            *self
        }
    }
}

/// Show the popup at the given location
pub fn show_popup(
    popup: &object_tree::PopupWindow,
    pos: i_slint_core::graphics::Point,
    close_on_click: bool,
    parent_comp: ComponentRefPin,
    parent_window_adapter: Rc<dyn WindowAdapter>,
    parent_item: &ItemRc,
) {
    generativity::make_guard!(guard);
    // FIXME: we should compile once and keep the cached compiled component
    let compiled = generate_component(&popup.component, guard);
    let inst = instantiate(
        compiled,
        Some(parent_comp),
        None,
        Some(&WindowOptions::UseExistingWindow(parent_window_adapter.clone())),
        Default::default(),
    );
    inst.run_setup_code();
    WindowInner::from_pub(parent_window_adapter.window()).show_popup(
        &vtable::VRc::into_dyn(inst),
        pos,
        close_on_click,
        parent_item,
    );
}
