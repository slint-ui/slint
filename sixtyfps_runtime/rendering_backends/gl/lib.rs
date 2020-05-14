use cgmath::Matrix4;
use core::ptr::NonNull;
use glow::{Context as GLContext, HasContext};
use lyon::path::math::Rect;
use lyon::tessellation::geometry_builder::{BuffersBuilder, VertexBuffers};
use lyon::tessellation::{FillAttributes, FillOptions, FillTessellator};
use sixtyfps_corelib::abi::datastructures::{ComponentImpl, ComponentType};
use sixtyfps_corelib::graphics::{
    Color, FillStyle, Frame as GraphicsFrame, GraphicsBackend, RenderingPrimitivesBuilder,
};
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

struct GLTexture {
    texture_id: <GLContext as HasContext>::Texture,
}

impl GLTexture {
    fn new(gl: &glow::Context, image: image::ImageBuffer<image::Rgba<u8>, Vec<u8>>) -> Self {
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
                image.width() as i32,
                image.height() as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                Some(&image.into_raw()),
            )
        }

        Self { texture_id }
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

pub struct GLRenderer {
    context: Rc<glow::Context>,
    path_program: Shader,
    image_program: Shader,
    #[cfg(target_arch = "wasm32")]
    window: winit::window::Window,
    #[cfg(not(target_arch = "wasm32"))]
    windowed_context: Option<glutin::WindowedContext<glutin::NotCurrent>>,
}

pub struct GLRenderingPrimitivesBuilder {
    context: Rc<glow::Context>,
    fill_tesselator: FillTessellator,

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
        source_rect: impl Into<Rect>,
        dest_rect: impl Into<Rect>,
        image: image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
    ) -> Self::RenderingPrimitive {
        let rect = dest_rect.into();
        let src_rect = source_rect.into();
        let image_width = image.width() as f32;
        let image_height = image.height() as f32;
        let src_left = src_rect.min_x() / image_width;
        let src_top = src_rect.min_y() / image_height;
        let src_right = src_rect.max_x() / image_width;
        let src_bottom = src_rect.max_y() / image_height;

        let vertex1 = Vertex { _pos: [rect.min_x(), rect.min_y()] };
        let tex_vertex1 = Vertex { _pos: [src_left, src_top] };
        let vertex2 = Vertex { _pos: [rect.max_x(), rect.min_y()] };
        let tex_vertex2 = Vertex { _pos: [src_right, src_top] };
        let vertex3 = Vertex { _pos: [rect.max_x(), rect.max_y()] };
        let tex_vertex3 = Vertex { _pos: [src_right, src_bottom] };
        let vertex4 = Vertex { _pos: [rect.min_x(), rect.max_y()] };
        let tex_vertex4 = Vertex { _pos: [src_left, src_bottom] };

        let vertices = GLArrayBuffer::new(
            &self.context,
            &vec![vertex1, vertex2, vertex3, vertex1, vertex3, vertex4],
        );
        let texture_vertices = GLArrayBuffer::new(
            &self.context,
            &vec![tex_vertex1, tex_vertex2, tex_vertex3, tex_vertex1, tex_vertex3, tex_vertex4],
        );

        let texture = GLTexture::new(&self.context, image);

        OpaqueRenderingPrimitive(GLRenderingPrimitive::Texture {
            vertices,
            texture_vertices,
            texture,
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
    component_type: *const ComponentType,
    component: NonNull<ComponentImpl>,
) {
    let component = unsafe {
        sixtyfps_corelib::abi::datastructures::ComponentBox::from_raw(
            NonNull::new_unchecked(component_type as *mut _),
            component,
        )
    };

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
