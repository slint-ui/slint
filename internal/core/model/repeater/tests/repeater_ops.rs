// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;

/// Remove elements from the model using the repeater op
#[test]
fn test_repeater_ops_splice() {
    let repeater: Repeater<SimpleItem> = Repeater::default();
    let repeater = core::pin::pin!(repeater);

    let model: Rc<VecModel<i32>> = Rc::new(VecModel::from(vec![1, 2, 3, 4, 5, 6, 7, 8]));

    repeater.set_model_binding({
        let model = model.clone();
        move || ModelRc::from(model.clone())
    });
    repeater.as_ref().model(); // Setup tracker

    repeater.as_ref().ensure_updated(|| SimpleItem::new(-1));

    {
        let inner = repeater.0.inner.borrow();
        assert_eq!(inner.instances.len(), 8);
        assert_eq!(inner.layout_state.item_index.row, 0);
        assert_eq!(inner.layout_state.item_index.instance_index, 0);

        for (index, (state, item)) in inner.instances.iter().enumerate() {
            assert_eq!(
                index as i32 + 1,
                as_typed::<SimpleItem>(item.as_ref().unwrap()).value.get()
            );
            assert_ne!(*state, RepeatedInstanceState::Dirty);
        }
    }

    {
        // Remove after current row -> current row still is the same
        let mut ops = RustRepeaterOps {
            inner: &repeater.0.inner,
            typed: &RepeaterTypedOps::<SimpleItem> {
                init: &|| SimpleItem::new(-1),
                model: &ModelRc::from(model.clone()),
            },
            instance_generation: repeater.as_ref().data().project_ref().instance_generation,
        };
        // Remove 4 at index 4
        ops.truncate(4);

        let inner = repeater.0.inner.borrow();
        assert_eq!(inner.instances.len(), 4);
        assert_eq!(inner.layout_state.item_index.row, 0);
        assert_eq!(inner.layout_state.item_index.instance_index, 0);
        for (index, (state, item)) in inner.instances.iter().enumerate() {
            assert_eq!(
                index as i32 + 1,
                as_typed::<SimpleItem>(item.as_ref().unwrap()).value.get()
            );
            assert_ne!(*state, RepeatedInstanceState::Dirty);
        }
    }

    {
        // Remove current row
        let mut ops = RustRepeaterOps {
            inner: &repeater.0.inner,
            typed: &RepeaterTypedOps::<SimpleItem> {
                init: &|| SimpleItem::new(-1),
                model: &ModelRc::from(model.clone()),
            },
            instance_generation: repeater.as_ref().data().project_ref().instance_generation,
        };
        // Remove first 2 items
        ops.remove_first(2);

        let inner = repeater.0.inner.borrow();
        assert_eq!(inner.instances.len(), 2);
        assert_eq!(
            inner.layout_state.item_index.row, 2,
            "Current row instance was removed. So we move to the first available instance"
        );
        assert_eq!(
            inner.layout_state.item_index.instance_index, 0,
            "Didn't change. Points now to the first remaining instance"
        );
        const OFFSET: i32 = 2; // First instance must now be the element with value 3
        for (index, (state, item)) in inner.instances.iter().enumerate() {
            assert_eq!(
                index as i32 + 1 + OFFSET,
                as_typed::<SimpleItem>(item.as_ref().unwrap()).value.get()
            );
            assert_ne!(*state, RepeatedInstanceState::Dirty);
        }
    }
}

#[test]
fn test_repeater() {
    let repeater: Repeater<SimpleItem> = Repeater::default();
    let repeater = core::pin::pin!(repeater);

    let model: Rc<VecModel<i32>> = Rc::new(VecModel::from(vec![1, 2, 3, 4, 5, 6, 7, 8]));

    repeater.set_model_binding({
        let model = model.clone();
        move || ModelRc::from(model.clone())
    });
    repeater.as_ref().model(); // Setup tracker

    repeater.as_ref().ensure_updated(|| SimpleItem::new(-1));

    {
        let inner = repeater.0.inner.borrow();
        assert_eq!(inner.instances.len(), 8);
        assert_eq!(inner.layout_state.item_index.row, 0);
        assert_eq!(inner.layout_state.item_index.instance_index, 0);
        for (index, (state, item)) in inner.instances.iter().enumerate() {
            assert_eq!(
                index as i32 + 1,
                as_typed::<SimpleItem>(item.as_ref().unwrap()).value.get()
            );
            assert_ne!(*state, RepeatedInstanceState::Dirty);
        }
    }

    // All items are there already
    repeater.0.inner.borrow_mut().set_current_row(2);

    {
        let inner = repeater.0.inner.borrow();
        assert_eq!(inner.instances.len(), 8);
        assert_eq!(inner.layout_state.item_index.row, 2);
        assert_eq!(inner.layout_state.item_index.instance_index, 2);
        // None are dirty
        for (index, (state, item)) in inner.instances.iter().enumerate() {
            assert_eq!(
                index as i32 + 1,
                as_typed::<SimpleItem>(item.as_ref().unwrap()).value.get()
            );
            assert_ne!(*state, RepeatedInstanceState::Dirty);
        }
    }

    {
        // Remove multiple instances so we have less
        let mut ops = RustRepeaterOps {
            inner: &repeater.0.inner,
            typed: &RepeaterTypedOps::<SimpleItem> {
                init: &|| SimpleItem::new(-1),
                model: &ModelRc::from(model.clone()),
            },
            instance_generation: repeater.as_ref().data().project_ref().instance_generation,
        };
        ops.truncate(4);

        let inner = repeater.0.inner.borrow();
        assert_eq!(inner.instances.len(), 4);
        assert_eq!(inner.layout_state.item_index.row, 2);
        assert_eq!(inner.layout_state.item_index.instance_index, 2);
    }

    // We are past the last instance so new instances must be created
    repeater.0.inner.borrow_mut().set_current_row(6);

    {
        let inner = repeater.0.inner.borrow();
        assert_eq!(inner.instances.len(), 7);
        assert_eq!(inner.layout_state.item_index.row, 6);
        assert_eq!(inner.layout_state.item_index.instance_index, 6);
        for (index, (state, item)) in inner.instances.iter().enumerate() {
            if index < 4 {
                assert_ne!(*state, RepeatedInstanceState::Dirty, "Index: {index}");
                assert_eq!(
                    index as i32 + 1,
                    as_typed::<SimpleItem>(item.as_ref().unwrap()).value.get()
                );
            } else {
                assert_eq!(*state, RepeatedInstanceState::Dirty, "Index: {index}");
            }
        }
    }

    const REMOVE_COUNT: usize = 2;
    {
        // Remove first 3 instances -> current row changes to still point to the same item
        let mut ops = RustRepeaterOps {
            inner: &repeater.0.inner,
            typed: &RepeaterTypedOps::<SimpleItem> {
                init: &|| SimpleItem::new(-1),
                model: &ModelRc::from(model.clone()),
            },
            instance_generation: repeater.as_ref().data().project_ref().instance_generation,
        };
        ops.remove_first(REMOVE_COUNT);

        let inner = repeater.0.inner.borrow();
        assert_eq!(inner.instances.len(), 7 - REMOVE_COUNT);
        assert_eq!(inner.layout_state.item_index.row, 6);
        assert_eq!(inner.layout_state.item_index.instance_index, 6 - REMOVE_COUNT);

        for (index, (state, item)) in inner.instances.iter().enumerate() {
            if index < 2 {
                assert_ne!(*state, RepeatedInstanceState::Dirty, "Index: {index}");
                assert_eq!(
                    index as i32 + 1 + REMOVE_COUNT as i32,
                    as_typed::<SimpleItem>(item.as_ref().unwrap()).value.get(),
                    "Index: {index}"
                );
            } else {
                assert_eq!(*state, RepeatedInstanceState::Dirty, "Index: {index}");
            }
        }
    }

    // Set to a row which is not yet instantiated because we removed before so new items must be created
    // we removed previously 2 elements, therefore there are 4 items left in the instance and the 4th is row 6
    // to get to row zero we have to add 2 elements (4 - 6 = -2)
    const ADDED_COUNT: usize = 2;
    repeater.0.inner.borrow_mut().set_current_row(0);

    {
        let inner = repeater.0.inner.borrow();
        assert_eq!(inner.instances.len(), 7 - REMOVE_COUNT + ADDED_COUNT);
        assert_eq!(inner.layout_state.item_index.row, 0);
        assert_eq!(
            inner.layout_state.item_index.instance_index, 0,
            "New items got instantiated to be able to represent row 0"
        );
        for (index, (state, item)) in inner.instances.iter().enumerate() {
            if index == 2 || index == 3 {
                assert_ne!(*state, RepeatedInstanceState::Dirty, "Index: {index}");
                assert_eq!(
                    index as i32 + 1 + REMOVE_COUNT as i32 - ADDED_COUNT as i32,
                    as_typed::<SimpleItem>(item.as_ref().unwrap()).value.get(),
                    "Index: {index}"
                );
            } else {
                assert_eq!(*state, RepeatedInstanceState::Dirty, "Index: {index}");
            }
        }
    }
}

#[test]
fn test_adding_element() {
    let repeater: Repeater<SimpleItem> = Repeater::default();
    let repeater = core::pin::pin!(repeater);
    let model: Rc<VecModel<i32>> = Rc::new(VecModel::from(Vec::new()));

    repeater.set_model_binding({
        let model = model.clone();
        move || ModelRc::from(model.clone())
    });
    repeater.as_ref().model(); // Setup tracker

    {
        let inner = repeater.0.inner.borrow();
        assert_eq!(inner.instances.len(), 0, "We don't have any items yet in the model");
        assert_eq!(inner.layout_state.item_index.row, 0);
    }

    model.push(2);
    {
        let inner = repeater.0.inner.borrow();
        assert_eq!(inner.instances.len(), 1, "We added one element");
        assert_eq!(
            inner.instances[0].0,
            RepeatedInstanceState::Dirty,
            "The item was just added so it is still dirty"
        );
        assert_eq!(inner.layout_state.item_index.row, 0);
        assert_eq!(inner.layout_state.item_index.instance_index, 0);
    }
    repeater.as_ref().ensure_updated(|| SimpleItem::new(-1));

    {
        let inner = repeater.0.inner.borrow();
        assert_eq!(inner.instances.len(), 1);
        assert_eq!(get_instance_inner_value(&inner, 0), 2);
    }

    // Append
    model.push(5);
    {
        let inner = repeater.0.inner.borrow();
        assert_eq!(inner.instances.len(), 2);
        assert_eq!(
            inner.layout_state.item_index.row, 0,
            "We appended the item so we don't have to adapt"
        );
        assert_ne!(
            inner.instances[0].0,
            RepeatedInstanceState::Dirty,
            "The item was already inside so it must not be dirty because of the previous ensure_updated"
        );
        assert_eq!(
            inner.instances[1].0,
            RepeatedInstanceState::Dirty,
            "The item was just added so it is still dirty"
        );
        assert_eq!(inner.layout_state.item_index.instance_index, 0);
        assert_eq!(get_instance_inner_value(&inner, 0), 2);
    }
    repeater.as_ref().ensure_updated(|| SimpleItem::new(-1));

    // Check that appended by checking their internal values
    {
        let inner = repeater.0.inner.borrow();
        assert_eq!(inner.instances.len(), 2);
        assert_eq!(get_instance_inner_value(&inner, 0), 2);
        assert_eq!(get_instance_inner_value(&inner, 1), 5);
    }

    // Prepend
    model.insert(0, 9);
    {
        let inner = repeater.0.inner.borrow();
        assert_eq!(inner.instances.len(), 3);
        assert_eq!(
            inner.layout_state.item_index.row, 1,
            "We inserted an element before the current element so we have to adapt"
        );
        assert_eq!(inner.layout_state.item_index.instance_index, 1);

        // All are dirty because the first was added and the others got moved because of the prepend
        for instance in &inner.instances {
            assert_eq!(instance.0, RepeatedInstanceState::Dirty);
        }
    }

    repeater.as_ref().ensure_updated(|| SimpleItem::new(-1));

    // Check that prepended by checking their internal values
    // All must got dirty because the first was prepended and the others shifted
    {
        assert_eq!(model.row_data(0).unwrap(), 9);
        assert_eq!(model.row_data(1).unwrap(), 2);
        assert_eq!(model.row_data(2).unwrap(), 5);

        let inner = repeater.0.inner.borrow();
        assert_eq!(inner.layout_state.item_index.instance_index, 1); // One was prepended
        assert_eq!(inner.instances.len(), 3);
        assert_eq!(get_instance_inner_value(&inner, 0), 9, "Was prepended");
        assert_eq!(
            get_instance_inner_value(&inner, inner.layout_state.item_index.instance_index),
            2
        );
        assert_eq!(get_instance_inner_value(&inner, 2), 5);
    }
}

#[test]
fn test_remove_elements() {
    let repeater: Repeater<SimpleItem> = Repeater::default();
    let repeater = core::pin::pin!(repeater);
    let model: Rc<VecModel<i32>> = Rc::new(VecModel::from(Vec::from_iter(2..6)));

    repeater.set_model_binding({
        let model = model.clone();
        move || ModelRc::from(model.clone())
    });
    repeater.as_ref().model(); // Setup tracker

    {
        let inner = repeater.0.inner.borrow();
        assert_eq!(model.row_count(), 4);
        assert_eq!(
            inner.instances.len(),
            0,
            "We didn't instantiate one yet because ensure updated was not yet called"
        );
        assert_eq!(inner.layout_state.item_index.row, 0);
        assert_eq!(inner.layout_state.item_index.instance_index, 0);
    }

    // Create instances
    repeater.as_ref().ensure_updated(|| SimpleItem::new(-1));

    {
        let inner = repeater.0.inner.borrow();
        assert_eq!(model.row_count(), 4, "We added 4 elements");
        assert_eq!(inner.instances.len(), 4, "4 instances are created for every element one");
        assert_eq!(inner.layout_state.item_index.row, 0);
        assert_eq!(inner.layout_state.item_index.instance_index, 0);

        for (index, (state, item)) in inner.instances.iter().enumerate() {
            assert_ne!(*state, RepeatedInstanceState::Dirty, "Index: {index}");
            assert_eq!(
                index as i32 + 2,
                as_typed::<SimpleItem>(item.as_ref().unwrap()).value.get(),
                "Index: {index}"
            );
        }
    }

    // Point as current element to the 3th one but the instance index is still at 0.
    // That means for the listview that the current item is at the beginning and
    // the next visible elements are after and no element before the current item is visible
    const REMOVE_COUNT: usize = 2; // Count of instances to remove
    {
        let mut inner = repeater.0.inner.borrow_mut();
        inner.set_current_row(2);
        assert_eq!(
            inner.layout_state.item_index.instance_index, 2,
            "Must move together with the row"
        );
        assert_eq!(model.row_data(inner.layout_state.item_index.row), Some(4));
        drop(inner);
        {
            // Remove first two instances
            let mut ops = RustRepeaterOps {
                inner: &repeater.0.inner,
                typed: &RepeaterTypedOps::<SimpleItem> {
                    init: &|| SimpleItem::new(-1),
                    model: &ModelRc::from(model.clone()),
                },
                instance_generation: repeater.as_ref().data().project_ref().instance_generation,
            };
            // Remove first REMOVE_COUNT items
            ops.remove_first(REMOVE_COUNT);
            let inner = repeater.0.inner.borrow();
            assert_eq!(
                inner.layout_state.item_index.instance_index, 0,
                "Must point to the first item because the first two are removed"
            );
            assert_eq!(
                model.row_data(inner.layout_state.item_index.row),
                Some(4),
                "The row must stay"
            );
        }
        let inner = repeater.0.inner.borrow();
        assert_eq!(inner.layout_state.item_index.instance_index, 0);
        assert_eq!(
            get_instance_inner_value(&inner, inner.layout_state.item_index.instance_index),
            4
        );
        for (index, (state, item)) in inner.instances.iter().enumerate() {
            assert_ne!(*state, RepeatedInstanceState::Dirty, "Index: {index}");
            assert_eq!(
                index as i32 + 2 + REMOVE_COUNT as i32,
                as_typed::<SimpleItem>(item.as_ref().unwrap()).value.get(),
                "Index: {index}"
            );
        }
    }

    // Remove one after the current element
    assert_eq!(model.remove(3), 5, "Last element must be 5");
    {
        let inner = repeater.0.inner.borrow();
        assert_eq!(model.row_count(), 3); // --> 2, 3, 4 are left
        assert_eq!(
            inner.instances.len(),
            1,
            "We had totally 4 instances. Above we removed 2 and now another one. The one left is the instance for the current row"
        );
        assert_eq!(
            inner.layout_state.item_index.row, 2,
            "The row must not change because the removed is after"
        );
        assert_eq!(
            inner.layout_state.item_index.instance_index, 0,
            "The instance index must not change because the removed is after"
        );
        assert_eq!(model.row_data(inner.layout_state.item_index.row), Some(4));

        // The current item is not dirty but all after
        assert_ne!(
            inner.instances[inner.layout_state.item_index.instance_index].0,
            RepeatedInstanceState::Dirty
        );

        for (index, (state, item)) in inner.instances.iter().enumerate() {
            if index <= inner.layout_state.item_index.instance_index {
                assert_ne!(*state, RepeatedInstanceState::Dirty, "Index: {index}");
                assert_eq!(
                    index as i32 + 2 + REMOVE_COUNT as i32,
                    as_typed::<SimpleItem>(item.as_ref().unwrap()).value.get(),
                    "Index: {index}"
                );
            } else {
                // All items after the current index are dirty
                assert_eq!(*state, RepeatedInstanceState::Dirty, "Index: {index}");
            }
        }
    }

    // Remove one before the current element
    // Remove one element of the three left
    assert_eq!(model.remove(1), 3, "Second element must have the internal value 3");
    {
        let inner = repeater.0.inner.borrow();
        assert_eq!(model.row_count(), 2); // -> 2, 4 left
        assert_eq!(
            inner.instances.len(),
            1,
            "The removed item was before the current row. So the instance does not change"
        );
        assert_eq!(inner.layout_state.item_index.row, 1);
        assert_eq!(
            inner.layout_state.item_index.instance_index, 0,
            "The instance did not change because the removed item was out of view"
        );
        assert_eq!(model.row_data(inner.layout_state.item_index.row), Some(4));
        assert_eq!(
            get_instance_inner_value(&inner, inner.layout_state.item_index.instance_index),
            4
        );
    }

    // Remove current item
    assert_eq!(model.remove(1), 4);
    {
        let inner = repeater.0.inner.borrow();
        assert_eq!(model.row_count(), 1); // -> item with value 2 left
        assert_eq!(inner.instances.len(), 0, "Current instance is now cleared as well");
        assert_eq!(
            inner.layout_state.item_index.row, 0,
            "row points now to the last remaining item"
        );
        assert_eq!(inner.layout_state.item_index.instance_index, 0);
        assert_eq!(
            model.row_data(inner.layout_state.item_index.row),
            Some(2),
            "row points now to the previous one"
        );
    }

    // Remove last element
    assert_eq!(model.remove(0), 2);
    {
        let inner = repeater.0.inner.borrow();
        assert_eq!(model.row_count(), 0);
        assert_eq!(inner.instances.len(), 0);
        assert_eq!(inner.layout_state.item_index.row, 0);
        assert_eq!(inner.layout_state.item_index.instance_index, 0);
        assert_eq!(model.row_data(inner.layout_state.item_index.row), None, "Should be empty");
    }
}

#[test]
fn test_row_added_without_rows() {
    let model = TestModel::new(5);
    let repeater = repeater_with_window(&model, 0..5, 2);
    let before = state(&repeater);
    // Before:
    //          0, 1, 2, 3, 4
    //                ^
    // Window: |-------------|
    // After:
    //          0, 1, 2, 3, 4
    //                ^
    // Window: |-------------|
    model.insert(3, &[]);
    assert_eq!(state(&repeater), before);
    assert!(!repeater.as_ref().data().project_ref().is_dirty.get());
}

#[test]
fn test_row_added_before_window() {
    let model = TestModel::new(10);
    let repeater = repeater_with_window(&model, 4..7, 5);
    // Before:
    //          0, 1, 2, 3, 4, 5, 6, 7, 8, 9
    //                         ^
    // Window:              |------|
    // After:
    //          0, 1, 100, 2, 3, 4, 5, 6, 7, 8, 9
    //                              ^
    // Window:                  |------|
    model.insert(2, &[100]);
    assert_eq!(
        state(&repeater),
        (
            // They must be dirty, because the row changed
            vec![Dirty, Dirty, Dirty],
            CurrentItem { row: 6, instance_index: 1 }
        )
    );
}

#[test]
fn test_row_added_at_window_start() {
    let model = TestModel::new(10);
    let repeater = repeater_with_window(&model, 2..6, 4);
    // Before:
    //          0, 1, 2, 3, 4, 5, 6, 7, 8, 9
    //                      ^
    // Window:       |----------|
    // After:
    //          0, 1, 100, 2, 3, 4, 5, 6, 7, 8, 9
    //                           ^
    // Window:        |--------------|
    model.insert(2, &[100]);
    assert_eq!(
        state(&repeater),
        (
            // The row changed for the item, so they get all dirty
            vec![Empty, Dirty, Dirty, Dirty, Dirty],
            CurrentItem { row: 5, instance_index: 3 }
        ),
        "We must prepend one item"
    );
}

#[test]
fn test_row_added_after_window() {
    let model = TestModel::new(10);
    let repeater = repeater_with_window(&model, 0..3, 0);
    let before = state(&repeater);
    // Before:
    //          0, 1, 2, 3, 4, 5, 6, 7, 8, 9
    //          ^
    // Window:  |------|
    // After:
    //          0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 100
    //          ^
    // Window:  |------|
    model.insert(10, &[100]);
    assert_eq!(
        state(&repeater),
        before,
        "State must not change because it is after the window and that would lead to a splitted window"
    );
}

#[test]
fn test_row_added_after_current_row() {
    let model = Rc::new(VecModel::from(vec![10, 20]));
    let repeater = repeater_for(&model);
    // Before:
    //          10, 20
    //           ^
    // Window: |-----|
    // After:
    //          5, 10, 20
    //             ^
    // Window: |---------|
    model.insert(0, 5);
    repeater.as_ref().ensure_updated(new_item);
    // Before:
    //          5, 10, 20
    //             ^
    // Window: |---------|
    // After:
    //          5, 10, 20, 30
    //             ^
    // Window: |-------------|
    model.push(30);
    assert_eq!(
        state(&repeater),
        (vec![Clean(5), Clean(10), Clean(20), Empty], CurrentItem { row: 1, instance_index: 1 })
    );
}

#[test]
fn test_row_added_at_current_row_keeps_earlier_instances_clean() {
    let model = Rc::new(VecModel::from(vec![10, 20]));
    let repeater = repeater_for(&model);
    // Before:
    //          10, 20
    //           ^
    // Window: |-----|
    // After:
    //          5, 10, 20
    //             ^
    // Window: |---------|
    model.insert(0, 5);
    repeater.as_ref().ensure_updated(new_item);
    // Before:
    //          5, 10, 20
    //             ^
    // Window: |---------|
    // After:
    //          5, 7, 10, 20
    //                ^
    // Window: |------------|
    model.insert(1, 7);
    assert_eq!(
        state(&repeater),
        (vec![Clean(5), Empty, Dirty, Dirty], CurrentItem { row: 2, instance_index: 2 })
    );
}

#[test]
fn test_rows_added_after_reset() {
    let model = Rc::new(VecModel::from(vec![10, 20]));
    let repeater = repeater_for(&model);
    // Before:
    //          10, 20
    //           ^
    // Window: |-----|
    // After:
    //          1, 2, 3
    //          ^
    // Window: |-------|
    model.set_vec(vec![1, 2, 3]);
    assert_eq!(state(&repeater), (vec![], CurrentItem { row: 0, instance_index: 0 }));

    repeater.as_ref().ensure_updated(new_item);
    assert_eq!(
        state(&repeater),
        (vec![Clean(1), Clean(2), Clean(3)], CurrentItem { row: 0, instance_index: 0 })
    );
    model.push(4);
    model.push(5);
    assert_eq!(
        state(&repeater),
        (
            vec![Clean(1), Clean(2), Clean(3), Empty, Empty],
            CurrentItem { row: 0, instance_index: 0 }
        )
    );
    ensure_updated_window(repeater.as_ref());
    assert_eq!(
        state(&repeater),
        (
            vec![Clean(1), Clean(2), Clean(3), Clean(4), Clean(5)],
            CurrentItem { row: 0, instance_index: 0 }
        )
    );
}

#[test]
fn test_row_removed_without_rows() {
    let model = TestModel::new(5);
    let repeater = repeater_with_window(&model, 0..5, 2);
    let before = state(&repeater);
    // Before:
    //           0, 1, 2, 3, 4
    //                 ^
    // Window:  |-------------|
    // After:
    //           0, 1, 2, 3, 4
    //                 ^
    // Window:  |-------------|
    model.remove(1..1);
    assert_eq!(state(&repeater), before);
    assert!(!repeater.as_ref().data().project_ref().is_dirty.get());
}

#[test]
fn test_row_removed_after_current_row() {
    let model = TestModel::new(8);
    let repeater = repeater_with_window(&model, 2..6, 3);
    let before = state(&repeater);
    // Before:
    //           0, 1, 2, 3, 4, 5, 6, 7
    //                    ^
    // Window:        |----------|
    // After:
    //           0, 1, 2, 3, 5, 6, 7
    //                    ^
    // Window:        |-------|
    model.remove(4..5);
    // Last one is dirty, because the row of it changed
    assert_eq!(state(&repeater), (vec![Clean(2), Clean(3), Dirty], before.1));
}

#[test]
fn test_rows_removed_across_window_end() {
    let model = TestModel::new(8);
    let repeater = repeater_with_window(&model, 2..6, 3);
    let before = state(&repeater);
    // Before:
    //           0, 1, 2, 3, 4, 5, 6, 7
    //                    ^
    // Window:        |----------|
    // After:
    //           0, 1, 2, 3, 4, 7
    //                    ^
    // Window:        |-------|
    model.remove(5..7);
    assert_eq!(state(&repeater), (vec![Clean(2), Clean(3), Clean(4)], before.1));
}

#[test]
fn test_rows_removed_after_window() {
    let model = TestModel::new(8);
    let repeater = repeater_with_window(&model, 2..6, 3);
    let before = state(&repeater);
    // Before:
    //           0, 1, 2, 3, 4, 5, 6, 7
    //                    ^
    // Window:        |----------|
    // After:
    //           0, 1, 2, 3, 4, 5
    //                    ^
    // Window:        |----------|
    model.remove(6..8);
    assert_eq!(state(&repeater), before);
}

#[test]
fn test_rows_removed_before_current_row_but_inside_window() {
    let model = TestModel::new(8);
    let repeater = repeater_with_window(&model, 2..7, 5);
    // Before:
    //         0, 1, 2, 3, 4, 5, 6, 7
    //                        ^
    // Window:       |------------|
    // After:
    //         0, 1, 2, 5, 6, 7
    //                  ^
    // Window:       |------|
    model.remove(3..5);
    // The second and the third must be dirty because their row changed
    assert_eq!(
        state(&repeater),
        (vec![Clean(2), Dirty, Dirty], CurrentItem { row: 3, instance_index: 1 })
    );
    ensure_updated_window(repeater.as_ref());
    assert_eq!(
        state(&repeater),
        (vec![Clean(2), Clean(5), Clean(6)], CurrentItem { row: 3, instance_index: 1 })
    );
}

#[test]
fn test_rows_removed_across_window_start() {
    let model = TestModel::new(10);
    let repeater = repeater_with_window(&model, 4..8, 6);
    // Before:
    // 0, 1, 2, 3, 4, 5, 6, 7, 8, 9
    //                   ^
    // Window:     |--------|
    // After:
    // 0, 1, 2, 5, 6, 7, 8, 9
    //             ^
    // Window: |------|
    model.remove(3..5);
    assert_eq!(
        state(&repeater),
        (vec![Dirty, Dirty, Dirty], CurrentItem { row: 4, instance_index: 1 })
    );
    ensure_updated_window(repeater.as_ref());
    assert_eq!(
        state(&repeater),
        (vec![Clean(5), Clean(6), Clean(7)], CurrentItem { row: 4, instance_index: 1 })
    );
}

#[test]
fn test_row_removed_current_row() {
    let model = TestModel::new(8);
    let repeater = repeater_with_window(&model, 2..6, 3);
    // Before:
    //   0, 1, 2, 3, 4, 5, 6, 7
    //            ^
    // Window: |--------|
    // After:
    //   0, 1, 2, 4, 5, 6, 7
    //            ^
    // Window: |-----|
    model.remove(3..4);
    assert_eq!(
        state(&repeater),
        (vec![Clean(2), Dirty, Dirty], CurrentItem { row: 3, instance_index: 1 })
    );
    ensure_updated_window(repeater.as_ref());
    assert_eq!(
        state(&repeater),
        (vec![Clean(2), Clean(4), Clean(5)], CurrentItem { row: 3, instance_index: 1 })
    );
}

#[test]
fn test_rows_removed_across_window_start_and_current_row() {
    let model = TestModel::new(10);
    let repeater = repeater_with_window(&model, 4..8, 5);
    // Before:
    //   0, 1, 2, 3, 4, 5, 6, 7, 8, 9
    //                  ^
    // Window:       |--------|
    // After:
    //   0, 1, 2, 6, 7, 8, 9
    //            ^
    // Window:   |----|
    model.remove(3..6);
    assert_eq!(state(&repeater), (vec![Dirty, Dirty], CurrentItem { row: 3, instance_index: 0 }));
    ensure_updated_window(repeater.as_ref());
    assert_eq!(
        state(&repeater),
        (vec![Clean(6), Clean(7)], CurrentItem { row: 3, instance_index: 0 })
    );
}

#[test]
fn test_rows_removed_from_current_row_across_window_end() {
    let model = TestModel::new(10);
    let repeater = repeater_with_window(&model, 2..6, 4);
    // Before:
    //   0, 1, 2, 3, 4, 5, 6, 7, 8, 9
    //               ^
    // Window: |-----------|
    // After:
    //   0, 1, 2, 3, 8, 9
    //            ^
    // Window: |---|
    model.remove(4..8);
    assert_eq!(
        state(&repeater),
        (vec![Clean(2), Clean(3)], CurrentItem { row: 3, instance_index: 1 })
    );
}

#[test]
fn test_all_rows_removed() {
    // Before:
    //         0, 1, 2, 3, 4
    //               ^
    // Window: |-----------|
    // After: empty
    // Window: empty
    let model = TestModel::new(5);
    let repeater = repeater_with_window(&model, 0..5, 2);
    model.remove(0..5);
    assert_eq!(state(&repeater), (vec![], CurrentItem { row: 0, instance_index: 0 }));
}

#[test]
fn test_current_row_removed_at_the_end() {
    // Newest first, capped at two rows
    let model = Rc::new(VecModel::from(vec![2, 1]));
    let repeater = repeater_for(&model);
    {
        let value = 3;
        // Example for value 3
        // Before:
        //         2, 1
        //         ^
        // Window: |--|
        // After:
        //         3, 2, 1
        //            ^
        // Window: |-----|
        model.insert(0, value);
        ensure_updated_window(repeater.as_ref());
        assert_eq!(
            state(&repeater),
            (
                vec![Clean(value), Clean(value - 1), Clean(value - 2)],
                CurrentItem { row: 1, instance_index: 1 }
            )
        );
        // Before:
        //         3, 2, 1
        //            ^
        // Window: |-----|
        // After:
        //         3, 2
        //            ^
        // Window: |---|
        model.remove(2);
        ensure_updated_window(repeater.as_ref());
        assert_eq!(
            state(&repeater),
            (vec![Clean(value), Clean(value - 1)], CurrentItem { row: 1, instance_index: 1 })
        );
    }
    {
        let value = 4;
        // Example for value 4
        // Before:
        //         3, 2
        //            ^
        // Window: |--|
        // After:
        //         4, 3, 2
        //               ^
        // Window: |-----|
        model.insert(0, value);
        ensure_updated_window(repeater.as_ref());
        assert_eq!(
            state(&repeater),
            (
                vec![Clean(value), Clean(value - 1), Clean(value - 2)],
                CurrentItem { row: 2, instance_index: 2 }
            )
        );
        // Before:
        //         4, 3, 2
        //               ^
        // Window: |-----|
        // After:
        //         4, 3 // behind 2 there are no elements so the direct previous gets the new active
        //            ^
        // Window: |---|
        model.remove(2);
        ensure_updated_window(repeater.as_ref());
        assert_eq!(
            state(&repeater),
            (vec![Clean(value), Clean(value - 1)], CurrentItem { row: 1, instance_index: 1 })
        );
    }
}

/// Remove first instance
#[test]
fn test_remove_first_instance() {
    let model = TestModel::new(8);
    let repeater = repeater_with_window(&model, 2..6, 4);
    // Before:
    //         0, 1, 2, 3, 4, 5, 6, 7
    //                     ^
    // Window:       |---------|
    // After:
    //         0, 1, 2, 3, 4, 5, 6, 7
    //                     ^
    // Window:         |-------|
    with_ops(repeater.as_ref(), |ops| ops.remove_first(1));
    assert_eq!(
        state(&repeater),
        (vec![Clean(3), Clean(4), Clean(5)], CurrentItem { row: 4, instance_index: 1 })
    );
}

#[test]
fn test_prepend_instances() {
    let model = TestModel::new(8);
    let repeater = repeater_with_window(&model, 2..6, 3);
    // Before:
    //         0, 1, 2, 3, 4, 5, 6, 7
    //                  ^
    // Window:       |---------|
    // After:
    //         0, 1, 2, 3, 4, 5, 6, 7
    //                  ^
    // Window: |---------------|
    with_ops(repeater.as_ref(), |ops| ops.prepend(2));
    assert_eq!(
        state(&repeater),
        (
            vec![Empty, Empty, Clean(2), Clean(3), Clean(4), Clean(5)],
            CurrentItem { row: 3, instance_index: 3 }
        )
    );
}

#[test]
fn test_push_instances() {
    let model = TestModel::new(8);
    let repeater = repeater_with_window(&model, 2..5, 3);
    // Before:
    //         0, 1, 2, 3, 4, 5, 6, 7
    //                  ^
    // Window:       |------|
    // Before:
    //         0, 1, 2, 3, 4, 5, 6, 7
    //                  ^
    // Window:       |------------|
    with_ops(repeater.as_ref(), |ops| ops.push(2));
    assert_eq!(
        state(&repeater),
        (
            vec![Clean(2), Clean(3), Clean(4), Empty, Empty],
            CurrentItem { row: 3, instance_index: 1 }
        )
    );
    ensure_updated_window(repeater.as_ref());
    assert_eq!(
        state(&repeater),
        (
            vec![Clean(2), Clean(3), Clean(4), Clean(5), Clean(6)],
            CurrentItem { row: 3, instance_index: 1 }
        )
    );
}

#[test]
fn test_truncate_after_current_instance() {
    let model = TestModel::new(8);
    let repeater = repeater_with_window(&model, 2..6, 3);
    // Before:
    //         0, 1, 2, 3, 4, 5, 6, 7
    //                  ^
    // Window:       |---------|
    // Before:
    //         0, 1, 2, 3, 4, 5, 6, 7
    //                  ^
    // Window:      |----|
    with_ops(repeater.as_ref(), |ops| ops.truncate(2));
    assert_eq!(
        state(&repeater),
        (vec![Clean(2), Clean(3)], CurrentItem { row: 3, instance_index: 1 })
    );
}

#[test]
fn test_truncate_removing_current_instance() {
    let model = TestModel::new(8);
    let repeater = repeater_with_window(&model, 2..6, 4);
    // Before:
    //         0, 1, 2, 3, 4, 5, 6, 7
    //                  ^
    // Window:       |---------|
    // Before:
    //         0, 1, 2, 3, 4, 5, 6, 7
    //               ^
    // Window:      |-|
    with_ops(repeater.as_ref(), |ops| ops.truncate(1));
    assert_eq!(
        state(&repeater),
        (vec![Clean(2)], CurrentItem { row: 2, instance_index: 0 }),
        "Moves to the last remaining instance"
    );
}

#[test]
fn test_clear_instances() {
    let model = TestModel::new(5);
    let repeater = repeater_with_window(&model, 0..5, 2);
    with_ops(repeater.as_ref(), |ops| ops.clear());
    assert_eq!(state(&repeater), (vec![], CurrentItem { row: 2, instance_index: 0 }));
}

#[test]
fn test_empty_instance() {
    let model = TestModel::new(50);
    let repeater = repeater_with_window(&model, 10..10, 10);
    with_ops(repeater.as_ref(), |ops| ops.clear());
    assert_eq!(state(&repeater), (vec![], CurrentItem { row: 10, instance_index: 0 }));

    repeater.0.inner.borrow_mut().set_current_row(25);
    assert_eq!(state(&repeater), (vec![], CurrentItem { row: 25, instance_index: 0 }));
    ensure_updated_window(repeater.as_ref());
    assert_eq!(state(&repeater), (vec![], CurrentItem { row: 25, instance_index: 0 }));
}

#[test]
fn test_set_current_row_within_window() {
    let model = TestModel::new(5);
    let repeater = repeater_with_window(&model, 0..5, 4);
    let slots = state(&repeater).0;
    // Before:
    //         0, 1, 2, 3, 4
    //                     ^
    // Window:|-------------|
    // After:
    //         0, 1, 2, 3, 4
    //            ^
    // Window:|-------------|
    repeater.0.inner.borrow_mut().set_current_row(1);
    assert_eq!(state(&repeater), (slots, CurrentItem { row: 1, instance_index: 1 }));
    let before = state(&repeater);
    repeater.0.inner.borrow_mut().set_current_row(1);
    assert_eq!(state(&repeater), before);
}

#[test]
fn test_set_current_row_append() {
    let model = TestModel::new(8);
    let repeater = repeater_with_window(&model, 0..5, 4);
    // Before:
    //         0, 1, 2, 3, 4, 5, 6, 7
    //                     ^
    // Window:|-------------|
    // After:
    //         0, 1, 2, 3, 4, 5, 6, 7
    //                              ^
    // Window:|----------------------|
    repeater.0.inner.borrow_mut().set_current_row(7);
    assert_eq!(
        state(&repeater),
        (
            vec![Clean(0), Clean(1), Clean(2), Clean(3), Clean(4), Empty, Empty, Empty],
            CurrentItem { row: 7, instance_index: 7 }
        )
    );
}

#[test]
fn test_instance_index_of_row() {
    let current = CurrentItem { row: 5, instance_index: 2 };
    assert_eq!(current.get_instance_index_opt(7), Some(4));
    assert_eq!(current.get_instance_index_opt(5), Some(2));
    assert_eq!(current.get_instance_index_opt(3), Some(0));
    assert_eq!(current.get_instance_index_opt(2), None);
    assert_eq!(current.get_instance_index(7), 4);
    assert_eq!(
        current.get_instance_index(2),
        usize::MAX,
        "We are wrapping around. In real this should never happen"
    );
}

#[test]
fn test_row_data_changed_after_insert_at_front() {
    let model = Rc::new(VecModel::from(vec![10, 20]));
    let repeater = repeater_for(&model);
    model.insert(0, 5);
    ensure_updated_window(repeater.as_ref());
    model.set_row_data(0, 6);
    model.set_row_data(2, 30);
    assert_eq!(state(&repeater).0, [Clean(6), Clean(10), Clean(30)]);
}

#[test]
fn test_measure_rows_after_clear_keeps_stored_index_in_sync() {
    let model = TestModel::new(100);
    let repeater = repeater_with_window(&model, 0..3, 0);
    let model_rc = ModelRc::from(model.clone());
    let mut ops = RustRepeaterOps {
        inner: &repeater.0.inner,
        typed: &RepeaterTypedOps::<SimpleItem> { init: &new_item, model: &model_rc },
        instance_generation: repeater.as_ref().data().project_ref().instance_generation,
    };
    // A ListView seeking to a distant row: it drops all instances,
    // creates the row, then the rows above it.
    let mut first_row = 0;
    let mut changed = false;
    ops.clear();
    measure_row(&mut ops, &mut first_row, 50, &mut changed); // Up to row 50 every row is set up
    measure_row(&mut ops, &mut first_row, 49, &mut changed);
    assert_eq!(first_row, 49);

    // Model notifications from a row's init code read the stored index during the layout pass.
    let (slots, item_index) = state(&repeater);
    assert_eq!(slots, [Clean(49), Clean(50)]);
    assert_eq!(
        item_index.row.checked_sub(item_index.instance_index),
        Some(49),
        "The stored index maps the first instance to row 49, got {item_index:?}"
    );
    assert_eq!(repeater.range(), 49..51);
}
