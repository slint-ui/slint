/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use core::cell::{Cell, RefCell};
use core::pin::Pin;
use std::rc::{Rc, Weak};

use sixtyfps_corelib::Property;

#[derive(Default, Clone)]
struct ModelPeerInner(Cell<bool>);

impl ModelPeerInner {
    fn notify(&self) {
        self.0.set(true);
    }
}

/// Represent a handle to a view that listen to change to a model. See [`Model::attach_peer`] and [`ModelNotify`]
pub struct ModelPeer {
    inner: Rc<ModelPeerInner>,
}

/// Dispatch notification from a [`Model`] to one or several [`ModelPeer`].
/// Typically, you would want to put this in the implementaiton of the Model
#[derive(Default)]
pub struct ModelNotify {
    inner: RefCell<weak_table::PtrWeakHashSet<Weak<ModelPeerInner>>>,
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
        self.inner.borrow_mut().insert(peer.inner);
    }

    fn notify(&self) {
        for peer in self.inner.borrow().iter() {
            peer.notify()
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

/// This field is put in a component when using the `for` syntax
/// It helps instantiating the components `C`
#[repr(C)]
pub struct Repeater<C: RepeatedComponent> {
    components: RefCell<Vec<Pin<Rc<C>>>>,
    model: Property<ModelHandle<C::Data>>,
    peer: RefCell<Option<ModelPeer>>,
}

impl<C: RepeatedComponent> Default for Repeater<C> {
    fn default() -> Self {
        Repeater {
            components: Default::default(),
            model: Default::default(),
            peer: Default::default(),
        }
    }
}

impl<C: RepeatedComponent> Repeater<C> {
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
            let peer_inner = Rc::new(ModelPeerInner(Cell::new(true)));
            *self.peer.borrow_mut() = Some(ModelPeer { inner: peer_inner.clone() });
            if let Some(m) = model.get() {
                m.attach_peer(ModelPeer { inner: peer_inner });
            }
        }
        if let Some(peer) = self.peer.borrow().as_ref() {
            if peer.inner.0.get() {
                peer.inner.0.set(false);
                self.components.borrow_mut().clear();

                if let Some(model) = model.get() {
                    for i in 0..model.row_count() {
                        let c = init();
                        c.update(i, model.row_data(i));
                        self.components.borrow_mut().push(c);
                    }
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
        window: &sixtyfps_corelib::eventloop::ComponentWindow,
    ) -> sixtyfps_corelib::input::InputEventResult {
        self.components.borrow()[idx].as_ref().input_event(event, window)
    }

    /// Return the amount of item currently in the component
    pub fn len(&self) -> usize {
        self.components.borrow().len()
    }

    /// Borrow the internal vector
    pub fn borrow_item_vec(&self) -> core::cell::Ref<Vec<Pin<Rc<C>>>> {
        self.components.borrow()
    }

    /// Recompute the layout of each chile elements
    pub fn compute_layout(&self) {
        for c in self.components.borrow().iter() {
            c.as_ref().compute_layout();
        }
    }
}

#[test]
fn simple_array_notify_test() {
    let model = VecModel::<u32>::from(vec![1, 2, 3]);
    let model = Rc::new(model);
    let inner = Rc::new(ModelPeerInner(Cell::new(false)));
    model.push(5);
    model.attach_peer(ModelPeer { inner: inner.clone() });
    model.attach_peer(ModelPeer { inner: Rc::new(ModelPeerInner(Cell::new(false))) });
    model.push(6);
    assert_eq!(inner.0.get(), true);
}
