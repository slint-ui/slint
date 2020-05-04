use glium::{
    implement_vertex, uniform, Display, Frame as GLiumFrame, IndexBuffer, Program, Surface,
    VertexBuffer,
};
use kurbo::{Affine, BezPath, PathEl, Point};
use lyon::path::PathEvent;
use lyon::tessellation::geometry_builder::{BuffersBuilder, VertexBuffers};
use lyon::tessellation::{FillAttributes, FillOptions, FillTessellator};

use sixtyfps_corelib::graphics::{Frame as GraphicsFrame, GraphicsBackend};

extern crate alloc;
use alloc::rc::Rc;

#[derive(Copy, Clone)]
pub struct PathVertex {
    pos: [f32; 2],
}

implement_vertex!(PathVertex, pos);

pub enum RenderingPrimitive {
    Path { geometry: VertexBuffers<PathVertex, u16> },
}

pub struct GLRenderer {
    display: Display,
    path_program: Rc<Program>,
    fill_tesselator: FillTessellator,
}

struct GLFrame {
    display: Display,
    glium_frame: GLiumFrame,
    path_program: Rc<Program>,
    root_transform: Affine,
}

impl GLRenderer {
    pub fn new(display: &Display) -> GLRenderer {
        const PATH_VERTEX_SHADER: &str = r#"#version 100
        attribute vec2 pos;
        uniform vec4 vertcolor;
        uniform mat4 matrix;
        varying lowp vec4 fragcolor;
        void main() {
            gl_Position = vec4(pos, 0.0, 1) * matrix;
            fragcolor = vertcolor;
        }"#;

        const PATH_FRAGMENT_SHADER: &str = r#"#version 100
        varying lowp vec4 fragcolor;
        void main() {
            gl_FragColor = fragcolor;
        }"#;

        GLRenderer {
            display: display.clone(),
            path_program: Rc::new(
                glium::Program::from_source(
                    display,
                    PATH_VERTEX_SHADER,
                    PATH_FRAGMENT_SHADER,
                    None,
                )
                .unwrap(),
            ),
            fill_tesselator: FillTessellator::new(),
        }
    }
}

impl GraphicsBackend for GLRenderer {
    type RenderingPrimitive = RenderingPrimitive;

    fn create_path_primitive(&mut self, path: &BezPath) -> Self::RenderingPrimitive {
        let mut geometry = VertexBuffers::new();

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

        RenderingPrimitive::Path { geometry: geometry }
    }

    fn new_frame(&self) -> Box<dyn GraphicsFrame<RenderingPrimitive = RenderingPrimitive>> {
        let (w, h) = self.display.get_framebuffer_dimensions();
        let root_transform =
            Affine::FLIP_Y * Affine::scale_non_uniform(1.0 / (w as f64), 1.0 / (h as f64));
        Box::new(GLFrame {
            display: self.display.clone(),
            glium_frame: self.display.draw(),
            path_program: self.path_program.clone(),
            root_transform,
        })
    }
}

impl GraphicsFrame for GLFrame {
    type RenderingPrimitive = RenderingPrimitive;

    fn render_primitive(&mut self, primitive: &RenderingPrimitive, transform: &Affine) {
        let transform = self.root_transform * *transform;

        match primitive {
            RenderingPrimitive::Path { geometry } => {
                let vertices = &geometry.vertices;
                let vertex_buffer = VertexBuffer::new(&self.display, &vertices).unwrap();
                let indices = &geometry.indices;
                let index_buffer = IndexBuffer::new(
                    &self.display,
                    glium::index::PrimitiveType::TrianglesList,
                    &indices,
                )
                .unwrap();

                let (r, g, b, a) = (0.0f32, 1.0f32, 0.0f32, 0.0f32);
                let coefs = transform.as_coeffs();
                let uniforms = uniform! {
                    vertcolor: (r, g, b, a),
                    matrix: [
                        [coefs[0] as f32, coefs[1] as f32, 0.0, coefs[4] as f32],
                        [coefs[2] as f32, coefs[3] as f32, 0.0, coefs[5] as f32],
                        [0.0, 0.0, 0.0, 0.0],
                        [0.0, 0.0, 0.0, 1.0],
                    ]
                };

                self.glium_frame
                    .draw(
                        &vertex_buffer,
                        &index_buffer,
                        &self.path_program,
                        &uniforms,
                        &Default::default(),
                    )
                    .unwrap();
            }
        }
    }

    fn submit(self: Box<Self>) {
        self.glium_frame.finish().unwrap();
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

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
