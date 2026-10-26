// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore Booleanv Floatv Integerv renderbuffer Storei

#pragma once

#include <vtk_glad.h>

#include <algorithm>
#include <array>
#include <cassert>
#include <cstdio>

// Slint's renderers expect the OpenGL state to be unchanged after the rendering notifier returns.
// Skia caches it, and FemtoVG relies on defaults for state it doesn't set.
// GLState covers what either of them, or VTK, may change.
struct GLState
{
    static constexpr std::array capabilities = {
        GL_BLEND,
        GL_COLOR_LOGIC_OP,
        GL_CULL_FACE,
        GL_DEPTH_CLAMP,
        GL_DEPTH_TEST,
        GL_DITHER,
        GL_FRAMEBUFFER_SRGB,
        GL_LINE_SMOOTH,
        GL_MULTISAMPLE,
        GL_POLYGON_OFFSET_FILL,
        GL_POLYGON_OFFSET_LINE,
        GL_POLYGON_OFFSET_POINT,
        GL_POLYGON_SMOOTH,
        GL_PRIMITIVE_RESTART,
        GL_PROGRAM_POINT_SIZE,
        GL_RASTERIZER_DISCARD,
        GL_SAMPLE_ALPHA_TO_COVERAGE,
        GL_SAMPLE_COVERAGE,
        GL_SCISSOR_TEST,
        GL_STENCIL_TEST,
        GL_TEXTURE_CUBE_MAP_SEAMLESS,
    };

    static constexpr std::array pixel_store_parameters = {
        GL_PACK_ALIGNMENT,    GL_PACK_IMAGE_HEIGHT,  GL_PACK_ROW_LENGTH,    GL_PACK_SKIP_IMAGES,
        GL_PACK_SKIP_PIXELS,  GL_PACK_SKIP_ROWS,     GL_UNPACK_ALIGNMENT,   GL_UNPACK_IMAGE_HEIGHT,
        GL_UNPACK_ROW_LENGTH, GL_UNPACK_SKIP_IMAGES, GL_UNPACK_SKIP_PIXELS, GL_UNPACK_SKIP_ROWS,
    };

    struct TextureTarget
    {
        GLenum target;
        GLenum binding;
    };
    static constexpr std::array texture_targets = {
        TextureTarget { GL_TEXTURE_1D, GL_TEXTURE_BINDING_1D },
        TextureTarget { GL_TEXTURE_2D, GL_TEXTURE_BINDING_2D },
        TextureTarget { GL_TEXTURE_2D_ARRAY, GL_TEXTURE_BINDING_2D_ARRAY },
        TextureTarget { GL_TEXTURE_2D_MULTISAMPLE, GL_TEXTURE_BINDING_2D_MULTISAMPLE },
        TextureTarget { GL_TEXTURE_3D, GL_TEXTURE_BINDING_3D },
        TextureTarget { GL_TEXTURE_BUFFER, GL_TEXTURE_BINDING_BUFFER },
        TextureTarget { GL_TEXTURE_CUBE_MAP, GL_TEXTURE_BINDING_CUBE_MAP },
        TextureTarget { GL_TEXTURE_RECTANGLE, GL_TEXTURE_BINDING_RECTANGLE },
    };
    // VTK hands out texture units starting from zero and uses only a few of them.
    static constexpr int max_texture_units = 16;

    struct TextureUnit
    {
        std::array<GLint, texture_targets.size()> bindings {};
        GLint sampler = 0;
        bool operator==(const TextureUnit &) const = default;
    };

    struct Stencil
    {
        GLint func = 0, ref = 0, value_mask = 0, fail = 0, depth_fail = 0, depth_pass = 0,
              write_mask = 0;
        bool operator==(const Stencil &) const = default;
    };

    std::array<GLboolean, capabilities.size()> enabled {};
    std::array<GLint, pixel_store_parameters.size()> pixel_store {};

    GLint draw_framebuffer = 0, read_framebuffer = 0, renderbuffer = 0;
    GLint draw_buffer = 0, read_buffer = 0;
    std::array<GLint, 4> viewport {}, scissor_box {};

    GLint program = 0, vertex_array = 0, array_buffer = 0, element_array_buffer = 0;
    GLint uniform_buffer = 0, pixel_pack_buffer = 0, pixel_unpack_buffer = 0;
    GLint copy_read_buffer = 0, copy_write_buffer = 0;

    GLint active_texture = 0;
    int texture_unit_count = 0;
    std::array<TextureUnit, max_texture_units> texture_units {};

    GLint blend_src_rgb = 0, blend_dst_rgb = 0, blend_src_alpha = 0, blend_dst_alpha = 0;
    GLint blend_equation_rgb = 0, blend_equation_alpha = 0;
    std::array<GLfloat, 4> blend_color {};

    GLint depth_func = 0;
    GLboolean depth_write_mask = GL_FALSE;
    std::array<GLfloat, 2> depth_range {};
    GLfloat clear_depth = 0;

    Stencil stencil_front, stencil_back;
    GLint clear_stencil = 0;

    std::array<GLboolean, 4> color_write_mask {};
    std::array<GLfloat, 4> clear_color {};

    GLint cull_face_mode = 0, front_face = 0;
    std::array<GLint, 2> polygon_mode {};
    GLfloat polygon_offset_factor = 0, polygon_offset_units = 0;
    GLfloat line_width = 0, point_size = 0;

    bool operator==(const GLState &) const = default;

    // Returns a copy that no longer refers to objects deleted since the capture.
    // Binding a deleted name is an error in core profiles.
    GLState without_deleted_objects() const
    {
        GLState s = *this;
        for (int unit = 0; unit < s.texture_unit_count; ++unit) {
            for (auto &binding : s.texture_units[unit].bindings) {
                if (!glIsTexture(binding)) {
                    binding = 0;
                }
            }
            if (GLAD_GL_VERSION_3_3 && !glIsSampler(s.texture_units[unit].sampler)) {
                s.texture_units[unit].sampler = 0;
            }
        }
        for (auto *buffer :
             { &s.array_buffer, &s.element_array_buffer, &s.uniform_buffer, &s.pixel_pack_buffer,
               &s.pixel_unpack_buffer, &s.copy_read_buffer, &s.copy_write_buffer }) {
            if (!glIsBuffer(*buffer)) {
                *buffer = 0;
            }
        }
        if (!glIsRenderbuffer(s.renderbuffer)) {
            s.renderbuffer = 0;
        }
        for (auto *framebuffer : { &s.draw_framebuffer, &s.read_framebuffer }) {
            if (!glIsFramebuffer(*framebuffer)) {
                *framebuffer = 0;
            }
        }
        if (!glIsProgram(s.program)) {
            s.program = 0;
        }
        if (!glIsVertexArray(s.vertex_array)) {
            s.vertex_array = 0;
        }
        return s;
    }

    static GLState capture()
    {
        GLState s;
        for (size_t i = 0; i < capabilities.size(); ++i) {
            s.enabled[i] = glIsEnabled(capabilities[i]);
        }
        for (size_t i = 0; i < pixel_store_parameters.size(); ++i) {
            glGetIntegerv(pixel_store_parameters[i], &s.pixel_store[i]);
        }

        glGetIntegerv(GL_DRAW_FRAMEBUFFER_BINDING, &s.draw_framebuffer);
        glGetIntegerv(GL_READ_FRAMEBUFFER_BINDING, &s.read_framebuffer);
        glGetIntegerv(GL_RENDERBUFFER_BINDING, &s.renderbuffer);
        glGetIntegerv(GL_DRAW_BUFFER, &s.draw_buffer);
        glGetIntegerv(GL_READ_BUFFER, &s.read_buffer);
        glGetIntegerv(GL_VIEWPORT, s.viewport.data());
        glGetIntegerv(GL_SCISSOR_BOX, s.scissor_box.data());

        glGetIntegerv(GL_CURRENT_PROGRAM, &s.program);
        glGetIntegerv(GL_VERTEX_ARRAY_BINDING, &s.vertex_array);
        glGetIntegerv(GL_ARRAY_BUFFER_BINDING, &s.array_buffer);
        glGetIntegerv(GL_ELEMENT_ARRAY_BUFFER_BINDING, &s.element_array_buffer);
        glGetIntegerv(GL_UNIFORM_BUFFER_BINDING, &s.uniform_buffer);
        glGetIntegerv(GL_PIXEL_PACK_BUFFER_BINDING, &s.pixel_pack_buffer);
        glGetIntegerv(GL_PIXEL_UNPACK_BUFFER_BINDING, &s.pixel_unpack_buffer);
        glGetIntegerv(GL_COPY_READ_BUFFER_BINDING, &s.copy_read_buffer);
        glGetIntegerv(GL_COPY_WRITE_BUFFER_BINDING, &s.copy_write_buffer);

        glGetIntegerv(GL_ACTIVE_TEXTURE, &s.active_texture);
        GLint unit_count = 0;
        glGetIntegerv(GL_MAX_COMBINED_TEXTURE_IMAGE_UNITS, &unit_count);
        s.texture_unit_count = std::min(unit_count, max_texture_units);
        for (int unit = 0; unit < s.texture_unit_count; ++unit) {
            glActiveTexture(GL_TEXTURE0 + unit);
            auto &u = s.texture_units[unit];
            for (size_t i = 0; i < texture_targets.size(); ++i) {
                glGetIntegerv(texture_targets[i].binding, &u.bindings[i]);
            }
            if (GLAD_GL_VERSION_3_3) {
                glGetIntegerv(GL_SAMPLER_BINDING, &u.sampler);
            }
        }
        glActiveTexture(s.active_texture);

        glGetIntegerv(GL_BLEND_SRC_RGB, &s.blend_src_rgb);
        glGetIntegerv(GL_BLEND_DST_RGB, &s.blend_dst_rgb);
        glGetIntegerv(GL_BLEND_SRC_ALPHA, &s.blend_src_alpha);
        glGetIntegerv(GL_BLEND_DST_ALPHA, &s.blend_dst_alpha);
        glGetIntegerv(GL_BLEND_EQUATION_RGB, &s.blend_equation_rgb);
        glGetIntegerv(GL_BLEND_EQUATION_ALPHA, &s.blend_equation_alpha);
        glGetFloatv(GL_BLEND_COLOR, s.blend_color.data());

        glGetIntegerv(GL_DEPTH_FUNC, &s.depth_func);
        glGetBooleanv(GL_DEPTH_WRITEMASK, &s.depth_write_mask);
        glGetFloatv(GL_DEPTH_RANGE, s.depth_range.data());
        glGetFloatv(GL_DEPTH_CLEAR_VALUE, &s.clear_depth);

        s.stencil_front = capture_stencil(GL_STENCIL_FUNC, GL_STENCIL_REF, GL_STENCIL_VALUE_MASK,
                                          GL_STENCIL_FAIL, GL_STENCIL_PASS_DEPTH_FAIL,
                                          GL_STENCIL_PASS_DEPTH_PASS, GL_STENCIL_WRITEMASK);
        s.stencil_back = capture_stencil(
                GL_STENCIL_BACK_FUNC, GL_STENCIL_BACK_REF, GL_STENCIL_BACK_VALUE_MASK,
                GL_STENCIL_BACK_FAIL, GL_STENCIL_BACK_PASS_DEPTH_FAIL,
                GL_STENCIL_BACK_PASS_DEPTH_PASS, GL_STENCIL_BACK_WRITEMASK);
        glGetIntegerv(GL_STENCIL_CLEAR_VALUE, &s.clear_stencil);

        glGetBooleanv(GL_COLOR_WRITEMASK, s.color_write_mask.data());
        glGetFloatv(GL_COLOR_CLEAR_VALUE, s.clear_color.data());

        glGetIntegerv(GL_CULL_FACE_MODE, &s.cull_face_mode);
        glGetIntegerv(GL_FRONT_FACE, &s.front_face);
        glGetIntegerv(GL_POLYGON_MODE, s.polygon_mode.data());
        glGetFloatv(GL_POLYGON_OFFSET_FACTOR, &s.polygon_offset_factor);
        glGetFloatv(GL_POLYGON_OFFSET_UNITS, &s.polygon_offset_units);
        glGetFloatv(GL_LINE_WIDTH, &s.line_width);
        glGetFloatv(GL_POINT_SIZE, &s.point_size);
        return s;
    }

    void restore() const
    {
        for (size_t i = 0; i < capabilities.size(); ++i) {
            enabled[i] ? glEnable(capabilities[i]) : glDisable(capabilities[i]);
        }

        // Pixel store parameters apply to transfers through the bound pixel buffers,
        // so restore the buffer bindings first.
        glBindBuffer(GL_PIXEL_PACK_BUFFER, pixel_pack_buffer);
        glBindBuffer(GL_PIXEL_UNPACK_BUFFER, pixel_unpack_buffer);
        for (size_t i = 0; i < pixel_store_parameters.size(); ++i) {
            glPixelStorei(pixel_store_parameters[i], pixel_store[i]);
        }

        // The draw and read buffer selections belong to the bound framebuffers.
        glBindFramebuffer(GL_DRAW_FRAMEBUFFER, draw_framebuffer);
        glBindFramebuffer(GL_READ_FRAMEBUFFER, read_framebuffer);
        glDrawBuffer(draw_buffer);
        glReadBuffer(read_buffer);
        glBindRenderbuffer(GL_RENDERBUFFER, renderbuffer);
        glViewport(viewport[0], viewport[1], viewport[2], viewport[3]);
        glScissor(scissor_box[0], scissor_box[1], scissor_box[2], scissor_box[3]);

        glUseProgram(program);
        // The element array buffer binding belongs to the bound vertex array.
        glBindVertexArray(vertex_array);
        if (vertex_array) {
            glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, element_array_buffer);
        }
        glBindBuffer(GL_ARRAY_BUFFER, array_buffer);
        glBindBuffer(GL_UNIFORM_BUFFER, uniform_buffer);
        glBindBuffer(GL_COPY_READ_BUFFER, copy_read_buffer);
        glBindBuffer(GL_COPY_WRITE_BUFFER, copy_write_buffer);

        for (int unit = 0; unit < texture_unit_count; ++unit) {
            glActiveTexture(GL_TEXTURE0 + unit);
            const auto &u = texture_units[unit];
            for (size_t i = 0; i < texture_targets.size(); ++i) {
                glBindTexture(texture_targets[i].target, u.bindings[i]);
            }
            if (GLAD_GL_VERSION_3_3) {
                glBindSampler(unit, u.sampler);
            }
        }
        glActiveTexture(active_texture);

        glBlendFuncSeparate(blend_src_rgb, blend_dst_rgb, blend_src_alpha, blend_dst_alpha);
        glBlendEquationSeparate(blend_equation_rgb, blend_equation_alpha);
        glBlendColor(blend_color[0], blend_color[1], blend_color[2], blend_color[3]);

        glDepthFunc(depth_func);
        glDepthMask(depth_write_mask);
        glDepthRange(depth_range[0], depth_range[1]);
        glClearDepth(clear_depth);

        restore_stencil(GL_FRONT, stencil_front);
        restore_stencil(GL_BACK, stencil_back);
        glClearStencil(clear_stencil);

        glColorMask(color_write_mask[0], color_write_mask[1], color_write_mask[2],
                    color_write_mask[3]);
        glClearColor(clear_color[0], clear_color[1], clear_color[2], clear_color[3]);

        glCullFace(cull_face_mode);
        glFrontFace(front_face);
        glPolygonMode(GL_FRONT_AND_BACK, polygon_mode[0]);
        glPolygonOffset(polygon_offset_factor, polygon_offset_units);
        glLineWidth(line_width);
        glPointSize(point_size);
    }

private:
    static Stencil capture_stencil(GLenum func, GLenum ref, GLenum value_mask, GLenum fail,
                                   GLenum depth_fail, GLenum depth_pass, GLenum write_mask)
    {
        Stencil s;
        glGetIntegerv(func, &s.func);
        glGetIntegerv(ref, &s.ref);
        glGetIntegerv(value_mask, &s.value_mask);
        glGetIntegerv(fail, &s.fail);
        glGetIntegerv(depth_fail, &s.depth_fail);
        glGetIntegerv(depth_pass, &s.depth_pass);
        glGetIntegerv(write_mask, &s.write_mask);
        return s;
    }

    static void restore_stencil(GLenum face, const Stencil &s)
    {
        glStencilFuncSeparate(face, s.func, s.ref, s.value_mask);
        glStencilOpSeparate(face, s.fail, s.depth_fail, s.depth_pass);
        glStencilMaskSeparate(face, s.write_mask);
    }
};

// Captures the OpenGL state on construction and restores it on destruction.
// Debug builds verify that the restored state matches the captured one.
class GLStateGuard
{
public:
    GLStateGuard()
    {
        report_errors("Slint's rendering");
        saved = GLState::capture();
    }
    GLStateGuard(const GLStateGuard &) = delete;
    GLStateGuard &operator=(const GLStateGuard &) = delete;

    ~GLStateGuard()
    {
        report_errors("VTK rendering");
        const GLState expected = saved.without_deleted_objects();
        expected.restore();
        report_errors("restoring the OpenGL state");
        assert(GLState::capture() == expected);
    }

private:
    // OpenGL keeps error flags until they're queried,
    // so drain them whenever control passes between Slint and VTK.
    static void report_errors(const char *during)
    {
        for (GLenum error = glGetError(); error != GL_NO_ERROR; error = glGetError()) {
            fprintf(stderr, "OpenGL error 0x%x during %s\n", error, during);
        }
    }

    GLState saved;
};
