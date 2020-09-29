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
use std::rc::{Rc, Weak};

use sixtyfps_corelib::{component::ComponentRefPin, Property};

type ModelPeerInner = dyn ViewAbstraction;

/// Represent a handle to a view that listen to change to a model. See [`Model::attach_peer`] and [`ModelNotify`]
pub struct ModelPeer {
    inner: Weak<RefCell<ModelPeerInner>>,
}

/// Dispatch notification from a [`Model`] to one or several [`ModelPeer`].
/// Typically, you would want to put this in the implementaiton of the Model
#[derive(Default)]
pub struct ModelNotify {
    inner: RefCell<weak_table::PtrWeakHashSet<Weak<RefCell<ModelPeerInner>>>>,
}

impl ModelNotify {
    /// Notify the peers that a specific row was changed
    pub fn row_changed(&self, _row: usize) {
        self.notify()
    }
    /// Notify the peers that rows were added
    pub fn row_added(&self, _index: usize, _count: usize) {
        self.notify()
    }
    /// Notify the peers that rows were removed
    pub fn row_removed(&self, _index: usize, _count: usize) {
        self.notify()
    }
    /// Attach one peer. The peer will be notified when the model changes
    pub fn attach(&self, peer: ModelPeer) {
        peer.inner.upgrade().map(|rc| self.inner.borrow_mut().insert(rc));
    }

    fn notify(&self) {
        for peer in self.inner.borrow().iter() {
            peer.borrow_mut().notify()
        }
    }
}

/// A Model is providing Data for the Repeater or ListView elements of the `.60` language
pub trait Model {
    /// The model data: A model is a set of row and each row has this data
    type Data;
    /// The amount of row in the model
    fn row_count(&self) -> usize;
    /// Returns the data for a particular row. This function should be called with `row < row_count()`.
    fn row_data(&self, row: usize) -> Self::Data;
    /// Sets the data for a particular row. This function should be called with `row < row_count()`.
    /// If the model cannot support data changes, then it is ok to do nothing (default implementation).
    /// If the model can update the data, it should also call row_changed on its internal `ModelNotify`.
    fn set_row_data(&self, _row: usize, _data: Self::Data) {}
    /// Should forward to the internal [`ModelNotify::attach`]
    fn attach_peer(&self, peer: ModelPeer);
}

/// A model backed by an SharedArray
#[derive(Default)]
pub struct VecModel<T> {
    array: RefCell<Vec<T>>,
    notify: ModelNotify,
}

impl<T: 'static> VecModel<T> {
    /// Allocate a new model from a slice
    pub fn from_slice(slice: &[T]) -> ModelHandle<T>
    where
        T: Clone,
    {
        Some(Rc::<Self>::new(slice.iter().cloned().collect::<Vec<T>>().into()))
    }

    /// Add a row at the end of the model
    pub fn push(&self, value: T) {
        self.array.borrow_mut().push(value);
        self.notify.row_added(self.array.borrow().len() - 1, 1)
    }
}

impl<T> From<Vec<T>> for VecModel<T> {
    fn from(array: Vec<T>) -> Self {
        VecModel { array: RefCell::new(array), notify: Default::default() }
    }
}

impl<T: Clone> Model for VecModel<T> {
    type Data = T;

    fn row_count(&self) -> usize {
        self.array.borrow().len()
    }

    fn row_data(&self, row: usize) -> Self::Data {
        self.array.borrow()[row].clone()
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        self.array.borrow_mut()[row] = data;
        self.notify.row_changed(row);
    }

    fn attach_peer(&self, peer: ModelPeer) {
        self.notify.attach(peer);
    }
}

impl Model for usize {
    type Data = i32;

    fn row_count(&self) -> usize {
        *self
    }

    fn row_data(&self, row: usize) -> Self::Data {
        row as i32
    }

    fn attach_peer(&self, _peer: ModelPeer) {
        // The model is read_only: nothing to do
    }
}

impl Model for bool {
    type Data = ();

    fn row_count(&self) -> usize {
        if *self {
            1
        } else {
            0
        }
    }

    fn row_data(&self, _row: usize) -> Self::Data {}

    fn attach_peer(&self, _peer: ModelPeer) {
        // The model is read_only: nothing to do
    }
}

/// Properties of type array in the .60 language are represented as
/// an [Option] of an [Rc] of somthing implemented the [Model] trait
pub type ModelHandle<T> = Option<Rc<dyn Model<Data = T>>>;

/// Component that can be instantiated by a repeater.
pub trait RepeatedComponent: sixtyfps_corelib::component::Component {
    /// The data corresponding to the model
    type Data: 'static;

    /// Update this component at the given index and the given data
    fn update(&self, index: usize, data: Self::Data);
}

trait ViewAbstraction {
    fn notify(&mut self);
}

struct RepeaterInner<C: RepeatedComponent> {
    components: Vec<Pin<Rc<C>>>,
    is_dirty: bool,
}

impl<C: RepeatedComponent> Default for RepeaterInner<C> {
    fn default() -> Self {
        RepeaterInner { components: Default::default(), is_dirty: Default::default() }
    }
}

impl<C: RepeatedComponent> Clone for RepeaterInner<C> {
    fn clone(&self) -> Self {
        panic!("Clone is there so we can make_mut the RepeaterInner, to dissociate the weaks, but there should only be one inner")
    }
}

impl<C: RepeatedComponent> ViewAbstraction for RepeaterInner<C> {
    fn notify(&mut self) {
        self.is_dirty = true
    }
}

/// This field is put in a component when using the `for` syntax
/// It helps instantiating the components `C`
pub struct Repeater<C: RepeatedComponent> {
    /// The Rc is shared between ModelPeer. The outer RefCell make it possible to re-initialize a new Rc when
    /// The model is changed. The inner RefCell make it possible to change the RepeaterInner when shared
    inner: RefCell<Rc<RefCell<RepeaterInner<C>>>>,
    model: Property<ModelHandle<C::Data>>,
}

impl<C: RepeatedComponent> Default for Repeater<C> {
    fn default() -> Self {
        Repeater { inner: Default::default(), model: Default::default() }
    }
}

impl<C: RepeatedComponent + 'static> Repeater<C> {
    /// Set the model binding
    pub fn set_model_binding(&self, binding: impl Fn() -> ModelHandle<C::Data> + 'static) {
        self.model.set_binding(binding);
    }

    /// Call this function to make sure that the model is updated.
    /// The init function is the function to create a component
    pub fn ensure_updated(self: Pin<&Self>, init: impl Fn() -> Pin<Rc<C>>) {
        #[allow(unsafe_code)]
        // Safety: Repeater does not implement drop and never let access model as mutable
        let model = unsafe { self.map_unchecked(|s| &s.model) };
        if model.is_dirty() {
            // Invalidate previuos weeks on the previous models
            Rc::make_mut(&mut self.inner.borrow_mut()).get_mut().is_dirty = true;
            if let Some(m) = model.get() {
                let peer: Rc<RefCell<dyn ViewAbstraction>> = self.inner.borrow().clone();
                m.attach_peer(ModelPeer { inner: Rc::downgrade(&peer) });
            }
        }
        let inner = self.inner.borrow();
        let mut inner = inner.borrow_mut();
        if inner.is_dirty {
            inner.is_dirty = false;
            inner.components.clear();

            if let Some(model) = model.get() {
                for i in 0..model.row_count() {
                    let c = init();
                    c.update(i, model.row_data(i));
                    inner.components.push(c);
                }
            }
        }
    }

    /// Call the visitor for each component
    pub fn visit(
        &self,
        order: sixtyfps_corelib::item_tree::TraversalOrder,
        mut visitor: sixtyfps_corelib::item_tree::ItemVisitorRefMut,
    ) -> sixtyfps_corelib::item_tree::VisitChildrenResult {
        for (i, c) in self.inner.borrow().borrow().components.iter().enumerate() {
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
        window: &sixtyfps_corelib::eventloop::ComponentWindow,
        app_component: &ComponentRefPin,
    ) -> sixtyfps_corelib::input::InputEventResult {
        self.inner.borrow().borrow().components[idx].as_ref().input_event(
            event,
            window,
            app_component,
        )
    }

    /// Forward a key event to a particular item
    pub fn key_event(
        &self,
        idx: usize,
        event: &sixtyfps_corelib::input::KeyEvent,
        window: &sixtyfps_corelib::eventloop::ComponentWindow,
    ) -> sixtyfps_corelib::input::KeyEventResult {
        self.inner.borrow().borrow().components[idx].as_ref().key_event(event, window)
    }

    /// Forward a focus event to a particular item
    pub fn focus_event(
        &self,
        idx: usize,
        event: &sixtyfps_corelib::input::FocusEvent,
        window: &sixtyfps_corelib::eventloop::ComponentWindow,
    ) -> sixtyfps_corelib::input::FocusEventResult {
        self.inner.borrow().borrow().components[idx].as_ref().focus_event(event, window)
    }

    /// Return the amount of item currently in the component
    pub fn len(&self) -> usize {
        self.inner.borrow().borrow().components.len()
    }

    /// Returns a vector containing all components
    pub fn components_vec(&self) -> Vec<Pin<Rc<C>>> {
        self.inner.borrow().borrow().components.clone()
    }

    /// Recompute the layout of each child elements
    pub fn compute_layout(&self) {
        for c in self.inner.borrow().borrow().components.iter() {
            c.as_ref().compute_layout();
        }
    }
}
