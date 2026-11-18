//! Optional, locally supplied casing atlas. Live scene/text/actions belong to the
//! dialogue presentation, never to pixels captured in the supplied picture.
use bevy::{image::ImageSampler,prelude::*,render::render_resource::TextureFormat};
use nvf_assets::GameData;

pub(crate) const ASSET:&str="Textures/Interface/NVForever/Dialogue/classic.png";
const WIDTH:u32=1447;
const HEIGHT:u32=1087;

/// Bounded black openings in the supplied blank template. The connected mask
/// follows the actual rounded pixel boundary; it does not cut a rectangular hole
/// through the metal corners. The source bytes remain untouched on disk.
const CLEAR:[Rect;4]=[
    Rect::new(38.,231.,587.,813.),
    Rect::new(631.,235.,1409.,393.),
    Rect::new(633.,440.,1409.,1020.),
    Rect::new(38.,920.,130.,982.),
];
const SEEDS:[UVec2;4]=[UVec2::new(300,500),UVec2::new(1000,310),
    UVec2::new(1000,700),UVec2::new(80,950)];

/// Asset preparation only; Loaded does not assert a rendered frame or working input.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub(crate) enum Status<'a> {WaitingForData,Missing,Invalid(&'a str),Loaded}

#[derive(Resource,Default)]
pub(crate) struct Cache {
    attempted:bool,
    error:Option<String>,
    art:Option<LoadedArt>,
}
impl Cache {
    /// Retry while GameData is absent; once it exists, read/decode at most once.
    /// A presentation/data reload can explicitly replace this cache with Default.
    pub(crate) fn prepare(&mut self,data:Option<&GameData>,images:&mut Assets<Image>)->Status<'_> {
        if !self.attempted {
            let Some(data)=data else{return Status::WaitingForData;};
            self.attempted=true;
            if let Some(bytes)=data.read(ASSET) {
                match decode(&bytes) {
                    Ok(image)=>self.art=Some(LoadedArt {image:images.add(image)}),
                    Err(error)=>{warn!("classic dialogue casing: {error}");self.error=Some(error);}
                }
            } else {warn!("Classic dialogue casing is unavailable; using the normal dialogue view");}
        }
        self.status()
    }
    pub(crate) fn status(&self)->Status<'_> {
        if self.art.is_some(){Status::Loaded}
        else if let Some(error)=self.error.as_deref(){Status::Invalid(error)}
        else if self.attempted {Status::Missing}
        else {Status::WaitingForData}
    }
    pub(crate) fn loaded(&self)->Option<&LoadedArt> {self.art.as_ref()}
}

pub(crate) fn install(app:&mut App) {app.init_resource::<Cache>();}

fn decode(bytes:&[u8])->Result<Image,String> {
    let mut decoder=png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader=decoder.read_info().map_err(|e|e.to_string())?;
    // This atlas's source rectangles are calibrated to this one layout. Reject a
    // different picture before allocating its decoded pixel buffer.
    if reader.info().width!=WIDTH || reader.info().height!=HEIGHT {
        return Err(format!("expected {WIDTH}x{HEIGHT} casing atlas, got {}x{}",reader.info().width,reader.info().height));
    }
    let size=reader.output_buffer_size().ok_or("PNG output size is unavailable")?;
    if size>(WIDTH as usize*HEIGHT as usize*4) {return Err("casing PNG is not an 8-bit RGB/RGBA image".into());}
    let mut pixels=vec![0;size];
    let info=reader.next_frame(&mut pixels).map_err(|e|e.to_string())?;
    pixels.truncate(info.buffer_size());
    let mut rgba=match (info.color_type,info.bit_depth) {
        (png::ColorType::Rgba,png::BitDepth::Eight)=>pixels,
        (png::ColorType::Rgb,png::BitDepth::Eight)=>pixels.as_chunks::<3>().0.iter().flat_map(|p|[p[0],p[1],p[2],255]).collect(),
        _=>return Err("casing PNG is not an 8-bit RGB/RGBA image".into()),
    };
    for (bounds,seed) in CLEAR.into_iter().zip(SEEDS) {
        if clear_opening(&mut rgba,bounds,seed)<16 {return Err("casing template has no expected clear opening".into());}
    }
    let mut image=nvf_assets::texture::checked_image(WIDTH,HEIGHT,1,TextureFormat::Rgba8UnormSrgb,rgba)?;
    image.sampler=ImageSampler::linear();
    image.texture_descriptor.label=Some("supplied classic dialogue casing");
    Ok(image)
}

// Four-neighbour fill stays inside each permitted aperture and clears only its
// connected black/transparent pixels. Dark metal elsewhere is never selected.
fn clear_opening(rgba:&mut [u8],bounds:Rect,seed:UVec2)->usize {
    let mut pending=vec![seed];let mut seen=vec![false;(WIDTH*HEIGHT) as usize];let mut count=0;
    while let Some(p)=pending.pop() {
        if p.x>=WIDTH || p.y>=HEIGHT || !bounds.contains(p.as_vec2()+Vec2::splat(0.5)) {continue;}
        let index=(p.y*WIDTH+p.x) as usize;
        if std::mem::replace(&mut seen[index],true) {continue;}
        let pixel=&mut rgba[index*4..index*4+4];
        if pixel[3]!=0 && pixel[..3].iter().any(|v|*v>8) {continue;}
        pixel.fill(0);count+=1;
        if p.x>0 {pending.push(UVec2::new(p.x-1,p.y));}
        if p.y>0 {pending.push(UVec2::new(p.x,p.y-1));}
        if p.x+1<WIDTH {pending.push(UVec2::new(p.x+1,p.y));}
        if p.y+1<HEIGHT {pending.push(UVec2::new(p.x,p.y+1));}
    }
    count
}

#[derive(Clone,Copy,Debug,PartialEq)]
struct Part {source:Rect,destination:Rect}
impl Part {
    fn map(&self,r:Rect)->Rect {
        let scale=self.destination.size()/self.source.size();
        Rect::from_corners(self.destination.min+(r.min-self.source.min)*scale,
            self.destination.min+(r.max-self.source.min)*scale)
    }
}
/// Geometry is window-logical relative to a safe origin. Each hardware band
/// samples the supplied art, while the ordinary camera and dialogue own the holes.
#[derive(Clone,Copy,Debug,PartialEq)]
pub(crate) struct Layout {
    pub(crate) size:Vec2,
    pub(crate) world:Rect,
    pub(crate) speech:Rect,
    pub(crate) responses:Rect,
    pub(crate) caps:Rect,
    pub(crate) wells:[Rect;3],
    pub(crate) red_button:Rect,
    pub(crate) barter:Rect,
    parts:[Part;8],
}
impl Layout {
    pub(crate) fn new(size:Vec2,caps_width:f32)->Option<Self> {
        if !size.is_finite() || size.min_element()<=0. || !caps_width.is_finite() || caps_width<1. {return None;}
        let header=48.0_f32.min(size.y*0.15);
        let controls=90.0;
        // Reflow the existing caps bezel, not its text: the full integer at the
        // real font's readable minimum must fit without touching Review/Barter.
        let cash=caps_width*154.0/72.0;
        let left=(size.x*610.0/WIDTH as f32).max(cash+72.0+80.0);
        let barter_left=left-80.0;
        let npc_band=48.0*207.0/127.0;
        let parts=[
            Part {source:Rect::new(0.,0.,610.,209.),destination:Rect::new(0.,0.,left,header)},
            Part {source:Rect::new(610.,0.,1447.,209.),destination:Rect::new(left,0.,size.x,header)},
            Part {source:Rect::new(0.,209.,610.,854.),destination:Rect::new(0.,header,left,size.y-controls)},
            Part {source:Rect::new(0.,854.,154.,1087.),destination:Rect::new(0.,size.y-controls,cash,size.y)},
            Part {source:Rect::new(154.,854.,428.,1087.),destination:Rect::new(cash,size.y-controls,barter_left,size.y)},
            Part {source:Rect::new(428.,854.,610.,1087.),destination:Rect::new(barter_left,size.y-controls,left,size.y)},
            Part {source:Rect::new(610.,209.,1447.,416.),destination:Rect::new(left,header,size.x,header+npc_band)},
            Part {source:Rect::new(610.,416.,1447.,1087.),destination:Rect::new(left,header+npc_band,size.x,size.y)},
        ];
        let world=parts[2].map(CLEAR[0]);
        let speech=parts[6].map(Rect::new(665.,249.,1373.,376.));
        let responses=parts[7].map(Rect::new(666.,466.,1376.,994.));
        let caps=parts[3].map(Rect::new(47.,929.,119.,972.));
        let red=parts[5].map(Rect::new(469.,950.,507.,982.));
        let barter=Rect::from_center_size(red.center(),Vec2::splat(48.));
        let wells=[parts[6].map(CLEAR[1]),parts[7].map(CLEAR[2]),parts[3].map(CLEAR[3])];
        let valid=|r:Rect|r.min.cmpge(Vec2::ZERO).all() && r.max.cmple(size).all();
        if [world,speech,responses,caps,barter].into_iter().any(|r|!valid(r))
            || world.size().min_element()<48. || speech.width()<48. || speech.height()<47.99
            || responses.width()<160. || responses.height()<96. || caps.width()<caps_width-0.01 || caps.height()<14.4 || barter.intersect(caps).size().min_element()>0.
            || [world,speech,responses].into_iter().any(|r|barter.intersect(r).size().min_element()>0.)
            || size.x<480. {return None;}
        Some(Self {size,world,speech,responses,caps,wells,red_button:red,barter,parts})
    }
}

pub(crate) struct LoadedArt {image:Handle<Image>}
impl LoadedArt {
    pub(crate) fn image_id(&self)->bevy::asset::AssetId<Image> {self.image.id()}
    /// Caller places the parent at the safe-canvas origin. Divide by Bevy UiScale
    /// exactly once; DPI already belongs to the camera's normal UI provider.
    /// The real housing presenter uses opacity 1.0; legacy frame opacity belongs
    /// only to the separately configurable Field Notes treatment.
    pub(crate) fn spawn(&self,parent:&mut ChildSpawnerCommands,layout:&Layout,ui_scale:f32,opacity:f32)->bool {
        if !ui_scale.is_finite() || ui_scale<=0. || !opacity.is_finite() {return false;}
        if layout.parts.iter().any(|p|!(p.destination.min/ui_scale).is_finite()
            || !(p.destination.max/ui_scale).is_finite()) {return false;}
        for part in &layout.parts {
            let r=part.destination;
            parent.spawn((
                Node {position_type:PositionType::Absolute,left:px(r.min.x/ui_scale),top:px(r.min.y/ui_scale),
                    width:px(r.width()/ui_scale),height:px(r.height()/ui_scale),..default()},
                ImageNode {image:self.image.clone(),rect:Some(part.source),color:Color::WHITE.with_alpha(opacity.clamp(0.,1.)),
                    image_mode:NodeImageMode::Stretch,..default()},
                Pickable::IGNORE,
            ));
        }
        true
    }
}


#[cfg(test)]
pub(super) mod tests {
    use super::*;
    pub(crate) fn authored_png()->Vec<u8> {
        let mut bytes=Vec::new();
        {
            let mut encoder=png::Encoder::new(&mut bytes,WIDTH,HEIGHT);
            encoder.set_color(png::ColorType::Rgba);encoder.set_depth(png::BitDepth::Eight);
            let mut writer=encoder.write_header().unwrap();
            let pixels:Vec<_>=(0..WIDTH*HEIGHT).flat_map(|index| {
                let p=Vec2::new((index%WIDTH) as f32+0.5,(index/WIDTH) as f32+0.5);
                if CLEAR.iter().any(|r|r.contains(p)) {[0,0,0,255]}else{[37,51,73,255]}
            }).collect();
            writer.write_image_data(&pixels).unwrap();
        }
        bytes
    }
    pub(crate) struct Files(pub std::path::PathBuf);
    impl Drop for Files {fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}
    pub(crate) fn files(with_font:bool)->(Files,GameData) {
        let root=std::env::temp_dir().join(format!("nvf-classic-presenter-{}-{}",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let files=Files(root);let path=files.0.join(ASSET);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();std::fs::write(path,authored_png()).unwrap();
        if with_font {
            // Independently authored bitmap font, through the real .fnt/.tex reader.
            let name=crate::interface::font_file(crate::interface::FONT_FILES[4]).replace('\\',"/");
            assert!(!std::path::Path::new(&name).is_absolute() && !name.split('/').any(|p|p==".."));
            let path=files.0.join(name);std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let mut bytes=vec![0;fnv_font::FILE_SIZE];
            bytes[..4].copy_from_slice(&25.0_f32.to_le_bytes());bytes[4..8].copy_from_slice(&1_u32.to_le_bytes());
            bytes[12..26].copy_from_slice(b"ClassicFixture");
            for code in 0..256 {
                let mut put=|slot:usize,v:f32| {
                    let at=296+code*56+slot*4;bytes[at..at+4].copy_from_slice(&v.to_le_bytes());
                };
                if code==32 {put(12,8.0);}else if (33..127).contains(&code) {
                    for (slot,value) in [(1,0.1),(2,0.1),(7,0.9),(8,0.9),(9,12.0),(10,20.0),(12,1.0),(13,20.0)] {put(slot,value);}
                }
            }
            std::fs::write(path,bytes).unwrap();
            let texture=files.0.join("textures/fonts/classicfixture.tex");std::fs::create_dir_all(texture.parent().unwrap()).unwrap();
            let mut pixels=8_u32.to_le_bytes().to_vec();pixels.extend(8_u32.to_le_bytes());pixels.extend([255;8*8*4]);
            std::fs::write(texture,pixels).unwrap();
        }
        let data=GameData {vfs:std::sync::Arc::new(fnv_vfs::Vfs::open(&files.0,&[]).unwrap()),
            schema:std::sync::Arc::new(fnv_nif::Schema::load_dir(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"),"/../../schemas/nif"))).unwrap())};
        (files,data)
    }
    #[test]
    fn supplied_image_decoder_removes_captured_content_and_keeps_hardware_pixels() {
        let image=decode(&authored_png()).unwrap();assert_eq!(image.size(),UVec2::new(WIDTH,HEIGHT));
        let bytes=image.data.as_ref().unwrap();
        for seed in SEEDS {let at=((seed.y*WIDTH+seed.x)*4) as usize;assert_eq!(&bytes[at..at+4],&[0,0,0,0]);}
        for (x,y) in [(100,100),(1200,100),(600,900),(400,1000)] {
            let at=((y*WIDTH+x)*4) as usize;assert_eq!(&bytes[at..at+4],&[37,51,73,255]);
        }
        assert!(decode(b"not a PNG").is_err());
        let mut short=Vec::new();{
            let mut encoder=png::Encoder::new(&mut short,1,1);encoder.set_color(png::ColorType::Rgba);
            encoder.write_header().unwrap().write_image_data(&[0;4]).unwrap();
        }
        assert!(decode(&short).is_err(),"a different atlas cannot use these source apertures");
    }
    #[test]
    fn runtime_vfs_asset_is_loaded_once_and_missing_data_never_fakes_presence() {
        let (files,data)=files(false);let mut cache=Cache::default();let mut images=Assets::<Image>::default();
        assert_eq!(cache.prepare(None,&mut images),Status::WaitingForData);assert!(cache.loaded().is_none());
        assert_eq!(cache.prepare(Some(&data),&mut images),Status::Loaded);
        let image=cache.loaded().unwrap().image.id();assert_eq!(images.len(),1);
        std::fs::remove_file(files.0.join(ASSET)).unwrap();
        assert_eq!(cache.prepare(Some(&data),&mut images),Status::Loaded);
        assert_eq!(cache.loaded().unwrap().image.id(),image);assert_eq!(images.len(),1,"no per-frame decode");
        let missing_root=files.0.join("missing");std::fs::create_dir(&missing_root).unwrap();
        let missing_data=GameData {vfs:std::sync::Arc::new(fnv_vfs::Vfs::open(&missing_root,&[]).unwrap()),schema:data.schema.clone()};
        let mut missing=Cache::default();assert_eq!(missing.prepare(Some(&missing_data),&mut images),Status::Missing);
        assert!(missing.loaded().is_none());
    }
    #[derive(Resource)]
    struct Fixture {art:LoadedArt,layout:Layout,scale:f32}
    fn paint(mut commands:Commands,fixture:Res<Fixture>) {
        commands.spawn(Node::default()).with_children(|p|assert!(fixture.art.spawn(p,&fixture.layout,fixture.scale,1.0)));
    }
    #[test]
    fn reflow_preserves_content_openings_and_spawns_only_passive_atlas_parts() {
        for width in [640.,800.] {for scale in [0.75,1.,1.5] {
            let layout=Layout::new(Vec2::new(width,360.),96.).unwrap();
            assert!(layout.world.height()>=48. && layout.responses.height()>=48.);
            assert_eq!(layout.barter.size(),Vec2::splat(48.));
            assert!(Layout::new(Vec2::new(width,120.),96.).is_none(),"no hidden target shrinking");
            let mut app=App::new();let mut images=Assets::<Image>::default();
            let art=LoadedArt {image:images.add(decode(&authored_png()).unwrap())};
            app.insert_resource(Fixture {art,layout:layout.clone(),scale}).add_systems(Update,paint);app.update();
            let mut pieces=app.world_mut().query::<(&Node,&ImageNode,&Pickable)>();let mut count=0;
            for (node,image,picking) in pieces.iter(app.world()) {
                count+=1;assert_eq!(*picking,Pickable::IGNORE);assert_eq!(image.color,Color::WHITE);
                let (Val::Px(x),Val::Px(y),Val::Px(w),Val::Px(h))=(node.left,node.top,node.width,node.height) else{panic!("actual positioned atlas piece");};
                assert!(x>=0. && y>=0. && w>0. && h>0.);
                assert!((x+w)*scale<=width+0.01 && (y+h)*scale<=360.01);
                let uv=image.rect.unwrap();assert!(uv.min.cmpge(Vec2::ZERO).all());
                assert!(uv.max.cmple(Vec2::new(WIDTH as f32,HEIGHT as f32)).all());
            }
            assert_eq!(count,8);
        }}
    }
}
