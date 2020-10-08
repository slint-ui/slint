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
use sixtyfps_corelib::eventloop::ComponentWindow;
use sixtyfps_corelib::{
    graphics::{
        ARGBColor, Color, Frame as GraphicsFrame, GraphicsBackend, GraphicsWindow,
        HighLevelRenderingPrimitive, Point, Rect, RenderingPrimitivesBuilder, RenderingVariable,
        Resource, Size,
    },
    SharedArray,
};
use smallvec::{smallvec, SmallVec};
use std::cell::RefCell;

extern crate alloc;
use alloc::rc::Rc;

mod texture;
use texture::{GLTexture, TextureAtlas};

mod shader;
use shader::{ImageShader, PathShader};

use shader::GlyphShader;

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
    Texture {
        vertices: GLArrayBuffer<Vertex>,
        texture_vertices: GLArrayBuffer<Vertex>,
        texture: texture::AtlasAllocation,
        image_size: Size,
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
    },
    ReleaseClip {
        vertices: Rc<GLArrayBuffer<Vertex>>,
        indices: Rc<GLIndexBuffer<u16>>,
    },
}

struct TextCursor {
    vertices: GLArrayBuffer<Vertex>,
    indices: GLIndexBuffer<u16>,
}

impl TextCursor {
    fn from_primitive(rect_primitive: GLRenderingPrimitive) -> Self {
        match rect_primitive {
            GLRenderingPrimitive::FillPath { vertices, indices } => Self { vertices, indices },
            _ => panic!("internal error: TextCursor can only be constructed from rectangle fill"),
        }
    }
}

pub struct GLRenderer {
    context: Rc<glow::Context>,
    path_shader: PathShader,
    image_shader: ImageShader,
    glyph_shader: GlyphShader,
    #[cfg(not(target_arch = "wasm32"))]
    platform_data: Rc<PlatformData>,
    texture_atlas: Rc<RefCell<TextureAtlas>>,
    #[cfg(target_arch = "wasm32")]
    window: Rc<winit::window::Window>,
    #[cfg(not(target_arch = "wasm32"))]
    windowed_context: Option<glutin::WindowedContext<glutin::NotCurrent>>,
    text_cursor_rect: Option<TextCursor>,
}

pub struct GLRenderingPrimitivesBuilder {
    context: Rc<glow::Context>,
    fill_tesselator: FillTessellator,
    stroke_tesselator: StrokeTessellator,
    texture_atlas: Rc<RefCell<TextureAtlas>>,
    #[cfg(not(target_arch = "wasm32"))]
    platform_data: Rc<PlatformData>,

    #[cfg(not(target_arch = "wasm32"))]
    windowed_context: glutin::WindowedContext<glutin::PossiblyCurrent>,

    text_cursor_rect: Option<TextCursor>,
}

pub struct GLFrame {
    context: Rc<glow::Context>,
    path_shader: PathShader,
    image_shader: ImageShader,
    glyph_shader: GlyphShader,
    root_matrix: cgmath::Matrix4<f32>,
    #[cfg(not(target_arch = "wasm32"))]
    windowed_context: glutin::WindowedContext<glutin::PossiblyCurrent>,
    text_cursor_rect: Option<TextCursor>,
    current_stencil_clip_value: u8,
}

impl GLRenderer {
    pub fn new(
        event_loop: &winit::event_loop::EventLoop<()>,
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

            // Try to maintain the existing size of the canvas element. A window created with winit
            // on the web will always have 1024x768 as size otherwise.
            let existing_canvas_size = winit::dpi::PhysicalSize::new(
                canvas.client_width() as u32,
                canvas.client_height() as u32,
            );

            let window =
                Rc::new(window_builder.with_canvas(Some(canvas)).build(&event_loop).unwrap());

            {
                let default_size = window.inner_size();
                let new_size = winit::dpi::PhysicalSize::new(
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

            use wasm_bindgen::JsCast;
            let webgl1_context = window
                .canvas()
                .get_context("webgl")
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
        #[cfg(not(target_arch = "wasm32"))]
        let platform_data = Rc::new(PlatformData::default());

        GLRenderer {
            context,
            path_shader,
            image_shader,
            glyph_shader,
            #[cfg(not(target_arch = "wasm32"))]
            platform_data,
            texture_atlas: Rc::new(RefCell::new(TextureAtlas::new())),
            #[cfg(target_arch = "wasm32")]
            window,
            #[cfg(not(target_arch = "wasm32"))]
            windowed_context: Some(unsafe { windowed_context.make_not_current().unwrap() }),
            text_cursor_rect: None,
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
        GLRenderingPrimitivesBuilder {
            context: self.context.clone(),
            fill_tesselator: FillTessellator::new(),
            stroke_tesselator: StrokeTessellator::new(),
            texture_atlas: self.texture_atlas.clone(),
            #[cfg(not(target_arch = "wasm32"))]
            platform_data: self.platform_data.clone(),

            #[cfg(not(target_arch = "wasm32"))]
            windowed_context: current_windowed_context,
            text_cursor_rect: self.text_cursor_rect.take(),
        }
    }

    fn finish_primitives(&mut self, mut builder: Self::RenderingPrimitivesBuilder) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.windowed_context =
                Some(unsafe { builder.windowed_context.make_not_current().unwrap() });
        }
        self.text_cursor_rect = builder.text_cursor_rect.take();
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

        let col: ARGBColor<f32> = (*clear_color).into();
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
            root_matrix: cgmath::ortho(0.0, width as f32, height as f32, 0.0, -1., 1.0),
            #[cfg(not(target_arch = "wasm32"))]
            windowed_context: current_windowed_context,
            text_cursor_rect: self.text_cursor_rect.take(),
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
        self.text_cursor_rect = frame.text_cursor_rect.take();
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
                    self.fill_rectangle(&rect, 0.).into_iter().collect()
                }
                HighLevelRenderingPrimitive::BorderRectangle {
                    width,
                    height,
                    border_width,
                    border_radius,
                } => {
                    use lyon::math::Point;

                    let border_offset = *border_width / 2.;

                    let rect = Rect::new(
                        Point::new(border_offset, border_offset),
                        Size::new(*width - border_width, *height - *border_width),
                    );

                    let mut primitives: SmallVec<_> =
                        self.fill_rectangle(&rect, *border_radius).into_iter().collect();

                    if *border_width > 0. {
                        let stroke = self.stroke_rectangle(&rect, *border_width, *border_radius);
                        primitives.extend(stroke);
                    }

                    primitives
                }
                HighLevelRenderingPrimitive::Image { source } => {
                    match source {
                        #[cfg(not(target_arch = "wasm32"))]
                        Resource::AbsoluteFilePath(path) => {
                            let mut image_path = std::env::current_exe().unwrap();
                            image_path.pop(); // pop of executable name
                            image_path.push(&*path.clone());
                            let image = image::open(image_path.as_path()).unwrap().into_rgba();
                            let image = image::ImageBuffer::<image::Rgba<u8>, &[u8]>::from_raw(
                                image.width(),
                                image.height(),
                                &image,
                            )
                            .unwrap();
                            smallvec![GLRenderingPrimitivesBuilder::create_image(
                                &self.context,
                                &mut *self.texture_atlas.borrow_mut(),
                                image
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
                                    move || {
                                        let texture_primitive =
                                            GLRenderingPrimitivesBuilder::create_image(
                                                &context,
                                                &mut *atlas.borrow_mut(),
                                                &html_image,
                                            );

                                        *shared_primitive.borrow_mut() = Some(texture_primitive);
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
                            let image_slice = slice.as_slice();
                            let image = image::load_from_memory(image_slice).unwrap().to_rgba();
                            let image = image::ImageBuffer::<image::Rgba<u8>, &[u8]>::from_raw(
                                image.width(),
                                image.height(),
                                &image,
                            )
                            .unwrap();
                            smallvec![GLRenderingPrimitivesBuilder::create_image(
                                &self.context,
                                &mut *self.texture_atlas.borrow_mut(),
                                image
                            )]
                        }
                        Resource::EmbeddedRgbaImage { width, height, data } => {
                            let image = image::ImageBuffer::<image::Rgba<u8>, &[u8]>::from_raw(
                                *width,
                                *height,
                                data.as_slice(),
                            )
                            .unwrap();
                            smallvec![GLRenderingPrimitivesBuilder::create_image(
                                &self.context,
                                &mut *self.texture_atlas.borrow_mut(),
                                image
                            )]
                        }
                        Resource::None => SmallVec::new(),
                    }
                }
                HighLevelRenderingPrimitive::Text { text, font_family, font_size } => {
                    if self.text_cursor_rect.is_none() {
                        let rect = Rect::new(Point::default(), Size::new(1., 1.));
                        self.text_cursor_rect = Some(TextCursor::from_primitive(
                            self.fill_rectangle(&rect, 0.).unwrap(),
                        ));
                    }

                    smallvec![self.create_glyph_runs(text, font_family, *font_size)]
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
                    self.fill_rectangle(&rect, 0.).map(|primitive| match primitive {
                        GLRenderingPrimitive::FillPath { vertices, indices } => {
                            GLRenderingPrimitive::ApplyClip{vertices: Rc::new(vertices), indices: Rc::new(indices)}
                        }
                        _ => panic!("internal error: unsupported clipping primitive returned by fill_rectangle")
                    }).into_iter().collect()
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

    fn fill_rectangle(&mut self, rect: &Rect, radius: f32) -> Option<GLRenderingPrimitive> {
        let mut geometry: VertexBuffers<Vertex, u16> = VertexBuffers::new();

        let mut geometry_builder = BuffersBuilder::new(&mut geometry, |pos: lyon::math::Point| {
            Vertex { _pos: [pos.x as f32, pos.y as f32] }
        });

        if radius > 0. {
            lyon::tessellation::basic_shapes::fill_rounded_rectangle(
                rect,
                &lyon::tessellation::basic_shapes::BorderRadii {
                    top_left: radius,
                    top_right: radius,
                    bottom_left: radius,
                    bottom_right: radius,
                },
                &lyon::tessellation::FillOptions::DEFAULT,
                &mut geometry_builder,
            )
            .unwrap();
        } else {
            lyon::tessellation::basic_shapes::fill_rectangle(
                rect,
                &lyon::tessellation::FillOptions::DEFAULT,
                &mut geometry_builder,
            )
            .unwrap();
        }

        self.fill_path_from_geometry(&geometry)
    }

    fn stroke_rectangle(
        &mut self,
        rect: &Rect,
        stroke_width: f32,
        radius: f32,
    ) -> Option<GLRenderingPrimitive> {
        let mut geometry: VertexBuffers<Vertex, u16> = VertexBuffers::new();

        let stroke_opts = StrokeOptions::DEFAULT.with_line_width(stroke_width);

        let mut geometry_builder =
            BuffersBuilder::new(&mut geometry, |pos: lyon::math::Point, _: StrokeAttributes| {
                Vertex { _pos: [pos.x as f32, pos.y as f32] }
            });

        if radius > 0. {
            lyon::tessellation::basic_shapes::stroke_rounded_rectangle(
                rect,
                &lyon::tessellation::basic_shapes::BorderRadii {
                    top_left: radius,
                    top_right: radius,
                    bottom_left: radius,
                    bottom_right: radius,
                },
                &stroke_opts,
                &mut geometry_builder,
            )
            .unwrap();
        } else {
            lyon::tessellation::basic_shapes::stroke_rectangle(
                rect,
                &stroke_opts,
                &mut geometry_builder,
            )
            .unwrap();
        }

        self.fill_path_from_geometry(&geometry)
    }

    fn create_image(
        context: &Rc<glow::Context>,
        atlas: &mut TextureAtlas,
        image: impl texture::UploadableAtlasImage,
    ) -> GLRenderingPrimitive {
        let image_size = Size::new(image.width() as _, image.height() as _);
        let rect =
            Rect::new(Point::new(0.0, 0.0), Size::new(image.width() as f32, image.height() as f32));

        let vertex1 = Vertex { _pos: [rect.min_x(), rect.min_y()] };
        let vertex2 = Vertex { _pos: [rect.max_x(), rect.min_y()] };
        let vertex3 = Vertex { _pos: [rect.max_x(), rect.max_y()] };
        let vertex4 = Vertex { _pos: [rect.min_x(), rect.max_y()] };

        let atlas_allocation = atlas.allocate_image_in_atlas(&context, image);

        let vertices = GLArrayBuffer::new(
            &context,
            &vec![vertex1, vertex2, vertex3, vertex1, vertex3, vertex4],
        );
        let texture_vertices =
            GLArrayBuffer::new(&context, &atlas_allocation.normalized_texture_coordinates());

        GLRenderingPrimitive::Texture {
            vertices,
            texture_vertices,
            texture: atlas_allocation,
            image_size,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn create_glyph_runs(
        &mut self,
        text: &str,
        font_family: &str,
        pixel_size: f32,
    ) -> GLRenderingPrimitive {
        let cached_glyphs = self.platform_data.glyph_cache.find_font(font_family, pixel_size);
        let mut cached_glyphs = cached_glyphs.borrow_mut();
        let mut atlas = self.texture_atlas.borrow_mut();
        let glyphs_runs = cached_glyphs.render_glyphs(&self.context, &mut atlas, text);
        GLRenderingPrimitive::GlyphRuns { glyph_runs: glyphs_runs }
    }

    #[cfg(target_arch = "wasm32")]
    fn create_glyph_runs(
        &mut self,
        text: &str,
        font_family: &str,
        pixel_size: f32,
    ) -> GLRenderingPrimitive {
        let font =
            sixtyfps_corelib::font::FONT_CACHE.with(|fc| fc.find_font(font_family, pixel_size));
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
        transform: &Matrix4<f32>,
        variables: SharedArray<RenderingVariable>,
    ) -> Vec<OpaqueRenderingPrimitive> {
        let matrix = self.root_matrix * transform;

        let mut rendering_var = variables.iter().peekable();

        let matrix = match rendering_var.peek() {
            Some(RenderingVariable::Translate(x_offset, y_offset)) => {
                rendering_var.next();
                matrix * Matrix4::from_translation(cgmath::Vector3::new(*x_offset, *y_offset, 0.))
            }
            _ => matrix,
        };

        primitive
            .gl_primitives
            .iter()
            .filter_map(|gl_primitive| {
                self.render_one_low_level_primitive(gl_primitive, &mut rendering_var, matrix)
            })
            .collect::<Vec<_>>()
    }
}

impl GLFrame {
    fn render_one_low_level_primitive<'a>(
        &mut self,
        gl_primitive: &GLRenderingPrimitive,
        rendering_var: &mut std::iter::Peekable<impl Iterator<Item = &'a RenderingVariable>>,
        matrix: Matrix4<f32>,
    ) -> Option<OpaqueRenderingPrimitive> {
        match gl_primitive {
            GLRenderingPrimitive::FillPath { vertices, indices } => {
                let col: ARGBColor<f32> = (*rendering_var.next().unwrap().as_color()).into();
                self.fill_path(&matrix, vertices, indices, col);
                None
            }
            GLRenderingPrimitive::Texture { vertices, texture_vertices, texture, image_size } => {
                let matrix = if let Some(scaled_width) = rendering_var.next() {
                    matrix
                        * Matrix4::from_nonuniform_scale(
                            scaled_width.as_scaled_width() / image_size.width,
                            1.,
                            1.,
                        )
                } else {
                    matrix
                };

                let matrix = if let Some(scaled_height) = rendering_var.next() {
                    matrix
                        * Matrix4::from_nonuniform_scale(
                            1.,
                            scaled_height.as_scaled_height() / image_size.height,
                            1.,
                        )
                } else {
                    matrix
                };

                self.render_texture(&matrix, vertices, texture_vertices, texture);
                None
            }
            GLRenderingPrimitive::GlyphRuns { glyph_runs } => {
                let col: ARGBColor<f32> = (*rendering_var.next().unwrap().as_color()).into();

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

                // Text selection is drawn in three phases:
                // 1. Draw the selection background rectangle, use regular stencil testing, write into the stencil buffer with GL_INCR
                // 2. Draw the glyphs, use regular stencil testing against current_stencil clip value + 1, don't write into the stencil buffer. This clips
                //    and draws only the glyphs of the selected text.
                // 3. Draw the glyphs, use regular stencil testing against current stencil clip value, don't write into the stencil buffer. This clips
                //    away the selected text and draws the non-selected part.
                // 4. We draw the selection background rectangle, use regular stencil testing, write into the stencil buffer with GL_DECR, use false color mask.
                //    This "removes" the selection rectangle from the stencil buffer again.

                let reset_stencil = match (rendering_var.peek(), &self.text_cursor_rect) {
                    (
                        Some(RenderingVariable::TextSelection(x, width, height)),
                        Some(text_cursor),
                    ) => {
                        rendering_var.next();
                        let foreground_color: ARGBColor<f32> =
                            (*rendering_var.next().unwrap().as_color()).into();
                        let background_color: ARGBColor<f32> =
                            (*rendering_var.next().unwrap().as_color()).into();

                        // Phase 1

                        let matrix = matrix
                            * Matrix4::from_translation(cgmath::Vector3::new(*x, 0., 0.))
                            * Matrix4::from_nonuniform_scale(*width, *height, 1.);

                        unsafe {
                            self.context.stencil_mask(0xff);
                            self.context.stencil_op(glow::KEEP, glow::KEEP, glow::INCR);
                        }

                        self.fill_path(
                            &matrix,
                            &text_cursor.vertices,
                            &text_cursor.indices,
                            background_color,
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

                        render_glyphs(foreground_color);

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
                    (reset_stencil, &self.text_cursor_rect)
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

                match (rendering_var.peek(), &self.text_cursor_rect) {
                    (Some(RenderingVariable::TextCursor(x, width, height)), Some(text_cursor)) => {
                        let matrix = matrix
                            * Matrix4::from_translation(cgmath::Vector3::new(*x, 0., 0.))
                            * Matrix4::from_nonuniform_scale(*width, *height, 1.);

                        self.fill_path(&matrix, &text_cursor.vertices, &text_cursor.indices, col);

                        rendering_var.next();
                    }
                    _ => {}
                }
                None
            }
            GLRenderingPrimitive::ApplyClip { vertices, indices } => {
                unsafe {
                    self.context.stencil_mask(0xff);
                    self.context.stencil_op(glow::KEEP, glow::KEEP, glow::INCR);
                    self.context.color_mask(false, false, false, false);
                }

                self.fill_path(
                    &matrix,
                    &vertices,
                    &indices,
                    ARGBColor { alpha: 0., red: 0., green: 0., blue: 0. },
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
                    }],
                })
            }

            GLRenderingPrimitive::ReleaseClip { vertices, indices } => {
                unsafe {
                    self.context.stencil_mask(0xff);
                    self.context.stencil_op(glow::KEEP, glow::KEEP, glow::DECR);
                    self.context.color_mask(false, false, false, false);
                }

                self.fill_path(
                    &matrix,
                    &vertices,
                    &indices,
                    ARGBColor { alpha: 0., red: 0., green: 0., blue: 0. },
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
            GLRenderingPrimitive::DynamicPrimitive { primitive } => primitive
                .borrow()
                .as_ref()
                .map(|p| self.render_one_low_level_primitive(p, rendering_var, matrix))
                .unwrap_or(None),
        }
    }

    fn fill_path(
        &self,
        matrix: &Matrix4<f32>,
        vertices: &GLArrayBuffer<Vertex>,
        indices: &GLIndexBuffer<u16>,
        color: ARGBColor<f32>,
    ) {
        self.path_shader.bind(
            &self.context,
            &to_gl_matrix(&matrix),
            &[color.red, color.green, color.blue, color.alpha],
            vertices,
            indices,
        );

        unsafe {
            self.context.draw_elements(glow::TRIANGLES, indices.len, glow::UNSIGNED_SHORT, 0);
        }

        self.path_shader.unbind(&self.context);
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
        color: ARGBColor<f32>,
    ) {
        self.glyph_shader.bind(
            &self.context,
            &to_gl_matrix(&matrix),
            &[color.red, color.green, color.blue, color.alpha],
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
pub mod native_widgets {}
