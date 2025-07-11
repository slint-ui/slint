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
                "precision mediump float;\n"
                "varying vec2 frag_position;\n"
                "uniform float effect_time;\n"
                "uniform float rotation_time;\n"
                "float roundRectDistance(vec2 pos, vec2 rect_size, float radius)\n"
                "{\n"
                "    vec2 q = abs(pos) - rect_size + radius;\n"
                "    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - radius;\n"
                "}\n"
                "void main() {\n"
                "    vec2 size = vec2(0.4, 0.5) + 0.2 * cos(effect_time / 500. + vec2(0.3, 0.2));\n"
                "    float radius = 0.5 * sin(effect_time / 300.);\n"
                "    float a = rotation_time / 800.0;\n"
                "    float d = roundRectDistance(mat2(cos(a), -sin(a), sin(a), cos(a)) * "
                "frag_position, size, radius);\n"
                "    vec3 col = (d > 0.0) ? vec3(sin(d * 0.2), 0.4 * cos(effect_time / 1000.0 + d "
                "* 0.8), "
                "sin(d * 1.2)) : vec3(0.2 * cos(d * 0.1), 0.17 * sin(d * 0.4), 0.96 * "
                "abs(sin(effect_time "
                "/ 500. - d * 0.9)));\n"
                "    col *= 0.8 + 0.5 * sin(50.0 * d);\n"
                "    col = mix(col, vec3(0.9), 1.0 - smoothstep(0.0, 0.03, abs(d) ));\n"
                "    gl_FragColor = vec4(col, 1.0);\n"
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
        if (enable_rotation) {
            glUniform1f(rotation_time_location, elapsed.count());
        } else {
            glUniform1f(rotation_time_location, 0.0);
        }

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
