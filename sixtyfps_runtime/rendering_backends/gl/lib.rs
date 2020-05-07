use cgmath::Matrix4;
use glow::{Context as GLContext, HasContext};
use kurbo::{BezPath, PathEl, Point, Rect};
use lyon::path::PathEvent;
use lyon::tessellation::geometry_builder::{BuffersBuilder, VertexBuffers};
use lyon::tessellation::{FillAttributes, FillOptions, FillTessellator};
use sixtyfps_corelib::graphics::{Color, FillStyle, Frame as GraphicsFrame, GraphicsBackend};
use std::marker;
use std::mem;

extern crate alloc;
use alloc::rc::Rc;

#[derive(Copy, Clone)]
struct PathVertex {
    pos: [f32; 2],
}

#[derive(Copy, Clone)]
struct ImageVertex {
    pos: [f32; 2],
    tex_pos: [f32; 2],
}

enum GLRenderingPrimitive {
    FillPath { geometry: GLGeometry<PathVertex, u16>, style: FillStyle },
    Texture {/*vertices: VertexBuffer<ImageVertex>, texture: Texture2d*/},
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

struct GLGeometry<VertexType, IndexType> {
    vertex_buffer_id: <GLContext as HasContext>::Buffer,
    index_buffer_id: <GLContext as HasContext>::Buffer,
    triangles: i32,
    _vertex_marker: marker::PhantomData<VertexType>,
    _index_marker: marker::PhantomData<IndexType>,
}

impl<VertexType, IndexType> GLGeometry<VertexType, IndexType> {
    fn new(gl: &glow::Context, data: VertexBuffers<VertexType, IndexType>) -> Self {
        let vertex_buffer_id = unsafe { gl.create_buffer().expect("vertex buffer") };

        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertex_buffer_id));

            let byte_len =
                mem::size_of_val(&data.vertices[0]) * data.vertices.len() / mem::size_of::<u8>();
            let byte_slice =
                std::slice::from_raw_parts(data.vertices.as_ptr() as *const u8, byte_len);
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, byte_slice, glow::STATIC_DRAW);
        }

        let index_buffer_id = unsafe { gl.create_buffer().expect("index buffer") };

        unsafe {
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(index_buffer_id));

            let byte_len =
                mem::size_of_val(&data.indices[0]) * data.indices.len() / mem::size_of::<u8>();
            let byte_slice =
                std::slice::from_raw_parts(data.indices.as_ptr() as *const u8, byte_len);
            gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, byte_slice, glow::STATIC_DRAW);
        }

        Self {
            vertex_buffer_id,
            index_buffer_id,
            triangles: data.indices.len() as i32,
            _vertex_marker: marker::PhantomData,
            _index_marker: marker::PhantomData,
        }
    }

    fn bind(&self, gl: &glow::Context, vertex_attribute_location: u32) {
        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vertex_buffer_id));
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.index_buffer_id));

            // TODO: generalize size/data_type
            gl.vertex_attrib_pointer_f32(vertex_attribute_location, 2, glow::FLOAT, false, 0, 0);
            gl.enable_vertex_attrib_array(vertex_attribute_location);
        }
    }

    // ### FIXME: call this function
    fn drop(&mut self, gl: &glow::Context) {
        unsafe {
            gl.delete_buffer(self.vertex_buffer_id);
            gl.delete_buffer(self.index_buffer_id);
        }
    }
}

pub struct GLRenderer {
    context: Rc<glow::Context>,
    path_program: Shader, // ### do not use RC<> given the ownership in GLRenderer
    image_program: Shader,
    fill_tesselator: FillTessellator,
}

pub struct GLFrame {
    context: Rc<glow::Context>,
    path_program: Shader,
    image_program: Shader,
    root_matrix: cgmath::Matrix4<f32>,
}

impl GLRenderer {
    pub fn new(context: glow::Context) -> GLRenderer {
        unsafe {
            let vertex_array = context.create_vertex_array().expect("Cannot create vertex array");
            context.bind_vertex_array(Some(vertex_array));
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
            path_program: path_program,
            image_program: image_program,
            fill_tesselator: FillTessellator::new(),
        }
    }
}

pub struct OpaqueRenderingPrimitive(GLRenderingPrimitive);

impl GraphicsBackend for GLRenderer {
    type RenderingPrimitive = OpaqueRenderingPrimitive;
    type Frame = GLFrame;

    fn create_path_fill_primitive(
        &mut self,
        path: &BezPath,
        style: FillStyle,
    ) -> Self::RenderingPrimitive {
        let mut geometry: VertexBuffers<PathVertex, u16> = VertexBuffers::new();

        let fill_opts = FillOptions::default();
        self.fill_tesselator
            .tessellate(
                PathConverter::new(path),
                &fill_opts,
                &mut BuffersBuilder::new(
                    &mut geometry,
                    |pos: lyon::math::Point, _: FillAttributes| PathVertex {
                        pos: [pos.x as f32, pos.y as f32],
                    },
                ),
            )
            .unwrap();

        let gl_geometry = GLGeometry::new(&self.context, geometry);

        OpaqueRenderingPrimitive(GLRenderingPrimitive::FillPath { geometry: gl_geometry, style })
    }

    fn create_image_primitive(
        &mut self,
        source_rect: impl Into<Rect>,
        dest_rect: impl Into<Rect>,
        image: image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
    ) -> Self::RenderingPrimitive {
        /*
        let dimensions = image.dimensions();
        let image = glium::texture::RawImage2d::from_raw_rgba(image.into_raw(), dimensions);
        let texture = glium::texture::Texture2d::new(&self.display, image).unwrap();

        let rect = dest_rect.into();
        let src_rect = source_rect.into();
        let image_width = dimensions.0 as f32;
        let image_height = dimensions.1 as f32;
        let src_left = (src_rect.x0 as f32) / image_width;
        let src_top = (src_rect.y0 as f32) / image_height;
        let src_right = (src_rect.x1 as f32) / image_width;
        let src_bottom = (src_rect.y1 as f32) / image_height;

        let vertex1 =
            ImageVertex { pos: [rect.x0 as f32, rect.y0 as f32], tex_pos: [src_left, src_top] };
        let vertex2 =
            ImageVertex { pos: [rect.x1 as f32, rect.y0 as f32], tex_pos: [src_right, src_top] };
        let vertex3 =
            ImageVertex { pos: [rect.x1 as f32, rect.y1 as f32], tex_pos: [src_right, src_bottom] };
        let vertex4 =
            ImageVertex { pos: [rect.x0 as f32, rect.y1 as f32], tex_pos: [src_left, src_bottom] };
        let shape = vec![vertex1, vertex2, vertex3, vertex1, vertex3, vertex4];

        let vertices = glium::VertexBuffer::new(&self.display, &shape).unwrap();

        */
        OpaqueRenderingPrimitive(GLRenderingPrimitive::Texture { /*texture, vertices*/ })
    }

    fn new_frame(&mut self, width: u32, height: u32, clear_color: &Color) -> GLFrame {
        // ### FIXME: make_current

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
        }
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
            GLRenderingPrimitive::FillPath { geometry, style } => {
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
                geometry.bind(&self.context, vertex_attribute_location);

                unsafe {
                    self.context.draw_elements(
                        glow::TRIANGLE_STRIP,
                        geometry.triangles,
                        glow::UNSIGNED_SHORT,
                        0,
                    );
                }
            }
            _ => {}
        }

        /*

            GLRenderingPrimitive::Texture { /*texture, vertices*/ } => {
                /*
                let indices = glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList);

                let uniforms = uniform! {
                    tex: texture,
                    matrix: matrix
                };

                self.glium_frame
                    .draw(vertices, &indices, &self.image_program, &uniforms, &draw_params)
                    .unwrap();
                    */
            }
        }
        */
    }

    fn submit(self) {}
}

struct PathConverter<'a> {
    first_point: Option<lyon::path::math::Point>,
    current_point: Option<lyon::path::math::Point>,
    shape_iter: Box<dyn Iterator<Item = kurbo::PathEl> + 'a>,
    deferred_begin: Option<PathEvent>,
    needs_closure: bool,
}

impl<'a> PathConverter<'a> {
    fn new(path: &'a BezPath) -> Self {
        PathConverter {
            first_point: None,
            current_point: None,
            shape_iter: Box::new(path.iter()),
            deferred_begin: None,
            needs_closure: false,
        }
    }
}

impl<'a> Iterator for PathConverter<'a> {
    type Item = PathEvent;
    fn next(&mut self) -> Option<Self::Item> {
        if self.deferred_begin.is_some() {
            return self.deferred_begin.take();
        }

        let path_el = self.shape_iter.next();
        match path_el {
            Some(PathEl::MoveTo(p)) => {
                let first = self.first_point;
                let last = self.current_point;

                self.current_point = Some(point_to_lyon_point(&p));
                let event = Some(PathEvent::Begin { at: self.current_point.unwrap() });

                if self.needs_closure {
                    self.first_point = self.current_point;
                    self.needs_closure = false;
                    self.deferred_begin = event;
                    Some(PathEvent::End { first: first.unwrap(), last: last.unwrap(), close: true })
                } else {
                    if self.first_point.is_none() {
                        self.first_point = self.current_point;
                    }
                    event
                }
            }
            Some(PathEl::LineTo(p)) => {
                self.needs_closure = true;
                let from = self.current_point.unwrap();
                let to = point_to_lyon_point(&p);
                self.current_point = Some(to);
                Some(PathEvent::Line { from, to })
            }
            Some(PathEl::QuadTo(ctrl, to)) => {
                self.needs_closure = true;

                let to = point_to_lyon_point(&to);
                let from = self.current_point.replace(to).unwrap();
                Some(PathEvent::Quadratic { from, ctrl: point_to_lyon_point(&ctrl), to })
            }
            Some(PathEl::CurveTo(ctrl1, ctrl2, to)) => {
                self.needs_closure = true;

                let to = point_to_lyon_point(&to);
                let from = self.current_point.replace(to).unwrap();
                Some(PathEvent::Cubic {
                    from,
                    ctrl1: point_to_lyon_point(&ctrl1),
                    ctrl2: point_to_lyon_point(&ctrl2),
                    to,
                })
            }
            Some(PathEl::ClosePath) => {
                self.needs_closure = false;
                let last = self.current_point.take().unwrap();
                let first = self.first_point.take().unwrap();
                Some(PathEvent::End { first, last, close: true })
            }
            None => {
                if self.needs_closure {
                    self.needs_closure = false;
                    let last = self.current_point.take().unwrap();
                    let first = self.first_point.take().unwrap();
                    Some(PathEvent::End { first, last, close: true })
                } else {
                    None
                }
            }
        }
    }
}

fn point_to_lyon_point(p: &Point) -> lyon::path::math::Point {
    lyon::path::math::Point::new(p.x as f32, p.y as f32)
}

impl Drop for GLRenderer {
    fn drop(&mut self) {
        self.path_program.drop(&self.context);
        self.image_program.drop(&self.context);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
