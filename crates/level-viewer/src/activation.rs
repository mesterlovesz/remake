//! Original object activation: one dispatcher for Nast_obiekt chains between
//! scene objects, the use key on o_obiekt, o_pauza timers, energy beams, light
//! groups and hazard zones. Server registry and activate(): object.lto
//! 0x1002cc08 / 0x10009350; client handlers: cshell.dll 0x1002f5a1..0x1002f750.
use bevy::{prelude::*,ecs::system::SystemParam};
use serde::{Deserialize,Serialize};
use serde_json::Value;
use std::collections::{BTreeMap,BTreeSet,VecDeque};
use crate::{InspectionCamera,Walking,SCALE};

/// Audible radius of a prop's animation sound (`sound01`, `sound10`; cshell 0x1002e520 pushes 1536.0).
pub const PROP_SOUND_RADIUS:f32=1536.0;
/// objects.txt entry of an o_obiekt: state animations, sounds.
type Definition=BTreeMap<String,String>;

#[derive(Resource,Default)] pub struct Activation {
    /// Scene objects by Name; like the retail registry the last duplicate wins.
    objects:BTreeMap<String,(String,Value)>,
    definitions:BTreeMap<String,Definition>,
    /// Names to activate. The original runs a whole chain synchronously, so
    /// everything queued is resolved within one frame.
    pub queue:VecDeque<String>,
    /// Activations by the player's use key, the door watcher or an NPC (`byPlayer` = 1: refused by `Gracz_otwiera` 0 objects), with the point
    /// `Od_gracza` swings away from when it is not the player. The rest of a chain always runs with byPlayer 0.
    pub manual:VecDeque<(String,Option<Vec3>)>,
    /// Door requests of actors (`doors::npc_open`): the door opens, its `Nast_obiekt` chain does not follow (object.lto 0x10009270).
    pub npc_requests:VecDeque<(String,Vec3)>,
    /// Player position (native units) for `Od_gracza`.
    pub(crate) player:Vec3,
    /// Toggled state: o_obiekt open, LightGroup lit, beam lit, hazard on Sila1.
    pub(crate) states:BTreeMap<String,bool>,
    pauses:BTreeMap<String,f32>,
    beams:BTreeMap<String,f32>,
    emitted:BTreeMap<String,f32>,
    /// o_obiekt transitions still playing (the use key ignores those).
    transitions:BTreeMap<String,f32>,
    warned:BTreeSet<String>,
    clock:f32,
    /// Player-volume markers: (name, kind, centre, half extents).
    markers:Vec<(String,String,Vec3,Vec3)>,
    /// `Death_nast_obiekt` targets waiting to be killed (props are killed by props::tick; light groups and beams here).
    pub(crate) deaths:VecDeque<String>,
    /// Killed light groups / beams: their state no longer toggles (object.lto 0x10010350).
    pub(crate) dead:BTreeSet<String>,
    /// LightGroup colour/flicker state (object.lto 0x1000ff90..0x10010350) and whether every group has sent its first colour.
    pub(crate) groups:BTreeMap<String,Group>,groups_sent:bool,
    /// Detector countdowns (run down always) and variable-marker repeat timers (run up inside).
    marker_timers:BTreeMap<String,f32>,
    /// Markers already triggered (`Jednokrotny` detectors, dialogue markers); saved with the game.
    pub(crate) fired:BTreeSet<String>,
}
/// Serializable part for manual saves.
#[derive(Clone,Default,Serialize,Deserialize)] pub(crate) struct Saved {
    #[serde(default)] states:BTreeMap<String,bool>,#[serde(default)] pauses:BTreeMap<String,f32>,#[serde(default)] beams:BTreeMap<String,f32>,
    #[serde(default)] fired:BTreeSet<String>,#[serde(default)] marker_timers:BTreeMap<String,f32>,#[serde(default)] dead:BTreeSet<String>,
}
#[derive(Component)] pub struct BeamSegment {beam:String}
/// A LightGroup's `CzestoscMigania` flicker: a hard square wave that starts dark, flips every `period` seconds and shows `StartColor` while the
/// phase is set (apply, object.lto 0x10010200; Update flips the phase and re-arms the timer, 0x1000ff90). Period 0 is a plain on/off group.
#[derive(Clone,Debug,Default,PartialEq)] pub struct Group {pub period:f32,pub color:Vec3,pub phase:bool,pub timer:f32}
impl Group {
    /// Advances the flicker; true when the phase flipped.
    pub fn step(&mut self,dt:f32)->bool {
        if self.period<=0.0 {return false;}
        // Lithtech.exe 0x46cd80: the object timer counts down by the server frame time and Update re-arms it with the full period, so the
        // remainder of the frame that flipped the phase is dropped (every half period lasts `period` plus up to one frame).
        self.timer+=dt;
        if self.timer>=self.period {self.timer=0.0;self.phase=!self.phase;return true;}
        false
    }
    /// Colour the group's lights show: black when off, dead, or (for a flickering group) in the dark half period.
    pub fn level(&self,on:bool,dead:bool)->Vec3 {if !dead && on && (self.period<=0.0 || self.phase) {self.color}else{Vec3::ZERO}}
}

impl Activation {
    pub fn from_scene(scene:&Value,objects_txt:&str)->Self {
        let catalog=object_definitions(objects_txt);
        let mut activation=Self::default();
        for object in scene["objects"].as_array().into_iter().flatten() {
            let kind=object["kind"].as_str().unwrap_or("");let p=&object["properties"];
            let Some(name)=p["Name"].as_str() else {continue};
            if !(kind.starts_with("o_") || kind.starts_with("b_door") || kind.starts_with("b_szuflada") || kind=="LightGroup") {continue;}
            if kind=="o_obiekt" {
                if let Some(definition)=catalog.get(&model_key(p["Model"].as_str().unwrap_or(""))) {activation.definitions.insert(name.into(),definition.clone());}
            }
            if kind=="LightGroup" {
                activation.states.insert(name.into(),p["StartOn"].as_i64()!=Some(0));
                let color=p["StartColor"].as_array().filter(|c|c.len()>=3).map_or(Vec3::ONE,|c|Vec3::new(c[0].as_f64().unwrap_or(255.0) as f32,c[1].as_f64().unwrap_or(255.0) as f32,c[2].as_f64().unwrap_or(255.0) as f32)/255.0);
                activation.groups.insert(name.into(),Group {period:p["CzestoscMigania"].as_f64().unwrap_or(0.0) as f32,color,..default()});
            }
            if matches!(kind,"o_marker_dialog"|"o_marker_wykrywacz_postaci"|"o_marker_zmienna") {
                activation.markers.push((name.into(),kind.into(),crate::doors::vector(&p["Pos"]),crate::doors::vector(&p["Promien"])));
            }
            activation.objects.insert(name.into(),(kind.into(),p.clone()));
        }
        // Lights name their group without a LightGroup object (shot-out bulbs switch such a group off).
        for object in scene["objects"].as_array().into_iter().flatten() {
            let group=object["properties"]["LightGroup"].as_str().unwrap_or("");
            if !group.is_empty() && !activation.objects.contains_key(group) {activation.objects.insert(group.into(),("LightGroup".into(),Value::Null));}
        }
        activation
    }
    /// Colour a LightGroup's lights show now.
    pub(crate) fn group_level(&self,name:&str)->Vec3 {
        let (on,dead)=(self.states.get(name).copied().unwrap_or(true),self.dead.contains(name));
        self.groups.get(name).map_or(if on && !dead {Vec3::ONE}else{Vec3::ZERO},|group|group.level(on,dead))
    }
    pub(crate) fn kind(&self,name:&str)->Option<&str> {self.objects.get(name).map(|(kind,_)|kind.as_str())}
    /// Whether the use key visibly does something to this o_obiekt: a chain
    /// target or state animations. Retail lets E toggle any o_obiekt.
    pub(crate) fn usable(&self,name:&str)->bool {
        self.objects.get(name).is_some_and(|(kind,p)|kind=="o_obiekt" && (p["Nast_obiekt"].as_str().is_some_and(|next|self.objects.contains_key(next))
            || self.definitions.get(name).is_some_and(|d|d.contains_key("anim01"))))
    }
    pub(crate) fn saved(&self)->Saved {Saved {states:self.states.clone(),pauses:self.pauses.clone(),beams:self.beams.clone(),fired:self.fired.clone(),marker_timers:self.marker_timers.clone(),dead:self.dead.clone()}}
    /// The `Death_nast_obiekt` of a scene object: the object that dies with it.
    pub(crate) fn death_next(&self,name:&str)->Option<String> {self.objects.get(name)?.1["Death_nast_obiekt"].as_str().filter(|n|!n.is_empty()).map(str::to_owned)}
    pub(crate) fn restore(&mut self,saved:&Saved) {
        self.states.extend(saved.states.clone());self.pauses=saved.pauses.clone();self.beams=saved.beams.clone();
        self.fired=saved.fired.clone();self.marker_timers=saved.marker_timers.clone();self.dead=saved.dead.clone();
    }
    /// Active hazard strength (o_marker_death Sila0, or Sila1 once toggled).
    pub(crate) fn hazard(&self,name:&str,p:&Value)->f32 {
        let key=if self.states.get(name).copied().unwrap_or(false) {"Sila1"}else{"Sila0"};
        p[key].as_f64().unwrap_or(0.0) as f32
    }
    fn warn(&mut self,message:String) {if self.warned.insert(message.clone()) {warn!("{message}");}}
}

/// Stable key of a scene object's name for `audio::SoundOwner`.
fn owner_key(name:&str)->u64 {name.bytes().fold(0xcbf29ce484222325u64,|hash,byte|(hash^byte as u64).wrapping_mul(0x100000001b3))}
fn model_key(path:&str)->String {path.replace('\\',"/").to_lowercase()}
/// objects.txt blocks keyed by model; the first block using a model wins,
/// like tools/export_props.py.
fn object_definitions(source:&str)->BTreeMap<String,Definition> {
    let mut result=BTreeMap::new();let mut current=Definition::new();
    let mut flush=|current:&mut Definition| {let block=std::mem::take(current);if let Some(model)=block.get("model") {result.entry(model_key(model)).or_insert(block);}};
    for line in source.lines() {
        let line=line.split("//").next().unwrap_or("").trim();
        let (key,value)=line.split_once(char::is_whitespace).map(|(k,v)|(k,v.trim())).unwrap_or((line,""));
        if key=="object" {flush(&mut current);current.insert("object".into(),value.into());}
        else if !key.is_empty() {current.insert(key.into(),value.into());}
    }
    flush(&mut current);
    result
}

#[derive(SystemParam)] pub struct Targets<'w,'s> {
    roster:ResMut<'w,crate::npcs::NpcRoster>,campaign:ResMut<'w,crate::campaign::Campaign>,
    doors:Query<'w,'s,&'static mut crate::doors::Door>,props:ResMut<'w,crate::models::PropAnimations>,
    travel:ResMut<'w,crate::travel::Travel>,lights:Option<ResMut<'w,crate::lighting::SourceLights>>,
    commands:Commands<'w,'s>,assets:Res<'w,AssetServer>,session:Res<'w,crate::settings::Session>,world:Res<'w,crate::props::PropWorld>,
}
impl Targets<'_,'_> {
    /// A positional one-shot at the object (silent/log handling lives in crate::audio); prop `sound01/sound10` carry radius 1536 (cshell 0x1002e621), not the 640 of door sounds.
    /// The prop's earlier use sound stops when a new one starts (cshell 0x1002e54e..0x1002e59b kills the handle kept at object +0x1f8).
    fn sound_at(&mut self,path:&str,position:Vec3,owner:&str) {
        if !path.is_empty() {crate::audio::play_near_owned(&mut self.commands,&self.assets,path,position,PROP_SOUND_RADIUS,owner_key(owner));}
    }
}

/// Activate `start` and follow every accepted Nast_obiekt at once (depth first,
/// as the server's synchronous recursion does).
fn dispatch(activation:&mut Activation,targets:&mut Targets,start:String,by_player:bool,activator:Option<Vec3>) {
    let mut stack=vec![start];let mut budget=512;let mut first=true;
    while let Some(name)=stack.pop() {
        let (by_player,who)=if std::mem::take(&mut first) {(by_player,activator.unwrap_or(activation.player))}else{(false,activation.player)};
        budget-=1;if budget==0 {activation.warn(format!("Aktiválási ciklus: {name}"));break;}
        let Some((kind,p))=activation.objects.get(&name).cloned() else {activation.warn(format!("Ismeretlen aktivált objektum: {name}"));continue};
        let accepted=match kind.as_str() {
            "b_door"|"b_szuflada"|"b_szuflada_przestrzelna"=>{
                match targets.doors.iter_mut().find(|door|door.name==name) {
                    // `Gracz_otwiera` 0 refuses the use key and NPCs before anything else (0x100020cd).
                    Some(door) if by_player && !door.usable()=>false,
                    // A level exit jumps even when a chain opens it (object.lto 0x100020c0).
                    Some(door) if !door.destination.is_empty()=>{targets.travel.pending=Some(door.destination.clone());info!("Láncolt pályaváltás: {name} → {}",door.destination);false},
                    // The door plays its own Glos_otwierany / Glos_zamykany when its state changes (doors::tick).
                    Some(mut door)=>door.activate(by_player,who),
                    None=>{let exit=crate::doors::destination(&p);if !exit.is_empty() {targets.travel.pending=Some(exit);false}else{true}},
                }
            },
            "o_obiekt"=>{
                // A destroyed object no longer accepts an activation (object.lto 0x10017580), so its chain ends.
                // Gracz_otwiera 0 refuses the use key too (0x10017580).
                if targets.world.field().is_dead(&name) || (by_player && p["Gracz_otwiera"].as_i64()==Some(0)) {false} else {
                // The server toggles and fires the chain at once; the client plays
                // anim01/sound01 (opening) or anim10/sound10 (closing), then loops the rest pose anim1 / anim0.
                let open=!activation.states.get(&name).copied().unwrap_or(false);activation.states.insert(name.clone(),open);
                let (anim,sound,rest)=if open {("anim01","sound01","anim1")}else{("anim10","sound10","anim0")};
                if let Some(definition)=activation.definitions.get(&name).cloned() {
                    if let Some(animation)=definition.get(anim).filter(|a|!a.is_empty()) {targets.props.pending.push((name.clone(),animation.clone()));}
                    if let Some(animation)=definition.get(rest).filter(|a|!a.is_empty()) {targets.props.rest.insert(name.clone(),animation.clone());}
                    if let Some(path)=definition.get(sound) {targets.sound_at(path,crate::doors::vector(&p["Pos"]),&name);}
                }
                activation.transitions.insert(name.clone(),0.0);
                true}
            },
            // Countdown restarts on each activation; its chain fires when it ends.
            "o_pauza"=>{activation.pauses.insert(name.clone(),p["Pauza"].as_f64().unwrap_or(0.0) as f32);false},
            "o_promien_energii" if activation.dead.contains(&name)=>true,
            "o_promien_energii"=>{
                let lit=!activation.states.get(&name).copied().unwrap_or(false);activation.states.insert(name.clone(),lit);
                if lit {activation.beams.insert(name.clone(),0.0);}else{activation.beams.remove(&name);}
                true
            },
            // A killed group stays dark but still passes the chain on (0x10010330).
            "LightGroup" if activation.dead.contains(&name)=>true,
            "LightGroup"=>{
                let lit=!activation.states.get(&name).copied().unwrap_or(true);activation.states.insert(name.clone(),lit);
                if let Some(lights)=&mut targets.lights {lights.set_group_level(&name,activation.group_level(&name));}
                true
            },
            // Every activation spawns, at most one per 0.5 s (cshell.dll 0x1002f601).
            "o_emiter_postaci"=>{
                let since=activation.clock-activation.emitted.get(&name).copied().unwrap_or(f32::NEG_INFINITY);
                if since>0.5 {activation.emitted.insert(name.clone(),activation.clock);if !targets.roster.activate_emitter(&name) {targets.roster.request_respawn(&name);}info!("Eredeti erősítés: {name}");}
                true
            },
            // Its dialogue plays once per level; the chain continues every time.
            "o_marker_dialog"=>{activation.fired.insert(name.clone());targets.campaign.marker_dialog(&name,p["Dialog"].as_str().unwrap_or(""));true},
            "o_marker_death"=>{let on=!activation.states.get(&name).copied().unwrap_or(false);activation.states.insert(name.clone(),on);true},
            _=>true,
        };
        if accepted {if let Some(next)=p["Nast_obiekt"].as_str().filter(|next|!next.is_empty()) {stack.push(next.into());}}
    }
}

/// Keeps Campaign::aux current so a manual save records doors, chains, fired markers and dropped weapons.
pub fn capture(mut campaign:ResMut<crate::campaign::Campaign>,activation:Res<Activation>,doors:Query<&crate::doors::Door>,pickups:Query<&crate::pickups::Pickup>,roster:Res<crate::npcs::NpcRoster>,time:Res<Time>,mut clock:Local<f32>) {
    *clock+=time.delta_secs();if *clock<0.5 {return;}*clock=0.0;
    campaign.aux=crate::campaign::Aux {
        activation:activation.saved(),hostiles_attacking:roster.hostiles_attacking,
        doors:doors.iter().map(|door|(door.name.clone(),door.state())).filter(|(_,(open,fraction))|*open || *fraction>0.0).collect(),
        drops:pickups.iter().filter_map(|item|Some((item.name.strip_prefix("drop:")?.to_owned(),item.kind.clone(),item.position.to_array()))).collect(),
    };
}
/// Applies a manual save's world state once the level is built (after campaign::load_world).
pub fn restore_aux(mut campaign:ResMut<crate::campaign::Campaign>,mut activation:ResMut<Activation>,mut doors:Query<&mut crate::doors::Door>,mut roster:ResMut<crate::npcs::NpcRoster>,mut drops:ResMut<crate::pickups::DropQueue>,lights:Option<ResMut<crate::lighting::SourceLights>>) {
    let Some(aux)=campaign.take_aux() else {return};
    activation.restore(&aux.activation);
    if let Some(mut lights)=lights {for name in aux.activation.states.keys() {if activation.kind(name)==Some("LightGroup") {lights.set_group_level(name,activation.group_level(name));}}}
    for mut door in &mut doors {if let Some((open,fraction))=aux.doors.get(&door.name) {door.restore(*open,*fraction);}}
    roster.hostiles_attacking=aux.hostiles_attacking;
    for (owner,kind,position) in aux.drops {drops.pending.push((owner,kind,Vec3::from_array(position)));}
}

pub(crate) fn prop_use_hit(origin:Vec3,direction:Vec3,body:&crate::models::PropBody,reach:f32,world:&retail_movement::CollisionWorld,doors:&[&crate::doors::Door])->Option<f32> {
    let low=body.center-body.half_size;let high=body.center+body.half_size;
    let pad=Vec3::splat(crate::doors::USE_AIM_PADDING);
    let distance=crate::doors::ray_box(origin,direction,low-pad,high+pad,reach)?;
    // Check the actual surface: aim assistance must not extend reach or go around walls.
    let surface=(origin+direction*distance).clamp(low,high);let delta=surface-origin;let range=delta.length();
    if range>reach.min(crate::doors::USE_RANGE) {return None;}
    let native=|v:Vec3|retail_movement::Vec3::new(v.x,v.y,v.z);
    if range>0.01 && world.raycast(native(origin),native(delta),range).is_some_and(|hit|hit.0<range-0.5) {return None;}
    if doors.iter().any(|door|door.blocks_use_ray(origin,surface)) {return None;}
    Some(distance)
}

/// E on an o_obiekt within 128 units of the eye (cshell.dll 0x1005faa0); doors
/// and NPC talk are resolved first and consume the key.
pub fn use_objects(mut activation:ResMut<Activation>,mut use_door:ResMut<crate::doors::DoorUse>,bodies:Query<&crate::models::PropBody>,camera:Single<&Transform,With<InspectionCamera>>,
    walking:Res<Walking>,bind:crate::options::Bindings,session:Res<crate::settings::Session>,intro:Res<crate::opening::Opening>,doors:Query<&crate::doors::Door>) {
    if intro.active || session.paused || session.dialogue_active || use_door.target_name.is_some() {return;}
    let origin=camera.translation/SCALE;let direction=*camera.forward();
    let native=|v:Vec3|retail_movement::Vec3::new(v.x,v.y,v.z);
    let reach=walking.world.raycast(native(origin),native(direction),128.0).map_or(128.0,|(hit,_)|hit+1.0);
    let doors=doors.iter().collect::<Vec<_>>();
    let target=bodies.iter().filter_map(|body|prop_use_hit(origin,direction,body,reach,&walking.world,&doors).map(|d|(d,body)))
        .min_by(|a,b|a.0.total_cmp(&b.0)).map(|(_,body)|body.name.clone());
    let Some(name)=target.filter(|name|activation.usable(name)) else {return};
    if activation.transitions.contains_key(&name) {return;}
    if use_door.hint.is_none() {use_door.hint=Some(format!("[{}] Használat / kapcsoló",bind.label(crate::keys_cfg::cmd::ACTION)));}
    // Level triggered like the other users of the action key: the prop manager (0x1002f750) is asked every frame and refuses while the prop's own animation runs.
    if bind.pressed(crate::keys_cfg::cmd::ACTION) && !use_door.consumed {use_door.consumed=true;activation.manual.push_back((name,None));}
}

pub fn tick(mut activation:ResMut<Activation>,mut targets:Targets,time:Res<Time>,intro:Res<crate::opening::Opening>,walking:Res<Walking>,
    mut meshes:ResMut<Assets<Mesh>>,mut materials:ResMut<Assets<StandardMaterial>>,mut segments:Query<(Entity,&BeamSegment,&mut Transform,&mut Visibility)>,mut material_cache:Local<BTreeMap<String,Handle<StandardMaterial>>>,mut beam_mesh:Local<Option<Handle<Mesh>>>) {
    if targets.session.paused || intro.active {return;}
    let dt=time.delta_secs().min(0.05);activation.clock+=dt;
    for value in activation.transitions.values_mut() {*value+=dt;}
    activation.transitions.retain(|_,elapsed|*elapsed<1.0);
    // o_pauza countdowns fire once when they drop below zero (object.lto 0x10008c30).
    let mut due=Vec::new();
    for (name,remaining) in activation.pauses.iter_mut() {*remaining-=dt;if *remaining<0.0 {due.push(name.clone());}}
    // LightGroup flicker: every group sends its colour once, then again whenever its phase flips.
    let first=!std::mem::replace(&mut activation.groups_sent,true);
    // The server clamps its frame time at 0.2 s (Lithtech.exe 0x476784..0x4767d0), not at the 0.05 s the game systems use.
    let server_dt=time.delta_secs().min(0.2);
    let flipped:Vec<String>=activation.groups.iter_mut().filter_map(|(name,group)|(group.step(server_dt) || first).then(||name.clone())).collect();
    if let Some(lights)=&mut targets.lights {for name in &flipped {lights.set_group_level(name,activation.group_level(name));}}
    for name in due {activation.pauses.remove(&name);if let Some(next)=activation.objects.get(&name).and_then(|(_,p)|p["Nast_obiekt"].as_str()).filter(|n|!n.is_empty()).map(str::to_owned) {activation.queue.push_back(next);}}
    // Player-volume markers (client special effects, cshell.dll 0x1002ca40 / 0x1002cc30 / 0x1002cf00).
    let p=walking.player.position;let (position,hull)=(Vec3::new(p.x,p.y,p.z),{let h=walking.player.half_size();Vec3::new(h.x,h.y,h.z)});
    for (name,kind,center,half) in activation.markers.clone() {
        let inside=(position-center).abs().cmple(half+hull).all();
        // The detector countdown runs always; the other markers only act while the player is inside.
        if kind=="o_marker_wykrywacz_postaci" {*activation.marker_timers.entry(name.clone()).or_insert(0.0)-=dt;}
        if !inside {continue;}
        let properties=activation.objects[&name].1.clone();
        match kind.as_str() {
            // Inside, a detector fires at once and then every Czestosc_wykrycia seconds unless Jednokrotny.
            "o_marker_wykrywacz_postaci"=>{
                if properties["Jednokrotny"].as_i64()==Some(1) && activation.fired.contains(&name) {continue;}
                let period=properties["Czestosc_wykrycia"].as_f64().unwrap_or(0.0) as f32;
                if period>0.01 {if activation.marker_timers[&name]>0.0 {continue;}activation.marker_timers.insert(name.clone(),period);}
                activation.fired.insert(name.clone());
                crate::audio::detector_sound(&mut targets.commands,&targets.assets,&properties,center);
                // Special1 (the chapel): the client creates "laska czapel" 128 units behind the player and raises LaskaCzapelRespawnowana (cshell.dll 0x1002c5d0).
                if properties["Special1"].as_i64()==Some(1) {
                    let forward=Quat::from_rotation_y(walking.yaw)*Vec3::NEG_Z;
                    targets.roster.request_client_spawn("laska czapel",position-forward*128.0,forward.x.atan2(forward.z));
                    targets.campaign.set_variable("LaskaCzapelRespawnowana",true);
                }
                activation.queue.push_back(name);
            },
            // A dialogue marker triggers once.
            "o_marker_dialog"=>{if !activation.fired.contains(&name) {activation.queue.push_back(name);}},
            // The variable marker sets/unsets its mission variables every 0.2 s (first at once).
            "o_marker_zmienna"=>{
                let timer=activation.marker_timers.entry(name.clone()).or_insert(0.0);*timer+=dt;
                if *timer<0.0 {continue;}
                *timer=-0.2;
                if let Some(variable)=properties["Set"].as_str().filter(|v|!v.is_empty()) {targets.campaign.set_variable(variable,true);}
                if let Some(variable)=properties["Unset"].as_str().filter(|v|!v.is_empty()) {targets.campaign.set_variable(variable,false);}
                activation.queue.push_back(name);
            },
            _=>{}
        }
    }
    activation.player=position;
    while let Some((name,activator))=activation.manual.pop_front() {dispatch(&mut activation,&mut targets,name,true,activator);}
    while let Some((name,point))=activation.npc_requests.pop_front() {if let Some(mut door)=targets.doors.iter_mut().find(|door|door.name==name) {door.activate(true,point);}}
    while let Some(name)=activation.queue.pop_front() {dispatch(&mut activation,&mut targets,name,false,None);}
    // Death_nast_obiekt: the named object dies too (server death handler 0x10009730); its own Death_nast_obiekt follows.
    while let Some(name)=activation.deaths.pop_front() {
        let Some((kind,p))=activation.objects.get(&name).cloned() else{continue};
        if !activation.dead.insert(name.clone()) {continue;}
        match kind.as_str() {
            "LightGroup"=>{activation.states.insert(name.clone(),false);if let Some(lights)=&mut targets.lights {lights.set_group_level(&name,Vec3::ZERO);}},
            "o_promien_energii"=>{activation.beams.remove(&name);activation.states.insert(name.clone(),false);},
            "o_obiekt"=>targets.world.field().kills.push(name.clone()),
            _=>{}
        }
        if let Some(next)=p["Death_nast_obiekt"].as_str().filter(|n|!n.is_empty()) {activation.deaths.push_back(next.into());}
    }
    // A lit beam switches itself off after Dlugosc_dzialania; nothing fires then.
    let mut ended=Vec::new();
    for elapsed in activation.beams.values_mut() {*elapsed+=dt;}
    for (name,elapsed) in &activation.beams {
        let limit=activation.objects.get(name).and_then(|(_,p)|p["Dlugosc_dzialania"].as_f64()).unwrap_or(0.0) as f32;
        if limit>0.0 && *elapsed>limit {ended.push(name.clone());}
    }
    for name in ended {activation.beams.remove(&name);activation.states.insert(name,false);}
    // Hazard zones: damage per second while the player's box overlaps (cshell.dll 0x1002c930).
    let mut damage=0.0;
    for (name,(kind,properties)) in &activation.objects {
        if kind!="o_marker_death" {continue;}
        let (center,half)=(crate::doors::vector(&properties["Pos"]),crate::doors::vector(&properties["Promien"]));
        if (position-center).abs().cmplt(half+hull).all() {damage+=activation.hazard(name,properties)*dt;}
    }
    // 0x1002ca2c goes through the health-change function, so the painkiller applies (its 60-frame level-start rule is not tracked here).
    if damage>0.0 && targets.campaign.health>0.0 {targets.campaign.change_health(-damage,60);}
    // Energy beams: a flickering bundle of jagged segments while lit.
    let mesh=beam_mesh.get_or_insert_with(||meshes.add(Cuboid::new(1.0,1.0,1.0))).clone();
    let lit:BTreeSet<String>=activation.beams.keys().cloned().collect();
    let mut existing=BTreeMap::<String,Vec<Entity>>::new();
    for (entity,segment,_,_) in &segments {if lit.contains(&segment.beam) {existing.entry(segment.beam.clone()).or_default().push(entity);}else{targets.commands.entity(entity).despawn();}}
    for name in &lit {
        let Some((_,p))=activation.objects.get(name) else {continue};
        let number=|key:&str,default:f32|p[key].as_f64().map_or(default,|v|v as f32);
        let (count,beams)=(number("Ilosc_segmentow",8.0).clamp(1.0,64.0) as usize,number("Ilosc_wiazek",1.0).clamp(1.0,8.0) as usize);
        if !existing.contains_key(name) {
            let color=crate::doors::vector(&p["Kolor"])/255.0;
            let material=material_cache.entry(name.clone()).or_insert_with(||materials.add(StandardMaterial {base_color:Color::srgb(color.x,color.y,color.z),emissive:LinearRgba::rgb(color.x,color.y,color.z)*2.0,unlit:true,alpha_mode:AlphaMode::Add,fog_enabled:false,..default()})).clone();
            let spawned=(0..count*beams).map(|_|targets.commands.spawn((crate::WorldGeometry,BeamSegment {beam:name.clone()},Mesh3d(mesh.clone()),MeshMaterial3d(material.clone()),Transform::default(),Visibility::Hidden)).id()).collect();
            existing.insert(name.clone(),spawned);
        }
    }
    let flicker=activation.clock;
    for (name,entities) in &existing {
        let Some((_,p))=activation.objects.get(name) else {continue};
        let number=|key:&str,default:f32|p[key].as_f64().map_or(default,|v|v as f32);
        let (length,count,twist,width,period)=(number("Dlugosc_segmentu",16.0),number("Ilosc_segmentow",8.0).clamp(1.0,64.0) as usize,number("Pokrecenie_wiazki",4.0),number("Szerokosc_promienia",1.0).max(0.3),number("Czas_zmiany",0.05).max(0.02));
        let angles=&p["Rotation"];let angle=|i:usize|angles[i].as_f64().unwrap_or(0.0) as f32;
        let rotation=Quat::from_euler(EulerRot::YXZ,angle(1),angle(0),angle(2));
        let forward=rotation*Vec3::Z;
        let origin=crate::doors::vector(&p["Pos"]);let step=(flicker/period) as u32;
        for (index,entity) in entities.iter().enumerate() {
            let (wire,link)=(index/count,index%count);
            // Retail (cshell 0x10001660): every link point gets `rand() % ftol(Pokrecenie_wiazki)` added to each world axis (whole units, never negative); the start stays on Pos.
            let jitter=|k:usize,axis:usize|(hash(step,name,wire*97+k*3+axis)*twist.max(1.0)).floor();
            let point=|k:usize|origin+forward*length*k as f32+if k==0 {Vec3::ZERO}else{Vec3::new(jitter(k,0),jitter(k,1),jitter(k,2))};
            let (a,b)=(point(link),point(link+1));let delta=b-a;
            if let Ok((_,_,mut transform,mut visibility))=segments.get_mut(*entity) {
                *transform=Transform {translation:(a+b)*0.5*SCALE,rotation:Quat::from_rotation_arc(Vec3::Z,delta.normalize_or(Vec3::Z)),scale:Vec3::new(width,width,delta.length())*SCALE};
                *visibility=Visibility::Visible;
            }
        }
    }
}
/// Deterministic per-step jitter in [0,1).
fn hash(step:u32,name:&str,k:usize)->f32 {
    let mut h:u32=2166136261;
    for byte in name.bytes().chain(step.to_le_bytes()).chain((k as u32).to_le_bytes()) {h=(h^byte as u32).wrapping_mul(16777619);}
    (h>>8) as f32/16777216.0
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn use_aim_accepts_an_off_center_prop_switch_but_not_a_hidden_one() {
        let body=crate::models::PropBody {name:"lever".into(),center:Vec3::ZERO,half_size:Vec3::splat(2.0)};
        let floor=retail_movement::CollisionWorld::from_obj("v -100 -100 -100\nv 100 -100 -100\nv 0 -100 100\nf 1 2 3").unwrap();
        let origin=Vec3::new(7.0,0.0,100.0);
        assert!(prop_use_hit(origin,-Vec3::Z,&body,128.0,&floor,&[]).is_some(),"small switches should allow a five-unit aim miss");
        assert!(prop_use_hit(Vec3::new(7.0,0.0,140.0),-Vec3::Z,&body,128.0,&floor,&[]).is_none());
        assert!(prop_use_hit(Vec3::new(7.0,0.0,130.0),-Vec3::Z,&body,129.0,&floor,&[]).is_none(),"wall tolerance cannot extend the real 128-unit reach");
        let wall=retail_movement::CollisionWorld::from_obj("v -100 -100 50\nv 100 -100 50\nv 100 100 50\nv -100 100 50\nf 1 2 3 4").unwrap();
        assert!(prop_use_hit(origin,-Vec3::Z,&body,128.0,&wall,&[]).is_none());
    }
    #[test] fn objects_txt_state_animations_are_found_by_model() {
        let catalog=object_definitions("object Wajcha2\n\nmodel models\\levelowe\\wajcha2.ltb\nanim0 zamkniety\nanim01 otwiera\nsound01 sounds\\lewelowe\\drzwi\\wajcha.wav\n// comment\nobject zarowka\nmodel models\\levelowe\\zarowka.ltb\nHP 99999999");
        let lever=&catalog["models/levelowe/wajcha2.ltb"];
        assert_eq!(lever["anim01"],"otwiera");assert_eq!(lever["object"],"Wajcha2");
        assert!(!catalog["models/levelowe/zarowka.ltb"].contains_key("anim01"));
    }
    #[test] fn only_levers_with_an_effect_prompt_for_the_use_key() {
        let scene=serde_json::json!({"objects":[
            {"kind":"o_obiekt","properties":{"Name":"o_obiekt1","Model":"models\\Levelowe\\wajcha2.ltb","Nast_obiekt":"b_door14"}},
            {"kind":"o_obiekt","properties":{"Name":"bin","Model":"models\\Levelowe\\kubel.ltb","Nast_obiekt":""}},
            {"kind":"o_obiekt","properties":{"Name":"lamp","Model":"models\\Levelowe\\zarowka.ltb","Nast_obiekt":"o_lampka7"}},
            {"kind":"b_door","properties":{"Name":"b_door14"}}]});
        let activation=Activation::from_scene(&scene,"object Wajcha2\nmodel models\\levelowe\\wajcha2.ltb\nanim01 otwiera\nobject kubel\nmodel models\\levelowe\\kubel.ltb\nobject zarowka\nmodel models\\levelowe\\zarowka.ltb");
        assert!(activation.usable("o_obiekt1"));
        assert!(!activation.usable("bin"),"a plain prop toggles silently, no prompt");
        assert!(!activation.usable("lamp"),"its chain target does not exist");
    }
    #[test] fn a_saved_game_keeps_fired_detectors_beams_and_light_groups() {
        let scene=serde_json::json!({"objects":[
            {"kind":"o_marker_wykrywacz_postaci","properties":{"Name":"det","Pos":[0,0,0],"Promien":[32,32,32],"Jednokrotny":1}},
            {"kind":"Light","properties":{"Name":"l","LightGroup":"LightGroup4","Pos":[0,0,0]}}]});
        let mut activation=Activation::from_scene(&scene,"");
        assert_eq!(activation.kind("LightGroup4"),Some("LightGroup"),"a group named only by its lights is still a target");
        activation.fired.insert("det".into());activation.states.insert("LightGroup4".into(),false);activation.beams.insert("beam".into(),1.5);
        let text=serde_json::to_string(&activation.saved()).unwrap();
        let mut fresh=Activation::from_scene(&scene,"");fresh.restore(&serde_json::from_str(&text).unwrap());
        assert!(fresh.fired.contains("det"));assert_eq!(fresh.states["LightGroup4"],false);assert_eq!(fresh.beams["beam"],1.5);
        assert!(serde_json::from_str::<Saved>("{}").is_ok(),"older saves without these fields still load");
    }
    #[test] fn hazard_strength_switches_to_sila1_when_toggled() {
        let mut activation=Activation::default();let p=serde_json::json!({"Sila0":50.0,"Sila1":0.0});
        assert_eq!(activation.hazard("o_marker_death5",&p),50.0);
        activation.states.insert("o_marker_death5".into(),true);
        assert_eq!(activation.hazard("o_marker_death5",&p),0.0);
    }
    #[test] fn light_group_flicker_starts_dark_and_flips_every_period() {
        let scene=serde_json::json!({"objects":[{"kind":"LightGroup","properties":{"Name":"LightGroup0","StartOn":1,"StartColor":[70,70,70],"CzestoscMigania":3.0}},
            {"kind":"LightGroup","properties":{"Name":"LightGroup10","StartOn":1,"StartColor":[255,255,255],"CzestoscMigania":0.0}}]});
        let mut activation=Activation::from_scene(&scene,"");
        let grey=Vec3::splat(70.0/255.0);
        assert_eq!(activation.group_level("LightGroup0"),Vec3::ZERO,"phase starts at 0: dark for the first period");
        assert_eq!(activation.group_level("LightGroup10"),Vec3::ONE);
        let group=activation.groups.get_mut("LightGroup0").unwrap();
        assert!(!group.step(2.9));assert!(group.step(0.2));
        assert_eq!(activation.group_level("LightGroup0"),grey);
        assert!(activation.groups.get_mut("LightGroup0").unwrap().step(3.0));assert_eq!(activation.group_level("LightGroup0"),Vec3::ZERO);
        // The overshoot of the flipping frame is dropped (the object timer is re-armed with the full period, Lithtech.exe 0x46cd80).
        let mut late=Group {period:3.0,..default()};assert!(late.step(3.2));assert!(!late.step(2.9),"2.9 s after the flip is still inside the period (the old carry-over flipped here)");assert!(late.step(0.2));
        activation.states.insert("LightGroup10".into(),false);assert_eq!(activation.group_level("LightGroup10"),Vec3::ZERO,"a toggled-off group is black");
        activation.states.insert("LightGroup10".into(),true);activation.dead.insert("LightGroup10".into());assert_eq!(activation.group_level("LightGroup10"),Vec3::ZERO,"a dead group stays dark");
    }
}
