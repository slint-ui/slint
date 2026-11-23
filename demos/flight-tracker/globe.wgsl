// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore fwidth smoothstep texel

// The globe as patches of a mesh that's fixed to it, see `patches.rs`, and around it the
// atmosphere's glow, the sun and its glare, and the stars, each as a mesh of its own, so that a
// pixel of the ground takes little more than a sample of its imagery.

struct Globals {
    view_projection: mat4x4<f32>,
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
@group(0) @binding(6) var tiles: texture_2d_array<f32>;
@group(0) @binding(7) var tile_sampler: sampler;

const PI: f32 = 3.14159265;
const TAU: f32 = 6.2831853;

// How much of the sun shows: none behind the globe, and none without space around the globe.
// It's in view where the line from the camera towards it passes the globe.
fn sun_in_view() -> f32 {
    let eye = globals.eye.xyz;
    let sun = globals.sun.xyz;
    let sun_along = -dot(eye, sun);
    let sun_miss = length(eye + sun * max(sun_along, 0.0));
    return select(1.0, smoothstep(0.995, 1.02, sun_miss), sun_along > 0.0) * globals.sun.w
        * globals.options.x;
}

// How fast the atmosphere's glow fades away from the globe, seen from `eye_distance`.
// Close to the ground, the camera is inside the soft glow that rims the globe from afar, so
// the glow thins to a line along the horizon there.
fn glow_thinness(eye_distance: f32) -> f32 {
    return mix(55.0, 450.0, smoothstep(0.4, 0.03, eye_distance - 1.0));
}

// The atmosphere's glow along the ray from `origin` in `direction`, which comes closest to the
// globe's center at `closest`. The sun lights it, reddish where the sun rises or sets.
fn atmosphere(origin: vec3<f32>, direction: vec3<f32>, closest: vec3<f32>) -> vec3<f32> {
    let sun = globals.sun.xyz;
    let sun_strength = globals.sun.w;
    let miss_distance = length(closest);
    let limb_sun = dot(closest / miss_distance, sun);
    let limb_light = mix(1.0, smoothstep(-0.2, 0.4, limb_sun), sun_strength);
    let limb_dusk = exp(-100.0 * limb_sun * limb_sun) * sun_strength;
    let halo = exp(-max(miss_distance - 1.0, 0.0) * glow_thinness(length(origin)));
    let halo_color = vec3<f32>(0.4, 0.65, 1.0);
    // Looking towards the sun, the thin air at the edge lights up with the light it scatters
    // forward.
    let sun_angle = dot(direction, sun);
    let forward_scattering = vec3<f32>(1.0, 0.85, 0.62) * pow(max(sun_angle, 0.0), 24.0) * 2.0 * sun_strength;
    return (halo_color * limb_light + vec3<f32>(1.0, 0.5, 0.2) * limb_dusk * 0.4 + forward_scattering) * halo * 0.8;
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

// The camera's side, up, and the direction from the globe's center to the camera, as columns.
fn camera_axes(towards: vec3<f32>) -> mat3x3<f32> {
    let helper = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(towards.y) > 0.9);
    let side = normalize(cross(helper, towards));
    return mat3x3<f32>(side, cross(towards, side), towards);
}

// The point where a ray from the camera touches the sphere of `radius` around the globe's center,
// `turn` of the way around the circle of these points.
fn touching(radius: f32, turn: f32) -> vec3<f32> {
    let eye = globals.eye.xyz;
    let eye_distance = length(eye);
    let toward_eye = radius * radius / eye_distance;
    let around = radius * sqrt(max(1.0 - toward_eye / eye_distance, 0.0));
    let angle = turn * TAU;
    return camera_axes(eye / eye_distance) * vec3<f32>(around * cos(angle), around * sin(angle), toward_eye);
}

struct GlowOutput {
    @builtin(position) position: vec4<f32>,
    // The glow with straight alpha.
    @location(0) color: vec3<f32>,
    @location(1) alpha: f32,
}

const GLOW_SEGMENTS: u32 = 128u;
const GLOW_RINGS: u32 = 6u;

@fragment
fn fs_glow(in: GlowOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(output(in.color * 1.0), in.alpha * 1.0);
}

struct SunOutput {
    @builtin(position) position: vec4<f32>,
    // The angles from the sun's center.
    @location(0) at: vec2<f32>,
    @location(1) strength: f32,
}

// The angle from the sun's center to the edge of its disc.
const SUN_RADIUS: f32 = 0.012;

// A square around the sun.
@vertex
fn vs_sun(@builtin(vertex_index) index: u32) -> SunOutput {
    let corner = vec2<f32>(f32((0x32u >> index) & 1u), f32((0x2cu >> index) & 1u)) * 2.0 - 1.0;
    let eye = globals.eye.xyz;
    let sun = globals.sun.xyz;
    let axes = camera_axes(sun);
    let direction = sun + (axes[0] * corner.x + axes[1] * corner.y) * SUN_RADIUS;
    var out: SunOutput;
    // Within the far plane, which is one radius beyond the globe's center.
    out.position = globals.view_projection * vec4<f32>(eye + direction * length(eye), 1.0);
    out.at = corner * SUN_RADIUS;
    out.strength = sun_in_view();
    return out;
}

@fragment
fn fs_sun(in: SunOutput) -> @location(0) vec4<f32> {
    let disc = smoothstep(SUN_RADIUS, SUN_RADIUS * 0.65, length(in.at * 1.0));
    return vec4<f32>(output(vec3<f32>(1.0, 0.95, 0.86) * disc * in.strength * 8.0), 0.0);
}

// A patch of the globe shows the imagery of one source: the globe's own, or a tile of a detail
// window.

struct PatchOutput {
    @builtin(position) position: vec4<f32>,
    // Where the patch's source is sampled.
    @location(0) uv: vec2<f32>,
    // Where the globe's own textures are sampled.
    @location(1) map_uv: vec2<f32>,
    // The light on the day's imagery.
    @location(2) daylight: vec3<f32>,
    // x: how much the city lights show, y: how much of the haze, z: how bright the haze is.
    @location(3) shading: vec3<f32>,
    // x: the layer of the detail window, y: how much of the patch shows, z: how much sunlight
    // reaches the ground.
    @location(4) source: vec3<f32>,
    // xy: where the clouds are sampled, zw: where the clouds are that shade the ground.
    @location(5) clouds: vec4<f32>,
    // The patch's part of its source: xy is its top left, zw its bottom right.
    @location(6) bounds: vec4<f32>,
}

// `grid` goes over the patch, which covers `rect` of the map: xy is its top left, zw its
// bottom right.
// `source` is the patch's imagery: xy scales the place on the map to the place in the texture,
// z is the layer of its detail window, and w how much of it shows.
// The light changes slowly over the globe, so it's computed per vertex.
@vertex
fn vs_patch(
    @location(0) grid: vec2<f32>,
    @location(1) rect: vec4<f32>,
    @location(2) source: vec4<f32>,
) -> PatchOutput {
    // The vertices on an edge are the same numbers in both patches that share it, so that no
    // pixel falls between them.
    let map_uv = rect.xy * (1.0 - grid) + rect.zw * grid;
    // The map's right edge is its left edge, at the date line.
    let longitude = (fract(map_uv.x) - 0.5) * TAU;
    let latitude = (0.5 - map_uv.y) * PI;
    let around = vec2<f32>(sin(longitude), cos(longitude));
    let up = vec2<f32>(sin(latitude), cos(latitude));
    let normal = vec3<f32>(up.y * around.x, up.x, up.y * around.y);
    let eye = globals.eye.xyz;
    let sun = globals.sun.xyz;
    let sun_strength = globals.sun.w;
    let sunlight = dot(normal, sun);
    let lambert = clamp((sunlight + 0.06) / 1.06, 0.0, 1.0);
    let darkness = smoothstep(0.08, -0.12, sunlight) * sun_strength;
    let reddening = mix(vec3<f32>(1.0, 0.58, 0.36), vec3<f32>(1.0), smoothstep(0.0, 0.3, sunlight));
    let sun_light = mix(vec3<f32>(1.0), reddening * lambert * 1.2, sun_strength);
    let moonlight = vec3<f32>(0.03, 0.04, 0.075) * darkness;
    let rim = 1.0 - max(dot(normal, normalize(eye - normal)), 0.0);
    let haze = rim * rim * rim * 0.35 * mix(1.0, smoothstep(-0.1, 0.4, sunlight), sun_strength);
    // The clouds drift slowly eastwards. Their shadow on a point comes from the clouds a little
    // towards the sun.
    let cloud_uv = map_uv + vec2<f32>(globals.options.z * 0.0004, 0.0);
    let east = vec3<f32>(around.y, 0.0, -around.x);
    let north = vec3<f32>(-up.x * around.x, up.y, -up.x * around.y);
    let sun_on_map = vec2<f32>(dot(sun, east) / max(up.y, 0.05) / TAU, -dot(sun, north) / PI);

    var out: PatchOutput;
    out.position = globals.view_projection * vec4<f32>(normal, 1.0);
    out.uv = map_uv * source.xy;
    out.map_uv = map_uv;
    out.daylight = sun_light + moonlight;
    out.shading = vec3<f32>(2.4 * darkness, haze, mix(1.0, lambert + 0.15, sun_strength));
    out.source = vec3<f32>(source.zw, lambert);
    out.clouds = vec4<f32>(cloud_uv, cloud_uv + sun_on_map * 0.004);
    out.bounds = rect * source.xyxy;
    return out;
}

// The ground of a patch that shows `imagery` and `city_light`, under `cloud`:
// x is how much cloud is over it, y how much is between it and the sun.
fn patch_ground(in: PatchOutput, imagery: vec3<f32>, city_light: f32, cloud: vec2<f32>) -> vec4<f32> {
    // NXP's Vivante Vulkan driver gets a vertex output wrong that's taken apart before any
    // arithmetic.
    let shading = in.shading * 1.0;
    let source = in.source * 1.0;
    let moonlight = vec3<f32>(0.03, 0.04, 0.075) * shading.x / 2.4;
    var ground = imagery * in.daylight * (1.0 - cloud.y * 0.45 * source.z)
        + vec3<f32>(1.0, 0.62, 0.3) * city_light * shading.x * (1.0 - cloud.x * 0.8);
    // The clouds, white in sunlight, reddened at dusk, and faintly blue under the moon.
    ground = mix(ground, in.daylight * 1.05 - moonlight * 0.35, cloud.x * 0.92);
    ground = mix(ground, vec3<f32>(0.4, 0.62, 1.0) * shading.z, shading.y);
    ground = ground * 1.2 / (1.0 + ground * 0.2);
    return vec4<f32>(output(ground), source.y);
}

// The clouds over a patch, as `patch_ground()` takes them.
fn patch_clouds(in: PatchOutput) -> vec2<f32> {
    let at = in.clouds * 1.0;
    return vec2<f32>(
        textureSample(cloud_cover, map_sampler, at.xy).r,
        textureSample(cloud_cover, map_sampler, at.zw).r
    ) * globals.viewport.w;
}

fn patch_layer(in: PatchOutput) -> i32 {
    return i32((in.source * 1.0).x + 0.5);
}

// Where a patch samples its tile.
// Next to a tile in the texture is whatever tile has the slot beside it, which may be of
// another place, so a sample stays half its footprint, and half a texel, inside the tile.
fn tile_uv(in: PatchOutput) -> vec2<f32> {
    let bounds = in.bounds * 1.0;
    let margin = max(fwidth(in.uv) * 0.5, vec2<f32>(0.5 / 2048.0));
    return clamp(in.uv, bounds.xy + margin, bounds.zw - margin);
}

// A patch with the globe's own imagery.
@fragment
fn fs_patch_map(in: PatchOutput) -> @location(0) vec4<f32> {
    let imagery = textureSample(day_imagery, map_sampler, in.uv).rgb;
    let city_light = textureSample(night_imagery, map_sampler, in.map_uv).r;
    return patch_ground(in, imagery, city_light, vec2<f32>(0.0));
}

@fragment
fn fs_patch_map_clouds(in: PatchOutput) -> @location(0) vec4<f32> {
    let imagery = textureSample(day_imagery, map_sampler, in.uv).rgb;
    let city_light = textureSample(night_imagery, map_sampler, in.map_uv).r;
    return patch_ground(in, imagery, city_light, patch_clouds(in));
}

// A patch with a tile of the day's imagery.
@fragment
fn fs_patch_tile(in: PatchOutput) -> @location(0) vec4<f32> {
    let imagery = textureSample(tiles, tile_sampler, tile_uv(in), patch_layer(in)).rgb;
    let city_light = textureSample(night_imagery, map_sampler, in.map_uv).r;
    return patch_ground(in, imagery, city_light, vec2<f32>(0.0));
}

@fragment
fn fs_patch_tile_clouds(in: PatchOutput) -> @location(0) vec4<f32> {
    let imagery = textureSample(tiles, tile_sampler, tile_uv(in), patch_layer(in)).rgb;
    let city_light = textureSample(night_imagery, map_sampler, in.map_uv).r;
    return patch_ground(in, imagery, city_light, patch_clouds(in));
}

// A patch in the dark with a tile of the night's lights.
@fragment
fn fs_patch_night(in: PatchOutput) -> @location(0) vec4<f32> {
    let imagery = textureSample(day_imagery, map_sampler, in.map_uv).rgb;
    let city_light = textureSample(tiles, tile_sampler, tile_uv(in), patch_layer(in)).r;
    return patch_ground(in, imagery, city_light, vec2<f32>(0.0));
}

@fragment
fn fs_patch_night_clouds(in: PatchOutput) -> @location(0) vec4<f32> {
    let imagery = textureSample(day_imagery, map_sampler, in.map_uv).rgb;
    let city_light = textureSample(tiles, tile_sampler, tile_uv(in), patch_layer(in)).r;
    return patch_ground(in, imagery, city_light, patch_clouds(in));
}

struct RouteOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    // x: how many pixels from the middle of the line, y: half the line's width.
    @location(1) across: vec2<f32>,
}

// Each segment of the flight's route is an instance of a quad that's a constant width on the
// screen. `start` and `end` are its ends on the ground, with the line's width in pixels in
// `start.w`, and `color` has straight alpha.
@vertex
fn vs_route(
    @builtin(vertex_index) index: u32,
    @location(0) start: vec4<f32>,
    @location(1) end: vec4<f32>,
    @location(2) color: vec4<f32>,
) -> RouteOutput {
    let along = f32((0x32u >> index) & 1u);
    let side = f32((0x2cu >> index) & 1u) * 2.0 - 1.0;
    let eye = globals.eye.xyz;
    // Slightly above the ground.
    let lifted_start = start.xyz * 1.0005;
    let lifted_end = end.xyz * 1.0005;
    let clip_start = globals.view_projection * vec4<f32>(lifted_start, 1.0);
    let clip_end = globals.view_projection * vec4<f32>(lifted_end, 1.0);
    var out: RouteOutput;
    out.color = color;
    // A pixel more than the line, for its smooth edges.
    let reach = start.w * 0.5 + 1.0;
    out.across = vec2<f32>(side * reach, start.w * 0.5);
    // With an end behind the camera or beyond the horizon, it collapses to nothing.
    if min(clip_start.w, clip_end.w) <= 1e-4 || min(dot(lifted_start, eye), dot(lifted_end, eye)) < 1.0 {
        out.position = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        return out;
    }
    let viewport = globals.viewport.xy;
    let on_screen = (clip_end.xy / clip_end.w - clip_start.xy / clip_start.w) * viewport;
    let direction = on_screen / max(length(on_screen), 1e-4);
    let offset = vec2<f32>(-direction.y, direction.x) * side * reach * 2.0 / viewport;
    let clip = mix(clip_start, clip_end, along);
    out.position = vec4<f32>(clip.xy + offset * clip.w, 0.0, clip.w);
    return out;
}

@fragment
fn fs_route(in: RouteOutput) -> @location(0) vec4<f32> {
    let across = in.across * 1.0;
    let edge = clamp(across.y + 0.5 - abs(across.x), 0.0, 1.0);
    let color = in.color * 1.0;
    return vec4<f32>(output(color.rgb), color.a * edge);
}

struct StarOutput {
    @builtin(position) position: vec4<f32>,
    // From the star's middle, 1 at its edge.
    @location(0) at: vec2<f32>,
    @location(1) color: vec3<f32>,
}

// A triangle around each star. `direction` is where it is, and `look` its brightness, its tint
// from bluish to yellowish, and what tells its size and its twinkling from the others'.
@vertex
fn vs_star(
    @builtin(vertex_index) index: u32,
    @location(0) direction: vec3<f32>,
    @location(1) look: vec3<f32>,
) -> StarOutput {
    // The triangle around the circle of radius 1.
    let corner = vec2<f32>(f32(index) - 1.0, select(-1.0, 2.0, index == 1u)) * vec2<f32>(1.7320508, 1.0);
    let axes = camera_axes(direction);
    let radius = (0.1 + 0.14 * look.z) / 260.0;
    let twinkle = 0.8 + 0.2 * sin(globals.options.z * (1.0 + 3.0 * look.y) + look.z * TAU);
    let tint = mix(vec3<f32>(0.75, 0.82, 1.0), vec3<f32>(1.0, 0.9, 0.75), look.y);
    var out: StarOutput;
    out.position = globals.view_projection
        * vec4<f32>(globals.eye.xyz + (direction + (axes[0] * corner.x + axes[1] * corner.y) * radius) * length(globals.eye.xyz), 1.0);
    out.at = corner;
    out.color = tint * look.x * twinkle * globals.options.x * 0.85;
    return out;
}

@fragment
fn fs_star(in: StarOutput) -> @location(0) vec4<f32> {
    let fade = smoothstep(1.0, 0.0, length(in.at * 1.0));
    return vec4<f32>(output(in.color * fade), 0.0);
}

struct GlareOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
}

const GLARE_SEGMENTS: u32 = 32u;
const GLARE_RINGS: u32 = 32u;

// The sun's glare, as rings around the sun, each about a tenth further from it than the one
// before, so that they're close together where the glare is sharp.
@vertex
fn vs_glare(@builtin(vertex_index) index: u32) -> GlareOutput {
    let quad = index / 6u;
    let corner = index % 6u;
    let turn = f32(quad % GLARE_SEGMENTS + ((0x32u >> corner) & 1u)) / f32(GLARE_SEGMENTS) * TAU;
    let ring = f32(quad / GLARE_SEGMENTS + ((0x2cu >> corner) & 1u)) / f32(GLARE_RINGS);
    let angle = 0.45 * (exp(5.0 * ring) - 1.0) / (exp(5.0) - 1.0);
    let sun = globals.sun.xyz;
    let axes = camera_axes(sun);
    let direction = sun * cos(angle) + (axes[0] * cos(turn) + axes[1] * sin(turn)) * sin(angle);
    let towards_sun = cos(angle);
    let glare = pow(towards_sun, 3000.0) * 2.5 + pow(towards_sun, 250.0) * 0.4 + pow(towards_sun, 40.0) * 0.05;
    var out: GlareOutput;
    out.position = globals.view_projection * vec4<f32>(globals.eye.xyz + direction * length(globals.eye.xyz), 1.0);
    // The outer ring fades out.
    let strength = sun_in_view();
    out.color = vec3<f32>(1.0, 0.95, 0.86) * glare * strength * select(1.0, 0.0, ring >= 1.0);
    // Without the sun in view, it collapses to nothing.
    if strength <= 0.0 {
        out.position = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    return out;
}

@fragment
fn fs_glare(in: GlareOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(output(in.color * 1.0), 0.0);
}

// The atmosphere's glow, as rings of `GLOW_SEGMENTS` quads from the globe's outline out, closer
// together towards it, after one ring that reaches under the outline.
// The patches are drawn over it, so that no pixel at the outline gets both or neither.
@vertex
fn vs_glow(@builtin(vertex_index) index: u32) -> GlowOutput {
    let quad = index / 6u;
    let corner = index % 6u;
    let turn = f32(quad % GLOW_SEGMENTS + ((0x32u >> corner) & 1u)) / f32(GLOW_SEGMENTS);
    let row = quad / GLOW_SEGMENTS + ((0x2cu >> corner) & 1u);
    let ring = f32(max(row, 1u) - 1u) / f32(GLOW_RINGS);
    let eye = globals.eye.xyz;
    let eye_distance = length(eye);
    let reach = min(6.0 / glow_thinness(eye_distance), 0.9 * (eye_distance - 1.0));
    let outline = touching(1.0 + reach * ring * ring, turn);
    let point = select(outline, touching(0.985, turn), row == 0u);
    let glow = atmosphere(eye, normalize(outline - eye), outline) * select(1.0, 0.0, ring >= 1.0);
    let alpha = mix(clamp(max(glow.r, max(glow.g, glow.b)), 0.0, 1.0), 1.0, globals.options.x);
    var out: GlowOutput;
    out.position = globals.view_projection * vec4<f32>(point, 1.0);
    out.color = glow / max(alpha, 1e-4);
    out.alpha = alpha;
    return out;
}

struct LimbOutput {
    @builtin(position) position: vec4<f32>,
    // The atmosphere's glow at the globe's outline, with straight alpha.
    @location(0) color: vec3<f32>,
    // x: how much of the glow shows rather than the ground, y: the glow's alpha.
    @location(1) cover: vec2<f32>,
}

const LIMB_SEGMENTS: u32 = 128u;
// How many pixels the ground takes to fade into the glow at the globe's outline.
const LIMB_WIDTH: f32 = 1.5;

// A strip along the inside of the globe's outline, over which the ground fades into the glow
// that surrounds it, so that the outline is smooth.
// A second strip around the outline is all glow: the patches' outline has other corners than
// the strip's, and reaches a little beyond it between them.
@vertex
fn vs_limb(@builtin(vertex_index) index: u32) -> LimbOutput {
    let quad = index / 6u;
    let corner = index % 6u;
    let turn = f32(quad % LIMB_SEGMENTS + ((0x32u >> corner) & 1u)) / f32(LIMB_SEGMENTS);
    let row = quad / LIMB_SEGMENTS + ((0x2cu >> corner) & 1u);
    // How many pixels inside the outline, and how much of the glow shows there.
    let inside = select(select(-0.75, 0.0, row == 1u), LIMB_WIDTH, row == 0u);
    let cover = select(1.0, 0.0, row == 0u);
    let eye = globals.eye.xyz;
    let eye_distance = length(eye);
    let point = touching(1.0, turn);
    let clip = globals.view_projection * vec4<f32>(point, 1.0);
    // The outline is a circle around this point.
    let middle = globals.view_projection * vec4<f32>(eye / (eye_distance * eye_distance), 1.0);
    let inwards = (middle.xy / middle.w - clip.xy / clip.w) * globals.viewport.xy;
    let step = inwards / max(length(inwards), 1e-4) * inside * 2.0 / globals.viewport.xy;
    let glow = atmosphere(eye, normalize(point - eye), point);
    let alpha = mix(clamp(max(glow.r, max(glow.g, glow.b)), 0.0, 1.0), 1.0, globals.options.x);
    var out: LimbOutput;
    out.position = vec4<f32>(clip.xy + step * clip.w, clip.zw);
    out.color = glow / max(alpha, 1e-4);
    out.cover = vec2<f32>(cover, alpha);
    return out;
}

// The color, which fades from the ground's to the glow's.
@fragment
fn fs_limb_color(in: LimbOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(output(in.color * 1.0), (in.cover * 1.0).x);
}

// The alpha, which fades from 1 to the glow's.
@fragment
fn fs_limb_alpha(in: LimbOutput) -> @location(0) vec4<f32> {
    let cover = in.cover * 1.0;
    return vec4<f32>(0.0, 0.0, 0.0, cover.x * (1.0 - cover.y));
}

struct GlintOutput {
    @builtin(position) position: vec4<f32>,
    // Where the map is sampled.
    @location(0) uv: vec2<f32>,
    // The glint on water.
    @location(1) glint: f32,
}

const GLINT_SEGMENTS: u32 = 24u;
const GLINT_RINGS: u32 = 8u;

// The sun's glint on the water, on a disc of the globe around where it's brightest.
// It changes slowly over the disc, so it's computed per vertex, and a pixel only looks up
// whether it's water.
@vertex
fn vs_glint(@builtin(vertex_index) index: u32) -> GlintOutput {
    let quad = index / 6u;
    let corner = index % 6u;
    let turn = f32(quad % GLINT_SEGMENTS + ((0x32u >> corner) & 1u)) / f32(GLINT_SEGMENTS) * TAU;
    let ring = f32(quad / GLINT_SEGMENTS + ((0x2cu >> corner) & 1u)) / f32(GLINT_RINGS);
    let eye = globals.eye.xyz;
    let sun = globals.sun.xyz;
    // The glint is brightest where the ground faces half way between the sun and the camera.
    var middle = normalize(sun + normalize(eye));
    middle = normalize(sun + normalize(eye - middle));
    middle = normalize(sun + normalize(eye - middle));
    let axes = camera_axes(middle);
    let angle = 0.16 * ring;
    let normal = middle * cos(angle) + (axes[0] * cos(turn) + axes[1] * sin(turn)) * sin(angle);
    let view = normalize(eye - normal);
    let lambert = clamp((dot(normal, sun) + 0.06) / 1.06, 0.0, 1.0);
    // The longitude is taken from the middle's, so that it doesn't jump at the date line.
    let middle_u = atan2(middle.x, middle.z) / TAU;
    let u = atan2(normal.x, normal.z) / TAU - middle_u;
    var out: GlintOutput;
    out.uv = vec2<f32>(middle_u + u - round(u) + 0.5, 0.5 - asin(clamp(normal.y, -1.0, 1.0)) / PI);
    // None beyond the horizon, where the disc is in front of the ground that faces the camera.
    out.glint = pow(max(dot(normal, normalize(sun + view)), 0.0), 500.0) * 0.15 * lambert
        * select(1.0, 0.0, dot(normal, eye) < 1.0);
    out.position = globals.view_projection * vec4<f32>(normal, 1.0);
    return out;
}

@fragment
fn fs_glint(in: GlintOutput) -> @location(0) vec4<f32> {
    // The land, a pixel wide at its edge however far the map is magnified.
    let coast_distance = textureSample(map, map_sampler, in.uv).r - 0.5;
    let land = clamp(coast_distance / max(fwidth(coast_distance), 1e-4) + 0.5, 0.0, 1.0);
    return vec4<f32>(output(vec3<f32>(1.0, 0.92, 0.8) * (in.glint * (1.0 - land))), 0.0);
}
