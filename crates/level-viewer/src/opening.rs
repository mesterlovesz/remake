//! The retail cutscene controller: cshell.dll c_cutscene (0x10011ce0 scenki.txt parser, 0x100130d0 start, 0x10013440 phase change,
//! 0x100132e0 end, 0x10013950 per-frame, 0x10013d90 proximity, 0x10014030 letterbox). Evidence and open points: docs/retail-scenes.md.
use bevy::{camera::visibility::NoFrustumCulling,prelude::*};
use serde::Deserialize;
use std::{collections::BTreeMap,fs};
use crate::{models::Model,ViewerConfig,InspectionCamera,SCALE,retail_ui::{Aid,BitmapText,RetailUi}};

type Cmds=Vec<(String,String)>;
/// The user-visible scenes some o_cutscene object names (rh1-wiezienie1, rh2-wiezienie2, rh12-lab2); the two `szczur` scenes are never placed.
const PLAYED:[&str;4]=["intro","intro zwei","ucieczka z 2 wiezienia","outro"];
/// `intro zwei` ends by itself once its scene clock passes this (cshell 0x100139f2 against the float at 0x10066128).
pub const INTRO_ZWEI_LIMIT:f32=16.0;
/// Letterbox bars are one sixth of the screen height each (cshell 0x10013f04, 0x10066240 = 0.1666667).
const BAR:f32=1.0/6.0;
/// Subtitles use the mincho font at 0.4 (cshell 0x1003422e), wrap at three quarters of the width and sit two lines above the bottom (0x1003450b).
const SUBTITLE_SCALE:f32=0.4;

#[derive(Clone,Deserialize)]
struct Phase {scene:String,duration:f32,model:String,settings:BTreeMap<String,String>,phase:BTreeMap<String,String>,sound:String,speech:String,subtitle:String,runworld:String}
#[derive(Clone,Deserialize)]
struct Attachment {model:String,skins:BTreeMap<String,String>,#[serde(default)]styles:BTreeMap<String,String>,socket:Option<String>,#[serde(default)]commands:Cmds,animation:Option<String>}
#[derive(Clone,Deserialize)]
struct CharPhase {animation:Option<String>,#[serde(rename="loop",default)]looping:bool,#[serde(default)]commands:Cmds}
#[derive(Clone,Deserialize)]
struct Character {default_phase:String,weapon_asset:Option<Attachment>,head_asset:Option<Attachment>,#[serde(default)]header:Cmds,phases:BTreeMap<String,CharPhase>}
#[derive(Deserialize)]
struct ActorDefinition {model:String,skins:BTreeMap<String,String>,#[serde(default)]styles:BTreeMap<String,String>,default:String,animations:BTreeMap<String,String>,#[serde(default)]definition:Option<Character>}

type Piece=(usize,Entity,Handle<Mesh>);
/// A character's `blikN` light (cshell 0x100448bc..0x10044c0a): an additive camera-facing sprite that follows the model socket `socket_blikN` (0x1004a4c0); `size` = full quad in native units.
struct Light {entity:Entity,socket:String,size:Vec2}
/// One character of a scene (`postacN`): the retail controller creates it through the character manager, so it has a body, head
/// and weapon models and runs the phases `setfaza` selects. Its transform is the scene socket every frame.
struct Performer {
    kind:String,scene:String,slot:String,body:Vec<Piece>,head:Vec<Piece>,weapons:Vec<Vec<Piece>>,lights:Vec<Light>,
    animation:String,looping:bool,clock:f32,phase:String,phase_clock:f32,fired:bool,cooldown:f32,armed:bool,matrix:Mat4,
    /// The level already has this character (don Mario): the scene sends the real one away and shows no second copy.
    borrowed:bool,
}
#[derive(Resource)]
pub struct Opening {
    pub active:bool,pub pending_scene:Option<String>,
    index:usize,phase_time:f32,scene_time:f32,phases:Vec<Phase>,timelines:BTreeMap<String,Vec<Phase>>,world:String,triggered:Vec<String>,
    models:BTreeMap<String,Model>,actors:BTreeMap<String,ActorDefinition>,origins:BTreeMap<String,Mat4>,images:Vec<Handle<Image>>,audio:Vec<Handle<AudioSource>>,
    wav_seconds:BTreeMap<String,f32>,performers:Vec<Performer>,sent_away:Vec<String>,
    /// The credits sequence (c_outromgr) that follows the `outro` phase; the scene stays "active" so the world stays frozen.
    pub end:Option<crate::endgame::EndGame>,
}
/// Marks every scene entity so a level change can hide them all.
#[derive(Component)] pub struct Actor;
#[derive(Component)] pub struct Letterbox;
/// Marker the audio diagnostic (audio_trace.rs) looks for; scene sounds are one-shots from crate::audio, which spawn their own entities.
#[derive(Component)] pub struct SceneAudio;
#[derive(Component)] pub struct SubtitleLine(pub usize);
#[derive(Component)] pub struct SubtitleColumn;

impl Opening {
    /// The `outro` cutscene is on screen; retail switches to `muza\outro.wav` there (cshell 0x100131ae).
    pub fn outro(&self)->bool {self.active && self.end.is_none() && self.phases.iter().any(|p|p.phase.contains_key("outro"))}
    pub fn restart(&mut self,active:bool) {
        self.active=active;self.pending_scene=None;self.index=usize::MAX;self.phase_time=0.0;self.scene_time=0.0;self.world.clear();self.triggered.clear();self.end=None;
        self.sent_away.clear();for performer in &mut self.performers {performer.armed=false;}
        if active {self.phases=self.timelines["intro"].clone();}
    }
    /// The scene currently on stage.
    pub fn scene_name(&self)->Option<&str> {self.phases.get(self.index).map(|p|p.scene.as_str())}
    /// Names of the scenes this level can play (its o_cutscene objects).
    pub fn scenes_here(&self)->Vec<String> {self.origins.keys().cloned().collect()}
}

/// Seconds a phase lasts: the shorter of `length` and its `glos` sound; a phase without a sound never waits, and one without
/// a length lasts as long as the sound (cshell 0x10013968..0x100139a0: the sound handle is checked first, then `length`).
pub fn phase_end(length:f32,sound:Option<f32>)->f32 {
    match sound {None=>0.0,Some(sound)=>if length>0.0 {length.min(sound)}else{sound}}
}
/// Retail skip: "use" or "fire" held; the outro cannot be skipped and `intro zwei` ends by itself after 16 s (cshell 0x100139bc..0x10013a06).
pub fn skip_wanted(scene:&str,outro:bool,scene_time:f32,use_held:bool,fire_held:bool)->bool {
    !outro && (use_held || fire_held || (scene=="intro zwei" && scene_time>INTRO_ZWEI_LIMIT))
}
/// Greedy word wrap at `limit` pixels, at most eight lines (cshell 0x10034160).
pub fn wrap_subtitle(text:&str,width:impl Fn(&str)->f32,limit:f32)->Vec<String> {
    let mut lines=Vec::<String>::new();let mut line=String::new();
    for word in text.split_whitespace() {
        let joined=if line.is_empty() {word.to_owned()}else{format!("{line} {word}")};
        if !line.is_empty() && width(&joined)>limit {lines.push(std::mem::take(&mut line));line=word.to_owned();}else{line=joined;}
    }
    if !line.is_empty() {lines.push(line);}
    lines.truncate(8);lines
}
fn has(commands:&[(String,String)],key:&str)->bool {commands.iter().any(|(k,_)|k==key)}
fn value<'a>(commands:&'a [(String,String)],key:&str)->Option<&'a str> {commands.iter().rev().find(|(k,_)|k==key).map(|(_,v)|v.as_str())}
fn number(commands:&[(String,String)],key:&str)->Option<f32> {value(commands,key)?.split_whitespace().next()?.parse().ok()}
/// `on_koniec_anim`, `on_koniec_anim0`...: the phase to enter when the animation ends.
fn on_animation_end(commands:&[(String,String)])->Option<&str> {
    commands.iter().find(|(k,_)|k.strip_prefix("on_koniec_anim").is_some_and(|s|s.bytes().all(|b|b.is_ascii_digit()))).map(|(_,v)|v.as_str())
}
fn socket_index(phase:&Phase,key:&str)->usize {phase.phase.get(key).and_then(|v|v.parse().ok()).unwrap_or(0)}
fn socket_name<'a>(phase:&'a Phase,index:usize)->Option<&'a String> {phase.settings.get(&format!("socket{index}"))}

fn spawn_model(commands:&mut Commands,meshes:&mut Assets<Mesh>,materials:&mut Assets<StandardMaterial>,assets:&AssetServer,images:&mut Vec<Handle<Image>>,
    model:&Model,skins:&BTreeMap<String,String>,styles:&BTreeMap<String,String>)->Vec<Piece> {
    (0..model.pieces.len()).map(|i| {
        let slot=model.pieces[i].texture.to_string();
        let image=skins.get(&slot).map(|path|assets.load(level_viewer::hd::path(path)));
        let alpha_mode=match styles.get(&slot) {Some(s) if s.contains("maska")=>AlphaMode::Mask(0.3),Some(_)=>AlphaMode::Blend,None=>AlphaMode::Opaque};
        if let Some(image)=&image {images.push(image.clone());}
        let mesh=meshes.add(model.mesh(i));
        let entity=commands.spawn((Actor,Mesh3d(mesh.clone()),MeshMaterial3d(materials.add(StandardMaterial {base_color_texture:image,unlit:true,cull_mode:None,alpha_mode,..default()})),
            Transform::from_scale(Vec3::splat(SCALE)),Visibility::Hidden,NoFrustumCulling)).id();
        (i,entity,mesh)
    }).collect()
}

/// First frame image and size of a retail sprite file (`sprites\blik1.spr`) from the decoration or effect export.
fn sprite_frame(output:&std::path::Path,path:&str)->Option<(String,Vec2)> {
    let key=path.replace('\\',"/").to_lowercase();
    for file in ["decor/sprites.json","retail_effects.json"] {
        let Ok(text)=fs::read_to_string(output.join(file)) else {continue};let Ok(manifest)=serde_json::from_str::<serde_json::Value>(&text) else {continue};
        let Some(sprite)=manifest["sprites"].get(&key) else {continue};
        let frame=sprite["frames"].as_array()?.first()?.as_str()?.to_owned();
        return Some((frame,Vec2::new(sprite["width"].as_f64()? as f32,sprite["height"].as_f64()? as f32)));
    }
    None
}

pub fn setup(mut commands:Commands,config:Res<ViewerConfig>,assets:Res<AssetServer>,mut meshes:ResMut<Assets<Mesh>>,mut materials:ResMut<Assets<StandardMaterial>>) {
    let phases:Vec<Phase>=serde_json::from_str(&fs::read_to_string(config.output.join("opening.json")).expect("opening export")).unwrap();
    let mut timelines:BTreeMap<String,Vec<Phase>>=fs::read_to_string(config.output.join("campaign_cutscenes.json")).ok().and_then(|s|serde_json::from_str(&s).ok()).unwrap_or_default();
    timelines.insert("intro".into(),phases.clone());
    timelines.retain(|name,_|PLAYED.contains(&name.as_str()));
    let actors:BTreeMap<String,ActorDefinition>=serde_json::from_str(&fs::read_to_string(config.output.join("actors.json")).expect("actor export")).unwrap();
    let mut models=BTreeMap::<String,Model>::new();let mut images=Vec::new();let mut audio=Vec::new();let mut wav_seconds=BTreeMap::new();
    let load=|path:&str,models:&mut BTreeMap<String,Model>| if !path.is_empty() && !models.contains_key(path) {models.insert(path.to_owned(),Model::load(&config.output,path));};
    for phase in timelines.values().flatten() {
        load(&phase.model,&mut models);
        for sound in [&phase.sound,&phase.speech].into_iter().filter(|s|!s.is_empty()) {
            audio.push(assets.load(crate::audio::asset_path(sound)));
            if let Some(seconds)=crate::dialogue::wav_seconds(&config.output.join(crate::audio::asset_path(sound))) {wav_seconds.insert(sound.clone(),seconds);}
        }
    }
    let kinds:std::collections::BTreeSet<&String>=timelines.values().flatten().flat_map(|p|p.settings.iter()).filter(|(k,_)|k.starts_with("postac")).map(|(_,kind)|kind).collect();
    for definition in kinds.iter().filter_map(|kind|actors.get(*kind)) {
        load(&definition.model,&mut models);
        if let Some(character) = &definition.definition {for attachment in [&character.weapon_asset,&character.head_asset].into_iter().flatten() {load(&attachment.model,&mut models);}}
    }
    let mut performers=Vec::new();let mut spawned=std::collections::BTreeSet::new();let light_quad=meshes.add(Rectangle::new(1.0,1.0));
    for phase in timelines.values().flatten() {
        if !spawned.insert(phase.scene.clone()) {continue;}
        for (key,kind) in phase.settings.iter().filter(|(k,_)|k.strip_prefix("postac").is_some_and(|s|!s.is_empty() && s.bytes().all(|b|b.is_ascii_digit()))) {
            let Some(definition)=actors.get(kind) else {continue};
            let body=spawn_model(&mut commands,&mut meshes,&mut materials,&assets,&mut images,&models[&definition.model],&definition.skins,&definition.styles);
            let (mut head,mut weapons,mut lights)=(Vec::new(),Vec::new(),Vec::new());
            if let Some(character)=&definition.definition {
                // `i_limuzyna` only: blik0/1 = blik1.spr at the front sockets (scale 0.2), blik2/3 = blikred.spr at the rear (0.1); white, always on.
                for index in 0..4 {
                    let (Some(path),Some(socket))=(value(&character.header,&format!("blik{index}")),value(&character.header,&format!("socket_blik{index}"))) else {continue};
                    let Some((frame,size))=sprite_frame(&config.output,path) else {continue};
                    let scale=number(&character.header,&format!("skala_blik{index}")).unwrap_or(1.0);
                    let material=materials.add(StandardMaterial {base_color_texture:Some(assets.load(frame)),unlit:true,cull_mode:None,alpha_mode:AlphaMode::Add,fog_enabled:false,..default()});
                    let entity=commands.spawn((Actor,Mesh3d(light_quad.clone()),MeshMaterial3d(material),Transform::default(),Visibility::Hidden,NoFrustumCulling)).id();
                    lights.push(Light {entity,socket:socket.to_owned(),size:size*scale*2.0});
                }
                if let Some(h)=&character.head_asset {head=spawn_model(&mut commands,&mut meshes,&mut materials,&assets,&mut images,&models[&h.model],&h.skins,&h.styles);}
                if let Some(w)=&character.weapon_asset {
                    // A second `socket_weapon1` carries a second copy of the gun (donmario_outro fires two MAC-10s).
                    let count=1+value(&character.header,"socket_weapon1").is_some() as usize;
                    for _ in 0..count {weapons.push(spawn_model(&mut commands,&mut meshes,&mut materials,&assets,&mut images,&models[&w.model],&w.skins,&w.styles));}
                }
            }
            performers.push(Performer {kind:kind.clone(),scene:phase.scene.clone(),slot:key[6..].to_owned(),body,head,weapons,lights,animation:String::new(),looping:true,clock:0.0,phase:String::new(),
                phase_clock:0.0,fired:false,cooldown:0.0,armed:false,matrix:Mat4::IDENTITY,borrowed:false});
        }
    }
    for top in [true,false] {
        commands.spawn((Letterbox,Visibility::Hidden,Node {position_type:PositionType::Absolute,left:px(0),right:px(0),height:percent(BAR*100.0),top:if top {px(0)}else{Val::Auto},bottom:if top {Val::Auto}else{px(0)},..default()},BackgroundColor(Color::BLACK),GlobalZIndex(10)));
    }
    // Up to eight subtitle lines; the block is bottom aligned, its last line ends two line heights above the bottom edge (0x1003450b: line i of n at
    // y = H + (i - n - 2) h). Empty lines take no room, so one line sits on the lower letterbox bar and longer texts grow upwards.
    commands.spawn((SubtitleColumn,Node {position_type:PositionType::Absolute,left:px(0),right:px(0),bottom:px(64.0*SUBTITLE_SCALE*2.0),flex_direction:FlexDirection::Column,align_items:AlignItems::Center,..default()},GlobalZIndex(11))).with_children(|column| {
        for i in 0..8 {column.spawn((SubtitleLine(i),BitmapText::new("mincho","",SUBTITLE_SCALE,Color::WHITE).aided(Aid::BACKED),Node {flex_direction:FlexDirection::Row,..default()}));}
    });
    commands.insert_resource(Opening {active:config.story,pending_scene:None,index:usize::MAX,phase_time:0.0,scene_time:0.0,phases,timelines,world:String::new(),triggered:Vec::new(),models,actors,
        origins:BTreeMap::new(),images,audio,wav_seconds,performers,sent_away:Vec::new(),end:None});
}

/// The retail text calls scale by `screen width / 1024` (0x10034226); the UI scale is `min(w / 1024, h / 768)`, so widescreen subtitles grow by the ratio.
pub fn subtitle_k(width:f32,ui_scale:f32)->f32 {width/1024.0/ui_scale.max(0.01)}
/// Glyph size and bottom offset of the subtitle block follow the window every frame.
pub fn subtitle_fit(window:Single<&Window>,scale:Res<UiScale>,mut lines:Query<&mut BitmapText,With<SubtitleLine>>,mut column:Query<&mut Node,With<SubtitleColumn>>) {
    let k=subtitle_k(window.width(),scale.0);
    for mut text in &mut lines {if text.scale!=SUBTITLE_SCALE*k {text.scale=SUBTITLE_SCALE*k;}}
    for mut node in &mut column {let bottom=px(64.0*SUBTITLE_SCALE*k*2.0);if node.bottom!=bottom {node.bottom=bottom;}}
}

/// Original o_cutscene proximity volumes (cshell 0x10013e04: the 3-D distance from the player to the object is below `detection_radius`).
pub fn triggers(mut opening:ResMut<Opening>,config:Res<ViewerConfig>,walking:Res<crate::Walking>,session:Res<crate::settings::Session>,front:Res<crate::frontend::Frontend>) {
    if session.paused {return;}
    if opening.world!=config.world {
        opening.world=config.world.clone();opening.triggered.clear();opening.origins.clear();
        let scene:serde_json::Value=serde_json::from_str(&fs::read_to_string(config.output.join(format!("{}.scene.json",config.world))).unwrap()).unwrap();
        for object in scene["objects"].as_array().unwrap().iter().filter(|o|o["kind"]=="o_cutscene") {
            let p=&object["properties"];let Some(name)=p["Rodzaj_cuts"].as_str() else{continue};
            opening.origins.insert(name.into(),Mat4::from_rotation_translation(Quat::from_rotation_y(p["Rotation"][1].as_f64().unwrap_or(0.0) as f32),crate::doors::vector(&p["Pos"])));
        }
    }
    if opening.active || session.dialogue_active || front.loading.is_some() {return;}
    let position=Vec3::new(walking.player.position.x,walking.player.position.y,walking.player.position.z);
    let requested=opening.pending_scene.take();
    let next=requested.filter(|name|opening.origins.contains_key(name) && opening.timelines.contains_key(name)).or_else(||opening.origins.iter().find_map(|(name,origin)| {
        if opening.triggered.contains(name) {return None;}
        let phases=opening.timelines.get(name)?;let first=phases.first()?;
        let radius=first.settings.get("detection_radius")?.parse::<f32>().ok()?;
        (radius>0.0 && position.distance(origin.w_axis.truncate())<radius).then(||name.clone())
    }));
    if let Some(name)=next {
        opening.phases=opening.timelines[&name].clone();opening.triggered.push(name.clone());opening.active=true;opening.index=usize::MAX;opening.phase_time=0.0;opening.scene_time=0.0;opening.end=None;
        info!("Átvezető indult: {name}");
    }
}

#[derive(bevy::ecs::system::SystemParam)]
pub struct Stage<'w,'s> {
    meshes:ResMut<'w,Assets<Mesh>>,visuals:Query<'w,'s,(&'static mut Transform,&'static mut Visibility),(Without<InspectionCamera>,Without<Letterbox>,Without<SubtitleLine>)>,
    bars:Query<'w,'s,&'static mut Visibility,(With<Letterbox>,Without<InspectionCamera>)>,lines:Query<'w,'s,(&'static SubtitleLine,&'static mut BitmapText)>,
    ui:Res<'w,RetailUi>,window:Single<'w,'s,&'static Window>,scale:Res<'w,UiScale>,keys:Res<'w,ButtonInput<KeyCode>>,buttons:Res<'w,ButtonInput<MouseButton>>,
}

fn show(stage:&mut Stage,entities:&[Piece],visible:bool) {for (_,entity,_) in entities {if let Ok((_,mut v))=stage.visuals.get_mut(*entity) {*v=if visible {Visibility::Visible}else{Visibility::Hidden};}}}

/// The scene is over: every character leaves, the subtitle clears and the level's own characters return (cshell 0x100132e0).
fn end_scene(opening:&mut Opening,stage:&mut Stage,roster:&mut crate::npcs::NpcRoster) {
    for performer in opening.performers.iter_mut() {
        performer.armed=false;
        let pieces:Vec<Piece>=performer.body.iter().chain(&performer.head).chain(performer.weapons.iter().flatten()).cloned().collect();
        show(stage,&pieces,false);
    }
    for name in std::mem::take(&mut opening.sent_away) {roster.set_scene_hidden(&name,false);}
    for (_,mut line) in stage.lines.iter_mut() {line.set("");}
}

/// A character enters one of its own phases (cshell 0x10041c60): the phase animation plays, a phase with `show_weapon` draws the gun.
fn enter_character_phase(performer:&mut Performer,character:&Character,name:&str) {
    let Some(phase)=character.phases.get(name) else {return};
    performer.phase=name.to_owned();performer.phase_clock=0.0;performer.fired=false;
    if let Some(animation)=phase.animation.as_ref().filter(|a|!a.is_empty()) {performer.animation=animation.clone();performer.looping=phase.looping;performer.clock=0.0;}
    if has(&phase.commands,"show_weapon") {performer.armed=true;}
    if has(&phase.commands,"drop_weapon") {performer.armed=false;}
}

pub fn tick(mut commands:Commands,mut opening:ResMut<Opening>,time:Res<Time>,assets:Res<AssetServer>,mut camera:Single<&mut Transform,(With<InspectionCamera>,Without<Actor>,Without<Letterbox>,Without<SubtitleLine>)>,
    mut travel:ResMut<crate::travel::Travel>,session:Res<crate::settings::Session>,mut props:ResMut<crate::models::PropAnimations>,mut roster:ResMut<crate::npcs::NpcRoster>,mut stage:Stage) {
    for mut v in &mut stage.bars {*v=if opening.active && opening.end.is_none() {Visibility::Visible}else{Visibility::Hidden};}
    if !opening.active || session.paused || opening.end.is_some() {return;}
    if !opening.images.iter().all(|h|assets.is_loaded_with_dependencies(h.id())) || !opening.audio.iter().all(|h|assets.is_loaded_with_dependencies(h.id())) {return;}
    let dt=time.delta_secs();
    let mut entered=false;
    if opening.index==usize::MAX {opening.index=0;opening.phase_time=0.0;opening.scene_time=0.0;entered=true;}
    else {opening.phase_time+=dt;opening.scene_time+=dt;}
    let index=opening.index;
    let scene=opening.phases[index].scene.clone();
    let outro=opening.phases.iter().any(|p|p.phase.contains_key("outro"));
    // Skip: the "use" or "fire" command held (E, Space, left mouse); retail then runs to the last runworld of the scene.
    let use_held=stage.keys.pressed(KeyCode::KeyE) || stage.keys.pressed(KeyCode::Space);
    if !entered && skip_wanted(&scene,outro,opening.scene_time,use_held,stage.buttons.pressed(MouseButton::Left)) {
        let target=opening.phases.iter().rev().find(|p|!p.runworld.is_empty()).map(|p|p.runworld.clone());
        end_scene(&mut opening,&mut stage,&mut roster);opening.active=false;
        if let Some(world) = target {travel.pending=Some(world.replace('\\',"/").trim_start_matches("worlds/").trim_end_matches(".dat").to_lowercase());}
        return;
    }
    // Phase change: the current phase ends with its sound or its `length`, whichever comes first.
    let mut index=index;
    if !entered {
        let phase=&opening.phases[index];
        let sound=(!phase.sound.is_empty()).then(||opening.wav_seconds.get(&phase.sound).copied().unwrap_or(0.0));
        if opening.phase_time>=phase_end(phase.duration,sound) {
            index+=1;
            if index>=opening.phases.len() {end_scene(&mut opening,&mut stage,&mut roster);opening.active=false;return;}
            opening.index=index;opening.phase_time=0.0;entered=true;
        }
    }
    let phase=opening.phases[index].clone();
    let new_scene=index==0 || opening.phases[index-1].scene!=phase.scene;
    if entered {
        if new_scene {opening.scene_time=0.0;}
        if !phase.runworld.is_empty() {
            end_scene(&mut opening,&mut stage,&mut roster);opening.active=false;
            travel.pending=Some(phase.runworld.replace('\\',"/").trim_start_matches("worlds/").trim_end_matches(".dat").to_lowercase());
            return;
        }
        if phase.phase.contains_key("outro") {
            end_scene(&mut opening,&mut stage,&mut roster);
            opening.end=Some(crate::endgame::EndGame::new());
            return;
        }
        info!("Nyitójelenet: {} / {}",phase.scene,phase.phase.get("anim").map(String::as_str).unwrap_or(""));
        let Opening {models,performers,actors,origins,sent_away,..}=&mut *opening;
        let Some(origin)=origins.get(&phase.scene).copied() else {return};
        let Some(rig)=models.get(&phase.model) else {return};
        let rig_pose=rig.pose(phase.phase.get("anim").map(String::as_str).unwrap_or(""),0.0,false);
        // The scene creates its characters: those the level already has are sent under the floor (`dupa` socket) and come back at the end.
        if new_scene {
            for name in std::mem::take(sent_away) {roster.set_scene_hidden(&name,false);}
            for performer in performers.iter_mut().filter(|p|p.scene!=phase.scene) {
                performer.armed=false;
                let pieces:Vec<Piece>=performer.body.iter().chain(&performer.head).chain(performer.weapons.iter().flatten()).cloned().collect();
                show(&mut stage,&pieces,false);
            }
            for performer in performers.iter_mut().filter(|p|p.scene==phase.scene) {
                performer.borrowed=roster.actors.iter().any(|a|a.definition_name==performer.kind);
                if performer.borrowed && !sent_away.contains(&performer.kind) {roster.set_scene_hidden(&performer.kind,true);sent_away.push(performer.kind.clone());}
                performer.armed=false;performer.clock=0.0;performer.phase.clear();
                let definition=&actors[&performer.kind];
                performer.animation=definition.default.clone();performer.looping=true;
                if let Some(character)=&definition.definition {let name=character.default_phase.clone();enter_character_phase(performer,character,&name);}
            }
        }
        for performer in performers.iter_mut().filter(|p|p.scene==phase.scene) {
            let definition=&actors[&performer.kind];
            // SetCurAnim(anim_postacN) restarts the clip every phase that names one (cshell 0x100450b0, loop on).
            if let Some(name)=phase.phase.get(&format!("anim_postac{}",performer.slot)) {
                let animation=definition.animations.get(name).unwrap_or(name);
                if models[&definition.model].animation_duration(animation).is_some() {performer.animation=animation.clone();performer.looping=true;performer.clock=0.0;}
            }
            // setfaza0 <phase> <character>: after the animation commands (cshell 0x100137b9).
            for (key,spec) in phase.phase.iter().filter(|(k,_)|k.starts_with("setfaza")) {
                let _ = key;
                if let Some((wanted,character_name))=spec.split_once(' ') {
                    if character_name.trim()==performer.kind {if let Some(character)=&definition.definition {enter_character_phase(performer,character,wanted.trim());}}
                }
            }
        }
        // The scene `sound` is a local (2D) sound in retail (PlaySoundInfo flags 0x1210 at 0x10013012: no 3D bit, the position and the radius 1280 it
        // also fills in are never used); "Javított 3D hangzás" places it at socket_glos. Earlier sounds keep playing (nothing stops the old handle).
        let glos_at=socket_name(&phase,socket_index(&phase,"socket_glos")).and_then(|name|rig.try_socket(&rig_pose,name)).map(|m|(origin*m).transform_point3(Vec3::ZERO));
        if !phase.sound.is_empty() {crate::audio::play_voice(&mut commands,&assets,&phase.sound,glos_at.unwrap_or(origin.w_axis.truncate()));}
        if !phase.speech.is_empty() {crate::audio::play_2d(&mut commands,&assets,&phase.speech);}
        let limit=stage.window.width()/stage.scale.0*0.75;let glyph=SUBTITLE_SCALE*subtitle_k(stage.window.width(),stage.scale.0);
        let lines={let font=stage.ui.fonts.get("mincho");wrap_subtitle(&phase.subtitle,|t|font.map_or(t.chars().count() as f32*32.0*glyph,|f|f.width(t,glyph)),limit)};
        for (line,mut text) in stage.lines.iter_mut() {text.set(lines.get(line.0).cloned().unwrap_or_default());}
        if let Some(value)=phase.phase.get("object_anim") {let parts:Vec<_>=value.split_whitespace().collect();if let (Some(animation),Some(target))=(parts.first(),parts.get(1)) {props.pending.push((target.to_string(),animation.to_string()));}}
    }
    // Camera and characters follow the sockets of the scene model every frame.
    let Opening {models,performers,actors,origins,phase_time,..}=&mut *opening;
    let Some(origin)=origins.get(&phase.scene).copied() else {return};
    let Some(rig)=models.get(&phase.model) else {return};
    let rig_pose=rig.pose(phase.phase.get("anim").map(String::as_str).unwrap_or(""),*phase_time,false);
    if let Some(cam) = socket_name(&phase,socket_index(&phase,"socket_kamera")).and_then(|name|rig.try_socket(&rig_pose,name)) {
        let (_,rotation,translation)=(origin*cam).to_scale_rotation_translation();
        camera.translation=translation*SCALE;camera.rotation=rotation*Quat::from_rotation_y(std::f32::consts::PI);
    }
    for performer in performers.iter_mut() {
        let visible=performer.scene==phase.scene && !performer.borrowed;
        if performer.scene!=phase.scene {continue;}
        let definition=&actors[&performer.kind];let model=&models[&definition.model];
        performer.clock+=dt;performer.phase_clock+=dt;performer.cooldown=(performer.cooldown-dt).max(0.0);
        if let Some(matrix)=socket_name(&phase,socket_index(&phase,&format!("socket_postac{}",performer.slot))).and_then(|name|rig.try_socket(&rig_pose,name)) {performer.matrix=origin*matrix;}
        let (scale,rotation,translation)=performer.matrix.to_scale_rotation_translation();
        let body=Transform {translation:translation*SCALE,rotation,scale:scale*SCALE};
        let pose=model.pose(&performer.animation,performer.clock,performer.looping);
        for (piece,entity,mesh) in &performer.body {
            if let Some(mesh)=stage.meshes.get_mut(mesh) {model.animate_mesh(*piece,&pose,mesh);}
            if let Ok((mut t,mut v))=stage.visuals.get_mut(*entity) {*t=body;*v=if visible {Visibility::Visible}else{Visibility::Hidden};}
        }
        for light in &performer.lights {
            let Some(joint)=model.try_socket(&pose,&light.socket) else {continue};
            let position=(body.to_matrix()*joint).transform_point3(Vec3::ZERO);
            if let Ok((mut t,mut v))=stage.visuals.get_mut(light.entity) {*t=Transform {translation:position,rotation:camera.rotation,scale:Vec3::new(light.size.x*SCALE*crate::mirror::QUAD_HAND,light.size.y*SCALE,1.0)};*v=if visible {Visibility::Visible}else{Visibility::Hidden};}
        }
        let Some(character)=&definition.definition else {continue};
        let attach=|asset:&Attachment,socket:&str,pieces:&[Piece],stage:&mut Stage,shown:bool| {
            let Some(attached)=models.get(&asset.model) else {return};
            let Some(joint)=model.try_socket(&pose,socket) else {return};
            let transform=Transform::from_matrix(body.to_matrix()*joint);
            let held=attached.pose(asset.animation.as_deref().unwrap_or(""),0.0,false);
            for (piece,entity,mesh) in pieces {
                if let Some(mesh)=stage.meshes.get_mut(mesh) {attached.animate_mesh(*piece,&held,mesh);}
                if let Ok((mut t,mut v))=stage.visuals.get_mut(*entity) {*t=transform;*v=if visible && shown {Visibility::Visible}else{Visibility::Hidden};}
            }
        };
        if let (Some(head),Some(socket))=(&character.head_asset,character.head_asset.as_ref().and_then(|h|h.socket.as_deref())) {attach(head,socket,&performer.head,&mut stage,true);}
        if let Some(gun)=&character.weapon_asset {
            let sockets=[gun.socket.as_deref(),value(&character.header,"socket_weapon1")];
            for (pieces,socket) in performer.weapons.iter().zip(sockets) {if let Some(socket)=socket {attach(gun,socket,pieces,&mut stage,performer.armed);}}
        }
        // Phase logic of the character (shots of `strzal_raz`, `on_koniec_anim` loops): cshell 0x100462e0 as the NPC code does it.
        let Some(phase_def)=character.phases.get(&performer.phase) else {continue};
        let mut next=None;
        let animation_length=model.animation_duration(&performer.animation).unwrap_or(0.0);
        if !phase_def.looping && performer.phase_clock>=animation_length.max(0.04) {next=on_animation_end(&phase_def.commands).map(str::to_owned);}
        let shooting=phase_def.commands.iter().any(|(k,_)|k.starts_with("strzal_raz"));
        if shooting && !performer.fired && performer.cooldown<=0.0 && performer.armed {
            performer.fired=true;
            let weapon=character.weapon_asset.as_ref();
            performer.cooldown=weapon.and_then(|w|number(&w.commands,"shot_latency")).unwrap_or(0.5).max(0.001);
            if let Some(gun)=weapon {
                let guns=1+phase_def.commands.iter().any(|(k,_)|k=="strzal_raz1") as usize;
                for barrel in 0..guns.min(performer.weapons.len()) {
                    let socket=if barrel==0 {gun.socket.as_deref()} else {value(&character.header,"socket_weapon1")};
                    let Some(attached)=models.get(&gun.model) else {continue};
                    let Some(joint)=socket.and_then(|s|model.try_socket(&pose,s)) else {continue};
                    let held=attached.pose(gun.animation.as_deref().unwrap_or(""),0.0,false);
                    let base=performer.matrix*joint;
                    let point=|key:&str|attached.try_socket(&held,value(&gun.commands,key)?).map(|s|(base*s).transform_point3(Vec3::ZERO));
                    let Some(muzzle)=point("socket_lezacy_strzal") else {continue};
                    let heading=performer.matrix.transform_vector3(Vec3::Z).normalize_or_zero();
                    roster.push_shot(crate::npcs::EnemyShot {origin:muzzle,endpoint:muzzle+heading*640.0,commands:gun.commands.clone(),casing:point("socket_lezacy_luska_strzal"),hits_player:false,melee:false,bite:false,extra:false,victim:false});
                    if barrel==0 {
                        if let Some(path)=value(&gun.commands,"sound_shoot") {
                            crate::audio::play_near(&mut commands,&assets,path,muzzle,crate::audio::npc_weapon_radius(number(&gun.commands,"glosnosc").unwrap_or(0.0),false,true));
                        }
                    }
                }
            }
        }
        if let Some(name)=next {enter_character_phase(performer,character,&name);}
    }
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn a_phase_ends_with_its_sound_or_its_length_whichever_is_first() {
        assert_eq!(phase_end(3.1,Some(4.5)),3.1);assert_eq!(phase_end(6.0,Some(6.0)),6.0);assert_eq!(phase_end(5.0,Some(3.0)),3.0);
        assert_eq!(phase_end(0.0,Some(2.0)),2.0,"no length: the sound decides");assert_eq!(phase_end(4.0,None),0.0,"no sound: the phase never waits");
    }
    #[test] fn use_fire_and_the_intro_zwei_clock_skip_but_never_the_outro() {
        assert!(skip_wanted("intro",false,1.0,true,false) && skip_wanted("intro",false,1.0,false,true) && !skip_wanted("intro",false,1.0,false,false));
        assert!(!skip_wanted("intro zwei",false,16.0,false,false) && skip_wanted("intro zwei",false,16.01,false,false));
        assert!(!skip_wanted("outro",true,100.0,true,true),"the outro cannot be skipped");
    }
    #[test] fn subtitles_wrap_by_width_and_stop_at_eight_lines() {
        let width=|t:&str|t.chars().count() as f32*10.0;
        assert_eq!(wrap_subtitle("aa bb cc dd",width,60.0),vec!["aa bb","cc dd"]);
        assert_eq!(wrap_subtitle("",width,60.0),Vec::<String>::new());
        assert_eq!(wrap_subtitle(&"word ".repeat(40),width,50.0).len(),8);
    }
    #[test] fn character_phases_pick_the_end_of_animation_callback() {
        let commands:Cmds=vec![("animacja".into(),"04".into()),("on_koniec_anim0".into(),"strzela".into())];
        assert_eq!(on_animation_end(&commands),Some("strzela"));assert!(has(&commands,"animacja") && number(&commands,"missing").is_none());
    }
    #[test] #[ignore] fn every_played_scene_names_only_files_the_export_has() {
        let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
        let audit:serde_json::Value=serde_json::from_str(&fs::read_to_string(root.join("scenes-audit.json")).unwrap()).unwrap();
        for scene in PLAYED {assert!(audit["scenes"][scene]["missing"].as_array().unwrap().is_empty(),"{scene}: {}",audit["scenes"][scene]["missing"]);}
        let phases:Vec<Phase>=serde_json::from_str(&fs::read_to_string(root.join("opening.json")).unwrap()).unwrap();
        assert_eq!(phases.iter().filter(|p|!p.runworld.is_empty()).count(),1);
        assert!((phases.iter().map(|p|p.duration).sum::<f32>()-77.97).abs()<0.01);
    }
}
