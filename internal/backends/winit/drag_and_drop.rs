// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The payload of an outgoing drag, and the conversions between winit's and Slint's
//! representation of a drag.

use corelib::window::DragRequest;
use i_slint_core as corelib;

/// A native drag built by `WinitWindowAdapter::start_drag`, ready for the event loop to hand
/// to `ActiveEventLoop::start_drag` (which is only reachable from inside the event handler).
pub(crate) struct PendingNativeDrag {
    pub(crate) window_id: winit::window::WindowId,
    pub(crate) data: Box<dyn winit::data_transfer::DataTransferSend>,
    /// Allowed actions, ordered by preference, as Wayland and macOS expect.
    pub(crate) actions: Vec<winit::event_loop::DndAction>,
    /// The image shown under the cursor while dragging, if any.
    pub(crate) icon: Option<winit::event_loop::DragIcon>,
}

/// The image of an outgoing drag, rendered to RGBA pixels ready for PNG encoding.
/// Not available on wasm, where winit has no drag-and-drop and the PNG encoder
/// would only grow the binary.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn drag_image_payload(
    request: &DragRequest,
) -> Option<corelib::graphics::SharedPixelBuffer<corelib::graphics::Rgba8Pixel>> {
    request.data().image().ok()?.to_rgba8()
}
#[cfg(target_arch = "wasm32")]
pub(crate) fn drag_image_payload(
    _request: &DragRequest,
) -> Option<corelib::graphics::SharedPixelBuffer<corelib::graphics::Rgba8Pixel>> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn encode_png(
    pixel_buffer: &corelib::graphics::SharedPixelBuffer<corelib::graphics::Rgba8Pixel>,
) -> Option<Vec<u8>> {
    let mut png = Vec::new();
    image::ImageEncoder::write_image(
        image::codecs::png::PngEncoder::new(&mut png),
        pixel_buffer.as_bytes(),
        pixel_buffer.width(),
        pixel_buffer.height(),
        image::ExtendedColorType::Rgba8,
    )
    .ok()?;
    Some(png)
}
#[cfg(target_arch = "wasm32")]
pub(crate) fn encode_png(
    _pixel_buffer: &corelib::graphics::SharedPixelBuffer<corelib::graphics::Rgba8Pixel>,
) -> Option<Vec<u8>> {
    None
}

/// Decode the encoded image bytes of an incoming drag, without going through the
/// image cache.
pub(crate) fn decode_dropped_image(
    bytes: &[u8],
    extension_hint: Option<&str>,
) -> Option<corelib::graphics::Image> {
    corelib::graphics::load_image_from_dynamic_data(
        bytes.into(),
        extension_hint.unwrap_or_default().as_bytes().into(),
    )
}

/// Map a winit drag action to Slint's `DragAction`. A `None` (e.g. unknown) action becomes
/// `DragAction::None`.
pub(crate) fn dnd_action_to_slint(
    action: Option<winit::event_loop::DndAction>,
) -> corelib::items::DragAction {
    use corelib::items::DragAction;
    use winit::event_loop::DndAction;
    match action {
        Some(DndAction::Move) => DragAction::Move,
        Some(DndAction::Copy) => DragAction::Copy,
        Some(DndAction::Link) => DragAction::Link,
        Some(DndAction::Ask) | Some(DndAction::Private) | None => DragAction::None,
        Some(_) => DragAction::None,
    }
}

/// The action proposed by the OS for an incoming drag, defaulting to `Copy` when the platform
/// did not supply one (some platforms, such as X11, only report the action when the drop
/// completes).
pub(crate) fn proposed_action_or_copy(
    action: Option<winit::event_loop::DndAction>,
) -> corelib::items::DragAction {
    let action = dnd_action_to_slint(action);
    if action == corelib::items::DragAction::None {
        corelib::items::DragAction::Copy
    } else {
        action
    }
}

/// Map a `DropArea`'s chosen action to the single valid winit drag action to report to the OS,
/// or `None` to reject the drag. Returned as an `Option` so the per-event report on the drag
/// hot path needs no allocation.
pub(crate) fn slint_action_to_dnd(
    action: corelib::items::DragAction,
) -> Option<winit::event_loop::DndAction> {
    use corelib::items::DragAction;
    match action {
        DragAction::Move => Some(winit::event_loop::DndAction::Move),
        DragAction::Copy => Some(winit::event_loop::DndAction::Copy),
        DragAction::Link => Some(winit::event_loop::DndAction::Link),
        DragAction::None => None,
        // `DragAction` is `#[non_exhaustive]`, so a catch-all is still required.
        #[cfg_attr(slint_nightly_test, allow(non_exhaustive_omitted_patterns))]
        _ => None,
    }
}
