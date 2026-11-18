//! Bundled casing atlas with a local override. Live scene/text/actions belong to the
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
pub(crate) enum Status<'a> {WaitingForData,Invalid(&'a str),Loaded}

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
            let image = match data.vfs.read(ASSET) {
                Some(Ok(bytes)) => decode(&bytes),
                Some(Err(error)) => Err(error.to_string()),
                None => decode_bundled(),
            };
            match image {
                Ok(image) => self.art = Some(LoadedArt { image: images.add(image) }),
                Err(error) => {
                    warn!("classic dialogue casing: {error}");
                    self.error = Some(error);
                }
            }
        }
        self.status()
    }
    pub(crate) fn status(&self)->Status<'_> {
        if self.art.is_some(){Status::Loaded}
        else if let Some(error)=self.error.as_deref(){Status::Invalid(error)}
        else {Status::WaitingForData}
    }
    pub(crate) fn loaded(&self)->Option<&LoadedArt> {self.art.as_ref()}
}

pub(crate) fn install(app:&mut App) {app.init_resource::<Cache>();}

#[cfg(not(target_os = "android"))]
fn decode_bundled() -> Result<Image, String> {
    decode(include_bytes!("../../assets/dialogue/classic.png"))
}

#[cfg(target_os = "android")]
fn decode_bundled() -> Result<Image, String> {
    let app = bevy::android::ANDROID_APP.get().ok_or("Android activity is unavailable")?;
    let mut asset = app.asset_manager().open(c"dialogue/classic.png")
        .ok_or("Bundled Classic dialogue casing is missing from the APK")?;
    decode(asset.buffer().map_err(|error| error.to_string())?)
}

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

/// One atlas crop and where it is drawn, in safe-canvas logical units.
#[derive(Clone,Copy,Debug,PartialEq)]
pub(crate) struct Part {pub(crate) source:Rect,pub(crate) destination:Rect}

// Atlas breakpoints. Segments between them are drawn at one uniform scale, so
// tubes, grille, speaker, lights, caps bezel, Review and Barter keep their native
// aspect. Only the listed flexible segments stretch: the two plain dark gaps
// beside the header tubes, the straight bars and interiors of the world, caps,
// speech and response openings. Bands meet in the dark gaps at y=194 and x=610.
const HEADER_X:[f32;6]=[0.,140.,150.,392.,408.,1447.];
const HEADER_Y:[f32;2]=[0.,194.];
const WORLD_X:[f32;4]=[0.,110.,520.,610.];
const WORLD_Y:[f32;4]=[194.,300.,745.,854.];
const CONTROLS_X:[f32;4]=[0.,60.,108.,610.];
const CONTROLS_Y:[f32;2]=[854.,1087.];
const RIGHT_X:[f32;4]=[610.,690.,1350.,1447.];
const RIGHT_Y:[f32;6]=[194.,262.,366.,490.,970.,1087.];
const SPEECH_TEXT:Rect=Rect{min:Vec2::new(665.,249.),max:Vec2::new(1373.,376.)};
const RESPONSES_TEXT:Rect=Rect{min:Vec2::new(666.,466.),max:Vec2::new(1376.,994.)};
/// The counter's opening less a small bezel margin; glyphs are sized from its height.
const CAPS_TEXT:Rect=Rect{min:Vec2::new(42.,924.),max:Vec2::new(126.,978.)};
const RED:Rect=Rect{min:Vec2::new(469.,950.),max:Vec2::new(507.,982.)};
/// [design] The smallest readable caps glyph, in logical units.
pub(crate) const CAPS_MIN:f32=12.0;
/// Source pixels at the art's outer edge repeated under a safe-area inset.
const EDGE:f32=4.0;

/// Piecewise-linear placement of atlas breakpoints along one axis.
#[derive(Clone,Copy,Debug,PartialEq)]
struct Axis<const N:usize> {src:[f32;N],dst:[f32;N]}
impl<const N:usize> Axis<N> {
    /// Native length at `scale`, except `flex` (segment index, length) entries.
    fn place(src:[f32;N],start:f32,scale:f32,flex:&[(usize,f32)])->Option<Self> {
        let mut dst=[start;N];
        for i in 1..N {
            let length=flex.iter().find(|(j,_)|*j==i-1).map_or((src[i]-src[i-1])*scale,|(_,l)|*l);
            if !length.is_finite() || length<0. {return None;}
            dst[i]=dst[i-1]+length;
        }
        Some(Self {src,dst})
    }
    fn map(&self,v:f32)->f32 {
        let i=(1..N).find(|&i|v<=self.src[i]).unwrap_or(N-1);
        let (a,b,c,d)=(self.src[i-1],self.src[i],self.dst[i-1],self.dst[i]);
        c+(v-a)*(d-c)/(b-a)
    }
    fn segments(&self)->impl Iterator<Item=((f32,f32),(f32,f32))>+'_ {
        (1..N).map(|i|((self.src[i-1],self.src[i]),(self.dst[i-1],self.dst[i])))
    }
}
#[derive(Clone,Copy,Debug,PartialEq)]
struct Band<const X:usize,const Y:usize> {x:Axis<X>,y:Axis<Y>}
impl<const X:usize,const Y:usize> Band<X,Y> {
    fn map(&self,r:Rect)->Rect {
        Rect::new(self.x.map(r.min.x),self.y.map(r.min.y),self.x.map(r.max.x),self.y.map(r.max.y))
    }
    fn parts(&self)->impl Iterator<Item=Part>+'_ {
        self.y.segments().flat_map(move|((sy0,sy1),(dy0,dy1))| self.x.segments().map(move|((sx0,sx1),(dx0,dx1))|
            Part {source:Rect::new(sx0,sy0,sx1,sy1),destination:Rect::new(dx0,dy0,dx1,dy1)}))
            .filter(|p|p.destination.size().min_element()>0.)
    }
}

/// Caps text metrics per unit of glyph size, for the exact value drawn.
#[derive(Clone,Copy,Debug,PartialEq)]
pub(crate) struct CapsText {pub(crate) left:f32,pub(crate) width:f32,pub(crate) height:f32}
impl CapsText {
    /// Logical start of the first glyph inside the counter opening.
    pub(crate) fn left(&self,size:f32)->f32 {self.left*size+1.}
    /// Logical width the value needs, including its softened overhang and margin.
    pub(crate) fn needed(&self,size:f32)->f32 {self.width*size+4.}
}

/// Geometry is window-logical relative to a safe origin. Every hardware band
/// samples the supplied art at one scale; the ordinary camera and dialogue own the
/// holes. `bleed` repeats the outer edge under the safe-area insets so the art
/// reaches every window edge while text and targets stay inside the safe canvas.
#[derive(Clone,Copy,Debug,PartialEq)]
pub(crate) struct Layout {
    pub(crate) size:Vec2,
    pub(crate) scale:f32,
    pub(crate) world:Rect,
    pub(crate) speech:Rect,
    pub(crate) responses:Rect,
    pub(crate) caps:Rect,
    pub(crate) caps_size:f32,
    pub(crate) wells:[Rect;3],
    pub(crate) red_button:Rect,
    pub(crate) barter:Rect,
    bleed:[f32;4],
    header:Band<6,2>,
    world_band:Band<4,4>,
    controls:Band<4,2>,
    right:Band<4,6>,
}
impl Layout {
    /// `bleed` is the left, top, right and bottom safe-area inset around `size`.
    pub(crate) fn new(size:Vec2,bleed:[f32;4],caps:CapsText)->Option<Self> {
        if !size.is_finite() || size.min_element()<=0. || bleed.iter().any(|b|!b.is_finite() || *b<0.)
            || !caps.width.is_finite() || caps.width<=0. || !caps.height.is_finite() || caps.height<=0. {return None;}
        // One scale fits the whole atlas; the spare length goes to flexible segments.
        let s=(size.y/HEIGHT as f32).min(size.x/WIDTH as f32);
        let spare=Vec2::new(size.x-WIDTH as f32*s,size.y-HEIGHT as f32*s).max(Vec2::ZERO);
        // Glyphs follow the counter opening's height; a longer value widens the
        // opening (its plain interior) rather than shrinking or truncating digits.
        let caps_size=(CAPS_TEXT.height()*s)/caps.height;
        let caps_extra=(caps.needed(caps_size)-CAPS_TEXT.width()*s).max(0.);
        let gaps=(HEADER_X[2]-HEADER_X[1])+(HEADER_X[4]-HEADER_X[3]);
        let header=Band {
            x:Axis::place(HEADER_X,0.,s,&[(1,(HEADER_X[2]-HEADER_X[1])*(s+spare.x/gaps)),
                (3,(HEADER_X[4]-HEADER_X[3])*(s+spare.x/gaps))])?,
            y:Axis::place(HEADER_Y,0.,s,&[])?,
        };
        let top=header.y.dst[1];
        let world_band=Band {
            x:Axis::place(WORLD_X,0.,s,&[(1,(WORLD_X[2]-WORLD_X[1])*s+caps_extra)])?,
            y:Axis::place(WORLD_Y,top,s,&[(1,(WORLD_Y[2]-WORLD_Y[1])*s+spare.y)])?,
        };
        let controls=Band {
            x:Axis::place(CONTROLS_X,0.,s,&[(1,(CONTROLS_X[2]-CONTROLS_X[1])*s+caps_extra)])?,
            y:Axis::place(CONTROLS_Y,world_band.y.dst[3],s,&[])?,
        };
        let left=world_band.x.dst[3];
        let fixed_right=(RIGHT_X[1]-RIGHT_X[0]+RIGHT_X[3]-RIGHT_X[2])*s;
        // Speech keeps one 48-unit row; responses take the rest of the column.
        let fixed_column=(RIGHT_Y[1]-RIGHT_Y[0]+RIGHT_Y[3]-RIGHT_Y[2]+RIGHT_Y[5]-RIGHT_Y[4])*s;
        let speech_fixed=(RIGHT_Y[1]-SPEECH_TEXT.min.y+SPEECH_TEXT.max.y-RIGHT_Y[2])*s;
        let speech_open=((RIGHT_Y[2]-RIGHT_Y[1])*s+spare.y*0.25).max(48.-speech_fixed);
        let right=Band {
            x:Axis::place(RIGHT_X,left,s,&[(1,size.x-left-fixed_right)])?,
            y:Axis::place(RIGHT_Y,top,s,&[(1,speech_open),(3,size.y-top-fixed_column-speech_open)])?,
        };
        let world=world_band.map(CLEAR[0]);
        let speech=right.map(SPEECH_TEXT);
        let responses=right.map(RESPONSES_TEXT);
        let caps_rect=controls.map(CAPS_TEXT);
        let red=controls.map(RED);
        let barter=Rect::from_center_size(red.center(),Vec2::splat(48.));
        let wells=[right.map(CLEAR[1]),right.map(CLEAR[2]),controls.map(CLEAR[3])];
        let valid=|r:Rect|r.min.cmpge(Vec2::ZERO).all() && r.max.cmple(size+0.01).all();
        if [world,speech,responses,caps_rect,barter].into_iter().any(|r|!valid(r))
            || world.size().min_element()<48. || speech.width()<48. || speech.height()<47.99
            || responses.width()<160. || responses.height()<96. || caps_size<CAPS_MIN
            || caps_rect.width()<caps.needed(caps_size)-0.01 || barter.intersect(caps_rect).size().min_element()>0.
            || [world,speech,responses].into_iter().any(|r|barter.intersect(r).size().min_element()>0.) {return None;}
        Some(Self {size,scale:s,world,speech,responses,caps:caps_rect,caps_size,wells,red_button:red,barter,bleed,
            header,world_band,controls,right})
    }

    /// Every drawn crop: the banded casing, then its outer edge under each inset.
    pub(crate) fn parts(&self)->impl Iterator<Item=Part>+'_ {
        self.header.parts().chain(self.world_band.parts()).chain(self.controls.parts()).chain(self.right.parts())
            .flat_map(move|part|std::iter::once(part).chain(self.bleed_of(part).into_iter().flatten()))
    }

    fn bleed_of(&self,part:Part)->[Option<Part>;8] {
        let [left,top,right,bottom]=self.bleed;
        let (s,d)=(part.source,part.destination);
        let near=|a:f32,b:f32|(a-b).abs()<0.01;
        let x=[(left>0. && near(d.min.x,0.)).then_some((s.min.x,s.min.x+EDGE,-left,0.)),
            (right>0. && near(d.max.x,self.size.x)).then_some((s.max.x-EDGE,s.max.x,self.size.x,self.size.x+right))];
        let y=[(top>0. && near(d.min.y,0.)).then_some((s.min.y,s.min.y+EDGE,-top,0.)),
            (bottom>0. && near(d.max.y,self.size.y)).then_some((s.max.y-EDGE,s.max.y,self.size.y,self.size.y+bottom))];
        let mut out=[None;8];
        let mut n=0;
        let mut push=|source:Rect,destination:Rect| {out[n]=Some(Part {source,destination});n+=1;};
        for (sx0,sx1,dx0,dx1) in x.into_iter().flatten() {
            push(Rect::new(sx0,s.min.y,sx1,s.max.y),Rect::new(dx0,d.min.y,dx1,d.max.y));
            for (sy0,sy1,dy0,dy1) in y.into_iter().flatten() {
                push(Rect::new(sx0,sy0,sx1,sy1),Rect::new(dx0,dy0,dx1,dy1));
            }
        }
        for (sy0,sy1,dy0,dy1) in y.into_iter().flatten() {
            push(Rect::new(s.min.x,sy0,s.max.x,sy1),Rect::new(d.min.x,dy0,d.max.x,dy1));
        }
        out
    }
}

pub(crate) struct LoadedArt {image:Handle<Image>}
impl LoadedArt {
    pub(crate) fn image_id(&self)->bevy::asset::AssetId<Image> {self.image.id()}
    /// Caller places the parent at the safe-canvas origin; bleed crops extend past
    /// it to the window edges. Divide by Bevy UiScale exactly once; DPI already
    /// belongs to the camera's normal UI provider. The real housing presenter uses
    /// opacity 1.0; legacy frame opacity belongs only to Field Notes.
    pub(crate) fn spawn(&self,parent:&mut ChildSpawnerCommands,layout:&Layout,ui_scale:f32,opacity:f32)->bool {
        if !ui_scale.is_finite() || ui_scale<=0. || !opacity.is_finite() {return false;}
        if layout.parts().any(|p|!(p.destination.min/ui_scale).is_finite()
            || !(p.destination.max/ui_scale).is_finite()) {return false;}
        for part in layout.parts() {
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
        files_with_art(with_font, true)
    }
    pub(crate) fn files_with_art(with_font: bool, override_art: bool) -> (Files, GameData) {
        let root=std::env::temp_dir().join(format!("nvf-classic-presenter-{}-{}",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let files = Files(root);
        std::fs::create_dir_all(&files.0).unwrap();
        if override_art {
            let path = files.0.join(ASSET);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, authored_png()).unwrap();
        }
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
            // Use an atlas tile large enough to model the softened bezel glyphs at 12 units.
            let mut pixels=32_u32.to_le_bytes().to_vec();pixels.extend(32_u32.to_le_bytes());pixels.extend([255;32*32*4]);
            std::fs::write(&texture,pixels).unwrap();
            // Main admits TEX only when its companion DDS also exists.
            std::fs::write(texture.with_extension("dds"), []).unwrap();
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
    fn runtime_vfs_override_is_loaded_once_and_malformed_data_is_refused() {
        let (files,data)=files(false);let mut cache=Cache::default();let mut images=Assets::<Image>::default();
        assert_eq!(cache.prepare(None,&mut images),Status::WaitingForData);assert!(cache.loaded().is_none());
        assert_eq!(cache.prepare(Some(&data),&mut images),Status::Loaded);
        let image=cache.loaded().unwrap().image.id();assert_eq!(images.len(),1);
        std::fs::remove_file(files.0.join(ASSET)).unwrap();
        assert_eq!(cache.prepare(Some(&data),&mut images),Status::Loaded);
        assert_eq!(cache.loaded().unwrap().image.id(),image);assert_eq!(images.len(),1,"no per-frame decode");
        std::fs::write(files.0.join(ASSET), b"not a PNG").unwrap();
        let mut malformed = Cache::default();
        assert!(matches!(malformed.prepare(Some(&data), &mut images), Status::Invalid(_)));
        assert!(malformed.loaded().is_none(), "an invalid override is not silently replaced");
    }
    #[derive(Resource)]
    struct Fixture {art:LoadedArt,layout:Layout,scale:f32}
    fn paint(mut commands:Commands,fixture:Res<Fixture>) {
        commands.spawn(Node::default()).with_children(|p|assert!(fixture.art.spawn(p,&fixture.layout,fixture.scale,1.0)));
    }
    /// Font5-like metrics: a 0.52-em advance per digit and a 1.2-em cell.
    fn caps(chars:usize)->CapsText {CapsText {left:0.0,width:0.52*chars as f32,height:1.2}}

    #[test]
    fn reflow_preserves_content_openings_and_spawns_only_passive_atlas_parts() {
        for width in [640.,800.] {for scale in [0.75,1.,1.5] {
            let layout=Layout::new(Vec2::new(width,360.),[0.;4],caps(1)).unwrap();
            assert!(layout.world.height()>=48. && layout.responses.height()>=48.);
            assert_eq!(layout.barter.size(),Vec2::splat(48.));
            assert!(Layout::new(Vec2::new(width,120.),[0.;4],caps(1)).is_none(),"no hidden target shrinking");
            let mut app=App::new();let mut images=Assets::<Image>::default();
            let art=LoadedArt {image:images.add(decode(&authored_png()).unwrap())};
            app.insert_resource(Fixture {art,layout,scale}).add_systems(Update,paint);app.update();
            let mut pieces=app.world_mut().query::<(&Node,&ImageNode,&Pickable)>();let mut count=0;
            for (node,image,picking) in pieces.iter(app.world()) {
                count+=1;assert_eq!(*picking,Pickable::IGNORE);assert_eq!(image.color,Color::WHITE);
                let (Val::Px(x),Val::Px(y),Val::Px(w),Val::Px(h))=(node.left,node.top,node.width,node.height) else{panic!("actual positioned atlas piece");};
                assert!(x>=-0.01 && y>=-0.01 && w>0. && h>0.);
                assert!((x+w)*scale<=width+0.01 && (y+h)*scale<=360.01);
                let uv=image.rect.unwrap();assert!(uv.min.cmpge(Vec2::ZERO).all());
                assert!(uv.max.cmple(Vec2::new(WIDTH as f32,HEIGHT as f32)).all());
            }
            assert_eq!(count,layout.parts().count());
            assert_eq!(count,5+9+3+15,"header, world, controls and dialogue bands without insets");
        }}
    }

    /// Hardware that must keep its native aspect: corners and bars' fixed ends,
    /// the tubes, grille, small tubes, cables, speaker, lights, caps bezel ends,
    /// Review and Barter.
    const HARDWARE:[Rect;8]=[
        Rect{min:Vec2::new(0.,0.),max:Vec2::new(140.,194.)},Rect{min:Vec2::new(150.,0.),max:Vec2::new(392.,194.)},
        Rect{min:Vec2::new(408.,0.),max:Vec2::new(1447.,194.)},Rect{min:Vec2::new(0.,854.),max:Vec2::new(60.,1087.)},
        Rect{min:Vec2::new(108.,854.),max:Vec2::new(610.,1087.)},Rect{min:Vec2::new(0.,194.),max:Vec2::new(110.,300.)},
        Rect{min:Vec2::new(520.,745.),max:Vec2::new(610.,854.)},Rect{min:Vec2::new(1350.,194.),max:Vec2::new(1447.,262.)},
    ];

    #[test]
    fn classic_fills_every_aspect_without_borders_keeping_hardware_native_and_text_inside_openings() {
        // Logical windows: 20:9, 19.5:9, 16:9, 4:3, a phone and a tablet in portrait,
        // each with a cutout/status/navigation inset the art must still cover.
        let cases=[
            (Vec2::new(880.,396.),[34.,0.,34.,16.]),(Vec2::new(845.,390.),[0.,0.,32.,20.]),
            (Vec2::new(800.,450.),[0.;4]),(Vec2::new(1024.,768.),[0.,0.,0.,20.]),
            (Vec2::new(393.,873.),[0.,28.,0.,24.]),(Vec2::new(800.,1280.),[0.,24.,0.,0.]),
        ];
        let mut response_widths=Vec::new();
        for (window,[l,t,r,b]) in cases {
            let safe=window-Vec2::new(l+r,t+b);
            let layout=Layout::new(safe,[l,t,r,b],caps(5)).unwrap_or_else(||panic!("Classic admitted at {window}"));
            let parts:Vec<_>=layout.parts().collect();
            // Every point of the window, insets included, lies under a casing crop.
            let mut y=0.5;
            while y<window.y {
                let mut x=0.5;
                while x<window.x {
                    let p=Vec2::new(x,y)-Vec2::new(l,t);
                    assert!(parts.iter().any(|part|part.destination.contains(p)),"uncovered {x},{y} in {window}");
                    x+=2.0;
                }
                y+=2.0;
            }
            // Hardware inside the safe canvas is drawn at one uniform scale.
            for part in parts.iter().filter(|p|p.destination.min.cmpge(Vec2::splat(-0.01)).all()
                && p.destination.max.cmple(safe+0.01).all()) {
                if HARDWARE.iter().any(|h|h.contains(part.source.min) && h.contains(part.source.max)) {
                    let scale=part.destination.size()/part.source.size();
                    assert!((scale.x-layout.scale).abs()<0.001 && (scale.y-layout.scale).abs()<0.001,
                        "{:?} stretched to {scale} instead of {} at {window}",part.source,layout.scale);
                }
            }
            // Text, the counter and Barter stay inside the safe canvas and their openings.
            let inside=|r:Rect|r.min.cmpge(Vec2::splat(-0.01)).all() && r.max.cmple(safe+0.01).all();
            assert!([layout.world,layout.speech,layout.responses,layout.caps,layout.barter].into_iter().all(inside));
            for (text,well) in [(layout.speech,layout.wells[0]),(layout.responses,layout.wells[1]),(layout.caps,layout.wells[2])] {
                assert!(well.contains(text.min) && well.contains(text.max),"{text:?} outside {well:?} at {window}");
            }
            assert!(layout.speech.height()>=47.99 && layout.responses.height()>=96. && layout.responses.width()>=160.);
            response_widths.push((window.x/window.y,layout.responses.width()/safe.x));
        }
        // Wider screens give the extra width to the dialogue openings.
        assert!(response_widths[0].1>response_widths[2].1 && response_widths[2].1>response_widths[3].1);
    }
}
