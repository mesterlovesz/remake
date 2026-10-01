//! Gunfire shared by every weapon: input, the world/NPC hit query and the original presentation
//! (muzzle flash, smoke, sparks, tracers, impacts, holes, debris, blood, casings; docs/retail-gunfire.md).
use bevy::{asset::RenderAssetUsages,image::{CompressedImageFormats,ImageSampler,ImageType},prelude::*,window::{CursorGrabMode,CursorOptions}};
use std::collections::BTreeMap;
use crate::{InspectionCamera,ViewerConfig,SCALE,Walking,fx::{self,Particle,Library,SpriteAsset,ModelAsset,Orient,between,pick,rnd,signed},settings::Session,sound};

#[derive(Default)]
pub struct ShotOutcome {pub hit:bool,pub killed:bool,pub name:String,pub world_hit:bool,pub end:Vec3}
#[derive(Resource,Default)]
pub struct Controls {pub fire:bool,pub held:bool,pub reload:bool,pub slot:Option<usize>,pub native_slot:Option<usize>}
/// A bullet hole that rides on a moving door (brush-local point and normal).
#[derive(Component)] pub struct DoorImpact {door:Entity,point:Vec3,normal:Vec3}
/// Blood on the screen after the player is hit (krewmonitor.spr, cshell 0x10033a64): fades over one second.
#[derive(Component)] pub struct BloodOverlay {age:f32}

#[derive(Resource)]
pub struct Effects {pub lib:Library,assets:AssetServer,weapons:BTreeMap<String,Vec<(String,String)>>,
    /// The "shot debris" option (0x100b2490): off leaves no debris pieces and no bullet marks (0x100088b8, 0x10008a55).
    pub debris:bool,
    /// Scene props: bullets test their boxes and queue damage (docs/retail-props.md).
    pub props:crate::props::PropWorld}
impl Effects {
    /// Raw item keys of a player weapon (`sprite0`, `hit_sprite`, `iskry_po_strzale_gracza`, ...).
    pub fn weapon(&self,id:&str)->&[(String,String)] {self.weapons.get(id).map_or(&[],Vec::as_slice)}
}
/// A numeric item key of a player weapon (`glosnosc`, `hit_smuga`, ...).
pub fn item_number(fx:&Effects,weapon:&str,key:&str)->Option<f32> {cn(fx.weapon(weapon),key)}
pub(crate) fn cv<'a>(commands:&'a [(String,String)],key:&str)->Option<&'a str> {commands.iter().rev().find(|(k,_)|k==key).map(|(_,v)|v.as_str()).filter(|v|!v.is_empty())}
pub(crate) fn cn(commands:&[(String,String)],key:&str)->Option<f32> {cv(commands,key)?.split_whitespace().next()?.parse::<f32>().ok().filter(|v|v.is_finite())}
fn has(commands:&[(String,String)],key:&str)->bool {commands.iter().any(|(k,_)|k==key)}

pub fn setup(mut commands:Commands,config:Res<ViewerConfig>,assets:Res<AssetServer>,mut meshes:ResMut<Assets<Mesh>>,mut images:ResMut<Assets<Image>>,mut materials:ResMut<Assets<StandardMaterial>>,props:Res<crate::props::PropWorld>) {
    commands.init_resource::<Controls>();
    let manifest:serde_json::Value=serde_json::from_str(&std::fs::read_to_string(config.output.join("retail_effects.json")).expect("run tools.export_effects for original sprites")).expect("original effects manifest");
    let mut sprites=BTreeMap::new();
    for (path,value) in manifest["sprites"].as_object().into_iter().flatten() {
        let frames:Vec<Handle<Image>>=value["frames"].as_array().into_iter().flatten().filter_map(|frame|frame.as_str()).map(|frame|assets.load(frame.to_string())).collect();
        if frames.is_empty() {continue;}
        let dimension=|key:&str|value[key].as_f64().unwrap_or(128.0) as f32;
        sprites.insert(fx::key(path),SpriteAsset {frames:frames.into(),fps:value["fps"].as_f64().unwrap_or(15.0) as f32,size:Vec2::new(dimension("width"),dimension("height"))});
    }
    // dziura*.dtx and slad.dtx export with inverted alpha: flip it so the crater is the opaque part.
    for (path,png) in manifest["textures"].as_object().into_iter().flatten() {
        let Some(png)=png.as_str() else {continue};
        let Ok(bytes)=std::fs::read(config.output.join(png)) else {continue};
        let Ok(mut image)=Image::from_buffer(&bytes,ImageType::Extension("png"),CompressedImageFormats::NONE,true,ImageSampler::Default,RenderAssetUsages::default()) else {continue};
        if let Some(pixels)=image.data.as_mut() {invert_decal_alpha(pixels);}
        let size=Vec2::new(image.width() as f32,image.height() as f32);
        sprites.insert(fx::key(path),SpriteAsset {frames:vec![images.add(image)].into(),fps:0.0,size});
    }
    let mut models=BTreeMap::new();
    for (path,value) in manifest["models"].as_object().into_iter().flatten() {
        let Some(skin)=value["skin"].as_str() else {continue};
        let model=crate::models::Model::load(&config.output,path);let pose=model.pose("",0.0,false);
        let material=materials.add(StandardMaterial {base_color_texture:Some(assets.load(level_viewer::hd::path(skin))),unlit:true,cull_mode:None,..default()});
        let parts:Vec<_>=(0..model.pieces.len()).map(|piece|{let mut mesh=model.mesh(piece);model.animate_mesh(piece,&pose,&mut mesh);(meshes.add(mesh),material.clone())}).collect();
        models.insert(fx::key(path),ModelAsset {parts:parts.into()});
    }
    let catalog:serde_json::Value=serde_json::from_str(&std::fs::read_to_string(config.output.join("retail_weapons.json")).expect("original weapon export")).expect("original weapon export format");
    // The grenade blast's own flash and fireball sprites (exported with the weapons, cshell 0x10053ab0) join the library next to the other hard-coded effect sprites.
    for layer in catalog["grenade_effect"]["layers"].as_array().into_iter().flatten() {
        let frames:Vec<Handle<Image>>=layer["frames"].as_array().into_iter().flatten().filter_map(|frame|frame.as_str()).map(|frame|assets.load(frame.to_string())).collect();
        let Some(path)=layer["sprite"].as_str() else {continue};if frames.is_empty() {continue;}
        let dimension=|key:&str|layer[key].as_f64().unwrap_or(64.0) as f32;
        sprites.insert(fx::key(path),SpriteAsset {frames:frames.into(),fps:layer["fps"].as_f64().unwrap_or(15.0) as f32,size:Vec2::new(dimension("width"),dimension("height"))});
    }
    let weapons=catalog["weapons"].as_array().into_iter().flatten().filter_map(|weapon|{
        let pairs=weapon["commands"].as_array()?.iter().filter_map(|pair|Some((pair[0].as_str()?.to_owned(),pair[1].as_str()?.to_owned()))).collect();
        Some((weapon["id"].as_str()?.to_owned(),pairs))
    }).collect();
    commands.insert_resource(Effects {lib:Library {quad:meshes.add(Rectangle::new(1.0,1.0)),sprites,models},assets:assets.clone(),weapons,debris:true,props:props.clone()});
}

fn invert_decal_alpha(pixels:&mut [u8]) {
    for pixel in pixels.chunks_exact_mut(4) {pixel[3]=255-pixel[3];}
}

pub struct GunfirePlugin;
impl Plugin for GunfirePlugin {
    fn build(&self,app:&mut App) {
        app.add_systems(Update,crate::retail_weapons::blast_probe);
        app.add_systems(Update,(follow_muzzle,attached_decals,npc_effects,blood_overlay,fx::realize,fx::tick,sound::update).chain().after(crate::retail_weapons::present));
    }
}
/// Muzzle flashes stay on the gun's socket (cshell 0x10053170).
fn follow_muzzle(native:Res<crate::retail_weapons::NativeArsenal>,view:Res<crate::view::ViewState>,mut particles:Query<&mut Particle>) {
    let Some(slot)=native.inventory.selected else {return};
    for mut particle in &mut particles {
        let Some(socket)=particle.follow.clone() else {continue};
        if let Some(point)=native.socket_view(slot,&socket,&view) {particle.pos=point/SCALE;}
    }
}
fn attached_decals(mut commands:Commands,doors:Query<&crate::doors::Door>,mut holes:Query<(Entity,&DoorImpact,&mut Particle)>) {
    for (entity,impact,mut hole) in &mut holes {
        let Ok(door)=doors.get(impact.door) else {commands.entity(entity).despawn();continue};
        let (rotation,translation)=door.pose();let normal=rotation*impact.normal;
        hole.pos=rotation*impact.point+translation+normal*0.5;hole.orient=Orient::Fixed(Quat::from_rotation_arc(Vec3::Z,normal));
    }
}

pub fn input(mut controls:ResMut<Controls>,session:Res<Session>,bind:crate::options::Bindings,cursor:Single<&CursorOptions>) {
    use crate::keys_cfg::cmd;
    *controls=Controls::default();
    if session.paused || session.dialogue_active || cursor.grab_mode==CursorGrabMode::None {return;}
    controls.fire=session.suppress_fire==0 && !session.dialogue_choices && bind.just_pressed(cmd::FIRE);
    controls.held=session.suppress_fire==0 && !session.dialogue_choices && bind.pressed(cmd::FIRE);
    // Level triggered like every action slot (0x10011960 asks the engine each frame; 0x10060d0f calls the reload while it is down).
    controls.reload=bind.pressed(cmd::RELOAD);
    controls.slot=if bind.just_pressed(cmd::WEAPON1) {Some(0)}else if bind.just_pressed(cmd::WEAPON1+1) {Some(1)}else{None};
    controls.native_slot=(0..8).position(|i|bind.just_pressed(cmd::WEAPON1+i));
    // Retail's previous / next weapon commands (F / G) end in an empty stub (0x1005ab50) and it has no wheel or Tab binding: there is no weapon cycling.
}

/// SurfaceFlags -> bullet mark (cshell 0x10008a55): 6 = tiles get a random `dziura0..2` crater, 1 = sky none, the rest `slad`.
#[derive(Debug,PartialEq)] pub enum Mark {Hole(usize),Scorch,Nothing}
pub fn mark_for(flags:u8,variant:usize)->Mark {match flags {1=>Mark::Nothing,6=>Mark::Hole(variant%3),_=>Mark::Scorch}}
/// SurfaceFlags -> debris (cshell 0x1002c870): 0 stone rubble, 6 rubble and smoke, 2 metal sparks, 3 wood splinters.
#[derive(Debug,PartialEq)] pub enum Debris {Nothing,Rubble(u32),RubbleSmoke(u32),Sparks(u32),Wood(u32)}
pub fn debris_for(flags:u8)->Debris {match flags {0=>Debris::Rubble(4),6=>Debris::RubbleSmoke(6),2=>Debris::Sparks(5),3=>Debris::Wood(5),_=>Debris::Nothing}}
/// Screen roll of the blood overlay: 0 when the attacker is ahead, +pi/2 when on the right of the displayed image (camera-local offset;
/// the image is flipped, see mirror.rs).
pub fn overlay_roll(local:Vec3)->f32 {(local.x*crate::mirror::SIGN).atan2(-local.z)}

struct WorldHit {distance:f32,normal:Vec3,flags:u8,door:Option<Entity>}
/// Nearest static or door hit with the SurfaceFlags of the struck polygon (native units).
fn world_hit(world:&retail_movement::CollisionWorld,doors:&Query<(Entity,&crate::doors::Door)>,surfaces:&crate::audio::Surfaces,origin:Vec3,direction:Vec3,range:f32)->Option<WorldHit> {
    let native=|v:Vec3|retail_movement::Vec3::new(v.x,v.y,v.z);
    let stone=world.raycast_face(native(origin),native(direction),range).map(|(distance,n,face)|WorldHit {distance,normal:Vec3::new(n.x,n.y,n.z),flags:surfaces.flags(face).unwrap_or(0) as u8,door:None});
    let brush=doors.iter().filter_map(|(entity,door)|door.shot_hit(origin,direction,range).map(|(distance,normal)|WorldHit {distance,normal,flags:surfaces.model(&door.name) as u8,door:Some(entity)}))
        .min_by(|a,b|a.distance.total_cmp(&b.distance));
    stone.into_iter().chain(brush).min_by(|a,b|a.distance.total_cmp(&b.distance))
}
fn decal(fx:&Effects,path:&str,pos:Vec3,normal:Vec3,units:f32,life:f32)->Option<Particle> {
    let size=fx.lib.sprite(path)?.size;
    Particle::sprite(&fx.lib,path,pos).map(|p|p.facing(normal).normal_alpha().scale(units/(2.0*size.x)).life(life))
}
pub(crate) fn ricochet(commands:&mut Commands,fx:&Effects,point:Vec3) {
    if rnd()<0.75 {sound::play_at(commands,&fx.assets,&format!("sounds/weapons/ryko{}.wav",pick(5)),point,1280.0);}
}
fn tracer(commands:&mut Commands,fx:&Effects,center:Vec3,direction:Vec3) {
    // smuga.spr streak, scale (0.12,40), 4000 u/s for 0.6 s (cshell 0x10006605).
    fx::spawn(commands,Particle::sprite(&fx.lib,"sprites/smuga.spr",center).map(|p|p.scale2(0.12,40.0).life(0.6).vel(direction*4000.0).streak()));
}
/// A blood puff: Spawn type 1, life 1, moves, gravity mode 3, growth 1.2, colour (0.8,0.8,0.8) = constant alpha 0.8, normal alpha; the fade argument (arg 17) is 0 at 0x10005511, 0x100055e6, 0x100056bb,
/// so the puff pops out at the end of its life instead of fading.
fn puff(fx:&Effects,name:&str,pos:Vec3,vel:Vec3)->Option<Particle> {
    Particle::sprite(&fx.lib,&format!("sprites/{name}.spr"),pos).map(|p|p.scale(0.05).life(1.0).growth(1.2).vel(vel).gravity(96.0).alpha(0.8).normal_alpha())
}
/// The random vector of cshell 0x10005360: y = r/16, x = r/16 - r/16, z = r/16 - r/16 with r = rand() & 0xff (triangular +-16, y 0..16).
fn blood_vector()->Vec3 {let r=||(rnd()*256.0).floor()/16.0;Vec3::new(r()-r(),r(),r()-r())}
/// NPC/player hit puffs (cshell 0x100053f0): krew1..3 each with a random velocity (0x10005360 x 0.75), krew4 one time in sixteen.
fn blood_puffs(commands:&mut Commands,fx:&Effects,point:Vec3) {
    for name in ["krew1","krew2","krew3"] {
        let vel=blood_vector()*0.75;
        fx::spawn(commands,puff(fx,name,point+vel*0.04,vel));
    }
    // krew4 (0x10005763): no motion flag, no growth, no gravity: a still splat 16 units above the hit, scale 0.05, alpha 0.8.
    if rnd()<1.0/16.0 {fx::spawn(commands,Particle::sprite(&fx.lib,"sprites/krew4.spr",point+Vec3::Y*16.0).map(|p|p.scale(0.05).life(1.0).alpha(0.8).normal_alpha()));}
}
/// One gore bit (0x100057fc, 0x10005a6e): krew_dodatki sprite N = rand % 11 + 1, scale 0.27, life 0.8 + (rand & 255) * 0.6/256, velocity 0x10005360 x 3, gravity mode 2 (240),
/// normal alpha, no fade; an odd rand makes it the floor-stain variant (type 0x23).
fn gore_bit(commands:&mut Commands,fx:&Effects,point:Vec3) {
    let vel=blood_vector()*3.0;let life=0.8+(rnd()*256.0).floor()*0.00234375;
    let bit=Particle::sprite(&fx.lib,&format!("sprites/krew_dodatki/{}.spr",1+pick(11)),point).map(|p|p.scale(0.27).life(life).vel(vel).gravity(240.0).normal_alpha());
    fx::spawn(commands,bit.map(|p|if rnd()<0.5 {p.leaving_pool()}else{p}));
}
/// Gore within 72 units of the player: 22 krew_dodatki bits (only with the shot debris option, 0x10005768).
fn gore(commands:&mut Commands,fx:&Effects,point:Vec3) {
    if !fx.debris {return;}
    for _ in 0..22 {gore_bit(commands,fx,point);}
}
/// A bullet into the lower part of a corpse (cshell 0x10005920, reached from the trace 0x1000686b when the hit is below the actor's centre minus half its half height):
/// one krew1 puff, six gore bits and three `miecho01..03` pieces (type 0x24: scale 0.7, life 0.8..1.4, velocity 0x10005360 x 3 with x and z doubled again by Spawn, gravity mode 3 (96), spin 6.283 rad/s about Y).
pub fn corpse_leg_blood(commands:&mut Commands,fx:&Effects,point:Vec3) {
    let vel=blood_vector()*0.75;
    fx::spawn(commands,Particle::sprite(&fx.lib,"sprites/krew1.spr",point+vel*0.04).map(|p|p.scale(0.05).life(1.0).growth(1.2).vel(vel).gravity(96.0).alpha(0.8).normal_alpha()));
    if fx.debris {for _ in 0..6 {gore_bit(commands,fx,point);}}
    for _ in 0..3 {
        let mut vel=blood_vector()*3.0;vel.x*=2.0;vel.z*=2.0;let life=0.8+(rnd()*256.0).floor()*0.00234375;
        fx::spawn(commands,Particle::model(&fx.lib,&format!("models/levelowe/kawalki/miecho0{}.ltb",1+pick(3)),point).map(|p|p.scale(0.7).life(life).vel(vel).gravity(96.0).spinning(6.283185)));
    }
}
/// A `krewpodloga1..4` spot on the floor (cshell 0x10009300 family).
pub fn floor_pool(commands:&mut Commands,fx:&Effects,point:Vec3,life:f32,scale:f32) {
    fx::spawn(commands,Particle::sprite(&fx.lib,&format!("sprites/krewpodloga{}.spr",1+pick(4)),point+Vec3::Y*0.5).map(|p|p.flat(Quat::from_rotation_arc(Vec3::Z,Vec3::Y)).normal_alpha().scale(scale).life(life)));
}
/// Splat on the wall behind the victim (cshell 0x10009300): a ray 32..256 units past the hit, vertical walls only.
fn wall_splat(commands:&mut Commands,fx:&Effects,world:&retail_movement::CollisionWorld,point:Vec3,direction:Vec3,large:bool) {
    let length=between(32.0,256.0);
    let Some(ray)=(direction*length+Vec3::new(signed(16.0),signed(16.0),signed(16.0))).try_normalize() else {return};
    let native=|v:Vec3|retail_movement::Vec3::new(v.x,v.y,v.z);
    let Some((distance,normal))=world.raycast(native(point),native(ray),length) else {return};
    let normal=Vec3::new(normal.x,normal.y,normal.z);
    if normal.y.abs()>0.3 {return;}
    let normal=if normal.dot(ray)>0.0 {-normal}else{normal};
    let (path,scale,life)=if large && rnd()<0.5 {(format!("sprites/bryzg{}.spr",["","2"][pick(2)]),0.2,60.0)}
        else{(format!("sprites/bryzgmaly{}.spr",["","1","2"][pick(3)]),if large {0.2}else{0.08},120.0)};
    fx::spawn(commands,Particle::sprite(&fx.lib,&path,point+ray*distance+normal*between(1.4,1.65)).map(|p|p.facing(normal).normal_alpha().alpha(0.9).scale(scale).life(life)));
}
/// Dead-NPC pool `sladkrwipoziom.spr` flat at the feet: hidden for 4 s, grows until 14 s, lives 180 s (cshell 0x100543b0).
fn dead_pool(commands:&mut Commands,fx:&Effects,world:&retail_movement::CollisionWorld,position:Vec3) {
    let native=|v:Vec3|retail_movement::Vec3::new(v.x,v.y,v.z);
    let from=position+Vec3::Y*16.0;
    let Some((distance,normal))=world.raycast(native(from),native(Vec3::NEG_Y),200.0) else {return};
    if normal.y<0.5 {return;}
    fx::spawn(commands,Particle::sprite(&fx.lib,"sprites/sladkrwipoziom.spr",from-Vec3::Y*(distance-0.5)).map(|p|p.flat(Quat::from_rotation_arc(Vec3::Z,Vec3::Y)).normal_alpha().delay(4.0).life(180.0).pooling(0.04)));
}

/// Impact of a bullet on the world (cshell 0x10005ce0): hit sprite, light, ricochet, `hit_smuga`, bullet hole and debris by SurfaceFlags.
/// Returns the hole entity so the caller can bind it to a door. Weapons without `hit_sprite` (melee) leave nothing.
fn impact(commands:&mut Commands,fx:&Effects,weapon:&[(String,String)],point:Vec3,normal:Vec3,flags:u8)->Option<Entity> {
    let sprite=cv(weapon,"hit_sprite")?;
    fx::spawn(commands,Particle::sprite(&fx.lib,sprite,point+normal*2.0).map(|p|p.scale(cn(weapon,"hit_sprite_skala").unwrap_or(0.05)).life(0.6)));
    fx::light(commands,point+normal*4.0,[0.25,0.2,0.15],32.0,0.1);
    ricochet(commands,fx,point);
    for _ in 0..cn(weapon,"hit_smuga").unwrap_or(0.0) as usize {
        let vel=Vec3::new(signed(32.0),between(-16.0,32.0),signed(32.0))*10.0;
        fx::spawn(commands,Particle::sprite(&fx.lib,"sprites/ogon.spr",point+normal*2.0).map(|p|p.scale2(0.05,0.5).life(between(0.3,0.6)).vel(vel).gravity(240.0).streak()));
    }
    let mut smoke=false;
    let (rubble,wood,sparks)=match if fx.debris {debris_for(flags)}else{Debris::Nothing} {Debris::Rubble(n)=>(n,0,0),Debris::RubbleSmoke(n)=>{smoke=true;(n,0,0)},Debris::Wood(n)=>(0,n,0),Debris::Sparks(n)=>(0,0,n),Debris::Nothing=>(0,0,0)};
    let outward=|direction:Vec3|if direction.dot(normal)<0.0 {direction-normal*direction.dot(normal)*2.0}else{direction};
    for (count,model,low,high) in [(rubble,"models/levelowe/kawalki/gruz0",1,4),(wood,"models/levelowe/kawalki/drewienko0",1,3)] {
        for _ in 0..count {
            let direction=outward(fx::debris_direction());
            fx::spawn(commands,Particle::model(&fx.lib,&format!("{model}{}.ltb",low+pick(high)),point+direction*2.0).map(|p|p.scale(between(0.2,0.6)).life(between(0.6,0.9)).vel(direction*160.0).gravity(240.0).tumbling(8.0)));
        }
    }
    for _ in 0..sparks {
        let direction=outward(fx::debris_direction());
        fx::spawn(commands,Particle::sprite(&fx.lib,"sprites/iskra.spr",point+direction*2.0).map(|p|p.scale2(0.1,0.2).life(0.5).vel(direction*80.0).streak().fade()));
    }
    if smoke {fx::spawn(commands,Particle::sprite(&fx.lib,"sprites/dymek.spr",point+normal*2.0).map(|p|p.scale(0.3).life(0.5).growth(2.5).vel(normal*64.0).fade()));}
    let hole=match if fx.debris {mark_for(flags,pick(3))}else{Mark::Nothing} {
        Mark::Hole(index)=>decal(fx,&format!("textures/sprajty/dziura{index}.dtx"),point+normal*between(0.4,0.65),normal,12.0,5.0),
        Mark::Scorch=>decal(fx,"textures/sprajty/slad.dtx",point+normal*between(0.4,0.65),normal,6.0,5.0),
        Mark::Nothing=>None,
    };
    fx::spawn(commands,hole)
}

/// Bullet impact on a prop (cshell 0x10007e1c..0x1000887f): hit sprite, light and ricochet as on the world, then the debris the
/// definition asks for: 6 sparks (`iskry_przy_trafieniu`), 8 rubble (`gruz_`), 8 wood splinters (`drewno_`), 4 paper scraps (`papier_`).
/// There is no bullet hole on an object.
pub(crate) fn prop_impact(commands:&mut Commands,fx:&Effects,weapon:&[(String,String)],point:Vec3,normal:Vec3,effects:u8) {
    if let Some(sprite)=cv(weapon,"hit_sprite") {
        fx::spawn(commands,Particle::sprite(&fx.lib,sprite,point+normal*2.0).map(|p|p.scale(cn(weapon,"hit_sprite_skala").unwrap_or(0.05)).life(0.6)));
        fx::light(commands,point+normal*4.0,[0.25,0.2,0.15],32.0,0.1);
        ricochet(commands,fx,point);
    }
    let outward=|direction:Vec3|if direction.dot(normal)<0.0 {direction-normal*direction.dot(normal)*2.0}else{direction};
    if effects&crate::props::HIT_SPARKS!=0 {for _ in 0..6 {
        let direction=outward(fx::debris_direction());
        fx::spawn(commands,Particle::sprite(&fx.lib,"sprites/iskra.spr",point+direction*2.0).map(|p|p.scale2(0.1,0.2).life(0.5).vel(direction*80.0).streak().fade()));
    }}
    for (flag,count,model,pieces,life) in [(crate::props::HIT_RUBBLE,8,"models/levelowe/kawalki/gruz0",4,0.6),(crate::props::HIT_WOOD,8,"models/levelowe/kawalki/drewienko0",3,0.4)] {
        if effects&flag==0 {continue;}
        for _ in 0..count {
            let direction=outward(fx::debris_direction());
            fx::spawn(commands,Particle::model(&fx.lib,&format!("{model}{}.ltb",1+pick(pieces)),point+direction*2.0).map(|p|p.scale(between(0.2,0.6)).life(life).vel(direction*160.0).gravity(240.0).tumbling(12.566)));
        }
    }
    if effects&crate::props::HIT_PAPER!=0 {for _ in 0..4 {
        let direction=outward(fx::debris_direction());
        fx::spawn(commands,Particle::model(&fx.lib,&format!("models/levelowe/kawalki/papier0{}.ltb",1+pick(3)),point+direction*2.0).map(|p|p.scale(0.5).life(0.7).vel(direction*96.0).gravity(240.0).tumbling(12.566)));
    }}
}

/// Where a shot is heard from: a melee swing 64 units ahead of the camera (cshell 0x10003f83), a bullet at the weapon's view offset from the camera
/// (`przes_right` / `przes_up` / `przes_forward` added to the eye, 0x10005033..0x100050b2; the muzzle flash is placed from the same point). Native units.
pub fn shot_position(melee:bool,camera:&Transform,offset:[f32;3])->Vec3 {
    if melee {camera.translation/SCALE+*camera.forward()*64.0}
    else {crate::mirror::to_world(camera,Vec3::new(offset[0],offset[1],-offset[2])*SCALE)/SCALE}
}
/// The 640-radius shot sound (`sound_shoot`, S3D) at `shot_position`.
pub fn shot_sound(commands:&mut Commands,fx:&Effects,d:&crate::retail_state::Definition,camera:&Transform) {
    if let Some(Some(path))=d.sounds.get("shoot") {sound::play_at(commands,&fx.assets,path,shot_position(d.melee,camera,d.offset),640.0);}
}
/// Muzzle presentation of a player shot: flashes on the first-person sockets, light, smoke, sparks and the ejected casing (cshell 0x10003f90).
pub fn player_shot(commands:&mut Commands,fx:&Effects,native:&crate::retail_weapons::NativeArsenal,slot:usize,camera:&Transform,gun:&crate::view::ViewState) {
    let weapon=fx.weapon(&native.definitions[slot].id);
    let (forward,right,up)=(*camera.forward(),crate::mirror::right(camera),*camera.up());
    let world=|view:Vec3|crate::mirror::to_world(camera,view)/SCALE;
    let mut muzzle=None;
    for index in 0..3 {
        let (Some(path),Some(socket))=(cv(weapon,&format!("sprite{index}")),cv(weapon,&format!("socket_blik{index}"))) else {continue};
        let path=match cn(weapon,"ile_sprite0") {Some(count) if index==0 && count>=2.0=>fx::variant(path,pick(count as usize)),_=>path.to_string()};
        let Some(view)=native.socket_view(slot,socket,gun) else {continue};
        let scale=cn(weapon,&format!("skala_sprite{index}")).unwrap_or(0.05);
        fx::spawn(commands,Particle::sprite(&fx.lib,&path,view/SCALE).map(|p|p.in_view().following(socket).scale(scale).life(0.12)));
        if index==0 {muzzle=Some(world(view));}
    }
    let Some(muzzle)=muzzle else {return};
    fx::light(commands,muzzle,[0.95,0.85,0.45],192.0,0.07);
    if let Some(path)=cv(weapon,"dym_po_strzale_gracza") {
        for _ in 0..cn(weapon,"ilosc_dymu_po_strzale_gracza").unwrap_or(1.0) as usize {
            let vel=(forward+right*signed(0.25)+up*signed(0.25))*256.0;
            fx::spawn(commands,Particle::sprite(&fx.lib,path,muzzle).map(|p|p.scale(cn(weapon,"skala_dymu_po_strzale_gracza").unwrap_or(0.3)).life(cn(weapon,"czas_dymu_po_strzale_gracza").unwrap_or(0.5))
                .growth(cn(weapon,"zwiekszanie_dymu_po_strzale_gracza").unwrap_or(2.5)).vel(vel).fade()));
        }
    }
    sparks(commands,fx,weapon,"gracza",muzzle,forward,right,up,0.125);
}
/// The empty case of the last shot leaves `socket_luska_strzal0` (cshell 0x10009ea0), not at the trigger pull but when the first shoot clip ends
/// or the next shot interrupts it (`TickOutcome::eject_casing`): velocity `F·(48..96) + R·(64..128) + U·(32..64)`, 10 s, bounces.
pub fn player_casing(commands:&mut Commands,fx:&Effects,native:&crate::retail_weapons::NativeArsenal,slot:usize,camera:&Transform,gun:&crate::view::ViewState) {
    let weapon=fx.weapon(&native.definitions[slot].id);
    let (forward,right,up)=(*camera.forward(),crate::mirror::right(camera),*camera.up());
    if let Some(view)=cv(weapon,"socket_luska_strzal0").and_then(|socket|native.socket_view(slot,socket,gun)) {
        let shell=if cn(weapon,"kul_na_raz").unwrap_or(0.0)>=2.0 {"models/misc/luska_do_obrzyna.ltb"}else{"models/misc/luska.ltb"};
        let vel=forward*between(48.0,96.0)+right*between(64.0,128.0)+up*between(32.0,64.0);
        fx::spawn(commands,Particle::model(&fx.lib,shell,crate::mirror::to_world(camera,view)/SCALE).map(|p|p.scale(0.6).life(10.0).vel(vel).gravity(640.0).bouncing(0.35,Some("sounds/weapons/luska1.wav")).tumbling(12.0)));
    }
}
/// The revolver's six empty casings fall out of the open cylinder (cshell 0x1000a1d0): one `models\misc\luska.ltb` per `socket_luska_reload0..5`,
/// velocity (32..64, -(1 + 32..64), 32..64) in world axes, 15 s, bouncing like every casing.
pub fn reload_casings(commands:&mut Commands,fx:&Effects,native:&crate::retail_weapons::NativeArsenal,slot:usize,camera:&Transform,gun:&crate::view::ViewState) {
    let weapon=fx.weapon(&native.definitions[slot].id);
    for index in 0..6 {
        let Some(view)=cv(weapon,&format!("socket_luska_reload{index}")).and_then(|socket|native.socket_view(slot,socket,gun)) else {continue};
        let vel=Vec3::new(between(32.0,64.0),-(1.0+between(32.0,64.0)),between(32.0,64.0));
        fx::spawn(commands,Particle::model(&fx.lib,"models/misc/luska.ltb",crate::mirror::to_world(camera,view)/SCALE).map(|p|p.scale(0.6).life(15.0).vel(vel).gravity(640.0).bouncing(0.35,Some("sounds/weapons/luska1.wav")).tumbling(12.0)));
    }
}
/// `iskry_po_strzale_*` muzzle sparks: velocity (F + a*R + b*U) * speed, streaks that fade (cshell 0x100047af).
#[allow(clippy::too_many_arguments)]
fn sparks(commands:&mut Commands,fx:&Effects,weapon:&[(String,String)],who:&str,at:Vec3,forward:Vec3,right:Vec3,up:Vec3,spread:f32) {
    for _ in 0..cn(weapon,&format!("iskry_po_strzale_{who}")).unwrap_or(0.0) as usize {
        let vel=(forward+right*signed(spread)+up*signed(spread))*cn(weapon,&format!("predkosc_iskier_po_strzale_{who}")).unwrap_or(500.0);
        fx::spawn(commands,Particle::sprite(&fx.lib,"sprites/iskra.spr",at).map(|p|p.scale(cn(weapon,&format!("skala_iskier_po_strzale_{who}")).unwrap_or(0.03)).life(cn(weapon,&format!("czas_iskier_po_strzale_{who}")).unwrap_or(0.15)).vel(vel).streak().fade()));
    }
}

/// Shared world query and presentation for a bullet: `weapon` is the item id whose keys drive the effects.
#[allow(clippy::too_many_arguments)]
pub fn shoot(commands:&mut Commands,world:&retail_movement::CollisionWorld,npcs:&mut crate::npcs::NpcRoster,doors:&Query<(Entity,&crate::doors::Door)>,surfaces:&crate::audio::Surfaces,effects:&Effects,weapon:&str,origin:Vec3,direction:Vec3,range:f32,damage:f32,tracer:bool)->bool {
    let outcome=shoot_outcome(commands,world,npcs,doors,surfaces,effects,weapon,origin,direction,range,&|_|damage,tracer);
    outcome.hit || outcome.world_hit
}

/// `origin` is a render-space camera position; `damage` maps the native-unit hit distance to damage (falloff).
#[allow(clippy::too_many_arguments)]
pub(crate) fn shoot_outcome(commands:&mut Commands,world:&retail_movement::CollisionWorld,npcs:&mut crate::npcs::NpcRoster,doors:&Query<(Entity,&crate::doors::Door)>,surfaces:&crate::audio::Surfaces,effects:&Effects,weapon:&str,origin:Vec3,direction:Vec3,range:f32,damage:&dyn Fn(f32)->f32,tracer:bool)->ShotOutcome {
    let eye=origin/SCALE;let keys=effects.weapon(weapon);
    let hit=world_hit(world,doors,surfaces,eye,direction,range);
    // Props are objects like characters: the nearest of world, prop and character takes the bullet.
    let prop=effects.props.field().ray(eye,direction,hit.as_ref().map_or(range,|h|h.distance));
    let prop_limit=prop.map_or(hit.as_ref().map_or(range,|h|h.distance),|p|p.0);
    // A corpse's lowest quarter takes the bullet (no damage, blood splash); the upper part of its box lets it through (cshell 0x10006836..0x10006898).
    let corpse=npcs.corpse_stop(eye,direction,prop_limit);
    let npc_hit=npcs.hit_scan_with(eye,direction,corpse.unwrap_or(prop_limit),damage,world);
    let prop=prop.filter(|p|npc_hit.as_ref().is_none_or(|n|p.0<n.distance) && corpse.is_none_or(|c|p.0<c));
    let corpse=corpse.filter(|c|npc_hit.as_ref().is_none_or(|n|*c<n.distance));
    let distance=npc_hit.as_ref().map(|h|h.distance).or(corpse).or_else(||prop.map(|p|p.0)).or_else(||hit.as_ref().map(|h|h.distance)).unwrap_or(range);
    let end=eye+direction*distance;
    // The corpse branch returns before the impact stimulus and the hit handlers (0x10009224): only the splash is made.
    if let (Some(_),None,None)=(corpse,&npc_hit,&prop) {
        if tracer && distance>320.0 {self::tracer(commands,effects,eye+direction*1400.0,direction);}
        corpse_leg_blood(commands,effects,end);
        return ShotOutcome {world_hit:true,end:end*SCALE,..default()};
    }
    npcs.add_noise(crate::npcs::Stimulus::impact(end));
    if tracer && distance>320.0 {self::tracer(commands,effects,eye+direction*1400.0,direction);}
    if let (Some((_,index,normal)),None)=(prop,&npc_hit) {
        let mut field=effects.props.field();field.hit(index,damage(distance),end);let flags=field.props[index].effects;drop(field);
        prop_impact(commands,effects,keys,end,normal,flags);
        return ShotOutcome {world_hit:true,end:end*SCALE,..default()};
    }
    if let Some(victim)=&npc_hit {
        ricochet(commands,effects,end);
        if victim.bleeds {
            blood_puffs(commands,effects,end);
            if end.distance(eye)<72.0 {gore(commands,effects,end);}
            if victim.stains {wall_splat(commands,effects,world,end,direction,has(keys,"duzy_bryzg"));}
        }
    }else if let Some(hit)=&hit {
        let normal=if hit.normal.dot(direction)>0.0 {-hit.normal}else{hit.normal};
        if let Some(entity)=impact(commands,effects,keys,end,normal,hit.flags) {
            if let Some(door)=hit.door.and_then(|entity|doors.get(entity).ok()) {
                let (rotation,translation)=door.1.pose();
                commands.entity(entity).insert(DoorImpact {door:door.0,point:rotation.conjugate()*(end-translation),normal:rotation.conjugate()*normal});
            }
        }
    }
    npc_hit.map(|victim|ShotOutcome {hit:true,killed:victim.killed,name:victim.name,world_hit:false,end:end*SCALE})
        .unwrap_or(ShotOutcome {world_hit:hit.is_some(),end:end*SCALE,..default()})
}

/// Enemy shots (cshell 0x100462e0): flash at the NPC weapon socket, light, smoke, sparks, tracer and casing, then the bullet's impact
/// (identical to the player's) or, when it hits the player, the wound; also blood pools of freshly killed actors.
#[allow(clippy::too_many_arguments)]
fn npc_effects(mut commands:Commands,mut roster:ResMut<crate::npcs::NpcRoster>,effects:Res<Effects>,surfaces:Res<crate::audio::Surfaces>,walking:Res<Walking>,doors:Query<(Entity,&crate::doors::Door)>,camera:Single<&Transform,With<InspectionCamera>>) {
    for (position,stains) in roster.drain_deaths() {if stains {dead_pool(&mut commands,&effects,&walking.world,position);}}
    for shot in roster.drain_shots() {
        if shot.melee {player_hit(&mut commands,&effects,&camera,shot.origin,if shot.bite {Grunt::Silent}else{Grunt::Shove});continue;}
        let weapon=&shot.commands;
        let Some(direction)=(shot.endpoint-shot.origin).try_normalize() else {continue};
        let reach=shot.endpoint.distance(shot.origin);
        let right=direction.cross(Vec3::Y).try_normalize().unwrap_or(Vec3::X);let up=right.cross(direction);
        // The muzzle is shown once per shot; a shotgun's later pellets only add their bullet path and impact.
        if !shot.extra {
        if let Some(path)=cv(weapon,"sprite_lezacy_oko_0") {
            let scale=cn(weapon,"skala_sprite_lezacy_oko_0").filter(|scale|*scale>0.0).unwrap_or(1.0);
            fx::spawn(&mut commands,Particle::sprite(&effects.lib,path,shot.origin).map(|p|p.scale(scale).life(0.25)));
        }
        fx::light(&mut commands,shot.origin,[0.95,0.85,0.45],128.0,0.07);
        if let Some(path)=cv(weapon,"dym_po_strzale_wroga") {
            fx::spawn(&mut commands,Particle::sprite(&effects.lib,path,shot.origin).map(|p|p.scale(cn(weapon,"skala_dymu_po_strzale_wroga").unwrap_or(0.3)).life(cn(weapon,"czas_dymu_po_strzale_wroga").unwrap_or(0.5))
                .growth(0.5).vel((direction+Vec3::Y*0.25)*64.0).fade()));
        }
        sparks(&mut commands,&effects,weapon,"wroga",shot.origin,direction,right,up,0.125);
        }
        if let Some(casing)=shot.casing {
            let shell=if cn(weapon,"kul_na_raz").unwrap_or(0.0)>=2.0 {"models/misc/luska_do_obrzyna.ltb"}else{"models/misc/luska.ltb"};
            let vel=Vec3::new(signed(64.0),between(32.0,64.0),signed(64.0));
            fx::spawn(&mut commands,Particle::model(&effects.lib,shell,casing).map(|p|p.scale(0.75).life(5.0).vel(vel).gravity(640.0).bouncing(0.35,Some("sounds/weapons/luska1.wav")).tumbling(12.0)));
        }
        if shot.victim {
            // Another character stood in the way: it bleeds where the bullet struck.
            if reach>128.0 {tracer(&mut commands,&effects,shot.origin+direction*0.4*reach,direction);}
            blood_puffs(&mut commands,&effects,shot.endpoint);
        } else if shot.hits_player {
            if reach>128.0 {tracer(&mut commands,&effects,shot.origin+direction*0.4*reach,direction);}
            player_hit(&mut commands,&effects,&camera,shot.origin,Grunt::Bullet(shot.endpoint));
        } else {
            let hit=world_hit(&walking.world,&doors,&surfaces,shot.origin,direction,reach);
            // A stray bullet also damages the prop it strikes with the weapon's own strength (the same trace, cshell 0x10005ce0).
            let prop=effects.props.field().ray(shot.origin,direction,hit.as_ref().map_or(reach,|h|h.distance));
            if reach>128.0 {tracer(&mut commands,&effects,shot.origin+direction*0.4*prop.map(|p|p.0).or_else(||hit.as_ref().map(|h|h.distance)).unwrap_or(reach),direction);}
            if let Some((distance,index,normal))=prop {
                let mut field=effects.props.field();field.hit(index,cn(weapon,"sila_wroga").or_else(||cn(weapon,"sila_strzalu")).unwrap_or(0.0),shot.origin+direction*distance);let flags=field.props[index].effects;drop(field);
                prop_impact(&mut commands,&effects,weapon,shot.origin+direction*distance,normal,flags);
            } else if let Some(hit)=hit {
                let normal=if hit.normal.dot(direction)>0.0 {-hit.normal}else{hit.normal};
                impact(&mut commands,&effects,weapon,shot.origin+direction*hit.distance,normal,hit.flags);
            }
        }
    }
}
/// The player is hit (cshell 0x10033a64, 0x1003b100): grunt, blood puffs and the krewmonitor overlay rolled toward the attacker.
/// The sound of the player's wound: a bullet plays `speech\hero\wcialo.wav` in 3D (radius 640) where it struck (cshell 0x10006a50, the trace's
/// `bl` = the struck object is the player), a shove plays `weapons\wcialo.wav` in 2D (0x10042486), a bite makes none (0x100422e2).
#[derive(Clone,Copy,Debug,PartialEq)] pub enum Grunt {Bullet(Vec3),Shove,Silent}
pub const GRUNT_RADIUS:f32=640.0;
pub(crate) fn player_hit(commands:&mut Commands,fx:&Effects,camera:&Transform,attacker:Vec3,grunt:Grunt) {
    match grunt {
        Grunt::Bullet(at)=>sound::play_at(commands,&fx.assets,"sounds/speech/hero/wcialo.wav",at,GRUNT_RADIUS),
        Grunt::Shove=>sound::play_2d(commands,&fx.assets,"sounds/weapons/wcialo.wav"),
        Grunt::Silent=>{},
    }
    let eye=camera.translation/SCALE;
    let toward=(attacker-eye).try_normalize().unwrap_or(Vec3::Z);
    blood_puffs(commands,fx,eye+toward*24.0-Vec3::Y*8.0);
    let Some(sprite)=fx.lib.sprite("sprites/krewmonitor.spr").and_then(|s|s.frames.first().cloned()) else {return};
    // 8 units ahead at scale 0.04 the 256 px sprite is 20.5 units wide: about 1.5 screen widths.
    let roll=overlay_roll(camera.rotation.inverse()*(attacker*SCALE-camera.translation));
    commands.spawn((BloodOverlay {age:0.0},ImageNode {image:sprite,..default()},GlobalZIndex(4),UiTransform {rotation:Rot2::radians(roll),..default()},
        Node {position_type:PositionType::Absolute,left:Val::Vw(-24.5),top:percent(50),margin:UiRect::top(Val::Vw(-74.5)),width:Val::Vw(149.0),height:Val::Vw(149.0),..default()}));
}
fn blood_overlay(mut commands:Commands,time:Res<Time>,session:Res<Session>,mut overlays:Query<(Entity,&mut BloodOverlay,&mut ImageNode)>) {
    if session.paused {return;}
    for (entity,mut overlay,mut image) in &mut overlays {
        overlay.age+=time.delta_secs();
        if overlay.age>=1.0 {commands.entity(entity).despawn();}else{image.color=Color::srgba(1.0,1.0,1.0,1.0-overlay.age);}
    }
}

#[cfg(test)]
mod tests {
    /// cshell 0x10005360: y in 0..16, x and z triangular in -16..16 (r/16 - r/16 with r a byte).
    #[test] fn the_hit_blood_vector_follows_the_retail_ranges() {
        for _ in 0..500 {let v=blood_vector();assert!((0.0..16.0).contains(&v.y) && v.x.abs()<16.0 && v.z.abs()<16.0);}
    }
    use super::*;
    #[test]
    fn decal_alpha_is_inverted_so_the_crater_centre_is_opaque() {
        let mut pixels=[30,30,30,255,5,5,5,0];
        invert_decal_alpha(&mut pixels);
        assert_eq!(pixels[3],0);
        assert_eq!(pixels[7],255);
    }
    #[test]
    fn surface_flags_choose_bullet_marks_and_debris() {
        assert_eq!(mark_for(6,4),Mark::Hole(1));assert_eq!(mark_for(0,0),Mark::Scorch);assert_eq!(mark_for(2,0),Mark::Scorch);assert_eq!(mark_for(1,0),Mark::Nothing);
        assert_eq!(debris_for(0),Debris::Rubble(4));assert_eq!(debris_for(6),Debris::RubbleSmoke(6));assert_eq!(debris_for(2),Debris::Sparks(5));assert_eq!(debris_for(3),Debris::Wood(5));
        assert_eq!(debris_for(1),Debris::Nothing);assert_eq!(debris_for(5),Debris::Nothing);
    }
    #[test]
    fn item_keys_read_text_numbers_and_flags() {
        let keys=vec![("hit_sprite".to_string(),"sprites\\weapons\\hit0.spr".to_string()),("hit_sprite_skala".into(),"0.09".into()),("duzy_bryzg".into(),String::new())];
        assert_eq!(cv(&keys,"hit_sprite"),Some("sprites\\weapons\\hit0.spr"));assert_eq!(cn(&keys,"hit_sprite_skala"),Some(0.09));
        assert!(has(&keys,"duzy_bryzg") && cv(&keys,"duzy_bryzg").is_none());assert!(!has(&keys,"hit_smuga"));
    }
    #[test]
    fn blood_overlay_rolls_toward_the_attacker() {
        assert!(overlay_roll(Vec3::NEG_Z).abs()<1e-6);
        assert!((overlay_roll(Vec3::X*crate::mirror::SIGN)-std::f32::consts::FRAC_PI_2).abs()<1e-6);
        assert!((overlay_roll(Vec3::Z).abs()-std::f32::consts::PI).abs()<1e-6);
    }
    #[test]
    fn a_shot_is_heard_from_the_weapon_a_swing_from_64_units_ahead() {
        let camera=Transform::from_translation(Vec3::new(1000.0,2000.0,3000.0)*SCALE);
        // The nightstick swing: 64 units along the view direction (cshell 0x10003f83).
        let swing=shot_position(true,&camera,[0.0,0.0,0.0]);
        assert!((swing-(Vec3::new(1000.0,2000.0,3000.0)+*camera.forward()*64.0)).length()<1e-2);
        // A bullet: the eye plus the weapon's view offset (przes_right, przes_up, przes_forward), a few units from the camera.
        let gun=shot_position(false,&camera,[-3.6,-12.5,0.6]);
        let local=(gun-Vec3::new(1000.0,2000.0,3000.0)).abs();
        assert!((local.length()-(3.6f32.powi(2)+12.5f32.powi(2)+0.6f32.powi(2)).sqrt()).abs()<1e-2,"{local:?}");
        assert!(((gun-Vec3::new(1000.0,2000.0,3000.0)).dot(*camera.forward())-0.6).abs()<1e-2,"forward offset");
        assert!((gun.y-(2000.0-12.5)).abs()<1e-2,"up offset");
    }
}
