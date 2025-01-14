// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::num::NonZeroU32;
use std::rc::Rc;

slint::include_modules!();

use glow::HasContext;

macro_rules! define_scoped_binding {
    (struct $binding_ty_name:ident => $obj_name:path, $param_name:path, $binding_fn:ident, $target_name:path) => {
        struct $binding_ty_name {
            saved_value: Option<$obj_name>,
            gl: Rc<glow::Context>,
        }

        impl $binding_ty_name {
            unsafe fn new(gl: &Rc<glow::Context>, new_binding: Option<$obj_name>) -> Self {
                let saved_value =
                    NonZeroU32::new(gl.get_parameter_i32($param_name) as u32).map($obj_name);

                gl.$binding_fn($target_name, new_binding);
                Self { saved_value, gl: gl.clone() }
            }
        }

        impl Drop for $binding_ty_name {
            fn drop(&mut self) {
                unsafe {
                    self.gl.$binding_fn($target_name, self.saved_value);
                }
            }
        }
    };
    (struct $binding_ty_name:ident => $obj_name:path, $param_name:path, $binding_fn:ident) => {
        struct $binding_ty_name {
            saved_value: Option<$obj_name>,
            gl: Rc<glow::Context>,
        }

        impl $binding_ty_name {
            unsafe fn new(gl: &Rc<glow::Context>, new_binding: Option<$obj_name>) -> Self {
                let saved_value =
                    NonZeroU32::new(gl.get_parameter_i32($param_name) as u32).map($obj_name);

                gl.$binding_fn(new_binding);
                Self { saved_value, gl: gl.clone() }
            }
        }

        impl Drop for $binding_ty_name {
            fn drop(&mut self) {
                unsafe {
                    self.gl.$binding_fn(self.saved_value);
                }
            }
        }
    };
}

define_scoped_binding!(struct ScopedTextureBinding => glow::NativeTexture, glow::TEXTURE_BINDING_2D, bind_texture, glow::TEXTURE_2D);
define_scoped_binding!(struct ScopedFrameBufferBinding => glow::NativeFramebuffer, glow::DRAW_FRAMEBUFFER_BINDING, bind_framebuffer, glow::DRAW_FRAMEBUFFER);
define_scoped_binding!(struct ScopedVBOBinding => glow::NativeBuffer, glow::ARRAY_BUFFER_BINDING, bind_buffer, glow::ARRAY_BUFFER);
define_scoped_binding!(struct ScopedVAOBinding => glow::NativeVertexArray, glow::VERTEX_ARRAY_BINDING, bind_vertex_array);

struct DemoTexture {
    texture: glow::Texture,
    width: u32,
    height: u32,
    fbo: glow::Framebuffer,
    gl: Rc<glow::Context>,
}

impl DemoTexture {
    unsafe fn new(gl: &Rc<glow::Context>, width: u32, height: u32) -> Self {
        let fbo = gl.create_framebuffer().expect("Unable to create framebuffer");

        let texture = gl.create_texture().expect("Unable to allocate texture");

        let _saved_texture_binding = ScopedTextureBinding::new(gl, Some(texture));

        let old_unpack_alignment = gl.get_parameter_i32(glow::UNPACK_ALIGNMENT);
        let old_unpack_row_length = gl.get_parameter_i32(glow::UNPACK_ROW_LENGTH);
        let old_unpack_skip_pixels = gl.get_parameter_i32(glow::UNPACK_SKIP_PIXELS);
        let old_unpack_skip_rows = gl.get_parameter_i32(glow::UNPACK_SKIP_ROWS);

        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
        gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, width as i32);
        gl.pixel_store_i32(glow::UNPACK_SKIP_PIXELS, 0);
        gl.pixel_store_i32(glow::UNPACK_SKIP_ROWS, 0);

        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA as _,
            width as _,
            height as _,
            0,
            glow::RGBA as _,
            glow::UNSIGNED_BYTE as _,
            glow::PixelUnpackData::Slice(None),
        );

        let _saved_fbo_binding = ScopedFrameBufferBinding::new(gl, Some(fbo));

        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(texture),
            0,
        );

        debug_assert_eq!(
            gl.check_framebuffer_status(glow::FRAMEBUFFER),
            glow::FRAMEBUFFER_COMPLETE
        );

        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, old_unpack_alignment);
        gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, old_unpack_row_length);
        gl.pixel_store_i32(glow::UNPACK_SKIP_PIXELS, old_unpack_skip_pixels);
        gl.pixel_store_i32(glow::UNPACK_SKIP_ROWS, old_unpack_skip_rows);

        Self { texture, width, height, fbo, gl: gl.clone() }
    }

    unsafe fn with_texture_as_active_fbo<R>(&self, callback: impl FnOnce() -> R) -> R {
        let _saved_fbo = ScopedFrameBufferBinding::new(&self.gl, Some(self.fbo));
        callback()
    }
}

impl Drop for DemoTexture {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_framebuffer(self.fbo);
            self.gl.delete_texture(self.texture);
        }
    }
}

struct DemoRenderer {
    gl: Rc<glow::Context>,
    program: glow::Program,
    vbo: glow::Buffer,
    vao: glow::VertexArray,
    effect_time_location: glow::UniformLocation,
    selected_light_color_position: glow::UniformLocation,
    start_time: web_time::Instant,
    displayed_texture: DemoTexture,
    next_texture: DemoTexture,
}

impl DemoRenderer {
    fn new(gl: glow::Context) -> Self {
        let gl = Rc::new(gl);
        unsafe {
            let program = gl.create_program().expect("Cannot create program");

            let (vertex_shader_source, fragment_shader_source) = (
                r#"#version 100
            attribute vec2 position;
            varying vec2 frag_position;
            void main() {
                frag_position = position;
                gl_Position = vec4(position, 0.0, 1.0);
            }"#,
                r#"#version 100
            precision highp float;
            varying vec2 frag_position;
            uniform vec3 selected_light_color;
            uniform float iTime;

            float sdRoundBox(vec3 p, vec3 b, float r)
            {
                vec3 q = abs(p) - b;
                return length(max(q,0.0)) + min(max(q.x,max(q.y,q.z)),0.0) - r;
            }

            vec3 rotateY(vec3 r, float angle)
            {
                mat3 rotation_matrix = mat3(cos(angle), 0, sin(angle), 0, 1, 0, -sin(angle), 0, cos(angle));
                return rotation_matrix * r;
            }

            vec3 rotateZ(vec3 r, float angle) {
                mat3 rotation_matrix = mat3(cos(angle), -sin(angle), 0, sin(angle), cos(angle), 0, 0, 0, 1);
                return rotation_matrix * r;
            }

            // Distance from the scene
            float scene(vec3 r)
            {
                vec3 pos = rotateZ(rotateY(r + vec3(-1.0, -1.0, 4.0), iTime), iTime);
                vec3 cube = vec3(0.5, 0.5, 0.5);
                float edge = 0.1;
                return sdRoundBox(pos, cube, edge);
            }

            // https://iquilezles.org/articles/normalsSDF
            vec3 normal( in vec3 pos )
            {
                vec2 e = vec2(1.0,-1.0)*0.5773;
                const float eps = 0.0005;
                return normalize( e.xyy*scene( pos + e.xyy*eps ) +
                                e.yyx*scene( pos + e.yyx*eps ) +
                                e.yxy*scene( pos + e.yxy*eps ) +
                                e.xxx*scene( pos + e.xxx*eps ) );
            }

            #define ITERATIONS 90
            #define EPS 0.0001

            vec4 render(vec2 fragCoord, vec3 light_color)
            {
                vec4 color = vec4(0, 0, 0, 1);

                vec3 camera = vec3(1.0, 2.0, 1.0);
                vec3 p = vec3(fragCoord.x, fragCoord.y + 1.0, -1.0);
                vec3 dir = normalize(p - camera);

                for(int i=0; i < ITERATIONS; i++)
                {
                    float dist = scene(p);
                    if(dist < EPS) {
                        break;
                    }
                    p = p + dir * dist;
                }

                vec3 surf_normal = normal(p);

                vec3 light_position = vec3(2.0, 4.0, -0.5);
                float light = 7.0 + 2.0 * dot(surf_normal, light_position);
                light /= 0.2 * pow(length(light_position - p), 3.5);

                return vec4(light * light_color.x, light * light_color.y, light * light_color.z, 1.0) * 2.0;
            }

            /*
            void mainImage(out vec4 fragColor, in vec2 fragCoord)
            {
                vec2 r = fragCoord.xy / iResolution.xy;
                r.x *= (iResolution.x / iResolution.y);
                fragColor = render(r, vec3(0.2, 0.5, 0.9));
            }
            */

            void main() {
                vec2 r = vec2(0.5 * frag_position.x + 1.0, 0.5 - 0.5 * frag_position.y);
                gl_FragColor = render(r, selected_light_color);
            }"#,
            );

            let shader_sources = [
                (glow::VERTEX_SHADER, vertex_shader_source),
                (glow::FRAGMENT_SHADER, fragment_shader_source),
            ];

            let mut shaders = Vec::with_capacity(shader_sources.len());

            for (shader_type, shader_source) in shader_sources.iter() {
                let shader = gl.create_shader(*shader_type).expect("Cannot create shader");
                gl.shader_source(shader, shader_source);
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    panic!("{}", gl.get_shader_info_log(shader));
                }
                gl.attach_shader(program, shader);
                shaders.push(shader);
            }

            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                panic!("{}", gl.get_program_info_log(program));
            }

            for shader in shaders {
                gl.detach_shader(program, shader);
                gl.delete_shader(shader);
            }

            let effect_time_location = gl.get_uniform_location(program, "iTime").unwrap();
            let selected_light_color_position =
                gl.get_uniform_location(program, "selected_light_color").unwrap();
            let position_location = gl.get_attrib_location(program, "position").unwrap();

            let vbo = gl.create_buffer().expect("Cannot create buffer");
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));

            let vertices = [-1.0f32, 1.0f32, -1.0f32, -1.0f32, 1.0f32, 1.0f32, 1.0f32, -1.0f32];

            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, vertices.align_to().1, glow::STATIC_DRAW);

            let vao = gl.create_vertex_array().expect("Cannot create vertex array");
            gl.bind_vertex_array(Some(vao));
            gl.enable_vertex_attrib_array(position_location);
            gl.vertex_attrib_pointer_f32(position_location, 2, glow::FLOAT, false, 8, 0);

            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            gl.bind_vertex_array(None);

            let displayed_texture = DemoTexture::new(&gl, 320, 200);
            let next_texture = DemoTexture::new(&gl, 320, 200);

            Self {
                gl,
                program,
                effect_time_location,
                selected_light_color_position,
                vbo,
                vao,
                start_time: web_time::Instant::now(),
                displayed_texture,
                next_texture,
            }
        }
    }
}

impl Drop for DemoRenderer {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_program(self.program);
            self.gl.delete_vertex_array(self.vao);
            self.gl.delete_buffer(self.vbo);
        }
    }
}

impl DemoRenderer {
    fn render(
        &mut self,
        light_red: f32,
        light_green: f32,
        light_blue: f32,
        width: u32,
        height: u32,
    ) -> slint::Image {
        unsafe {
            let gl = &self.gl;

            gl.use_program(Some(self.program));

            let _saved_vbo = ScopedVBOBinding::new(gl, Some(self.vbo));
            let _saved_vao = ScopedVAOBinding::new(gl, Some(self.vao));

            if self.next_texture.width != width || self.next_texture.height != height {
                let mut new_texture = DemoTexture::new(gl, width, height);
                std::mem::swap(&mut self.next_texture, &mut new_texture);
            }

            self.next_texture.with_texture_as_active_fbo(|| {
                let mut saved_viewport: [i32; 4] = [0, 0, 0, 0];
                gl.get_parameter_i32_slice(glow::VIEWPORT, &mut saved_viewport);

                gl.viewport(0, 0, self.next_texture.width as _, self.next_texture.height as _);
                let elapsed = self.start_time.elapsed().as_millis() as f32 / 500.;
                gl.uniform_1_f32(Some(&self.effect_time_location), elapsed);

                gl.uniform_3_f32(
                    Some(&self.selected_light_color_position),
                    light_red,
                    light_green,
                    light_blue,
                );

                gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);

                gl.viewport(
                    saved_viewport[0],
                    saved_viewport[1],
                    saved_viewport[2],
                    saved_viewport[3],
                );
            });

            gl.use_program(None);
        }

        let result_texture = unsafe {
            slint::BorrowedOpenGLTextureBuilder::new_gl_2d_rgba_texture(
                self.next_texture.texture.0,
                (self.next_texture.width, self.next_texture.height).into(),
            )
            .build()
        };

        std::mem::swap(&mut self.next_texture, &mut self.displayed_texture);

        result_texture
    }
}

fn main() {
    slint::BackendSelector::new()
        .require_opengl_es()
        .select()
        .expect("Unable to create Slint backend with OpenGL ES renderer");

    let app = App::new().unwrap();

    let mut underlay = None;

    let app_weak = app.as_weak();

    app.window()
        .set_rendering_notifier(move |state, graphics_api| {
            // eprintln!("rendering state {:#?}", state);

            match state {
                slint::RenderingState::RenderingSetup => {
                    let context = match graphics_api {
                        slint::GraphicsAPI::NativeOpenGL { get_proc_address } => unsafe {
                            glow::Context::from_loader_function_cstr(|s| get_proc_address(s))
                        },
                        _ => return,
                    };
                    underlay = Some(DemoRenderer::new(context))
                }
                slint::RenderingState::BeforeRendering => {
                    if let (Some(underlay), Some(app)) = (underlay.as_mut(), app_weak.upgrade()) {
                        let texture = underlay.render(
                            app.get_selected_red(),
                            app.get_selected_green(),
                            app.get_selected_blue(),
                            app.get_requested_texture_width() as u32,
                            app.get_requested_texture_height() as u32,
                        );
                        app.set_texture(slint::Image::from(texture));
                        app.window().request_redraw();
                    }
                }
                slint::RenderingState::AfterRendering => {}
                slint::RenderingState::RenderingTeardown => {
                    drop(underlay.take());
                }
                _ => {}
            }
        })
        .expect("Unable to set rendering notifier");

    app.run().unwrap();
}
