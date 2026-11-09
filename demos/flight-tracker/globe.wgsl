// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore dpdx dpdy fwidth smoothstep texel

// The globe, its atmosphere, and the stars behind it, ray traced in one pass over the screen.

struct Globals {
    view_projection: mat4x4<f32>,
    inverse_view_projection: mat4x4<f32>,
    // xyz: the camera's position.
    eye: vec4<f32>,
    // xyz: the direction towards the sun, w: how dark the night side is.
    sun: vec4<f32>,
    // x: how much of space shows, the stars and the black, rather than transparency,
    // y: how wide the borders are in pixels, z: the seconds since the start, for the twinkling
    // stars, w: how strongly the borders show.
    options: vec4<f32>,
    // xy: the size of the frame in pixels, z: 1 where the shader encodes the output as sRGB,
    // because the target can't, w: how strongly the clouds show.
    viewport: vec4<f32>,
}

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var map: texture_2d<f32>;
@group(0) @binding(2) var map_sampler: sampler;
@group(0) @binding(3) var day_imagery: texture_2d<f32>;
@group(0) @binding(4) var night_imagery: texture_2d<f32>;
@group(0) @binding(5) var cloud_cover: texture_2d<f32>;

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
    let brightness = min(pow((random.x - 0.92) / 0.08, 2.0) * 2.0, 1.0) * twinkle;
    let tint = mix(vec3<f32>(0.75, 0.82, 1.0), vec3<f32>(1.0, 0.9, 0.75), random.y);
    return tint * brightness * smoothstep(0.1 + 0.14 * random.z, 0.0, distance);
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
    let city_light = textureSampleGrad(
        night_imagery,
        map_sampler,
        uv,
        vec2<f32>(du_smooth.x, dv.x),
        vec2<f32>(du_smooth.y, dv.y)
    ).r;
    // The clouds drift slowly eastwards. Their shadow on a point comes from the clouds a
    // little towards the sun.
    let cloud_strength = globals.viewport.w;
    let cloud_uv = uv + vec2<f32>(globals.options.z * 0.0004, 0.0);
    let cloud = textureSampleGrad(
        cloud_cover,
        map_sampler,
        cloud_uv,
        vec2<f32>(du_smooth.x, dv.x),
        vec2<f32>(du_smooth.y, dv.y)
    ).r * cloud_strength;
    let east = vec3<f32>(cos(lon), 0.0, -sin(lon));
    let north = vec3<f32>(-sin(lat) * sin(lon), cos(lat), -sin(lat) * cos(lon));
    let sun_on_map = vec2<f32>(
        dot(globals.sun.xyz, east) / max(cos(lat), 0.05) / TAU,
        -dot(globals.sun.xyz, north) / PI
    );
    let cloud_shadow = textureSampleGrad(
        cloud_cover,
        map_sampler,
        cloud_uv + sun_on_map * 0.004,
        vec2<f32>(du_smooth.x, dv.x),
        vec2<f32>(du_smooth.y, dv.y)
    ).r * cloud_strength;
    // The land, a pixel wide at its edge however far the map is magnified.
    let coast_distance = map_texel.r - 0.5;
    let coast_change = max(fwidth(coast_distance), 1e-4);
    let land = clamp(coast_distance / coast_change + 0.5, 0.0, 1.0);
    let edge = clamp((1.0 - miss_distance) / max(fwidth(miss_distance), 1e-5) + 0.5, 0.0, 1.0);
    let coverage = select(0.0, edge, along > 0.0);

    // 0 lights the whole globe evenly, 1 darkens the side away from the sun.
    let sun_strength = globals.sun.w;

    // The space around the globe, and the atmosphere where the sun lights it, reddish where
    // the sun rises or sets.
    let starlight = stars(direction) * smoothstep(1.02, 1.12, miss_distance);
    let limb_sun = dot(normalize(closest), sun);
    let limb_light = mix(1.0, smoothstep(-0.2, 0.4, limb_sun), sun_strength);
    let limb_dusk = exp(-pow(limb_sun / 0.1, 2.0)) * sun_strength;
    let halo = exp(-max(miss_distance - 1.0, 0.0) * 55.0) * select(0.0, 1.0, along > 0.0);
    let halo_color = vec3<f32>(0.4, 0.65, 1.0);
    let glow = (halo_color * limb_light + vec3<f32>(1.0, 0.5, 0.2) * limb_dusk * 0.4) * halo * 0.8;

    // The ground.
    let view = -direction;
    let facing = max(dot(normal, view), 0.0);
    let fresnel = pow(1.0 - facing, 5.0);
    let sunlight = dot(normal, sun);
    // The sunlight fades with its angle to the ground, slightly wrapped so that the edge to the
    // night is soft, and reddens where it crosses the most air, low over the horizon.
    let lambert = clamp((sunlight + 0.06) / 1.06, 0.0, 1.0);
    let reddening = mix(vec3<f32>(1.0, 0.58, 0.36), vec3<f32>(1.0), smoothstep(0.0, 0.3, sunlight));
    let sun_light = mix(vec3<f32>(1.0), reddening * lambert * 1.2, sun_strength);
    // The night side under faint, bluish moonlight, and the cities' lights coming on at dusk.
    let darkness = smoothstep(0.08, -0.12, sunlight) * sun_strength;
    let moonlight = vec3<f32>(0.03, 0.04, 0.075) * darkness;
    let lights = vec3<f32>(1.0, 0.62, 0.3) * city_light * 2.4 * darkness;

    var ground = imagery * (sun_light + moonlight) * (1.0 - cloud_shadow * 0.45 * lambert);
    ground += lights * (1.0 - cloud * 0.8);
    let half_way = normalize(sun + view);
    let glint = pow(max(dot(normal, half_way), 0.0), 500.0) * (1.0 - land) * 0.15 * lambert;
    ground += vec3<f32>(1.0, 0.92, 0.8) * glint * (1.0 - cloud);
    // The clouds, white in sunlight, reddened at dusk, and faintly blue under the moon.
    ground = mix(ground, sun_light * 1.05 + moonlight * 0.7, cloud * 0.92);
    // Towards the edge of the disk, the day side fades into the atmosphere's blue haze.
    let haze = pow(1.0 - facing, 3.0) * 0.35 * mix(1.0, smoothstep(-0.1, 0.4, sunlight), sun_strength);
    ground = mix(ground, vec3<f32>(0.4, 0.62, 1.0) * mix(1.0, lambert + 0.15, sun_strength), haze);
    ground += vec3<f32>(0.35, 0.6, 1.0) * fresnel * 0.03 * (1.0 - darkness);
    // A soft shoulder, so that bright deserts and the glint don't clip.
    ground = ground * 1.2 / (1.0 + ground * 0.2);

    // Where there's no space, the frame is transparent but for the atmosphere's glow.
    let space = globals.options.x;
    let glow_alpha = clamp(max(glow.r, max(glow.g, glow.b)), 0.0, 1.0);
    let premultiplied = mix(glow, ground, coverage) + starlight * (1.0 - coverage) * space;
    let alpha = mix(coverage + (1.0 - coverage) * glow_alpha, 1.0, space);
    // Slint shows the frame with straight alpha.
    return vec4<f32>(output(premultiplied / max(alpha, 1e-4)), alpha);
}

fn output(color: vec3<f32>) -> vec3<f32> {
    if globals.viewport.z == 0.0 {
        return color;
    }
    let linear = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
    let curve = 1.055 * pow(linear, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(curve, linear * 12.92, linear <= vec3<f32>(0.0031308));
}

struct BorderOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) ground: vec3<f32>,
    // How many pixels from the middle of the line.
    @location(1) across: f32,
}

// Each border segment is an instance of a quad that's a constant width on the screen.
@vertex
fn vs_border(
    @builtin(vertex_index) index: u32,
    @location(0) start: vec3<f32>,
    @location(1) end: vec3<f32>,
) -> BorderOutput {
    let along = f32((0x32u >> index) & 1u);
    let side = f32((0x2cu >> index) & 1u) * 2.0 - 1.0;
    let eye = globals.eye.xyz;
    let clip_start = globals.view_projection * vec4<f32>(start, 1.0);
    let clip_end = globals.view_projection * vec4<f32>(end, 1.0);
    var out: BorderOutput;
    out.ground = mix(start, end, along);
    // Behind the camera or the globe, it collapses to nothing.
    if min(clip_start.w, clip_end.w) <= 1e-4 || max(dot(start, eye), dot(end, eye)) < 1.0 {
        out.position = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        return out;
    }
    let viewport = globals.viewport.xy;
    let on_screen = (clip_end.xy / clip_end.w - clip_start.xy / clip_start.w) * viewport;
    let direction = on_screen / max(length(on_screen), 1e-4);
    // A pixel more than the line, for its smooth edges, and past its ends, to close the joints.
    let reach = globals.options.y * 0.5 + 1.0;
    let offset = (vec2<f32>(-direction.y, direction.x) * side + direction * (along * 2.0 - 1.0))
        * reach * 2.0 / viewport;
    let clip = mix(clip_start, clip_end, along);
    out.position = vec4<f32>(clip.xy + offset * clip.w, 0.0, clip.w);
    out.across = side * reach;
    return out;
}

@fragment
fn fs_border(in: BorderOutput) -> @location(0) vec4<f32> {
    let normal = normalize(in.ground);
    let view = normalize(globals.eye.xyz - in.ground);
    // Lines bunch up towards the horizon, so they fade before they get there.
    let facing = smoothstep(0.0, 0.15, dot(normal, view));
    let sunlight = dot(normal, globals.sun.xyz);
    let day = mix(1.0, smoothstep(-0.02, 0.05, sunlight), globals.sun.w);
    let edge = clamp(globals.options.y * 0.5 + 0.5 - abs(in.across), 0.0, 1.0);
    return vec4<f32>(output(vec3<f32>(0.95, 0.95, 0.85)), edge * facing * day * 0.45 * globals.options.w);
}
