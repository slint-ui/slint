/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
};

use sixtyfps_corelib::graphics::{
    Color, FontMetrics, FontRequest, Point, Rect, RenderingCache, Resource, Size,
};
use sixtyfps_corelib::item_rendering::{CachedRenderingData, ItemRenderer};
use sixtyfps_corelib::items::ImageFit;
use sixtyfps_corelib::items::Item;
use sixtyfps_corelib::items::{TextHorizontalAlignment, TextVerticalAlignment};
use sixtyfps_corelib::properties::Property;
use sixtyfps_corelib::window::ComponentWindow;
use sixtyfps_corelib::SharedString;

mod graphics_window;
use graphics_window::*;
pub(crate) mod eventloop;

type CanvasRc = Rc<RefCell<femtovg::Canvas<femtovg::renderer::OpenGl>>>;

pub const DEFAULT_FONT_SIZE: f32 = 12.;
pub const DEFAULT_FONT_WEIGHT: i32 = 400; // CSS normal

enum ImageData {
    GPUSide {
        id: femtovg::ImageId,
        canvas: CanvasRc,
        /// If present, this boolean property indicates whether the image has been uploaded yet or
        /// if that operation is still pending. If not present, then the image *is* available. This is
        /// used for remote HTML image loading and the property will be used to correctly track dependencies
        /// to graphics items that query for the size.
        upload_pending: Option<core::pin::Pin<Box<Property<bool>>>>,
    },
    CPUSide {
        decoded_image: image::DynamicImage,
    },
}

impl Drop for ImageData {
    fn drop(&mut self) {
        match self {
            ImageData::GPUSide { id, canvas, .. } => {
                canvas.borrow_mut().delete_image(*id);
            }
            ImageData::CPUSide { .. } => {}
        }
    }
}

struct CachedImage(RefCell<ImageData>);

impl CachedImage {
    fn new_on_cpu(decoded_image: image::DynamicImage) -> Self {
        Self(RefCell::new(ImageData::CPUSide { decoded_image }))
    }

    fn new_on_gpu(
        canvas: &CanvasRc,
        image_id: femtovg::ImageId,
        upload_pending_notifier: Option<core::pin::Pin<Box<Property<bool>>>>,
    ) -> Self {
        Self(RefCell::new(ImageData::GPUSide {
            id: image_id,
            canvas: canvas.clone(),
            upload_pending: upload_pending_notifier,
        }))
    }

    // Upload the image to the GPU? if that hasn't happened yet. This function could take just a canvas
    // as parameter, but since an upload requires a current context, this is "enforced" by taking
    // a renderer instead (which implies a current context).
    fn ensure_uploaded_to_gpu(&self, current_renderer: &GLItemRenderer) -> femtovg::ImageId {
        use std::convert::TryFrom;

        let canvas = &current_renderer.shared_data.canvas;

        let img = &mut *self.0.borrow_mut();

        if let ImageData::CPUSide { decoded_image } = img {
            let image_source = femtovg::ImageSource::try_from(&*decoded_image).unwrap();
            let image_id = canvas
                .borrow_mut()
                .create_image(image_source, femtovg::ImageFlags::empty())
                .unwrap();

            *img = ImageData::GPUSide { id: image_id, canvas: canvas.clone(), upload_pending: None }
        };

        match &img {
            ImageData::GPUSide { id, .. } => *id,
            _ => unreachable!(),
        }
    }

    fn size(&self) -> Size {
        use std::convert::TryFrom;

        match &*self.0.borrow() {
            ImageData::GPUSide { id, canvas, upload_pending } => {
                if upload_pending
                    .as_ref()
                    .map_or(false, |pending_property| pending_property.as_ref().get())
                {
                    Ok((1, 1))
                } else {
                    canvas.borrow().image_info(*id).map(|info| (info.width(), info.height()))
                }
            }
            ImageData::CPUSide { decoded_image: data } => {
                femtovg::ImageSource::try_from(data).map(|image| image.dimensions())
            }
        }
        .map(|(width, height)| euclid::size2(width as f32, height as f32))
        .unwrap_or_default()
    }

    #[cfg(target_arch = "wasm32")]
    fn notify_loaded(&self) {
        if let ImageData::GPUSide { upload_pending, .. } = &*self.0.borrow() {
            upload_pending.as_ref().map(|pending_property| pending_property.as_ref().set(true));
        }
    }
}

#[derive(PartialEq, Eq, Hash, Debug)]
enum ImageCacheKey {
    Path(String),
    EmbeddedData(by_address::ByAddress<&'static [u8]>),
}
#[derive(Clone)]
enum ItemGraphicsCacheEntry {
    Image(Rc<CachedImage>),
}

impl ItemGraphicsCacheEntry {
    fn as_image(&self) -> &Rc<CachedImage> {
        match self {
            ItemGraphicsCacheEntry::Image(image) => image,
            //_ => panic!("internal error. image requested for non-image gpu data"),
        }
    }
}

struct FontCache(HashMap<FontCacheKey, femtovg::FontId>);

impl Default for FontCache {
    fn default() -> Self {
        Self(HashMap::new())
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod fonts_fontdb;
#[cfg(not(target_arch = "wasm32"))]
pub use fonts_fontdb::register_application_font_from_memory;
#[cfg(not(target_arch = "wasm32"))]
use fonts_fontdb::*;

#[cfg(target_arch = "wasm32")]
mod fonts_wasm;
#[cfg(target_arch = "wasm32")]
pub use fonts_wasm::register_application_font_from_memory;
#[cfg(target_arch = "wasm32")]
use fonts_wasm::*;

impl FontCache {
    fn load_single_font(&mut self, canvas: &CanvasRc, request: &FontRequest) -> femtovg::FontId {
        self.0
            .entry(FontCacheKey { family: request.family.clone(), weight: request.weight.unwrap() })
            .or_insert_with(|| {
                try_load_app_font(canvas, &request)
                    .unwrap_or_else(|| load_system_font(canvas, &request))
            })
            .clone()
    }

    fn font(&mut self, canvas: &CanvasRc, mut request: FontRequest, scale_factor: f32) -> GLFont {
        request.pixel_size = request.pixel_size.or(Some(DEFAULT_FONT_SIZE * scale_factor));
        request.weight = request.weight.or(Some(DEFAULT_FONT_WEIGHT));

        let primary_font = self.load_single_font(canvas, &request);
        let fallbacks = font_fallbacks_for_request(&request);

        let fonts = core::iter::once(primary_font)
            .chain(
                fallbacks
                    .iter()
                    .map(|fallback_request| self.load_single_font(canvas, &fallback_request)),
            )
            .collect::<Vec<_>>();

        GLFont { fonts, canvas: canvas.clone(), pixel_size: request.pixel_size.unwrap() }
    }
}

struct GLRendererData {
    canvas: CanvasRc,

    #[cfg(target_arch = "wasm32")]
    window: Rc<winit::window::Window>,
    #[cfg(target_arch = "wasm32")]
    event_loop_proxy: Rc<winit::event_loop::EventLoopProxy<eventloop::CustomEvent>>,
    item_graphics_cache: RefCell<RenderingCache<Option<ItemGraphicsCacheEntry>>>,

    // Cache used to avoid repeatedly decoding images from disk. The weak references are
    // drained after flushing the renderer commands to the screen.
    image_cache: RefCell<HashMap<ImageCacheKey, Weak<CachedImage>>>,

    loaded_fonts: RefCell<FontCache>,
}

impl GLRendererData {
    #[cfg(target_arch = "wasm32")]
    fn load_html_image(&self, url: &str) -> Rc<CachedImage> {
        let image_id = self
            .canvas
            .borrow_mut()
            .create_image_empty(1, 1, femtovg::PixelFormat::Rgba8, femtovg::ImageFlags::empty())
            .unwrap();

        let cached_image = Rc::new(CachedImage::new_on_gpu(
            &self.canvas,
            image_id,
            Some(Box::pin(Default::default())),
        ));

        let html_image = web_sys::HtmlImageElement::new().unwrap();
        html_image.set_cross_origin(Some("anonymous"));
        html_image.set_onload(Some(
            &wasm_bindgen::closure::Closure::once_into_js({
                let canvas_weak = Rc::downgrade(&self.canvas);
                let html_image = html_image.clone();
                let image_id = image_id.clone();
                let window_weak = Rc::downgrade(&self.window);
                let cached_image_weak = Rc::downgrade(&cached_image);
                let event_loop_proxy_weak = Rc::downgrade(&self.event_loop_proxy);
                move || {
                    let (canvas, window, event_loop_proxy, cached_image) = match (
                        canvas_weak.upgrade(),
                        window_weak.upgrade(),
                        event_loop_proxy_weak.upgrade(),
                        cached_image_weak.upgrade(),
                    ) {
                        (
                            Some(canvas),
                            Some(window),
                            Some(event_loop_proxy),
                            Some(cached_image),
                        ) => (canvas, window, event_loop_proxy, cached_image),
                        _ => return,
                    };
                    canvas
                        .borrow_mut()
                        .realloc_image(
                            image_id,
                            html_image.width() as usize,
                            html_image.height() as usize,
                            femtovg::PixelFormat::Rgba8,
                            femtovg::ImageFlags::empty(),
                        )
                        .unwrap();
                    canvas.borrow_mut().update_image(image_id, &html_image.into(), 0, 0).unwrap();

                    // As you can paint on a HTML canvas at any point in time, request_redraw()
                    // on a winit window only queues an additional internal event, that'll be
                    // be dispatched as the next event. We are however not in an event loop
                    // call, so we also need to wake up the event loop.
                    window.request_redraw();
                    event_loop_proxy.send_event(crate::eventloop::CustomEvent::WakeUpAndPoll).ok();

                    cached_image.notify_loaded();
                }
            })
            .into(),
        ));
        html_image.set_src(&url);

        cached_image
    }

    // Look up the given image cache key in the image cache and upgrade the weak reference to a strong one if found,
    // otherwise a new image is created/loaded from the given callback.
    fn lookup_image_in_cache_or_create(
        &self,
        cache_key: ImageCacheKey,
        image_create_fn: impl Fn() -> Rc<CachedImage>,
    ) -> Rc<CachedImage> {
        match self.image_cache.borrow_mut().entry(cache_key) {
            std::collections::hash_map::Entry::Occupied(mut existing_entry) => {
                existing_entry.get().upgrade().unwrap_or_else(|| {
                    let new_image = image_create_fn();
                    existing_entry.insert(Rc::downgrade(&new_image));
                    new_image
                })
            }
            std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                let new_image = image_create_fn();
                vacant_entry.insert(Rc::downgrade(&new_image));
                new_image
            }
        }
    }

    // Try to load the image the given resource points to
    fn load_image_resource(&self, resource: Resource) -> Option<ItemGraphicsCacheEntry> {
        Some(ItemGraphicsCacheEntry::Image(match resource {
            Resource::None => return None,
            Resource::AbsoluteFilePath(path) => {
                self.lookup_image_in_cache_or_create(ImageCacheKey::Path(path.to_string()), || {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        Rc::new(CachedImage::new_on_cpu(
                            image::open(std::path::Path::new(&path.as_str())).unwrap(),
                        ))
                    }
                    #[cfg(target_arch = "wasm32")]
                    self.load_html_image(&path)
                })
            }
            Resource::EmbeddedData(data) => self.lookup_image_in_cache_or_create(
                ImageCacheKey::EmbeddedData(by_address::ByAddress(data.as_slice())),
                || {
                    Rc::new(CachedImage::new_on_cpu(
                        image::load_from_memory(data.as_slice()).unwrap(),
                    ))
                },
            ),
            Resource::EmbeddedRgbaImage { .. } => todo!(),
        }))
    }

    // Load the image from the specified Resource property (via getter fn), unless it was cached in the item's rendering
    // cache.
    fn load_cached_item_image(
        &self,
        item_cache: &CachedRenderingData,
        source_property_getter: impl Fn() -> Resource,
    ) -> Option<Rc<CachedImage>> {
        let mut cache = self.item_graphics_cache.borrow_mut();
        item_cache
            .ensure_up_to_date(&mut cache, || self.load_image_resource(source_property_getter()))
            .map(|gpu_resource| {
                let image = gpu_resource.as_image();
                image.clone()
            })
    }
}

pub struct GLRenderer {
    shared_data: Rc<GLRendererData>,
    #[cfg(not(target_arch = "wasm32"))]
    windowed_context: Option<glutin::WindowedContext<glutin::NotCurrent>>,
}

impl GLRenderer {
    pub fn new(
        event_loop: &winit::event_loop::EventLoop<eventloop::CustomEvent>,
        window_builder: winit::window::WindowBuilder,
        #[cfg(target_arch = "wasm32")] canvas_id: &str,
    ) -> GLRenderer {
        #[cfg(not(target_arch = "wasm32"))]
        let (windowed_context, renderer) = {
            let windowed_context = glutin::ContextBuilder::new()
                .with_vsync(true)
                .build_windowed(window_builder, &event_loop)
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
        let event_loop_proxy = Rc::new(event_loop.create_proxy());

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

            let window =
                Rc::new(window_builder.with_canvas(Some(canvas)).build(&event_loop).unwrap());

            // Try to maintain the existing size of the canvas element. A window created with winit
            // on the web will always have 1024x768 as size otherwise.

            let resize_canvas = {
                let event_loop_proxy = event_loop_proxy.clone();
                let canvas = web_sys::window()
                    .unwrap()
                    .document()
                    .unwrap()
                    .get_element_by_id(canvas_id)
                    .unwrap()
                    .dyn_into::<web_sys::HtmlCanvasElement>()
                    .unwrap();
                let window = window.clone();
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

            #[cfg(target_arch = "wasm32")]
            window,
            #[cfg(target_arch = "wasm32")]
            event_loop_proxy,

            item_graphics_cache: Default::default(),
            image_cache: Default::default(),
            loaded_fonts: Default::default(),
        };

        GLRenderer {
            shared_data: Rc::new(shared_data),
            #[cfg(not(target_arch = "wasm32"))]
            windowed_context: Some(unsafe { windowed_context.make_not_current().unwrap() }),
        }
    }

    /// Returns a new item renderer instance. At this point rendering begins and the backend ensures that the
    /// window background was cleared with the specified clear_color.
    fn new_renderer(&mut self, clear_color: &Color) -> GLItemRenderer {
        let (size, scale_factor) = {
            let window = self.window();
            (window.inner_size(), window.scale_factor() as f32)
        };

        #[cfg(not(target_arch = "wasm32"))]
        let current_windowed_context =
            unsafe { self.windowed_context.take().unwrap().make_current().unwrap() };

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
            #[cfg(not(target_arch = "wasm32"))]
            windowed_context: current_windowed_context,
            scale_factor,
        }
    }

    /// Complete the item rendering by calling this function. This will typically flush any remaining/pending
    /// commands to the underlying graphics subsystem.
    fn flush_renderer(&mut self, _renderer: GLItemRenderer) {
        self.shared_data.canvas.borrow_mut().flush();

        #[cfg(not(target_arch = "wasm32"))]
        {
            _renderer.windowed_context.swap_buffers().unwrap();

            self.windowed_context =
                Some(unsafe { _renderer.windowed_context.make_not_current().unwrap() });
        }

        self.shared_data.image_cache.borrow_mut().retain(|_, cached_image_weak| {
            cached_image_weak
                .upgrade()
                .map_or(false, |cached_image_rc| Rc::strong_count(&cached_image_rc) > 1)
        });
    }

    fn window(&self) -> &winit::window::Window {
        #[cfg(not(target_arch = "wasm32"))]
        return self.windowed_context.as_ref().unwrap().window();
        #[cfg(target_arch = "wasm32")]
        return &self.shared_data.window;
    }

    /// Returns a FontMetrics trait object that can be used to measure text and that matches the given font request as
    /// closely as possible.
    fn font_metrics(&mut self, request: FontRequest) -> Box<dyn FontMetrics> {
        Box::new(GLFontMetrics {
            request,
            scale_factor: self.window().scale_factor() as f32,
            shared_data: self.shared_data.clone(),
        })
    }
}

pub struct GLItemRenderer {
    shared_data: Rc<GLRendererData>,
    #[cfg(not(target_arch = "wasm32"))]
    windowed_context: glutin::WindowedContext<glutin::PossiblyCurrent>,
    scale_factor: f32,
}

fn rect_to_path(r: Rect) -> femtovg::Path {
    let mut path = femtovg::Path::new();
    path.rect(r.min_x(), r.min_y(), r.width(), r.height());
    path
}

impl ItemRenderer for GLItemRenderer {
    fn draw_rectangle(
        &mut self,
        pos: Point,
        rect: std::pin::Pin<&sixtyfps_corelib::items::Rectangle>,
    ) {
        // TODO: cache path in item to avoid re-tesselation
        let mut path = rect_to_path(rect.geometry());
        let paint = femtovg::Paint::color(rect.color().into());
        self.shared_data.canvas.borrow_mut().save_with(|canvas| {
            canvas.translate(pos.x, pos.y);
            canvas.fill_path(&mut path, paint)
        })
    }

    fn draw_border_rectangle(
        &mut self,
        pos: Point,
        rect: std::pin::Pin<&sixtyfps_corelib::items::BorderRectangle>,
    ) {
        // If the border width exceeds the width, just fill the rectangle.
        let border_width = rect.border_width().min(rect.width() / 2.);
        // In CSS the border is entirely towards the inside of the boundary
        // geometry, while in femtovg the line with for a stroke is 50% in-
        // and 50% outwards. We choose the CSS model, so the inner rectangle
        // is adjusted accordingly.
        let mut path = femtovg::Path::new();
        path.rounded_rect(
            rect.x() + border_width / 2.,
            rect.y() + border_width / 2.,
            rect.width() - border_width,
            rect.height() - border_width,
            rect.border_radius(),
        );

        let fill_paint = femtovg::Paint::color(rect.color().into());

        let mut border_paint = femtovg::Paint::color(rect.border_color().into());
        border_paint.set_line_width(border_width);

        self.shared_data.canvas.borrow_mut().save_with(|canvas| {
            canvas.translate(pos.x, pos.y);
            canvas.fill_path(&mut path, fill_paint);
            canvas.stroke_path(&mut path, border_paint);
        })
    }

    fn draw_image(&mut self, pos: Point, image: std::pin::Pin<&sixtyfps_corelib::items::Image>) {
        self.draw_image_impl(
            pos + euclid::Vector2D::new(image.x(), image.y()),
            &image.cached_rendering_data,
            sixtyfps_corelib::items::Image::FIELD_OFFSETS.source.apply_pin(image),
            Rect::default(),
            image.width(),
            image.height(),
            image.image_fit(),
        );
    }

    fn draw_clipped_image(
        &mut self,
        pos: Point,
        clipped_image: std::pin::Pin<&sixtyfps_corelib::items::ClippedImage>,
    ) {
        let source_clip_rect = Rect::new(
            [clipped_image.source_clip_x() as _, clipped_image.source_clip_y() as _].into(),
            [clipped_image.source_clip_width() as _, clipped_image.source_clip_height() as _]
                .into(),
        );

        self.draw_image_impl(
            pos + euclid::Vector2D::new(clipped_image.x(), clipped_image.y()),
            &clipped_image.cached_rendering_data,
            sixtyfps_corelib::items::ClippedImage::FIELD_OFFSETS.source.apply_pin(clipped_image),
            source_clip_rect,
            clipped_image.width(),
            clipped_image.height(),
            clipped_image.image_fit(),
        );
    }

    fn draw_text(&mut self, pos: Point, text: std::pin::Pin<&sixtyfps_corelib::items::Text>) {
        self.draw_text_impl(
            pos + euclid::Vector2D::new(text.x(), text.y()),
            text.width(),
            text.height(),
            &text.text(),
            text.font_request(),
            text.color(),
            text.horizontal_alignment(),
            text.vertical_alignment(),
        );
    }

    fn draw_text_input(
        &mut self,
        pos: Point,
        text_input: std::pin::Pin<&sixtyfps_corelib::items::TextInput>,
    ) {
        let pos = pos + euclid::Vector2D::new(text_input.x(), text_input.y());
        let font = self.shared_data.loaded_fonts.borrow_mut().font(
            &self.shared_data.canvas,
            text_input.font_request(),
            self.scale_factor,
        );

        let metrics = self.draw_text_impl(
            pos,
            text_input.width(),
            text_input.height(),
            &text_input.text(),
            text_input.font_request(),
            text_input.color(),
            text_input.horizontal_alignment(),
            text_input.vertical_alignment(),
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
                [selection_start_x, pos.y].into(),
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
                );
            }

            self.draw_text_impl(
                pos,
                text_input.width(),
                text_input.height(),
                &text_input.text(),
                text_input.font_request(),
                text_input.selection_foreground_color().into(),
                text_input.horizontal_alignment(),
                text_input.vertical_alignment(),
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
                .unwrap_or_else(|| pos.x + metrics.width());
            let mut cursor_rect = femtovg::Path::new();
            cursor_rect.rect(
                cursor_x,
                pos.y,
                text_input.text_cursor_width() * self.scale_factor,
                font.height(),
            );
            self.shared_data
                .canvas
                .borrow_mut()
                .fill_path(&mut cursor_rect, femtovg::Paint::color(text_input.color().into()));
        }
    }

    fn draw_path(&mut self, _pos: Point, _path: std::pin::Pin<&sixtyfps_corelib::items::Path>) {
        //todo!()
    }

    fn combine_clip(&mut self, pos: Point, clip: std::pin::Pin<&sixtyfps_corelib::items::Clip>) {
        let clip_rect = clip.geometry().translate([pos.x, pos.y].into());
        self.shared_data.canvas.borrow_mut().intersect_scissor(
            clip_rect.min_x(),
            clip_rect.min_y(),
            clip_rect.width(),
            clip_rect.height(),
        );
    }

    fn save_state(&mut self) {
        self.shared_data.canvas.borrow_mut().save();
    }

    fn restore_state(&mut self) {
        self.shared_data.canvas.borrow_mut().restore();
    }

    fn draw_cached_pixmap(
        &mut self,
        item_cache: &CachedRenderingData,
        pos: Point,
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
            None => return,
        };
        let mut canvas = self.shared_data.canvas.borrow_mut();

        let image_info = canvas.image_info(image_id).unwrap();
        let (width, height) = (image_info.width() as f32, image_info.height() as f32);
        let fill_paint = femtovg::Paint::image(image_id, pos.x, pos.y, width, height, 0.0, 1.0);
        let mut path = femtovg::Path::new();
        path.rect(pos.x, pos.y, width, height);
        canvas.fill_path(&mut path, fill_paint);
    }

    fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl GLItemRenderer {
    fn draw_text_impl(
        &mut self,
        pos: Point,
        max_width: f32,
        max_height: f32,
        text: &str,
        font_request: FontRequest,
        color: Color,
        horizontal_alignment: TextHorizontalAlignment,
        vertical_alignment: TextVerticalAlignment,
    ) -> femtovg::TextMetrics {
        let font = self.shared_data.loaded_fonts.borrow_mut().font(
            &self.shared_data.canvas,
            font_request,
            self.scale_factor,
        );

        let mut paint = font.paint();
        paint.set_color(color.into());

        let mut canvas = self.shared_data.canvas.borrow_mut();
        let (text_width, text_height) = {
            let text_metrics = canvas.measure_text(0., 0., &text, paint).unwrap();
            let font_metrics = canvas.measure_font(paint).unwrap();
            (text_metrics.width(), font_metrics.height())
        };

        let translate_x = match horizontal_alignment {
            TextHorizontalAlignment::align_left => 0.,
            TextHorizontalAlignment::align_center => max_width / 2. - text_width / 2.,
            TextHorizontalAlignment::align_right => max_width - text_width,
        };

        let translate_y = match vertical_alignment {
            TextVerticalAlignment::align_top => 0.,
            TextVerticalAlignment::align_center => max_height / 2. - text_height / 2.,
            TextVerticalAlignment::align_bottom => max_height - text_height,
        };

        canvas.fill_text(pos.x + translate_x, pos.y + translate_y, text, paint).unwrap()
    }

    fn draw_image_impl(
        &mut self,
        pos: Point,
        item_cache: &CachedRenderingData,
        source_property: std::pin::Pin<&Property<Resource>>,
        source_clip_rect: Rect,
        target_width: f32,
        target_height: f32,
        image_fit: ImageFit,
    ) {
        let cached_image =
            match self.shared_data.load_cached_item_image(item_cache, || source_property.get()) {
                Some(image) => image,
                None => return,
            };

        let image_id = cached_image.ensure_uploaded_to_gpu(&self);
        let image_size = cached_image.size();

        let (source_width, source_height) = if source_clip_rect.is_empty() {
            (image_size.width, image_size.height)
        } else {
            (source_clip_rect.width() as _, source_clip_rect.height() as _)
        };

        let target_width = if target_width == 0. { image_size.width } else { target_width };
        let target_height = if target_height == 0. { image_size.height } else { target_height };

        let fill_paint = femtovg::Paint::image(
            image_id,
            -source_clip_rect.min_x(),
            -source_clip_rect.min_y(),
            image_size.width,
            image_size.height,
            0.0,
            1.0,
        );

        let mut path = femtovg::Path::new();
        path.rect(0., 0., source_width, source_height);

        self.shared_data.canvas.borrow_mut().save_with(|canvas| {
            canvas.translate(pos.x, pos.y);

            match image_fit {
                ImageFit::fill => {
                    canvas.scale(target_width / source_width, target_height / source_height);
                }
                ImageFit::contain => {
                    let ratio =
                        f32::max(target_width / source_width, target_height / source_height);
                    canvas.scale(ratio, ratio)
                }
            };

            canvas.fill_path(&mut path, fill_paint);
        })
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct FontCacheKey {
    family: SharedString,
    weight: i32,
}

struct GLFont {
    fonts: Vec<femtovg::FontId>,
    pixel_size: f32,
    canvas: CanvasRc,
}

impl GLFont {
    fn measure(&self, text: &str) -> femtovg::TextMetrics {
        self.canvas.borrow_mut().measure_text(0., 0., text, self.paint()).unwrap()
    }

    fn height(&self) -> f32 {
        self.canvas.borrow_mut().measure_font(self.paint()).unwrap().height()
    }

    fn paint(&self) -> femtovg::Paint {
        let mut paint = femtovg::Paint::default();
        paint.set_font(&self.fonts);
        paint.set_font_size(self.pixel_size);
        paint.set_text_baseline(femtovg::Baseline::Top);
        paint
    }
}

struct GLFontMetrics {
    request: FontRequest,
    scale_factor: f32,
    shared_data: Rc<GLRendererData>,
}

impl FontMetrics for GLFontMetrics {
    fn text_width(&self, text: &str) -> f32 {
        self.font().measure(text).width()
    }

    fn text_offset_for_x_position<'a>(&self, text: &'a str, x: f32) -> usize {
        let metrics = self.font().measure(text);
        let mut current_x = 0.;
        for glyph in metrics.glyphs {
            if current_x + glyph.advance_x / 2. >= x {
                return glyph.byte_index;
            }
            current_x += glyph.advance_x;
        }
        return text.len();
    }

    fn height(&self) -> f32 {
        self.shared_data.canvas.borrow_mut().measure_font(self.font().paint()).unwrap().height()
    }
}

impl GLFontMetrics {
    fn font(&self) -> GLFont {
        self.shared_data.loaded_fonts.borrow_mut().font(
            &self.shared_data.canvas,
            self.request.clone(),
            self.scale_factor,
        )
    }
}

pub fn create_window() -> ComponentWindow {
    ComponentWindow::new(GraphicsWindow::new(|event_loop, window_builder| {
        GLRenderer::new(
            &event_loop.get_winit_event_loop(),
            window_builder,
            #[cfg(target_arch = "wasm32")]
            "canvas",
        )
    }))
}

#[cfg(target_arch = "wasm32")]
pub fn create_gl_window_with_canvas_id(canvas_id: String) -> ComponentWindow {
    ComponentWindow::new(GraphicsWindow::new(move |event_loop, window_builder| {
        GLRenderer::new(&event_loop.get_winit_event_loop(), window_builder, &canvas_id)
    }))
}

#[doc(hidden)]
#[cold]
pub fn use_modules() {
    sixtyfps_corelib::use_modules();
}

pub type NativeWidgets = ();
pub type NativeGlobals = ();
pub mod native_widgets {}
pub const HAS_NATIVE_STYLE: bool = false;
pub const IS_AVAILABLE: bool = true;
