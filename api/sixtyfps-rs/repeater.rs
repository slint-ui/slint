use core::cell::RefCell;
use core::pin::Pin;
use std::rc::Rc;

/// Component that can be instantiated by a repeater.
pub trait RepeatedComponent: sixtyfps_corelib::abi::datastructures::Component {
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
    pub fn visit(&self, mut visitor: sixtyfps_corelib::abi::datastructures::ItemVisitorRefMut) {
        for c in self.components.borrow().iter() {
            c.as_ref().visit_children_item(-1, visitor.borrow_mut());
        }
    }
}
