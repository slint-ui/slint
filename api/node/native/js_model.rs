// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

use i_slint_compiler::langtype::Type;
use i_slint_core::model::Model;
use neon::prelude::*;
use std::cell::Cell;
use std::rc::{Rc, Weak};

/// Model coming from JS
pub struct JsModel {
    notify: i_slint_core::model::ModelNotify,
    /// The index of the value in the PersistentContext
    value_index: u32,
    data_type: Type,
}

impl JsModel {
    pub fn new<'cx>(
        obj: Handle<'cx, JsObject>,
        data_type: Type,
        cx: &mut impl Context<'cx>,
        persistent_context: &crate::persistent_context::PersistentContext<'cx>,
    ) -> NeonResult<Rc<Self>> {
        let val = obj.as_value(cx);
        let model = Rc::new(JsModel {
            notify: Default::default(),
            value_index: persistent_context.allocate(cx, val),
            data_type,
        });

        let mut notify = SlintModelNotify::new::<_, JsValue, _>(cx, std::iter::empty())?;
        cx.borrow_mut(&mut notify, |mut notify| notify.0 = Rc::downgrade(&model));
        let notify = notify.as_value(cx);
        obj.set(cx, "notify", notify)?;

        Ok(model)
    }

    pub fn get_object<'cx>(
        &self,
        cx: &mut impl Context<'cx>,
        persistent_context: &crate::persistent_context::PersistentContext<'cx>,
    ) -> JsResult<'cx, JsObject> {
        persistent_context.get(cx, self.value_index)?.downcast_or_throw(cx)
    }
}

impl Model for JsModel {
    type Data = slint_interpreter::Value;

    fn row_count(&self) -> usize {
        let r = Cell::new(0usize);
        crate::run_with_global_context(&|cx, persistent_context| {
            let obj = self.get_object(cx, persistent_context).unwrap();
            let _ = obj
                .get(cx, "rowCount")
                .ok()
                .and_then(|func| func.downcast::<JsFunction>().ok())
                .and_then(|func| func.call(cx, obj, std::iter::empty::<Handle<JsValue>>()).ok())
                .and_then(|res| res.downcast::<JsNumber>().ok())
                .map(|num| r.set(num.value() as _));
        });
        r.get()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        if row >= self.row_count() {
            None
        } else {
            let r = Cell::new(slint_interpreter::Value::default());
            crate::run_with_global_context(&|cx, persistent_context| {
                let row = JsNumber::new(cx, row as f64);
                let obj = self.get_object(cx, persistent_context).unwrap();
                let _ = obj
                    .get(cx, "rowData")
                    .ok()
                    .and_then(|func| func.downcast::<JsFunction>().ok())
                    .and_then(|func| func.call(cx, obj, std::iter::once(row)).ok())
                    .and_then(|res| {
                        crate::to_eval_value(res, self.data_type.clone(), cx, persistent_context)
                            .ok()
                    })
                    .map(|res| r.set(res));
            });
            Some(r.into_inner())
        }
    }

    fn model_tracker(&self) -> &dyn i_slint_core::model::ModelTracker {
        &self.notify
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        crate::run_with_global_context(&|cx, persistent_context| {
            let row = JsNumber::new(cx, row as f64).as_value(cx);
            let data = crate::to_js_value(data.clone(), cx, persistent_context).unwrap();
            let obj = self.get_object(cx, persistent_context).unwrap();
            let _ = obj
                .get(cx, "setRowData")
                .ok()
                .and_then(|func| func.downcast::<JsFunction>().ok())
                .and_then(|func| func.call(cx, obj, [row, data].iter().cloned()).ok());
        });
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

struct WrappedJsModel(Weak<JsModel>);

declare_types! {
    class SlintModelNotify for WrappedJsModel {
        init(_) {
            Ok(WrappedJsModel(Weak::default()))
        }
        method rowDataChanged(mut cx) {
            let this = cx.this();
            let row = cx.argument::<JsNumber>(0)?.value() as usize;
            if let Some(model) = cx.borrow(&this, |x| x.0.upgrade()) {
                model.notify.row_changed(row)
            }
            Ok(JsUndefined::new().as_value(&mut cx))
        }
        method rowAdded(mut cx) {
            let this = cx.this();
            let row = cx.argument::<JsNumber>(0)?.value() as usize;
            let count = cx.argument::<JsNumber>(1)?.value() as usize;
            if let Some(model) = cx.borrow(&this, |x| x.0.upgrade()) {
                model.notify.row_added(row, count)
            }
            Ok(JsUndefined::new().as_value(&mut cx))
        }
        method rowRemoved(mut cx) {
            let this = cx.this();
            let row = cx.argument::<JsNumber>(0)?.value() as usize;
            let count = cx.argument::<JsNumber>(1)?.value() as usize;
            if let Some(model) = cx.borrow(&this, |x| x.0.upgrade()) {
                model.notify.row_removed(row, count)
            }
            Ok(JsUndefined::new().as_value(&mut cx))
        }
        method reset(mut cx) {
            let this = cx.this();
            if let Some(model) = cx.borrow(&this, |x| x.0.upgrade()) {
                model.notify.reset()
            }
            Ok(JsUndefined::new().as_value(&mut cx))
        }

    }

}
