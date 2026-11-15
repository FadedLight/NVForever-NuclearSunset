use super::*;
use bevy::{camera::RenderTarget, ecs::system::RunSystemOnce, window::{PrimaryWindow, WindowRef, WindowResolution}};
use crate::{menu::{ActiveMenu, PromptKeys, model::{Category, Command, MenuModel, Side}}, pipboy::{Page, Phase, PipBoy}, sim::{SimState, test_world}, ui_sounds::UiSound};
use fnv_sim::{FormId, forms::{BaseData, ItemKind}, inventory::Extra};

pub(crate) const GEAR: FormId = FormId(0xFA100);
const MOD: FormId = FormId(0xFA101);

pub(crate) fn fixture(modding: bool, native_configuration: bool) -> App {
    let mut gear = test_world::item_base(GEAR.0);
    gear.name = Some("Authored fixture gear".into());
    if let BaseData::Item(item) = &mut gear.data {
        item.kind = if modding { ItemKind::Weapon } else { ItemKind::Armor };
        item.health = Some(100); item.value = Some(100); item.armor_dt = Some(5.0);
    }
    let mut modification = test_world::item_base(MOD.0);
    modification.name = Some("Authored fixture capacity mod".into());
    if let BaseData::Item(item) = &mut modification.data { item.kind = ItemKind::ItemMod; }
    let mut caps = test_world::item_base(fnv_game::repair::CAPS.0);
    if let BaseData::Item(item) = &mut caps.data { item.kind = ItemKind::Misc; }
    let mut vendor = test_world::item_base(0xFA103);
    vendor.sig = *b"NPC_";
    vendor.data = BaseData::Actor(fnv_sim::forms::ActorBase { entries: Vec::new(), attributes: None });
    let mut placed = test_world::placed(0xFA104, 0xFA103); placed.sig = *b"ACHR";
    let mut sim = test_world::sim(vec![gear, modification, caps, vendor], vec![placed]);
    sim.world.add_item(FormId::PLAYER_REF, GEAR, if modding { 1 } else { 2 }).unwrap();
    let inv = sim.world.inventory_mut(FormId::PLAYER_REF).unwrap();
    assert!(inv.set_condition_one(GEAR, Extra::default(), 0.4));
    if !modding { assert!(inv.set_condition_one(GEAR, Extra::default(), 0.2)); }
    sim.world.add_item(FormId::PLAYER_REF, MOD, 2).unwrap();
    let mut model = MenuModel::open_inventory(&mut sim.world).unwrap();
    model.apply(&mut sim.world, Command::SetCategory(Side::Player, if modding { Category::Weapons } else { Category::Apparel }));
    model.selected[Side::Player as usize] = model.list(Side::Player).rows.iter().position(|r| r.item == GEAR && r.condition == Some(0.4)).unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::input::InputPlugin, MaintenanceTouchPlugin))
        .init_resource::<player_repair::PipRepair>().init_resource::<item_mod::PipItemMod>()
        .init_resource::<PromptKeys>().init_resource::<SafeInsets>().init_resource::<UiScale>()
        .insert_resource(sim).insert_resource(ActiveMenu(Some(model)))
        .insert_resource(PipBoy { phase: Phase::Up, page: Page::Items, ..default() })
        .insert_resource(crate::script::ScriptState::over_no_plugins())
        .insert_resource(crate::tiles::vanilla::TileMenus { repair_menu: native_configuration, item_mod: native_configuration, ..default() })
        .add_message::<player_repair::RepairInput>().add_message::<item_mod::ModInput>().add_message::<UiSound>()
        .insert_resource(crate::PlayWorld(crate::PlayData {
            lo: Box::leak(Box::new(fnv_esm::LoadOrder::new(&[]).unwrap())), schema: Box::leak(Box::default()), index: Box::leak(Box::default()),
            world: 1, interior: None, eye: [0.0; 3], heading_deg: 0.0, pitch_deg: 0.0, auto_walk: 0.0, start_at_cell_spot: false, packages: false,
        }))
        .add_systems(Startup, crate::combat::init_combat)
        .add_systems(Update, (player_repair::keys, item_mod::keys).chain());
    app.world_mut().resource_mut::<State>().enabled = true;
    let window = app.world_mut().spawn((Window { resolution: WindowResolution::new(640, 360), focused: true, ..default() }, PrimaryWindow)).id();
    let camera = app.world_mut().spawn((Camera { computed: bevy::camera::ComputedCameraValues {
        target_info: Some(bevy::camera::RenderTargetInfo { physical_size: UVec2::new(640, 360), scale_factor: 1.0 }), ..default()
    }, ..default() }, RenderTarget::Window(WindowRef::Entity(window)))).id();
    app.update();
    app.world_mut().resource_mut::<crate::combat::Combat>().forms.weapons.insert(GEAR, fnv_combat::data::WeaponStats {
        base_damage: 20, mods: vec![fnv_combat::data::WeaponMod { slot: 1, item: Some(MOD), effect: Some("Increase Clip Capacity".into()), value_a: 4.0, value_b: 0.0 }], ..default()
    });
    if modding { app.world_mut().write_message(item_mod::ModInput::Open); }
    else { app.world_mut().write_message(player_repair::RepairInput::Open); }
    app.update();
    let context = Context::from_window(window, camera, app.world().get::<Window>(window).unwrap(), SafeInsets::default(), 1.0);
    app.world_mut().run_system_once(move |mut b: Bridge| b.prepare(Some(context), true)).unwrap();
    app.update();
    assert!(app.world().resource::<State>().visible());
    assert_eq!(app.world().resource::<State>().painted, Some(app.world().resource::<State>().generation));
    app
}
pub(crate) fn count(app: &App, item: FormId) -> i32 { app.world().resource::<SimState>().world.peek(FormId::PLAYER_REF).unwrap().inventory.as_ref().unwrap().count(item) }
fn event(app: &mut App, id: u64, phase: TouchPhase, at: Vec2) {
    let window = app.world().resource::<State>().context.unwrap().window;
    let input = TouchInput { window, id, phase, position: at, force: None };
    assert!(app.world_mut().run_system_once(move |mut b: Bridge| b.event(&input, true)).unwrap());
}
fn center(app: &App, action: Action) -> Vec2 {
    let state = app.world().resource::<State>();
    sheet::Geometry::new(state.context.unwrap()).center(state.source.as_deref().unwrap(), action)
}
pub(crate) fn raw_tap(app: &mut App, id: u64, action: Action) {
    let at = center(app, action);
    event(app, id, TouchPhase::Started, at);
    event(app, id, TouchPhase::Ended, at);
}
pub(crate) fn tap(app: &mut App, id: u64, action: Action) { raw_tap(app, id, action); app.update(); }

#[test]
fn actual_donor_consumer_selects_then_confirms_once_and_cancel_preserves_units() {
    for native_configuration in [false, true] {
        let mut app = fixture(false, native_configuration);
        tap(&mut app, 1, Action::Select(1));
        assert_eq!(count(&app, GEAR), 2, "selection must never consume a donor");
        assert_eq!(app.world().resource::<player_repair::PipRepair>().0.as_ref().unwrap().selected, 1);
        raw_tap(&mut app, 2, Action::Confirm);
        raw_tap(&mut app, 3, Action::Confirm);
        app.update();
        assert_eq!(count(&app, GEAR), 1, "duplicate painted confirmations must not consume twice");
        let inv = app.world().resource::<SimState>().world.peek(FormId::PLAYER_REF).unwrap().inventory.as_ref().unwrap();
        assert!(inv.groups().any(|(id, unit, n)| id == GEAR && n == 1 && unit.condition_or_whole() > 0.4));
        let mut canceled = fixture(false, native_configuration);
        tap(&mut canceled, 4, Action::Select(1)); tap(&mut canceled, 5, Action::Cancel);
        assert!(!canceled.world().resource::<player_repair::PipRepair>().is_open());
        assert_eq!(count(&canceled, GEAR), 2);
    }
}

#[test]
fn actual_mod_consumer_installs_and_removes_one_and_rejects_stale_inventory() {
    for native_configuration in [false, true] {
        let mut app = fixture(true, native_configuration);
        tap(&mut app, 10, Action::Select(1)); assert_eq!(count(&app, MOD), 2);
        raw_tap(&mut app, 11, Action::Confirm); raw_tap(&mut app, 12, Action::Confirm); app.update();
        assert_eq!(count(&app, MOD), 1);
        assert_eq!(app.world().resource::<item_mod::PipItemMod>().0.as_ref().unwrap().offer.extra.mods, 1);
        tap(&mut app, 13, Action::Confirm);
        assert_eq!(count(&app, MOD), 2);
        assert_eq!(app.world().resource::<item_mod::PipItemMod>().0.as_ref().unwrap().offer.extra.mods, 0);
        tap(&mut app, 14, Action::Cancel); assert_eq!(count(&app, MOD), 2);
        let mut stale = fixture(true, native_configuration); tap(&mut stale, 15, Action::Select(1));
        raw_tap(&mut stale, 16, Action::Confirm);
        assert_eq!(stale.world_mut().resource_mut::<SimState>().world.inventory_mut(FormId::PLAYER_REF).unwrap().remove(MOD, 2), 2);
        stale.update();
        assert_eq!(count(&stale, MOD), 0);
        assert_eq!(stale.world().resource::<item_mod::PipItemMod>().0.as_ref().unwrap().offer.extra.mods, 0);
    }
}

#[test]
fn unrelated_finger_cannot_take_owner_and_hardware_return_is_action_free() {
    let mut app = fixture(false, false);
    let row = center(&app, Action::Select(1));
    event(&mut app, 20, TouchPhase::Started, row);
    event(&mut app, 21, TouchPhase::Moved, row); event(&mut app, 21, TouchPhase::Ended, row);
    assert!(app.world_mut().run_system_once(|b: Bridge| b.owns(20)).unwrap());
    event(&mut app, 20, TouchPhase::Ended, row); app.update();
    assert_eq!(app.world().resource::<player_repair::PipRepair>().0.as_ref().unwrap().selected, 1);
    let confirm = center(&app, Action::Confirm);
    event(&mut app, 22, TouchPhase::Started, confirm);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyW);
    app.world_mut().run_system_once(|mut b: Bridge| b.hardware_intent()).unwrap();
    assert!(app.world_mut().run_system_once(|b: Bridge| b.owns(22) && b.available() && !b.captures_touch()).unwrap());
    event(&mut app, 22, TouchPhase::Ended, confirm); app.update();
    assert!(app.world().resource::<ButtonInput<KeyCode>>().pressed(KeyCode::KeyW), "hardware state is never cleared by sheet cancellation");
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::KeyW);
    event(&mut app, 23, TouchPhase::Started, confirm); app.update();
    event(&mut app, 23, TouchPhase::Ended, confirm); app.update();
    assert_eq!(count(&app, GEAR), 2, "first returning contact cannot confirm");
    tap(&mut app, 24, Action::Select(1)); tap(&mut app, 25, Action::Confirm);
    assert_eq!(count(&app, GEAR), 1);
}

#[test]
fn actual_consumer_refuses_late_focus_viewport_resize_and_modal_changes() {
    for case in 0..7 {
        let mut app = fixture(false, false); tap(&mut app, 30, Action::Select(1)); raw_tap(&mut app, 31, Action::Confirm);
        let context = app.world().resource::<State>().context.unwrap();
        match case {
            0 => app.world_mut().get_mut::<Window>(context.window).unwrap().focused = false,
            1 => app.world_mut().get_mut::<Camera>(context.camera).unwrap().viewport = Some(bevy::camera::Viewport { physical_size: UVec2::new(320, 360), ..default() }),
            2 => app.world_mut().get_mut::<Window>(context.window).unwrap().resolution.set(800.0, 360.0),
            3 => { app.insert_resource(crate::pause::Paused(true)); }
            4 => { app.world_mut().entity_mut(context.camera).insert(RenderTarget::Image(Handle::<Image>::default().into())); }
            5 => app.world_mut().get_mut::<Camera>(context.camera).unwrap().computed.target_info = None,
            _ => app.world_mut().get_mut::<Camera>(context.camera).unwrap().computed.target_info.as_mut().unwrap().physical_size = UVec2::new(320, 360),
        }
        app.update(); assert_eq!(count(&app, GEAR), 2, "late gate case {case}");
    }
}

#[test]
fn actual_list_scroll_keeps_selection_and_inventory_and_changed_donor_is_refreshed() {
    let mut app = fixture(false, false);
    let at = center(&app, Action::Select(0));
    event(&mut app, 40, TouchPhase::Started, at);
    event(&mut app, 40, TouchPhase::Moved, at - Vec2::Y * 60.0); app.update();
    let session = app.world().resource::<player_repair::PipRepair>().0.as_ref().unwrap();
    assert_eq!((session.first, session.selected), (1, 0));
    assert_eq!(count(&app, GEAR), 2);
    assert!(app.world_mut().run_system_once(|b: Bridge| b.owns(40)).unwrap(), "same-subject scroll refresh preserves the owner");
    event(&mut app, 40, TouchPhase::Ended, at - Vec2::Y * 60.0); app.update();
    assert_eq!(app.world().resource::<player_repair::PipRepair>().0.as_ref().unwrap().selected, 0);
    tap(&mut app, 41, Action::Select(1)); raw_tap(&mut app, 42, Action::Confirm);
    let donor = app.world().resource::<player_repair::PipRepair>().0.as_ref().unwrap().offer.parts[0].clone();
    let removed: i32 = app.world_mut().resource_mut::<SimState>().world.inventory_mut(FormId::PLAYER_REF).unwrap()
        .take_at(donor.item, 1, fnv_sim::inventory::Pick::row(Some(donor.condition), donor.extra.mods)).iter().map(|g| g.1).sum();
    assert_eq!(removed, 1); app.update();
    assert_eq!(count(&app, GEAR), 1);
    let inv = app.world().resource::<SimState>().world.peek(FormId::PLAYER_REF).unwrap().inventory.as_ref().unwrap();
    assert!(inv.groups().any(|(id, unit, n)| id == GEAR && n == 1 && unit.condition == Some(0.4)), "stale donor confirmation must not change the target");
    assert!(app.world().resource::<player_repair::PipRepair>().0.as_ref().unwrap().offer.parts.is_empty());
}

#[test]
fn actual_sheet_taffy_bounds_match_raw_targets_at_density_scale_and_safe_insets() {
    use bevy::app::{HierarchyPropagatePlugin, PropagateSet};
    use bevy::asset::{AssetApp, AssetPlugin};
    use bevy::camera::{Camera2d, ComputedCameraValues, RenderTargetInfo, visibility::VisibilityPlugin};
    use bevy::text::{TextPlugin, detect_text_needs_rerender, load_font_assets_into_font_collection};
    use bevy::ui::{ComputedUiRenderTargetInfo, sync_font_size_to_em_size, ui_layout_system, ui_surface::UiSurface,
        update::{propagate_ui_target_cameras, update_clipping_system}, widget::measure_text_system};
    for (density, ui_scale) in [(1.0, 1.0), (2.0, 1.5)] {
        let mut app = fixture(false, false);
        app.add_plugins((AssetPlugin::default(), TextPlugin, bevy::transform::TransformPlugin))
            .init_asset::<Mesh>().init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
            .add_plugins(VisibilityPlugin)
            .add_plugins(HierarchyPropagatePlugin::<ComputedUiTargetCamera>::new(PostUpdate))
            .add_plugins(HierarchyPropagatePlugin::<ComputedUiRenderTargetInfo>::new(PostUpdate))
            .init_resource::<UiSurface>();
        app.add_systems(PostUpdate, (ApplyDeferred, propagate_ui_target_cameras, sync_font_size_to_em_size,
            measure_text_system, ui_layout_system, update_clipping_system).chain().after(sheet::draw)
            .after(load_font_assets_into_font_collection).after(detect_text_needs_rerender));
        app.configure_sets(PostUpdate, PropagateSet::<ComputedUiTargetCamera>::default().after(propagate_ui_target_cameras).before(measure_text_system));
        app.configure_sets(PostUpdate, PropagateSet::<ComputedUiRenderTargetInfo>::default().after(propagate_ui_target_cameras).before(measure_text_system));
        let old = app.world().resource::<State>().context.unwrap();
        let physical = (Vec2::new(640.0, 360.0) * density).as_uvec2();
        let mut resolution = WindowResolution::new(physical.x, physical.y); resolution.set_scale_factor_override(Some(density));
        app.world_mut().get_mut::<Window>(old.window).unwrap().resolution = resolution;
        let safe = SafeInsets { bottom: 24.0, left: 12.0, right: 12.0, top: 0.0 };
        app.insert_resource(safe).insert_resource(UiScale(ui_scale));
        app.world_mut().entity_mut(old.camera).insert((Camera2d, Camera { computed: ComputedCameraValues {
            target_info: Some(RenderTargetInfo { physical_size: physical, scale_factor: density }), ..default()
        }, ..default() }));
        let context = Context::from_window(old.window, old.camera, app.world().get::<Window>(old.window).unwrap(), safe, ui_scale);
        app.world_mut().run_system_once(move |mut b: Bridge| b.prepare(Some(context), true)).unwrap();
        for _ in 0..6 { app.update(); }
        let mut rows = app.world_mut().query::<(&sheet::SheetRow, &ComputedNode, &UiGlobalTransform)>();
        let painted: Vec<_> = rows.iter(app.world()).map(|(row, n, t)| (row.0, Rect::from_center_size(t.transform_point2(Vec2::ZERO), n.size()))).collect();
        assert_eq!(painted.len(), 2);
        for (index, rect) in painted {
            assert!((rect.height() - ROW * density).abs() < 0.05, "actual target stays48 logical px at {density}/{ui_scale}");
            let logical = rect.center() / density;
            assert_eq!(sheet::Geometry::new(context).target(logical, app.world().resource::<State>().source.as_deref().unwrap()), Some(Action::Select(index)));
        }
        let mut buttons = app.world_mut().query::<(&sheet::SheetButton, &ComputedNode, &UiGlobalTransform)>();
        for (button, node, transform) in buttons.iter(app.world()) {
            assert!((node.size().y - ROW * density).abs() < 0.05);
            if button.0 == Action::Cancel {
                assert_eq!(sheet::Geometry::new(context).target(transform.transform_point2(Vec2::ZERO) / density, app.world().resource::<State>().source.as_deref().unwrap()), Some(Action::Cancel));
            }
        }
        tap(&mut app, 70, Action::Select(1)); tap(&mut app, 71, Action::Confirm);
        assert_eq!(count(&app, GEAR), 1, "rendered/raw geometry must still reach the real consumer");
    }
}

#[test]
fn saved_opt_out_cancels_pending_and_held_confirmation_and_reloads_without_touching_other_ini_sections() {
    assert!(State::for_platform(true).enabled);
    assert!(!State::for_platform(false).enabled);
    let path = std::env::temp_dir().join(format!("nvf-maintenance-setting-{}-{}.ini", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::write(&path, "[Unrelated]\nkeep=present\n[Settings]\ntouch-maintenance-v1=1\n").unwrap();
    let mut app = fixture(false, false);
    app.insert_resource(crate::settings::SettingsFile(Some(path.clone())));
    let index = app.world().resource::<crate::settings::Settings>().index(SETTING_KEY).unwrap();
    tap(&mut app, 80, Action::Select(1)); raw_tap(&mut app, 81, Action::Confirm);
    let at = center(&app, Action::Confirm); event(&mut app, 82, TouchPhase::Started, at);
    assert_eq!(crate::settings::flip(app.world_mut(), index), Some(false));
    app.update(); assert_eq!(count(&app, GEAR), 2);
    assert!(app.world_mut().run_system_once(|b: Bridge| b.owns(82) && !b.available()).unwrap());
    event(&mut app, 82, TouchPhase::Ended, at);
    assert!(!app.world_mut().run_system_once(|b: Bridge| b.owns(82)).unwrap());
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("keep=present") && saved.contains("touch-maintenance-v1=0"));
    let mut reload = App::new();
    reload.insert_resource(State::for_platform(true)).add_plugins(MaintenanceTouchPlugin)
        .insert_resource(crate::settings::SettingsFile(Some(path.clone())));
    crate::settings::apply_stored(reload.world_mut());
    assert!(!reload.world().resource::<State>().enabled, "explicit saved opt-out overrides Android default");
    assert_eq!(crate::settings::flip(app.world_mut(), index), Some(true)); app.update();
    tap(&mut app, 83, Action::Confirm); assert_eq!(count(&app, GEAR), 1, "fresh stable input works after enabling");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn actual_script_modal_or_other_maintenance_owner_opened_after_raw_confirm_cannot_consume() {
    use fnv_game::game::menus;
    for modding in [false, true] {
        for blocker in [menus::MESSAGE, if modding { menus::REPAIR } else { menus::ITEM_MOD }] {
            let mut app = fixture(modding, true);
            tap(&mut app, 110, Action::Select(1)); raw_tap(&mut app, 111, Action::Confirm);
            app.world_mut().resource_mut::<crate::script::ScriptState>().rt.as_mut().unwrap().game.open_menu(blocker);
            app.update();
            assert_eq!(count(&app, if modding { MOD } else { GEAR }), 2, "late actual top-menu {blocker} owns the action");
            {
                let mut script = app.world_mut().resource_mut::<crate::script::ScriptState>();
                let game = &mut script.rt.as_mut().unwrap().game;
                game.close_menu(); game.open_menu(if modding { menus::ITEM_MOD } else { menus::REPAIR });
            }
            tap(&mut app, 112, Action::Confirm);
            assert_eq!(count(&app, if modding { MOD } else { GEAR }), 1, "fresh explicit confirmation works for its actual owner kind");
        }
    }
}
