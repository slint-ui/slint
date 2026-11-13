// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;
use crate::SharedString;
use crate::accessibility::AccessibleStringProperty;
use crate::cursor::MouseCursorInner;
use crate::input::{
    FocusEvent, FocusEventResult, InputEventFilterResult, InputEventResult, InternalKeyEvent,
    KeyEventResult, MouseEvent,
};
use crate::item_tree::{IndexRange, ItemTree, ItemTreeNode, ItemTreeWeak, ItemWeak};
use crate::items::{AccessibleRole, ItemRc, RenderingResult};
use crate::layout::LayoutInfo;
use crate::lengths::{LogicalRect, LogicalSize};
use crate::model::{ModelNotify, ModelTracker, VecModel};
use crate::slice::Slice;
use crate::window::{WindowAdapter, WindowAdapterRc};
use alloc::boxed::Box;
use alloc::rc::Rc;
use std::vec;
use vtable::VRc;
mod simple_item;
use simple_item::SimpleItem;

mod listview;
mod repeater_ops; // repeater ops tests // listview tests

type ItemRendererRef<'a> = &'a mut dyn crate::item_rendering::ItemRenderer;

fn ensure_updated_window(repeater: Pin<&Repeater<SimpleItem>>) {
    let current = repeater.0.inner.borrow().layout_state.item_index.clone();
    let model = repeater.0.model.get_internal();
    let mut ops = RustRepeaterOps {
        inner: &repeater.0.inner,
        typed: &RepeaterTypedOps::<SimpleItem> { init: &new_item, model: &model },
        instance_generation: repeater.as_ref().data().project_ref().instance_generation,
    };
    for instance_index in 0..ops.len() {
        ops.ensure_updated(instance_index, current.row - current.instance_index + instance_index);
    }
}

// Get value stored in SimpleItem
fn get_instance_inner_value(inner: &core::cell::Ref<'_, RepeaterInner>, index: usize) -> i32 {
    inner.instances[index].1.as_ref().map(as_typed::<SimpleItem>).unwrap().value.get()
}

const ITEM_HEIGHT: f32 = 100.;

/// Unlike `VecModel`, inserts and removes several rows with a single notification.
struct TestModel {
    rows: RefCell<Vec<i32>>,
    notify: ModelNotify,
}

impl TestModel {
    /// Rows `0..count`, each holding its own index
    fn new(count: usize) -> Rc<Self> {
        Rc::new(Self {
            rows: RefCell::new((0..count as i32).collect()),
            notify: Default::default(),
        })
    }

    fn insert(&self, index: usize, values: &[i32]) {
        self.rows.borrow_mut().splice(index..index, values.iter().copied());
        self.notify.row_added(index, values.len());
    }

    fn remove(&self, rows: core::ops::Range<usize>) {
        self.rows.borrow_mut().drain(rows.clone());
        self.notify.row_removed(rows.start, rows.len());
    }
}

impl Model for TestModel {
    type Data = i32;

    fn row_count(&self) -> usize {
        self.rows.borrow().len()
    }

    fn row_data(&self, row: usize) -> Option<i32> {
        self.rows.borrow().get(row).copied()
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }
}

#[derive(Debug, PartialEq)]
enum Slot {
    Clean(i32),
    /// Marked dirty, still showing the data of its last update
    Dirty,
    /// Marked dirty, without an instance yet
    Empty,
}
use Slot::{Clean, Dirty, Empty};

/// The instance slots and the current item
fn state(repeater: &Repeater<SimpleItem>) -> (Vec<Slot>, ItemIndexRelationShip) {
    let inner = repeater.0.inner.borrow();
    let slots = inner
        .instances
        .iter()
        .map(|(state, instance)| match (state, instance) {
            (RepeatedInstanceState::Clean, Some(instance)) => {
                Clean(as_typed::<SimpleItem>(instance).value.get())
            }
            (RepeatedInstanceState::Dirty, Some(_)) => Dirty,
            (RepeatedInstanceState::Dirty, None) => Empty,
            (RepeatedInstanceState::Clean, None) => panic!("clean slot without an instance"),
        })
        .collect();
    (slots, inner.layout_state.item_index.clone())
}

fn new_item() -> ItemTreeRc<SimpleItem> {
    SimpleItem::new(-1)
}

/// A repeater with clean instances for the rows in `window` only, as a ListView leaves it
fn repeater_with_window(
    model: &Rc<TestModel>,
    window: core::ops::Range<usize>,
    current_row: usize,
) -> Pin<Box<Repeater<SimpleItem>>> {
    let repeater = Box::pin(Repeater::default());
    let model_rc = ModelRc::from(model.clone());
    repeater.set_model_binding(move || model_rc.clone());
    repeater.as_ref().model(); // Setup tracker
    let mut inner = repeater.0.inner.borrow_mut();
    inner.instances = window
        .clone()
        .map(|row| {
            let instance = SimpleItem::new(model.row_data(row).unwrap());
            (RepeatedInstanceState::Clean, Some(vtable::VRc::into_dyn(instance)))
        })
        .collect();
    inner.layout_state.item_index =
        ItemIndexRelationShip { row: current_row, instance_index: current_row - window.start };
    drop(inner);
    repeater.0.is_dirty.set(false);
    repeater
}

/// A repeater outside a ListView, with instances for all rows
fn repeater_for(model: &Rc<VecModel<i32>>) -> Pin<Box<Repeater<SimpleItem>>> {
    let repeater = Box::pin(Repeater::default());
    let model = ModelRc::from(model.clone());
    repeater.set_model_binding(move || model.clone());
    repeater.as_ref().ensure_updated(new_item);
    repeater
}

fn with_ops(repeater: Pin<&Repeater<SimpleItem>>, f: impl FnOnce(&mut RustRepeaterOps<'_>)) {
    let model = ModelRc::default();
    f(&mut RustRepeaterOps {
        inner: &repeater.0.inner,
        typed: &RepeaterTypedOps::<SimpleItem> { init: &new_item, model: &model },
        instance_generation: repeater.as_ref().data().project_ref().instance_generation,
    });
}
