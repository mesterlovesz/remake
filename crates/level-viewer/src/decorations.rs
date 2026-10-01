//! Retail decoration and atmosphere objects: rain (`d_emiter_opadu`) and glow sprites (`d_sprite`).
//!
//! Evidence (cshell.dll): rain create 0x100305c0, per-frame update 0x10031110 (called with the effect
//! list from the update dispatcher 0x100641b0), draw 0x10030d90, drop respawn 0x10030ae0. See
//! docs/retail-audio.md "Rain" for the constants. d_sprite: object.lto class 0x1000f620 (PreCreate builds
//! an engine sprite, InitialUpdate colours it, Update appends W_oku/Pionowy sprites to
//! scripts\cs\sprajty.txt and deletes the server object), cshell loader 0x100027b0, client node create
//! 0x10002440 and per-frame update 0x10002a30, engine sprite quad Lithtech.exe 0x53e1b0.
//! Everything is drawn through `Quads`: one dynamic mesh per texture, rewritten each frame, so a level
//! costs a handful of draw calls.
use bevy::{asset::RenderAssetUsages,camera::{visibility::{NoFrustumCulling,RenderLayers},ClearColorConfig},core_pipeline::tonemapping::Tonemapping,image::{CompressedImageFormats,ImageAddressMode,ImageLoaderSettings,ImageSampler,ImageSamplerDescriptor,ImageType},mesh::{Indices,PrimitiveTopology},prelude::*};
use crate::{InspectionCamera,ViewerConfig,Walking,WorldGeometry,SCALE,settings::Session};

fn native(v:Vec3)->retail_movement::Vec3 {retail_movement::Vec3::new(v.x,v.y,v.z)}

/// xorshift32; retail uses C `rand()` masked to the low bits, only uniformity matters.
struct Rng(u32);
impl Rng {
    fn next(&mut self)->u32 {let mut x=self.0;x^=x<<13;x^=x>>17;x^=x<<5;self.0=x;x>>8}
    fn below(&mut self,mask:u32)->f32 {(self.next()&mask) as f32}
    /// C `rand() % 100`.
    fn percent(&mut self)->f32 {(self.next()%100) as f32}
}

/// A dynamic batch of quads sharing one texture and additive blending.
/// Retail additive objects blend ONE/ONE with the fog colour forced to black (Lithtech.exe 0x53d5b0),
/// so intensity is carried in the RGB vertex colour and the alpha stays 1.
pub struct Quads {mesh:Handle<Mesh>,entity:Entity,pos:Vec<[f32;3]>,col:Vec<[f32;4]>,cap:usize,used:usize,previous:usize}
impl Quads {
    pub fn new(capacity:usize,texture:Option<Handle<Image>>,commands:&mut Commands,meshes:&mut Assets<Mesh>,materials:&mut Assets<StandardMaterial>)->Self {
        let cap=capacity.max(1);
        let mut mesh=Mesh::new(PrimitiveTopology::TriangleList,RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION,vec![[0.0f32;3];cap*4]);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR,vec![[0.0f32;4];cap*4]);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0,(0..cap).flat_map(|_|[[0.0f32,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0]]).collect::<Vec<_>>());
        mesh.insert_indices(Indices::U32((0..cap as u32).flat_map(|q|[q*4,q*4+1,q*4+2,q*4,q*4+2,q*4+3]).collect()));
        let mesh=meshes.add(mesh);
        let material=materials.add(StandardMaterial {base_color_texture:texture,unlit:true,cull_mode:None,alpha_mode:AlphaMode::Add,fog_enabled:false,..default()});
        let entity=commands.spawn((Mesh3d(mesh.clone()),MeshMaterial3d(material),Transform::default(),Visibility::Hidden,NoFrustumCulling,WorldGeometry)).id();
        Self {mesh,entity,pos:vec![[0.0;3];cap*4],col:vec![[0.0;4];cap*4],cap,used:0,previous:0}
    }
    pub fn begin(&mut self) {self.used=0;}
    /// Corners in native units, top-left, top-right, bottom-right, bottom-left (UV order); colours are linear.
    pub fn quad(&mut self,corners:[Vec3;4],colors:[[f32;4];4]) {
        if self.used>=self.cap {return;}
        for i in 0..4 {self.pos[self.used*4+i]=(corners[i]*SCALE).to_array();self.col[self.used*4+i]=colors[i];}
        self.used+=1;
    }
    pub fn finish(&mut self,meshes:&mut Assets<Mesh>,visibility:&mut Query<&mut Visibility>) {
        if let Ok(mut shown)=visibility.get_mut(self.entity) {
            let want=if self.used>0 {Visibility::Inherited}else{Visibility::Hidden};
            if *shown!=want {*shown=want;}
        }
        if self.used==0 && self.previous==0 {return;}
        for i in self.used*4..self.previous.max(self.used)*4 {self.pos[i]=[0.0;3];}
        self.previous=self.used;
        if let Some(mesh)=meshes.get_mut(&self.mesh) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION,self.pos.clone());
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR,self.col.clone());
        }
    }
}

#[repr(C)] #[derive(Default,Clone,Copy)]
struct SystemTime {year:u16,month:u16,weekday:u16,day:u16,hour:u16,minute:u16,second:u16,millis:u16}
#[cfg(windows)]
unsafe extern "system" {fn GetLocalTime(time:*mut SystemTime);fn GetSystemTime(time:*mut SystemTime);}
/// (local, UTC) wall clock: the hour hand reads GetLocalTime, the minute and second hands GetSystemTime (object.lto imports 0x10025008 / 0x10025000).
fn wall_clock()->(SystemTime,SystemTime) {
    let (mut local,mut utc)=(SystemTime::default(),SystemTime::default());
    #[cfg(windows)]
    unsafe {GetLocalTime(&mut local);GetSystemTime(&mut utc);}
    #[cfg(not(windows))]
    {
        let seconds=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0,|d|d.as_secs());
        utc=SystemTime {hour:(seconds/3600%24) as u16,minute:(seconds/60%60) as u16,second:(seconds%60) as u16,..default()};local=utc;
    }
    (local,utc)
}

/// World-model brushes that move every frame: the clock hands (`b_wskazowka_*`, object.lto 0x10007bb0 / 0x10008650)
/// and `b_rotator` (0x10002fd0). The server sets the ABSOLUTE object rotation from the wall clock every 1 ms; a
/// rotator advances a phase by dt*Krok_Fali/30 and swings by sin(phase/2)*Obrot radians (so it is nearly still).
#[derive(Clone,Copy)]
enum MoverKind {Hour,Minute,Second,Rotor{turn:Vec3,tilt:Vec3,slide:Vec3,step:f32}}
#[derive(Component)]
pub struct Mover {parts:Vec<Entity>,pivot:Vec3,kind:MoverKind,about_z:bool,flip:bool,phase:f32}

/// Hook for `main.rs` model spawning: turns a moving brush's entities into a `Mover`.
pub fn attach(name:&str,object:&serde_json::Value,parts:&[Entity],commands:&mut Commands) {
    let _=name;
    let kind_name=object["kind"].as_str().unwrap_or("");
    let p=&object["properties"];
    let vector=|key:&str|Vec3::new(p[key][0].as_f64().unwrap_or(0.0) as f32,p[key][1].as_f64().unwrap_or(0.0) as f32,p[key][2].as_f64().unwrap_or(0.0) as f32);
    let number=|key:&str|p[key].as_f64().unwrap_or(0.0) as f32;
    let kind=match kind_name {
        "b_wskazowka_godzinowa"=>MoverKind::Hour,
        "b_wskazowka_minutowa"=>MoverKind::Minute,
        "b_wskazowka_sekundowa"=>MoverKind::Second,
        "DemoSkyWorldModel"=>{for part in parts {commands.entity(*part).insert(RenderLayers::layer(SKY_LAYER));}return;}
        "b_rotator"=>MoverKind::Rotor {turn:Vec3::new(number("ObrotX"),number("ObrotY"),number("ObrotZ")),tilt:Vec3::new(number("PrzechylX"),number("PrzechylY"),number("PrzechylZ")),
            slide:Vec3::new(number("PrzesuwX"),number("PrzesuwY"),number("PrzesuwZ")),step:number("Krok_Fali")},
        _=>return,
    };
    commands.spawn((Mover {parts:parts.to_vec(),pivot:vector("Pos"),kind,about_z:p["Na_osi_Z"].as_u64()==Some(1),flip:p["Flip_wskazowek"].as_u64()==Some(1),phase:0.0},WorldGeometry));
}

/// Moves the brushes of every `Mover`; runs with the paused-aware clock like the other decorations.
pub fn animate(mut movers:Query<&mut Mover>,mut transforms:Query<&mut Transform,Without<Mover>>,time:Res<Time>,session:Res<Session>) {
    if movers.is_empty() {return;}
    let (local,utc)=wall_clock();
    for mut mover in &mut movers {
        let dt=if session.paused {0.0}else{time.delta_secs().min(0.1)};
        let sign=if mover.flip {-1.0}else{1.0};
        // With the display mirror (mirror.rs) a positive retail angle turns as it does in LithTech; the old mirrored view needed it negated.
        let handed=if crate::mirror::MIRRORED {1.0}else{-1.0};
        let turn=|fraction:f32|handed*sign*fraction*std::f32::consts::TAU;
        let axis_rotation=|angle:f32,about_z:bool|if about_z {Quat::from_rotation_z(angle)}else{Quat::from_rotation_x(angle)};
        let (rotation,offset)=match mover.kind {
            MoverKind::Hour=>(axis_rotation(turn(((local.hour%12) as f32*60.0+local.minute as f32)/720.0),mover.about_z),Vec3::ZERO),
            MoverKind::Minute=>(axis_rotation(turn(utc.minute as f32/60.0),mover.about_z),Vec3::ZERO),
            MoverKind::Second=>(axis_rotation(turn(utc.second as f32/60.0),mover.about_z),Vec3::ZERO),
            MoverKind::Rotor {turn: swing,tilt,slide,step}=>{
                mover.phase+=dt*step/30.0;
                if mover.phase>4.0*std::f32::consts::PI {mover.phase=0.0;}
                let half=(mover.phase*0.5).sin();
                let angles=swing*half+tilt*half*0.01;
                (Quat::from_euler(EulerRot::YXZ,handed*angles.y,handed*angles.x,handed*angles.z),slide*mover.phase.sin())
            }
        };
        let pivot=mover.pivot*SCALE;
        let translation=pivot-rotation*pivot+offset*SCALE;
        for part in &mover.parts {
            if let Ok(mut transform)=transforms.get_mut(*part) {transform.rotation=rotation;transform.translation=translation;}
        }
    }
}

/// Retail vertex colours are D3D gamma-space bytes; Bevy blends in linear light.
fn gamma(r:f32,g:f32,b:f32)->[f32;4] {Color::srgb(r,g,b).to_linear().to_f32_array()}
fn to_linear(value:f32)->f32 {Color::srgb(value,value,value).to_linear().red}
fn to_srgb(value:f32)->f32 {Srgba::from(LinearRgba::gray(value.max(0.0))).red}
/// D3D adds ONE/ONE in gamma space: encoded result = encoded background + value. Bevy adds linear light, which is far
/// weaker on a dark background, so the light to add is `lin(bg+value)-lin(bg)` for the level's typical background `bg`.
fn add_over(value:f32,bg:f32)->f32 {(to_linear((bg+value).min(1.0))-to_linear(bg)).max(0.0)}
/// The texture of an additive sprite with its object colour `alpha` and the gamma-space add folded in.
fn bake_additive(path:&std::path::Path,alpha:f32,bg:[f32;3])->Option<Image> {
    let bytes=std::fs::read(path).ok()?;
    let mut image=Image::from_buffer(&bytes,ImageType::Extension("png"),CompressedImageFormats::NONE,true,ImageSampler::Default,RenderAssetUsages::default()).ok()?;
    for pixel in image.data.as_mut()?.chunks_exact_mut(4) {
        for channel in 0..3 {pixel[channel]=(to_srgb(add_over(pixel[channel] as f32/255.0*alpha,bg[channel])).min(1.0)*255.0+0.5) as u8;}
    }
    Some(image)
}

struct Drop {pos:Vec3,ground:f32,wait:f32,live:bool}
struct Ripple {pos:Vec3,age:f32}
struct Rain {
    drops:Vec<Drop>,age:f32,rng:Rng,streaks:Quads,ripples:Vec<Ripple>,ripple_frames:Vec<Quads>,
    speed:f32,ripple_half:f32,ripple_time:f32,show_drops:bool,background:[f32;3],
}
/// A retail `d_sprite`. Objects with W_oku (769 of 809) are client sprites from scripts\cs\sprajty.txt (object.lto
/// 0x1000f8bc, cshell 0x10002a30): line-of-sight tested against the eye every dist/5120 s and faded out at 4/s when
/// hidden; the rest are plain engine sprites. Sprite width is 2*texW*Skala.x (Lithtech.exe 0x53e33b).
struct Sprite {
    pos:Vec3,half:Vec2,alpha:f32,small:bool,fog:bool,eye:bool,vertical:bool,frames:Vec<usize>,fps:f32,fade:f32,timer:f32,blocked:bool,
}
#[derive(Clone,Copy,PartialEq)]
enum Role {Behind,Sun,Front,Flash}
struct FlarePart {batch:usize,half:Vec2,alpha:f32,odl:f32,role:Role}
/// A retail `d_lens_flare`: object.lto 0x1000e270 writes the 11 parts to scripts\cs\lensflare.txt, cshell 0x100229b0
/// creates one additive sprite per part (scale Skala, colour Alfa) and 0x10023010 places them every frame.
struct Flare {dir:Vec3,parts:Vec<FlarePart>}
#[derive(Component)]
pub struct Decor {sky_def:Option<SkyDef>,rain:Option<Rain>,sprites:Vec<Sprite>,flares:Vec<Flare>,smokes:Vec<Smoke>,waves:Vec<Waves>,sky:Vec<[Vec3;3]>,batches:Vec<Quads>,fog:Option<(f32,f32)>,far:Option<f32>,clock:f32}

/// A retail `d_emiter_dymu`: object.lto 0x1000aa60 sends message 0x6f 4.4 s after the level starts, cshell
/// spawns a puff at `Czestosc_emisji` intervals (0x10014480) and moves/expands it every frame (0x10014860).
struct Smoke {
    pos:Vec3,delay:f32,timer:f32,period:f32,life:f32,expand:f32,wind:Vec3,spread:Vec3,velocity:Vec3,max_speed:f32,scale:Vec3,fog:bool,
    frames:Vec<usize>,fps:f32,size:Vec2,puffs:Vec<Puff>,rng:Rng,
}
struct Puff {pos:Vec3,velocity:Vec3,scale:Vec3,age:f32,left:f32}

/// A retail `d_fala3d` (object.lto 0x1000cb30 sends message 0x75, cshell 0x1003e380 animates the engine PolyGrid):
/// a grid of `X_ilosc_kratek` x `Z_ilosc_kratek` cells whose height bytes are `sin(px)*sin(pz)*127` with the phases
/// advanced by `X_Speed`/`Z_Speed` per second and by 0.2*PI*index per cell, drawn with the water texture at `Alfa`.
struct Waves {
    cells:(usize,usize),cell:Vec2,center:Vec3,amplitude:f32,speed:Vec2,pan_speed:Vec2,phase:Vec2,pan:Vec2,mesh:Handle<Mesh>,material:Handle<StandardMaterial>,positions:Vec<[f32;3]>,
}

/// The retail sky world (`DemoSkyWorldModel`, object.lto 0x1001fcc0): the first object with SkyDims > 0 sends the engine a
/// SkyDef {pos - dims, pos + dims, pos - dims*Inner, pos + dims*Inner}; the sky brushes sit in that far corner of the map
/// and are drawn behind the level from a camera that turns with the eye but only slides inside the inner box as the eye
/// crosses the level. `level` is the eye range (collision bounds) that maps onto the inner box.
struct SkyDef {center:Vec3,inner:Vec3,level:(Vec3,Vec3),fog:Option<(Color,f32,f32)>}
#[derive(Component)] pub struct SkyCamera;
const SKY_LAYER:usize=6;

/// Which quad batch (texture) each sprite frame draws into, and how many quads a batch needs.
#[derive(Default)]
struct Plan {names:Vec<(String,Option<u32>)>,capacity:Vec<usize>}
impl Plan {
    /// A batch drawing the frame as is.
    fn slot(&mut self,frame:&str)->usize {self.entry((frame.to_owned(),None))}
    /// A batch drawing the frame baked for gamma-space addition at the object colour `alpha` (per cent).
    fn baked(&mut self,frame:&str,alpha:f32)->usize {self.entry((frame.to_owned(),Some((alpha*100.0).round().max(0.0) as u32)))}
    fn entry(&mut self,key:(String,Option<u32>))->usize {
        let index=self.names.iter().position(|n|*n==key).unwrap_or_else(||{self.names.push(key);self.capacity.push(0);self.names.len()-1});
        self.capacity[index]+=1;index
    }
}

/// Sky (SurfaceFlags 1) triangles of the collision world: `<world>.collision.surfaces.json` holds one
/// texture-flag value per collision.obj face (written by tools/export_world.py).
fn sky_triangles(config:&ViewerConfig)->Vec<[Vec3;3]> {
    let read=|name:&str|std::fs::read_to_string(config.output.join(format!("{}.{name}",config.world))).ok();
    let (Some(obj),Some(flags))=(read("collision.obj"),read("collision.surfaces.json")) else {return Vec::new()};
    let Ok(flags)=serde_json::from_str::<serde_json::Value>(&flags) else {return Vec::new()};
    let Some(flags)=flags["faces"].as_array() else {return Vec::new()};
    let mut points=Vec::new();let mut sky=Vec::new();let mut face=0;
    for line in obj.lines() {
        let mut part=line.split_whitespace();
        match part.next() {
            Some("v") => {let v:Vec<f32>=part.filter_map(|p|p.parse().ok()).collect();if v.len()>=3 {points.push(Vec3::new(v[0],v[1],v[2]));}}
            Some("f") => {
                let i:Vec<usize>=part.filter_map(|p|p.split('/').next()?.parse::<usize>().ok()).collect();
                if i.len()>=3 && flags.get(face).and_then(serde_json::Value::as_u64)==Some(1) {
                    if let (Some(a),Some(b),Some(c))=(points.get(i[0]-1),points.get(i[1]-1),points.get(i[2]-1)) {sky.push([*a,*b,*c]);}
                }
                face+=1;
            }
            _ => {}
        }
    }
    sky
}

/// Möller–Trumbore, both sides; distance along the (unit) direction.
fn ray_triangle(origin:Vec3,direction:Vec3,tri:&[Vec3;3])->Option<f32> {
    let (e1,e2)=(tri[1]-tri[0],tri[2]-tri[0]);
    let p=direction.cross(e2);let det=e1.dot(p);
    if det.abs()<1e-9 {return None;}
    let s=origin-tri[0];let u=s.dot(p)/det;
    if !(0.0..=1.0).contains(&u) {return None;}
    let q=s.cross(e1);let v=direction.dot(q)/det;
    if v<0.0 || u+v>1.0 {return None;}
    let t=e2.dot(q)/det;
    (t>=0.0).then_some(t)
}

pub fn setup_world(commands:&mut Commands,meshes:&mut Assets<Mesh>,materials:&mut Assets<StandardMaterial>,assets:&AssetServer,config:&ViewerConfig,scene:&serde_json::Value) {
    let objects:&[serde_json::Value]=scene["objects"].as_array().map_or(&[],|v|v.as_slice());
    let manifest:serde_json::Value=std::fs::read_to_string(config.output.join("decor/sprites.json")).ok()
        .and_then(|text|serde_json::from_str(&text).ok()).unwrap_or_default();
    // Background the additive effects are seen against: the fog colour (retail adds in gamma space).
    let world=objects.iter().find(|o|o["kind"]=="WorldProperties").map(|o|&o["properties"]);
    let background:[f32;3]=world.filter(|w|w["Mgla_Wlaczona"].as_u64()==Some(1)).map_or([0.1;3],|w|[0,1,2].map(|i|w["Kolor_Mgly"][i].as_f64().unwrap_or(25.0) as f32/255.0));
    let mut rain=None;
    if let Some(emitter)=objects.iter().find(|o|o["kind"]=="d_emiter_opadu") {
        let p=&emitter["properties"];
        let ripple=p["Krag_Ziemi"].as_str().map(|s|s.replace('\\',"/").to_lowercase()).and_then(|name|manifest["sprites"].get(&name).cloned());
        let frames:Vec<String>=ripple.as_ref().and_then(|r|r["frames"].as_array()).map(|f|f.iter().filter_map(|x|x.as_str().map(str::to_owned)).collect()).unwrap_or_default();
        let count=p["Ilosc"].as_f64().unwrap_or(64.0) as usize;
        let ripple_frames=frames.iter().map(|frame|Quads::new(count,Some(bake_additive(&config.output.join(frame),1.0,background).map_or_else(||assets.load(frame.clone()),|image|assets.add(image))),commands,meshes,materials)).collect();
        rain=Some(Rain {
            drops:(0..count).map(|_|Drop {pos:Vec3::ZERO,ground:0.0,wait:0.0,live:false}).collect(),
            age:0.0,rng:Rng(0x9e3779b9),streaks:Quads::new(count,None,commands,meshes,materials),background,
            ripples:Vec::new(),ripple_frames,
            // create 0x10030718: Kierunek.y * 0.75 is the fall speed; the ripple sprite is created with the fixed scale 0.4 (0x100309a2).
            speed:p["Kierunek"][1].as_f64().unwrap_or(-1640.0).abs() as f32*0.75,
            ripple_half:0.4*ripple.as_ref().and_then(|r|r["width"].as_f64()).unwrap_or(32.0) as f32,
            ripple_time:p["Czas_Ziemi"].as_f64().unwrap_or(0.5) as f32,show_drops:p["Wyswietlac_krople"].as_u64()==Some(1),
        });
    }
    let sky_def=spawn_sky_camera(objects,config,commands);
    let mut plan=Plan::default();
    let sprites=spawn_sprites(objects,&manifest,&mut plan);
    let flares=spawn_flares(objects,&manifest,&mut plan);
    let smokes=spawn_smokes(objects,&manifest,&mut plan);
    let waves=spawn_waves(objects,&manifest,commands,meshes,materials,assets);
    let batches=plan.names.iter().zip(&plan.capacity).map(|((frame,alpha),count)|{
        let texture=match alpha {
            Some(alpha)=>bake_additive(&config.output.join(frame),*alpha as f32/100.0,background).map_or_else(||assets.load(frame.clone()),|image|assets.add(image)),
            None=>assets.load(frame.clone()),
        };
        Quads::new(*count,Some(texture),commands,meshes,materials)
    }).collect();
    let sky=if rain.is_some() || !flares.is_empty() {sky_triangles(config)}else{Vec::new()};
    let fog=objects.iter().find(|o|o["kind"]=="WorldProperties").map(|o|&o["properties"]).filter(|p|p["Mgla_Wlaczona"].as_u64()==Some(1))
        .and_then(|p|Some((p["Start_Mgly"].as_f64()? as f32,p["Koniec_Mgly"].as_f64()? as f32))).filter(|(start,end)|end>start);
    // WorldProperties Widocznosc is the client's `FarZ` console command (cshell 0x1005b484): the far clip plane.
    let far=objects.iter().find(|o|o["kind"]=="WorldProperties").and_then(|o|o["properties"]["Widocznosc"].as_f64()).map(|v|v as f32*SCALE);
    commands.spawn((Decor {sky_def,rain,sprites,flares,smokes,waves,sky,batches,fog,far,clock:0.0},WorldGeometry));
}

fn spawn_sprites(objects:&[serde_json::Value],manifest:&serde_json::Value,plan:&mut Plan)->Vec<Sprite> {
    let mut sprites=Vec::new();
    for object in objects.iter().filter(|o|o["kind"]=="d_sprite") {
        let p=&object["properties"];
        let Some(name)=p["Nazwa_plika"].as_str().map(|n|n.replace('\\',"/").to_lowercase()) else {continue};
        // sprites\mortyr.spr does not exist in the retail install: the engine draws nothing for it.
        let Some(def)=manifest["sprites"].get(&name) else {continue};
        let (Some(pos),Some(frames))=(p["Pos"].as_array(),def["frames"].as_array()) else {continue};
        let scale=|index:usize|p["Skala"].get(index).and_then(serde_json::Value::as_f64).unwrap_or(1.0) as f32;
        let flag=|key:&str|p[key].as_u64().unwrap_or(0)!=0;
        let alpha=p["Alpha"].as_f64().unwrap_or(1.0) as f32;
        let frames:Vec<usize>=frames.iter().filter_map(|f|f.as_str()).map(|frame|plan.baked(frame,alpha)).collect();
        let coordinate=|i:usize|pos[i].as_f64().unwrap_or(0.0) as f32;
        sprites.push(Sprite {
            pos:Vec3::new(coordinate(0),coordinate(1),coordinate(2)),
            half:Vec2::new(def["width"].as_f64().unwrap_or(0.0) as f32*scale(0),def["height"].as_f64().unwrap_or(0.0) as f32*scale(1)),
            alpha:p["Alpha"].as_f64().unwrap_or(1.0) as f32,small:flag("Maly_w_zblizeniu"),fog:!flag("Wylacz_z_mgly"),eye:flag("W_oku"),vertical:flag("Pionowy"),
            frames,fps:def["fps"].as_f64().unwrap_or(15.0) as f32,fade:1.0,timer:0.0,blocked:false,
        });
    }
    sprites
}

fn spawn_flares(objects:&[serde_json::Value],manifest:&serde_json::Value,plan:&mut Plan)->Vec<Flare> {
    let mut flares=Vec::new();
    for object in objects.iter().filter(|o|o["kind"]=="d_lens_flare") {
        let p=&object["properties"];
        let angle=|i:usize|p["Rotation"].get(i).and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32;
        // The light travels along the object's forward axis; the flare sits at the opposite end (0x10023248).
        let dir=Quat::from_euler(EulerRot::YXZ,angle(1),angle(0),angle(2))*Vec3::Z;
        let number=|key:String|p[key.as_str()].as_f64().unwrap_or(0.0) as f32;
        // File order of scripts\cs\lensflare.txt: Za_centrum3..1, Centrum, Przed_centrum1..6, Blysk.
        let mut layout:Vec<(String,String,String,String,Role)>=Vec::new();
        for i in (1..=3).rev() {layout.push((format!("Za_centrum{i}"),format!("Za_odl{i}"),format!("Za_skala{i}"),format!("Za_alfa{i}"),Role::Behind));}
        layout.push(("Centrum".into(),String::new(),"Centrum_skala".into(),"Centrum_alfa".into(),Role::Sun));
        for i in 1..=6 {layout.push((format!("Przed_centrum{i}"),format!("Przed_odl{i}"),format!("Przed_skala{i}"),format!("Przed_alfa{i}"),Role::Front));}
        layout.push(("Blysk".into(),String::new(),"Blysk_skala".into(),"Blysk_alfa".into(),Role::Flash));
        let mut parts=Vec::new();
        for (sprite,odl,scale,alfa,role) in layout {
            // Parts whose sprite is missing from the retail install (flare1\przed*.spr, za*.spr, centrum.spr) create no object.
            let Some(def)=p[sprite.as_str()].as_str().map(|n|n.replace('\\',"/").to_lowercase()).and_then(|n|manifest["sprites"].get(&n)) else {continue};
            let Some(frame)=def["frames"].get(0).and_then(serde_json::Value::as_str) else {continue};
            let skala=number(scale);
            parts.push(FlarePart {batch:plan.slot(frame),half:Vec2::new(def["width"].as_f64().unwrap_or(0.0) as f32*skala,def["height"].as_f64().unwrap_or(0.0) as f32*skala),
                alpha:number(alfa),odl:if odl.is_empty() {0.0}else{number(odl)},role});
        }
        if !parts.is_empty() {flares.push(Flare {dir,parts});}
    }
    flares
}

fn spawn_smokes(objects:&[serde_json::Value],manifest:&serde_json::Value,plan:&mut Plan)->Vec<Smoke> {
    let mut smokes=Vec::new();
    for (index,object) in objects.iter().filter(|o|o["kind"]=="d_emiter_dymu").enumerate() {
        let p=&object["properties"];
        let Some(def)=p["Nazwa_plika0"].as_str().map(|n|n.replace('\\',"/").to_lowercase()).and_then(|n|manifest["sprites"].get(&n)) else {continue};
        let Some(frames)=def["frames"].as_array() else {continue};
        let vector=|key:&str|Vec3::new(p[key][0].as_f64().unwrap_or(0.0) as f32,p[key][1].as_f64().unwrap_or(0.0) as f32,p[key][2].as_f64().unwrap_or(0.0) as f32);
        let number=|key:&str|p[key].as_f64().unwrap_or(0.0) as f32;
        let (period,life)=(number("Czestosc_emisji").max(0.001),number("Czas_zycia"));
        // Every frame batch may hold every live puff of this emitter.
        let capacity=(life/period) as usize+2;
        let frames:Vec<usize>=frames.iter().filter_map(|f|f.as_str()).map(|frame|{let slot=plan.baked(frame,1.0);plan.capacity[slot]+=capacity-1;slot}).collect();
        smokes.push(Smoke {
            pos:vector("Pos"),delay:4.4,timer:0.0,period,life,expand:number("Rozszerzanie"),wind:vector("Wiatr"),spread:vector("Rozsiew_startu"),velocity:vector("Predkosc_pocz"),
            max_speed:number("Max_predkosc"),scale:vector("Skala"),fog:p["Wylacz_z_mgly"].as_u64()==Some(0),frames,fps:def["fps"].as_f64().unwrap_or(15.0) as f32,
            size:Vec2::new(def["width"].as_f64().unwrap_or(0.0) as f32,def["height"].as_f64().unwrap_or(0.0) as f32),puffs:Vec::new(),rng:Rng(0x2545f491u32.wrapping_add(index as u32*7919)),
        });
    }
    smokes
}

/// Spawns the sky camera (drawn first, then the level camera draws over it) when the level has a sky world.
fn spawn_sky_camera(objects:&[serde_json::Value],config:&ViewerConfig,commands:&mut Commands)->Option<SkyDef> {
    let vector=|value:&serde_json::Value|Vec3::new(value[0].as_f64().unwrap_or(0.0) as f32,value[1].as_f64().unwrap_or(0.0) as f32,value[2].as_f64().unwrap_or(0.0) as f32);
    let object=objects.iter().find(|o|o["kind"]=="DemoSkyWorldModel" && vector(&o["properties"]["SkyDims"]).min_element()>0.0)?;
    let p=&object["properties"];
    let dims=vector(&p["SkyDims"]);
    let inner=dims*Vec3::new(p["InnerPercentX"].as_f64().unwrap_or(0.1) as f32,p["InnerPercentY"].as_f64().unwrap_or(0.1) as f32,p["InnerPercentZ"].as_f64().unwrap_or(0.1) as f32);
    let (mut low,mut high)=(Vec3::splat(f32::INFINITY),Vec3::splat(f32::NEG_INFINITY));
    if let Ok(source)=std::fs::read_to_string(config.output.join(format!("{}.collision.obj",config.world))) {
        for line in source.lines().filter(|l|l.starts_with("v ")) {
            let v:Vec<f32>=line.split_whitespace().skip(1).filter_map(|c|c.parse().ok()).collect();
            if v.len()>=3 {let v=Vec3::new(v[0],v[1],v[2]);low=low.min(v);high=high.max(v);}
        }
    }
    if !low.is_finite() {low=Vec3::splat(-1.0);high=Vec3::splat(1.0);}
    // WorldProperties Mgla_Nieba: fog for the sky world uses its own near/far (client SkyFogEnable/NearZ/FarZ, cshell 0x1005b63f).
    let world=objects.iter().find(|o|o["kind"]=="WorldProperties").map(|o|&o["properties"]);
    let rgb=world.map(|w|(0..3).map(|i|w["Kolor_Mgly"][i].as_f64().unwrap_or(0.0) as f32/255.0).collect::<Vec<f32>>()).unwrap_or_else(||vec![0.0;3]);
    let color=Color::srgb(rgb[0],rgb[1],rgb[2]);
    let fog=world.filter(|w|w["Mgla_Nieba"].as_u64()==Some(1)).map(|w|(color,w["Start_Mgly_Nieba"].as_f64().unwrap_or(1.0) as f32*SCALE,w["Koniec_Mgly_Nieba"].as_f64().unwrap_or(1000.0) as f32*SCALE));
    let mut camera=commands.spawn((SkyCamera,Camera3d::default(),Camera {order:-1,clear_color:ClearColorConfig::Custom(color),..default()},Tonemapping::None,
        Transform::from_translation(vector(&p["Pos"])*SCALE),RenderLayers::layer(SKY_LAYER),WorldGeometry));
    if let Some((color,start,end))=fog {camera.insert(DistanceFog {color,falloff:FogFalloff::Linear {start,end},..default()});}
    Some(SkyDef {center:vector(&p["Pos"]),inner,level:(low,high),fog})
}

fn spawn_waves(objects:&[serde_json::Value],manifest:&serde_json::Value,commands:&mut Commands,meshes:&mut Assets<Mesh>,materials:&mut Assets<StandardMaterial>,assets:&AssetServer)->Vec<Waves> {
    let mut waves=Vec::new();
    for object in objects.iter().filter(|o|o["kind"]=="d_fala3d") {
        let p=&object["properties"];
        let number=|key:&str|p[key].as_f64().unwrap_or(0.0) as f32;
        let Some(frame)=p["Nazwa_plika"].as_str().map(|n|n.replace('\\',"/").to_lowercase()).and_then(|n|manifest["sprites"].get(&n)).and_then(|d|d["frames"].get(0)).and_then(serde_json::Value::as_str) else {continue};
        let (nx,nz)=(number("X_ilosc_kratek") as usize,number("Z_ilosc_kratek") as usize);
        if nx==0 || nz==0 {continue;}
        let color=&p["Kolor"];
        let channel=|i:usize|color[i].as_f64().unwrap_or(255.0) as f32/255.0;
        let texture:Handle<Image>=assets.load_with_settings(frame.to_owned(),|settings:&mut ImageLoaderSettings|{
            settings.sampler=ImageSampler::Descriptor(ImageSamplerDescriptor {address_mode_u:ImageAddressMode::Repeat,address_mode_v:ImageAddressMode::Repeat,..ImageSamplerDescriptor::linear()});
        });
        let (scale_x,scale_z)=(number("SkalaX").max(0.001),number("SkalaZ").max(0.001));
        let vertices=(nx+1)*(nz+1);
        let mut mesh=Mesh::new(PrimitiveTopology::TriangleList,RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION,vec![[0.0f32;3];vertices]);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0,(0..=nz).flat_map(|j|(0..=nx).map(move |i|[i as f32/nx as f32*scale_x,j as f32/nz as f32*scale_z])).collect::<Vec<_>>());
        mesh.insert_indices(Indices::U32((0..nz).flat_map(|j|(0..nx).flat_map(move |i|{let a=(j*(nx+1)+i) as u32;let b=a+1;let c=a+(nx as u32+1);[a,b,c,b,c+1,c]})).collect()));
        let mesh=meshes.add(mesh);
        let material=materials.add(StandardMaterial {base_color:Color::srgba(channel(0),channel(1),channel(2),number("Alfa")),base_color_texture:Some(texture),unlit:true,cull_mode:None,alpha_mode:AlphaMode::Blend,..default()});
        commands.spawn((Mesh3d(mesh.clone()),MeshMaterial3d(material.clone()),Transform::default(),NoFrustumCulling,WorldGeometry));
        let position=&p["Pos"];
        waves.push(Waves {
            cells:(nx,nz),cell:Vec2::new(number("Szer_kratki"),number("Dl_kratki")),center:Vec3::new(position[0].as_f64().unwrap_or(0.0) as f32,position[1].as_f64().unwrap_or(0.0) as f32,position[2].as_f64().unwrap_or(0.0) as f32),
            amplitude:number("Amplituda"),speed:Vec2::new(number("X_Speed"),number("Z_Speed")),pan_speed:Vec2::new(number("Przes_Tex_X"),number("Przes_Tex_Z")),
            phase:Vec2::ZERO,pan:Vec2::ZERO,mesh,material,positions:vec![[0.0;3];vertices],
        });
    }
    waves
}

/// First hit along the ray is a sky polygon (SurfaceFlags 1) within `max`: the rain and lens flare visibility test.
fn sky_first(walking:&Walking,sky:&[[Vec3;3]],from:Vec3,direction:Vec3,max:f32)->bool {
    let world=walking.world.raycast(native(from),native(direction),max).map(|(distance,_)|distance);
    let hit=sky.iter().filter_map(|tri|ray_triangle(from,direction,tri)).fold(None::<f32>,|best,t|Some(best.map_or(t,|b|b.min(t))));
    match (hit,world) {(Some(s),Some(w))=>s<=w+0.5,(Some(s),None)=>s<=max,_=>false}
}

/// One retail respawn (0x10030ae0): a point around the eye; it rains only when a 10240-unit ray straight up
/// first meets a sky polygon, otherwise the drop waits one second.
fn respawn(drop:&mut Drop,rng:&mut Rng,eye:Vec3,sky:&[[Vec3;3]],walking:&Walking) {
    drop.pos=Vec3::new(eye.x+rng.below(0x3ff)-512.0,eye.y+rng.below(0x3ff)*0.5,eye.z+rng.below(0x3ff)-512.0);
    if sky_first(walking,sky,drop.pos,Vec3::Y,10240.0) {
        drop.live=true;
        drop.ground=walking.world.raycast(native(drop.pos),native(Vec3::NEG_Y),1280.0).map_or(drop.pos.y-1280.0,|(t,_)|drop.pos.y-t);
    }else{drop.live=false;drop.wait=1.0;}
}

pub fn update(mut decor:Query<&mut Decor>,camera:Single<&Transform,With<InspectionCamera>>,mut projection:Single<&mut Projection,With<InspectionCamera>>,walking:Res<Walking>,time:Res<Time>,session:Res<Session>,mut meshes:ResMut<Assets<Mesh>>,mut materials:ResMut<Assets<StandardMaterial>>,mut visibility:Query<&mut Visibility>,mut main_camera:Single<&mut Camera,With<InspectionCamera>>,mut sky_camera:Query<(&mut Transform,&mut Projection),(With<SkyCamera>,Without<InspectionCamera>)>) {
    for mut decor in &mut decor {
        if let Some(sky) = &decor.sky_def {
            // The level camera must not wipe the sky drawn by the order -1 camera; the sky slides inside its inner box as the eye crosses the level.
            main_camera.clear_color=ClearColorConfig::None;
            let (low,high)=sky.level;
            let along=((camera.translation/SCALE-low)/(high-low).max(Vec3::splat(1.0))).clamp(Vec3::ZERO,Vec3::ONE);
            for (mut transform,mut sky_projection) in &mut sky_camera {
                transform.translation=(sky.center+(along*2.0-Vec3::ONE)*sky.inner)*SCALE;
                transform.rotation=camera.rotation;
                *sky_projection=(*projection).clone();
            }
        }
        if let Some(far)=decor.far.take() {
            match &mut **projection {
                Projection::Perspective(perspective)=>perspective.far=far,
                Projection::Custom(custom)=>if let Some(retail)=custom.get_mut::<crate::view::RetailProjection>() {retail.perspective.far=far;},
                _=>{}
            }
        }
    }
    if session.paused {return;}
    let dt=time.delta_secs().min(0.1);
    let camera=*camera;
    for mut decor in &mut decor {
        let decor=&mut *decor;
        decor.clock+=dt;
        if let Some(rain)=&mut decor.rain {rain_step(rain,&decor.sky,dt,&camera,&walking,&mut meshes,&mut visibility);}
        sprite_step(decor,dt,&camera,&walking,&mut meshes,&mut visibility);
        for waves in &mut decor.waves {wave_step(waves,dt,&mut meshes,&mut materials);}
    }
}

fn rain_step(rain:&mut Rain,sky:&[[Vec3;3]],dt:f32,camera:&Transform,walking:&Walking,meshes:&mut Assets<Mesh>,visibility:&mut Query<&mut Visibility>) {
    let eye=camera.translation/SCALE;
    let right=camera.rotation*Vec3::X;
    rain.age+=dt;
    // The server sends the rain effect once, 3.1 s after the level starts (0x1000c19d).
    if rain.age<3.1 {return;}
    let Rain {drops,rng,streaks,ripples,ripple_frames,speed,ripple_half,ripple_time,show_drops,background,..}=rain;
    for drop in drops.iter_mut() {
        if !drop.live {
            drop.wait-=dt;
            if drop.wait<=0.0 {respawn(drop,rng,eye,sky,walking);}
            continue;
        }
        let d=drop.pos-eye;
        if d.x.abs()>1024.0 || d.y.abs()>1024.0 || d.z.abs()>1024.0 {respawn(drop,rng,eye,sky,walking);continue;}
        let y=drop.pos.y-*speed*dt;
        if y>drop.ground {drop.pos.y=y;continue;}
        // Landed: a ripple 2 units above the ground when the landing lies in the half space the retail test
        // 0x10031000 accepts (dot(eye position, direction to the landing) > 0), then a new drop.
        let landing=Vec3::new(drop.pos.x,drop.ground+2.0,drop.pos.z);
        respawn(drop,rng,eye,sky,walking);
        let toward=(landing-eye).normalize_or_zero();
        if *show_drops && !ripple_frames.is_empty() && eye.dot(toward)>0.0 {ripples.push(Ripple {pos:landing,age:0.0});}
    }
    ripples.retain_mut(|r|{r.age+=dt;r.age<*ripple_time});
    // Drawn as one vertical camera-facing streak per drop, 30 units tall, 0.8 wide (0x10030ea7..0x10030f97).
    let across=Vec3::new(right.x,0.0,right.z).try_normalize().unwrap_or(Vec3::X)*0.4;
    let colour=|r:f32,g:f32,b:f32|{let bg=*background;[add_over(r/255.0,bg[0]),add_over(g/255.0,bg[1]),add_over(b/255.0,bg[2]),1.0]};
    let (top,bottom)=(colour(7.0,17.0,12.0),colour(15.0,35.0,25.0));
    streaks.begin();
    if *show_drops {
        for drop in drops.iter().filter(|d|d.live) {
            let p=drop.pos;
            streaks.quad([p-across+Vec3::Y*15.0,p+across+Vec3::Y*15.0,p+across-Vec3::Y*15.0,p-across-Vec3::Y*15.0],[top,top,bottom,bottom]);
        }
    }
    streaks.finish(meshes,visibility);
    for frames in ripple_frames.iter_mut() {frames.begin();}
    let count=ripple_frames.len();
    for ripple in ripples.iter() {
        let frame=((ripple.age/ *ripple_time*count as f32) as usize).min(count-1);
        let (half,p)=(*ripple_half,ripple.pos);
        ripple_frames[frame].quad([p+Vec3::new(-half,0.0,-half),p+Vec3::new(half,0.0,-half),p+Vec3::new(half,0.0,half),p+Vec3::new(-half,0.0,half)],[[1.0,1.0,1.0,1.0];4]);
    }
    for frames in ripple_frames.iter_mut() {frames.finish(meshes,visibility);}
}

/// cshell 0x1003e421..0x1003e4b1: `byte = (int)(sin(px)*sin(pz)*127)`; `px` starts at the phase and gains `col*0.2*PI`
/// after every cell of a row, `pz` starts at its phase and gains `row*0.2*PI` after every row.
fn wave_heights(phase:Vec2,columns:usize,rows:usize)->Vec<f32> {
    let mut heights=Vec::with_capacity(columns*rows);
    let mut pz=phase.y;
    for row in 0..rows {
        let mut px=phase.x;
        for column in 0..columns {
            heights.push(((px.sin()*pz.sin()*127.0) as i32).clamp(-128,127) as f32);
            px+=column as f32*std::f32::consts::TAU*0.1;
        }
        pz+=row as f32*std::f32::consts::TAU*0.1;
    }
    heights
}

fn wave_step(waves:&mut Waves,dt:f32,meshes:&mut Assets<Mesh>,materials:&mut Assets<StandardMaterial>) {
    let (nx,nz)=waves.cells;
    let heights=wave_heights(waves.phase,nx+1,nz+1);
    waves.phase+=waves.speed*dt;
    waves.pan+=waves.pan_speed*dt;
    let half=Vec2::new(nx as f32*waves.cell.x,nz as f32*waves.cell.y)*0.5;
    for j in 0..=nz {
        for i in 0..=nx {
            let position=Vec3::new(waves.center.x-half.x+i as f32*waves.cell.x,waves.center.y+heights[j*(nx+1)+i]/127.0*waves.amplitude,waves.center.z-half.y+j as f32*waves.cell.y);
            waves.positions[j*(nx+1)+i]=(position*SCALE).to_array();
        }
    }
    if let Some(mesh)=meshes.get_mut(&waves.mesh) {mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION,waves.positions.clone());}
    if let Some(material)=materials.get_mut(&waves.material) {material.uv_transform=bevy::math::Affine2::from_translation(waves.pan);}
}

/// Screen-edge fade of a flare (cshell 0x10022e20 hides it when the eye looks away from the sun; 0x10022ee0 and
/// 0x10023010 turn the sines of the angle in the xz and xy planes into 1 - |sin|/limit, limit 1.7267 or 0.8634).
fn flare_fade(dir:Vec3,forward:Vec3)->Option<f32> {
    if dir.dot(forward)>=0.0 {return None;}
    let sine=|a:Vec2,b:Vec2,cross:f32|{let length=a.length()*b.length();cross/if length<1e-4 {0.01}else{length}};
    let across_xz=sine(Vec2::new(dir.x,dir.z),Vec2::new(forward.x,forward.z),dir.z*forward.x-forward.z*dir.x).abs();
    let across_xy=sine(Vec2::new(dir.x,dir.y),Vec2::new(forward.x,forward.y),dir.y*forward.x-forward.y*dir.x).abs();
    let (value,limit)=if across_xy>across_xz {(across_xy,0.86335)}else{(across_xz,1.72671)};
    let t=(limit-value)/limit;
    (t>0.0).then_some(t)
}

/// Linear fog with black fog colour (additive sprites), Lithtech.exe 0x53d5b0: 1 at the start, 0 at the end.
fn fog_factor(fog:Option<(f32,f32)>,depth:f32)->f32 {fog.map_or(1.0,|(start,end)|((end-depth)/(end-start)).clamp(0.0,1.0))}

/// The "small when close" sprite flag scales the quad by 0.1 at 10 units up to 2.0 at 500 units (0x53e352..0x53e3a1).
fn close_scale(depth:f32)->f32 {((depth-10.0)*0.00204082).clamp(0.0,1.0)*1.9+0.1}

fn sprite_step(decor:&mut Decor,dt:f32,camera:&Transform,walking:&Walking,meshes:&mut Assets<Mesh>,visibility:&mut Query<&mut Visibility>) {
    if decor.sprites.is_empty() && decor.flares.is_empty() && decor.smokes.is_empty() {return;}
    let eye=camera.translation/SCALE;
    let (right,up,forward)=(crate::mirror::right(camera),camera.rotation*Vec3::Y,camera.rotation*Vec3::NEG_Z);
    let Decor {sprites,flares,smokes,sky,batches,fog,clock,..}=decor;
    for batch in batches.iter_mut() {batch.begin();}
    for sprite in sprites.iter_mut() {
        let to_eye=eye-sprite.pos;
        let depth=-to_eye.dot(forward);
        // Sprites nearer than 7 units (or behind the eye) are not drawn (0x53e2ee).
        if depth<7.0 {continue;}
        if sprite.eye {
            sprite.timer-=dt;
            if sprite.timer<=0.0 {
                let distance=to_eye.length();
                sprite.timer=distance*0.25*0.00078125;
                let direction=to_eye/distance.max(0.001);
                sprite.blocked=distance>6.0 && walking.world.raycast(native(sprite.pos+direction*3.0),native(direction),distance-3.0).is_some_and(|(t,_)|t<distance-6.0);
                if !sprite.blocked {sprite.fade=1.0;}
            }
            if sprite.blocked {sprite.fade=(sprite.fade-4.0*dt).max(0.0);}
        }
        let intensity=sprite.fade*if sprite.fog {fog_factor(*fog,depth)}else{1.0};
        if intensity<=0.002 {continue;}
        let scale=if sprite.small {close_scale(depth)}else{1.0};
        let (across,vertical)=if sprite.vertical {
            let away=Vec3::new(-to_eye.x,0.0,-to_eye.z);
            (Vec3::Y.cross(away).try_normalize().unwrap_or(right),Vec3::Y)
        }else{(right,up)};
        let (w,h)=(across*(sprite.half.x*scale),vertical*(sprite.half.y*scale));
        let frame=sprite.frames.get(((*clock*sprite.fps) as usize)%sprite.frames.len().max(1)).copied();
        let Some(batch)=frame.and_then(|f|batches.get_mut(f)) else {continue};
        let color=gamma(intensity,intensity,intensity);
        batch.quad([sprite.pos-w+h,sprite.pos+w+h,sprite.pos+w-h,sprite.pos-w-h],[color;4]);
    }
    for flare in flares.iter() {
        let Some(t)=flare_fade(flare.dir,forward) else {continue};
        if !sky_first(walking,sky,eye,-flare.dir,20480.0) {continue;}
        // The apparent sun is 32 units from the eye along the light; the parts lie on the line to the point 32 units ahead.
        let (sun,center)=(eye-flare.dir*32.0,eye+forward*32.0);
        let step=(center-sun)*0.1;
        for part in &flare.parts {
            let (pos,level)=match part.role {
                Role::Behind=>(sun-step*part.odl,(t*part.alpha).sqrt()),
                Role::Sun=>(sun,(t*part.alpha).sqrt()),
                Role::Front=>(sun+step*part.odl,(t*part.alpha).sqrt()),
                Role::Flash=>{if t<0.75 {continue;}(sun,((t-0.75)*4.0*part.alpha).powi(2))}
            };
            if (pos-eye).dot(forward)<7.0 || level<=0.002 {continue;}
            let (w,h)=(right*part.half.x,up*part.half.y);
            let color=gamma(level.min(1.0),level.min(1.0),level.min(1.0));
            batches[part.batch].quad([pos-w+h,pos+w+h,pos+w-h,pos-w-h],[color;4]);
        }
    }
    for smoke in smokes.iter_mut() {
        smoke.delay-=dt;
        if smoke.delay>0.0 {continue;}
        smoke.timer-=dt;
        if smoke.timer<0.0 {
            // 0x10014480: uniform +-spread around the emitter (difference of two 0..0.99 draws), initial velocity, own scale.
            let mut draw=|extent:f32|extent*(smoke.rng.percent()-smoke.rng.percent())*0.01;
            let spread=Vec3::new(draw(smoke.spread.x),draw(smoke.spread.y),draw(smoke.spread.z));
            smoke.puffs.push(Puff {pos:smoke.pos+spread,velocity:smoke.velocity,scale:smoke.scale,age:0.0,left:smoke.life});
            smoke.timer=smoke.period;
        }
        let Smoke {puffs,rng,wind,max_speed,expand,frames,fps,size,fog: fogged,..}=smoke;
        puffs.retain_mut(|puff|{
            puff.left-=dt;puff.age+=dt;
            if puff.left<0.0 {return false;}
            // 0x10014938: the wind is added every frame with a random 0.01..1.99 factor, speed is clamped to Max_predkosc.
            let mut jitter=||1.0+rng.percent()*0.01-rng.percent()*0.01;
            puff.velocity+=Vec3::new(wind.x*jitter(),wind.y*jitter(),wind.z*jitter())*(dt*60.0);
            if puff.velocity.length()>*max_speed {puff.velocity=puff.velocity.normalize_or_zero()* *max_speed;}
            puff.pos+=puff.velocity*dt;
            puff.scale*=1.0+dt* *expand;
            true
        });
        for puff in puffs.iter() {
            let depth=(puff.pos-eye).dot(forward);
            if depth<7.0 {continue;}
            let level=if *fogged {fog_factor(*fog,depth)}else{1.0};
            if level<=0.002 {continue;}
            let frame=frames.get(((puff.age* *fps) as usize)%frames.len().max(1)).copied();
            let Some(batch)=frame.and_then(|f|batches.get_mut(f)) else {continue};
            let (w,h)=(right*(size.x*puff.scale.x),up*(size.y*puff.scale.y));
            batch.quad([puff.pos-w+h,puff.pos+w+h,puff.pos+w-h,puff.pos-w-h],[gamma(level,level,level);4]);
        }
    }
    for batch in batches.iter_mut() {batch.finish(meshes,visibility);}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_up_meets_a_sky_quad_and_misses_beside_it() {
        let quad=[Vec3::new(-100.0,500.0,-100.0),Vec3::new(100.0,500.0,-100.0),Vec3::new(100.0,500.0,100.0)];
        assert_eq!(ray_triangle(Vec3::new(10.0,0.0,-50.0),Vec3::Y,&quad),Some(500.0));
        assert_eq!(ray_triangle(Vec3::new(-90.0,0.0,90.0),Vec3::Y,&quad),None);
        assert_eq!(ray_triangle(Vec3::new(10.0,600.0,-50.0),Vec3::Y,&quad),None);
    }

    #[test]
    fn close_sprites_shrink_and_far_ones_double() {
        assert!((close_scale(10.0)-0.1).abs()<1e-4 && (close_scale(255.0)-1.05).abs()<0.01 && (close_scale(500.0)-2.0).abs()<0.01 && (close_scale(9000.0)-2.0).abs()<0.01);
    }

    #[test]
    fn linear_fog_fades_additive_sprites_to_black() {
        assert_eq!(fog_factor(Some((1.0,4001.0)),4001.0),0.0);
        assert!((fog_factor(Some((1.0,4001.0)),2001.0)-0.5).abs()<1e-4);
        assert_eq!(fog_factor(None,1.0e9),1.0);
    }

    #[test]
    fn flare_shows_when_looking_at_the_sun_and_fades_toward_the_edges() {
        let dir=Vec3::new(-0.428,-0.860,-0.276).normalize();
        assert!((flare_fade(dir,-dir).unwrap()-1.0).abs()<1e-3);
        assert!(flare_fade(dir,dir).is_none());
        let level=flare_fade(dir,Vec3::new(0.0,0.0,-1.0));
        assert!(level.is_none_or(|t|t<0.2));
    }

    #[test]
    fn wave_heights_follow_the_retail_chirp() {
        let heights=wave_heights(Vec2::new(std::f32::consts::FRAC_PI_2,std::f32::consts::FRAC_PI_2),3,2);
        // row 0, column 0: sin(pi/2)*sin(pi/2)*127; the second column adds 0 to px, the third adds 0.2*pi.
        assert_eq!((heights[0],heights[1]),(127.0,127.0));
        assert!((heights[2]-(std::f32::consts::FRAC_PI_2+std::f32::consts::TAU*0.1).sin()*127.0).abs()<1.0);
        assert_eq!(heights.len(),6);
    }

    #[test]
    fn gamma_space_addition_is_stronger_than_linear_on_a_dark_background() {
        // Adding 35/255 to a background of 37/255 in D3D gives 72/255; the linear-light amount that reproduces it.
        let amount=add_over(35.0/255.0,37.0/255.0);
        assert!((to_srgb(to_linear(37.0/255.0)+amount)*255.0-72.0).abs()<0.5);
        assert!(amount>2.5*to_linear(35.0/255.0));
        assert_eq!(add_over(0.0,0.5),0.0);
    }

    #[test]
    fn respawn_box_matches_the_retail_spawn_volume() {
        let mut rng=Rng(1234);
        for _ in 0..200 {
            let (x,y,z)=(rng.below(0x3ff)-512.0,rng.below(0x3ff)*0.5,rng.below(0x3ff)-512.0);
            assert!((-512.0..=511.0).contains(&x) && (0.0..=511.5).contains(&y) && (-512.0..=511.0).contains(&z));
        }
    }
}
