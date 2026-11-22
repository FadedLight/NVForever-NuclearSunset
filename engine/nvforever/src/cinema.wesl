// The engine's Cinema pass (cinema.rs), NVR's Cinema look from its preset's strengths:
// chromatic aberration, the picture overlaid on itself, a vignette, then film grain.
// Our own formulas [engineering choice], on the graded display picture.

import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;

struct Cinema {
    // x grain amplitude, y aberration (share of height at the corner), z overlay, w grain frame.
    params: vec4<f32>,
    // x vignette darkness, y radius it starts at.
    vignette: vec4<f32>,
}

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@group(0) @binding(2) var<uniform> cinema: Cinema;

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3(0.2126, 0.7152, 0.0722));
}

// A hash of a pixel and the grain frame, 0..1 (integer hash: no banding at 4K).
fn hash(p: vec2<u32>, frame: u32) -> f32 {
    var h = p.x * 1973u + p.y * 9277u + frame * 26699u;
    h = (h ^ (h >> 15u)) * 0x2c1b3c6du;
    h = (h ^ (h >> 12u)) * 0x297a2d39u;
    h = h ^ (h >> 15u);
    return f32(h & 0xffffffu) / 16777215.0;
}

@fragment
fn composite(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let src = textureSampleLevel(source, source_sampler, in.uv, 0.0);
    var c = src.rgb;
    let dims = vec2<f32>(textureDimensions(source));
    // From the centre, in units of the picture's height (round, whatever the aspect).
    let d = (in.uv - 0.5) * vec2(dims.x / dims.y, 1.0);
    let r2 = dot(d, d);

    // Chromatic aberration: red pushed out and blue pulled in along the radius, growing
    // with the square of the distance (nothing at the centre, where the reticle is).
    let ab = cinema.params.y;
    if ab > 0.0 {
        let shift = (in.uv - 0.5) * (ab * r2 / 0.6);
        c.r = textureSampleLevel(source, source_sampler, in.uv + shift, 0.0).r;
        c.b = textureSampleLevel(source, source_sampler, in.uv - shift, 0.0).b;
    }

    // The picture overlaid on itself (the overlay blend), mixed in by its strength: darks
    // darker, lights lighter, mid-grey unchanged.
    let ov = cinema.params.z;
    if ov > 0.0 {
        let x = clamp(c, vec3(0.0), vec3(1.0));
        let blended = select(1.0 - 2.0 * (1.0 - x) * (1.0 - x), 2.0 * x * x, x < vec3(0.5));
        c = mix(c, blended, ov);
    }

    // Vignette: from its radius (share of the half-diagonal) out to the corners.
    let dark = cinema.vignette.x;
    if dark > 0.0 {
        let half_diag = length(vec2(dims.x / dims.y, 1.0) * 0.5);
        let t = smoothstep(cinema.vignette.y, 1.0 + cinema.vignette.y * 0.5, sqrt(r2) / half_diag);
        c *= 1.0 - clamp(dark * 0.5, 0.0, 0.9) * t;
    }

    // Film grain, mean zero, strongest in the mid-tones (no lifted blacks at night, no
    // speckled whites in the sky's glare).
    let g = cinema.params.x;
    if g > 0.0 {
        let p = vec2<u32>(floor(in.position.xy));
        let n = hash(p, u32(cinema.params.w)) + hash(p + vec2(7u, 3u), u32(cinema.params.w) + 1u) - 1.0;
        let l = clamp(luminance(c), 0.0, 1.0);
        c += vec3(n * g * 4.0 * l * (1.0 - l));
    }
    return vec4(c, src.a);
}
