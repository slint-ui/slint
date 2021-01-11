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
    Color, Font, FontRequest, GraphicsBackend, Point, Rect, RenderingCache, Resource,
};
use sixtyfps_corelib::item_rendering::{CachedRenderingData, ItemRenderer};
use sixtyfps_corelib::items::Item;
use sixtyfps_corelib::items::{TextHorizontalAlignment, TextVerticalAlignment};
use sixtyfps_corelib::window::ComponentWindow;
use sixtyfps_corelib::SharedString;

mod graphics_window;
use graphics_window::*;
pub(crate) mod eventloop;

type CanvasRc = Rc<RefCell<femtovg::Canvas<femtovg::renderer::OpenGl>>>;
type ItemRenderingCacheRc = Rc<RefCell<RenderingCache<Option<GPUCachedData>>>>;

pub const DEFAULT_FONT_SIZE: f32 = 12.;
pub const DEFAULT_FONT_WEIGHT: i32 = 400; // CSS normal

struct CachedImage {
    id: femtovg::ImageId,
    canvas: CanvasRc,
}

impl Drop for CachedImage {
    fn drop(&mut self) {
        self.canvas.borrow_mut().delete_image(self.id)
    }
}

#[derive(PartialEq, Eq, Hash, Debug)]
enum ImageCacheKey {
    Path(String),
    EmbeddedData(by_address::ByAddress<&'static [u8]>),
}
// Cache used to avoid repeatedly decoding images from disk. The weak references are
// drained after flushing the renderer commands to the screen.
type ImageCacheRc = Rc<RefCell<HashMap<ImageCacheKey, Weak<CachedImage>>>>;

#[derive(Clone)]
enum GPUCachedData {
    Image(Rc<CachedImage>),
}

impl GPUCachedData {
    fn as_image(&self) -> &Rc<CachedImage> {
        match self {
            GPUCachedData::Image(image) => image,
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
    fn font(&mut self, canvas: &CanvasRc, mut request: FontRequest, scale_factor: f32) -> GLFont {
        request.pixel_size = request.pixel_size.or(Some(DEFAULT_FONT_SIZE * scale_factor));
        request.weight = request.weight.or(Some(DEFAULT_FONT_WEIGHT));

        GLFont {
            font_id: self
                .0
                .entry(FontCacheKey {
                    family: request.family.clone(),
                    weight: request.weight.unwrap(),
                })
                .or_insert_with(|| {
                    try_load_app_font(canvas, &request)
                        .unwrap_or_else(|| load_system_font(canvas, &request))
                })
                .clone(),
            canvas: canvas.clone(),
            pixel_size: request.pixel_size.unwrap(),
        }
    }
}

pub struct GLRenderer {
    canvas: CanvasRc,

    #[cfg(target_arch = "wasm32")]
    window: Rc<winit::window::Window>,
    #[cfg(target_arch = "wasm32")]
    event_loop_proxy: Rc<winit::event_loop::EventLoopProxy<eventloop::CustomEvent>>,
    #[cfg(not(target_arch = "wasm32"))]
    windowed_context: Option<glutin::WindowedContext<glutin::NotCurrent>>,

    item_rendering_cache: ItemRenderingCacheRc,
    image_cache: ImageCacheRc,

    loaded_fonts: Rc<RefCell<FontCache>>,
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

        GLRenderer {
            canvas: Rc::new(RefCell::new(canvas)),
            #[cfg(target_arch = "wasm32")]
            window,
            #[cfg(target_arch = "wasm32")]
            event_loop_proxy,
            #[cfg(not(target_arch = "wasm32"))]
            windowed_context: Some(unsafe { windowed_context.make_not_current().unwrap() }),
            item_rendering_cache: Default::default(),
            image_cache: Default::default(),
            loaded_fonts: Default::default(),
        }
    }
}

impl GraphicsBackend for GLRenderer {
    type ItemRenderer = GLItemRenderer;

    fn new_renderer(&mut self, clear_color: &Color) -> GLItemRenderer {
        let (size, scale_factor) = {
            let window = self.window();
            (window.inner_size(), window.scale_factor() as f32)
        };

        #[cfg(not(target_arch = "wasm32"))]
        let current_windowed_context =
            unsafe { self.windowed_context.take().unwrap().make_current().unwrap() };

        {
            let mut canvas = self.canvas.borrow_mut();
            // We pass 1.0 as dpi / device pixel ratio as femtovg only uses this factor to scale
            // text metrics. Since we do the entire translation from logical pixels to physical
            // pixels on our end, we don't need femtovg to scale a second time.
            canvas.set_size(size.width, size.height, 1.0);
            canvas.clear_rect(0, 0, size.width, size.height, clear_color.into());
        }

        GLItemRenderer {
            canvas: self.canvas.clone(),
            #[cfg(target_arch = "wasm32")]
            window: self.window.clone(),
            #[cfg(not(target_arch = "wasm32"))]
            windowed_context: current_windowed_context,
            #[cfg(target_arch = "wasm32")]
            event_loop_proxy: self.event_loop_proxy.clone(),
            item_rendering_cache: self.item_rendering_cache.clone(),
            image_cache: self.image_cache.clone(),
            scale_factor,
            loaded_fonts: self.loaded_fonts.clone(),
        }
    }

    fn flush_renderer(&mut self, _renderer: GLItemRenderer) {
        self.canvas.borrow_mut().flush();

        #[cfg(not(target_arch = "wasm32"))]
        {
            _renderer.windowed_context.swap_buffers().unwrap();

            self.windowed_context =
                Some(unsafe { _renderer.windowed_context.make_not_current().unwrap() });
        }

        self.image_cache.borrow_mut().retain(|_, cached_image_weak| {
            cached_image_weak
                .upgrade()
                .map_or(false, |cached_image_rc| Rc::strong_count(&cached_image_rc) > 1)
        });
    }

    fn release_item_graphics_cache(&self, data: &CachedRenderingData) {
        data.release(&mut self.item_rendering_cache.borrow_mut())
    }

    fn window(&self) -> &winit::window::Window {
        #[cfg(not(target_arch = "wasm32"))]
        return self.windowed_context.as_ref().unwrap().window();
        #[cfg(target_arch = "wasm32")]
        return &self.window;
    }

    fn font(&mut self, request: FontRequest) -> Box<dyn Font> {
        Box::new(self.loaded_fonts.borrow_mut().font(
            &self.canvas,
            request,
            self.window().scale_factor() as f32,
        ))
    }
}

pub struct GLItemRenderer {
    canvas: CanvasRc,

    #[cfg(target_arch = "wasm32")]
    window: Rc<winit::window::Window>,
    #[cfg(not(target_arch = "wasm32"))]
    windowed_context: glutin::WindowedContext<glutin::PossiblyCurrent>,
    #[cfg(target_arch = "wasm32")]
    event_loop_proxy: Rc<winit::event_loop::EventLoopProxy<eventloop::CustomEvent>>,

    item_rendering_cache: ItemRenderingCacheRc,
    image_cache: ImageCacheRc,
    loaded_fonts: Rc<RefCell<FontCache>>,
    scale_factor: f32,
}

impl GLItemRenderer {
    #[cfg(target_arch = "wasm32")]
    fn load_html_image(&self, url: &str) -> femtovg::ImageId {
        let image_id = self
            .canvas
            .borrow_mut()
            .create_image_empty(1, 1, femtovg::PixelFormat::Rgba8, femtovg::ImageFlags::empty())
            .unwrap();

        let html_image = web_sys::HtmlImageElement::new().unwrap();
        html_image.set_cross_origin(Some("anonymous"));
        html_image.set_onload(Some(
            &wasm_bindgen::closure::Closure::once_into_js({
                let canvas_weak = Rc::downgrade(&self.canvas);
                let html_image = html_image.clone();
                let image_id = image_id.clone();
                let window_weak = Rc::downgrade(&self.window);
                let event_loop_proxy_weak = Rc::downgrade(&self.event_loop_proxy);
                move || {
                    let (canvas, window, event_loop_proxy) = match (
                        canvas_weak.upgrade(),
                        window_weak.upgrade(),
                        event_loop_proxy_weak.upgrade(),
                    ) {
                        (Some(canvas), Some(window), Some(event_loop_proxy)) => {
                            (canvas, window, event_loop_proxy)
                        }
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
                }
            })
            .into(),
        ));
        html_image.set_src(&url);

        image_id
    }

    // Look up the given image cache key in the image cache and upgrade the weak reference to a strong one if found,
    // otherwise a new image is created/loaded from the given callback.
    fn lookup_image_in_cache_or_create(
        &self,
        cache_key: ImageCacheKey,
        image_create_fn: impl Fn() -> femtovg::ImageId,
    ) -> Rc<CachedImage> {
        match self.image_cache.borrow_mut().entry(cache_key) {
            std::collections::hash_map::Entry::Occupied(mut existing_entry) => {
                existing_entry.get().upgrade().unwrap_or_else(|| {
                    let new_image =
                        Rc::new(CachedImage { id: image_create_fn(), canvas: self.canvas.clone() });
                    existing_entry.insert(Rc::downgrade(&new_image));
                    new_image
                })
            }
            std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                let new_image =
                    Rc::new(CachedImage { id: image_create_fn(), canvas: self.canvas.clone() });
                vacant_entry.insert(Rc::downgrade(&new_image));
                new_image
            }
        }
    }

    // Try to load the image the given resource points to
    fn load_image_resource(&self, resource: Resource) -> Option<GPUCachedData> {
        Some(GPUCachedData::Image(match resource {
            Resource::None => return None,
            Resource::AbsoluteFilePath(path) => {
                self.lookup_image_in_cache_or_create(ImageCacheKey::Path(path.to_string()), || {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        self.canvas
                            .borrow_mut()
                            .load_image_file(
                                std::path::Path::new(&path.as_str()),
                                femtovg::ImageFlags::empty(),
                            )
                            .unwrap()
                    }
                    #[cfg(target_arch = "wasm32")]
                    self.load_html_image(&path)
                })
            }
            Resource::EmbeddedData(data) => self.lookup_image_in_cache_or_create(
                ImageCacheKey::EmbeddedData(by_address::ByAddress(data.as_slice())),
                || {
                    self.canvas
                        .borrow_mut()
                        .load_image_mem(data.as_slice(), femtovg::ImageFlags::empty())
                        .unwrap()
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
    ) -> Option<(Rc<CachedImage>, femtovg::ImageInfo)> {
        let mut cache = self.item_rendering_cache.borrow_mut();
        item_cache
            .ensure_up_to_date(&mut cache, || self.load_image_resource(source_property_getter()))
            .map(|gpu_resource| {
                let image = gpu_resource.as_image();
                (image.clone(), self.canvas.borrow().image_info(image.id).unwrap())
            })
    }
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
        self.canvas.borrow_mut().save_with(|canvas| {
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

        self.canvas.borrow_mut().save_with(|canvas| {
            canvas.translate(pos.x, pos.y);
            canvas.fill_path(&mut path, fill_paint);
            canvas.stroke_path(&mut path, border_paint);
        })
    }

    fn draw_image(&mut self, pos: Point, image: std::pin::Pin<&sixtyfps_corelib::items::Image>) {
        let (cached_image, image_info) =
            match self.load_cached_item_image(&image.cached_rendering_data, || image.source()) {
                Some(image) => image,
                None => return,
            };

        let image_id = cached_image.id;

        let (image_width, image_height) = (image_info.width() as f32, image_info.height() as f32);
        let (source_width, source_height) = (image_width, image_height);
        let fill_paint =
            femtovg::Paint::image(image_id, 0., 0., source_width, source_height, 0.0, 1.0);

        let mut path = femtovg::Path::new();
        path.rect(0., 0., image_width, image_height);

        self.canvas.borrow_mut().save_with(|canvas| {
            canvas.translate(pos.x + image.x(), pos.y + image.y());

            let scaled_width = image.width();
            let scaled_height = image.height();
            if scaled_width > 0. && scaled_height > 0. {
                canvas.scale(scaled_width / image_width, scaled_height / image_height);
            }

            canvas.fill_path(&mut path, fill_paint);
        })
    }

    fn draw_clipped_image(
        &mut self,
        pos: Point,
        clipped_image: std::pin::Pin<&sixtyfps_corelib::items::ClippedImage>,
    ) {
        let (cached_image, image_info) = match self
            .load_cached_item_image(&clipped_image.cached_rendering_data, || clipped_image.source())
        {
            Some(image) => image,
            None => return,
        };

        let source_clip_rect = Rect::new(
            [clipped_image.source_clip_x() as _, clipped_image.source_clip_y() as _].into(),
            [0., 0.].into(),
        );

        let (image_width, image_height) = (image_info.width() as f32, image_info.height() as f32);
        let (source_width, source_height) = if source_clip_rect.is_empty() {
            (image_width, image_height)
        } else {
            (source_clip_rect.width() as _, source_clip_rect.height() as _)
        };
        let fill_paint = femtovg::Paint::image(
            cached_image.id,
            source_clip_rect.min_x(),
            source_clip_rect.min_y(),
            source_width,
            source_height,
            0.0,
            1.0,
        );

        let mut path = femtovg::Path::new();
        path.rect(0., 0., image_width, image_height);

        self.canvas.borrow_mut().save_with(|canvas| {
            canvas.translate(pos.x + clipped_image.x(), pos.y + clipped_image.y());

            let scaled_width = clipped_image.width();
            let scaled_height = clipped_image.height();
            if scaled_width > 0. && scaled_height > 0. {
                canvas.scale(scaled_width / image_width, scaled_height / image_height);
            }

            canvas.fill_path(&mut path, fill_paint);
        })
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
        let font = self.loaded_fonts.borrow_mut().font(
            &self.canvas,
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

            self.canvas.borrow_mut().fill_path(
                &mut rect_to_path(selection_rect),
                femtovg::Paint::color(text_input.selection_background_color().into()),
            );

            self.canvas.borrow_mut().save();
            self.canvas.borrow_mut().intersect_scissor(
                selection_rect.min_x(),
                selection_rect.min_y(),
                selection_rect.width(),
                selection_rect.height(),
            );

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

            self.canvas.borrow_mut().restore();
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
            self.canvas
                .borrow_mut()
                .fill_path(&mut cursor_rect, femtovg::Paint::color(text_input.color().into()));
        }
    }

    fn draw_path(&mut self, _pos: Point, _path: std::pin::Pin<&sixtyfps_corelib::items::Path>) {
        //todo!()
    }

    fn combine_clip(&mut self, pos: Point, clip: &std::pin::Pin<&sixtyfps_corelib::items::Clip>) {
        let clip_rect = clip.geometry().translate([pos.x, pos.y].into());
        self.canvas.borrow_mut().intersect_scissor(
            clip_rect.min_x(),
            clip_rect.min_y(),
            clip_rect.width(),
            clip_rect.height(),
        );
    }

    fn save_state(&mut self) {
        self.canvas.borrow_mut().save();
    }

    fn restore_state(&mut self) {
        self.canvas.borrow_mut().restore();
    }

    fn draw_cached_pixmap(
        &mut self,
        item_cache: &CachedRenderingData,
        pos: Point,
        update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    ) {
        let canvas = &self.canvas;
        let mut cache = self.item_rendering_cache.borrow_mut();

        let cached_image = item_cache.ensure_up_to_date(&mut cache, || {
            let mut cached_image = None;
            update_fn(&mut |width: u32, height: u32, data: &[u8]| {
                use rgb::FromSlice;
                let img = imgref::Img::new(data.as_rgba(), width as usize, height as usize);
                if let Some(image_id) =
                    canvas.borrow_mut().create_image(img, femtovg::ImageFlags::PREMULTIPLIED).ok()
                {
                    cached_image = Some(GPUCachedData::Image(Rc::new(CachedImage {
                        id: image_id,
                        canvas: canvas.clone(),
                    })))
                };
            });
            cached_image
        });
        let image_id = match cached_image {
            Some(x) => x.as_image().id,
            None => return,
        };
        let mut canvas = self.canvas.borrow_mut();

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
        let font =
            self.loaded_fonts.borrow_mut().font(&self.canvas, font_request, self.scale_factor);

        let paint = font.paint(color.into());

        let (text_width, text_height) = {
            let text_metrics = self.canvas.borrow_mut().measure_text(0., 0., &text, paint).unwrap();
            let font_metrics = self.canvas.borrow_mut().measure_font(paint).unwrap();
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

        self.canvas
            .borrow_mut()
            .fill_text(pos.x + translate_x, pos.y + translate_y, text, paint)
            .unwrap()
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct FontCacheKey {
    family: SharedString,
    weight: i32,
}

struct GLFont {
    font_id: femtovg::FontId,
    pixel_size: f32,
    canvas: CanvasRc,
}

impl Font for GLFont {
    fn text_width(&self, text: &str) -> f32 {
        self.measure(text).width()
    }

    fn text_offset_for_x_position<'a>(&self, text: &'a str, x: f32) -> usize {
        let metrics = self.measure(text);
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
        let mut paint = femtovg::Paint::default();
        paint.set_font(&[self.font_id]);
        paint.set_font_size(self.pixel_size);
        self.canvas.borrow_mut().measure_font(paint).unwrap().height()
    }
}

impl GLFont {
    fn measure(&self, text: &str) -> femtovg::TextMetrics {
        let mut paint = femtovg::Paint::default();
        paint.set_font(&[self.font_id]);
        paint.set_font_size(self.pixel_size);
        self.canvas.borrow_mut().measure_text(0., 0., text, paint).unwrap()
    }
}

impl GLFont {
    fn paint(&self, color: Color) -> femtovg::Paint {
        let mut paint = femtovg::Paint::color(color.into());
        paint.set_font(&[self.font_id]);
        paint.set_font_size(self.pixel_size);
        paint.set_text_baseline(femtovg::Baseline::Top);
        paint
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
