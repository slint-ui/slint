// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell:ignore proptype
use i_slint_compiler::langtype::Type;
use i_slint_core::window::WindowInner;
use napi::bindgen_prelude::*;
use napi::{Env, Result};
use slint_interpreter::{ComponentHandle, ComponentInstance, Value};

use crate::{JsAnchorOwner, JsWindow};

use super::JsComponentDefinition;

#[napi(js_name = "ComponentInstance")]
pub struct JsComponentInstance {
    /// Per-instance anchor-ID counter, shared with [`JsAnchorOwner`] via `Rc`.
    /// Declared before `inner` so it's dropped first:
    /// when `inner` drops its models and DataTransfer values,
    /// `Weak::upgrade()` already returns `None` and the pinned side's
    /// `Drop` skips NAPI calls.
    anchor_seq: std::rc::Rc<std::cell::Cell<u32>>,
    inner: ComponentInstance,
}

impl From<ComponentInstance> for JsComponentInstance {
    fn from(instance: ComponentInstance) -> Self {
        Self { inner: instance, anchor_seq: std::rc::Rc::new(std::cell::Cell::new(0)) }
    }
}

impl JsComponentInstance {
    fn anchor_owner(&self, env: &Env, this: &This<Object<'_>>) -> Result<JsAnchorOwner> {
        Ok(JsAnchorOwner {
            owner_weak: crate::weak_ref::weak_ref_from_object(env, &this.object)?,
            seq: std::rc::Rc::downgrade(&self.anchor_seq),
        })
    }

    /// Build the Rust closure for `set_callback` / `set_global_callback`.
    ///
    /// The JS function is stored as a property on `this` (not as a NAPI GC root).
    /// The closure holds a weak reference and looks the function up at call time.
    fn make_callback_handler(
        env: Env,
        owner: JsAnchorOwner,
        prop_key: String,
        return_type: Type,
        callback_name: String,
    ) -> impl Fn(&[Value]) -> Value {
        let weak_this = owner.owner_weak.clone();
        move |args: &[Value]| {
            let Some(obj) = crate::weak_ref::weak_ref_get_object(&weak_this, env) else {
                return Value::Void;
            };
            let Ok(func) =
                obj.get_named_property::<Function<'_, crate::DynArgs, Unknown<'_>>>(&prop_key)
            else {
                return Value::Void;
            };

            let js_args: Vec<napi::sys::napi_value> = args
                .iter()
                .filter_map(|v| Some(super::value::to_js_unknown(&env, v).ok()?.raw()))
                .collect();

            let result = match func.call(crate::DynArgs(js_args)) {
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
            } else if let Ok(value) = super::to_value(&env, result, &return_type, &owner) {
                value
            } else {
                eprintln!("Node.js: cannot convert return type of callback {callback_name}");
                slint_interpreter::default_value_for_type(&return_type)
            }
        }
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
    pub fn set_property(
        &self,
        env: &Env,
        this: This<Object<'_>>,
        prop_name: String,
        js_value: Unknown<'_>,
    ) -> Result<()> {
        let (ty, _) = self
            .inner
            .definition()
            .properties_and_callbacks()
            .find_map(|(name, proptype)| if name == prop_name { Some(proptype) } else { None })
            .ok_or(())
            .map_err(|_| {
                napi::Error::from_reason(format!("Property {prop_name} not found in the component"))
            })?;

        let owner = self.anchor_owner(env, &this)?;
        self.inner
            .set_property(&prop_name, super::value::to_value(env, js_value, &ty, &owner)?)
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
        this: This<Object<'_>>,
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

        let owner = self.anchor_owner(env, &this)?;
        self.inner
            .set_global_property(
                global_name.as_str(),
                &prop_name,
                super::value::to_value(env, js_value, &ty, &owner)?,
            )
            .map_err(|e| napi::Error::from_reason(format!("{e}")))?;

        Ok(())
    }

    #[napi]
    pub fn set_callback(
        &self,
        env: &Env,
        mut this: This<Object<'_>>,
        callback_name: String,
        callback: DynFunction<'_>,
    ) -> Result<()> {
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

        if let Type::Callback(cb_type) = ty {
            let prop_key = format!("__slint_cb_{callback_name}");
            crate::set_hidden_property(&mut this.object, &prop_key, &callback)?;

            let owner = self.anchor_owner(env, &this)?;
            let handler = Self::make_callback_handler(
                *env,
                owner,
                prop_key,
                cb_type.return_type.clone(),
                callback_name.clone(),
            );
            self.inner
                .set_callback(callback_name.as_str(), handler)
                .map_err(|_| napi::Error::from_reason("Cannot set callback."))?;

            return Ok(());
        }

        Err(napi::Error::from_reason(format!("{callback_name} is not a callback").as_str()))
    }

    #[napi]
    pub fn set_global_callback(
        &self,
        env: &Env,
        mut this: This<Object<'_>>,
        global_name: String,
        callback_name: String,
        callback: DynFunction<'_>,
    ) -> Result<()> {
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

        if let Type::Callback(cb_type) = ty {
            let prop_key = format!("__slint_gcb_{global_name}_{callback_name}");
            crate::set_hidden_property(&mut this.object, &prop_key, &callback)?;

            let owner = self.anchor_owner(env, &this)?;
            let handler = Self::make_callback_handler(
                *env,
                owner,
                prop_key,
                cb_type.return_type.clone(),
                callback_name.clone(),
            );
            self.inner
                .set_global_callback(global_name.as_str(), callback_name.as_str(), handler)
                .map_err(|_| napi::Error::from_reason("Cannot set callback."))?;

            return Ok(());
        }

        Err(napi::Error::from_reason(format!("{callback_name} is not a callback").as_str()))
    }

    fn invoke_args(
        env: &Env,
        anchor_owner: &JsAnchorOwner,
        callback_name: &String,
        arguments: Vec<Unknown<'_>>,
        args: &[Type],
    ) -> Result<Vec<Value>> {
        let count = args.len();
        let args = arguments
            .into_iter()
            .zip(args)
            .map(|(a, ty)| super::value::to_value(env, a, ty, anchor_owner))
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
        this: This<Object<'_>>,
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

        let owner = self.anchor_owner(env, &this)?;
        let args = match ty {
            Type::Callback(function) | Type::Function(function) => {
                Self::invoke_args(env, &owner, &callback_name, callback_arguments, &function.args)?
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
        this: This<Object<'_>>,
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

        let owner = self.anchor_owner(env, &this)?;
        let args = match ty {
            Type::Callback(function) | Type::Function(function) => {
                Self::invoke_args(env, &owner, &callback_name, callback_arguments, &function.args)?
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
    pub fn send_mouse_click(&self, _x: f64, _y: f64) {
        #[cfg(feature = "testing")]
        {
            let window_adapter = WindowInner::from_pub(self.inner.window()).window_adapter();
            i_slint_backend_testing::testing_backend::send_mouse_click(
                _x as f32,
                _y as f32,
                &window_adapter,
            );
        }
    }

    #[napi]
    pub fn send_keyboard_string_sequence(&self, _sequence: String) {
        #[cfg(feature = "testing")]
        {
            let window_adapter = WindowInner::from_pub(self.inner.window()).window_adapter();
            i_slint_backend_testing::testing_backend::send_keyboard_string_sequence(
                &_sequence.into(),
                &window_adapter,
            );
        }
    }

    #[napi]
    pub fn send_key_combo(&self, keys: Vec<String>) {
        use i_slint_core::platform::WindowEvent;
        let window = self.inner.window();
        for key in &keys {
            window.dispatch_event(WindowEvent::KeyPressed { text: key.into() });
        }
        for key in keys.iter().rev() {
            window.dispatch_event(WindowEvent::KeyReleased { text: key.into() });
        }
    }

    #[napi]
    pub fn window(&self) -> Result<JsWindow> {
        if !self.inner.definition().is_window() {
            return Err(napi::Error::from_reason(
                "this component is not windowed (for example because it inherits from SystemTrayIcon) and has no window",
            ));
        }
        Ok(JsWindow { inner: WindowInner::from_pub(self.inner.window()).window_adapter() })
    }
}

/// A JS function that accepts a dynamic number of arguments.
pub type DynFunction<'a> = Function<'a, crate::DynArgs, Unknown<'static>>;
