// Height fog (height_fog.rs): the picture's surfaces fade into the fog colour by the fog
// along the ray from the eye, whose density falls off exponentially with height; towards
// the sun the fog takes on some of the sunlight. Never on the sky. Engineering choices,
// written for this engine.

import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;

struct HeightFog {
    world_from_clip: mat4x4<f32>,
    // xyz: the eye, w: near plane.
    eye: vec4<f32>,
    // rgb: fog colour (linear), w: density at the reference height.
    color: vec4<f32>,
    // x: 1 / e-folding height, y: reference height, z: deepest below it, w: sky depth from.
    shape: vec4<f32>,
    // xyz: towards the sun, w: scatter strength.
    sun: vec4<f32>,
    sun_color: vec4<f32>,
}

@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var picture_sampler: sampler;
@group(0) @binding(2) var<uniform> fog: HeightFog;
@if(MULTISAMPLED) @group(0) @binding(3) var depth: texture_depth_multisampled_2d;
@if(!MULTISAMPLED) @group(0) @binding(3) var depth: texture_depth_2d;

// The optical depth of the fog from the eye to `p`.
fn optical_depth(p: vec3<f32>) -> f32 {
    let b = fog.shape.x;
    let h0 = fog.shape.y;
    let eye = fog.eye.xyz;
    let dist = length(p - eye);
    // Heights above the reference, held so a deep drop below it stays finite.
    let he = max(eye.y - h0, -fog.shape.z);
    let hp = max(p.y - h0, -fog.shape.z);
    let dh = hp - he;
    // The mean of exp(-b h) over the ray, in closed form; a level ray's limit is exp(-b he).
    var mean = exp(-b * he);
    if abs(b * dh) > 1e-4 {
        mean = (exp(-b * he) - exp(-b * hp)) / (b * dh);
    }
    return fog.color.w * dist * mean;
}

@fragment
fn composite(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let src = textureLoad(picture, vec2<i32>(floor(in.position.xy)), 0);
    let dims = vec2<i32>(textureDimensions(depth));
    let q = clamp(vec2<i32>(in.uv * vec2<f32>(dims)), vec2(0), dims - 1);
    let d = textureLoad(depth, q, 0);
    // The sky (nothing drawn, or the sky dome) keeps its own colour.
    if d <= 0.0 || fog.eye.w / d > fog.shape.w {
        return src;
    }
    let w = fog.world_from_clip * vec4(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0, d, 1.0);
    let p = w.xyz / w.w;
    let t = exp(-optical_depth(p));
    let ray = normalize(p - fog.eye.xyz);
    // A soft lobe around the sun.
    let towards = pow(max(dot(ray, fog.sun.xyz), 0.0), 8.0);
    let haze = fog.color.rgb + fog.sun_color.rgb * fog.sun.w * towards;
    return vec4(src.rgb * t + haze * (1.0 - t), src.a);
}
