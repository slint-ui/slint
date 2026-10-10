// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Spots of light and shade drawn into a floor's light map, see `Renderer::draw_light_map`.
// The color channels add light, and alpha multiplies the floor's own light.

struct Spots {
    // The center (xy) in clip space.
    centers: array<vec4<f32>, 192>,
    // The half axes along the spot's length (xy) and across it (zw), in clip space.
    axes: array<vec4<f32>, 192>,
    // The light added at full strength (rgb), and how much of the floor's light it takes
    // away there (a).
    colors: array<vec4<f32>, 192>,
    // x: 1 for a round spot, 0 for a soft rectangle; y and z: the rectangle's soft edge
    // along and across, as a share of the half axis.
    shapes: array<vec4<f32>, 192>,
}

// Arrays of vectors rather than of structs: NXP's Vivante driver miscompiles reading a
// struct from an array.
@group(0) @binding(0) var<uniform> spots: Spots;

struct Varying {
    @builtin(position) clip: vec4<f32>,
    // From -1 to 1 across the spot.
    @location(0) local: vec2<f32>,
    // The Vivante driver garbles vec4 varyings.
    @location(1) light: vec3<f32>,
    @location(2) shape: vec3<f32>,
    @location(3) shade: f32,
}

// The corners come from an array: the Vivante driver also miscompiles vectors built from
// vertex inputs.
@vertex
fn vs_spot(@builtin(vertex_index) vertex: u32, @builtin(instance_index) instance: u32) -> Varying {
    var corners = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, 1.0),
    );
    let corner = corners[vertex];
    let axes = spots.axes[instance];
    let position = spots.centers[instance].xy + axes.xy * corner.x + axes.zw * corner.y;
    var out: Varying;
    out.clip = vec4<f32>(position, 0.0, 1.0);
    out.local = corner;
    out.light = spots.colors[instance].rgb;
    out.shade = spots.colors[instance].a;
    out.shape = spots.shapes[instance].xyz;
    return out;
}

@fragment
fn fs_spot(in: Varying) -> @location(0) vec4<f32> {
    // Round spots fade like `textures::radial`.
    let r = min(length(in.local), 1.0);
    let round = select(0.45 * (1.0 - (r - 0.3) / 0.7), 1.0 - r / 0.3 * 0.55, r < 0.3);
    let along = 1.0 - smoothstep(1.0 - in.shape.y, 1.0, abs(in.local.x));
    let across = 1.0 - smoothstep(1.0 - in.shape.z, 1.0, abs(in.local.y));
    // The Vivante driver gets `mix` wrong here.
    let strength = select(along * across, round, in.shape.x > 0.5);
    return vec4<f32>(in.light * strength, 1.0 - in.shade * strength);
}
