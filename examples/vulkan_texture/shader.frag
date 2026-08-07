// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Ray marches a rounded cube out of a signed distance field. Same effect as the
// opengl_texture and wgpu_texture examples, so the three can be compared side by side.

#version 450

layout(location = 0) in vec2 frag_position;
layout(location = 0) out vec4 out_color;

layout(push_constant) uniform PushConstants {
    // xyz: the light color picked in the UI, w: seconds since start.
    vec4 light_color_and_time;
}
pc;

float sdRoundBox(vec3 p, vec3 b, float r) {
    vec3 q = abs(p) - b;
    return length(max(q, 0.0)) + min(max(q.x, max(q.y, q.z)), 0.0) - r;
}

vec3 rotateY(vec3 r, float angle) {
    mat3 rotation_matrix =
        mat3(cos(angle), 0, sin(angle), 0, 1, 0, -sin(angle), 0, cos(angle));
    return rotation_matrix * r;
}

vec3 rotateZ(vec3 r, float angle) {
    mat3 rotation_matrix =
        mat3(cos(angle), -sin(angle), 0, sin(angle), cos(angle), 0, 0, 0, 1);
    return rotation_matrix * r;
}

// Distance from the scene
float scene(vec3 r) {
    float iTime = pc.light_color_and_time.w;
    vec3 pos = rotateZ(rotateY(r + vec3(-1.0, -1.0, 4.0), iTime), iTime);
    vec3 cube = vec3(0.5, 0.5, 0.5);
    float edge = 0.1;
    return sdRoundBox(pos, cube, edge);
}

// https://iquilezles.org/articles/normalsSDF
vec3 normal(in vec3 pos) {
    vec2 e = vec2(1.0, -1.0) * 0.5773;
    const float eps = 0.0005;
    return normalize(e.xyy * scene(pos + e.xyy * eps) + e.yyx * scene(pos + e.yyx * eps) +
                     e.yxy * scene(pos + e.yxy * eps) + e.xxx * scene(pos + e.xxx * eps));
}

#define ITERATIONS 90
#define EPS 0.0001

vec4 render(vec2 fragCoord, vec3 light_color) {
    vec3 camera = vec3(1.0, 2.0, 1.0);
    vec3 p = vec3(fragCoord.x, fragCoord.y + 1.0, -1.0);
    vec3 dir = normalize(p - camera);

    for (int i = 0; i < ITERATIONS; i++) {
        float dist = scene(p);
        if (dist < EPS) {
            break;
        }
        p = p + dir * dist;
    }

    vec3 surf_normal = normal(p);

    vec3 light_position = vec3(2.0, 4.0, -0.5);
    float light = 7.0 + 2.0 * dot(surf_normal, light_position);
    light /= 0.2 * pow(length(light_position - p), 3.5);

    return vec4(light * light_color, 1.0) * 2.0;
}

void main() {
    vec3 selected_light_color = pc.light_color_and_time.xyz;
    vec2 r = vec2(0.5 * frag_position.x + 1.0, 0.5 - 0.5 * frag_position.y);
    out_color = render(r, selected_light_color);
}
