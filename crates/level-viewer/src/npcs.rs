//! Original placed characters and phase animations. Native coordinates throughout.
//! Combat/path choices are a bounded reconstruction; see docs/gameplay-export.md.
use bevy::{camera::visibility::NoFrustumCulling, prelude::*};
use retail_movement::{CollisionWorld, Vec3 as NativeVec3};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::Arc};
use crate::models::Model;
mod ai;

pub type PhaseCommands = Vec<(String, String)>;

#[derive(Clone,Copy)]
struct PlayerTarget {position:Vec3,half_extents:Vec3}

// Local MSVCRT rand stream. Its distribution matches the source; the original
// seed and all other game systems' consumption of that stream are not known.
struct ShotRng(u32);
impl Default for ShotRng {fn default()->Self {Self(1)}}
impl ShotRng {
    fn next(&mut self)->i32 {
        self.0=self.0.wrapping_mul(214013).wrapping_add(2531011);
        ((self.0>>16)&0x7fff) as i32
    }
    fn offset(&mut self,spread:f32)->f32 {((self.next()%1000-self.next()%1000) as f32)*spread*0.001}
}
fn source_difficulty(index:u8)->f32 {match index {0=>0.33,1=>0.67,_=>1.0}}
fn enemy_bullet_damage(strength:f32,difficulty:f32)->f32 {(strength.max(0.0)*difficulty*1.3).trunc()}
fn enemy_shot_endpoint(origin:Vec3,target:Vec3,spread:f32,difficulty:f32,rng:&mut ShotRng)->Vec3 {
    let delta=target-origin;
    let range=(640.0+24.0*(256.0-spread).max(0.0)).min(1.5*delta.length());
    let mut endpoint=origin+delta.normalize_or_zero()*range;
    if spread!=0.0 {
        let effective=spread*(2.0-difficulty);
        endpoint+=Vec3::new(rng.offset(effective),rng.offset(effective),rng.offset(effective));
    }
    endpoint
}
fn bullet_hits_player(world:&CollisionWorld,origin:Vec3,endpoint:Vec3,player:PlayerTarget,
    blocked:impl Fn(Vec3,Vec3)->bool)->bool {
    let delta=endpoint-origin;let distance=delta.length();
    let Some(direction)=delta.try_normalize() else {return false;};
    let Some(hit)=ray_box(origin,direction,player.position,player.half_extents).filter(|hit|*hit<=distance) else {return false;};
    world.raycast(native(origin),native(direction),hit).is_none_or(|(wall,_)|wall>=hit)
        && !blocked(origin,origin+direction*hit)
}

#[derive(Clone, Default, Deserialize)]
struct Phase {
    animation: Option<String>, duration: Option<f32>, dimensions: Option<[f32; 3]>,
    #[serde(rename="loop")] looping: bool,
    #[serde(default)] commands: PhaseCommands,
}
#[derive(Clone, Deserialize)]
struct Clip { dimensions:[f32;3] }
#[derive(Clone, Deserialize)]
struct Weapon {
    model:String, animation:String, skins:BTreeMap<String,String>, styles:BTreeMap<String,String>,
    socket:Option<String>, commands:PhaseCommands,
}
#[derive(Clone, Deserialize)]
struct Character {
    model:String, skins:BTreeMap<String,String>, styles:BTreeMap<String,String>,
    #[serde(default)] skin_variants:BTreeMap<String,BTreeMap<String,String>>,
    default_phase:String, hp:Option<f32>, hostile:bool, collision_half_extents:Option<[f32;3]>,
    flags:Vec<String>, header:PhaseCommands, phases:BTreeMap<String,Phase>,
    model_animations:BTreeMap<String,Clip>, weapon_asset:Option<Weapon>,
    /// glowa_specjalna characters (mayor, bartenders, Stella) and head props (cigarette, tie): a model on the body's head socket.
    #[serde(default)] head_asset:Option<Weapon>,
}
#[derive(Clone, Deserialize)]
struct Spawn {
    name:String, definition_name:String, pos:[f32;3], rotation:[f32;4], source_object_index:usize,
    #[serde(default)] properties:BTreeMap<String,serde_json::Value>,
}
/// A .pth holds several independent graphs whose place ids restart, so a node
/// is keyed by (graph, id). `pos` is the top position, `real_pos` the floor position the actors stand on
/// (cshell 0x1003d700 writes both); `commands` keeps the `idxN` records whose N is the compass slot (0 = +Z ... 7 = -X+Z).
#[derive(Clone, Deserialize)]
struct NavNode {
    id:usize, pos:[f32;3], #[serde(default)] real_pos:Option<[f32;3]>, neighbors:Vec<usize>, #[serde(default)] graph:usize,
    #[serde(default)] commands:PhaseCommands,
    /// Resolved by `Navigation::indexed`: neighbour node per compass slot, floor position, enabled flag (+0x70), door flag per slot (+0x61).
    #[serde(skip)] slots:[Option<usize>;8], #[serde(skip)] real:[f32;3], #[serde(skip)] enabled:bool, #[serde(skip)] door:[bool;8],
}
impl NavNode {
    #[cfg(test)]
    fn new(id:usize,pos:[f32;3],neighbors:Vec<usize>,graph:usize)->Self {
        Self {id,pos,real_pos:None,neighbors,graph,commands:Vec::new(),slots:[None;8],real:[0.0;3],enabled:true,door:[false;8]}
    }
}
#[derive(Default, Deserialize)]
struct Navigation { #[serde(default)] nodes:Vec<NavNode>, #[serde(skip)] index:BTreeMap<(usize,usize),usize> }
impl Navigation {
    fn indexed(mut self)->Self {
        self.index=self.nodes.iter().enumerate().map(|(i,n)|((n.graph,n.id),i)).collect();
        for i in 0..self.nodes.len() {
            let node=&self.nodes[i];
            let mut slots=[None;8];
            let explicit:Vec<(usize,usize)>=node.commands.iter().filter_map(|(k,v)|Some((k.strip_prefix("idx")?.parse::<usize>().ok()?,v.trim().parse::<usize>().ok()?))).collect();
            if explicit.is_empty() {for (slot,id) in node.neighbors.iter().take(8).enumerate() {slots[slot]=self.index.get(&(node.graph,*id)).copied();}}
            else {for (slot,id) in explicit {if slot<8 {slots[slot]=self.index.get(&(node.graph,id)).copied();}}}
            let real=node.real_pos.unwrap_or([node.pos[0],node.pos[1]-66.0,node.pos[2]]);
            let node=&mut self.nodes[i];node.slots=slots;node.real=real;node.enabled=true;node.door=[false;8];
        }
        self
    }
    fn pos(&self,node:usize)->Vec3 {Vec3::from(self.nodes[node].pos)}
    /// Flags every link whose segment (between the floor positions raised by 32, in either direction) crosses a door (cshell 0x1003ca80).
    fn mark_door_links(&mut self,doors:&[&crate::doors::Door]) {
        for i in 0..self.nodes.len() {
            for slot in 0..8 {
                let Some(j)=self.nodes[i].slots[slot] else {continue};
                let (a,b)=(self.real(i)+Vec3::Y*32.0,self.real(j)+Vec3::Y*32.0);
                self.nodes[i].door[slot]=doors.iter().any(|d|d.crosses(a,b)||d.crosses(b,a));
            }
        }
    }
    /// Floor position: what an actor stands on (its centre is this plus its half height).
    fn real(&self,node:usize)->Vec3 {Vec3::from(self.nodes[node].real)}
}
#[derive(Deserialize)]
struct MarkerDef {kind:String,#[serde(default)] pos:[f32;3],#[serde(default)] properties:BTreeMap<String,serde_json::Value>}
#[derive(Deserialize)]
struct Gameplay {
    characters:BTreeMap<String,Character>, npcs:Vec<Spawn>, #[serde(default)] navigation:Navigation,
    #[serde(default)] emitters:Vec<Spawn>, #[serde(default)] markers:Vec<MarkerDef>,
}

/// A weapon a dead character lets fall: `owner` keys the pickup (the second weapon of a two-handed character gets `name+1`).
#[derive(Clone,Debug)]
pub struct WeaponDrop {pub owner:String,pub kind:String,pub position:Vec3}

/// See `Npc::diagnostics`.
#[derive(Clone,Debug)]
pub struct NpcDiagnostics {pub phase:String,pub contact:bool,pub provoked:bool,pub seen:bool,pub moving:bool,pub place:Option<usize>,pub ammo:u32,pub reach:f32,pub cone_degrees:f32,
    pub on_kontakt:Option<String>,pub strzal:Option<f32>,pub speed:Option<f32>,pub yaw:f32,pub pitch:f32,pub armed:bool,pub progress:f32,pub seg_len:f32,pub next:Option<usize>,pub path_len:usize}

pub struct Npc {
    pub name:String,
    pub definition_name:String,
    /// Hull centre (native units): the floor position of the path node plus the half height while walking.
    pub position:Vec3,
    pub half_extents:Vec3,
    pub hp:f32,
    pub hostile:bool,
    pub phase:String,
    pub visible:bool,
    /// Hidden by the "insignificant characters off" option (extras: prisoners, rats, cockroaches, newspaper).
    pub hidden:bool,
    pub rotation:Quat,
    definition:Arc<Character>,
    elapsed:f32,
    weapon_visible:bool,
    /// The retail dead flag (+0x144): separate from `hp`, a character with `HP 0` is alive until it is hit.
    dead:bool,
    /// Seconds since death (corpse clean-up after 30 s, cshell.dll 0x10049d80).
    dead_for:f32,
    /// cshell +0x149: the player's eye can see the actor's chest, refreshed every `distance / 2560` s (0x10043720).
    seen:bool,seen_timer:f32,
    /// +0x126 the actor noticed the player (only evaluated in phases with `on_kontakt`); +0x127 a script provoked it (`hostileattack`).
    contact:bool,provoked:bool,
    /// +0x128 the script tagged the actor this tick (`ifaction`, `ifplayerseenby`, ...): `SetPhase` refuses its `patrol` phases (0x10041cbe).
    pub aware:bool,
    /// Head yaw offset from the body (cshell 0x100490b0): towards the player inside `glowa_do_gracza_dist` / `_kat` while seen, smoothed 0.9 per frame.
    head_yaw:f32,
    /// +0x14a set by `estimate_do_gracza`: the walk ends when closer than `do_gracza`.
    chase:bool,
    /// +0x158: face the player while sidestepping (set by `estimate_kryjowka`, cleared by every phase entry).
    face_player:bool,
    /// Path graph state (+0x210 place, +0x214 target, +0x218 moving, +0x220 progress, +0x21c segment length, +0x28 direction).
    place:Option<usize>,next:Option<usize>,moving:bool,progress:f32,seg_len:f32,seg_dir:Vec3,
    /// +0x274 path list (never cleared, like the original), +0x2f80 count, +0x2f84 index, +0x97 fired at the last node.
    path_buf:[Option<usize>;64],path_len:usize,path_index:usize,fired_at_node:bool,
    /// Yaw +0x174 (current) and +0x170 (target), gun pitch +0x17c and +0x178, radians.
    yaw:f32,yaw_target:f32,pitch:f32,pitch_target:f32,
    /// +0x224 seconds since the last `strzal` shot, rounds left in the two weapon objects, +0xa0 step sound timer.
    fire_timer:f32,ammo:[u32;2],step_timer:f32,
    /// Model offset that decays 10 % per frame after a snap (+0x10).
    vel:Vec3,
    /// Animation the model currently plays (looping phases with the same clip do not restart it, 0x10041e16).
    anim:String,
    /// The phase entry still owes its world-dependent part: seen refresh, `strzal_raz`, path start, melee.
    start_pending:bool,
    body:Vec<(usize,Entity,Handle<Mesh>)>,
    weapon:Vec<Entity>,
    head:Vec<(usize,Entity,Handle<Mesh>)>,
    /// Seconds since the last cigarette smoke streak (cshell 0x10044f00: one every 0.05 s).
    smoke_timer:f32,
    /// The head model's own clip and its clock (cshell PlayAnim 0x100450b0, always looping): SetPhase restarts it with the phase's `animacja_glowa` (0x10041ea4), a phase without
    /// one leaves it running; dialogue nodes switch the speaker to `gada` / `mruga` (0x10019b39, 0x10018c60, 0x1001950f). `head_request` waits for the head model to confirm the clip exists.
    head_clip:String,head_clock:f32,pub(crate) head_request:Option<String>,
    // Missing source clips use a disclosed stable pose; phase callbacks retain source names.
    rendered_animation:String,
}
impl Npc {
    /// Present in the world: not hidden by a mission command or by the insignificant characters option.
    pub fn shown(&self)->bool {self.visible && !self.hidden}
    pub fn insignificant(&self)->bool {self.definition.flags.iter().any(|f|f=="insignificant")}
    /// The original leaves no weapon pickup for characters flagged nie_zostawiaj_gana.
    pub fn dropped_weapon(&self)->Option<&str> {
        if self.definition.flags.iter().any(|f|f=="nie_zostawiaj_gana") {return None;}
        command_value(&self.definition.header,"weapon")
    }
    /// Experience the player earns for the kill (`exp_gained` in postacie.txt).
    pub fn exp_gained(&self)->u32 {number(&self.definition.header,"exp_gained").map_or(0,|v|v.max(0.0) as u32)}
    pub fn alive(&self)->bool {!self.dead}
    /// Where the body (model object, hit box) is: the path position lifted by the phase's `mod_y` (the helicopter hovers 480 above its path, cshell 0x1004aac1).
    pub fn body_center(&self)->Vec3 {self.position+Vec3::Y*number(self.commands(),"mod_y").unwrap_or(0.0)}
    pub fn eye(&self)->Vec3 {self.body_center()+Vec3::Y*self.half_extents.y*0.65}
    /// The `seen` flag (+0x149: the player's eye sees the chest, refreshed every update whatever the phase): what `ifplayerseenby` (0x1001a060) and
    /// `ifhostileblizejniz` (0x10019fc0) test. It does not need `on_kontakt` or a contact cone.
    pub fn seen_player(&self)->bool {self.alive() && self.seen}
    /// The original's contact flag (+0x126, only maintained while the phase has `on_kontakt`): what `ifseenbyhostile` (0x10019f70) and `hostileattack` test.
    pub fn noticed_player(&self)->bool {self.alive() && self.shown() && self.contact}
    pub fn player_seen(&self,_world:&CollisionWorld,_player_position:Vec3)->bool {self.noticed_player()}
    pub fn player_seen_with_doors(&self,_world:&CollisionWorld,_player_position:Vec3,_doors:&[&crate::doors::Door])->bool {self.noticed_player()}
    fn commands(&self)->&[(String,String)] {
        self.definition.phases.get(&self.phase).map(|p|p.commands.as_slice()).unwrap_or(&[])
    }
    /// A snapshot of the retail AI state for probes.
    pub fn diagnostics(&self)->NpcDiagnostics {
        let header=&self.definition.header;
        NpcDiagnostics {phase:self.phase.clone(),contact:self.contact,provoked:self.provoked,seen:self.seen,moving:self.moving,place:self.place,ammo:self.ammo[0],
            reach:number(header,"odleglosc_kontaktu").unwrap_or(640.0),cone_degrees:number(header,"kat_kontaktu").unwrap_or(0.0),
            on_kontakt:ai::single_pub(self.commands(),"on_kontakt"),strzal:number(self.commands(),"strzal").filter(|s|*s>0.0),speed:number(self.commands(),"speed").filter(|s|*s>0.0),
            yaw:self.yaw,pitch:self.pitch,armed:self.definition.weapon_asset.is_some(),progress:self.progress,seg_len:self.seg_len,next:self.next,path_len:self.path_len}
    }
    /// Does any phase of the definition react to contact, fire or patrol? (probe selection)
    pub fn has_phase_with(&self,key:&str)->bool {self.definition.phases.values().any(|p|has(&p.commands,key))}
    /// The names of the phases carrying a command.
    pub fn phases_with(&self,key:&str)->Vec<String> {self.definition.phases.iter().filter(|(_,p)|has(&p.commands,key)).map(|(n,_)|n.clone()).collect()}
    fn from_spawn(spawn:Spawn,definition:Arc<Character>)->Self {
        let fallback=definition.model_animations.values().next().map(|a|a.dimensions).unwrap_or([14.0,52.0,14.0]);
        let rotation=Quat::from_euler(EulerRot::YXZ,spawn.rotation[1],spawn.rotation[0],spawn.rotation[2]);
        let forward=rotation*Vec3::Z;let yaw=ai::yaw_of(forward.x,forward.z);
        let clip=weapon_clip(&definition);
        let second=command_value(&definition.header,"socket_weapon1").is_some_and(|s|!s.is_empty());
        Self {name:spawn.name,definition_name:spawn.definition_name,position:Vec3::from(spawn.pos),
            half_extents:Vec3::from(definition.collision_half_extents.unwrap_or(fallback)),
            hp:definition.hp.unwrap_or(0.0),hostile:definition.hostile,phase:String::new(),visible:true,hidden:false,
            rotation:Quat::from_rotation_y(yaw),
            definition,elapsed:0.0,weapon_visible:false,dead:false,dead_for:0.0,seen:false,seen_timer:0.0,contact:false,provoked:false,aware:false,head_yaw:0.0,chase:false,face_player:false,
            place:None,next:None,moving:false,progress:0.0,seg_len:0.0,seg_dir:Vec3::ZERO,path_buf:[None;64],path_len:0,path_index:0,fired_at_node:false,
            yaw,yaw_target:yaw,pitch:0.0,pitch_target:0.0,fire_timer:0.0,ammo:[clip,if second {clip}else{0}],step_timer:0.0,vel:Vec3::ZERO,anim:String::new(),start_pending:false,
            body:Vec::new(),weapon:Vec::new(),head:Vec::new(),smoke_timer:0.0,head_clip:String::new(),head_clock:0.0,head_request:None,rendered_animation:String::new()}
    }
}
/// Rounds in a full clip: the weapon item's `ammo_amount` (cshell 0x1001bad8 fills the weapon object from item +0xb24).
fn weapon_clip(definition:&Character)->u32 {
    definition.weapon_asset.as_ref().and_then(|w|number(&w.commands,"ammo_amount").or_else(||number(&w.commands,"max_ammo"))).map_or(0,|n|n.max(0.0) as u32)
}

/// A noise or light the actors react to (cshell 0x10045cf0 creates it, 0x10045df0 tests it): noticed inside `always`, inside `los`
/// or `cone` when the segment to the actor is free; it lives `life` frames of the actor update.
#[derive(Clone,Debug,PartialEq)]
pub struct Stimulus {pub pos:Vec3,pub always:f32,pub los:f32,pub cone:f32,pub life:u32}
impl Stimulus {
    /// The player's laser dot and flashlight pool (0x1000ee32, 0x1000b7c0): only the 640-unit visual radius.
    pub fn light(pos:Vec3)->Self {Self {pos,always:0.0,los:0.0,cone:crate::weapons_alt::STIMULUS_RADIUS,life:1}}
    /// A shot of the player (0x10005dd9 with the player flag set): 128 through walls, `glosnosc` with a free line, one frame.
    pub fn player_gunshot(pos:Vec3,loudness:f32)->Self {Self {pos,always:128.0,los:loudness,cone:0.0,life:1}}
    /// A shot of an actor (the same call without the flag): two frames.
    pub fn gunshot(pos:Vec3,loudness:f32)->Self {Self {life:2,..Self::player_gunshot(pos,loudness)}}
    /// The end point of every bullet (0x10007e17).
    pub fn impact(pos:Vec3)->Self {Self {pos,always:200.0,los:0.0,cone:1024.0,life:2}}
    /// A character hurt (0x10043128, 96 above its feet): by a bullet 192 / 1024 heard, by a melee weapon (item +0xb18) 64 / 1024 seen.
    pub fn wound(pos:Vec3,melee:bool)->Self {let pos=pos+Vec3::Y*96.0;if melee {Self {pos,always:64.0,los:0.0,cone:1024.0,life:2}}else{Self {pos,always:192.0,los:1024.0,cone:0.0,life:2}}}
    /// A character dying (0x10042d02, 66 above its feet).
    pub fn death(pos:Vec3)->Self {Self {pos:pos+Vec3::Y*66.0,always:256.0,los:1280.0,cone:2048.0,life:2}}
    /// A character that just made contact shouts (0x10049b38).
    pub fn shout(pos:Vec3)->Self {Self {pos,always:128.0,los:640.0,cone:1280.0,life:2}}
    /// A grenade going off (0x1005b170).
    pub fn explosion(pos:Vec3)->Self {Self {pos,always:196.0,los:2048.0,cone:0.0,life:1}}
}

/// What hurt a character: a bullet, a melee weapon or an explosion (which has its own loop and makes no wound noise).
#[derive(Clone,Copy,PartialEq,Eq)]
enum Hit {Bullet,Melee,Explosion}

#[derive(Resource,Default)]
pub struct NpcRoster {
    pub actors:Vec<Npc>,
    /// The mission's `hostileattack` has fired at least once (kept for save files); the per-actor state is `Npc::provoked`.
    pub hostiles_attacking:bool,
    /// The player's laser dot and flashlight pool this frame (cshell 0x1000b7c0 / 0x1000ee32: 640-unit visual stimuli that need a free line).
    pub visual_stimuli:Vec<Vec3>,
    /// A scripted cutscene runs (cshell `[0x10071978]`): actors that do not move still update, contact and relocation are off.
    pub cutscene:bool,
    /// The player's health is gone (`[player+0xcc]`): nobody notices anything.
    pub player_dead:bool,
    /// The noises and lights of the running frames (see `Stimulus`).
    pub noises:Vec<Stimulus>,
    pending_commands:Vec<(String,PhaseCommands)>,
    pending_damage:f32,
    /// Sound file, native position and retail audible radius (see crate::audio).
    pending_sounds:Vec<(String,Vec3,f32)>,
    pending_shots:Vec<EnemyShot>,
    pending_deaths:Vec<(Vec3,bool)>,
    pending_drops:Vec<WeaponDrop>,
    /// The hidden console variable `Invisible` (set from the options every frame): characters never see the player (0x10043720), neither with the weapon (0x10043860),
    /// notice stimuli (0x10045df0) nor make contact (0x100499ba).
    pub invisible:bool,
    /// Segments (floor node to floor node, raised 32) an actor is about to cross that a closed door blocks; `tick` opens the doors.
    door_requests:Vec<Vec3>,
    models:BTreeMap<String,Arc<Model>>,
    navigation:Navigation,
    dormant:BTreeMap<String,Npc>,
    /// Emitter placements; every later activation spawns another copy.
    templates:BTreeMap<String,(Spawn,Arc<Character>)>,
    respawns:Vec<String>,
    /// Definitions of the level, and characters the client creates itself: (definition, position, yaw).
    characters:BTreeMap<String,Arc<Character>>,
    client_spawns:Vec<(String,Vec3,f32)>,
    /// o_marker_nierespawnu_postaci volumes: emitters inside them do not spawn again.
    no_respawn:Vec<(Vec3,Vec3)>,
    shot_rng:ShotRng,
    // Sounds use their own stream (retail shares the one C rand()) so shot spreads stay reproducible.
    sound_rng:ShotRng,
    /// Random picks of the phase machine (`rand() % n`, 0x10045610).
    ai_rng:ShotRng,
    /// Door links (`NavNode::door`) are computed once the doors exist.
    links_ready:bool,
    /// The level section says `nie_sprawdzaj_drzwi` (the prologue): no path link gets a door flag (cshell 0x1003ca80 returns before it looks).
    pub skip_door_flags:bool,
    /// Counters for the headless AI probe.
    pub stats:NpcStats,
}
/// What the actors did since the level began (probes and tests only).
#[derive(Default,Clone,Debug)]
pub struct NpcStats {pub shots:u32,pub bullets_on_player:u32,pub melee:u32,pub damage_to_player:f32,pub kills:u32,pub door_asks:u32}
pub struct NpcHit {pub index:usize,pub name:String,pub distance:f32,pub killed:bool,pub bleeds:bool,pub stains:bool}
/// One enemy attack for the presentation (gunfire::enemy_fire): muzzle-to-endpoint bullet or a melee blow. A bite (`gryzie`, 0x100422e2) makes no
/// grunt; a shove (`odepchnij_gracza`, 0x10042460) plays weapons/wcialo.wav.
/// `extra` marks the second and later pellet of a `kul_na_raz` shot (only its bullet path and impact are shown); `victim`: the bullet struck another character.
pub struct EnemyShot {pub origin:Vec3,pub endpoint:Vec3,pub commands:PhaseCommands,pub casing:Option<Vec3>,pub hits_player:bool,pub melee:bool,pub bite:bool,pub extra:bool,pub victim:bool}

impl NpcRoster {
    pub fn activate_emitter(&mut self,name:&str)->bool {
        let Some(mut actor)=self.dormant.remove(name) else {return false;};
        actor.visible=true;
        let phase=actor.definition.default_phase.clone();
        self.actors.push(actor);let index=self.actors.len()-1;
        let _=self.enter_phase(index,&phase,true);true
    }
    /// An emitter whose dormant actor is already out spawns a new copy (cshell.dll 0x1002f601).
    pub fn request_respawn(&mut self,name:&str) {
        let Some((spawn,_))=self.templates.get(name) else {return};
        let at=Vec3::from(spawn.pos);
        if !self.no_respawn.iter().any(|(center,half)|(at-*center).abs().cmple(*half).all()) {self.respawns.push(name.into());}
    }
    /// A character the client creates on its own (the chapel's "laska czapel", cshell.dll 0x1002c5d0).
    pub fn request_client_spawn(&mut self,definition:&str,position:Vec3,yaw:f32) {
        if self.characters.contains_key(definition) {self.client_spawns.push((definition.into(),position,yaw));}
    }
    /// Distance from a point to the nearest path link (floor positions raised by `height`); probes use it to check that walkers stay on the graph.
    pub fn distance_to_links(&self,point:Vec3,height:f32)->Option<f32> {
        let nav=&self.navigation;
        nav.nodes.iter().enumerate().flat_map(|(i,n)|n.slots.iter().flatten().map(move|j|(i,*j))).map(|(i,j)|{
            let (a,b)=(nav.real(i)+Vec3::Y*height,nav.real(j)+Vec3::Y*height);let ab=b-a;
            let t=if ab.length_squared()>0.0 {((point-a).dot(ab)/ab.length_squared()).clamp(0.0,1.0)}else{0.0};(a+ab*t).distance(point)
        }).min_by(f32::total_cmp)
    }
    /// A noise or light for the actors (see `Stimulus`).
    pub fn add_stimulus(&mut self,pos:Vec3,always:f32,los:f32,cone:f32,life:u32) {self.noises.push(Stimulus {pos,always,los,cone,life});}
    /// Raises a stimulus for the next actor updates.
    pub fn add_noise(&mut self,noise:Stimulus) {self.noises.push(noise);}
    /// The `default_faza` of an instance (StartDialog resets its speaker to it, cshell 0x10019762).
    pub fn default_phase_of(&self,name:&str)->Option<String> {self.actors.iter().find(|a|a.name==name && a.alive()).map(|a|a.definition.default_phase.clone())}
    /// A living `ruchomy` (mobile) actor within `radius` of `point` (3D): what keeps a door open (cshell 0x1002ff40, definition flag +0xfac).
    pub fn mobile_near(&self,point:Vec3,radius:f32)->bool {
        self.actors.iter().any(|actor|actor.alive() && actor.definition.flags.iter().any(|f|f=="ruchomy") && actor.position.distance(point)<radius)
    }
    /// Points that ask the doors to open (message 0x34): after an estimate at the actor + 32 (cshell 0x100420b3) and on node arrivals next to a door link at
    /// the node's floor + 32 (0x100485d1). The list of one actor update; `doors::npc_open` opens the doors within 128 units of each point.
    pub fn door_openers(&self)->Vec<Vec3> {self.door_requests.clone()}
    /// The mission command `hostileattack` (cshell 0x1001a52d): every actor that currently notices the player is provoked, which lets
    /// its `on_kontakt` phase start (0x10049a6e). Actors that notice the player later wait for the next execution.
    pub fn provoke_noticing(&mut self) {
        self.hostiles_attacking=true;
        for actor in &mut self.actors {if actor.alive() && actor.contact {actor.provoked=true;}}
    }
    /// The script's per-tick tags (`Mission::aware`, +0x128): exactly the listed instances are aware.
    pub fn set_aware(&mut self,ids:&[String]) {for actor in &mut self.actors {actor.aware=ids.iter().any(|id|*id==actor.name);}}
    /// A dialogue node's `hostileattack` (cshell 0x10019811): its speaker is provoked (+0x127) and loses the `aware` tag (+0x128). The reaction
    /// itself (`on_kontakt`) still needs the contact test of the next actor update (0x10049a4e).
    pub fn provoke_speaker(&mut self,id:&str) {self.hostiles_attacking=true;if let Some(actor)=self.actors.iter_mut().find(|a|a.name==id && a.alive()) {actor.provoked=true;actor.aware=false;}}
    /// StartDialog resets its speaker to `default_faza` with the tag cleared meanwhile (0x10019773..0x10019804: +0x128 saved, cleared, SetPhase, restored), so a
    /// patrolling default phase is allowed. The actor then turns towards the player (+0x158).
    pub fn reset_speaker(&mut self,id:&str,phase:&str) {
        let Some(index)=self.actors.iter().position(|a|a.name==id && a.alive()) else {return};
        let aware=std::mem::replace(&mut self.actors[index].aware,false);
        self.transition(index,phase);
        self.actors[index].aware=aware;self.actors[index].face_player=true;
    }
    /// The end of a dialogue (0x10019550..0x10019585): the tag is gone and every formerly tagged actor re-enters its current phase, forced.
    pub fn release_aware(&mut self,ids:&[String]) {
        for id in ids {
            let Some(index)=self.actors.iter().position(|a|a.name==*id) else {continue};
            self.actors[index].aware=false;
            let phase=self.actors[index].phase.clone();
            if !phase.is_empty() && self.actors[index].alive() {self.transition(index,&phase);}
        }
    }
    /// `setfaza`: the named instance, else the first placed instance of the
    /// definition; a corpse never changes phase (it would stand up again). The
    /// return is for inspection only; drain_commands is the single authoritative
    /// source for mission phase_enter.
    pub fn set_phase(&mut self,name:&str,phase:&str)->PhaseCommands {
        let index=self.actors.iter().position(|a|a.name==name).or_else(||self.actors.iter().position(|a|a.definition_name==name));
        match index.filter(|i|self.actors[*i].alive()) {Some(index)=>self.transition(index,phase),None=>Vec::new()}
    }
    /// A dialogue plays the speaker's head clip (`gada` when a node starts, `mruga` when its answer list appears and when it ends), looping from 0 (cshell 0x10019b39, 0x10018c60, 0x1001950f).
    pub fn set_head_clip(&mut self,name:&str,clip:&str) {
        if let Some(actor)=self.actors.iter_mut().find(|a|a.name==name) {if actor.alive() {actor.head_request=Some(clip.to_owned());}}
    }
    /// `setallfaza` (0x1001a6a2): every living instance of the definition whose `seen` flag (+0x149) is set.
    pub fn set_phase_seeing(&mut self,name:&str,phase:&str,seen:&[bool]) {
        for i in 0..self.actors.len() {
            if self.actors[i].definition_name==name && self.actors[i].alive() && seen.get(i).copied().unwrap_or(false) {self.transition(i,phase);}
        }
    }
    /// Every script phase change is forced (cshell 0x10041c60 third argument 1): a walking actor jumps to its target node first.
    fn transition(&mut self,index:usize,phase:&str)->PhaseCommands {self.enter_phase(index,phase,true).unwrap_or_default()}
    /// Restores a saved actor through the regular phase entry so pose, hitbox and
    /// weapon visibility match the phase; reactivates saved reinforcements.
    pub fn restore_actor(&mut self,name:&str,position:Vec3,hp:f32,phase:&str,visible:bool) {
        if !self.actors.iter().any(|a|a.name==name) {if let Some(actor)=self.dormant.remove(name) {self.actors.push(actor);}}
        let Some(index)=self.actors.iter().position(|a|a.name==name) else {return};
        let _=self.enter_phase(index,phase,false);
        let actor=&mut self.actors[index];
        actor.phase=phase.into();actor.position=position;actor.hp=hp;actor.visible=visible;
        actor.dead=hp<0.0 || (hp<=0.0 && actor.definition.hp.is_some_and(|h|h>0.0));
        actor.place=None;actor.next=None;actor.moving=false;actor.start_pending=false;actor.contact=false;actor.provoked=false;
    }
    pub fn drain_commands(&mut self)->Vec<(String,PhaseCommands)> {std::mem::take(&mut self.pending_commands)}
    pub fn drain_damage(&mut self)->f32 {std::mem::take(&mut self.pending_damage)}
    pub fn drain_shots(&mut self)->Vec<EnemyShot> {std::mem::take(&mut self.pending_shots)}
    /// A shot fired by a cutscene character (opening.rs): the same presentation as an NPC execution shot.
    pub fn push_shot(&mut self,shot:EnemyShot) {self.pending_shots.push(shot);}
    /// Cutscene: the level's own instance of a character the scene moves out of the way (cshell 0x100130d0 saves and restores its position).
    pub fn set_scene_hidden(&mut self,definition:&str,hidden:bool) {for actor in self.actors.iter_mut().filter(|a|a.definition_name==definition) {actor.visible=!hidden;}}
    /// The player died (cshell 0x10060070): every living character re-enters its default phase (`default_faza`).
    pub fn player_died(&mut self) {for index in 0..self.actors.len() {if self.actors[index].alive() {let phase=self.actors[index].definition.default_phase.clone();self.transition(index,&phase);}}}
    /// Positions of actors killed since the last call and whether their definition leaves a blood pool (`plama_krwi`, cshell 0x10042e56).
    pub fn drain_deaths(&mut self)->Vec<(Vec3,bool)> {std::mem::take(&mut self.pending_deaths)}
    /// Weapons released by the characters killed since the last call (cshell `Kill` 0x10042ca0, 0x10042eec..0x10043085).
    pub fn drain_drops(&mut self)->Vec<WeaponDrop> {std::mem::take(&mut self.pending_drops)}
    /// The character `ifaction` names (cshell.dll 0x10019b80): a living actor whose box, grown by 32, contains the point 72 units ahead of the eye.
    /// The probe has **no line-of-sight test** (a thin wall between the eye and the actor does not stop it) and no `insignificant` filter; it
    /// walks the actor list and takes the first match (the list order is not recoverable here: several boxes at one point are broken by
    /// the distance, which matters only for overlapping actors). Corpses are never picked.
    pub fn use_target(&self,origin:Vec3,direction:Vec3,_world:&CollisionWorld)->Option<String> {
        let point=origin+direction.normalize_or_zero()*72.0;
        self.actors.iter().filter(|a|a.alive() && a.shown())
            .filter(|a|(point-a.body_center()).abs().cmplt(a.half_extents+Vec3::splat(32.0)).all())
            .min_by(|a,b|a.body_center().distance_squared(origin).total_cmp(&b.body_center().distance_squared(origin))).map(|a|a.name.clone())
    }
    /// Where a bullet ends on a corpse. The bullet filter (cshell 0x100578a0) lets a dead character's box through to the trace, which then only stops in
    /// the lowest quarter of the box (hit below `centre - half height / 2`, 0x1000686b..0x10006898, the blood splash 0x10005920); above that the bullet flies
    /// on from the hit point (0x10006963: 16 units further, the next object). Distance along the ray, `None` when no corpse takes it within `max`.
    pub fn corpse_stop(&self,origin:Vec3,direction:Vec3,max:f32)->Option<f32> {
        let direction=direction.try_normalize()?;let end=origin+direction*max;
        self.actors.iter().filter(|a|a.dead && a.shown()).filter_map(|a|{
            let (center,half)=(a.body_center(),a.half_extents);
            let entry=segment_box_entry(origin,end,center,half)?;
            ((origin+direction*entry).y<center.y-half.y*0.5).then_some(entry)
        }).min_by(f32::total_cmp)
    }
    /// Nearest living, visible actor box along a ray, without damage (the laser trace).
    pub fn ray_distance(&self,origin:Vec3,direction:Vec3,max:f32)->Option<f32> {
        let direction=direction.try_normalize()?;
        self.actors.iter().filter(|a|a.alive() && a.shown()).filter_map(|a|ray_box(origin,direction,a.body_center(),a.half_extents)).filter(|d|*d<=max).min_by(f32::total_cmp)
    }
    /// cshell 0x1000f420: every living, visible actor whose box strictly holds one of the probe points takes `damage` once.
    pub fn melee_hits(&mut self,points:&[Vec3],damage:f32)->Vec<NpcHit> {
        let targets:Vec<usize>=self.actors.iter().enumerate().filter(|(_,a)|a.alive() && a.shown() && points.iter().any(|p|crate::weapons_alt::inside_box(*p,a.body_center(),a.half_extents))).map(|(i,_)|i).collect();
        targets.into_iter().map(|index|{
            let name=self.actors[index].name.clone();
            let bleeds=!self.actors[index].definition.flags.iter().any(|flag|flag=="nie_krwaw");
            let stains=self.actors[index].definition.flags.iter().any(|flag|flag=="plama_krwi");
            let killed=self.damage_actor(index,damage,Hit::Melee);
            NpcHit {index,name,distance:0.0,killed,bleeds,stains}
        }).collect()
    }
    #[cfg(test)]
    fn hit_scan(&mut self,origin:Vec3,direction:Vec3,max_distance:f32,damage:f32,world:&CollisionWorld)->Option<NpcHit> {
        self.hit_scan_with(origin,direction,max_distance,|_|damage,world)
    }
    /// Ray against living, visible actors; damage is chosen from the native-unit hit distance (falloff).
    pub fn hit_scan_with(&mut self,origin:Vec3,direction:Vec3,max_distance:f32,damage:impl Fn(f32)->f32,world:&CollisionWorld)->Option<NpcHit> {
        if !origin.is_finite() || !direction.is_finite() || !max_distance.is_finite() || max_distance<=0.0 {return None;}
        let direction=direction.try_normalize()?;
        let limit=world.raycast(native(origin),native(direction),max_distance).map_or(max_distance,|(d,_)|d);
        let (index,distance)=self.actors.iter().enumerate().filter(|(_,a)|a.alive() && a.shown())
            .filter_map(|(i,a)|ray_box(origin,direction,a.body_center(),a.half_extents).filter(|d|*d<limit).map(|d|(i,d)))
            .min_by(|a,b|a.1.total_cmp(&b.1))?;
        let name=self.actors[index].name.clone();
        let bleeds=!self.actors[index].definition.flags.iter().any(|flag|flag=="nie_krwaw");
        let stains=self.actors[index].definition.flags.iter().any(|flag|flag=="plama_krwi");
        let killed=self.damage_actor(index,damage(distance),Hit::Bullet);
        let hit=NpcHit {index,name,distance,killed,bleeds,stains};
        Some(hit)
    }
    /// cshell `Hit` (0x10043090) and the explosion loop (0x10043180): only mobile characters take damage; HP is subtracted and the actor dies
    /// below zero (an explosion: at zero). Being hit does not interrupt a phase (there is no flinch or on_hurt reaction) but makes a noise (192 always / 1024 with a free line).
    fn damage_actor(&mut self,index:usize,damage:f32,kind:Hit)->bool {
        let actor=&mut self.actors[index];
        if actor.dead || damage<=0.0 || !actor.definition.flags.iter().any(|f|f=="ruchomy") {return false;}
        actor.hp-=damage;let body=actor.body_center();
        let killed=if kind==Hit::Explosion {actor.hp<=0.0}else{actor.hp<0.0};
        if kind!=Hit::Explosion {self.noises.push(Stimulus::wound(body,kind==Hit::Melee));}
        if killed {self.kill(index);}
        killed
    }
    pub fn blast(&mut self,center:Vec3,world:&CollisionWorld,blocked:impl Fn(Vec3,Vec3)->bool) {
        // The explosion is heard within 196 units and seen within 2048 with a free line (cshell 0x1005b170).
        self.noises.push(Stimulus::explosion(center));
        let origin=center+Vec3::Y*16.0;
        // The ray helper 0x10051c20 (called at 0x1004328d with only the target and the laser dots ignored) stops at every other model: a character or a corpse
        // standing between the blast and the target shields it.
        let targets:Vec<_>=self.actors.iter().enumerate().filter(|(_,a)|a.shown() && a.alive())
            .filter_map(|(index,a)|{let body=a.body_center();let distance=body.distance(center);
                (distance<640.0 && line_of_sight(world,origin,body) && !blocked(origin,body)
                    && !self.actors.iter().enumerate().any(|(j,other)|j!=index && other.shown() && segment_box_entry(origin,body,other.body_center(),other.half_extents).is_some()))
                    .then_some((index,(640.0-distance)*0.78125))}).collect();
        for (index,damage) in targets {self.damage_actor(index,damage,Hit::Explosion);}
    }
}

fn has(commands:&[(String,String)],key:&str)->bool {commands.iter().any(|(k,_)|k==key)}
fn has_numbered(commands:&[(String,String)],key:&str)->bool {
    commands.iter().any(|(candidate,_)|candidate==key || candidate.strip_prefix(key).is_some_and(|suffix|!suffix.is_empty() && suffix.bytes().all(|c|c.is_ascii_digit())))
}
fn command_value<'a>(commands:&'a [(String,String)],key:&str)->Option<&'a str> {commands.iter().rev().find(|(k,_)|k==key).map(|(_,value)|value.as_str())}
fn phase_sounds(commands:&[(String,String)],position:Vec3)->impl Iterator<Item=(String,Vec3,f32)>+'_ {
    commands.iter().filter(|(key,_)|key=="sound").map(move|(_,path)|(path.clone(),position,crate::audio::PHASE_SOUND_RADIUS))
}
/// The "halt" shout of a `sound_on_kontakt` phase: `sounds_on_kontakt N` picks haltN at random.
fn halt_sound(path:&str,variants:Option<f32>,rng:&mut ShotRng,position:Vec3,sees_player:bool)->(String,Vec3,f32) {
    let variants=variants.map_or(0,|n|n as u32);
    let pick=if variants>1 {rng.next() as u32}else{0};
    (crate::audio::halt_path(path,variants,pick),position,crate::audio::halt_radius(sees_player))
}
/// A `graj_dzwiek_smierci` character screams `deadN.wav` from the folder of its `sound_on_kontakt` file.
fn death_scream(definition:&Character,position:Vec3,rng:&mut ShotRng)->Option<(String,Vec3,f32)> {
    if !definition.flags.iter().any(|flag|flag=="graj_dzwiek_smierci") {return None;}
    let halt=definition.phases.values().flat_map(|phase|phase.commands.iter()).filter(|(key,_)|key=="sound_on_kontakt").map(|(_,path)|path.as_str()).last()?;
    crate::audio::death_scream_path(halt,0)?;
    crate::audio::death_scream_path(halt,rng.next() as u32).map(|path|(path,position,crate::audio::DEATH_SCREAM_RADIUS))
}
/// Native-space muzzle and casing sockets of the weapon an actor holds, and the gun's forward axis (the hand socket's +Z): body * hand socket * the weapon's
/// `socket_lezacy_strzal` / `socket_lezacy_luska_strzal` (cshell 0x100462e0).
fn weapon_sockets(models:&BTreeMap<String,Arc<Model>>,actor:&Npc)->Option<(Vec3,Option<Vec3>,Vec3)> {
    let weapon=actor.definition.weapon_asset.as_ref()?;
    let body=models.get(&actor.definition.model)?;
    let phase=actor.definition.phases.get(&actor.phase);
    let pose=body.pose(&actor.rendered_animation,actor.elapsed,phase.is_some_and(|p|p.looping));
    let base=body_transform(actor).to_matrix()*body.try_socket(&pose,weapon.socket.as_ref()?)?;
    let attached=models.get(&weapon.model)?;
    let held=attached.pose(&weapon.animation,0.0,false);
    let point=|key:&str|attached.try_socket(&held,command_value(&weapon.commands,key)?).map(|socket|(base*socket).transform_point3(Vec3::ZERO)/crate::SCALE);
    let forward=base.transform_vector3(Vec3::Z).normalize_or_zero();
    Some((point("socket_lezacy_strzal")?,point("socket_lezacy_luska_strzal"),forward))
}
fn number(commands:&[(String,String)],key:&str)->Option<f32> {
    commands.iter().rev().find(|(k,_)|k==key)?.1.split_whitespace().next()?.parse::<f32>().ok().filter(|v|v.is_finite())
}
fn native(v:Vec3)->NativeVec3 {NativeVec3::new(v.x,v.y,v.z)}
fn bevy_vec(v:NativeVec3)->Vec3 {Vec3::new(v.x,v.y,v.z)}
pub fn line_of_sight(world:&CollisionWorld,origin:Vec3,target:Vec3)->bool {
    let delta=target-origin;let distance=delta.length();
    distance<0.01 || world.raycast(native(origin),native(delta),distance).is_none_or(|(hit,_)|hit>=distance-0.2)
}
fn door_blocks(doors:&[&crate::doors::Door],origin:Vec3,target:Vec3)->bool {
    let delta=target-origin;let distance=delta.length();
    distance>0.01 && doors.iter().any(|door|door.shot_hit(origin,delta/distance,distance).is_some_and(|(hit,_)|hit<distance-0.2))
}
/// Distance along the segment `a`..`b` to the point where it enters the box, `None` when it misses or starts inside (Lithtech.exe 0x42b390 without the
/// "from inside" query flag, which no cshell query sets): such a segment never hits the box.
pub fn segment_box_entry(a:Vec3,b:Vec3,center:Vec3,half:Vec3)->Option<f32> {
    if (a-center).abs().cmple(half).all() {return None;}
    let delta=b-a;let length=delta.length();
    if length<1e-6 {return None;}
    let entry=ray_box(a,delta/length,center,half)?;
    (entry<=length).then_some(entry)
}
fn choose_animation(character:&Character,requested:Option<&str>)->String {
    requested.filter(|name|character.model_animations.contains_key(*name))
        .or_else(||["stoi","stoi_nic","nuda","martwy"].into_iter().find(|name|character.model_animations.contains_key(*name)))
        .or_else(||character.model_animations.keys().next().map(String::as_str)).unwrap_or("").into()
}
fn ray_box(origin:Vec3,direction:Vec3,center:Vec3,half:Vec3)->Option<f32> {
    let low=center-half;let high=center+half;let mut enter=0.0_f32;let mut leave=f32::INFINITY;
    for i in 0..3 {
        if direction[i].abs()<0.000001 {if origin[i]<low[i] || origin[i]>high[i] {return None;}}
        else {let a=(low[i]-origin[i])/direction[i];let b=(high[i]-origin[i])/direction[i];enter=enter.max(a.min(b));leave=leave.min(a.max(b));}
    }
    (enter<=leave).then_some(enter)
}
/// Largest drop the spawn snap applies here (see `snap_spawn`).
const SNAP_LIMIT:f32=32.0;
/// cshell 0x100432e0: a segment from the actor's centre 256 units straight down (filter 0x10057750: everything except the actor, usable doors and items);
/// when it hits, the actor is put on the hit point plus its half height. Run for an actor whose default phase is not `static` (0x10043589) and whenever the
/// phase `estimate` is entered (0x10041d0e; not repeated here: the path nodes already carry the floor heights). The engine ray also meets props and
/// world models, which are not part of the static hull: a drop of more than 32 units, or onto a slope, is left alone (a vehicle roof, a table) instead of
/// being taken to the floor beneath it.
pub fn snap_spawn(world:&CollisionWorld,position:Vec3,half:Vec3)->Vec3 {
    let Some((distance,normal))=world.raycast(native(position),native(Vec3::NEG_Y),256.0) else {return position};
    let snapped=Vec3::new(position.x,position.y-distance+half.y,position.z);
    if normal.y>0.6 && snapped.y<=position.y && position.y-snapped.y<=SNAP_LIMIT {snapped}else{position}
}
fn lift_from_floor_below(world:&CollisionWorld,position:Vec3,half:Vec3)->Vec3 {
    let origin=position+Vec3::Y*2.0;
    let bottom=position.y-half.y;
    if let Some((distance,normal))=world.raycast(native(origin),native(Vec3::NEG_Y),half.y+128.0) {
        let floor=origin.y-distance;
        if normal.y>0.6 && floor>bottom+0.04 && floor<=origin.y {
            return Vec3::new(position.x,floor+half.y+0.04,position.z);
        }
    }
    position
}
fn lift_interpenetrating_spawn(world:&CollisionWorld,position:Vec3,half:Vec3)->Vec3 {
    let origin=position+Vec3::Y*2.0;
    // Two retail underground spawns place the authored center five units
    // below the floor. Check this nearby upper surface before a lower one.
    let top=position+Vec3::Y*(half.y+2.0);
    if let Some((distance,normal))=world.raycast(native(top),native(Vec3::NEG_Y),half.y+2.0) {
        let floor=top.y-distance;
        if normal.y>0.6 && floor>origin.y && floor<=position.y+half.y*0.25 {
            return Vec3::new(position.x,floor+half.y+0.04,position.z);
        }
    }
    lift_from_floor_below(world,position,half)
}
fn paper_ground_position(world:&CollisionWorld,position:Vec3,half:Vec3)->Vec3 {
    // Retail newspaper spawns are authored at Y=-5 even where the pavement
    // is above them. A thin paper should follow the nearest local surface,
    // without the 24-unit stair correction reserved for walking actors.
    let origin=position+Vec3::Y*32.0;
    world.raycast(native(origin),native(Vec3::NEG_Y),640.0)
        .filter(|(_,normal)|normal.y>0.6)
        .map_or(position,|(distance,_)|Vec3::new(position.x,origin.y-distance+half.y+0.04,position.z))
}
fn settle_spawn(world:&CollisionWorld,actor:&mut Npc) {
    if actor.definition_name=="gazeta" {actor.position=paper_ground_position(world,actor.position,actor.half_extents);}
    // Retail 0x10043589 skips floor placement for static phases. Their authored
    // origin anchors seated/exercise poses; raising by hull half-height floats them.
    else if !has(actor.commands(),"static") {
        actor.position=lift_interpenetrating_spawn(world,actor.position,actor.half_extents);
        actor.position=snap_spawn(world,actor.position,actor.half_extents);
    }
}
pub fn setup_world(mut commands:Commands,config:Res<crate::ViewerConfig>,assets:Res<AssetServer>,walking:Res<crate::Walking>,
    mut meshes:ResMut<Assets<Mesh>>,mut materials:ResMut<Assets<StandardMaterial>>,mut roster:ResMut<NpcRoster>) {
    // WorldGeometry entities were removed by the parent world setup.
    roster.actors.clear();roster.dormant.clear();roster.templates.clear();roster.respawns.clear();roster.client_spawns.clear();roster.pending_commands.clear();roster.pending_sounds.clear();roster.pending_shots.clear();roster.pending_deaths.clear();roster.pending_drops.clear();roster.pending_damage=0.0;roster.hostiles_attacking=false;roster.noises.clear();roster.door_requests.clear();roster.links_ready=false;roster.cutscene=false;roster.player_dead=false;
    let path=config.output.join(format!("{}.gameplay.json",config.world));
    let data=match std::fs::read_to_string(&path).ok().and_then(|s|serde_json::from_str::<Gameplay>(&s).ok()) {
        Some(data)=>data,None=>{warn!("NPC export unavailable: {}",path.display());return;}
    };
    roster.navigation=data.navigation.indexed();
    // o_marker_niechodzenia_postaci: the nodes inside the volume (grown by 16, from 16 below to 48 above) are switched off once per level (cshell 0x1003cd60).
    for marker in data.markers.iter().filter(|m|m.kind=="o_marker_niechodzenia_postaci") {
        let half=marker.properties.get("Promien").and_then(|p|p.as_array()).map(|p|Vec3::new(p[0].as_f64().unwrap_or(0.0) as f32,p[1].as_f64().unwrap_or(0.0) as f32,p[2].as_f64().unwrap_or(0.0) as f32)).unwrap_or(Vec3::ZERO);
        let centre=Vec3::from(marker.pos);
        for node in &mut roster.navigation.nodes {
            let p=Vec3::from(node.pos);
            if p.x-16.0<centre.x+half.x && p.x+16.0>centre.x-half.x && p.z-16.0<centre.z+half.z && p.z+16.0>centre.z-half.z && p.y-48.0<centre.y+half.y && p.y+16.0>centre.y-half.y {node.enabled=false;}
        }
    }
    let characters=data.characters.into_iter().map(|(name,c)|(name,Arc::new(c))).collect::<BTreeMap<_,_>>();
    roster.characters=characters.clone();
    let volume=|m:&MarkerDef|(Vec3::from(m.pos),m.properties.get("Promien").and_then(|p|p.as_array()).map(|p|Vec3::new(p[0].as_f64().unwrap_or(0.0) as f32,p[1].as_f64().unwrap_or(0.0) as f32,p[2].as_f64().unwrap_or(0.0) as f32)).unwrap_or(Vec3::ZERO));
    roster.no_respawn=data.markers.iter().filter(|m|m.kind=="o_marker_nierespawnu_postaci").map(volume).collect();
    let mut material_cache=BTreeMap::<String,Handle<StandardMaterial>>::new();
    for (spawn,emitter) in data.npcs.into_iter().map(|s|(s,false)).chain(data.emitters.into_iter().map(|s|(s,true))) {
        let Some(definition)=characters.get(&spawn.definition_name).cloned() else {continue;};
        if emitter {roster.templates.insert(spawn.name.clone(),(spawn.clone(),definition.clone()));}
        let actor=spawn_npc(&mut commands,&config,&assets,&walking.world,&mut meshes,&mut materials,&mut roster,&mut material_cache,spawn,definition,emitter);
        if emitter {roster.dormant.insert(actor.name.clone(),actor);}else{roster.actors.push(actor);}
    }
    info!("{} original NPCs loaded for {}",roster.actors.len(),config.world);
}
/// Fulfils repeated emitter activations with fresh, uniquely named copies.
pub fn respawn_emitters(mut commands:Commands,config:Res<crate::ViewerConfig>,assets:Res<AssetServer>,walking:Res<crate::Walking>,
    mut meshes:ResMut<Assets<Mesh>>,mut materials:ResMut<Assets<StandardMaterial>>,mut roster:ResMut<NpcRoster>,mut material_cache:Local<BTreeMap<String,Handle<StandardMaterial>>>) {
    for name in std::mem::take(&mut roster.respawns) {
        let Some((mut spawn,definition))=roster.templates.get(&name).cloned() else {continue};
        let copies=roster.actors.iter().filter(|a|a.name==name || a.name.starts_with(&format!("{name}~"))).count();
        spawn.name=format!("{name}~{copies}");
        let actor=spawn_npc(&mut commands,&config,&assets,&walking.world,&mut meshes,&mut materials,&mut roster,&mut material_cache,spawn,definition,false);
        roster.actors.push(actor);
    }
    for (definition,position,yaw) in std::mem::take(&mut roster.client_spawns) {
        let Some(character)=roster.characters.get(&definition).cloned() else {continue};
        let spawn=Spawn {name:definition.clone(),definition_name:definition.clone(),pos:position.to_array(),rotation:[0.0,yaw,0.0,0.0],source_object_index:0,properties:default()};
        let actor=spawn_npc(&mut commands,&config,&assets,&walking.world,&mut meshes,&mut materials,&mut roster,&mut material_cache,spawn,character,false);
        roster.actors.push(actor);
    }
}
/// Builds one actor with its body and weapon entities; the caller files it as active or dormant.
#[allow(clippy::too_many_arguments)]
fn spawn_npc(commands:&mut Commands,config:&crate::ViewerConfig,assets:&AssetServer,world:&CollisionWorld,meshes:&mut Assets<Mesh>,
    materials:&mut Assets<StandardMaterial>,roster:&mut NpcRoster,material_cache:&mut BTreeMap<String,Handle<StandardMaterial>>,
    spawn:Spawn,definition:Arc<Character>,emitter:bool)->Npc {
    let model=roster.models.entry(definition.model.clone()).or_insert_with(||Arc::new(Model::load(&config.output,&definition.model))).clone();
    let original_skins=definition.skin_variants.values().nth(spawn.source_object_index%definition.skin_variants.len().max(1)).unwrap_or(&definition.skins).clone();
    let mut skins=original_skins.clone();
    let personal_face=ensure_umbrella_face(&config.output) && apply_umbrella_face(&config.world,&spawn.definition_name,&mut skins);
    let mut actor=Npc::from_spawn(spawn,definition.clone());actor.visible=!emitter;
    let initial=actor.begin_phase(&definition.default_phase,false,&roster.navigation).unwrap_or_default();
    settle_spawn(world,&mut actor);
    if !emitter {
        roster.pending_sounds.extend(phase_sounds(&initial,actor.position));
        roster.pending_commands.push((actor.name.clone(),initial));
    }
    let phase=definition.phases.get(&actor.phase);
    let pose=model.pose(&actor.rendered_animation,0.0,phase.is_some_and(|p|p.looping));
    for (piece_index,piece) in model.pieces.iter().enumerate() {
        let parts=if personal_face && piece.texture==1 {
            // The speaking mouth has its own UV island. Keep the original
            // teeth there while applying the supplied face to the rest.
            let (face,teeth)=model.split_uv_region(piece_index,|[u,v]|u>0.68 && v<0.26);
            vec![(face,&skins),(teeth,&original_skins)]
        } else {vec![(model.mesh(piece_index),&skins)]};
        for (mut mesh,part_skins) in parts {
            model.animate_mesh(piece_index,&pose,&mut mesh);let mesh=meshes.add(mesh);
            let material=material_handle(piece.texture,part_skins,&definition.styles,assets,materials,material_cache);
            let entity=commands.spawn((Mesh3d(mesh.clone()),MeshMaterial3d(material),crate::WorldGeometry,NoFrustumCulling,
                body_transform(&actor),if actor.shown() {Visibility::Visible}else{Visibility::Hidden})).id();
            actor.body.push((piece_index,entity,mesh));
        }
    }
    if let Some(weapon)=&definition.weapon_asset {
        if weapon.socket.as_ref().is_some_and(|s|model.try_socket(&pose,s).is_none()) {warn!("Gyári csatlakozópont hiányzik: {} / {:?}",actor.definition_name,weapon.socket);}
        if let Some(socket)=weapon.socket.as_ref().filter(|s|model.try_socket(&pose,s).is_some()) {
            let attachment=roster.models.entry(weapon.model.clone()).or_insert_with(||Arc::new(Model::load(&config.output,&weapon.model))).clone();
            let attachment_pose=attachment.pose(&weapon.animation,0.0,false);
            let transform=Transform::from_matrix(body_transform(&actor).to_matrix()*model.socket(&pose,socket));
            for (i,piece) in attachment.pieces.iter().enumerate() {
                let mut mesh=attachment.mesh(i);attachment.animate_mesh(i,&attachment_pose,&mut mesh);
                let material=material_handle(piece.texture,&weapon.skins,&weapon.styles,assets,materials,material_cache);
                actor.weapon.push(commands.spawn((Mesh3d(meshes.add(mesh)),MeshMaterial3d(material),crate::WorldGeometry,
                    NoFrustumCulling,transform,if actor.shown() && actor.weapon_visible {Visibility::Visible}else{Visibility::Hidden})).id());
            }
        }
    }
    if let Some(head)=&definition.head_asset {
        if let Some(socket)=head.socket.as_ref().filter(|s|model.try_socket(&pose,s).is_some()) {
            let attachment=roster.models.entry(head.model.clone()).or_insert_with(||Arc::new(Model::load(&config.output,&head.model))).clone();
            let attachment_pose=attachment.pose(&head.animation,0.0,false);
            let transform=Transform::from_matrix(body_transform(&actor).to_matrix()*model.socket(&pose,socket));
            for (i,piece) in attachment.pieces.iter().enumerate() {
                let mut mesh=attachment.mesh(i);attachment.animate_mesh(i,&attachment_pose,&mut mesh);let mesh=meshes.add(mesh);
                let material=material_handle(piece.texture,&head.skins,&head.styles,assets,materials,material_cache);
                let entity=commands.spawn((Mesh3d(mesh.clone()),MeshMaterial3d(material),crate::WorldGeometry,
                    NoFrustumCulling,transform,if actor.shown() {Visibility::Visible}else{Visibility::Hidden})).id();
                actor.head.push((i,entity,mesh));
            }
        }else{warn!("Gyári fejcsatlakozó hiányzik: {} / {:?}",actor.definition_name,head.socket);}
    }
    actor
}
fn body_transform(actor:&Npc)->Transform {
    Transform {translation:(actor.body_center()+actor.vel)*crate::SCALE,rotation:actor.rotation,scale:Vec3::splat(crate::SCALE)}
}
/// Fitted with `tools/fit_umbrella_face.py`; the community easter egg is included
/// in every build independently of the retail export.
const UMBRELLA_FACE:&str="mods/umbrella_face.png";
/// The fitted face is part of the build (crates/level-viewer/assets/umbrella_face.png, compiled in), so a fresh `output/` made by the exporters still gets it:
/// the file is refreshed in `output/mods/` if an older export has another face.
static UMBRELLA_FACE_PNG:&[u8]=include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),"/assets/umbrella_face.png"));
pub(crate) fn ensure_umbrella_face(output:&std::path::Path)->bool {
    let path=output.join(UMBRELLA_FACE);
    if std::fs::read(&path).is_ok_and(|bytes| bytes==UMBRELLA_FACE_PNG) {return true;}
    if let Some(parent)=path.parent() {let _=std::fs::create_dir_all(parent);}
    std::fs::write(&path,UMBRELLA_FACE_PNG).is_ok()
}
fn apply_umbrella_face(world:&str,character:&str,skins:&mut BTreeMap<String,String>)->bool {
    // Deliberate easter egg for the Hungarian community, in both editions.
    if world=="rh3-miasteczko0" && character=="cywil1p" {
        skins.insert("1".into(),UMBRELLA_FACE.into());
        true
    }else {
        false
    }
}
fn material_handle(slot:u32,skins:&BTreeMap<String,String>,styles:&BTreeMap<String,String>,assets:&AssetServer,
    materials:&mut Assets<StandardMaterial>,cache:&mut BTreeMap<String,Handle<StandardMaterial>>)->Handle<StandardMaterial> {
    let skin=skins.get(&slot.to_string());let style=styles.get(&slot.to_string());let key=format!("{skin:?}|{style:?}");
    cache.entry(key).or_insert_with(||materials.add(StandardMaterial {
        base_color_texture:skin.map(|s|assets.load(level_viewer::hd::path(s))),unlit:false,cull_mode:None,
        alpha_mode:if style.is_some_and(|s|s.contains("maska")) {AlphaMode::Mask(0.3)}else{AlphaMode::Opaque},..crate::models::matte()
    })).clone()
}
pub fn tick(time:Res<Time>,session:Res<crate::settings::Session>,opening:Res<crate::opening::Opening>,
    walking:Res<crate::Walking>,mut roster:ResMut<NpcRoster>,mut meshes:ResMut<Assets<Mesh>>,
    mut visuals:Query<(&mut Transform,&mut Visibility)>,doors:Query<&crate::doors::Door>,props:Res<crate::props::PropWorld>,
    mut commands:Commands,assets:Res<AssetServer>,effects:Option<Res<crate::gunfire::Effects>>) {
    if session.paused {return;}
    // A cutscene freezes the simulation (the visual sync below still runs for characters it moves).
    // Retail's AI has no dialogue check (cshell 0x1004ad30..): only the halted mission after the player's death (`dialogue_active`) stops it here.
    if !opening.active && !session.dialogue_active {
        {
            let door_refs=doors.iter().collect::<Vec<_>>();
            // The path links that cross a door are known once the doors exist (cshell 0x1003ca80).
            // Filter 0x100577c0 accepts only objects with user flag 0x8, i.e. doors and drawers with `Gracz_otwiera` (object.lto 0x10001959, 0x10003b91): glass, locked gates and grates do not flag a link.
            if !roster.links_ready {roster.links_ready=true;if !roster.skip_door_flags {roster.navigation.mark_door_links(&door_refs.iter().copied().filter(|d|d.usable()).collect::<Vec<_>>());}
                let (links,doors_on)=(roster.navigation.nodes.iter().map(|n|n.slots.iter().flatten().count()).sum::<usize>(),roster.navigation.nodes.iter().map(|n|n.door.iter().filter(|d|**d).count()).sum::<usize>());
                info!("NPC útvonalak: {} csomópont, {links} kapcsolat, ebből {doors_on} ajtón át",roster.navigation.nodes.len());}
            let player=PlayerTarget {position:bevy_vec(walking.player.position),half_extents:bevy_vec(walking.player.half_size())};
            roster.simulate_with_barriers(&walking.world,player,source_difficulty(session.preferences.difficulty),time.delta_secs().min(0.05),
                |origin,target|door_blocks(&door_refs,origin,target) || props.field().blocks_segment(origin,target),
                |position,half,delta|door_refs.iter().filter_map(|door|door.sweep(position,half,delta)).min_by(|a,b|a.0.total_cmp(&b.0)));
        }
    }
    else {for actor in &mut roster.actors {actor.elapsed+=time.delta_secs().min(0.05);}}
    // Head clips: a requested clip replaces the current one (and restarts it) only when the head model has it; the clock runs on.
    {
        let roster=&mut *roster;let dt=time.delta_secs().min(0.05);
        for actor in &mut roster.actors {
            actor.head_clock+=dt;
            let Some(request)=actor.head_request.take() else {continue};
            let known=actor.definition.head_asset.as_ref().and_then(|h|roster.models.get(&h.model)).is_some_and(|m|m.animation_duration(&request).is_some());
            if known {actor.head_clip=request;actor.head_clock=0.0;}
        }
    }
    // Silent runs create no audio player; radii are the retail audible ranges (docs/retail-audio.md).
    for (path,position,radius) in std::mem::take(&mut roster.pending_sounds) {crate::audio::play_near(&mut commands,&assets,&path,position,radius);}
    // Cigarette smoke (cshell 0x10044f00, called for every actor of the normal update loop): a phase with `dym_papierosa`, a `socket_papieros`
    // and the player within 640 units; every 0.05 s one `sprites\papieros.spr` streak (scale 0.02 x 0.2, 2.4 s, rising 12 units/s, colour (0.1,0.15,0.2), its alpha 0.1 is unused by ONE/ONE) at that socket.
    if let (Some(effects),false)=(&effects,opening.active) {
        let player=bevy_vec(walking.player.position);let dt=time.delta_secs().min(0.05);let roster=&mut *roster;
        for actor in &mut roster.actors {
            let Some(socket)=command_value(&actor.definition.header,"socket_papieros").filter(|s|!s.is_empty()) else {continue;};
            let Some(phase)=actor.definition.phases.get(&actor.phase).filter(|p|p.commands.iter().any(|(k,_)|k=="dym_papierosa")) else {continue;};
            if !actor.shown() || actor.position.distance(player)>640.0 {continue;}
            actor.smoke_timer+=dt;
            if actor.smoke_timer<0.05 {continue;}
            actor.smoke_timer=0.0;
            let Some(model)=roster.models.get(&actor.definition.model) else {continue;};
            let pose=model.pose(&actor.rendered_animation,actor.elapsed,phase.looping);
            let Some(local)=model.try_socket(&pose,socket) else {continue;};
            let at=(body_transform(actor).to_matrix()*local).w_axis.truncate()/crate::SCALE;
            crate::fx::spawn(&mut commands,crate::fx::Particle::sprite(&effects.lib,"sprites/papieros.spr",at)
                .map(|p|p.scale2(0.02,0.2).life(2.4).vel(Vec3::Y*12.0).streak().color([0.1,0.15,0.2])));
        }
    }
    // Head tracking (0x100490b0..0x10049580): a phase with `glowa_do_gracza_dist` > 8 turns the head to the player while he is nearer than that, within `glowa_do_gracza_kat`
    // degrees of the body's heading and the actor is seen; otherwise straight. The offset follows its target by 0.9 per frame.
    let player_at=bevy_vec(walking.player.position);
    for actor in &mut roster.actors {
        let target=actor.definition.phases.get(&actor.phase).and_then(|p|{
            let dist=number(&p.commands,"glowa_do_gracza_dist").filter(|d|*d>8.0)?;
            let kat=number(&p.commands,"glowa_do_gracza_kat").unwrap_or(0.0).to_radians();
            let flat=player_at-actor.position;
            let angle=(flat.x.atan2(flat.z)-actor.yaw+std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)-std::f32::consts::PI;
            (actor.position.distance(player_at)<dist && angle.abs()<kat && actor.seen).then_some(angle)
        }).unwrap_or(0.0);
        actor.head_yaw+=(target-actor.head_yaw)*0.9;
    }
    for actor in &roster.actors {
        let Some(model)=roster.models.get(&actor.definition.model) else {continue;};
        let phase=actor.definition.phases.get(&actor.phase);
        let pose=model.pose(&actor.rendered_animation,actor.elapsed,phase.is_some_and(|p|p.looping));
        for (piece,entity,mesh) in &actor.body {
            if let Some(mesh)=meshes.get_mut(mesh) {model.animate_mesh(*piece,&pose,mesh);}
            if let Ok((mut transform,mut visibility))=visuals.get_mut(*entity) {
                *transform=body_transform(actor);*visibility=if actor.shown() {Visibility::Visible}else{Visibility::Hidden};
            }
        }
        if let Some(socket)=actor.definition.weapon_asset.as_ref().and_then(|w|w.socket.as_ref()).filter(|s|model.try_socket(&pose,s).is_some()) {
            let transform=Transform::from_matrix(body_transform(actor).to_matrix()*model.socket(&pose,socket));
            // The weapon object follows the hand only while the current phase has `show_weapon` (cshell 0x10049f38: every other phase leaves the object where it
            // was; the flag that made it visible stays, 0x10041ed1).
            let attached=phase.is_some_and(|p|has(&p.commands,"show_weapon"));
            for entity in &actor.weapon {
                if let Ok((mut current,mut visibility))=visuals.get_mut(*entity) {
                    if attached {*current=transform;}
                    *visibility=if actor.shown() && actor.weapon_visible {Visibility::Visible}else{Visibility::Hidden};
                }
            }
        }
        if let Some(socket)=actor.definition.head_asset.as_ref().and_then(|h|h.socket.as_ref()).filter(|s|model.try_socket(&pose,s).is_some()) {
            let neck=body_transform(actor).to_matrix()*model.socket(&pose,socket);
            let pivot=neck.w_axis.truncate();
            let transform=Transform::from_matrix(Mat4::from_translation(pivot)*Mat4::from_rotation_y(actor.head_yaw)*Mat4::from_translation(-pivot)*neck);
            // `animacja_glowa` names the head model's own clip (talking `gada`, blinking `mruga`); the head keeps its first clip until a phase names one.
            let head=actor.definition.head_asset.as_ref().unwrap();
            let clip=if actor.head_clip.is_empty() {head.animation.as_str()}else{actor.head_clip.as_str()};
            let head_pose=roster.models.get(&head.model).map(|m|(m.clone(),m.pose(clip,actor.head_clock,true)));
            for (piece,entity,mesh) in &actor.head {
                if let Some((head_model,head_pose))=&head_pose {if let Some(mesh)=meshes.get_mut(mesh) {head_model.animate_mesh(*piece,head_pose,mesh);}}
                if let Ok((mut current,mut visibility))=visuals.get_mut(*entity) {*current=transform;*visibility=if actor.shown() {Visibility::Visible}else{Visibility::Hidden};}
            }
        }
    }
}

fn sweep_actor(position:Vec3,half:Vec3,delta:Vec3,actor:&Npc)->Option<(f32,Vec3)> {
    if !actor.alive() || !actor.shown() || actor.definition.flags.iter().any(|f|f=="nonsolid") {return None;}
    let centre=actor.body_center();let extent=half+actor.half_extents;let local=position-centre;
    if local.abs().cmple(extent).all() {
        let depth=extent-local.abs();let axis=if depth.x<depth.z {0}else{2};
        let mut normal=Vec3::ZERO;normal[axis]=if local[axis]>=0.0 {1.0}else{-1.0};
        return (delta.dot(normal)< -0.00001).then_some((0.0,normal));
    }
    let length=delta.length();if length<0.00001 {return None;}
    let distance=ray_box(position,delta/length,centre,extent)?;
    if distance>length {return None;}
    let hit=local+delta*(distance/length);let gap=(hit.abs()-extent).abs();
    let axis=if gap.x<gap.y && gap.x<gap.z {0}else if gap.y<gap.z {1}else{2};
    let mut normal=Vec3::ZERO;normal[axis]=if hit[axis]>=0.0 {1.0}else{-1.0};
    Some((distance/length,normal))
}

/// The result of pushing the player's hull out of the actors' boxes.
pub(crate) struct HullCorrection {pub centre:Vec3,pub velocity:Vec3,pub standing_on_actor:bool}
/// Sweep the hull from the last frame's centre to the new one against the actors (and the world) and slide along what it meets, at most four times.
/// Landing on an actor's box reports `standing_on_actor` (the controller cannot see the box).
pub(crate) fn correct_hull(actors:&[Npc],world:&CollisionWorld,old:Vec3,centre:Vec3,half:Vec3,mut velocity:Vec3)->HullCorrection {
    let (mut position,mut remaining)=(old,centre-old);let mut standing_on_actor=false;
    let needs_correction=actors.iter().any(|a|sweep_actor(position,half,remaining,a).is_some());
    if !needs_correction {return HullCorrection {centre,velocity,standing_on_actor};}
    for _ in 0..4 {
        if remaining.length_squared()<0.000001 {break;}
        let actor_hit=actors.iter().filter_map(|a|sweep_actor(position,half,remaining,a)).min_by(|a,b|a.0.total_cmp(&b.0));
        let world_hit=world.sweep_box(native(position),native(half),native(remaining),0.0).map(|(t,n)|(t,bevy_vec(n)));
        let Some((fraction,normal))=[actor_hit,world_hit].into_iter().flatten().min_by(|a,b|a.0.total_cmp(&b.0)) else {position+=remaining;break;};
        if actor_hit.is_some_and(|h|h.0<=fraction) && normal.y>0.5 {standing_on_actor=true;}
        position+=remaining*(fraction-0.01/remaining.length()).max(0.0);remaining*=1.0-fraction;
        remaining-=normal*remaining.dot(normal).min(0.0);velocity-=normal*velocity.dot(normal).min(0.0);
    }
    HullCorrection {centre:position,velocity,standing_on_actor}
}
/// Run immediately after movement and before door correction. The renderer and
/// both controllers receive the same corrected hull centre; IW4 timers remain intact.
/// The centre is tracked, not the feet: a stance change moves the feet by 34 units while the player stays where he is (docs/retail-movement-audit.md).
pub fn block_player(roster:Res<NpcRoster>,mut walking:ResMut<crate::Walking>,
    session:Res<crate::settings::Session>,opening:Res<crate::opening::Opening>,travel:Res<crate::travel::Travel>,
    mut camera:Single<&mut Transform,With<crate::InspectionCamera>>,mut previous:Local<Option<(u64,Vec3)>>) {
    if session.paused || opening.active {return;}
    if walking.teleported {*previous=Some((travel.arrived,bevy_vec(walking.player.position)));return;}
    let (centre,half,velocity)=(bevy_vec(walking.player.position),bevy_vec(walking.player.half_size()),bevy_vec(walking.player.velocity));
    let mut resolved=centre;
    if let Some((revision,old))=*previous {if revision==travel.arrived && centre.distance(old)<150.0 {
        let correction=correct_hull(&roster.actors,&walking.world,old,centre,half,velocity);
        if correction.standing_on_actor {walking.player.external_support=true;}
        if correction.centre!=centre {resolved=correction.centre;walking.player.velocity=native(correction.velocity);}
    }}
    if (resolved-centre).length_squared()>0.000001 {
        walking.player.position=native(resolved);
        camera.translation+=(resolved-centre)*crate::SCALE;
    }
    *previous=Some((travel.arrived,resolved));
}

#[cfg(test)]
mod tests;
