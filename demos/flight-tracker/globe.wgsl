// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore dpdx dpdy fwidth smoothstep texel

// The globe, its atmosphere, and the stars behind it, ray traced in one pass over the screen.

struct Globals {
    inverse_view_projection: mat4x4<f32>,
    // xyz: the camera's position.
    eye: vec4<f32>,
    // xyz: the direction towards the sun, w: how dark the night side is.
    sun: vec4<f32>,
    // y: the borders' opacity, z: the seconds since the start, for the twinkling stars.
    options: vec4<f32>,
    // x: how many texels the map's distances reach, y: the same for its signed distances to
    // the borders.
    spreads: vec4<f32>,
}

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var map: texture_2d<f32>;
@group(0) @binding(2) var map_sampler: sampler;
@group(0) @binding(3) var day_imagery: texture_2d<f32>;

const PI: f32 = 3.14159265;
const TAU: f32 = 6.2831853;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) ndc: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let ndc = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u)) * 2.0 - 1.0;
    return VertexOutput(vec4<f32>(ndc, 0.0, 1.0), ndc);
}

fn hash3(p: vec3<f32>) -> vec3<f32> {
    var q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    q += dot(q, q.yxz + 33.33);
    return fract((q.xxy + q.yxx) * q.zyx);
}

fn stars(direction: vec3<f32>) -> vec3<f32> {
    let cells = 260.0;
    let p = direction * cells;
    let cell = floor(p);
    let random = hash3(cell);
    if random.x < 0.92 {
        return vec3<f32>(0.0);
    }
    let center = cell + 0.25 + 0.5 * hash3(cell + 17.0);
    let distance = length(p - center);
    let twinkle = 0.8 + 0.2 * sin(globals.options.z * (1.0 + 3.0 * random.y) + random.z * TAU);
    let brightness = pow((random.x - 0.92) / 0.08, 3.0) * twinkle;
    let tint = mix(vec3<f32>(0.75, 0.82, 1.0), vec3<f32>(1.0, 0.9, 0.75), random.y);
    return tint * brightness * smoothstep(0.12, 0.0, distance);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let near = globals.inverse_view_projection * vec4<f32>(in.ndc, 0.0, 1.0);
    let far = globals.inverse_view_projection * vec4<f32>(in.ndc, 1.0, 1.0);
    let origin = globals.eye.xyz;
    let direction = normalize(far.xyz / far.w - near.xyz / near.w);
    let sun = globals.sun.xyz;

    // The ray's closest approach to the globe's center decides whether it hits the globe.
    let along = -dot(origin, direction);
    let closest = origin + direction * along;
    let miss_distance = length(closest);
    let hit_depth = sqrt(max(1.0 - miss_distance * miss_distance, 0.0));
    let normal = normalize(origin + direction * (along - hit_depth));

    // Derivatives need uniform control flow, so the map is sampled for every pixel.
    let lon = atan2(normal.x, normal.z);
    let lat = asin(clamp(normal.y, -1.0, 1.0));
    let uv = vec2<f32>(lon / TAU + 0.5, 0.5 - lat / PI);
    // The longitude jumps at the date line; the derivative of a copy shifted by half a turn
    // doesn't jump there.
    let shifted = fract(uv.x + 0.5);
    let du = vec2<f32>(dpdx(uv.x), dpdy(uv.x));
    let du_shifted = vec2<f32>(dpdx(shifted), dpdy(shifted));
    let du_smooth = select(du, du_shifted, length(du_shifted) < length(du));
    let dv = vec2<f32>(dpdx(uv.y), dpdy(uv.y));
    let map_texel = textureSampleGrad(
        map,
        map_sampler,
        uv,
        vec2<f32>(du_smooth.x, dv.x),
        vec2<f32>(du_smooth.y, dv.y)
    );
    let imagery = textureSampleGrad(
        day_imagery,
        map_sampler,
        uv,
        vec2<f32>(du_smooth.x, dv.x),
        vec2<f32>(du_smooth.y, dv.y)
    ).rgb;
    // The land and the borders, a pixel wide at their edges however far the map is magnified.
    let size = vec2<f32>(textureDimensions(map));
    let texels_per_pixel = max(
        max(length(vec2<f32>(du_smooth.x, dv.x) * size), length(vec2<f32>(du_smooth.y, dv.y) * size)),
        1e-3
    );
    let spread = globals.spreads.x;
    let coast_distance = map_texel.r - 0.5;
    let coast_change = max(fwidth(coast_distance), 1e-4);
    let land = clamp(coast_distance / coast_change + 0.5, 0.0, 1.0);
    // Between texels, the interpolated distance to a border is up to half a texel too far, so
    // where a texel is about a pixel, borders are at least a texel wide.
    let border_texels = (1.0 - map_texel.g) * spread;
    let border_half_width = max(0.6, 0.5 * texels_per_pixel);
    let minified_border = 1.0 - smoothstep(
        border_half_width - 0.5 * texels_per_pixel,
        border_half_width + 0.5 * texels_per_pixel,
        border_texels
    );
    // Where the map is magnified, the signed distance draws borders a pixel wide. It also
    // changes sign between unrelated borders, which only counts within the half texel that
    // the unsigned distance can be off by.
    let side_pixels = abs(map_texel.a - 0.5) * 2.0 * globals.spreads.y / texels_per_pixel;
    let magnified_border = (1.0 - smoothstep(0.2, 1.2, side_pixels))
        * (1.0 - smoothstep(0.55, 0.85, border_texels));
    let border = mix(magnified_border, minified_border, smoothstep(0.5, 1.0, texels_per_pixel))
        * globals.options.y;
    let edge = clamp((1.0 - miss_distance) / max(fwidth(miss_distance), 1e-5) + 0.5, 0.0, 1.0);
    let coverage = select(0.0, edge, along > 0.0);

    // 0 lights the whole globe evenly, 1 darkens the side away from the sun.
    let sun_strength = globals.sun.w;

    // The space around the globe, and the atmosphere where the sun lights it, reddish where
    // the sun rises or sets.
    var space = stars(direction) * smoothstep(1.02, 1.12, miss_distance);
    let limb_sun = dot(normalize(closest), sun);
    let limb_light = mix(1.0, smoothstep(-0.3, 0.25, limb_sun), sun_strength);
    let limb_dusk = exp(-pow(limb_sun / 0.18, 2.0)) * sun_strength;
    let halo = exp(-max(miss_distance - 1.0, 0.0) * 22.0) * select(0.0, 1.0, along > 0.0);
    let halo_color = vec3<f32>(0.35, 0.6, 1.0);
    space += (halo_color * limb_light + vec3<f32>(1.0, 0.5, 0.2) * limb_dusk * 0.6) * halo * 0.9;

    // The ground.
    let city_light = map_texel.b;
    let view = -direction;
    let facing = max(dot(normal, view), 0.0);
    let fresnel = pow(1.0 - facing, 4.0);
    let sunlight = dot(normal, sun);
    let day = mix(1.0, smoothstep(-0.04, 0.1, sunlight), sun_strength);
    // Twilight lasts while the sun is up to 12° below the horizon.
    let twilight = (1.0 - day) * smoothstep(-0.21, 0.0, sunlight) * sun_strength;
    // The line where the sun rises and sets.
    let terminator = exp(-pow(sunlight / 0.02, 2.0)) * sun_strength;
    let terminator_color = vec3<f32>(1.0, 0.62, 0.3);
    let diffuse = mix(1.0, clamp(sunlight * 1.2 + 0.25, 0.0, 1.0), sun_strength * 0.8);

    var lit = imagery * diffuse;
    let half_way = normalize(sun + view);
    let glint = pow(max(dot(normal, half_way), 0.0), 300.0) * (1.0 - land) * 0.5 * day;
    lit += vec3<f32>(1.0, 0.92, 0.8) * glint;
    lit = mix(lit, vec3<f32>(0.95, 0.95, 0.85), border * 0.45 * day);
    let night_ground = imagery * vec3<f32>(0.02, 0.025, 0.04);
    let lights = vec3<f32>(1.0, 0.72, 0.38) * city_light * land * 1.6;
    var ground = mix(night_ground + lights + vec3<f32>(0.09, 0.05, 0.06) * twilight, lit, day);
    ground += vec3<f32>(0.35, 0.6, 1.0) * fresnel * mix(0.05, 0.7, day);
    ground += terminator_color * terminator * 0.22;

    let color = mix(space, ground, coverage);
    return vec4<f32>(color, 1.0);
}
