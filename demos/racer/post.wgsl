// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore Khronos

// High quality's bloom and tone mapping, see `Bloom` in `renderer.rs`: the bright parts of
// the scene are halved in size step by step, blurred back up, and added to the scene.

// The texture each pass reads, and the scene, for the last pass that adds the bloom to it.
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var scene: texture_2d<f32>;
@group(0) @binding(2) var linear: sampler;

struct Varying {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// One triangle that covers the view.
@vertex
fn vs_full(@builtin(vertex_index) vertex: u32) -> Varying {
    let x = f32(vertex / 2u) * 4.0 - 1.0;
    let y = f32(vertex % 2u) * 4.0 - 1.0;
    var out: Varying;
    out.clip = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>(x * 0.5 + 0.5, 0.5 - y * 0.5);
    return out;
}

// The average of the four texels around `uv`, which halves the size.
fn down(uv: vec2<f32>) -> vec3<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(source));
    var sum = vec3<f32>(0.0);
    for (var i = 0; i < 4; i++) {
        let offset = vec2<f32>(f32(i % 2) - 0.5, f32(i / 2) - 0.5) * texel;
        sum += textureSampleLevel(source, linear, uv + offset, 0.0).rgb;
    }
    return sum / 4.0;
}

// The light above the threshold, fading in over a soft knee, from the scene at half size.
@fragment
fn fs_bright(in: Varying) -> @location(0) vec4<f32> {
    // Single bright pixels, such as a tiny light far away, would flicker as they move.
    let color = min(down(in.uv), vec3<f32>(16.0));
    let brightness = max(color.r, max(color.g, color.b));
    let threshold = 0.9;
    let knee = 0.4;
    let soft = clamp(brightness - threshold + knee, 0.0, 2.0 * knee);
    let share = max(soft * soft / (4.0 * knee), brightness - threshold) / max(brightness, 1e-4);
    return vec4<f32>(color * share, 1.0);
}

@fragment
fn fs_down(in: Varying) -> @location(0) vec4<f32> {
    return vec4<f32>(down(in.uv), 1.0);
}

// A tent filter over the smaller level, added to the larger one it's drawn into.
@fragment
fn fs_up(in: Varying) -> @location(0) vec4<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(source));
    var sum = vec3<f32>(0.0);
    for (var x = -1; x <= 1; x++) {
        for (var y = -1; y <= 1; y++) {
            let weight = f32((2 - abs(x)) * (2 - abs(y)));
            let offset = vec2<f32>(f32(x), f32(y)) * texel;
            sum += textureSampleLevel(source, linear, in.uv + offset, 0.0).rgb * weight;
        }
    }
    return vec4<f32>(sum / 16.0, 1.0);
}

// The scene with its bloom, tone mapped.
@fragment
fn fs_composite(in: Varying) -> @location(0) vec4<f32> {
    let bloom = textureSampleLevel(source, linear, in.uv, 0.0).rgb;
    var color = textureSampleLevel(scene, linear, in.uv, 0.0).rgb + bloom * 0.6;

    // Khronos PBR Neutral tone mapping, as three.js's `NeutralToneMapping`.
    let low = min(color.r, min(color.g, color.b));
    color -= select(0.04, low - 6.25 * low * low, low < 0.08);
    let peak = max(color.r, max(color.g, color.b));
    if peak >= 0.76 {
        let new_peak = 1.0 - 0.0576 / (peak - 0.52);
        color *= new_peak / peak;
        color = mix(color, vec3<f32>(new_peak), 1.0 - 1.0 / (0.15 * (peak - new_peak) + 1.0));
    }
    return vec4<f32>(color, 1.0);
}
