use crate::{dynamic_type, eval};

use core::convert::TryInto;
use core::ptr::NonNull;
use dynamic_type::{Instance, InstanceBox};
use object_tree::{Element, ElementRc};
use sixtyfps_compilerlib::layout::{GridLayout, Layout, LayoutItem, PathLayout};
use sixtyfps_compilerlib::typeregister::Type;
use sixtyfps_compilerlib::*;
use sixtyfps_corelib::component::{ComponentRefPin, ComponentVTable};
use sixtyfps_corelib::graphics::Resource;
use sixtyfps_corelib::item_tree::{
    ItemTreeNode, ItemVisitorRefMut, TraversalOrder, VisitChildrenResult,
};
use sixtyfps_corelib::items::{Flickable, ItemRef, ItemVTable, PropertyAnimation, Rectangle};
use sixtyfps_corelib::layout::LayoutInfo;
use sixtyfps_corelib::properties::{InterpolatedPropertyValue, PropertyTracker};
use sixtyfps_corelib::rtti::{self, FieldOffset, PropertyInfo};
use sixtyfps_corelib::slice::Slice;
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
    /// Offset of the `Vec<ComponentBox>`
    pub(crate) offset: FieldOffset<Instance<'par_id>, RepeaterVec<'sub_id>>,
    /// The model
    pub(crate) model: expression_tree::Expression,
    /// Offset of the PropertyTracker
    property_tracker: Option<FieldOffset<Instance<'par_id>, PropertyTracker>>,
}

type RepeaterVec<'id> = core::cell::RefCell<Vec<ComponentBox<'id>>>;

pub(crate) struct ComponentExtraData {
    mouse_grabber: core::cell::Cell<sixtyfps_corelib::item_tree::VisitChildrenResult>,
    pub(crate) window: RefCell<Option<sixtyfps_corelib::eventloop::ComponentWindow>>,
}

impl Default for ComponentExtraData {
    fn default() -> Self {
        Self {
            mouse_grabber: core::cell::Cell::new(
                sixtyfps_corelib::item_tree::VisitChildrenResult::CONTINUE,
            ),
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
    pub(crate) custom_signals: HashMap<String, FieldOffset<Instance<'id>, Signal<()>>>,
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
    let InstanceRef { instance, component_type } =
        unsafe { InstanceRef::from_pin_ref(component, guard) };
    sixtyfps_corelib::item_tree::visit_item_tree(
        instance,
        component,
        component_type.item_tree.as_slice().into(),
        index,
        order,
        v,
        |_, order, mut visitor, index| {
            generativity::make_guard!(guard);
            let rep_in_comp = component_type.repeater[index].unerase(guard);
            let mut vec = rep_in_comp.offset.apply(&*instance).borrow_mut();
            if let Some(listener_offset) = rep_in_comp.property_tracker {
                let listener = listener_offset.apply_pin(instance);
                if listener.is_dirty() {
                    listener.evaluate(|| {
                        match eval::eval_expression(
                            &rep_in_comp.model,
                            InstanceRef { instance, component_type },
                            &mut Default::default(),
                        ) {
                            crate::Value::Number(count) => populate_model(
                                &mut *vec,
                                rep_in_comp,
                                component,
                                (0..count as i32)
                                    .into_iter()
                                    .map(|v| crate::Value::Number(v as f64)),
                            ),
                            crate::Value::Array(a) => {
                                populate_model(&mut *vec, rep_in_comp, component, a.into_iter())
                            }
                            crate::Value::Bool(b) => populate_model(
                                &mut *vec,
                                rep_in_comp,
                                component,
                                (if b { Some(crate::Value::Void) } else { None }).into_iter(),
                            ),
                            _ => panic!("Unsupported model"),
                        }
                    });
                }
            }
            match order {
                TraversalOrder::FrontToBack => {
                    for (i, x) in vec.iter().enumerate() {
                        if x.borrow()
                            .as_ref()
                            .visit_children_item(-1, order, visitor.borrow_mut())
                            .has_aborted()
                        {
                            return VisitChildrenResult::abort(i, 0);
                        }
                    }
                }
                TraversalOrder::BackToFront => {
                    for (i, x) in vec.iter().enumerate().rev() {
                        if x.borrow()
                            .as_ref()
                            .visit_children_item(-1, order, visitor.borrow_mut())
                            .has_aborted()
                        {
                            return VisitChildrenResult::abort(i, 0);
                        }
                    }
                }
            }

            VisitChildrenResult::CONTINUE
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
    (
        T::name(),
        Rc::new(ItemRTTI {
            vtable: T::static_vtable(),
            type_info: dynamic_type::StaticTypeInfo::new::<T>(),
            properties: T::properties()
                .into_iter()
                .map(|(k, v)| (k, Box::new(v) as Box<dyn eval::ErasedPropertyInfo>))
                .collect(),
            signals: T::signals().into_iter().map(|(k, v)| (k, v.get_byte_offset())).collect(),
        }),
    )
}

/// Create a ComponentDescription from a source.
/// The path corresponding to the source need to be passed as well (path is used for diagnostics
/// and loading relative assets)
pub fn load<'id>(
    source: String,
    path: &std::path::Path,
    include_paths: &[std::path::PathBuf],
    guard: generativity::Guard<'id>,
) -> Result<Rc<ComponentDescription<'id>>, sixtyfps_compilerlib::diagnostics::BuildDiagnostics> {
    let (syntax_node, diag) = parser::parse(source, Some(path));
    if diag.has_error() {
        let mut d = sixtyfps_compilerlib::diagnostics::BuildDiagnostics::default();
        d.add(diag);
        return Err(d);
    }
    let compiler_config = CompilerConfiguration { include_paths, ..Default::default() };
    let (root_component, diag) = compile_syntax_node(syntax_node, diag, &compiler_config);
    if diag.has_error() {
        return Err(diag);
    }
    Ok(generate_component(&root_component, guard))
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
                rtti_for::<Flickable>(),
                rtti_for::<Window>(),
            ]
            .iter()
            .cloned(),
        );
        #[cfg(feature = "qt_style")]
        rtti.extend(
            [
                rtti_for::<qt_style::QtStyleButton>(),
                rtti_for::<qt_style::QtStyleCheckBox>(),
                rtti_for::<qt_style::QtStyleSpinBox>(),
                rtti_for::<qt_style::QtStyleSlider>(),
            ]
            .iter()
            .cloned(),
        );
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
                    offset: builder.add_field_type::<RepeaterVec>(),
                    model: repeated.model.clone(),
                    property_tracker: if repeated.model.is_constant() {
                        None
                    } else {
                        Some(builder.add_field_type::<PropertyTracker>())
                    },
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
            Type::Signal => {
                custom_signals.insert(name.clone(), builder.add_field_type::<Signal<()>>());
                continue;
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

    let t = ComponentVTable { visit_children_item, layout_info, compute_layout, input_event };
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

fn populate_model<'par_id, 'sub_id>(
    vec: &mut Vec<ComponentBox<'sub_id>>,
    rep_in_comp: &RepeaterWithinComponent<'par_id, 'sub_id>,
    component: ComponentRefPin,
    model: impl Iterator<Item = eval::Value> + ExactSizeIterator,
) {
    vec.resize_with(model.size_hint().1.unwrap(), || {
        instantiate(rep_in_comp.component_to_repeat.clone(), Some(component))
    });
    for (i, (x, val)) in vec.iter().zip(model).enumerate() {
        rep_in_comp
            .component_to_repeat
            .set_property(x.borrow(), "index", i.try_into().unwrap())
            .unwrap();
        rep_in_comp.component_to_repeat.set_property(x.borrow(), "model_data", val).unwrap();
    }
}

pub fn instantiate<'id>(
    component_type: Rc<ComponentDescription<'id>>,
    parent_ctx: Option<ComponentRefPin>,
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
        extra_data.window.replace(Some(sixtyfps_rendering_backend_gl::create_gl_window()));
    }

    for item_within_component in component_type.items.values() {
        unsafe {
            let item = item_within_component.item_from_component(mem);
            let elem = item_within_component.elem.borrow();
            for (prop, expr) in &elem.bindings {
                let ty = elem.lookup_property(prop.as_str());
                if ty == Type::Signal {
                    let signal = item_within_component
                        .rtti
                        .signals
                        .get(prop.as_str())
                        .map(|o| &*(item.as_ptr().add(*o) as *const Signal<()>))
                        .or_else(|| {
                            component_type
                                .custom_signals
                                .get(prop.as_str())
                                .map(|o| o.apply(instance_ref.as_ref()))
                        })
                        .unwrap_or_else(|| panic!("unkown signal {}", prop));
                    let expr = expr.clone();
                    let component_type = component_type.clone();
                    let instance = component_box.instance.as_ptr();
                    let c = Pin::new_unchecked(vtable::VRef::from_raw(
                        NonNull::from(&component_type.ct).cast(),
                        instance.cast(),
                    ));
                    signal.set_handler(move |_| {
                        generativity::make_guard!(guard);
                        eval::eval_expression(
                            &expr,
                            InstanceRef::from_pin_ref(c, guard),
                            &mut Default::default(),
                        );
                    })
                } else {
                    if let Some(prop_rtti) =
                        item_within_component.rtti.properties.get(prop.as_str())
                    {
                        let maybe_animation =
                            animation_for_element_property(instance_ref, &elem, prop);

                        if expr.is_constant() {
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

                        if expr.is_constant() {
                            let v =
                                eval::eval_expression(expr, instance_ref, &mut Default::default());
                            prop_info.set(Pin::new_unchecked(&*mem.add(*offset)), v, None).unwrap();
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
                                    Pin::new_unchecked(&*mem.add(*offset)),
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
        if !rep_in_comp.model.is_constant() {
            continue;
        }
        let mut vec = rep_in_comp.offset.apply(instance_ref.as_ref()).borrow_mut();
        match eval::eval_expression(&rep_in_comp.model, instance_ref, &mut Default::default()) {
            crate::Value::Number(count) => populate_model(
                &mut *vec,
                rep_in_comp,
                component_box.borrow(),
                (0..count as i32).into_iter().map(|v| crate::Value::Number(v as f64)),
            ),
            crate::Value::Array(a) => {
                populate_model(&mut *vec, rep_in_comp, component_box.borrow(), a.into_iter())
            }
            crate::Value::Bool(b) => populate_model(
                &mut *vec,
                rep_in_comp,
                component_box.borrow(),
                (if b { Some(crate::Value::Void) } else { None }).into_iter(),
            ),
            _ => panic!("Unsupported model"),
        }
    }

    component_box
}

use sixtyfps_corelib::layout::*;

pub struct GridLayoutWithCells<'a> {
    grid: &'a GridLayout,
    cells: Vec<GridLayoutCellData<'a>>,
    spacing: f32,
}

#[derive(derive_more::From)]
enum LayoutTreeItem<'a> {
    GridLayout(GridLayoutWithCells<'a>),
    PathLayout(&'a PathLayout),
}

impl<'a> LayoutTreeItem<'a> {
    fn layout_info(&self) -> LayoutInfo {
        match self {
            LayoutTreeItem::GridLayout(grid_layout) => {
                grid_layout_info(&Slice::from(grid_layout.cells.as_slice()), grid_layout.spacing)
            }
            LayoutTreeItem::PathLayout(_) => todo!(),
        }
    }
}

trait LayoutItemCodeGen {
    fn get_property_ref<'a>(
        &'a self,
        component: InstanceRef,
        name: &str,
    ) -> Option<&'a Property<f32>>;
    fn get_layout_info<'a, 'b>(
        &'a self,
        component: InstanceRef,
        layout_tree: &'b mut Vec<LayoutTreeItem<'a>>,
    ) -> LayoutInfo;
}

impl LayoutItemCodeGen for LayoutItem {
    fn get_property_ref<'a>(
        &'a self,
        component: InstanceRef,
        name: &str,
    ) -> Option<&'a Property<f32>> {
        match self {
            LayoutItem::Element(e) => e.get_property_ref(component, name),
            LayoutItem::Layout(l) => l.get_property_ref(component, name),
        }
    }
    fn get_layout_info<'a, 'b>(
        &'a self,
        component: InstanceRef,
        layout_tree: &'b mut Vec<LayoutTreeItem<'a>>,
    ) -> LayoutInfo {
        match self {
            LayoutItem::Element(e) => e.get_layout_info(component, layout_tree),
            LayoutItem::Layout(l) => l.get_layout_info(component, layout_tree),
        }
    }
}

impl LayoutItemCodeGen for Layout {
    fn get_property_ref<'a>(
        &'a self,
        component: InstanceRef,
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
        component: InstanceRef,
        layout_tree: &'b mut Vec<LayoutTreeItem<'a>>,
    ) -> LayoutInfo {
        let self_as_layout_tree_item = collect_layouts_recursively(layout_tree, &self, component);
        self_as_layout_tree_item.layout_info()
    }
}

impl LayoutItemCodeGen for ElementRc {
    fn get_property_ref<'a>(
        &'a self,
        component: InstanceRef,
        name: &str,
    ) -> Option<&'a Property<f32>> {
        let item = &component.component_type.items[self.borrow().id.as_str()];
        unsafe {
            item.rtti.properties.get(name).map(|p| {
                &*(component.as_ptr().add(item.offset).add(p.offset()) as *const Property<f32>)
            })
        }
    }
    fn get_layout_info<'a, 'b>(
        &'a self,
        component: InstanceRef,
        _layout_tree: &'b mut Vec<LayoutTreeItem<'a>>,
    ) -> LayoutInfo {
        let item = &component.component_type.items[self.borrow().id.as_str()];
        unsafe { item.item_from_component(component.as_ptr()).as_ref().layouting_info() }
    }
}

fn collect_layouts_recursively<'a, 'b>(
    layout_tree: &'b mut Vec<LayoutTreeItem<'a>>,
    layout: &'a Layout,
    component: InstanceRef,
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
                    let mut layout_info = cell.item.get_layout_info(component, layout_tree);
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
            layout_tree.push(GridLayoutWithCells { grid: grid_layout, cells, spacing }.into());
        }
        Layout::PathLayout(layout) => layout_tree.push(layout.into()),
    }
    layout_tree.last().unwrap()
}

impl<'a> LayoutTreeItem<'a> {
    fn solve(&self, instance_ref: InstanceRef) {
        let resolve_prop_ref = |prop_ref: &expression_tree::Expression| {
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
                    spacing: grid_layout.grid.spacing.as_ref().map(resolve_prop_ref).unwrap_or(0.),
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
                        let rep_index =
                            instance_ref.component_type.repeater_names[elem.borrow().id.as_str()];
                        generativity::make_guard!(guard);
                        let rep_in_comp =
                            instance_ref.component_type.repeater[rep_index].unerase(guard);
                        let vec = rep_in_comp.offset.apply(&*instance_ref.instance).borrow();
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

extern "C" fn input_event(
    component: ComponentRefPin,
    mouse_event: sixtyfps_corelib::input::MouseEvent,
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
            ItemTreeNode::Item { item, .. } => item.apply_pin(instance).as_ref().input_event(event),
            ItemTreeNode::DynamicTree { index } => {
                generativity::make_guard!(guard);
                let rep_in_comp = &component_type.repeater[index].unerase(guard);
                let vec = rep_in_comp.offset.apply(&*instance).borrow();
                vec[rep_index].borrow().as_ref().input_event(event)
            }
        };
        match res {
            sixtyfps_corelib::input::InputEventResult::GrabMouse => (res, mouse_grabber),
            _ => (res, VisitChildrenResult::CONTINUE),
        }
    } else {
        sixtyfps_corelib::input::process_ungrabbed_mouse_event(component, mouse_event)
    };
    extra_data.mouse_grabber.set(new_grab);
    status
}

extern "C" fn compute_layout(component: ComponentRefPin) {
    generativity::make_guard!(guard);
    // This is fine since we can only be called with a component that with our vtable which is a ComponentDescription
    let instance_ref = unsafe { InstanceRef::from_pin_ref(component, guard) };

    instance_ref.component_type.original.layout_constraints.borrow().iter().for_each(|layout| {
        let mut inverse_layout_tree = Vec::new();

        collect_layouts_recursively(&mut inverse_layout_tree, &layout, instance_ref);

        inverse_layout_tree.iter().rev().for_each(|layout| {
            layout.solve(instance_ref);
        });
    });
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
