//! Wind-blown dust: the rules (docs/design/volumetric-medium.md, "Wind-blown dust: the
//! rules"; #3032 stage 4). [design] An addition of NVForever's own, not a fact about the
//! original game, never a parity item and never used to explain a difference from it.
//!
//! **Nothing is drawn by this module and nothing runs it yet**: it is the first slice, the
//! rules and their small state, tested here. The next slice puts [`WindDust`] in a resource
//! and gives its [`DustFrame`] to the air's grid. Until then only the tests use it, so the
//! dead code lint is off outside them (the attribute below goes with the first caller).
//!
//! - **How much** ([`strength_from_wind`]): the weather's wind byte, blended through a weather
//!   fade as the trees' wind is (`tree::wind_vector`). 0 on a calm day, 1 at the common wind,
//!   3 in strong wind.
//! - **None while drops fall** ([`dry_share`], [`precipitation_falls`], [`Drops`]): it thins
//!   out before a weather's precipitation starts, is 0 while any falls or any drop of any
//!   precipitation placed so far is still in the air, and comes back slowly after the last.
//! - **No jump** ([`Follow`]): the wind's strength glides, and the amount rises by at most
//!   [`RISE_RATE`] a second whatever moved its target (the end of rain, a forced weather, a
//!   turned-round fade).
//! - **Where it goes** ([`wind_velocity`], [`Drift`]): the sum of the wind's steps, so a turn
//!   changes the way from then on and never the place.
//! - **Colour** ([`dust_colour`]): the weather's fog colour tinted by the hue of its light.
//! - **Weather values only**: no form IDs, no editor IDs and no file names decide anything, so
//!   a mod's weather gets what its values say. What is not a finite number counts as none.

#![cfg_attr(not(test), allow(dead_code))]

use crate::sky::SkyState;
use crate::weather_fx::{fade_share_on, Role};
use bevy::prelude::*;
use fnv_world::sky::{ColorType, Weather};

/// The strength from the wind speed byte: straight lines through these knots, the last held
/// above its byte [design, start values: the common 50 is 1 and 127 is 3, the ends of the
/// useful range in the prototype's stills, where 1 was barely seen, 3 was clearly dust and 8
/// heavy [observed: our own frames, 2026-11-25]. Most weathers store 50, HVDustStorm 127 and
/// NVHooverFinalBattle 130 [observed: `fnv_world::sky::Weather::wind_speed`]; the calm end
/// (none to 20) is chosen].
pub const STRENGTH_KNOTS: [(f32, f32); 4] = [(0.0, 0.0), (20.0, 0.0), (50.0, 1.0), (127.0, 3.0)];
/// The most the amount can be: the last knot.
pub const MOST: f32 = STRENGTH_KNOTS[STRENGTH_KNOTS.len() - 1].1;
/// How fast the amount may rise, a second: an ordinary day's dust is back 10 s after the last
/// drop, a strong wind's after 30 [design, start value].
pub const RISE_RATE: f32 = 0.1;
/// How fast the amount may fall, a second, when nothing cuts it to 0 at once: the whole range
/// in a second, quicker than an ordinary fade into rain asks for [design, start value].
pub const FALL_RATE: f32 = 3.0;
/// A longer frame counts as this many seconds, so a hitch is no jump [design].
pub const LONGEST_STEP: f32 = 0.25;
/// How fast the dust goes in a wind of byte 255, units a second: 118 at the common 50
/// [design, start value: about 1.7 m/s at 70 units a metre].
pub const SPEED_AT_FULL: f32 = 600.0;
/// The time constant with which the dust's velocity follows the wind's, seconds [design: a
/// script's SetWindDirection is instant].
pub const TURN_SECONDS: f32 = 2.0;
/// [`Drift`] wraps after this many units, where both octaves of the patches must repeat: 48
/// cells of 2,000 and 160 cells of 600 [design; the cells are the next slice's start values].
pub const DRIFT_PERIOD: f64 = 96_000.0;
/// A shared system's drops are waited for this long after their longest life, for the births
/// of the frame that took the model down [design].
pub const LIFE_MARGIN: f32 = 0.25;
/// The longest life taken for a shared system's drops when it is not known, and the longest
/// they are waited for at all [design].
pub const UNKNOWN_LIFE: f32 = 30.0;
pub const LONGEST_WAIT: f32 = 120.0;
/// WTHR DATA classification bits [documented: xEdit; `fnv_world::sky::Weather`].
const RAINY: u8 = 4;
const SNOW: u8 = 8;

/// `x` held to `lo..hi`; `lo` when it is not a finite number.
fn held(x: f32, lo: f32, hi: f32) -> f32 {
    if x.is_finite() { x.clamp(lo, hi) } else { lo }
}

/// A frame's length as the rules take it: 0 (nothing moves) when it is not a finite number.
fn step(dt: f32) -> f32 {
    held(dt, 0.0, LONGEST_STEP)
}

/// How far the weather has faded in, 0..1.
fn progress(sky: &SkyState) -> f32 {
    held(sky.fade, 0.0, 1.0)
}

/// The strength (0..[`MOST`]) a wind speed byte asks for; it need not be whole (two weathers'
/// blended through a fade).
pub fn strength_from_wind(byte: f32) -> f32 {
    let byte = held(byte, 0.0, 255.0);
    for pair in STRENGTH_KNOTS.windows(2) {
        let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
        if byte <= x1 {
            return y0 + (y1 - y0) * (byte - x0) / (x1 - x0);
        }
    }
    MOST
}

/// The wind speed byte now, the two weathers' blended through the fade: as
/// `tree::wind_vector` has it, but not rounded to its thousandths and the same in an
/// interior. The weather outside goes on while the player is inside and the dust with it, so
/// it stands where it should when they come out [design].
pub fn wind_byte(sky: &SkyState) -> f32 {
    let now = f32::from(sky.weather.wind_speed);
    match &sky.prev {
        Some(p) => {
            let old = f32::from(p.wind_speed);
            old + (now - old) * progress(sky)
        }
        None => now,
    }
}

/// A weather's share of the dust (0 or 1): none with the Rainy flag, classified Rainy or Snow,
/// or with a precipitation model. The flag and the classes cut it without a model too: Honest
/// Hearts' rain has none and is drawn by a script [documented: docs/spec-notes/world.md §6].
pub fn dry_share(w: &Weather) -> f32 {
    if w.rainy || w.classification & (RAINY | SNOW) != 0 || w.precipitation_model.is_some() {
        0.0
    } else {
        1.0
    }
}

/// Whether a weather's own precipitation falls now, by weather_fx.rs's rule: the one shown or
/// coming in from its begin-fade-in byte, the one going out until its end-fade-out byte
/// [hypothesis there: from the fields' names]. Drops still in the air are [`Drops`]'.
pub fn precipitation_falls(sky: &SkyState) -> bool {
    let t = progress(sky);
    let falls = |w: &Weather, role| w.precipitation_model.is_some() && fade_share_on(w.precipitation_fade, role);
    match &sky.prev {
        Some(p) => falls(&sky.weather, Role::Incoming(t)) || falls(p, Role::Outgoing(t)),
        None => falls(&sky.weather, Role::Steady),
    }
}

/// The weathers' dry share now (0..1): [`dry_share`] blended through the fade; 0 while either's
/// precipitation falls, reached in a straight line from the fade's start to the incoming
/// precipitation's start and left from the outgoing one's stop to the fade's end [design]. A
/// byte that leaves no room (begin 0, end 255) lets it go or come at once, as the rain does;
/// [`Follow`] then still lets it come back slowly.
pub fn dry_through_fade(sky: &SkyState) -> f32 {
    let now = dry_share(&sky.weather);
    let Some(p) = &sky.prev else { return now };
    if precipitation_falls(sky) {
        return 0.0;
    }
    let t = progress(sky);
    let mut dry = dry_share(p) + (now - dry_share(p)) * t;
    if sky.weather.precipitation_model.is_some() {
        let begin = f32::from(sky.weather.precipitation_fade.0) / 255.0;
        dry *= if begin > 0.0 { (1.0 - t / begin).clamp(0.0, 1.0) } else { 0.0 };
    }
    if p.precipitation_model.is_some() {
        let end = f32::from(p.precipitation_fade.1) / 255.0;
        dry *= if end < 1.0 { ((t - end) / (1.0 - end)).clamp(0.0, 1.0) } else { 0.0 };
    }
    held(dry, 0.0, 1.0)
}

/// What the weather asks for this frame, before anything follows it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Asked {
    /// [`strength_from_wind`] of [`wind_byte`], 0..[`MOST`].
    pub strength: f32,
    /// [`dry_through_fade`], 0..1.
    pub dry: f32,
    /// No dust at all, at once: precipitation falls, or a wet weather is shown with no fade
    /// left (a script's ForceWeather is instant).
    pub cut: bool,
}

/// [`Asked`] by the sky as it is.
pub fn asked(sky: &SkyState) -> Asked {
    let dry = dry_through_fade(sky);
    let cut = precipitation_falls(sky) || (sky.prev.is_none() && dry <= 0.0);
    Asked { strength: strength_from_wind(wind_byte(sky)), dry, cut }
}

/// Whose drops fall, by what particles.rs keeps a system under (its `Source`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DropSource {
    /// The precipitation model's own systems, by its path. Their count is trusted alone.
    Model(String),
    /// An add-on node of it (a child emitter), by its node index. Every emitter of one index
    /// feeds one system (particles.rs `Key`), so a burning barrel of the same index may keep
    /// the count above 0 for ever: the wait also ends when the longest life is over.
    Addon(i32),
}

/// A source placed this frame, with the longest a drop of it lives (seconds), when known; read
/// for [`DropSource::Addon`] only.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub source: DropSource,
    pub longest_life: Option<f32>,
}

impl Placed {
    /// How long after it is taken down the source's drops may be in the air; `None`: as long
    /// as any is counted.
    fn wait(&self) -> Option<f32> {
        match self.source {
            DropSource::Model(_) => None,
            DropSource::Addon(_) => {
                let life = self.longest_life.filter(|l| l.is_finite() && *l > 0.0).unwrap_or(UNKNOWN_LIFE);
                Some((life + LIFE_MARGIN).min(LONGEST_WAIT))
            }
        }
    }
}

/// Every source of drops placed so far whose drops may still be in the air. An earlier version
/// remembered one model: rain that gave way to snow was forgotten with its drops still
/// falling, and drops of a model's add-on nodes were never counted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drops {
    /// Each source and the seconds it may still be waited for (`None`: by its count alone).
    sources: Vec<(DropSource, Option<f32>)>,
}

impl Drops {
    /// A frame: `placed` is what stands now (all of it, also what has given no drop yet),
    /// `alive` the count of a source's drops alive after this frame's births and deaths.
    /// Whether drops may be in the air.
    pub fn follow(&mut self, placed: &[Placed], alive: impl Fn(&DropSource) -> usize, dt: f32) -> bool {
        let dt = step(dt);
        self.sources.retain_mut(|(source, left)| {
            if placed.iter().any(|p| p.source == *source) {
                return true;
            }
            if let Some(left) = left {
                *left -= dt;
                if *left <= 0.0 {
                    return false;
                }
            }
            alive(source) > 0
        });
        for p in placed {
            match self.sources.iter_mut().find(|(s, _)| *s == p.source) {
                Some((_, left)) => *left = p.wait(),
                None => self.sources.push((p.source.clone(), p.wait())),
            }
        }
        !self.sources.is_empty()
    }

    /// The sources still waited for (a debug line, the tests).
    pub fn waiting(&self) -> usize {
        self.sources.len()
    }
}

/// The amount as it moves from frame to frame.
///
/// An earlier version followed its target as it was except after a turned-round fade, so one
/// frame after the rain's end it stood at full strength. Here the glide is not a state that a
/// cut could forget to set: every rise, whatever its reason, is held to [`RISE_RATE`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Follow {
    /// The strength and the amount shown last; `None` before the first frame.
    shown: Option<(f32, f32)>,
}

impl Follow {
    /// The amount a frame later (0..[`MOST`]). The strength glides at [`RISE_RATE`] both ways
    /// (a forced weather changes the wind at once). The dry share acts as the fade has it, so
    /// the amount reaches 0 where the rain starts. Their product rises by at most
    /// [`RISE_RATE`] and falls by at most [`FALL_RATE`] a second; 0 at once while cut
    /// (`asked.cut` or `drops`). The first frame shows its target.
    pub fn follow(&mut self, asked: Asked, drops: bool, dt: f32) -> f32 {
        let (wanted, dry) = (held(asked.strength, 0.0, MOST), held(asked.dry, 0.0, 1.0));
        let dt = step(dt);
        let (strength, last) = match self.shown {
            Some((s, a)) => (s + (wanted - s).clamp(-RISE_RATE * dt, RISE_RATE * dt), a),
            None => (wanted, wanted * dry),
        };
        let amount = if asked.cut || drops {
            0.0
        } else {
            held(last + (strength * dry - last).clamp(-FALL_RATE * dt, RISE_RATE * dt), 0.0, MOST)
        };
        self.shown = Some((strength, amount));
        amount
    }
}

/// The wind's velocity now (Bevy world x and z, units a second): the trees' direction
/// (`tree::wind_vector`: fixed, or a script's SetWindDirection, the angle the wind blows to
/// [documented: GECK wiki SetWindDirection]) at the speed of [`wind_byte`]; `None` for a
/// direction that is not a finite number.
pub fn wind_velocity(sky: &SkyState) -> Option<Vec2> {
    let d = crate::tree::wind_vector(sky);
    let d = Vec2::new(d.x, d.z).try_normalize()?;
    let v = d * (wind_byte(sky) / 255.0 * SPEED_AT_FULL);
    v.is_finite().then_some(v)
}

/// How far the wind has carried the dust: the sum of its velocity over the game's clock, in
/// f64, kept within one [`DRIFT_PERIOD`] on each axis (the patches repeat there, so wrapping
/// moves nothing).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Drift {
    /// Bevy world x and z, 0..[`DRIFT_PERIOD`].
    at: [f64; 2],
    /// The dust's velocity, which follows the wind's; `None` before the first wind.
    velocity: Option<Vec2>,
}

impl Drift {
    /// A frame: the velocity goes toward `wind` over [`TURN_SECONDS`] (it is `wind` at first,
    /// and stays as it is for `None`), and the dust goes with it.
    pub fn advance(&mut self, wind: Option<Vec2>, dt: f32) {
        let dt = step(dt);
        let wind = wind.filter(|w| w.is_finite());
        self.velocity = match (self.velocity, wind) {
            (Some(v), Some(w)) => Some(v + (w - v) * (1.0 - (-dt / TURN_SECONDS).exp())),
            (v, w) => v.or(w),
        };
        let v = self.velocity().to_array();
        for (at, v) in self.at.iter_mut().zip(v) {
            *at = wrapped(*at + f64::from(dt) * f64::from(v));
        }
    }

    /// The patches' phase at `camera` (Bevy world x and z): (camera - drift) within
    /// 0..[`DRIFT_PERIOD`], to which a shader adds a place relative to the camera. In f64, so
    /// a camera far from the origin keeps its fraction.
    pub fn phase(&self, camera: [f64; 2]) -> Vec2 {
        let p = |k: usize| {
            let p = wrapped(camera[k] - self.at[k]) as f32;
            // The f32 nearest to a number just under the period is the period.
            if p < DRIFT_PERIOD as f32 { p } else { 0.0 }
        };
        Vec2::new(p(0), p(1))
    }

    /// The dust's velocity now (Bevy world x and z, units a second).
    pub fn velocity(&self) -> Vec2 {
        self.velocity.unwrap_or(Vec2::ZERO)
    }
}

/// `x` within 0..[`DRIFT_PERIOD`]; 0 when it is not a finite number.
fn wrapped(x: f64) -> f64 {
    let p = x.rem_euclid(DRIFT_PERIOD);
    // A tiny negative number's remainder rounds up to the period itself.
    if p.is_finite() && p < DRIFT_PERIOD { p } else { 0.0 }
}

/// The dust's colour (linear): the fog's, tinted by the hue of the light on it, its ambient
/// plus its sunlight (divided by the sum's Rec. 709 luminance, each channel held to 0..2; no
/// tint without light). Never a colour of ours: a black fog's dust is black [design].
pub fn dust_colour(fog: [f32; 3], ambient: [f32; 3], sunlight: [f32; 3]) -> [f32; 3] {
    let light: [f32; 3] = std::array::from_fn(|i| held(ambient[i], 0.0, 1e3) + held(sunlight[i], 0.0, 1e3));
    let luminance = 0.2126 * light[0] + 0.7152 * light[1] + 0.0722 * light[2];
    std::array::from_fn(|i| {
        let hue = if luminance > 1e-4 { held(light[i] / luminance, 0.0, 2.0) } else { 1.0 };
        held(fog[i], 0.0, 1.0) * hue
    })
}

/// [`dust_colour`] of the weather now (its colours at the hour, blended through a fade, with a
/// script's changes).
pub fn sky_dust_colour(sky: &SkyState) -> [f32; 3] {
    dust_colour(sky.color(ColorType::Fog), sky.color(ColorType::Ambient), sky.color(ColorType::Sunlight))
}

/// What the frame's dust is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DustFrame {
    /// 0..[`MOST`]; 0 in an interior.
    pub amount: f32,
    /// Linear, each channel 0..2.
    pub colour: [f32; 3],
    /// Cut to 0 by rain, snow or drops in the air (a log line, a debug view).
    pub wet: bool,
    /// In an interior.
    pub indoors: bool,
}

/// The rules' state between frames.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindDust {
    follow: Follow,
    drops: Drops,
    pub drift: Drift,
}

impl WindDust {
    /// A frame, after the sky's and the particles', by the game's clock (`dt` 0 while it is
    /// paused: nothing moves). In an interior (`indoors`: the streamer's cell, which may have
    /// no lighting of its own; or the sky's interior lighting) the amount is 0 and the rest
    /// goes on as the weather outside does.
    pub fn advance(&mut self, sky: &SkyState, indoors: bool, placed: &[Placed], alive: impl Fn(&DropSource) -> usize, dt: f32) -> DustFrame {
        let drops = self.drops.follow(placed, alive, dt);
        let asked = asked(sky);
        let amount = self.follow.follow(asked, drops, dt);
        self.drift.advance(wind_velocity(sky), dt);
        let indoors = indoors || sky.interior.is_some();
        DustFrame { amount: if indoors { 0.0 } else { amount }, colour: sky_dust_colour(sky), wet: asked.cut || drops, indoors }
    }

    /// A load, a new game or a long trip: the next frame shows its target (0 if it is cut).
    /// What is in the air and how far the dust has drifted stay.
    pub fn arrive(&mut self) {
        self.follow = Follow::default();
    }

    /// The sources of drops waited for (a debug line, the tests).
    pub fn waiting(&self) -> usize {
        self.drops.waiting()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;
    const RAIN: &str = "meshes\\a\\rain.nif";
    const SNOWFALL: &str = "meshes\\a\\snow.nif";

    fn weather(wind: u8) -> Weather {
        Weather { classification: 1, wind_speed: wind, ..default() }
    }

    /// A weather with a precipitation model and its fade bytes.
    fn wet(fades: (u8, u8)) -> Weather {
        Weather { precipitation_model: Some("a\\rain.nif".into()), precipitation_fade: fades, ..weather(127) }
    }

    fn sky(w: Weather) -> SkyState {
        let mut sky = SkyState::default();
        sky.weather = w;
        sky.fade = 1.0;
        sky.hour = 12.0;
        sky
    }

    fn fading(from: Weather, to: Weather, fade: f32) -> SkyState {
        let mut s = sky(to);
        s.prev = Some(from);
        s.fade = fade;
        s
    }

    fn model(path: &str) -> Placed {
        Placed { source: DropSource::Model(path.into()), longest_life: None }
    }

    fn addon(index: i32, longest_life: Option<f32>) -> Placed {
        Placed { source: DropSource::Addon(index), longest_life }
    }

    /// A count of `n` drops of `of`, 0 of any other source.
    fn drops(of: &Placed, n: usize) -> impl Fn(&DropSource) -> usize {
        let of = of.source.clone();
        move |s| if *s == of { n } else { 0 }
    }

    fn no_drops(_: &DropSource) -> usize {
        0
    }

    /// What the weather asks for with nothing to follow: the steady amount.
    fn target(s: &SkyState) -> f32 {
        let a = asked(s);
        if a.cut { 0.0 } else { a.strength * a.dry }
    }

    #[test]
    fn a_calm_day_has_none_an_ordinary_one_and_a_windy_one_three() {
        assert_eq!(strength_from_wind(0.0), 0.0);
        assert_eq!(strength_from_wind(20.0), 0.0);
        assert!((strength_from_wind(50.0) - 1.0).abs() < 1e-6);
        assert!((strength_from_wind(127.0) - 3.0).abs() < 1e-6);
        assert_eq!(strength_from_wind(255.0), MOST);
        assert_eq!(strength_from_wind(1e9), MOST, "held above the byte's range");
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -5.0] {
            assert_eq!(strength_from_wind(bad), 0.0, "{bad}");
        }
        let mut last = 0.0;
        for k in 0..=2550 {
            let a = strength_from_wind(k as f32 / 10.0);
            assert!(a >= last && a - last < 0.004 && a <= MOST, "rising without a step at {}", k as f32 / 10.0);
            last = a;
        }
    }

    #[test]
    fn the_strength_goes_evenly_from_one_weather_to_the_next() {
        assert_eq!(wind_byte(&sky(weather(50))), 50.0);
        assert_eq!(wind_byte(&fading(weather(50), weather(127), 0.0)), 50.0);
        assert_eq!(wind_byte(&fading(weather(50), weather(127), 1.0)), 127.0);
        assert_eq!(wind_byte(&fading(weather(0), weather(100), 0.25)), 25.0);
        let mut last = target(&sky(weather(0)));
        for i in 0..=1000 {
            let a = target(&fading(weather(0), weather(255), i as f32 / 1000.0));
            assert!(a >= last && a - last < 0.01, "{last} -> {a}");
            last = a;
        }
        assert_eq!(last, target(&sky(weather(255))), "the fade's end is the weather after it");
        // A fade that is not a number, or past its ends.
        for (bad, want) in [(f32::NAN, 50.0), (f32::INFINITY, 50.0), (-3.0, 50.0), (7.0, 127.0)] {
            assert_eq!(wind_byte(&fading(weather(50), weather(127), bad)), want, "{bad}");
        }
        // The trees' wind, to its thousandths.
        for s in [sky(weather(50)), sky(weather(255)), fading(weather(50), weather(127), 0.4)] {
            assert!((crate::tree::wind_vector(&s).w * 255.0 - wind_byte(&s)).abs() < 0.13);
        }
    }

    #[test]
    fn rain_snow_and_precipitation_models_give_none_by_their_values_alone() {
        let wets = [
            Weather { rainy: true, ..weather(127) },
            Weather { classification: RAINY, ..weather(127) },
            Weather { classification: SNOW | 1, ..weather(127) },
            wet((0, 255)),
        ];
        for w in &wets {
            assert_eq!(dry_share(w), 0.0);
            assert!(asked(&sky(w.clone())).cut, "shown with no fade left: gone at once");
        }
        for class in [0, 1, 2, 3, 16, 128] {
            assert_eq!(dry_share(&Weather { classification: class, ..weather(127) }), 1.0, "class {class}");
        }
        // No name and no ID changes anything.
        let named = Weather { form_id: 0xE3792, edid: Some("HVDustStorm".into()), plugin: Some("Rain.esp".into()), ..weather(50) };
        assert_eq!(asked(&sky(named)), asked(&sky(weather(50))));
        // Into a weather that is wet by its flag alone: evenly to 0, and cut at the end.
        let mut last = f32::INFINITY;
        for i in 0..=100 {
            let s = fading(weather(127), wets[0].clone(), i as f32 / 100.0);
            let a = target(&s);
            assert!(!asked(&s).cut && a <= last && (i == 0 || last - a < 0.04), "{last} -> {a}");
            last = a;
        }
        assert_eq!(last, 0.0);
    }

    #[test]
    fn dust_is_gone_before_rain_falls_and_only_back_after_it_stopped() {
        let steady = target(&sky(weather(127)));
        for byte in [0u8, 1, 52, 255] {
            let mut last = f32::INFINITY;
            for i in 0..=255 {
                let s = fading(weather(127), wet((byte, 128)), i as f32 / 255.0);
                if fade_share_on((byte, 128), Role::Incoming(s.fade)) {
                    assert!(asked(&s).cut && asked(&s).dry == 0.0, "byte {byte}: rain falls at {}", s.fade);
                }
                let a = target(&s);
                assert!(a <= last, "byte {byte}: falls only");
                last = a;
            }
            if byte > 0 {
                assert!((target(&fading(weather(127), wet((byte, 128)), 0.0)) - steady).abs() < 1e-5, "no step at the start");
            }
            let mut last = 0.0;
            for i in 0..=255 {
                let s = fading(wet((128, byte)), weather(127), i as f32 / 255.0);
                if fade_share_on((128, byte), Role::Outgoing(s.fade)) {
                    assert!(asked(&s).cut && asked(&s).dry == 0.0, "byte {byte}: rain still falls at {}", s.fade);
                }
                let a = target(&s);
                assert!(a >= last, "byte {byte}: rises only");
                last = a;
            }
            if byte < 255 {
                assert!((last - steady).abs() < 1e-5, "whole again at the end");
            }
        }
        for i in 0..=20 {
            assert_eq!(target(&fading(wet((52, 52)), wet((200, 52)), i as f32 / 20.0)), 0.0, "rain to rain");
        }
        // A fade that is not a number counts as its start.
        let s = fading(wet((52, 52)), weather(127), f32::NAN);
        assert!(asked(&s).cut && asked(&s).dry == 0.0);
    }

    #[test]
    fn a_fade_into_rain_is_followed_to_nothing_without_a_step() {
        // 30 s to the weather, rain from a fifth of it on: the amount goes down its line.
        let mut f = Follow::default();
        let mut last = f.follow(asked(&sky(weather(127))), false, DT);
        assert!((last - 3.0).abs() < 1e-5);
        for frame in 0..=1800 {
            let s = fading(weather(127), wet((51, 128)), frame as f32 / 1800.0);
            let a = f.follow(asked(&s), false, DT);
            assert!(a <= last + 1e-6 && last - a < 0.02, "frame {frame}: {last} -> {a}");
            assert!(!precipitation_falls(&s) || a == 0.0);
            last = a;
        }
        assert_eq!(last, 0.0);
    }

    #[test]
    fn after_rain_the_dust_comes_back_at_its_rate_never_at_once() {
        // Every way rain can end: its fade's last frame (end byte 255, no room to fade in), a
        // forced dry weather, the last drop of it.
        let mut f = Follow::default();
        assert_eq!(f.follow(asked(&sky(wet((0, 255)))), false, DT), 0.0);
        assert_eq!(f.follow(asked(&fading(wet((0, 255)), weather(127), 0.999)), false, DT), 0.0);
        let dry = asked(&sky(weather(127)));
        assert_eq!(f.follow(dry, true, DT), 0.0, "its last drops still fall");
        let mut last = 0.0;
        for frame in 1..=600 {
            let a = f.follow(dry, false, DT);
            assert!(a > last && a - last <= RISE_RATE * DT + 1e-5, "frame {frame}: {last} -> {a}");
            last = a;
        }
        assert!((last - 1.0).abs() < 0.01, "ten seconds at a tenth a second: {last}");
        for _ in 0..1300 {
            last = f.follow(dry, false, DT);
        }
        assert!((last - 3.0).abs() < 1e-3, "and all of it in thirty: {last}");
        // Cut again half way and back: from 0 again.
        assert_eq!(f.follow(dry, true, DT), 0.0);
        assert!(f.follow(dry, false, DT) <= RISE_RATE * DT + 1e-6);
    }

    #[test]
    fn a_forced_weather_never_makes_the_amount_jump() {
        let run = |from: Weather, to: Weather| {
            let mut f = Follow::default();
            let mut last = f.follow(asked(&sky(from)), false, DT);
            let s = sky(to);
            for _ in 0..(40 * 60) {
                let a = f.follow(asked(&s), false, DT);
                assert!((a - last).abs() <= RISE_RATE * DT + 1e-5, "{last} -> {a}");
                last = a;
            }
            assert!((last - target(&s)).abs() < 1e-3, "{last}, it gets there");
        };
        run(weather(0), weather(255));
        run(weather(255), weather(0));
        run(weather(50), weather(127));
        // Through the sky's own forced change, to a wet weather and back.
        let mut s = sky(weather(127));
        let mut f = Follow::default();
        assert!(f.follow(asked(&s), false, DT) > 2.9);
        s.script_weather(Weather { rainy: true, form_id: 0x77, ..weather(127) }, [None; 6], true, true);
        assert!(s.prev.is_none() && s.weather.rainy);
        assert_eq!(f.follow(asked(&s), false, DT), 0.0, "wet at once, gone at once");
        s.script_weather(Weather { form_id: 0x78, ..weather(127) }, [None; 6], true, true);
        assert!(s.prev.is_none() && !s.weather.rainy);
        let a = f.follow(asked(&s), false, DT);
        assert!(a > 0.0 && a <= RISE_RATE * DT + 1e-6, "dry at once, back slowly: {a}");
    }

    #[test]
    fn a_turned_round_fade_never_lets_the_amount_jump_up() {
        // A tenth into a wet weather, not raining yet; turned back, nine tenths of the way.
        let before = fading(weather(127), wet((52, 52)), 0.1);
        let after = fading(wet((52, 52)), weather(127), 0.9);
        assert!(!precipitation_falls(&before) && !precipitation_falls(&after));
        assert!(target(&after) - target(&before) > 0.5, "the target jumps");
        let mut f = Follow::default();
        let a = f.follow(asked(&before), false, DT);
        let b = f.follow(asked(&after), false, DT);
        assert!(b > a && b - a <= RISE_RATE * DT + 1e-6, "{a} -> {b}");
        // The other way round it goes down within a second, not in a frame.
        let mut f = Follow::default();
        let a = f.follow(asked(&after), false, DT);
        let b = f.follow(asked(&before), false, DT);
        assert!(b < a && a - b <= FALL_RATE * DT + 1e-6, "{a} -> {b}");
    }

    #[test]
    fn a_pause_holds_a_hitch_is_one_step_and_wild_numbers_stay_in_range() {
        let dry = Asked { strength: 3.0, dry: 1.0, cut: false };
        let mut f = Follow::default();
        f.follow(Asked { cut: true, ..dry }, false, DT);
        assert_eq!(f.follow(dry, false, 0.0), 0.0, "paused");
        assert!(f.follow(dry, false, 1e6) <= RISE_RATE * LONGEST_STEP + 1e-6, "a hitch");
        let wild = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0, 0.0, 0.5, 1e30];
        for &strength in &wild {
            for &dry in &wild {
                for &dt in &wild {
                    for cut in [false, true] {
                        let a = f.follow(Asked { strength, dry, cut }, false, dt);
                        assert!((0.0..=MOST).contains(&a) && (!cut || a == 0.0), "{a} for {strength} {dry} {dt}");
                        let mut first = Follow::default();
                        let a = first.follow(Asked { strength, dry, cut }, false, dt);
                        assert!((0.0..=MOST).contains(&a) && (!cut || a == 0.0));
                    }
                }
            }
        }
    }

    #[test]
    fn every_model_placed_is_waited_for_until_its_last_drop() {
        let (rain, snow) = (model(RAIN), model(SNOWFALL));
        let mut d = Drops::default();
        assert!(!d.follow(&[], |_| 99, DT), "nothing placed: another effect's particles are nobody's drops");
        assert!(d.follow(std::slice::from_ref(&rain), no_drops, DT), "placed, no drop yet");
        // Rain gives way to snow in one frame; the rain's drops are still falling.
        assert!(d.follow(std::slice::from_ref(&snow), drops(&rain, 40), DT));
        assert_eq!(d.waiting(), 2);
        // The snow is taken down with no flake left: the rain's drops are still falling.
        assert!(d.follow(&[], drops(&rain, 12), DT), "the first version forgot them");
        assert_eq!(d.waiting(), 1);
        assert!(!d.follow(&[], no_drops, DT));
        assert_eq!(d.waiting(), 0);
        assert!(!d.follow(&[], drops(&rain, 5), DT), "and is not taken up again by itself");
        // Placed again and again: one entry each.
        for _ in 0..100 {
            d.follow(&[rain.clone(), snow.clone()], no_drops, DT);
        }
        assert_eq!(d.waiting(), 2);
    }

    #[test]
    fn a_child_emitters_drops_count_but_another_emitter_of_its_kind_holds_nothing_up() {
        let (rain, child) = (model(RAIN), addon(7, Some(2.0)));
        let mut d = Drops::default();
        assert!(d.follow(&[rain.clone(), child.clone()], no_drops, DT));
        // The model's own drops are gone, its add-on node's are not.
        assert!(d.follow(&[], drops(&child, 3), DT));
        assert!(!d.follow(&[], no_drops, DT), "gone before their longest life");
        // A burning barrel of the same index keeps the shared count up: the longest life ends it.
        d.follow(&[rain.clone(), child.clone()], no_drops, DT);
        let mut waited = 0.0;
        while d.follow(&[], drops(&child, 50), DT) {
            waited += DT;
            assert!(waited < 10.0, "held up for ever");
        }
        assert!((2.0..2.4).contains(&waited), "{waited}");
        // Standing, its time does not run; a model's own drops have none.
        for _ in 0..600 {
            assert!(d.follow(std::slice::from_ref(&child), drops(&child, 50), DT));
        }
        d.follow(std::slice::from_ref(&rain), no_drops, DT);
        for _ in 0..(200 * 60) {
            assert!(d.follow(&[], drops(&rain, 1), DT), "as long as one falls");
        }
        // Paused: nothing runs out.
        let mut d = Drops::default();
        d.follow(std::slice::from_ref(&child), no_drops, DT);
        for _ in 0..1000 {
            assert!(d.follow(&[], drops(&child, 50), 0.0));
        }
    }

    #[test]
    fn a_life_that_is_not_known_or_not_a_number_is_waited_for_within_bounds() {
        for (life, want) in [
            (None, UNKNOWN_LIFE + LIFE_MARGIN),
            (Some(f32::NAN), UNKNOWN_LIFE + LIFE_MARGIN),
            (Some(f32::INFINITY), UNKNOWN_LIFE + LIFE_MARGIN),
            (Some(-4.0), UNKNOWN_LIFE + LIFE_MARGIN),
            (Some(1.5), 1.5 + LIFE_MARGIN),
            (Some(1e30), LONGEST_WAIT),
        ] {
            assert_eq!(addon(1, life).wait(), Some(want), "{life:?}");
        }
        assert_eq!(model(RAIN).wait(), None);
        let mut d = Drops::default();
        d.follow(&[addon(1, None)], no_drops, f32::NAN);
        assert!(d.follow(&[], |_| 1, f32::NAN) && d.follow(&[], |_| 1, -5.0));
    }

    #[test]
    fn a_turn_changes_the_way_and_never_the_place() {
        let mut d = Drift::default();
        let (east, south) = (Vec2::new(118.0, 0.0), Vec2::new(0.0, 118.0));
        let (mut at, mut v) = (d.phase([0.0; 2]), None::<Vec2>);
        for frame in 0..1800 {
            // A right angle at once, then straight back, then a stop (a forced calm weather).
            let wind = match frame {
                0..600 => east,
                600..1000 => south,
                1000..1400 => -south,
                _ => Vec2::ZERO,
            };
            d.advance(Some(wind), DT);
            let now = d.phase([0.0; 2]);
            let moved = (now - at).abs();
            let moved = moved.min(Vec2::splat(DRIFT_PERIOD as f32) - moved);
            assert!(moved.length() <= 118.0 * DT + 0.02, "frame {frame}: {moved}");
            if let Some(v) = v {
                assert!((d.velocity() - v).length() <= 2.0 * 118.0 * DT / TURN_SECONDS + 1e-3, "frame {frame}: no lurch");
            }
            (at, v) = (now, Some(d.velocity()));
        }
        assert!(d.velocity().length() < 6.0, "it comes to rest: {}", d.velocity());
    }

    #[test]
    fn the_drift_is_the_winds_way_and_its_first_wind_is_whole() {
        let mut d = Drift::default();
        d.advance(None, DT);
        assert_eq!((d.velocity(), d.phase([5.0, 6.0])), (Vec2::ZERO, Vec2::new(5.0, 6.0)));
        for _ in 0..600 {
            d.advance(Some(Vec2::new(100.0, -50.0)), DT);
        }
        // Ten seconds on, the patch that was at the origin is at (1000, -500).
        let p = d.phase([1000.0, -500.0]);
        let off = p.min(Vec2::splat(DRIFT_PERIOD as f32) - p);
        assert!(off.max_element() < 0.1, "{p}");
        // A paused or broken clock, or a wind that is not a number, changes nothing.
        let before = d;
        d.advance(Some(Vec2::new(100.0, -50.0)), 0.0);
        d.advance(Some(Vec2::new(0.0, 500.0)), f32::NAN);
        d.advance(Some(Vec2::new(0.0, 500.0)), -1.0);
        assert_eq!(d, before);
        d.advance(Some(Vec2::new(f32::NAN, 1.0)), DT);
        assert_eq!(d.velocity(), before.velocity(), "the wind it had");
    }

    #[test]
    fn the_phase_keeps_its_fraction_far_out_and_its_period_fits_both_octaves() {
        for cell in [2_000.0, 600.0] {
            assert_eq!(DRIFT_PERIOD % cell, 0.0, "a whole number of cells of {cell}");
        }
        let mut d = Drift::default();
        for _ in 0..7 {
            d.advance(Some(Vec2::new(-133.3, 77.7)), 0.2);
        }
        for cam in [0.0, 1234.5, -77_777.25, 1.0e6, -1.0e6, 1.0e6 + 0.37, -1e-12, f64::NAN, f64::INFINITY, -1e300] {
            let p = d.phase([cam, -cam]);
            assert!(p.cmpge(Vec2::ZERO).all() && p.cmplt(Vec2::splat(DRIFT_PERIOD as f32)).all(), "{p} at {cam}");
        }
        let moved = d.phase([1.0e6 + 1.0, 0.0]).x - d.phase([1.0e6, 0.0]).x;
        assert!((moved - 1.0).abs() < 0.01, "one unit a million units out: {moved}");
        // Where it wraps, nothing moves: a period on is the same place.
        let (a, b) = (d.phase([1234.5, 9.0]), d.phase([1234.5 + DRIFT_PERIOD, 9.0 - DRIFT_PERIOD]));
        assert!((a - b).abs().max_element() < 1e-2, "{a} {b}");
    }

    #[test]
    fn the_velocity_is_the_trees_direction_at_the_weathers_speed() {
        let s = sky(weather(50));
        let v = wind_velocity(&s).unwrap();
        let tree = crate::tree::wind_vector(&s);
        assert!((v.length() - 50.0 / 255.0 * SPEED_AT_FULL).abs() < 1e-2, "{v}");
        assert!(v.normalize().dot(Vec2::new(tree.x, tree.z)) > 0.9999);
        assert_eq!(wind_velocity(&sky(weather(0))), Some(Vec2::ZERO), "still air");
        // A script's direction, and one that is no direction at all.
        let mut turned = sky(weather(50));
        turned.script_wind = Some(0.0);
        assert!((wind_velocity(&turned).unwrap().normalize() - Vec2::X).length() < 1e-4);
        for bad in [f32::NAN, f32::INFINITY] {
            turned.script_wind = Some(bad);
            assert_eq!(wind_velocity(&turned), None, "{bad}");
        }
    }

    #[test]
    fn the_colour_is_the_fogs_tinted_by_the_light_and_stays_finite() {
        let lin = |b: [u8; 3]| b.map(fnv_world::sky::srgb_to_linear);
        // A bluish fog under a warm sun and a bluish sky.
        let fog = lin([150, 170, 190]);
        let c = dust_colour(fog, lin([85, 105, 140]), lin([255, 225, 170]));
        assert!(c[0] > fog[0] && c[2] < fog[2], "warmer: {c:?}");
        // White light, or none, is the fog's own colour; its brightness does not matter.
        for c in [dust_colour(fog, [0.0; 3], [0.0; 3]), dust_colour(fog, [0.3; 3], [0.2; 3]), dust_colour(fog, [30.0; 3], [2.0; 3])] {
            assert!((Vec3::from(c) - Vec3::from(fog)).abs().max_element() < 1e-5, "{c:?}");
        }
        assert_eq!(dust_colour([0.0; 3], [0.4, 0.3, 0.2], [1.0; 3]), [0.0; 3], "a black fog's dust is black");
        let wild = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0, 1e30, 0.0, 1e-9, 0.5];
        for &a in &wild {
            for &b in &wild {
                for &f in &wild {
                    let c = dust_colour([f, 0.3, f], [a, b, 0.1], [b, a, 0.2]);
                    assert!(c.iter().all(|v| (0.0..=2.0).contains(v)), "{c:?} for {a} {b} {f}");
                }
            }
        }
        // A weather with no colours at all, and a fade that is not a number.
        for s in [sky(weather(50)), fading(weather(50), weather(127), f32::NAN)] {
            assert!(sky_dust_colour(&s).iter().all(|v| (0.0..=2.0).contains(v)));
        }
    }

    #[test]
    fn a_frame_puts_the_rules_together() {
        let mut w = WindDust::default();
        let s = sky(weather(50));
        let f = w.advance(&s, false, &[], no_drops, DT);
        assert!((f.amount - 1.0).abs() < 1e-6 && !f.wet && !f.indoors, "the first frame shows its target: {f:?}");
        // A model placed on a dry weather (the test aid, a script): cut while it stands and
        // while its drops fall, then slowly back.
        let rain = model(RAIN);
        let f = w.advance(&s, false, std::slice::from_ref(&rain), no_drops, DT);
        assert!(f.amount == 0.0 && f.wet);
        let f = w.advance(&s, false, &[], drops(&rain, 8), DT);
        assert!(f.amount == 0.0 && f.wet && w.waiting() == 1);
        let f = w.advance(&s, false, &[], no_drops, DT);
        assert!(f.amount > 0.0 && f.amount <= RISE_RATE * DT + 1e-6 && !f.wet, "{f:?}");
        // Inside, none; the weather outside goes on, so coming out is no jump back to the rain.
        for _ in 0..300 {
            let f = w.advance(&s, true, &[], no_drops, DT);
            assert!(f.amount == 0.0 && f.indoors);
        }
        let f = w.advance(&s, false, &[], no_drops, DT);
        assert!((f.amount - 0.5).abs() < 0.02, "five seconds of the way back: {f:?}");
        // The sky's interior lighting says it too, and the wind goes on there.
        let mut inside = sky(weather(50));
        inside.interior = Some(fnv_world::lighting::CellLighting::default());
        let f = w.advance(&inside, false, &[], no_drops, DT);
        assert!(f.amount == 0.0 && f.indoors && w.drift.velocity().length() > 100.0);
        // A load: the target at once; but not into rain.
        w.arrive();
        assert!((w.advance(&s, false, &[], no_drops, DT).amount - 1.0).abs() < 1e-6);
        w.arrive();
        assert_eq!(w.advance(&sky(wet((0, 255))), false, &[], no_drops, DT).amount, 0.0);
        assert!(w.advance(&s, false, &[], no_drops, DT).amount <= RISE_RATE * DT + 1e-6);
    }

    #[test]
    fn rain_that_starts_while_the_player_is_inside_is_not_forgotten() {
        let mut w = WindDust::default();
        let (dry, rain) = (sky(weather(127)), sky(wet((0, 255))));
        w.advance(&dry, false, &[], no_drops, DT);
        w.advance(&rain, true, &[], no_drops, DT);
        w.advance(&dry, true, &[], no_drops, DT);
        let f = w.advance(&dry, false, &[], no_drops, DT);
        assert!(f.amount <= 2.0 * RISE_RATE * DT + 1e-6, "no full strength at the door: {f:?}");
    }
}
