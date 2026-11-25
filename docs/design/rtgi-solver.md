# The bounce-light solver (`engine/nvf-rtgi`, #3009)

The plan is a crate that works out the light that an interior's lamps throw back from its
surfaces, as a small 3D texture, on the CPU and without Bevy. It is built in parts, each with
tests on scenes it makes itself. **Built: part 1, the ray scene.** Nothing in the game calls it
yet: there is no setting, no upload and no shader. It ships no data.

## The whole

The bounced light is to take the place of a part of the flat ambient, fitted so that the mean
brightness of a room stays what it was. Interiors only, off by default, one bounce, no ray tracing hardware. The
parts, in order:

1. **The ray scene** (this note): the triangles of a cell and what a ray meets.
2. The light: the engine's light sum at a place, the bounce gathered around a point in the air,
   and the grid of points that can be reached.
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
| looks at the caller's check | every 65,536 instances counted and triangles placed, tree nodes of 4,096 triangles and more | [design] cheap against the work between two of them |
| fixed limits | 2^26 triangles, 64 threads | [design] no count can overflow |

## When there is nothing

`prepare` gives a typed failure (`Failed`): cancelled, unavailable with a reason (too many
triangles, a thread count out of range), or a fault of the crate itself. Input that is degraded
but usable is left out or replaced, and counted in `SceneStats`: broken triangles, missing
textures, normals, texture coordinates, vertex colours or materials with numbers that are none. A
material whose cutoff is no number is opaque: no light goes through it. Its other numbers become
white and the plain texture coordinates.

## Cancelling

The caller's check is asked every 65,536 instances that are counted, at every instance that is put
in place and every 65,536 of its triangles, at every axis of the tree's larger nodes, and at every texture. It may be asked on any
thread, and it must be cheap.

What still runs on without a question, at the limits: the arrays of boxes and the copy into the
tree's order for 2,097,152 triangles, a few passes over them on one thread; one pass of the root's
bins; one whole texture; counting what is wrong with a scene. None of these is timed.
