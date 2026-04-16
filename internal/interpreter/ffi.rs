// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::dynamic_item_tree::ErasedItemTreeBox;

use super::*;
use core::ptr::NonNull;
use i_slint_core::model::{Model, ModelNotify, SharedVectorModel};
use i_slint_core::slice::Slice;
use i_slint_core::window::WindowAdapter;
use std::ffi::{CString, c_void};
use vtable::VRef;

/// Construct a new Value in the given memory location
#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_value_new() -> Box<Value> {
    Box::new(Value::default())
}

/// Construct a new Value in the given memory location
#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_value_clone(other: &Value) -> Box<Value> {
    Box::new(other.clone())
}

/// Destruct the value in that memory location
#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_value_destructor(val: Box<Value>) {
    drop(val);
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_value_eq(a: &Value, b: &Value) -> bool {
    a == b
}

/// Construct a new Value in the given memory location as string
#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_value_new_string(str: &SharedString) -> Box<Value> {
    Box::new(Value::String(str.clone()))
}

/// Construct a new Value in the given memory location as double
#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_value_new_double(double: f64) -> Box<Value> {
    Box::new(Value::Number(double))
}

/// Construct a new Value in the given memory location as bool
#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_value_new_bool(b: bool) -> Box<Value> {
    Box::new(Value::Bool(b))
}

/// Construct a new Value in the given memory location as array model
#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_value_new_array_model(
    a: &SharedVector<Box<Value>>,
) -> Box<Value> {
    let vec = a.iter().map(|vb| vb.as_ref().clone()).collect::<SharedVector<_>>();
    Box::new(Value::Model(ModelRc::new(SharedVectorModel::from(vec))))
}

/// Construct a new Value in the given memory location as Brush
#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_value_new_brush(brush: &Brush) -> Box<Value> {
    Box::new(Value::Brush(brush.clone()))
}

/// Construct a new Value in the given memory location as Struct
#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_value_new_struct(struc: &StructOpaque) -> Box<Value> {
    Box::new(Value::Struct(struc.as_struct().clone()))
}

/// Construct a new Value in the given memory location as image
#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_value_new_image(img: &Image) -> Box<Value> {
    Box::new(Value::Image(img.clone()))
}

/// Construct a new Value containing a model in the given memory location
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_value_new_model(
    model: NonNull<u8>,
    vtable: &ModelAdaptorVTable,
) -> Box<Value> {
    Box::new(Value::Model(ModelRc::new(ModelAdaptorWrapper(unsafe {
        vtable::VBox::from_raw(NonNull::from(vtable), model)
    }))))
}

/// If the value contains a model set from [`slint_interpreter_value_new_model]` with the same vtable pointer,
/// return the model that was set.
/// Returns a null ptr otherwise
#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_value_to_model(
    val: &Value,
    vtable: &ModelAdaptorVTable,
) -> *const u8 {
    if let Value::Model(m) = val
        && let Some(m) = m.as_any().downcast_ref::<ModelAdaptorWrapper>()
        && core::ptr::eq(m.0.get_vtable() as *const _, vtable as *const _)
    {
        return m.0.as_ptr();
    }
    core::ptr::null()
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_value_type(val: &Value) -> ValueType {
    val.value_type()
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_value_to_string(val: &Value) -> Option<&SharedString> {
    match val {
        Value::String(v) => Some(v),
        _ => None,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_value_to_number(val: &Value) -> Option<&f64> {
    match val {
        Value::Number(v) => Some(v),
        _ => None,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_value_to_bool(val: &Value) -> Option<&bool> {
    match val {
        Value::Bool(v) => Some(v),
        _ => None,
    }
}

/// Extracts a `SharedVector<ValueOpaque>` out of the given value `val`, writes that into the
/// `out` parameter and returns true; returns false if the value does not hold an extractable
/// array.
#[unsafe(no_mangle)]
#[allow(clippy::borrowed_box)]
pub extern "C" fn slint_interpreter_value_to_array(
    val: &Box<Value>,
    out: &mut SharedVector<Box<Value>>,
) -> bool {
    match val.as_ref() {
        Value::Model(m) => {
            let vec = m.iter().map(Box::new).collect::<SharedVector<_>>();
            *out = vec;
            true
        }
        _ => false,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_value_to_brush(val: &Value) -> Option<&Brush> {
    match val {
        Value::Brush(b) => Some(b),
        _ => None,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_value_to_struct(val: &Value) -> *const StructOpaque {
    match val {
        Value::Struct(s) => s as *const Struct as *const StructOpaque,
        _ => std::ptr::null(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_value_to_image(val: &Value) -> Option<&Image> {
    match val {
        Value::Image(img) => Some(img),
        _ => None,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_value_enum_to_string(
    val: &Value,
    result: &mut SharedString,
) -> bool {
    match val {
        Value::EnumerationValue(_, value) => {
            *result = SharedString::from(value);
            true
        }
        _ => false,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_value_new_enum(
    name: Slice<u8>,
    value: Slice<u8>,
) -> Box<Value> {
    Box::new(Value::EnumerationValue(
        std::str::from_utf8(&name).unwrap().to_string(),
        std::str::from_utf8(&value).unwrap().to_string(),
    ))
}

#[repr(C)]
#[cfg(target_pointer_width = "64")]
pub struct StructOpaque([usize; 6]);
#[repr(C)]
#[cfg(target_pointer_width = "32")]
pub struct StructOpaque([u64; 4]);
const _: [(); std::mem::size_of::<StructOpaque>()] = [(); std::mem::size_of::<Struct>()];
const _: [(); std::mem::align_of::<StructOpaque>()] = [(); std::mem::align_of::<Struct>()];

impl StructOpaque {
    fn as_struct(&self) -> &Struct {
        // Safety: there should be no way to construct a StructOpaque without it holding an actual Struct
        unsafe { std::mem::transmute::<&StructOpaque, &Struct>(self) }
    }
    fn as_struct_mut(&mut self) -> &mut Struct {
        // Safety: there should be no way to construct a StructOpaque without it holding an actual Struct
        unsafe { std::mem::transmute::<&mut StructOpaque, &mut Struct>(self) }
    }
}

/// Construct a new Struct in the given memory location
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_struct_new(val: *mut StructOpaque) {
    unsafe { std::ptr::write(val as *mut Struct, Struct::default()) }
}

/// Construct a new Struct in the given memory location
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_struct_clone(
    other: &StructOpaque,
    val: *mut StructOpaque,
) {
    unsafe { std::ptr::write(val as *mut Struct, other.as_struct().clone()) }
}

/// Destruct the struct in that memory location
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_struct_destructor(val: *mut StructOpaque) {
    drop(unsafe { std::ptr::read(val as *mut Struct) })
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_struct_get_field(
    stru: &StructOpaque,
    name: Slice<u8>,
) -> *mut Value {
    if let Some(value) = stru.as_struct().get_field(std::str::from_utf8(&name).unwrap()) {
        Box::into_raw(Box::new(value.clone()))
    } else {
        std::ptr::null_mut()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_struct_set_field(
    stru: &mut StructOpaque,
    name: Slice<u8>,
    value: &Value,
) {
    stru.as_struct_mut().set_field(std::str::from_utf8(&name).unwrap().into(), value.clone())
}

type StructIterator<'a> = std::collections::hash_map::Iter<'a, SmolStr, Value>;
#[repr(C)]
pub struct StructIteratorOpaque<'a>([usize; 5], std::marker::PhantomData<StructIterator<'a>>);
const _: [(); std::mem::size_of::<StructIteratorOpaque>()] =
    [(); std::mem::size_of::<StructIterator>()];
const _: [(); std::mem::align_of::<StructIteratorOpaque>()] =
    [(); std::mem::align_of::<StructIterator>()];

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_struct_iterator_destructor(
    val: *mut StructIteratorOpaque,
) {
    #[allow(clippy::drop_non_drop)] // the drop is a no-op but we still want to be explicit
    drop(unsafe { std::ptr::read(val as *mut StructIterator) })
}

/// Advance the iterator and return the next value, or a null pointer
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_struct_iterator_next<'a>(
    iter: &'a mut StructIteratorOpaque,
    k: &mut Slice<'a, u8>,
) -> *mut Value {
    if let Some((str, val)) =
        unsafe { (*(iter as *mut StructIteratorOpaque as *mut StructIterator)).next() }
    {
        *k = Slice::from_slice(str.as_bytes());
        Box::into_raw(Box::new(val.clone()))
    } else {
        *k = Slice::default();
        std::ptr::null_mut()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_struct_make_iter(
    stru: &StructOpaque,
) -> StructIteratorOpaque<'_> {
    let ret_it: StructIterator = stru.as_struct().0.iter();
    unsafe {
        let mut r = std::mem::MaybeUninit::<StructIteratorOpaque>::uninit();
        std::ptr::write(r.as_mut_ptr() as *mut StructIterator, ret_it);
        r.assume_init()
    }
}

/// Get a property. Returns a null pointer if the property does not exist.
#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_component_instance_get_property(
    inst: &ErasedItemTreeBox,
    name: Slice<u8>,
) -> *mut Value {
    generativity::make_guard!(guard);
    let comp = inst.unerase(guard);
    match comp
        .description()
        .get_property(comp.borrow(), &normalize_identifier(std::str::from_utf8(&name).unwrap()))
    {
        Ok(val) => Box::into_raw(Box::new(val)),
        Err(_) => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_component_instance_set_property(
    inst: &ErasedItemTreeBox,
    name: Slice<u8>,
    val: &Value,
) -> bool {
    generativity::make_guard!(guard);
    let comp = inst.unerase(guard);
    comp.description()
        .set_property(
            comp.borrow(),
            &normalize_identifier(std::str::from_utf8(&name).unwrap()),
            val.clone(),
        )
        .is_ok()
}

/// Invoke a callback or function. Returns raw boxed value on success and null ptr on failure.
#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_component_instance_invoke(
    inst: &ErasedItemTreeBox,
    name: Slice<u8>,
    args: Slice<Box<Value>>,
) -> *mut Value {
    let args = args.iter().map(|vb| vb.as_ref().clone()).collect::<Vec<_>>();
    generativity::make_guard!(guard);
    let comp = inst.unerase(guard);
    match comp.description().invoke(
        comp.borrow(),
        &normalize_identifier(std::str::from_utf8(&name).unwrap()),
        args.as_slice(),
    ) {
        Ok(val) => Box::into_raw(Box::new(val)),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Wrap the user_data provided by the native code and call the drop function on Drop.
///
/// Safety: user_data must be a pointer that can be destroyed by the drop_user_data function.
/// callback must be a valid callback that initialize the `ret`
pub struct CallbackUserData {
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
    callback: extern "C" fn(user_data: *mut c_void, arg: Slice<Box<Value>>) -> Box<Value>,
}

impl Drop for CallbackUserData {
    fn drop(&mut self) {
        if let Some(x) = self.drop_user_data {
            x(self.user_data)
        }
    }
}

impl CallbackUserData {
    pub unsafe fn new(
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
        callback: extern "C" fn(user_data: *mut c_void, arg: Slice<Box<Value>>) -> Box<Value>,
    ) -> Self {
        Self { user_data, drop_user_data, callback }
    }

    pub fn call(&self, args: &[Value]) -> Value {
        let args = args.iter().map(|v| v.clone().into()).collect::<Vec<_>>();
        (self.callback)(self.user_data, Slice::from_slice(args.as_ref())).as_ref().clone()
    }
}

/// Set a handler for the callback.
/// The `callback` function must initialize the `ret` (the `ret` passed to the callback is initialized and is assumed initialized after the function)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_instance_set_callback(
    inst: &ErasedItemTreeBox,
    name: Slice<u8>,
    callback: extern "C" fn(user_data: *mut c_void, arg: Slice<Box<Value>>) -> Box<Value>,
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
) -> bool {
    let ud = unsafe { CallbackUserData::new(user_data, drop_user_data, callback) };

    generativity::make_guard!(guard);
    let comp = inst.unerase(guard);
    comp.description()
        .set_callback_handler(
            comp.borrow(),
            &normalize_identifier(std::str::from_utf8(&name).unwrap()),
            Box::new(move |args| ud.call(args)),
        )
        .is_ok()
}

/// Get a global property. Returns a raw boxed value on success; nullptr otherwise.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_instance_get_global_property(
    inst: &ErasedItemTreeBox,
    global: Slice<u8>,
    property_name: Slice<u8>,
) -> *mut Value {
    generativity::make_guard!(guard);
    let comp = inst.unerase(guard);
    match comp
        .description()
        .get_global(comp.borrow(), &normalize_identifier(std::str::from_utf8(&global).unwrap()))
        .and_then(|g| {
            g.as_ref()
                .get_property(&normalize_identifier(std::str::from_utf8(&property_name).unwrap()))
        }) {
        Ok(val) => Box::into_raw(Box::new(val)),
        Err(_) => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_component_instance_set_global_property(
    inst: &ErasedItemTreeBox,
    global: Slice<u8>,
    property_name: Slice<u8>,
    val: &Value,
) -> bool {
    generativity::make_guard!(guard);
    let comp = inst.unerase(guard);
    comp.description()
        .get_global(comp.borrow(), &normalize_identifier(std::str::from_utf8(&global).unwrap()))
        .and_then(|g| {
            g.as_ref()
                .set_property(
                    &normalize_identifier(std::str::from_utf8(&property_name).unwrap()),
                    val.clone(),
                )
                .map_err(|_| ())
        })
        .is_ok()
}

/// The `callback` function must initialize the `ret` (the `ret` passed to the callback is initialized and is assumed initialized after the function)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_instance_set_global_callback(
    inst: &ErasedItemTreeBox,
    global: Slice<u8>,
    name: Slice<u8>,
    callback: extern "C" fn(user_data: *mut c_void, arg: Slice<Box<Value>>) -> Box<Value>,
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
) -> bool {
    let ud = unsafe { CallbackUserData::new(user_data, drop_user_data, callback) };

    generativity::make_guard!(guard);
    let comp = inst.unerase(guard);
    comp.description()
        .get_global(comp.borrow(), &normalize_identifier(std::str::from_utf8(&global).unwrap()))
        .and_then(|g| {
            g.as_ref().set_callback_handler(
                &normalize_identifier(std::str::from_utf8(&name).unwrap()),
                Box::new(move |args| ud.call(args)),
            )
        })
        .is_ok()
}

/// Invoke a global callback or function. Returns raw boxed value on success; nullptr otherwise.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_instance_invoke_global(
    inst: &ErasedItemTreeBox,
    global: Slice<u8>,
    callable_name: Slice<u8>,
    args: Slice<Box<Value>>,
) -> *mut Value {
    let args = args.iter().map(|vb| vb.as_ref().clone()).collect::<Vec<_>>();
    generativity::make_guard!(guard);
    let comp = inst.unerase(guard);
    let callable_name = std::str::from_utf8(&callable_name).unwrap();
    match comp
        .description()
        .get_global(comp.borrow(), &normalize_identifier(std::str::from_utf8(&global).unwrap()))
        .and_then(|g| {
            if matches!(
                comp.description()
                    .original
                    .root_element
                    .borrow()
                    .lookup_property(callable_name)
                    .property_type,
                i_slint_compiler::langtype::Type::Function { .. }
            ) {
                g.as_ref()
                    .eval_function(&normalize_identifier(callable_name), args.as_slice().to_vec())
            } else {
                g.as_ref().invoke_callback(&normalize_identifier(callable_name), args.as_slice())
            }
        }) {
        Ok(val) => Box::into_raw(Box::new(val)),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Show or hide
#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_component_instance_show(
    inst: &ErasedItemTreeBox,
    is_visible: bool,
) {
    generativity::make_guard!(guard);
    let comp = inst.unerase(guard);
    match is_visible {
        true => comp.borrow_instance().window_adapter().window().show().unwrap(),
        false => comp.borrow_instance().window_adapter().window().hide().unwrap(),
    }
}

/// Return a window for the component
///
/// The out pointer must be uninitialized and must be destroyed with
/// slint_windowrc_drop after usage
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_instance_window(
    inst: &ErasedItemTreeBox,
    out: *mut *const i_slint_core::window::ffi::WindowAdapterRcOpaque,
) {
    assert_eq!(
        core::mem::size_of::<Rc<dyn WindowAdapter>>(),
        core::mem::size_of::<i_slint_core::window::ffi::WindowAdapterRcOpaque>()
    );
    unsafe {
        core::ptr::write(
            out as *mut *const Rc<dyn WindowAdapter>,
            inst.window_adapter_ref().unwrap() as *const _,
        )
    }
}

/// Instantiate an instance from a definition.
///
/// The `out` must be uninitialized and is going to be initialized after the call
/// and need to be destroyed with slint_interpreter_component_instance_destructor
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_instance_create(
    def: &ComponentDefinitionOpaque,
    out: *mut ComponentInstance,
) {
    unsafe { std::ptr::write(out, def.as_component_definition().create().unwrap()) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_instance_component_definition(
    inst: &ErasedItemTreeBox,
    component_definition_ptr: *mut ComponentDefinitionOpaque,
) {
    generativity::make_guard!(guard);
    let definition = ComponentDefinition { inner: inst.unerase(guard).description().into() };
    unsafe { std::ptr::write(component_definition_ptr as *mut ComponentDefinition, definition) };
}

#[vtable::vtable]
#[repr(C)]
pub struct ModelAdaptorVTable {
    pub row_count: extern "C" fn(VRef<ModelAdaptorVTable>) -> usize,
    pub row_data: unsafe extern "C" fn(VRef<ModelAdaptorVTable>, row: usize) -> *mut Value,
    pub set_row_data: extern "C" fn(VRef<ModelAdaptorVTable>, row: usize, value: Box<Value>),
    pub get_notify: extern "C" fn(VRef<'_, ModelAdaptorVTable>) -> &ModelNotifyOpaque,
    pub drop: extern "C" fn(VRefMut<ModelAdaptorVTable>),
}

struct ModelAdaptorWrapper(vtable::VBox<ModelAdaptorVTable>);
impl Model for ModelAdaptorWrapper {
    type Data = Value;

    fn row_count(&self) -> usize {
        self.0.row_count()
    }

    fn row_data(&self, row: usize) -> Option<Value> {
        let val_ptr = unsafe { self.0.row_data(row) };
        if val_ptr.is_null() { None } else { Some(*unsafe { Box::from_raw(val_ptr) }) }
    }

    fn model_tracker(&self) -> &dyn i_slint_core::model::ModelTracker {
        self.0.get_notify().as_model_notify()
    }

    fn set_row_data(&self, row: usize, data: Value) {
        let val = Box::new(data);
        self.0.set_row_data(row, val);
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

#[repr(C)]
#[cfg(target_pointer_width = "64")]
pub struct ModelNotifyOpaque([usize; 8]);
#[repr(C)]
#[cfg(target_pointer_width = "32")]
pub struct ModelNotifyOpaque([usize; 12]);
/// Asserts that ModelNotifyOpaque is at least as large as ModelNotify, otherwise this would overflow
const _: usize = std::mem::size_of::<ModelNotifyOpaque>() - std::mem::size_of::<ModelNotify>();
const _: usize = std::mem::align_of::<ModelNotifyOpaque>() - std::mem::align_of::<ModelNotify>();

impl ModelNotifyOpaque {
    fn as_model_notify(&self) -> &ModelNotify {
        // Safety: there should be no way to construct a ModelNotifyOpaque without it holding an actual ModelNotify
        unsafe { std::mem::transmute::<&ModelNotifyOpaque, &ModelNotify>(self) }
    }
}

/// Construct a new ModelNotifyNotify in the given memory region
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_model_notify_new(val: *mut ModelNotifyOpaque) {
    unsafe { std::ptr::write(val as *mut ModelNotify, ModelNotify::default()) };
}

/// Destruct the value in that memory location
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_model_notify_destructor(val: *mut ModelNotifyOpaque) {
    drop(unsafe { std::ptr::read(val as *mut ModelNotify) })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_model_notify_row_changed(
    notify: &ModelNotifyOpaque,
    row: usize,
) {
    notify.as_model_notify().row_changed(row);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_model_notify_row_added(
    notify: &ModelNotifyOpaque,
    row: usize,
    count: usize,
) {
    notify.as_model_notify().row_added(row, count);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_model_notify_reset(notify: &ModelNotifyOpaque) {
    notify.as_model_notify().reset();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_model_notify_row_removed(
    notify: &ModelNotifyOpaque,
    row: usize,
    count: usize,
) {
    notify.as_model_notify().row_removed(row, count);
}

// FIXME: Figure out how to re-export the one from compilerlib
/// DiagnosticLevel describes the severity of a diagnostic.
#[derive(Clone)]
#[repr(u8)]
pub enum DiagnosticLevel {
    /// The diagnostic belongs to an error.
    Error,
    /// The diagnostic belongs to a warning.
    Warning,
    /// The diagnostic is a note
    Note,
}

/// Diagnostic describes the aspects of either a warning or an error, along
/// with its location and a description. Diagnostics are typically returned by
/// slint::interpreter::ComponentCompiler::diagnostics() in a vector.
#[derive(Clone)]
#[repr(C)]
pub struct Diagnostic {
    /// The message describing the warning or error.
    message: SharedString,
    /// The path to the source file where the warning or error is located.
    source_file: SharedString,
    /// The line within the source file. Line numbers start at 1.
    line: usize,
    /// The column within the source file. Column numbers start at 1.
    column: usize,
    /// The level of the diagnostic, such as a warning or an error.
    level: DiagnosticLevel,
}

#[repr(transparent)]
pub struct ComponentCompilerOpaque(#[allow(deprecated)] NonNull<ComponentCompiler>);

#[allow(deprecated)]
impl ComponentCompilerOpaque {
    fn as_component_compiler(&self) -> &ComponentCompiler {
        // Safety: there should be no way to construct a ComponentCompilerOpaque without it holding an actual ComponentCompiler
        unsafe { self.0.as_ref() }
    }
    fn as_component_compiler_mut(&mut self) -> &mut ComponentCompiler {
        // Safety: there should be no way to construct a ComponentCompilerOpaque without it holding an actual ComponentCompiler
        unsafe { self.0.as_mut() }
    }
}

#[unsafe(no_mangle)]
#[allow(deprecated)]
pub unsafe extern "C" fn slint_interpreter_component_compiler_new(
    compiler: *mut ComponentCompilerOpaque,
) {
    unsafe {
        *compiler = ComponentCompilerOpaque(NonNull::new_unchecked(Box::into_raw(Box::new(
            ComponentCompiler::default(),
        ))));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_compiler_destructor(
    compiler: *mut ComponentCompilerOpaque,
) {
    drop(unsafe { Box::from_raw((*compiler).0.as_ptr()) })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_compiler_set_include_paths(
    compiler: &mut ComponentCompilerOpaque,
    paths: &SharedVector<SharedString>,
) {
    compiler
        .as_component_compiler_mut()
        .set_include_paths(paths.iter().map(|path| path.as_str().into()).collect())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_compiler_set_style(
    compiler: &mut ComponentCompilerOpaque,
    style: Slice<u8>,
) {
    compiler.as_component_compiler_mut().set_style(std::str::from_utf8(&style).unwrap().to_string())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_compiler_set_translation_domain(
    compiler: &mut ComponentCompilerOpaque,
    translation_domain: Slice<u8>,
) {
    compiler
        .as_component_compiler_mut()
        .set_translation_domain(std::str::from_utf8(&translation_domain).unwrap().to_string())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_compiler_get_style(
    compiler: &ComponentCompilerOpaque,
    style_out: &mut SharedString,
) {
    *style_out =
        compiler.as_component_compiler().style().map_or(SharedString::default(), |s| s.into());
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_compiler_get_include_paths(
    compiler: &ComponentCompilerOpaque,
    paths: &mut SharedVector<SharedString>,
) {
    paths.extend(
        compiler
            .as_component_compiler()
            .include_paths()
            .iter()
            .map(|path| path.to_str().map_or_else(Default::default, |str| str.into())),
    );
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_compiler_get_diagnostics(
    compiler: &ComponentCompilerOpaque,
    out_diags: &mut SharedVector<Diagnostic>,
) {
    #[allow(deprecated)]
    out_diags.extend(compiler.as_component_compiler().diagnostics.iter().map(|diagnostic| {
        let (line, column) = diagnostic.line_column();
        Diagnostic {
            message: diagnostic.message().into(),
            source_file: diagnostic
                .source_file()
                .and_then(|path| path.to_str())
                .map_or_else(Default::default, |str| str.into()),
            line,
            column,
            level: match diagnostic.level() {
                i_slint_compiler::diagnostics::DiagnosticLevel::Error => DiagnosticLevel::Error,
                i_slint_compiler::diagnostics::DiagnosticLevel::Warning => DiagnosticLevel::Warning,
                i_slint_compiler::diagnostics::DiagnosticLevel::Note => DiagnosticLevel::Note,
                _ => DiagnosticLevel::Warning,
            },
        }
    }));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_compiler_build_from_source(
    compiler: &mut ComponentCompilerOpaque,
    source_code: Slice<u8>,
    path: Slice<u8>,
    component_definition_ptr: *mut ComponentDefinitionOpaque,
) -> bool {
    match spin_on::spin_on(compiler.as_component_compiler_mut().build_from_source(
        std::str::from_utf8(&source_code).unwrap().to_string(),
        std::str::from_utf8(&path).unwrap().to_string().into(),
    )) {
        Some(definition) => {
            unsafe {
                std::ptr::write(component_definition_ptr as *mut ComponentDefinition, definition)
            };
            true
        }
        None => false,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_compiler_build_from_path(
    compiler: &mut ComponentCompilerOpaque,
    path: Slice<u8>,
    component_definition_ptr: *mut ComponentDefinitionOpaque,
) -> bool {
    use std::str::FromStr;
    match spin_on::spin_on(
        compiler
            .as_component_compiler_mut()
            .build_from_path(PathBuf::from_str(std::str::from_utf8(&path).unwrap()).unwrap()),
    ) {
        Some(definition) => {
            unsafe {
                std::ptr::write(component_definition_ptr as *mut ComponentDefinition, definition)
            };
            true
        }
        None => false,
    }
}

/// PropertyDescriptor is a simple structure that's used to describe a property declared in .slint
/// code. It is returned from in a vector from
/// slint::interpreter::ComponentDefinition::properties().
#[derive(Clone)]
#[repr(C)]
pub struct PropertyDescriptor {
    /// The name of the declared property.
    property_name: SharedString,
    /// The type of the property.
    property_type: ValueType,
}

#[repr(C)]
// Note: This needs to stay the size of 1 pointer to allow for the null pointer definition
// in the C++ wrapper to allow for the null state.
pub struct ComponentDefinitionOpaque([usize; 1]);
/// Asserts that ComponentCompilerOpaque is as large as ComponentCompiler and has the same alignment, to make transmute safe.
const _: [(); std::mem::size_of::<ComponentDefinitionOpaque>()] =
    [(); std::mem::size_of::<ComponentDefinition>()];
const _: [(); std::mem::align_of::<ComponentDefinitionOpaque>()] =
    [(); std::mem::align_of::<ComponentDefinition>()];

impl ComponentDefinitionOpaque {
    fn as_component_definition(&self) -> &ComponentDefinition {
        // Safety: there should be no way to construct a ComponentDefinitionOpaque without it holding an actual ComponentDefinition
        unsafe { std::mem::transmute::<&ComponentDefinitionOpaque, &ComponentDefinition>(self) }
    }
}

/// Construct a new Value in the given memory location
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_definition_clone(
    other: &ComponentDefinitionOpaque,
    def: *mut ComponentDefinitionOpaque,
) {
    unsafe {
        std::ptr::write(def as *mut ComponentDefinition, other.as_component_definition().clone())
    }
}

/// Destruct the component definition in that memory location
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_definition_destructor(
    val: *mut ComponentDefinitionOpaque,
) {
    drop(unsafe { std::ptr::read(val as *mut ComponentDefinition) })
}

/// Returns the list of properties of the component the component definition describes
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_definition_properties(
    def: &ComponentDefinitionOpaque,
    props: &mut SharedVector<PropertyDescriptor>,
) {
    props.extend(def.as_component_definition().properties().map(
        |(property_name, property_type)| PropertyDescriptor {
            property_name: property_name.into(),
            property_type,
        },
    ))
}

/// Returns the list of callback names of the component the component definition describes
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_definition_callbacks(
    def: &ComponentDefinitionOpaque,
    callbacks: &mut SharedVector<SharedString>,
) {
    callbacks.extend(def.as_component_definition().callbacks().map(|name| name.into()))
}

/// Returns the list of function names of the component the component definition describes
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_definition_functions(
    def: &ComponentDefinitionOpaque,
    functions: &mut SharedVector<SharedString>,
) {
    functions.extend(def.as_component_definition().functions().map(|name| name.into()))
}

/// Return the name of the component definition
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_definition_name(
    def: &ComponentDefinitionOpaque,
    name: &mut SharedString,
) {
    *name = def.as_component_definition().name().into()
}

/// Returns a vector of strings with the names of all exported global singletons.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_definition_globals(
    def: &ComponentDefinitionOpaque,
    names: &mut SharedVector<SharedString>,
) {
    names.extend(def.as_component_definition().globals().map(|name| name.into()))
}

/// Returns a vector of the property descriptors of the properties of the specified publicly exported global
/// singleton. Returns true if a global exists under the specified name; false otherwise.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_definition_global_properties(
    def: &ComponentDefinitionOpaque,
    global_name: Slice<u8>,
    properties: &mut SharedVector<PropertyDescriptor>,
) -> bool {
    if let Some(property_it) =
        def.as_component_definition().global_properties(std::str::from_utf8(&global_name).unwrap())
    {
        properties.extend(property_it.map(|(property_name, property_type)| PropertyDescriptor {
            property_name: property_name.into(),
            property_type,
        }));
        true
    } else {
        false
    }
}

/// Returns a vector of the names of the callbacks of the specified publicly exported global
/// singleton. Returns true if a global exists under the specified name; false otherwise.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_definition_global_callbacks(
    def: &ComponentDefinitionOpaque,
    global_name: Slice<u8>,
    names: &mut SharedVector<SharedString>,
) -> bool {
    if let Some(name_it) =
        def.as_component_definition().global_callbacks(std::str::from_utf8(&global_name).unwrap())
    {
        names.extend(name_it.map(|name| name.into()));
        true
    } else {
        false
    }
}

/// Returns a vector of the names of the functions of the specified publicly exported global
/// singleton. Returns true if a global exists under the specified name; false otherwise.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_interpreter_component_definition_global_functions(
    def: &ComponentDefinitionOpaque,
    global_name: Slice<u8>,
    names: &mut SharedVector<SharedString>,
) -> bool {
    if let Some(name_it) =
        def.as_component_definition().global_functions(std::str::from_utf8(&global_name).unwrap())
    {
        names.extend(name_it.map(|name| name.into()));
        true
    } else {
        false
    }
}

fn slint_go_strdup(text: impl AsRef<str>) -> *mut core::ffi::c_char {
    CString::new(text.as_ref()).unwrap().into_raw()
}

/// Frees a C string previously returned by the Slint Go FFI.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_string_free(value: *mut core::ffi::c_char) {
    if !value.is_null() {
        drop(unsafe { CString::from_raw(value) });
    }
}

/// Compiles Slint source code into a compilation result handle for Go bindings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_compile_source(
    source: Slice<u8>,
    path: Slice<u8>,
) -> *mut CompilationResult {
    unsafe { slint_go_compile_source_with_include_paths(source, path, Slice::default()) }
}

/// Compiles Slint source code into a compilation result handle for Go bindings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_compile_source_with_include_paths(
    source: Slice<u8>,
    path: Slice<u8>,
    include_paths: Slice<u8>,
) -> *mut CompilationResult {
    let mut compiler = Compiler::default();
    let include_paths = std::str::from_utf8(&include_paths).unwrap();
    if !include_paths.is_empty() {
        compiler
            .set_include_paths(include_paths.split('\n').map(std::path::PathBuf::from).collect());
    }
    let result = spin_on::spin_on(compiler.build_from_source(
        std::str::from_utf8(&source).unwrap().to_owned(),
        std::path::PathBuf::from(std::str::from_utf8(&path).unwrap()),
    ));
    Box::into_raw(Box::new(result))
}

/// Compiles a Slint file from disk into a compilation result handle for Go bindings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_compile_path(path: Slice<u8>) -> *mut CompilationResult {
    let result = spin_on::spin_on(
        Compiler::default()
            .build_from_path(std::path::PathBuf::from(std::str::from_utf8(&path).unwrap())),
    );
    Box::into_raw(Box::new(result))
}

/// Destroys a compilation result returned by the Slint Go FFI.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_compilation_result_destructor(result: *mut CompilationResult) {
    if !result.is_null() {
        drop(unsafe { Box::from_raw(result) });
    }
}

/// Returns true if the compilation result contains at least one error diagnostic.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_compilation_result_has_errors(
    result: *const CompilationResult,
) -> bool {
    unsafe { &*result }.has_errors()
}

/// Returns diagnostics as a human-readable string. The caller owns the returned buffer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_compilation_result_diagnostics(
    result: *const CompilationResult,
) -> *mut core::ffi::c_char {
    let diagnostics = unsafe { &*result }
        .diagnostics()
        .map(|diagnostic| {
            let (line, column) = diagnostic.line_column();
            match diagnostic.source_file() {
                Some(path) => std::format!(
                    "{}:{}:{}: {:?}: {}",
                    path.display(),
                    line,
                    column,
                    diagnostic.level(),
                    diagnostic.message()
                ),
                None => std::format!("{:?}: {}", diagnostic.level(), diagnostic.message()),
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    slint_go_strdup(diagnostics)
}

/// Retrieves a compiled component definition by name from a compilation result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_compilation_result_component(
    result: *const CompilationResult,
    name: Slice<u8>,
) -> *mut ComponentDefinition {
    unsafe { &*result }
        .component(std::str::from_utf8(&name).unwrap())
        .map(|definition| Box::into_raw(Box::new(definition)))
        .unwrap_or(core::ptr::null_mut())
}

/// Destroys a component definition returned by the Slint Go FFI.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_component_definition_destructor(
    definition: *mut ComponentDefinition,
) {
    if !definition.is_null() {
        drop(unsafe { Box::from_raw(definition) });
    }
}

/// Creates a component instance from a component definition. Returns null on failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_component_definition_create(
    definition: *const ComponentDefinition,
    error_message: *mut *mut core::ffi::c_char,
) -> *mut ComponentInstance {
    match unsafe { &*definition }.create() {
        Ok(instance) => Box::into_raw(Box::new(instance)),
        Err(err) => {
            if !error_message.is_null() {
                unsafe { *error_message = slint_go_strdup(err.to_string()) };
            }
            core::ptr::null_mut()
        }
    }
}

/// Destroys a component instance returned by the Slint Go FFI.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_component_instance_destructor(instance: *mut ComponentInstance) {
    if !instance.is_null() {
        drop(unsafe { Box::from_raw(instance) });
    }
}

/// Shows a component instance. Returns false on platform failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_component_instance_show(
    instance: *const ComponentInstance,
) -> bool {
    unsafe { &*instance }.show().is_ok()
}

/// Hides a component instance. Returns false on platform failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_component_instance_hide(
    instance: *const ComponentInstance,
) -> bool {
    unsafe { &*instance }.hide().is_ok()
}

/// Runs a component instance. Returns false on platform failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_component_instance_run(
    instance: *const ComponentInstance,
) -> bool {
    unsafe { &*instance }.run().is_ok()
}

/// Gets a public property from a component instance. Returns null if the property does not exist.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_component_instance_get_property(
    instance: *const ComponentInstance,
    name: Slice<u8>,
) -> *mut Value {
    unsafe { &*instance }
        .get_property(std::str::from_utf8(&name).unwrap())
        .ok()
        .map(|value| Box::into_raw(Box::new(value)))
        .unwrap_or(core::ptr::null_mut())
}

/// Sets a public property on a component instance. Returns false on failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_component_instance_set_property(
    instance: *const ComponentInstance,
    name: Slice<u8>,
    value: *const Value,
) -> bool {
    unsafe { &*instance }
        .set_property(std::str::from_utf8(&name).unwrap(), unsafe { (&*value).clone() })
        .is_ok()
}

/// Invokes a public callback or function. Returns null if the callable does not exist.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_component_instance_invoke(
    instance: *const ComponentInstance,
    name: Slice<u8>,
    args: Slice<*mut Value>,
) -> *mut Value {
    let args = args.iter().map(|value| unsafe { (&**value).clone() }).collect::<Vec<_>>();
    unsafe { &*instance }
        .invoke(std::str::from_utf8(&name).unwrap(), &args)
        .ok()
        .map(|value| Box::into_raw(Box::new(value)))
        .unwrap_or(core::ptr::null_mut())
}

/// Gets a public property from an exported global singleton. Returns null if it does not exist.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_component_instance_get_global_property(
    instance: *const ComponentInstance,
    global: Slice<u8>,
    property: Slice<u8>,
) -> *mut Value {
    unsafe { &*instance }
        .get_global_property(
            std::str::from_utf8(&global).unwrap(),
            std::str::from_utf8(&property).unwrap(),
        )
        .ok()
        .map(|value| Box::into_raw(Box::new(value)))
        .unwrap_or(core::ptr::null_mut())
}

/// Sets a public property on an exported global singleton. Returns false on failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_component_instance_set_global_property(
    instance: *const ComponentInstance,
    global: Slice<u8>,
    property: Slice<u8>,
    value: *const Value,
) -> bool {
    unsafe { &*instance }
        .set_global_property(
            std::str::from_utf8(&global).unwrap(),
            std::str::from_utf8(&property).unwrap(),
            unsafe { (&*value).clone() },
        )
        .is_ok()
}

/// Invokes a public callback or function on an exported global singleton.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_component_instance_invoke_global(
    instance: *const ComponentInstance,
    global: Slice<u8>,
    callable: Slice<u8>,
    args: Slice<*mut Value>,
) -> *mut Value {
    let args = args.iter().map(|value| unsafe { (&**value).clone() }).collect::<Vec<_>>();
    unsafe { &*instance }
        .invoke_global(
            std::str::from_utf8(&global).unwrap(),
            std::str::from_utf8(&callable).unwrap(),
            &args,
        )
        .ok()
        .map(|value| Box::into_raw(Box::new(value)))
        .unwrap_or(core::ptr::null_mut())
}

type SlintGoCallback = extern "C" fn(
    user_data: *mut core::ffi::c_void,
    args: *const *mut Value,
    arg_len: usize,
) -> *mut Value;

struct SlintGoCallbackHolder {
    user_data: usize,
    callback: SlintGoCallback,
}

impl SlintGoCallbackHolder {
    fn invoke(&self, args: &[Value]) -> Value {
        let mut raw_args =
            args.iter().cloned().map(|value| Box::into_raw(Box::new(value))).collect::<Vec<_>>();
        let result = (self.callback)(
            self.user_data as *mut core::ffi::c_void,
            raw_args.as_ptr(),
            raw_args.len(),
        );
        raw_args.drain(..).for_each(|value| drop(unsafe { Box::from_raw(value) }));
        if result.is_null() { Value::Void } else { *unsafe { Box::from_raw(result) } }
    }
}

/// Installs a callback handler on a component instance.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_component_instance_set_callback(
    instance: *const ComponentInstance,
    name: Slice<u8>,
    user_data: usize,
    callback: SlintGoCallback,
) -> bool {
    let holder = SlintGoCallbackHolder { user_data, callback };
    unsafe { &*instance }
        .set_callback(std::str::from_utf8(&name).unwrap(), move |args| holder.invoke(args))
        .is_ok()
}

/// Installs a callback handler on an exported global singleton.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_component_instance_set_global_callback(
    instance: *const ComponentInstance,
    global: Slice<u8>,
    name: Slice<u8>,
    user_data: usize,
    callback: SlintGoCallback,
) -> bool {
    let holder = SlintGoCallbackHolder { user_data, callback };
    unsafe { &*instance }
        .set_global_callback(
            std::str::from_utf8(&global).unwrap(),
            std::str::from_utf8(&name).unwrap(),
            move |args| holder.invoke(args),
        )
        .is_ok()
}

/// Creates a new void value handle.
#[unsafe(no_mangle)]
pub extern "C" fn slint_go_value_new() -> *mut Value {
    Box::into_raw(Box::new(Value::Void))
}

/// Clones a value handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_value_clone(value: *const Value) -> *mut Value {
    Box::into_raw(Box::new(unsafe { (&*value).clone() }))
}

/// Destroys a value handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_value_destructor(value: *mut Value) {
    if !value.is_null() {
        drop(unsafe { Box::from_raw(value) });
    }
}

/// Creates a number value handle.
#[unsafe(no_mangle)]
pub extern "C" fn slint_go_value_new_number(value: f64) -> *mut Value {
    Box::into_raw(Box::new(Value::Number(value)))
}

/// Creates a string value handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_value_new_string(value: Slice<u8>) -> *mut Value {
    Box::into_raw(Box::new(Value::String(std::str::from_utf8(&value).unwrap().into())))
}

/// Creates a bool value handle.
#[unsafe(no_mangle)]
pub extern "C" fn slint_go_value_new_bool(value: bool) -> *mut Value {
    Box::into_raw(Box::new(Value::Bool(value)))
}

/// Advances the mock animation time used by the testing backend.
#[unsafe(no_mangle)]
pub extern "C" fn slint_testing_mock_elapsed_time(time_in_ms: u64) {
    i_slint_core::tests::slint_mock_elapsed_time(time_in_ms);
}

/// Creates an enumeration value handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_value_new_enumeration_value(
    enum_name: Slice<u8>,
    value: Slice<u8>,
) -> *mut Value {
    Box::into_raw(Box::new(Value::EnumerationValue(
        std::str::from_utf8(&enum_name).unwrap().into(),
        std::str::from_utf8(&value).unwrap().into(),
    )))
}

/// Returns the public type classification for a value handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_value_type(value: *const Value) -> ValueType {
    unsafe { &*value }.value_type()
}

/// Converts a string or enum value to a newly allocated C string. Returns null if not applicable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_value_to_string(value: *const Value) -> *mut core::ffi::c_char {
    match unsafe { &*value } {
        Value::String(string) => slint_go_strdup(string.as_str()),
        Value::EnumerationValue(_, value) => slint_go_strdup(value),
        _ => core::ptr::null_mut(),
    }
}

/// Extracts a number from a value. Returns false if the value is not numeric.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_value_to_number(value: *const Value, out: *mut f64) -> bool {
    if let Value::Number(number) = unsafe { &*value } {
        unsafe { *out = *number };
        true
    } else {
        false
    }
}

/// Extracts a bool from a value. Returns false if the value is not bool.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_go_value_to_bool(value: *const Value, out: *mut bool) -> bool {
    if let Value::Bool(boolean) = unsafe { &*value } {
        unsafe { *out = *boolean };
        true
    } else {
        false
    }
}
