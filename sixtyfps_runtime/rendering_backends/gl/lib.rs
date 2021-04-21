/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*!

*NOTE*: This library is an internal crate for the [SixtyFPS project](https://sixtyfps.io).
This crate should not be used directly by application using SixtyFPS.
You should use the `sixtyfps` crate instead.

*/
#![doc(html_logo_url = "https://sixtyfps.io/resources/logo.drawio.svg")]

use std::cell::RefCell;
use std::collections::HashMap;
use std::pin::Pin;
use std::rc::Rc;

use sixtyfps_corelib::graphics::{
    Brush, Color, FontMetrics, FontRequest, ImageReference, IntRect, Point, Rect, RenderingCache,
    Size,
};
use sixtyfps_corelib::item_rendering::{CachedRenderingData, ItemRenderer};
use sixtyfps_corelib::items::{
    FillRule, ImageFit, TextHorizontalAlignment, TextOverflow, TextVerticalAlignment, TextWrap,
};
use sixtyfps_corelib::properties::Property;
use sixtyfps_corelib::window::ComponentWindow;
use sixtyfps_corelib::SharedString;
use sixtyfps_corelib::SharedVector;

mod graphics_window;
use graphics_window::*;
pub(crate) mod eventloop;
mod images;
mod svg;
use images::*;

type CanvasRc = Rc<RefCell<femtovg::Canvas<femtovg::renderer::OpenGl>>>;

pub const DEFAULT_FONT_SIZE: f32 = 12.;
pub const DEFAULT_FONT_WEIGHT: i32 = 400; // CSS normal

#[derive(PartialEq, Eq, Hash, Debug)]
enum ImageCacheKey {
    Path(String),
    EmbeddedData(by_address::ByAddress<&'static [u8]>),
}

impl ImageCacheKey {
    fn new(resource: &ImageReference) -> Option<Self> {
        Some(match resource {
            ImageReference::None => return None,
            ImageReference::AbsoluteFilePath(path) => {
                if path.is_empty() {
                    return None;
                }
                Self::Path(path.to_string())
            }
            ImageReference::EmbeddedData(data) => {
                Self::EmbeddedData(by_address::ByAddress(data.as_slice()))
            }
            ImageReference::EmbeddedRgbaImage { .. } => return None,
        })
    }
}

#[derive(Clone)]
enum ItemGraphicsCacheEntry {
    Image(Rc<CachedImage>),
    ScalableImage {
        // This variant is used for SVG images, where `scalable_source` refers to the parsed
        // SVG tree here, just to keep it in the image cache.
        scalable_source: Rc<CachedImage>,
        scaled_image: Rc<CachedImage>,
    },
    ColorizedImage {
        // This original image Rc is kept here to keep the image in the shared image cache, so that
        // changes to the colorization brush will not require re-uploading the image.
        original_image: Rc<CachedImage>,
        colorized_image: Rc<CachedImage>,
    },
    // The font selection is expensive because it is also based on the concrete rendered text, so this is cached here to speed up re-paints
    Font(GLFont),
}

impl ItemGraphicsCacheEntry {
    fn as_image(&self) -> &Rc<CachedImage> {
        match self {
            ItemGraphicsCacheEntry::Image(image) => image,
            ItemGraphicsCacheEntry::ColorizedImage { colorized_image, .. } => colorized_image,
            ItemGraphicsCacheEntry::ScalableImage { scaled_image, .. } => scaled_image,
            _ => panic!("internal error. image requested for non-image gpu data"),
        }
    }
    fn is_colorized_image(&self) -> bool {
        matches!(self, ItemGraphicsCacheEntry::ColorizedImage { .. })
    }
    fn as_font(&self) -> &GLFont {
        match self {
            ItemGraphicsCacheEntry::Font(font) => font,
            _ => panic!("internal error. font requested for non-font gpu data"),
        }
    }
}

struct FontCache(HashMap<FontCacheKey, femtovg::FontId>);

impl Default for FontCache {
    fn default() -> Self {
        Self(HashMap::new())
    }
}

mod fonts;
pub use fonts::register_font_from_memory;
use fonts::*;

impl FontCache {
    fn load_single_font(&mut self, canvas: &CanvasRc, request: &FontRequest) -> femtovg::FontId {
        self.0
            .entry(FontCacheKey {
                family: request.family.clone().unwrap_or_default(),
                weight: request.weight.unwrap(),
            })
            .or_insert_with(|| {
                try_load_app_font(canvas, &request)
                    .unwrap_or_else(|| load_system_font(canvas, &request))
            })
            .clone()
    }

    fn font(
        &mut self,
        canvas: &CanvasRc,
        mut request: FontRequest,
        scale_factor: f32,
        reference_text: &str,
    ) -> GLFont {
        request.pixel_size = Some(request.pixel_size.unwrap_or(DEFAULT_FONT_SIZE) * scale_factor);
        request.weight = request.weight.or(Some(DEFAULT_FONT_WEIGHT));

        let primary_font = self.load_single_font(canvas, &request);
        let fallbacks = font_fallbacks_for_request(&request, reference_text);

        let fonts = core::iter::once(primary_font)
            .chain(
                fallbacks
                    .iter()
                    .map(|fallback_request| self.load_single_font(canvas, &fallback_request)),
            )
            .collect::<SharedVector<_>>();

        GLFont { fonts, canvas: canvas.clone(), pixel_size: request.pixel_size.unwrap() }
    }
}

// glutin's WindowedContext tries to enforce being current or not. Since we need the WindowedContext's window() function
// in the GL renderer regardless whether we're current or not, we wrap the two states back into one type.
#[cfg(not(target_arch = "wasm32"))]
enum WindowedContextWrapper {
    NotCurrent(glutin::WindowedContext<glutin::NotCurrent>),
    Current(glutin::WindowedContext<glutin::PossiblyCurrent>),
}

#[cfg(not(target_arch = "wasm32"))]
impl WindowedContextWrapper {
    fn window(&self) -> &winit::window::Window {
        match self {
            Self::NotCurrent(context) => context.window(),
            Self::Current(context) => context.window(),
        }
    }

    fn make_current(self) -> Self {
        match self {
            Self::NotCurrent(not_current_ctx) => {
                let current_ctx = unsafe { not_current_ctx.make_current().unwrap() };
                Self::Current(current_ctx)
            }
            this @ Self::Current(_) => this,
        }
    }

    fn make_not_current(self) -> Self {
        match self {
            this @ Self::NotCurrent(_) => this,
            Self::Current(current_ctx_rc) => {
                Self::NotCurrent(unsafe { current_ctx_rc.make_not_current().unwrap() })
            }
        }
    }

    fn swap_buffers(&mut self) {
        match self {
            WindowedContextWrapper::NotCurrent(_) => {}
            WindowedContextWrapper::Current(current_ctx) => {
                current_ctx.swap_buffers().unwrap();
            }
        }
    }
}

struct GLRendererData {
    canvas: CanvasRc,

    #[cfg(target_arch = "wasm32")]
    window: Rc<winit::window::Window>,
    #[cfg(not(target_arch = "wasm32"))]
    windowed_context: RefCell<Option<WindowedContextWrapper>>,
    #[cfg(target_arch = "wasm32")]
    event_loop_proxy: Rc<winit::event_loop::EventLoopProxy<eventloop::CustomEvent>>,
    item_graphics_cache: RefCell<RenderingCache<Option<ItemGraphicsCacheEntry>>>,

    // Cache used to avoid repeatedly decoding images from disk. Entries with a count
    // of 1 are drained after flushing the renderer commands to the screen.
    image_cache: RefCell<HashMap<ImageCacheKey, Rc<CachedImage>>>,

    loaded_fonts: RefCell<FontCache>,

    // Layers that were scheduled for rendering where we can't delete the femtovg::ImageId yet
    // because that can only happen after calling `flush`. Otherwise femtovg ends up processing
    // `set_render_target` commands with image ids that have been deleted.
    layer_images_to_delete_after_flush: RefCell<Vec<CachedImage>>,
}

impl GLRendererData {
    // Look up the given image cache key in the image cache and upgrade the weak reference to a strong one if found,
    // otherwise a new image is created/loaded from the given callback.
    fn lookup_image_in_cache_or_create(
        &self,
        cache_key: ImageCacheKey,
        image_create_fn: impl Fn() -> Option<Rc<CachedImage>>,
    ) -> Option<Rc<CachedImage>> {
        Some(match self.image_cache.borrow_mut().entry(cache_key) {
            std::collections::hash_map::Entry::Occupied(existing_entry) => {
                existing_entry.get().clone()
            }
            std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                let new_image = image_create_fn()?;
                vacant_entry.insert(new_image.clone());
                new_image
            }
        })
    }

    // Try to load the image the given resource points to
    fn load_image_resource(&self, resource: ImageReference) -> Option<Rc<CachedImage>> {
        let cache_key = ImageCacheKey::new(&resource)?;

        self.lookup_image_in_cache_or_create(cache_key, || {
            CachedImage::new_from_resource(&resource, self)
        })
    }

    // Load the item cache entry from the specified load factory fn, unless it was cached in the
    // item's rendering cache.
    fn load_item_graphics_cache_with_function(
        &self,
        item_cache: &CachedRenderingData,
        load_fn: impl FnOnce() -> Option<ItemGraphicsCacheEntry>,
    ) -> Option<ItemGraphicsCacheEntry> {
        let mut cache = self.item_graphics_cache.borrow_mut();
        item_cache.ensure_up_to_date(&mut cache, || load_fn())
    }
}

pub struct GLRenderer {
    shared_data: Rc<GLRendererData>,
}

impl GLRenderer {
    pub(crate) fn new(
        event_loop: &dyn crate::eventloop::EventLoopInterface,
        window_builder: winit::window::WindowBuilder,
        #[cfg(target_arch = "wasm32")] canvas_id: &str,
    ) -> GLRenderer {
        #[cfg(not(target_arch = "wasm32"))]
        let (windowed_context, renderer) = {
            let windowed_context = glutin::ContextBuilder::new()
                .with_vsync(true)
                .build_windowed(window_builder, event_loop.event_loop_target())
                .unwrap();
            let windowed_context = unsafe { windowed_context.make_current().unwrap() };

            let renderer = femtovg::renderer::OpenGl::new(|symbol| {
                windowed_context.get_proc_address(symbol) as *const _
            })
            .unwrap();

            #[cfg(target_os = "macos")]
            {
                use cocoa::appkit::NSView;
                use winit::platform::macos::WindowExtMacOS;
                let ns_view = windowed_context.window().ns_view();
                let view_id: cocoa::base::id = ns_view as *const _ as *mut _;
                unsafe {
                    NSView::setLayerContentsPlacement(view_id, cocoa::appkit::NSViewLayerContentsPlacement::NSViewLayerContentsPlacementTopLeft)
                }
            }

            (windowed_context, renderer)
        };

        #[cfg(target_arch = "wasm32")]
        let event_loop_proxy = Rc::new(event_loop.event_loop_proxy().clone());

        #[cfg(target_arch = "wasm32")]
        let (window, renderer) = {
            use wasm_bindgen::JsCast;

            let canvas = web_sys::window()
                .unwrap()
                .document()
                .unwrap()
                .get_element_by_id(canvas_id)
                .unwrap()
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .unwrap();

            use winit::platform::web::WindowBuilderExtWebSys;
            use winit::platform::web::WindowExtWebSys;

            let existing_canvas_size = winit::dpi::LogicalSize::new(
                canvas.client_width() as u32,
                canvas.client_height() as u32,
            );

            let window = Rc::new(
                window_builder
                    .with_canvas(Some(canvas.clone()))
                    .build(&event_loop.event_loop_target())
                    .unwrap(),
            );

            // Try to maintain the existing size of the canvas element. A window created with winit
            // on the web will always have 1024x768 as size otherwise.

            let resize_canvas = {
                let event_loop_proxy = event_loop_proxy.clone();
                let window = window.clone();
                let canvas = canvas.clone();
                move |_: web_sys::Event| {
                    let existing_canvas_size = winit::dpi::LogicalSize::new(
                        canvas.client_width() as u32,
                        canvas.client_height() as u32,
                    );

                    window.set_inner_size(existing_canvas_size);
                    window.request_redraw();
                    event_loop_proxy.send_event(eventloop::CustomEvent::WakeUpAndPoll).ok();
                }
            };

            let resize_closure =
                wasm_bindgen::closure::Closure::wrap(Box::new(resize_canvas) as Box<dyn FnMut(_)>);
            web_sys::window()
                .unwrap()
                .add_event_listener_with_callback("resize", resize_closure.as_ref().unchecked_ref())
                .unwrap();
            resize_closure.forget();

            {
                let default_size = window.inner_size().to_logical(window.scale_factor());
                let new_size = winit::dpi::LogicalSize::new(
                    if existing_canvas_size.width > 0 {
                        existing_canvas_size.width
                    } else {
                        default_size.width
                    },
                    if existing_canvas_size.height > 0 {
                        existing_canvas_size.height
                    } else {
                        default_size.height
                    },
                );
                if new_size != default_size {
                    window.set_inner_size(new_size);
                }
            }

            let renderer =
                femtovg::renderer::OpenGl::new_from_html_canvas(&window.canvas()).unwrap();
            (window, renderer)
        };

        let canvas = femtovg::Canvas::new(renderer).unwrap();

        let shared_data = GLRendererData {
            canvas: Rc::new(RefCell::new(canvas)),

            #[cfg(not(target_arch = "wasm32"))]
            windowed_context: RefCell::new(Some(WindowedContextWrapper::NotCurrent(unsafe {
                windowed_context.make_not_current().unwrap()
            }))),
            #[cfg(target_arch = "wasm32")]
            window,
            #[cfg(target_arch = "wasm32")]
            event_loop_proxy,

            item_graphics_cache: Default::default(),
            image_cache: Default::default(),
            loaded_fonts: Default::default(),
            layer_images_to_delete_after_flush: Default::default(),
        };

        GLRenderer { shared_data: Rc::new(shared_data) }
    }

    /// Returns a new item renderer instance. At this point rendering begins and the backend ensures that the
    /// window background was cleared with the specified clear_color.
    fn new_renderer(
        &mut self,
        clear_color: &Color,
        scale_factor: f32,
        default_font_properties: &Pin<Rc<Property<FontRequest>>>,
    ) -> GLItemRenderer {
        let size = self.window().inner_size();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let ctx = &mut *self.shared_data.windowed_context.borrow_mut();
            *ctx = ctx.take().unwrap().make_current().into();
        }

        {
            let mut canvas = self.shared_data.canvas.borrow_mut();
            // We pass 1.0 as dpi / device pixel ratio as femtovg only uses this factor to scale
            // text metrics. Since we do the entire translation from logical pixels to physical
            // pixels on our end, we don't need femtovg to scale a second time.
            canvas.set_size(size.width, size.height, 1.0);
            canvas.clear_rect(0, 0, size.width, size.height, clear_color.into());
        }

        GLItemRenderer {
            shared_data: self.shared_data.clone(),
            scale_factor,
            default_font_properties: default_font_properties.clone(),
            state: vec![State {
                scissor: Rect::new(Point::default(), Size::new(size.width as _, size.height as _)),
                global_alpha: 1.,
                layer: None,
            }],
        }
    }

    /// Complete the item rendering by calling this function. This will typically flush any remaining/pending
    /// commands to the underlying graphics subsystem.
    fn flush_renderer(&mut self, _renderer: GLItemRenderer) {
        self.shared_data.canvas.borrow_mut().flush();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut ctx = self.shared_data.windowed_context.borrow_mut().take().unwrap();
            ctx.swap_buffers();

            *self.shared_data.windowed_context.borrow_mut() = ctx.make_not_current().into();
        }

        self.shared_data
            .image_cache
            .borrow_mut()
            .retain(|_, cached_image| Rc::strong_count(cached_image) > 1);

        std::mem::take(&mut *self.shared_data.layer_images_to_delete_after_flush.borrow_mut());
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn window(&self) -> std::cell::Ref<winit::window::Window> {
        std::cell::Ref::map(self.shared_data.windowed_context.borrow(), |ctx| {
            ctx.as_ref().unwrap().window()
        })
    }

    #[cfg(target_arch = "wasm32")]
    fn window(&self) -> &winit::window::Window {
        return &self.shared_data.window;
    }

    /// Returns a FontMetrics trait object that can be used to measure text and that matches the given font request as
    /// closely as possible.
    fn font_metrics(
        &mut self,
        item_graphics_cache: &sixtyfps_corelib::item_rendering::CachedRenderingData,
        font_request_fn: &dyn Fn() -> FontRequest,
        scale_factor: Pin<&Property<f32>>,
        reference_text: Pin<&Property<SharedString>>,
    ) -> Box<dyn FontMetrics> {
        let font = self
            .shared_data
            .load_item_graphics_cache_with_function(item_graphics_cache, || {
                Some(ItemGraphicsCacheEntry::Font(self.shared_data.loaded_fonts.borrow_mut().font(
                    &self.shared_data.canvas,
                    font_request_fn(),
                    scale_factor.get(),
                    &reference_text.get(),
                )))
            })
            .unwrap()
            .as_font()
            .clone();
        Box::new(GLFontMetrics {
            font,
            letter_spacing: font_request_fn().letter_spacing,
            scale_factor: scale_factor.get(),
        })
    }

    /// Returns the size of image referenced by the specified resource. These are image pixels, not adjusted
    /// to the window scale factor.
    fn image_size(
        &self,
        source: core::pin::Pin<&sixtyfps_corelib::properties::Property<ImageReference>>,
    ) -> sixtyfps_corelib::graphics::Size {
        self.shared_data
            .load_image_resource(source.get())
            .and_then(|image| image.size())
            .unwrap_or_else(|| Size::new(1., 1.))
    }
}

// Layers are stored in the renderer's State and flushed to the screen (or current rendering target)
// in restore_state() by filling the target_path.
struct Layer {
    image: CachedImage,
    target_path: femtovg::Path,
}

#[derive(Clone)]
struct State {
    scissor: Rect,
    global_alpha: f32,
    layer: Option<Rc<Layer>>,
}

pub struct GLItemRenderer {
    shared_data: Rc<GLRendererData>,
    scale_factor: f32,
    default_font_properties: Pin<Rc<Property<FontRequest>>>,
    /// track the state manually since femtovg don't have accessor for its state
    state: Vec<State>,
}

fn rect_with_radius_to_path(rect: Rect, border_radius: f32) -> femtovg::Path {
    let mut path = femtovg::Path::new();
    let x = rect.origin.x;
    let y = rect.origin.y;
    let width = rect.size.width;
    let height = rect.size.height;
    // If we're drawing a circle, use directly connected bezier curves instead of
    // ones with intermediate LineTo verbs, as `rounded_rect` creates, to avoid
    // rendering artifacts due to those edges.
    if width == height && border_radius * 2. == width {
        path.circle(x + border_radius, y + border_radius, border_radius);
    } else {
        path.rounded_rect(x, y, width, height, border_radius);
    }
    path
}

fn rect_to_path(r: Rect) -> femtovg::Path {
    rect_with_radius_to_path(r, 0.)
}

fn adjust_rect_and_border_for_inner_drawing(rect: &mut Rect, border_width: &mut f32) {
    // If the border width exceeds the width, just fill the rectangle.
    *border_width = border_width.min((rect.size.width as f32) / 2.);
    // adjust the size so that the border is drawn within the geometry
    rect.origin.x += *border_width / 2.;
    rect.origin.y += *border_width / 2.;
    rect.size.width -= *border_width;
    rect.size.height -= *border_width;
}

fn item_rect<Item: sixtyfps_corelib::items::Item>(item: Pin<&Item>, scale_factor: f32) -> Rect {
    let geometry = item.geometry();
    euclid::rect(0., 0., geometry.width() * scale_factor, geometry.height() * scale_factor)
}

impl ItemRenderer for GLItemRenderer {
    fn draw_rectangle(&mut self, rect: std::pin::Pin<&sixtyfps_corelib::items::Rectangle>) {
        let geometry = item_rect(rect, self.scale_factor);
        if geometry.is_empty() {
            return;
        }
        // TODO: cache path in item to avoid re-tesselation
        let mut path = rect_to_path(geometry);
        let paint = match self.brush_to_paint(rect.background(), &mut path) {
            Some(paint) => paint,
            None => return,
        };
        self.shared_data.canvas.borrow_mut().fill_path(&mut path, paint)
    }

    fn draw_border_rectangle(
        &mut self,
        rect: std::pin::Pin<&sixtyfps_corelib::items::BorderRectangle>,
    ) {
        let mut geometry = item_rect(rect, self.scale_factor);
        if geometry.is_empty() {
            return;
        }

        let mut border_width = rect.border_width() * self.scale_factor;
        // In CSS the border is entirely towards the inside of the boundary
        // geometry, while in femtovg the line with for a stroke is 50% in-
        // and 50% outwards. We choose the CSS model, so the inner rectangle
        // is adjusted accordingly.
        adjust_rect_and_border_for_inner_drawing(&mut geometry, &mut border_width);

        let mut path =
            rect_with_radius_to_path(geometry, rect.border_radius() * self.scale_factor());

        let fill_paint = self.brush_to_paint(rect.background(), &mut path);

        let border_paint = self.brush_to_paint(rect.border_color(), &mut path).map(|mut paint| {
            paint.set_line_width(border_width);
            paint
        });

        let mut canvas = self.shared_data.canvas.borrow_mut();
        fill_paint.map(|paint| canvas.fill_path(&mut path, paint));
        border_paint.map(|border_paint| canvas.stroke_path(&mut path, border_paint));
    }

    fn draw_image(&mut self, image: std::pin::Pin<&sixtyfps_corelib::items::Image>) {
        self.draw_image_impl(
            &image.cached_rendering_data,
            sixtyfps_corelib::items::Image::FIELD_OFFSETS.source.apply_pin(image),
            IntRect::default(),
            sixtyfps_corelib::items::Image::FIELD_OFFSETS.width.apply_pin(image),
            sixtyfps_corelib::items::Image::FIELD_OFFSETS.height.apply_pin(image),
            image.image_fit(),
            None,
        );
    }

    fn draw_clipped_image(
        &mut self,
        clipped_image: std::pin::Pin<&sixtyfps_corelib::items::ClippedImage>,
    ) {
        let source_clip_rect = IntRect::new(
            [clipped_image.source_clip_x(), clipped_image.source_clip_y()].into(),
            [clipped_image.source_clip_width(), clipped_image.source_clip_height()].into(),
        );

        self.draw_image_impl(
            &clipped_image.cached_rendering_data,
            sixtyfps_corelib::items::ClippedImage::FIELD_OFFSETS.source.apply_pin(clipped_image),
            source_clip_rect,
            sixtyfps_corelib::items::ClippedImage::FIELD_OFFSETS.width.apply_pin(clipped_image),
            sixtyfps_corelib::items::ClippedImage::FIELD_OFFSETS.height.apply_pin(clipped_image),
            clipped_image.image_fit(),
            Some(
                sixtyfps_corelib::items::ClippedImage::FIELD_OFFSETS
                    .colorize
                    .apply_pin(clipped_image),
            ),
        );
    }

    fn draw_text(&mut self, text: std::pin::Pin<&sixtyfps_corelib::items::Text>) {
        let max_width = text.width() * self.scale_factor;
        let max_height = text.height() * self.scale_factor;

        if max_width <= 0. || max_height <= 0. {
            return;
        }

        let string = text.text();
        let string = string.as_str();
        let vertical_alignment = text.vertical_alignment();
        let horizontal_alignment = text.horizontal_alignment();
        let font = self
            .shared_data
            .load_item_graphics_cache_with_function(&text.cached_rendering_data, || {
                Some(ItemGraphicsCacheEntry::Font(
                    self.shared_data.loaded_fonts.borrow_mut().font(
                        &self.shared_data.canvas,
                        text.unresolved_font_request()
                            .merge(&self.default_font_properties.as_ref().get()),
                        self.scale_factor,
                        &text.text(),
                    ),
                ))
            })
            .unwrap()
            .as_font()
            .clone();

        let wrap = text.wrap() == TextWrap::word_wrap;
        let letter_spacing = text.letter_spacing() * self.scale_factor;
        let text_size =
            font.text_size(letter_spacing, string, if wrap { Some(max_width) } else { None });

        let paint = match self
            .brush_to_paint(text.color(), &mut rect_to_path(item_rect(text, self.scale_factor)))
        {
            Some(paint) => font.init_paint(letter_spacing, paint),
            None => return,
        };

        let mut canvas = self.shared_data.canvas.borrow_mut();

        let font_metrics = canvas.measure_font(paint).unwrap();

        let mut y = match vertical_alignment {
            TextVerticalAlignment::top => 0.,
            TextVerticalAlignment::center => max_height / 2. - text_size.height / 2.,
            TextVerticalAlignment::bottom => max_height - text_size.height,
        };

        let mut draw_line = |canvas: &mut femtovg::Canvas<_>, to_draw: &str| {
            let text_metrics = canvas.measure_text(0., 0., to_draw, paint).unwrap();
            let translate_x = match horizontal_alignment {
                TextHorizontalAlignment::left => 0.,
                TextHorizontalAlignment::center => max_width / 2. - text_metrics.width() / 2.,
                TextHorizontalAlignment::right => max_width - text_metrics.width(),
            };
            canvas.fill_text(translate_x, y, to_draw, paint).unwrap();
            y += font_metrics.height();
        };

        if wrap {
            let mut start = 0;
            while start < string.len() {
                let index = canvas.break_text(max_width, &string[start..], paint).unwrap();
                if index == 0 {
                    // FIXME the word is too big to be shown, but we should still break, ideally
                    break;
                }
                let index = start + index;
                // trim is there to remove the \n
                draw_line(&mut canvas, string[start..index].trim());
                start = index;
            }
        } else {
            let elide = text.overflow() == TextOverflow::elide;
            'lines: for line in string.lines() {
                let text_metrics = canvas.measure_text(0., 0., line, paint).unwrap();
                if text_metrics.width() > max_width {
                    let w = max_width
                        - if elide {
                            canvas.measure_text(0., 0., "…", paint).unwrap().width()
                        } else {
                            0.
                        };
                    let mut current_x = 0.;
                    for glyph in text_metrics.glyphs {
                        current_x += glyph.advance_x;
                        if current_x >= w {
                            let txt = &line[..glyph.byte_index];
                            if elide {
                                let elided = format!("{}…", txt);
                                draw_line(&mut canvas, &elided);
                            } else {
                                draw_line(&mut canvas, txt);
                            }
                            continue 'lines;
                        }
                    }
                }
                draw_line(&mut canvas, line);
            }
        }
    }

    fn draw_text_input(&mut self, text_input: std::pin::Pin<&sixtyfps_corelib::items::TextInput>) {
        let width = text_input.width() * self.scale_factor;
        let height = text_input.height() * self.scale_factor;
        if width <= 0. || height <= 0. {
            return;
        }

        let font = self
            .shared_data
            .load_item_graphics_cache_with_function(&text_input.cached_rendering_data, || {
                Some(ItemGraphicsCacheEntry::Font(
                    self.shared_data.loaded_fonts.borrow_mut().font(
                        &self.shared_data.canvas,
                        text_input
                            .unresolved_font_request()
                            .merge(&self.default_font_properties.as_ref().get()),
                        self.scale_factor,
                        &text_input.text(),
                    ),
                ))
            })
            .unwrap()
            .as_font()
            .clone();

        let paint = match self.brush_to_paint(
            text_input.color(),
            &mut rect_to_path(item_rect(text_input, self.scale_factor)),
        ) {
            Some(paint) => paint,
            None => return,
        };

        let letter_spacing = text_input.letter_spacing() * self.scale_factor;
        let metrics = self.draw_text_impl(
            width,
            height,
            sixtyfps_corelib::items::TextInput::FIELD_OFFSETS.text.apply_pin(text_input),
            &font,
            paint,
            text_input.horizontal_alignment(),
            text_input.vertical_alignment(),
            letter_spacing,
        );

        // This way of drawing selected text isn't quite 100% correct. Due to femtovg only being able to
        // have a simple rectangular selection - due to the use of the scissor clip - the selected text is
        // drawn *over* the unselected text. If the selection background color is transparent, then that means
        // that glyphs are blended twice, which may lead to artifacts.
        // It would be better to draw the selected text and non-selected text without overlap.
        if text_input.has_selection() {
            let (anchor_pos, cursor_pos) = text_input.selection_anchor_and_cursor();
            let mut selection_start_x = 0.;
            let mut selection_end_x = 0.;
            for glyph in &metrics.glyphs {
                if glyph.byte_index == anchor_pos {
                    selection_start_x = glyph.x;
                }
                if glyph.byte_index == (cursor_pos as i32 - 1).max(0) as usize {
                    selection_end_x = glyph.x + glyph.advance_x;
                }
            }

            let selection_rect = Rect::new(
                [selection_start_x, 0.].into(),
                [selection_end_x - selection_start_x, font.height()].into(),
            );

            {
                let mut canvas = self.shared_data.canvas.borrow_mut();
                canvas.fill_path(
                    &mut rect_to_path(selection_rect),
                    femtovg::Paint::color(text_input.selection_background_color().into()),
                );

                canvas.save();
                canvas.intersect_scissor(
                    selection_rect.min_x(),
                    selection_rect.min_y(),
                    selection_rect.width(),
                    selection_rect.height(),
                )
            }

            self.draw_text_impl(
                width,
                height,
                sixtyfps_corelib::items::TextInput::FIELD_OFFSETS.text.apply_pin(text_input),
                &font,
                femtovg::Paint::color(text_input.selection_foreground_color().into()),
                text_input.horizontal_alignment(),
                text_input.vertical_alignment(),
                letter_spacing,
            );

            self.shared_data.canvas.borrow_mut().restore();
        };

        let cursor_index = text_input.cursor_position();
        if cursor_index >= 0 && text_input.cursor_visible() {
            let cursor_x = metrics
                .glyphs
                .iter()
                .find_map(|glyph| {
                    if glyph.byte_index == cursor_index as usize {
                        Some(glyph.x)
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| metrics.width());
            let mut cursor_rect = femtovg::Path::new();
            cursor_rect.rect(
                cursor_x,
                0.,
                text_input.text_cursor_width() * self.scale_factor,
                font.height(),
            );
            let text_paint = self.brush_to_paint(text_input.color(), &mut cursor_rect);
            text_paint.map(|text_paint| {
                self.shared_data.canvas.borrow_mut().fill_path(&mut cursor_rect, text_paint)
            });
        }
    }

    fn draw_path(&mut self, path: std::pin::Pin<&sixtyfps_corelib::items::Path>) {
        let elements = path.elements();
        if matches!(elements, sixtyfps_corelib::PathData::None) {
            return;
        }

        let (offset, path_events) = path.fitted_path_events();

        let mut fpath = femtovg::Path::new();

        /// Contrary to the SVG spec, femtovg does not use the orientation of the path to
        /// know if it needs to fill or not some part, it uses its own Solidity enum.
        /// We must then compute ourself the orientation and set the Solidity accordingly.
        #[derive(Default)]
        struct OrientationCalculator {
            area: f32,
            prev: Point,
        }

        impl OrientationCalculator {
            fn add_point(&mut self, p: Point) {
                self.area += (p.x - self.prev.x) * (p.y + self.prev.y);
                self.prev = p;
            }
        }

        use femtovg::Solidity;

        let mut orient = OrientationCalculator::default();

        for x in path_events.iter() {
            match x {
                lyon_path::Event::Begin { at } => {
                    fpath.solidity(if orient.area < 0. { Solidity::Hole } else { Solidity::Solid });
                    fpath.move_to(at.x * self.scale_factor, at.y * self.scale_factor);
                    orient.area = 0.;
                    orient.prev = at;
                }
                lyon_path::Event::Line { from: _, to } => {
                    fpath.line_to(to.x * self.scale_factor, to.y * self.scale_factor);
                    orient.add_point(to);
                }
                lyon_path::Event::Quadratic { from: _, ctrl, to } => {
                    fpath.quad_to(
                        ctrl.x * self.scale_factor,
                        ctrl.y * self.scale_factor,
                        to.x * self.scale_factor,
                        to.y * self.scale_factor,
                    );
                    orient.add_point(to);
                }

                lyon_path::Event::Cubic { from: _, ctrl1, ctrl2, to } => {
                    fpath.bezier_to(
                        ctrl1.x * self.scale_factor,
                        ctrl1.y * self.scale_factor,
                        ctrl2.x * self.scale_factor,
                        ctrl2.y * self.scale_factor,
                        to.x * self.scale_factor,
                        to.y * self.scale_factor,
                    );
                    orient.add_point(to);
                }
                lyon_path::Event::End { last: _, first: _, close } => {
                    fpath.solidity(if orient.area < 0. { Solidity::Hole } else { Solidity::Solid });
                    if close {
                        fpath.close()
                    }
                }
            }
        }

        let fill_paint = self.brush_to_paint(path.fill(), &mut fpath).map(|mut fill_paint| {
            fill_paint.set_fill_rule(match path.fill_rule() {
                FillRule::nonzero => femtovg::FillRule::NonZero,
                FillRule::evenodd => femtovg::FillRule::EvenOdd,
            });
            fill_paint
        });

        let border_paint = self.brush_to_paint(path.stroke(), &mut fpath).map(|mut paint| {
            paint.set_line_width(path.stroke_width() * self.scale_factor);
            paint
        });

        self.shared_data.canvas.borrow_mut().save_with(|canvas| {
            canvas.translate(offset.x, offset.y);
            fill_paint.map(|fill_paint| canvas.fill_path(&mut fpath, fill_paint));
            border_paint.map(|border_paint| canvas.stroke_path(&mut fpath, border_paint));
        })
    }

    /// Draws a rectangular shadow shape, which is usually placed underneath another rectangular shape
    /// with an offset (the drop-shadow-offset-x/y).
    /// The rendering happens in two phases:
    ///   * The (possibly round) rectangle is filled with the shadow color.
    ///   * A blurred shadow border is drawn using femtovg's box gradient. The shadow border is the
    ///     shape of a slightly bigger rounded rect with the inner shape subtracted. That's because
    //      we don't want the box gradient to visually "leak" into the inside.
    fn draw_box_shadow(&mut self, box_shadow: std::pin::Pin<&sixtyfps_corelib::items::BoxShadow>) {
        // TODO: cache path in item to avoid re-tesselation

        let blur = box_shadow.blur() * self.scale_factor;
        let offset_x = box_shadow.offset_x() * self.scale_factor;
        let offset_y = box_shadow.offset_y() * self.scale_factor;
        let width = box_shadow.width() * self.scale_factor;
        let height = box_shadow.height() * self.scale_factor;

        let shadow_outer_rect: euclid::Rect<f32, euclid::UnknownUnit> =
            euclid::rect(offset_x - blur / 2., offset_y - blur / 2., width + blur, height + blur);

        let shadow_inner_rect: euclid::Rect<f32, euclid::UnknownUnit> =
            euclid::rect(offset_x + blur / 2., offset_y + blur / 2., width - blur, height - blur);

        let shadow_fill_rect: euclid::Rect<f32, euclid::UnknownUnit> = euclid::rect(
            shadow_outer_rect.min_x() + blur / 2.,
            shadow_outer_rect.min_y() + blur / 2.,
            width,
            height,
        );

        let radius = box_shadow.border_radius() * self.scale_factor;
        let shadow_paint = femtovg::Paint::box_gradient(
            shadow_fill_rect.min_x(),
            shadow_fill_rect.min_y(),
            shadow_fill_rect.width(),
            shadow_fill_rect.height(),
            radius,
            blur,
            box_shadow.color().into(),
            Color::from_argb_u8(0, 0, 0, 0).into(),
        );

        let mut shadow_path = femtovg::Path::new();
        shadow_path.rounded_rect(
            shadow_outer_rect.min_x(),
            shadow_outer_rect.min_y(),
            shadow_outer_rect.width(),
            shadow_outer_rect.height(),
            radius,
        );
        shadow_path.rounded_rect(
            shadow_inner_rect.min_x(),
            shadow_inner_rect.min_y(),
            shadow_inner_rect.width(),
            shadow_inner_rect.height(),
            radius,
        );
        shadow_path.solidity(femtovg::Solidity::Hole);

        let mut canvas = self.shared_data.canvas.borrow_mut();
        let mut shadow_inner_fill_path = femtovg::Path::new();
        shadow_inner_fill_path.rounded_rect(
            shadow_inner_rect.min_x(),
            shadow_inner_rect.min_y(),
            shadow_inner_rect.width(),
            shadow_inner_rect.height(),
            radius - blur / 2.,
        );
        let fill = femtovg::Paint::color(box_shadow.color().into());
        canvas.fill_path(&mut shadow_inner_fill_path, fill);

        canvas.fill_path(&mut shadow_path, shadow_paint);
    }

    fn combine_clip(&mut self, mut clip_rect: Rect, mut radius: f32, mut border_width: f32) {
        let clip = &mut self.state.last_mut().unwrap().scissor;
        match clip.intersection(&clip_rect) {
            Some(r) => {
                *clip = r;
            }
            None => {
                *clip = Rect::default();
            }
        };

        // Femtovg renders evenly 50% inside and 50% outside of the border width. The
        // adjust_rect_and_border_for_inner_drawing adjusts the rect so that for drawing it
        // would be entirely an *inner* border. However for clipping we want the rect that's
        // entirely inside, hence the doubling of the width and consequently radius adjustment.
        border_width *= self.scale_factor * 2.;
        radius *= self.scale_factor * 0.75;
        clip_rect *= self.scale_factor;

        adjust_rect_and_border_for_inner_drawing(&mut clip_rect, &mut border_width);
        self.shared_data.canvas.borrow_mut().intersect_scissor(
            clip_rect.min_x(),
            clip_rect.min_y(),
            clip_rect.width(),
            clip_rect.height(),
        );

        // This is the very expensive clipping code path, where we change the current render target
        // to be an intermediate image and then fill the clip path with that image.
        if radius > 0. {
            let clip_path = rect_with_radius_to_path(clip_rect, radius);
            self.set_clip_path(clip_path)
        }
    }

    fn get_current_clip(&self) -> Rect {
        self.state.last().unwrap().scissor.clone()
    }

    fn save_state(&mut self) {
        self.shared_data.canvas.borrow_mut().save();
        self.state.push(self.state.last().unwrap().clone());
    }

    fn restore_state(&mut self) {
        if let Some(mut layer_to_restore) = self
            .state
            .pop()
            .and_then(|state| state.layer)
            .and_then(|layer| Rc::try_unwrap(layer).ok())
        {
            let paint = layer_to_restore.image.as_paint();

            self.shared_data
                .layer_images_to_delete_after_flush
                .borrow_mut()
                .push(layer_to_restore.image);

            let mut canvas = self.shared_data.canvas.borrow_mut();

            let restored_rendering_target = self
                .state
                .last()
                .unwrap()
                .layer
                .as_ref()
                .map_or(femtovg::RenderTarget::Screen, |layer| layer.image.as_render_target());
            canvas.set_render_target(restored_rendering_target);

            // Balanced in set_clip_path, back to original drawing conditions when set_clip_path() was called.
            canvas.restore();
            canvas.fill_path(&mut layer_to_restore.target_path, paint);
        }
        self.shared_data.canvas.borrow_mut().restore();
    }

    fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    fn draw_cached_pixmap(
        &mut self,
        item_cache: &CachedRenderingData,
        update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    ) {
        let canvas = &self.shared_data.canvas;
        let mut cache = self.shared_data.item_graphics_cache.borrow_mut();

        let cache_entry = item_cache.ensure_up_to_date(&mut cache, || {
            let mut cached_image = None;
            update_fn(&mut |width: u32, height: u32, data: &[u8]| {
                use rgb::FromSlice;
                let img = imgref::Img::new(data.as_rgba(), width as usize, height as usize);
                if let Some(image_id) =
                    canvas.borrow_mut().create_image(img, femtovg::ImageFlags::PREMULTIPLIED).ok()
                {
                    cached_image = Some(ItemGraphicsCacheEntry::Image(Rc::new(
                        CachedImage::new_on_gpu(canvas, image_id, None),
                    )))
                };
            });
            cached_image
        });
        let image_id = match cache_entry {
            Some(ItemGraphicsCacheEntry::Image(image)) => image.ensure_uploaded_to_gpu(&self),
            Some(ItemGraphicsCacheEntry::ColorizedImage { .. }) => unreachable!(),
            Some(ItemGraphicsCacheEntry::ScalableImage { .. }) => unreachable!(),
            Some(ItemGraphicsCacheEntry::Font(_)) => unreachable!(),
            None => return,
        };
        let mut canvas = self.shared_data.canvas.borrow_mut();

        let image_info = canvas.image_info(image_id).unwrap();
        let (width, height) = (image_info.width() as f32, image_info.height() as f32);
        let fill_paint = femtovg::Paint::image(image_id, 0., 0., width, height, 0.0, 1.0);
        let mut path = femtovg::Path::new();
        path.rect(0., 0., width, height);
        canvas.fill_path(&mut path, fill_paint);
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn translate(&mut self, x: f32, y: f32) {
        self.shared_data
            .canvas
            .borrow_mut()
            .translate(x * self.scale_factor, y * self.scale_factor);
        let clip = &mut self.state.last_mut().unwrap().scissor;
        *clip = clip.translate((-x, -y).into())
    }

    fn rotate(&mut self, angle_in_degrees: f32) {
        let angle_in_radians = angle_in_degrees.to_radians();
        self.shared_data.canvas.borrow_mut().rotate(angle_in_radians);
        let clip = &mut self.state.last_mut().unwrap().scissor;
        // Compute the bounding box of the rotated rectangle
        let (sin, cos) = angle_in_radians.sin_cos();
        let rotate_point = |p: Point| (p.x * cos - p.y * sin, p.x * sin + p.y * cos);
        let corners = [
            rotate_point(clip.origin),
            rotate_point(clip.origin + euclid::vec2(clip.width(), 0.)),
            rotate_point(clip.origin + euclid::vec2(0., clip.height())),
            rotate_point(clip.origin + clip.size),
        ];
        let origin: Point = (
            corners.iter().fold(f32::MAX, |a, b| b.0.min(a)),
            corners.iter().fold(f32::MAX, |a, b| b.1.min(a)),
        )
            .into();
        let end: Point = (
            corners.iter().fold(f32::MIN, |a, b| b.0.max(a)),
            corners.iter().fold(f32::MIN, |a, b| b.1.max(a)),
        )
            .into();
        *clip = Rect::new(origin, (end - origin).into());
    }

    fn apply_opacity(&mut self, opacity: f32) {
        let state = &mut self.state.last_mut().unwrap().global_alpha;
        *state *= opacity;
        self.shared_data.canvas.borrow_mut().set_global_alpha(*state);
    }
}

impl GLItemRenderer {
    fn draw_text_impl(
        &mut self,
        max_width: f32,
        max_height: f32,
        text: Pin<&Property<SharedString>>,
        font: &GLFont,
        paint: femtovg::Paint,
        horizontal_alignment: TextHorizontalAlignment,
        vertical_alignment: TextVerticalAlignment,
        letter_spacing: f32,
    ) -> femtovg::TextMetrics {
        let paint = font.init_paint(letter_spacing, paint);

        let mut canvas = self.shared_data.canvas.borrow_mut();
        let (text_width, text_height) = {
            let text_metrics = canvas.measure_text(0., 0., &text.as_ref().get(), paint).unwrap();
            let font_metrics = canvas.measure_font(paint).unwrap();
            (text_metrics.width(), font_metrics.height())
        };

        let translate_x = match horizontal_alignment {
            TextHorizontalAlignment::left => 0.,
            TextHorizontalAlignment::center => max_width / 2. - text_width / 2.,
            TextHorizontalAlignment::right => max_width - text_width,
        };

        let translate_y = match vertical_alignment {
            TextVerticalAlignment::top => 0.,
            TextVerticalAlignment::center => max_height / 2. - text_height / 2.,
            TextVerticalAlignment::bottom => max_height - text_height,
        };

        canvas.fill_text(translate_x, translate_y, text.get(), paint).unwrap()
    }

    fn colorize_image(
        &self,
        original_cache_entry: ItemGraphicsCacheEntry,
        colorize_property: Option<Pin<&Property<Brush>>>,
    ) -> ItemGraphicsCacheEntry {
        let colorize_brush = colorize_property.map_or(Brush::default(), |prop| prop.get());
        if colorize_brush.is_transparent() {
            return original_cache_entry;
        };
        let original_image = original_cache_entry.as_image();

        let image_size = match original_image.size() {
            Some(size) => size,
            None => return original_cache_entry,
        };

        let image_id = original_image.ensure_uploaded_to_gpu(&self);
        let colorized_image = self
            .shared_data
            .canvas
            .borrow_mut()
            .create_image_empty(
                image_size.width as _,
                image_size.height as _,
                femtovg::PixelFormat::Rgba8,
                femtovg::ImageFlags::PREMULTIPLIED,
            )
            .expect("internal error allocating temporary texture for image colorization");

        let mut image_rect = femtovg::Path::new();
        image_rect.rect(0., 0., image_size.width, image_size.height);
        let brush_paint = self.brush_to_paint(colorize_brush, &mut image_rect).unwrap();

        self.shared_data.canvas.borrow_mut().save_with(|canvas| {
            canvas.reset();
            canvas.scale(1., -1.); // Image are rendered upside down
            canvas.translate(0., -image_size.height);
            canvas.set_render_target(femtovg::RenderTarget::Image(colorized_image));

            canvas.global_composite_operation(femtovg::CompositeOperation::Copy);
            canvas.fill_path(
                &mut image_rect,
                femtovg::Paint::image(
                    image_id,
                    0.,
                    0.,
                    image_size.width,
                    image_size.height,
                    0.,
                    1.0,
                ),
            );

            canvas.global_composite_operation(femtovg::CompositeOperation::SourceIn);
            canvas.fill_path(&mut image_rect, brush_paint);

            canvas.set_render_target(femtovg::RenderTarget::Screen);
        });

        ItemGraphicsCacheEntry::ColorizedImage {
            original_image: original_image.clone(),
            colorized_image: Rc::new(CachedImage::new_on_gpu(
                &self.shared_data.canvas,
                colorized_image,
                None,
            )),
        }
    }

    fn draw_image_impl(
        &mut self,
        item_cache: &CachedRenderingData,
        source_property: std::pin::Pin<&Property<ImageReference>>,
        source_clip_rect: IntRect,
        target_width: std::pin::Pin<&Property<f32>>,
        target_height: std::pin::Pin<&Property<f32>>,
        image_fit: ImageFit,
        colorize_property: Option<Pin<&Property<Brush>>>,
    ) {
        let target_w = target_width.get() * self.scale_factor;
        let target_h = target_height.get() * self.scale_factor;

        if target_w <= 0. || target_h <= 0. {
            return;
        }

        let cached_image = loop {
            let image_cache_entry =
                self.shared_data.load_item_graphics_cache_with_function(item_cache, || {
                    self.shared_data
                        .load_image_resource(source_property.get())
                        .and_then(|cached_image| {
                            cached_image.as_renderable(
                                // The condition at the entry of the function ensures that width/height are positive
                                [
                                    (target_width.get() * self.scale_factor) as u32,
                                    (target_height.get() * self.scale_factor) as u32,
                                ]
                                .into(),
                            )
                        })
                        .map(|cache_entry| self.colorize_image(cache_entry, colorize_property))
                });

            // Check if the image in the cache is loaded. If not, don't draw any image and we'll return
            // later when the callback from load_html_image has issued a repaint
            let cached_image = match image_cache_entry {
                Some(entry) if entry.as_image().size().is_some() => entry,
                _ => {
                    return;
                }
            };

            // It's possible that our item cache contains an image but it's not colorized yet because it was only
            // placed there via the `image_size` function (which doesn't colorize). So we may have to invalidate our
            // item cache and try again.
            if colorize_property.map_or(false, |prop| !prop.get().is_transparent())
                && !cached_image.is_colorized_image()
            {
                let mut cache = self.shared_data.item_graphics_cache.borrow_mut();
                item_cache.release(&mut cache);
                continue;
            }

            break cached_image.as_image().clone();
        };

        let image_id = cached_image.ensure_uploaded_to_gpu(&self);
        let image_size = cached_image.size().unwrap_or_default();

        let (source_width, source_height) = if source_clip_rect.is_empty() {
            (image_size.width, image_size.height)
        } else {
            (source_clip_rect.width() as _, source_clip_rect.height() as _)
        };

        let mut source_x = source_clip_rect.min_x() as f32;
        let mut source_y = source_clip_rect.min_y() as f32;

        let mut image_fit_offset = Point::default();

        // The source_to_target scale is applied to the paint that holds the image as well as path
        // begin rendered.
        let (source_to_target_scale_x, source_to_target_scale_y) = match image_fit {
            ImageFit::fill => (target_w / source_width, target_h / source_height),
            ImageFit::cover => {
                let ratio = f32::max(target_w / source_width, target_h / source_height);

                if source_width > target_w / ratio {
                    source_x += (source_width - target_w / ratio) / 2.;
                }
                if source_height > target_h / ratio {
                    source_y += (source_height - target_h / ratio) / 2.
                }

                (ratio, ratio)
            }
            ImageFit::contain => {
                let ratio = f32::min(target_w / source_width, target_h / source_height);

                if source_width < target_w / ratio {
                    image_fit_offset.x = (target_w - source_width * ratio) / 2.;
                }
                if source_height < target_h / ratio {
                    image_fit_offset.y = (target_h - source_height * ratio) / 2.
                }

                (ratio, ratio)
            }
        };

        let fill_paint = femtovg::Paint::image(
            image_id,
            -source_x,
            -source_y,
            image_size.width,
            image_size.height,
            0.0,
            1.0,
        );

        let mut path = femtovg::Path::new();
        path.rect(0., 0., source_width, source_height);

        self.shared_data.canvas.borrow_mut().save_with(|canvas| {
            canvas.translate(image_fit_offset.x, image_fit_offset.y);

            canvas.scale(source_to_target_scale_x, source_to_target_scale_y);

            canvas.fill_path(&mut path, fill_paint);
        })
    }

    fn brush_to_paint(&self, brush: Brush, path: &mut femtovg::Path) -> Option<femtovg::Paint> {
        if brush.is_transparent() {
            return None;
        }
        Some(match brush {
            Brush::SolidColor(color) => femtovg::Paint::color(color.into()),
            Brush::LinearGradient(gradient) => {
                // `canvas.path_bbox()` applies the current transform. However we're not interested in that, since
                // we operate in item local coordinates with the `path` parameter as well as the resulting
                // paint.
                let path_bounds = {
                    let mut canvas = self.shared_data.canvas.borrow_mut();
                    canvas.save();
                    canvas.reset_transform();
                    let bbox = canvas.path_bbox(path);
                    canvas.restore();
                    bbox
                };

                let path_width = path_bounds.maxx - path_bounds.minx;
                let path_height = path_bounds.maxy - path_bounds.miny;

                let transform = euclid::Transform2D::scale(path_width, path_height)
                    .then_translate(euclid::Vector2D::new(path_bounds.minx, path_bounds.miny));

                let (start, end) = sixtyfps_corelib::graphics::line_for_angle(gradient.angle());

                let start: Point = transform.transform_point(start);
                let end: Point = transform.transform_point(end);

                let stops = gradient
                    .stops()
                    .map(|stop| (stop.position, stop.color.into()))
                    .collect::<Vec<_>>();
                femtovg::Paint::linear_gradient_stops(start.x, start.y, end.x, end.y, &stops)
            }
            _ => return None,
        })
    }

    // Set the specified path for clipping. This is done by redirecting rendering into
    // an intermediate image and using that to fill the clip path on the next restore_state()
    // call. Therefore this can only be called once per save_state()!
    fn set_clip_path(&mut self, mut path: femtovg::Path) {
        let path_bounds = {
            let mut canvas = self.shared_data.canvas.borrow_mut();
            canvas.save();
            canvas.reset_transform();
            let bbox = canvas.path_bbox(&mut path);
            canvas.restore();
            bbox
        };

        let layer_width = path_bounds.maxx - path_bounds.minx;
        let layer_height = path_bounds.maxy - path_bounds.miny;

        let clip_buffer_img = CachedImage::new_empty_on_gpu(
            &self.shared_data.canvas,
            layer_width as _,
            layer_height as _,
        );
        {
            let mut canvas = self.shared_data.canvas.borrow_mut();

            // Balanced with the *first* restore() call in restore_state(), followed by
            // the original restore() later in restore_state().
            canvas.save();

            canvas.set_render_target(clip_buffer_img.as_render_target());

            canvas.reset();

            canvas.clear_rect(
                0,
                0,
                layer_width as _,
                layer_height as _,
                femtovg::Color::rgba(0, 0, 0, 0),
            );
            canvas.global_composite_operation(femtovg::CompositeOperation::SourceOver);
        }
        self.state.last_mut().unwrap().layer =
            Some(Rc::new(Layer { image: clip_buffer_img, target_path: path }));
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct FontCacheKey {
    family: SharedString,
    weight: i32,
}

#[derive(Clone)]
struct GLFont {
    fonts: SharedVector<femtovg::FontId>,
    pixel_size: f32,
    canvas: CanvasRc,
}

impl GLFont {
    fn measure(&self, letter_spacing: f32, text: &str) -> femtovg::TextMetrics {
        self.canvas
            .borrow_mut()
            .measure_text(0., 0., text, self.init_paint(letter_spacing, femtovg::Paint::default()))
            .unwrap()
    }

    fn height(&self) -> f32 {
        self.canvas
            .borrow_mut()
            .measure_font(self.init_paint(
                /*letter spacing does not influence height*/ 0.,
                femtovg::Paint::default(),
            ))
            .unwrap()
            .height()
    }

    fn init_paint(&self, letter_spacing: f32, mut paint: femtovg::Paint) -> femtovg::Paint {
        paint.set_font(&self.fonts);
        paint.set_font_size(self.pixel_size);
        paint.set_text_baseline(femtovg::Baseline::Top);
        paint.set_letter_spacing(letter_spacing);
        paint
    }

    fn text_size(&self, letter_spacing: f32, text: &str, max_width: Option<f32>) -> Size {
        let paint = self.init_paint(letter_spacing, femtovg::Paint::default());
        let mut canvas = self.canvas.borrow_mut();
        let font_metrics = canvas.measure_font(paint).unwrap();
        let mut y = 0.;
        let mut width = 0.;
        let mut height = 0.;
        let mut start = 0;
        if let Some(max_width) = max_width {
            while start < text.len() {
                let index = canvas.break_text(max_width, &text[start..], paint).unwrap();
                if index == 0 {
                    break;
                }
                let index = start + index;
                let mesure = canvas.measure_text(0., 0., &text[start..index], paint).unwrap();
                start = index;
                height = y + mesure.height();
                y += font_metrics.height();
                width = mesure.width().max(width);
            }
        } else {
            for line in text.lines() {
                let mesure = canvas.measure_text(0., 0., line, paint).unwrap();
                height = y + mesure.height();
                y += font_metrics.height();
                width = mesure.width().max(width);
            }
        }
        euclid::size2(width, height)
    }
}

struct GLFontMetrics {
    font: GLFont,
    letter_spacing: Option<f32>,
    scale_factor: f32,
}

impl FontMetrics for GLFontMetrics {
    fn text_size(&self, text: &str) -> Size {
        self.font.text_size(self.letter_spacing.unwrap_or_default(), text, None) / self.scale_factor
    }

    fn text_offset_for_x_position<'a>(&self, text: &'a str, x: f32) -> usize {
        let x = x * self.scale_factor;
        let metrics = self.font.measure(self.letter_spacing.unwrap_or_default(), text);
        let mut current_x = 0.;
        for glyph in metrics.glyphs {
            if current_x + glyph.advance_x / 2. >= x {
                return glyph.byte_index;
            }
            current_x += glyph.advance_x;
        }
        return text.len();
    }
}

#[cfg(target_arch = "wasm32")]
pub fn create_gl_window_with_canvas_id(canvas_id: String) -> ComponentWindow {
    let window = sixtyfps_corelib::window::Window::new(|window| {
        GraphicsWindow::new(window, move |event_loop, window_builder| {
            GLRenderer::new(event_loop, window_builder, &canvas_id)
        })
    });
    ComponentWindow(window)
}

#[doc(hidden)]
#[cold]
#[cfg(not(target_arch = "wasm32"))]
pub fn use_modules() {}

pub type NativeWidgets = ();
pub type NativeGlobals = ();
pub mod native_widgets {}
pub const HAS_NATIVE_STYLE: bool = false;
pub const IS_AVAILABLE: bool = true;

thread_local!(pub(crate) static CLIPBOARD : std::cell::RefCell<copypasta::ClipboardContext> = std::cell::RefCell::new(copypasta::ClipboardContext::new().unwrap()));

pub struct Backend;
impl sixtyfps_corelib::backend::Backend for Backend {
    fn create_window(&'static self) -> ComponentWindow {
        let window = sixtyfps_corelib::window::Window::new(|window| {
            GraphicsWindow::new(window, |event_loop, window_builder| {
                GLRenderer::new(
                    event_loop,
                    window_builder,
                    #[cfg(target_arch = "wasm32")]
                    "canvas",
                )
            })
        });
        ComponentWindow(window)
    }

    fn run_event_loop(&'static self, behavior: sixtyfps_corelib::backend::EventLoopQuitBehavior) {
        crate::eventloop::run(behavior);
    }

    fn quit_event_loop(&'static self) {
        crate::eventloop::with_window_target(|event_loop| {
            event_loop.event_loop_proxy().send_event(crate::eventloop::CustomEvent::Exit).ok();
        })
    }

    fn register_font_from_memory(
        &'static self,
        data: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        self::register_font_from_memory(data)
    }

    fn register_font_from_path(
        &'static self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self::register_font_from_path(path)
    }

    fn set_clipboard_text(&'static self, text: String) {
        use copypasta::ClipboardProvider;
        CLIPBOARD.with(|clipboard| clipboard.borrow_mut().set_contents(text).ok());
    }

    fn clipboard_text(&'static self) -> Option<String> {
        use copypasta::ClipboardProvider;
        CLIPBOARD.with(|clipboard| clipboard.borrow_mut().get_contents().ok())
    }

    fn post_event(&'static self, event: Box<dyn FnOnce() + Send>) {
        let e = crate::eventloop::CustomEvent::UserEvent(event);
        #[cfg(not(target_arch = "wasm32"))]
        crate::eventloop::GLOBAL_PROXY.get_or_init(Default::default).lock().unwrap().send_event(e);
        #[cfg(target_arch = "wasm32")]
        crate::eventloop::with_window_target(|event_loop| {
            event_loop.event_loop_proxy().send_event(e).ok();
        })
    }
}
