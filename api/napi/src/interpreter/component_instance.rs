// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use napi::{Env, Error, JsFunction, JsUnknown, Result};
use slint_interpreter::{ComponentHandle, ComponentInstance, Value};

use super::JsComponentDefinition;

#[napi(js_name = "JsComponentInstance")]
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
    pub fn new() -> Self {
        unreachable!("ComponentDefinition can only be created by using ComponentCompiler.")
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
            .map_err(|e| Error::from_reason(format!("{e}")))?;
        super::value::to_js_unknown(&env, &value)
    }

    #[napi]
    pub fn set_property(&self, env: Env, name: String, js_value: JsUnknown) -> Result<()> {
        let value = super::value::to_value(
            &env,
            &self.inner,
            js_value,
            &self
                .inner
                .get_property(&name)
                .map_err(|_| napi::Error::from_reason("Cannot get property."))?,
        )?;
        self.inner.set_property(&name, value).map_err(|e| Error::from_reason(format!("{e}")))?;
        Ok(())
    }

    #[napi]
    pub fn set_callback(&self, env: Env, name: String, callback: JsFunction) -> Result<()> {
        self.inner
            .set_callback(name.as_str(), move |values| {
                println!("---");
                let result = callback
                    .call(
                        None,
                        values
                            .iter()
                            .map(|v| super::value::to_js_unknown(&env, v).unwrap())
                            .collect::<Vec<JsUnknown>>()
                            .as_ref(),
                    )
                    .unwrap();

                super::js_unknown_to_value(env, result).unwrap()
            })
            .map_err(|_| napi::Error::from_reason("Cannot set callback."))?;

        Ok(())
    }

    #[napi]
    pub fn invoke(&self, env: Env, name: String, mut value: Vec<JsUnknown>) -> Result<()> {
        self.inner
            .invoke(
                name.as_str(),
                value
                    .drain(0..(value.len()))
                    .map(|unknown| super::value::js_unknown_to_value(env, unknown).unwrap())
                    .collect::<Vec<Value>>()
                    .as_ref(),
            )
            .map_err(|_| napi::Error::from_reason("Cannot invoke callback."))?;

        Ok(())
    }
}
