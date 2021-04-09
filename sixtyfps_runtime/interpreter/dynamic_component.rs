/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use crate::{api::Value, dynamic_type, eval};

use core::convert::TryInto;
use core::ptr::NonNull;
use dynamic_type::{Instance, InstanceBox};
use expression_tree::NamedReference;
use object_tree::{Element, ElementRc};
use sixtyfps_compilerlib::langtype::Type;
use sixtyfps_compilerlib::layout::{
    Layout, LayoutConstraints, LayoutGeometry, LayoutItem, PathLayout,
};
use sixtyfps_compilerlib::*;
use sixtyfps_compilerlib::{diagnostics::BuildDiagnostics, object_tree::PropertyDeclaration};
use sixtyfps_compilerlib::{expression_tree::Expression, langtype::PropertyLookupResult};
use sixtyfps_corelib::component::{Component, ComponentRef, ComponentRefPin, ComponentVTable};
use sixtyfps_corelib::graphics::{ImageReference, Rect};
use sixtyfps_corelib::item_tree::{
    ItemTreeNode, ItemVisitorRefMut, ItemVisitorVTable, TraversalOrder, VisitChildrenResult,
};
use sixtyfps_corelib::items::{
    Flickable, ItemRc, ItemRef, ItemVTable, ItemWeak, PropertyAnimation,
};
use sixtyfps_corelib::layout::{LayoutInfo, Padding};
use sixtyfps_corelib::model::RepeatedComponent;
use sixtyfps_corelib::model::Repeater;
use sixtyfps_corelib::properties::InterpolatedPropertyValue;
use sixtyfps_corelib::rtti::{self, AnimatedBindingKind, FieldOffset, PropertyInfo};
use sixtyfps_corelib::slice::Slice;
use sixtyfps_corelib::window::ComponentWindow;
use sixtyfps_corelib::{Brush, Color, Property, SharedString};
use std::collections::HashMap;
use std::{pin::Pin, rc::Rc};

pub struct ComponentBox<'id> {
    instance: InstanceBox<'id>,
    component_type: Rc<ComponentDescription<'id>>,
}

impl<'id> ComponentBox<'id> {
    /// Borrow this component as a `Pin<ComponentRef>`
    pub fn borrow<'a>(&'a self) -> ComponentRefPin<'a> {
        self.borrow_instance().borrow()
    }

    /// Safety: the lifetime is not unique
    pub fn description(&self) -> Rc<ComponentDescription<'id>> {
        return self.component_type.clone();
    }

    pub fn borrow_instance<'a>(&'a self) -> InstanceRef<'a, 'id> {
        InstanceRef { instance: self.instance.as_pin_ref(), component_type: &self.component_type }
    }

    pub fn window(&self) -> ComponentWindow {
        (*self
            .component_type
            .window_offset
            .apply_pin(self.instance.as_pin_ref())
            .as_pin_ref()
            .unwrap())
        .clone()
    }
}

impl<'id> Drop for ComponentBox<'id> {
    fn drop(&mut self) {
        let instance_ref = self.borrow_instance();
        match eval::window_ref(instance_ref) {
            Some(window) => {
                let items = self
                    .component_type
                    .items
                    .values()
                    .map(|item_within_component| unsafe {
                        item_within_component.item_from_component(instance_ref.as_ptr())
                    })
                    .collect::<Vec<_>>();

                window.free_graphics_resources(&Slice::from_slice(items.as_slice()));
            }
            None => {}
        }
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

    fn listview_layout(self: Pin<&Self>, offset_y: &mut f32, viewport_width: Pin<&Property<f32>>) {
        generativity::make_guard!(guard);
        let s = self.unerase(guard);

        self.borrow().as_ref().apply_layout(Default::default());
        s.component_type
            .set_property(s.borrow(), "y", Value::Number(*offset_y as f64))
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
        *offset_y += h;
        let vp_w = viewport_width.get();
        if vp_w < w {
            viewport_width.set(w);
        }
    }

    fn box_layout_data<'a>(self: Pin<&'a Self>) -> BoxLayoutCellData<'a> {
        generativity::make_guard!(guard);
        let s = self.unerase(guard);

        let root_item = &s.component_type.original.root_element.clone();
        let get_prop = |name: &str| {
            let PropertyLookupResult { resolved_name, property_type } =
                root_item.borrow().lookup_property(name);
            if property_type == Type::Length {
                let nr = NamedReference::new(root_item, resolved_name.as_ref());
                let p = get_property_ptr(&nr, s.borrow_instance());
                // Safety: assuming get_property_ptr returned a valid pointer,
                // we know that `Type::Length` is a property of type `f32`
                Some(unsafe { &*(p as *const Property<f32>) })
            } else {
                None
            }
        };

        let root_c = &s.component_type.original.layouts.borrow().root_constraints;
        BoxLayoutCellData {
            constraint: self.borrow().as_ref().layout_info(),
            x: get_prop("x"),
            y: get_prop("y"),
            width: if root_c.fixed_width { None } else { get_prop("width") },
            height: if root_c.fixed_height { None } else { get_prop("height") },
        }
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

    fn layout_info(self: Pin<&Self>) -> sixtyfps_corelib::layout::LayoutInfo {
        self.borrow().as_ref().layout_info()
    }
    fn apply_layout(self: Pin<&Self>, r: sixtyfps_corelib::graphics::Rect) {
        self.borrow().as_ref().apply_layout(r)
    }
    fn get_item_ref<'a>(self: Pin<&'a Self>, index: usize) -> Pin<ItemRef<'a>> {
        // We're having difficulties transferring the lifetime to a pinned reference
        // to the other ComponentVTable with the same life time. So skip the vtable
        // indirection and call our implementation directly.
        unsafe { get_item_ref(self.get_ref().borrow(), index) }
    }
    fn parent_item(self: Pin<&Self>, index: usize, result: &mut ItemWeak) {
        self.borrow().as_ref().parent_item(index, result)
    }
}

sixtyfps_corelib::ComponentVTable_static!(static COMPONENT_BOX_VT for ErasedComponentBox);

pub(crate) struct ComponentExtraData {
    pub(crate) globals: HashMap<String, Pin<Rc<dyn crate::global_component::GlobalComponent>>>,
    pub(crate) self_weak:
        once_cell::unsync::OnceCell<vtable::VWeak<ComponentVTable, ErasedComponentBox>>,
}

impl Default for ComponentExtraData {
    fn default() -> Self {
        Self { globals: HashMap::new(), self_weak: Default::default() }
    }
}

struct ErasedRepeaterWithinComponent<'id>(RepeaterWithinComponent<'id, 'static>);
impl<'id, 'sub_id> From<RepeaterWithinComponent<'id, 'sub_id>>
    for ErasedRepeaterWithinComponent<'id>
{
    fn from(from: RepeaterWithinComponent<'id, 'sub_id>) -> Self {
        // Safety: this is safe as we erase the sub_id lifetim.
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
    unsafe fn get_untaged(&self) -> &RepeaterWithinComponent<'id, 'static> {
        &self.0
    }
}

type Callback = sixtyfps_corelib::Callback<[Value], Value>;

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
    item_tree: Vec<ItemTreeNode<crate::dynamic_type::Instance<'id>>>,
    pub(crate) items: HashMap<String, ItemWithinComponent>,
    pub(crate) custom_properties: HashMap<String, PropertiesWithinComponent>,
    pub(crate) custom_callbacks: HashMap<String, FieldOffset<Instance<'id>, Callback>>,
    repeater: Vec<ErasedRepeaterWithinComponent<'id>>,
    /// Map the Element::id of the repeater to the index in the `repeater` vec
    pub repeater_names: HashMap<String, usize>,
    /// Offset to a Option<ComponentPinRef>
    pub(crate) parent_component_offset:
        Option<FieldOffset<Instance<'id>, Option<ComponentRefPin<'id>>>>,
    /// Offset to the window reference
    pub(crate) window_offset:
        FieldOffset<Instance<'id>, Option<sixtyfps_corelib::window::ComponentWindow>>,
    /// Offset of a ComponentExtraData
    pub(crate) extra_data_offset: FieldOffset<Instance<'id>, ComponentExtraData>,
    /// Keep the Rc alive
    pub(crate) original: Rc<object_tree::Component>,
    // Copy of original.root_element.property_declarations, without a guarded refcell
    public_properties: HashMap<String, PropertyDeclaration>,
}

impl<'id> ComponentDescription<'id> {
    /// The name of this Component as written in the .60 file
    pub fn id(&self) -> &str {
        self.original.id.as_str()
    }

    /// List of publicly declared properties or callbacks
    pub fn properties(
        &self,
    ) -> impl ExactSizeIterator<Item = (String, sixtyfps_compilerlib::langtype::Type)> + '_ {
        self.public_properties.iter().map(|(s, v)| (s.clone(), v.property_type.clone()))
    }

    /// Instantiate a runtime component from this ComponentDescription
    pub fn create(
        self: Rc<Self>,
        #[cfg(target_arch = "wasm32")] canvas_id: String,
    ) -> vtable::VRc<ComponentVTable, ErasedComponentBox> {
        #[cfg(not(target_arch = "wasm32"))]
        let window = sixtyfps_rendering_backend_default::backend().create_window();
        #[cfg(target_arch = "wasm32")]
        let window = sixtyfps_rendering_backend_gl::create_gl_window_with_canvas_id(canvas_id);
        self.create_with_existing_window(window)
    }

    #[doc(hidden)]
    pub fn create_with_existing_window(
        self: Rc<Self>,
        window: sixtyfps_corelib::window::ComponentWindow,
    ) -> vtable::VRc<ComponentVTable, ErasedComponentBox> {
        let component_ref = instantiate(self, None, window);
        component_ref
            .as_pin_ref()
            .window()
            .set_component(&vtable::VRc::into_dyn(component_ref.clone()));
        component_ref.run_setup_code();
        component_ref
    }

    /// Set a value to property.
    ///
    /// Returns an error if the component is not an instance corresponding to this ComponentDescription,
    /// or if the property with this name does not exist in this component
    pub fn set_property(
        &self,
        component: ComponentRefPin,
        name: &str,
        value: Value,
    ) -> Result<(), ()> {
        if !core::ptr::eq((&self.ct) as *const _, component.get_vtable() as *const _) {
            return Err(());
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
        component: ComponentRef,
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
                    sixtyfps_corelib::rtti::AnimatedBindingKind::NotAnimated,
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
        let x = self.custom_callbacks.get(name).ok_or(())?;
        let sig = x.apply(unsafe { &*(component.as_ptr() as *const dynamic_type::Instance) });
        sig.set_handler(handler);
        Ok(())
    }

    /// Emits the specified callback
    ///
    /// Returns an error if the component is not an instance corresponding to this ComponentDescription,
    /// or if the callback with this name does not exist in this component
    pub fn invoke_callback(
        &self,
        component: ComponentRefPin,
        name: &str,
        args: &[Value],
    ) -> Result<Value, ()> {
        if !core::ptr::eq((&self.ct) as *const _, component.get_vtable() as *const _) {
            return Err(());
        }
        let x = self.custom_callbacks.get(name).ok_or(())?;
        let sig = x.apply(unsafe { &*(component.as_ptr() as *const dynamic_type::Instance) });
        Ok(sig.call(args))
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
    sixtyfps_corelib::item_tree::visit_item_tree(
        instance_ref.instance,
        &vtable::VRc::into_dyn(comp_rc),
        instance_ref.component_type.item_tree.as_slice().into(),
        index,
        order,
        v,
        |_, order, visitor, index| {
            // `ensure_updated` needs a 'static lifetime so we must call get_untaged.
            // Safety: we do not mix the component with other component id in this function
            let rep_in_comp = unsafe { instance_ref.component_type.repeater[index].get_untaged() };
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
        let window = instance_ref
            .component_type
            .window_offset
            .apply(instance_ref.as_ref())
            .as_ref()
            .unwrap()
            .clone();
        let instance = instantiate(
            rep_in_comp.component_to_repeat.clone(),
            Some(instance_ref.borrow()),
            window,
        );
        instance.run_setup_code();
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
        let assume_property_f32 =
            |prop| unsafe { Pin::new_unchecked(&*(prop as *const Property<f32>)) };
        let get_prop = |nr: &NamedReference| -> f32 {
            eval::load_property(instance_ref, &nr.element(), nr.name()).unwrap().try_into().unwrap()
        };
        repeater.ensure_updated_listview(
            init,
            assume_property_f32(get_property_ptr(&lv.viewport_width, instance_ref)),
            assume_property_f32(get_property_ptr(&lv.viewport_height, instance_ref)),
            assume_property_f32(get_property_ptr(&lv.viewport_y, instance_ref)),
            get_prop(&lv.listview_width),
            assume_property_f32(get_property_ptr(&lv.listview_height, instance_ref)),
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
pub async fn load<'id>(
    source: String,
    path: std::path::PathBuf,
    mut compiler_config: CompilerConfiguration,
    guard: generativity::Guard<'id>,
) -> (Result<Rc<ComponentDescription<'id>>, ()>, sixtyfps_compilerlib::diagnostics::BuildDiagnostics)
{
    if compiler_config.style.is_none() && std::env::var("SIXTYFPS_STYLE").is_err() {
        // Defaults to native if it exists:
        compiler_config.style = Some(if sixtyfps_rendering_backend_default::HAS_NATIVE_STYLE {
            "native".to_owned()
        } else {
            "ugly".to_owned()
        });
    }

    let mut diag = BuildDiagnostics::default();
    let syntax_node = parser::parse(source, Some(path.as_path()), &mut diag);
    if diag.has_error() {
        return (Err(()), diag);
    }
    let (doc, diag) = compile_syntax_node(syntax_node, diag, compiler_config).await;
    if diag.has_error() {
        return (Err(()), diag);
    }
    (Ok(generate_component(&doc.root_component, guard)), diag)
}

fn generate_component<'id>(
    component: &Rc<object_tree::Component>,
    guard: generativity::Guard<'id>,
) -> Rc<ComponentDescription<'id>> {
    let mut rtti = HashMap::new();
    {
        use sixtyfps_corelib::items::*;
        rtti.extend(
            [
                rtti_for::<Image>(),
                rtti_for::<ClippedImage>(),
                rtti_for::<Text>(),
                rtti_for::<Rectangle>(),
                rtti_for::<BorderRectangle>(),
                rtti_for::<TouchArea>(),
                rtti_for::<FocusScope>(),
                rtti_for::<Path>(),
                rtti_for::<Flickable>(),
                rtti_for::<Window>(),
                rtti_for::<TextInput>(),
                rtti_for::<Clip>(),
                rtti_for::<BoxShadow>(),
                rtti_for::<Rotate>(),
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
        sixtyfps_rendering_backend_default::NativeWidgets::push(&mut rtti);
    }
    let rtti = Rc::new(rtti);

    let mut tree_array = vec![];
    let mut items_types = HashMap::<String, ItemWithinComponent>::new();
    let mut builder = dynamic_type::TypeBuilder::new(guard);

    let mut repeater = vec![];
    let mut repeater_names = HashMap::new();

    generator::build_array_helper(component, |rc_item, child_offset, parent_index| {
        let item = rc_item.borrow();
        if let Some(repeated) = &item.repeated {
            tree_array.push(ItemTreeNode::DynamicTree { index: repeater.len(), parent_index });
            let base_component = item.base_type.as_component();
            repeater_names.insert(item.id.clone(), repeater.len());
            generativity::make_guard!(guard);
            repeater.push(
                RepeaterWithinComponent {
                    component_to_repeat: generate_component(base_component, guard),
                    offset: builder.add_field_type::<Repeater<ErasedComponentBox>>(),
                    model: repeated.model.clone(),
                }
                .into(),
            );
        } else {
            let rt = rtti.get(&*item.base_type.as_native().class_name).unwrap_or_else(|| {
                panic!("Native type not registered: {}", item.base_type.as_native().class_name)
            });

            let offset = if item.is_flickable_viewport {
                let parent =
                    &items_types[&object_tree::find_parent_element(rc_item).unwrap().borrow().id];
                assert_eq!(
                    parent.elem.borrow().base_type.as_native().class_name.as_str(),
                    "Flickable"
                );
                parent.offset + Flickable::FIELD_OFFSETS.viewport.get_byte_offset()
            } else {
                builder.add_field(rt.type_info)
            };
            tree_array.push(ItemTreeNode::Item {
                item: unsafe { vtable::VOffset::from_raw(rt.vtable, offset) },
                children_index: child_offset,
                chilren_count: item.children.len() as u32,
                parent_index,
            });
            items_types.insert(
                item.id.clone(),
                ItemWithinComponent { offset, rtti: rt.clone(), elem: rc_item.clone() },
            );
        }
    });

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
            Type::Length => animated_property_info::<f32>(),
            Type::LogicalLength => animated_property_info::<f32>(),
            Type::Image => property_info::<ImageReference>(),
            Type::Bool => property_info::<bool>(),
            Type::Callback { .. } => {
                custom_callbacks.insert(name.clone(), builder.add_field_type::<Callback>());
                continue;
            }
            Type::Struct { name: Some(name), .. } if name.ends_with("::StateInfo") => {
                property_info::<sixtyfps_corelib::properties::StateInfo>()
            }
            Type::Struct { .. } => property_info::<Value>(),
            Type::Array(_) => property_info::<Value>(),
            Type::Percent => property_info::<f32>(),
            Type::Enumeration(e) => match e.name.as_ref() {
                "LayoutAlignment" => property_info::<sixtyfps_corelib::layout::LayoutAlignment>(),
                "TextHorizontalAlignment" => {
                    property_info::<sixtyfps_corelib::items::TextHorizontalAlignment>()
                }
                "TextVerticalAlignment" => {
                    property_info::<sixtyfps_corelib::items::TextVerticalAlignment>()
                }
                "TextWrap" => property_info::<sixtyfps_corelib::items::TextWrap>(),
                "TextOverflow" => property_info::<sixtyfps_corelib::items::TextOverflow>(),
                "ImageFit" => property_info::<sixtyfps_corelib::items::ImageFit>(),
                "FillRule" => property_info::<sixtyfps_corelib::items::FillRule>(),
                _ => panic!("unkown enum"),
            },
            _ => panic!("bad type"),
        };
        custom_properties.insert(
            name.clone(),
            PropertiesWithinComponent { offset: builder.add_field(type_info), prop },
        );
    }
    if component.parent_element.upgrade().is_some() {
        let (prop, type_info) = property_info::<u32>();
        custom_properties.insert(
            "index".into(),
            PropertiesWithinComponent { offset: builder.add_field(type_info), prop },
        );
        // FIXME: make it a property for the correct type instead of being generic
        let (prop, type_info) = property_info::<Value>();
        custom_properties.insert(
            "model_data".into(),
            PropertiesWithinComponent { offset: builder.add_field(type_info), prop },
        );
    } else {
        let (prop, type_info) = property_info::<f32>();
        custom_properties.insert(
            "scale_factor".into(),
            PropertiesWithinComponent { offset: builder.add_field(type_info), prop },
        );
    }

    let parent_component_offset = if component.parent_element.upgrade().is_some() {
        Some(builder.add_field_type::<Option<ComponentRefPin>>())
    } else {
        None
    };

    let window_offset = builder.add_field_type::<Option<ComponentWindow>>();

    let extra_data_offset = builder.add_field_type::<ComponentExtraData>();

    let public_properties = component.root_element.borrow().property_declarations.clone();

    let t = ComponentVTable {
        visit_children_item,
        layout_info,
        apply_layout,
        get_item_ref,
        parent_item,
        drop_in_place,
        dealloc,
    };
    let t = ComponentDescription {
        ct: t,
        dynamic_type: builder.build(),
        item_tree: tree_array,
        items: items_types,
        custom_properties,
        custom_callbacks,
        original: component.clone(),
        repeater,
        repeater_names,
        parent_component_offset,
        window_offset,
        extra_data_offset,
        public_properties,
    };

    Rc::new(t)
}

pub fn animation_for_property(
    component: InstanceRef,
    element: &Element,
    property_name: &str,
) -> AnimatedBindingKind {
    match element.property_animations.get(property_name) {
        Some(sixtyfps_compilerlib::object_tree::PropertyAnimation::Static(anim_elem)) => {
            AnimatedBindingKind::Animation(eval::new_struct_with_bindings(
                &anim_elem.borrow().bindings,
                &mut eval::EvalLocalContext::from_component_instance(component),
            ))
        }
        Some(sixtyfps_compilerlib::object_tree::PropertyAnimation::Transition {
            animations,
            state_ref,
        }) => {
            let component_ptr = component.as_ptr();
            let vtable = NonNull::from(&component.component_type.ct).cast();
            let animations = animations.clone();
            let state_ref = state_ref.clone();
            AnimatedBindingKind::Transition(Box::new(
                move || -> (PropertyAnimation, sixtyfps_corelib::animations::Instant) {
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
                    let state_info: sixtyfps_corelib::properties::StateInfo =
                        state.try_into().unwrap();
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
                    return Default::default();
                },
            ))
        }
        None => AnimatedBindingKind::NotAnimated,
    }
}

pub fn instantiate<'id>(
    component_type: Rc<ComponentDescription<'id>>,
    parent_ctx: Option<ComponentRefPin>,
    window: sixtyfps_corelib::window::ComponentWindow,
) -> vtable::VRc<ComponentVTable, ErasedComponentBox> {
    let mut instance = component_type.dynamic_type.clone().create_instance();

    if let Some(parent) = parent_ctx {
        *component_type.parent_component_offset.unwrap().apply_mut(instance.as_mut()) =
            Some(parent);
    } else {
        let extra_data = component_type.extra_data_offset.apply_mut(instance.as_mut());
        extra_data.globals = component_type
            .original
            .used_global
            .borrow()
            .iter()
            .map(|g| (g.id.clone(), crate::global_component::instantiate(g)))
            .collect();
    }
    *component_type.window_offset.apply_mut(instance.as_mut()) = Some(window);

    let component_box = ComponentBox { instance, component_type: component_type.clone() };
    let instance_ref = component_box.borrow_instance();

    sixtyfps_corelib::component::init_component_items(
        instance_ref.instance,
        instance_ref.component_type.item_tree.as_slice().into(),
        &eval::window_ref(instance_ref).unwrap(),
    );

    for item_within_component in component_type.items.values() {
        unsafe {
            let item = item_within_component.item_from_component(instance_ref.as_ptr());
            let elem = item_within_component.elem.borrow();
            for (unresolved_prop_name, expr) in &elem.bindings {
                let PropertyLookupResult { resolved_name, property_type } =
                    elem.lookup_property(unresolved_prop_name.as_str());
                if let Type::Callback { .. } = property_type {
                    let expr = expr.clone();
                    let component_type = component_type.clone();
                    let instance = component_box.instance.as_ptr();
                    let c = Pin::new_unchecked(vtable::VRef::from_raw(
                        NonNull::from(&component_type.ct).cast(),
                        instance.cast(),
                    ));
                    if let Some(callback) =
                        item_within_component.rtti.callbacks.get(resolved_name.as_ref())
                    {
                        callback.set_handler(
                            item,
                            Box::new(move |args| {
                                generativity::make_guard!(guard);
                                let mut local_context =
                                    eval::EvalLocalContext::from_function_arguments(
                                        InstanceRef::from_pin_ref(c, guard),
                                        args.iter().cloned().collect(),
                                    );
                                eval::eval_expression(&expr, &mut local_context)
                            }),
                        )
                    } else if let Some(callback_offset) =
                        component_type.custom_callbacks.get(resolved_name.as_ref())
                    {
                        let callback = callback_offset.apply(instance_ref.as_ref());
                        callback.set_handler(move |args| {
                            generativity::make_guard!(guard);
                            let mut local_context = eval::EvalLocalContext::from_function_arguments(
                                InstanceRef::from_pin_ref(c, guard),
                                args.iter().cloned().collect(),
                            );
                            eval::eval_expression(&expr, &mut local_context)
                        })
                    } else {
                        panic!("unkown callback {}", unresolved_prop_name)
                    }
                } else {
                    if let Some(prop_rtti) =
                        item_within_component.rtti.properties.get(resolved_name.as_ref())
                    {
                        let maybe_animation =
                            animation_for_property(instance_ref, &elem, resolved_name.as_ref());
                        let mut e = Some(&expr.expression);
                        while let Some(Expression::TwoWayBinding(nr, next)) = &e {
                            // Safety: The compiler must have ensured that the properties exist and are of the same type
                            prop_rtti.link_two_ways(item, get_property_ptr(&nr, instance_ref));
                            e = next.as_deref();
                        }
                        if let Some(e) = e {
                            if e.is_constant() {
                                prop_rtti.set(
                                    item,
                                    eval::eval_expression(
                                        e,
                                        &mut eval::EvalLocalContext::from_component_instance(
                                            instance_ref,
                                        ),
                                    ),
                                    maybe_animation.as_animation(),
                                );
                            } else {
                                let e = e.clone();
                                let component_type = component_type.clone();
                                let instance = component_box.instance.as_ptr();
                                let c = Pin::new_unchecked(vtable::VRef::from_raw(
                                    NonNull::from(&component_type.ct).cast(),
                                    instance.cast(),
                                ));

                                prop_rtti.set_binding(
                                    item,
                                    Box::new(move || {
                                        generativity::make_guard!(guard);
                                        eval::eval_expression(
                                            &e,
                                            &mut eval::EvalLocalContext::from_component_instance(
                                                InstanceRef::from_pin_ref(c, guard),
                                            ),
                                        )
                                    }),
                                    maybe_animation,
                                );
                            }
                        }
                    } else if let Some(PropertiesWithinComponent {
                        offset, prop: prop_info, ..
                    }) = component_type.custom_properties.get(resolved_name.as_ref())
                    {
                        let c = Pin::new_unchecked(vtable::VRef::from_raw(
                            NonNull::from(&component_type.ct).cast(),
                            component_box.instance.as_ptr().cast(),
                        ));

                        let is_state_info = match property_type {
                            Type::Struct { name: Some(name), .. }
                                if name.ends_with("::StateInfo") =>
                            {
                                true
                            }
                            _ => false,
                        };
                        if is_state_info {
                            let prop = Pin::new_unchecked(
                                &*(instance_ref.as_ptr().add(*offset)
                                    as *const Property<sixtyfps_corelib::properties::StateInfo>),
                            );
                            let e = expr.expression.clone();
                            sixtyfps_corelib::properties::set_state_binding(prop, move || {
                                generativity::make_guard!(guard);
                                eval::eval_expression(
                                    &e,
                                    &mut eval::EvalLocalContext::from_component_instance(
                                        InstanceRef::from_pin_ref(c, guard),
                                    ),
                                )
                                .try_into()
                                .unwrap()
                            });
                            continue;
                        }

                        let maybe_animation = animation_for_property(
                            instance_ref,
                            &component_type.original.root_element.borrow(),
                            resolved_name.as_ref(),
                        );
                        let item = Pin::new_unchecked(&*instance_ref.as_ptr().add(*offset));

                        let mut e = Some(&expr.expression);
                        while let Some(Expression::TwoWayBinding(nr, next)) = &e {
                            // Safety: The compiler must have ensured that the properties exist and are of the same type
                            prop_info.link_two_ways(item, get_property_ptr(&nr, instance_ref));
                            e = next.as_deref();
                        }
                        if let Some(e) = e {
                            if e.is_constant() {
                                let v = eval::eval_expression(
                                    e,
                                    &mut eval::EvalLocalContext::from_component_instance(
                                        instance_ref,
                                    ),
                                );
                                prop_info.set(item, v, None).unwrap();
                            } else {
                                let e = e.clone();
                                prop_info
                                    .set_binding(
                                        item,
                                        Box::new(move || {
                                            generativity::make_guard!(guard);
                                            eval::eval_expression(
                                                &e,
                                                &mut eval::EvalLocalContext::from_component_instance(InstanceRef::from_pin_ref(c, guard)),
                                            )
                                        }),
                                        maybe_animation,
                                    )
                                    .unwrap();
                            }
                        }
                    } else {
                        panic!("unkown property {}", unresolved_prop_name);
                    }
                }
            }
        }
    }

    for rep_in_comp in &component_type.repeater {
        generativity::make_guard!(guard);
        let rep_in_comp = rep_in_comp.unerase(guard);

        let repeater = rep_in_comp.offset.apply_pin(instance_ref.instance);
        let expr = rep_in_comp.model.clone();
        let component_type = component_type.clone();
        let instance = component_box.instance.as_ptr();
        let c = unsafe {
            Pin::new_unchecked(vtable::VRef::from_raw(
                NonNull::from(&component_type.ct).cast(),
                instance.cast(),
            ))
        };
        repeater.set_model_binding(move || {
            generativity::make_guard!(guard);
            let m = eval::eval_expression(
                &expr,
                &mut eval::EvalLocalContext::from_component_instance(unsafe {
                    InstanceRef::from_pin_ref(c, guard)
                }),
            );
            sixtyfps_corelib::model::ModelHandle(Some(Rc::new(
                crate::value_model::ValueModel::new(m),
            )))
        });
    }

    let comp_rc = vtable::VRc::new(ErasedComponentBox::from(component_box));
    {
        generativity::make_guard!(guard);
        let comp = comp_rc.unerase(guard);
        let weak = vtable::VRc::downgrade(&comp_rc);
        let instance_ref = comp.borrow_instance();
        instance_ref.self_weak().set(weak).ok();
    }

    comp_rc
}

fn get_property_ptr(nr: &NamedReference, instance: InstanceRef) -> *const () {
    let element = nr.element();
    generativity::make_guard!(guard);
    let enclosing_component = eval::enclosing_component_for_element(&element, instance, guard);
    let element = element.borrow();
    if element.id == element.enclosing_component.upgrade().unwrap().root_element.borrow().id {
        if let Some(x) = enclosing_component.component_type.custom_properties.get(nr.name()) {
            return unsafe { enclosing_component.as_ptr().add(x.offset).cast() };
        };
    };
    let item_info = enclosing_component
        .component_type
        .items
        .get(element.id.as_str())
        .unwrap_or_else(|| panic!("Unkown element for {}.{}", element.id, nr.name()));
    core::mem::drop(element);
    let item = unsafe { item_info.item_from_component(enclosing_component.as_ptr()) };
    unsafe { item.as_ptr().add(item_info.rtti.properties.get(nr.name()).unwrap().offset()).cast() }
}

use sixtyfps_corelib::layout::*;

struct LayoutWithCells<'a, C> {
    geometry: &'a sixtyfps_compilerlib::layout::LayoutGeometry,
    cells: Vec<C>,
    spacing: f32,
    padding: Padding,
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

    pub fn borrow<'a>(&'a self) -> ComponentRefPin<'a> {
        // Safety: it is safe to access self.0 here because the 'id lifetime does not leak
        self.0.borrow()
    }

    pub fn window(&self) -> ComponentWindow {
        self.0.window()
    }

    pub fn run_setup_code(&self) {
        generativity::make_guard!(guard);
        let compo_box = self.unerase(guard);
        let instance_ref = compo_box.borrow_instance();
        for extra_init_code in self.0.component_type.original.setup_code.borrow().iter() {
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

type RepeatedComponentRc = vtable::VRc<ComponentVTable, ErasedComponentBox>;
enum BoxLayoutCellTmpData<'a> {
    Item(BoxLayoutCellData<'a>),
    Repeater(Vec<RepeatedComponentRc>),
}

impl<'a> BoxLayoutCellTmpData<'a> {
    fn into_cells(cells: &'a [Self]) -> Vec<BoxLayoutCellData<'a>> {
        let mut c = Vec::with_capacity(cells.len());
        for x in cells.iter() {
            match x {
                BoxLayoutCellTmpData::Item(cell) => {
                    c.push((*cell).clone());
                }
                BoxLayoutCellTmpData::Repeater(vec) => {
                    c.extend(vec.iter().map(|x| x.as_pin_ref().box_layout_data()))
                }
            }
        }
        c
    }
}

#[derive(derive_more::From)]
enum LayoutTreeItem<'a> {
    GridLayout(LayoutWithCells<'a, GridLayoutCellData<'a>>),
    BoxLayout(
        LayoutWithCells<'a, BoxLayoutCellTmpData<'a>>,
        bool,
        sixtyfps_corelib::layout::LayoutAlignment,
    ),
    PathLayout(&'a PathLayout),
}

impl<'a> LayoutTreeItem<'a> {
    fn layout_info(&self) -> LayoutInfo {
        match self {
            LayoutTreeItem::GridLayout(grid_layout) => grid_layout_info(
                &Slice::from(grid_layout.cells.as_slice()),
                grid_layout.spacing,
                &grid_layout.padding,
            ),
            LayoutTreeItem::BoxLayout(box_layout, is_horizontal, alignment) => {
                let cells = BoxLayoutCellTmpData::into_cells(&box_layout.cells);
                box_layout_info(
                    &Slice::from(cells.as_slice()),
                    box_layout.spacing,
                    &box_layout.padding,
                    *alignment,
                    *is_horizontal,
                )
            }
            LayoutTreeItem::PathLayout(_) => todo!(),
        }
    }

    fn geometry(&self) -> Option<&LayoutGeometry> {
        match self {
            Self::GridLayout(LayoutWithCells { geometry, .. })
            | Self::BoxLayout(LayoutWithCells { geometry, .. }, _, _) => Some(geometry),
            _ => None,
        }
    }
}

fn get_layout_info<'a, 'b>(
    item: &'a LayoutItem,
    component: InstanceRef<'a, '_>,
    layout_tree: &'b mut Vec<LayoutTreeItem<'a>>,
    window: &ComponentWindow,
) -> LayoutInfo {
    let layout_info = item.layout.as_ref().map(|l| {
        let layout_tree_item = collect_layouts_recursively(layout_tree, l, component, window);
        layout_tree_item.layout_info()
    });
    let elem_info = item.element.as_ref().map(|elem| {
        let item = &component
            .component_type
            .items
            .get(elem.borrow().id.as_str())
            .unwrap_or_else(|| panic!("Internal error: Item {} not found", elem.borrow().id));
        unsafe { item.item_from_component(component.as_ptr()).as_ref().layouting_info(window) }
    });

    match (layout_info, elem_info) {
        (None, None) => Default::default(),
        (None, Some(x)) => x,
        (Some(x), None) => x,
        (Some(layout_info), Some(elem_info)) => layout_info.merge(&elem_info),
    }
}

fn fill_layout_info_constraints(
    layout_info: &mut LayoutInfo,
    constraints: &LayoutConstraints,
    expr_eval: &impl Fn(&NamedReference) -> f32,
) {
    let is_percent =
        |nr: &NamedReference| Expression::PropertyReference(nr.clone()).ty() == Type::Percent;
    constraints.minimum_width.as_ref().map(|e| {
        if !is_percent(e) {
            layout_info.min_width = expr_eval(e)
        } else {
            layout_info.min_width_percent = expr_eval(e)
        }
    });
    constraints.maximum_width.as_ref().map(|e| {
        if !is_percent(e) {
            layout_info.max_width = expr_eval(e)
        } else {
            layout_info.max_width_percent = expr_eval(e)
        }
    });
    constraints.minimum_height.as_ref().map(|e| {
        if !is_percent(e) {
            layout_info.min_height = expr_eval(e)
        } else {
            layout_info.min_height_percent = expr_eval(e)
        }
    });
    constraints.maximum_height.as_ref().map(|e| {
        if !is_percent(e) {
            layout_info.max_height = expr_eval(e)
        } else {
            layout_info.max_height_percent = expr_eval(e)
        }
    });
    constraints.preferred_width.as_ref().map(|e| {
        layout_info.preferred_width = expr_eval(e);
    });
    constraints.preferred_height.as_ref().map(|e| {
        layout_info.preferred_height = expr_eval(e);
    });
    constraints.horizontal_stretch.as_ref().map(|e| layout_info.horizontal_stretch = expr_eval(e));
    constraints.vertical_stretch.as_ref().map(|e| layout_info.vertical_stretch = expr_eval(e));
}

fn collect_layouts_recursively<'a, 'b>(
    layout_tree: &'b mut Vec<LayoutTreeItem<'a>>,
    layout: &'a Layout,
    component: InstanceRef<'a, '_>,
    window: &ComponentWindow,
) -> &'b LayoutTreeItem<'a> {
    let assume_property_f32 = |nr: &Option<NamedReference>| {
        nr.as_ref().map(|nr| {
            let p = get_property_ptr(nr, component);
            unsafe { &*(p as *const Property<f32>) }
        })
    };
    let expr_eval = |nr: &NamedReference| -> f32 {
        eval::load_property(component, &nr.element(), nr.name()).unwrap().try_into().unwrap()
    };

    match layout {
        Layout::GridLayout(grid_layout) => {
            let cells = grid_layout
                .elems
                .iter()
                .map(|cell| {
                    let mut layout_info =
                        get_layout_info(&cell.item, component, layout_tree, window);
                    fill_layout_info_constraints(
                        &mut layout_info,
                        &cell.item.constraints,
                        &expr_eval,
                    );
                    let rect = cell.item.rect();

                    GridLayoutCellData {
                        x: assume_property_f32(&rect.x_reference),
                        y: assume_property_f32(&rect.y_reference),
                        width: assume_property_f32(&rect.width_reference),
                        height: assume_property_f32(&rect.height_reference),
                        col: cell.col,
                        row: cell.row,
                        colspan: cell.colspan,
                        rowspan: cell.rowspan,
                        constraint: layout_info,
                    }
                })
                .collect();
            let spacing = grid_layout.geometry.spacing.as_ref().map_or(0., expr_eval);
            let padding = Padding {
                left: grid_layout.geometry.padding.left.as_ref().map_or(0., expr_eval),
                right: grid_layout.geometry.padding.right.as_ref().map_or(0., expr_eval),
                top: grid_layout.geometry.padding.top.as_ref().map_or(0., expr_eval),
                bottom: grid_layout.geometry.padding.bottom.as_ref().map_or(0., expr_eval),
            };
            layout_tree.push(
                LayoutWithCells { geometry: &grid_layout.geometry, cells, spacing, padding }.into(),
            );
        }
        Layout::BoxLayout(box_layout) => {
            let mut make_box_layout_cell_data = |cell| {
                let mut layout_info = get_layout_info(cell, component, layout_tree, window);
                fill_layout_info_constraints(&mut layout_info, &cell.constraints, &expr_eval);
                let rect = cell.rect();

                BoxLayoutCellData {
                    x: assume_property_f32(&rect.x_reference),
                    y: assume_property_f32(&rect.y_reference),
                    width: assume_property_f32(&rect.width_reference),
                    height: assume_property_f32(&rect.height_reference),
                    constraint: layout_info,
                }
            };

            let cells = box_layout
                .elems
                .iter()
                .map(|item| match &item.element {
                    Some(elem) if elem.borrow().repeated.is_some() => {
                        generativity::make_guard!(guard);
                        let rep = get_repeater_by_name(component, elem.borrow().id.as_str(), guard);
                        rep.0.as_ref().ensure_updated(|| {
                            let window = component
                                .component_type
                                .window_offset
                                .apply(component.as_ref())
                                .as_ref()
                                .unwrap()
                                .clone();
                            let instance =
                                instantiate(rep.1.clone(), Some(component.borrow()), window);
                            instance.run_setup_code();
                            instance
                        });

                        BoxLayoutCellTmpData::Repeater(
                            rep.0.as_ref().components_vec().into_iter().collect(),
                        )
                    }
                    _ => BoxLayoutCellTmpData::Item(make_box_layout_cell_data(item)),
                })
                .collect::<Vec<_>>();

            let spacing = box_layout.geometry.spacing.as_ref().map_or(0., expr_eval);
            let padding = Padding {
                left: box_layout.geometry.padding.left.as_ref().map_or(0., expr_eval),
                right: box_layout.geometry.padding.right.as_ref().map_or(0., expr_eval),
                top: box_layout.geometry.padding.top.as_ref().map_or(0., expr_eval),
                bottom: box_layout.geometry.padding.bottom.as_ref().map_or(0., expr_eval),
            };
            let alignment = box_layout
                .geometry
                .alignment
                .as_ref()
                .map(|nr| {
                    eval::load_property(component, &nr.element(), nr.name())
                        .unwrap()
                        .try_into()
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            layout_tree.push(LayoutTreeItem::BoxLayout(
                LayoutWithCells { geometry: &box_layout.geometry, cells, spacing, padding },
                box_layout.is_horizontal,
                alignment,
            ));
        }
        Layout::PathLayout(layout) => layout_tree.push(layout.into()),
    }
    layout_tree.last().unwrap()
}

impl<'a> LayoutTreeItem<'a> {
    fn solve(&self, instance_ref: InstanceRef) {
        let resolve_prop_ref = |prop_ref: &Option<NamedReference>| {
            prop_ref.as_ref().map_or(0., |nr| {
                eval::load_property(instance_ref, &nr.element(), nr.name())
                    .unwrap()
                    .try_into()
                    .unwrap_or(0.)
            })
        };

        if let Some(geometry) = self.geometry() {
            // Set the properties that depends on the constraints
            if geometry.materialized_constraints.has_explicit_restrictions() {
                let info = self.layout_info();
                let apply_materialized_constraint = |nr: &Option<NamedReference>, c: f32| {
                    if let Some(nr) = nr {
                        let p = get_property_ptr(nr, instance_ref);
                        let p_ref = unsafe { &*(p as *const Property<f32>) };
                        p_ref.set(c);
                    };
                };
                let c = &geometry.materialized_constraints;
                apply_materialized_constraint(&c.minimum_width, info.min_width);
                apply_materialized_constraint(&c.minimum_height, info.min_height);
                apply_materialized_constraint(&c.maximum_width, info.max_width);
                apply_materialized_constraint(&c.maximum_height, info.max_height);
                apply_materialized_constraint(&c.vertical_stretch, info.vertical_stretch);
                apply_materialized_constraint(&c.horizontal_stretch, info.horizontal_stretch);
            }
        }

        match self {
            Self::GridLayout(grid_layout) => {
                solve_grid_layout(&GridLayoutData {
                    width: resolve_prop_ref(&grid_layout.geometry.rect.width_reference),
                    height: resolve_prop_ref(&grid_layout.geometry.rect.height_reference),
                    x: resolve_prop_ref(&grid_layout.geometry.rect.x_reference),
                    y: resolve_prop_ref(&grid_layout.geometry.rect.y_reference),
                    spacing: grid_layout.spacing,
                    padding: &grid_layout.padding,
                    cells: Slice::from(grid_layout.cells.as_slice()),
                });
            }
            Self::BoxLayout(box_layout, is_horizontal, alignment) => {
                let cells = BoxLayoutCellTmpData::into_cells(&box_layout.cells);
                solve_box_layout(
                    &BoxLayoutData {
                        width: resolve_prop_ref(&box_layout.geometry.rect.width_reference),
                        height: resolve_prop_ref(&box_layout.geometry.rect.height_reference),
                        x: resolve_prop_ref(&box_layout.geometry.rect.x_reference),
                        y: resolve_prop_ref(&box_layout.geometry.rect.y_reference),
                        spacing: box_layout.spacing,
                        padding: &box_layout.padding,
                        cells: Slice::from(cells.as_slice()),
                        alignment: *alignment,
                    },
                    *is_horizontal,
                );
            }
            Self::PathLayout(path_layout) => {
                use sixtyfps_corelib::layout::*;

                let mut items = vec![];
                for elem in &path_layout.elements {
                    let mut push_layout_data = |elem: &ElementRc, instance_ref: InstanceRef| {
                        let item_info =
                            &instance_ref.component_type.items[elem.borrow().id.as_str()];
                        let get_prop = |name| {
                            item_info.rtti.properties.get(name).map(|p| unsafe {
                                &*(instance_ref.as_ptr().add(item_info.offset).add(p.offset())
                                    as *const Property<f32>)
                            })
                        };

                        let item = unsafe { item_info.item_from_component(instance_ref.as_ptr()) };
                        let get_prop_value = |name| {
                            item_info
                                .rtti
                                .properties
                                .get(name)
                                .map(|p| p.get(item))
                                .unwrap_or_default()
                        };
                        items.push(PathLayoutItemData {
                            x: get_prop("x"),
                            y: get_prop("y"),
                            width: get_prop_value("width").try_into().unwrap_or_default(),
                            height: get_prop_value("height").try_into().unwrap_or_default(),
                        });
                    };

                    if elem.borrow().repeated.is_none() {
                        push_layout_data(elem, instance_ref)
                    } else {
                        generativity::make_guard!(guard);
                        let repeater =
                            get_repeater_by_name(instance_ref, elem.borrow().id.as_str(), guard);
                        let vec = repeater.0.components_vec();
                        for sub_comp in vec.iter() {
                            generativity::make_guard!(guard);
                            push_layout_data(
                                &elem.borrow().base_type.as_component().root_element,
                                sub_comp.unerase(guard).borrow_instance(),
                            )
                        }
                    }
                }

                let path_elements = eval::convert_path(
                    &path_layout.path,
                    &mut eval::EvalLocalContext::from_component_instance(instance_ref),
                );

                solve_path_layout(&PathLayoutData {
                    items: Slice::from(items.as_slice()),
                    elements: &path_elements,
                    x: resolve_prop_ref(&path_layout.rect.x_reference),
                    y: resolve_prop_ref(&path_layout.rect.y_reference),
                    width: resolve_prop_ref(&path_layout.rect.width_reference),
                    height: resolve_prop_ref(&path_layout.rect.height_reference),
                    offset: resolve_prop_ref(&Some(path_layout.offset_reference.clone())),
                });
            }
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

extern "C" fn layout_info(component: ComponentRefPin) -> LayoutInfo {
    generativity::make_guard!(guard);
    // This is fine since we can only be called with a component that with our vtable which is a ComponentDescription
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };

    let component_layouts = instance_ref.component_type.original.layouts.borrow();
    let result = if let Some(idx) = component_layouts.main_layout {
        let mut inverse_layout_tree = Default::default();
        collect_layouts_recursively(
            &mut inverse_layout_tree,
            &component_layouts[idx],
            instance_ref,
            &eval::window_ref(instance_ref).unwrap(),
        )
        .layout_info()
    } else {
        instance_ref.root_item().as_ref().layouting_info(&eval::window_ref(instance_ref).unwrap())
    };
    if component_layouts.root_constraints.has_explicit_restrictions() {
        let mut info = LayoutInfo::default();
        fill_layout_info_constraints(
            &mut info,
            &component_layouts.root_constraints,
            &|nr: &NamedReference| {
                eval::load_property(instance_ref, &nr.element(), nr.name())
                    .unwrap()
                    .try_into()
                    .unwrap()
            },
        );
        result.merge(&info)
    } else {
        result
    }
}

extern "C" fn apply_layout(component: ComponentRefPin, _r: sixtyfps_corelib::graphics::Rect) {
    generativity::make_guard!(guard);
    // This is fine since we can only be called with a component that with our vtable which is a ComponentDescription
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };
    let window = eval::window_ref(instance_ref).unwrap();

    instance_ref.component_type.original.layouts.borrow().iter().for_each(|layout| {
        let mut inverse_layout_tree = Vec::new();

        collect_layouts_recursively(&mut inverse_layout_tree, &layout, instance_ref, &window);

        inverse_layout_tree.iter().rev().for_each(|layout| {
            layout.solve(instance_ref);
        });
    });

    for rep_in_comp in &instance_ref.component_type.repeater {
        generativity::make_guard!(g);
        let rep_in_comp = rep_in_comp.unerase(g);
        ensure_repeater_updated(instance_ref, rep_in_comp);
        rep_in_comp.offset.apply_pin(instance_ref.instance).compute_layout();
    }
}

unsafe extern "C" fn get_item_ref(component: ComponentRefPin, index: usize) -> Pin<ItemRef> {
    generativity::make_guard!(guard);
    let instance_ref = InstanceRef::from_pin_ref(component, guard);
    match &instance_ref.component_type.item_tree.as_slice()[index] {
        ItemTreeNode::Item { item, .. } => core::mem::transmute::<Pin<ItemRef>, Pin<ItemRef>>(
            item.apply_pin(instance_ref.instance),
        ),
        ItemTreeNode::DynamicTree { .. } => panic!("get_item_ref called on dynamic tree"),
    }
}

unsafe extern "C" fn parent_item(component: ComponentRefPin, index: usize, result: &mut ItemWeak) {
    generativity::make_guard!(guard);
    let instance_ref = InstanceRef::from_pin_ref(component, guard);
    if index == 0 {
        let parent_item_index = instance_ref
            .component_type
            .original
            .parent_element
            .upgrade()
            .and_then(|e| e.borrow().item_index.get().map(|x| *x));
        if let (Some(parent_offset), Some(parent_index)) =
            (instance_ref.component_type.parent_component_offset, parent_item_index)
        {
            if let Some(parent) = parent_offset.apply(instance_ref.as_ref()) {
                generativity::make_guard!(new_guard);
                let parent_instance = InstanceRef::from_pin_ref(*parent, new_guard);
                let parent_rc = parent_instance
                    .self_weak()
                    .get()
                    .unwrap()
                    .clone()
                    .into_dyn()
                    .upgrade()
                    .unwrap();
                *result = ItemRc::new(parent_rc, parent_index).parent_item();
            };
        }
        return;
    }
    let parent_index = match &instance_ref.component_type.item_tree.as_slice()[index] {
        ItemTreeNode::Item { parent_index, .. } => parent_index,
        ItemTreeNode::DynamicTree { parent_index, .. } => parent_index,
    };
    let self_rc = instance_ref.self_weak().get().unwrap().clone().into_dyn().upgrade().unwrap();
    *result = ItemRc::new(self_rc, *parent_index as _).downgrade();
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
        &*self.instance
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

    pub fn root_item(&self) -> Pin<ItemRef> {
        let info = &self.component_type.items
            [self.component_type.original.root_element.borrow().id.as_str()];
        unsafe { info.item_from_component(self.as_ptr()) }
    }

    pub fn self_weak(
        &self,
    ) -> &once_cell::unsync::OnceCell<vtable::VWeak<ComponentVTable, ErasedComponentBox>> {
        let extra_data = self.component_type.extra_data_offset.apply(self.as_ref());
        &extra_data.self_weak
    }
}

/// Show the popup at the given location
pub fn show_popup(
    popup: &object_tree::PopupWindow,
    x: f32,
    y: f32,
    parent_comp: ComponentRefPin,
    parent_window: ComponentWindow,
) {
    generativity::make_guard!(guard);
    // FIXME: we should compile once and keep the cached compiled component
    let compiled = generate_component(&popup.component, guard);
    let window = sixtyfps_rendering_backend_default::backend().create_window();
    let inst = instantiate(compiled, Some(parent_comp), window);
    inst.run_setup_code();
    parent_window
        .show_popup(&vtable::VRc::into_dyn(inst), sixtyfps_corelib::graphics::Point::new(x, y));
}
