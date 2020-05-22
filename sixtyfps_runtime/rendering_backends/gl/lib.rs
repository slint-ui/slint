use cgmath::Matrix4;
use glow::{Context as GLContext, HasContext};
use itertools::Itertools;
use lyon::tessellation::geometry_builder::{BuffersBuilder, VertexBuffers};
use lyon::tessellation::{FillAttributes, FillOptions, FillTessellator};
use sixtyfps_corelib::abi::datastructures::{ComponentVTable, Point, Rect, Size};
use sixtyfps_corelib::graphics::{
    Color, FillStyle, Frame as GraphicsFrame, GraphicsBackend, HasRenderingPrimitive,
    RenderingPrimitive, RenderingPrimitivesBuilder,
};
use std::cell::RefCell;

extern crate alloc;
use alloc::rc::Rc;

mod texture;
use texture::{GLTexture, TextureAtlas};

mod shader;
use shader::{GlyphShader, ImageShader, PathShader};

mod buffers;
use buffers::{GLArrayBuffer, GLIndexBuffer};

mod text;

mod fontcache;
use fontcache::FontCache;

#[derive(Copy, Clone)]
pub(crate) struct Vertex {
    _pos: [f32; 2],
}

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
    GlyphRuns {
        glyph_runs: Vec<GlyphRun>,
        color: Color,
    },
}

pub struct GLRenderer {
    context: Rc<glow::Context>,
    path_shader: PathShader,
    image_shader: ImageShader,
    glyph_shader: GlyphShader,
    texture_atlas: Rc<RefCell<TextureAtlas>>,
    font_cache: Rc<RefCell<FontCache>>,
    #[cfg(target_arch = "wasm32")]
    window: winit::window::Window,
    #[cfg(not(target_arch = "wasm32"))]
    windowed_context: Option<glutin::WindowedContext<glutin::NotCurrent>>,
}

pub struct GLRenderingPrimitivesBuilder {
    context: Rc<glow::Context>,
    fill_tesselator: FillTessellator,
    texture_atlas: Rc<RefCell<TextureAtlas>>,
    font_cache: Rc<RefCell<FontCache>>,

    #[cfg(not(target_arch = "wasm32"))]
    windowed_context: glutin::WindowedContext<glutin::PossiblyCurrent>,
}

pub struct GLFrame {
    context: Rc<glow::Context>,
    path_shader: PathShader,
    image_shader: ImageShader,
    glyph_shader: GlyphShader,
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
        let glyph_shader = GlyphShader::new(&context);

        GLRenderer {
            context: Rc::new(context),
            path_shader,
            image_shader,
            glyph_shader,
            texture_atlas: Rc::new(RefCell::new(TextureAtlas::new())),
            font_cache: Rc::new(RefCell::new(FontCache::default())),
            #[cfg(target_arch = "wasm32")]
            window,
            #[cfg(not(target_arch = "wasm32"))]
            windowed_context: Some(unsafe { windowed_context.make_not_current().unwrap() }),
        }
    }
}

pub struct OpaqueRenderingPrimitive {
    gl_primitive: GLRenderingPrimitive,
    rendering_primitive: Option<RenderingPrimitive>,
}

impl HasRenderingPrimitive for OpaqueRenderingPrimitive {
    fn primitive(&self) -> Option<&RenderingPrimitive> {
        self.rendering_primitive.as_ref()
    }
}

impl From<GLRenderingPrimitive> for OpaqueRenderingPrimitive {
    fn from(gl_primitive: GLRenderingPrimitive) -> Self {
        Self { gl_primitive, rendering_primitive: None }
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
            texture_atlas: self.texture_atlas.clone(),
            font_cache: self.font_cache.clone(),

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
            glyph_shader: self.glyph_shader.clone(),
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

    fn create_path_fill_primitive(
        &mut self,
        path: &lyon::path::Path,
        style: FillStyle,
    ) -> Self::LowLevelRenderingPrimitive {
        let mut geometry: VertexBuffers<Vertex, u16> = VertexBuffers::new();

        let fill_opts = FillOptions::default();
        self.fill_tesselator
            .tessellate_path(
                path.as_slice(),
                &fill_opts,
                &mut BuffersBuilder::new(
                    &mut geometry,
                    |pos: lyon::math::Point, _: FillAttributes| Vertex {
                        _pos: [pos.x as f32, pos.y as f32],
                    },
                ),
            )
            .unwrap();

        let vertices = GLArrayBuffer::new(&self.context, &geometry.vertices);
        let indices = GLIndexBuffer::new(&self.context, &geometry.indices);

        GLRenderingPrimitive::FillPath { vertices, indices, style }.into()
    }

    fn create_image_primitive(
        &mut self,
        image: image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
    ) -> Self::LowLevelRenderingPrimitive {
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
        .into()
    }

    fn create_glyphs(
        &mut self,
        text: &str,
        font_family: &str,
        pixel_size: f32,
        color: Color,
    ) -> Self::LowLevelRenderingPrimitive {
        let mut font_cache = self.font_cache.borrow_mut();
        let font = font_cache.find_font(font_family, pixel_size);
        let mut font = font.borrow_mut();
        let glyphs =
            font.string_to_glyphs(&self.context, &mut self.texture_atlas.borrow_mut(), text);

        let mut x = 0.;

        let glyph_runs = font
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

        GLRenderingPrimitive::GlyphRuns { glyph_runs, color }.into()
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
        match &primitive.gl_primitive {
            GLRenderingPrimitive::FillPath { vertices, indices, style } => {
                let (r, g, b, a) = match style {
                    FillStyle::SolidColor(color) => color.as_rgba_f32(),
                };

                self.path_shader.bind(&self.context, &gl_matrix, &[r, g, b, a], vertices, indices);

                unsafe {
                    self.context.draw_elements(
                        glow::TRIANGLE_STRIP,
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
            GLRenderingPrimitive::GlyphRuns { glyph_runs, color } => {
                let (r, g, b, a) = color.as_rgba_f32();

                for GlyphRun { vertices, texture_vertices, texture, vertex_count } in glyph_runs {
                    self.glyph_shader.bind(
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
        }
    }
}

impl Drop for GLRenderer {
    fn drop(&mut self) {
        self.path_shader.drop(&self.context);
        self.image_shader.drop(&self.context);
        self.glyph_shader.drop(&self.context);
    }
}

/// Run the given component
/// Both pointer must be valid until the call to vtable.destroy
/// vtable will is a *const, and inner like a *mut
#[no_mangle]
pub extern "C" fn sixtyfps_runtime_run_component_with_gl_renderer(
    component: vtable::VRefMut<'static, ComponentVTable>,
) {
    sixtyfps_corelib::run_component(component, |event_loop, window_builder| {
        GLRenderer::new(&event_loop, window_builder)
    });
}
