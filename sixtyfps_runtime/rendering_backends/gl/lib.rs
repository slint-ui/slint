use cgmath::Matrix4;
use glow::{Context as GLContext, HasContext};
use kurbo::{BezPath, PathEl, Point, Rect};
use lyon::path::PathEvent;
use lyon::tessellation::geometry_builder::{BuffersBuilder, VertexBuffers};
use lyon::tessellation::{FillAttributes, FillOptions, FillTessellator};
use sixtyfps_corelib::graphics::{Color, FillStyle, Frame as GraphicsFrame, GraphicsBackend};

extern crate alloc;
use alloc::rc::Rc;

#[derive(Copy, Clone)]
struct PathVertex {
    pos: [f32; 2],
}

//implement_vertex!(PathVertex, pos);

#[derive(Copy, Clone)]
struct ImageVertex {
    pos: [f32; 2],
    tex_pos: [f32; 2],
}

//implement_vertex!(ImageVertex, pos, tex_pos);

enum GLRenderingPrimitive {
    FillPath {
        /*vertices: VertexBuffer<PathVertex>, indices: IndexBuffer<u16>, */ style: FillStyle,
    },
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

    fn drop(&mut self, gl: &GLContext) {
        unsafe {
            gl.delete_program(self.program);
        }
    }
}

pub struct GLRenderer {
    windowed_context: Rc<glutin::WindowedContext<glutin::PossiblyCurrent>>,
    context: Rc<glow::Context>,
    path_program: Shader, // ### do not use RC<> given the ownership in GLRenderer
    image_program: Shader,
    fill_tesselator: FillTessellator,
}

pub struct GLFrame {
    windowed_context: Rc<glutin::WindowedContext<glutin::PossiblyCurrent>>,
    context: Rc<glow::Context>,
    path_program: Shader,
    image_program: Shader,
    root_matrix: cgmath::Matrix4<f32>,
}

impl GLRenderer {
    pub fn new(windowed_context: glutin::WindowedContext<glutin::NotCurrent>) -> GLRenderer {
        let windowed_context = unsafe { windowed_context.make_current().unwrap() };

        let context = glow::Context::from_loader_function(|s| {
            windowed_context.get_proc_address(s) as *const _
        });

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
            windowed_context: Rc::new(windowed_context),
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

        /*
                let vertices = VertexBuffer::new(&self.display, &geometry.vertices).unwrap();
                let indices = IndexBuffer::new(
                    &self.display,
                    glium::index::PrimitiveType::TrianglesList,
                    &geometry.indices,
                )
                .unwrap();
        */

        OpaqueRenderingPrimitive(GLRenderingPrimitive::FillPath { /*vertices, indices, */ style, })
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

    fn new_frame(&mut self, clear_color: &Color) -> GLFrame {
        // ### FIXME: make_current

        let size = self.windowed_context.window().inner_size();

        let (r, g, b, a) = clear_color.as_rgba_f32();
        unsafe {
            self.context.clear_color(r, g, b, a);
            self.context.clear(glow::COLOR_BUFFER_BIT);
        };
        /*
        let (w, h) = self.display.get_framebuffer_dimensions();
        let mut glium_frame = self.display.draw();
        glium_frame.clear(None, Some(clear_color.as_rgba_f32()), false, None, None);
        */
        GLFrame {
            windowed_context: self.windowed_context.clone(),
            context: self.context.clone(),
            path_program: self.path_program.clone(),
            image_program: self.image_program.clone(),
            root_matrix: cgmath::ortho(0.0, size.width as f32, size.height as f32, 0.0, -1., 1.0),
        }
    }
}

impl GraphicsFrame for GLFrame {
    type RenderingPrimitive = OpaqueRenderingPrimitive;

    fn render_primitive(&mut self, primitive: &OpaqueRenderingPrimitive, transform: &Matrix4<f32>) {
        let matrix: [[f32; 4]; 4] = (self.root_matrix * transform).into();

        //let draw_params =
        //    glium::DrawParameters { blend: glium::Blend::alpha_blending(), ..Default::default() };

        match &primitive.0 {
            GLRenderingPrimitive::FillPath { /*ref vertices, ref indices, */style } => {
                /*
                let (r, g, b, a) = match style {
                    FillStyle::SolidColor(color) => color.as_rgba_f32(),
                };
                let uniforms = uniform! {
                    vertcolor: (r, g, b, a),
                    matrix: matrix
                };

                self.glium_frame
                    .draw(vertices, indices, &self.path_program, &uniforms, &draw_params)
                    .unwrap();
                    */
            }
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
    }

    fn submit(self) {
        self.windowed_context.swap_buffers().unwrap();
    }
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
