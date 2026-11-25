# The volumetric medium (#3032)

[design] An addition of NVForever's own, an engineering choice for the enhanced look: not a
fact about the original game (which draws its weather's clouds as flat layers on the sky
dome), never a parity item, and never to be used to explain a
difference from the original's frames. It is switched by the row "Volumetric clouds
(experimental)" (`--graphics-only volumetric-clouds`), off by default and in every preset.
Code: `engine/nvforever/src/medium.rs`, `medium_trace.wesl`, `medium_resolve.wesl`,
`medium_upsample.wesl`; the cloud source in `volumetric_clouds.rs` and
`volumetric_clouds_march.wesl` (graphics-enhanced.md §3.23).

## What it is, and why one system

Clouds, sunlight in the air, wind-blown dust and the weather's haze are all light scattered
and absorbed along the view ray. Built as separate effects, each would march the same air,
count the same light twice where they overlap, and disagree at their edges (a shaft of light
that ignores the cloud it passes, haze that is drawn over a cloud it should be behind). One
medium gathers the light of every source along each ray once, and the picture is composed
from it once. **Built so far are the clouds alone**; sunlight in the air, dust and haze are later
changes. The published technique is Bauer's "Creating the Atmospheric World of Red Dead
Redemption 2: A Complete and Integrated Solution" (SIGGRAPH 2019, Advances in Real-Time
Rendering in Games) [documented: Bauer 2019, slides 42-56]; the one-ray-in-several update is
also in Schneider's "The Real-Time Volumetric Cloudscapes of Horizon Zero Dawn" (SIGGRAPH
2015). Everything here is written from the talks' descriptions; no code from any project.

The talk has two representations [documented: Bauer 2019, slides 42-49]: a froxel grid near
the camera (lit and integrated slice by slice) and a ray march beyond it. **Only the far
representation exists yet** (stage 1): the grid comes with the sunlight in the
air (stage 3, a later change).

## The far buffer as built

**Where in the frame.** Two passes between the depth prepass and the main pass (where the
ambient occlusion's lighting pass sits), so the main pass reads a finished buffer; the picture
is composed in the main pass by the sky proxy (the dome's own mesh drawn again, depth test
Equal, in the sky's blended band, premultiplied over the dome, the stars and the moon). So
the clouds are composed in the linear HDR picture before tonemapping and before TAA, FSR and
DLSS, which treat them as sky (FSR's reactive reference draws the same proxy from the same
buffer). The row keeps the main view HDR (graphics.rs). Stage 1 is sky only: the rays are not
cut by the scene's depth, so the buffer has no depth discontinuities and the reconstruction
has no depth rejection yet; the weighted distance is kept for the stages that apply the
medium to the world.

**Sizes and formats.** The published image is a share of the main pass's render size in each
direction (the row's level: a third at Normal, two fifths at High, a quarter at Performance, "The
levels" below; half until 2026-11-23), rounded up (the render size is the upscalers' when FSR or DLSS draws:
`MainPassResolutionOverride`), `Rgba16Float`: rgb the light scattered toward the camera,
already weighted by what it hides (premultiplied, linear, the weather's colours times the sky
level), a the transmittance (1: nothing in the way). The divisor is a parameter, the scale
(`max(1, ceil(render / scale))` per axis, the level's 3, 2.5 or 4;
`NVF_PLAY_DEBUG_MEDIUM_SCALE=<1..4>` whatever the level) [design, #3032 stage 2a]. The
trace target is a third of that again
(one texel per 3x3 tile; `ceil(published / side)` for a tile side of 1 to 3): light `Rgba16Float` and the transmittance-weighted distance
`R32Float` (the cloud source's range, hundreds of thousands of units, is beyond half floats). Two history sets (light
`Rgba16Float`, distance and the cast count with the age `Rg32Float`) alternate. At scale 2 and side 2, 2560x1440: 37.8 MiB; 1920x1080:
21.3 MiB; at High 23.3 and 13.1 MiB [design: the sizes follow from the formats]. The published image is a stable
`Handle<Image>` the proxy's material binds, made again (a new handle) when the size changes,
since a material's bind group keeps the texture view it was made with.

**The levels** [design, 2026-11-23]. The row is a list: Off, Performance,
Normal, High (`volumetric_clouds.rs` `CloudLevel`; `--graphics-only volumetric-clouds` and an INI
written while it was a switch are Normal). High is the look judged by eye on 2026-11-23; Normal and
Performance differ in two things only:

| Level | Buffer scale | Probe spacing |
|---|---|---|
| High | 2.5 | as designed (800 or 1,600 units with the domain) |
| Normal | 3 | twice that |
| Performance | 4 | twice that |

The tile side stays 3 and the fine step's rule the same (a longer one pixelated in motion
[design: judged by eye in a play test, 2026-11-23]); what follows the cast ray's footprint
(`ray_spread`: the noise levels, the fine step far off) follows the buffer's texel, as with `NVF_PLAY_DEBUG_MEDIUM_SCALE`. The spacing's
factor is in the uniform (`profile.z`, `coarse_spacing` times `VolumeInputs::probe`); the march's text
does not know the levels. The block cap (16 cells) changes nothing near the camera: the near block is 8
cells of 120 units at High and exactly 16 at twice the spacing (4 and 8 in thin weather), and four
halvings bracket 16. Where the place width widens the spacing, the cap is met sooner than at High, so
a far block may be less than twice as long: more probes, never fewer, and it is the measured
prototype's own rule, which doubled the spacing in the shader under the same cap. The misses are the
refined march's own transect figures at twice the spacing (graphics-enhanced.md §3.23: 1.55 % of the
mass chords at amount 0.05, 1.66 % at 0.3; the baseline, High's spacing, 0.41 % and 0.45 %
[hypothesis: counted on a numerical mirror of the placement before the banks, not committed, and not
repeated on the bank field]).

A live change of the level is a change of the size the render world wants for the same epoch: one
hand-back (`a new far buffer is being made`), a new image and a new history, as for a render size
change; the old image goes when the material takes the new one. Log: `volumetric clouds: level <name>
(far buffer scale <s>, probe spacing x<f>)`.

Cost of the trace on an RTX 4080 SUPER at 1080p, fair at 60:20 / Zion at 150:18, its worst view [observed,
2026-11-23: the set-up of graphics-enhanced.md §3.23 ("The set-up of the cost figures"), the flat march with
the ceiling clip as edited shaders on c5a6c2993, the scales by `NVF_PLAY_DEBUG_MEDIUM_SCALE`, the spacing by
an edit, one run each]: High, the baseline of the others, 1.25 / 2.52 ms; scale 3 alone
1.07 / 1.97; scale 4 alone 0.99 / 1.32; twice the spacing alone 1.17 / 2.32. The levels as built
(99c5df603; `--graphics-only` `anisotropic,volumetric-clouds-high`, `anisotropic,volumetric-clouds` and
`anisotropic,volumetric-clouds-performance`): High, the baseline, 1.28 / 2.59; Normal 1.07 / 1.90;
Performance 1.01 / 1.32 [observed, 2026-11-23: the same set-up, one run each].

**The cast pattern** [documented: Bauer 2019, slides 51-54; our own form], as built at side 2 (the
default until 2026-11-23; side 3, the default now, casts each of a tile's nine texels once in nine
frames). Each frame one ray per 2x2 tile is cast: the tile's corners in the order (0,0), (1,0), (1,1), (0,1), the index the
frame's phase plus one on every other tile in a checkerboard, so neighbouring tiles never cast
the same corner in the same frame and every texel is cast once in four frames; a tile cut by
the buffer's edge cycles through the corners it has (a 1x1 buffer casts its one texel every
frame). The phase advances only on a whole reconstruction. Every tile is traced: the talk's
placement by the depth's minimum and maximum (slide 55) is not needed while the buffer is sky
only. The march's start offset is the R2 low-discrepancy sequence over the texel (Roberts,
"The Unreasonable Effectiveness of Quasirandom Sequences", 2018), turned by the golden ratio
at each of the texel's casts, so the history averages different offsets. No noise texture is
shipped.

**The tile side** is a parameter (3 by default; `NVF_PLAY_DEBUG_MEDIUM_TILE=<1|2|3>`) [design,
#3032 stage 2a]. At side 1 every texel is cast every frame and still goes through the history
(the running mean, the floor share and the clamp in force, as above): its nearest cast ray is
always its own, its age 0, and its start offset turns at every cast, modulo 64 (the phase's
upload modulo 256 holds that period too). The shaders take the side from the uniforms (the trace's
`tiles.z`, the reconstruction's `pattern.x`), so every side runs the same pipelines; side 2 is the
pattern above to the bit (a test compares it with stage 1's corners and offsets, edges and the
phase's wrap included). The 3x3 neighbourhood of the clamp is 3x3 tiles at either side (3x3
texels at side 1), and the motion ramp is in texels of the buffer, so another scale or side
changes the clamp's footprint and the ramp's angular meaning: judge those settings on their own.

**The ray.** Origin the camera, a unit direction through the texel's centre from the view's
projection **without** the upscalers' jitter (camera-relative, so nothing is lost far from
the world's origin), and the stretch `[t_min, t_max]` the medium owns (0 to the source's range
in this stage).

**The reconstruction** [documented: Bauer 2019, slide 52, adapted]. For each texel of the
published image:
- the box of the light and transmittance of the fresh rays of the 3x3 tiles around it;
- the cast ray nearest it among the 3x3 tiles (its own tile's on a tie): its distance seeds the
  history's lookup;
- its history: the point on its view ray at the distance the nearest cast ray found (by
  direction alone when that ray met nothing) is moved into last frame's view (last frame's
  unjittered projection and camera, camera-relative with the translation between the frames
  added) and last frame's buffer is read there, bilinear; behind last frame's camera or off
  its screen there is none; the history is held to this frame's rays by the clamp in force
  (below);
- a texel cast this frame takes its ray, blended with its history: **where we differ from the
  talk** [engineering choice]: the talk dithers and leans on TAA after the reconstruction
  (slide 48), but our default anti-aliasing is multisampling with no temporal pass after it, so
  a fresh ray takes a share of 0.1, still and moving (`NVF_PLAY_DEBUG_MEDIUM_FRESH`,
  `NVF_PLAY_DEBUG_MEDIUM_FRESH_MOVING`), as a floor under a running mean of the texel's own
  casts: share = max(floor, 1 / (max(count, 1) + 1)), the count being the texel's own casts since its
  history became valid (0 for a texel filled from the rays around or with none), carried with the
  history like the distance (nearest) and held at 255; so the first own cast after a cut
  averages with the fill (below), the two then weigh as one cast, and from the ninth the floor
  decides [engineering choice]. Where the step (share x (ray - history)) is smaller
  than the half float's spacing at the history's value or equal to it (2^-11 of it, the lower
  bound within a binade; 2^-24 below the normal range; a blend on the midpoint between two
  half floats rounds back to the even one) the blend may store the history again, so that
  channel alone moves toward the ray by the gap or twice that spacing, whichever is smaller
  (one or two stored steps), never past it. The threshold is conservative: it also catches
  some blends that would have stored one step away, and those then move two (history 0.5 under
  a ray of 0.49755859375 blends to exactly one step below and is nudged two); the cast is taken whole
  (light, transmittance and distance) only when the history is empty, the ray sees cloud and
  the blend would still store as empty (a thin cloud with stored transmittance 0.998046875 over
  empty history would otherwise blend to 0.9998, store as empty sky and never appear); the
  empty rule then applies as before. A first version took the whole ray whenever the
  transmittance stalled, which is the ordinary case where the transmittance is steady from ray
  to ray and the light is not, and brought the shimmer back (the figures below);
- a skipped texel with history takes the clamped history, its cast count, its age plus one
  (held at 15), and its distance
  carried into this frame's camera: the point on the history texel's own ray (last frame's
  camera) at its distance, moved by the camera's displacement, measured from here; a history
  clamped to empty sky gets the empty distance back;
- with no valid history, the fresh rays of the 3x3 tiles weighed by distance (next).

**The fill and the clip weigh the rays** [engineering choice: blocks where new cloud
enters the screen in a turn, and at a cut, judged by eye in a play test, 2026-11-23]. A texel with no valid history
was filled from the nearest of the nine fresh rays, so neighbouring texels held one value. It now
takes their mean under the weight exp(-d² / (2 x 0.36 x side²)), d the distance in texels from the
texel to the ray's own (at the buffer's edge a tile the neighbourhood is held to counts once, where it
lies: weight 0 for the copy). Premultiplied light and transmittance are both linear in what is composed, so
the mean composes to the mean of the rays. The variance clip's mean and spread use the same
weights, so the band is not one over a tile; the box does not. At side 1 no texel is ever filled
and the clip keeps its nine equal weights. The fill stands in for one cast: the texel's first own
cast averages with it, so the soft fill does not break into single raw rays in the frames after;
the count itself stays the texel's own casts. After n casts the fill's share is 1 / (2n) to the
tenth, then a tenth less a cast: 1/18 after nine (81 frames at side 3) [design: computed from the
share]. With a cast of its own behind it a texel of a still view is computed as before. The
distance stays the nearest ray's, so a mixed fill can hold cloud with an empty distance, as a
texel whose last cast met nothing already could; only the distance view reads it.
Stills of a view turning at 60 degrees a second: blocks at the entering edge with the nearest fill, none
with the weighted one [observed, 2026-11-23, judged by eye: the cost set-up of graphics-enhanced.md §3.23
in fair weather with `NVF_PLAY_DEBUG_MEDIUM_TURN=60` from the view 60:20, four stills 3.7 s apart from 60 s;
the shaders of c5a6c2993 edited, without and with the rule, and the rule as built, 99c5df603]. The
cost, one `exp` per ray per texel (nine), is not measured.

**The share and the clamp, as chosen** [engineering choice; a play test of the
experiment aids, judged by eye, 2026-11-19]. The first defaults (a share of 0.5 rising to 1
in motion, the history clamped to the box of the fresh rays) shimmered, still and moving: each
texel's new ray, cast every fourth frame with a turned start offset, let half of its 28-step
grain through, and in motion all of it. Measured on one machine, a still view (a played run
standing at Goodsprings looking up at clear noon, the cloud drift frozen), eight shots 70 ms apart, over
sky free of the HUD, the per-pixel temporal standard deviation in 8-bit levels and the share of pixels
stepping more than 2 levels between shots: the reference and the prototype 0.09 / 0.3 % (the
floor); a share of 0.5 0.36 / 2.4 %; 0.2 0.19 / 0.9 %; 0.1 with the box, the variance clip or
no clamp 0.14 / 0.4 to 0.5 % [observed, 2026-11-19, the development build 992ec21f5 and the aids below; the
heading and the size were not kept]. Shares from 0.05 to 0.2 kept low in motion were all acceptable, with
some shimmer left on tiny clouds; fixed or blue-noise offsets brought shimmer back; and with no clamp
specks held steady, while a loose variance clip brought their shimmer back [design: judged by eye in the
play test, 2026-11-19]. Mid-turn stills at 30°/s showed no trail even with no clamp [observed, 2026-11-19,
judged by eye: the same build and place, `NVF_PLAY_DEBUG_MEDIUM_TURN=30`]. So the share is 0.1 still and moving,
the offsets keep turning, and the clamp is `moving`: none while the view is still or slow, the
variance clip (the rays' weighted mean plus or minus 1.25 standard deviations per channel, the spread
taken from the deviations) fading in between 1 and 4 texels a frame of history motion. Why:
stage 1 is sky only, so there is no disocclusion for a clamp to catch; standing still the
camera's reprojection is exact (it lands on texel centres; the cloud content it carries can
still be stale, see the known limits) and a clamp can only harm a feature
smaller than a tile, which is hit one frame in four while on the other frames a band built only
from this frame's rays, which missed it, pulls the remembered speck toward empty sky however
loose the band (nine rays that all miss give a band of no width); in motion the history is
resampled every frame, and the clip bounds the blur and any trail.

**The measures a change to the reconstruction is held to** [observed, 2026-11-19, the
development builds named, one machine, the same steps each time]. First, standing still, as above: eight
shots 70 ms apart, the per-pixel temporal standard deviation in 8-bit levels and the share of pixels
stepping more than 2 levels between shots. Second, after a cut: the view snapped by 155°, shots
30 to 700 ms later (at about 300 frames a second), each one's mean difference from a shot
1.6 s later. At efe796057 (share 0.1, the `moving` clamp): clear noon 0.14 / 0.4 %, overcast
0.14 / 0.2 %; after the cut 1.31 1.18 1.03 0.92 0.79 0.53 0.25 0.16. At df4bbab7a (the running
mean and the first stall rule, which took the whole ray when the transmittance stalled): clear
0.27 / 1.1 %, overcast 0.25 / 0.7 %; after the cut 0.51 0.45 0.40 0.40 0.37 0.34 0.31 0.29, so
the running mean settles the first frames (0.51 against 1.31 at 30 ms) and the stall rule's
raw rays kept the picture from settling below about 0.3. The reference mode: 0.09 / 0.3 %.

A sample whose stored transmittance rounds to one adds no light in the composition (a very
thin edge would otherwise add its colour at zero alpha). The phase is uploaded modulo 256, the
pattern's period, so it stays an exact float.

**Cuts.** The history is dropped (one frame filled from the fresh rays around, no fade) on:
the camera's cut as the upscalers already test it (`UpscaleCamera::cut_from`, now applied to
the main view in every anti-aliasing mode: a frame skipped, a move of 400 units, a turn of
more than 45 degrees, a lens jump, a frame over 250 ms, a loading screen coming up (not its
leaving, "Under a loading screen" below), into or out of dialogue); another view, anti-aliasing in force, render extent, size or epoch (a history belongs to one
of each); the row switched on again or a trace asked for again (a gap since the last whole
reconstruction); and a jump of the sky, counted by the cloud source from the live sky: a
weather change with no fade (a script's ForceWeather), indoors and out (by the cell the camera
is in), another worldspace's dome, and the game clock (the hour, and the simulation's continuous
game time where there is one, so a jump of whole days to the same hour is seen and no day
boundary is invented) stepping back at all or ahead by more
than twice what the time scale and the frame's delta predict plus two of the sky clock's
half-minute steps (the wait and sleep menus, a console `set gamehour`, a jump of whole days;
a mod's high time scale alone does not cut). A running weather fade, its reversal or end, the
clock's own advance, midnight and walking over a cell's edge are not cuts: the clamp and the
four-frame refresh follow them.

**Under a loading screen** [design: a split second after the screen left, the clouds
were blocky, then resolved, judged by eye in a play test, 2026-11-23]. The screen is an interface node over a world
that is drawn all the same: the main camera stays active, the streaming, the sky, the cover and the
medium run, only `Time` stands still. So the history fills behind it, with the camera where play goes
on from; but the upscalers' cut test counts the change of play state out of `Loading` as a cut, and
that dropped the history in the first frame of play: nine frames of texels filled from the
casts around [design: upscaler/prep.rs `play_state_cut`, medium.rs `history_cut`]. The medium takes
`Loading` as `Playing` (`view_state`), so the screen's leaving keeps the history; its coming up still
cuts (the generation), and so do the travel's jump, the sky's jumps and indoors to out behind it. The
upscalers' own test is unchanged. A settled screen also waits for the clouds (`clouds_wait`): while the
row is on and a trace is asked for or to come (outdoors, above water, a cover composed or being
composed; a weather with no cloud layer has none, and the producer says so, `CloudCoverEmpty`), until the buffer is drawn and nine frames (the tile's texels) have been composed since the
last cut, seen so three frames in a row (`READY_FRAMES`: the render world's counts are a frame or two
old, and a cut starts the three again; a jump of the sky the history has not seen yet counts as one);
not with the row off, indoors, under water, under a clear or cloudless sky or a weather shown flat, or with
a pipeline that failed for good while nothing is drawn in its stead; at most 1 s of real time from when
all else was in, which fits the nine down to about nine frames a second and which the
settle time of 0.25 s is part of, so at 60 frames a second the nine are in before the screen would
leave anyway. After the nine, eight texels of nine have had their first own cast over a fill, half of it
each ("The fill" above): the picture is whole but still settling. The `loading screen phases` line names
`the clouds`; `loading screen: the volumetric clouds were drawn behind it, every texel cast` says that
the screen left with them ready, a debug line that the limit ended the wait.

Covered: the start in the world (`--play`, a save loaded from the menu, whose new run starts under
its own screen), load doors, fast travel, a script's or the console's move, all of which go through
`TravelRequest` and a screen. Not covered, the nine frames still seen: a New Game from the main
menu (it loads behind the menu and the intro film, without a screen), waiting and sleeping and a
console `set gamehour` (the sky's jump), dialogue's close-up in and out, a first frame of play over
250 ms, a script that changes the weather at once or the clock in the first frames of play, the row
or its level changed in play.

**The upsample.** The proxy's fragment reads the buffer at the screen position its own world
position has under the view's unjittered projection, bilinear, clamped to the edge. That
works for TAA, FSR and DLSS alike without knowing a backend's jitter sign: the fragment's
world position lies on its own jittered ray, and the unjittered projection maps it to where
the buffer holds that ray. The talk's dithered four-tap upsample (slide 56) leans on TAA and is
not used.

**What each mode sees.** MSAA: the proxy's edges against the world are the dome's own, per
sample; no jitter. TAA: the proxy is drawn jittered and resolved with the sky; the buffer is
not jittered. FSR and DLSS: the buffer is the level's share of the render size, composed before the upscaler.
FXAA and SMAA: after the composition, as for the sky.

**The handover.** The flat cloud layers give way only to a buffer of the right size written
whole by ready pipelines, the proxy's own draw pipeline among them: an epoch moves on every
change that could make the buffer wrong (the trace asked for or not, the view's size,
multisampling, HDR, upscaler or jitter, a new image), the render world echoes the epoch it
traced for, stamped with the render frame and the view, and the proxy is drawn held (adding
nothing) until a report of this epoch, this view and at most two frames old says the write was
whole and the proxy's own main-pass pipeline in that view (the id bevy_pbr specialised for
that entity, read every frame) is ready; then the hold is released and the flat layers leave
the main view in the same frame. A live change of the mesh pipeline's key (the shadow filter,
multisampling, HDR, the prepasses, the ambient occlusion in the lighting as its view bindings
are given, the sun visibility's bits that the Cloud shadows and Terrain shadows rows switch) is
part of the main world's view key, so the volume hands back before the frame in which bevy_pbr
would skip the proxy, until the new variant is ready. The rule: anything that enters the main
view's mesh pipeline key (bevy_pbr's `check_views_need_specialization`) and can change in play
must be in that key, otherwise the hand-back arrives up to two frames late; the inputs left out
are constant for the run (features the game does not use, the main camera's projection and
fog, the view transmission texture, which every 3D camera carries at its default quality and
nothing changes in play, and the tonemapping bits, which only a non-HDR view takes while the
row forces HDR). An absent entry in bevy_pbr's specialisation cache means
the proxy is not drawn that frame (bevy_pbr keeps an entry across frames and queues nothing
without one), so the verdict is not whole; a variant that fails, or an entry missing or not
ready after 10 s, is named in the log once. Each render frame publishes one verdict, once, at
the end of the frame (never a provisional one the main world could read meanwhile); a frame
whose pass did not run publishes "not written", and the image's extent is checked before
anything is written or the history moves. A verdict "not written" hands back at once; a whole
verdict more than two frames old keeps the volume as it is (a late verdict is not a bad one,
and the flat layers swapping in for a frame are a flash of their own) up to eight frames, and
hands back beyond. Every hand-back from a drawn volume logs `volumetric medium: hand-back to the
flat layers: <reason>` (the first at once, then at most one batch a second, the last reason with a
count of those not
shown) and the return `volumetric medium: the volume takes over again (<n> hand-backs so far)`;
the smoke scenarios forbid a hand-back in a run where nothing changes. A
loss the main world can see hands back in the same frame. A frame the render world alone
cannot draw leaves a buffer that had a whole write as it was (one frame of last frame's
clouds) and clears one that never had. The flat layers move to a render layer of their own while the volume draws, so
the water's mirror, the scope's lens and the extra cameras keep the weather's flat clouds. Under water the
request is taken back in `PostUpdate`, after that frame's camera was found under water (`underwater_fog`)
and before the handover (`trace_above_water`), so the flat layers are back in the frame it goes under.

**The cloud source's noise** (#3032 stage 2b). The trace binds, after the two uniforms, the
cloud source's generated base noise (3D), a repeating linear sampler (linear between levels too)
and the erosion noise (3D), in both entries; the source frame carries the two images' ids, and no
write is whole until both are on the GPU (`WriteCause::NoiseNotReady`; the source logs once
when the bake takes more than 10 s, and the medium once per activation when the generated images
are not both on the GPU after 10 s: `volumetric medium: the clouds' noise textures were generated
but are not both on the GPU after 10 s (base ..., erosion ...)`). The trace's bind group is kept while its buffers, both
views and the noise's sampler stay the same; the sampler is made with the first trace and
released with the activation, as the bind group is.

**The reference.** `NVF_PLAY_DEBUG_MEDIUM=reference`: the published image at the full render
size, every texel traced every frame, no history, the first prototype's per-pixel offset. It
traces the cloud source as it is now (since stage 2a the domain, the placement and the types),
so it is no longer the first prototype's picture; the measures quoted below for the prototype
and stage 1 are of the commits they name.

## The seam for sources

The medium's trace calls one function of each source: a ray in (`MediumRay`: origin, unit
direction, `[t_min, t_max]`, and its spread), the light it gathers out (`MediumSample`: the premultiplied colour,
the transmittance and the transmittance-weighted distance, -1 for an empty ray). Inside the
cloud source each sample asks three questions of its own functions: how much it takes
(extinction), whether the light reaches it (direct visibility) and what light arrives from all
around (incident ambient). A second source over the same stretch (haze beyond the near field,
later) must add its extinction and emission at the same samples of the same loop: adding a
second finished colour over the first would count the light through the shared stretch twice
[documented: Bauer 2019, slides 43-49]. The reconstruction depends only on the colour, the
transmittance and the distance, so a source changes nothing there. **The spread** (#3032 stage 2a, the
seam's one added value): the width per unit of distance of what one
cast ray answers for each frame, the cast texel's angular width times the tile side (so it
follows the buffer's scale, the tile side and the reference mode's full size), at the screen's
centre, from the view's real projection and the buffer's extent (medium.rs `ray_spread`, carried
in the trace uniform's `origin.w`; the wider axis on a texel the per-axis rounding leaves not
square). A source leaves out what is finer than `spread x distance`: the cloud source fades its
erosion's detail and its placement octaves by it (the placement through the footprint on the
bent shell); a later haze source will want it too. Direct visibility and the
ambient light are the two inputs later global illumination or ray tracing would replace.

## Weather mods

These are requirements of every stage:
- Every input comes from the load order's weather at run time through the live sky state (the
  shown weather, the one fading out, the fade, the hour, the sun): the cloud textures (the
  cover), the cloud colours and speeds, the sunlight colour, the sky level. Nothing is keyed to
  a weather's FormID, EditorID or plugin (the IDs are compared only to tell a fade from a
  snap); no constant fits only the base game's records.
- Scripted changes (SetWeather, ForceWeather, a mod's weather cycle) go through the same
  state: no per-weather result is kept past a change, a fade is blended, a snap is a cut.
- **When the medium cannot make sense of a weather, that weather is shown the flat way**,
  never a wrong sky: a weather with an active cloud layer whose texture cannot give plain 2D
  alpha gets no cover at all, whole, also while it is one side of a fade. Readable means: the
  sky's own loader would take the file (one set of shape rules: no cube map, sides and mip
  levels in range, its whole payload there), it is no volume texture whatever its depth, its
  format holds alpha, and a BC1 (DXT1) file uses its one-bit alpha somewhere (one whose blocks
  are all opaque carries none); a DXT5 or ARGB file at full alpha everywhere is a true full
  cover. A texture in no archive is unreadable too. The checks read the header and the sizes; only a BC1 file's top-level alpha is scanned
  for a transparent texel, and the producer decodes the top level's alpha (as Cloud shadows does)
  to compose the cover: no texture is decoded whole to RGBA to learn whether it has alpha. Each
  weather gets a cover of its own only from a composition made once every active layer was read;
  while any active layer of either weather of a fade is still being read nothing is published (the
  flat layers show), and the volume is drawn only while every live weather with a weight has its
  own cover (a live weather that lacks one is composed at once, not at the interval), with one
  exception: while a fade's incoming weather counts for at most 5 % and its first cover is being composed,
  the measured outgoing cover is taken whole, for at most one real second (cloud_shadows.rs `CoverWait`).
  The
  volume's cover recomposes on the real clock, so a paused game does not stretch that (with
  Cloud shadows on too, both keep the game's clock as before). While the volume wants the cover, a
  texture still being read after 10 s is named in the log once and its weather stays flat (with
  Cloud shadows alone none of this exists). A weather with a value the medium reads that
  is not a finite number at run time (a fog
  distance, a colour, the sky level, the cover's projection), on either side of a fade, is shown
  flat rather than repaired. A weather with no cloud texture, or an empty cover, is not drawn
  either. In an interior cell (by the cell the camera is in) and under a water surface (whether
  or not its water record could be read) the volume is not drawn.
- A mod changes the amount, the tint, the drift and the wind of the clouds; it cannot change
  their shapes or, since stage 2a, where they stand ([design]: the weather's composed cover gives the amount, its mean; the placement is the cloud
  source's own field, graphics-enhanced.md §3.23). A mod whose point is painted cloud forms
  loses them on the layers that become volume; the row is off by default.
- Finite extreme values (a sky level of 3e38, a fog distance of 0) give finite, bounded,
  non-negative uniforms (tested); no range beyond that is guessed. A weather record's non-finite
  field is the loader's 0.0 for the medium as for everything else (the fog, the flat sky and the
  lighting all run on that value): the medium agrees with what the rest of the frame shows and
  does not second-guess the loader from the raw record [design, 2026-11-19].
- Nothing is written anywhere a mod reads; the medium needs no record.

## The clouds' light volume (#3032 stage 2g, checkpoint B1)

An engineering choice for the enhanced look (graphics-enhanced.md §3.23 has the whole account). The
clouds' optical depth toward the light is no longer marched by eight taps from every lit sample but
kept on two nested lattices around the camera and read once, after the first two of the eight
intervals toward the light are marched [design; Kim and Neumann 2001, Bauer 2019 slides 33 and 35,
documented]. Near level 128 x 128 at 500 units, far level 128 x 128 at 4,000 units, both a layer every 1,000 units
(25 at the domain's 24,000), world-aligned in the wind's field coordinates, swept top down on the GPU before the medium's
trace, on its encoder (`nvf_cloud_light_far`, `nvf_cloud_light_near`, then `nvf_medium_trace`).
Since B2 each level has a front set the trace reads and a back set swept over 8 (near) or 32 (far)
frames from one frozen snapshot of the uniform; the sets swap when the back is finished and the
trace reads each front through its own window (graphics-enhanced.md §3.23). It follows the weather
through the march's own uniform (the same density as the view march, the same sun or moon
direction), at most 16 or 80 frames behind; a weather that jumps (a forced weather, a tuning file),
a clock jump, the moon taking over or a teleport drops the sets that no longer fit and the
eight taps stand in until a sweep finishes, so a weather change or a mod's weather needs nothing of its
own; with no light, or with
`NVF_PLAY_DEBUG_VOLUMETRIC_CLOUDS_LIGHT_VOLUME=off`, nothing is held and the trace runs its legacy
variant, all eight taps. The figures that chose it, printed by trial tests on the CPU mirror
of the body of 2026-11-20 (removed with that body, so they cannot be printed
again here) [hypothesis]: |p95| against the fine depth to the top 0.013 to 0.028 for
the volume alone, 0.05 to 0.11 for the eight taps (the baseline); the largest crease 0.05 to 0.29 against
the taps' 0.30 to 1.75.
Since stage 2i M2 a shared reader of the completed fronts (`LightVolumeReader`,
`light_volume_reader.wesl`) gives the ground's cloud shadows their depth (graphics-enhanced.md §3.23,
"Ground shadows from the light volume").
Not built: a cross-fade between generations.

## What is not done

The near froxel grid and light in the air (stage 3), a second cloud layer, wind-blown
dust, haze and fog as one medium (stage 4), clouds in front of terrain (the medium applied to
world fragments), depth-aware ray placement (slide 55) and
depth rejection in the clamp (slide 52: not needed while the buffer is sky only), the
dithered four-tap upsample (slide 56), and a DLSS smoke scenario (DLSS needs files a player
supplies). A tile side of 4 (one ray in sixteen) is not built [design]:
a full tile needs 16 placements and an offset period of 1,024 phases (the upload's modulus
with it), and its 15 skipped frames fit the age's packing; what is missing is a fair schedule
for the tiles the buffer's edge cuts to 3, 6, 9 or 12 texels, tested across the phase's wrap with
both shaders agreeing, and a turn and cut run that accepts the clamp's footprint 12 texels wide
and a half-life of about 105 frames at the share of 0.1. The 3x3 search for the nearest cast ray
would still do (the farthest own corner is 3√2 texels away, any excluded tile at least five). The flat layers' visible shares order the layers by the dome's ranks
(`2 x layer + set`) while Bevy's transparent sort also takes each shape's distance, so at some
camera pitches two overlapping flat layers can be drawn the other way round than the shares
assume (a wrong tint share where two opaque layers overlap); fixing it belongs to the sky's own
sort keys and would change the flat sky with the row off, so it is left for an issue of its own.

## Known limits of stage 1

- **An opaque-only BC1 cloud texture shows its weather flat.** Of the base game's and the DLCs'
  sky textures, the one DXT1 file a weather names as a cloud layer is
  `textures\nvdlc04\sky\nvdlc04cloudovercast.dds` (Lonesome Road, 1024x1024; none of its
  65,536 blocks has a transparent texel), named in one cloud layer by 7 of LonesomeRoad.esm's 15
  weathers: NVDLC04HRWeatherFoggy, NVDLC04NukedWeather, NVDLC04LRInt02Weather,
  NVDLC04LRIntWeather, NVDLC04DivideVistaBWeather, NVDLC04HRWeatherClear and
  NVDLC04LRWeatherFoggy. Where that layer is active these weathers are shown flat (before this
  rule the volume would have made them a featureless full deck and lost the texture's painted
  pattern). No weather of FalloutNV.esm (63), DeadMoney.esm (8), HonestHearts.esm (9) or
  OldWorldBlues.esm (3) names a DXT1 cloud texture, so nothing there changes [documented: the
  plugins' WTHR records and the archives' DDS headers and blocks, read on
  2026-11-19; not yet looked at in a run]. The code keys nothing to these weathers or this
  file: the rule is the texture's alpha, as for any mod's texture.
- The cast count is not reset when a fast turn's clip replaces most of a texel's history (that
  would raise the share in motion, the shimmer the play test rejected), so after a fast turn stops a
  clipped texel settles at the floor share: about a fifth of its remaining error is left after
  a second at 60 frames a second (0.9^15) [design: computed from the share].
- The count is the nearest history texel's while the light is a bilinear mix of four, so near
  a junction with young or filled texels an established count can ride on partly young light
  and hold it at the floor share [hypothesis: inferred from the code, not seen in a run].
  The least count of the bilinear taps was tried (6047e2e33) and raised the grain in turns:
  the frames' spatial grain at 30°/s 1.13 against 0.97 to 0.99 in five earlier builds with the
  low share in motion, at 120°/s 1.24 against 1.09 to 1.12 [observed, 2026-11-19, one machine:
  the place of the still measure above with `NVF_PLAY_DEBUG_MEDIUM_TURN` at 30 and 120], since a minimum over the footprint each frame spreads low
  counts faster than casts rebuild them. If the case is ever seen, a count weighted through the
  bilinear weights (an effective sample count) is the thing to examine.
- A feature smaller than a tile can still flicker while the view turns fast: the clip is on
  then, and the specks come back as they did with a loose clamp.
- The camera's reprojection is exact, but the cloud content a texel remembers can be stale:
  when an established texel's clouds change under a still camera (cover drifting with the wind,
  a fade, the light moving within the time-scale allowance), it follows at the floor share,
  0.9^n of the old contrast left after n casts, one cast in four frames: half after about 26
  frames, a fifth after 60, a tenth after 88 [design: computed from the share, before storage
  rounding; not looked at in a run, not reported in the play test]. A young texel (fewer than
  nine own casts since a cut) follows faster by the running mean. Stage 2 is to examine moving
  the history with the cover's own drift (the wind) in the reprojection; no detector of a
  confirmed discrepancy is built.
- When the clouds come in front of terrain (the stage that applies the medium to the world), a
  history texel whose depth no longer matches the scene's must be rejected by a depth test (the
  talk's depth discrepancy, slide 52) rather than by any colour clamp.
- The flat layers' sort order (above) and the cloud look itself (stage 2).

## Cost

Indicative figures [observed: measured runs on one machine, 2026-11-19, at the development build
09d1abc43; Goodsprings at 13:00, `--no-actors`, sun shadows on, ambient occlusion, longer
shadows, light glow and sun shafts on; the median of the last five 4 s periods of two rounds;
whole frame in ms and the GPU spans of the `gpu time:` log line]:

| Case | Row off | First prototype | Reference | Default (trace / reconstruct) |
|---|---|---|---|---|
| 2560x1440, NVWastelandClear | 3.21 | 7.1 to 8.4 (its march 3.9 to 5.2) | 7.48 (trace 4.28) | 3.98 to 4.21 (0.61 to 0.93 / 0.04 to 0.09) |
| 2560x1440, turning at 60 deg/s | 3.63 | | | 4.23 to 4.44 (0.71 to 0.87) |
| 2560x1440, NVBlackMountainWeather | 3.30 | | | 4.32 to 4.47 (0.97 to 1.13 / 0.09 to 0.10) |
| 1920x1080, clear | 2.93 | 5.76 | | 3.19 to 3.28 (0.64 / 0.03) |
| FSR Quality, 2560x1440 output | 3.11 | | | 3.60 (0.50) |

These are stage 1's figures: a slab of 28 view steps, one ray in four into a half-size buffer. Against
the stage's aim of 1 ms at 2560x1440 the whole frame grew, with the row off as the baseline, by +0.8 to
+1.0 ms in clear weather (about a fifth of the first prototype's +4.3 over the same baseline), met at the
edge, and by +1.0 to +1.17 ms under overcast, missed. The trace's span followed the march's step count (4
view steps 0.17 ms, 28 steps 0.61 to 0.93, 56 steps 1.57 at 2560x1440). They do not hold for the clouds
as they are now, whose trace is measured in "The levels" above and in graphics-enhanced.md §3.23, "The
view march" (each names its baseline). At stage 1, per ray, the small pass cost
more than the full-size reference (about 3 to 8 ns a ray against 1.2 to 2.1); the frame was
close to CPU-bound in the small cases, so the GPU may have been clocked down, and the default's
rays lie two texels apart on the half-size buffer, so neighbouring threads read the cover and
the noise less coherently and leave the march at more different steps [hypothesis; the tile
entry's own work per ray, the corner choice and the R2 offset, is a few integer operations
beside a 28-step march].

## Debug aids

Developer aids (engine/nvforever/src/dev_aids.rs): they exist only in tests, in debug builds and in a build
with the `dev-aids` feature (in engine/: `cargo build --release -p nvforever --features dev-aids`). A release
build without `dev-aids` reads none of these variables, has neither the tune file's follower nor the turn's system, and runs with
the defaults given here. Read once at the start, usable in a played run (they do not make a run unattended):
- `NVF_PLAY_DEBUG_MEDIUM=reference`: the reference above.
- `NVF_PLAY_DEBUG_MEDIUM_VIEW=fresh|age|history|clamped|distance`: the buffer shows, opaque
  over the sky: `fresh` green where a ray was cast this frame, grey where the texel was rebuilt
  from its history, red where it had none; `age` the frames since its last ray (white at 8,
  stored up to 15; the cast count shares its channel and is not shown);
  `history` red where no valid history was found, the picture elsewhere; `clamped` how much the
  clamp moved the history (x4); `distance` the weighted distance (white at the source's range), blue for
  an empty ray. Ignored in the reference.
- `NVF_PLAY_DEBUG_MEDIUM_FRESH=<0..1>`: the fresh share's floor (0.1).
- `NVF_PLAY_DEBUG_MEDIUM_FRESH_MOVING=<0..1>`: the share the motion ramp rises to (0.1): the share
  is max(floor, ramp(motion) x this), so with a low value a moving view keeps accumulating
  through the reprojected history (the clamp and the fallback for a texel with no history stay).
- `NVF_PLAY_DEBUG_MEDIUM_OFFSETS=turning|fixed|blue`: the default mode's ray start offsets:
  `turning` (the default) the R2 offset of the texel turned by the golden ratio at each cast;
  `fixed` the texel's R2 offset at every cast (no grain in time, a fixed grain in space); `blue`
  interleaved gradient noise of the texel (Jimenez 2014) turned by the golden ratio at each cast;
  `centre` 0.5 for every ray at every cast (#3032 B1b: a pattern still seen with it is not the
  offsets'). No effect in the reference mode.
- The log tallies the reconstruction's history cuts (#3032 B1b): after the first 120 frames,
  then after a later window of 120 with cuts only when its dominant cause (the most counted of
  a gap, a camera cut, a frame over 250 ms, a sky jump, a new history) differs from the last
  logged one's (a camera cut now and then is ordinary play and writes no line
  every two seconds), `volumetric medium: history dropped on <n> of the last <m> frames (a gap,
  a camera cut of which a frame over 250 ms, a sky jump, a new or missing history)`;
  `NVF_PLAY_DEBUG_MEDIUM_TALLY=1` logs every window that had a cut. A history dropped every
  frame leaves eight texels of nine (three of four at side 2) filled from the cast rays around: soft,
  unsteady edges whatever the fresh share or the offsets.
- `NVF_PLAY_DEBUG_MEDIUM_CLAMP=moving|box|variance|off`: how the history is held to this frame's
  rays: `moving` (the default, above); `box` the box of the fresh rays of the 3x3 tiles (the
  first default); `variance` their weighted mean plus or
  minus k standard deviations per channel (Salvi, "An Excursion in Temporal Supersampling", GDC
  2016), kept to what the stored format holds; `off` none (for diagnosis: the history as
  reprojected; the no-history fallback and the cuts stay). `NVF_PLAY_DEBUG_MEDIUM_CLAMP_K=<0.5..8>`
  sets k (1.25, also `moving`'s). The `clamped` view shows how far the clamp in force moved the
  history.
- `NVF_PLAY_DEBUG_MEDIUM_TURN=<degrees a second>`: the view turns at that rate (positive to the
  right; the player's heading in a played run, the free camera's yaw in the viewer), for
  pictures and cost runs mid-turn.
- `NVF_PLAY_DEBUG_MEDIUM_SCALE=<1..4>`: the default mode's published image is the render size
  divided by this, rounded up, whatever the row's level (a whole number held to 1..4, or 2.5; unset
  or unreadable: the level's own).
  No effect in the reference mode.
- `NVF_PLAY_DEBUG_MEDIUM_HISTORY_FILTER=linear|cubic|adaptive` (adaptive): how the history is read
  between its texels while it moves: `linear` bilinear; `cubic` with the Catmull-Rom correction, held to
  the range of the fresh rays and the bilinear history; `adaptive` cubic, with the cast count limited by
  the squared bilinear weights.
- `NVF_PLAY_DEBUG_VOLUMETRIC_CLOUDS_VIEW=sun|ambient|depth` (#3032 stage 2e, L1; graphics-enhanced.md
  §3.23): the cloud source lit by the direct light alone, by the ambient alone, or the optical
  depth toward the light it measured (grey on a log scale, magenta where the light was skipped;
  per ray the opacity-weighted mean, opaque). `VIEW=opacity` shows camera-ray opacity, `body-height`
  where the visible material sits between its local base and top; `cover`, `vigour`, `count` and
  `noise` are in §3.23. `VIEW=none` restores the picture.
- `NVF_PLAY_DEBUG_VOLUMETRIC_CLOUDS_TUNE=<file>` (#3032 stage 2e, L1): the cloud source's aids
  tuned live from a `NAME=value` file, looked at at most every 0.5 s of real time while the row is
  on (live: `SHAPE`, `EROSION`, `DENSITY`, `STEPS`, `TYPE`, `VIEW`, `WISP`, the light's weights and
  `SUNCOLOUR`; start-only: `HEIGHT`, `HORIZON`).
  Each change of the effective values counts as a sky jump for the history (dropped
  once), so paired pictures after an edit start from fresh rays. A key the file does not set (a
  deleted line included) takes its start value: its start variable, else its start default
  (graphics-enhanced.md §3.23, "Debug aids").
- `NVF_PLAY_DEBUG_VOLUMETRIC_CLOUDS_LIGHT_VOLUME=off|near|far|both` (#3032 stage 2g, B1; default
  both; off: the eight taps, nothing held), `..._LIGHT_VOLUME_CELLS=<near>,<far>` (the levels'
  spacings across, units; 500 and 4,000), `NVF_PLAY_DEBUG_VOLUMETRIC_CLOUDS_LIGHT_TAPS=0..8`
  (the near taps before the volume is read, 2; 8 is the legacy path; at least one is marched whatever
  is asked, and the logs state the count marched) and
  `..._LIGHT_VOLUME_SPREAD=<near>,<far>` (the frames a sweep takes, 1 to 256; default 8 and 32;
  `1,1` is B1's whole sweep every frame).
- `NVF_PLAY_DEBUG_VOLUMETRIC_CLOUDS_` `SHAPE`, `EROSION`, `STEPS`, `TYPE`, `HEIGHT`, `HORIZON`, `WISP`,
  `PUFF`, `AIR`, `REFINED_STRIDE`, `LIGHT_REUSE` and the light's `OCTAVE`, `BACKSCATTER`, `UPPER`, `LOWER`,
  `GROUND`, `FORWARD`, `SUN`, `LEVEL`, `OCCLUDE`, `POWDER`, `SUNCOLOUR` (alias `SUNCOLOR`), and
  `NVF_PLAY_DEBUG_VOLUMETRIC_CLOUDS=<0..4>` (`DENSITY`): the cloud source's start values, ranges and meanings
  are graphics-enhanced.md §3.23, "Debug aids". The cloud source's light is the source's own business:
  the medium's seam (`MediumRay` in, `MediumSample` out) and its history do not see it. The wisps' phases
  are the base noise's, so they do not drift against the body and the reprojection is unaffected.
  Smoke: `volumetric-clouds` checks the defaults, `volumetric-clouds-no-aids` that a release build ignores the
  aids.
- `NVF_PLAY_DEBUG_CLOUD_SHADOWS_SOURCE=weather|volume|auto|share|depth` (#3032 stage 2i, M2; graphics-enhanced.md
  §3.23, "Ground shadows from the light volume"; default auto): where Cloud shadows come from while
  that row and Volumetric clouds are on (the layers whatever it says while Volumetric clouds is off). `auto`: the light volume's completed fronts where they have data, the weather's
  flat layers elsewhere, mixed through the windows' guard bands and while a level fades in (about a
  second); the layers whole unless the volumetric sky is what the player sees: this row on with the
  medium's buffer shown (its composing material ready, the flat sky hidden), its clouds asked of the
  medium, and a front to read (sun_visibility.rs `AutoGate`);
  `weather`: the layers only (the shadows before M2, for paired pictures); `volume`: the volume only,
  no shadow where it has no data; `share` and `depth`: debug views of the reader in place of K, its
  covered share (presence and guard bands included, 0 no data) and its optical depth over 8 (shown
  as grey with `NVF_PLAY_DEBUG_SUN_VISIBILITY=cloud`). Log line `cloud shadows: source <what and
  why>` whenever the source in use changes while Volumetric clouds is on, and when it goes off.
- `NVF_PLAY_DEBUG_MEDIUM_TILE=<1|2|3>`: one ray per tile of this side a frame (3); 1 casts every
  texel every frame through the history. Any other value is 3 (4 is not built). No effect in the
  reference mode.

Log lines: `volumetric medium: far buffer <w>x<h> (a third of the main pass's render size), one ray
in nine a frame into <w>x<h>, ...` (the scale in words: `the main pass's render size`, `half`,
`two fifths of`, `a third of`, `a quarter of`; the tile: `one ray in nine`, `one ray in four` or `one ray per texel`; or `(the main
pass's render size), every pixel traced each
frame (reference), ...`) when it is made or resized; `volumetric medium: trace pass drawn (...)`
once; `volumetric medium: far buffer <w>x<h> written whole and volumetric_clouds_pipeline ready,
the clouds drawn from it` when the flat layers give way; `volumetric medium: far buffer
released`. Smoke: `volumetric-clouds` (Normal), `volumetric-clouds-taa` (High),
`volumetric-clouds-fsr` (Performance), the same two turning with `NVF_SMOKE_DEV_AIDS` set (`-taa-turn`, `-fsr-turn`:
they turn the view by an aid),
`volumetric-clouds-level-change` (the three, in play).
