// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

slint::include_modules!();

use glow::HasContext;

struct EGLUnderlay {
    gl: glow::Context,
    program: glow::Program,
    effect_time_location: glow::UniformLocation,
    rotation_time_location: glow::UniformLocation,
    vbo: glow::Buffer,
    vao: glow::VertexArray,
    start_time: web_time::Instant,
    rotation_time: f32,
    last_rotation_enabled: bool,
    rotation_pause_offset: f32,
}

impl EGLUnderlay {
    fn new(gl: glow::Context) -> Self {
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
            #ifdef GL_FRAGMENT_PRECISION_HIGH
                precision highp float;
            #else
                precision mediump float;
            #endif
            varying vec2 frag_position;
            uniform float effect_time;
            uniform float rotation_time;
            const vec3 COLOR_BG_DARK   = vec3(0.106, 0.106, 0.118);
            const vec3 COLOR_DIAMOND   = vec3(0.137, 0.149, 0.184);
            const vec3 COLOR_ACCENT    = vec3(0.12, 0.35, 0.75);
            mat2 rotate(float angle) {
                float s = sin(angle);
                float c = cos(angle);
                return mat2(c, -s, s, c);
            }
            void main() {
                vec2 p_coords = frag_position;
                float perspective_strength = 0.09;
                float divisor = 1.0 + (-p_coords.y + 1.0) * perspective_strength;
                p_coords /= divisor;
                p_coords.y *= (1.0 + perspective_strength * 1.5);
                const float MAX_ANGLE_DEGREES = 10.0;
                float max_angle_rad = radians(MAX_ANGLE_DEGREES);
                float oscillating_factor = sin(rotation_time / 1700.0);
                float angle = oscillating_factor * max_angle_rad;
                mat2 rotation_matrix = rotate(angle);
                vec2 uv = rotation_matrix * p_coords * 6.0;
                vec2 grid_id = floor(uv);
                vec2 grid_uv = fract(uv) - 0.5;
                float manhattan_dist = abs(grid_uv.x) + abs(grid_uv.y);
                float wave_time = effect_time / 300.0;
                float wave_offset = grid_id.x * 0.5 + grid_id.y * 0.15;
                float accent_alpha = 0.5 + 0.5 * sin(wave_time + wave_offset);
                accent_alpha = pow(accent_alpha, 2.0);
                float diamond_size = 0.5;
                float border_thickness = 0.03;
                float diamond_fill_mask = 1.0 - smoothstep(diamond_size, diamond_size, manhattan_dist);
                float border_glow_mask = smoothstep(diamond_size - border_thickness, diamond_size, manhattan_dist) -
                                        smoothstep(diamond_size, diamond_size + border_thickness, manhattan_dist);
                vec3 final_color = COLOR_BG_DARK;
                final_color = mix(final_color, COLOR_DIAMOND, diamond_fill_mask);
                final_color = mix(final_color, COLOR_ACCENT, border_glow_mask * accent_alpha);
                gl_FragColor = vec4(final_color, 1.0);
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

            let effect_time_location = gl.get_uniform_location(program, "effect_time").unwrap();
            let rotation_time_location = gl.get_uniform_location(program, "rotation_time").unwrap();
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

            Self {
                gl,
                program,
                effect_time_location,
                rotation_time_location,
                vbo,
                vao,
                start_time: web_time::Instant::now(),
                rotation_time: 0.0,
                last_rotation_enabled: true,
                rotation_pause_offset: 0.0,
            }
        }
    }
}

impl Drop for EGLUnderlay {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_program(self.program);
            self.gl.delete_vertex_array(self.vao);
            self.gl.delete_buffer(self.vbo);
        }
    }
}

impl EGLUnderlay {
    fn render(&mut self, rotation_enabled: bool) {
        unsafe {
            let gl = &self.gl;

            gl.use_program(Some(self.program));

            // Retrieving the buffer with glow only works with native builds right now. For WASM this requires https://github.com/grovesNL/glow/pull/190
            // That means we can't properly restore the vao/vbo, but this is okay for now as this only works with femtovg, which doesn't rely on
            // these bindings to persist across frames.
            #[cfg(not(target_arch = "wasm32"))]
            let old_buffer =
                std::num::NonZeroU32::new(gl.get_parameter_i32(glow::ARRAY_BUFFER_BINDING) as u32)
                    .map(glow::NativeBuffer);
            #[cfg(target_arch = "wasm32")]
            let old_buffer = None;

            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));

            #[cfg(not(target_arch = "wasm32"))]
            let old_vao =
                std::num::NonZeroU32::new(gl.get_parameter_i32(glow::VERTEX_ARRAY_BINDING) as u32)
                    .map(glow::NativeVertexArray);
            #[cfg(target_arch = "wasm32")]
            let old_vao = None;

            gl.bind_vertex_array(Some(self.vao));

            let elapsed = self.start_time.elapsed().as_millis() as f32;
            gl.uniform_1_f32(Some(&self.effect_time_location), elapsed);

            // Handle the rotation and freezing of rotation via the UI toggle.
            if rotation_enabled {
                if !self.last_rotation_enabled {
                    self.rotation_pause_offset = elapsed - self.rotation_time;
                }
                self.rotation_time = elapsed - self.rotation_pause_offset;
            }

            gl.uniform_1_f32(Some(&self.rotation_time_location), self.rotation_time);

            self.last_rotation_enabled = rotation_enabled;

            gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);

            gl.bind_buffer(glow::ARRAY_BUFFER, old_buffer);
            gl.bind_vertex_array(old_vao);
            gl.use_program(None);
        }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

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
                        #[cfg(not(target_arch = "wasm32"))]
                        slint::GraphicsAPI::NativeOpenGL { get_proc_address } => unsafe {
                            glow::Context::from_loader_function_cstr(|s| get_proc_address(s))
                        },
                        #[cfg(target_arch = "wasm32")]
                        slint::GraphicsAPI::WebGL { canvas_element_id, context_type } => {
                            use wasm_bindgen::JsCast;

                            let canvas = web_sys::window()
                                .unwrap()
                                .document()
                                .unwrap()
                                .get_element_by_id(canvas_element_id)
                                .unwrap()
                                .dyn_into::<web_sys::HtmlCanvasElement>()
                                .unwrap();

                            let webgl1_context = canvas
                                .get_context(context_type)
                                .unwrap()
                                .unwrap()
                                .dyn_into::<web_sys::WebGl2RenderingContext>()
                                .unwrap();

                            glow::Context::from_webgl2_context(webgl1_context)
                        }
                        _ => return,
                    };
                    underlay = Some(EGLUnderlay::new(context))
                }
                slint::RenderingState::BeforeRendering => {
                    if let (Some(underlay), Some(app)) = (underlay.as_mut(), app_weak.upgrade()) {
                        underlay.render(app.get_rotation_enabled());
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
