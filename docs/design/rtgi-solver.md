# The bounce-light solver (`engine/nvf-rtgi`, #3009)

The plan is a crate that works out the light that an interior's lamps throw back from its
surfaces, as a small 3D texture, on the CPU and without Bevy. It is built in parts, each with
tests on scenes it makes itself. **Built: part 1, the ray scene, and part 2, the light gathered at
points of the air.** Nothing in the game calls it yet: there is no setting, no upload and no shader.
It ships no data.

## The whole

The bounced light is to take the place of a part of the flat ambient, fitted so that the mean
brightness of a room stays what it was. Interiors only, off by default, one bounce, no ray tracing hardware. The
parts, in order:

1. **The ray scene** (built): the triangles of a cell and what a ray meets.
2. **The light** (built): the engine's light sum at a place, the bounce gathered around a point in
   the air, and the grid of points that can be reached.
3. The solve: the points packed as a texture, and its reading on the CPU.
4. The fit: how much of the ambient goes and how strong the bounce in its place is.
5. The shader that reads such a texture, with nothing drawn by default.
6. The collector: a room's geometry, textures and lamps from the running game.
7. The job: a solve on entry, the fade from stock light, and what happens when there is none.
8. Light that changes during a visit, and the night.
9. The setting.

Parts 1 to 4 change nothing in the game. This note grows with them.

## The ray scene

Input is plain data: a cell's meshes, instances, materials and textures (RGBA8 with mips).
`prepare` puts all triangles into one soup in world space and builds a bounding volume tree over
it (binned surface area heuristic, the halves built in parallel on at most as many threads as it
is told; the same tree from any number of them).

- A ray against a triangle is the watertight test of Woop, Benthin and Wald in single precision,
  the edges again in double precision where one is 0: no ray slips between two triangles that
  share an edge [documented: "Watertight Ray/Triangle Intersection", Journal of Computer Graphics
  Techniques 2(1), 2013].
- [design] A ray stops at every surface, on either side. A hit on a side that is not drawn is flagged
  (`Hit::hidden`); light that reaches it is the caller's business (a solver takes it for black).
- [design] Below the alpha cutoff a ray goes on.
- [design] Of hits at exactly the same distance the first in the scene's order is met. Two different
  triangles in one plane (a decal on its wall) can differ by a rounding, so either may be met.
- An instance that mirrors keeps its front, its shading normal and its texture.
- A ray that runs along a box's face meets what ends on that face: an axis the ray does not move
  along is tested against the closed box. [design] A component below 1e-20 of the largest, or
  whose reciprocal overflows, counts as none.

## The light

A cell's light is plain data as well (`LightState`): ambient, directional lights and lamps, in
display values, for one state of the light. `reach` takes it with a prepared scene and the seeds,
places in the air to start from, and gives the points that can be reached with what is gathered at
each (`Reached`).

**The sum** at a surface is the same formula as the engine's
[documented: `engine/nvforever/src/lights/display_shader.rs`]:

```text
S    = ambient + sum over directionals (display x saturate(N.L))
               + sum over lamps (display x attenuation x cone x saturate(N.L))
out  = min(albedo x max(S, 0)^2.2, 1), per channel
```

The attenuation is `saturate(1 - d^2 / range^2)` [documented: `engine/nvforever/src/lights.rs`
`GAME_ATTENUATION`], the cone the square of `saturate((cos - cos outer) / max(cos inner - cos
outer, 1e-4))` [documented: bevy_pbr 0.20.1 `render/light.rs`, `render/pbr_lighting.wesl`].
Right at a lamp N.L is shorter (over at least 0.01 units) and the cone's direction is not, as
there. It is the formula, not the engine's bits: the engine adds its terms in another order.
Whether the sum is the original's is the engine's question, not this crate's. Left out of it:
ambient occlusion, what the engine does to a flashlight on first-person shapes, and its weights
for grass.

**What is gathered** at a point comes from rays over the sphere, the same directions at every
point. What a ray brings depends on what it meets:

- [design] A lit surface sends on what the lamps **add** to what it shows: the surface under
  ambient, directional and lamps, less the same under ambient and directional alone, each raised
  to the power and clipped first. So a room without lamps bounces nothing, whatever its ambient,
  and with a brighter ambient the same lamp adds more, up to the clip.
- [design] A lamp counts where a shadow ray finds it. The directional part is taken without one,
  on both sides of the difference.
- [design] A single-sided surface's back is black. A double-sided one is lit on the side that is met,
  by the interpolated normal turned round, and its rays leave from that side.
- [design] A surface drawn on both sides without being double-sided keeps its front's normal. A lamp
  on the side met is then behind it, unless the interpolated normal is tilted towards the lamp.
- [design] An unlit surface (a window, a screen) sends a share of its own colour, clipped to what
  is displayed, lamps or not.
- [design] A miss brings nothing. There is one bounce.

[design] Any surface can stand in a lamp's way, whatever it is made of. **A lamp in a closed
shade bounces nothing** outside of it, while on the screen a lamp without a shadow lights its
room through the shade: the engine draws placed lamps without one unless its ALC shadows row is
on [documented: `engine/nvforever/src/light_shadows.rs`]. A test pins this as it is. How many of
a room's lamps it concerns is not counted.

Kept are the two lowest bands of the light L from the directions w (`Moments`): C0 = mean(L) and
Cj = 2 mean(L wj). The light on a surface that faces n is C0 + C.n [design]. With exact means
over the sphere that is exact for light that has only those bands; the means of 512 rays are near
them (a test holds two such lights to 0.004). It can be below 0 on the side turned away from a
small source: what to do then is the reader's business.

**The points** are the multiples of the spacing in the cell's own coordinates. A seed starts those
of the 8 around it that it sees and that are in the air, else those of the 64 around it. From
there the flood goes on to neighbours along an axis, inside the box about the triangles, with
nothing on the way. A point is in the air when its rays meet something and at most a quarter of it
shows its back. `Coverage` says how a flood went: seeds that found nothing, points, and the parts
they are in. A `Probe` says what its rays did: `lamps` counts the lamps that were looked at, all
those of each lit hit's cell of the lamps' grid, whether their spheres hold the hit or not.

The same input gives the same points and the same light on any number of threads, on one machine
and toolchain. Between toolchains that is not shown.

## Units

Bevy's world space in game units (y up, -z north), linear colour unless called a display value.

## Numbers

All are ours, none is a claim about the original game.

| Number | Value | Why |
|---|---|---|
| alpha and colour mips | the largest level of at most 256 and of at most 64 texels a side, else the smallest there is | [design] one fixed level a lookup: a ray has no pixel to choose it by |
| rays' lift | 0.01 units, or 4e-6 of the largest coordinate | [design] a ray does not meet the surface it leaves |
| roundings | a box's far side 6e-7 of itself further, boxes kept 4e-6 of it past the nearest hit | [design] without the first, rays aimed at edges and corners slip out of a closed room; without the second a nearer hit can be passed over. A test fails without either |
| tree | 16 bins, leaves of at most 8, halves by count below depth 48 | [design] the stack of a walk is bounded |
| limit of a scene | 2,097,152 triangles of all instances together (`SceneLimits`) | [design] a budget for play: about 288 bytes each while they are prepared, 576 MiB for the triangles alone by their sizes, not measured (the input and its textures come on top), refused before any is made |
| spacing | 64 units | [design] a room's corners and door frames still have points of their own |
| rays a point | 512, on a Fibonacci spiral | [design] fixed directions, the same for every point: no noise from place to place |
| emission | 0.25, at most 1 | [design] counting an unlit surface's whole colour makes windows the brightest lamps of a room |
| flood | 8 points around a seed, then 64; 6 neighbours | [design] a seed in a corner still finds a point; only open ways along the axes count |
| inside geometry | more than 1/4 backs | [design] a point beside one open back still counts |
| the lamps' grid | 32 cells along its longest side, 1 unit at least; a sphere 0.1 % and 1 unit more (of the square), a cell 8 x f32's epsilon of the largest coordinate wider on each side | [design] a ray's end looks at the lamps near it; the additions are for roundings, so that none that adds light is left out |
| furthest seed, lamp | 1e9 spacings, 2^21 units | [design] further off is in no cell, and what the lamps' cells are wider by stays small. A seed there finds nothing, a lamp there is input out of range |
| limits of a gathering | 40,000 points, reached or not; 65,536 seeds (`GatherLimits`) | [design] a budget for play; 1,024 seeds' candidates are held at a time |
| looks at the caller's check (scene) | every 65,536 instances counted and triangles placed, tree nodes of 4,096 triangles and more | [design] cheap against the work between two of them |
| fixed limits | 2^26 triangles, 64 threads, 4,096 rays of a point, 4,096 lamps and as many directionals, 2^24 entries of the lamps' table | [design] no count can overflow |

## When there is nothing

`prepare` and `reach` give a typed failure (`Failed`): cancelled, unavailable with a reason (too
many triangles, points, seeds or lamps, no seeds, nothing reached, a number out of range), or a
fault of the crate itself. A light whose numbers are not finite is refused, a spot without a
direction too. A seed whose numbers are not is one that found nothing. What is degraded but
usable of a scene is left out or replaced, and counted in `SceneStats`: broken triangles, missing
textures, normals, texture coordinates, vertex colours or materials with numbers that are none. A
material whose cutoff is no number is opaque: no light goes through it. Its other numbers become
white and the plain texture coordinates.

## Cancelling

The caller's check is asked every 65,536 instances that are counted, at every instance that is put
in place and every 65,536 of its triangles, at every axis of the tree's larger nodes, and at every texture.
While light is gathered it is asked at every cell of the lamps' table, before every point, before
every group of up to 64 segments of one start, and every 4,096 steps of counting parts. It may be
asked on any thread, and it must be cheap.

What still runs on without a question, at the limits: the arrays of boxes and the copy into the
tree's order for 2,097,152 triangles, a few passes over them on one thread; one pass of the root's
bins; one whole texture; counting what is wrong with a scene; one point of 4,096 rays; a pass over
the seeds, and over the points of one step of the flood. None of these is timed.
