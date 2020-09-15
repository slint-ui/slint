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
use std::rc::Rc;

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
        vertcolor: &[f32; 4],
        pos: &GLArrayBuffer<Vertex>,
        indices: &GLIndexBuffer<u16>,
    ) {
        self.inner.use_program(&gl);

        unsafe {
            gl.uniform_matrix_4_f32_slice(Some(&self.matrix_location), false, matrix);

            gl.uniform_4_f32(
                Some(&self.vertcolor_location),
                vertcolor[0],
                vertcolor[1],
                vertcolor[2],
                vertcolor[3],
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
        text_color: &[f32; 4],
        tex: &GLTexture,
        pos: &GLArrayBuffer<Vertex>,
        tex_pos: &GLArrayBuffer<Vertex>,
    ) {
        self.inner.use_program(&gl);

        unsafe {
            gl.uniform_matrix_4_f32_slice(Some(&self.matrix_location), false, matrix);

            gl.uniform_4_f32(
                Some(&self.text_color_location),
                text_color[0],
                text_color[1],
                text_color[2],
                text_color[3],
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
