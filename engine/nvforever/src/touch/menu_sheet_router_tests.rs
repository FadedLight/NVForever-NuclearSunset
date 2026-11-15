// Actual parent reader and deferred menu-sheet replay. No second raw reader.
mod menu_sheet_router_controls {
    use super::*;
    use nvf_play::{menu::{self, barter_touch, model::{Command, MenuModel}}, ui_pointer::{self, PointerState}};

    #[derive(Resource, Default)]
    struct Seen { envelopes: usize, underlay_presses: usize }
    #[derive(Component)]
    struct Underlay;
    fn observe(mut input: MessageReader<menu::MenuInput>,
        buttons: Query<&PointerState, (With<Underlay>, Changed<PointerState>)>, mut seen: ResMut<Seen>) {
        seen.envelopes += input.read().filter(|input| matches!(input.0, Command::CommitBarter(_))).count();
        seen.underlay_presses += buttons.iter().filter(|state| **state == PointerState::Pressed).count();
    }
    fn authored_barter() -> (nvf_play::sim::SimState, MenuModel) {
        use fnv_esm::{raw::{Subrecord, write_subrecords}, LoadOrder, Plugin, Schema};
        use fnv_sim::{FormId, FormStore, RefTable, SimConfig};
        fn sub(sig: &[u8;4], data: Vec<u8>) -> Subrecord { Subrecord { sig:*sig, data, extended:false } }
        fn z(text:&str)->Vec<u8>{[text.as_bytes(),&[0]].concat()}
        fn record(sig:&[u8;4], id:u32, fields:Vec<Subrecord>)->Vec<u8>{
            let data=write_subrecords(&fields);
            [&sig[..],&(data.len() as u32).to_le_bytes(),&[0;4],&id.to_le_bytes(),&[0;4],&15u16.to_le_bytes(),&[0;2],&data].concat()
        }
        fn top(sig:&[u8;4], body:Vec<u8>)->Vec<u8>{
            [b"GRUP".as_slice(),&((24+body.len()) as u32).to_le_bytes(),sig,&0i32.to_le_bytes(),&[0;8],&body].concat()
        }
        let misc=|(id,name,value):(u32,&str,i32)|record(b"MISC",id,vec![sub(b"FULL",z(name)),
            sub(b"DATA",[value.to_le_bytes(),1f32.to_le_bytes()].concat())]);
        let coins=FormId(0xf);let item=FormId(0xfa20);let stock_base=FormId(0xfa21);
        let bytes=[record(b"TES4",0,vec![sub(b"HEDR",[1.34f32.to_le_bytes(),0i32.to_le_bytes(),0x10000u32.to_le_bytes()].concat())]),
            top(b"MISC",[misc((coins.0,"Coins",1)),misc((item.0,"Authored supplies",10))].concat()),
            top(b"CONT",record(b"CONT",stock_base.0,vec![sub(b"FULL",z("Authored merchant")),sub(b"DATA",[&[0u8][..],&0f32.to_le_bytes()].concat())])),
            top(b"NPC_",record(b"NPC_",7,vec![sub(b"DATA",[&100i32.to_le_bytes()[..],&[5;7]].concat())]))].concat();
        let plugins=[Plugin::parse(&bytes,"Router.esm".into()).unwrap()];
        let order=LoadOrder::new(&plugins).unwrap();
        let schema=Schema::load_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/fnv")).unwrap();
        let forms=Box::leak(Box::new(FormStore::build(&order,&schema)));
        assert!(forms.issues.is_empty(),"{:?}",forms.issues.to_json());
        let refs=Box::leak(Box::new(RefTable::build(&order,&schema)));
        let mut world=fnv_sim::World::new(forms,refs,SimConfig::default());
        let stock=world.place(stock_base,None,[0.;3],[0.;3],1).unwrap();
        for holder in [FormId::PLAYER_REF,stock] {world.add_item(holder,coins,1000).unwrap();world.add_item(holder,item,8).unwrap();}
        let m=MenuModel::open_barter(&mut world,stock,stock,coins,100.,0).unwrap();
        (nvf_play::sim::SimState {forms,refs,world},m)
    }
    fn scene_with_sheet(dialogue:bool)->(App,Entity,Entity) {
        let (mut app,window)=default_menu_touch_scene(nvf_play::pipboy::Page::Items,false);
        let camera=app.world().resource::<nvf_play::pipboy::PipBoyScreen>().camera;
        app.world_mut().resource_mut::<nvf_play::pipboy::PipBoy>().phase=nvf_play::pipboy::Phase::Down;
        let (sim,m)=authored_barter();app.insert_resource(sim).insert_resource(menu::ActiveMenu(Some(m)));
        barter_touch::install(&mut app);
        let row=app.world().resource::<nvf_play::settings::Settings>().index(barter_touch::KEY).unwrap();
        if !(app.world().resource::<nvf_play::settings::Settings>().list()[row].get)(app.world()) {
            assert_eq!(nvf_play::settings::flip(app.world_mut(),row),Some(true));
        }
        app.insert_resource(nvf_play::dialogue_touch::TouchDialogue {enabled:true,..default()});
        app.world_mut().resource_mut::<NextState<nvf_play::PlayState>>().set(if dialogue {nvf_play::PlayState::Dialogue}else{nvf_play::PlayState::Menu});
        app.init_resource::<Seen>().init_resource::<bevy::ui::UiStack>().add_plugins(ui_pointer::UiPointerPlugin)
            .add_systems(Update,observe.after(barter_touch::Dispatch));
        for _ in 0..5 {app.update();}
        assert!(app.world().resource::<barter_touch::TouchBarter>().visible());
        assert_eq!(*app.world().resource::<State<nvf_play::PlayState>>().get(),if dialogue {nvf_play::PlayState::Dialogue}else{nvf_play::PlayState::Menu});
        (app,window,camera)
    }
    fn painted_footer(app:&mut App)->Vec2 {
        let world=app.world_mut();
        let mut q=world.query::<(Entity,&Text)>();
        let label=q.iter(world).find_map(|(entity,text)|(text.0=="Choose amount").then_some(entity)).expect("real alternate sheet label");
        let tile=world.get::<ChildOf>(label).unwrap().parent();
        let node=world.get::<ComputedNode>(tile).unwrap();
        assert!(node.size().min_element()>=48.,"actual laid-out footer has usable hit dimensions");
        let physical=world.get::<bevy::ui::UiGlobalTransform>(tile).unwrap().translation;
        let mut windows=world.query_filtered::<&Window,With<PrimaryWindow>>();
        physical/windows.single(world).unwrap().scale_factor()
    }
    fn underlay(app:&mut App,camera:Entity,point:Vec2)->Entity {
        let button=app.world_mut().spawn((Underlay,ui_pointer::Button,UiTargetCamera(camera),
            Node {position_type:PositionType::Absolute,left:px(point.x-24.),top:px(point.y-24.),width:px(48.),height:px(48.),..default()})).id();
        for _ in 0..3 {app.update();}
        assert!(app.world().get::<ComputedNode>(button).unwrap().size().min_element()>=48.);
        let mut stack=app.world_mut().resource_mut::<bevy::ui::UiStack>();
        // Deliberately leave the real underlay focus target in the real UI stack:
        // ownership must suppress its touch, not rely on an opaque sheet's z-order.
        stack.uinodes=vec![button];stack.partition.clear();stack.partition.push(0..1);
        button
    }
    fn raw_tap(app:&mut App,window:Entity,id:u64,point:Vec2){
        touch(app,window,id,TouchPhase::Started,point.x,point.y);app.update();
        touch(app,window,id,TouchPhase::Ended,point.x,point.y);app.update();
    }
    #[test]
    fn sole_raw_sheet_router_barter_in_dialogue_precedes_dialogue_and_real_underlay_focus() {
        let (mut app,window,camera)=scene_with_sheet(true);let point=painted_footer(&mut app);
        let button=underlay(&mut app,camera,point);
        assert!(app.world().resource::<modal::Capture>().blocking(),"real Dialogue capture is active underneath barter");
        touch(&mut app,window,4100,TouchPhase::Started,point.x,point.y);app.update();
        assert!(!app.world().resource::<nvf_play::ui_pointer::TouchUiCapture>().allows(4100));
        assert!(app.world().resource::<TouchState>().contacts.is_empty());
        assert!(app.world().resource::<TouchState>().ui_capture);
        assert!(!app.world().resource::<nvf_play::control::HeldSneakInput>().active);
        assert!(!app.world().resource::<nvf_play::pipboy::touch_items::TouchItems>().owns(4100));
        assert_ne!(*app.world().get::<PointerState>(button).unwrap(),PointerState::Pressed);
        touch(&mut app,window,4100,TouchPhase::Ended,point.x,point.y);app.update();
        for _ in 0..4 {app.update();}
        assert_eq!(app.world().resource::<Seen>().envelopes,1,"one real guarded MenuInput reaches the existing consumer boundary");
        assert_eq!(app.world().resource::<Seen>().underlay_presses,0);
        let generic=app.world().resource::<DefaultMenuTouchSeen>();
        assert_eq!((generic.presses,generic.releases,generic.scrolls),(0,0,0));
        assert!(!app.world().resource::<ButtonInput<KeyCode>>().pressed(KeyCode::Enter));
    }
    #[test]
    fn sole_raw_sheet_router_keeps_ordinary_unowned_window_button_touch_enabled() {
        let (mut app,window,camera)=scene_with_sheet(false);let point=painted_footer(&mut app);
        app.world_mut().resource_mut::<menu::ActiveMenu>().0=None;
        for _ in 0..5 {app.update();}
        assert!(!app.world().resource::<barter_touch::TouchBarter>().visible());
        let button=underlay(&mut app,camera,point);
        touch(&mut app,window,4101,TouchPhase::Started,point.x,point.y);app.update();
        assert!(app.world().resource::<nvf_play::ui_pointer::TouchUiCapture>().allows(4101));
        assert_eq!(*app.world().get::<PointerState>(button).unwrap(),PointerState::Pressed);
        touch(&mut app,window,4101,TouchPhase::Ended,point.x,point.y);app.update();
        assert_eq!(app.world().resource::<Seen>().underlay_presses,1);
        assert_eq!(app.world().resource::<Seen>().envelopes,0);
    }
    #[test]
    fn sole_raw_sheet_router_edge_overflow_drops_whole_batch_then_neutral_accepts_fresh_contact() {
        let (mut app,window,_)=scene_with_sheet(false);let point=painted_footer(&mut app);
        touch(&mut app,window,4102,TouchPhase::Started,point.x,point.y);
        for _ in 0..129 {touch(&mut app,window,4102,TouchPhase::Moved,point.x,point.y);}
        app.update();
        assert_eq!(app.world().resource::<Seen>().envelopes,0);
        assert!(!app.world().resource::<nvf_play::ui_pointer::TouchUiCapture>().allows(4102));
        touch(&mut app,window,4102,TouchPhase::Ended,point.x,point.y);app.update();
        assert_eq!(app.world().resource::<Seen>().envelopes,0);
        assert!(!app.world().resource::<nvf_play::ui_pointer::TouchUiCapture>().allows(4102),"release frame remains quarantined");
        for _ in 0..6 {app.update();}
        assert!(app.world().resource::<barter_touch::TouchBarter>().visible());
        let point=painted_footer(&mut app);raw_tap(&mut app,window,4103,point);
        assert_eq!(app.world().resource::<Seen>().envelopes,1,"fresh contact after actual neutral recovers");
        assert!(app.world().resource::<TouchState>().contacts.is_empty());
        assert!(!app.world().resource::<nvf_play::pipboy::touch_items::TouchItems>().busy());
    }
}
