// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore Blinn Khronos metalness Phong smoothstep texel

// The racing hall's shaders, see `renderer.rs`.
//
// NXP's Vivante Vulkan driver miscompiles vectors and structs built from function
// parameters or vertex inputs, such as `vec4<f32>(position, 1.0)`. So there are no helper
// functions, the vertex inputs are separate parameters, and the vertex shaders multiply
// the matrices column by column.
//
// Every draw is instanced: `i0` to `i3` are the columns of an instance's transform, and
// `tint` multiplies its color and opacity, see `Instance` in `renderer.rs`.

struct Spot {
    // xyz: position; w: the distance where the light ends.
    position: vec4<f32>,
    // xyz: from the target towards the light; w: cosine of the cone angle.
    direction: vec4<f32>,
    // rgb: color times intensity over pi; w: cosine of the angle where the penumbra starts.
    color: vec4<f32>,
}

struct Globals {
    // The perspective projection's x and y scale, and its z scale and offset.
    projection: vec4<f32>,
    camera_position: vec4<f32>,
    // Hemisphere light colors times intensity over pi.
    sky: vec4<f32>,
    ground: vec4<f32>,
    sun_direction: vec4<f32>,
    sun_color: vec4<f32>,
    spots: array<Spot, 5>,
}

struct Object {
    model: mat4x4<f32>,
    model_view: mat4x4<f32>,
    // rgb: linear color; a: opacity.
    color: vec4<f32>,
    // xy: scale; zw: offset.
    uv_transform: vec4<f32>,
    // x: roughness; y: metalness.
    params: vec4<f32>,
}

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var<uniform> object: Object;
@group(2) @binding(0) var color_texture: texture_2d<f32>;
@group(2) @binding(1) var color_sampler: sampler;

// Low quality lights each vertex, which is exact for the flat surfaces and the
// directional and hemisphere lights of the hall.
struct LowVarying {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // The vertex color, times the light for lit surfaces.
    @location(1) color: vec3<f32>,
    @location(2) alpha: f32,
}

@vertex
fn vs_unlit_low(
    @location(0) p: vec3<f32>,
    @location(1) n: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec3<f32>,
    @location(4) i0: vec4<f32>,
    @location(5) i1: vec4<f32>,
    @location(6) i2: vec4<f32>,
    @location(7) i3: vec4<f32>,
    @location(8) tint: vec4<f32>,
) -> LowVarying {
    let q = i0.xyz * p.x + i1.xyz * p.y + i2.xyz * p.z + i3.xyz;
    let m = object.model_view;
    let view = m[0].xyz * q.x + m[1].xyz * q.y + m[2].xyz * q.z + m[3].xyz;
    let projection = globals.projection;
    var out: LowVarying;
    out.clip = vec4<f32>(view.xy * projection.xy, view.z * projection.z + projection.w, -view.z);
    out.uv = uv * object.uv_transform.xy + object.uv_transform.zw;
    out.color = color * tint.rgb;
    out.alpha = tint.a;
    return out;
}

@vertex
fn vs_lit_low(
    @location(0) p: vec3<f32>,
    @location(1) n: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec3<f32>,
    @location(4) i0: vec4<f32>,
    @location(5) i1: vec4<f32>,
    @location(6) i2: vec4<f32>,
    @location(7) i3: vec4<f32>,
    @location(8) tint: vec4<f32>,
) -> LowVarying {
    let q = i0.xyz * p.x + i1.xyz * p.y + i2.xyz * p.z + i3.xyz;
    let m = object.model_view;
    let view = m[0].xyz * q.x + m[1].xyz * q.y + m[2].xyz * q.z + m[3].xyz;
    let projection = globals.projection;
    let model = object.model;
    let r = i0.xyz * n.x + i1.xyz * n.y + i2.xyz * n.z;
    let normal = normalize(model[0].xyz * r.x + model[1].xyz * r.y + model[2].xyz * r.z);
    let hemisphere = mix(globals.ground.rgb, globals.sky.rgb, normal.y * 0.5 + 0.5);
    let sun = globals.sun_color.rgb * max(dot(normal, globals.sun_direction.xyz), 0.0);
    var out: LowVarying;
    out.clip = vec4<f32>(view.xy * projection.xy, view.z * projection.z + projection.w, -view.z);
    out.uv = uv * object.uv_transform.xy + object.uv_transform.zw;
    out.color = color * tint.rgb * (hemisphere + sun);
    out.alpha = tint.a;
    return out;
}

@fragment
fn fs_low(in: LowVarying) -> @location(0) vec4<f32> {
    let texel = textureSample(color_texture, color_sampler, in.uv);
    let color = object.color.rgb * texel.rgb * in.color;
    return vec4<f32>(color, object.color.a * texel.a * in.alpha);
}

// A light map, such as the floor's with its pools of light and shade: its color adds light,
// and its alpha multiplies the surface's own. Low quality samples it only on surfaces that
// have one, high quality on all lit surfaces.
@group(3) @binding(0) var light_texture: texture_2d<f32>;
@group(3) @binding(1) var light_sampler: sampler;

struct LightVarying {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) light_uv: vec2<f32>,
    @location(3) alpha: f32,
}

@vertex
fn vs_lit_low_light(
    @location(0) p: vec3<f32>,
    @location(1) n: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec3<f32>,
    @location(4) i0: vec4<f32>,
    @location(5) i1: vec4<f32>,
    @location(6) i2: vec4<f32>,
    @location(7) i3: vec4<f32>,
    @location(8) tint: vec4<f32>,
) -> LightVarying {
    let q = i0.xyz * p.x + i1.xyz * p.y + i2.xyz * p.z + i3.xyz;
    let m = object.model_view;
    let view = m[0].xyz * q.x + m[1].xyz * q.y + m[2].xyz * q.z + m[3].xyz;
    let projection = globals.projection;
    let model = object.model;
    let r = i0.xyz * n.x + i1.xyz * n.y + i2.xyz * n.z;
    let normal = normalize(model[0].xyz * r.x + model[1].xyz * r.y + model[2].xyz * r.z);
    let hemisphere = mix(globals.ground.rgb, globals.sky.rgb, normal.y * 0.5 + 0.5);
    let sun = globals.sun_color.rgb * max(dot(normal, globals.sun_direction.xyz), 0.0);
    var out: LightVarying;
    out.clip = vec4<f32>(view.xy * projection.xy, view.z * projection.z + projection.w, -view.z);
    out.uv = uv * object.uv_transform.xy + object.uv_transform.zw;
    out.color = color * tint.rgb * (hemisphere + sun);
    out.alpha = tint.a;
    out.light_uv = uv;
    return out;
}

@fragment
fn fs_low_light(in: LightVarying) -> @location(0) vec4<f32> {
    // The light map stands in for the surface's texture, see `World::apply_quality`.
    let color = object.color.rgb * in.color;
    let light = textureSample(light_texture, light_sampler, in.light_uv);
    return vec4<f32>(color * light.a + light.rgb, object.color.a * in.alpha);
}

struct HighVarying {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) world: vec3<f32>,
    @location(3) normal: vec3<f32>,
    @location(4) alpha: f32,
    @location(5) light_uv: vec2<f32>,
}

@vertex
fn vs_high(
    @location(0) p: vec3<f32>,
    @location(1) n: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec3<f32>,
    @location(4) i0: vec4<f32>,
    @location(5) i1: vec4<f32>,
    @location(6) i2: vec4<f32>,
    @location(7) i3: vec4<f32>,
    @location(8) tint: vec4<f32>,
) -> HighVarying {
    let q = i0.xyz * p.x + i1.xyz * p.y + i2.xyz * p.z + i3.xyz;
    let model = object.model;
    let world = model[0].xyz * q.x + model[1].xyz * q.y + model[2].xyz * q.z + model[3].xyz;
    let m = object.model_view;
    let view = m[0].xyz * q.x + m[1].xyz * q.y + m[2].xyz * q.z + m[3].xyz;
    let projection = globals.projection;
    var out: HighVarying;
    out.clip = vec4<f32>(view.xy * projection.xy, view.z * projection.z + projection.w, -view.z);
    out.uv = uv * object.uv_transform.xy + object.uv_transform.zw;
    out.color = color * tint.rgb;
    out.alpha = tint.a;
    out.world = world;
    out.light_uv = uv;
    let r = i0.xyz * n.x + i1.xyz * n.y + i2.xyz * n.z;
    out.normal = model[0].xyz * r.x + model[1].xyz * r.y + model[2].xyz * r.z;
    return out;
}

@fragment
fn fs_unlit_high(in: HighVarying) -> @location(0) vec4<f32> {
    let texel = textureSample(color_texture, color_sampler, in.uv);
    var color = object.color.rgb * texel.rgb * in.color;

    // Khronos PBR Neutral tone mapping, as three.js's `NeutralToneMapping`.
    let low = min(color.r, min(color.g, color.b));
    color -= select(0.04, low - 6.25 * low * low, low < 0.08);
    let peak = max(color.r, max(color.g, color.b));
    if peak >= 0.76 {
        let new_peak = 1.0 - 0.0576 / (peak - 0.52);
        color *= new_peak / peak;
        color = mix(color, vec3<f32>(new_peak), 1.0 - 1.0 / (0.15 * (peak - new_peak) + 1.0));
    }
    return vec4<f32>(color, object.color.a * texel.a * in.alpha);
}

@fragment
fn fs_lit_high(in: HighVarying) -> @location(0) vec4<f32> {
    let texel = textureSample(color_texture, color_sampler, in.uv);
    let albedo = object.color.rgb * texel.rgb * in.color;
    let roughness = object.params.x;
    let metalness = object.params.y;
    let normal = normalize(in.normal);
    let to_camera = globals.camera_position.xyz - in.world;
    let view = normalize(to_camera);
    let diffuse = albedo * (1.0 - metalness);
    let specular = mix(vec3<f32>(0.04), albedo, metalness);
    // Normalized Blinn-Phong in place of the GGX distribution of three.js's standard material.
    let alpha = max(roughness * roughness, 0.02);
    let shininess = 2.0 / (alpha * alpha) - 2.0;
    let normalization = (shininess + 8.0) / 8.0;

    let hemisphere = mix(globals.ground.rgb, globals.sky.rgb, normal.y * 0.5 + 0.5);
    // The hall's reflections, roughly: smooth and metal surfaces pick up the sky light.
    var color = hemisphere * (diffuse + specular * (1.0 - roughness) * 2.0);

    let sun_light = max(dot(normal, globals.sun_direction.xyz), 0.0);
    let sun_half = normalize(globals.sun_direction.xyz + view);
    let sun_shine = normalization * pow(max(dot(normal, sun_half), 0.0), shininess);
    color += globals.sun_color.rgb * sun_light * (diffuse + specular * sun_shine);

    for (var i = 0; i < 5; i++) {
        let spot = globals.spots[i];
        let to_light = spot.position.xyz - in.world;
        let distance = length(to_light);
        let light = to_light / distance;
        let cone = smoothstep(spot.direction.w, spot.color.w, dot(light, spot.direction.xyz));
        let range = clamp(1.0 - pow(distance / spot.position.w, 4.0), 0.0, 1.0);
        let falloff = range * range / max(distance, 0.01);
        let incoming = max(dot(normal, light), 0.0) * cone * falloff;
        let half_vector = normalize(light + view);
        let shine = normalization * pow(max(dot(normal, half_vector), 0.0), shininess);
        color += spot.color.rgb * incoming * (diffuse + specular * shine);
    }

    let light = textureSample(light_texture, light_sampler, in.light_uv);
    color = color * light.a + light.rgb;

    // Khronos PBR Neutral tone mapping, as in `fs_unlit_high`.
    let low = min(color.r, min(color.g, color.b));
    color -= select(0.04, low - 6.25 * low * low, low < 0.08);
    let peak = max(color.r, max(color.g, color.b));
    if peak >= 0.76 {
        let new_peak = 1.0 - 0.0576 / (peak - 0.52);
        color *= new_peak / peak;
        color = mix(color, vec3<f32>(new_peak), 1.0 - 1.0 / (0.15 * (peak - new_peak) + 1.0));
    }
    return vec4<f32>(color, object.color.a * texel.a * in.alpha);
}

