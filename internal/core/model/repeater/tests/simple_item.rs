// Simple Item used in the tests for mocking
use super::*;

#[derive(Default)]
pub(super) struct SimpleItem {
    pub(super) value: crate::model::Cell<i32>,
    pub(super) instantiated: bool,
}

impl SimpleItem {
    pub(super) fn new(data: i32) -> VRc<ItemTreeVTable, Self> {
        let mut _self = Self::default();
        _self.value.set(data);
        let self_rc = VRc::new(_self);
        self_rc
    }
}

crate::item_tree::ItemTreeVTable_static!(static TEST_COMPONENT_VT for SimpleItem);
impl RepeatedItemTree for SimpleItem {
    type Data = i32;

    fn update(&self, _index: usize, data: Self::Data) {
        self.value.set(data);
    }

    fn listview_layout(self: Pin<&Self>, offset_y: &mut LogicalLength) -> LogicalLength {
        *offset_y += LogicalLength::new(ITEM_HEIGHT);
        LogicalLength::default()
    }
}

impl crate::items::Item for SimpleItem {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn deinit(self: Pin<&Self>, _window_adapter: &Rc<dyn WindowAdapter>) {}

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _cross_axis_constraint: Coord,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> LayoutInfo {
        unimplemented!("Not implemented");
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: &MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        _: &mut MouseCursorInner,
    ) -> InputEventFilterResult {
        unimplemented!("Not implemented");
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: &MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        _: &mut MouseCursorInner,
    ) -> InputEventResult {
        unimplemented!("Not implemented");
        InputEventResult::EventIgnored
    }

    fn capture_key_event(
        self: Pin<&Self>,
        _: &InternalKeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> KeyEventResult {
        unimplemented!("Not implemented");
        KeyEventResult::EventIgnored
    }

    fn key_event(
        self: Pin<&Self>,
        _: &InternalKeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> KeyEventResult {
        unimplemented!("Not implemented");
        KeyEventResult::EventIgnored
    }

    fn focus_event(
        self: Pin<&Self>,
        _: &FocusEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> FocusEventResult {
        unimplemented!("Not implemented");
        FocusEventResult::FocusIgnored
    }

    fn render(
        self: Pin<&Self>,
        backend: &mut ItemRendererRef,
        self_rc: &ItemRc,
        size: LogicalSize,
    ) -> RenderingResult {
        unimplemented!("Not implemented");
        RenderingResult::ContinueRenderingChildren
    }

    fn bounding_rect(
        self: core::pin::Pin<&Self>,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        geometry: LogicalRect,
    ) -> LogicalRect {
        unimplemented!("Not implemented");
        geometry
    }

    fn clips_children(self: core::pin::Pin<&Self>) -> bool {
        unimplemented!("Not implemented");
        false
    }
}

impl ItemTree for SimpleItem {
    fn visit_children_item(
        self: core::pin::Pin<&Self>,
        _1: isize,
        _2: crate::item_tree::TraversalOrder,
        _3: vtable::VRefMut<crate::item_tree::ItemVisitorVTable>,
    ) -> crate::item_tree::VisitChildrenResult {
        unimplemented!("Not needed for this test")
    }

    fn get_item_ref(
        self: core::pin::Pin<&Self>,
        index: u32,
    ) -> core::pin::Pin<vtable::VRef<'_, crate::items::ItemVTable>> {
        unimplemented!("Not needed for this test")
    }

    fn get_item_tree(self: core::pin::Pin<&Self>) -> Slice<'_, ItemTreeNode> {
        unimplemented!("Not implemented");
        Slice::default()
    }

    fn parent_node(self: core::pin::Pin<&Self>, result: &mut ItemWeak) {
        unimplemented!("Not implemented");
        *result = ItemWeak::default();
    }

    fn embed_component(
        self: core::pin::Pin<&Self>,
        _parent_component: &ItemTreeWeak,
        _item_tree_index: u32,
    ) -> bool {
        unimplemented!("Not implemented");
        false
    }

    fn ensure_instantiated(mut self: core::pin::Pin<&Self>) -> bool {
        false
    }

    fn layout_info(self: core::pin::Pin<&Self>, o: Orientation) -> LayoutInfo {
        // return LayoutInfo {
        //     max: wi.width.get_internal().0,
        //     max_percent: 100.,
        //     min: wi.width.get_internal().0,
        //     min_percent: 100.,
        //     preferred: wi.width.get_internal().0,
        //     stretch: 1.,
        // };
        unimplemented!("Not needed for this test")
    }

    fn subtree_index(self: core::pin::Pin<&Self>) -> usize {
        unimplemented!("Not implemented");
        0
    }

    fn get_subtree_range(self: core::pin::Pin<&Self>, subtree_index: u32) -> IndexRange {
        unimplemented!("Not implemented");
        IndexRange { start: 0, end: 0 }
    }

    fn get_subtree(
        self: core::pin::Pin<&Self>,
        subtree_index: u32,
        component_index: usize,
        result: &mut ItemTreeWeak,
    ) {
        unimplemented!("Not implemented");
    }

    fn accessible_role(self: Pin<&Self>, _: u32) -> AccessibleRole {
        unimplemented!("Not needed for this test")
    }

    fn accessible_string_property(
        self: Pin<&Self>,
        _: u32,
        _: AccessibleStringProperty,
        _: &mut crate::SharedString,
    ) -> bool {
        unimplemented!("Not implemented");
        false
    }

    fn item_element_infos(self: Pin<&Self>, _: u32, _: &mut SharedString) -> bool {
        unimplemented!("Not implemented");
        false
    }

    fn element_testable_properties(self: Pin<&Self>, _: u32, _: &mut SharedString) -> bool {
        unimplemented!("Not implemented");
    }

    fn element_testable_property_value(
        self: Pin<&Self>,
        _: u32,
        _: Slice<u8>,
        _: &mut SharedString,
    ) -> bool {
        unimplemented!("Not implemented");
    }

    fn slint_context(self: Pin<&Self>, _result: &mut Option<crate::SlintContext>) {
        unimplemented!("Not implemented");
    }

    fn window_adapter(self: Pin<&Self>, _do_create: bool, result: &mut Option<WindowAdapterRc>) {
        unimplemented!("Not implemented");
        *result = None;
    }

    fn item_geometry(self: Pin<&Self>, _: u32) -> crate::lengths::LogicalRect {
        crate::lengths::LogicalRect::new(
            euclid::Point2D::new(0 as Coord, 0 as Coord),
            euclid::Size2D::new(100 as Coord, ITEM_HEIGHT),
        )
    }

    fn accessibility_action(
        self: core::pin::Pin<&Self>,
        _: u32,
        _: &crate::accessibility::AccessibilityAction,
    ) {
        unimplemented!("Not needed for this test")
    }

    fn supported_accessibility_actions(
        self: core::pin::Pin<&Self>,
        _: u32,
    ) -> crate::accessibility::SupportedAccessibilityAction {
        unimplemented!("Not needed for this test")
    }
}
