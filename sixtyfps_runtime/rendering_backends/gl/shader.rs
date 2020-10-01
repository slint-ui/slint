/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use super::{
    buffers::{GLArrayBuffer, GLIndexBuffer},
    texture::GLTexture,
    GLContext, Vertex,
};
use glow::HasContext;
use sixtyfps_corelib::graphics::ARGBColor;
use std::rc::Rc;

fn premultiply_alpha(col: ARGBColor<f32>) -> ARGBColor<f32> {
    ARGBColor {
        alpha: col.alpha,
        red: col.red * col.alpha,
        green: col.green * col.alpha,
        blue: col.blue * col.alpha,
    }
}

struct Shader {
    program: <GLContext as HasContext>::Program,
    context: Rc<glow::Context>,
}

impl Shader {
    fn new(gl: &Rc<GLContext>, vertex_shader_source: &str, fragment_shader_source: &str) -> Shader {
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

        Shader { context: gl.clone(), program }
    }

    pub fn use_program(&self, gl: &glow::Context) {
        unsafe {
            gl.use_program(Some(self.program));
        }
    }
}

impl Drop for Shader {
    fn drop(&mut self) {
        unsafe {
            self.context.delete_program(self.program);
        }
    }
}

#[derive(Clone)]
pub(crate) struct PathShader {
    inner: Rc<Shader>,
    matrix_location: <GLContext as HasContext>::UniformLocation,
    vertcolor_location: <GLContext as HasContext>::UniformLocation,
    pos_location: u32,
}

impl PathShader {
    pub fn new(gl: &Rc<glow::Context>) -> Self {
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

        let inner = Rc::new(Shader::new(&gl, PATH_VERTEX_SHADER, PATH_FRAGMENT_SHADER));

        let matrix_location = unsafe { gl.get_uniform_location(inner.program, "matrix").unwrap() };
        let vertcolor_location =
            unsafe { gl.get_uniform_location(inner.program, "vertcolor").unwrap() };

        let pos_location = unsafe { gl.get_attrib_location(inner.program, "pos").unwrap() };

        Self { inner, matrix_location, vertcolor_location, pos_location }
    }

    pub fn bind(
        &self,
        gl: &glow::Context,
        matrix: &[f32; 16],
        vertcolor: ARGBColor<f32>,
        pos: &GLArrayBuffer<Vertex>,
        indices: &GLIndexBuffer<u16>,
    ) {
        self.inner.use_program(&gl);

        let vertcolor = premultiply_alpha(vertcolor);

        unsafe {
            gl.uniform_matrix_4_f32_slice(Some(&self.matrix_location), false, matrix);

            gl.uniform_4_f32(
                Some(&self.vertcolor_location),
                vertcolor.red,
                vertcolor.green,
                vertcolor.blue,
                vertcolor.alpha,
            )
        };

        pos.bind(&gl, self.pos_location);

        indices.bind(&gl);
    }

    pub fn unbind(&self, gl: &glow::Context) {
        unsafe {
            gl.disable_vertex_attrib_array(self.pos_location);
        }
    }
}

#[derive(Clone)]
pub(crate) struct RectShader {
    inner: Rc<Shader>,
    matrix_location: <GLContext as HasContext>::UniformLocation,
    vertcolor_location: <GLContext as HasContext>::UniformLocation,
    pos_location: u32,
    rect_size_location: <GLContext as HasContext>::UniformLocation,
    radius_location: <GLContext as HasContext>::UniformLocation,
    border_width_location: <GLContext as HasContext>::UniformLocation,
    border_color_location: <GLContext as HasContext>::UniformLocation,
}

impl RectShader {
    pub fn new(gl: &Rc<glow::Context>) -> Self {
        const RECT_VERTEX_SHADER: &str = r#"#version 100
        attribute vec2 pos;
        uniform vec4 vertcolor;
        uniform mat4 matrix;
        varying lowp vec4 fragcolor;
        varying lowp vec2 fragpos;

        void main() {
            gl_Position = matrix * vec4(pos, 0.0, 1);
            fragcolor = vertcolor;
            fragpos = pos;
        }"#;

        const RECT_FRAGMENT_SHADER: &str = r#"#version 100
        precision mediump float;
        uniform vec2 rectsize;
        uniform float radius;
        uniform float border_width;
        uniform lowp vec4 border_color;
        varying lowp vec4 fragcolor;
        varying lowp vec2 fragpos;

        float roundRectDistance(vec2 pos, vec2 rect_size, float radius)
        {
            vec2 q = abs(pos) - rect_size + radius;
            return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - radius;
        }

        float fillAlpha(float dist)
        {
            return clamp(-dist, 0.0, 1.0);
        }

        float innerBorderAlpha(float dist, float border_width)
        {
            float alpha1 = clamp(dist + border_width, 0.0, 1.0);
            float alpha2 = clamp(dist, 0.0, 1.0);
            return alpha1 - alpha2;
        }

        void main() {
            float dist = roundRectDistance(fragpos - rectsize, rectsize, radius);
            vec4 col = mix(vec4(0., 0., 0., 0.), fragcolor, fillAlpha(dist));
            col = mix(col, border_color, innerBorderAlpha(dist, border_width));
            gl_FragColor = col;
        }"#;

        let inner = Rc::new(Shader::new(&gl, RECT_VERTEX_SHADER, RECT_FRAGMENT_SHADER));

        let matrix_location = unsafe { gl.get_uniform_location(inner.program, "matrix").unwrap() };
        let vertcolor_location =
            unsafe { gl.get_uniform_location(inner.program, "vertcolor").unwrap() };
        let rect_size_location =
            unsafe { gl.get_uniform_location(inner.program, "rectsize").unwrap() };
        let radius_location = unsafe { gl.get_uniform_location(inner.program, "radius").unwrap() };
        let border_width_location =
            unsafe { gl.get_uniform_location(inner.program, "border_width").unwrap() };
        let border_color_location =
            unsafe { gl.get_uniform_location(inner.program, "border_color").unwrap() };

        let pos_location = unsafe { gl.get_attrib_location(inner.program, "pos").unwrap() };

        Self {
            inner,
            matrix_location,
            vertcolor_location,
            pos_location,
            rect_size_location,
            radius_location,
            border_width_location,
            border_color_location,
        }
    }

    pub fn bind(
        &self,
        gl: &glow::Context,
        matrix: &[f32; 16],
        vertcolor: ARGBColor<f32>,
        rect_size: &[f32; 2],
        radius: f32,
        border_width: f32,
        border_color: ARGBColor<f32>,
        pos: &GLArrayBuffer<Vertex>,
        indices: &GLIndexBuffer<u16>,
    ) {
        self.inner.use_program(&gl);

        let vertcolor = premultiply_alpha(vertcolor);
        let border_color = premultiply_alpha(border_color);

        unsafe {
            gl.uniform_matrix_4_f32_slice(Some(&self.matrix_location), false, matrix);

            gl.uniform_4_f32(
                Some(&self.vertcolor_location),
                vertcolor.red,
                vertcolor.green,
                vertcolor.blue,
                vertcolor.alpha,
            );

            gl.uniform_2_f32(Some(&self.rect_size_location), rect_size[0], rect_size[1]);

            gl.uniform_1_f32(Some(&self.radius_location), radius);

            gl.uniform_1_f32(Some(&self.border_width_location), border_width);
            gl.uniform_4_f32(
                Some(&self.border_color_location),
                border_color.red,
                border_color.green,
                border_color.blue,
                border_color.alpha,
            );
        };

        pos.bind(&gl, self.pos_location);

        indices.bind(&gl);
    }

    pub fn unbind(&self, gl: &glow::Context) {
        unsafe {
            gl.disable_vertex_attrib_array(self.pos_location);
        }
    }
}

#[derive(Clone)]
pub(crate) struct ImageShader {
    inner: Rc<Shader>,
    matrix_location: <GLContext as HasContext>::UniformLocation,
    tex_location: <GLContext as HasContext>::UniformLocation,
    pos_location: u32,
    tex_pos_location: u32,
}

impl ImageShader {
    pub fn new(gl: &Rc<glow::Context>) -> Self {
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

        let inner = Rc::new(Shader::new(&gl, IMAGE_VERTEX_SHADER, IMAGE_FRAGMENT_SHADER));

        let matrix_location = unsafe { gl.get_uniform_location(inner.program, "matrix").unwrap() };
        let tex_location = unsafe { gl.get_uniform_location(inner.program, "tex").unwrap() };

        let pos_location = unsafe { gl.get_attrib_location(inner.program, "pos").unwrap() };
        let tex_pos_location = unsafe { gl.get_attrib_location(inner.program, "tex_pos").unwrap() };

        Self { inner, matrix_location, tex_location, pos_location, tex_pos_location }
    }

    pub fn bind(
        &self,
        gl: &glow::Context,
        matrix: &[f32; 16],
        tex: &GLTexture,
        pos: &GLArrayBuffer<Vertex>,
        tex_pos: &GLArrayBuffer<Vertex>,
    ) {
        self.inner.use_program(&gl);

        unsafe { gl.uniform_matrix_4_f32_slice(Some(&self.matrix_location), false, matrix) };

        tex.bind_to_location(&self.tex_location);

        pos.bind(&gl, self.pos_location);

        tex_pos.bind(&gl, self.tex_pos_location);
    }

    pub fn unbind(&self, gl: &glow::Context) {
        unsafe {
            gl.disable_vertex_attrib_array(self.pos_location);
            gl.disable_vertex_attrib_array(self.tex_pos_location);
        }
    }
}

#[derive(Clone)]
pub(crate) struct GlyphShader {
    inner: Rc<Shader>,
    matrix_location: <GLContext as HasContext>::UniformLocation,
    text_color_location: <GLContext as HasContext>::UniformLocation,
    tex_location: <GLContext as HasContext>::UniformLocation,
    pos_location: u32,
    tex_pos_location: u32,
}

impl GlyphShader {
    pub fn new(gl: &Rc<glow::Context>) -> Self {
        const GLYPH_VERTEX_SHADER: &str = r#"#version 100
        attribute vec2 pos;
        attribute vec2 tex_pos;
        uniform mat4 matrix;
        uniform vec4 text_color;
        varying highp vec2 frag_tex_pos;
        varying lowp vec4 fragcolor;
        void main() {
            gl_Position = matrix * vec4(pos, 0.0, 1);
            frag_tex_pos = tex_pos;
            fragcolor = text_color;
        }"#;

        const GLYPH_FRAGMENT_SHADER: &str = r#"#version 100
        varying highp vec2 frag_tex_pos;
        varying lowp vec4 fragcolor;
        uniform sampler2D tex;
        void main() {
            gl_FragColor = fragcolor * texture2D(tex, frag_tex_pos).a;
        }"#;

        let inner = Rc::new(Shader::new(&gl, GLYPH_VERTEX_SHADER, GLYPH_FRAGMENT_SHADER));

        let matrix_location = unsafe { gl.get_uniform_location(inner.program, "matrix").unwrap() };
        let text_color_location =
            unsafe { gl.get_uniform_location(inner.program, "text_color").unwrap() };
        let tex_location = unsafe { gl.get_uniform_location(inner.program, "tex").unwrap() };

        let pos_location = unsafe { gl.get_attrib_location(inner.program, "pos").unwrap() };

        let tex_pos_location = unsafe { gl.get_attrib_location(inner.program, "tex_pos").unwrap() };

        Self {
            inner,
            matrix_location,
            text_color_location,
            tex_location,
            pos_location,
            tex_pos_location,
        }
    }

    pub fn bind(
        &self,
        gl: &glow::Context,
        matrix: &[f32; 16],
        text_color: ARGBColor<f32>,
        tex: &GLTexture,
        pos: &GLArrayBuffer<Vertex>,
        tex_pos: &GLArrayBuffer<Vertex>,
    ) {
        self.inner.use_program(&gl);

        let text_color = premultiply_alpha(text_color);

        unsafe {
            gl.uniform_matrix_4_f32_slice(Some(&self.matrix_location), false, matrix);

            gl.uniform_4_f32(
                Some(&self.text_color_location),
                text_color.red,
                text_color.green,
                text_color.blue,
                text_color.alpha,
            )
        };

        tex.bind_to_location(&self.tex_location);

        pos.bind(&gl, self.pos_location);

        tex_pos.bind(&gl, self.tex_pos_location);
    }

    pub fn unbind(&self, gl: &glow::Context) {
        unsafe {
            gl.disable_vertex_attrib_array(self.pos_location);
            gl.disable_vertex_attrib_array(self.tex_pos_location);
        }
    }
}
