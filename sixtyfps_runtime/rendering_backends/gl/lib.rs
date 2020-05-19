use cgmath::Matrix4;
use glow::{Context as GLContext, HasContext};
use image::Pixel;
use lyon::path::math::Rect;
use lyon::tessellation::geometry_builder::{BuffersBuilder, VertexBuffers};
use lyon::tessellation::{FillAttributes, FillOptions, FillTessellator};
use pathfinder_geometry::{
    rect::RectI, transform2d::Transform2F, vector::Vector2F, vector::Vector2I,
};
use sixtyfps_corelib::abi::datastructures::ComponentVTable;
use sixtyfps_corelib::graphics::{
    Color, FillStyle, Frame as GraphicsFrame, GraphicsBackend, RenderingPrimitivesBuilder,
};
use std::cell::RefCell;
use std::marker;
use std::mem;

extern crate alloc;
use alloc::rc::Rc;

#[derive(Copy, Clone)]
struct Vertex {
    _pos: [f32; 2],
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
    GlyphRun {
        vertices: GLArrayBuffer<Vertex>,
        texture_vertices: GLArrayBuffer<Vertex>,
        texture: GLTexture,
        vertex_count: i32,
    },
}

#[derive(Clone)]
struct Shader {
    program: <GLContext as HasContext>::Program,
}

impl Shader {
    fn new(gl: &GLContext, vertex_shader_source: &str, fragment_shader_source: &str) -> Shader {
        let program = unsafe { gl.create_program().expect("Cannot create program") };

        let shader_sources = [
            (glow::VERTEX_SHADER, vertex_shader_source),
            (glow::FRAGMENT_SHADER, fragment_shader_source),
        ];

        let mut shaders = Vec::with_capacity(shader_sources.len());

        for (shader_type, shader_source) in shader_sources.iter() {
            unsafe {
                let shader = gl.create_shader(*shader_type).expect("Cannot create shader");
                gl.shader_source(shader, &shader_source);
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    panic!(gl.get_shader_info_log(shader));
                }
                gl.attach_shader(program, shader);
                shaders.push(shader);
            }
        }

        unsafe {
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                panic!(gl.get_program_info_log(program));
            }

            for shader in shaders {
                gl.detach_shader(program, shader);
                gl.delete_shader(shader);
            }
        }

        Shader { program }
    }

    fn use_program(&self, gl: &glow::Context) {
        unsafe {
            gl.use_program(Some(self.program));
        }
    }

    fn drop(&mut self, gl: &GLContext) {
        unsafe {
            gl.delete_program(self.program);
        }
    }
}

struct GLArrayBuffer<ArrayMemberType> {
    buffer_id: <GLContext as HasContext>::Buffer,
    _type_marker: marker::PhantomData<ArrayMemberType>,
}

impl<ArrayMemberType> GLArrayBuffer<ArrayMemberType> {
    fn new(gl: &glow::Context, data: &[ArrayMemberType]) -> Self {
        let buffer_id = unsafe { gl.create_buffer().expect("vertex buffer") };

        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(buffer_id));

            let byte_len = mem::size_of_val(&data[0]) * data.len() / mem::size_of::<u8>();
            let byte_slice = std::slice::from_raw_parts(data.as_ptr() as *const u8, byte_len);
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, byte_slice, glow::STATIC_DRAW);
        }

        Self { buffer_id, _type_marker: marker::PhantomData }
    }

    fn bind(&self, gl: &glow::Context, attribute_location: u32) {
        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.buffer_id));

            // TODO #5: generalize GL array buffer size/data_type handling beyond f32
            gl.vertex_attrib_pointer_f32(
                attribute_location,
                (mem::size_of::<ArrayMemberType>() / mem::size_of::<f32>()) as i32,
                glow::FLOAT,
                false,
                0,
                0,
            );
            gl.enable_vertex_attrib_array(attribute_location);
        }
    }

    // TODO #3: make sure we release GL resources
    /*
    fn drop(&mut self, gl: &glow::Context) {
        unsafe {
            gl.delete_buffer(self.buffer_id);
        }
    }
    */
}

struct GLIndexBuffer<IndexType> {
    buffer_id: <GLContext as HasContext>::Buffer,
    len: i32,
    _vertex_marker: marker::PhantomData<IndexType>,
}

impl<IndexType> GLIndexBuffer<IndexType> {
    fn new(gl: &glow::Context, data: &[IndexType]) -> Self {
        let buffer_id = unsafe { gl.create_buffer().expect("vertex buffer") };

        unsafe {
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(buffer_id));

            let byte_len = mem::size_of_val(&data[0]) * data.len() / mem::size_of::<u8>();
            let byte_slice = std::slice::from_raw_parts(data.as_ptr() as *const u8, byte_len);
            gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, byte_slice, glow::STATIC_DRAW);
        }

        Self { buffer_id, len: data.len() as i32, _vertex_marker: marker::PhantomData }
    }

    fn bind(&self, gl: &glow::Context) {
        unsafe {
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.buffer_id));
        }
    }

    // TODO #3: make sure we release GL resources
    /*
    fn drop(&mut self, gl: &glow::Context) {
        unsafe {
            gl.delete_buffer(self.buffer_id);
        }
    }
    */
}

#[derive(Copy, Clone)]
struct GLTexture {
    texture_id: <GLContext as HasContext>::Texture,
}

impl GLTexture {
    fn new_with_size_and_data(
        gl: &glow::Context,
        width: i32,
        height: i32,
        data: Option<&[u8]>,
    ) -> Self {
        let texture_id = unsafe { gl.create_texture().unwrap() };

        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(texture_id));

            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);

            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                width,
                height,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                data,
            )
        }

        Self { texture_id }
    }

    fn set_sub_image(
        &mut self,
        gl: &glow::Context,
        x: i32,
        y: i32,
        image: image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
    ) {
        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(self.texture_id));
            gl.tex_sub_image_2d_u8_slice(
                glow::TEXTURE_2D,
                0,
                x,
                y,
                image.width() as i32,
                image.height() as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                Some(&image.into_raw()),
            );
        }
    }

    fn bind_to_location(
        &self,
        gl: &glow::Context,
        texture_location: <glow::Context as glow::HasContext>::UniformLocation,
    ) {
        unsafe {
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.texture_id));
            gl.uniform_1_i32(Some(texture_location), 0);
        }
    }

    // TODO #3: make sure we release GL resources
    /*
    fn drop(&mut self, gl: &glow::Context) {
        unsafe {
            gl.delete_texture(self.texture_id);
        }
    }
    */
}

struct AtlasTextureAllocation {
    texture: GLTexture,
    texture_coordinates: RectI,
    normalized_coordinates: [Vertex; 6],
}

impl AtlasTextureAllocation {
    fn new(
        texture: GLTexture,
        allocator: &guillotiere::AtlasAllocator,
        allocation: guillotiere::Allocation,
    ) -> Self {
        let atlas_width = allocator.size().width as f32;
        let atlas_height = allocator.size().height as f32;
        let min = allocation.rectangle.min;
        let size = allocation.rectangle.max - allocation.rectangle.min;
        let origin = Vector2I::new(min.x, min.y);
        let size = Vector2I::new(size.x, size.y);
        let texture_coordinates = RectI::new(origin, size);

        let tex_left = (texture_coordinates.min_x() as f32) / atlas_width;
        let tex_top = (texture_coordinates.min_y() as f32) / atlas_height;
        let tex_right = (texture_coordinates.max_x() as f32) / atlas_width;
        let tex_bottom = (texture_coordinates.max_y() as f32) / atlas_height;

        let tex_vertex1 = Vertex { _pos: [tex_left, tex_top] };
        let tex_vertex2 = Vertex { _pos: [tex_right, tex_top] };
        let tex_vertex3 = Vertex { _pos: [tex_right, tex_bottom] };
        let tex_vertex4 = Vertex { _pos: [tex_left, tex_bottom] };

        AtlasTextureAllocation {
            texture,
            texture_coordinates,
            normalized_coordinates: [
                tex_vertex1,
                tex_vertex2,
                tex_vertex3,
                tex_vertex1,
                tex_vertex3,
                tex_vertex4,
            ],
        }
    }
}

struct GLAtlasTexture {
    texture: GLTexture,
    allocator: guillotiere::AtlasAllocator,
}

impl GLAtlasTexture {
    fn new(gl: &glow::Context) -> Self {
        let allocator = guillotiere::AtlasAllocator::new(guillotiere::Size::new(2048, 2048));
        let texture = GLTexture::new_with_size_and_data(
            gl,
            allocator.size().width,
            allocator.size().height,
            None,
        );
        Self { texture, allocator }
    }

    fn allocate(
        &mut self,
        requested_width: i32,
        requested_height: i32,
    ) -> Option<guillotiere::Allocation> {
        self.allocator.allocate(guillotiere::Size::new(requested_width, requested_height))
    }
}

struct AtlasAllocation {
    atlas_index: usize,
    sub_texture: AtlasTextureAllocation,
}

struct TextureAtlas {
    atlases: Vec<GLAtlasTexture>,
}

impl TextureAtlas {
    fn new() -> Self {
        Self { atlases: vec![] }
    }

    fn allocate_region(
        &mut self,
        gl: &glow::Context,
        requested_width: i32,
        requested_height: i32,
    ) -> AtlasAllocation {
        for (i, atlas) in self.atlases.iter_mut().enumerate() {
            if let Some(allocation) = atlas.allocate(requested_width, requested_height) {
                return AtlasAllocation {
                    atlas_index: i,
                    sub_texture: AtlasTextureAllocation::new(
                        atlas.texture,
                        &atlas.allocator,
                        allocation,
                    ),
                };
            }
        }

        let mut new_atlas = GLAtlasTexture::new(&gl);
        let atlas_allocation = new_atlas.allocate(requested_width, requested_height).unwrap();
        let atlas_index = self.atlases.len();
        let alloc = AtlasAllocation {
            atlas_index,
            sub_texture: AtlasTextureAllocation::new(
                new_atlas.texture,
                &new_atlas.allocator,
                atlas_allocation,
            ),
        };
        self.atlases.push(new_atlas);
        alloc
    }

    fn allocate_image_in_atlas(
        &mut self,
        gl: &glow::Context,
        image: image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
    ) -> AtlasAllocation {
        let requested_width = image.width() as i32;
        let requested_height = image.height() as i32;

        let allocation = self.allocate_region(gl, requested_width, requested_height);

        let texture = &mut self.atlases[allocation.atlas_index].texture;
        texture.set_sub_image(
            gl,
            allocation.sub_texture.texture_coordinates.min_x(),
            allocation.sub_texture.texture_coordinates.min_y(),
            image,
        );

        allocation
    }
}

struct PreRenderedGlyph {
    glyph_allocation: AtlasAllocation,
    advance: f32,
}

struct GLFont {
    font: font_kit::font::Font,
    glyphs: std::collections::hash_map::HashMap<u32, PreRenderedGlyph>,
}

impl Default for GLFont {
    fn default() -> Self {
        let font = font_kit::source::SystemSource::new()
            .select_best_match(
                &[font_kit::family_name::FamilyName::SansSerif],
                &font_kit::properties::Properties::new(),
            )
            .unwrap()
            .load()
            .unwrap();
        let glyphs = std::collections::hash_map::HashMap::new();
        Self { font, glyphs }
    }
}

impl GLFont {
    fn layout_glyphs<'a>(
        &'a mut self,
        gl: &glow::Context,
        atlas: &mut TextureAtlas,
        text: &'a str,
    ) -> GlyphIter<'a> {
        let pixel_size: f32 = 48.0 * 72. / 96.;

        let font_metrics = self.font.metrics();

        let scale_from_font_units = pixel_size / font_metrics.units_per_em as f32;

        let baseline_y = font_metrics.ascent * scale_from_font_units;
        let hinting = font_kit::hinting::HintingOptions::None;
        let raster_opts = font_kit::canvas::RasterizationOptions::GrayscaleAa;

        text.chars().for_each(|ch| {
            let glyph_id = self.font.glyph_for_char(ch).unwrap();
            if self.glyphs.contains_key(&glyph_id) {
                return;
            }

            let advance = self.font.advance(glyph_id).unwrap().x() * scale_from_font_units;

            // ### TODO: use tight bounding box
            let glyph_height =
                (font_metrics.ascent - font_metrics.descent + 1.) * scale_from_font_units;
            let glyph_width = advance;
            let mut canvas = font_kit::canvas::Canvas::new(
                Vector2I::new(glyph_width.ceil() as i32, glyph_height.ceil() as i32),
                font_kit::canvas::Format::A8,
            );
            self.font
                .rasterize_glyph(
                    &mut canvas,
                    glyph_id,
                    pixel_size,
                    Transform2F::from_translation(Vector2F::new(0., baseline_y)),
                    hinting,
                    raster_opts,
                )
                .unwrap();

            let glyph_image = image::ImageBuffer::from_fn(
                canvas.size.x() as u32,
                canvas.size.y() as u32,
                |x, y| {
                    let idx = (x as usize) + (y as usize) * canvas.stride;
                    let alpha = canvas.pixels[idx];
                    image::Rgba::<u8>::from_channels(0, 0, 0, alpha)
                },
            );

            let glyph_allocation = atlas.allocate_image_in_atlas(gl, glyph_image);

            let glyph = PreRenderedGlyph { glyph_allocation, advance };

            self.glyphs.insert(glyph_id, glyph);
        });

        GlyphIter { gl_font: self, char_it: text.chars() }
    }
}

struct GlyphIter<'a> {
    gl_font: &'a GLFont,
    char_it: std::str::Chars<'a>,
}

impl<'a> Iterator for GlyphIter<'a> {
    type Item = &'a PreRenderedGlyph;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ch) = self.char_it.next() {
            let glyph_id = self.gl_font.font.glyph_for_char(ch).unwrap();
            let glyph = &self.gl_font.glyphs[&glyph_id];
            Some(glyph)
        } else {
            None
        }
    }
}

pub struct GLRenderer {
    context: Rc<glow::Context>,
    path_program: Shader,
    image_program: Shader,
    texture_atlas: Rc<RefCell<TextureAtlas>>,
    font: Rc<RefCell<GLFont>>,
    #[cfg(target_arch = "wasm32")]
    window: winit::window::Window,
    #[cfg(not(target_arch = "wasm32"))]
    windowed_context: Option<glutin::WindowedContext<glutin::NotCurrent>>,
}

pub struct GLRenderingPrimitivesBuilder {
    context: Rc<glow::Context>,
    fill_tesselator: FillTessellator,
    texture_atlas: Rc<RefCell<TextureAtlas>>,
    font: Rc<RefCell<GLFont>>,

    #[cfg(not(target_arch = "wasm32"))]
    windowed_context: glutin::WindowedContext<glutin::PossiblyCurrent>,
}

pub struct GLFrame {
    context: Rc<glow::Context>,
    path_program: Shader,
    image_program: Shader,
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

        const PATH_VERTEX_SHADER: &str = r#"#version 100
        attribute vec2 pos;
        uniform vec4 vertcolor;
        uniform mat4 matrix;
        varying lowp vec4 fragcolor;

        void main() {
            gl_Position = matrix * vec4(pos, 0.0, 1);
            fragcolor = vertcolor;
        }"#;

        const PATH_FRAGMENT_SHADER: &str = r#"#version 100
        precision mediump float;
        varying lowp vec4 fragcolor;
        void main() {
            gl_FragColor = fragcolor;
        }"#;

        let path_program = Shader::new(&context, PATH_VERTEX_SHADER, PATH_FRAGMENT_SHADER);

        const IMAGE_VERTEX_SHADER: &str = r#"#version 100
        attribute vec2 pos;
        attribute vec2 tex_pos;
        uniform mat4 matrix;
        varying highp vec2 frag_tex_pos;
        void main() {
            gl_Position = matrix * vec4(pos, 0.0, 1);
            frag_tex_pos = tex_pos;
        }"#;

        const IMAGE_FRAGMENT_SHADER: &str = r#"#version 100
        varying highp vec2 frag_tex_pos;
        uniform sampler2D tex;
        void main() {
            gl_FragColor = texture2D(tex, frag_tex_pos);
        }"#;

        let image_program = Shader::new(&context, IMAGE_VERTEX_SHADER, IMAGE_FRAGMENT_SHADER);

        GLRenderer {
            context: Rc::new(context),
            path_program,
            image_program,
            texture_atlas: Rc::new(RefCell::new(TextureAtlas::new())),
            font: Rc::new(RefCell::new(GLFont::default())),
            #[cfg(target_arch = "wasm32")]
            window,
            #[cfg(not(target_arch = "wasm32"))]
            windowed_context: Some(unsafe { windowed_context.make_not_current().unwrap() }),
        }
    }
}

pub struct OpaqueRenderingPrimitive(GLRenderingPrimitive);

impl GraphicsBackend for GLRenderer {
    type RenderingPrimitive = OpaqueRenderingPrimitive;
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
            font: self.font.clone(),

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
            path_program: self.path_program.clone(),
            image_program: self.image_program.clone(),
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
    type RenderingPrimitive = OpaqueRenderingPrimitive;

    fn create_path_fill_primitive(
        &mut self,
        path: &lyon::path::Path,
        style: FillStyle,
    ) -> Self::RenderingPrimitive {
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

        OpaqueRenderingPrimitive(GLRenderingPrimitive::FillPath { vertices, indices, style })
    }

    fn create_image_primitive(
        &mut self,
        dest_rect: impl Into<Rect>,
        image: image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
    ) -> Self::RenderingPrimitive {
        let rect = dest_rect.into();

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

        OpaqueRenderingPrimitive(GLRenderingPrimitive::Texture {
            vertices,
            texture_vertices,
            texture: atlas_allocation.sub_texture.texture,
        })
    }

    fn create_glyphs(&mut self, text: &str, _color: &Color) -> Self::RenderingPrimitive {
        let mut glyph_vertices = vec![];
        let mut glyph_texture_vertices = vec![];

        let mut texture = None;

        let mut x = 0.;
        for glyph in self.font.borrow_mut().layout_glyphs(
            &self.context,
            &mut self.texture_atlas.borrow_mut(),
            text,
        ) {
            let glyph_width = glyph.glyph_allocation.sub_texture.texture_coordinates.width() as f32;
            let glyph_height =
                glyph.glyph_allocation.sub_texture.texture_coordinates.height() as f32;

            let vertex1 = Vertex { _pos: [x, 0.] };
            let vertex2 = Vertex { _pos: [x + glyph_width, 0.] };
            let vertex3 = Vertex { _pos: [x + glyph_width, glyph_height] };
            let vertex4 = Vertex { _pos: [x, glyph_height] };

            glyph_vertices
                .extend_from_slice(&[vertex1, vertex2, vertex3, vertex1, vertex3, vertex4]);

            glyph_texture_vertices
                .extend_from_slice(&glyph.glyph_allocation.sub_texture.normalized_coordinates);

            // ### TODO: support multi-atlas texture glyph runs
            texture = Some(glyph.glyph_allocation.sub_texture.texture);

            x += glyph.advance;
        }

        let vertices = GLArrayBuffer::new(&self.context, &glyph_vertices);
        let texture_vertices = GLArrayBuffer::new(&self.context, &glyph_texture_vertices);

        OpaqueRenderingPrimitive(GLRenderingPrimitive::GlyphRun {
            vertices,
            texture_vertices,
            texture: texture.unwrap(),
            vertex_count: glyph_vertices.len() as i32,
        })
    }
}

impl GraphicsFrame for GLFrame {
    type RenderingPrimitive = OpaqueRenderingPrimitive;

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
        match &primitive.0 {
            GLRenderingPrimitive::FillPath { vertices, indices, style } => {
                self.path_program.use_program(&self.context);

                let matrix_location = unsafe {
                    self.context.get_uniform_location(self.path_program.program, "matrix")
                };
                unsafe {
                    self.context.uniform_matrix_4_f32_slice(matrix_location, false, &gl_matrix)
                };

                let (r, g, b, a) = match style {
                    FillStyle::SolidColor(color) => color.as_rgba_f32(),
                };

                let color_location = unsafe {
                    self.context.get_uniform_location(self.path_program.program, "vertcolor")
                };
                unsafe { self.context.uniform_4_f32(color_location, r, g, b, a) };

                let vertex_attribute_location = unsafe {
                    self.context.get_attrib_location(self.path_program.program, "pos").unwrap()
                };
                vertices.bind(&self.context, vertex_attribute_location);

                indices.bind(&self.context);

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
                self.image_program.use_program(&self.context);

                let matrix_location = unsafe {
                    self.context.get_uniform_location(self.image_program.program, "matrix")
                };
                unsafe {
                    self.context.uniform_matrix_4_f32_slice(matrix_location, false, &gl_matrix)
                };

                let texture_location = unsafe {
                    self.context.get_uniform_location(self.image_program.program, "tex").unwrap()
                };
                texture.bind_to_location(&self.context, texture_location);

                let vertex_attribute_location = unsafe {
                    self.context.get_attrib_location(self.image_program.program, "pos").unwrap()
                };
                vertices.bind(&self.context, vertex_attribute_location);

                let vertex_texture_attribute_location = unsafe {
                    self.context.get_attrib_location(self.image_program.program, "tex_pos").unwrap()
                };
                texture_vertices.bind(&self.context, vertex_texture_attribute_location);

                unsafe {
                    self.context.draw_arrays(glow::TRIANGLES, 0, 6);
                }
            }
            GLRenderingPrimitive::GlyphRun {
                vertices,
                texture_vertices,
                texture,
                vertex_count,
            } => {
                self.image_program.use_program(&self.context);

                let matrix_location = unsafe {
                    self.context.get_uniform_location(self.image_program.program, "matrix")
                };
                unsafe {
                    self.context.uniform_matrix_4_f32_slice(matrix_location, false, &gl_matrix)
                };

                let texture_location = unsafe {
                    self.context.get_uniform_location(self.image_program.program, "tex").unwrap()
                };
                texture.bind_to_location(&self.context, texture_location);

                let vertex_attribute_location = unsafe {
                    self.context.get_attrib_location(self.image_program.program, "pos").unwrap()
                };
                vertices.bind(&self.context, vertex_attribute_location);

                let vertex_texture_attribute_location = unsafe {
                    self.context.get_attrib_location(self.image_program.program, "tex_pos").unwrap()
                };
                texture_vertices.bind(&self.context, vertex_texture_attribute_location);

                unsafe {
                    self.context.draw_arrays(glow::TRIANGLES, 0, *vertex_count);
                }
            }
        }
    }
}

impl Drop for GLRenderer {
    fn drop(&mut self) {
        self.path_program.drop(&self.context);
        self.image_program.drop(&self.context);
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

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
