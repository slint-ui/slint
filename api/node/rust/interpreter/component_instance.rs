// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::langtype::Type;
use i_slint_core::window::WindowInner;
use napi::{Env, Error, JsFunction, JsUnknown, NapiRaw, NapiValue, Ref, Result};
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
    pub fn get_property(&self, env: Env, name: String) -> Result<JsUnknown> {
        let value = self
            .inner
            .get_property(name.as_ref())
            .map_err(|e| Error::from_reason(e.to_string()))?;
        super::value::to_js_unknown(&env, &value)
    }

    #[napi]
    pub fn set_property(&self, env: Env, prop_name: String, js_value: JsUnknown) -> Result<()> {
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
            .set_property(&prop_name, super::value::to_value(&env, js_value, &ty)?)
            .map_err(|e| Error::from_reason(format!("{e}")))?;

        Ok(())
    }

    #[napi]
    pub fn get_global_property(
        &self,
        env: Env,
        global_name: String,
        name: String,
    ) -> Result<JsUnknown> {
        if !self.definition().globals().contains(&global_name) {
            return Err(napi::Error::from_reason(format!("Global {global_name} not found")));
        }
        let value = self
            .inner
            .get_global_property(global_name.as_ref(), name.as_ref())
            .map_err(|e| Error::from_reason(e.to_string()))?;
        super::value::to_js_unknown(&env, &value)
    }

    #[napi]
    pub fn set_global_property(
        &self,
        env: Env,
        global_name: String,
        prop_name: String,
        js_value: JsUnknown,
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
                super::value::to_value(&env, js_value, &ty)?,
            )
            .map_err(|e| Error::from_reason(format!("{e}")))?;

        Ok(())
    }

    #[napi]
    pub fn set_callback(
        &self,
        env: Env,
        callback_name: String,
        callback: JsFunction,
    ) -> Result<()> {
        let function_ref = RefCountedReference::new(&env, callback)?;

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
                        let Ok(callback) = function_ref.get::<JsFunction>() else {
                            eprintln!("Node.js: cannot get reference of callback {callback_name} because it has the wrong type");
                            return Value::Void;
                        };

                        let result = match callback
                            .call(
                                None,
                                args.iter()
                                    .map(|v| super::value::to_js_unknown(&env, v).unwrap())
                                    .collect::<Vec<JsUnknown>>()
                                    .as_ref()
                            ) {
                            Ok(result) => result,
                            Err(err) => {
                                crate::console_err!(env, "Node.js: Invoking callback '{callback_name}' failed: {err}");
                                return Value::Void;
                            }
                        };

                        if matches!(return_type, Type::Void) {
                            Value::Void
                        } else if let Ok(value) = super::to_value(&env, result, &return_type) {
                            return value;
                        } else {
                            eprintln!("Node.js: cannot convert return type of callback {callback_name}");
                            return slint_interpreter::default_value_for_type(&return_type);
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
        env: Env,
        global_name: String,
        callback_name: String,
        callback: JsFunction,
    ) -> Result<()> {
        let function_ref = RefCountedReference::new(&env, callback)?;

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
                    let global_name = global_name.clone();
                    let callback_name = callback_name.clone();

                    move |args| {
                        let Ok(callback) = function_ref.get::<JsFunction>() else {
                            eprintln!(
                                "Node.js: cannot get reference of callback {callback_name} of global {global_name} because it has the wrong type"
                            );
                            return Value::Void;
                        };

                        let result = match callback
                            .call(
                                None,
                                args.iter()
                                    .map(|v| super::value::to_js_unknown(&env, v).unwrap())
                                    .collect::<Vec<JsUnknown>>()
                                    .as_ref()
                            ) {
                            Ok(result) => result,
                            Err(err) => {
                                crate::console_err!(env, "Node.js: Invoking global callback '{callback_name}' failed: {err}");
                                return Value::Void;
                            }
                        };

                        if matches!(return_type, Type::Void) {
                            Value::Void
                        } else if let Ok(value) = super::to_value(&env, result, &return_type) {
                            return value;
                        } else {
                            eprintln!("Node.js: cannot convert return type of callback {callback_name}");
                            return slint_interpreter::default_value_for_type(&return_type);
                        }
                    }
                })
                .map_err(|_| napi::Error::from_reason("Cannot set callback."))?;

            return Ok(());
        }

        Err(napi::Error::from_reason(format!("{callback_name} is not a callback").as_str()))
    }

    fn invoke_args(
        env: Env,
        callback_name: &String,
        arguments: Vec<JsUnknown>,
        args: &[Type],
    ) -> Result<Vec<Value>> {
        let count = args.len();
        let args = arguments
            .into_iter()
            .zip(args)
            .map(|(a, ty)| super::value::to_value(&env, a, ty))
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
    pub fn invoke(
        &self,
        env: Env,
        callback_name: String,
        callback_arguments: Vec<JsUnknown>,
    ) -> Result<JsUnknown> {
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
        super::to_js_unknown(&env, &result)
    }

    #[napi]
    pub fn invoke_global(
        &self,
        env: Env,
        global_name: String,
        callback_name: String,
        callback_arguments: Vec<JsUnknown>,
    ) -> Result<JsUnknown> {
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
        super::to_js_unknown(&env, &result)
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

// Wrapper around Ref<>, which requires manual ref-counting.
pub struct RefCountedReference {
    env: Env,
    reference: Ref<()>,
}

impl RefCountedReference {
    pub fn new<T: NapiRaw>(env: &Env, value: T) -> Result<Self> {
        Ok(Self { env: *env, reference: env.create_reference(value)? })
    }

    pub fn get<T: NapiValue>(&self) -> Result<T> {
        self.env.get_reference_value(&self.reference)
    }
}

impl Drop for RefCountedReference {
    fn drop(&mut self) {
        self.reference.unref(self.env).unwrap();
    }
}
