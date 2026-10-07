// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore parsererror

use alloc::rc::Rc;
use alloc::string::String;

use super::ImageCacheKey;
use crate::Property;
use crate::graphics::IntSize;

pub struct HTMLImage {
    pub dom_element: web_sys::HtmlImageElement,
    /// If present, this boolean property indicates whether the image has been uploaded yet or
    /// if that operation is still pending. If not present, then the image *is* available. This is
    /// used for remote HTML image loading and the property will be used to correctly track dependencies
    /// to graphics items that query for the size.
    image_load_pending: core::pin::Pin<Rc<Property<bool>>>,
    is_svg: bool,
    /// The size extracted from the SVG source's width/height/viewBox attributes.
    /// The browser reports no useful natural size for SVGs without intrinsic dimensions,
    /// so this takes precedence over naturalWidth/naturalHeight.
    svg_intrinsic_size: Option<IntSize>,
    /// The blob URL to revoke on drop when the image was created from in-memory data.
    object_url: Option<String>,
}

impl HTMLImage {
    pub fn new(url: &str) -> Self {
        let is_svg = url.ends_with(".svg") || url.ends_with(".svgz");
        Self::new_impl(url, is_svg, None)
    }

    /// Creates an image from in-memory encoded data,
    /// by handing the data to the browser as a blob URL.
    /// `mime_type` selects the browser's decoder, for example `image/png` or `image/svg+xml`.
    pub fn new_from_data(data: &[u8], mime_type: &str) -> Option<Self> {
        let blob_parts = js_sys::Array::new();
        blob_parts.push(&js_sys::Uint8Array::from(data));
        let options = web_sys::BlobPropertyBag::new();
        options.set_type(mime_type);
        let blob =
            web_sys::Blob::new_with_u8_array_sequence_and_options(&blob_parts, &options).ok()?;
        let url = web_sys::Url::create_object_url_with_blob(&blob).ok()?;

        let is_svg = mime_type == "image/svg+xml";
        let svg_intrinsic_size = if is_svg {
            core::str::from_utf8(data).ok().and_then(svg_intrinsic_size)
        } else {
            None
        };

        let mut image = Self::new_impl(&url, is_svg, svg_intrinsic_size);
        image.object_url = Some(url);
        Some(image)
    }

    fn new_impl(url: &str, is_svg: bool, svg_intrinsic_size: Option<IntSize>) -> Self {
        let dom_element = web_sys::HtmlImageElement::new().unwrap();

        let image_load_pending = Rc::pin(Property::new(true));

        // Setting crossOrigin on blob: or data: URLs can taint the WebGL canvas.
        if url.starts_with("http:") || url.starts_with("https:") {
            dom_element.set_cross_origin(Some("anonymous"));
        }
        dom_element.set_onload(Some(
            &wasm_bindgen::closure::Closure::once_into_js({
                let image_load_pending = image_load_pending.clone();
                move || {
                    image_load_pending.as_ref().set(false);

                    // As you can paint on a HTML canvas at any point in time, request_redraw()
                    // on a winit window only queues an additional internal event, that'll be
                    // be dispatched as the next event. We are however not in an event loop
                    // call, so we also need to wake up the event loop and redraw then.
                    let _ = crate::api::invoke_from_event_loop(|| {});
                }
            })
            .into(),
        ));
        // The renderer reads the element's width/height attributes when turning the image
        // into a texture, so seed them with the intrinsic size.
        if let Some(size) = svg_intrinsic_size {
            dom_element.set_width(size.width);
            dom_element.set_height(size.height);
        }
        dom_element.set_src(url);

        Self { dom_element, image_load_pending, is_svg, svg_intrinsic_size, object_url: None }
    }

    /// Returns true once the browser finished loading the image and it can be turned into a texture.
    /// Reading this registers a dependency, so a caller within a property binding is re-evaluated
    /// when loading completes.
    pub fn is_loaded(&self) -> bool {
        !self.image_load_pending.as_ref().get()
    }

    pub fn size(&self) -> Option<IntSize> {
        if let Some(size) = self.svg_intrinsic_size {
            return Some(size);
        }
        match self.image_load_pending.as_ref().get() {
            true => None,
            false => Some(IntSize::new(
                self.dom_element.natural_width(),
                self.dom_element.natural_height(),
            )),
        }
    }

    pub fn source(&self) -> alloc::string::String {
        self.dom_element.src()
    }

    pub fn is_svg(&self) -> bool {
        self.is_svg
    }
}

impl Drop for HTMLImage {
    fn drop(&mut self) {
        if let Some(url) = &self.object_url {
            let _ = web_sys::Url::revoke_object_url(url);
        }
    }
}

impl super::OpaqueImage for HTMLImage {
    fn size(&self) -> IntSize {
        self.size().unwrap_or_default()
    }
    fn cache_key(&self) -> ImageCacheKey {
        ImageCacheKey::URL(self.source().into())
    }
}

/// Extracts the intrinsic size of an SVG from its width/height/viewBox attributes,
/// the same way the native (resvg) code path determines it.
fn svg_intrinsic_size(svg_source: &str) -> Option<IntSize> {
    let document = web_sys::DomParser::new()
        .ok()?
        .parse_from_string(svg_source, web_sys::SupportedType::ImageSvgXml)
        .ok()?;
    let root = document.document_element()?;
    // Parse errors produce a <parsererror> root element.
    if !root.tag_name().eq_ignore_ascii_case("svg") {
        return None;
    }
    let width = root.get_attribute("width").as_deref().and_then(parse_svg_length);
    let height = root.get_attribute("height").as_deref().and_then(parse_svg_length);
    let view_box = root.get_attribute("viewBox").as_deref().and_then(parse_view_box);
    let (width, height) = match (width, height, view_box) {
        (Some(w), Some(h), _) => (w, h),
        (Some(w), None, Some((vw, vh))) => (w, w * vh / vw),
        (None, Some(h), Some((vw, vh))) => (h * vw / vh, h),
        (None, None, Some((vw, vh))) => (vw, vh),
        _ => return None,
    };
    (width.is_finite() && height.is_finite() && width > 0. && height > 0.)
        .then(|| IntSize::new(width.round().max(1.) as u32, height.round().max(1.) as u32))
}

/// Parses an SVG length attribute into CSS pixels.
/// Percentages and font-relative units have no absolute value and yield None.
fn parse_svg_length(value: &str) -> Option<f64> {
    let value = value.trim();
    let (number, factor) = if let Some(n) = value.strip_suffix("px") {
        (n, 1.)
    } else if let Some(n) = value.strip_suffix("pt") {
        (n, 96. / 72.)
    } else if let Some(n) = value.strip_suffix("pc") {
        (n, 16.)
    } else if let Some(n) = value.strip_suffix("mm") {
        (n, 96. / 25.4)
    } else if let Some(n) = value.strip_suffix("cm") {
        (n, 96. / 2.54)
    } else if let Some(n) = value.strip_suffix("in") {
        (n, 96.)
    } else if value.ends_with(|c: char| c.is_ascii_digit() || c == '.') {
        (value, 1.)
    } else {
        return None;
    };
    number.trim().parse::<f64>().ok().map(|n| n * factor)
}

/// Returns the (width, height) of a viewBox attribute value.
fn parse_view_box(value: &str) -> Option<(f64, f64)> {
    let mut parts =
        value.split(|c: char| c.is_ascii_whitespace() || c == ',').filter(|p| !p.is_empty());
    let _min_x = parts.next()?;
    let _min_y = parts.next()?;
    let width = parts.next()?.parse::<f64>().ok()?;
    let height = parts.next()?.parse::<f64>().ok()?;
    Some((width, height))
}
