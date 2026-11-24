# BetterFlashlightNVSE INI spotlight configuration

**Clean-room provenance:** Only the public **Better Flashlight NVSE** mod page
(`https://www.nexusmods.com/newvegas/mods/98905`, v1.1.6) and shipped
`Data\Config\BetterFlashlightNVSE.ini` file (section names, key names, defaults, and
inline comments) were consulted. No DLL (`BetterFlashlightNVSE.dll`), PDB, compiled
shader (`.fxo`), or third-party binary or source code was opened, disassembled, or
consulted.

## VFS discovery and vanilla-unchanged gate

(`engine/nvforever/src/flashlight_ini.rs`, `engine/nvforever/src/flashlight.rs`; extends
the native player spotlight described in `world.md` §8.)

- At `Startup`, NVForever looks up `config\betterflashlightnvse.ini` case-insensitively
  through `nvf_assets::GameData::read` (covering loose files, MO2 layered roots, and
  BSAs) [documented: Better Flashlight NVSE v1.1.6
  (`https://www.nexusmods.com/newvegas/mods/98905`), shipped
  `Data\Config\BetterFlashlightNVSE.ini` install path].
- When the INI is absent, or when `[Main] RenderEffects = 0` or `[Flashlight] Enabled = 0`
  [documented: Better Flashlight NVSE v1.1.6 `Data\Config\BetterFlashlightNVSE.ini`
  `[Main]` and `[Flashlight]` (`;disable to turn the mod off entirely`)], NVForever
  leaves its vanilla Pip-Boy `SpotLight` (`COLOUR`, `BRIGHTNESS`, `RANGE`, `OUTER_DEG`,
  `INNER_DEG`, head/eye `placement`) and toggle behavior completely unchanged [design].
- Parsing operates strictly on raw `&[u8]` slices (stripping optional UTF-8 BOM,
  CRLF/LF endings, `;`/`#` comments, ASCII case-insensitive section/key matching, and
  finite `f32` validation) with safe engineering clamps (`Angle * SpotSize` clamped to
  `1°..=85°` strictly below Bevy's `π/2` `SpotLight` limit; `SpotSize` `0.1..=4.0`;
  `Distance` `64..=20000`; `ColorR/G/B` `0..=4`; `Dimmer`/`Intensity` `0..=16`;
  `WarmupTime` `0..=10 s`; `Flicker` `0..=2`; `OffsetX/Y/Z` `-200..=200`; `[Battery]`
  rates/threshold `0..=100`) and omitted-key fallback defaults (`SpotSize = 1.0`,
  `[Battery] Enabled = false`, and the shipped INI's numeric baseline for omitted
  spotlight/battery fields) [design].

## Wired sections and keys

Every stored field has a live consumer in `engine/nvforever/src/flashlight.rs`:

- `[Main] RenderEffects` and `[Flashlight] Enabled`: master activation gate
  [documented: Better Flashlight NVSE v1.1.6 `Data\Config\BetterFlashlightNVSE.ini`].
- `[Flashlight] Angle`, `SpotSize`, `Distance`: mapped onto native Bevy `SpotLight`
  `outer_angle` (`Angle * SpotSize`, treating `Angle` as base cone half-angle in degrees
  [hypothesis]), `inner_angle` preserving NVForever's vanilla `14 / 30` softness ratio
  [design], and `range` (`Distance`, game units) [documented: Better Flashlight NVSE
  v1.1.6 `Data\Config\BetterFlashlightNVSE.ini` (`;light cone`,
  `;spot size scale - shrinks the pool (cone + cookie together). 1.0 = full Angle, lower = tighter`)].
  `configured_spotlight` preserves `shadow_maps_enabled = true` and
  `shadow_map_near_z = NEAR` so native wall shadow occlusion and `#3121` first-person
  anti-washout scaling (`lights/display_shader.rs`) continue to apply [design].
- `[Flashlight] ColorR`, `ColorG`, `ColorB`: passed as HDR-capable display colour
  multipliers to `lights::display_color` [documented: Better Flashlight NVSE v1.1.6
  `Data\Config\BetterFlashlightNVSE.ini` (`;color (values above 1.0 are HDR)`);
  design: native display-unit mapping].
- `[Interior] Dimmer`, `Intensity` and `[Exterior] Dimmer`, `Intensity` (with
  `[Flashlight] Dimmer`, `Intensity` as legacy per-field fallback): dynamically chosen
  by current cell interior state (`nvf_play::Location::interior`, fallback
  `SkyState::interior`) and mapped to display brightness
  `BRIGHTNESS * dimmer * intensity * warmup * flicker` via `lights::lumens`
  [documented: Better Flashlight NVSE v1.1.6 `Data\Config\BetterFlashlightNVSE.ini`
  (`;legacy fallback for configs created before interior/exterior profiles`,
  `;brightness used in interior cells`, `;brightness used in exterior cells`);
  design: linear product mapping into NVForever display units].
- `[Flashlight] WarmupTime` and `Flicker`: incandescent smoothstep ramp over
  `WarmupTime` seconds (`0` = instant) and deterministic sporadic brief brightness dips
  bounded in `0.15..=1.0` whose frequency and depth scale with `Flicker` (`0` = steady)
  [documented: Better Flashlight NVSE v1.1.6 `Data\Config\BetterFlashlightNVSE.ini`
  (`;sporadic flicker - mostly steady with occasional brief dips. 0 = off, higher = deeper/more often`,
  `;warm-up ramp on toggle (seconds). incandescent feel. 0 = instant (LED)`);
  design: smoothstep ramp and SplitMix64 0.08 s time-slot dip curve].
- `[Flashlight] AttachToWeapon` and `OffsetX`, `OffsetY`, `OffsetZ`: when enabled and a
  weapon is drawn in hand, aligns the spotlight to the weapon's `ProjectileNode` muzzle
  and `+X` barrel axis (`PlayerBody::weapon_aim`, falling back to `WeaponAnim::muzzle`
  with view rotation, or camera/head-eye placement when unarmed/holstered or when
  `AttachToWeapon = 0`) [documented: Better Flashlight NVSE v1.1.6
  `Data\Config\BetterFlashlightNVSE.ini` (`;light follows weapon when armed, otherwise camera`,
  `;position offset relative to attach point (weapon or camera)`); design: native
  weapon-node alignment], then applies `(OffsetX, OffsetY, OffsetZ)` in the attach
  point's local game-space `(right, forward, up)` frame [hypothesis: FNV local axis
  convention for `OffsetX/Y/Z`, since the INI comment does not specify individual axis
  orientations].
- `[Battery] Enabled`, `DrainRate`, `RechargeRate`, `RechargeThreshold`: while actively
  playing (`PlayState::Playing` and not `Paused`), drains charge (`100 -> 0`) at
  `DrainRate` units/s when on, automatically shuts off at `0` and locks out re-toggling
  until recharged past `RechargeThreshold` (when `> 0`), recharges at `RechargeRate`
  units/s while off, and resets to `100%` (clearing lockout) on save load or new game
  [documented: Better Flashlight NVSE v1.1.6 `Data\Config\BetterFlashlightNVSE.ini`
  (`;drains while the Pip-Boy flashlight is on, recharges while off. Charge resets to full on load/new game.`,
  `;charge needed after full depletion before the flashlight can turn on again`);
  design: native `FlashlightRuntime` state machine without altering the save format].

## Intentionally unbuilt / out-of-scope sections and keys

Skipped during parsing with no dead config fields:

- `[Hotkeys]` (`DimmerIncrease`, `DimmerDecrease`, `IntensityIncrease`,
  `IntensityDecrease`): DirectInput scan-code hotkeys and live INI saving are not built.
- `[EngineLight]` (`Enabled`, `Dimmer`, `Specular`, `Atten0`, `Atten1`, `Atten2`):
  separate Gamebryo `NiPointLight` attenuation pass is not built.
- `[MaterialLight]` (`Enabled`, `Render`, `Intensity`, `Specular`, `SpecularPower`,
  `SpecularBoost`, `NormalStrength`, `MaxGeometry`, `DebugMode`, `LogProjection`):
  D3D9 additive geometry pass is not built; Bevy's native `SpotLight` already lights
  world materials natively.
- `[Volumetric]` (`Enabled`, `FirstPersonEnabled`, `ThirdPersonEnabled`, `Strength`,
  `Anisotropy`, `HeightFalloff`, `MaxDistance`, `FirstPersonStrength`,
  `FirstPersonOffsetRight`, `FirstPersonOffsetDown`): screen-space single-scattering
  volumetric shaft pass is not built.
- `[Flashlight]` D3D9 post-process / interop keys (`SoftEdges`, `DisableNVRFlashlight`,
  `RenderShadows` — marked stubbed in v1 by the INI itself, while NVForever relies on
  native `SpotLight` shadow maps for wall occlusion and `#3121` first-person anti-washout
  scaling — `NearFade`, `HotspotLimit`, `CookieStrength`) are not built.
- `[Battery]` HUD icon placement keys (`IconPosX`, `IconPosY`, `IconScale`,
  `PulseThreshold`, `IconSet`) are not built.
