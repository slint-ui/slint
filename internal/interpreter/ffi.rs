// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use crate::dynamic_component::ErasedComponentBox;

use super::*;
use core::ptr::NonNull;
use i_slint_core::model::{Model, ModelNotify, SharedVectorModel};
use i_slint_core::slice::Slice;
use i_slint_core::window::WindowAdapter;
use std::ffi::c_void;
use vtable::VRef;

#[repr(C)]
#[cfg(target_pointer_width = "64")]
pub struct ValueOpaque([usize; 7]);
#[repr(C)]
#[cfg(target_pointer_width = "32")]
#[repr(align(8))]
pub struct ValueOpaque([usize; 9]);
/// Asserts that ValueOpaque is as large as Value and has the same alignment, to make transmute safe.
const _: [(); std::mem::size_of::<ValueOpaque>()] = [(); std::mem::size_of::<Value>()];
const _: [(); std::mem::align_of::<ValueOpaque>()] = [(); std::mem::align_of::<Value>()];

impl ValueOpaque {
    fn as_value(&self) -> &Value {
        // Safety: there should be no way to construct a ValueOpaque without it holding an actual Value
        unsafe { std::mem::transmute::<&ValueOpaque, &Value>(self) }
    }
}

/// Construct a new Value in the given memory location
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_value_new(val: *mut ValueOpaque) {
    std::ptr::write(val as *mut Value, Value::default())
}

/// Construct a new Value in the given memory location
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_value_clone(other: &ValueOpaque, val: *mut ValueOpaque) {
    std::ptr::write(val as *mut Value, other.as_value().clone())
}

/// Destruct the value in that memory location
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_value_destructor(val: *mut ValueOpaque) {
    drop(std::ptr::read(val as *mut Value))
}

#[no_mangle]
pub extern "C" fn slint_interpreter_value_eq(a: &ValueOpaque, b: &ValueOpaque) -> bool {
    a.as_value() == b.as_value()
}

/// Construct a new Value in the given memory location as string
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_value_new_string(
    str: &SharedString,
    val: *mut ValueOpaque,
) {
    std::ptr::write(val as *mut Value, Value::String(str.clone()))
}

/// Construct a new Value in the given memory location as double
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_value_new_double(double: f64, val: *mut ValueOpaque) {
    std::ptr::write(val as *mut Value, Value::Number(double))
}

/// Construct a new Value in the given memory location as bool
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_value_new_bool(b: bool, val: *mut ValueOpaque) {
    std::ptr::write(val as *mut Value, Value::Bool(b))
}

/// Construct a new Value in the given memory location as array model
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_value_new_array_model(
    a: &SharedVector<ValueOpaque>,
    val: *mut ValueOpaque,
) {
    // Safety: We assert that Value and ValueOpaque have the same size and alignment
    let vec = std::mem::transmute::<SharedVector<ValueOpaque>, SharedVector<Value>>(a.clone());
    std::ptr::write(
        val as *mut Value,
        Value::Model(ModelRc::new(SharedVectorModel::<Value>::from(vec))),
    )
}

/// Construct a new Value in the given memory location as Brush
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_value_new_brush(brush: &Brush, val: *mut ValueOpaque) {
    std::ptr::write(val as *mut Value, Value::Brush(brush.clone()))
}

/// Construct a new Value in the given memory location as Struct
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_value_new_struct(
    struc: &StructOpaque,
    val: *mut ValueOpaque,
) {
    std::ptr::write(val as *mut Value, Value::Struct(struc.as_struct().clone()))
}

/// Construct a new Value in the given memory location as image
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_value_new_image(img: &Image, val: *mut ValueOpaque) {
    std::ptr::write(val as *mut Value, Value::Image(img.clone()))
}

/// Construct a new Value containing a model in the given memory location
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_value_new_model(
    model: NonNull<u8>,
    vtable: &ModelAdaptorVTable,
    val: *mut ValueOpaque,
) {
    std::ptr::write(
        val as *mut Value,
        Value::Model(ModelRc::new(ModelAdaptorWrapper(vtable::VBox::from_raw(
            NonNull::from(vtable),
            model,
        )))),
    )
}

#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_value_type(val: *const ValueOpaque) -> ValueType {
    (&*val).as_value().value_type()
}

#[no_mangle]
pub extern "C" fn slint_interpreter_value_to_string(val: &ValueOpaque) -> Option<&SharedString> {
    match val.as_value() {
        Value::String(v) => Some(v),
        _ => None,
    }
}

#[no_mangle]
pub extern "C" fn slint_interpreter_value_to_number(val: &ValueOpaque) -> Option<&f64> {
    match val.as_value() {
        Value::Number(v) => Some(v),
        _ => None,
    }
}

#[no_mangle]
pub extern "C" fn slint_interpreter_value_to_bool(val: &ValueOpaque) -> Option<&bool> {
    match val.as_value() {
        Value::Bool(v) => Some(v),
        _ => None,
    }
}

/// Extracts a SharedVector<ValueOpaque> out of the given value `val`, writes that into the
/// `out` parameter and returns true; returns false if the value does not hold an extractable
/// array.
#[no_mangle]
pub extern "C" fn slint_interpreter_value_to_array(
    val: &ValueOpaque,
    out: *mut SharedVector<ValueOpaque>,
) -> bool {
    match val.as_value() {
        Value::Model(m) => {
            let vec = if let Some(model) = m.as_any().downcast_ref::<SharedVectorModel<Value>>() {
                model.shared_vector()
            } else {
                m.iter().collect()
            };

            // Safety: We assert that Value and ValueOpaque have the same size and alignment
            unsafe {
                std::ptr::write(
                    out,
                    std::mem::transmute::<SharedVector<Value>, SharedVector<ValueOpaque>>(vec),
                );
            }

            true
        }
        _ => false,
    }
}

#[no_mangle]
pub extern "C" fn slint_interpreter_value_to_brush(val: &ValueOpaque) -> Option<&Brush> {
    match val.as_value() {
        Value::Brush(b) => Some(b),
        _ => None,
    }
}

#[no_mangle]
pub extern "C" fn slint_interpreter_value_to_struct(val: &ValueOpaque) -> Option<&StructOpaque> {
    match val.as_value() {
        Value::Struct(s) => Some(unsafe { std::mem::transmute::<&Struct, &StructOpaque>(s) }),
        _ => None,
    }
}

#[no_mangle]
pub extern "C" fn slint_interpreter_value_to_image(val: &ValueOpaque) -> Option<&Image> {
    match val.as_value() {
        Value::Image(img) => Some(img),
        _ => None,
    }
}

#[repr(C)]
#[cfg(target_pointer_width = "64")]
pub struct StructOpaque([usize; 6]);
#[repr(C)]
#[cfg(target_pointer_width = "32")]
pub struct StructOpaque([usize; 8]);
/// Asserts that StructOpaque is at least as large as Struct, otherwise this would overflow
const _: usize = std::mem::size_of::<StructOpaque>() - std::mem::size_of::<Struct>();

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
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_struct_new(val: *mut StructOpaque) {
    std::ptr::write(val as *mut Struct, Struct::default())
}

/// Construct a new Struct in the given memory location
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_struct_clone(
    other: &StructOpaque,
    val: *mut StructOpaque,
) {
    std::ptr::write(val as *mut Struct, other.as_struct().clone())
}

/// Destruct the struct in that memory location
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_struct_destructor(val: *mut StructOpaque) {
    drop(std::ptr::read(val as *mut Struct))
}

#[no_mangle]
pub extern "C" fn slint_interpreter_struct_get_field<'a>(
    stru: &'a StructOpaque,
    name: Slice<u8>,
) -> Option<&'a ValueOpaque> {
    stru.as_struct()
        .get_field(std::str::from_utf8(&name).unwrap())
        .and_then(|v| unsafe { (v as *const Value as *const ValueOpaque).as_ref() })
}

#[no_mangle]
pub extern "C" fn slint_interpreter_struct_set_field<'a>(
    stru: &'a mut StructOpaque,
    name: Slice<u8>,
    value: &ValueOpaque,
) {
    stru.as_struct_mut()
        .set_field(std::str::from_utf8(&name).unwrap().into(), value.as_value().clone())
}

type StructIterator<'a> = std::collections::hash_map::Iter<'a, String, Value>;
#[repr(C)]
pub struct StructIteratorOpaque<'a>([usize; 5], std::marker::PhantomData<StructIterator<'a>>);
const _: usize =
    std::mem::size_of::<StructIteratorOpaque>() - std::mem::size_of::<StructIterator>();

#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_struct_iterator_destructor(
    val: *mut StructIteratorOpaque,
) {
    drop(std::ptr::read(val as *mut StructIterator))
}

/// The result of the slint_interpreter_struct_iterator_next function
/// If the iterator was at the end, the key will be empty, and the value will be None
#[repr(C)]
pub struct StructIteratorResult<'a> {
    k: Slice<'a, u8>,
    v: Option<&'a Value>,
}

/// Advance the iterator and return the next value, or an
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_struct_iterator_next<'a>(
    iter: &'a mut StructIteratorOpaque,
) -> StructIteratorResult<'a> {
    if let Some((str, val)) = (*(iter as *mut StructIteratorOpaque as *mut StructIterator)).next() {
        StructIteratorResult { k: Slice::from_slice(str.as_bytes()), v: Some(val) }
    } else {
        StructIteratorResult { k: Slice::default(), v: None }
    }
}

#[no_mangle]
pub extern "C" fn slint_interpreter_struct_make_iter(stru: &StructOpaque) -> StructIteratorOpaque {
    let ret_it: StructIterator = stru.as_struct().0.iter();
    unsafe {
        let mut r = std::mem::MaybeUninit::<StructIteratorOpaque>::uninit();
        std::ptr::write(r.as_mut_ptr() as *mut StructIterator, ret_it);
        r.assume_init()
    }
}

/// Get a property.
/// The `out` parameter must be uninitialized. If this function returns true, the out will be initialized
/// to the resulting value. If this function returns false, out is unchanged
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_component_instance_get_property(
    inst: &ErasedComponentBox,
    name: Slice<u8>,
    out: *mut ValueOpaque,
) -> bool {
    generativity::make_guard!(guard);
    let comp = inst.unerase(guard);
    match comp
        .description()
        .get_property(comp.borrow(), &normalize_identifier(std::str::from_utf8(&name).unwrap()))
    {
        Ok(val) => {
            std::ptr::write(out as *mut Value, val);
            true
        }
        Err(_) => false,
    }
}

#[no_mangle]
pub extern "C" fn slint_interpreter_component_instance_set_property(
    inst: &ErasedComponentBox,
    name: Slice<u8>,
    val: &ValueOpaque,
) -> bool {
    generativity::make_guard!(guard);
    let comp = inst.unerase(guard);
    comp.description()
        .set_property(
            comp.borrow(),
            &normalize_identifier(std::str::from_utf8(&name).unwrap()),
            val.as_value().clone(),
        )
        .is_ok()
}

/// Invoke a callback or function
/// The `out` parameter must be uninitialized. If this function returns true, the out will be initialized
/// to the resulting value. If this function returns false, out is unchanged
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_component_instance_invoke(
    inst: &ErasedComponentBox,
    name: Slice<u8>,
    args: Slice<ValueOpaque>,
    out: *mut ValueOpaque,
) -> bool {
    let args = std::mem::transmute::<Slice<ValueOpaque>, Slice<Value>>(args);
    generativity::make_guard!(guard);
    let comp = inst.unerase(guard);
    match comp.description().invoke(
        comp.borrow(),
        &normalize_identifier(std::str::from_utf8(&name).unwrap()),
        args.as_slice(),
    ) {
        Ok(val) => {
            std::ptr::write(out as *mut Value, val);
            true
        }
        Err(_) => false,
    }
}

/// Wrap the user_data provided by the native code and call the drop function on Drop.
///
/// Safety: user_data must be a pointer that can be destroyed by the drop_user_data function.
/// callback must be a valid callback that initialize the `ret`
struct CallbackUserData {
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
    callback: extern "C" fn(user_data: *mut c_void, arg: Slice<ValueOpaque>, ret: *mut ValueOpaque),
}

impl Drop for CallbackUserData {
    fn drop(&mut self) {
        if let Some(x) = self.drop_user_data {
            x(self.user_data)
        }
    }
}

impl CallbackUserData {
    fn call(&self, args: &[Value]) -> Value {
        unsafe {
            let args = std::mem::transmute::<&[Value], &[ValueOpaque]>(args);
            let mut ret = std::mem::MaybeUninit::<Value>::uninit();
            (self.callback)(
                self.user_data,
                Slice::from_slice(args),
                ret.as_mut_ptr() as *mut ValueOpaque,
            );
            ret.assume_init()
        }
    }
}

/// Set a handler for the callback.
/// The `callback` function must initialize the `ret` (the `ret` passed to the callback is initialized and is assumed initialized after the function)
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_component_instance_set_callback(
    inst: &ErasedComponentBox,
    name: Slice<u8>,
    callback: extern "C" fn(user_data: *mut c_void, arg: Slice<ValueOpaque>, ret: *mut ValueOpaque),
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
) -> bool {
    let ud = CallbackUserData { user_data, drop_user_data, callback };

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

/// Get a global property.
/// The `out` parameter must be uninitialized. If this function returns true, the out will be initialized
/// to the resulting value. If this function returns false, out is unchanged
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_component_instance_get_global_property(
    inst: &ErasedComponentBox,
    global: Slice<u8>,
    property_name: Slice<u8>,
    out: *mut ValueOpaque,
) -> bool {
    generativity::make_guard!(guard);
    let comp = inst.unerase(guard);
    match comp
        .description()
        .get_global(comp.borrow(), &normalize_identifier(std::str::from_utf8(&global).unwrap()))
        .and_then(|g| {
            g.as_ref()
                .get_property(&normalize_identifier(std::str::from_utf8(&property_name).unwrap()))
        }) {
        Ok(val) => {
            std::ptr::write(out as *mut Value, val);
            true
        }
        Err(_) => false,
    }
}

#[no_mangle]
pub extern "C" fn slint_interpreter_component_instance_set_global_property(
    inst: &ErasedComponentBox,
    global: Slice<u8>,
    property_name: Slice<u8>,
    val: &ValueOpaque,
) -> bool {
    generativity::make_guard!(guard);
    let comp = inst.unerase(guard);
    comp.description()
        .get_global(comp.borrow(), &normalize_identifier(std::str::from_utf8(&global).unwrap()))
        .and_then(|g| {
            g.as_ref()
                .set_property(
                    &normalize_identifier(std::str::from_utf8(&property_name).unwrap()),
                    val.as_value().clone(),
                )
                .map_err(|_| ())
        })
        .is_ok()
}

/// The `callback` function must initialize the `ret` (the `ret` passed to the callback is initialized and is assumed initialized after the function)
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_component_instance_set_global_callback(
    inst: &ErasedComponentBox,
    global: Slice<u8>,
    name: Slice<u8>,
    callback: extern "C" fn(user_data: *mut c_void, arg: Slice<ValueOpaque>, ret: *mut ValueOpaque),
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
) -> bool {
    let ud = CallbackUserData { user_data, drop_user_data, callback };

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

/// Invoke a global callback or function.
/// The `out` parameter must be uninitialized. If this function returns true, the out will be initialized
/// to the resulting value. If this function returns false, out is unchanged
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_component_instance_invoke_global(
    inst: &ErasedComponentBox,
    global: Slice<u8>,
    callable_name: Slice<u8>,
    args: Slice<ValueOpaque>,
    out: *mut ValueOpaque,
) -> bool {
    let args = std::mem::transmute::<Slice<ValueOpaque>, Slice<Value>>(args);
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
                g.as_ref().eval_function(
                    &normalize_identifier(callable_name),
                    args.as_slice().iter().cloned().collect(),
                )
            } else {
                g.as_ref().invoke_callback(&normalize_identifier(callable_name), args.as_slice())
            }
        }) {
        Ok(val) => {
            std::ptr::write(out as *mut Value, val);
            true
        }
        Err(_) => false,
    }
}

/// Show or hide
#[no_mangle]
pub extern "C" fn slint_interpreter_component_instance_show(
    inst: &ErasedComponentBox,
    is_visible: bool,
) {
    generativity::make_guard!(guard);
    let comp = inst.unerase(guard);
    if is_visible {
        comp.borrow_instance().window_adapter().show().unwrap();
    } else {
        comp.borrow_instance().window_adapter().hide().unwrap();
    }
}

/// Return a window for the component
///
/// The out pointer must be uninitialized and must be destroyed with
/// slint_windowrc_drop after usage
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_component_instance_window(
    inst: &ErasedComponentBox,
    out: *mut *const i_slint_core::window::ffi::WindowAdapterRcOpaque,
) {
    assert_eq!(
        core::mem::size_of::<Rc<dyn WindowAdapter>>(),
        core::mem::size_of::<i_slint_core::window::ffi::WindowAdapterRcOpaque>()
    );
    core::ptr::write(out as *mut *const Rc<dyn WindowAdapter>, inst.window_adapter() as *const _)
}

/// Instantiate an instance from a definition.
///
/// The `out` must be uninitialized and is going to be initialized after the call
/// and need to be destroyed with slint_interpreter_component_instance_destructor
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_component_instance_create(
    def: &ComponentDefinitionOpaque,
    out: *mut ComponentInstance,
) {
    std::ptr::write(out, def.as_component_definition().create().unwrap())
}

#[vtable::vtable]
#[repr(C)]
pub struct ModelAdaptorVTable {
    pub row_count: extern "C" fn(VRef<ModelAdaptorVTable>) -> usize,
    pub row_data:
        unsafe extern "C" fn(VRef<ModelAdaptorVTable>, row: usize, out: *mut ValueOpaque) -> bool,
    pub set_row_data: extern "C" fn(VRef<ModelAdaptorVTable>, row: usize, value: &ValueOpaque),
    pub get_notify: extern "C" fn(VRef<ModelAdaptorVTable>) -> &ModelNotifyOpaque,
    pub drop: extern "C" fn(VRefMut<ModelAdaptorVTable>),
}

struct ModelAdaptorWrapper(vtable::VBox<ModelAdaptorVTable>);
impl Model for ModelAdaptorWrapper {
    type Data = Value;

    fn row_count(&self) -> usize {
        self.0.row_count()
    }

    fn row_data(&self, row: usize) -> Option<Value> {
        unsafe {
            let mut v = std::mem::MaybeUninit::<Value>::uninit();
            if self.0.row_data(row, v.as_mut_ptr() as *mut ValueOpaque) {
                Some(v.assume_init())
            } else {
                None
            }
        }
    }

    fn model_tracker(&self) -> &dyn i_slint_core::model::ModelTracker {
        self.0.get_notify().as_model_notify()
    }

    fn set_row_data(&self, row: usize, data: Value) {
        let val: &ValueOpaque = unsafe { std::mem::transmute::<&Value, &ValueOpaque>(&data) };
        self.0.set_row_data(row, val);
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

impl ModelNotifyOpaque {
    fn as_model_notify(&self) -> &ModelNotify {
        // Safety: there should be no way to construct a ModelNotifyOpaque without it holding an actual ModelNotify
        unsafe { std::mem::transmute::<&ModelNotifyOpaque, &ModelNotify>(self) }
    }
}

/// Construct a new ModelNotifyNotify in the given memory region
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_model_notify_new(val: *mut ModelNotifyOpaque) {
    std::ptr::write(val as *mut ModelNotify, ModelNotify::default());
}

/// Destruct the value in that memory location
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_model_notify_destructor(val: *mut ModelNotifyOpaque) {
    drop(std::ptr::read(val as *mut ModelNotify))
}

#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_model_notify_row_changed(
    notify: &ModelNotifyOpaque,
    row: usize,
) {
    notify.as_model_notify().row_changed(row);
}

#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_model_notify_row_added(
    notify: &ModelNotifyOpaque,
    row: usize,
    count: usize,
) {
    notify.as_model_notify().row_added(row, count);
}

#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_model_notify_reset(notify: &ModelNotifyOpaque) {
    notify.as_model_notify().reset();
}

#[no_mangle]
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
#[repr(C)]
pub enum DiagnosticLevel {
    /// The diagnostic belongs to an error.
    Error,
    /// The diagnostic belongs to a warning.
    Warning,
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
    /// The column within the source file. Column numbers start at 0.
    column: usize,
    /// The level of the diagnostic, such as a warning or an error.
    level: DiagnosticLevel,
}

#[repr(C)]
#[cfg(target_pointer_width = "64")]
pub struct ComponentCompilerOpaque([usize; 13]);

#[repr(C)]
#[cfg(target_pointer_width = "32")]
#[repr(align(8))]
pub struct ComponentCompilerOpaque([usize; 16]);

/// Asserts that ComponentCompilerOpaque is as large as ComponentCompiler and has the same alignment, to make transmute safe.
const _: [(); std::mem::size_of::<ComponentCompilerOpaque>()] =
    [(); std::mem::size_of::<ComponentCompiler>()];
const _: [(); std::mem::align_of::<ComponentCompilerOpaque>()] =
    [(); std::mem::align_of::<ComponentCompiler>()];

impl ComponentCompilerOpaque {
    fn as_component_compiler(&self) -> &ComponentCompiler {
        // Safety: there should be no way to construct a ComponentCompilerOpaque without it holding an actual ComponentCompiler
        unsafe { std::mem::transmute::<&ComponentCompilerOpaque, &ComponentCompiler>(self) }
    }
    fn as_component_compiler_mut(&mut self) -> &mut ComponentCompiler {
        // Safety: there should be no way to construct a ComponentCompilerOpaque without it holding an actual ComponentCompiler
        unsafe { std::mem::transmute::<&mut ComponentCompilerOpaque, &mut ComponentCompiler>(self) }
    }
}

#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_component_compiler_new(
    compiler: *mut ComponentCompilerOpaque,
) {
    std::ptr::write(compiler as *mut ComponentCompiler, ComponentCompiler::default())
}

#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_component_compiler_destructor(
    compiler: *mut ComponentCompilerOpaque,
) {
    drop(std::ptr::read(compiler as *mut ComponentCompiler))
}

#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_component_compiler_set_include_paths(
    compiler: &mut ComponentCompilerOpaque,
    paths: &SharedVector<SharedString>,
) {
    compiler
        .as_component_compiler_mut()
        .set_include_paths(paths.iter().map(|path| path.as_str().into()).collect())
}

#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_component_compiler_set_style(
    compiler: &mut ComponentCompilerOpaque,
    style: Slice<u8>,
) {
    compiler.as_component_compiler_mut().set_style(std::str::from_utf8(&style).unwrap().to_string())
}

#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_component_compiler_get_style(
    compiler: &ComponentCompilerOpaque,
    style_out: &mut SharedString,
) {
    *style_out =
        compiler.as_component_compiler().style().map_or(SharedString::default(), |s| s.into());
}

#[no_mangle]
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

#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_component_compiler_get_diagnostics(
    compiler: &ComponentCompilerOpaque,
    out_diags: &mut SharedVector<Diagnostic>,
) {
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
                _ => DiagnosticLevel::Warning,
            },
        }
    }));
}

#[no_mangle]
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
            std::ptr::write(component_definition_ptr as *mut ComponentDefinition, definition);
            true
        }
        None => false,
    }
}

#[no_mangle]
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
            std::ptr::write(component_definition_ptr as *mut ComponentDefinition, definition);
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
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_component_definition_clone(
    other: &ComponentDefinitionOpaque,
    def: *mut ComponentDefinitionOpaque,
) {
    std::ptr::write(def as *mut ComponentDefinition, other.as_component_definition().clone())
}

/// Destruct the component definition in that memory location
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_component_definition_destructor(
    val: *mut ComponentDefinitionOpaque,
) {
    drop(std::ptr::read(val as *mut ComponentDefinition))
}

/// Returns the list of properties of the component the component definition describes
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_component_definition_properties(
    def: &ComponentDefinitionOpaque,
    props: &mut SharedVector<PropertyDescriptor>,
) {
    props.extend((&*def).as_component_definition().properties().map(
        |(property_name, property_type)| PropertyDescriptor {
            property_name: property_name.into(),
            property_type,
        },
    ))
}

/// Returns the list of callback names of the component the component definition describes
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_component_definition_callbacks(
    def: &ComponentDefinitionOpaque,
    callbacks: &mut SharedVector<SharedString>,
) {
    callbacks.extend((&*def).as_component_definition().callbacks().map(|name| name.into()))
}

/// Return the name of the component definition
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_component_definition_name(
    def: &ComponentDefinitionOpaque,
    name: &mut SharedString,
) {
    *name = (&*def).as_component_definition().name().into()
}

/// Returns a vector of strings with the names of all exported global singletons.
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_component_definition_globals(
    def: &ComponentDefinitionOpaque,
    names: &mut SharedVector<SharedString>,
) {
    names.extend((&*def).as_component_definition().globals().map(|name| name.into()))
}

/// Returns a vector of the property descriptors of the properties of the specified publicly exported global
/// singleton. Returns true if a global exists under the specified name; false otherwise.
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_component_definition_global_properties(
    def: &ComponentDefinitionOpaque,
    global_name: Slice<u8>,
    properties: &mut SharedVector<PropertyDescriptor>,
) -> bool {
    if let Some(property_it) = (&*def)
        .as_component_definition()
        .global_properties(std::str::from_utf8(&global_name).unwrap())
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
#[no_mangle]
pub unsafe extern "C" fn slint_interpreter_component_definition_global_callbacks(
    def: &ComponentDefinitionOpaque,
    global_name: Slice<u8>,
    names: &mut SharedVector<SharedString>,
) -> bool {
    if let Some(name_it) = (&*def)
        .as_component_definition()
        .global_callbacks(std::str::from_utf8(&global_name).unwrap())
    {
        names.extend(name_it.map(|name| name.into()));
        true
    } else {
        false
    }
}
