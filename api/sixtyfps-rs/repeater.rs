/// Component that can be instantiated by a repeater.
pub trait RepeatedComponent: sixtyfps_corelib::abi::datastructures::Component + Default {
    /// The data corresponding to the model
    type Data;

    /// Update this component at the given index and the given data
    fn update(&self, index: usize, data: Self::Data);
}

/// This field is put in a component when using the `for` syntax
/// It helps instantiating the components `C`
#[derive(Default)]
pub struct Repeater<C> {
    components: Vec<core::pin::Pin<Box<C>>>,
}

impl<Data, C> Repeater<C>
where
    C: RepeatedComponent<Data = Data>,
{
    /// Called when the model is changed
    pub fn update_model<'a>(&mut self, data: impl Iterator<Item = Data>)
    where
        Data: 'a,
    {
        self.components.clear();
        for (i, d) in data.enumerate() {
            let c = C::default();
            c.update(i, d);
            self.components.push(Box::pin(c));
        }
    }

    /// Call the visitor for each component
    pub fn visit(&self, mut visitor: sixtyfps_corelib::abi::datastructures::ItemVisitorRefMut) {
        for c in &self.components {
            c.as_ref().visit_children_item(-1, visitor.borrow_mut());
        }
    }
}
