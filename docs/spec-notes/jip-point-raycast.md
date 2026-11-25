# JIP LN `GetPointRayCastPos` direction angles and miss output

Plain-language notes for `GetPointRayCastPos` in `crates/fnv-game/src/nvse_jip3.rs`.
No JIP LN DLL or implementation source was consulted.

## 1. Signature and documented behavior

- `(success:bool) GetPointRayCastPos originX originY originZ rotationX rotationY xOut yOut zOut [collisionFlag] [maxRange]`
  casts a ray from `(originX, originY, originZ)` in the direction defined by `(rotationX, rotationY)`
  (degrees) up to `maxRange` and writes the resulting world-space coordinates into the three
  output script variables `(xOut, yOut, zOut)`, returning `1` on hit and `0` on miss
  [documented: GECK wiki
  [GetPointRayCastPos](https://geckwiki.com/index.php?title=GetPointRayCastPos);
  `schemas/script/functions.json`].
- Original-game capture probes (`ray1` / `ray2`, `tools/capture/jip-probes/ray1.steps.json`)
  were previously unable to compile script-variable output arguments in the runtime probe
  harness, leaving the angle signs and no-hit output values uncaptured.

## 2. Direction angle convention and no-hit endpoint

- **Direction angles `[hypothesis]`:** `GetPointRayCastPos` uses right-handed Euler rotation
  `Rz(rotZ) · Rx(rotX) · +Y = (-sin(rotZ)·cos(rotX), cos(rotZ)·cos(rotX), sin(rotX))`
  (equivalent to `facing(-rotX, -rotZ)` — pitch positive up, yaw counter-clockwise from `+Y`),
  rather than `GetAngle`'s down-positive pitch and clockwise heading.
  - Public mod scripts rely on this convention:
    - B42 True Leaning (v0.20b, `ln_B42Leaning.txt`, lines 202 and 255) passes
      `-Player.GetAngle X` as `rotX` and `-player.getangle Z + (80 * xlH)` as `rotZ`
      (with `xlH = +1` for leaning left and `xlH = -1` for leaning right).
    - Aim Collision Indicator (`GameLoop.gek`, lines 46, 59, and 72) passes the intrinsic
      world Euler angles returned by `GetNifRotAlt "Bip01 Looking" ... 3 2` directly for the
      forward ray and `(fBlockRotX * -1)` / `(fBlockRotZ - 180)` for the backward ray.
- **No-hit output coordinates `[hypothesis]`:** When the ray does not hit geometry within
  `maxRange`, `GetPointRayCastPos` returns `0` (`false`) and writes the ray endpoint
  `origin + dir * maxRange` into `(xOut, yOut, zOut)` rather than `(0, 0, 0)`.
  - Both B42 True Leaning (`ln_B42Leaning.txt`, lines 255–261) and Aim Collision Indicator
    (`GameLoop.gek`, lines 72–77) unconditionally compute
    `GetVector3DDistance xOut yOut zOut originX originY originZ` without branching on the
    boolean return value and expect a distance equal to `maxRange` when nothing is hit
    (checking `xlM > 150` / `(xlM - 20) / 20` and `fDistance > (fDistanceMax - 0.1)`).
    Writing `(0, 0, 0)` caused `GetVector3DDistance` to measure the distance to world origin,
    blocking clearance and corner checks in interior cells near `(0, 0, 0)`.
- **Collision flags:** `CastKind::World` (excluding the player) is still used regardless of
  `collisionFlag` `[hypothesis]`; default reach when `maxRange` is omitted or `<= 0` remains
  `50_000.0` game units `[hypothesis]`.
- **Follow-up native probe:** Confirm `rotX`/`rotZ` signs (e.g. `-90 0` down vs `90 0` up,
  `0 -90` east vs `0 90` west) and `(xOut, yOut, zOut)` on miss in the original game with a
  compiled script-variable probe.
