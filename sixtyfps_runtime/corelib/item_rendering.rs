/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#![warn(missing_docs)]
//! module for rendering the tree of items

use super::graphics::{GraphicsBackend, GraphicsWindow};
use super::items::ItemRef;
use crate::component::ComponentRc;
use crate::eventloop::ComponentWindow;
use crate::item_tree::ItemVisitorResult;
use crate::slice::Slice;
use std::cell::{Cell, RefCell};

/// This structure must be present in items that are Rendered and contains information.
/// Used by the backend.
#[derive(Default, Debug)]
#[repr(C)]
pub struct CachedRenderingData {
    /// Used and modified by the backend, should be initialized to 0 by the user code
    pub(crate) cache_index: Cell<usize>,
    /// Set to false initially and when changes happen that require updating the cache
    pub(crate) cache_ok: Cell<bool>,
}

pub(crate) fn render_component_items<Backend: GraphicsBackend>(
    component: &ComponentRc,
    renderer: &mut Backend::ItemRenderer,
    window: &std::rc::Rc<GraphicsWindow<Backend>>,
    origin: crate::graphics::Point,
) {
    use crate::items::ItemRenderer;
    let window = ComponentWindow::new(window.clone());

    let renderer = RefCell::new(renderer);

    crate::item_tree::visit_items_with_post_visit(
        component,
        crate::item_tree::TraversalOrder::BackToFront,
        |_, item, _, translation| {
            let saved_clip = renderer.borrow_mut().clip_rects();

            item.as_ref().render(
                *translation,
                &mut (*renderer.borrow_mut() as &mut dyn crate::items::ItemRenderer),
            );

            let origin = item.as_ref().geometry().origin;
            let translation = *translation + euclid::Vector2D::new(origin.x, origin.y);

            (ItemVisitorResult::Continue(translation), saved_clip)
        },
        |_, _, saved_clip| {
            renderer.borrow_mut().reset_clip(saved_clip);
        },
        origin,
    );
}

pub(crate) fn free_item_rendering_data<'a, Backend: GraphicsBackend>(
    items: &Slice<'a, core::pin::Pin<ItemRef<'a>>>,
    _renderer: &mut Backend,
) {
    for item in items.iter() {
        // TODO
    }
}
