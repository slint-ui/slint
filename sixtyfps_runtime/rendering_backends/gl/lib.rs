use cgmath::Matrix4;
use glow::{Context as GLContext, HasContext};
#[cfg(not(target_arch = "wasm32"))]
use itertools::Itertools;
use lyon::tessellation::geometry_builder::{BuffersBuilder, VertexBuffers};
use lyon::tessellation::{
    FillAttributes, FillOptions, FillTessellator, StrokeAttributes, StrokeOptions,
    StrokeTessellator,
};
use sixtyfps_corelib::abi::datastructures::{
    Color, ComponentWindow, ComponentWindowOpaque, Point, Rect, RenderingPrimitive, Resource, Size,
};
use sixtyfps_corelib::graphics::{
    FillStyle, Frame as GraphicsFrame, GraphicsBackend, GraphicsWindow, HasRenderingPrimitive,
    RenderingPrimitivesBuilder,
};
use smallvec::{smallvec, SmallVec};
use std::cell::RefCell;

extern crate alloc;
use alloc::rc::Rc;

mod texture;
use texture::{GLTexture, TextureAtlas};

mod shader;
use shader::{ImageShader, PathShader};

#[cfg(not(target_arch = "wasm32"))]
use shader::GlyphShader;

mod buffers;
use buffers::{GLArrayBuffer, GLIndexBuffer};

#[cfg(not(target_arch = "wasm32"))]
mod glyphcache;
#[cfg(not(target_arch = "wasm32"))]
use glyphcache::GlyphCache;

#[cfg(not(target_arch = "wasm32"))]
struct PlatformData {
    glyph_cache: GlyphCache,
    glyph_shader: GlyphShader,
}

#[cfg(target_arch = "wasm32")]
struct PlatformData {}

impl PlatformData {
    #[cfg(not(target_arch = "wasm32"))]
    fn new(context: &glow::Context) -> Self {
        Self { glyph_cache: GlyphCache::default(), glyph_shader: GlyphShader::new(&context) }
    }

    #[cfg(target_arch = "wasm32")]
    fn new(_context: &glow::Context) -> Self {
        Self {}
    }

    #[cfg(target_arch = "wasm32")]
    fn drop(&mut self, _context: &glow::Context) {}
    #[cfg(not(target_arch = "wasm32"))]
    fn drop(&mut self, context: &glow::Context) {
        self.glyph_shader.drop(&context);
    }
}

#[derive(Copy, Clone)]
pub(crate) struct Vertex {
    _pos: [f32; 2],
}

#[cfg(not(target_arch = "wasm32"))]
struct GlyphRun {
    vertices: GLArrayBuffer<Vertex>,
    texture_vertices: GLArrayBuffer<Vertex>,
    texture: GLTexture,
    vertex_count: i32,
}

enum GLRenderingPrimitive {
    FillPath {
        vertices: GLArrayBuffer<Vertex>,
        indices: GLIndexBuffer<u16>,
        style: FillStyle,
    },
    Texture {
        vertices: GLArrayBuffer<Vertex>,
        texture_vertices: GLArrayBuffer<Vertex>,
        texture: GLTexture,
    },
    #[cfg(not(target_arch = "wasm32"))]
    GlyphRuns {
        glyph_runs: Vec<GlyphRun>,
        color: Color,
    },
}

pub struct GLRenderer {
    context: Rc<glow::Context>,
    path_shader: PathShader,
    image_shader: ImageShader,
    platform_data: Rc<RefCell<PlatformData>>,
    texture_atlas: Rc<RefCell<TextureAtlas>>,
    #[cfg(target_arch = "wasm32")]
    window: winit::window::Window,
    #[cfg(not(target_arch = "wasm32"))]
    windowed_context: Option<glutin::WindowedContext<glutin::NotCurrent>>,
}

pub struct GLRenderingPrimitivesBuilder {
    context: Rc<glow::Context>,
    fill_tesselator: FillTessellator,
    stroke_tesselator: StrokeTessellator,
    texture_atlas: Rc<RefCell<TextureAtlas>>,
    #[cfg(not(target_arch = "wasm32"))]
    platform_data: Rc<RefCell<PlatformData>>,

    #[cfg(not(target_arch = "wasm32"))]
    windowed_context: glutin::WindowedContext<glutin::PossiblyCurrent>,
}

pub struct GLFrame {
    context: Rc<glow::Context>,
    path_shader: PathShader,
    image_shader: ImageShader,
    #[cfg(not(target_arch = "wasm32"))]
    platform_data: Rc<RefCell<PlatformData>>,
    root_matrix: cgmath::Matrix4<f32>,
    #[cfg(not(target_arch = "wasm32"))]
    windowed_context: glutin::WindowedContext<glutin::PossiblyCurrent>,
}

impl GLRenderer {
    pub fn new(
        event_loop: &winit::event_loop::EventLoop<()>,
        window_builder: winit::window::WindowBuilder,
    ) -> GLRenderer {
        #[cfg(not(target_arch = "wasm32"))]
        let (windowed_context, context) = {
            let windowed_context = glutin::ContextBuilder::new()
                .with_vsync(true)
                .build_windowed(window_builder, &event_loop)
                .unwrap();
            let windowed_context = unsafe { windowed_context.make_current().unwrap() };

            let gl_context = glow::Context::from_loader_function(|s| {
                windowed_context.get_proc_address(s) as *const _
            });

            (windowed_context, gl_context)
        };

        #[cfg(target_arch = "wasm32")]
        let (window, context) = {
            let canvas = web_sys::window()
                .unwrap()
                .document()
                .unwrap()
                .get_element_by_id("canvas")
                .unwrap()
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .unwrap();

            use winit::platform::web::WindowBuilderExtWebSys;
            use winit::platform::web::WindowExtWebSys;

            let window = window_builder.with_canvas(Some(canvas)).build(&event_loop).unwrap();

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

        let path_shader = PathShader::new(&context);
        let image_shader = ImageShader::new(&context);
        let platform_data = Rc::new(RefCell::new(PlatformData::new(&context)));

        GLRenderer {
            context: Rc::new(context),
            path_shader,
            image_shader,
            platform_data,
            texture_atlas: Rc::new(RefCell::new(TextureAtlas::new())),
            #[cfg(target_arch = "wasm32")]
            window,
            #[cfg(not(target_arch = "wasm32"))]
            windowed_context: Some(unsafe { windowed_context.make_not_current().unwrap() }),
        }
    }
}

type GLRenderingPrimitives = SmallVec<[GLRenderingPrimitive; 1]>;

pub struct OpaqueRenderingPrimitive {
    gl_primitives: GLRenderingPrimitives,
    rendering_primitive: RenderingPrimitive,
}

impl HasRenderingPrimitive for OpaqueRenderingPrimitive {
    fn primitive(&self) -> &RenderingPrimitive {
        &self.rendering_primitive
    }
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
        }
    }

    fn finish_primitives(&mut self, _builder: Self::RenderingPrimitivesBuilder) {
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
        }

        let (r, g, b, a) = clear_color.as_rgba_f32();
        unsafe {
            self.context.clear_color(r, g, b, a);
            self.context.clear(glow::COLOR_BUFFER_BIT);
        };

        GLFrame {
            context: self.context.clone(),
            path_shader: self.path_shader.clone(),
            image_shader: self.image_shader.clone(),
            #[cfg(not(target_arch = "wasm32"))]
            platform_data: self.platform_data.clone(),
            root_matrix: cgmath::ortho(0.0, width as f32, height as f32, 0.0, -1., 1.0),
            #[cfg(not(target_arch = "wasm32"))]
            windowed_context: current_windowed_context,
        }
    }

    fn present_frame(&mut self, _frame: Self::Frame) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            _frame.windowed_context.swap_buffers().unwrap();

            self.windowed_context =
                Some(unsafe { _frame.windowed_context.make_not_current().unwrap() });
        }
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

    fn create(&mut self, primitive: RenderingPrimitive) -> Self::LowLevelRenderingPrimitive {
        OpaqueRenderingPrimitive {
            gl_primitives: match &primitive {
                RenderingPrimitive::NoContents => smallvec::SmallVec::new(),
                RenderingPrimitive::Rectangle { x: _, y: _, width, height, color } => {
                    use lyon::math::Point;

                    let rect = Rect::new(Point::default(), Size::new(*width, *height));
                    self.fill_rectangle(&rect, 0., *color).into_iter().collect()
                }
                RenderingPrimitive::BorderRectangle {
                    x: _,
                    y: _,
                    width,
                    height,
                    color,
                    border_width,
                    border_radius,
                    border_color,
                } => {
                    use lyon::math::Point;

                    let rect = Rect::new(Point::default(), Size::new(*width, *height));

                    let mut primitives: SmallVec<_> =
                        self.fill_rectangle(&rect, *border_radius, *color).into_iter().collect();

                    if *border_width > 0. {
                        let stroke = self.stroke_rectangle(
                            &Rect::new(Point::default(), Size::new(*width, *height)),
                            *border_color,
                            *border_width,
                            *border_radius,
                        );
                        primitives.extend(stroke);
                    }

                    primitives
                }
                RenderingPrimitive::Image { x: _, y: _, source } => {
                    match source {
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
                            smallvec![self.create_image(image)]
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
                            smallvec![self.create_image(image)]
                        }
                        Resource::EmbeddedDataOwned { width, height, data } => {
                            let image = image::ImageBuffer::<image::Rgba<u8>, &[u8]>::from_raw(
                                *width,
                                *height,
                                data.as_slice(),
                            )
                            .unwrap();
                            smallvec![self.create_image(image)]
                        }
                        Resource::None => SmallVec::new(),
                    }
                }
                RenderingPrimitive::Text {
                    x: _,
                    y: _,
                    text,
                    font_family,
                    font_pixel_size,
                    color,
                } => {
                    let pixel_size =
                        if *font_pixel_size != 0. { *font_pixel_size } else { 48.0 * 72. / 96. };
                    smallvec![self.create_glyph_runs(text, font_family, pixel_size, *color)]
                }
                RenderingPrimitive::Path {
                    x: _,
                    y: _,
                    width,
                    height,
                    elements,
                    fill_color,
                    stroke_color,
                    stroke_width,
                } => {
                    let mut primitives = SmallVec::new();

                    let path_iter = elements.iter().fitted(*width, *height);

                    if *fill_color != Color::TRANSPARENT {
                        primitives.extend(
                            self.fill_path(path_iter.iter(), FillStyle::SolidColor(*fill_color))
                                .into_iter(),
                        );
                    }

                    if *stroke_color != Color::TRANSPARENT {
                        primitives.extend(
                            self.stroke_path(path_iter.iter(), *stroke_color, *stroke_width)
                                .into_iter(),
                        );
                    }

                    primitives
                }
            },
            rendering_primitive: primitive,
        }
    }
}

impl GLRenderingPrimitivesBuilder {
    fn fill_path_from_geometry(
        &self,
        geometry: &VertexBuffers<Vertex, u16>,
        style: FillStyle,
    ) -> Option<GLRenderingPrimitive> {
        if geometry.vertices.len() == 0 || geometry.indices.len() == 0 {
            return None;
        }

        let vertices = GLArrayBuffer::new(&self.context, &geometry.vertices);
        let indices = GLIndexBuffer::new(&self.context, &geometry.indices);

        Some(GLRenderingPrimitive::FillPath { vertices, indices, style }.into())
    }

    fn fill_path(
        &mut self,
        path: impl IntoIterator<Item = lyon::path::PathEvent>,
        style: FillStyle,
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

        self.fill_path_from_geometry(&geometry, style)
    }

    fn stroke_path(
        &mut self,
        path: impl IntoIterator<Item = lyon::path::PathEvent>,
        stroke_color: Color,
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

        self.fill_path_from_geometry(&geometry, FillStyle::SolidColor(stroke_color))
    }

    fn fill_rectangle(
        &mut self,
        rect: &Rect,
        radius: f32,
        color: Color,
    ) -> Option<GLRenderingPrimitive> {
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

        self.fill_path_from_geometry(&geometry, FillStyle::SolidColor(color))
    }

    fn stroke_rectangle(
        &mut self,
        rect: &Rect,
        stroke_color: Color,
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

        self.fill_path_from_geometry(&geometry, FillStyle::SolidColor(stroke_color))
    }

    fn create_image(
        &mut self,
        image: image::ImageBuffer<image::Rgba<u8>, &[u8]>,
    ) -> GLRenderingPrimitive {
        let source_size = image.dimensions();
        let rect =
            Rect::new(Point::new(0.0, 0.0), Size::new(source_size.0 as f32, source_size.1 as f32));

        let vertex1 = Vertex { _pos: [rect.min_x(), rect.min_y()] };
        let vertex2 = Vertex { _pos: [rect.max_x(), rect.min_y()] };
        let vertex3 = Vertex { _pos: [rect.max_x(), rect.max_y()] };
        let vertex4 = Vertex { _pos: [rect.min_x(), rect.max_y()] };

        let mut atlas = self.texture_atlas.borrow_mut();
        let atlas_allocation = atlas.allocate_image_in_atlas(&self.context, image);

        let vertices = GLArrayBuffer::new(
            &self.context,
            &vec![vertex1, vertex2, vertex3, vertex1, vertex3, vertex4],
        );
        let texture_vertices =
            GLArrayBuffer::new(&self.context, &atlas_allocation.sub_texture.normalized_coordinates);

        GLRenderingPrimitive::Texture {
            vertices,
            texture_vertices,
            texture: atlas_allocation.sub_texture.texture,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn create_glyph_runs(
        &mut self,
        text: &str,
        font_family: &str,
        pixel_size: f32,
        color: Color,
    ) -> GLRenderingPrimitive {
        let mut pd = self.platform_data.borrow_mut();
        let cached_glyphs = pd.glyph_cache.find_font(font_family, pixel_size);
        let mut cached_glyphs = cached_glyphs.borrow_mut();
        let mut atlas = self.texture_atlas.borrow_mut();
        let glyphs = cached_glyphs.string_to_glyphs(&self.context, &mut atlas, text);

        let mut x = 0.;

        let glyph_runs = cached_glyphs
            .layout_glyphs(glyphs)
            .map(|cached_glyph| {
                let glyph_width =
                    cached_glyph.glyph_allocation.sub_texture.texture_coordinates.width() as f32;
                let glyph_height =
                    cached_glyph.glyph_allocation.sub_texture.texture_coordinates.height() as f32;

                let vertex1 = Vertex { _pos: [x, 0.] };
                let vertex2 = Vertex { _pos: [x + glyph_width, 0.] };
                let vertex3 = Vertex { _pos: [x + glyph_width, glyph_height] };
                let vertex4 = Vertex { _pos: [x, glyph_height] };

                let vertices = [vertex1, vertex2, vertex3, vertex1, vertex3, vertex4];
                let texture_vertices =
                    cached_glyph.glyph_allocation.sub_texture.normalized_coordinates;

                let texture = cached_glyph.glyph_allocation.sub_texture.texture;

                x += cached_glyph.advance;

                (vertices, texture_vertices, texture)
            })
            .group_by(|(_, _, texture)| *texture)
            .into_iter()
            .map(|(texture, glyph_it)| {
                let glyph_count = glyph_it.size_hint().0;
                let mut vertices: Vec<Vertex> = Vec::with_capacity(glyph_count * 6);
                let mut texture_vertices: Vec<Vertex> = Vec::with_capacity(glyph_count * 6);

                for (glyph_vertices, glyph_texture_vertices) in
                    glyph_it.map(|(vertices, texture_vertices, _)| (vertices, texture_vertices))
                {
                    vertices.extend(&glyph_vertices);
                    texture_vertices.extend(&glyph_texture_vertices);
                }

                let vertex_count = vertices.len() as i32;
                GlyphRun {
                    vertices: GLArrayBuffer::new(&self.context, &vertices),
                    texture_vertices: GLArrayBuffer::new(&self.context, &texture_vertices),
                    texture,
                    vertex_count,
                }
            })
            .collect();

        GLRenderingPrimitive::GlyphRuns { glyph_runs, color }
    }

    #[cfg(target_arch = "wasm32")]
    fn create_glyph_runs(
        &mut self,
        text: &str,
        font_family: &str,
        pixel_size: f32,
        color: Color,
    ) -> GLRenderingPrimitive {
        let font =
            sixtyfps_corelib::font::FONT_CACHE.with(|fc| fc.find_font(font_family, pixel_size));
        let text_canvas = font.render_text(text, color);

        let texture = GLTexture::new_from_canvas(&self.context, &text_canvas);

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

        GLRenderingPrimitive::Texture { vertices, texture_vertices, texture }
    }
}

impl GraphicsFrame for GLFrame {
    type LowLevelRenderingPrimitive = OpaqueRenderingPrimitive;

    fn render_primitive(&mut self, primitive: &OpaqueRenderingPrimitive, transform: &Matrix4<f32>) {
        let matrix = self.root_matrix * transform;
        let gl_matrix: [f32; 16] = [
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
        ];
        primitive.gl_primitives.iter().for_each(|gl_primitive| match gl_primitive {
            GLRenderingPrimitive::FillPath { vertices, indices, style } => {
                let (r, g, b, a) = match style {
                    FillStyle::SolidColor(color) => color.as_rgba_f32(),
                };

                self.path_shader.bind(&self.context, &gl_matrix, &[r, g, b, a], vertices, indices);

                unsafe {
                    self.context.draw_elements(
                        glow::TRIANGLES,
                        indices.len,
                        glow::UNSIGNED_SHORT,
                        0,
                    );
                }
            }
            GLRenderingPrimitive::Texture { vertices, texture_vertices, texture } => {
                self.image_shader.bind(
                    &self.context,
                    &gl_matrix,
                    texture,
                    vertices,
                    texture_vertices,
                );

                unsafe {
                    self.context.draw_arrays(glow::TRIANGLES, 0, 6);
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            GLRenderingPrimitive::GlyphRuns { glyph_runs, color } => {
                let (r, g, b, a) = color.as_rgba_f32();

                for GlyphRun { vertices, texture_vertices, texture, vertex_count } in glyph_runs {
                    self.platform_data.borrow().glyph_shader.bind(
                        &self.context,
                        &gl_matrix,
                        &[r, g, b, a],
                        texture,
                        vertices,
                        texture_vertices,
                    );

                    unsafe {
                        self.context.draw_arrays(glow::TRIANGLES, 0, *vertex_count);
                    }
                }
            }
        });
    }
}

impl Drop for GLRenderer {
    fn drop(&mut self) {
        self.path_shader.drop(&self.context);
        self.image_shader.drop(&self.context);
        self.platform_data.borrow_mut().drop(&self.context);
    }
}

#[no_mangle]
pub unsafe extern "C" fn sixtyfps_component_window_gl_renderer_init(
    out: *mut ComponentWindowOpaque,
) {
    assert_eq!(
        core::mem::size_of::<ComponentWindow>(),
        core::mem::size_of::<ComponentWindowOpaque>()
    );
    core::ptr::write(out as *mut ComponentWindow, create_gl_window());
}

pub fn create_gl_window() -> ComponentWindow {
    ComponentWindow::new(GraphicsWindow::new(|event_loop, window_builder| {
        GLRenderer::new(&event_loop.get_winit_event_loop(), window_builder)
    }))
}

#[doc(hidden)]
#[cold]
pub fn use_modules() {
    sixtyfps_corelib::use_modules();
}
