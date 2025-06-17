// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) frag_position: vec2<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32
) -> VertexOutput {
    var output: VertexOutput;

    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0,  3.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0)
    );

    let pos = positions[vertex_index];
    output.position = vec4<f32>(pos.x, -pos.y, 0.0, 1.0);
    output.frag_position = pos;
    return output;
}

struct PushConstants {
    effect_time: f32,
    rotation_time: f32,
};

var<push_constant> pc: PushConstants;

fn roundRectDistance(pos: vec2<f32>, rect_size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(pos) - rect_size + vec2<f32>(radius);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - radius;
}

@fragment
fn fs_main(@location(0) frag_position: vec2<f32>) -> @location(0) vec4<f32> {
    let size = vec2<f32>(0.4, 0.5) + 0.2 * cos(pc.effect_time / 500.0 + vec2<f32>(0.3, 0.2));
    let radius = 0.5 * sin(pc.effect_time / 300.0);
    let a = pc.rotation_time / 800.0;
    let rot = mat2x2<f32>(
        cos(a), -sin(a),
        sin(a), cos(a)
    );
    let rotated_pos = rot * frag_position;
    let d = roundRectDistance(rotated_pos, size, radius);

    var col: vec3<f32>;
    if (d > 0.0) {
        col = vec3<f32>(
            sin(d * 0.2),
            0.4 * cos(pc.effect_time / 1000.0 + d * 0.8),
            sin(d * 1.2)
        );
    } else {
        col = vec3<f32>(
            0.2 * cos(d * 0.1),
            0.17 * sin(d * 0.4),
            0.96 * abs(sin(pc.effect_time / 500.0 - d * 0.9))
        );
    }

    col *= 0.8 + 0.5 * sin(50.0 * d);
    col = mix(col, vec3<f32>(0.9), 1.0 - smoothstep(0.0, 0.03, abs(d)));

    return vec4<f32>(col, 1.0);   
}
