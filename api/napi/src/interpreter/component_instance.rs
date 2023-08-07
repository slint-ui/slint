// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use i_slint_compiler::langtype::Type;
use napi::{Env, Error, JsFunction, JsUnknown, NapiRaw, NapiValue, Ref, Result};
use slint_interpreter::{ComponentHandle, ComponentInstance, Value};

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
    pub fn run(&self) {
        self.inner.run().unwrap()
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
        let ty = self
            .inner
            .definition()
            .properties_and_callbacks()
            .find_map(|(name, proptype)| if name == prop_name { Some(proptype) } else { None })
            .ok_or(())
            .map_err(|_| {
                napi::Error::from_reason(format!("Property {prop_name} not found in the component"))
            })?;

        self.inner
            .set_property(&prop_name, super::value::to_value(&env, js_value, ty)?)
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

        let ty = self
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

        if let Type::Callback { return_type, .. } = ty {
            self.inner
                .set_callback(callback_name.as_str(), {
                    let return_type = return_type.clone();

                    move |args| {
                        let callback: JsFunction = function_ref.get().unwrap();
                        let result = callback
                            .call(
                                None,
                                args.iter()
                                    .map(|v| super::value::to_js_unknown(&env, v).unwrap())
                                    .collect::<Vec<JsUnknown>>()
                                    .as_ref(),
                            )
                            .unwrap();

                        if let Some(return_type) = &return_type {
                            super::to_value(&env, result, *(*return_type).clone()).unwrap()
                        } else {
                            Value::Void
                        }
                    }
                })
                .map_err(|_| napi::Error::from_reason("Cannot set callback."))?;

            return Ok(());
        }

        Err(napi::Error::from_reason(format!("{} is not a callback", callback_name).as_str()))
    }

    #[napi]
    pub fn invoke(
        &self,
        env: Env,
        callback_name: String,
        arguments: Vec<JsUnknown>,
    ) -> Result<JsUnknown> {
        let ty = self
            .inner
            .definition()
            .properties_and_callbacks()
            .find_map(|(name, proptype)| if name == callback_name { Some(proptype) } else { None })
            .ok_or(())
            .map_err(|_| {
                napi::Error::from_reason(
                    format!("Callback {} not found in the component", callback_name).as_str(),
                )
            })?;

        let args = if let Type::Callback { args, .. } = ty {
            let count = args.len();
            let args = arguments
                .into_iter()
                .zip(args.into_iter())
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
            args
        } else {
            return Err(napi::Error::from_reason(
                format!("{} is not a callback", callback_name).as_str(),
            ));
        };

        let result = self
            .inner
            .invoke(callback_name.as_str(), args.as_slice())
            .map_err(|_| napi::Error::from_reason("Cannot invoke callback."))?;
        super::to_js_unknown(&env, &result)
    }
}

// Wrapper around Ref<>, which requires manual ref-counting.
struct RefCountedReference {
    env: Env,
    reference: Ref<()>,
}

impl RefCountedReference {
    fn new<T: NapiRaw>(env: &Env, value: T) -> Result<Self> {
        Ok(Self { env: env.clone(), reference: env.create_reference(value)? })
    }

    fn get<T: NapiValue>(&self) -> Result<T> {
        self.env.get_reference_value(&self.reference)
    }
}

impl Drop for RefCountedReference {
    fn drop(&mut self) {
        self.reference.unref(self.env).unwrap();
    }
}
