//! Destructible and solid `o_obiekt` props (docs/retail-props.md). The client prop manager in cshell.dll (0x1002b2d0..0x1002f750)
//! parses scripts\objects.txt, creates one model object per scene prop, takes bullet damage from the weapon trace (0x1002bf50 called
//! at 0x10007f2a) and runs the death effects (0x1002e690): sound, `death_podmien` wreck, explosion and debris.
use bevy::{prelude::*,ecs::system::SystemParam};
use serde::Deserialize;
use std::{collections::BTreeMap,path::Path,sync::{Arc,Mutex}};
use crate::{SCALE,fx::{Particle,pick,rnd},gunfire::Effects};

/// `death_*` keys OR their bit into the definition's mask at +0x1294 (cshell 0x1002e1fe..0x1002e3a9).
pub const WYBUCH:u32=1;pub const KEEPALIVE:u32=2;
/// Radius of the `death_sound` (0x1002e78f pushes 960.0).
pub const DEATH_SOUND_RADIUS:f32=960.0;

/// One scripts\objects.txt block (parse function cshell 0x1002d496; struct offsets in the comments of `parse_defs`).
#[derive(Clone,Debug,Default,PartialEq)]
pub struct Def {
    pub name:String,pub model:String,pub hp:f32,pub mass:f32,pub solid:bool,pub gravity:bool,pub insignificant:i32,
    /// `przezroczysty_dla_blikow` (user flag 4: glints pass through), `iskry/gruz/drewno/papier_przy_trafieniu` (hit debris).
    pub blik:bool,pub sparks:bool,pub rubble:bool,pub wood:bool,pub paper:bool,
    pub death:u32,pub death_sound:String,pub swap:String,
    pub anim0:String,pub anim1:String,pub anim01:String,pub anim10:String,pub anim_raz:String,pub sound01:String,pub sound10:String,
    pub styles:BTreeMap<String,String>,
}
/// Debris `death_*` masks: ceramika 4, ceramika_malo 8, blacha 0x10, blacha_malo 0x20, blacha_duzo 0x40, deski 0x80, deski_malo 0x100, prety 0x200, prety_malo 0x400.
fn death_bit(key:&str)->u32 {
    match key {"death_wybuch"=>WYBUCH,"death_keepalive"=>KEEPALIVE,"death_ceramika"=>0x4,"death_ceramika_malo"=>0x8,"death_blacha"=>0x10,"death_blacha_malo"=>0x20,"death_blacha_duzo"=>0x40,
        "death_deski"=>0x80,"death_deski_malo"=>0x100,"death_prety"=>0x200,"death_prety_malo"=>0x400,_=>0}
}
fn number(text:&str)->f32 {
    let end=text.char_indices().find(|(i,c)|!(c.is_ascii_digit() || *c=='.' || (*i==0 && (*c=='-' || *c=='+')))).map_or(text.len(),|(i,_)|i);
    text[..end].parse().unwrap_or(0.0)
}
pub fn parse_defs(source:&str)->BTreeMap<String,Def> {
    let mut defs=BTreeMap::new();let mut current:Option<Def>=None;
    let mut flush=|current:&mut Option<Def>|{if let Some(def)=current.take() {defs.entry(def.name.clone()).or_insert(def);}};
    for line in source.lines().chain(std::iter::once("object")) {
        let line=line.split("//").next().unwrap_or("").trim();
        if line.is_empty() {continue;}
        let (key,value)=line.split_once(char::is_whitespace).map(|(k,v)|(k,v.trim())).unwrap_or((line,""));
        let key=key.to_ascii_lowercase();
        if key=="object" {flush(&mut current);if !value.is_empty() {current=Some(Def {name:value.into(),..default()});}continue;}
        let Some(def)=current.as_mut() else{continue};
        let text=value.to_owned();
        match key.as_str() {
            "model"=>def.model=text,"hp"=>def.hp=number(value),"mass"=>def.mass=number(value),"solid"=>def.solid=true,"gravity"=>def.gravity=true,
            // +0x1298: 1 for the bare key, or atoi(value) when that is non-zero.
            "insignificant"=>{def.insignificant=1;let n=number(value) as i32;if n!=0 {def.insignificant=n;}},
            "przezroczysty_dla_blikow"=>def.blik=true,"iskry_przy_trafieniu"=>def.sparks=true,"gruz_przy_trafieniu"=>def.rubble=true,
            "drewno_przy_trafieniu"=>def.wood=true,"papier_przy_trafieniu"=>def.paper=true,
            "death_sound"=>def.death_sound=text,"death_podmien"=>def.swap=text,
            "anim0"=>def.anim0=text,"anim1"=>def.anim1=text,"anim01"=>def.anim01=text,"anim10"=>def.anim10=text,"anim_raz"=>def.anim_raz=text,
            "sound01"=>def.sound01=text,"sound10"=>def.sound10=text,
            k if k.len()==3 && k.starts_with("rs") && k.as_bytes()[2].is_ascii_digit()=>{def.styles.insert(k[2..].into(),text);},
            k=>def.death|=death_bit(k),
        }
    }
    defs
}

/// Exported art of one definition (tools/export_props.py `definitions` -> props_defs.json).
#[derive(Clone,Default,Deserialize)]
pub struct DefAssets {pub model:String,#[serde(default)]pub skins:BTreeMap<String,Vec<String>>,#[serde(default)]pub swap_skins:BTreeMap<String,Vec<String>>,#[serde(default)]pub fps:BTreeMap<String,f32>}
#[derive(Deserialize,Default)] struct AssetFile {#[serde(default)]definitions:BTreeMap<String,DefAssets>,#[serde(default)]debris:BTreeMap<String,String>}
#[derive(Resource,Default)]
pub struct PropData {pub defs:BTreeMap<String,Def>,pub assets:BTreeMap<String,DefAssets>,pub debris:BTreeMap<String,String>}
impl PropData {
    pub fn load(output:&Path)->Self {
        let defs=parse_defs(&std::fs::read_to_string(output.join("decoded_scripts/objects.txt")).unwrap_or_default());
        let file:AssetFile=std::fs::read_to_string(output.join("props_defs.json")).ok().and_then(|s|serde_json::from_str(&s).ok()).unwrap_or_default();
        Self {defs,assets:file.definitions,debris:file.debris}
    }
}

/// Displacement of a surviving prop per bullet: away from the hit along `origin - point`, `damage * 10 / mass` units long (cshell 0x1002c044..0x1002c08c).
pub fn push_delta(origin:Vec3,point:Vec3,damage:f32,mass:f32)->Vec3 {if mass>0.0 {(origin-point).normalize_or_zero()*(damage*10.0/mass)}else{Vec3::ZERO}}

/// Bit set of the debris a bullet knocks off a prop (`*_przy_trafieniu`).
pub const HIT_SPARKS:u8=1;pub const HIT_RUBBLE:u8=2;pub const HIT_WOOD:u8=4;pub const HIT_PAPER:u8=8;
#[derive(Clone,Debug)]
pub struct PropState {
    pub name:String,pub def:String,pub center:Vec3,pub rotation:[f32;4],pub half:Vec3,pub solid:bool,pub hp:f32,pub dead:bool,pub effects:u8,
    /// The placed position (the death effects and the bullet push direction keep using it) and objects.txt `mass` (0 = no push).
    pub origin:Vec3,pub mass:f32,
    /// Every entity of the instance (body marker, support box, one per model piece).
    pub entities:Vec<Entity>,
}
/// All props of the level: the geometry every trace and every mover tests, plus damage waiting for `props::tick`.
#[derive(Default)]
pub struct PropField {pub props:Vec<PropState>,pub hits:Vec<(usize,f32,Vec3)>,pub kills:Vec<String>}
impl PropField {
    pub fn add(&mut self,state:PropState)->usize {self.props.push(state);self.props.len()-1}
    pub fn find(&self,name:&str)->Option<usize> {self.props.iter().position(|p|!p.name.is_empty() && p.name==name)}
    pub fn is_dead(&self,name:&str)->bool {self.find(name).is_some_and(|i|self.props[i].dead)}
    /// Nearest living prop box on the segment: (distance, index, face normal). Every visible prop stops bullets, solid or not
    /// (small breakables such as wall pictures are `solid`-less yet destructible).
    pub fn ray(&self,origin:Vec3,direction:Vec3,max:f32)->Option<(f32,usize,Vec3)> {
        self.props.iter().enumerate().filter(|(_,p)|!p.dead && p.half.cmpgt(Vec3::ZERO).all())
            .filter_map(|(i,p)|crate::doors::ray_box(origin,direction,p.center-p.half,p.center+p.half,max).map(|d|{
                let local=(origin+direction*d-p.center)/p.half;
                let axis=if local.x.abs()>=local.y.abs() && local.x.abs()>=local.z.abs() {0}else if local.y.abs()>=local.z.abs() {1}else{2};
                let mut normal=Vec3::ZERO;normal[axis]=local[axis].signum();(d,i,normal)
            })).min_by(|a,b|a.0.total_cmp(&b.0))
    }
    /// Does the segment enter the box of a living prop? The engine tests every model object by its dims box whether or not it is solid, and
    /// the sight and bullet filters of cshell (0x10057940, 0x100578a0) let a prop through only when it is an item (user flag 0x40).
    pub fn blocks_segment(&self,a:Vec3,b:Vec3)->bool {
        self.props.iter().any(|p|!p.dead && p.half.cmpgt(Vec3::ZERO).all() && crate::npcs::segment_box_entry(a,b,p.center,p.half).is_some())
    }
    /// Swept box against the solid living props (contact blocks only inward motion; a box that starts inside may leave).
    pub fn sweep(&self,position:Vec3,half:Vec3,delta:Vec3)->Option<(f32,Vec3)> {
        self.props.iter().filter(|p|p.solid && !p.dead).filter_map(|p|sweep_box(position,half,delta,p.center,p.half)).min_by(|a,b|a.0.total_cmp(&b.0))
    }
    pub fn hit(&mut self,index:usize,damage:f32,point:Vec3) {self.hits.push((index,damage,point));}
    /// Melee reaches every prop whose box around the placed position (a shoved prop keeps testing its old box) contains the strike point (cshell 0x1000f829..0x1000fa25).
    pub fn strike(&self,point:Vec3)->Vec<usize> {
        self.props.iter().enumerate().filter(|(_,p)|!p.dead && (point-p.origin).abs().cmple(p.half).all()).map(|(i,_)|i).collect()
    }
    /// Reset for a new level.
    pub fn clear(&mut self) {self.props.clear();self.hits.clear();self.kills.clear();}
}
/// Slab sweep of a moving box against a static one; `None` when it already overlaps (so it can escape) or never touches within the step.
pub fn sweep_box(position:Vec3,half:Vec3,delta:Vec3,center:Vec3,other:Vec3)->Option<(f32,Vec3)> {
    let extent=other+half;let relative=position-center;
    if relative.abs().cmplt(extent).all() {return None;}
    let (mut enter,mut leave)=(f32::NEG_INFINITY,f32::INFINITY);let mut normal=Vec3::ZERO;
    for axis in 0..3 {
        if delta[axis].abs()<1e-6 {if relative[axis].abs()>=extent[axis] {return None;}continue;}
        let (a,b)=((-extent[axis]-relative[axis])/delta[axis],(extent[axis]-relative[axis])/delta[axis]);
        let (near,far)=if a<b {(a,b)}else{(b,a)};
        if near>enter {enter=near;normal=Vec3::ZERO;normal[axis]=if delta[axis]>0.0 {-1.0}else{1.0};}
        leave=leave.min(far);if enter>leave {return None;}
    }
    (enter>=0.0 && enter<=1.0).then_some((enter,normal))
}

#[derive(Resource,Clone,Default)]
pub struct PropWorld(pub Arc<Mutex<PropField>>);
impl PropWorld {
    pub fn field(&self)->std::sync::MutexGuard<'_,PropField> {self.0.lock().unwrap_or_else(|poisoned|poisoned.into_inner())}
}

/// One debris kind of the death handler (cshell 0x1002e86f..0x1002f565): pieces, velocity factor, and the Spawn type's model set (0x1004f906..0x10050f83).
pub struct Debris {pub mask:u32,pub count:usize,pub factor:f32,pub stem:&'static str,pub models:usize,pub scale:f32,pub trail:bool}
pub const DEBRIS:[Debris;9]=[
    Debris {mask:0x4,count:8,factor:8.0,stem:"ceramika",models:4,scale:1.0,trail:true},
    Debris {mask:0x8,count:6,factor:5.0,stem:"ceramika",models:4,scale:0.5,trail:false},
    Debris {mask:0x10,count:8,factor:8.0,stem:"blacha",models:5,scale:0.5,trail:true},
    Debris {mask:0x20,count:6,factor:5.0,stem:"blacha",models:5,scale:0.2,trail:false},
    Debris {mask:0x40,count:8,factor:8.0,stem:"blacha",models:5,scale:1.0,trail:true},
    Debris {mask:0x200,count:8,factor:8.0,stem:"prety",models:5,scale:1.0,trail:true},
    Debris {mask:0x400,count:6,factor:5.0,stem:"prety",models:5,scale:0.5,trail:false},
    Debris {mask:0x80,count:8,factor:8.0,stem:"deski_polamane",models:5,scale:0.7,trail:true},
    Debris {mask:0x100,count:8,factor:5.0,stem:"deski_polamane",models:5,scale:0.25,trail:false},
];
/// Library key of a debris model file.
pub fn debris_path(stem:&str,index:usize)->String {format!("models/levelowe/kawalki/{stem}{:02}.ltb",index+1)}

/// Piece offset from the prop origin, x/z in (-32,32) and y in (-16,32) (`r*0.032 - r*0.032`, `r*0.032 - r*0.016`, cshell 0x1002e86f).
pub fn debris_offset(random:&mut impl FnMut()->f32)->Vec3 {
    Vec3::new((random()-random())*32.0,random()*32.0-random()*16.0,(random()-random())*32.0)
}

/// Death debris of `mask` around `at`: gravity mode 1 (640), 5..15 s, tumbling, bouncing; the big pieces drag an `ogon.spr` streak (until they land).
pub fn burst(commands:&mut Commands,fx:&Effects,at:Vec3,mask:u32) {
    for kind in DEBRIS.iter().filter(|kind|mask&kind.mask!=0) {
        for _ in 0..kind.count {
            let offset=debris_offset(&mut rnd);let velocity=offset*kind.factor;let position=at+offset;
            let life=5.0+(rnd()*1000.0).floor()*0.01;
            crate::fx::spawn(commands,Particle::model(&fx.lib,&debris_path(kind.stem,pick(kind.models)),position).map(|p|p.scale(kind.scale).life(life).vel(velocity).gravity(640.0).bouncing(0.35,None).tumbling(std::f32::consts::TAU)));
            if kind.trail {crate::fx::spawn(commands,Particle::sprite(&fx.lib,"sprites/ogon.spr",position).map(|p|p.scale2(0.25,1.6).life(0.8).vel(velocity).gravity(640.0)));}
        }
    }
}

/// Debris flags a def sets for bullet impacts.
pub fn hit_effects(def:&Def)->u8 {(def.sparks as u8*HIT_SPARKS)|(def.rubble as u8*HIT_RUBBLE)|(def.wood as u8*HIT_WOOD)|(def.paper as u8*HIT_PAPER)}

/// Registers the `kawalki` debris models in the effect library (their skins come from props_defs.json).
pub fn register_debris(mut effects:Option<ResMut<Effects>>,data:Option<Res<PropData>>,config:Res<crate::ViewerConfig>,assets:Res<AssetServer>,mut meshes:ResMut<Assets<Mesh>>,mut materials:ResMut<Assets<StandardMaterial>>) {
    let (Some(effects),Some(data))=(effects.as_mut(),data) else{return};
    if data.debris.is_empty() || effects.lib.models.contains_key(&crate::fx::key(&debris_path("gruz",0))) {return;}
    for (path,skin) in &data.debris {
        let model=crate::models::Model::load(&config.output,path);let pose=model.pose("",0.0,false);
        let material=materials.add(StandardMaterial {base_color_texture:Some(assets.load(level_viewer::hd::path(skin))),unlit:true,cull_mode:None,..default()});
        let parts:Vec<_>=(0..model.pieces.len()).map(|piece|{let mut mesh=model.mesh(piece);model.animate_mesh(piece,&pose,&mut mesh);(meshes.add(mesh),material.clone())}).collect();
        effects.lib.models.insert(crate::fx::key(path),crate::fx::ModelAsset {parts:parts.into()});
    }
}

/// Animated `.spr` skins (the burning wrecks' flame, plomien1.spr 15 fps) advance on the shared level clock.
#[derive(Resource,Default)]
pub struct SkinClock(pub Vec<crate::models::SkinAnim>);
pub fn animate_skins(clock:Res<SkinClock>,time:Res<Time>,session:Res<crate::settings::Session>,mut materials:ResMut<Assets<StandardMaterial>>,mut age:Local<f32>) {
    if session.paused {return;}
    *age+=time.delta_secs();
    for skin in &clock.0 {
        let frame=((*age*skin.fps) as usize)%skin.frames.len().max(1);
        if let (Some(material),Some(image))=(materials.get_mut(&skin.material),skin.frames.get(frame)) {material.base_color_texture=Some(image.clone());}
    }
}

#[derive(SystemParam)]
pub struct Ctx<'w,'s> {
    commands:Commands<'w,'s>,meshes:ResMut<'w,Assets<Mesh>>,materials:ResMut<'w,Assets<StandardMaterial>>,assets:Res<'w,AssetServer>,config:Res<'w,crate::ViewerConfig>,
    world:Res<'w,PropWorld>,data:Option<Res<'w,PropData>>,effects:Option<Res<'w,Effects>>,blast:Option<Res<'w,crate::retail_weapons::BlastAssets>>,
    walking:Res<'w,crate::Walking>,roster:ResMut<'w,crate::npcs::NpcRoster>,campaign:ResMut<'w,crate::campaign::Campaign>,doors:Query<'w,'s,(Entity,&'static crate::doors::Door)>,
    session:Res<'w,crate::settings::Session>,animations:ResMut<'w,crate::models::PropAnimations>,activation:ResMut<'w,crate::activation::Activation>,skins:ResMut<'w,SkinClock>,
    movers:Query<'w,'s,(Option<&'static mut Transform>,Option<&'static mut crate::models::PropBody>,Option<&'static mut crate::models::PropSupport>)>,
}

/// Applies queued bullet damage and `Death_nast_obiekt` kills: a prop whose hit points reach zero runs `die`.
pub fn tick(mut ctx:Ctx) {
    if ctx.session.paused {return;}
    let mut deaths=Vec::new();let mut pushes=Vec::new();
    {
        let mut field=ctx.world.field();
        for (index,damage,point) in std::mem::take(&mut field.hits) {
            let state=&mut field.props[index];
            if state.dead {continue;}
            state.hp-=damage;
            if state.hp<=0.0 {deaths.push(index);}
            // A bullet that does not destroy the prop shoves it `damage*10/mass` units away from the hit (cshell 0x1002c020..0x1002c09b).
            else if state.mass>0.0 {let delta=push_delta(state.origin,point,damage,state.mass);state.center+=delta;pushes.push((index,delta));}
        }
        // Death_nast_obiekt kills the named prop outright (cshell 0x1002e660 passes -HP).
        for name in std::mem::take(&mut field.kills) {if let Some(index)=field.find(&name) {if !field.props[index].dead && !deaths.contains(&index) {deaths.push(index);}}}
    }
    for (index,delta) in pushes {
        let entities=ctx.world.field().props[index].entities.clone();
        for entity in entities {if let Ok((transform,body,support))=ctx.movers.get_mut(entity) {
            if let Some(mut t)=transform {t.translation+=delta*SCALE;}
            if let Some(mut b)=body {b.center+=delta;}
            if let Some(mut s)=support {s.center+=delta;}
        }}
    }
    while let Some(index) = deaths.pop() {
        let next=die(&mut ctx,index);
        for name in next {
            let field=ctx.world.field();
            if let Some(target)=field.find(&name) {if !field.props[target].dead && !deaths.contains(&target) {deaths.push(target);}}
            else {drop(field);ctx.activation.deaths.push_back(name);}
        }
    }
}

/// The client death handler 0x1002e690 for one prop; returns the `Death_nast_obiekt` names still to kill.
fn die(ctx:&mut Ctx,index:usize)->Vec<String> {
    let (state,def)={
        let field=ctx.world.field();let state=field.props[index].clone();
        let def=ctx.data.as_ref().and_then(|d|d.defs.get(&state.def)).cloned().unwrap_or_default();(state,def)
    };
    let keep=def.death&KEEPALIVE!=0;
    {
        let mut field=ctx.world.field();
        // Keepalive props stay in the world at zero health and die again on the next hit.
        if !keep {field.props[index].dead=true;}
    }
    let at=state.center;
    if !def.death_sound.is_empty() {crate::audio::play_near(&mut ctx.commands,&ctx.assets,&def.death_sound,at,DEATH_SOUND_RADIUS);}
    // death_podmien: the wreck replaces the object at the same place (a fresh definition instance, `anim_raz` plays once).
    if !def.swap.is_empty() {swap_in(ctx,&state,&def.swap);}
    if !keep {for entity in &state.entities {ctx.commands.entity(*entity).despawn();}}
    if def.death&WYBUCH!=0 {explode(ctx,at);}
    if let Some(effects)=ctx.effects.as_deref() {burst(&mut ctx.commands,effects,at,def.death);}
    ctx.activation.death_next(&state.name).into_iter().collect()
}

/// Grenade-class explosion at a prop: the type-8 sprites, the 640-unit blast on characters (max 500) and on the player (max 100) (cshell 0x1005b120).
fn explode(ctx:&mut Ctx,center:Vec3) {
    let n=|v:Vec3|retail_movement::Vec3::new(v.x,v.y,v.z);
    let occluded=|origin:Vec3,target:Vec3|{let delta=target-origin;ctx.doors.iter().any(|(_,door)|door.shot_hit(origin,delta.normalize_or_zero(),delta.length()).is_some())};
    let world=&ctx.walking.world;
    ctx.roster.blast(center,world,|origin,target|{let delta=target-origin;ctx.doors.iter().any(|(_,door)|door.shot_hit(origin,delta.normalize_or_zero(),delta.length()).is_some())});
    let player=ctx.walking.player.position;let target=Vec3::new(player.x,player.y,player.z);let distance=target.distance(center);let origin=center+Vec3::Y*32.0;let delta=target-origin;
    if distance<640.0 && world.raycast(n(origin),n(delta.normalize_or_zero()),delta.length()).is_none() && !occluded(origin,target) {
        ctx.campaign.health=(ctx.campaign.health-(640.0-distance)*0.15625*ctx.campaign.stats.pain_factor()).max(0.0);
    }
    if let Some(blast)=ctx.blast.as_deref() {crate::retail_weapons::spawn_blast(&mut ctx.commands,blast,&ctx.assets,center);}
}

/// Spawns the `death_podmien` definition where `state` stood.
fn swap_in(ctx:&mut Ctx,state:&PropState,swap:&str) {
    let Some(data)=ctx.data.as_deref() else{return};
    let (Some(def),Some(assets))=(data.defs.get(swap),data.assets.get(swap)) else{warn!("Missing wreck definition {swap}");return};
    let (position,rotation)=(state.center,state.rotation);
    let name=format!("{}!{}",state.name,swap);
    let idle=if def.anim0.is_empty() {None}else{Some(def.anim0.clone())};
    let instance=crate::models::Instance {name:&name,definition:swap,model:&assets.model,pos:position,rotation,animation:if def.anim0.is_empty() {""}else{&def.anim0},skins:&assets.skins,fps:&assets.fps,styles:&def.styles,idle:idle.clone()};
    let mut cache=BTreeMap::new();
    let spawned=crate::models::spawn_instance(&mut ctx.commands,&mut ctx.meshes,&mut ctx.materials,&ctx.assets,&ctx.config,&mut cache,&instance);
    let mut entities=spawned.parts.clone();
    let body=ctx.commands.spawn((crate::WorldGeometry,crate::models::PropBody {name:name.clone(),center:position,half_size:spawned.half})).id();entities.push(body);
    if def.solid || def.gravity {entities.push(ctx.commands.spawn((crate::WorldGeometry,crate::models::PropSupport {center:position,half_size:spawned.half})).id());}
    ctx.skins.0.extend(spawned.skins);
    ctx.world.field().add(PropState {name,def:swap.into(),center:position,rotation,half:spawned.half,solid:def.solid || def.gravity,hp:def.hp,dead:false,effects:hit_effects(def),origin:position,mass:def.mass,entities});
    if !def.anim_raz.is_empty() {ctx.animations.pending.push((format!("{}!{}",state.name,swap),def.anim_raz.clone()));}
}

#[cfg(test)]
mod tests {
    use super::*;
    const SAMPLE:&str="// header\nobject limuzyna prolog\nsolid\nmodel models\\levelowe\\limuzyna_prolog.ltb\nskin0 skins\\levelowe\\limuzyna.dtx\nrs1 rs\\cien.ltb\nmass 100\nHP 20000\niskry_przy_trafieniu\ndeath_wybuch\ndeath_podmien limuzyna_bucha\n\nobject papierek\ninsignificant\nmodel a.ltb\nHP 20\npapier_przy_trafieniu\ndeath_blacha_malo\ndeath_deski\ndeath_sound sounds\\x.wav\n\nobject strong\ninsignificant 2\nmodel b.ltb\nHP 1e3\ngravity\nprzezroczysty_dla_blikow\nanim0 idle\nanim_raz bucha\n";
    #[test]
    fn objects_txt_keys_fill_the_definition_like_the_client_parser() {
        let defs=parse_defs(SAMPLE);
        let car=&defs["limuzyna prolog"];
        assert!(car.solid && car.sparks && !car.gravity);assert_eq!(car.hp,20000.0);assert_eq!(car.mass,100.0);assert_eq!(car.swap,"limuzyna_bucha");
        assert_eq!(car.death,WYBUCH);assert_eq!(car.styles["1"],"rs\\cien.ltb");assert_eq!(car.model,"models\\levelowe\\limuzyna_prolog.ltb");
        let paper=&defs["papierek"];
        assert_eq!(paper.insignificant,1);assert!(paper.paper);assert_eq!(paper.death,0x20|0x80);assert_eq!(paper.death_sound,"sounds\\x.wav");
        let strong=&defs["strong"];
        assert_eq!(strong.insignificant,2);assert!(strong.gravity && strong.blik);assert_eq!(strong.anim0,"idle");assert_eq!(strong.anim_raz,"bucha");
        assert_eq!(hit_effects(car),HIT_SPARKS);assert_eq!(hit_effects(paper),HIT_PAPER);
    }
    fn barrel(x:f32,solid:bool)->PropState {PropState {name:format!("b{x}"),def:"b".into(),center:Vec3::new(x,30.0,0.0),rotation:[0.0;4],half:Vec3::new(19.0,31.0,18.0),solid,hp:100.0,dead:false,effects:0,origin:Vec3::new(x,30.0,0.0),mass:100.0,entities:vec![]}}
    #[test]
    fn bullets_hit_the_nearest_living_prop_and_movers_only_meet_solid_ones() {
        let mut field=PropField::default();field.add(barrel(100.0,true));let far=field.add(barrel(300.0,false));
        let (distance,index,normal)=field.ray(Vec3::new(0.0,30.0,0.0),Vec3::X,1000.0).unwrap();
        assert_eq!(index,0);assert!((distance-81.0).abs()<0.01,"{distance}");assert_eq!(normal,Vec3::NEG_X);
        field.props[0].dead=true;
        assert_eq!(field.ray(Vec3::new(0.0,30.0,0.0),Vec3::X,1000.0).map(|h|h.1),Some(far),"non-solid props are shot too");
        assert!(field.sweep(Vec3::new(0.0,30.0,0.0),Vec3::new(16.0,58.0,16.0),Vec3::X*1000.0).is_none(),"a dead barrel and a non-solid one do not block");
        field.props[0].dead=false;
        let (fraction,normal)=field.sweep(Vec3::new(0.0,30.0,0.0),Vec3::new(16.0,58.0,16.0),Vec3::X*100.0).unwrap();
        assert!((fraction*100.0-(100.0-19.0-16.0)).abs()<0.01);assert_eq!(normal,Vec3::NEG_X);
        assert!(field.sweep(Vec3::new(100.0,30.0,0.0),Vec3::new(16.0,58.0,16.0),Vec3::X*10.0).is_none(),"a mover that starts inside can leave");
        assert_eq!(field.strike(Vec3::new(105.0,40.0,5.0)),vec![0],"melee points inside the box");
    }
    #[test]
    fn bullets_shove_light_props_by_damage_times_ten_over_mass() {
        let d=push_delta(Vec3::new(100.0,0.0,0.0),Vec3::new(90.0,0.0,0.0),30.0,100.0);assert!((d-Vec3::X*3.0).length()<1e-4,"{d:?}");
        let light=push_delta(Vec3::new(0.0,10.0,0.0),Vec3::new(0.0,0.0,0.0),30.0,1.0);assert!((light-Vec3::Y*300.0).length()<1e-3);
        assert_eq!(push_delta(Vec3::X,Vec3::ZERO,30.0,0.0),Vec3::ZERO,"no mass, no push");assert_eq!(push_delta(Vec3::X,Vec3::X,30.0,10.0),Vec3::ZERO);
    }
    #[test]
    fn death_debris_follows_the_client_handler_table() {
        let count=|mask:u32|DEBRIS.iter().filter(|d|mask&d.mask!=0).map(|d|d.count).sum::<usize>();
        assert_eq!(count(0x4),8);assert_eq!(count(0x8),6);assert_eq!(count(0x10|0x20|0x40),22);assert_eq!(count(0x80|0x100|0x200|0x400),30);assert_eq!(count(WYBUCH|KEEPALIVE),0);
        assert_eq!(debris_path("blacha",4),"models/levelowe/kawalki/blacha05.ltb");
        let mut seed=0.0f32;let mut stream=move||{seed=(seed+0.37)%1.0;seed};
        for _ in 0..50 {let o=debris_offset(&mut stream);assert!(o.x.abs()<=32.0 && o.z.abs()<=32.0 && o.y>-16.1 && o.y<=32.0);}
        let velocity=Vec3::new(10.0,2.0,-4.0)*DEBRIS[0].factor;assert_eq!(velocity,Vec3::new(80.0,16.0,-32.0));
    }
    #[test]
    #[ignore="requires local original exports"]
    fn every_definition_has_its_model_and_skins_exported() {
        let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
        let data=PropData::load(&root);
        assert_eq!(data.defs.len(),229);assert!(!data.assets.is_empty(),"run: python -m tools.export_props definitions <GYARI> output");
        let mut missing=Vec::new();
        for (name,def) in &data.defs {
            let Some(assets)=data.assets.get(name) else{missing.push(format!("{name}: no exported assets"));continue};
            if !root.join(format!("{}.json",assets.model)).is_file() {missing.push(format!("{name}: model {}",assets.model));}
            for frames in assets.skins.values().chain(assets.swap_skins.values()) {for frame in frames {if !root.join(frame).is_file() {missing.push(format!("{name}: skin {frame}"));}}}
            if !def.swap.is_empty() && !data.defs.contains_key(&def.swap) {missing.push(format!("{name}: death_podmien {} undefined",def.swap));}
        }
        for (model,skin) in &data.debris {if !root.join(format!("{model}.json")).is_file() || !root.join(skin).is_file() {missing.push(format!("debris {model}"));}}
        assert!(missing.is_empty(),"{missing:#?}");
    }
}
