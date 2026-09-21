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

/// The URI list naming `paths`, or `None` if one of them has no URI form: a partial list
/// would offer files that the drag doesn't carry.
///
/// A relative path names the same file it would name when opened, the one in the current
/// directory, since a URI can only name a file absolutely.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn file_paths_to_uris<'a>(
    paths: impl IntoIterator<Item = &'a std::path::Path>,
) -> Option<winit::data_transfer::SendData> {
    let uris = paths
        .into_iter()
        .map(|path| {
            url::Url::from_file_path(std::path::absolute(path).ok()?).ok().map(String::from)
        })
        .collect::<Option<Vec<_>>>()?;
    (!uris.is_empty()).then_some(winit::data_transfer::SendData::Uris(uris))
}
#[cfg(target_arch = "wasm32")]
pub(crate) fn file_paths_to_uris<'a>(
    _paths: impl IntoIterator<Item = &'a std::path::Path>,
) -> Option<winit::data_transfer::SendData> {
    None
}

/// The local files an incoming URI list names, or `None` if one of the URIs names none:
/// a partial list would claim files that the drag doesn't carry.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn dropped_file_paths(uris: &[String]) -> Option<Vec<std::path::PathBuf>> {
    let paths = uris
        .iter()
        .map(|uri| url::Url::parse(uri).ok()?.to_file_path().ok())
        .collect::<Option<Vec<_>>>()?;
    (!paths.is_empty()).then_some(paths)
}
#[cfg(target_arch = "wasm32")]
pub(crate) fn dropped_file_paths(_uris: &[String]) -> Option<Vec<std::path::PathBuf>> {
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

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn uris(paths: &[&str]) -> Option<winit::data_transfer::SendData> {
        file_paths_to_uris(paths.iter().map(Path::new))
    }

    #[test]
    fn a_relative_path_names_the_file_in_the_current_directory() {
        let in_current_dir = std::env::current_dir().unwrap().join("a.txt");
        let expected = String::from(url::Url::from_file_path(&in_current_dir).unwrap());
        assert_eq!(uris(&["a.txt"]), Some(winit::data_transfer::SendData::Uris(vec![expected])));
    }

    #[test]
    fn the_paths_survive_the_round_trip() {
        // Built from the current directory, so that they are absolute on every platform.
        let here = std::env::current_dir().unwrap();
        let paths = [here.join("a.txt"), here.join("b c.png")];
        let winit::data_transfer::SendData::Uris(sent) =
            file_paths_to_uris(paths.iter().map(PathBuf::as_path)).unwrap()
        else {
            panic!("file paths are sent as a URI list");
        };
        assert_eq!(dropped_file_paths(&sent), Some(paths.to_vec()));
    }

    #[test]
    fn nothing_is_offered_and_nothing_is_taken_without_a_path() {
        assert_eq!(uris(&[]), None);
        assert_eq!(dropped_file_paths(&[]), None);
    }

    #[test]
    fn a_list_that_mixes_files_with_other_uris_is_refused() {
        let here = std::env::current_dir().unwrap();
        let file = String::from(url::Url::from_file_path(here.join("a.txt")).unwrap());
        assert!(dropped_file_paths(std::slice::from_ref(&file)).is_some());
        assert_eq!(dropped_file_paths(&[file, String::from("https://slint.dev")]), None);
    }
}
