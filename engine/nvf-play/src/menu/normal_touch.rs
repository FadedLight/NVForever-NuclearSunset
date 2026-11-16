//! Finger input over the ordinary barter presentation. No replacement UI is drawn.
//! The wrapper owns raw input/provenance; this adapter delays a tap until End and
//! resolves its one-shot envelope inside the ordinary MenuInput consumer.
use super::{model::{Category, Command, MenuModel, Mode, Quantity, RowKey, Side, Trade},
    ActiveMenu, CmdButton, MenuInput, RowButton, AmountMeter};
use bevy::{ecs::system::SystemParam, input::touch::{TouchInput, TouchPhase},
    prelude::*, ui::{CalculatedClip, ComputedUiTargetCamera, FocusPolicy, UiGlobalTransform, UiStack}};

pub use super::barter_touch::Frame;
const SLOP: f32 = 12.0;
const MAX_IDS: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Target { Row(Side, usize), Command(Command), Meter(f32), Pane(Side) }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Source { Node(Entity), Tile(fnv_ui::TileId, fnv_ui::TileId) }
#[derive(Clone, Copy, Debug)]
pub(crate) struct Hit { pub source: Source, pub target: Target, pub pane: Option<Side>, pub row_step:f32 }

#[derive(Clone, Copy, PartialEq)]
struct Stamp { window: Entity, camera: Entity, size: Vec2,
    safe: crate::touch_safe_area::SafeInsets, density: f32, ui_scale: f32, menu_units:f32 }
impl From<Frame> for Stamp {
    fn from(f: Frame) -> Self { Self { window:f.window, camera:f.camera, size:f.size,
        safe:f.safe, density:f.density, ui_scale:f.ui_scale, menu_units:crate::interface::unit_scale() } }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct RowStamp { side: Side, index: usize, key: RowKey, count: i32,
    bartered: bool, price: Option<u32> }
impl RowStamp {
    fn of(m: &MenuModel, side: Side, index: usize) -> Option<Self> {
        let r=m.list(side).rows.get(index)?;
        Some(Self { side,index,key:RowKey::at(&m.list(side).rows,index)?,count:r.count,
            bartered:r.bartered,price:r.price.map(f32::to_bits) })
    }
    fn matches(&self,m:&MenuModel)->bool { Self::of(m,self.side,self.index).as_ref()==Some(self) }
}
/// Identity actually painted on a hand-built row, checked before accepting a tap.
#[derive(Component)]
pub(crate) struct RenderedRow(RowStamp);
impl RenderedRow {
    pub(crate) fn of(m:&MenuModel,side:Side,index:usize)->Option<Self> {
        RowStamp::of(m,side,index).map(Self)
    }
}
#[derive(Clone)]
struct Guard { mode:Mode, category:[Category;2], quantity:Option<Quantity>,
    focus:Side, selected:[usize;2], row:Option<RowStamp>, trade:Option<Trade> }
impl Guard {
    fn of(m:&MenuModel,target:Target)->Self {
        let row=match target {
            Target::Row(s,i)=>RowStamp::of(m,s,i),
            _=>m.quantity.and_then(|q|RowStamp::of(m,q.side,q.row)),
        };
        Self { mode:m.mode,category:m.category,quantity:m.quantity,focus:m.focus,selected:m.selected,
            row,trade:m.trade.clone() }
    }
    fn matches(&self,m:&MenuModel)->bool {
        self.mode==m.mode && self.category==m.category && self.quantity==m.quantity
            && self.focus==m.focus && self.selected==m.selected
            && self.row.as_ref().is_none_or(|r|r.matches(m))
            && self.trade==m.trade
    }
}
/// Source state attached by the ordinary painter, not reconstructed from the
/// current model at a later touch. A stale frame cannot approve a new quote.
#[derive(Component)]
pub(crate) struct RenderedMenu(Guard);
impl RenderedMenu {
    pub(crate) fn of(m:&MenuModel)->Self {Self(Guard::of(m,Target::Command(Command::Accept)))}
}
struct Contact { window:Entity,id:u64,stamp:Stamp,origin:Vec2,previous:Vec2,
    hit:Hit,guard:Guard,dragged:bool,vertical:bool,remainder:f64 }
struct Tap { frame:Frame,id:u64,point:Vec2,source:Source,target:Target,
    guard:Guard,commands:[Option<Command>;2] }
struct Scroll { frame:Frame,id:u64,mode:Mode,category:[Category;2],side:Side,
    pixels:f64,rows:i64,native:bool }
struct Armed { token:u64,epoch:u64,tap:Tap }
#[derive(Clone,Copy)]
pub(crate) struct NativeTap {
    pub token:u64,pub frame:Frame,pub id:u64,pub point:Vec2,pub source:Source,pub target:Target,
}

#[derive(Resource,Default)]
pub struct NormalBarter {
    frame:Option<Frame>,return_frame:Option<Frame>,stamp:Option<Stamp>,mode:Option<Mode>,epoch:u64,
    contact:Option<Contact>,pending:Option<Tap>,scroll:Option<Scroll>,armed:Option<Armed>,
    native:Option<NativeTap>,
    retired:Vec<(Entity,u64)>,overflow:Option<Entity>,hidden:bool,barrier:bool,draining:Option<Entity>,
    secondary:bool,next:u64,
}
impl NormalBarter {
    fn retire(&mut self,w:Entity,id:u64) {
        if self.retired.contains(&(w,id)){return}
        if self.retired.len()<MAX_IDS {self.retired.push((w,id));}
        else {
            self.overflow=Some(w);self.contact=None;self.pending=None;self.scroll=None;self.armed=None;self.native=None;
            self.draining=Some(w);self.epoch=self.epoch.wrapping_add(1);
        }
    }
    pub fn cancel(&mut self) {
        if let Some(w)=self.contact.as_ref().map(|c|c.window)
            .or_else(||self.pending.as_ref().map(|t|t.frame.window))
            .or_else(||self.armed.as_ref().map(|a|a.tap.frame.window)) {self.draining=Some(w);}
        if let Some(c)=self.contact.take(){self.retire(c.window,c.id);}
        if let Some(t)=self.pending.take(){self.retire(t.frame.window,t.id);}
        if let Some(a)=self.armed.take(){self.retire(a.tap.frame.window,a.tap.id);}
        self.scroll=None;self.native=None;self.barrier=true;self.epoch=self.epoch.wrapping_add(1);
    }
    /// This witness must mean no held touch or release/cancel edge in any window.
    /// It never clears Touches, raw messages or hardware state.
    pub fn observe_neutral(&mut self,neutral:bool) {
        if neutral {self.retired.clear();self.overflow=None;self.draining=None;self.secondary=false;}
    }
    pub fn hardware_intent(&mut self){self.cancel();self.hidden=true;}
    pub fn restore_touch(&mut self){self.cancel();self.hidden=false;}
    pub fn captures_touch(&self,window:Entity,id:u64)->bool {
        self.retired.contains(&(window,id)) || self.overflow==Some(window)
            || self.frame.is_some_and(|f|f.window==window && !self.hidden)
    }
    pub fn visible(&self)->bool {self.frame.is_some() && !self.hidden}
    pub(crate) fn take_native(&mut self)->Option<NativeTap> {
        let n=self.native.take()?;
        self.armed.as_ref().filter(|a|a.token==n.token && a.epoch==self.epoch
            && self.frame==Some(n.frame) && self.stamp==Some(Stamp::from(n.frame))
            && !self.hidden).map(|_|n)
    }
    pub(crate) fn native_commands(&self,n:NativeTap,commands:&[Command])->Option<u64> {
        let a=self.armed.as_ref().filter(|a|a.token==n.token && a.epoch==self.epoch)?;
        a.tap.commands.into_iter().flatten().eq(commands.iter().copied()).then_some(n.token)
    }
    pub(crate) fn finish_native(&mut self,token:u64,emitted:bool) {
        if !emitted && self.armed.as_ref().is_some_and(|a|a.token==token){self.armed=None;}
    }
}

#[derive(SystemParam)]
struct Admission<'w,'s> {
    live:super::barter_touch::LiveAdmission<'w,'s>,
    script:Option<Res<'w,crate::script::ScriptState>>,
    creator:Option<Res<'w,crate::creator::CreatorMenu>>,
}
impl Admission<'_,'_> {
    fn allows(&self,f:Frame,id:u64,m:&MenuModel)->bool {
        matches!(m.mode,Mode::Barter(_)) && self.live.allows(f,id)
            && super::barter_touch::menu_allows(self.script.as_deref(),self.creator.as_deref(),m)
    }
}
type NodeData=(&'static ComputedNode,&'static UiGlobalTransform,&'static ComputedUiTargetCamera,
    &'static InheritedVisibility,Option<&'static CalculatedClip>,
    Option<&'static RowButton>,Option<&'static CmdButton>,Option<&'static AmountMeter>,
    Option<&'static RenderedRow>,Option<&'static FocusPolicy>);
#[derive(SystemParam)]
struct Geometry<'w,'s> {
    stack:Option<Res<'w,UiStack>>,
    nodes:Query<'w,'s,NodeData>,
    tiles:Option<Res<'w,crate::tiles::barter_menu::BarterTiles>>,
    parents:Query<'w,'s,&'static ChildOf>,
    menus:Query<'w,'s,&'static RenderedMenu>,
}
impl Geometry<'_,'_> {
    fn hit(&self,script:Option<&crate::script::ScriptState>,f:Frame,m:&MenuModel,point:Vec2)->Option<Hit> {
        if let Some(tiles)=self.tiles.as_ref().filter(|t|t.menu().is_some()) {
            let rt=script?.rt.as_ref()?;
            if m.quantity.is_none() || tiles.normal_touch_quantity(&rt.game.ui_tree) {
                return tiles.normal_touch_hit(&rt.game.ui_tree,m,point,f.size);
            }
        }
        let physical=point*f.density;
        for &entity in self.stack.as_ref()?.uinodes.iter().rev() {
            let Ok((node,transform,camera,visible,clip,row,command,meter,painted,focus))=self.nodes.get(entity) else {continue};
            if camera.get()!=Some(f.camera) || !visible.get() || !node.size().is_finite()
                || node.size().min_element()<=0.0 || !node.contains_point(*transform,physical)
                || clip.is_some_and(|c|!c.contains_point(physical)) {continue}
            let mut ancestor=Some(entity);
            let mut current=false;
            while let Some(parent)=ancestor {
                if let Ok(painted)=self.menus.get(parent) {current=painted.0.matches(m);break}
                ancestor=self.parents.get(parent).ok().map(ChildOf::parent);
            }
            if !current {continue}
            let target=if let Some(row)=row {
                if m.quantity.is_some() || painted.is_none_or(|p|!p.0.matches(m)){continue}
                Target::Row(row.0,row.1)
            } else if let Some(command)=command {
                if m.quantity.is_some() && !matches!(command.0,Command::Amount(_)|Command::AmountStep(_)|Command::AmountOk|Command::AmountCancel) {
                    continue
                }
                Target::Command(command.0)
            }
            else if meter.is_some() {
                let part=node.normalize_point(*transform,physical)?.x+0.5;
                Target::Meter(part.clamp(0.0,1.0))
            } else {
                if focus.is_some_and(|f|*f==FocusPolicy::Block){return None}
                continue
            };
            return Some(Hit {source:Source::Node(entity),target,
                pane:if let Target::Row(s,_)=target {Some(s)} else {None},
                row_step:node.size().y/f.density});
        }
        None
    }
}

/// Parent's sole-reader adapter. There is no MessageReader or virtual mouse here.
#[derive(SystemParam)]
pub struct Bridge<'w,'s> {
    state:ResMut<'w,NormalBarter>,active:Option<Res<'w,ActiveMenu>>,
    options:Option<Res<'w,super::barter_touch::Options>>,
    admission:Admission<'w,'s>,geometry:Geometry<'w,'s>,
}
impl Bridge<'_,'_> {
    /// Once per real parent replay frame, before its raw batch. Unconsumed work
    /// expires here instead of being retried when a later native menu reappears.
    pub fn prepare(&mut self,f:Frame) {
        self.state.pending=None;self.state.scroll=None;self.state.armed=None;self.state.native=None;
        self.state.observe_neutral(f.neutral);self.state.barrier=false;
        let mode=self.active.as_ref().and_then(|a|a.0.as_ref()).map(|m|m.mode)
            .filter(|m|matches!(m,Mode::Barter(_)));
        let stamp=Stamp::from(f);
        if self.state.stamp!=Some(stamp) || self.state.mode!=mode || !f.admitted || f.fresh
            || self.options.as_ref().is_some_and(|o|!o.enabled) {
            let had_owner=self.state.contact.is_some() || self.state.pending.is_some() || self.state.armed.is_some();
            self.state.cancel();
            if !had_owner && f.admitted && !f.fresh {self.state.barrier=false;}
        }
        if self.state.stamp.is_some_and(|old|old.window!=f.window){
            self.state.secondary=false;
            if f.admitted && !f.fresh {self.state.barrier=false;}
        }
        self.state.mode=mode;self.state.stamp=Some(stamp);
        self.state.return_frame=mode.filter(|_|f.admitted
            && self.options.as_ref().is_none_or(|o|o.enabled)).map(|_|f);
        self.state.frame=mode.filter(|_|f.admitted && !f.fresh
            && self.options.as_ref().is_none_or(|o|o.enabled)).map(|_|f);
    }
    pub fn event(&mut self,e:&TouchInput)->bool {
        if self.state.retired.contains(&(e.window,e.id)) || self.state.overflow==Some(e.window){return true}
        let Some(f)=self.state.frame.filter(|f|f.window==e.window) else{return false};
        let Some(m)=self.active.as_ref().and_then(|a|a.0.as_ref()) else{return false};
        if !self.admission.allows(f,e.id,m) || !e.position.is_finite() {
            self.state.cancel();return true
        }
        if e.phase==TouchPhase::Started && self.state.hidden {
            self.state.restore_touch();self.state.retire(e.window,e.id);self.state.draining=Some(e.window);return true
        }
        if self.state.hidden{return false}
        if self.state.barrier || self.state.draining==Some(e.window) {
            if e.phase==TouchPhase::Started{self.state.retire(e.window,e.id);}
            return true
        }
        if e.phase==TouchPhase::Started {
            if self.state.contact.is_some() {
                if self.state.contact.as_ref().is_some_and(|c|c.window==e.window && c.id==e.id) {self.state.cancel();}
                else {self.state.secondary=true;}
                self.state.retire(e.window,e.id);return true
            }
            if let Some(hit)=self.geometry.hit(self.admission.script.as_deref(),f,m,e.position)
                .filter(|h|matches!(h.source,Source::Tile(..)) || h.row_step.is_finite() && h.row_step>0.0) {
                self.state.contact=Some(Contact {window:e.window,id:e.id,stamp:Stamp::from(f),
                    origin:e.position,previous:e.position,hit,guard:Guard::of(m,hit.target),
                    dragged:false,vertical:false,remainder:0.0});
            } else {self.state.retire(e.window,e.id);}
            return true
        }
        if !self.state.contact.as_ref().is_some_and(|c|c.window==e.window && c.id==e.id){return true}
        let mut c=self.state.contact.take().unwrap();
        if e.phase==TouchPhase::Canceled || c.stamp!=Stamp::from(f)
            || c.guard.mode!=m.mode || c.guard.category!=m.category || c.guard.quantity!=m.quantity {
            self.state.pending=None;self.state.scroll=None;self.state.barrier=true;
            self.state.draining=Some(e.window);
            self.state.retire(e.window,e.id);return true
        }
        c.dragged|=e.position.distance(c.origin)>SLOP;
        if c.dragged && c.hit.pane.is_some() {
            let d=e.position-c.origin;c.vertical|=d.y.abs()>=d.x.abs();
        }
        if let Some(side)=c.hit.pane.filter(|_|c.vertical) {
            c.remainder+=f64::from(c.previous.y)-f64::from(e.position.y);
            let native=matches!(c.hit.source,Source::Tile(..));
            let (pixels,rows)=if native {
                let pixels=c.remainder;c.remainder=0.0;(pixels,0)
            } else {
                // Ordinary hand-built lists draw whole rows. Quantize from this
                // contact's actual painted row height (wrapping/scale included),
                // not a fixed logical-pixel constant; no 1:1 pixel-scroll claim.
                let step=f64::from(c.hit.row_step);
                let rows=(c.remainder/step).trunc() as i64;
                c.remainder%=step;
                (0.0,rows)
            };
            if pixels!=0.0 || rows!=0 {
                match self.state.scroll.as_mut() {
                    Some(s) if s.side==side=>{s.pixels+=pixels;s.rows=s.rows.saturating_add(rows);}
                    None=>self.state.scroll=Some(Scroll {frame:f,id:e.id,mode:m.mode,
                        category:m.category,side,pixels,rows,native}),
                    _=>{}
                }
            }
        }
        c.previous=e.position;
        if e.phase==TouchPhase::Ended {
            if !c.dragged && c.guard.matches(m)
                && let Some(hit)=self.geometry.hit(self.admission.script.as_deref(),f,m,e.position)
                    .filter(|h|h.source==c.hit.source && same_target(h.target,c.hit.target)) {
                self.state.pending=Some(Tap {frame:f,id:e.id,point:e.position,
                    source:hit.source,target:hit.target,guard:c.guard,
                    commands:commands(hit.target,m)});
            }
            self.state.barrier=true;
            if self.state.secondary{self.state.draining=Some(e.window);}
            self.state.retire(e.window,e.id);
        } else {self.state.contact=Some(c);}
        true
    }
    pub fn captures_touch(&self,w:Entity,id:u64)->bool {self.state.captures_touch(w,id)}
    pub fn visible(&self)->bool {self.state.visible()}
    pub fn committed_this_batch(&self)->bool {self.state.barrier && self.state.pending.is_some()}
    pub fn cancel(&mut self){self.state.cancel();self.state.frame=None;self.state.return_frame=None;}
    pub fn hardware_intent(&mut self){self.state.hardware_intent();}
    pub fn restore_touch(&mut self){self.state.restore_touch();}
    pub fn observe_neutral(&mut self,n:bool){self.state.observe_neutral(n);}
    /// The parent may call this only with the actual selector-consumed Started
    /// edge, after preparing this current normal menu and actual primary frame.
    pub fn consume_touch_return(&mut self,e:&TouchInput)->bool {
        let Some(f)=self.state.return_frame.filter(|f|f.window==e.window) else{return false};
        let Some(m)=self.active.as_ref().and_then(|a|a.0.as_ref()) else{return false};
        if e.phase!=TouchPhase::Started || !matches!(m.mode,Mode::Barter(_))
            || !self.admission.live.allows_consumed_return(f,e.id)
            || !super::barter_touch::menu_allows(self.admission.script.as_deref(),
                self.admission.creator.as_deref(),m) {return false}
        self.state.restore_touch();self.state.retire(e.window,e.id);self.state.draining=Some(e.window);true
    }
}
pub(crate) fn same_target(a:Target,b:Target)->bool {
    match(a,b){(Target::Meter(_),Target::Meter(_))=>true,_=>a==b}
}
fn commands(t:Target,m:&MenuModel)->[Option<Command>;2] {
    match t {
        Target::Row(s,i)=>{
            let v=crate::tiles::item_menu::row_commands(s,i,m.focus==s && m.selected[s as usize]==i,false);
            [v.first().copied(),v.get(1).copied()]
        }
        Target::Command(c)=>[Some(c),None],
        Target::Meter(part)=>[m.quantity.map(|q|Command::Amount(super::model::amount_at(part,q.max))),None],
        Target::Pane(_)=>[None,None],
    }
}

fn dispatch(mut state:ResMut<NormalBarter>,mut active:ResMut<ActiveMenu>,
    live:super::barter_touch::LiveAdmission,
    (mut script,creator):(Option<ResMut<crate::script::ScriptState>>,Option<Res<crate::creator::CreatorMenu>>),
    mut tiles:Option<ResMut<crate::tiles::barter_menu::BarterTiles>>,mut out:MessageWriter<MenuInput>) {
    if state.scroll.is_none() && state.pending.is_none(){return}
    let Some(m)=active.0.as_mut() else{state.cancel();return};
    if let Some(s)=state.scroll.take() {
        if state.frame==Some(s.frame) && state.stamp==Some(Stamp::from(s.frame))
            && s.mode==m.mode && s.category==m.category && m.quantity.is_none()
            && live.allows(s.frame,s.id)
            && super::barter_touch::menu_allows(script.as_deref(),creator.as_deref(),m) {
            if s.native {
                if let Some((tiles,rt))=tiles.as_mut().zip(script.as_mut().and_then(|s|s.rt.as_mut())) {
                    tiles.normal_touch_scroll(&mut rt.game.ui_tree,s.side,s.pixels,s.frame.size);
                }
            } else {
                let i=s.side as usize;
                m.scroll[i]=(m.scroll[i] as i64).saturating_add(s.rows)
                    .clamp(0,m.list(s.side).rows.len().saturating_sub(1) as i64) as usize;
            }
            m.pointer_pick=true;
        }
    }
    let Some(tap)=state.pending.take() else{return};
    if state.frame!=Some(tap.frame) || !live.allows(tap.frame,tap.id)
        || !super::barter_touch::menu_allows(script.as_deref(),creator.as_deref(),m) {return}
    if matches!(tap.source,Source::Tile(..)) && script.as_ref().is_none_or(|s|s.rt.is_none()){return}
    state.next=state.next.wrapping_add(1).max(1);
    let token=state.next;let epoch=state.epoch;
    let native=matches!(tap.source,Source::Tile(..)).then_some(NativeTap {
        token,frame:tap.frame,id:tap.id,point:tap.point,source:tap.source,target:tap.target });
    state.armed=Some(Armed {token,epoch,tap});
    if let Some(native)=native {state.native=Some(native);}
    else {out.write(MenuInput(Command::CommitNormalBarter(token)));}
}

/// Native feed validates the same current gates, lending its already-owned
/// ScriptState. It must not add an aliased ScriptState SystemParam.
#[derive(SystemParam)]
pub(crate) struct NativeAdmission<'w,'s> {
    live:super::barter_touch::LiveAdmission<'w,'s>,
    options:Option<Res<'w,super::barter_touch::Options>>,
    creator:Option<Res<'w,crate::creator::CreatorMenu>>,
}
impl NativeAdmission<'_,'_> {
    pub(crate) fn allows(&self,n:NativeTap,m:&MenuModel,game:&fnv_game::Game<'_>)->bool {
        matches!(m.mode,Mode::Barter(_)) && self.options.as_ref().is_none_or(|o|o.enabled)
            && self.live.allows(n.frame,n.id)
            && super::barter_touch::game_allows(Some(game),self.creator.as_deref(),m)
    }
}
#[derive(SystemParam)]
pub(crate) struct Commits<'w,'s> {
    state:Option<ResMut<'w,NormalBarter>>,
    options:Option<Res<'w,super::barter_touch::Options>>,
    live:super::barter_touch::LiveAdmission<'w,'s>,
    creator:Option<Res<'w,crate::creator::CreatorMenu>>,
    geometry:Geometry<'w,'s>,
}
impl Commits<'_,'_> {
    pub(crate) fn discard(&mut self,token:u64){
        if let Some(s)=self.state.as_mut()
            && s.armed.as_ref().is_some_and(|a|a.token==token){s.armed=None;}
    }
    pub(crate) fn resolve(&mut self,token:u64,m:&mut MenuModel,world:&mut fnv_sim::World<'_>,
        script:Option<&crate::script::ScriptState>)->Option<[Option<Command>;2]> {
        let s=self.state.as_mut()?;
        if s.armed.as_ref()?.token!=token{return None}
        let a=s.armed.take()?;
        if a.epoch!=s.epoch || s.hidden || s.frame!=Some(a.tap.frame)
            || s.stamp!=Some(Stamp::from(a.tap.frame))
            || self.options.as_ref().is_some_and(|o|!o.enabled)
            || !self.live.allows(a.tap.frame,a.tap.id)
            || !super::barter_touch::menu_allows(script,self.creator.as_deref(),m) {return None}
        if m.refresh(world).is_err() || !a.tap.guard.matches(m)
            || !self.geometry.hit(script,a.tap.frame,m,a.tap.point)
                .is_some_and(|h|h.source==a.tap.source && same_target(h.target,a.tap.target)) {
            m.status=Some("Trade changed. Review the item and amount again.".into());return None
        }
        Some(a.tap.commands)
    }
}
#[derive(SystemSet,Debug,Clone,Copy,PartialEq,Eq,Hash)]
pub struct Dispatch;
pub(super) fn install(app:&mut App){
    app.init_resource::<NormalBarter>().add_systems(Update,
        dispatch.in_set(Dispatch).before(super::MenuSystems).before(crate::tiles::input::feed)
            .run_if(resource_exists::<ActiveMenu>));
}
