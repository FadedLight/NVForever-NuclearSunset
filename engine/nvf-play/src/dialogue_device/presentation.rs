use super::*;
use bevy::{asset::RenderAssetUsages, camera::{visibility::RenderLayers, ImageRenderTarget, RenderTarget},
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
    window::PrimaryWindow};
use crate::{dialogue_menu::DialogueMenu, dialogue_touch::{TouchDialogue, appearance::{Appearance, Theme}}};

#[derive(Clone, PartialEq)]
struct Key {
    kind: DeviceKind,
    window: Entity,
    display: Rect,
    density: f32,
    scale: f32,
    list: Option<crate::dialogue::ListId>,
}

#[derive(Resource, Default)]
struct Owned {
    key: Option<Key>,
    generation: u64,
    owner: Option<Entity>,
    camera: Option<Entity>,
    image: Option<Handle<Image>>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<Owned>().add_systems(Update, prepare
        .after(crate::PlaySet::Scripts).after(crate::dialogue_split::Resolve)
        .before(crate::PlaySet::Ui));
}

#[allow(clippy::too_many_arguments)]
fn prepare(
    mut commands: Commands,
    appearance: Res<Appearance>, touch: Res<TouchDialogue>, menu: Res<DialogueMenu>,
    state: Option<Res<State<crate::PlayState>>>, paused: Option<Res<crate::pause::Paused>>,
    script: Option<Res<crate::script::ScriptState>>, creator: Option<Res<crate::creator::CreatorMenu>>,
    safe: Res<crate::touch_safe_area::SafeInsets>, scale: Option<Res<UiScale>>,
    windows: Query<(Entity, &Window), With<PrimaryWindow>>,
    mut images: Option<ResMut<Assets<Image>>>, mut owned: ResMut<Owned>,
) {
    let kind = match appearance.theme {
        Theme::RetroConsole => Some(DeviceKind::Terminal),
        Theme::WristTerminal => Some(DeviceKind::PipBoy),
        _ => None,
    };
    let key = kind.filter(|_| touch.enabled && menu.view.is_some()
        && crate::dialogue_touch::available(state.as_deref(), paused.as_deref(), script.as_deref(), creator.as_deref()))
        .and_then(|kind| {
            let (entity, window) = windows.single().ok()?;
            let density = window.scale_factor();
            // A physical device owns its frame. Fitting it into half a phone
            // window would shrink its glass below usable touch/text dimensions.
            let safe=crate::touch_safe_area::SafeInsets::from_array([safe.left,safe.top,safe.right,safe.bottom])?;
            let minimum=Vec2::new(safe.left,safe.top);
            let maximum=Vec2::new(window.width()-safe.right,window.height()-safe.bottom);
            if !minimum.is_finite() || !maximum.is_finite() || !maximum.cmpgt(minimum).all() {return None;}
            let logical = Rect::from_corners(minimum,maximum);
            let display = Rect::from_corners(logical.min * density, logical.max * density);
            let scale = scale.as_ref().map_or(1.0, |s| s.0);
            (positive_rect(display) && density.is_finite() && density > 0.0
                && scale.is_finite() && scale > 0.0 && window.focused)
                .then(|| Key {kind, window: entity, display, density, scale, list: menu.list.clone()})
        });
    if key == owned.key { return; }
    owned.generation = owned.generation.wrapping_add(1);
    owned.key = key.clone();
    let Some(key) = key else {
        if let Some(camera) = owned.camera.take() { commands.entity(camera).despawn(); }
        if let Some(owner) = owned.owner.take() { commands.entity(owner).despawn(); }
        if let Some(image) = owned.image.take() {
            if let Some(images) = images.as_deref_mut() { images.remove(image.id()); }
        }
        commands.remove_resource::<Request>();
        return;
    };
    let Some(images) = images.as_deref_mut() else {
        // A model presentation requires a real render provider; a core-only
        // synthetic App cannot turn a requested model into a flat approximation.
        owned.key = None;
        return;
    };
    let target = UVec2::new(1280, 960);
    let image = owned.image.get_or_insert_with(|| {
        let mut image = Image::new_fill(Extent3d {width: target.x, height: target.y,
            depth_or_array_layers: 1}, TextureDimension::D2, &[0, 0, 0, 255],
            TextureFormat::Bgra8UnormSrgb, RenderAssetUsages::default());
        image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
        images.add(image)
    }).clone();
    let camera = *owned.camera.get_or_insert_with(|| commands.spawn((
        Name::new("Dialogue device content camera"), Camera2d,
        Camera { order: -4, clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
        RenderTarget::Image(ImageRenderTarget::from(image.clone())), RenderLayers::none(),
    )).id());
    let owner = *owned.owner.get_or_insert_with(|| commands.spawn(Name::new("Dialogue device surface owner")).id());
    let stamp = Stamp {owner, generation: owned.generation, window: key.window,
        kind: key.kind, display: key.display, density: key.density};
    let size = match key.kind {
        DeviceKind::Terminal => crate::terminal::screen::CANVAS,
        DeviceKind::PipBoy => crate::pipboy::CANVAS,
    };
    commands.insert_resource(Request {stamp, image, camera, target,
        canvas: Rect::from_corners(Vec2::ZERO, size)});
}
