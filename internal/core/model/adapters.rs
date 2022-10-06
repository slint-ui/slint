// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! This module contains adapter models.

use core::{borrow::Borrow, marker::PhantomData};

use super::*;

/// Provides rows that are generated by a map function based on the rows of another Model
///
/// When the other Model is updated, the `MapModel` is updated accordingly.
///
/// ## Example
///
/// Here we have a [`VecModel`] holding rows of a custom type `Name`.
/// It is then mapped into a `MapModel` of [`SharedString`]s
///
/// ```
/// # use slint::{Model, VecModel, SharedString, MapModel};
/// #[derive(Clone)]
/// struct Name {
///     first: String,
///     last: String,
/// }
///
/// let model = VecModel::from(vec![
///     Name { first: "Hans".to_string(), last: "Emil".to_string() },
///     Name { first: "Max".to_string(), last: "Mustermann".to_string() },
///     Name { first: "Roman".to_string(), last: "Tisch".to_string() },
/// ]);
///
/// let mapped_model = MapModel::new(model, |n|
///     slint::format!("{}, {}", n.last, n.first)
/// );
///
/// assert_eq!(mapped_model.row_data(0).unwrap(), SharedString::from("Emil, Hans"));
/// assert_eq!(mapped_model.row_data(1).unwrap(), SharedString::from("Mustermann, Max"));
/// assert_eq!(mapped_model.row_data(2).unwrap(), SharedString::from("Tisch, Roman"));
///
/// ```
///
/// Alternatively you can use the shortcut [`ModelExt::map`].
/// ```
/// # use slint::{Model, ModelExt, VecModel, SharedString, MapModel};
/// # #[derive(Clone)]
/// # struct Name {
/// #     first: String,
/// #     last: String,
/// # }
/// let mapped_model = VecModel::from(vec![
///     Name { first: "Hans".to_string(), last: "Emil".to_string() },
///     Name { first: "Max".to_string(), last: "Mustermann".to_string() },
///     Name { first: "Roman".to_string(), last: "Tisch".to_string() },
/// ])
/// .map(|n| slint::format!("{}, {}", n.last, n.first));
/// # assert_eq!(mapped_model.row_data(0).unwrap(), SharedString::from("Emil, Hans"));
/// # assert_eq!(mapped_model.row_data(1).unwrap(), SharedString::from("Mustermann, Max"));
/// # assert_eq!(mapped_model.row_data(2).unwrap(), SharedString::from("Tisch, Roman"));
/// ```
///
/// If you want to modify the underlying [`VecModel`] you can give it a [`Rc`] of the MapModel:
/// ```
/// # use std::rc::Rc;
/// # use slint::{Model, VecModel, SharedString, MapModel};
/// # #[derive(Clone)]
/// # struct Name {
/// #     first: String,
/// #     last: String,
/// # }
/// let model = Rc::new(VecModel::from(vec![
///     Name { first: "Hans".to_string(), last: "Emil".to_string() },
///     Name { first: "Max".to_string(), last: "Mustermann".to_string() },
///     Name { first: "Roman".to_string(), last: "Tisch".to_string() },
/// ]));
///
/// let mapped_model = MapModel::new(model.clone(), |n|
///     slint::format!("{}, {}", n.last, n.first)
/// );
///
/// model.set_row_data(1, Name { first: "Minnie".to_string(), last: "Musterfrau".to_string() });
///
/// assert_eq!(mapped_model.row_data(0).unwrap(), SharedString::from("Emil, Hans"));
/// assert_eq!(mapped_model.row_data(1).unwrap(), SharedString::from("Musterfrau, Minnie"));
/// assert_eq!(mapped_model.row_data(2).unwrap(), SharedString::from("Tisch, Roman"));
///
/// ```
pub struct MapModel<M, F> {
    wrapped_model: M,
    map_function: F,
}

impl<M, F, T, U> Model for MapModel<M, F>
where
    M: 'static,
    F: 'static,
    F: Fn(T) -> U,
    M: Model<Data = T>,
{
    type Data = U;

    fn row_count(&self) -> usize {
        self.wrapped_model.row_count()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        self.wrapped_model.row_data(row).map(|x| (self.map_function)(x))
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        self.wrapped_model.model_tracker()
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

impl<M, F, T, U> MapModel<M, F>
where
    M: 'static,
    F: 'static,
    F: Fn(T) -> U,
    M: Model<Data = T>,
{
    pub fn new(model: M, map_function: F) -> Self {
        Self { wrapped_model: model, map_function }
    }
}

#[test]
fn test_map_model() {
    let wrapped_rc = Rc::new(VecModel::from(vec![1, 2, 3]));
    let map = MapModel::new(wrapped_rc.clone(), |x| x.to_string());

    wrapped_rc.set_row_data(2, 42);
    wrapped_rc.push(4);

    assert_eq!(map.row_data(2).unwrap(), "42");
    assert_eq!(map.row_data(3).unwrap(), "4");
    assert_eq!(map.row_data(1).unwrap(), "2");
}

struct FilterModelInner<M, F>
where
    M: Model + 'static,
    F: Fn(&M::Data) -> bool + 'static,
{
    wrapped_model: M,
    filter_function: F,
    // This vector saves the indices of the elements that are not filtered out
    mapping: RefCell<Vec<usize>>,
    notify: ModelNotify,
}

impl<M, F> FilterModelInner<M, F>
where
    M: Model + 'static,
    F: Fn(&M::Data) -> bool + 'static,
{
    fn build_mapping_vec(&self) {
        let mut mapping = self.mapping.borrow_mut();
        *mapping = self
            .wrapped_model
            .iter()
            .enumerate()
            .filter_map(|(i, e)| (self.filter_function)(&e).then(|| i))
            .collect();
    }
}

impl<M, F> ModelChangeListener for FilterModelInner<M, F>
where
    M: Model + 'static,
    F: Fn(&M::Data) -> bool + 'static,
{
    fn row_changed(&self, row: usize) {
        let mut mapping = self.mapping.borrow_mut();

        let (index, is_contained) = match mapping.binary_search(&row) {
            Ok(index) => (index, true),
            Err(index) => (index, false),
        };

        let should_be_contained =
            (self.filter_function)(&self.wrapped_model.row_data(row).unwrap());

        if is_contained && should_be_contained {
            drop(mapping);
            self.notify.row_changed(index);
        } else if !is_contained && should_be_contained {
            mapping.insert(index, row);
            drop(mapping);
            self.notify.row_added(index, 1);
        } else if is_contained && !should_be_contained {
            mapping.remove(index);
            drop(mapping);
            self.notify.row_removed(index, 1);
        }
    }

    fn row_added(&self, index: usize, count: usize) {
        if count == 0 {
            return;
        }

        let insertion: Vec<usize> = self
            .wrapped_model
            .iter()
            .enumerate()
            .skip(index)
            .take(count)
            .filter_map(|(i, e)| (self.filter_function)(&e).then(|| i))
            .collect();

        if !insertion.is_empty() {
            let mut mapping = self.mapping.borrow_mut();
            let insertion_point = mapping.binary_search(&index).unwrap_or_else(|ip| ip);

            let old_mapping_len = mapping.len();
            mapping.resize(old_mapping_len + insertion.len(), 0);
            mapping
                .copy_within(insertion_point..old_mapping_len, insertion_point + insertion.len());
            mapping[insertion_point..insertion_point + insertion.len()].copy_from_slice(&insertion);

            mapping.iter_mut().skip(insertion_point + insertion.len()).for_each(|i| *i += count);

            drop(mapping);
            self.notify.row_added(insertion_point, insertion.len());
        }
    }

    fn row_removed(&self, index: usize, count: usize) {
        if count == 0 {
            return;
        }
        let mut mapping = self.mapping.borrow_mut();

        let start = mapping.binary_search(&index).unwrap_or_else(|s| s);
        let end = mapping.binary_search(&(index + count)).unwrap_or_else(|e| e);
        let range = start..end;

        if !range.is_empty() {
            mapping.copy_within(end.., start);
            let new_size = mapping.len() - range.len();
            mapping.truncate(new_size);

            mapping.iter_mut().skip(start).for_each(|i| *i -= count);

            drop(mapping);
            self.notify.row_removed(start, range.len());
        }
    }

    fn reset(&self) {
        self.build_mapping_vec();
        self.notify.reset();
    }
}

/// Provides a filtered subset of rows by another [`Model`].
///
/// When the other Model is updated, the `FilterModel` is updated accordingly.
///
/// ## Example
///
/// Here we have a [`VecModel`] holding [`SharedString`]s.
/// It is then filtered into a `FilterModel`.
///
/// ```
/// # use slint::{Model, VecModel, SharedString, FilterModel};
/// let model = VecModel::from(vec![
///     SharedString::from("Lorem"),
///     SharedString::from("ipsum"),
///     SharedString::from("dolor"),
/// ]);
///
/// let filtered_model = FilterModel::new(model, |s| s.contains('o'));
///
/// assert_eq!(filtered_model.row_data(0).unwrap(), SharedString::from("Lorem"));
/// assert_eq!(filtered_model.row_data(1).unwrap(), SharedString::from("dolor"));
/// ```
///
/// Alternatively you can use the shortcut [`ModelExt::filter`].
/// ```
/// # use slint::{Model, ModelExt, VecModel, SharedString, FilterModel};
/// let filtered_model = VecModel::from(vec![
///     SharedString::from("Lorem"),
///     SharedString::from("ipsum"),
///     SharedString::from("dolor"),
/// ]).filter(|s| s.contains('o'));
/// # assert_eq!(filtered_model.row_data(0).unwrap(), SharedString::from("Lorem"));
/// # assert_eq!(filtered_model.row_data(1).unwrap(), SharedString::from("dolor"));
/// ```
///
/// If you want to modify the underlying [`VecModel`] you can give it a [`Rc`] of the FilterModel:
/// ```
/// # use std::rc::Rc;
/// # use slint::{Model, VecModel, SharedString, FilterModel};
/// let model = Rc::new(VecModel::from(vec![
///     SharedString::from("Lorem"),
///     SharedString::from("ipsum"),
///     SharedString::from("dolor"),
/// ]));
///
/// let filtered_model = FilterModel::new(model.clone(), |s| s.contains('o'));
///
/// assert_eq!(filtered_model.row_data(0).unwrap(), SharedString::from("Lorem"));
/// assert_eq!(filtered_model.row_data(1).unwrap(), SharedString::from("dolor"));
///
/// model.set_row_data(1, SharedString::from("opsom"));
///
/// assert_eq!(filtered_model.row_data(0).unwrap(), SharedString::from("Lorem"));
/// assert_eq!(filtered_model.row_data(1).unwrap(), SharedString::from("opsom"));
/// assert_eq!(filtered_model.row_data(2).unwrap(), SharedString::from("dolor"));
/// ```
pub struct FilterModel<M, F>(Pin<Box<ModelChangeListenerContainer<FilterModelInner<M, F>>>>)
where
    M: Model + 'static,
    F: Fn(&M::Data) -> bool + 'static;

impl<M, F> FilterModel<M, F>
where
    M: Model + 'static,
    F: Fn(&M::Data) -> bool + 'static,
{
    /// Creates a new FilterModel based on the given `wrapped_model` and filtered by `filter_function`.
    /// Alternativly you can use [`ModelExt::filter`] on your Model.
    pub fn new(wrapped_model: M, filter_function: F) -> Self {
        let filter_model_inner = FilterModelInner {
            wrapped_model,
            filter_function,
            mapping: RefCell::new(Vec::new()),
            notify: Default::default(),
        };

        filter_model_inner.build_mapping_vec();

        let container = Box::pin(ModelChangeListenerContainer::new(filter_model_inner));

        container.wrapped_model.model_tracker().attach_peer(container.as_ref().model_peer());

        Self(container)
    }

    /// Manually reapply the filter. You need to run this e.g. if the filtering function compares
    /// against mutable state and it has changed.
    pub fn apply_filter(&self) {
        self.0.reset();
    }
    /// Gets the row index of the underlying unfiltered model for a given filtered row index.
    pub fn unfiltered_row(&self, filtered_row: usize) -> usize {
        self.0.mapping.borrow()[filtered_row]
    }
}

impl<M, F> Model for FilterModel<M, F>
where
    M: Model + 'static,
    F: Fn(&M::Data) -> bool + 'static,
{
    type Data = M::Data;

    fn row_count(&self) -> usize {
        self.0.mapping.borrow().len()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        self.0
            .mapping
            .borrow()
            .get(row)
            .map(|&wrapped_row| self.0.wrapped_model.row_data(wrapped_row).unwrap())
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.0.notify
    }
}

#[test]
fn test_filter_model() {
    let wrapped_rc = Rc::new(VecModel::from(vec![1, 2, 3, 4, 5, 6]));
    let filter = FilterModel::new(wrapped_rc.clone(), |x| x % 2 == 0);

    assert_eq!(filter.row_data(0).unwrap(), 2);
    assert_eq!(filter.row_data(1).unwrap(), 4);
    assert_eq!(filter.row_data(2).unwrap(), 6);
    assert_eq!(filter.row_count(), 3);

    wrapped_rc.remove(1);
    assert_eq!(filter.row_data(0).unwrap(), 4);
    assert_eq!(filter.row_data(1).unwrap(), 6);
    assert_eq!(filter.row_count(), 2);

    wrapped_rc.push(8);
    wrapped_rc.push(7);
    assert_eq!(filter.row_data(0).unwrap(), 4);
    assert_eq!(filter.row_data(1).unwrap(), 6);
    assert_eq!(filter.row_data(2).unwrap(), 8);
    assert_eq!(filter.row_count(), 3);

    wrapped_rc.set_row_data(1, 2);
    assert_eq!(filter.row_data(0).unwrap(), 2);
    assert_eq!(filter.row_data(1).unwrap(), 4);
    assert_eq!(filter.row_data(2).unwrap(), 6);
    assert_eq!(filter.row_data(3).unwrap(), 8);
    assert_eq!(filter.row_count(), 4);

    wrapped_rc.insert(2, 12);
    assert_eq!(filter.row_data(0).unwrap(), 2);
    assert_eq!(filter.row_data(1).unwrap(), 12);
    assert_eq!(filter.row_data(2).unwrap(), 4);
    assert_eq!(filter.row_data(3).unwrap(), 6);
    assert_eq!(filter.row_data(4).unwrap(), 8);
    assert_eq!(filter.row_count(), 5);
}

trait SortHelper<D> {
    fn sort(&self, lhs: &D, rhs: &D) -> core::cmp::Ordering;
}

struct AscendingSortHelper;

impl<D> SortHelper<D> for AscendingSortHelper
where
    D: core::cmp::Ord,
{
    fn sort(&self, lhs: &D, rhs: &D) -> core::cmp::Ordering {
        lhs.cmp(rhs)
    }
}

struct FnSortHelper<F, D>
where
    F: FnMut(&D, &D) -> core::cmp::Ordering + 'static,
{
    sort_function: RefCell<F>,
    _type: PhantomData<D>,
}

impl<F, D> SortHelper<D> for FnSortHelper<F, D>
where
    F: FnMut(&D, &D) -> core::cmp::Ordering + 'static,
{
    fn sort(&self, lhs: &D, rhs: &D) -> core::cmp::Ordering {
        (self.sort_function.borrow_mut())(lhs, rhs)
    }
}

struct SortModelInner<M>
where
    M: Model + 'static,
{
    wrapped_model: M,
    sort_helper: Box<dyn SortHelper<M::Data>>,
    // This vector saves the indices of the elements in sorted order.
    mapping: RefCell<Vec<usize>>,
    notify: ModelNotify,
    sorted_rows_dirty: Cell<bool>,
}

impl<M> SortModelInner<M>
where
    M: Model + 'static,
{
    fn build_mapping_vec(&self) {
        if !self.sorted_rows_dirty.get() {
            return;
        }

        let mut mapping = self.mapping.borrow_mut();

        mapping.clear();
        mapping.extend((0..self.wrapped_model.row_count()).into_iter());
        mapping.sort_by(|lhs, rhs| {
            self.sort_helper.sort(
                &self.wrapped_model.row_data(*lhs).unwrap(),
                &self.wrapped_model.row_data(*rhs).unwrap(),
            )
        });

        self.sorted_rows_dirty.set(false);
    }
}

impl<M> ModelChangeListener for SortModelInner<M>
where
    M: Model + 'static,
{
    fn row_changed(&self, row: usize) {
        if self.sorted_rows_dirty.get() {
            self.reset();
            return;
        }

        let removed_index = self.mapping.borrow().binary_search(&row).unwrap();
        self.mapping.borrow_mut().remove(removed_index);

        let changed_data = self.wrapped_model.borrow().row_data(row).unwrap();
        let insertion_index = self.mapping.borrow().partition_point(|existing_row| {
            self.sort_helper
                .sort(&self.wrapped_model.borrow().row_data(*existing_row).unwrap(), &changed_data)
                == std::cmp::Ordering::Less
        });

        self.mapping.borrow_mut().insert(insertion_index, row);

        if insertion_index == removed_index {
            self.notify.row_changed(removed_index);
        } else {
            self.notify.row_removed(removed_index, 1);
            self.notify.row_added(insertion_index, 1);
        }
    }

    fn row_added(&self, index: usize, count: usize) {
        if count == 0 {
            return;
        }

        if self.sorted_rows_dirty.get() {
            self.reset();
            return;
        }

        // Adjust the existing sorted row indices to match the updated source model
        for row in self.mapping.borrow_mut().iter_mut() {
            if *row >= index {
                *row += count;
            }
        }

        for row in index..(index + count) {
            let changed_data = self.wrapped_model.borrow().row_data(row).unwrap();
            let insertion_index = self.mapping.borrow().partition_point(|existing_row| {
                self.sort_helper.sort(
                    &self.wrapped_model.borrow().row_data(*existing_row).unwrap(),
                    &changed_data,
                ) == std::cmp::Ordering::Less
            });

            self.mapping.borrow_mut().insert(insertion_index, row);
            self.notify.row_added(insertion_index, 1)
        }
    }

    fn row_removed(&self, index: usize, count: usize) {
        if count == 0 {
            return;
        }

        if self.sorted_rows_dirty.get() {
            self.reset();
            return;
        }

        let mut removed_rows = vec![];

        let mut i = 0;

        loop {
            if i >= self.mapping.borrow().len() {
                break;
            }

            let sort_index = *self.mapping.borrow().get(i).unwrap();

            if sort_index >= index {
                if sort_index < index + count {
                    removed_rows.push(i);
                    self.mapping.borrow_mut().remove(i);
                    continue;
                } else {
                    *self.mapping.borrow_mut().get_mut(i).unwrap() -= count;
                }
            }

            i += 1;
        }

        for removed_row in removed_rows {
            self.notify.row_removed(removed_row, 1);
        }
    }

    fn reset(&self) {
        self.sorted_rows_dirty.set(true);
        self.notify.reset();
    }
}

/// Provides a sorted view of rows by another [`Model`].
///
/// When the other Model is updated, the `Sorted` is updated accordingly.
///
/// ## Example
///
/// Here we have a [`VecModel`] holding [`SharedString`]s.
/// It is then sorted into a `SortModel`.
///
/// ```
/// # use slint::{Model, VecModel, SharedString, SortModel};
/// let model = VecModel::from(vec![
///     SharedString::from("Lorem"),
///     SharedString::from("ipsum"),
///     SharedString::from("dolor"),
/// ]);
///
/// let sorted_model = SortModel::new(model, |lhs, rhs| lhs.to_lowercase().cmp(&rhs.to_lowercase()));
///
/// assert_eq!(sorted_model.row_data(0).unwrap(), SharedString::from("dolor"));
/// assert_eq!(sorted_model.row_data(1).unwrap(), SharedString::from("ipsum"));
/// assert_eq!(sorted_model.row_data(2).unwrap(), SharedString::from("Lorem"));
/// ```
///
/// Alternatively you can use the shortcut [`ModelExt::sort_by`].
/// ```
/// # use slint::{Model, ModelExt, VecModel, SharedString, SortModel};
/// let sorted_model = VecModel::from(vec![
///     SharedString::from("Lorem"),
///     SharedString::from("ipsum"),
///     SharedString::from("dolor"),
/// ]).sort_by(|lhs, rhs| lhs.to_lowercase().cmp(&rhs.to_lowercase()));
/// # assert_eq!(sorted_model.row_data(0).unwrap(), SharedString::from("dolor"));
/// # assert_eq!(sorted_model.row_data(1).unwrap(), SharedString::from("ipsum"));
/// # assert_eq!(sorted_model.row_data(2).unwrap(), SharedString::from("Lorem"));
/// ```
///
/// It is also possible to get a ascending sorted  `SortModel` order for `std::cmp::Ord` type items.
///
/// ```
/// # use slint::{Model, VecModel, SortModel};
/// let model = VecModel::from(vec![
///     5,
///     1,
///     3,
/// ]);
///
/// let sorted_model = SortModel::new_ascending(model);
///
/// assert_eq!(sorted_model.row_data(0).unwrap(), 1);
/// assert_eq!(sorted_model.row_data(1).unwrap(), 3);
/// assert_eq!(sorted_model.row_data(2).unwrap(), 5);
/// ```
///
/// Alternatively you can use the shortcut [`ModelExt::sort`].
/// ```
/// # use slint::{Model, ModelExt, VecModel, SharedString, SortModel};
/// let sorted_model = VecModel::from(vec![
///     5,
///     1,
///     3,
/// ]).sort();
/// # assert_eq!(sorted_model.row_data(0).unwrap(), 1);
/// # assert_eq!(sorted_model.row_data(1).unwrap(), 3);
/// # assert_eq!(sorted_model.row_data(2).unwrap(), 5);
/// ```
///
/// If you want to modify the underlying [`VecModel`] you can give it a [`Rc`] of the SortModel:
/// ```
/// # use std::rc::Rc;
/// # use slint::{Model, VecModel, SharedString, SortModel};
/// let model = Rc::new(VecModel::from(vec![
///     SharedString::from("Lorem"),
///     SharedString::from("ipsum"),
///     SharedString::from("dolor"),
/// ]));
///
/// let sorted_model = SortModel::new(model.clone(), |lhs, rhs| lhs.to_lowercase().cmp(&rhs.to_lowercase()));
///
/// assert_eq!(sorted_model.row_data(0).unwrap(), SharedString::from("dolor"));
/// assert_eq!(sorted_model.row_data(1).unwrap(), SharedString::from("ipsum"));
/// assert_eq!(sorted_model.row_data(2).unwrap(), SharedString::from("Lorem"));
///
/// model.set_row_data(1, SharedString::from("opsom"));
///
/// assert_eq!(sorted_model.row_data(0).unwrap(), SharedString::from("dolor"));
/// assert_eq!(sorted_model.row_data(1).unwrap(), SharedString::from("Lorem"));
/// assert_eq!(sorted_model.row_data(2).unwrap(), SharedString::from("opsom"));
/// ```
pub struct SortModel<M>(Pin<Box<ModelChangeListenerContainer<SortModelInner<M>>>>)
where
    M: Model + 'static;

impl<M> SortModel<M>
where
    M: Model + 'static,
{
    /// Creates a new SortModel based on the given `wrapped_model` and sorted by `sort_function`.
    /// Alternativly you can use [`ModelExt::sort_by`] on your Model.
    pub fn new<F>(wrapped_model: M, sort_function: F) -> Self
    where
        F: FnMut(&M::Data, &M::Data) -> core::cmp::Ordering + 'static,
    {
        let sorted_model_inner = SortModelInner {
            wrapped_model,
            sort_helper: Box::new(FnSortHelper {
                sort_function: RefCell::new(sort_function),
                _type: PhantomData::default(),
            }),
            mapping: RefCell::new(Vec::new()),
            notify: Default::default(),
            sorted_rows_dirty: Cell::new(true),
        };

        let container = Box::pin(ModelChangeListenerContainer::new(sorted_model_inner));

        container.wrapped_model.model_tracker().attach_peer(container.as_ref().model_peer());

        Self(container)
    }

    /// Creates a new SortModel based on the given `wrapped_model` and sorted in ascending order.
    /// Alternativly you can use [`ModelExt::sort`] on your Model.
    pub fn new_ascending(wrapped_model: M) -> Self
    where
        M::Data: core::cmp::Ord,
    {
        let sorted_model_inner = SortModelInner {
            wrapped_model,
            sort_helper: Box::new(AscendingSortHelper),
            mapping: RefCell::new(Vec::new()),
            notify: Default::default(),
            sorted_rows_dirty: Cell::new(true),
        };

        let container = Box::pin(ModelChangeListenerContainer::new(sorted_model_inner));

        container.wrapped_model.model_tracker().attach_peer(container.as_ref().model_peer());

        Self(container)
    }

    /// Manually reapply the sorting. You need to run this e.g. if the sort function compares
    /// against mutable state and it has changed.
    pub fn apply_sorting(&self) {
        self.0.reset();
    }

    /// Gets the row index of the underlying unsorted model for a given sorted row index.
    pub fn unsorted_row(&self, sorted_row: usize) -> usize {
        self.0.build_mapping_vec();
        self.0.mapping.borrow()[sorted_row]
    }
}

impl<M> Model for SortModel<M>
where
    M: Model + 'static,
{
    type Data = M::Data;

    fn row_count(&self) -> usize {
        self.0.wrapped_model.row_count()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        self.0.build_mapping_vec();

        self.0
            .mapping
            .borrow()
            .get(row)
            .map(|&wrapped_row| self.0.wrapped_model.row_data(wrapped_row).unwrap())
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.0.notify
    }
}

#[cfg(test)]
mod sort_tests {
    use super::*;

    #[derive(Default)]
    struct TestView {
        // Track the parameters reported by the model (row counts, indices, etc.).
        // The last field in the tuple is the row size the model reports at the time
        // of callback
        changed_rows: RefCell<Vec<usize>>,
        added_rows: RefCell<Vec<(usize, usize)>>,
        removed_rows: RefCell<Vec<(usize, usize)>>,
        reset: RefCell<usize>,
    }

    impl TestView {
        fn clear(&self) {
            self.changed_rows.borrow_mut().clear();
            self.added_rows.borrow_mut().clear();
            self.removed_rows.borrow_mut().clear();
        }
    }

    impl ModelChangeListener for TestView {
        fn row_changed(&self, row: usize) {
            self.changed_rows.borrow_mut().push(row);
        }

        fn row_added(&self, index: usize, count: usize) {
            self.added_rows.borrow_mut().push((index, count));
        }

        fn row_removed(&self, index: usize, count: usize) {
            self.removed_rows.borrow_mut().push((index, count));
        }
        fn reset(&self) {
            *self.reset.borrow_mut() += 1;
        }
    }

    #[test]
    fn test_sorted_model_insert() {
        let wrapped_rc = Rc::new(VecModel::from(vec![3, 4, 1, 2]));
        let sorted_model = SortModel::new(wrapped_rc.clone(), |lhs, rhs| lhs.cmp(rhs));

        let observer = Box::pin(ModelChangeListenerContainer::<TestView>::default());
        sorted_model.model_tracker().attach_peer(Pin::as_ref(&observer).model_peer());

        assert_eq!(sorted_model.row_count(), 4);
        assert_eq!(sorted_model.row_data(0).unwrap(), 1);
        assert_eq!(sorted_model.row_data(1).unwrap(), 2);
        assert_eq!(sorted_model.row_data(2).unwrap(), 3);
        assert_eq!(sorted_model.row_data(3).unwrap(), 4);

        wrapped_rc.insert(0, 10);

        assert_eq!(observer.added_rows.borrow().len(), 1);
        assert!(observer.added_rows.borrow().eq(&[(4, 1)]));
        assert!(observer.changed_rows.borrow().is_empty());
        assert!(observer.removed_rows.borrow().is_empty());
        assert_eq!(*observer.reset.borrow(), 0);
        observer.clear();

        assert_eq!(sorted_model.row_count(), 5);
        assert_eq!(sorted_model.row_data(0).unwrap(), 1);
        assert_eq!(sorted_model.row_data(1).unwrap(), 2);
        assert_eq!(sorted_model.row_data(2).unwrap(), 3);
        assert_eq!(sorted_model.row_data(3).unwrap(), 4);
        assert_eq!(sorted_model.row_data(4).unwrap(), 10);
    }

    #[test]
    fn test_sorted_model_remove() {
        let wrapped_rc = Rc::new(VecModel::from(vec![3, 4, 1, 2]));
        let sorted_model = SortModel::new(wrapped_rc.clone(), |lhs, rhs| lhs.cmp(rhs));

        let observer = Box::pin(ModelChangeListenerContainer::<TestView>::default());
        sorted_model.model_tracker().attach_peer(Pin::as_ref(&observer).model_peer());

        assert_eq!(sorted_model.row_count(), 4);
        assert_eq!(sorted_model.row_data(0).unwrap(), 1);
        assert_eq!(sorted_model.row_data(1).unwrap(), 2);
        assert_eq!(sorted_model.row_data(2).unwrap(), 3);
        assert_eq!(sorted_model.row_data(3).unwrap(), 4);

        // Remove the entry with the value 4
        wrapped_rc.remove(1);

        assert!(observer.added_rows.borrow().is_empty());
        assert!(observer.changed_rows.borrow().is_empty());
        assert_eq!(observer.removed_rows.borrow().len(), 1);
        assert!(observer.removed_rows.borrow().eq(&[(3, 1)]));
        assert_eq!(*observer.reset.borrow(), 0);
        observer.clear();

        assert_eq!(sorted_model.row_count(), 3);
        assert_eq!(sorted_model.row_data(0).unwrap(), 1);
        assert_eq!(sorted_model.row_data(1).unwrap(), 2);
        assert_eq!(sorted_model.row_data(2).unwrap(), 3);
    }

    #[test]
    fn test_sorted_model_changed() {
        let wrapped_rc = Rc::new(VecModel::from(vec![3, 4, 1, 2]));
        let sorted_model = SortModel::new(wrapped_rc.clone(), |lhs, rhs| lhs.cmp(rhs));

        let observer = Box::pin(ModelChangeListenerContainer::<TestView>::default());
        sorted_model.model_tracker().attach_peer(Pin::as_ref(&observer).model_peer());

        assert_eq!(sorted_model.row_count(), 4);
        assert_eq!(sorted_model.row_data(0).unwrap(), 1);
        assert_eq!(sorted_model.row_data(1).unwrap(), 2);
        assert_eq!(sorted_model.row_data(2).unwrap(), 3);
        assert_eq!(sorted_model.row_data(3).unwrap(), 4);

        // Change the entry with the value 4 to 10 -> maintain order
        wrapped_rc.set_row_data(1, 10);

        assert!(observer.added_rows.borrow().is_empty());
        assert_eq!(observer.changed_rows.borrow().len(), 1);
        assert_eq!(*observer.changed_rows.borrow().get(0).unwrap(), 3);
        assert!(observer.removed_rows.borrow().is_empty());
        assert_eq!(*observer.reset.borrow(), 0);
        observer.clear();

        assert_eq!(sorted_model.row_count(), 4);
        assert_eq!(sorted_model.row_data(0).unwrap(), 1);
        assert_eq!(sorted_model.row_data(1).unwrap(), 2);
        assert_eq!(sorted_model.row_data(2).unwrap(), 3);
        assert_eq!(sorted_model.row_data(3).unwrap(), 10);

        // Change the entry with the value 10 to 0 -> new order with remove and insert
        wrapped_rc.set_row_data(1, 0);

        assert_eq!(observer.added_rows.borrow().len(), 1);
        assert!(observer.added_rows.borrow().get(0).unwrap().eq(&(0, 1)));
        assert!(observer.changed_rows.borrow().is_empty());
        assert_eq!(observer.removed_rows.borrow().len(), 1);
        assert!(observer.removed_rows.borrow().get(0).unwrap().eq(&(3, 1)));
        assert_eq!(*observer.reset.borrow(), 0);
        observer.clear();

        assert_eq!(sorted_model.row_count(), 4);
        assert_eq!(sorted_model.row_data(0).unwrap(), 0);
        assert_eq!(sorted_model.row_data(1).unwrap(), 1);
        assert_eq!(sorted_model.row_data(2).unwrap(), 2);
        assert_eq!(sorted_model.row_data(3).unwrap(), 3);
    }
}
