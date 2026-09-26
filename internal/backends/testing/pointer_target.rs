// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::ElementHandle;
use i_slint_core::item_tree::{
    ItemRc, ItemTreeRc, ItemVisitorVTable, ParentItemTraversalMode, TraversalOrder,
    VisitChildrenResult,
};
use i_slint_core::items::*;
use i_slint_core::lengths::{LogicalPoint, LogicalRect, LogicalVector};
use i_slint_core::window::{PopupWindowLocation, WindowInner, WindowKind};
use std::pin::Pin;

/// A read-only prediction for a left press at an element's transformed center.
/// Unknown item policies return `unsupported`; input filters and callbacks are never invoked.
#[derive(Clone, Debug)]
pub struct PointerTarget {
    /// One of ready, covered, clipped, disabled, busy, no-target, or unsupported.
    pub status: &'static str,
    /// The center in the native window's logical coordinate system.
    pub position: i_slint_core::api::LogicalPosition,
    /// A description of the observed obstruction or unsupported policy.
    pub detail: String,
    /// Whether an interactive Flickable ancestor can reveal the center.
    pub scrollable: bool,
}

fn parent(item: &ItemRc) -> Option<ItemRc> {
    item.parent_item(ParentItemTraversalMode::StopAtPopups)
}

fn local_point(item: &ItemRc, point: LogicalPoint) -> Option<LogicalPoint> {
    let origin = item.geometry().origin;
    let p = item.map_to_native_window(origin);
    let x = item.map_to_native_window(origin + LogicalVector::new(1., 0.)) - p;
    let y = item.map_to_native_window(origin + LogicalVector::new(0., 1.)) - p;
    let determinant = x.x * y.y - x.y * y.x;
    if !determinant.is_finite() || determinant.abs() < f32::EPSILON {
        return None;
    }
    let d = point - p;
    Some(LogicalPoint::new(
        (d.x * y.y - d.y * y.x) / determinant,
        (x.x * d.y - x.y * d.x) / determinant,
    ))
}

fn policy(item: &ItemRc) -> Option<bool> {
    if let Some(touch) = item.downcast::<TouchArea>() {
        return Some(touch.as_pin_ref().enabled());
    }
    if let Some(text) = item.downcast::<TextInput>() {
        return Some(text.as_pin_ref().enabled());
    }
    macro_rules! passive {
        ($($ty:ty),*) => { if $(item.downcast::<$ty>().is_some())||* { return Some(false); } };
    }
    passive!(
        Empty,
        Rectangle,
        BasicBorderRectangle,
        BorderRectangle,
        Clip,
        Opacity,
        Layer,
        Transform,
        WindowItem,
        ComplexText,
        SimpleText,
        StyledTextItem,
        ClippedImage,
        KeyBinding,
        ImageItem,
        FocusScope,
        BoxShadow,
        TooltipArea,
        DropArea,
        ComponentContainer
    );
    #[cfg(not(target_os = "android"))]
    passive!(ContextMenu);
    if let Some(flick) = item.downcast::<Flickable>() {
        // A mouse press can be delayed before reaching a child; its final target is queried below.
        return Some(flick.as_pin_ref().accepts_mouse_press(item));
    }
    None
}

fn hit(
    item: ItemRc,
    point: LogicalPoint,
    transforms: bool,
) -> Option<Result<ItemRc, (&'static str, String)>> {
    let geometry = item.geometry();
    let inside = geometry.contains(point);
    let mut local = point - geometry.origin.to_vector();
    if let Some(transform) = item.children_transform() {
        if !transforms {
            return Some(Err(("unsupported", "Renderer does not support this transform".into())));
        }
        let Some(inverse) = transform.inverse() else {
            return Some(Err(("unsupported", "Non-invertible transform".into())));
        };
        local = inverse.transform_point(local.cast()).cast();
    }
    if item.borrow().as_ref().clips_children()
        && !LogicalRect::new(Default::default(), geometry.size).contains(local)
    {
        return None;
    }
    let accepts = if inside || item.borrow().as_ref().clips_children() {
        if item.downcast::<Flickable>().is_some_and(|f| f.as_pin_ref().captures_mouse_press()) {
            return Some(Err(("busy", "A Flickable is capturing a scroll gesture".into())));
        }
        match policy(&item) {
            Some(p) => p,
            None => {
                let handle = ElementHandle { item: item.downgrade(), element_index: 0 };
                return Some(Err((
                    "unsupported",
                    format!(
                        "{} has no read-only pointer policy",
                        handle.type_name().unwrap_or_else(|| "Item".into())
                    ),
                )));
            }
        }
    } else {
        false
    };
    let mut result = None;
    let mut visitor = |tree: &ItemTreeRc, index: u32, _: Pin<ItemRef>| {
        result = hit(ItemRc::new(tree.clone(), index), local, transforms);
        if result.is_some() {
            VisitChildrenResult::abort(index, 0)
        } else {
            VisitChildrenResult::CONTINUE
        }
    };
    vtable::new_vref!(let mut visitor: VRefMut<ItemVisitorVTable> for i_slint_core::item_tree::ItemVisitor = &mut visitor);
    ItemTreeRc::borrow_pin(item.item_tree()).as_ref().visit_children_item(
        item.index() as isize,
        TraversalOrder::FrontToBack,
        visitor,
    );
    result.or_else(|| accepts.then_some(Ok(item)))
}

impl ElementHandle {
    /// Inspect a left press without changing pointer, hover, focus, or scroll state.
    /// The prediction covers built-in input items and fails closed for unknown policies.
    pub fn pointer_target(&self) -> Result<PointerTarget, String> {
        let item = self.item.upgrade().ok_or("Stale element")?;
        let adapter = self.window_adapter().ok_or("Element has no window")?;
        let window = WindowInner::from_pub(adapter.window());
        window.ensure_tree_instantiated();
        let position = self.absolute_center();
        let point = LogicalPoint::new(position.x, position.y);
        let mut result =
            PointerTarget { status: "ready", position, detail: String::new(), scrollable: false };
        let mut ancestors = Vec::new();
        let mut current = Some(item.clone());
        while let Some(ancestor) = current {
            current = parent(&ancestor);
            if ancestor.downcast::<Flickable>().is_some_and(|f| f.as_pin_ref().interactive()) {
                result.scrollable = true;
            }
            ancestors.push(ancestor);
        }
        let mut fail = |status, detail: &str| {
            result.status = status;
            result.detail = detail.into();
            result.clone()
        };
        if window.has_pointer_operation() {
            return Ok(fail("busy", "A pointer gesture is already active"));
        }
        if self.accessible_enabled() == Some(false) || self.computed_opacity() <= 0. {
            return Ok(fail("disabled", "Target is disabled or invisible"));
        }
        for ancestor in &ancestors {
            if ancestor.children_transform().is_some()
                && !adapter.renderer().supports_transformations()
            {
                return Ok(fail("unsupported", "Renderer does not support this transform"));
            }
            let Some(local) = local_point(ancestor, point) else {
                return Ok(fail("unsupported", "Non-invertible transform"));
            };
            if ancestor.borrow().as_ref().clips_children()
                && !LogicalRect::new(Default::default(), ancestor.geometry().size).contains(local)
            {
                return Ok(fail("clipped", "Target center is outside an ancestor's viewport"));
            }
        }
        let mut root = ItemRc::new_root(window.component());
        let mut offset = LogicalPoint::default();
        for popup in window.active_popups().iter().rev() {
            if matches!(popup.window_kind, WindowKind::ToolTip) {
                continue;
            }
            match &popup.location {
                PopupWindowLocation::ChildWindow(origin) => {
                    root = ItemRc::new_root(popup.component.clone());
                    offset = *origin;
                    if !root.geometry().contains(point - offset.to_vector()) {
                        return Ok(fail("covered", "An open popup intercepts the target"));
                    }
                    break;
                }
                PopupWindowLocation::TopLevel(_) => {
                    return Ok(fail("unsupported", "Inspect the native popup window separately"));
                }
            }
        }
        let point_in_root = point - offset.to_vector();
        if !root.geometry().contains(point_in_root) {
            return Ok(fail("clipped", "Target center is outside the window"));
        }
        match hit(root, point_in_root, adapter.renderer().supports_transformations()) {
            Some(Ok(receiver)) => {
                let label = ElementHandle { item: receiver.downgrade(), element_index: 0 };
                let mut current = Some(receiver);
                while let Some(candidate) = current {
                    if candidate == item {
                        return Ok(result);
                    }
                    current = parent(&candidate);
                }
                Ok(fail(
                    "covered",
                    &format!(
                        "Covered by {}",
                        label
                            .accessible_label()
                            .or_else(|| label.id())
                            .unwrap_or_else(|| "another input item".into())
                    ),
                ))
            }
            Some(Err((status, detail))) => Ok(fail(status, &detail)),
            None => Ok(fail("no-target", "No input item receives a press at the target center")),
        }
    }

    /// Check the target again after hover callbacks, then perform one left click.
    /// A false second return value means no press was sent.
    pub async fn checked_click(&self) -> Result<(PointerTarget, bool), String> {
        use i_slint_core::platform::WindowEvent;
        let before = self.pointer_target()?;
        if before.status != "ready" {
            return Ok((before, false));
        }
        let adapter = self.window_adapter().ok_or("Element has no window")?;
        let window = adapter.window();
        window.dispatch_event(WindowEvent::PointerMoved { position: before.position });
        let after_hover = self.pointer_target()?;
        if after_hover.status != "ready" || after_hover.position != before.position {
            return Ok((after_hover, false));
        }
        window.dispatch_event(WindowEvent::PointerPressed {
            position: before.position,
            button: PointerEventButton::Left,
        });
        super::wait_for(std::time::Duration::from_millis(50)).await;
        window.dispatch_event(WindowEvent::PointerReleased {
            position: before.position,
            button: PointerEventButton::Left,
        });
        Ok((after_hover, true))
    }

    /// Reveal the center using interactive Flickable ancestors, then inspect its target.
    /// This changes scrolling; it doesn't send pointer events or activate the element.
    pub fn scroll_into_view(&self) -> Result<PointerTarget, String> {
        let before = self.pointer_target()?;
        if before.status == "clipped" && before.scrollable {
            self.item.upgrade().ok_or("Stale element")?.scroll_center_into_view();
        }
        self.pointer_target()
    }
}
