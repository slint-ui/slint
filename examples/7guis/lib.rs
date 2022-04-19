pub mod wrapper {
    use slint::{Model, ModelNotify};
    use std::{
        cell::{Ref, RefCell},
        ops::{Index, IndexMut},
    };

    pub enum Change {
        Nothing,
        RowModified(usize),
        RowAppended(usize),
        RowRemoved(usize),
    }

    pub trait WrapperBuilder
    where
        Self: IndexMut<usize, Output = Self::WrappedData>,
    {
        type WrappedData;
        type Data;

        fn get_row(&mut self, index: usize) -> &Self::Data;

        fn row_len(&self) -> usize;

        fn map<T, F>(self, f: F) -> WrapperMap<Self, F, T>
        where
            Self: Sized,
            F: Fn(&Self::Data) -> T,
            T: Clone,
        {
            WrapperMap::new(self, f)
        }

        fn filter<F>(self, f: F) -> WrapperFilter<Self, F>
        where
            Self: Sized,
            F: Fn(&Self::Data) -> bool,
        {
            WrapperFilter::new(self, f)
        }

        fn apply_filters(&mut self);

        fn build(self) -> WrapperModel<Self>
        where
            Self: Sized,
        {
            WrapperModel::new(self)
        }

        fn len(&self) -> usize;

        fn get(&self, index: usize) -> Option<&Self::WrappedData>;
        fn get_mut(&mut self, index: usize) -> Option<&mut Self::WrappedData>;

        fn element_changed(&self, index: usize) -> Change;

        fn push(&mut self, element: Self::WrappedData) -> Change;

        fn remove(&mut self, index: usize) -> Change;

        fn index_for_row(&self, row: usize) -> usize;
    }

    pub struct WrapperModel<I> {
        builder: RefCell<I>,
        notify: ModelNotify,
    }

    pub struct RefNotify<'a, I>
    where
        I: WrapperBuilder,
    {
        model: &'a WrapperModel<I>,
        index: usize,
    }

    impl<'a, I> RefNotify<'a, I>
    where
        I: WrapperBuilder,
    {
        pub fn set(&self, value: I::WrappedData) {
            let c;
            {
                let mut builder = self.model.builder.borrow_mut();
                builder[self.index] = value;
                c = builder.element_changed(self.index);
            }
            match c {
                Change::RowModified(index) => self.model.notify.row_changed(index),
                _ => (),
            }
        }
    }

    impl<I> WrapperModel<I>
    where
        I: WrapperBuilder,
    {
        fn new(builder: I) -> WrapperModel<I> {
            Self { builder: RefCell::new(builder), notify: Default::default() }
        }

        pub fn apply_filters(&self) {
            let old_size;
            let new_size;
            {
                let mut b = self.builder.borrow_mut();
                old_size = b.row_len();
                b.apply_filters();
                new_size = b.row_len();
            }
            self.notify.row_removed(0, old_size);
            self.notify.row_added(0, new_size);
        }

        pub fn get(&self, index: usize) -> Ref<I::WrappedData> {
            Ref::map(self.builder.borrow(), |b| b.index(index))
        }

        pub fn get_mut(&self, index: usize) -> RefNotify<I> {
            RefNotify { model: &self, index }
        }

        pub fn push(&self, element: I::WrappedData) {
            match self.builder.borrow_mut().push(element) {
                Change::RowAppended(index) => self.notify.row_added(index, 1),
                _ => (),
            }
        }

        pub fn remove(&self, index: usize) {
            match self.builder.borrow_mut().remove(index) {
                Change::RowRemoved(index) => self.notify.row_removed(index, 1),
                _ => (),
            }
        }

        pub fn index_for_row(&self, row: usize) -> usize {
            self.builder.borrow().index_for_row(row)
        }
    }

    impl<I> Model for WrapperModel<I>
    where
        I: WrapperBuilder,
        I::Data: Clone,
    {
        type Data = I::Data;
        fn row_count(&self) -> usize {
            self.builder.borrow().row_len()
        }

        fn row_data(&self, index: usize) -> Option<<Self as slint::Model>::Data> {
            // TODO: check if in bounds
            Some(self.builder.borrow_mut().get_row(index).clone())
        }
        fn model_tracker(&self) -> &dyn slint::ModelTracker {
            &self.notify
        }
    }

    pub struct VecWrapper<T> {
        array: Vec<T>,
    }

    impl<T> VecWrapper<T> {
        pub fn new() -> Self {
            Self { array: Vec::new() }
        }
    }

    impl<T> From<Vec<T>> for VecWrapper<T> {
        fn from(array: Vec<T>) -> Self {
            Self { array }
        }
    }

    impl<T> Index<usize> for VecWrapper<T> {
        type Output = T;

        fn index(&self, index: usize) -> &Self::Output {
            self.array.index(index)
        }
    }

    impl<T> IndexMut<usize> for VecWrapper<T> {
        fn index_mut(&mut self, index: usize) -> &mut Self::Output {
            self.array.index_mut(index)
        }
    }

    impl<T> WrapperBuilder for VecWrapper<T> {
        type Data = T;
        type WrappedData = T;

        fn get_row(&mut self, index: usize) -> &T {
            self.array.index(index)
        }

        fn row_len(&self) -> usize {
            self.array.len()
        }
        fn apply_filters(&mut self) {}

        fn len(&self) -> usize {
            self.array.len()
        }

        fn get(&self, index: usize) -> Option<&Self::WrappedData> {
            self.array.get(index)
        }

        fn get_mut(&mut self, index: usize) -> Option<&mut Self::WrappedData> {
            self.array.get_mut(index)
        }

        fn element_changed(&self, index: usize) -> Change {
            Change::RowModified(index)
        }

        fn push(&mut self, element: Self::WrappedData) -> Change {
            self.array.push(element);
            Change::RowAppended(self.array.len() - 1)
        }

        fn index_for_row(&self, row: usize) -> usize {
            row
        }

        fn remove(&mut self, index: usize) -> Change {
            self.array.remove(index);
            Change::RowRemoved(index)
        }
    }

    pub struct WrapperMap<I, F, T> {
        inner: I,
        current_mapped: Option<T>,
        f: F,
    }

    impl<I, F, T> WrapperMap<I, F, T>
    where
        I: WrapperBuilder,
        T: Clone,
    {
        fn new(inner: I, f: F) -> Self {
            Self { inner, current_mapped: None, f }
        }
    }

    impl<I, F, T> Index<usize> for WrapperMap<I, F, T>
    where
        I: WrapperBuilder,
    {
        type Output = I::WrappedData;

        fn index(&self, index: usize) -> &Self::Output {
            self.inner.index(index)
        }
    }

    impl<I, F, T> IndexMut<usize> for WrapperMap<I, F, T>
    where
        I: WrapperBuilder,
    {
        fn index_mut(&mut self, index: usize) -> &mut Self::Output {
            self.inner.index_mut(index)
        }
    }

    impl<I, F, T> WrapperBuilder for WrapperMap<I, F, T>
    where
        I: WrapperBuilder,
        F: Fn(&I::Data) -> T,
    {
        type Data = T;
        type WrappedData = I::WrappedData;
        fn get_row(&mut self, index: usize) -> &Self::Data {
            let inner_row = self.inner.get_row(index);
            self.current_mapped = Some((self.f)(inner_row));
            self.current_mapped.as_ref().unwrap()
        }
        fn row_len(&self) -> usize {
            self.inner.row_len()
        }
        fn apply_filters(&mut self) {
            self.inner.apply_filters();
        }

        fn len(&self) -> usize {
            self.inner.len()
        }

        fn get(&self, index: usize) -> Option<&Self::WrappedData> {
            self.inner.get(index)
        }

        fn get_mut(&mut self, index: usize) -> Option<&mut Self::WrappedData> {
            self.inner.get_mut(index)
        }

        fn element_changed(&self, index: usize) -> Change {
            self.inner.element_changed(index)
        }

        fn push(&mut self, element: Self::WrappedData) -> Change {
            self.inner.push(element)
        }

        fn index_for_row(&self, row: usize) -> usize {
            self.inner.index_for_row(row)
        }

        fn remove(&mut self, index: usize) -> Change {
            self.inner.remove(index)
        }
    }

    pub struct WrapperFilter<I, F> {
        inner: I,
        filtered: Vec<usize>,
        f: F,
    }

    impl<I, F> WrapperFilter<I, F>
    where
        I: WrapperBuilder,
        F: Fn(&I::Data) -> bool,
    {
        fn new(inner: I, f: F) -> Self {
            let mut s = Self { inner, filtered: vec![], f };
            s.filter_elements();
            s
        }

        fn filter_elements(&mut self) {
            self.filtered.clear();

            for i in 0..self.inner.row_len() {
                if (self.f)(self.inner.get_row(i)) {
                    self.filtered.push(i);
                }
            }
        }
    }

    impl<I, F> Index<usize> for WrapperFilter<I, F>
    where
        I: WrapperBuilder,
    {
        type Output = I::WrappedData;

        fn index(&self, index: usize) -> &Self::Output {
            self.inner.index(index)
        }
    }

    impl<I, F> IndexMut<usize> for WrapperFilter<I, F>
    where
        I: WrapperBuilder,
    {
        fn index_mut(&mut self, index: usize) -> &mut Self::Output {
            self.inner.index_mut(index)
        }
    }

    impl<I, F> WrapperBuilder for WrapperFilter<I, F>
    where
        I: WrapperBuilder,
        F: Fn(&I::Data) -> bool,
    {
        type Data = I::Data;
        type WrappedData = I::WrappedData;

        fn get_row(&mut self, index: usize) -> &Self::Data {
            self.inner.get_row(self.filtered[index])
        }

        fn row_len(&self) -> usize {
            self.filtered.len()
        }

        fn apply_filters(&mut self) {
            self.inner.apply_filters();
            self.filter_elements();
        }

        fn len(&self) -> usize {
            self.inner.len()
        }

        fn get(&self, index: usize) -> Option<&Self::WrappedData> {
            self.inner.get(index)
        }

        fn get_mut(&mut self, index: usize) -> Option<&mut Self::WrappedData> {
            self.inner.get_mut(index)
        }

        fn element_changed(&self, index: usize) -> Change {
            // TODO: filtered out after change?
            match self.inner.element_changed(index) {
                Change::RowModified(index) => {
                    let mut filtered_index = 0;
                    for (outer_index, inner_index) in self.filtered.iter().enumerate() {
                        if *inner_index == index {
                            filtered_index = outer_index;
                            break;
                        }
                    }
                    Change::RowModified(filtered_index)
                }
                _ => Change::Nothing,
            }
        }

        fn push(&mut self, element: Self::WrappedData) -> Change {
            match self.inner.push(element) {
                Change::RowAppended(index) => {
                    if (self.f)(self.inner.get_row(index)) {
                        self.filtered.push(index);
                        Change::RowAppended(self.filtered.len() - 1)
                    } else {
                        Change::Nothing
                    }
                }
                _ => Change::Nothing,
            }
        }

        fn index_for_row(&self, row: usize) -> usize {
            self.inner.index_for_row(self.filtered[row])
        }

        fn remove(&mut self, index: usize) -> Change {
            match self.inner.remove(index) {
                Change::RowRemoved(index) => {
                    let mut to_be_removed = None;
                    for (outer_index, inner_index) in self.filtered.iter_mut().enumerate() {
                        if *inner_index > index {
                            *inner_index -= 1;
                        } else if *inner_index == index {
                            to_be_removed = Some(outer_index);
                        }
                    }
                    match to_be_removed {
                        Some(to_be_removed) => {
                            self.filtered.remove(to_be_removed);
                            Change::RowRemoved(to_be_removed)
                        }
                        None => Change::Nothing,
                    }
                }
                _ => Change::Nothing,
            }
        }
    }
}
