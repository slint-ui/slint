/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use crate::{dynamic_type, eval};

use core::convert::TryInto;
use core::ptr::NonNull;
use dynamic_type::{Instance, InstanceBox};
use expression_tree::NamedReference;
use object_tree::{Element, ElementRc};
use sixtyfps_compilerlib::expression_tree::Expression;
use sixtyfps_compilerlib::layout::{GridLayout, Layout, LayoutElement, LayoutItem, PathLayout};
use sixtyfps_compilerlib::typeregister::Type;
use sixtyfps_compilerlib::*;
use sixtyfps_corelib::component::{Component, ComponentRefPin, ComponentVTable};
use sixtyfps_corelib::graphics::Resource;
use sixtyfps_corelib::input::{FocusEventResult, KeyEventResult};
use sixtyfps_corelib::item_tree::{
    ItemTreeNode, ItemVisitorRefMut, TraversalOrder, VisitChildrenResult,
};
use sixtyfps_corelib::items::{Flickable, ItemRef, ItemVTable, PropertyAnimation, Rectangle};
use sixtyfps_corelib::layout::{LayoutInfo, Padding};
use sixtyfps_corelib::model::Repeater;
use sixtyfps_corelib::properties::InterpolatedPropertyValue;
use sixtyfps_corelib::rtti::{self, FieldOffset, PropertyInfo};
use sixtyfps_corelib::slice::Slice;
use sixtyfps_corelib::{eventloop::ComponentWindow, input::FocusEvent};
use sixtyfps_corelib::{Color, Property, SharedString, Signal};
use std::collections::HashMap;
use std::{cell::RefCell, pin::Pin, rc::Rc};

pub struct ComponentBox<'id> {
    instance: InstanceBox<'id>,
    component_type: Rc<ComponentDescription<'id>>,
}

impl<'id> ComponentBox<'id> {
    /// Borrow this component as a `Pin<ComponentRef>`
    pub fn borrow(&self) -> ComponentRefPin {
        unsafe {
            Pin::new_unchecked(vtable::VRef::from_raw(
                NonNull::from(&self.component_type.ct).cast(),
                self.instance.as_ptr().cast(),
            ))
        }
    }

    /// Safety: the lifetime is not unique
    pub fn description(&self) -> Rc<ComponentDescription<'id>> {
        return self.component_type.clone();
    }

    pub fn root_item(&self) -> Pin<ItemRef> {
        let component = self.borrow();
        let component_type = unsafe {
            &*(component.get_vtable() as *const ComponentVTable as *const ComponentDescription)
        };

        let info = &component_type.items[component_type.original.root_element.borrow().id.as_str()];

        unsafe { info.item_from_component(component.as_ptr()) }
    }

    pub fn borrow_instance<'a>(&'a self) -> InstanceRef<'a, 'id> {
        InstanceRef { instance: self.instance.as_pin_ref(), component_type: &self.component_type }
    }

    pub fn window(&self) -> sixtyfps_corelib::eventloop::ComponentWindow {
        self.component_type
            .extra_data_offset
            .apply_pin(self.instance.as_pin_ref())
            .window
            .borrow()
            .as_ref()
            .unwrap()
            .clone()
    }
}

impl<'id> Drop for ComponentBox<'id> {
    fn drop(&mut self) {
        match eval::window_ref(self.borrow_instance()) {
            Some(window) => {
                window.free_graphics_resources(self.borrow());
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
}

pub(crate) struct PropertiesWithinComponent {
    pub(crate) offset: usize,
    pub(crate) prop: Box<dyn PropertyInfo<u8, eval::Value>>,
}

pub(crate) struct RepeaterWithinComponent<'par_id, 'sub_id> {
    /// The component description of the items to repeat
    pub(crate) component_to_repeat: Rc<ComponentDescription<'sub_id>>,
    /// The model
    pub(crate) model: Expression,
    /// Offset of the `Repeater`
    pub(crate) offset: FieldOffset<Instance<'par_id>, Repeater<ComponentBox<'sub_id>>>,
}

impl<'id> sixtyfps_corelib::model::RepeatedComponent for ComponentBox<'id> {
    type Data = eval::Value;

    fn update(&self, index: usize, data: Self::Data) {
        self.component_type
            .set_property(self.borrow(), "index", index.try_into().unwrap())
            .unwrap();
        self.component_type.set_property(self.borrow(), "model_data", data).unwrap();
    }

    fn listview_layout(self: Pin<&Self>, offset_y: &mut f32, viewport_width: Pin<&Property<f32>>) {
        self.as_ref().compute_layout();
        self.component_type
            .set_property(self.borrow(), "y", eval::Value::Number(*offset_y as f64))
            .expect("cannot set y");
        let h: f32 = self
            .component_type
            .get_property(self.borrow(), "height")
            .expect("missing height")
            .try_into()
            .expect("height not the right type");
        let w: f32 = self
            .component_type
            .get_property(self.borrow(), "width")
            .expect("missing width")
            .try_into()
            .expect("width not the right type");
        *offset_y += h;
        let vp_w = viewport_width.get();
        if vp_w < w {
            viewport_width.set(w);
        }
    }
}

impl<'id> Component for ComponentBox<'id> {
    fn visit_children_item(
        self: ::core::pin::Pin<&Self>,
        index: isize,
        order: TraversalOrder,
        visitor: ItemVisitorRefMut,
    ) -> VisitChildrenResult {
        self.borrow().as_ref().visit_children_item(index, order, visitor)
    }

    fn input_event(
        self: ::core::pin::Pin<&Self>,
        mouse_event: sixtyfps_corelib::input::MouseEvent,
        window: &ComponentWindow,
        app_component: &ComponentRefPin,
    ) -> sixtyfps_corelib::input::InputEventResult {
        self.borrow().as_ref().input_event(mouse_event, window, app_component)
    }

    fn key_event(
        self: ::core::pin::Pin<&Self>,
        event: &sixtyfps_corelib::input::KeyEvent,
        window: &ComponentWindow,
    ) -> sixtyfps_corelib::input::KeyEventResult {
        self.borrow().as_ref().key_event(event, window)
    }

    fn focus_event(
        self: ::core::pin::Pin<&Self>,
        event: &sixtyfps_corelib::input::FocusEvent,
        window: &ComponentWindow,
    ) -> sixtyfps_corelib::input::FocusEventResult {
        self.borrow().as_ref().focus_event(event, window)
    }

    fn layout_info(self: ::core::pin::Pin<&Self>) -> sixtyfps_corelib::layout::LayoutInfo {
        self.borrow().as_ref().layout_info()
    }
    fn compute_layout(self: ::core::pin::Pin<&Self>) {
        self.borrow().as_ref().compute_layout()
    }
}

pub(crate) struct ComponentExtraData {
    mouse_grabber: core::cell::Cell<VisitChildrenResult>,
    focus_item: core::cell::Cell<VisitChildrenResult>,
    pub(crate) window: RefCell<Option<ComponentWindow>>,
}

impl Default for ComponentExtraData {
    fn default() -> Self {
        Self {
            mouse_grabber: core::cell::Cell::new(VisitChildrenResult::CONTINUE),
            focus_item: core::cell::Cell::new(VisitChildrenResult::CONTINUE),
            window: RefCell::new(None),
        }
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
    pub(crate) custom_signals: HashMap<String, FieldOffset<Instance<'id>, Signal<[eval::Value]>>>,
    repeater: Vec<ErasedRepeaterWithinComponent<'id>>,
    /// Map the Element::id of the repeater to the index in the `repeater` vec
    pub repeater_names: HashMap<String, usize>,
    /// Offset to a Option<ComponentPinRef>
    pub(crate) parent_component_offset:
        Option<FieldOffset<Instance<'id>, Option<ComponentRefPin<'id>>>>,
    /// Offset of a ComponentExtraData
    pub(crate) extra_data_offset: FieldOffset<Instance<'id>, ComponentExtraData>,
    /// Keep the Rc alive
    pub(crate) original: Rc<object_tree::Component>,
}

extern "C" fn visit_children_item(
    component: ComponentRefPin,
    index: isize,
    order: TraversalOrder,
    v: ItemVisitorRefMut,
) -> VisitChildrenResult {
    generativity::make_guard!(guard);
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };
    sixtyfps_corelib::item_tree::visit_item_tree(
        instance_ref.instance,
        component,
        instance_ref.component_type.item_tree.as_slice().into(),
        index,
        order,
        v,
        |_, order, visitor, index| {
            // `ensure_updated` needs a 'static lifetime so we must call get_untaged.
            // Safety: we do not mix the component with other component id in this function
            let rep_in_comp = unsafe { instance_ref.component_type.repeater[index].get_untaged() };
            let repeater = rep_in_comp.offset.apply_pin(instance_ref.instance);
            let init = || {
                Rc::pin(instantiate(
                    rep_in_comp.component_to_repeat.clone(),
                    Some(component),
                    #[cfg(target_arch = "wasm32")]
                    String::new(),
                ))
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
                    eval::load_property(instance_ref, &nr.element.upgrade().unwrap(), &nr.name)
                        .unwrap()
                        .try_into()
                        .unwrap()
                };
                repeater.ensure_updated_listview(
                    init,
                    assume_property_f32(get_property_ptr(&lv.viewport_width, instance_ref)),
                    assume_property_f32(get_property_ptr(&lv.viewport_height, instance_ref)),
                    assume_property_f32(get_property_ptr(&lv.viewport_y, instance_ref)),
                    get_prop(&lv.listview_width),
                    get_prop(&lv.listview_height),
                );
            } else {
                repeater.ensure_updated(init);
            }
            repeater.visit(order, visitor)
        },
    )
}

/// Information attached to a builtin item
pub(crate) struct ItemRTTI {
    vtable: &'static ItemVTable,
    type_info: dynamic_type::StaticTypeInfo,
    pub(crate) properties: HashMap<&'static str, Box<dyn eval::ErasedPropertyInfo>>,
    /// The uszie is an offset within this item to the Signal.
    /// Ideally, we would need a vtable::VFieldOffset<ItemVTable, corelib::Signal<()>>
    pub(crate) signals: HashMap<&'static str, usize>,
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
        signals: T::signals().into_iter().map(|(k, v)| (k, v.get_byte_offset())).collect(),
    };
    (T::name(), Rc::new(rtti))
}

/// Flickable is special because some of its property applies to the viewport.
/// This adds the viewport property in the flickable's property list
fn rtti_for_flickable() -> (&'static str, Rc<ItemRTTI>) {
    let (name, mut rtti) = rtti_for::<Flickable>();

    use rtti::BuiltinItem;
    let rect_prop = &["viewport_x", "viewport_y", "viewport_width", "viewport_height"];

    struct FlickableViewPortPropertyInfo(&'static dyn rtti::PropertyInfo<Rectangle, eval::Value>);
    fn viewport(flick: Pin<ItemRef>) -> Pin<&Rectangle> {
        Flickable::FIELD_OFFSETS
            .viewport
            .apply_pin(ItemRef::downcast_pin::<Flickable>(flick).unwrap())
    }

    impl eval::ErasedPropertyInfo for FlickableViewPortPropertyInfo {
        fn get(&self, item: Pin<ItemRef>) -> eval::Value {
            (*self.0).get(viewport(item)).unwrap()
        }
        fn set(
            &self,
            item: Pin<ItemRef>,
            value: eval::Value,
            animation: Option<PropertyAnimation>,
        ) {
            (*self.0).set(viewport(item), value, animation).unwrap()
        }
        fn set_binding(
            &self,
            item: Pin<ItemRef>,
            binding: Box<dyn Fn() -> eval::Value>,
            animation: Option<PropertyAnimation>,
        ) {
            (*self.0).set_binding(viewport(item), binding, animation).unwrap();
        }
        fn offset(&self) -> usize {
            (*self.0).offset() + Flickable::FIELD_OFFSETS.viewport.get_byte_offset()
        }

        unsafe fn link_two_ways(&self, item: Pin<ItemRef>, property2: *const ()) {
            (*self.0).link_two_ways(viewport(item), property2)
        }
    }

    Rc::get_mut(&mut rtti).unwrap().properties.extend(
        Rectangle::properties().into_iter().filter_map(|(k, v)| {
            Some((
                *rect_prop.iter().find(|x| x.ends_with(k))?,
                Box::new(FlickableViewPortPropertyInfo(v)) as Box<dyn eval::ErasedPropertyInfo>,
            ))
        }),
    );

    (name, rtti)
}

/// Create a ComponentDescription from a source.
/// The path corresponding to the source need to be passed as well (path is used for diagnostics
/// and loading relative assets)
pub fn load<'id>(
    source: String,
    path: &std::path::Path,
    compiler_config: &CompilerConfiguration,
    guard: generativity::Guard<'id>,
) -> (Result<Rc<ComponentDescription<'id>>, ()>, sixtyfps_compilerlib::diagnostics::BuildDiagnostics)
{
    let (syntax_node, diag) = parser::parse(source, Some(path));
    if diag.has_error() {
        let mut d = sixtyfps_compilerlib::diagnostics::BuildDiagnostics::default();
        d.add(diag);
        return (Err(()), d);
    }
    let (doc, diag) = compile_syntax_node(syntax_node, diag, compiler_config);
    if diag.has_error() {
        return (Err(()), diag);
    }
    (Ok(generate_component(&doc.root_component, guard)), diag)
}

fn generate_component<'id>(
    root_component: &Rc<object_tree::Component>,
    guard: generativity::Guard<'id>,
) -> Rc<ComponentDescription<'id>> {
    let mut rtti = HashMap::new();
    {
        use sixtyfps_corelib::items::*;
        rtti.extend(
            [
                rtti_for::<Image>(),
                rtti_for::<Text>(),
                rtti_for::<Rectangle>(),
                rtti_for::<BorderRectangle>(),
                rtti_for::<TouchArea>(),
                rtti_for::<Path>(),
                rtti_for_flickable(),
                rtti_for::<Window>(),
                rtti_for::<TextInput>(),
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

    generator::build_array_helper(root_component, |rc_item, child_offset, is_flickable_rect| {
        let item = rc_item.borrow();
        if is_flickable_rect {
            use vtable::HasStaticVTable;
            let offset =
                items_types[&item.id].offset + Flickable::FIELD_OFFSETS.viewport.get_byte_offset();
            tree_array.push(ItemTreeNode::Item {
                item: unsafe { vtable::VOffset::from_raw(Rectangle::static_vtable(), offset) },
                children_index: tree_array.len() as u32 + 1,
                chilren_count: item.children.len() as _,
            });
        } else if let Some(repeated) = &item.repeated {
            tree_array.push(ItemTreeNode::DynamicTree { index: repeater.len() });
            let base_component = item.base_type.as_component();
            repeater_names.insert(item.id.clone(), repeater.len());
            generativity::make_guard!(guard);
            repeater.push(
                RepeaterWithinComponent {
                    component_to_repeat: generate_component(base_component, guard),
                    offset: builder.add_field_type::<Repeater<ComponentBox>>(),
                    model: repeated.model.clone(),
                }
                .into(),
            );
        } else {
            let rt = rtti.get(&*item.base_type.as_native().class_name).unwrap_or_else(|| {
                panic!("Native type not registered: {}", item.base_type.as_native().class_name)
            });
            let offset = builder.add_field(rt.type_info);
            tree_array.push(ItemTreeNode::Item {
                item: unsafe { vtable::VOffset::from_raw(rt.vtable, offset) },
                children_index: child_offset,
                chilren_count: if generator::is_flickable(rc_item) {
                    1
                } else {
                    item.children.len() as _
                },
            });
            items_types.insert(
                item.id.clone(),
                ItemWithinComponent { offset, rtti: rt.clone(), elem: rc_item.clone() },
            );
        }
    });

    let mut custom_properties = HashMap::new();
    let mut custom_signals = HashMap::new();
    fn property_info<T: Clone + Default + 'static>(
    ) -> (Box<dyn PropertyInfo<u8, eval::Value>>, dynamic_type::StaticTypeInfo)
    where
        T: std::convert::TryInto<eval::Value>,
        eval::Value: std::convert::TryInto<T>,
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
    ) -> (Box<dyn PropertyInfo<u8, eval::Value>>, dynamic_type::StaticTypeInfo)
    where
        T: std::convert::TryInto<eval::Value>,
        eval::Value: std::convert::TryInto<T>,
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

    for (name, decl) in &root_component.root_element.borrow().property_declarations {
        if decl.is_alias.is_some() {
            continue;
        }
        let (prop, type_info) = match decl.property_type {
            Type::Float32 => animated_property_info::<f32>(),
            Type::Int32 => animated_property_info::<i32>(),
            Type::String => property_info::<SharedString>(),
            Type::Color => animated_property_info::<Color>(),
            Type::Duration => animated_property_info::<i64>(),
            Type::Length => animated_property_info::<f32>(),
            Type::LogicalLength => animated_property_info::<f32>(),
            Type::Resource => property_info::<Resource>(),
            Type::Bool => property_info::<bool>(),
            Type::Signal { .. } => {
                custom_signals
                    .insert(name.clone(), builder.add_field_type::<Signal<[eval::Value]>>());
                continue;
            }
            Type::Object(_) => property_info::<eval::Value>(),
            Type::Array(_) => property_info::<eval::Value>(),
            Type::Component(ref c) if c.root_element.borrow().base_type == Type::Void => {
                property_info::<eval::Value>()
            }
            _ => panic!("bad type"),
        };
        custom_properties.insert(
            name.clone(),
            PropertiesWithinComponent { offset: builder.add_field(type_info), prop },
        );
    }
    if root_component.parent_element.upgrade().is_some() {
        let (prop, type_info) = property_info::<u32>();
        custom_properties.insert(
            "index".into(),
            PropertiesWithinComponent { offset: builder.add_field(type_info), prop },
        );
        // FIXME: make it a property for the correct type instead of being generic
        let (prop, type_info) = property_info::<eval::Value>();
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

    let parent_component_offset = if root_component.parent_element.upgrade().is_some() {
        Some(builder.add_field_type::<Option<ComponentRefPin>>())
    } else {
        None
    };

    let extra_data_offset = builder.add_field_type::<ComponentExtraData>();

    extern "C" fn layout_info(_: ComponentRefPin) -> LayoutInfo {
        todo!()
    }

    let t = ComponentVTable {
        visit_children_item,
        layout_info,
        compute_layout,
        input_event,
        key_event,
        focus_event,
    };
    let t = ComponentDescription {
        ct: t,
        dynamic_type: builder.build(),
        item_tree: tree_array,
        items: items_types,
        custom_properties,
        custom_signals,
        original: root_component.clone(),
        repeater,
        repeater_names,
        parent_component_offset,
        extra_data_offset,
    };

    Rc::new(t)
}

pub fn animation_for_property(
    component: InstanceRef,
    all_animations: &HashMap<String, ElementRc>,
    property_name: &str,
) -> Option<PropertyAnimation> {
    match all_animations.get(property_name) {
        Some(anim_elem) => Some(eval::new_struct_with_bindings(
            &anim_elem.borrow().bindings,
            component,
            &mut Default::default(),
        )),
        None => None,
    }
}

fn animation_for_element_property(
    component: InstanceRef,
    element: &Element,
    property_name: &str,
) -> Option<PropertyAnimation> {
    animation_for_property(component, &element.property_animations, property_name)
}

pub fn instantiate<'id>(
    component_type: Rc<ComponentDescription<'id>>,
    parent_ctx: Option<ComponentRefPin>,
    #[cfg(target_arch = "wasm32")] canvas_id: String,
) -> ComponentBox<'id> {
    let instance = component_type.dynamic_type.clone().create_instance();
    let mem = instance.as_ptr().as_ptr() as *mut u8;
    let component_box = ComponentBox { instance, component_type: component_type.clone() };
    let instance_ref = component_box.borrow_instance();

    if let Some(parent) = parent_ctx {
        unsafe {
            *(mem.add(component_type.parent_component_offset.unwrap().get_byte_offset())
                as *mut Option<ComponentRefPin>) = Some(parent);
        }
    } else {
        let extra_data = component_type.extra_data_offset.apply(instance_ref.as_ref());
        #[cfg(not(target_arch = "wasm32"))]
        extra_data.window.replace(Some(sixtyfps_rendering_backend_default::create_window()));
        #[cfg(target_arch = "wasm32")]
        extra_data.window.replace(Some(
            sixtyfps_rendering_backend_gl::create_gl_window_with_canvas_id(canvas_id),
        ));
    }

    sixtyfps_corelib::component::init_component_items(
        instance_ref.instance,
        instance_ref.component_type.item_tree.as_slice().into(),
        &eval::window_ref(instance_ref).unwrap(),
    );

    for item_within_component in component_type.items.values() {
        unsafe {
            let item = item_within_component.item_from_component(mem);
            let elem = item_within_component.elem.borrow();
            for (prop, expr) in &elem.bindings {
                let ty = elem.lookup_property(prop.as_str());
                if let Type::Signal { .. } = ty {
                    let expr = expr.clone();
                    let component_type = component_type.clone();
                    let instance = component_box.instance.as_ptr();
                    let c = Pin::new_unchecked(vtable::VRef::from_raw(
                        NonNull::from(&component_type.ct).cast(),
                        instance.cast(),
                    ));
                    if let Some(signal_offset) =
                        item_within_component.rtti.signals.get(prop.as_str())
                    {
                        let signal = &*(item.as_ptr().add(*signal_offset) as *const Signal<()>);
                        signal.set_handler(move |_: &()| {
                            generativity::make_guard!(guard);
                            eval::eval_expression(
                                &expr,
                                InstanceRef::from_pin_ref(c, guard),
                                &mut Default::default(),
                            );
                        })
                    } else if let Some(signal_offset) =
                        component_type.custom_signals.get(prop.as_str())
                    {
                        let signal = signal_offset.apply(instance_ref.as_ref());
                        signal.set_handler(move |args| {
                            generativity::make_guard!(guard);
                            let mut local_context = eval::EvalLocalContext::from_function_arguments(
                                args.iter().cloned().collect(),
                            );
                            eval::eval_expression(
                                &expr,
                                InstanceRef::from_pin_ref(c, guard),
                                &mut local_context,
                            );
                        })
                    } else {
                        panic!("unkown signal {}", prop)
                    }
                } else {
                    if let Some(prop_rtti) =
                        item_within_component.rtti.properties.get(prop.as_str())
                    {
                        let maybe_animation =
                            animation_for_element_property(instance_ref, &elem, prop);
                        if let Expression::TwoWayBinding(nr) = &expr.expression {
                            // Safety: The compiler must have ensured that the properties exist and are of the same type
                            prop_rtti.link_two_ways(item, get_property_ptr(&nr, instance_ref));
                        } else if expr.is_constant() {
                            prop_rtti.set(
                                item,
                                eval::eval_expression(expr, instance_ref, &mut Default::default()),
                                maybe_animation,
                            );
                        } else {
                            let expr = expr.clone();
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
                                        &expr,
                                        InstanceRef::from_pin_ref(c, guard),
                                        &mut Default::default(),
                                    )
                                }),
                                maybe_animation,
                            );
                        }
                    } else if let Some(PropertiesWithinComponent {
                        offset, prop: prop_info, ..
                    }) = component_type.custom_properties.get(prop.as_str())
                    {
                        let maybe_animation = animation_for_property(
                            instance_ref,
                            &component_type.original.root_element.borrow().property_animations,
                            prop,
                        );
                        let item = Pin::new_unchecked(&*mem.add(*offset));

                        if let Expression::TwoWayBinding(nr) = &expr.expression {
                            // Safety: The compiler must have ensured that the properties exist and are of the same type
                            prop_info.link_two_ways(item, get_property_ptr(&nr, instance_ref));
                        } else if expr.is_constant() {
                            let v =
                                eval::eval_expression(expr, instance_ref, &mut Default::default());
                            prop_info.set(item, v, None).unwrap();
                        } else {
                            let expr = expr.clone();
                            let component_type = component_type.clone();
                            let instance = component_box.instance.as_ptr();
                            let c = Pin::new_unchecked(vtable::VRef::from_raw(
                                NonNull::from(&component_type.ct).cast(),
                                instance.cast(),
                            ));
                            prop_info
                                .set_binding(
                                    item,
                                    Box::new(move || {
                                        generativity::make_guard!(guard);
                                        eval::eval_expression(
                                            &expr,
                                            InstanceRef::from_pin_ref(c, guard),
                                            &mut Default::default(),
                                        )
                                    }),
                                    maybe_animation,
                                )
                                .unwrap();
                        }
                    } else {
                        panic!("unkown property {}", prop);
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
                unsafe { InstanceRef::from_pin_ref(c, guard) },
                &mut Default::default(),
            );
            Some(Rc::new(crate::value_model::ValueModel::new(m)))
        });
    }

    for extra_init_code in component_type.original.setup_code.borrow().iter() {
        eval::eval_expression(extra_init_code, instance_ref, &mut Default::default());
    }

    component_box
}

fn get_property_ptr(nr: &NamedReference, instance: InstanceRef) -> *const () {
    let element = nr.element.upgrade().unwrap();
    generativity::make_guard!(guard);
    let enclosing_component = eval::enclosing_component_for_element(&element, instance, guard);
    let element = element.borrow();
    if element.id == element.enclosing_component.upgrade().unwrap().root_element.borrow().id {
        if let Some(x) = enclosing_component.component_type.custom_properties.get(&nr.name) {
            return unsafe { enclosing_component.as_ptr().add(x.offset).cast() };
        };
    };
    let item_info = enclosing_component
        .component_type
        .items
        .get(element.id.as_str())
        .unwrap_or_else(|| panic!("Unkown element for {}.{}", element.id, nr.name));
    core::mem::drop(element);
    let item = unsafe { item_info.item_from_component(enclosing_component.as_ptr()) };
    unsafe {
        item.as_ptr().add(item_info.rtti.properties.get(nr.name.as_str()).unwrap().offset()).cast()
    }
}

use sixtyfps_corelib::layout::*;

pub struct GridLayoutWithCells<'a> {
    grid: &'a GridLayout,
    cells: Vec<GridLayoutCellData<'a>>,
    spacing: f32,
    padding: Padding,
}

#[derive(derive_more::From)]
enum LayoutTreeItem<'a> {
    GridLayout(GridLayoutWithCells<'a>),
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
            LayoutTreeItem::PathLayout(_) => todo!(),
        }
    }
}

trait LayoutItemCodeGen {
    fn get_property_ref<'a>(
        &self,
        component: InstanceRef<'a, '_>,
        name: &str,
    ) -> Option<&'a Property<f32>>;
    fn get_layout_info<'a, 'b>(
        &'a self,
        component: InstanceRef<'a, '_>,
        layout_tree: &'b mut Vec<LayoutTreeItem<'a>>,
        window: &ComponentWindow,
    ) -> LayoutInfo;
}

impl LayoutItemCodeGen for LayoutItem {
    fn get_property_ref<'a>(
        &self,
        component: InstanceRef<'a, '_>,
        name: &str,
    ) -> Option<&'a Property<f32>> {
        match self {
            LayoutItem::Element(e) => e.get_property_ref(component, name),
            LayoutItem::Layout(l) => l.get_property_ref(component, name),
        }
    }
    fn get_layout_info<'a, 'b>(
        &'a self,
        component: InstanceRef<'a, '_>,
        layout_tree: &'b mut Vec<LayoutTreeItem<'a>>,
        window: &ComponentWindow,
    ) -> LayoutInfo {
        match self {
            LayoutItem::Element(e) => e.get_layout_info(component, layout_tree, window),
            LayoutItem::Layout(l) => l.get_layout_info(component, layout_tree, window),
        }
    }
}

impl LayoutItemCodeGen for Layout {
    fn get_property_ref<'a>(
        &self,
        component: InstanceRef<'a, '_>,
        name: &str,
    ) -> Option<&'a Property<f32>> {
        let moved_property_name = match self.rect().mapped_property_name(name) {
            Some(name) => name,
            None => return None,
        };
        let prop = component.component_type.custom_properties.get(moved_property_name).unwrap();
        Some(unsafe { &*(component.as_ptr().add(prop.offset) as *const Property<f32>) })
    }
    fn get_layout_info<'a, 'b>(
        &'a self,
        component: InstanceRef<'a, '_>,
        layout_tree: &'b mut Vec<LayoutTreeItem<'a>>,
        window: &ComponentWindow,
    ) -> LayoutInfo {
        let self_as_layout_tree_item =
            collect_layouts_recursively(layout_tree, &self, component, window);
        self_as_layout_tree_item.layout_info()
    }
}

impl LayoutItemCodeGen for LayoutElement {
    fn get_property_ref<'a>(
        &self,
        component: InstanceRef<'a, '_>,
        name: &str,
    ) -> Option<&'a Property<f32>> {
        let item =
            &component.component_type.items.get(self.element.borrow().id.as_str()).unwrap_or_else(
                || panic!("Internal error: Item {} not found", self.element.borrow().id),
            );
        unsafe {
            item.rtti.properties.get(name).map(|p| {
                &*(component.as_ptr().add(item.offset).add(p.offset()) as *const Property<f32>)
            })
        }
    }
    fn get_layout_info<'a, 'b>(
        &'a self,
        component: InstanceRef<'a, '_>,
        layout_tree: &'b mut Vec<LayoutTreeItem<'a>>,
        window: &ComponentWindow,
    ) -> LayoutInfo {
        let item =
            &component.component_type.items.get(self.element.borrow().id.as_str()).unwrap_or_else(
                || panic!("Internal error: Item {} not found", self.element.borrow().id),
            );
        let element_info =
            unsafe { item.item_from_component(component.as_ptr()).as_ref().layouting_info(window) };

        match &self.layout {
            Some(layout) => {
                let layout_info = layout.get_layout_info(component, layout_tree, window);
                layout_info.merge(&element_info)
            }
            None => element_info,
        }
    }
}

fn collect_layouts_recursively<'a, 'b>(
    layout_tree: &'b mut Vec<LayoutTreeItem<'a>>,
    layout: &'a Layout,
    component: InstanceRef<'a, '_>,
    window: &ComponentWindow,
) -> &'b LayoutTreeItem<'a> {
    match layout {
        Layout::GridLayout(grid_layout) => {
            let expr_eval = |expr| {
                eval::eval_expression(expr, component, &mut Default::default()).try_into().unwrap()
            };
            let cells = grid_layout
                .elems
                .iter()
                .map(|cell| {
                    let get_prop = |name| cell.item.get_property_ref(component, name);
                    let mut layout_info = cell.item.get_layout_info(component, layout_tree, window);
                    cell.minimum_width.as_ref().map(|e| layout_info.min_width = expr_eval(e));
                    cell.maximum_width.as_ref().map(|e| layout_info.max_width = expr_eval(e));
                    cell.minimum_height.as_ref().map(|e| layout_info.min_height = expr_eval(e));
                    cell.maximum_height.as_ref().map(|e| layout_info.max_height = expr_eval(e));

                    GridLayoutCellData {
                        x: get_prop("x"),
                        y: get_prop("y"),
                        width: get_prop("width"),
                        height: get_prop("height"),
                        col: cell.col,
                        row: cell.row,
                        colspan: cell.colspan,
                        rowspan: cell.rowspan,
                        constraint: layout_info,
                    }
                })
                .collect();
            let spacing = grid_layout.spacing.as_ref().map_or(0., expr_eval);
            let padding = Padding {
                left: grid_layout.padding.left.as_ref().map_or(0., expr_eval),
                right: grid_layout.padding.right.as_ref().map_or(0., expr_eval),
                top: grid_layout.padding.top.as_ref().map_or(0., expr_eval),
                bottom: grid_layout.padding.bottom.as_ref().map_or(0., expr_eval),
            };
            layout_tree
                .push(GridLayoutWithCells { grid: grid_layout, cells, spacing, padding }.into());
        }
        Layout::PathLayout(layout) => layout_tree.push(layout.into()),
    }
    layout_tree.last().unwrap()
}

impl<'a> LayoutTreeItem<'a> {
    fn solve(&self, instance_ref: InstanceRef) {
        let resolve_prop_ref = |prop_ref: &Expression| {
            eval::eval_expression(&prop_ref, instance_ref, &mut Default::default())
                .try_into()
                .unwrap_or_default()
        };

        match self {
            Self::GridLayout(grid_layout) => {
                solve_grid_layout(&GridLayoutData {
                    width: resolve_prop_ref(&grid_layout.grid.rect.width_reference),
                    height: resolve_prop_ref(&grid_layout.grid.rect.height_reference),
                    x: resolve_prop_ref(&grid_layout.grid.rect.x_reference),
                    y: resolve_prop_ref(&grid_layout.grid.rect.y_reference),
                    spacing: grid_layout.spacing,
                    padding: &grid_layout.padding,
                    cells: Slice::from(grid_layout.cells.as_slice()),
                });
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
                        let vec = repeater.components_vec();
                        for sub_comp in vec.iter() {
                            push_layout_data(
                                &elem.borrow().base_type.as_component().root_element,
                                sub_comp.borrow_instance(),
                            )
                        }
                    }
                }

                let path_elements =
                    eval::convert_path(&path_layout.path, instance_ref, &mut Default::default());

                solve_path_layout(&PathLayoutData {
                    items: Slice::from(items.as_slice()),
                    elements: &path_elements,
                    x: resolve_prop_ref(&path_layout.rect.x_reference),
                    y: resolve_prop_ref(&path_layout.rect.y_reference),
                    width: resolve_prop_ref(&path_layout.rect.width_reference),
                    height: resolve_prop_ref(&path_layout.rect.height_reference),
                    offset: resolve_prop_ref(&path_layout.offset_reference),
                });
            }
        }
    }
}

pub fn get_repeater_by_name<'a, 'id>(
    instance_ref: InstanceRef<'a, '_>,
    name: &str,
    guard: generativity::Guard<'id>,
) -> std::pin::Pin<&'a Repeater<ComponentBox<'id>>> {
    let rep_index = instance_ref.component_type.repeater_names[name];
    let rep_in_comp = instance_ref.component_type.repeater[rep_index].unerase(guard);
    rep_in_comp.offset.apply_pin(instance_ref.instance)
}

extern "C" fn input_event(
    component: ComponentRefPin,
    mouse_event: sixtyfps_corelib::input::MouseEvent,
    window: &sixtyfps_corelib::eventloop::ComponentWindow,
    app_component: &ComponentRefPin,
) -> sixtyfps_corelib::input::InputEventResult {
    // This is fine since we can only be called with a component that with our vtable which is a ComponentDescription
    let component_type = unsafe { get_component_type(component) };
    let instance = unsafe { Pin::new_unchecked(&*component.as_ptr().cast::<Instance>()) };
    let extra_data = component_type.extra_data_offset.apply(&*instance);

    let mouse_grabber = extra_data.mouse_grabber.get();
    let (status, new_grab) = if let Some((item_index, rep_index)) = mouse_grabber.aborted_indexes()
    {
        let tree = &component_type.item_tree;
        let offset = sixtyfps_corelib::item_tree::item_offset(instance, tree, item_index);
        let mut event = mouse_event.clone();
        event.pos -= offset.to_vector();
        let res = match tree[item_index] {
            ItemTreeNode::Item { item, .. } => {
                item.apply_pin(instance).as_ref().input_event(event, window, app_component.clone())
            }
            ItemTreeNode::DynamicTree { index } => {
                generativity::make_guard!(guard);
                let rep_in_comp = component_type.repeater[index].unerase(guard);
                rep_in_comp.offset.apply_pin(instance).input_event(
                    rep_index,
                    event,
                    window,
                    app_component,
                )
            }
        };
        match res {
            sixtyfps_corelib::input::InputEventResult::GrabMouse => (res, mouse_grabber),
            _ => (res, VisitChildrenResult::CONTINUE),
        }
    } else {
        sixtyfps_corelib::input::process_ungrabbed_mouse_event(
            component,
            mouse_event,
            window,
            app_component.clone(),
        )
    };
    extra_data.mouse_grabber.set(new_grab);
    status
}

extern "C" fn key_event(
    component: ComponentRefPin,
    key_event: &sixtyfps_corelib::input::KeyEvent,
    window: &sixtyfps_corelib::eventloop::ComponentWindow,
) -> KeyEventResult {
    // This is fine since we can only be called with a component that with our vtable which is a ComponentDescription
    let component_type = unsafe { get_component_type(component) };
    let instance = unsafe { Pin::new_unchecked(&*component.as_ptr().cast::<Instance>()) };
    let extra_data = component_type.extra_data_offset.apply(&*instance);
    if let Some((item_index, rep_index)) = extra_data.focus_item.get().aborted_indexes() {
        let tree = &component_type.item_tree;
        match tree[item_index] {
            ItemTreeNode::Item { item, .. } => {
                item.apply_pin(instance).as_ref().key_event(key_event, window)
            }
            ItemTreeNode::DynamicTree { index } => {
                generativity::make_guard!(guard);
                let rep_in_comp = &component_type.repeater[index].unerase(guard);
                rep_in_comp.offset.apply_pin(instance).key_event(rep_index, key_event, window)
            }
        }
    } else {
        KeyEventResult::EventIgnored
    }
}

extern "C" fn focus_event(
    component: ComponentRefPin,
    event: &FocusEvent,
    window: &sixtyfps_corelib::eventloop::ComponentWindow,
) -> FocusEventResult {
    // This is fine since we can only be called with a component that with our vtable which is a ComponentDescription
    let component_type = unsafe { get_component_type(component) };
    let instance = unsafe { Pin::new_unchecked(&*component.as_ptr().cast::<Instance>()) };
    let extra_data = component_type.extra_data_offset.apply(&*instance);

    match event {
        FocusEvent::FocusIn(_) => {
            let (event_result, visit_result) =
                sixtyfps_corelib::input::locate_and_activate_focus_item(component, event, window);
            if event_result == FocusEventResult::FocusItemFound {
                extra_data.focus_item.set(visit_result)
            }
            event_result
        }
        FocusEvent::FocusOut | FocusEvent::WindowReceivedFocus | FocusEvent::WindowLostFocus => {
            if let Some((item_index, rep_index)) = extra_data.focus_item.get().aborted_indexes() {
                let tree = &component_type.item_tree;
                match tree[item_index] {
                    ItemTreeNode::Item { item, .. } => {
                        item.apply_pin(instance).as_ref().focus_event(&event, window)
                    }
                    ItemTreeNode::DynamicTree { index } => {
                        generativity::make_guard!(guard);
                        let rep_in_comp = &component_type.repeater[index].unerase(guard);
                        rep_in_comp
                            .offset
                            .apply_pin(instance)
                            .focus_event(rep_index, &event, window);
                    }
                };
                // Preserve the focus_item field unless we're clearing it as part of a focus out phase.
                if matches!(event, sixtyfps_corelib::input::FocusEvent::FocusOut) {
                    extra_data.focus_item.set(VisitChildrenResult::CONTINUE);
                }
                FocusEventResult::FocusItemFound // We had a focus item and "found" it and notified it
            } else {
                FocusEventResult::FocusItemNotFound
            }
        }
    }
}

extern "C" fn compute_layout(component: ComponentRefPin) {
    generativity::make_guard!(guard);
    // This is fine since we can only be called with a component that with our vtable which is a ComponentDescription
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };
    let window = eval::window_ref(instance_ref).unwrap();

    instance_ref.component_type.original.layout_constraints.borrow().iter().for_each(|layout| {
        let mut inverse_layout_tree = Vec::new();

        collect_layouts_recursively(&mut inverse_layout_tree, &layout, instance_ref, &window);

        inverse_layout_tree.iter().rev().for_each(|layout| {
            layout.solve(instance_ref);
        });
    });

    for rep_in_comp in &instance_ref.component_type.repeater {
        generativity::make_guard!(g);
        let rep_in_comp = rep_in_comp.unerase(g);
        rep_in_comp.offset.apply_pin(instance_ref.instance).compute_layout();
    }
}

/// Get the component description from a ComponentRef
///
/// Safety: the component must have been created by the interpreter
pub unsafe fn get_component_type<'a>(component: ComponentRefPin<'a>) -> &'a ComponentDescription {
    &*(Pin::into_inner_unchecked(component).get_vtable() as *const ComponentVTable
        as *const ComponentDescription)
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
}
