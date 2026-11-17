//! Where the settings page shows each row: its tab, and the category (a heading) it stands
//! under in that tab (#1953, the menu overhaul; docs/design/settings-page.md). Presentation
//! only: a row keeps the key, the note, the default and the group it registered with
//! (`crate::settings`), and the registry keeps its order, which the values, the INI and the
//! test aids go by. This table only says where the page draws it [design].
//!
//! **Eight tabs, the original's four categories first** [design: #1953; the original's
//! Settings opens Gameplay, Display, Audio and Controls, observed in the original's pause
//! menu, 2026-11-09; the owner's menu overhaul, 2026-11-15]: Gameplay, Display, Audio,
//! Controls, then Graphics, NVF Features, Tweaks and Debug ([`TAB_ORDER`]). Each tab lists
//! its [`SECTIONS`] in order, each a **category**: a heading the player can open and close,
//! an empty line after each open one ([`ResolvedLayout::tab_entries`]).
//!
//! **The Controls tab's own rows** (`crate::settings_view::ControlRow`: the input device,
//! each control's key or button, Invert Y, the sensitivities, Rumble) are named here too, by
//! [`CONTROL_PREFIX`] and their name (`controls:device`, `controls:bind:Jump`), beside the
//! registered rows of the same category (Button Style under Input) [design: the overhaul].
//!
//! **NVF Features** holds only what the owner names (`NVF_FEATURES_KEYS`, a test keeps
//! it so) [design: owner 2026-11-15: "do not move any feature there without my say so"].
//!
//! **A row the table does not name** stays under the group it registered with, used as its
//! tab with no heading, after the tab's named rows, in registry order; a group that is
//! none of the tabs becomes a tab of its own after Debug ([`resolve_layout`]). The engine's
//! own rows are all named (a test keeps it so); the fallback is for what a later change
//! adds before it places its row.
//!
//! [`Placement::subgroup`] is the Tweaks tab's group (Gameplay, Combat, … as Stewie's
//! Tweaks sorts them): the tab shows one group at a time, chosen by its Category row
//! (`crate::settings_view`), its categories closed until opened [design: owner 2026-11-15].
//! Every row #1892 registers (`tweak-<key>`) has its group and category here already
//! ([`group_section`]; the list for the owner's review: docs/design/settings-tweaks-layout.md):
//! the group the plugin's catalogue gives its INI key, and in a category the lists first,
//! then the switches, then the sliders, each by label, a row that only matters with
//! another right after that one [design: owner 2026-11-15].
//!
//! What a row's visibility is (the colour sliders only for Custom, the controller rows only
//! with a controller, a row its module hides) is not here: the page decides it each frame
//! (`crate::settings_view`), so a hidden row keeps its tab and category.

use crate::settings::Setting;

/// Where a row is shown: its tab, the Tweaks tab's group, the category (heading) over it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Placement {
    pub tab: &'static str,
    /// The Tweaks tab's group (`None` on every other tab).
    pub subgroup: Option<&'static str>,
    pub heading: Option<&'static str>,
}

impl Placement {
    /// A tab of its own, with no heading.
    pub const fn tab(tab: &'static str) -> Placement {
        Placement { tab, subgroup: None, heading: None }
    }

    /// Its tab, group and heading in English joined by " > " (`Gameplay > Dialogue`), as
    /// the log names it.
    pub fn path(&self) -> String {
        [Some(self.tab), self.subgroup, self.heading].into_iter().flatten().collect::<Vec<_>>().join(" > ")
    }

    /// A category of the Tweaks tab (closed until the player opens it) [design: owner
    /// 2026-11-15].
    pub fn closed_by_default(&self) -> bool {
        self.tab == TWEAKS_TAB
    }
}

/// A run of rows under one heading: registered rows by their key, the Controls tab's own by
/// [`CONTROL_PREFIX`] and their name.
pub struct Section {
    pub placement: Placement,
    pub keys: &'static [&'static str],
}

/// The tabs, in the order the page lists them [design: #1953; the overhaul].
pub const DESKTOP_TAB_ORDER: [&str; 8] = ["Gameplay", "Display", "Audio", "Controls", "Graphics", NVF_FEATURES_TAB, TWEAKS_TAB, "Debug"];
pub const MOBILE_TAB: &str = "Mobile";
pub const MOBILE_TAB_ORDER: [&str; 9] = ["Gameplay", "Display", "Audio", "Controls", MOBILE_TAB, "Graphics", NVF_FEATURES_TAB, TWEAKS_TAB, "Debug"];
#[cfg(target_os = "android")]
pub const TAB_ORDER: [&str; 9] = MOBILE_TAB_ORDER;
#[cfg(not(target_os = "android"))]
pub const TAB_ORDER: [&str; 8] = DESKTOP_TAB_ORDER;
/// The tab whose own rows (`crate::bindings`) the table names by [`CONTROL_PREFIX`].
pub const CONTROLS_TAB: &str = "Controls";
/// The tab #1892's tweaks register under.
pub const TWEAKS_TAB: &str = "Tweaks";
/// NVForever's own features the owner names [design: owner 2026-11-15].
pub const NVF_FEATURES_TAB: &str = "NVF Features";
/// A Controls row in the table: `controls:` and its name (`crate::settings_view::ControlRow`).
pub const CONTROL_PREFIX: &str = "controls:";

const fn section(tab: &'static str, heading: Option<&'static str>, keys: &'static [&'static str]) -> Section {
    Section { placement: Placement { tab, subgroup: None, heading }, keys }
}

/// A category of the Tweaks tab: under its group `subgroup`, headed `heading`.
const fn group_section(subgroup: &'static str, heading: &'static str, keys: &'static [&'static str]) -> Section {
    Section { placement: Placement { tab: TWEAKS_TAB, subgroup: Some(subgroup), heading: Some(heading) }, keys }
}

/// The rows the NVF Features tab may hold: only what the owner named there [design: owner
/// 2026-11-15]. Adding a key needs the owner's word; a bigger gameplay change is offered to
/// the owner for this tab, never moved here unasked (docs/design/settings-page.md).
pub const NVF_FEATURES_KEYS: [&str; 9] = [
    "advanced-movement",
    "movement-weight",
    "movement-inertia",
    "weapon-handling",
    "advanced-recoil",
    "weapon-inertia",
    "animation-smoothing",
    "camera-motion",
    "grass-fire",
];

/// Every named row's tab and category, in the order the page shows them [design: the
/// owner's menu overhaul, 2026-11-15, tab by tab; docs/design/settings-page.md §2]. A key
/// not registered in a run is left out (`resolve_layout`), so rows of features still to
/// come (the movement rows, #1892's tweaks) can have their place already. The Tweaks tab's
/// groups come in the plugin's order: Gameplay, Combat, Quality of Life, HUD, Menus,
/// Hotkeys, Cheats, Misc.
pub const SECTIONS: &[Section] = &[
    // #2688's sound cover with the game's own rules (a bigger gameplay change: NVF Features is the
    // owner's call, recommended to them, not moved there).
    section("Gameplay", Some("Game"), &["difficulty", "hardcore", "reload-slows", "UsePipBoy", "sound-cover"]),
    // "V.A.T.S." as the data spells it (MESG `HelpVATS`).
    section("Gameplay", Some("V.A.T.S."), &["vats-style", "vats-slowdown", "kill-camera"]),
    section("Gameplay", Some("Camera"), &["detached-camera", "true-iron-sights", "first-person-body", "weapon-lower", "field-of-view", "viewmodel-field-of-view", "touch-pipboy-size-v1", "touch-pipboy-tab-swipes"]),
    section("Gameplay", Some("Dialogue"), &["dialogue-zoom", "dialogue-look", "dialogue-recentre", "menu-pause", "dialogue-goodbye-hint", "lip-sync", "touch-dialogue", "dialogue-choice-text-size", "dialogue-subtitle-text-size", "dialogue-touch-choice", "dialogue-speaker-side", "dialogue-theme-v1", "dialogue-background-opacity-v1", "dialogue-background-tint-v1", "dialogue-frame-opacity-v1", "dialogue-hints-v1"]),
    section("Gameplay", Some("Saving"), &["autosave-travel", "autosave-fast-travel", "autosave-wait", "autosave-sleep", "autosave-scripts"]),
    section("Gameplay", Some("Bug Reports"), &["send-reports", "reports-delete"]),
    section("Display", Some("Screen"), &["monitor", "resolution", "borderless"]),
    section("Display", Some("Frame Rate"), &["display-sync", "frame-cap", "physics-rate"]),
    section("Display", Some("Upscaling and Anti-Aliasing"), &["upscaler", "upscaler-quality", "dlss-model", "fsr-library", "anti-aliasing", "sharpen"]),
    section("Display", Some("Frame Generation and Latency"), &["frame-generation", "frame-generation-frames", "latency-reduction"]),
    section("Display", Some("Interface"), &["language", "menu-font", "sneak-gauges", "dynamic-crosshair", "crosshair-style", "ui-scale", "menu-scale", "hud-scale-v2", "hud-health-size-v1", "hud-action-points-size-v1", "hud-compass-size-v1", "barter-menu-size-v1", "repair-menu-size-v1", "workbench-menu-size-v1"]),
    section("Display", Some("Colours"), &["hud-colour", "hud-red", "hud-green", "hud-blue", "pipboy-colour", "pipboy-red", "pipboy-green", "pipboy-blue"]),
    section(
        "Display",
        Some("Performance Overlay"),
        &[
            "fps-counter",
            "fps-counter-fps",
            "fps-counter-graph",
            "fps-counter-split",
            "fps-counter-cpu",
            "fps-counter-gpu",
            "fps-counter-vram",
            "fps-counter-ram",
            "fps-counter-loaded",
            "fps-counter-upscaling",
            "fps-counter-low",
        ],
    ),
    section("Audio", Some("Volume"), &["volume-master", "volume-music", "volume-footstep", "volume-voice", "volume-effects", "volume-radio"]),
    section("Audio", Some("Sound"), &["music", "voice", "reverb", "voice-reverb"]),
    section("Controls", Some("Input"), &["controls:device", "button-style", "controls:automatic-input", "controls:rumble"]),
    section("Controls", Some("Look"), &["controls:invert", "controls:sensitivity", "controls:vertical", "controls:aim-sensitivity"]),
    section("Controls", Some("Aim Assist"), &["aim-assist", "aim-slowdown", "aim-lock"]),
    section(
        "Controls",
        Some("Movement"),
        &[
            "controls:bind:Forward",
            "controls:bind:Back",
            "controls:bind:Slide Left",
            "controls:bind:Slide Right",
            "controls:bind:Jump",
            "controls:bind:Crouch/Sneak",
            "controls:bind:Run",
            "controls:bind:Always Run",
            "controls:bind:Auto Move",
        ],
    ),
    section("Controls", Some("Combat"), &["controls:bind:Use", "controls:bind:Block", "controls:bind:Ready Item", "controls:bind:Vats", "controls:bind:Ammo Swap", "controls:bind:Lower Weapon"]),
    section(
        "Controls",
        Some("Actions"),
        &["controls:bind:Activate", "controls:bind:Grab", "controls:bind:Menu Mode", "controls:bind:Rest", "controls:bind:Toggle POV", "controls:bind:QuickSave", "controls:bind:QuickLoad"],
    ),
    section(
        "Controls",
        Some("Hotkeys"),
        &["controls:bind:Hotkey1", "controls:bind:Hotkey3", "controls:bind:Hotkey4", "controls:bind:Hotkey5", "controls:bind:Hotkey6", "controls:bind:Hotkey7", "controls:bind:Hotkey8"],
    ),
    // The tweaks' own keys (#1892, `crate::bindings::TWEAK_CONTROLS` on), none bound until the
    // player binds one, and the console's and the screenshot's (bound): their own category
    // after the game's [design: the menu overhaul, merged with #1892; owner 2026-11-16:
    // "Additional Features", the console and screenshot keys moved here as bindings].
    section(
        "Controls",
        Some("Additional Features"),
        &[
            "controls:bind:Holster Weapon",
            "controls:bind:Toggle Light",
            "controls:bind:Toggle Crosshair",
            "controls:bind:Toggle Sneak Label",
            "controls:bind:Toggle Menus",
            "controls:bind:Exit to Main Menu",
            "controls:bind:Exit Game",
            "controls:bind:No Collision",
            "controls:bind:Pip-Boy Stats",
            "controls:bind:Pip-Boy Items",
            "controls:bind:Pip-Boy Data",
            "controls:bind:Local Map",
            "controls:bind:World Map",
            "controls:bind:Quests",
            "controls:bind:Radio",
            "controls:bind:Toggle Iron Sights",
            "controls:bind:Drop Weapon",
            "controls:bind:Radio Volume Up",
            "controls:bind:Radio Volume Down",
            "controls:bind:Last Weapon",
            "controls:bind:Smooth Camera",
            "controls:bind:Repeat Activate",
            "controls:bind:Toggle Sighting Node",
            "controls:bind:Place Marker Here",
            "controls:bind:Hide HUD",
            "controls:bind:Skip Radio Song",
            "controls:bind:Console",
            "controls:bind:Screenshot",
        ],
    ),
    section("Graphics", Some("Quality"), &["quality"]),
    section("Graphics", Some("Shadows"), &["sun-shadows", "shadow-filter", "shadow-distance", "fine-shadows"]),
    section("Graphics", Some("Lighting and Effects"), &["ambient-occlusion", "light-shafts", "bloom", "grade", "glow", "eye-adaptation", "neon-glow"]),
    section("Graphics", Some("Water"), &["water-reflections", "water-mirror"]),
    section("Graphics", Some("Detail and Distance"), &["distant-lod", "grass", "anisotropic", "terrain-height-blend", "terrain-anti-tiling", "smooth-terrain", "object-distance", "actor-distance", "item-distance", "grass-distance"]),
    section("Graphics", Some("World and Animation"), &["head-tracking", "foot-ik", "sky-clock"]),
    // The owner's own layout of the movement work (tech/movement-inertia, 2026-11-15).
    section(NVF_FEATURES_TAB, Some("Advanced Movement"), &["advanced-movement", "movement-weight"]),
    section(NVF_FEATURES_TAB, Some("Gameplay"), &["movement-inertia", "weapon-handling", "advanced-recoil"]),
    section(NVF_FEATURES_TAB, Some("Visual Only"), &["weapon-inertia", "animation-smoothing", "camera-motion"]),
    section(NVF_FEATURES_TAB, Some("World Interaction"), &["grass-fire"]),
    // The Tweaks tab (#1892), placed ahead of it: Stewie's Tweaks' groups, each in its
    // categories (docs/design/settings-tweaks-layout.md) [design: owner 2026-11-15].
    // Tweaks > Gameplay.
    group_section(
        "Gameplay",
        "Character and Experience",
        &[
            "tweak-free-wild-wasteland",
            "tweak-frenzy-kill-xp",
            "tweak-no-skill-tags",
            "tweak-custom-skill-points",
            "tweak-skill-point-base",
            "tweak-skill-points-per-int",
            "tweak-cap-skills-by-special",
            "tweak-cap-skills-spare-tagged",
            "tweak-skill-cap-base",
            "tweak-skill-cap-most",
            "tweak-skill-cap-per-special",
            "tweak-skill-cap-per-luck",
            "tweak-xp-multiplier",
            "tweak-key-xp-share",
            "tweak-password-xp-share",
            "tweak-kill-xp-per-level",
            "tweak-max-level",
            "tweak-perks-per-level",
            "tweak-special-points",
        ],
    ),
    group_section(
        "Gameplay",
        "Movement and Jumping",
        &[
            "tweak-armor-sinks",
            "tweak-no-aiming-slowdown",
            "tweak-jump-swims-up",
            "tweak-speed-scales-jump",
            "tweak-jump-ap-cost",
            "tweak-run-ap-cost",
            "tweak-agility-speed",
            "tweak-agility-jump",
            "tweak-back-pace",
            "tweak-crippled-leg-jump",
            "tweak-strafe-pace",
            "tweak-breath-regain",
            "tweak-walk-angle",
            "tweak-wading-pace",
        ],
    ),
    group_section(
        "Gameplay",
        "Weapons and Reloading",
        &[
            "tweak-aiming-draws-weapon",
            "tweak-full-reload-swaps",
            "tweak-hold-breath",
            "tweak-breath-ap",
            "tweak-breath-sway",
            "tweak-jam-reload-pace",
            "tweak-wheel-cycles-hotkeys",
            "tweak-no-hotkeys-reloading",
            "tweak-god-mode-no-jams",
            "tweak-npcs-earn-casings",
            "tweak-per-bullet-reload",
            "tweak-reload-press-queue",
            "tweak-reload-rounds-per-press",
            "tweak-power-attack-if-blocking",
            "tweak-ready-does-not-draw",
            "tweak-reload-when-empty",
            "tweak-reload-while-firing",
            "tweak-weapons-unhurt-by-hits",
            "tweak-no-wear-on-dead",
            "tweak-scope-wheel-zoom",
            "tweak-melee-reach",
            "tweak-npc-weapon-volume",
            "tweak-scope-zoom-time",
            "tweak-semi-auto-queue",
            "tweak-player-weapon-volume",
        ],
    ),
    group_section(
        "Gameplay",
        "V.A.T.S.",
        &[
            "tweak-vats-target-projectiles",
            "tweak-vats-target-mines",
            "tweak-vats-ignore-non-explosive",
            "tweak-vats-target-own-projectiles",
            "tweak-vats-drops-disarmed-shots",
            "tweak-vats-hip-fire",
            "tweak-vats-throw-visibility",
        ],
    ),
    group_section(
        "Gameplay",
        "Explosives",
        &[
            "tweak-contact-mines-npcs",
            "tweak-contact-mines-player",
            "tweak-disarm-needs-skill",
            "tweak-disarm-mult",
            "tweak-disarm-explode-mult",
            "tweak-explosion-radius-buff",
            "tweak-blasts-spare-items",
            "tweak-mine-blasts-heard",
            "tweak-npcs-drop-live-grenades",
            "tweak-planting-needs-skill",
            "tweak-knockdown-by-strength",
        ],
    ),
    group_section(
        "Gameplay",
        "Damage and Health",
        &[
            "tweak-aid-at-full-health",
            "tweak-crit-ignores-fire-rate",
            "tweak-character-radiation",
            "tweak-character-radiation-stages",
            "tweak-crippling-never-disarms",
            "tweak-fall-deaths-dismember",
            "tweak-drowning-drains-ap",
            "tweak-hardcore-tweaks",
            "tweak-hardcore-ammo-weight",
            "tweak-hardcore-essential-companions",
            "tweak-hardcore-sleep-heals",
            "tweak-keep-fall-height-on-load",
            "tweak-no-npc-addiction",
            "tweak-no-player-stagger",
            "tweak-power-armor-radiation",
            "tweak-power-armor-no-training",
            "tweak-standing-sneak-attacks",
            "tweak-armor-condition-penalty",
            "tweak-crippled-fall-pain",
            "tweak-fall-height-endurance",
            "tweak-power-armor-fall",
            "tweak-power-armor-limbs",
            "tweak-sleep-heal-min-hours",
        ],
    ),
    group_section(
        "Gameplay",
        "Weight",
        &[
            "tweak-grabbed-weighs",
            "tweak-run-overencumbered",
            "tweak-over-jump-ap",
            "tweak-over-run-ap",
            "tweak-weightless-worn-armor",
            "tweak-weightless-worn-power-armor",
            "tweak-encumbered-ap-regen",
        ],
    ),
    group_section(
        "Gameplay",
        "Locks, Hacking and Stealing",
        &[
            "tweak-pickpocket-caught-detects",
            "tweak-force-spares-lock",
            "tweak-grabbing-is-theft",
            "tweak-hack-dud-when-full",
            "tweak-hack-spare-guessed",
            "tweak-hack-no-repeat-words",
            "tweak-hack-no-single-chars",
            "tweak-key-name",
            "tweak-no-grab-owned",
            "tweak-no-terminal-lockout",
            "tweak-pick-without-skill",
            "tweak-pickpocket-overhaul",
            "tweak-pp-reward-xp",
            "tweak-pp-free-items",
            "tweak-pp-agility",
            "tweak-pp-base",
            "tweak-pp-detection",
            "tweak-pp-value",
            "tweak-pp-weight",
            "tweak-pp-luck",
            "tweak-pp-sneak",
            "tweak-pp-perception",
            "tweak-pp-worn",
            "tweak-pickpocket-fleeing",
            "tweak-stealing-no-alarm",
            "tweak-remember-pin-health",
            "tweak-loot-unconscious",
            "tweak-auto-unlock-terminals",
            "tweak-auto-unlock-margin",
            "tweak-skip-easy-locks",
            "tweak-skip-lock-margin",
            "tweak-lockpick-force-near",
        ],
    ),
    group_section(
        "Gameplay",
        "Trading and Repair",
        &[
            "tweak-barter-by-kind",
            "tweak-barter-water-unscaled",
            "tweak-barter-buy-aid",
            "tweak-barter-buy-ammo",
            "tweak-barter-buy-armor",
            "tweak-barter-buy-misc",
            "tweak-barter-buy-weapon",
            "tweak-barter-buy-weapon-mod",
            "tweak-barter-sell-aid",
            "tweak-barter-sell-ammo",
            "tweak-barter-sell-armor",
            "tweak-barter-sell-misc",
            "tweak-barter-sell-weapon",
            "tweak-barter-sell-weapon-mod",
            "tweak-repair-scales-crafting",
            "tweak-vendor-lists",
            "tweak-repair-caps-container",
            "tweak-repair-kits-in-list",
            "tweak-repair-at-workbench",
            "tweak-modding-needs-skill",
            "tweak-barter-repair-discount",
            "tweak-repair-xp",
            "tweak-free-item-price",
        ],
    ),
    group_section(
        "Gameplay",
        "Companions",
        &[
            "tweak-hostiles-prefer-player",
            "tweak-prefer-player-start",
            "tweak-prefer-player-mid",
            "tweak-prefer-player-detection",
            "tweak-light-step-companions",
            "tweak-no-companion-item-damage",
            "tweak-revive-companions",
            "tweak-revive-down-seconds",
            "tweak-map-summon-companions",
            "tweak-summon-unconscious",
        ],
    ),
    group_section(
        "Gameplay",
        "Travel and World",
        &[
            "tweak-travel-costs-bottle",
            "tweak-fast-travel-encumbered",
            "tweak-hold-activate",
            "tweak-hold-containers",
            "tweak-hold-furniture",
            "tweak-hold-water",
            "tweak-disable-fast-travel",
            "tweak-no-travel-crippled-leg",
            "tweak-quick-use",
            "tweak-sleep-on-chairs",
            "tweak-wait-only-seated",
            "tweak-clear-marker-within",
            "tweak-mark-nearby-places",
        ],
    ),
    // Tweaks > Combat.
    group_section("Combat", "In Combat", &["tweak-no-combat-music", "tweak-hide-enemy-markers", "tweak-no-healing-in-combat", "tweak-no-pipboy-in-combat", "tweak-kill-reward-ap"]),
    group_section("Combat", "V.A.T.S.", &["tweak-first-person-vats", "tweak-vats-burst-stops-on-kill", "tweak-vats-ignores-companions", "tweak-vats-auto-head", "tweak-vats-entry-ap"]),
    group_section(
        "Combat",
        "Criticals and Sneak Attacks",
        &["tweak-fatal-crits-gib", "tweak-no-sneak-criticals", "tweak-npcs-sneak-crit-player", "tweak-sneak-criticals-head", "tweak-sneak-criticals-melee", "tweak-crit-damage-scale"],
    ),
    group_section(
        "Combat",
        "Weapons and Armour",
        &[
            "tweak-npcs-disarm-player",
            "tweak-npc-melee-part-damage",
            "tweak-hold-release-throws",
            "tweak-no-condition-damage-buffer",
            "tweak-no-condition-damage-penalty",
            "tweak-no-spin-up",
            "tweak-npc-dr-ignores-condition",
            "tweak-npc-dt-ignores-condition",
            "tweak-npc-weapons-whole",
            "tweak-weapon-requirements-matter",
            "tweak-power-attack-ap",
        ],
    ),
    group_section(
        "Combat",
        "Companions and Blasts",
        &["tweak-no-disarm-companions", "tweak-teammates-meltdown", "tweak-no-self-meltdown", "tweak-no-companion-kill-xp", "tweak-no-self-explosion-damage"],
    ),
    // Tweaks > Quality of Life.
    group_section(
        "Quality of Life",
        "Controls and Movement",
        &[
            "tweak-seat-keeps-on-activate",
            "tweak-console-skips-player",
            "tweak-repeat-jumping",
            "tweak-hold-to-sneak",
            "tweak-hold-to-wait",
            "tweak-move-during-pipboy-raise",
            "tweak-no-knockdown-god",
            "tweak-no-land-clip",
            "tweak-toggle-device",
            "tweak-seated-wide",
        ],
    ),
    group_section(
        "Quality of Life",
        "Weapons and Aiming",
        &[
            "tweak-dpad-cycles-hotkeys",
            "tweak-double-tap-ammo",
            "tweak-hi-res-npc-weapons",
            "tweak-holster-real-time",
            "tweak-hotkey-toggles-in-hand",
            "tweak-night-vision-toggle",
            "tweak-night-vision-by-day",
            "tweak-night-vision-off-first",
            "tweak-no-sight-zoom",
            "tweak-no-weapon-move-penalty",
            "tweak-npcs-drop-holstered",
            "tweak-empty-reload-swaps-ammo",
            "tweak-vats-shift-ignores-friends",
            "tweak-swap-ammo-holstered",
            "tweak-deselect-hotkeys",
            "tweak-vats-keeps-holstered",
            "tweak-aim-turn-x",
            "tweak-aim-turn-y",
            "tweak-scope-delay",
        ],
    ),
    group_section(
        "Quality of Life",
        "Items and Repair",
        &[
            "tweak-equip-broken-items",
            "tweak-hide-misc-quest",
            "tweak-keep-broken-equipped",
            "tweak-no-poison-confirm",
            "tweak-repair-needs-weapon-skill",
            "tweak-unequip-broken-armor",
            "tweak-stack-figures",
        ],
    ),
    group_section("Quality of Life", "Companions", &["tweak-quiet-followers", "tweak-robots-heal-with-scrap", "tweak-robot-heal-base", "tweak-robot-heal-per-repair", "tweak-robot-heal-expert"]),
    group_section(
        "Quality of Life",
        "Minigames and Machines",
        &["tweak-faster-hacking", "tweak-no-tutorials", "tweak-pick-locks-with-key", "tweak-vigor-review-first", "tweak-show-current-skill", "tweak-slots-hold-to-spin"],
    ),
    group_section("Quality of Life", "Travel and Loading", &["tweak-unsafe-fast-travel", "tweak-midair-fast-travel", "tweak-pause-on-load", "tweak-plain-going-out"]),
    group_section(
        "Quality of Life",
        "Messages and Sounds",
        &[
            "tweak-mark-unspoken",
            "tweak-no-quest-set-on-complete",
            "tweak-no-quest-set-on-objective",
            "tweak-no-empty-book-message",
            "tweak-god-mode-quiet-landing",
            "tweak-no-reputation-messages",
            "tweak-xp-sound-at-cap",
            "tweak-player-pain-gap",
        ],
    ),
    group_section("Quality of Life", "Conversations and Radio", &["tweak-no-auto-continue-dialogue", "tweak-select-unavailable-radios", "tweak-dialogue-music", "tweak-dialogue-radio"]),
    group_section(
        "Quality of Life",
        "Screen and Camera",
        &[
            "tweak-faster-pad-orbit",
            "tweak-move-during-vats-camera",
            "tweak-no-chem-wore-off",
            "tweak-no-explosion-blur",
            "tweak-no-interior-fog",
            "tweak-scale-explosion-shake",
            "tweak-explosion-shake",
            "tweak-shift-screenshot-hides",
            "tweak-skip-death-cam",
            "tweak-skip-death-cam-attack",
            "tweak-kill-cam-exit-key",
            "tweak-min-splatter",
        ],
    ),
    // Tweaks > HUD.
    group_section(
        "HUD",
        "Messages",
        &[
            "tweak-crit-message-limb",
            "tweak-discovered-in-corner",
            "tweak-descriptive-marker-added",
            "tweak-no-caps-added",
            "tweak-no-dehydration-messages",
            "tweak-no-location-popup",
            "tweak-no-objective-popup",
            "tweak-no-quest-failed",
            "tweak-no-sleep-messages",
            "tweak-no-unconscious-message",
            "tweak-no-xp-meter-combat",
            "tweak-objectives-on-arrival",
            "tweak-objectives-gap",
            "tweak-no-xp-messages",
            "tweak-no-karma-messages",
            "tweak-item-added-seconds",
        ],
    ),
    group_section(
        "HUD",
        "Compass and Place Names",
        &[
            "tweak-compass-fade-left",
            "tweak-compass-pips-danger",
            "tweak-hide-unmapped-places",
            "tweak-compass-doors",
            "tweak-compass-doors-fade",
            "tweak-compass-doors-interior",
            "tweak-compass-doors-exterior",
            "tweak-compass-pips-fade",
            "tweak-compass-places-fade",
            "tweak-neutral-foes-compass",
            "tweak-nearest-unfound",
            "tweak-hud-marker-name",
            "tweak-marker-name-hide-unfound",
            "tweak-marker-name-units",
            "tweak-marker-name-angle",
            "tweak-compass-quest-height",
            "tweak-hud-region-names",
            "tweak-compass-marker-height",
            "tweak-companion-pip",
            "tweak-quest-marker-reach",
            "tweak-recent-dead-secs",
        ],
    ),
    group_section(
        "HUD",
        "Crosshair",
        &["tweak-crosshair-when-aiming", "tweak-no-red-crosshair-cloaked", "tweak-hide-crosshair-first", "tweak-hide-crosshair-third", "tweak-hide-crosshair-reloading", "tweak-no-red-crosshair"],
    ),
    group_section(
        "HUD",
        "Bars and Indicators",
        &[
            "tweak-ammo-label-under",
            "tweak-armor-condition",
            "tweak-armor-in-weapon-place",
            "tweak-armor-flash-below",
            "tweak-armor-red-below",
            "tweak-armor-visible-below",
            "tweak-hud-fatigue",
            "tweak-buffed-health-bars",
            "tweak-no-grenade-indicator",
            "tweak-grenade-indicator-destructibles",
            "tweak-precise-rad-meter",
            "tweak-rad-decimals",
            "tweak-hide-ammo-label",
            "tweak-no-limb-indicator",
            "tweak-grenade-indicator-harmless",
            "tweak-weapon-cnd-flash-below",
            "tweak-red-ap-below",
            "tweak-red-hp-below",
            "tweak-weapon-cnd-red-below",
        ],
    ),
    group_section("HUD", "Sneaking", &["tweak-detected-by-whom", "tweak-detected-names", "tweak-no-sneak-label", "tweak-shift-talks-sneaking", "tweak-sneak-label-standing"]),
    group_section(
        "HUD",
        "Prompts and Subtitles",
        &["tweak-hide-unknown-names", "tweak-mark-unvisited-doors", "tweak-terminal-password-prompt", "tweak-subtitle-names", "tweak-water-shows-h2o", "tweak-subtitle-distance"],
    ),
    group_section("HUD", "Other", &["tweak-female-facial-hair", "tweak-no-hotkey-wheel", "tweak-date-style"]),
    // Tweaks > Menus.
    group_section(
        "Menus",
        "Pip-Boy Items",
        &[
            "tweak-tab-click-sorts",
            "tweak-compare-armor",
            "tweak-compare-weapons",
            "tweak-mark-other-ammo",
            "tweak-hide-quest-item-figures",
            "tweak-no-right-click-drop",
            "tweak-single-item-figures",
            "tweak-drop-key",
            "tweak-remember-item-places",
            "tweak-weapon-ammo-use",
            "tweak-show-poison-effects",
            "tweak-book-effects",
            "tweak-usable-ammo-first",
        ],
    ),
    group_section(
        "Menus",
        "Pip-Boy Stats and Data",
        &[
            "tweak-pipboy-12-hour",
            "tweak-detailed-stats",
            "tweak-effect-time-left",
            "tweak-stats-xp-percent",
            "tweak-hide-empty-reputation",
            "tweak-hide-completed-quests",
            "tweak-hide-read-notes",
            "tweak-mark-notes-unread",
            "tweak-mark-recurring-challenges",
            "tweak-no-show-quest-location",
            "tweak-notes-of-all-quests",
            "tweak-quest-text-in-menus",
            "tweak-show-fame-infamy",
            "tweak-sort-misc-stats",
            "tweak-embolden-tags",
        ],
    ),
    group_section(
        "Menus",
        "Maps",
        &[
            "tweak-companions-location-font",
            "tweak-no-min-companion-distance",
            "tweak-map-door-kinds",
            "tweak-map-unconscious-companions",
            "tweak-local-map-reset-cells",
            "tweak-map-marker-distance",
            "tweak-map-recentre-key",
            "tweak-map-remembers",
            "tweak-map-zoom-at-pointer",
            "tweak-recent-markers-red",
            "tweak-no-marker-popup",
            "tweak-marker-removed-on-it",
            "tweak-marker-shift-removes",
            "tweak-hide-marker-reputation",
            "tweak-quiet-undiscovered",
        ],
    ),
    group_section(
        "Menus",
        "Containers and Barter",
        &[
            "tweak-barter-caps-change",
            "tweak-full-barter-total",
            "tweak-containers-show-ammo",
            "tweak-hide-hotkeyed-barter",
            "tweak-hide-hotkeyed-containers",
            "tweak-hide-equipped-barter",
            "tweak-hide-equipped-containers",
            "tweak-sync-categories",
            "tweak-quantity-shows-price",
            "tweak-quantity-shows-weight",
            "tweak-quantity-companion-cap",
            "tweak-title-case-categories",
            "tweak-invert-title-wheel",
            "tweak-right-click-previous-category",
            "tweak-store-all",
            "tweak-take-all-confirm",
            "tweak-take-all-confirm-min",
            "tweak-take-all-stays",
            "tweak-warn-encumbrance",
        ],
    ),
    group_section(
        "Menus",
        "Repair and Crafting",
        &[
            "tweak-hide-hotkeyed-repair",
            "tweak-recipe-caps",
            "tweak-repair-all-confirm",
            "tweak-repair-all-threshold",
            "tweak-repair-preview",
            "tweak-repair-low-first",
            "tweak-repair-same-first",
            "tweak-repair-any-armor",
        ],
    ),
    group_section(
        "Menus",
        "Level-Up and Perks",
        &["tweak-levelup-wheel", "tweak-living-anatomy-dr", "tweak-hide-trait-ranks", "tweak-perk-plugin-names", "tweak-skip-skill-page", "tweak-preview-perks"],
    ),
    group_section(
        "Menus",
        "Terminals and Minigames",
        &[
            "tweak-blackjack-total",
            "tweak-grey-read-notes",
            "tweak-minigames-timescale",
            "tweak-lockpick-move-keys",
            "tweak-lockpick-mouse",
            "tweak-no-exit-hacking",
            "tweak-terminal-number-keys",
            "tweak-tab-closes-terminals",
        ],
    ),
    group_section("Menus", "Messages and Conversations", &["tweak-need-icons", "tweak-keep-acting-notes", "tweak-no-pointer-dialogue", "tweak-no-pointer-message-box"]),
    group_section("Menus", "Main and Pause Menus", &["tweak-continue-shows-save", "tweak-no-loading-tips", "tweak-quick-load-in-menus", "tweak-tab-back-in-pause"]),
    group_section(
        "Menus",
        "Keys and Mouse",
        &["tweak-key-repeat-accel", "tweak-key-repeat-delay", "tweak-key-repeat-mult", "tweak-key-repeat-delay-alt", "tweak-key-repeat-mult-alt", "tweak-horizontal-look"],
    ),
    // Tweaks > Hotkeys.
    group_section("Hotkeys", "Hide HUD Key", &["tweak-hide-hud-ammo", "tweak-hide-hud-needs", "tweak-hide-hud-compass", "tweak-hide-hud-on-load", "tweak-hide-hud-sneak"]),
    group_section("Hotkeys", "Other Keys", &["tweak-place-marker-here-says", "tweak-repeat-activate-takes-only"]),
    // Tweaks > Cheats.
    group_section("Cheats", "Weight", &["tweak-weightless-aid", "tweak-weightless-ammo", "tweak-weightless-armor", "tweak-weightless-misc", "tweak-weightless-weapons"]),
    group_section(
        "Cheats",
        "Hacking, Stealing and Casinos",
        &["tweak-hack-without-skill", "tweak-no-casino-bans", "tweak-no-max-casino-bet", "tweak-pickpocket-after-caught", "tweak-pickpocket-worn"],
    ),
    group_section(
        "Cheats",
        "Travel, Rest and Skills",
        &["tweak-console-homes", "tweak-fast-travel-enemies", "tweak-god-mode-free-crafting", "tweak-reallocate-skill-points", "tweak-sleep-in-owned-beds", "tweak-wait-anywhere"],
    ),
    // Tweaks > Misc.
    group_section(
        "Misc",
        "Sound and Radio",
        &[
            "tweak-level-sound-a",
            "tweak-level-sound-every-a",
            "tweak-level-sound-b",
            "tweak-level-sound-every-b",
            "tweak-discovered-sound",
            "tweak-armor-sounds-3d",
            "tweak-heartbeat-fade",
            "tweak-heartbeat-fade-seconds",
            "tweak-no-voice-filter",
            "tweak-quiet-hotkeys",
            "tweak-quiet-teammate-steps",
            "tweak-radiation-radio-static",
            "tweak-radio-music",
            "tweak-radio-talk",
        ],
    ),
    group_section(
        "Misc",
        "Camera",
        &[
            "tweak-kill-cam-hit-location",
            "tweak-kill-cam-outside-vats",
            "tweak-vats-stop-following-dead",
            "tweak-vanity-returns-first",
            "tweak-pov-follows-weapon",
            "tweak-camera-wheel-pov",
            "tweak-kill-cam-chance",
        ],
    ),
    group_section(
        "Misc",
        "Effects and Animation",
        &[
            "tweak-armor-blocks-blood",
            "tweak-scale-ash-piles",
            "tweak-detached-beams",
            "tweak-power-armor-no-screen-blood",
            "tweak-no-stealth-shimmer-pipboy",
            "tweak-no-turn-clip",
            "tweak-splash-torso-gib",
            "tweak-weapon-impacts",
        ],
    ),
    group_section(
        "Misc",
        "Controller and Keys",
        &[
            "tweak-custom-dead-zones",
            "tweak-left-dead-zone",
            "tweak-right-dead-zone",
            "tweak-invert-x",
            "tweak-keys-with-pad",
            "tweak-responsive-stick",
            "tweak-no-weapon-hotkeys",
            "tweak-pipboy-buzz",
            "tweak-duplicate-binds",
            "tweak-custom-trigger-dead-zones",
            "tweak-lt-dead-zone",
            "tweak-rt-dead-zone",
        ],
    ),
    group_section(
        "Misc",
        "Console",
        &[
            "tweak-better-flycam",
            "tweak-help-is-console",
            "tweak-print-plugin-names",
            "tweak-console-output-file",
            "tweak-console-model-details",
            "tweak-console-text-shadow",
            "tweak-console-timestamps",
            "tweak-console-compress-repeats",
            "tweak-remember-console-history",
        ],
    ),
    group_section("Misc", "Menus and Conversations", &["tweak-no-skip-dialogue", "tweak-pipboy-opens-items", "tweak-terminal-fade", "tweak-terminal-fade-seconds"]),
    group_section("Misc", "Starting and Loading", &["tweak-auto-continue", "tweak-god-mode-off-on-load", "tweak-no-respec", "tweak-no-name-limit", "tweak-click-to-exit-loading"]),
    group_section(
        "Misc",
        "World and Items",
        &[
            "tweak-no-travel-time",
            "tweak-improved-weather",
            "tweak-weather-trip-distance",
            "tweak-lethal-knockdowns",
            "tweak-no-selling",
            "tweak-no-skill-books-at-max",
            "tweak-no-ammo-casings",
            "tweak-pick-up-disarmed",
            "tweak-no-container-after-pick",
            "tweak-no-door-after-pick",
            "tweak-region-borders-message-only",
        ],
    ),
    section("Debug", None, &["overlay", "hit-markers", "god-mode", "navmesh", "terrain-wireframe", "scope-3d"]),
    // Android-owned rows keep their registered keys and callbacks on desktop.
    section("Gameplay", Some("Inventory menus"), &["touch-barter-v1", "touch-maintenance-v1", "touch-terminal-v1", "touch-companion-v1"]),
    section("Controls", Some("Touch"), &["touch-layout-editor-v1", "touch-practice-v1", "touch-hotbar", "touch-hotbar-slots", "touch-hotbar-size", "touch-hotbar-tray", "touch-tray-editor-v1", "touch-bottom-swipe-target-v1", "touch-pipboy-swipe-v1", "touch-pipboy-distance-v1", "touch-pipboy-dismiss-left", "touch-pipboy-dismiss-right", "touch-pipboy-dismiss-side-v2", "touch-pipboy-dismiss-travel-v1", "touch-pipboy-close-button", "touch-pipboy-button-graphics", "touch-pipboy-model-wheel", "touch-direct-pipboy-v1", "touch-world-activation-v1", "touch-icon-family", "adaptive-touch-actions", "lockpick-touch", "lockpick-touch-size", "lockpick-touch-hints", "touch-gyro-v1", "touch-gyro-sensitivity-v1", "touch-gyro-ads-v1", "touch-gyro-toggle-v1", "touch-gyro-ads-only-v1", "touch-gyro-recenter-v1", "touch-gyro-invert-x-v1", "touch-gyro-invert-y-v1", "touch-analog-movement-v1", "touch-fire-drag-look-v1", "touch-auto-move-lock-v1", "touch-vats-double-tap-v1", "touch-vats-targeting-v1"]),
    section("Controls", Some("Controller"), &["controller-ads-v1", "controller-dead-zone-v1", "controller-toggle-aim-v1", "controller-hold-sneak-v1"]),
    section("Display", Some("Mobile performance"), &["mobile-performance", "mobile-fps", "mobile-resolution", "mobile-thermal-auto", "mobile-thermal-status"]),
];

/// Android claims only its named presentation overrides. Main's original controller
/// rows keep their Input/Aim Assist categories, while all hardware routes stay Controls.
pub const MOBILE_SECTIONS: &[Section] = &[
    section(MOBILE_TAB, Some("Touch layout and guide"), &[
        "touch-layout-editor-v1", "touch-practice-v1", "touch-icon-family", "adaptive-touch-actions",
    ]),
    section(MOBILE_TAB, Some("Movement and gestures"), &[
        "touch-analog-movement-v1", "touch-fire-drag-look-v1",
        "touch-auto-move-lock-v1", "touch-vats-double-tap-v1", "touch-vats-targeting-v1", "touch-world-activation-v1",
    ]),
    section(MOBILE_TAB, Some("Gyroscope"), &[
        "touch-gyro-v1", "touch-gyro-sensitivity-v1", "touch-gyro-ads-v1",
        "touch-gyro-toggle-v1", "touch-gyro-ads-only-v1", "touch-gyro-recenter-v1", "touch-gyro-invert-x-v1", "touch-gyro-invert-y-v1",
    ]),
    section(MOBILE_TAB, Some("Inventory and quick access"), &[
        "touch-hotbar", "touch-hotbar-slots", "touch-hotbar-size", "touch-hotbar-tray", "touch-tray-editor-v1",
        "touch-pipboy-swipe-v1", "touch-bottom-swipe-target-v1", "touch-pipboy-distance-v1", "touch-pipboy-size-v1", "touch-pipboy-tab-swipes", "touch-direct-pipboy-v1",
        "touch-pipboy-dismiss-left", "touch-pipboy-dismiss-right", "touch-pipboy-dismiss-side-v2", "touch-pipboy-dismiss-travel-v1", "touch-pipboy-close-button",
        "touch-pipboy-button-graphics", "touch-pipboy-model-wheel",
        "touch-barter-v1", "touch-maintenance-v1", "touch-terminal-v1", "touch-companion-v1",
    ]),
    section(MOBILE_TAB, Some("Dialogue"), &[
        "touch-dialogue", "dialogue-choice-text-size", "dialogue-subtitle-text-size", "dialogue-touch-choice", "dialogue-speaker-side",
        "dialogue-theme-v1", "dialogue-background-opacity-v1", "dialogue-background-tint-v1", "dialogue-frame-opacity-v1", "dialogue-hints-v1",
    ]),
    section(MOBILE_TAB, Some("Lockpicking"), &["lockpick-touch", "lockpick-touch-size", "lockpick-touch-hints"]),
    section(MOBILE_TAB, Some("Interface size"), &[
        "menu-scale", "ui-scale", "hud-scale-v2", "hud-health-size-v1", "hud-action-points-size-v1", "hud-compass-size-v1",
        "barter-menu-size-v1", "repair-menu-size-v1", "workbench-menu-size-v1",
    ]),
    section(CONTROLS_TAB, Some("Controller"), &ANDROID_CONTROLLER_KEYS),
    section(MOBILE_TAB, Some("Performance and battery"), &[
        "mobile-performance", "mobile-fps", "mobile-resolution", "mobile-thermal-auto", "mobile-thermal-status",
    ]),
];

/// Rows that cannot apply while another row is off: (the row, the row it needs, why), the
/// row greyed with its value kept and the reason in its note while that row is off (a
/// switch off, a list on its first choice) [design: the menu overhaul, owner 2026-11-15:
/// "rows that can't apply: grey them out"]. `crate::settings::Settings::disabled_status`
/// reads it; a row of a missing module is passed over.
pub const NEEDS: &[(&str, &str, &str)] = &[
    ("dialogue-recentre", "dialogue-look", "Needs Turn to Each Other."),
    ("volume-music", "music", "Needs Play Music."),
    ("volume-voice", "voice", "Needs Dialogue Voice."),
    ("crosshair-style", "dynamic-crosshair", "Needs Dynamic Crosshair."),
    ("aim-slowdown", "aim-assist", "Needs Aim Assist."),
    ("aim-lock", "aim-assist", "Needs Aim Assist."),
    ("shadow-filter", "sun-shadows", "Needs Sun Shadows."),
    ("shadow-distance", "sun-shadows", "Needs Sun Shadows."),
    ("fine-shadows", "sun-shadows", "Needs Sun Shadows."),
    ("eye-adaptation", "glow", "Needs Light Glow (HDR)."),
    ("neon-glow", "glow", "Needs Light Glow (HDR)."),
    ("grass-distance", "grass", "Needs Grass."),
    ("fps-counter-fps", "fps-counter", "Needs the Frame Rate Counter."),
    ("fps-counter-graph", "fps-counter", "Needs the Frame Rate Counter."),
    ("fps-counter-split", "fps-counter", "Needs the Frame Rate Counter."),
    ("fps-counter-cpu", "fps-counter", "Needs the Frame Rate Counter."),
    ("fps-counter-gpu", "fps-counter", "Needs the Frame Rate Counter."),
    ("fps-counter-vram", "fps-counter", "Needs the Frame Rate Counter."),
    ("fps-counter-ram", "fps-counter", "Needs the Frame Rate Counter."),
    ("fps-counter-loaded", "fps-counter", "Needs the Frame Rate Counter."),
    ("fps-counter-upscaling", "fps-counter", "Needs the Frame Rate Counter."),
    ("fps-counter-low", "fps-counter", "Needs the Frame Rate Counter."),
    ("fps-counter-low", "fps-counter-fps", "Needs Counter: Frame Rate."),
];

/// The Controls tab's rows that are shown only while the input device is a controller
/// (`crate::bindings::InputDevice`) [design: #1953].
pub const ANDROID_CONTROLLER_KEYS: [&str; 4] = ["controller-ads-v1", "controller-dead-zone-v1", "controller-toggle-aim-v1", "controller-hold-sneak-v1"];
pub const CONTROLLER_KEYS: [&str; 8] = ["button-style", "aim-assist", "aim-slowdown", "aim-lock",
    "controller-ads-v1", "controller-dead-zone-v1", "controller-toggle-aim-v1", "controller-hold-sneak-v1"];

/// Whether row `key` is shown only with a controller ([`CONTROLLER_KEYS`]).
pub fn controller_only(key: &str) -> bool {
    CONTROLLER_KEYS.contains(&key)
}

/// The table's faults: a key named twice, a tab not in [`TAB_ORDER`], a group outside the
/// Tweaks tab, a category without a heading on a tab with categories, a key in NVF Features
/// the owner did not name.
pub fn check(sections: &[Section]) -> Result<(), String> {
    let mut seen: Vec<&str> = Vec::new();
    for s in sections {
        if !TAB_ORDER.contains(&s.placement.tab) {
            return Err(format!("{}: not one of the tabs", s.placement.tab));
        }
        if s.placement.subgroup.is_some() && s.placement.tab != TWEAKS_TAB {
            return Err(format!("{}: a group is for the Tweaks tab only", s.placement.path()));
        }
        if s.keys.is_empty() {
            return Err(format!("{}: an empty category", s.placement.path()));
        }
        for k in s.keys {
            if seen.contains(k) {
                return Err(format!("{k}: placed twice"));
            }
            if s.placement.tab == NVF_FEATURES_TAB && !NVF_FEATURES_KEYS.contains(k) {
                return Err(format!("{k}: NVF Features holds only what the owner named there"));
            }
            if k.starts_with(CONTROL_PREFIX) && s.placement.tab != CONTROLS_TAB {
                return Err(format!("{k}: a Controls row outside the Controls tab"));
            }
            seen.push(k);
        }
    }
    for tab in TAB_ORDER {
        let of_tab: Vec<&Section> = sections.iter().filter(|s| s.placement.tab == tab).collect();
        if of_tab.len() > 1 && of_tab.iter().any(|s| s.placement.heading.is_none()) {
            return Err(format!("{tab}: every category of a tab with several needs a heading"));
        }
    }
    Ok(())
}

/// One line of a section: a registered row (its index in the registry) or a Controls row
/// (its name after [`CONTROL_PREFIX`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutItem {
    Setting(usize),
    Control(&'static str),
}

/// A section of the rows: its place and its lines in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedSection {
    pub placement: Placement,
    /// Its registered rows (indices in the registry), in order.
    pub rows: Vec<usize>,
    /// Its lines in order: the registered rows and the Controls rows among them.
    pub items: Vec<LayoutItem>,
}

/// A tab and its sections.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedTab {
    pub name: &'static str,
    pub sections: Vec<ResolvedSection>,
}

/// The Tweaks tab's group for rows the table does not place in one yet (a tweak added after
/// the table was written): listed last, so such a row is never out of sight [design].
pub const UNGROUPED: &str = "Other";

impl ResolvedTab {
    /// The Tweaks tab's groups, in the table's order, and [`UNGROUPED`] last while a row of
    /// the tab has none (no groups on another tab).
    pub fn subgroups(&self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for g in self.sections.iter().filter_map(|s| s.placement.subgroup) {
            if !out.contains(&g) {
                out.push(g);
            }
        }
        if !out.is_empty() && self.sections.iter().any(|s| s.placement.subgroup.is_none()) {
            out.push(UNGROUPED);
        }
        out
    }
}

/// One line of a tab as the table gives it: a category's heading, a registered row, a
/// Controls row, or the empty line after an open category.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutEntry {
    Heading(Placement),
    Setting(usize),
    Control(&'static str),
    Gap,
}

impl From<LayoutItem> for LayoutEntry {
    fn from(i: LayoutItem) -> LayoutEntry {
        match i {
            LayoutItem::Setting(i) => LayoutEntry::Setting(i),
            LayoutItem::Control(c) => LayoutEntry::Control(c),
        }
    }
}

/// The table applied to the registered rows ([`resolve_layout`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResolvedLayout {
    pub tabs: Vec<ResolvedTab>,
    /// Each registered row's place, by its index.
    placements: Vec<Placement>,
    /// The rows the table does not name, the Tweaks tab's left out
    /// ([`ResolvedLayout::unnamed`]).
    unnamed: usize,
}

/// The table applied to `list`, the registry in its order (module notes): the tabs (an
/// empty one too), each named section with the rows registered and its Controls rows (a
/// missing optional row left out, a section left with none dropped), then each tab's
/// unnamed rows, then the unnamed groups' tabs. Every registered row is in it once.
pub fn resolve_layout(list: &[Setting]) -> ResolvedLayout {
    resolve_layout_with(list, &TAB_ORDER, if cfg!(target_os = "android") { MOBILE_SECTIONS } else { &[] })
}

#[cfg(test)]
pub(crate) fn resolve_desktop_layout(list: &[Setting]) -> ResolvedLayout {
    resolve_layout_with(list, &DESKTOP_TAB_ORDER, &[])
}

fn resolve_layout_with(list: &[Setting], tab_order: &[&'static str], overrides: &[Section]) -> ResolvedLayout {
    let index: std::collections::HashMap<&str, usize> = list.iter().enumerate().map(|(i, s)| (s.key, i)).collect();
    let mut placements: Vec<Option<Placement>> = vec![None; list.len()];
    let mut tabs: Vec<ResolvedTab> = tab_order.iter().map(|&name| ResolvedTab { name, sections: Vec::new() }).collect();
    for s in overrides.iter().chain(SECTIONS) {
        let mut items = Vec::new();
        for k in s.keys {
            if let Some(c) = k.strip_prefix(CONTROL_PREFIX) {
                items.push(LayoutItem::Control(c));
            } else if let Some(&i) = index.get(k).filter(|&&i| placements[i].is_none()) {
                placements[i] = Some(s.placement);
                items.push(LayoutItem::Setting(i));
            }
        }
        if items.is_empty() {
            continue;
        }
        let rows = items.iter().filter_map(|i| if let LayoutItem::Setting(i) = i { Some(*i) } else { None }).collect();
        if let Some(t) = tabs.iter_mut().find(|t| t.name == s.placement.tab) {
            t.sections.push(ResolvedSection { placement: s.placement, rows, items });
        }
    }
    // The unnamed rows, by tab, in registry order: one section after each tab's named ones.
    let mut unnamed: Vec<(usize, Vec<usize>)> = Vec::new();
    for (i, s) in list.iter().enumerate() {
        if placements[i].is_some() {
            continue;
        }
        placements[i] = Some(Placement::tab(s.group));
        let t = match tabs.iter().position(|t| t.name == s.group) {
            Some(t) => t,
            None => {
                tabs.push(ResolvedTab { name: s.group, sections: Vec::new() });
                tabs.len() - 1
            }
        };
        match unnamed.iter_mut().find(|(at, _)| *at == t) {
            Some((_, rows)) => rows.push(i),
            None => unnamed.push((t, vec![i])),
        }
    }
    // The Tweaks tab's rows are not counted: #1892 registers them under Tweaks without
    // naming them here, by design (module notes), so the count stays the rows another
    // tab's table left out [design: #1953, #1892].
    let count = unnamed.iter().filter(|(t, _)| tabs[*t].name != TWEAKS_TAB).map(|(_, r)| r.len()).sum();
    for (t, rows) in unnamed {
        let placement = Placement::tab(tabs[t].name);
        let items = rows.iter().map(|&i| LayoutItem::Setting(i)).collect();
        tabs[t].sections.push(ResolvedSection { placement, rows, items });
    }
    ResolvedLayout { tabs, placements: placements.into_iter().map(|p| p.unwrap_or(Placement::tab(""))).collect(), unnamed: count }
}

impl ResolvedLayout {
    /// The tabs' names, in order.
    pub fn tab_names(&self) -> Vec<&'static str> {
        self.tabs.iter().map(|t| t.name).collect()
    }

    pub fn tab(&self, name: &str) -> Option<&ResolvedTab> {
        self.tabs.iter().find(|t| t.name == name)
    }

    /// Registered row `i`'s place.
    pub fn placement_of(&self, i: usize) -> Option<Placement> {
        self.placements.get(i).copied()
    }

    /// The place of the Controls row named `name` (after [`CONTROL_PREFIX`]).
    pub fn control_placement(&self, name: &str) -> Option<Placement> {
        self.tabs.iter().flat_map(|t| &t.sections).find(|s| s.items.iter().any(|i| matches!(i, LayoutItem::Control(c) if *c == name))).map(|s| s.placement)
    }

    /// How many registered rows the table does not name, leaving out the Tweaks tab's
    /// ([`TWEAKS_TAB`]), which #1892 lists in registry order without naming them here
    /// [design: #1953, #1892].
    pub fn unnamed(&self) -> usize {
        self.unnamed
    }

    /// Every registered row in the page's order: tab by tab, section by section (what
    /// search and the Changed list go by).
    pub fn ordered_settings(&self) -> impl Iterator<Item = usize> + '_ {
        self.tabs.iter().flat_map(|t| t.sections.iter().flat_map(|s| s.rows.iter().copied()))
    }

    /// The section at `p`.
    pub fn section(&self, p: &Placement) -> Option<&ResolvedSection> {
        self.tab(p.tab)?.sections.iter().find(|s| s.placement == *p)
    }

    /// Tab `name`'s lines (of its group `subgroup` on the Tweaks tab; `None`: every
    /// section): each category with a line `visible` says is shown, its heading (when it has
    /// one) and, while `open` says it is open, those lines; after each open category with
    /// another after it, an empty line [design: the overhaul]. A category whose lines are all
    /// hidden is left out, heading too; a hidden line keeps its place for when it shows.
    pub fn tab_entries(&self, name: &str, subgroup: Option<&str>, visible: impl Fn(LayoutItem) -> bool, open: impl Fn(&Placement) -> bool) -> Vec<LayoutEntry> {
        let mut out = Vec::new();
        let Some(t) = self.tab(name) else { return out };
        let mut after_open = false;
        let of_group = |s: &&ResolvedSection| match subgroup {
            None => true,
            Some(UNGROUPED) => s.placement.subgroup.is_none(),
            g => s.placement.subgroup == g,
        };
        for s in t.sections.iter().filter(of_group) {
            let items: Vec<LayoutItem> = s.items.iter().copied().filter(|&i| visible(i)).collect();
            if items.is_empty() {
                continue;
            }
            if after_open {
                out.push(LayoutEntry::Gap);
            }
            let headed = s.placement.heading.is_some();
            let shows = !headed || open(&s.placement);
            if headed {
                out.push(LayoutEntry::Heading(s.placement));
            }
            if shows {
                out.extend(items.into_iter().map(LayoutEntry::from));
            }
            after_open = shows;
        }
        out
    }

    /// How many of the category at `p`'s lines `visible` says are shown (what its heading
    /// counts while chosen).
    pub fn shown_count(&self, p: &Placement, visible: impl Fn(LayoutItem) -> bool) -> usize {
        self.section(p).map_or(0, |s| s.items.iter().filter(|&&i| visible(i)).count())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;
    use super::{resolve_desktop_layout as resolve_layout, DESKTOP_TAB_ORDER as TAB_ORDER};

    fn row(key: &'static str, group: &'static str) -> Setting {
        Setting { key, group, label: key, note: "", switches: "", get: |_| false, set: |_, _| {}, slider: None }
    }

    /// The registry as `Settings::add` orders it.
    fn registry(rows: &[(&'static str, &'static str)]) -> Vec<Setting> {
        let mut s = Settings::default();
        for (k, g) in rows {
            s.add(row(k, g));
        }
        s.list().to_vec()
    }

    /// A tab's lines, every category open, every line shown: `[Heading]`, keys, `@control`,
    /// and "" for an empty line.
    fn keys(list: &[Setting], l: &ResolvedLayout, tab: &str) -> Vec<String> {
        lines(list, l, tab, |_| true)
    }

    fn lines(list: &[Setting], l: &ResolvedLayout, tab: &str, open: impl Fn(&Placement) -> bool) -> Vec<String> {
        l.tab_entries(tab, None, |_| true, open)
            .into_iter()
            .map(|e| match e {
                LayoutEntry::Heading(p) => format!("[{}]", p.heading.unwrap_or("")),
                LayoutEntry::Setting(i) => list[i].key.to_string(),
                LayoutEntry::Control(c) => format!("@{c}"),
                LayoutEntry::Gap => String::new(),
            })
            .collect()
    }

    /// Every row the registry can hold today (upstream main be6db0948: #1966's quality row
    /// (its enhanced row removed by the menu overhaul), #2014's counter rows, the VATS rows,
    /// #2198's upscaler rows, #2169's FSR library row, #2242's upscaling counter row, #2317's
    /// Generated frames row, the field of view rows, the menu scale and Physics rate rows,
    /// #2662's grass fires, #2550's terrain height blending, #2551's terrain anti-tiling,
    /// #2688's sound cover, #2149's smooth terrain and its Debug row terrain wireframe, the
    /// overhaul's menu font, and the Advanced Movement rows of tech/movement-inertia, Weapon
    /// Inertia and Camera Motion in place of weapon sway and head bob), by the group it
    /// registers with.
    const KNOWN: [(&str, &str); 193] = [
        ("detached-camera", "Gameplay"),
        ("advanced-movement", "Gameplay"),
        ("movement-weight", "Gameplay"),
        ("movement-inertia", "Gameplay"),
        ("weapon-handling", "Gameplay"),
        ("advanced-recoil", "Gameplay"),
        ("animation-smoothing", "Gameplay"),
        ("sound-cover", "Gameplay"),
        ("menu-pause", "Gameplay"),
        ("dialogue-goodbye-hint", "Gameplay"),
        ("reload-slows", "Gameplay"),
        ("grass-fire", "Gameplay"),
        ("music", "Gameplay"),
        ("voice", "Gameplay"),
        ("dialogue-look", "Gameplay"),
        ("dialogue-recentre", "Gameplay"),
        ("dialogue-zoom", "Gameplay"),
        ("dynamic-crosshair", "Gameplay"),
        ("crosshair-style", "Gameplay"),
        ("sneak-gauges", "Gameplay"),
        ("UsePipBoy", "Gameplay"),
        ("kill-camera", "Gameplay"),
        ("vats-style", "Gameplay"),
        ("vats-slowdown", "Gameplay"),
        ("difficulty", "Gameplay"),
        ("hardcore", "Gameplay"),
        ("lip-sync", "Gameplay"),
        ("true-iron-sights", "Gameplay"),
        ("scope-3d", "Gameplay"),
        ("button-style", "Controller"),
        ("aim-assist", "Controller"),
        ("aim-slowdown", "Controller"),
        ("aim-lock", "Controller"),
        ("autosave-travel", "Saving"),
        ("autosave-fast-travel", "Saving"),
        ("autosave-wait", "Saving"),
        ("autosave-sleep", "Saving"),
        ("autosave-scripts", "Saving"),
        ("hud-colour", "Interface"),
        ("hud-red", "Interface"),
        ("hud-green", "Interface"),
        ("hud-blue", "Interface"),
        ("pipboy-colour", "Interface"),
        ("pipboy-red", "Interface"),
        ("pipboy-green", "Interface"),
        ("pipboy-blue", "Interface"),
        ("language", "Interface"),
        ("menu-font", "Interface"),
        ("head-tracking", "World"),
        ("foot-ik", "World"),
        ("sky-clock", "World"),
        ("resolution", "Display"),
        ("borderless", "Display"),
        ("monitor", "Display"),
        ("first-person-body", "Display"),
        ("weapon-inertia", "Display"),
        ("camera-motion", "Display"),
        ("weapon-lower", "Display"),
        ("sun-shadows", "Display"),
        ("grass", "Display"),
        ("distant-lod", "Display"),
        ("grade", "Display"),
        ("field-of-view", "Display"),
        ("viewmodel-field-of-view", "Display"),
        ("ui-scale", "Display"),
        ("menu-scale", "Display"),
        ("fps-counter", "Display"),
        ("fps-counter-fps", "Display"),
        ("fps-counter-low", "Display"),
        ("fps-counter-graph", "Display"),
        ("fps-counter-split", "Display"),
        ("fps-counter-cpu", "Display"),
        ("fps-counter-gpu", "Display"),
        ("fps-counter-vram", "Display"),
        ("fps-counter-ram", "Display"),
        ("fps-counter-loaded", "Display"),
        ("fps-counter-upscaling", "Display"),
        ("display-sync", "Display"),
        ("frame-cap", "Display"),
        ("physics-rate", "Display"),
        ("object-distance", "Display"),
        ("actor-distance", "Display"),
        ("item-distance", "Display"),
        ("shadow-distance", "Display"),
        ("grass-distance", "Display"),
        ("upscaler", "Graphics"),
        ("upscaler-quality", "Graphics"),
        ("dlss-model", "Graphics"),
        ("fsr-library", "Graphics"),
        ("frame-generation", "Graphics"),
        ("frame-generation-frames", "Graphics"),
        ("anti-aliasing", "Graphics"),
        ("latency-reduction", "Graphics"),
        ("shadow-filter", "Graphics"),
        ("ambient-occlusion", "Graphics"),
        ("fine-shadows", "Graphics"),
        ("bloom", "Graphics"),
        ("glow", "Graphics"),
        ("eye-adaptation", "Graphics"),
        ("neon-glow", "Graphics"),
        ("sharpen", "Graphics"),
        ("anisotropic", "Graphics"),
        ("terrain-height-blend", "Graphics"),
        ("terrain-anti-tiling", "Graphics"),
        ("smooth-terrain", "Graphics"),
        ("light-shafts", "Graphics"),
        ("water-reflections", "Graphics"),
        ("water-mirror", "Graphics"),
        ("quality", "Graphics"),
        ("volume-master", "Sound"),
        ("volume-music", "Sound"),
        ("volume-footstep", "Sound"),
        ("volume-voice", "Sound"),
        ("volume-effects", "Sound"),
        ("volume-radio", "Sound"),
        ("reverb", "Sound"),
        ("voice-reverb", "Sound"),
        ("overlay", "Debug"),
        ("hit-markers", "Debug"),
        ("god-mode", "Debug"),
        ("navmesh", "Debug"),
        ("terrain-wireframe", "Debug"),
        ("send-reports", "Reports"),
        ("reports-delete", "Reports"),
            ("lockpick-touch-hints", "Touch"),
        ("touch-pipboy-dismiss-left", "Controls"),
        ("touch-pipboy-dismiss-right", "Controls"),
        ("touch-pipboy-dismiss-side-v2", "Controls"),
        ("touch-pipboy-dismiss-travel-v1", "Controls"),
        ("touch-pipboy-close-button", "Controls"),
        ("touch-pipboy-button-graphics", "Controls"),
        ("touch-pipboy-model-wheel", "Controls"),
        ("touch-terminal-v1", "Gameplay"),
        ("touch-companion-v1", "Gameplay"),
        ("touch-layout-editor-v1", "Controls"),
        ("touch-barter-v1", "Gameplay"),
        ("touch-maintenance-v1", "Gameplay"),
        ("touch-practice-v1", "Controller"),
        ("touch-hotbar", "Controls"),
        ("touch-hotbar-slots", "Controls"),
        ("touch-hotbar-size", "Controls"),
        ("touch-hotbar-tray", "Controls"),
        ("touch-tray-editor-v1", "Controls"),
        ("touch-bottom-swipe-target-v1", "Controls"),
        ("touch-pipboy-swipe-v1", "Controller"),
        ("touch-pipboy-distance-v1", "Controller"),
        ("touch-direct-pipboy-v1", "Controls"),
        ("touch-world-activation-v1", "Controls"),
        ("touch-dialogue", "Gameplay"),
        ("dialogue-choice-text-size", "Gameplay"),
        ("dialogue-subtitle-text-size", "Gameplay"),
        ("dialogue-touch-choice", "Gameplay"),
        ("dialogue-speaker-side", "Gameplay"),
        ("dialogue-theme-v1", "Gameplay"),
        ("dialogue-background-opacity-v1", "Gameplay"),
        ("dialogue-background-tint-v1", "Gameplay"),
        ("dialogue-frame-opacity-v1", "Gameplay"),
        ("dialogue-hints-v1", "Gameplay"),
        ("controller-ads-v1", "Controller"),
        ("controller-dead-zone-v1", "Controller"),
        ("controller-toggle-aim-v1", "Controller"),
        ("controller-hold-sneak-v1", "Controller"),
        ("touch-icon-family", "Touch"),
        ("adaptive-touch-actions", "Touch"),
        ("lockpick-touch", "Touch"),
        ("lockpick-touch-size", "Touch"),
        ("touch-gyro-v1", "Controller"),
        ("touch-gyro-sensitivity-v1", "Controller"),
        ("touch-gyro-ads-v1", "Controller"),
        ("touch-gyro-toggle-v1", "Controller"),
        ("touch-gyro-ads-only-v1", "Controller"),
        ("touch-gyro-recenter-v1", "Controller"),
        ("touch-gyro-invert-x-v1", "Controller"),
        ("touch-gyro-invert-y-v1", "Controller"),
        ("touch-analog-movement-v1", "Touch"),
        ("touch-fire-drag-look-v1", "Touch"),
        ("touch-auto-move-lock-v1", "Touch"),
        ("touch-vats-double-tap-v1", "Touch"),
        ("touch-vats-targeting-v1", "Touch"),
        ("touch-pipboy-size-v1", "Gameplay"),
        ("touch-pipboy-tab-swipes", "Gameplay"),
        ("hud-scale-v2", "Display"),
        ("barter-menu-size-v1", "Display"),
        ("repair-menu-size-v1", "Display"),
        ("workbench-menu-size-v1", "Display"),
        ("hud-health-size-v1", "Display"),
        ("hud-action-points-size-v1", "Display"),
        ("hud-compass-size-v1", "Display"),
        ("mobile-performance", "Display"),
        ("mobile-fps", "Display"),
        ("mobile-resolution", "Display"),
        ("mobile-thermal-auto", "Display"),
        ("mobile-thermal-status", "Display"),
    ];

    /// What every tweak's key starts with (#1892's `tweaks.rs`: `concat!("tweak-", $key)`).
    const TWEAK_PREFIX: &str = "tweak-";

    /// Every row #1892 registers under Tweaks (PR head 25bc53a28, `engine/nvf-play/src/tweaks.rs`),
    /// in its order, without [`TWEAK_PREFIX`]: what the table places ahead of it.
    const TWEAK_ROWS: &str = "\
        discovered-sound level-sound-every-a level-sound-a level-sound-every-b level-sound-b xp-multiplier \
        no-xp-messages no-karma-messages no-quest-failed no-location-popup no-objective-popup disable-fast-travel \
        midair-fast-travel fast-travel-enemies fast-travel-encumbered no-red-crosshair hide-enemy-markers \
        no-sneak-label no-reputation-messages no-combat-music no-loading-tips no-explosion-blur no-chem-wore-off \
        kill-cam-exit-key skip-skill-page no-empty-book-message recipe-caps weapon-ammo-use book-effects \
        deselect-hotkeys map-remembers terminal-number-keys faster-hacking dialogue-music dialogue-radio \
        console-skips-player holster-real-time power-armor-no-screen-blood no-red-crosshair-cloaked \
        god-mode-quiet-landing console-compress-repeats console-timestamps living-anatomy-dr sync-categories \
        perks-per-level special-points pick-without-skill hack-without-skill no-terminal-lockout no-casino-bans \
        loot-unconscious revive-companions revive-down-seconds pickpocket-worn no-sneak-criticals reload-when-empty \
        no-travel-time power-armor-fall weightless-armor weightless-aid weightless-ammo weightless-weapons \
        weightless-misc weightless-worn-armor show-poison-effects no-poison-confirm map-summon-companions \
        summon-unconscious no-min-companion-distance map-unconscious-companions companions-location-font \
        map-door-kinds hostiles-prefer-player prefer-player-start prefer-player-mid prefer-player-detection \
        equip-broken-items keep-broken-equipped hardcore-tweaks hardcore-ammo-weight hardcore-essential-companions \
        hardcore-sleep-heals weightless-worn-power-armor no-travel-crippled-leg wait-only-seated kill-reward-ap \
        jump-ap-cost run-ap-cost no-selling custom-skill-points skill-point-base skill-points-per-int \
        no-unconscious-message no-voice-filter no-skip-dialogue lockpick-mouse no-max-casino-bet no-aiming-slowdown \
        repeat-jumping hold-to-sneak vats-entry-ap power-attack-ap quest-marker-reach scope-zoom-time drop-key \
        no-container-after-pick no-door-after-pick mark-unvisited-doors no-exit-hacking no-grab-owned agility-speed \
        auto-unlock-terminals auto-unlock-margin weapons-unhurt-by-hits god-mode-no-jams knockdown-by-strength \
        drowning-drains-ap aiming-draws-weapon walk-angle crippled-leg-jump aid-at-full-health grabbing-is-theft \
        vendor-lists key-name no-skill-tags no-npc-addiction pickpocket-fleeing full-reload-swaps melee-reach \
        repair-xp repair-caps-container no-wear-on-dead encumbered-ap-regen light-step-companions no-player-stagger \
        ready-does-not-draw jam-reload-pace no-hotkeys-reloading wading-pace sleep-on-chairs stealing-no-alarm \
        no-companion-kill-xp no-disarm-companions no-self-explosion-damage sneak-criticals-melee teammates-meltdown \
        speed-scales-jump no-tutorials no-knockdown-god show-current-skill min-splatter xp-sound-at-cap hold-to-wait \
        no-sight-zoom hide-misc-quest tab-click-sorts hotkey-toggles-in-hand no-interior-fog mark-unspoken \
        no-caps-added subtitle-names compass-pips-danger neutral-foes-compass sneak-label-standing no-xp-meter-combat \
        no-limb-indicator skip-death-cam skip-death-cam-attack help-is-console heartbeat-fade heartbeat-fade-seconds \
        terminal-fade terminal-fade-seconds containers-show-ammo descriptive-marker-added no-grenade-indicator \
        grenade-indicator-harmless grenade-indicator-destructibles keep-acting-notes hide-completed-quests \
        hide-marker-reputation no-show-quest-location hide-trait-ranks hide-quest-item-figures single-item-figures \
        mark-other-ammo show-fame-infamy take-all-confirm take-all-confirm-min repair-all-confirm repair-all-threshold \
        take-all-stays hide-equipped-barter hide-equipped-containers title-case-categories no-right-click-drop \
        quiet-undiscovered repair-any-armor sort-misc-stats effect-time-left perk-plugin-names item-added-seconds \
        pipboy-12-hour notes-of-all-quests aim-turn-x aim-turn-y pickpocket-after-caught wait-anywhere radio-music \
        radio-talk camera-wheel-pov quiet-hotkeys detached-beams god-mode-off-on-load no-stealth-shimmer-pipboy \
        quiet-teammate-steps no-weapon-hotkeys pipboy-opens-items hide-crosshair-reloading kill-xp-per-level \
        key-xp-share password-xp-share invert-x nearest-unfound discovered-in-corner power-armor-radiation \
        power-armor-limbs fall-height-endurance invert-title-wheel armor-condition armor-visible-below armor-red-below \
        armor-flash-below armor-in-weapon-place scale-explosion-shake explosion-shake toggle-device hud-fatigue \
        key-repeat-accel key-repeat-delay key-repeat-mult key-repeat-delay-alt key-repeat-mult-alt precise-rad-meter \
        rad-decimals compass-doors compass-doors-exterior compass-doors-interior compass-doors-fade detected-by-whom \
        detected-names quest-text-in-menus continue-shows-save auto-continue hide-ammo-label barter-by-kind \
        barter-water-unscaled barter-sell-armor barter-sell-weapon barter-sell-aid barter-sell-ammo barter-sell-misc \
        barter-sell-weapon-mod barter-buy-armor barter-buy-weapon barter-buy-aid barter-buy-ammo barter-buy-misc \
        barter-buy-weapon-mod free-item-price skip-easy-locks skip-lock-margin pick-up-disarmed npc-weapons-whole \
        lethal-knockdowns duplicate-binds crippling-never-disarms no-spin-up god-mode-free-crafting pov-follows-weapon \
        kill-cam-chance subtitle-distance sneak-criticals-head compare-weapons warn-encumbrance mark-nearby-places \
        scale-ash-piles pipboy-buzz per-bullet-reload reload-rounds-per-press reload-press-queue reload-while-firing \
        semi-auto-queue repair-scales-crafting no-healing-in-combat power-attack-if-blocking breath-regain \
        vats-hip-fire pickpocket-caught-detects agility-jump lockpick-force-near contact-mines-player \
        contact-mines-npcs sleep-heal-min-hours barter-repair-discount armor-condition-penalty vats-ignores-companions \
        no-pipboy-in-combat frenzy-kill-xp free-wild-wasteland stats-xp-percent repair-same-first repair-low-first \
        repair-kits-in-list repair-preview crippled-fall-pain quantity-companion-cap armor-blocks-blood embolden-tags \
        pickpocket-overhaul pp-base pp-agility pp-luck pp-sneak pp-perception pp-detection pp-value pp-weight pp-worn \
        pp-free-items pp-reward-xp hold-activate hold-containers hold-furniture hold-water quick-use \
        vats-throw-visibility quiet-followers crit-message-limb no-dehydration-messages no-sleep-messages need-icons \
        repair-at-workbench character-radiation character-radiation-stages reallocate-skill-points \
        local-map-reset-cells hold-release-throws npcs-disarm-player npcs-drop-live-grenades water-shows-h2o no-respec \
        weapon-impacts robots-heal-with-scrap robot-heal-base robot-heal-per-repair robot-heal-expert \
        no-weapon-move-penalty unequip-broken-armor hide-crosshair-first hide-crosshair-third no-skill-books-at-max \
        red-hp-below red-ap-below weapon-cnd-red-below weapon-cnd-flash-below blackjack-total player-pain-gap \
        pause-on-load swap-ammo-holstered slots-hold-to-spin no-auto-continue-dialogue shift-talks-sneaking \
        no-quest-set-on-objective no-quest-set-on-complete hide-read-notes tab-closes-terminals \
        tab-back-in-pause unsafe-fast-travel vats-auto-head vats-drops-disarmed-shots vats-target-projectiles \
        vats-target-mines vats-target-own-projectiles vats-ignore-non-explosive vats-burst-stops-on-kill \
        repair-needs-weapon-skill date-style hide-unknown-names vats-keeps-holstered no-hotkey-wheel ammo-label-under \
        companion-pip female-facial-hair terminal-password-prompt lockpick-move-keys store-all map-recentre-key \
        map-marker-distance quantity-shows-price quantity-shows-weight hide-empty-reputation quick-load-in-menus \
        preview-perks pick-locks-with-key right-click-previous-category mark-notes-unread full-barter-total \
        compass-fade-left custom-dead-zones left-dead-zone right-dead-zone custom-trigger-dead-zones lt-dead-zone \
        rt-dead-zone responsive-stick faster-pad-orbit vanity-returns-first horizontal-look vats-shift-ignores-friends \
        remember-console-history console-output-file console-text-shadow click-to-exit-loading \
        select-unavailable-radios radiation-radio-static barter-caps-change no-self-meltdown max-level \
        standing-sneak-attacks no-companion-item-damage modding-needs-skill planting-needs-skill travel-costs-bottle \
        jump-swims-up armor-sinks remember-pin-health force-spares-lock no-condition-damage-penalty \
        no-condition-damage-buffer crit-ignores-fire-rate crit-damage-scale npc-dr-ignores-condition \
        npc-dt-ignores-condition explosion-radius-buff npcs-sneak-crit-player fatal-crits-gib fall-deaths-dismember \
        splash-torso-gib npcs-drop-holstered npcs-earn-casings first-person-vats no-ammo-casings crosshair-when-aiming \
        move-during-vats-camera stack-figures seated-wide no-turn-clip no-land-clip hide-unmapped-places \
        vigor-review-first scope-delay no-pointer-message-box no-pointer-dialogue levelup-wheel compare-armor \
        detailed-stats minigames-timescale no-name-limit seat-keeps-on-activate plain-going-out \
        mark-recurring-challenges hide-hotkeyed-containers hide-hotkeyed-barter hide-hotkeyed-repair \
        map-zoom-at-pointer console-homes shift-screenshot-hides print-plugin-names \
        weapon-requirements-matter console-model-details back-pace strafe-pace hack-no-single-chars \
        hack-no-repeat-words hack-dud-when-full hack-spare-guessed run-overencumbered over-run-ap over-jump-ap \
        npc-melee-part-damage disarm-needs-skill disarm-mult disarm-explode-mult cap-skills-by-special skill-cap-base \
        skill-cap-per-special skill-cap-per-luck skill-cap-most cap-skills-spare-tagged hold-breath breath-sway \
        breath-ap scope-wheel-zoom wheel-cycles-hotkeys dpad-cycles-hotkeys keys-with-pad double-tap-ammo \
        improved-weather vats-stop-following-dead night-vision-toggle night-vision-off-first night-vision-by-day \
        kill-cam-outside-vats kill-cam-hit-location weather-trip-distance compass-pips-fade compass-places-fade \
        buffed-health-bars recent-dead-secs armor-sounds-3d mine-blasts-heard blasts-spare-items grabbed-weighs \
        usable-ammo-first better-flycam recent-markers-red compass-quest-height compass-marker-height hud-marker-name \
        hud-region-names move-during-pipboy-raise region-borders-message-only marker-name-angle \
        marker-name-hide-unfound marker-name-units hide-hud-compass hide-hud-ammo hide-hud-needs hide-hud-sneak \
        hide-hud-on-load grey-read-notes remember-item-places player-weapon-volume npc-weapon-volume \
        keep-fall-height-on-load no-marker-popup marker-shift-removes marker-removed-on-it clear-marker-within \
        place-marker-here-says repeat-activate-takes-only hi-res-npc-weapons empty-reload-swaps-ammo \
        objectives-on-arrival objectives-gap sleep-in-owned-beds power-armor-no-training";

    /// [`TWEAK_ROWS`] as keys, under the Tweaks tab's group.
    fn tweak_rows() -> Vec<(&'static str, &'static str)> {
        TWEAK_ROWS.split_whitespace().map(|k| (&*Box::leak(format!("{TWEAK_PREFIX}{k}").into_boxed_str()), TWEAKS_TAB)).collect()
    }

    /// Whether `key` is one of [`TWEAK_ROWS`].
    fn is_tweak(key: &str) -> bool {
        key.strip_prefix(TWEAK_PREFIX).is_some_and(|k| TWEAK_ROWS.split_whitespace().any(|r| r == k))
    }

    /// Stewie's Tweaks' groups as the Tweaks tab lists them [design: owner 2026-11-15].
    const TWEAK_GROUPS: [&str; 8] = ["Gameplay", "Combat", "Quality of Life", "HUD", "Menus", "Hotkeys", "Cheats", "Misc"];

    #[test]
    fn the_table_names_each_key_once_in_one_of_the_tabs() {
        assert_eq!(check(SECTIONS), Ok(()));
        let twice = [section("Gameplay", Some("A"), &["a"]), section("Debug", None, &["a"])];
        assert!(check(&twice).unwrap_err().contains("placed twice"));
        let odd = [section("Sound", None, &["a"])];
        assert!(check(&odd).is_err());
        let unasked = [section(NVF_FEATURES_TAB, Some("New"), &["a-big-change"])];
        assert!(check(&unasked).unwrap_err().contains("owner named"), "NVF Features only by the owner's word");
        let control = [section("Gameplay", Some("A"), &["controls:device"])];
        assert!(check(&control).is_err());
        let headless = [section("Gameplay", Some("A"), &["a"]), section("Gameplay", None, &["b"])];
        assert!(check(&headless).is_err());
        // Every row there is has its place in the table; the table names no other but the
        // Controls tab's own rows and #1892's tweaks, each of those on the Tweaks tab.
        let named: Vec<&str> = SECTIONS.iter().flat_map(|s| s.keys.iter().copied()).filter(|k| !k.starts_with(CONTROL_PREFIX)).collect();
        for (k, _) in KNOWN {
            assert!(named.contains(&k), "{k} has no place");
        }
        for k in &named {
            assert!(KNOWN.iter().any(|(n, _)| n == k) || is_tweak(k), "{k} is placed but no row has it");
        }
        for s in SECTIONS {
            for k in s.keys.iter().filter(|k| k.starts_with(TWEAK_PREFIX)) {
                assert_eq!(s.placement.tab, TWEAKS_TAB, "{k} is a tweak");
            }
        }
    }

    #[test]
    fn the_tabs_come_first_with_every_row_once_in_the_tables_order() {
        let list = registry(&KNOWN);
        let l = resolve_layout(&list);
        assert_eq!(l.tab_names(), TAB_ORDER, "no group is a tab of its own: every row is named");
        assert_eq!(l.unnamed(), 0);
        let mut all: Vec<usize> = l.ordered_settings().collect();
        assert_eq!(all.len(), list.len());
        all.sort_unstable();
        assert_eq!(all, (0..list.len()).collect::<Vec<_>>(), "each registered row once");
        assert_eq!(keys(&list, &l, "Gameplay")[..11], ["[Game]", "difficulty", "hardcore", "reload-slows", "UsePipBoy", "sound-cover", "", "[V.A.T.S.]", "vats-style", "vats-slowdown", "kill-camera"]);
        assert_eq!(keys(&list, &l, "Audio"), ["[Volume]", "volume-master", "volume-music", "volume-footstep", "volume-voice", "volume-effects", "volume-radio", "", "[Sound]", "music", "voice", "reverb", "voice-reverb"]);
        assert_eq!(keys(&list, &l, "Controls")[..6], ["[Input]", "@device", "button-style", "@automatic-input", "@rumble", ""]);
        assert_eq!(
            keys(&list, &l, NVF_FEATURES_TAB),
            ["[Advanced Movement]", "advanced-movement", "movement-weight", "", "[Gameplay]", "movement-inertia", "weapon-handling", "advanced-recoil", "", "[Visual Only]", "weapon-inertia", "animation-smoothing", "camera-motion", "", "[World Interaction]", "grass-fire"]
        );
        assert_eq!(keys(&list, &l, "Debug"), ["overlay", "hit-markers", "god-mode", "navmesh", "terrain-wireframe", "scope-3d"]);
        assert!(keys(&list, &l, "Tweaks").is_empty(), "an empty tab is still a tab");
        let at = |k: &str| l.placement_of(list.iter().position(|s| s.key == k).unwrap()).unwrap().path();
        assert_eq!(at("menu-pause"), "Gameplay > Dialogue");
        assert_eq!(at("hud-red"), "Display > Colours");
        assert_eq!(at("upscaler"), "Display > Upscaling and Anti-Aliasing");
        assert_eq!(at("sky-clock"), "Graphics > World and Animation");
        assert_eq!(at("grass-fire"), "NVF Features > World Interaction");
        assert_eq!(at("button-style"), "Controls > Input");
        assert_eq!(at("terrain-anti-tiling"), "Graphics > Detail and Distance", "beside terrain height blending (#2551)");
        assert_eq!(at("sound-cover"), "Gameplay > Game", "#2688");
        assert_eq!(l.control_placement("bind:Jump").unwrap().path(), "Controls > Movement");
        // The registry itself is left as it was: its order and each row's group.
        let again = registry(&KNOWN);
        assert!(list.iter().zip(&again).all(|(a, b)| a.key == b.key && a.group == b.group && a.label == b.label && a.note == b.note));
    }

    #[test]
    fn a_closed_category_keeps_its_heading_and_an_empty_line_follows_only_an_open_one() {
        let list = registry(&KNOWN);
        let l = resolve_layout(&list);
        let closed = |p: &Placement| p.heading != Some("V.A.T.S.") && p.heading != Some("Camera");
        let g = lines(&list, &l, "Gameplay", closed);
        assert_eq!(g[..10], ["[Game]", "difficulty", "hardcore", "reload-slows", "UsePipBoy", "sound-cover", "", "[V.A.T.S.]", "[Camera]", "[Dialogue]"]);
        let p = l.placement_of(list.iter().position(|s| s.key == "vats-style").unwrap()).unwrap();
        assert_eq!(l.shown_count(&p, |_| true), 3);
        assert_eq!(l.shown_count(&p, |i| i != LayoutItem::Setting(list.iter().position(|s| s.key == "vats-slowdown").unwrap())), 2);
    }

    #[test]
    fn a_missing_row_and_an_empty_category_are_left_out() {
        // No lip sync, no player body; no head tracking, foot IK or sky clock at all.
        let list = registry(&[("difficulty", "Gameplay"), ("menu-pause", "Gameplay"), ("field-of-view", "Display"), ("god-mode", "Debug")]);
        let l = resolve_layout(&list);
        assert_eq!(keys(&list, &l, "Gameplay"), ["[Game]", "difficulty", "", "[Camera]", "field-of-view", "", "[Dialogue]", "menu-pause"]);
        assert!(keys(&list, &l, "Graphics").is_empty());
        // A hidden row hides its category when it is the only one, and keeps its place.
        let hidden = list.iter().position(|s| s.key == "menu-pause").unwrap();
        let shown = l.tab_entries("Gameplay", None, |i| i != LayoutItem::Setting(hidden), |_| true);
        assert!(!shown.iter().any(|e| matches!(e, LayoutEntry::Heading(p) if p.heading == Some("Dialogue"))));
        assert_ne!(shown.last(), Some(&LayoutEntry::Gap), "no empty line at the end");
        assert_eq!(l.placement_of(hidden).unwrap().path(), "Gameplay > Dialogue");
    }

    #[test]
    fn an_unnamed_row_stays_under_its_own_group() {
        let list = registry(&[("difficulty", "Gameplay"), ("new-switch", "Gameplay"), ("odd-sound", "Sound"), ("x1", "Mods"), ("volume-master", "Sound"), ("x2", "Extras"), ("x3", "Mods")]);
        let l = resolve_layout(&list);
        // After Debug, in the order the groups first appear in the registry; Sound is not Audio.
        assert_eq!(l.tab_names()[8..], ["Sound", "Mods", "Extras"]);
        assert_eq!(keys(&list, &l, "Gameplay"), ["[Game]", "difficulty", "", "new-switch"]);
        assert_eq!(keys(&list, &l, "Sound"), ["odd-sound"]);
        assert_eq!(keys(&list, &l, "Audio"), ["[Volume]", "volume-master"]);
        assert_eq!(keys(&list, &l, "Mods"), ["x1", "x3"]);
        let new = list.iter().position(|s| s.key == "new-switch").unwrap();
        assert_eq!(l.placement_of(new), Some(Placement::tab("Gameplay")));
        assert_eq!(l.unnamed(), 5);
    }

    #[test]
    fn hundreds_of_unnamed_tweaks_keep_the_registrys_order() {
        // A tweak the table does not name yet: under Tweaks, in the order registered.
        let names: Vec<&'static str> = (0..650)
            .map(|i| {
                let n: &'static str = Box::leak(format!("tweak-x{i:03}").into_boxed_str());
                n
            })
            .collect();
        let mut rows: Vec<(&'static str, &'static str)> = vec![("difficulty", "Gameplay")];
        rows.extend(names.iter().map(|n| (*n, "Tweaks")));
        rows.push(("god-mode", "Debug"));
        let list = registry(&rows);
        let l = resolve_layout(&list);
        assert_eq!(l.tab_names(), TAB_ORDER);
        let shown = keys(&list, &l, "Tweaks");
        assert_eq!(shown.len(), 650, "no heading: all in one run");
        assert!(shown.iter().zip(&names).all(|(a, b)| a == b), "in the order registered");
        assert_eq!(l.ordered_settings().count(), list.len());
        assert_eq!(l.unnamed(), 0, "the Tweaks tab's rows are listed by design, not counted as left out of the table");
    }

    #[test]
    fn every_tweak_of_1892_has_its_group_and_category() {
        let tweaks = tweak_rows();
        // 560 less the console and screenshot keys, bindings since 2026-11-16 (Controls).
        assert_eq!(tweaks.len(), 558);
        let mut rows: Vec<(&'static str, &'static str)> = KNOWN.to_vec();
        rows.extend(&tweaks);
        let list = registry(&rows);
        let l = resolve_layout(&list);
        assert_eq!(l.unnamed(), 0, "every tweak named");
        assert_eq!(l.tab_names(), TAB_ORDER);
        let t = l.tab(TWEAKS_TAB).unwrap();
        for s in &t.sections {
            assert!(s.placement.subgroup.is_some() && s.placement.heading.is_some(), "{}: a group and a heading", s.placement.path());
            assert!(s.placement.closed_by_default());
        }
        assert_eq!(t.sections.iter().map(|s| s.rows.len()).sum::<usize>(), tweaks.len(), "the tweaks, and only they, on the tab");
        let at = |k: &str| l.placement_of(list.iter().position(|s| s.key == k).unwrap()).unwrap().path();
        assert_eq!(at("tweak-no-xp-messages"), "Tweaks > HUD > Messages");
        assert_eq!(at("tweak-weightless-ammo"), "Tweaks > Cheats > Weight");
        assert_eq!(at("tweak-weightless-worn-armor"), "Tweaks > Gameplay > Weight");
        // A row that only matters with another comes right after it.
        let next = |k: &str| list[l.ordered_settings().skip_while(|&i| list[i].key != k).nth(1).unwrap()].key;
        assert_eq!(next("tweak-take-all-confirm"), "tweak-take-all-confirm-min");
        assert_eq!(next("tweak-revive-companions"), "tweak-revive-down-seconds");
        // The engine's own tabs are as they were.
        assert_eq!(keys(&list, &l, "Audio"), keys(&registry(&KNOWN), &resolve_layout(&registry(&KNOWN)), "Audio"));
    }

    #[test]
    fn the_tweaks_tab_lists_stewies_eight_groups_in_order_one_at_a_time() {
        let list = registry(&tweak_rows());
        let l = resolve_layout(&list);
        let t = l.tab(TWEAKS_TAB).unwrap();
        assert_eq!(t.subgroups(), TWEAK_GROUPS);
        // Each group's categories together, in the table's order.
        let mut runs: Vec<&str> = t.sections.iter().filter_map(|s| s.placement.subgroup).collect();
        runs.dedup();
        assert_eq!(runs, TWEAK_GROUPS, "no group split by another");
        // One group at a time, its categories closed: only their headings.
        let hotkeys = l.tab_entries(TWEAKS_TAB, Some("Hotkeys"), |_| true, |_| false);
        let headings: Vec<&str> = hotkeys.iter().filter_map(|e| if let LayoutEntry::Heading(p) = e { p.heading } else { None }).collect();
        assert_eq!(headings, ["Hide HUD Key", "Other Keys"]);
        assert_eq!(hotkeys.len(), 2, "closed: no rows, no empty line");
        // No other tab has a group.
        assert!(l.tabs.iter().filter(|t| t.name != TWEAKS_TAB).all(|t| t.subgroups().is_empty()));
        // A tweak the table does not place yet: under the last group, Other, never out of sight.
        let mut rows = tweak_rows();
        rows.push(("tweak-new-one", "Tweaks"));
        let list = registry(&rows);
        let l = resolve_layout(&list);
        let t = l.tab(TWEAKS_TAB).unwrap();
        assert_eq!(t.subgroups().last(), Some(&UNGROUPED));
        let other = l.tab_entries(TWEAKS_TAB, Some(UNGROUPED), |_| true, |_| true);
        assert_eq!(other, [LayoutEntry::Setting(list.iter().position(|s| s.key == "tweak-new-one").unwrap())]);
        assert!(l.tab_entries(TWEAKS_TAB, Some("Hotkeys"), |_| true, |_| true).iter().all(|e| *e != other[0]));
    }
    fn mobile_visibility(list: &[Setting]) -> crate::settings_view::Visibility {
        let hidden = ["touch-pipboy-dismiss-left", "touch-pipboy-dismiss-right"].into_iter()
            .filter_map(|key| Some((list.iter().position(|r| r.key == key)?,
                list.iter().position(|r| r.key == "touch-pipboy-dismiss-side-v2")?))).collect();
        crate::settings_view::Visibility { hidden, ..Default::default() }
    }

    #[test]
    fn android_groups_the_same_registered_rows_once_without_changing_the_registry() {
        let list = registry(&KNOWN);
        let before: Vec<_> = list.iter().map(|s| (s.key, s.group, s.label, s.note)).collect();
        let mobile = resolve_layout_with(&list, &MOBILE_TAB_ORDER, MOBILE_SECTIONS);
        assert_eq!(mobile.tab_names(), MOBILE_TAB_ORDER);
        assert_eq!(mobile.unnamed(), 0);
        let mut indices: Vec<_> = mobile.ordered_settings().collect();
        assert_eq!(indices.len(), KNOWN.len());
        indices.sort_unstable();
        assert_eq!(indices, (0..list.len()).collect::<Vec<_>>());
        let mut mobile_keys = std::collections::HashSet::new();
        for section in MOBILE_SECTIONS {
            for &key in section.keys {
                assert!(mobile_keys.insert(key), "duplicate Android placement: {key}");
                assert!(SECTIONS.iter().any(|s| s.keys.contains(&key)), "new or misspelled key: {key}");
                let index = list.iter().position(|s| s.key == key).unwrap();
                assert_eq!(mobile.placement_of(index), Some(section.placement));
            }
        }
        assert_eq!(mobile_keys.len(), 71);
        let added = ["hud-scale-v2", "barter-menu-size-v1", "repair-menu-size-v1", "workbench-menu-size-v1", "dialogue-background-tint-v1", "touch-pipboy-dismiss-side-v2", "touch-pipboy-dismiss-travel-v1"];
        assert!(added.iter().all(|key| mobile_keys.contains(key)));
        assert_eq!(mobile_keys.iter().filter(|key| !added.contains(key)).count(), 64, "four original Main controller rows retain their new Main categories");
        assert!(!mobile_keys.contains("physics-rate"), "upstream screen setting is shared, not mobile-only");
        let grass_fire = list.iter().position(|row| row.key == "grass-fire").unwrap();
        assert_eq!(mobile.placement_of(grass_fire).unwrap().path(), "NVF Features > World Interaction");
        assert!(!mobile_keys.contains("grass-fire"), "upstream world setting is shared, not mobile-only");
        for (index, row) in list.iter().enumerate() {
            if !mobile_keys.contains(row.key) {
                assert_eq!(mobile.placement_of(index), resolve_desktop_layout(&list).placement_of(index));
            }
        }
        assert_eq!(before, list.iter().map(|s| (s.key, s.group, s.label, s.note)).collect::<Vec<_>>());
        assert_eq!(resolve_desktop_layout(&list).tab_names(), DESKTOP_TAB_ORDER);
    }

    #[test]
    fn android_mobile_sections_use_existing_visibility_search_and_hardware_routes() {
        use crate::bindings::{Control, InputDevice};
        use crate::settings_view::{self as view, ControlRow, Found, RowId, SettingsScope, ViewEntry, Visibility};
        let list = registry(&KNOWN);
        let mobile = resolve_layout_with(&list, &MOBILE_TAB_ORDER, MOBILE_SECTIONS);
        let at = |key| list.iter().position(|s| s.key == key).unwrap();
        let empty = Found::default();
        let touch = view::project(&mobile, &list, SettingsScope::Tab(MOBILE_TAB), &mobile_visibility(&list), &empty, &view::Folding::default());
        let keyboard_controls = view::project(&mobile, &list, SettingsScope::Tab(CONTROLS_TAB), &mobile_visibility(&list), &empty, &view::Folding::default());
        let pad = view::project(&mobile, &list, SettingsScope::Tab(CONTROLS_TAB), &Visibility { device: InputDevice::Controller, ..mobile_visibility(&list) }, &empty, &view::Folding::default());
        assert!(!keyboard_controls.iter().any(|e| matches!(e, ViewEntry::Heading(p) if p.heading == Some("Controller"))));
        assert!(pad.iter().any(|e| matches!(e, ViewEntry::Heading(p) if p.heading == Some("Controller"))));
        for &key in &CONTROLLER_KEYS {
            let entry = ViewEntry::Row(RowId::Setting(at(key)));
            assert!(!keyboard_controls.contains(&entry));
            assert!(pad.contains(&entry));
        }
        let controls = view::project(&mobile, &list, SettingsScope::Tab(CONTROLS_TAB), &Visibility { device: InputDevice::Controller, ..mobile_visibility(&list) }, &empty, &view::Folding::default());
        assert!(controls.contains(&ViewEntry::Row(RowId::Control(ControlRow::Binding(Control::Jump, InputDevice::Controller)))));
        let search = Found { search: view::search(&mobile, &list, "controller-ads-v1", |r| r.name()), changed: Vec::new() };
        let projected = view::project(&mobile, &list, SettingsScope::Search, &mobile_visibility(&list), &search, &view::Folding::default());
        assert!(projected.contains(&ViewEntry::Row(RowId::Control(ControlRow::Device))), "hidden pad setting keeps native device-switch route");
        assert!(!projected.contains(&ViewEntry::Row(RowId::Setting(at("controller-ads-v1")))));
        let (scope, selected, _) = view::normalize(&mobile, &list, SettingsScope::Tab(CONTROLS_TAB), Some(RowId::Setting(at("controller-ads-v1"))), &mobile_visibility(&list), &empty, &view::Folding::default());
        assert_eq!((scope, selected), (SettingsScope::Tab(CONTROLS_TAB), Some(RowId::Control(ControlRow::Device))));
        for (key, heading) in [
            ("touch-vats-targeting-v1", "Movement and gestures"),
            ("touch-practice-v1", "Touch layout and guide"), ("touch-gyro-v1", "Gyroscope"),
            ("touch-hotbar", "Inventory and quick access"), ("touch-dialogue", "Dialogue"),
            ("menu-scale", "Interface size"), ("mobile-performance", "Performance and battery"),
        ] {
            assert!(touch.contains(&ViewEntry::Row(RowId::Setting(at(key)))));
            assert_eq!(mobile.placement_of(at(key)).unwrap().heading, Some(heading));
        }
        let chooser = RowId::Setting(at("touch-pipboy-dismiss-side-v2"));
        assert!(touch.contains(&ViewEntry::Row(chooser)));
        assert!(touch.contains(&ViewEntry::Row(RowId::Setting(at("touch-pipboy-dismiss-travel-v1")))));
        for key in ["touch-pipboy-dismiss-left", "touch-pipboy-dismiss-right"] {
            let legacy = RowId::Setting(at(key));
            assert!(!touch.contains(&ViewEntry::Row(legacy)));
            assert_eq!(view::parent_of(&list, at(key), &mobile_visibility(&list)), Some(chooser));
            let found = Found { search: view::search(&mobile, &list, key, |r| r.name()), changed: Vec::new() };
            let result = view::project(&mobile, &list, SettingsScope::Search, &mobile_visibility(&list), &found, &view::Folding::default());
            assert!(result.contains(&ViewEntry::Row(chooser)));
            assert!(!result.contains(&ViewEntry::Row(legacy)));
        }
        assert!(view::rail(&mobile).contains(&view::RailItem::Tab(MOBILE_TAB)));
        assert_eq!(view::step_selectable(&touch, Some(RowId::Setting(at("touch-practice-v1"))), 1),
            Some(RowId::Setting(at("touch-icon-family"))));
    }

    #[test]
    fn mobile_row_identity_still_edits_the_registered_state_and_changed_list() {
        use bevy::prelude::*;
        use crate::settings::{self, SettingsFile};
        use crate::settings_view::{self as view, Found, RowId, SettingsScope};
        #[derive(Resource, Default)]
        struct Consent(bool);
        let mut app = App::new();
        app.init_resource::<Consent>().insert_resource(SettingsFile(None));
        for (key, group) in KNOWN {
            let mut setting = row(key, group);
            if key == "mobile-thermal-auto" {
                setting.get = |world| world.resource::<Consent>().0;
                setting.set = |world, on| world.resource_mut::<Consent>().0 = on;
            }
            settings::add_setting(&mut app, setting);
        }
        let list = app.world().resource::<Settings>().list().to_vec();
        let baseline = app.world().resource::<Settings>().values(app.world());
        let mobile = resolve_layout_with(&list, &MOBILE_TAB_ORDER, MOBILE_SECTIONS);
        assert_eq!(app.world().resource::<Settings>().values(app.world()), baseline);
        let original_index = app.world().resource::<Settings>().index("mobile-thermal-auto").unwrap();
        let id = RowId::Setting(original_index);
        let (scope, chosen, _) = view::normalize(&mobile, &list, SettingsScope::Tab("Display"), Some(id), &mobile_visibility(&list), &Found::default(), &view::Folding::default());
        assert_eq!((scope, chosen), (SettingsScope::Tab(MOBILE_TAB), Some(id)));
        assert_eq!(settings::flip(app.world_mut(), original_index), Some(true));
        assert!(app.world().resource::<Consent>().0);
        let changed = settings::changed_rows(app.world(), &baseline);
        assert_eq!(changed, [original_index]);
        assert_eq!(view::changed(&mobile, &changed, |_| false), [id]);
        let found = view::search(&mobile, &list, "mobile-thermal-auto", |r| r.name());
        assert_eq!(found, [id]);
        assert_eq!(view::listing(&mobile, &found)[0], view::ViewEntry::Crumb(Placement {
            tab: MOBILE_TAB, subgroup: None, heading: Some("Performance and battery"),
        }));
        assert_eq!(app.world().resource::<Settings>().list().iter().map(|s| (s.key, s.group)).collect::<Vec<_>>(),
            list.iter().map(|s| (s.key, s.group)).collect::<Vec<_>>());
    }

    #[test]
    fn platform_profile_and_missing_optional_mobile_rows_keep_the_shared_page() {
        let list = registry(&[("difficulty", "Gameplay"), ("touch-gyro-v1", "Controller"), ("odd", "Mods")]);
        let expected = resolve_layout_with(&list, &super::TAB_ORDER,
            if cfg!(target_os = "android") { MOBILE_SECTIONS } else { &[] });
        assert_eq!(super::resolve_layout(&list), expected);
        let mobile = resolve_layout_with(&list, &MOBILE_TAB_ORDER, MOBILE_SECTIONS);
        assert_eq!(keys(&list, &mobile, MOBILE_TAB), ["[Gyroscope]", "touch-gyro-v1"]);
        assert_eq!(mobile.tab_names().last(), Some(&"Mods"));
        assert_eq!(mobile.unnamed(), 1);
        assert!(mobile.tab_entries(MOBILE_TAB, None, |_| false, |_| true).is_empty(), "no orphan headings");
    }

}
