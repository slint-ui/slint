// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::langtype::Type;
use i_slint_core::window::WindowInner;
use napi::bindgen_prelude::*;
use napi::{Env, Result};
use slint_interpreter::{ComponentHandle, ComponentInstance, Value};

use crate::JsWindow;

use super::JsComponentDefinition;

#[napi(js_name = "ComponentInstance")]
pub struct JsComponentInstance {
    inner: ComponentInstance,
}

impl From<ComponentInstance> for JsComponentInstance {
    fn from(instance: ComponentInstance) -> Self {
        Self { inner: instance }
    }
}

#[napi]
impl JsComponentInstance {
    #[napi(constructor)]
    pub fn new() -> napi::Result<Self> {
        Err(napi::Error::from_reason(
            "ComponentInstance can only be created by using ComponentCompiler.".to_string(),
        ))
    }

    #[napi]
    pub fn definition(&self) -> JsComponentDefinition {
        self.inner.definition().into()
    }

    #[napi]
    pub fn get_property<'a>(&self, env: &'a Env, name: String) -> Result<Unknown<'a>> {
        let value = self
            .inner
            .get_property(name.as_ref())
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        super::value::to_js_unknown(env, &value)
    }

    #[napi]
    pub fn set_property(&self, env: &Env, prop_name: String, js_value: Unknown<'_>) -> Result<()> {
        let (ty, _) = self
            .inner
            .definition()
            .properties_and_callbacks()
            .find_map(|(name, proptype)| if name == prop_name { Some(proptype) } else { None })
            .ok_or(())
            .map_err(|_| {
                napi::Error::from_reason(format!("Property {prop_name} not found in the component"))
            })?;

        self.inner
            .set_property(&prop_name, super::value::to_value(env, js_value, &ty)?)
            .map_err(|e| napi::Error::from_reason(format!("{e}")))?;

        Ok(())
    }

    #[napi]
    pub fn get_global_property<'a>(
        &self,
        env: &'a Env,
        global_name: String,
        name: String,
    ) -> Result<Unknown<'a>> {
        if !self.definition().globals().contains(&global_name) {
            return Err(napi::Error::from_reason(format!("Global {global_name} not found")));
        }
        let value = self
            .inner
            .get_global_property(global_name.as_ref(), name.as_ref())
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        super::value::to_js_unknown(env, &value)
    }

    #[napi]
    pub fn set_global_property(
        &self,
        env: &Env,
        global_name: String,
        prop_name: String,
        js_value: Unknown<'_>,
    ) -> Result<()> {
        let (ty, _) = self
            .inner
            .definition()
            .global_properties_and_callbacks(global_name.as_str())
            .ok_or(napi::Error::from_reason(format!("Global {global_name} not found")))?
            .find_map(|(name, proptype)| if name == prop_name { Some(proptype) } else { None })
            .ok_or(())
            .map_err(|_| {
                napi::Error::from_reason(format!(
                    "Property {prop_name} of global {global_name} not found in the component"
                ))
            })?;

        self.inner
            .set_global_property(
                global_name.as_str(),
                &prop_name,
                super::value::to_value(env, js_value, &ty)?,
            )
            .map_err(|e| napi::Error::from_reason(format!("{e}")))?;

        Ok(())
    }

    #[napi]
    pub fn set_callback(
        &self,
        env: &Env,
        callback_name: String,
        callback: DynFunction<'_>,
    ) -> Result<()> {
        let function_ref = StoredFunction::new(&callback)?;
        let env = *env;

        let (ty, _) = self
            .inner
            .definition()
            .properties_and_callbacks()
            .find_map(|(name, proptype)| if name == callback_name { Some(proptype) } else { None })
            .ok_or(())
            .map_err(|_| {
                napi::Error::from_reason(format!(
                    "Callback {callback_name} not found in the component"
                ))
            })?;

        if let Type::Callback(callback) = ty {
            self.inner
                .set_callback(callback_name.as_str(), {
                    let return_type = callback.return_type.clone();
                    let callback_name = callback_name.clone();

                    move |args| {
                        let js_args: Vec<napi::sys::napi_value> = args
                            .iter()
                            .filter_map(|v| Some(super::value::to_js_unknown(&env, v).ok()?.raw()))
                            .collect();

                        let result = match function_ref.call(&env, js_args) {
                            Ok(result) => result,
                            Err(err) => {
                                crate::console_err!(
                                    env,
                                    "Node.js: Invoking callback '{callback_name}' failed: {err}"
                                );
                                return Value::Void;
                            }
                        };

                        if matches!(return_type, Type::Void) {
                            Value::Void
                        } else if let Ok(value) = super::to_value(&env, result, &return_type) {
                            value
                        } else {
                            eprintln!(
                                "Node.js: cannot convert return type of callback {callback_name}"
                            );
                            slint_interpreter::default_value_for_type(&return_type)
                        }
                    }
                })
                .map_err(|_| napi::Error::from_reason("Cannot set callback."))?;

            return Ok(());
        }

        Err(napi::Error::from_reason(format!("{callback_name} is not a callback").as_str()))
    }

    #[napi]
    pub fn set_global_callback(
        &self,
        env: &Env,
        global_name: String,
        callback_name: String,
        callback: DynFunction<'_>,
    ) -> Result<()> {
        let function_ref = StoredFunction::new(&callback)?;
        let env = *env;

        let (ty, _) = self
            .inner
            .definition()
            .global_properties_and_callbacks(global_name.as_str())
            .ok_or(napi::Error::from_reason(format!("Global {global_name} not found")))?
            .find_map(|(name, proptype)| if name == callback_name { Some(proptype) } else { None })
            .ok_or(())
            .map_err(|_| {
                napi::Error::from_reason(format!(
                    "Callback {callback_name} of global {global_name} not found in the component"
                ))
            })?;

        if let Type::Callback(callback) = ty {
            self.inner
                .set_global_callback(global_name.as_str(), callback_name.as_str(), {
                    let return_type = callback.return_type.clone();
                    let _global_name = global_name.clone();
                    let callback_name = callback_name.clone();

                    move |args| {
                        let js_args: Vec<napi::sys::napi_value> = args
                            .iter()
                            .filter_map(|v| Some(super::value::to_js_unknown(&env, v).ok()?.raw()))
                            .collect();

                        let result = match function_ref.call(&env, js_args) {
                            Ok(result) => result,
                            Err(err) => {
                                crate::console_err!(env, "Node.js: Invoking global callback '{callback_name}' failed: {err}");
                                return Value::Void;
                            }
                        };

                        if matches!(return_type, Type::Void) {
                            Value::Void
                        } else if let Ok(value) = super::to_value(&env, result, &return_type) {
                            value
                        } else {
                            eprintln!("Node.js: cannot convert return type of callback {callback_name}");
                            slint_interpreter::default_value_for_type(&return_type)
                        }
                    }
                })
                .map_err(|_| napi::Error::from_reason("Cannot set callback."))?;

            return Ok(());
        }

        Err(napi::Error::from_reason(format!("{callback_name} is not a callback").as_str()))
    }

    fn invoke_args(
        env: &Env,
        callback_name: &String,
        arguments: Vec<Unknown<'_>>,
        args: &[Type],
    ) -> Result<Vec<Value>> {
        let count = args.len();
        let args = arguments
            .into_iter()
            .zip(args)
            .map(|(a, ty)| super::value::to_value(env, a, ty))
            .collect::<Result<Vec<_>, _>>()?;
        if args.len() != count {
            return Err(napi::Error::from_reason(
                format!(
                    "{} expect {} arguments, but {} where provided",
                    callback_name,
                    count,
                    args.len()
                )
                .as_str(),
            ));
        }
        Ok(args)
    }

    #[napi]
    pub fn invoke<'a>(
        &self,
        env: &'a Env,
        callback_name: String,
        callback_arguments: Vec<Unknown<'_>>,
    ) -> Result<Unknown<'a>> {
        let (ty, _) = self
            .inner
            .definition()
            .properties_and_callbacks()
            .find_map(|(name, proptype)| if name == callback_name { Some(proptype) } else { None })
            .ok_or(())
            .map_err(|_| {
                napi::Error::from_reason(
                    format!("Callback {callback_name} not found in the component").as_str(),
                )
            })?;

        let args = match ty {
            Type::Callback(function) | Type::Function(function) => {
                Self::invoke_args(env, &callback_name, callback_arguments, &function.args)?
            }
            _ => {
                return Err(napi::Error::from_reason(
                    format!("{callback_name} is not a callback or a function").as_str(),
                ));
            }
        };

        let result = self
            .inner
            .invoke(callback_name.as_str(), args.as_slice())
            .map_err(|_| napi::Error::from_reason("Cannot invoke callback."))?;
        super::to_js_unknown(env, &result)
    }

    #[napi]
    pub fn invoke_global<'a>(
        &self,
        env: &'a Env,
        global_name: String,
        callback_name: String,
        callback_arguments: Vec<Unknown<'_>>,
    ) -> Result<Unknown<'a>> {
        let (ty, _) = self
            .inner
            .definition()
            .global_properties_and_callbacks(global_name.as_str())
            .ok_or(napi::Error::from_reason(format!("Global {global_name} not found")))?
            .find_map(|(name, proptype)| if name == callback_name { Some(proptype) } else { None })
            .ok_or(())
            .map_err(|_| {
                napi::Error::from_reason(
                    format!(
                        "Callback {callback_name} of global {global_name} not found in the component"
                    )
                    .as_str(),
                )
            })?;

        let args = match ty {
            Type::Callback(function) | Type::Function(function) => {
                Self::invoke_args(env, &callback_name, callback_arguments, &function.args)?
            }
            _ => {
                return Err(napi::Error::from_reason(
                    format!(
                        "{callback_name} is not a callback or a function on global {global_name}"
                    )
                    .as_str(),
                ));
            }
        };

        let result = self
            .inner
            .invoke_global(global_name.as_str(), callback_name.as_str(), args.as_slice())
            .map_err(|_| napi::Error::from_reason("Cannot invoke callback."))?;
        super::to_js_unknown(env, &result)
    }

    #[napi]
    pub fn send_mouse_click(&self, x: f64, y: f64) {
        slint_interpreter::testing::send_mouse_click(&self.inner, x as f32, y as f32);
    }

    #[napi]
    pub fn send_keyboard_string_sequence(&self, sequence: String) {
        slint_interpreter::testing::send_keyboard_string_sequence(&self.inner, sequence.into());
    }

    #[napi]
    pub fn window(&self) -> Result<JsWindow> {
        Ok(JsWindow { inner: WindowInner::from_pub(self.inner.window()).window_adapter() })
    }
}

/// A reference-counted handle to a JS object, for storing object references
/// that outlive the current scope (e.g. model implementations).
pub struct RefCountedReference {
    inner: Option<napi::UnknownRef>,
    env: Env,
}

impl RefCountedReference {
    pub fn new(env: &Env, value: &Object) -> Result<Self> {
        let unknown = (*value).into_unknown(env)?;
        Ok(Self { inner: Some(unknown.create_ref()?), env: *env })
    }

    pub fn get_unknown(&self) -> Result<Unknown<'_>> {
        self.inner
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason("Reference already dropped"))?
            .get_value(&self.env)
    }
}

impl Drop for RefCountedReference {
    fn drop(&mut self) {
        if let Some(r) = self.inner.take() {
            let _: napi::Result<()> = r.unref(&self.env);
        }
    }
}

/// A stored reference to a JS function that can be called with dynamic arguments.
/// Uses `FunctionRef` for lifecycle management and compat `JsFunction::call` for invocation.
/// Type alias for a JS function that accepts dynamic arguments.
pub type DynFunction<'a> = Function<'a, crate::DynArgs, Unknown<'static>>;

/// A stored reference to a JS function that can be called with dynamic arguments.
pub struct StoredFunction {
    func_ref: FunctionRef<crate::DynArgs, Unknown<'static>>,
}

impl StoredFunction {
    pub fn new(func: &DynFunction<'_>) -> Result<Self> {
        Ok(Self { func_ref: func.create_ref()? })
    }

    /// Call the function with dynamic raw JS values.
    pub fn call(&self, env: &Env, args: Vec<napi::sys::napi_value>) -> Result<Unknown<'_>> {
        let func = self.func_ref.borrow_back(env)?;
        func.call(crate::DynArgs(args))
    }
}
