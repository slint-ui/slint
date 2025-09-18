// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "scene.h"

#include <cstdlib>
#include <iostream>
#include <stdlib.h>
#include <stdio.h>
#include <chrono>
#include <cassert>
#include <concepts>

#include <GLES3/gl3.h>
#include <GLES3/gl3platform.h>

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

#define DEFINE_SCOPED_BINDING(StructName, ParamName, BindingFn, TargetName)                        \
    struct StructName                                                                              \
    {                                                                                              \
        GLuint saved_value = {};                                                                   \
        StructName() = delete;                                                                     \
        StructName(const StructName &) = delete;                                                   \
        StructName &operator=(const StructName &) = delete;                                        \
        StructName(GLuint new_value)                                                               \
        {                                                                                          \
            glGetIntegerv(ParamName, (GLint *)&saved_value);                                       \
            BindingFn(TargetName, new_value);                                                      \
        }                                                                                          \
        ~StructName()                                                                              \
        {                                                                                          \
            BindingFn(TargetName, saved_value);                                                    \
        }                                                                                          \
    }

DEFINE_SCOPED_BINDING(ScopedTextureBinding, GL_TEXTURE_BINDING_2D, glBindTexture, GL_TEXTURE_2D);
DEFINE_SCOPED_BINDING(ScopedFrameBufferBinding, GL_DRAW_FRAMEBUFFER_BINDING, glBindFramebuffer,
                      GL_DRAW_FRAMEBUFFER);
DEFINE_SCOPED_BINDING(ScopedVBOBinding, GL_ARRAY_BUFFER_BINDING, glBindBuffer, GL_ARRAY_BUFFER);

struct ScopedVAOBinding
{
    GLuint saved_value = {};
    ScopedVAOBinding() = delete;
    ScopedVAOBinding(const ScopedVAOBinding &) = delete;
    ScopedVAOBinding &operator=(const ScopedVAOBinding &) = delete;
    ScopedVAOBinding(GLuint new_value)
    {
        glGetIntegerv(GL_VERTEX_ARRAY_BINDING, (GLint *)&saved_value);
        glBindVertexArray(new_value);
    }
    ~ScopedVAOBinding() { glBindVertexArray(saved_value); }
};

struct DemoTexture
{
    GLuint texture;
    int width;
    int height;
    GLuint fbo;

    DemoTexture(int width, int height) : width(width), height(height)
    {
        glGenFramebuffers(1, &fbo);
        glGenTextures(1, &texture);

        ScopedTextureBinding activeTexture(texture);

        GLint old_unpack_alignment;
        glGetIntegerv(GL_UNPACK_ALIGNMENT, &old_unpack_alignment);
        GLint old_unpack_row_length;
        glGetIntegerv(GL_UNPACK_ROW_LENGTH, &old_unpack_row_length);
        GLint old_unpack_skip_pixels;
        glGetIntegerv(GL_UNPACK_SKIP_PIXELS, &old_unpack_skip_pixels);
        GLint old_unpack_skip_rows;
        glGetIntegerv(GL_UNPACK_SKIP_ROWS, &old_unpack_skip_rows);

        glPixelStorei(GL_UNPACK_ALIGNMENT, 1);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
        glPixelStorei(GL_UNPACK_ROW_LENGTH, width);
        glPixelStorei(GL_UNPACK_SKIP_PIXELS, 0);
        glPixelStorei(GL_UNPACK_SKIP_ROWS, 0);

        glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA, width, height, 0, GL_RGBA, GL_UNSIGNED_BYTE,
                     nullptr);

        ScopedFrameBufferBinding activeFBO(fbo);

        glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, texture, 0);

        assert(glCheckFramebufferStatus(GL_FRAMEBUFFER) == GL_FRAMEBUFFER_COMPLETE);

        glPixelStorei(GL_UNPACK_ALIGNMENT, old_unpack_alignment);
        glPixelStorei(GL_UNPACK_ROW_LENGTH, old_unpack_row_length);
        glPixelStorei(GL_UNPACK_SKIP_PIXELS, old_unpack_skip_pixels);
        glPixelStorei(GL_UNPACK_SKIP_ROWS, old_unpack_skip_rows);
    }
    DemoTexture(const DemoTexture &) = delete;
    DemoTexture &operator=(const DemoTexture &) = delete;
    ~DemoTexture()
    {
        glDeleteFramebuffers(1, &fbo);
        glDeleteTextures(1, &texture);
    }

    template<std::invocable<> Callback>
    void with_active_fbo(Callback callback)
    {
        ScopedFrameBufferBinding activeFBO(fbo);
        callback();
    }
};

class DemoRenderer
{
public:
    DemoRenderer(slint::ComponentWeakHandle<App> app) : app_weak(app) { }

    void operator()(slint::RenderingState state, slint::GraphicsAPI)
    {
        switch (state) {
        case slint::RenderingState::RenderingSetup:
            setup();
            break;
        case slint::RenderingState::BeforeRendering:
            if (auto app = app_weak.lock()) {
                auto red = (*app)->get_selected_red();
                auto green = (*app)->get_selected_green();
                auto blue = (*app)->get_selected_blue();
                auto width = (*app)->get_requested_texture_width();
                auto height = (*app)->get_requested_texture_height();
                auto texture = render(red, green, blue, width, height);
                (*app)->set_texture(texture);
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
                R"(#version 100
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
            })";

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

        GLuint position_location = glGetAttribLocation(program, "position");
        effect_time_location = glGetUniformLocation(program, "iTime");
        selected_light_color_position = glGetUniformLocation(program, "selected_light_color");

        displayed_texture = std::make_unique<DemoTexture>(320, 200);
        next_texture = std::make_unique<DemoTexture>(320, 200);

        glGenVertexArrays(1, &vao);
        glGenBuffers(1, &vbo);

        ScopedVBOBinding savedVBO(vbo);
        ScopedVAOBinding savedVAO(vao);

        const float vertices[] = { -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0, -1.0 };
        glBufferData(GL_ARRAY_BUFFER, sizeof(vertices) * sizeof(vertices[0]), &vertices,
                     GL_STATIC_DRAW);

        glEnableVertexAttribArray(position_location);
        glVertexAttribPointer(position_location, 2, GL_FLOAT, false, 8, 0);
    }

    slint::Image render(float red, float green, float blue, int width, int height)
    {
        ScopedVBOBinding savedVBO(vbo);
        ScopedVAOBinding savedVAO(vao);

        glUseProgram(program);

        if (next_texture->width != width || next_texture->height != height) {
            auto new_texture = std::make_unique<DemoTexture>(width, height);
            std::swap(next_texture, new_texture);
        }

        next_texture->with_active_fbo([&]() {
            GLint saved_viewport[4];
            glGetIntegerv(GL_VIEWPORT, &saved_viewport[0]);

            glViewport(0, 0, next_texture->width, next_texture->height);

            auto elapsed = std::chrono::duration_cast<std::chrono::milliseconds>(
                                   std::chrono::steady_clock::now() - start_time)
                    / 500.;
            glUniform1f(effect_time_location, elapsed.count());
            glUniform3f(selected_light_color_position, red, green, blue);

            glDrawArrays(GL_TRIANGLE_STRIP, 0, 4);

            glViewport(saved_viewport[0], saved_viewport[1], saved_viewport[2], saved_viewport[3]);
        });

        glUseProgram(0);

        auto resultTexture = slint::Image::create_from_borrowed_gl_2d_rgba_texture(
                next_texture->texture,
                { static_cast<uint32_t>(next_texture->width),
                  static_cast<uint32_t>(next_texture->height) });

        std::swap(next_texture, displayed_texture);

        return resultTexture;
    }

    void teardown()
    {
        glDeleteProgram(program);
        glDeleteVertexArrays(1, &vao);
        glDeleteBuffers(1, &vbo);
    }

    slint::ComponentWeakHandle<App> app_weak;
    GLuint vbo;
    GLuint vao;
    GLuint program = 0;
    GLuint effect_time_location = 0;
    GLuint selected_light_color_position = 0;
    std::chrono::time_point<std::chrono::steady_clock> start_time =
            std::chrono::steady_clock::now();
    std::unique_ptr<DemoTexture> displayed_texture;
    std::unique_ptr<DemoTexture> next_texture;
};

int main()
{
    auto app = App::create();

    if (auto error = app->window().set_rendering_notifier(DemoRenderer(app))) {
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
