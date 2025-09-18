// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "scene.h"

#include <cstdlib>
#include <iostream>
#include <stdlib.h>
#include <stdio.h>
#include <chrono>

#include <GLES2/gl2.h>
#include <GLES2/gl2platform.h>

static GLint compile_shader(GLuint program, GLuint shader_type, const GLchar *const *source)
{
    auto shader_id = glCreateShader(shader_type);
    glShaderSource(shader_id, 1, source, nullptr);
    glCompileShader(shader_id);

    GLint compiled = 0;
    glGetShaderiv(shader_id, GL_COMPILE_STATUS, &compiled);
    if (!compiled) {
        GLint infoLen = 0;
        glGetShaderiv(shader_id, GL_INFO_LOG_LENGTH, &infoLen);
        if (infoLen > 1) {
            char *infoLog = reinterpret_cast<char *>(malloc(sizeof(char) * infoLen));
            glGetShaderInfoLog(shader_id, infoLen, NULL, infoLog);
            fprintf(stderr, "Error compiling %s shader:\n%s\n",
                    shader_type == GL_FRAGMENT_SHADER ? "fragment shader" : "vertex shader",
                    infoLog);
            free(infoLog);
        }
        glDeleteShader(shader_id);
        exit(1);
    }
    glAttachShader(program, shader_id);

    return shader_id;
}

class OpenGLUnderlay
{
public:
    OpenGLUnderlay(slint::ComponentWeakHandle<App> app) : app_weak(app) { }

    void operator()(slint::RenderingState state, slint::GraphicsAPI)
    {
        switch (state) {
        case slint::RenderingState::RenderingSetup:
            setup();
            break;
        case slint::RenderingState::BeforeRendering:
            if (auto app = app_weak.lock()) {
                render((*app)->get_rotation_enabled());
                (*app)->window().request_redraw();
            }
            break;
        case slint::RenderingState::AfterRendering:
            break;
        case slint::RenderingState::RenderingTeardown:
            teardown();
            break;
        }
    }

private:
    void setup()
    {
        program = glCreateProgram();

        const GLchar *const fragment_shader =
                "#version 100\n"
                "#ifdef GL_FRAGMENT_PRECISION_HIGH\n"
                "    precision highp float;\n"
                "#else\n"
                "    precision mediump float;\n"
                "#endif\n"
                "varying vec2 frag_position;\n"
                "uniform float effect_time;\n"
                "uniform float rotation_time;\n"
                "const vec3 COLOR_BG_DARK   = vec3(0.106, 0.106, 0.118);\n"
                "const vec3 COLOR_DIAMOND   = vec3(0.137, 0.149, 0.184);\n"
                "const vec3 COLOR_ACCENT    = vec3(0.12, 0.35, 0.75);\n"
                "mat2 rotate(float angle) {\n"
                "    float s = sin(angle);\n"
                "    float c = cos(angle);\n"
                "    return mat2(c, -s, s, c);\n"
                "}\n"
                "void main() {\n"
                "    vec2 p_coords = frag_position;\n"
                "    float perspective_strength = 0.09;\n"
                "    float divisor = 1.0 + (-p_coords.y + 1.0) * perspective_strength;\n"
                "    p_coords /= divisor;\n"
                "    p_coords.y *= (1.0 + perspective_strength * 1.5);\n"
                "    const float MAX_ANGLE_DEGREES = 10.0;\n"
                "    float max_angle_rad = radians(MAX_ANGLE_DEGREES);\n"
                "    float oscillating_factor = sin(rotation_time / 1700.0);\n"
                "    float angle = oscillating_factor * max_angle_rad;\n"
                "    mat2 rotation_matrix = rotate(angle);\n"
                "    vec2 uv = rotation_matrix * p_coords * 6.0;\n"
                "    vec2 grid_id = floor(uv);\n"
                "    vec2 grid_uv = fract(uv) - 0.5;\n"
                "    float manhattan_dist = abs(grid_uv.x) + abs(grid_uv.y);\n"
                "    float wave_time = effect_time / 300.0;\n"
                "    float wave_offset = grid_id.x * 0.5 + grid_id.y * 0.15;\n"
                "    float accent_alpha = 0.5 + 0.5 * sin(wave_time + wave_offset);\n"
                "    accent_alpha = pow(accent_alpha, 2.0);\n"
                "    float diamond_size = 0.5;\n"
                "    float border_thickness = 0.03;\n"
                "    float diamond_fill_mask = 1.0 - smoothstep(diamond_size, diamond_size, "
                "manhattan_dist);\n"
                "    float border_glow_mask = smoothstep(diamond_size - border_thickness, "
                "diamond_size, manhattan_dist) -\n"
                "                            smoothstep(diamond_size, diamond_size + "
                "border_thickness, manhattan_dist);\n"
                "    vec3 final_color = COLOR_BG_DARK;\n"
                "    final_color = mix(final_color, COLOR_DIAMOND, diamond_fill_mask);\n"
                "    final_color = mix(final_color, COLOR_ACCENT, border_glow_mask * "
                "accent_alpha);\n"
                "    gl_FragColor = vec4(final_color, 1.0);\n"
                "}\n";

        const GLchar *const vertex_shader = "#version 100\n"
                                            "attribute vec2 position;\n"
                                            "varying vec2 frag_position;\n"
                                            "void main() {\n"
                                            "    frag_position = position;\n"
                                            "    gl_Position = vec4(position, 0.0, 1.0);\n"
                                            "}\n";

        auto fragment_shader_id = compile_shader(program, GL_FRAGMENT_SHADER, &fragment_shader);
        auto vertex_shader_id = compile_shader(program, GL_VERTEX_SHADER, &vertex_shader);

        GLint linked = 0;
        glLinkProgram(program);
        glGetProgramiv(program, GL_LINK_STATUS, &linked);

        if (!linked) {
            GLint infoLen = 0;
            glGetProgramiv(program, GL_INFO_LOG_LENGTH, &infoLen);
            if (infoLen > 1) {
                char *infoLog = reinterpret_cast<char *>(malloc(sizeof(char) * infoLen));
                glGetProgramInfoLog(program, infoLen, NULL, infoLog);
                fprintf(stderr, "Error linking shader:\n%s\n", infoLog);
                free(infoLog);
            }
            glDeleteProgram(program);
            exit(1);
        }
        glDetachShader(program, fragment_shader_id);
        glDetachShader(program, vertex_shader_id);

        position_location = glGetAttribLocation(program, "position");
        effect_time_location = glGetUniformLocation(program, "effect_time");
        rotation_time_location = glGetUniformLocation(program, "rotation_time");
    }

    void render(bool enable_rotation)
    {
        glUseProgram(program);
        const float vertices[] = { -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0, -1.0 };
        glVertexAttribPointer(position_location, 2, GL_FLOAT, GL_FALSE, 0, vertices);
        glEnableVertexAttribArray(position_location);

        auto elapsed = std::chrono::duration_cast<std::chrono::milliseconds>(
                std::chrono::steady_clock::now() - start_time);
        glUniform1f(effect_time_location, elapsed.count());

        // Handle the rotation and freezing of rotation via the UI toggle.
        if (enable_rotation) {
            if (!last_rotation_enabled) {
                rotation_pause_offset = elapsed.count() - rotation_time;
            }
            rotation_time = elapsed.count() - rotation_pause_offset;
        }

        glUniform1f(rotation_time_location, rotation_time);

        last_rotation_enabled = enable_rotation;

        glDrawArrays(GL_TRIANGLE_STRIP, 0, 4);
        glUseProgram(0);
    }

    void teardown() { glDeleteProgram(program); }

    slint::ComponentWeakHandle<App> app_weak;
    GLuint program = 0;
    GLuint position_location = 0;
    GLuint effect_time_location = 0;
    GLuint rotation_time_location = 0;
    std::chrono::time_point<std::chrono::steady_clock> start_time =
            std::chrono::steady_clock::now();
    double rotation_time = 0.0;
    bool last_rotation_enabled = true;
    double rotation_pause_offset = 0.0;
};

int main()
{
    auto app = App::create();

    if (auto error = app->window().set_rendering_notifier(OpenGLUnderlay(app))) {
        if (*error == slint::SetRenderingNotifierError::Unsupported) {
            fprintf(stderr,
                    "This example requires the use of a GL renderer. Please run with the "
                    "environment variable SLINT_BACKEND=winit-femtovg set.\n");
        } else {
            fprintf(stderr, "Unknown error calling set_rendering_notifier\n");
        }
        exit(EXIT_FAILURE);
    }

    app->run();
}
