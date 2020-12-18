/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use cgmath::Matrix4;
use glow::{Context as GLContext, HasContext};
use lyon::tessellation::geometry_builder::{BuffersBuilder, VertexBuffers};
use lyon::tessellation::{
    FillAttributes, FillOptions, FillTessellator, StrokeAttributes, StrokeOptions,
    StrokeTessellator,
};
use sixtyfps_corelib::graphics::{
    Color, Frame as GraphicsFrame, GraphicsBackend, GraphicsWindow, HighLevelRenderingPrimitive,
    IntRect, Point, Rect, RenderingPrimitivesBuilder, RenderingVariables, Resource, RgbaColor,
    Size,
};
use sixtyfps_corelib::{eventloop::ComponentWindow, font::FontRequest};
use smallvec::{smallvec, SmallVec};
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
};

mod texture;
use texture::{GLTexture, TextureAtlas};

mod shader;
use shader::{GlyphShader, ImageShader, PathShader, RectShader};

mod buffers;
use buffers::{GLArrayBuffer, GLIndexBuffer};

#[cfg(not(target_arch = "wasm32"))]
mod glyphcache;
#[cfg(not(target_arch = "wasm32"))]
use glyphcache::GlyphCache;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct PlatformData {
    glyph_cache: GlyphCache,
}

#[derive(Copy, Clone)]
pub(crate) struct Vertex {
    _pos: [f32; 2],
}

pub struct GlyphRun {
    pub(crate) vertices: GLArrayBuffer<Vertex>,
    pub(crate) texture_vertices: GLArrayBuffer<Vertex>,
    pub(crate) texture: Rc<GLTexture>,
    pub(crate) vertex_count: i32,
}

enum GLRenderingPrimitive {
    FillPath {
        vertices: GLArrayBuffer<Vertex>,
        indices: GLIndexBuffer<u16>,
    },
    Rectangle {
        vertices: GLArrayBuffer<Vertex>,
        indices: GLIndexBuffer<u16>,
        rect_size: Size,
    },
    Texture {
        vertices: GLArrayBuffer<Vertex>,
        texture_vertices: GLArrayBuffer<Vertex>,
        texture: Rc<texture::AtlasAllocation>,
    },
    #[cfg(target_arch = "wasm32")]
    DynamicPrimitive {
        primitive: Rc<RefCell<Option<GLRenderingPrimitive>>>,
    },
    GlyphRuns {
        glyph_runs: Vec<GlyphRun>,
    },
    ApplyClip {
        vertices: Rc<GLArrayBuffer<Vertex>>,
        indices: Rc<GLIndexBuffer<u16>>,
        rect_size: Size,
    },
    ReleaseClip {
        vertices: Rc<GLArrayBuffer<Vertex>>,
        indices: Rc<GLIndexBuffer<u16>>,
        rect_size: Size,
    },
}

struct NormalRectangle {
    vertices: GLArrayBuffer<Vertex>,
    indices: GLIndexBuffer<u16>,
}

#[derive(PartialEq, Eq, Hash, Debug)]
enum TextureCacheKey {
    #[cfg(not(target_arch = "wasm32"))]
    Path(String),
    EmbeddedData(by_address::ByAddress<&'static [u8]>),
}

pub struct GLRenderer {
    context: Rc<glow::Context>,
    path_shader: PathShader,
    image_shader: ImageShader,
    glyph_shader: GlyphShader,
    rect_shader: RectShader,
    #[cfg(not(target_arch = "wasm32"))]
    platform_data: Rc<PlatformData>,
    texture_atlas: Rc<RefCell<TextureAtlas>>,
    #[cfg(target_arch = "wasm32")]
    window: Rc<winit::window::Window>,
    #[cfg(target_arch = "wasm32")]
    event_loop_proxy:
        Rc<winit::event_loop::EventLoopProxy<sixtyfps_corelib::eventloop::CustomEvent>>,
    #[cfg(not(target_arch = "wasm32"))]
    windowed_context: Option<glutin::WindowedContext<glutin::NotCurrent>>,
    normal_rectangle: Option<NormalRectangle>,
    /// When creating new rendering primitives, the cache allows the re-use of textures across
    /// primitives and frames. Each time the rendering primitives builder finishes, the
    /// cache is drained of all weak references that have no more strong references. Retaining
    /// across frames works because the new primitive is created before the old one is deleted.
    texture_cache: Rc<RefCell<HashMap<TextureCacheKey, Weak<texture::AtlasAllocation>>>>,
}

pub struct GLRenderingPrimitivesBuilder {
    context: Rc<glow::Context>,
    fill_tesselator: FillTessellator,
    stroke_tesselator: StrokeTessellator,
    texture_atlas: Rc<RefCell<TextureAtlas>>,
    #[cfg(not(target_arch = "wasm32"))]
    platform_data: Rc<PlatformData>,

    #[cfg(target_arch = "wasm32")]
    window: Rc<winit::window::Window>,
    #[cfg(target_arch = "wasm32")]
    event_loop_proxy:
        Rc<winit::event_loop::EventLoopProxy<sixtyfps_corelib::eventloop::CustomEvent>>,
    #[cfg(not(target_arch = "wasm32"))]
    windowed_context: glutin::WindowedContext<glutin::PossiblyCurrent>,

    texture_cache: Rc<RefCell<HashMap<TextureCacheKey, Weak<texture::AtlasAllocation>>>>,
}

pub struct GLFrame {
    context: Rc<glow::Context>,
    path_shader: PathShader,
    image_shader: ImageShader,
    glyph_shader: GlyphShader,
    rect_shader: RectShader,
    root_matrix: cgmath::Matrix4<f32>,
    #[cfg(not(target_arch = "wasm32"))]
    windowed_context: glutin::WindowedContext<glutin::PossiblyCurrent>,
    normal_rectangle: Option<NormalRectangle>,
    current_stencil_clip_value: u8,
}

impl GLRenderer {
    pub fn new(
        event_loop: &winit::event_loop::EventLoop<sixtyfps_corelib::eventloop::CustomEvent>,
        window_builder: winit::window::WindowBuilder,
        #[cfg(target_arch = "wasm32")] canvas_id: &str,
    ) -> GLRenderer {
        #[cfg(not(target_arch = "wasm32"))]
        let (windowed_context, context) = {
            let windowed_context = glutin::ContextBuilder::new()
                .with_vsync(true)
                .build_windowed(window_builder, &event_loop)
                .unwrap();
            let windowed_context = unsafe { windowed_context.make_current().unwrap() };

            let gl_context = unsafe {
                glow::Context::from_loader_function(|s| {
                    windowed_context.get_proc_address(s) as *const _
                })
            };

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

            (windowed_context, gl_context)
        };

        #[cfg(target_arch = "wasm32")]
        let event_loop_proxy = Rc::new(event_loop.create_proxy());

        #[cfg(target_arch = "wasm32")]
        let (window, context) = {
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
                    event_loop_proxy
                        .send_event(sixtyfps_corelib::eventloop::CustomEvent::WakeUpAndPoll)
                        .ok();
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

            let mut attrs = web_sys::WebGlContextAttributes::new();
            attrs.stencil(true);
            attrs.antialias(false);

            use wasm_bindgen::JsCast;
            let webgl1_context = window
                .canvas()
                .get_context_with_context_options("webgl", attrs.as_ref())
                .unwrap()
                .unwrap()
                .dyn_into::<web_sys::WebGlRenderingContext>()
                .unwrap();
            (window, glow::Context::from_webgl1_context(webgl1_context))
        };

        let vertex_array_object =
            unsafe { context.create_vertex_array().expect("Cannot create vertex array") };
        unsafe {
            context.bind_vertex_array(Some(vertex_array_object));
        }

        let context = Rc::new(context);
        let path_shader = PathShader::new(&context);
        let image_shader = ImageShader::new(&context);
        let glyph_shader = GlyphShader::new(&context);
        let rect_shader = RectShader::new(&context);
        #[cfg(not(target_arch = "wasm32"))]
        let platform_data = Rc::new(PlatformData::default());

        GLRenderer {
            context,
            path_shader,
            image_shader,
            glyph_shader,
            rect_shader,
            #[cfg(not(target_arch = "wasm32"))]
            platform_data,
            texture_atlas: Rc::new(RefCell::new(TextureAtlas::new())),
            #[cfg(target_arch = "wasm32")]
            window,
            #[cfg(target_arch = "wasm32")]
            event_loop_proxy,
            #[cfg(not(target_arch = "wasm32"))]
            windowed_context: Some(unsafe { windowed_context.make_not_current().unwrap() }),
            normal_rectangle: None,
            texture_cache: Default::default(),
        }
    }
}

type GLRenderingPrimitives = SmallVec<[GLRenderingPrimitive; 1]>;

pub struct OpaqueRenderingPrimitive {
    gl_primitives: GLRenderingPrimitives,
}

impl GraphicsBackend for GLRenderer {
    type LowLevelRenderingPrimitive = OpaqueRenderingPrimitive;
    type Frame = GLFrame;
    type RenderingPrimitivesBuilder = GLRenderingPrimitivesBuilder;

    fn new_rendering_primitives_builder(&mut self) -> Self::RenderingPrimitivesBuilder {
        #[cfg(not(target_arch = "wasm32"))]
        let current_windowed_context =
            unsafe { self.windowed_context.take().unwrap().make_current().unwrap() };

        {
            if self.normal_rectangle.is_none() {
                let vertex1 = Vertex { _pos: [0., 0.] };
                let vertex2 = Vertex { _pos: [0., 1.] };
                let vertex3 = Vertex { _pos: [1., 1.] };
                let vertex4 = Vertex { _pos: [1., 0.] };

                let vertices =
                    GLArrayBuffer::new(&self.context, &vec![vertex1, vertex2, vertex3, vertex4]);

                let indices = GLIndexBuffer::new(&self.context, &[0, 1, 2, 0, 2, 3]);

                self.normal_rectangle = Some(NormalRectangle { vertices, indices });
            }
        }

        GLRenderingPrimitivesBuilder {
            context: self.context.clone(),
            fill_tesselator: FillTessellator::new(),
            stroke_tesselator: StrokeTessellator::new(),
            texture_atlas: self.texture_atlas.clone(),
            #[cfg(not(target_arch = "wasm32"))]
            platform_data: self.platform_data.clone(),

            #[cfg(target_arch = "wasm32")]
            window: self.window.clone(),
            #[cfg(target_arch = "wasm32")]
            event_loop_proxy: self.event_loop_proxy.clone(),
            #[cfg(not(target_arch = "wasm32"))]
            windowed_context: current_windowed_context,

            texture_cache: self.texture_cache.clone(),
        }
    }

    fn finish_primitives(&mut self, _builder: Self::RenderingPrimitivesBuilder) {
        self.texture_cache.borrow_mut().retain(|_, cached_texture| {
            cached_texture
                .upgrade()
                .map_or(false, |cached_texture| Rc::strong_count(&cached_texture) > 1)
        });

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.windowed_context =
                Some(unsafe { _builder.windowed_context.make_not_current().unwrap() });
        }
    }

    fn new_frame(&mut self, width: u32, height: u32, clear_color: &Color) -> GLFrame {
        #[cfg(not(target_arch = "wasm32"))]
        let current_windowed_context =
            unsafe { self.windowed_context.take().unwrap().make_current().unwrap() };

        unsafe {
            self.context.viewport(0, 0, width as i32, height as i32);

            self.context.enable(glow::BLEND);
            self.context.blend_func(glow::ONE, glow::ONE_MINUS_SRC_ALPHA);

            self.context.enable(glow::STENCIL_TEST);
            self.context.stencil_func(glow::EQUAL, 0, 0xff);

            self.context.stencil_op(glow::KEEP, glow::KEEP, glow::KEEP);
            self.context.stencil_mask(0);
        }

        let col: RgbaColor<f32> = (*clear_color).into();
        unsafe {
            self.context.stencil_mask(0xff);
            self.context.clear_stencil(0);
            self.context.clear_color(col.red, col.green, col.blue, col.alpha);
            self.context.clear(glow::COLOR_BUFFER_BIT | glow::STENCIL_BUFFER_BIT);
            self.context.stencil_mask(0);
        };

        GLFrame {
            context: self.context.clone(),
            path_shader: self.path_shader.clone(),
            image_shader: self.image_shader.clone(),
            glyph_shader: self.glyph_shader.clone(),
            rect_shader: self.rect_shader.clone(),
            root_matrix: cgmath::ortho(0.0, width as f32, height as f32, 0.0, -1., 1.0),
            #[cfg(not(target_arch = "wasm32"))]
            windowed_context: current_windowed_context,
            normal_rectangle: self.normal_rectangle.take(),
            current_stencil_clip_value: 0,
        }
    }

    fn present_frame(&mut self, mut frame: Self::Frame) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            frame.windowed_context.swap_buffers().unwrap();

            self.windowed_context =
                Some(unsafe { frame.windowed_context.make_not_current().unwrap() });
        }
        self.normal_rectangle = frame.normal_rectangle.take();
    }
    fn window(&self) -> &winit::window::Window {
        #[cfg(not(target_arch = "wasm32"))]
        return self.windowed_context.as_ref().unwrap().window();
        #[cfg(target_arch = "wasm32")]
        return &self.window;
    }
}

impl RenderingPrimitivesBuilder for GLRenderingPrimitivesBuilder {
    type LowLevelRenderingPrimitive = OpaqueRenderingPrimitive;

    fn create(
        &mut self,
        primitive: HighLevelRenderingPrimitive,
    ) -> Self::LowLevelRenderingPrimitive {
        OpaqueRenderingPrimitive {
            gl_primitives: match &primitive {
                HighLevelRenderingPrimitive::NoContents => smallvec::SmallVec::new(),
                HighLevelRenderingPrimitive::Rectangle { width, height } => {
                    use lyon::math::Point;

                    let rect = Rect::new(Point::default(), Size::new(*width, *height));
                    smallvec![self.fill_rectangle(&rect)]
                }
                HighLevelRenderingPrimitive::Image { source, source_clip_rect } => {
                    match source {
                        #[cfg(not(target_arch = "wasm32"))]
                        Resource::AbsoluteFilePath(path) => {
                            let mut image_path = std::env::current_exe().unwrap();
                            image_path.pop(); // pop of executable name
                            image_path.push(&*path.clone());

                            let atlas_allocation = self.cached_texture(
                                TextureCacheKey::Path(image_path.to_string_lossy().to_string()),
                                || image::open(image_path.as_path()).unwrap().into_rgba8(),
                            );

                            smallvec![GLRenderingPrimitivesBuilder::create_texture(
                                &self.context,
                                atlas_allocation,
                                source_clip_rect
                            )]
                        }
                        #[cfg(target_arch = "wasm32")]
                        Resource::AbsoluteFilePath(path) => {
                            let shared_primitive = Rc::new(RefCell::new(None));

                            let html_image = web_sys::HtmlImageElement::new().unwrap();
                            html_image.set_cross_origin(Some("anonymous"));
                            html_image.set_onload(Some(
                                &wasm_bindgen::closure::Closure::once_into_js({
                                    let context = self.context.clone();
                                    let atlas = self.texture_atlas.clone();
                                    let html_image = html_image.clone();
                                    let shared_primitive = shared_primitive.clone();
                                    let window = self.window.clone();
                                    let event_loop_proxy = self.event_loop_proxy.clone();
                                    let source_clip_rect = *source_clip_rect;
                                    move || {
                                        let texture_primitive =
                                            GLRenderingPrimitivesBuilder::create_image(
                                                &context,
                                                &mut *atlas.borrow_mut(),
                                                &html_image,
                                                &source_clip_rect,
                                            );

                                        *shared_primitive.borrow_mut() = Some(texture_primitive);
                                        // As you can paint on a HTML canvas at any point in time, request_redraw()
                                        // on a winit window only queues an additional internal event, that'll be
                                        // be dispatched as the next event. We are however not in an event loop
                                        // call, so we also need to wake up the event loop.
                                        window.request_redraw();
                                        event_loop_proxy.send_event(
                                            sixtyfps_corelib::eventloop::CustomEvent::WakeUpAndPoll,
                                        ).ok();
                                    }
                                })
                                .into(),
                            ));
                            html_image.set_src(path);
                            smallvec![GLRenderingPrimitive::DynamicPrimitive {
                                primitive: shared_primitive
                            }]
                        }
                        Resource::EmbeddedData(slice) => {
                            let atlas_allocation = self.cached_texture(
                                TextureCacheKey::EmbeddedData(by_address::ByAddress(
                                    slice.as_slice(),
                                )),
                                || {
                                    let image_slice = slice.as_slice();

                                    image::load_from_memory(image_slice).unwrap().to_rgba8()
                                },
                            );

                            smallvec![GLRenderingPrimitivesBuilder::create_texture(
                                &self.context,
                                atlas_allocation,
                                source_clip_rect
                            )]
                        }
                        Resource::EmbeddedRgbaImage { width, height, data } => {
                            // Safety: a slice of u32 can be transmuted to a slice of u8
                            let slice = unsafe { data.as_slice().align_to().1 };
                            let image = image::ImageBuffer::<image::Rgba<u8>, &[u8]>::from_raw(
                                *width, *height, slice,
                            )
                            .unwrap();
                            smallvec![GLRenderingPrimitivesBuilder::create_image(
                                &self.context,
                                &mut *self.texture_atlas.borrow_mut(),
                                image,
                                &source_clip_rect
                            )]
                        }
                        Resource::None => SmallVec::new(),
                    }
                }
                HighLevelRenderingPrimitive::Text { text, font_request } => {
                    smallvec![self.create_glyph_runs(text, font_request)]
                }
                HighLevelRenderingPrimitive::Path { width, height, elements, stroke_width } => {
                    let mut primitives = SmallVec::new();

                    let path_iter = elements.iter_fitted(*width, *height);

                    primitives.extend(self.fill_path(path_iter.iter()).into_iter());

                    primitives
                        .extend(self.stroke_path(path_iter.iter(), *stroke_width).into_iter());

                    primitives
                }
                HighLevelRenderingPrimitive::ClipRect { width, height } => {
                    use lyon::math::Point;

                    let rect = Rect::new(Point::default(), Size::new(*width, *height));
                    smallvec![match self.fill_rectangle(&rect) {
                        GLRenderingPrimitive::Rectangle { vertices, indices, rect_size } => {
                            GLRenderingPrimitive::ApplyClip{vertices: Rc::new(vertices), indices: Rc::new(indices), rect_size}
                        }
                        _ => panic!("internal error: unsupported clipping primitive returned by fill_rectangle")
                    }]
                }
            },
        }
    }
}

impl GLRenderingPrimitivesBuilder {
    fn fill_path_from_geometry(
        &self,
        geometry: &VertexBuffers<Vertex, u16>,
    ) -> Option<GLRenderingPrimitive> {
        if geometry.vertices.len() == 0 || geometry.indices.len() == 0 {
            return None;
        }

        let vertices = GLArrayBuffer::new(&self.context, &geometry.vertices);
        let indices = GLIndexBuffer::new(&self.context, &geometry.indices);

        Some(GLRenderingPrimitive::FillPath { vertices, indices }.into())
    }

    fn fill_path(
        &mut self,
        path: impl IntoIterator<Item = lyon::path::PathEvent>,
    ) -> Option<GLRenderingPrimitive> {
        let mut geometry: VertexBuffers<Vertex, u16> = VertexBuffers::new();

        let fill_opts = FillOptions::default();
        self.fill_tesselator
            .tessellate(
                path,
                &fill_opts,
                &mut BuffersBuilder::new(
                    &mut geometry,
                    |pos: lyon::math::Point, _: FillAttributes| Vertex {
                        _pos: [pos.x as f32, pos.y as f32],
                    },
                ),
            )
            .unwrap();

        self.fill_path_from_geometry(&geometry)
    }

    fn stroke_path(
        &mut self,
        path: impl IntoIterator<Item = lyon::path::PathEvent>,
        stroke_width: f32,
    ) -> Option<GLRenderingPrimitive> {
        let mut geometry: VertexBuffers<Vertex, u16> = VertexBuffers::new();

        let stroke_opts = StrokeOptions::DEFAULT.with_line_width(stroke_width);

        self.stroke_tesselator
            .tessellate(
                path,
                &stroke_opts,
                &mut BuffersBuilder::new(
                    &mut geometry,
                    |pos: lyon::math::Point, _: StrokeAttributes| Vertex {
                        _pos: [pos.x as f32, pos.y as f32],
                    },
                ),
            )
            .unwrap();

        self.fill_path_from_geometry(&geometry)
    }

    fn fill_rectangle(&mut self, rect: &Rect) -> GLRenderingPrimitive {
        let vertex1 = Vertex { _pos: [rect.min_x(), rect.min_y()] };
        let vertex2 = Vertex { _pos: [rect.min_x(), rect.max_y()] };
        let vertex3 = Vertex { _pos: [rect.max_x(), rect.max_y()] };
        let vertex4 = Vertex { _pos: [rect.max_x(), rect.min_y()] };

        let vertices = GLArrayBuffer::new(&self.context, &vec![vertex1, vertex2, vertex3, vertex4]);

        let indices = GLIndexBuffer::new(&self.context, &[0, 1, 2, 0, 2, 3]);

        GLRenderingPrimitive::Rectangle { vertices, indices, rect_size: rect.size }.into()
    }

    fn create_image(
        context: &Rc<glow::Context>,
        atlas: &mut TextureAtlas,
        image: impl texture::UploadableAtlasImage,
        source_rect: &IntRect,
    ) -> GLRenderingPrimitive {
        let atlas_allocation = atlas.allocate_image_in_atlas(&context, image);

        Self::create_texture(context, Rc::new(atlas_allocation), source_rect)
    }

    fn create_texture(
        context: &Rc<glow::Context>,
        atlas_allocation: Rc<texture::AtlasAllocation>,
        source_rect: &IntRect,
    ) -> GLRenderingPrimitive {
        let rect = Rect::new(
            Point::new(0.0, 0.0),
            Size::new(
                atlas_allocation.texture_coordinates.width() as f32,
                atlas_allocation.texture_coordinates.height() as f32,
            ),
        );

        let vertex1 = Vertex { _pos: [rect.min_x(), rect.min_y()] };
        let vertex2 = Vertex { _pos: [rect.max_x(), rect.min_y()] };
        let vertex3 = Vertex { _pos: [rect.max_x(), rect.max_y()] };
        let vertex4 = Vertex { _pos: [rect.min_x(), rect.max_y()] };

        let vertices = GLArrayBuffer::new(
            &context,
            &vec![vertex1, vertex2, vertex3, vertex1, vertex3, vertex4],
        );
        let texture_vertices = GLArrayBuffer::new(
            &context,
            &atlas_allocation.normalized_texture_coordinates_with_source_rect(source_rect),
        );

        GLRenderingPrimitive::Texture { vertices, texture_vertices, texture: atlas_allocation }
    }

    fn cached_texture<Img: texture::UploadableAtlasImage>(
        &self,
        key: TextureCacheKey,
        create_fn: impl Fn() -> Img,
    ) -> Rc<texture::AtlasAllocation> {
        match self.texture_cache.borrow_mut().entry(key) {
            std::collections::hash_map::Entry::Occupied(mut existing_entry) => {
                existing_entry.get().upgrade().unwrap_or_else(|| {
                    let result = Rc::new(
                        self.texture_atlas
                            .borrow_mut()
                            .allocate_image_in_atlas(&self.context, create_fn()),
                    );
                    existing_entry.insert(Rc::downgrade(&result));
                    result
                })
            }
            std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                let result = Rc::new(
                    self.texture_atlas
                        .borrow_mut()
                        .allocate_image_in_atlas(&self.context, create_fn()),
                );
                vacant_entry.insert(Rc::downgrade(&result));
                result
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn create_glyph_runs(
        &mut self,
        text: &str,
        font_request: &FontRequest,
    ) -> GLRenderingPrimitive {
        let cached_glyphs = self.platform_data.glyph_cache.find_font(font_request);
        let mut cached_glyphs = cached_glyphs.borrow_mut();
        let mut atlas = self.texture_atlas.borrow_mut();
        let glyphs_runs = cached_glyphs.render_glyphs(&self.context, &mut atlas, text);
        GLRenderingPrimitive::GlyphRuns { glyph_runs: glyphs_runs }
    }

    #[cfg(target_arch = "wasm32")]
    fn create_glyph_runs(
        &mut self,
        text: &str,
        font_request: &FontRequest,
    ) -> GLRenderingPrimitive {
        let font = sixtyfps_corelib::font::FONT_CACHE.with(|fc| fc.find_font(font_request));
        let text_canvas = font.render_text(text);

        let texture = Rc::new(GLTexture::new_from_canvas(&self.context, &text_canvas));

        let rect = Rect::new(
            Point::new(0.0, 0.0),
            Size::new(text_canvas.width() as f32, text_canvas.height() as f32),
        );

        let vertex1 = Vertex { _pos: [rect.min_x(), rect.min_y()] };
        let vertex2 = Vertex { _pos: [rect.max_x(), rect.min_y()] };
        let vertex3 = Vertex { _pos: [rect.max_x(), rect.max_y()] };
        let vertex4 = Vertex { _pos: [rect.min_x(), rect.max_y()] };

        let tex_vertex1 = Vertex { _pos: [0., 0.] };
        let tex_vertex2 = Vertex { _pos: [1., 0.] };
        let tex_vertex3 = Vertex { _pos: [1., 1.] };
        let tex_vertex4 = Vertex { _pos: [0., 1.] };

        let normalized_coordinates: [Vertex; 6] =
            [tex_vertex1, tex_vertex2, tex_vertex3, tex_vertex1, tex_vertex3, tex_vertex4];

        let vertices = GLArrayBuffer::new(
            &self.context,
            &vec![vertex1, vertex2, vertex3, vertex1, vertex3, vertex4],
        );
        let texture_vertices = GLArrayBuffer::new(&self.context, &normalized_coordinates);
        let vertex_count = 6;

        let glyph_runs = vec![GlyphRun { vertices, texture_vertices, texture, vertex_count }];

        GLRenderingPrimitive::GlyphRuns { glyph_runs }
    }
}

fn to_gl_matrix(matrix: &Matrix4<f32>) -> [f32; 16] {
    [
        matrix.x[0],
        matrix.x[1],
        matrix.x[2],
        matrix.x[3],
        matrix.y[0],
        matrix.y[1],
        matrix.y[2],
        matrix.y[3],
        matrix.z[0],
        matrix.z[1],
        matrix.z[2],
        matrix.z[3],
        matrix.w[0],
        matrix.w[1],
        matrix.w[2],
        matrix.w[3],
    ]
}

impl GraphicsFrame for GLFrame {
    type LowLevelRenderingPrimitive = OpaqueRenderingPrimitive;

    fn render_primitive(
        &mut self,
        primitive: &OpaqueRenderingPrimitive,
        translation: Point,
        variables: RenderingVariables,
    ) -> Vec<OpaqueRenderingPrimitive> {
        let mut matrix = self.root_matrix
            * Matrix4::from_translation(cgmath::Vector3::new(translation.x, translation.y, 0.));

        if let RenderingVariables::Text { translate, .. } = &variables {
            matrix = matrix
                * Matrix4::from_translation(cgmath::Vector3::new(translate.x, translate.y, 0.))
        };

        primitive
            .gl_primitives
            .iter()
            .filter_map(|gl_primitive| {
                self.render_one_low_level_primitive(gl_primitive, &variables, matrix)
            })
            .collect::<Vec<_>>()
    }
}

impl GLFrame {
    fn render_one_low_level_primitive<'a>(
        &mut self,
        gl_primitive: &GLRenderingPrimitive,
        rendering_var: &RenderingVariables,
        matrix: Matrix4<f32>,
    ) -> Option<OpaqueRenderingPrimitive> {
        match (gl_primitive, rendering_var) {
            (
                GLRenderingPrimitive::FillPath { vertices, indices },
                RenderingVariables::Path { fill, .. },
            ) => {
                self.fill_path(&matrix, vertices, indices, (*fill).into());
                None
            }
            (
                GLRenderingPrimitive::Rectangle { vertices, indices, rect_size },
                RenderingVariables::Rectangle { fill, stroke, border_radius, border_width },
            ) => {
                self.draw_rect(
                    &matrix,
                    vertices,
                    indices,
                    (*fill).into(),
                    *border_radius,
                    *border_width,
                    (*stroke).into(),
                    *rect_size,
                );
                None
            }
            (
                GLRenderingPrimitive::Texture { vertices, texture_vertices, texture },
                RenderingVariables::Image { scaled_width, scaled_height, fit },
            ) => {
                let texture_width = texture.texture_coordinates.width() as f32;
                let texture_height = texture.texture_coordinates.height() as f32;

                let matrix = match fit {
                    sixtyfps_corelib::items::ImageFit::fill => {
                        matrix
                            * Matrix4::from_nonuniform_scale(
                                scaled_width / texture_width,
                                scaled_height / texture_height,
                                1.,
                            )
                    }
                    sixtyfps_corelib::items::ImageFit::contain => {
                        let ratio =
                            f32::max(scaled_width / texture_width, scaled_height / texture_height);
                        matrix * Matrix4::from_nonuniform_scale(ratio, ratio, 1.)
                    }
                };

                self.render_texture(&matrix, vertices, texture_vertices, texture);
                None
            }
            (
                GLRenderingPrimitive::Texture { vertices, texture_vertices, texture },
                RenderingVariables::NoContents,
            ) => {
                self.render_texture(&matrix, vertices, texture_vertices, texture);
                None
            }
            (
                GLRenderingPrimitive::GlyphRuns { glyph_runs },
                RenderingVariables::Text { color, cursor, selection, .. },
            ) => {
                let render_glyphs = |text_color| {
                    for GlyphRun { vertices, texture_vertices, texture, vertex_count } in glyph_runs
                    {
                        self.render_glyph_run(
                            &matrix,
                            vertices,
                            texture_vertices,
                            texture,
                            *vertex_count,
                            text_color,
                        );
                    }
                };

                let col = (*color).into();

                // Text selection is drawn in three phases:
                // 1. Draw the selection background rectangle, use regular stencil testing, write into the stencil buffer with GL_INCR
                // 2. Draw the glyphs, use regular stencil testing against current_stencil clip value + 1, don't write into the stencil buffer. This clips
                //    and draws only the glyphs of the selected text.
                // 3. Draw the glyphs, use regular stencil testing against current stencil clip value, don't write into the stencil buffer. This clips
                //    away the selected text and draws the non-selected part.
                // 4. We draw the selection background rectangle, use regular stencil testing, write into the stencil buffer with GL_DECR, use false color mask.
                //    This "removes" the selection rectangle from the stencil buffer again.

                let reset_stencil = match (selection, &self.normal_rectangle) {
                    (Some(selection), Some(text_cursor)) => {
                        let (x, width, height, foreground_color, background_color) = **selection;
                        let matrix = matrix
                            * Matrix4::from_translation(cgmath::Vector3::new(x, 0., 0.))
                            * Matrix4::from_nonuniform_scale(width, height, 1.);

                        unsafe {
                            self.context.stencil_mask(0xff);
                            self.context.stencil_op(glow::KEEP, glow::KEEP, glow::INCR);
                        }

                        self.fill_path(
                            &matrix,
                            &text_cursor.vertices,
                            &text_cursor.indices,
                            background_color.into(),
                        );

                        unsafe {
                            self.context.stencil_mask(0);
                            self.context.stencil_op(glow::KEEP, glow::KEEP, glow::KEEP);
                        }

                        // Phase 2

                        unsafe {
                            self.context.stencil_func(
                                glow::EQUAL,
                                (self.current_stencil_clip_value + 1) as i32,
                                0xff,
                            );
                        }

                        render_glyphs(foreground_color.into());

                        unsafe {
                            self.context.stencil_func(
                                glow::EQUAL,
                                self.current_stencil_clip_value as i32,
                                0xff,
                            );
                        }

                        Some(matrix)
                    }
                    _ => None, // no stencil to reset
                };

                // Phase 3

                render_glyphs(col);

                if let (Some(selection_matrix), Some(text_cursor)) =
                    (reset_stencil, &self.normal_rectangle)
                {
                    // Phase 4
                    unsafe {
                        self.context.stencil_mask(0xff);
                        self.context.stencil_op(glow::KEEP, glow::KEEP, glow::DECR);
                        self.context.color_mask(false, false, false, false);
                    }

                    self.fill_path(
                        &selection_matrix,
                        &text_cursor.vertices,
                        &text_cursor.indices,
                        col,
                    );
                    unsafe {
                        self.context.stencil_mask(0);
                        self.context.color_mask(true, true, true, true);
                        self.context.stencil_op(glow::KEEP, glow::KEEP, glow::REPLACE);
                    }
                }

                match (cursor, &self.normal_rectangle) {
                    (Some(cursor), Some(text_cursor)) => {
                        let (x, width, height) = **cursor;
                        let matrix = matrix
                            * Matrix4::from_translation(cgmath::Vector3::new(x, 0., 0.))
                            * Matrix4::from_nonuniform_scale(width, height, 1.);

                        self.fill_path(&matrix, &text_cursor.vertices, &text_cursor.indices, col);
                    }
                    _ => {}
                }
                None
            }
            (GLRenderingPrimitive::ApplyClip { vertices, indices, rect_size }, _) => {
                unsafe {
                    self.context.stencil_mask(0xff);
                    self.context.stencil_op(glow::KEEP, glow::KEEP, glow::INCR);
                    self.context.color_mask(false, false, false, false);
                }

                self.draw_rect(
                    &matrix,
                    &vertices,
                    &indices,
                    RgbaColor { alpha: 0., red: 0., green: 0., blue: 0. },
                    0.,
                    0.,
                    RgbaColor { alpha: 0., red: 0., green: 0., blue: 0. },
                    *rect_size,
                );

                unsafe {
                    self.context.stencil_mask(0);
                    self.context.stencil_op(glow::KEEP, glow::KEEP, glow::KEEP);
                    self.context.color_mask(true, true, true, true);
                }

                self.current_stencil_clip_value += 1;

                unsafe {
                    self.context.stencil_func(
                        glow::EQUAL,
                        self.current_stencil_clip_value as i32,
                        0xff,
                    );
                }

                Some(OpaqueRenderingPrimitive {
                    gl_primitives: smallvec![GLRenderingPrimitive::ReleaseClip {
                        vertices: vertices.clone(),
                        indices: indices.clone(),
                        rect_size: *rect_size,
                    }],
                })
            }

            (GLRenderingPrimitive::ReleaseClip { vertices, indices, rect_size }, _) => {
                unsafe {
                    self.context.stencil_mask(0xff);
                    self.context.stencil_op(glow::KEEP, glow::KEEP, glow::DECR);
                    self.context.color_mask(false, false, false, false);
                }

                self.draw_rect(
                    &matrix,
                    &vertices,
                    &indices,
                    RgbaColor { alpha: 0., red: 0., green: 0., blue: 0. },
                    0.,
                    0.,
                    RgbaColor { alpha: 0., red: 0., green: 0., blue: 0. },
                    *rect_size,
                );

                unsafe {
                    self.context.stencil_mask(0);
                    self.context.stencil_op(glow::KEEP, glow::KEEP, glow::KEEP);
                    self.context.color_mask(true, true, true, true);
                }

                self.current_stencil_clip_value -= 1;

                unsafe {
                    self.context.stencil_func(
                        glow::EQUAL,
                        self.current_stencil_clip_value as i32,
                        0xff,
                    );
                }

                None
            }

            #[cfg(target_arch = "wasm32")]
            (GLRenderingPrimitive::DynamicPrimitive { primitive }, var) => primitive
                .borrow()
                .as_ref()
                .map(|p| self.render_one_low_level_primitive(p, var, matrix))
                .unwrap_or(None),
            _ => panic!("Mismatch rendering variables"),
        }
    }

    fn fill_path(
        &self,
        matrix: &Matrix4<f32>,
        vertices: &GLArrayBuffer<Vertex>,
        indices: &GLIndexBuffer<u16>,
        color: RgbaColor<f32>,
    ) {
        self.path_shader.bind(&self.context, &to_gl_matrix(&matrix), color, vertices, indices);

        unsafe {
            self.context.draw_elements(glow::TRIANGLES, indices.len, glow::UNSIGNED_SHORT, 0);
        }

        self.path_shader.unbind(&self.context);
    }

    fn draw_rect(
        &self,
        matrix: &Matrix4<f32>,
        vertices: &GLArrayBuffer<Vertex>,
        indices: &GLIndexBuffer<u16>,
        color: RgbaColor<f32>,
        radius: f32,
        border_width: f32,
        border_color: RgbaColor<f32>,
        rect_size: Size,
    ) {
        // Make sure the border fits into the rectangle
        let radius = if radius * 2. > rect_size.width { rect_size.width / 2. } else { radius };
        let radius = if radius * 2. > rect_size.height { rect_size.height / 2. } else { radius };

        self.rect_shader.bind(
            &self.context,
            &to_gl_matrix(&matrix),
            color,
            &[rect_size.width / 2., rect_size.height / 2.],
            radius,
            border_width,
            border_color,
            vertices,
            indices,
        );

        unsafe {
            self.context.draw_elements(glow::TRIANGLES, indices.len, glow::UNSIGNED_SHORT, 0);
        }

        self.rect_shader.unbind(&self.context);
    }

    fn render_texture(
        &self,
        matrix: &Matrix4<f32>,
        vertices: &GLArrayBuffer<Vertex>,
        texture_vertices: &GLArrayBuffer<Vertex>,
        texture: &texture::AtlasAllocation,
    ) {
        self.image_shader.bind(
            &self.context,
            &to_gl_matrix(&matrix),
            texture.atlas.texture.as_ref(),
            vertices,
            texture_vertices,
        );

        unsafe {
            self.context.draw_arrays(glow::TRIANGLES, 0, 6);
        }

        self.image_shader.unbind(&self.context);
    }

    fn render_glyph_run(
        &self,
        matrix: &Matrix4<f32>,
        vertices: &GLArrayBuffer<Vertex>,
        texture_vertices: &GLArrayBuffer<Vertex>,
        texture: &texture::GLTexture,
        vertex_count: i32,
        color: RgbaColor<f32>,
    ) {
        self.glyph_shader.bind(
            &self.context,
            &to_gl_matrix(&matrix),
            color,
            texture,
            vertices,
            texture_vertices,
        );

        unsafe {
            self.context.draw_arrays(glow::TRIANGLES, 0, vertex_count);
        }

        self.glyph_shader.unbind(&self.context);
    }
}

pub fn create_gl_window() -> ComponentWindow {
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

pub mod renderer;
