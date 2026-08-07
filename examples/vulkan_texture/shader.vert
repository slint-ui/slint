// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// A single oversized triangle covering the whole target, so the fragment shader below runs
// once per pixel. No vertex buffer: the positions come from the vertex index.

#version 450

layout(location = 0) out vec2 frag_position;

vec2 positions[3] = vec2[](vec2(-1.0, 3.0), vec2(-1.0, -1.0), vec2(3.0, -1.0));

void main() {
    vec2 pos = positions[gl_VertexIndex];
    gl_Position = vec4(pos, 0.0, 1.0);
    frag_position = pos;
}
