//! Independent saved policy; visual suppression never changes input ownership.
use bevy::prelude::*;
use nvf_play::{settings::{self, Setting}, touch_safe_area::SafeInsets};

#[derive(Resource, Clone, Copy, PartialEq, Eq)]
pub(super) struct Options {
    pub left: bool,
    pub right: bool,
    pub close: bool,
    pub graphics: bool,
    pub wheel: bool,
}
impl Default for Options {
    fn default() -> Self { Self { left: false, right: false, close: false, graphics: true, wheel: true } }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Target { Left, Right, Close }
impl Options {
    pub fn regions(self, size: Vec2, safe: SafeInsets) -> Vec<(Target, Rect)> {
        let values=[safe.left,safe.top,safe.right,safe.bottom];
        if !size.is_finite() || size.min_element()<144.0 || values.iter().any(|v|!v.is_finite() || *v<0.0) { return Vec::new(); }
        let min=Vec2::new(safe.left,safe.top)+Vec2::splat(8.0);
        let max=size-Vec2::new(safe.right,safe.bottom)-Vec2::splat(8.0);
        if (max-min).min_element()<96.0 { return Vec::new(); }
        let y=((min.y+max.y)*0.5).clamp(min.y+48.0,max.y-48.0);
        let mut out=Vec::with_capacity(3);
        if self.left {out.push((Target::Left,Rect::from_center_size(Vec2::new(min.x+24.0,y),Vec2::new(48.0,96.0))));}
        if self.right {out.push((Target::Right,Rect::from_center_size(Vec2::new(max.x-24.0,y),Vec2::new(48.0,96.0))));}
        if self.close {out.push((Target::Close,Rect::from_center_size(Vec2::new((min.x+max.x)*0.5,max.y-24.0),Vec2::splat(48.0))));}
        out
    }
    /// The graphics bit is intentionally absent: paint changes cannot retire input.
    pub fn dismissal(self) -> (bool,bool,bool) { (self.left,self.right,self.close) }
}
fn changed(world:&mut World,before:Options) {
    let after=*world.resource::<Options>();
    if before.dismissal()!=after.dismissal() {
        if let Some(mut arm)=world.get_resource_mut::<nvf_play::pipboy::gesture::ArmGesture>() {arm.abort();}
    }
    if before.wheel && !after.wheel {super::pipboy_model::cancel_wheel(world);}
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<Options>();
    macro_rules! switch {
        ($key:literal,$label:literal,$note:literal,$field:ident) => {
            settings::add_setting(app,Setting { key:$key,group:"Controls",label:$label,note:$note,switches:"",
                get:|w|w.resource::<Options>().$field,set:|w,v| {let before=*w.resource::<Options>();w.resource_mut::<Options>().$field=v;changed(w,before);},slider:None });
        };
    }
    switch!("touch-pipboy-dismiss-left","Pip-Boy left dismissal handle","Drag down from the safe left handle to dismiss. Reverse before release to keep it open.",left);
    switch!("touch-pipboy-dismiss-right","Pip-Boy right dismissal handle","Drag down from the safe right handle to dismiss. Reverse before release to keep it open.",right);
    switch!("touch-pipboy-close-button","Pip-Boy close button","Adds an explicit close target. Its touch region stays active when button graphics are hidden.",close);
    switch!("touch-pipboy-button-graphics","Show Pip-Boy button graphics","Shows added button and handle graphics. Hiding them retains identical active touch regions and actions.",graphics);
    switch!("touch-pipboy-model-wheel","Touch Pip-Boy model wheel","Drag the physical wheel on the rendered Pip-Boy to scroll the active list. Mouse, controller and scrollbars stay available.",wheel);
}

/// Explicit full-primary-window UI target; does not change the default UI owner.
#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct OverlayCanvas<'w,'s> {
    default: bevy::ui::DefaultUiCamera<'w,'s>,
    windows: Query<'w,'s,(Entity,&'static Window),With<bevy::window::PrimaryWindow>>,
    cameras: Query<'w,'s,(Entity,&'static Camera,&'static bevy::camera::RenderTarget)>,
    scale: Option<Res<'w,UiScale>>,
}
#[derive(Clone,Copy,PartialEq)]
pub(super) struct PaintFrame {pub camera:Entity,pub scale:f32}
impl OverlayCanvas<'_, '_> {
    pub fn frame(&self)->Option<PaintFrame> {
        let (window,w)=self.windows.single().ok()?;
        let logical=w.resolution.size();let physical=UVec2::new(w.physical_width(),w.physical_height());
        let scale=self.scale.as_ref().map_or(1.0,|s|s.0);
        if !logical.is_finite() || !scale.is_finite() || scale<=0.0 || !(logical/scale).is_finite() || !(48.0/scale).is_finite() {return None;}
        let valid=|camera:&Camera,target:&bevy::camera::RenderTarget|camera.is_active
            && camera.physical_target_size()==Some(physical)
            && camera.physical_viewport_rect()==Some(URect::from_corners(UVec2::ZERO,physical))
            && camera.logical_viewport_rect()==Some(Rect::from_corners(Vec2::ZERO,logical))
            && matches!(target.normalize(Some(window)),Some(bevy::camera::NormalizedRenderTarget::Window(w)) if w.entity()==window);
        let camera=self.default.get().filter(|e|self.cameras.get(*e).is_ok_and(|(_,c,t)|valid(c,t)))
            .or_else(||self.cameras.iter().filter(|(_,c,t)|valid(c,t)).max_by_key(|(e,c,_)|(c.order,std::cmp::Reverse(e.to_bits()))).map(|(e,_,_)|e))?;
        Some(PaintFrame {camera,scale})
    }
}

#[cfg(test)]
mod tests;
