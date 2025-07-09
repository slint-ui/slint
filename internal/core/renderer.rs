// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::pin::Pin;

use crate::api::PlatformError;
use crate::graphics::{Rgba8Pixel, SharedPixelBuffer};
use crate::item_tree::ItemTreeRef;
use crate::items::TextWrap;
use crate::lengths::{LogicalLength, LogicalPoint, LogicalRect, LogicalSize, ScaleFactor};
use crate::window::WindowAdapter;

/// This trait represents a Renderer that can render a slint scene.
///
/// This trait is [sealed](https://rust-lang.github.io/api-guidelines/future-proofing.html#sealed-traits-protect-against-downstream-implementations-c-sealed),
/// meaning that you are not expected to implement this trait
/// yourself, but you should use the provided one from Slint such as
/// [`SoftwareRenderer`](crate::software_renderer::SoftwareRenderer)
pub trait Renderer: RendererSealed {}
impl<T: RendererSealed> Renderer for T {}

/// Implementation details behind [`Renderer`], but since this
/// trait is not exported in the public API, it is not possible for the
/// users to re-implement these functions.
pub trait RendererSealed {
    /// Returns the size of the given text in logical pixels.
    /// When set, `max_width` means that one need to wrap the text, so it does not go further than that,
    /// using the wrapping type passed by `text_wrap`.
    fn text_size(
        &self,
        font_request: crate::graphics::FontRequest,
        text: &str,
        max_width: Option<LogicalLength>,
        scale_factor: ScaleFactor,
        text_wrap: TextWrap,
    ) -> LogicalSize;

    /// Returns the metrics of the given font.
    fn font_metrics(
        &self,
        font_request: crate::graphics::FontRequest,
        scale_factor: ScaleFactor,
    ) -> crate::items::FontMetrics;

    /// Returns the (UTF-8) byte offset in the text property that refers to the character that contributed to
    /// the glyph cluster that's visually nearest to the given coordinate. This is used for hit-testing,
    /// for example when receiving a mouse click into a text field. Then this function returns the "cursor"
    /// position.
    fn text_input_byte_offset_for_position(
        &self,
        text_input: Pin<&crate::items::TextInput>,
        pos: LogicalPoint,
        font_request: crate::graphics::FontRequest,
        scale_factor: ScaleFactor,
    ) -> usize;

    /// That's the opposite of [`Self::text_input_byte_offset_for_position`]
    /// It takes a (UTF-8) byte offset in the text property, and returns a Rectangle
    /// left to the char. It is one logical pixel wide and ends at the baseline.
    fn text_input_cursor_rect_for_byte_offset(
        &self,
        text_input: Pin<&crate::items::TextInput>,
        byte_offset: usize,
        font_request: crate::graphics::FontRequest,
        scale_factor: ScaleFactor,
    ) -> LogicalRect;

    /// Clear the caches for the items that are being removed
    fn free_graphics_resources(
        &self,
        _component: ItemTreeRef,
        _items: &mut dyn Iterator<Item = Pin<crate::items::ItemRef<'_>>>,
    ) -> Result<(), crate::platform::PlatformError> {
        Ok(())
    }

    /// Mark a given region as dirty regardless whether the items actually are dirty.
    ///
    /// Example: when a PopupWindow disappears, the region under the popup needs to be redrawn
    fn mark_dirty_region(&self, _region: crate::item_rendering::DirtyRegion) {}

    #[cfg(feature = "std")] // FIXME: just because of the Error
    /// This function can be used to register a custom TrueType font with Slint,
    /// for use with the `font-family` property. The provided slice must be a valid TrueType
    /// font.
    fn register_font_from_memory(
        &self,
        _data: &'static [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        Err("This renderer does not support registering custom fonts.".into())
    }

    #[cfg(feature = "std")]
    /// This function can be used to register a custom TrueType font with Slint,
    /// for use with the `font-family` property. The provided path must refer to a valid TrueType
    /// font.
    fn register_font_from_path(
        &self,
        _path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Err("This renderer does not support registering custom fonts.".into())
    }

    fn register_bitmap_font(&self, _font_data: &'static crate::graphics::BitmapFont) {
        crate::debug_log!("Internal error: The current renderer cannot load fonts build with the `EmbedForSoftwareRenderer` option. Please use the software Renderer, or disable that option when building your slint files");
    }

    /// This function is called through the public API to register a callback that the backend needs to invoke during
    /// different phases of rendering.
    fn set_rendering_notifier(
        &self,
        _callback: Box<dyn crate::api::RenderingNotifier>,
    ) -> Result<(), crate::api::SetRenderingNotifierError> {
        Err(crate::api::SetRenderingNotifierError::Unsupported)
    }

    fn set_window_adapter(&self, _window_adapter: &Rc<dyn WindowAdapter>);

    fn resize(&self, _size: crate::api::PhysicalSize) -> Result<(), PlatformError> {
        Ok(())
    }

    /// Re-implement this function to support Window::take_snapshot(), i.e. return
    /// the contents of the window in an image buffer.
    fn take_snapshot(&self) -> Result<SharedPixelBuffer<Rgba8Pixel>, PlatformError> {
        Err("WindowAdapter::take_snapshot is not implemented by the platform".into())
    }
}
