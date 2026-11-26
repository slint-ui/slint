// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;

#[test]
fn test_listview_scroll() {
    let model = TestModel::new(100000);
    let repeater = Box::pin(Repeater::<SimpleItem>::default());
    let model_rc = ModelRc::from(model.clone());
    repeater.set_model_binding(move || model_rc.clone());
    let content_y = Box::pin(Property::new(LogicalLength::default()));
    let listview_height = Box::pin(Property::new(LogicalLength::new((ITEM_HEIGHT * 3.) as Coord)));
    let update_listview = || {
        repeater.as_ref().ensure_updated_listview(
            new_item,
            None,
            None,
            content_y.as_ref(),
            LogicalLength::new(100 as Coord),
            listview_height.as_ref(),
        );
    };

    update_listview();
    assert_eq!(state(&repeater).0, [Clean(0), Clean(1), Clean(2)]);

    let scroll_down_by = |scroll: f32| {
        let new_content_y = LogicalLength::new(content_y.as_ref().get().0 - scroll);
        content_y.as_ref().set(new_content_y);
    };
    let scroll_up_by = |scroll: f32| scroll_down_by(-scroll);

    scroll_down_by(5. * ITEM_HEIGHT);
    update_listview();
    // Scrolled by 5 rows: only 5 to 7 are kept
    assert_eq!(state(&repeater).0, [Clean(5), Clean(6), Clean(7)]);
    assert_eq!(state(&repeater).1.row, 5, "We scrolled to row 6 (index 5)");

    scroll_up_by(2. * ITEM_HEIGHT);
    update_listview();
    assert_eq!(state(&repeater).0, [Clean(3), Clean(4), Clean(5)]);

    scroll_down_by(0.5 * ITEM_HEIGHT);
    update_listview();
    assert_eq!(state(&repeater).0, [Clean(3), Clean(4), Clean(5), Clean(6)]);

    scroll_down_by(0.5 * ITEM_HEIGHT);
    update_listview();
    assert_eq!(state(&repeater).0, [Clean(4), Clean(5), Clean(6)]);
}

#[test]
fn test_listview_scrolled_after_insert_at_front() {
    let model = TestModel::new(10);
    let repeater = Box::pin(Repeater::<SimpleItem>::default());
    let model_rc = ModelRc::from(model.clone());
    repeater.set_model_binding(move || model_rc.clone());
    let content_y = Box::pin(Property::new(LogicalLength::default()));
    let listview_height = Box::pin(Property::new(LogicalLength::new((ITEM_HEIGHT * 3.) as Coord)));
    let update = || {
        repeater.as_ref().ensure_updated_listview(
            new_item,
            None,
            None,
            content_y.as_ref(),
            LogicalLength::new(100 as Coord),
            listview_height.as_ref(),
        );
    };

    update();
    assert_eq!(state(&repeater).0, [Clean(0), Clean(1), Clean(2)]);
    model.insert(0, &[100]);
    update();
    assert_eq!(
        state(&repeater).0,
        [Clean(0), Clean(1), Clean(2)],
        "The item was prepended before the first item. So we don't wanna move the view"
    );
    assert_eq!(content_y.as_ref().get().get(), -ITEM_HEIGHT, "The new row 0 is above the view");

    let scroll_down_by = |scroll: f32| {
        let new_content_y = LogicalLength::new(content_y.as_ref().get().0 - scroll);
        content_y.as_ref().set(new_content_y);
    };
    // Scrolled by 3.5 rows, so row 3 and 6 are partially visible
    scroll_down_by(ITEM_HEIGHT * 7. / 2.);
    update();
    assert_eq!(state(&repeater).0, [Clean(3), Clean(4), Clean(5), Clean(6)]);
}

#[test]
fn test_listview_scrolled_to_end_after_remove_at_end() {
    let model = TestModel::new(10);
    let repeater = Box::pin(Repeater::<SimpleItem>::default());
    let model_rc = ModelRc::from(model.clone());
    repeater.set_model_binding(move || model_rc.clone());
    let content_y = Box::pin(Property::new(LogicalLength::default()));
    let listview_height = Box::pin(Property::new(LogicalLength::new((ITEM_HEIGHT * 3.) as Coord)));
    let update = || {
        repeater.as_ref().ensure_updated_listview(
            new_item,
            None,
            None,
            content_y.as_ref(),
            LogicalLength::new(100 as Coord),
            listview_height.as_ref(),
        );
    };

    update();
    content_y.as_ref().set(LogicalLength::new(-7. * ITEM_HEIGHT));
    update();
    assert_eq!(state(&repeater).0, [Clean(7), Clean(8), Clean(9)]);

    model.remove(8..10);
    update();
    assert_eq!(
        state(&repeater).0,
        [Clean(5), Clean(6), Clean(7)],
        "The last rows fill the view instead of leaving a gap below row 7"
    );
    assert_eq!(content_y.as_ref().get().get(), -5. * ITEM_HEIGHT);
}
