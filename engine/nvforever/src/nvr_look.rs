//! New Vegas Reloaded's look, carried over from the load order's NVR settings
//! (`nvse\plugins\NewVegasReloaded.dll.toml`, the copy the MO2 profile's mods make win;
//! Nuclear Sunset's comes from "Hikikomori NVR").
//!
//! NVR's own shaders are Direct3D 9 programs driven by its DLL and cannot run here
//! (results\REPORT-shaders-survey.md). What a preset says about the look is mostly how far it
//! moves NVR's effects from NVR's own defaults, so that is what is taken over: each value as
//! a ratio to NVR's shipped default ([`NVR_DEFAULTS`]), applied to the engine's equivalent
//! pass (`post.rs`: the enhanced Bloom and Sharpening). Day, night and interior values are
//! blended by the sun's height as NVR picks them [design].
//!
//! Only the world picture's passes are touched: `post.rs` runs in the 3D camera's
//! post-process chain, before the interface is drawn, so the HUD and menus never pass
//! through it.

use crate::post::{PostEffects, BLOOM_INTENSITY, SHARPENING};
use bevy::prelude::*;
use nvf_assets::GameData;

/// Where NVR keeps its settings, relative to Data.
pub const NVR_SETTINGS: &str = "nvse\\plugins\\NewVegasReloaded.dll.toml";

/// NVR's shipped defaults for the values used here [observed: NVR's
/// `NewVegasReloaded.dll.defaults.toml`]: Bloom Strength 1.0 in each of Main, Night and
/// Interiors; Sharpening Strength 0.75, off.
const NVR_DEFAULTS: Defaults = Defaults { bloom: 1.0, sharpen: 0.75, fog: 0.4 };

struct Defaults {
    bloom: f32,
    sharpen: f32,
    /// Volumetric Fog Main Amount.
    fog: f32,
}

/// A ratio to NVR's default is held to this range, so one extreme preset value cannot
/// blow the picture out or switch an effect off [engineering choice].
const RATIO_RANGE: (f32, f32) = (0.25, 3.0);

/// The look read from the NVR settings; all ratios are 1 without them.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct NvrLook {
    /// Bloom strength over NVR's default: day (Main), night, interiors.
    pub bloom: [f32; 3],
    /// Sharpening strength over NVR's default, `None` when NVR's sharpening is off.
    pub sharpen: Option<f32>,
    /// Volumetric Fog Amount (exteriors) over NVR's default; 0 when NVR's fog is off or
    /// there is no NVR, so `height_fog.rs` adds nothing to a game without it.
    pub fog: f32,
    /// NVR's Cinema values when its Cinema is on (`cinema.rs` draws them), else `None`.
    pub cinema: Option<Cinema>,
    /// NVR's Water Lens when on (water drops on the lens after surfacing, `cinema.rs`).
    pub water_lens: Option<WaterLens>,
}

/// NVR's Water Lens settings [observed: NVR's defaults file, `[_Shaders.WaterLens]`: "Short
/// refraction water drop animation on the lens when exiting water"; Hikikomori keeps NVR's
/// defaults: Amount 0.1, Time 180, TimeMultA 0.1, TimeMultB 0.2, Viscosity 0.05]. The file
/// says nothing of the units; how each is used here is ours (`cinema.rs`) [hypothesis].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterLens {
    /// Refraction strength.
    pub amount: f32,
    /// How long the drops last, read as frames at 60 a second.
    pub time: f32,
    /// How fast the drops run down the lens (the two speed factors).
    pub mult_a: f32,
    pub mult_b: f32,
    /// How fast the drops thin out as they run.
    pub viscosity: f32,
}

/// NVR's Cinema settings as the preset gives them, each held to a range [engineering
/// choice] so a stray value cannot cover the picture. What each one means is NVR's own
/// description in its defaults file [observed: `NewVegasReloaded.dll.defaults.toml`
/// `[_Shaders.Cinema.Main]` comments: grain "animated ... at 24fps", chromatic aberration
/// "separates the red/green and blue channels slightly", overlay "overlays the screen image
/// onto itself", vignette darkness and the radius it starts at]. The letterbox (AspectRatio,
/// Mode, LetterBoxDepth) is not taken over: black bars over play are not wanted in this
/// engine, and Hikikomori's Mode 3 is outside the 0-2 the defaults file lists [observed].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cinema {
    pub grain: f32,
    pub aberration: f32,
    pub overlay: f32,
    pub vignette_darkness: f32,
    pub vignette_radius: f32,
}

impl Default for NvrLook {
    fn default() -> Self {
        NvrLook { bloom: [1.0; 3], sharpen: Some(1.0), fog: 0.0, cinema: None, water_lens: None }
    }
}

fn ratio(value: Option<f32>, default: f32) -> f32 {
    value.filter(|v| v.is_finite()).map_or(1.0, |v| (v / default).clamp(RATIO_RANGE.0, RATIO_RANGE.1))
}

/// The value of `key` in `[section]` of NVR's TOML (flat `key = number|bool` lines, `#`
/// comments), as a number (`true`/`false` as 1/0).
fn value(text: &str, section: &str, key: &str) -> Option<f32> {
    let mut inside = false;
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            // NVR's TOML names the sections `_Shaders.…`, Cartographer's presets `Shaders.…`
            // [observed: Hikikomori NVR's `config\Cartographer\NVRPresets\*.ini`].
            inside = name.trim().trim_start_matches('_').eq_ignore_ascii_case(section.trim_start_matches('_'));
            continue;
        }
        if !inside {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        if k.trim().eq_ignore_ascii_case(key) {
            let v = v.trim().trim_matches('"');
            return match v.to_ascii_lowercase().as_str() {
                "true" => Some(1.0),
                "false" => Some(0.0),
                _ => v.parse().ok(),
            };
        }
    }
    None
}

impl NvrLook {
    /// The look of an NVR settings file's text.
    pub fn parse(text: &str) -> Self {
        let bloom_on = value(text, "_Shaders.Bloom.Status", "Enabled").is_none_or(|v| v > 0.5);
        let bloom = |part: &str| if bloom_on { ratio(value(text, &format!("_Shaders.Bloom.{part}"), "Strength"), NVR_DEFAULTS.bloom) } else { 1.0 };
        let sharpen_on = value(text, "_Shaders.Sharpening.Status", "Enabled").is_some_and(|v| v > 0.5);
        NvrLook {
            bloom: [bloom("Main"), bloom("Night"), bloom("Interiors")],
            sharpen: sharpen_on.then(|| ratio(value(text, "_Shaders.Sharpening.Main", "Strength"), NVR_DEFAULTS.sharpen)),
            fog: if value(text, "_Shaders.VolumetricFog.Status", "Enabled").is_some_and(|v| v > 0.5) {
                ratio(value(text, "_Shaders.VolumetricFog.Main", "Amount"), NVR_DEFAULTS.fog)
            } else {
                0.0
            },
            cinema: value(text, "_Shaders.Cinema.Status", "Enabled").is_some_and(|v| v > 0.5).then(|| {
                let get = |key: &str, max: f32| value(text, "_Shaders.Cinema.Main", key).filter(|v| v.is_finite()).unwrap_or(0.0).clamp(0.0, max);
                Cinema {
                    grain: get("FilmGrainAmount", 1.0),
                    aberration: get("ChromaticAberration", 3.0),
                    overlay: get("OverlayStrength", 1.0),
                    vignette_darkness: get("VignetteDarkness", 3.0),
                    vignette_radius: get("VignetteRadius", 2.0),
                }
            }),
            water_lens: value(text, "_Shaders.WaterLens.Status", "Enabled").is_some_and(|v| v > 0.5).then(|| {
                let get = |key: &str, default: f32, max: f32| value(text, "_Shaders.WaterLens.Main", key).filter(|v| v.is_finite()).unwrap_or(default).clamp(0.0, max);
                WaterLens { amount: get("Amount", 0.1, 1.0), time: get("Time", 180.0, 1200.0), mult_a: get("TimeMultA", 0.1, 2.0), mult_b: get("TimeMultB", 0.2, 2.0), viscosity: get("Viscosity", 0.05, 1.0) }
            }),
        }
    }

    /// The bloom ratio for a view with `day` (0 night .. 1 day) outdoors, or indoors.
    pub fn bloom_now(&self, day: f32, interior: bool) -> f32 {
        if interior {
            return self.bloom[2];
        }
        let d = day.clamp(0.0, 1.0);
        self.bloom[1] + (self.bloom[0] - self.bloom[1]) * d
    }
}

/// Cartographer's choice of NVR preset per place [observed: Hikikomori NVR's
/// `config\Cartographer\NVR.ini`: `[Main]` `Exterior=Default`, `Interior=Default`, and
/// `[Overrides]` `<worldspace EditorID>=<preset>,<flags>…`, e.g. `Wasteland=DCWorld`,
/// `DLC01PittWorld=ThePitt`; each preset is `config\Cartographer\NVRPresets\<preset>.ini`
/// with NVR's settings]. The flags after the preset name are not taken over.
const CARTOGRAPHER_MAP: &str = "config\\Cartographer\\NVR.ini";
const CARTOGRAPHER_PRESETS: &str = "config\\Cartographer\\NVRPresets";

/// The preset Cartographer names for a worldspace (or an interior), from NVR.ini's text.
pub fn cartographer_preset(map: &str, world: Option<&str>, interior: bool) -> Option<String> {
    let mut section = String::new();
    let mut main = None;
    let mut over = None;
    for line in map.lines() {
        let line = line.split(';').next().unwrap_or("").trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name.trim().to_ascii_lowercase();
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let preset = v.split(',').next().unwrap_or("").trim();
        if preset.is_empty() {
            continue;
        }
        match section.as_str() {
            "main" if k.trim().eq_ignore_ascii_case(if interior { "Interior" } else { "Exterior" }) => main = Some(preset.to_string()),
            "overrides" if !interior && world.is_some_and(|w| k.trim().eq_ignore_ascii_case(w)) => over = Some(preset.to_string()),
            _ => {}
        }
    }
    over.or(main)
}

/// The NVR settings texts the look is made from: NVR's own, and Cartographer's map.
#[derive(Resource, Default)]
struct NvrSources {
    base: Option<String>,
    map: Option<String>,
}

pub struct NvrLookPlugin;

impl Plugin for NvrLookPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, (load, follow_place, apply).chain().after(crate::post::image_space_glow));
    }
}

/// Where the camera is, as Cartographer chooses by it: (worldspace EditorID, interior).
fn place_of(world: Option<&crate::world::WorldData>, sky: Option<&crate::sky::SkyState>) -> (Option<String>, bool) {
    let interior = sky.is_some_and(|s| s.interior.is_some());
    let edid = world.and_then(|w| w.index.worldspaces.get(&w.world)).and_then(|ws| ws.edid.clone());
    (edid, interior)
}

/// Cartographer's preset for where the camera is, put in place whenever the place changes;
/// NVR's own settings where Cartographer names none or its preset is missing.
fn follow_place(
    mut commands: Commands,
    data: Option<Res<GameData>>,
    sources: Option<Res<NvrSources>>,
    world: Option<Res<crate::world::WorldData>>,
    sky: Option<Res<crate::sky::SkyState>>,
    mut last: Local<Option<(Option<String>, bool)>>,
) {
    let (Some(data), Some(sources)) = (data, sources) else { return };
    let Some(map) = sources.map.as_deref() else { return };
    let place = place_of(world.as_deref(), sky.as_deref());
    if last.as_ref() == Some(&place) {
        return;
    }
    *last = Some(place.clone());
    let preset = cartographer_preset(map, place.0.as_deref(), place.1);
    let text = preset.as_ref().and_then(|p| data.read(&format!("{CARTOGRAPHER_PRESETS}\\{p}.ini"))).map(|b| String::from_utf8_lossy(&b).into_owned());
    let (look, from) = match (&text, &sources.base) {
        (Some(t), _) => (NvrLook::parse(t), format!("Cartographer preset {}", preset.as_deref().unwrap_or("?"))),
        (None, Some(b)) => (NvrLook::parse(b), NVR_SETTINGS.to_string()),
        (None, None) => return,
    };
    info!(
        "nvr look: {} {} uses {from}: bloom day x{:.2}, night x{:.2}, interiors x{:.2}; sharpening {}; height fog x{:.2}; cinema {}",
        if place.1 { "interior in" } else { "worldspace" },
        place.0.as_deref().unwrap_or("?"),
        look.bloom[0],
        look.bloom[1],
        look.bloom[2],
        look.sharpen.map_or("off".into(), |s| format!("x{s:.2}")),
        look.fog,
        if look.cinema.is_some() { "on" } else { "off" }
    );
    commands.insert_resource(look);
}

/// Read the NVR settings once the game's data is there.
fn load(mut commands: Commands, data: Option<Res<GameData>>, look: Option<Res<NvrLook>>) {
    if look.is_some() {
        return;
    }
    let Some(data) = data else { return };
    // `NVF_NVR_LOOK=off`: the engine's own look, for comparing.
    if std::env::var("NVF_NVR_LOOK").is_ok_and(|v| v.eq_ignore_ascii_case("off")) {
        info!("nvr look: off (NVF_NVR_LOOK)");
        commands.insert_resource(NvrLook::default());
        return;
    }
    let map = data.read(CARTOGRAPHER_MAP).map(|b| String::from_utf8_lossy(&b).into_owned());
    if map.is_some() {
        info!("nvr look: Cartographer's {CARTOGRAPHER_MAP} picks the NVR preset per worldspace");
    }
    let base = data.read(NVR_SETTINGS).map(|b| String::from_utf8_lossy(&b).into_owned());
    commands.insert_resource(NvrSources { base: base.clone(), map });
    let look = match base.map(String::into_bytes) {
        Some(bytes) => {
            let look = NvrLook::parse(&String::from_utf8_lossy(&bytes));
            info!("nvr look: from {NVR_SETTINGS}: bloom day x{:.2}, night x{:.2}, interiors x{:.2}; sharpening {}; height fog x{:.2}; cinema {}", look.bloom[0], look.bloom[1], look.bloom[2], look.sharpen.map_or("off".into(), |s| format!("x{s:.2}")), look.fog, look.cinema.map_or("off".into(), |c| format!("grain {:.2}, aberration {:.2}, overlay {:.2}, vignette {:.2}/{:.2}", c.grain, c.aberration, c.overlay, c.vignette_darkness, c.vignette_radius)));
            info!("nvr look: water lens {}", look.water_lens.map_or("off".into(), |w| format!("amount {:.2}, {:.0} frames", w.amount, w.time)));
            look
        }
        None => {
            info!("nvr look: no {NVR_SETTINGS} in the load order; the engine's own look");
            NvrLook::default()
        }
    };
    commands.insert_resource(look);
}

/// Put the look on the main cameras every frame: NVR's Bloom replaces the game's own bloom,
/// so its ratio scales the light glow (Bevy's bloom, its strength set from the image space
/// just before, `post::image_space_glow`) and the enhanced Bloom when that is on; NVR's
/// Sharpening scales the enhanced Sharpening (`graphics.rs` makes [`PostEffects`] anew with
/// the engine's own strengths whenever the settings change).
fn apply(
    look: Option<Res<NvrLook>>,
    sky: Option<Res<crate::sky::SkyState>>,
    mut cams: Query<&mut PostEffects>,
    mut glows: Query<&mut bevy::post_process::bloom::Bloom, With<crate::post::ImageSpaceGlow>>,
) {
    let Some(look) = look else { return };
    let (day, interior) = sky.as_deref().map_or((1.0, false), |s| (smoothstep(-2.0, 3.0, s.sun_elevation()), s.interior.is_some()));
    let ratio = look.bloom_now(day, interior);
    // `image_space_glow` writes the strength afresh every frame it has the sky (and only
    // then), so it is scaled once per frame on top of that, never twice.
    if sky.is_some() {
        for mut b in &mut glows {
            b.intensity *= ratio;
        }
    }
    let bloom = BLOOM_INTENSITY * ratio;
    // NVR's sharpening off: the engine's own sharpening is the player's setting, kept.
    let sharpen = SHARPENING * look.sharpen.unwrap_or(1.0);
    for mut e in &mut cams {
        let mut p = e.params;
        if p.z > 0.0 {
            p.z = bloom;
        }
        if p.w > 0.0 {
            p.w = sharpen.min(1.0);
        }
        if p != e.params {
            e.params = p;
        }
    }
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HIKIKOMORI: &str = "[_Shaders.Bloom]\n\n[_Shaders.Bloom.Interiors]\nStrength = 2.3\n\n[_Shaders.Bloom.Main]\nBlending = 0.1\nStrength = 0.4 # day\n\n[_Shaders.Bloom.Night]\nStrength = 5.0\n\n[_Shaders.Bloom.Status]\nEnabled = true\n\n[_Shaders.Sharpening.Main]\nStrength = 1.0\n\n[_Shaders.Sharpening.Status]\nEnabled = true\n";

    #[test]
    fn an_nvr_preset_becomes_ratios_to_nvrs_defaults() {
        let look = NvrLook::parse(HIKIKOMORI);
        assert!((look.bloom[0] - 0.4).abs() < 1e-6);
        assert_eq!(look.bloom[1], RATIO_RANGE.1, "night 5.0 is held to the range");
        assert!((look.bloom[2] - 2.3).abs() < 1e-6);
        assert!((look.sharpen.unwrap() - 1.0 / 0.75).abs() < 1e-6);
        // Day and night blend by the sun; interiors take their own.
        assert!((look.bloom_now(1.0, false) - 0.4).abs() < 1e-6);
        assert!((look.bloom_now(0.0, false) - 3.0).abs() < 1e-6);
        assert!((look.bloom_now(0.0, true) - 2.3).abs() < 1e-6);
    }

    #[test]
    fn cartographer_picks_the_preset_by_worldspace_and_reads_its_sections() {
        let map = "[Main]\nExterior=Default\nInterior=Default\n\n[Overrides]\nWastelandNV=Default,NoPBR,Performance\n;TTW\nWasteland=DCWorld,NoPBR,Performance\nDLC01PittWorld=ThePitt,NoPBR\n";
        assert_eq!(cartographer_preset(map, Some("wasteland"), false).as_deref(), Some("DCWorld"));
        assert_eq!(cartographer_preset(map, Some("DLC01PittWorld"), false).as_deref(), Some("ThePitt"));
        assert_eq!(cartographer_preset(map, Some("SomewhereElse"), false).as_deref(), Some("Default"));
        assert_eq!(cartographer_preset(map, Some("Wasteland"), true).as_deref(), Some("Default"), "interiors take [Main] Interior");
        // A preset's sections have no leading underscore.
        let pitt = "[Shaders.Bloom.Main]\nStrength=0.0\n[Shaders.Bloom.Status]\nEnabled=true\n";
        assert_eq!(NvrLook::parse(pitt).bloom[0], RATIO_RANGE.0, "Pitt's bloom 0 is held to the range");
    }

    #[test]
    fn missing_or_switched_off_nvr_settings_change_nothing() {
        assert_eq!(NvrLook::parse(""), NvrLook { bloom: [1.0; 3], sharpen: None, fog: 0.0, cinema: None, water_lens: None });
        let lens = "[_Shaders.WaterLens.Main]\nAmount = 0.1\nTime = 180.0\nTimeMultA = 0.1\nTimeMultB = 0.2\nViscosity = 0.05\n[_Shaders.WaterLens.Status]\nEnabled = true\n";
        assert_eq!(NvrLook::parse(lens).water_lens, Some(WaterLens { amount: 0.1, time: 180.0, mult_a: 0.1, mult_b: 0.2, viscosity: 0.05 }));
        assert_eq!(NvrLook::parse(&lens.replace("true", "false")).water_lens, None);
        let fog = "[_Shaders.VolumetricFog.Main]\nAmount = 1.553\n[_Shaders.VolumetricFog.Status]\nEnabled = true\n";
        assert_eq!(NvrLook::parse(fog).fog, RATIO_RANGE.1, "Hikikomori's 1.553 over 0.4 is held to the range");
        assert_eq!(NvrLook::parse(&fog.replace("true", "false")).fog, 0.0);
        let off = HIKIKOMORI.replace("[_Shaders.Bloom.Status]\nEnabled = true", "[_Shaders.Bloom.Status]\nEnabled = false");
        assert_eq!(NvrLook::parse(&off).bloom, [1.0; 3]);
        assert_eq!(value("[A]\nx = true\n[B]\nx = 2\n", "b", "X"), Some(2.0));
    }

    #[test]
    fn nvr_cinema_is_read_only_when_on() {
        // Hikikomori NVR's Cinema [observed: its NewVegasReloaded.dll.toml].
        let on = "[_Shaders.Cinema.Main]\nAspectRatio = 2.05\nChromaticAberration = 1.0\nFilmGrainAmount = 0.3\nLetterBoxDepth = 0.0\nMode = 3\nOverlayStrength = 0.2\nVignetteDarkness = 0.0\nVignetteRadius = 0.0\n\n[_Shaders.Cinema.Status]\nEnabled = true\n";
        let c = NvrLook::parse(on).cinema.expect("on");
        assert_eq!(c, Cinema { grain: 0.3, aberration: 1.0, overlay: 0.2, vignette_darkness: 0.0, vignette_radius: 0.0 });
        assert_eq!(NvrLook::parse(&on.replace("Enabled = true", "Enabled = false")).cinema, None);
        assert_eq!(NvrLook::parse(&on.replace("FilmGrainAmount = 0.3", "FilmGrainAmount = 40")).cinema.unwrap().grain, 1.0, "held to its range");
    }
}
