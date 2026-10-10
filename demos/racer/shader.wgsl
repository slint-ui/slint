// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore Blinn metalness Phong smoothstep texel untextured

// The race track's shaders, see `renderer.rs`.
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
    // rgb: the sky at the horizon, which the haze takes; w: the haze's density per meter.
    horizon: vec4<f32>,
    // rgb: the sky overhead; w: 1 while the sun's shadows come from `shadow_map`.
    zenith: vec4<f32>,
    // From the world to the shadow map's clip space.
    shadow: mat4x4<f32>,
    spots: array<Spot, 5>,
}

struct Object {
    model: mat4x4<f32>,
    model_view: mat4x4<f32>,
    // rgb: linear color; a: opacity.
    color: vec4<f32>,
    // xy: scale; zw: offset.
    uv_transform: vec4<f32>,
    // x: roughness; y: metalness; z: clear coat; w: how much the haze covers the surface,
    // negative for additive glows, which fade instead.
    params: vec4<f32>,
}

@group(0) @binding(0) var<uniform> globals: Globals;
// The depth of the shapes seen from the sun, in high quality.
@group(0) @binding(1) var shadow_map: texture_depth_2d;
@group(0) @binding(2) var shadow_sampler: sampler_comparison;
@group(1) @binding(0) var<uniform> object: Object;
@group(2) @binding(0) var color_texture: texture_2d<f32>;
@group(2) @binding(1) var color_sampler: sampler;

// Medium quality draws with the high quality shaders, without the bloom. So they tone-map
// themselves, and take the sun's shadows from fewer samples.
override medium: bool = false;

// Low quality lights each vertex, which is exact for the flat surfaces and the
// sun and hemisphere lights.
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

// Even a 1x1 white texture costs Vivante GPUs a sample, so surfaces without a texture skip it.
@fragment
fn fs_low_untextured(in: LowVarying) -> @location(0) vec4<f32> {
    return vec4<f32>(object.color.rgb * in.color, object.color.a * in.alpha);
}

// A light map, such as the ground's with its pools of light: its color adds light,
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

// Draws the depth of the shapes seen from the sun, for the shadows in high quality.
@vertex
fn vs_shadow(
    @location(0) p: vec3<f32>,
    @location(4) i0: vec4<f32>,
    @location(5) i1: vec4<f32>,
    @location(6) i2: vec4<f32>,
    @location(7) i3: vec4<f32>,
) -> @builtin(position) vec4<f32> {
    let q = i0.xyz * p.x + i1.xyz * p.y + i2.xyz * p.z + i3.xyz;
    let model = object.model;
    let world = model[0].xyz * q.x + model[1].xyz * q.y + model[2].xyz * q.z + model[3].xyz;
    return globals.shadow * vec4<f32>(world, 1.0);
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

    // The distance haze, as in `fs_lit_high`.
    let to_point = in.world - globals.camera_position.xyz;
    let haze = 1.0 - exp(-max(length(to_point) - 25.0, 0.0) * globals.horizon.w);
    let glow = pow(max(dot(normalize(to_point), globals.sun_direction.xyz), 0.0), 8.0);
    let haze_color = globals.horizon.rgb + globals.sun_color.rgb * glow * 0.08;
    let amount = object.params.w;
    color = select(mix(color, haze_color, haze * amount), color * (1.0 - haze), amount < 0.0);

    if medium {
        // As in `fs_composite` in `post.wgsl`.
        let low = min(color.r, min(color.g, color.b));
        color -= select(0.04, low - 6.25 * low * low, low < 0.08);
        let peak = max(color.r, max(color.g, color.b));
        if peak >= 0.76 {
            let new_peak = 1.0 - 0.0576 / (peak - 0.52);
            color *= new_peak / peak;
            color = mix(color, vec3<f32>(new_peak), 1.0 - 1.0 / (0.15 * (peak - new_peak) + 1.0));
        }
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
    // The sky in the direction of the reflection, from the horizon up, and the ground below.
    let reflected = reflect(-view, normal);
    let sky_reflection = select(
        globals.ground.rgb * 0.6,
        mix(globals.horizon.rgb, globals.zenith.rgb, smoothstep(0.0, 0.5, reflected.y)),
        reflected.y > 0.0,
    );
    let n_dot_v = max(dot(normal, view), 0.0);
    // Schlick's Fresnel term: surfaces reflect more at grazing angles.
    let grazing = pow(1.0 - n_dot_v, 5.0);
    let fresnel = specular + (vec3<f32>(1.0) - specular) * grazing;
    let gloss = (1.0 - roughness) * (1.0 - roughness);
    var color = hemisphere * diffuse + sky_reflection * fresnel * gloss;

    // How much of the sun reaches the surface: the shadow map's depths compared around the
    // surface's place in it, nudged out along the normal so surfaces don't shadow themselves.
    var sunlit = 1.0;
    if globals.zenith.w > 0.5 {
        let in_shadow_map = globals.shadow * vec4<f32>(in.world + normal * 0.1, 1.0);
        let shadow_uv = vec2<f32>(in_shadow_map.x * 0.5 + 0.5, 0.5 - in_shadow_map.y * 0.5);
        let texel = 1.0 / f32(textureDimensions(shadow_map).x);
        // Medium quality takes one sample, which the sampler blends from four texels.
        let reach = select(1, 0, medium);
        var lit = 0.0;
        for (var dx = -reach; dx <= reach; dx++) {
            for (var dy = -reach; dy <= reach; dy++) {
                let offset = vec2<f32>(f32(dx), f32(dy)) * texel * 1.5;
                lit += textureSampleCompareLevel(
                    shadow_map,
                    shadow_sampler,
                    shadow_uv + offset,
                    in_shadow_map.z,
                );
            }
        }
        let inside = all(shadow_uv > vec2<f32>(0.0)) && all(shadow_uv < vec2<f32>(1.0))
            && in_shadow_map.z < 1.0;
        let samples = f32((2 * reach + 1) * (2 * reach + 1));
        sunlit = select(1.0, lit / samples, inside);
    }
    let sun_light = max(dot(normal, globals.sun_direction.xyz), 0.0) * sunlit;
    let sun_half = normalize(globals.sun_direction.xyz + view);
    let sun_shine = normalization * pow(max(dot(normal, sun_half), 0.0), shininess);
    color += globals.sun_color.rgb * sun_light * (diffuse + specular * sun_shine);

    // A clear coat reflects the sky and the lights sharply over the paint.
    let coat = object.params.z;
    let coat_fresnel = 0.04 + 0.96 * grazing;
    var coat_light = sky_reflection * coat_fresnel;
    let coat_sun = pow(max(dot(normal, sun_half), 0.0), 900.0) * 113.0;
    coat_light += globals.sun_color.rgb * sun_light * coat_sun * coat_fresnel;

    for (var i = 0; i < 5; i++) {
        let spot = globals.spots[i];
        let to_light = spot.position.xyz - in.world;
        let distance = length(to_light);
        let light = to_light / distance;
        let cone = smoothstep(spot.direction.w, spot.color.w, dot(light, spot.direction.xyz));
        let range = clamp(1.0 - pow(distance / spot.position.w, 4.0), 0.0, 1.0);
        let falloff = range * range / max(distance, 0.01);
        let incoming = spot.color.rgb * max(dot(normal, light), 0.0) * cone * falloff;
        let n_dot_h = max(dot(normal, normalize(light + view)), 0.0);
        let shine = normalization * pow(n_dot_h, shininess);
        color += incoming * (diffuse + specular * shine);
        coat_light += incoming * pow(n_dot_h, 900.0) * 113.0 * coat_fresnel;
    }
    color = color * (1.0 - coat * coat_fresnel) + coat_light * coat;

    let light = textureSample(light_texture, light_sampler, in.light_uv);
    color = color * light.a + light.rgb;

    // The distance haze, brighter towards the sun, which lights it. It starts a little
    // away from the camera, and is fainter in the shade, such as in a tunnel.
    let to_point = in.world - globals.camera_position.xyz;
    let haze = 1.0 - exp(-max(length(to_point) - 25.0, 0.0) * globals.horizon.w);
    let glow = pow(max(dot(normalize(to_point), globals.sun_direction.xyz), 0.0), 8.0);
    let haze_color = globals.horizon.rgb + globals.sun_color.rgb * glow * 0.08;
    color = mix(color, haze_color * (0.3 + 0.7 * sunlit), haze * object.params.w);

    if medium {
        // As in `fs_composite` in `post.wgsl`.
        let low = min(color.r, min(color.g, color.b));
        color -= select(0.04, low - 6.25 * low * low, low < 0.08);
        let peak = max(color.r, max(color.g, color.b));
        if peak >= 0.76 {
            let new_peak = 1.0 - 0.0576 / (peak - 0.52);
            color *= new_peak / peak;
            color = mix(color, vec3<f32>(new_peak), 1.0 - 1.0 / (0.15 * (peak - new_peak) + 1.0));
        }
    }

    return vec4<f32>(color, object.color.a * texel.a * in.alpha);
}

