/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use core::cell::RefCell;
use core::pin::Pin;
use std::rc::Rc;

/// Component that can be instantiated by a repeater.
pub trait RepeatedComponent: sixtyfps_corelib::component::Component {
    /// The data corresponding to the model
    type Data;

    /// Update this component at the given index and the given data
    fn update(&self, index: usize, data: Self::Data);
}

/// This field is put in a component when using the `for` syntax
/// It helps instantiating the components `C`
pub struct Repeater<C> {
    components: RefCell<Vec<Pin<Rc<C>>>>,
}

impl<C> Default for Repeater<C> {
    fn default() -> Self {
        Repeater { components: Default::default() }
    }
}

impl<Data, C> Repeater<C>
where
    C: RepeatedComponent<Data = Data>,
{
    /// Called when the model is changed
    pub fn update_model<'a>(&self, data: impl Iterator<Item = Data>, init: impl Fn() -> Pin<Rc<C>>)
    where
        Data: 'a,
    {
        self.components.borrow_mut().clear();
        for (i, d) in data.enumerate() {
            let c = init();
            c.update(i, d);
            self.components.borrow_mut().push(c);
        }
    }

    /// Call the visitor for each component
    pub fn visit(
        &self,
        order: sixtyfps_corelib::item_tree::TraversalOrder,
        mut visitor: sixtyfps_corelib::item_tree::ItemVisitorRefMut,
    ) -> sixtyfps_corelib::item_tree::VisitChildrenResult {
        for (i, c) in self.components.borrow().iter().enumerate() {
            if c.as_ref().visit_children_item(-1, order, visitor.borrow_mut()).has_aborted() {
                return sixtyfps_corelib::item_tree::VisitChildrenResult::abort(i, 0);
            }
        }
        sixtyfps_corelib::item_tree::VisitChildrenResult::CONTINUE
    }

    /// Forward an input event to a particular item
    pub fn input_event(
        &self,
        idx: usize,
        event: sixtyfps_corelib::input::MouseEvent,
    ) -> sixtyfps_corelib::input::InputEventResult {
        self.components.borrow()[idx].as_ref().input_event(event)
    }

    /// Return the amount of item currently in the component
    pub fn len(&self) -> usize {
        self.components.borrow().len()
    }

    /// Borrow the internal vector
    pub fn borrow_item_vec(&self) -> core::cell::Ref<Vec<Pin<Rc<C>>>> {
        self.components.borrow()
    }
}
