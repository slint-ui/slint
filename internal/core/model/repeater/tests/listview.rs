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
    let scroll_up_by = |scroll: f32| scroll_down_by(-scroll);

    // Scrolled by 3.5 rows, so row 3 and 6 are partially visible
    scroll_down_by(ITEM_HEIGHT * 7. / 2.);
    update();
    assert_eq!(state(&repeater).0, [Clean(3), Clean(4), Clean(5), Clean(6)]);
}

#[test]
fn test_listview_scrolled_snap_to_center() {
    let model = TestModel::new(10);
    let repeater = Box::pin(Repeater::<SimpleItem>::default());
    let model_rc = ModelRc::from(model.clone());
    repeater.set_model_binding(move || model_rc.clone());

    let flickable = Box::pin(Flickable::default());
    flickable.snap_mode.set(SnapMode::Center);
    let content_y = Flickable::FIELD_OFFSETS.content_y().apply_pin(flickable.as_ref());
    let listview_height = Box::pin(Property::new(LogicalLength::new((ITEM_HEIGHT * 4.) as Coord)));

    let update = || {
        repeater.as_ref().ensure_updated_listview(
            new_item,
            Some(flickable.as_ref()),
            None,
            None,
            content_y.as_ref(),
            LogicalLength::new(100 as Coord),
            listview_height.as_ref(),
        );
    };

    update();
    //----------------------
    //
    //
    // 0
    // 1
    // 2
    // ---------------------
    assert_eq!(state(&repeater).0, [Clean(0), Clean(1), Clean(2)]);
    model.insert(0, &[100]);
    update();
    //----------------------
    //
    // 100
    // 0
    // 1
    // 2
    // ---------------------
    assert_eq!(
        state(&repeater).0,
        [Clean(100), Clean(0), Clean(1), Clean(2)],
        "The item was prepended before the first item. Because of the center snapping the item will be shown just before the first"
    );
    assert_eq!(content_y.as_ref().get().get(), -ITEM_HEIGHT);

    let scroll_down_by = |scroll: f32| {
        let new_content_y = LogicalLength::new(content_y.as_ref().get().0 - scroll);
        content_y.as_ref().set(new_content_y);
    };
    let scroll_up_by = |scroll: f32| scroll_down_by(-scroll);

    //----------------------
    // 100
    // 0
    // 1
    // 2
    // 3
    // ---------------------
    scroll_down_by(ITEM_HEIGHT);
    update();
    assert_eq!(state(&repeater).0, [Clean(100), Clean(0), Clean(1), Clean(2), Clean(3)]);

    //----------------------
    //
    //
    // 100
    // 0
    // 1
    // ---------------------
    scroll_up_by(2. * ITEM_HEIGHT);
    update();
    assert_eq!(state(&repeater).0, [Clean(100), Clean(0), Clean(1)]);
}
