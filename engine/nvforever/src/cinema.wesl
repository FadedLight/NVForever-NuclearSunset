// The engine's Cinema pass (cinema.rs), NVR's Cinema look from its preset's strengths:
// chromatic aberration, the picture overlaid on itself, a vignette, then film grain.
// Our own formulas [engineering choice], on the graded display picture.

import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;

struct Cinema {
    // x grain amplitude, y aberration (share of height at the corner), z overlay, w grain frame.
    params: vec4<f32>,
    // x vignette darkness, y radius it starts at.
    vignette: vec4<f32>,
    // Water drops: x left (1..0), y refraction, z run, w surfacing number.
    lens: vec4<f32>,
}

// One hash of a drop cell and the surfacing number, 0..1 (four values).
fn cell_hash(c: vec2<i32>, n: u32) -> vec4<f32> {
    let p = vec2<u32>(bitcast<u32>(c.x), bitcast<u32>(c.y));
    return vec4(hash(p, n * 4u), hash(p, n * 4u + 1u), hash(p, n * 4u + 2u), hash(p, n * 4u + 3u));
}

// Rows of drop cells over the picture's height [engineering choice].
const DROP_ROWS: f32 = 9.0;

// Where the drops bend the picture: a uv offset in xy, and in z how much of a drop covers here.
fn drops(uv: vec2<f32>, aspect: f32) -> vec3<f32> {
    let left = cinema.lens.x;
    var off = vec2(0.0);
    var cover = 0.0;
    // In units of cells, x stretched by the aspect so the drops stay round.
    let base = vec2(uv.x * aspect, uv.y) * DROP_ROWS;
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let cell = vec2<i32>(floor(base)) + vec2(dx, dy);
            let h = cell_hash(cell, u32(cinema.lens.w));
            if h.x > 0.45 {
                continue;
            }
            // Each drop runs down at its own pace, bigger ones faster.
            let r = 0.18 + 0.22 * h.w;
            let run = cinema.lens.z * DROP_ROWS * (0.4 + 1.2 * h.z) * (r / 0.4);
            let centre = vec2<f32>(cell) + vec2(0.2 + 0.6 * h.y, 0.2 + 0.6 * h.z + run);
            let d = (base - centre) / r;
            let q = dot(d, d);
            if q < 1.0 {
                // A lens-shaped drop: steepest at its rim, flat in its middle.
                let dome = sqrt(1.0 - q);
                off -= d * (1.0 - dome) * cinema.lens.y;
                cover = max(cover, smoothstep(1.0, 0.7, q));
            }
        }
    }
    // Drops thin out as they go: smaller bends and edges as `left` falls.
    let fade = smoothstep(0.0, 0.6, left);
    return vec3(off * fade * vec2(1.0 / aspect, 1.0), cover * fade);
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
    let dims = vec2<f32>(textureDimensions(source));
    // Water drops on the lens bend where the picture is read from.
    var uv = in.uv;
    var cover = 0.0;
    if cinema.lens.x > 0.0 {
        let w = drops(in.uv, dims.x / dims.y);
        uv = clamp(in.uv + w.xy, vec2(0.0), vec2(1.0));
        cover = w.z;
    }
    let src = textureSampleLevel(source, source_sampler, in.uv, 0.0);
    var c = textureSampleLevel(source, source_sampler, uv, 0.0).rgb;
    // From the centre, in units of the picture's height (round, whatever the aspect).
    let d = (in.uv - 0.5) * vec2(dims.x / dims.y, 1.0);
    let r2 = dot(d, d);

    // Chromatic aberration: red pushed out and blue pulled in along the radius, growing
    // with the square of the distance (nothing at the centre, where the reticle is).
    let ab = cinema.params.y;
    if ab > 0.0 {
        // From the place the drops moved the read to, so the fringes follow the drops.
        let shift = (in.uv - 0.5) * (ab * r2 / 0.6);
        c.r = textureSampleLevel(source, source_sampler, uv + shift, 0.0).r;
        c.b = textureSampleLevel(source, source_sampler, uv - shift, 0.0).b;
    }
    // A drop's rim catches a little light [engineering choice: 4 % lift at full cover, so a
    // drop reads as water on the lens and not only as a bend].
    c += vec3(0.04) * cover;

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
