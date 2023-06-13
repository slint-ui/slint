// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use napi::{Env, Error, JsUnknown, Result};
use slint_interpreter::{ComponentHandle, ComponentInstance};

use super::JsComponentDefinition;

#[napi(js_name = "JsComponentInstance")]
pub struct JsComponentInstance {
    internal: ComponentInstance,
}

impl From<ComponentInstance> for JsComponentInstance {
    fn from(instance: ComponentInstance) -> Self {
        Self { internal: instance }
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
        self.internal.definition().into()
    }

    #[napi]
    pub fn run(&self) {
        self.internal.run().unwrap()
    }

    #[napi]
    pub fn get_property(&self, env: Env, name: String) -> Result<JsUnknown> {
        let value = self
            .internal
            .get_property(name.as_ref())
            .map_err(|e| Error::from_reason(format!("{e}")))?;
        super::value::to_js_unknown(&env, &value)
    }

    #[napi]
    pub fn set_property(&self, env: Env, name: String, js_value: JsUnknown) -> Result<()> {
        let expected_type = self
            .internal
            .definition()
            .properties()
            .find_map(
                |(prop_name, prop_type)| {
                    if name == prop_name {
                        Some(prop_type)
                    } else {
                        None
                    }
                },
            )
            .ok_or_else(|| Error::from_reason(format!("Cannot set unknown property {name}")))?;
        let value = super::value::to_value(&env, js_value, expected_type)?;
        self.internal.set_property(&name, value).map_err(|e| Error::from_reason(format!("{e}")))?;
        Ok(())
    }
}
