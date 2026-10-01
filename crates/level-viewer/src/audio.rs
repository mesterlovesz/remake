//! Retail sound rules (docs/retail-audio.md) and the one-shot playback wrappers.
//! Pure functions decide which file plays and how far it carries; `play_2d` and
//! `play_near` decide whether anything is created at all. Silent runs never spawn
//! an AudioPlayer, and MESTER_AUDIO_LOG=1 logs what WOULD play (`AUDIO ...` lines)
//! instead of playing, so sound wiring can be checked headlessly.
//!
//! Positional sounds: `play_near` is the single place that stands in for
//! `sound::play_at` (agent/gunfire, linear falloff). It only applies the retail
//! radius as a hard cut-off and plays at full volume. When play_at lands, its
//! body becomes one call to it; every positional call site here (npcs::tick,
//! doors::tick, campaign detectors) already goes through it.
use bevy::prelude::*;
use retail_movement::{CollisionWorld,Vec3 as NativeVec3};
use crate::{InspectionCamera,ViewerConfig,Walking,SCALE,campaign::Campaign,frontend::Frontend,settings::Session,travel::Travel};

/// NPC footsteps `glos_buta1`, 2048 when the NPC sees the player (cshell 0x1004ab51, 0x1004ab63).
pub fn npc_step_radius(sees_player:bool)->f32 {if sees_player {2048.0}else{640.0}}
/// `sound_on_kontakt` "halt" shout (cshell 0x10049aff, 0x10049b0c).
pub fn halt_radius(sees_player:bool)->f32 {if sees_player {2048.0}else{1280.0}}
/// `death_sound`-style scream of a `graj_dzwiek_smierci` character (cshell 0x10042e2c).
pub const DEATH_SCREAM_RADIUS:f32=4024.0;
/// Phase `sound` key.
pub const PHASE_SOUND_RADIUS:f32=2048.0;
/// Door and drawer `Glos_otwierany` / `Glos_zamykany`, played at the object origin.
pub const DOOR_RADIUS:f32=640.0;
/// An NPC weapon reaches `glosnosc`, four times that while it sees the player or in a cutscene.
pub fn npc_weapon_radius(loudness:f32,sees_player:bool,cutscene:bool)->f32 {if sees_player || cutscene {loudness*4.0}else{loudness}}

pub const JUMP:&str="sounds/speech/hero/skok.wav";
pub const HEARTBEAT:&str="sounds/speech/mason/SERDUCHO.WAV";

/// Player footstep pair chosen by the SurfaceFlags under the feet (cshell 0x10061366); the two
/// variants alternate, the first step of a level being variant 2 (the retail toggle starts at 0).
pub fn footstep_path(surface:u16,step:usize)->&'static str {
    let pair=match surface {
        0|6=>["sounds/hero/KROKK1.WAV","sounds/hero/KROKK2.WAV"],
        2|4=>["sounds/hero/KROKM1.WAV","sounds/hero/KROKM2.WAV"],
        3=>["sounds/hero/KROKD1.WAV","sounds/hero/KROKD2.WAV"],
        _=>["sounds/hero/KROK1.WAV","sounds/hero/KROK2.WAV"],
    };
    pair[step%2]
}

/// `sounds_on_kontakt N` makes the digit before ".wav" a random 0..N (cshell 0x10049a9d): only when
/// N > 1 and the path is longer than five characters; a digit above 9 is clamped to 9.
pub fn halt_path(path:&str,variants:u32,pick:u32)->String {
    if variants<=1 || path.len()<=5 || !path.is_ascii() {return path.to_owned();}
    let digit=char::from(b'0'+(pick%variants).min(9) as u8);
    let cut=path.len()-5;
    format!("{}{digit}{}",&path[..cut],&path[cut+1..])
}

/// A `graj_dzwiek_smierci` character screams `deadN.wav` (N = rand % 8) from the folder of its
/// `sound_on_kontakt` file: the last nine characters ("halt0.wav") are overwritten (cshell 0x10042dc5).
pub fn death_scream_path(halt:&str,pick:u32)->Option<String> {
    if halt.len()<=9 || !halt.is_ascii() {return None;}
    Some(format!("{}dead{}.wav",&halt[..halt.len()-9],pick%8))
}

/// Retail footstep timer (cshell 0x1004ab2b): a phase with `odstep_glosow_buta` above 0.09 s counts
/// down every frame and steps when it passes zero; otherwise the timer stays at zero.
pub fn step_due(timer:&mut f32,interval:f32,dt:f32)->bool {
    if interval<=0.09 {*timer=0.0;return false;}
    *timer-=dt;
    if *timer<0.0 {*timer=interval;true}else{false}
}

/// Heartbeat (cshell 0x10061bcb): once when health drops below 50 and once below 20.
pub fn heartbeat_crossed(old:f32,new:f32)->bool {(old>=50.0 && new<50.0) || (old>=20.0 && new<20.0)}

/// The 300 upward impulse of the retail jump (cshell 0x10060bc3) shows as a large vertical speed
/// leaving the ground; stance changes only add 64.
pub fn jump_started(was_grounded:bool,was_vy:f32,vy:f32)->bool {was_grounded && was_vy<100.0 && vy>=200.0}

/// One SurfaceFlags value per collision.obj face (`<world>.collision.surfaces.json`).
#[derive(Resource,Default)]
pub struct Surfaces {world:String,faces:Vec<u16>,models:std::collections::BTreeMap<String,u16>}
impl Surfaces {
    pub fn parse(text:&str)->Vec<u16> {
        serde_json::from_str::<serde_json::Value>(text).ok().and_then(|v|v["faces"].as_array().map(|faces|faces.iter().map(|f|f.as_u64().unwrap_or(0) as u16).collect())).unwrap_or_default()
    }
    pub fn flags(&self,face:usize)->Option<u16> {self.faces.get(face).copied()}
    /// Dominant SurfaceFlags of a world model (a door or drawer), 0 when unknown.
    pub fn model(&self,name:&str)->u16 {self.models.get(name).copied().unwrap_or(0)}
    fn parse_models(text:&str)->std::collections::BTreeMap<String,u16> {
        serde_json::from_str::<serde_json::Value>(text).ok().and_then(|v|v["world_models"].as_object().map(|models|models.iter().map(|(name,flags)|(name.clone(),flags.as_u64().unwrap_or(0) as u16)).collect())).unwrap_or_default()
    }
    /// Gunfire probe aid (MESTER_TEST_SURFACE): every collision face reports these SurfaceFlags.
    pub fn force(&mut self,flags:u16) {self.faces.iter_mut().for_each(|face|*face=flags);}
}
pub fn sync_surfaces(config:Res<ViewerConfig>,mut surfaces:ResMut<Surfaces>) {
    if surfaces.world==config.world {return;}
    surfaces.world=config.world.clone();
    let text=std::fs::read_to_string(config.output.join(format!("{}.collision.surfaces.json",config.world))).unwrap_or_default();
    surfaces.faces=Surfaces::parse(&text);surfaces.models=Surfaces::parse_models(&text);
}
/// SurfaceFlags under the player: a ray from the centre +32 down to -256 (cshell 0x100614e0). A miss
/// leaves the zeroed IntersectInfo, so flags 0; without exported surfaces the generic pair plays.
pub fn ground_surface(world:&CollisionWorld,surfaces:&Surfaces,centre:NativeVec3)->Option<u16> {
    match world.raycast_face(centre+NativeVec3::Y*32.0,NativeVec3::NEG_Y,288.0) {Some((_,_,face))=>surfaces.flags(face),None=>Some(0)}
}

/// A detector's `Dzwiek` (only wiez_wn1: DOMOFON.WAV): a 3D sound within `Zasieg_dzwieku` of the marker, or 2D
/// when `Dzwiek_3D` is 0. Returns the file and its radius (None for 2D).
pub fn detector_cue(properties:&serde_json::Value)->Option<(String,Option<f32>)> {
    let path=properties["Dzwiek"].as_str().filter(|path|!path.is_empty())?;
    Some((path.to_owned(),(properties["Dzwiek_3D"].as_i64()!=Some(0)).then(||properties["Zasieg_dzwieku"].as_f64().unwrap_or(0.0) as f32)))
}
pub fn detector_sound(commands:&mut Commands,assets:&AssetServer,properties:&serde_json::Value,position:Vec3) {
    match detector_cue(properties) {
        Some((path,Some(radius)))=>play_near(commands,assets,&path,position,radius),
        Some((path,None))=>play_2d(commands,assets,&path),
        None=>{},
    }
}

/// Output-relative asset path of a retail sound ("sounds\\weapons\\ryko0.wav" or "sounds/x.wav").
pub fn asset_path(path:&str)->String {
    let path=path.replace('\\',"/");
    if path.starts_with("audio/") {path}else{format!("audio/{path}")}
}
/// MESTER_AUDIO_LOG makes every sound log an `AUDIO` line instead of playing.
pub fn logging()->bool {std::env::var_os("MESTER_AUDIO_LOG").is_some()}

/// One sound the game asked for, whether or not it is audible (`MESTER_CUE_LOG=1` prints it as a `CUE` line): the record the headless
/// probes assert on (docs/retail-audio-parity.md). `radius` is None for a 2D cue.
#[derive(Clone,Debug,PartialEq)]
pub struct Cue {pub path:String,pub radius:Option<f32>,pub distance:Option<f32>,pub heard:bool}
#[derive(Resource,Default)]
pub struct Cues {pub list:Vec<Cue>}
impl Cues {
    const CAP:usize=4096;
    pub fn push(&mut self,cue:Cue) {if self.list.len()>=Self::CAP {self.list.drain(..Self::CAP/2);}self.list.push(cue);}
    /// Cues whose path contains `part` (case-insensitive).
    pub fn heard(&self,part:&str)->usize {let part=part.to_ascii_lowercase();self.list.iter().filter(|cue|cue.heard && cue.path.to_ascii_lowercase().contains(&part)).count()}
}
/// MESTER_CUE_LOG=1 prints every requested sound: `CUE 2d <file>` or `CUE 3d r=<radius> d=<distance> <file>` (`CUE far ...` beyond its radius).
pub fn cue_log()->bool {std::env::var_os("MESTER_CUE_LOG").is_some()}
/// Records a cue in the log and in the `Cues` resource. Music cues (`CUE music <file>`) go through `note_music`.
pub fn note_cue(world:&mut World,path:&str,radius:Option<f32>,distance:Option<f32>,heard:bool) {
    if cue_log() {
        match (radius,distance,heard) {
            (None,..)=>info!("CUE 2d {path}"),
            (Some(radius),Some(distance),true)=>info!("CUE 3d r={radius} d={distance:.0} {path}"),
            (Some(radius),distance,false)=>info!("CUE far r={radius} d={:.0} {path}",distance.unwrap_or(f32::NAN)),
            (Some(radius),None,true)=>info!("CUE 3d r={radius} d=? {path}"),
        }
    }
    if let Some(mut cues)=world.get_resource_mut::<Cues>() {cues.push(Cue {path:path.to_owned(),radius,distance,heard});}
}
pub fn note_music(world:&mut World,what:&str) {
    if cue_log() {info!("CUE music {what}");}
    if let Some(mut cues)=world.get_resource_mut::<Cues>() {cues.push(Cue {path:format!("music:{what}"),radius:None,distance:None,heard:true});}
}

/// Radius of a sound that keeps its source position but has no falloff: retail plays it as a local (2D) sound (PlaySound flag 0x200),
/// the improved 3D option only pans it.
pub const UNBOUNDED:f32=1.0e7;

/// What a sound request is: its cue (radius None for a retail 2D sound, also a positional `UNBOUNDED` one) and the gain it starts at.
pub fn classify(at:Option<(Vec3,f32)>,listener:Option<Vec3>,improved:bool)->(Option<f32>,Option<f32>,f32) {
    let distance=at.zip(listener).map(|((position,_),listener)|listener.distance(position));
    let level=at.map_or(1.0,|(_,radius)|if radius<=0.0 {0.0}else{distance.map_or(1.0,|distance|crate::sound::level(improved,distance,radius))});
    let retail_2d=at.is_none_or(|(_,radius)|radius>=UNBOUNDED);
    (at.filter(|_|!retail_2d).map(|(_,radius)|radius),distance.filter(|_|!retail_2d),level)
}

fn play_world(world:&mut World,assets:&AssetServer,path:&str,at:Option<(Vec3,f32)>,scoped:bool,owner:Option<u64>) {
    let asset=asset_path(path);
    // Linear falloff 1-d/R (Lithtech.exe 0x480940); sound::update keeps the gain current, settings::update multiplies it in.
    let listener=world.query_filtered::<&Transform,With<InspectionCamera>>().iter(world).next().map(|t|t.translation/SCALE);
    let (radius,distance,level)=classify(at,listener,crate::spatial::enabled(world));
    note_cue(world,&asset,radius,distance,level>0.0);
    if level<=0.0 {return;}
    let log=logging();
    if !log && world.get_resource::<Session>().is_none_or(|session|!session.spawns_audio()) {return;}
    if world.get_resource::<ViewerConfig>().is_some_and(|config|!config.output.join(&asset).is_file()) {
        if log {warn!("AUDIO missing {asset}");}
        return;
    }
    if log {
        match at {Some((_,radius))=>info!("AUDIO near r={radius} {asset}"),None=>info!("AUDIO 2d {asset}")}
        return;
    }
    // A prop's new use sound replaces the one it still plays (cshell 0x1002e54e..0x1002e59b kills the old handle).
    if let Some(owner)=owner {
        let old:Vec<Entity>=world.query::<(Entity,&SoundOwner)>().iter(world).filter(|(_,o)|o.0==owner).map(|(entity,_)|entity).collect();
        for entity in old {if let Ok(entity)=world.get_entity_mut(entity) {entity.despawn();}}
    }
    let handle=assets.load::<bevy::audio::AudioSource>(asset);
    let mut sound=world.spawn((AudioPlayer::new(handle),PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(level))));
    if scoped {sound.insert(crate::WorldGeometry);}
    if let Some(owner)=owner {sound.insert(SoundOwner(owner));}
    if let Some((position,radius))=at {sound.insert((crate::sound::Positional {position,radius},crate::sound::Gain(level)));}
}
/// The object that started a sound; a new sound of the same owner stops the old one.
#[derive(Component)] pub struct SoundOwner(pub u64);
/// A player-only 2D one-shot (footsteps, jump, heartbeat).
pub fn play_2d(commands:&mut Commands,assets:&AssetServer,path:&str) {
    let (path,assets)=(path.to_owned(),assets.clone());
    commands.queue(move |world:&mut World|play_world(world,&assets,&path,None,false,None));
}
/// A positional one-shot at a native-unit position: linear falloff to silence at the retail `radius`
/// (see crate::sound). Ends with the level.
pub fn play_near(commands:&mut Commands,assets:&AssetServer,path:&str,position:Vec3,radius:f32) {
    let (path,assets)=(path.to_owned(),assets.clone());
    commands.queue(move |world:&mut World|play_world(world,&assets,&path,Some((position,radius)),true,None));
}
/// A positional one-shot owned by an object: the object's earlier sound stops when this one starts (prop use sounds, cshell 0x1002e520).
pub fn play_near_owned(commands:&mut Commands,assets:&AssetServer,path:&str,position:Vec3,radius:f32,owner:u64) {
    let (path,assets)=(path.to_owned(),assets.clone());
    commands.queue(move |world:&mut World|play_world(world,&assets,&path,Some((position,radius)),true,Some(owner)));
}
/// A retail 2D (local) sound whose source has a position (cutscene voices, cshell 0x10012fa8 flags 0x1210): full volume and no pan like
/// retail; with "Javított 3D hangzás" it comes from `position` without falloff.
pub fn play_voice(commands:&mut Commands,assets:&AssetServer,path:&str,position:Vec3) {play_near(commands,assets,path,position,UNBOUNDED);}

/// The retail footstep of `walking` on the surface below it; `step` counts steps since the level began.
pub fn player_step(commands:&mut Commands,assets:&AssetServer,step:usize) {
    let assets=assets.clone();
    commands.queue(move |world:&mut World| {
        let surface=world.get_resource::<Walking>().zip(world.get_resource::<Surfaces>()).and_then(|(walking,surfaces)|ground_surface(&walking.world,surfaces,walking.player.position));
        play_world(world,&assets,footstep_path(surface.unwrap_or(u16::MAX),step),None,false,None);
    });
}

fn jump_sound(walking:Res<Walking>,mut previous:Local<(bool,f32)>,mut commands:Commands,assets:Res<AssetServer>) {
    let (grounded,vy)=(walking.player.grounded,walking.player.velocity.y);
    if jump_started(previous.0,previous.1,vy) {play_2d(&mut commands,&assets,JUMP);}
    *previous=(grounded,vy);
}
fn heartbeat(campaign:Res<Campaign>,front:Res<Frontend>,travel:Res<Travel>,mut previous:Local<Option<(u64,f32)>>,mut commands:Commands,assets:Res<AssetServer>) {
    // Level loads and checkpoint restores set health directly; only play changes in the level.
    if front.loading.is_some() {*previous=None;return;}
    if let Some((arrived,old))=*previous {if arrived==travel.arrived && heartbeat_crossed(old,campaign.health) {play_2d(&mut commands,&assets,HEARTBEAT);}}
    *previous=Some((travel.arrived,campaign.health));
}

/// MESTER_AUDIO_PROBE=1 in a capture run (with MESTER_AUDIO_LOG=1): steps once on each distinct floor
/// surface found around the spawn, then drops health below 50 and 20 and jumps, so the log shows the files
/// the real wiring would play. Nothing is audible.
fn probe(camera:Single<&Transform,With<InspectionCamera>>,time:Res<Time>,config:Res<ViewerConfig>,mut walking:ResMut<Walking>,mut campaign:ResMut<Campaign>,mut commands:Commands,assets:Res<AssetServer>,mut state:Local<(usize,Vec<(NativeVec3,u16)>)>) {
    if config.capture.is_none() || time.elapsed_secs()<2.0 {return;}
    let t=time.elapsed_secs();
    if state.1.is_empty() && state.0==0 {
        let (surfaces,centre)=(Surfaces::parse(&std::fs::read_to_string(config.output.join(format!("{}.collision.surfaces.json",config.world))).unwrap_or_default()),walking.player.position);
        let mut found=Vec::<(NativeVec3,u16)>::new();
        for i in -30..=30 {for j in -30..=30 {
            let top=NativeVec3::new(centre.x+i as f32*150.0,centre.y+400.0,centre.z+j as f32*150.0);
            if let Some((distance,_,face))=walking.world.raycast_face(top,NativeVec3::NEG_Y,900.0) {
                let flags=surfaces.get(face).copied().unwrap_or(0);
                if !found.iter().any(|(_,known)|*known==flags) {found.push((top+NativeVec3::NEG_Y*distance+NativeVec3::Y*58.2,flags));}
            }
        }}
        info!("AUDIO probe surfaces {:?}",found.iter().map(|(_,f)|*f).collect::<Vec<_>>());
        state.1=found;state.0=1;
    }
    let step=state.0;
    if step>=1 && step<=state.1.len() && t>2.0+step as f32*0.3 {
        let (at,_)=state.1[step-1];
        walking.player.position=at;walking.player.velocity=NativeVec3::ZERO;
        player_step(&mut commands,&assets,step);state.0+=1;
    } else if step==state.1.len()+1 && t>2.0+step as f32*0.3+0.5 {
        campaign.health=45.0;state.0+=1;
    } else if step==state.1.len()+2 && t>2.0+step as f32*0.3+1.0 {
        campaign.health=15.0;walking.player.velocity.y=280.0;state.0+=1;
    } else if step==state.1.len()+3 && t>2.0+step as f32*0.3+1.5 {
        // One door sound beside the listener (logged) and one far outside its radius (silent); with the trace, five ricochets 300 units
        // left, right, behind, ahead and above the camera (their per-ear gains are the "Javított 3D hangzás" evidence).
        let here=Vec3::new(walking.player.position.x,walking.player.position.y,walking.player.position.z);
        play_near(&mut commands,&assets,"sounds/psss.wav",here+Vec3::X*300.0,DOOR_RADIUS);
        play_near(&mut commands,&assets,"sounds/psss.wav",here+Vec3::X*5000.0,DOOR_RADIUS);
        let ears=crate::spatial::Listener::from_camera(&camera,crate::mirror::MIRRORED);
        for (file,direction) in [("ryko0",-ears.right),("ryko1",ears.right),("ryko2",-ears.forward),("ryko3",ears.forward),("ryko4",ears.up)] {play_near(&mut commands,&assets,&format!("sounds/weapons/{file}.wav"),ears.position+direction*300.0,1280.0);}
        state.0+=1;
    }
}

/// Registers every retail sound rule in this module and the music.
pub struct RetailAudio;
impl Plugin for RetailAudio {
    fn build(&self,app:&mut App) {
        app.init_resource::<Surfaces>().init_resource::<Cues>().init_resource::<crate::music::Music>()
            .add_systems(Update,(sync_surfaces,heartbeat,jump_sound.after(crate::move_camera),crate::music::tick));
        if std::env::var_os("MESTER_AUDIO_PROBE").is_some() {app.add_systems(Update,probe.before(heartbeat));}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footsteps_follow_the_surface_flags_and_alternate() {
        let name=|surface,step|footstep_path(surface,step).rsplit('/').next().unwrap();
        // First step of a level is variant 2, then 1, 2, ...
        assert_eq!((name(0,1),name(0,2),name(0,3)),("KROKK2.WAV","KROKK1.WAV","KROKK2.WAV"));
        assert_eq!(name(6,1),"KROKK2.WAV");
        assert_eq!((name(2,1),name(2,2),name(4,1),name(4,2)),("KROKM2.WAV","KROKM1.WAV","KROKM2.WAV","KROKM1.WAV"));
        assert_eq!((name(3,1),name(3,2)),("KROKD2.WAV","KROKD1.WAV"));
        for other in [1,5,7,10,u16::MAX] {assert_eq!((name(other,1),name(other,2)),("KROK2.WAV","KROK1.WAV"));}
    }
    #[test]
    fn footstep_and_speech_files_are_in_the_audio_export() {
        let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
        if !root.join("audio").is_dir() {return;}
        let mut paths=vec![JUMP,HEARTBEAT];
        for surface in [0,2,3,9] {for step in [1,2] {paths.push(footstep_path(surface,step));}}
        for path in paths {assert!(root.join(asset_path(path)).is_file(),"missing {path}");}
    }
    #[test]
    fn halt_shout_picks_the_digit_before_the_extension() {
        assert_eq!(halt_path("sounds\\enemies\\police\\halt0.wav",8,13),"sounds\\enemies\\police\\halt5.wav");
        assert_eq!(halt_path("sounds\\enemies\\police\\halt0.wav",1,13),"sounds\\enemies\\police\\halt0.wav");
        assert_eq!(halt_path("sounds\\enemies\\police\\halt0.wav",0,13),"sounds\\enemies\\police\\halt0.wav");
        assert_eq!(halt_path("a.wav",8,3),"a.wav");
        assert_eq!(halt_path("x\\halt0.wav",20,19),"x\\halt9.wav");
    }
    #[test]
    fn death_scream_reuses_the_halt_folder() {
        assert_eq!(death_scream_path("sounds\\enemies\\police\\halt0.wav",11).as_deref(),Some("sounds\\enemies\\police\\dead3.wav"));
        assert_eq!(death_scream_path("sounds\\enemies\\macaroni\\halt7.wav",7).as_deref(),Some("sounds\\enemies\\macaroni\\dead7.wav"));
        assert_eq!(death_scream_path("halt0.wav",1),None);
        assert_eq!(death_scream_path("",1),None);
    }
    #[test]
    fn npc_sound_radii_and_footstep_timer_follow_retail() {
        assert_eq!((npc_step_radius(false),npc_step_radius(true)),(640.0,2048.0));
        assert_eq!((halt_radius(false),halt_radius(true)),(1280.0,2048.0));
        assert_eq!((npc_weapon_radius(1640.0,false,false),npc_weapon_radius(1640.0,true,false),npc_weapon_radius(1640.0,false,true)),(1640.0,6560.0,6560.0));
        assert_eq!(npc_weapon_radius(0.0,true,false),0.0);
        // odstep_glosow_buta 0.4: steps on the first frame, then every 0.4 s; 0.09 and below never step.
        let mut timer=0.0;let mut steps=0;
        for _ in 0..100 {if step_due(&mut timer,0.4,0.05) {steps+=1;}}
        assert_eq!(steps,13);
        let mut timer=0.5;
        assert!(!step_due(&mut timer,0.09,0.05) && timer==0.0);
        assert!(!step_due(&mut timer,0.0,0.05));
    }
    #[test]
    fn heartbeat_plays_once_below_fifty_and_once_below_twenty() {
        assert!(heartbeat_crossed(50.0,49.9) && heartbeat_crossed(100.0,10.0));
        assert!(!heartbeat_crossed(49.0,45.0) && !heartbeat_crossed(60.0,50.0) && !heartbeat_crossed(50.0,50.0));
        assert!(heartbeat_crossed(25.0,19.9) && !heartbeat_crossed(19.0,5.0));
        // A run of small hits from 100: fires at the 50 and 20 crossings only.
        let fired=(0..100).map(|i|(100-i) as f32).zip((1..=100).map(|i|(100-i) as f32)).filter(|(old,new)|heartbeat_crossed(*old,*new)).count();
        assert_eq!(fired,2);
        // Healing back over 50 and dropping again plays it again.
        assert!(heartbeat_crossed(55.0,40.0) && !heartbeat_crossed(40.0,55.0));
    }
    #[test]
    fn jump_is_the_upward_impulse_from_the_ground() {
        assert!(jump_started(true,0.0,275.0) && jump_started(true,0.0,250.0));
        assert!(!jump_started(true,0.0,64.0),"a stance change only adds 64");
        assert!(!jump_started(false,0.0,300.0) && !jump_started(true,300.0,320.0) && !jump_started(true,-100.0,-90.0));
    }
    #[test]
    fn ground_ray_reads_the_surface_of_the_face_below_the_player() {
        // Face 0 is metal (2) under x<0, face 1 wood (3) under x>0, both at y=0; face 2 tiles (6) at y=-100.
        let obj="v -100 0 -100\nv 0 0 -100\nv 0 0 100\nv -100 0 100\nv 100 0 -100\nv 100 0 100\nv -100 -100 -100\nv 100 -100 -100\nv 100 -100 100\nv -100 -100 100\nf 1 2 3 4\nf 2 5 6 3\nf 7 8 9 10\n";
        let world=CollisionWorld::from_obj(obj).unwrap();
        let surfaces=Surfaces {world:String::new(),faces:vec![2,3,6],models:Default::default()};
        assert_eq!(ground_surface(&world,&surfaces,NativeVec3::new(-50.0,58.0,0.0)),Some(2));
        assert_eq!(ground_surface(&world,&surfaces,NativeVec3::new(50.0,58.0,0.0)),Some(3));
        // Standing on face 0 the ray (+32 .. -256) never reaches the lower floor first.
        assert_eq!(ground_surface(&world,&surfaces,NativeVec3::new(-50.0,-42.0,0.0)),Some(6));
        assert_eq!(ground_surface(&world,&surfaces,NativeVec3::new(500.0,58.0,0.0)),Some(0),"a miss keeps the zeroed IntersectInfo");
        assert_eq!(ground_surface(&world,&Surfaces::default(),NativeVec3::new(50.0,58.0,0.0)),None,"no export: generic pair");
    }
    #[test]
    fn detector_sound_is_3d_within_its_range_unless_dzwiek_3d_is_zero() {
        let domofon=serde_json::json!({"Dzwiek":"sounds/ambient/DOMOFON.WAV","Dzwiek_3D":1,"Zasieg_dzwieku":640.0});
        assert_eq!(detector_cue(&domofon),Some(("sounds/ambient/DOMOFON.WAV".to_string(),Some(640.0))));
        assert_eq!(detector_cue(&serde_json::json!({"Dzwiek":"a.wav","Dzwiek_3D":0,"Zasieg_dzwieku":640.0})),Some(("a.wav".to_string(),None)));
        assert_eq!(detector_cue(&serde_json::json!({"Dzwiek":"","Dzwiek_3D":1,"Zasieg_dzwieku":640.0})),None,"every other detector is silent");
        assert_eq!(detector_cue(&serde_json::json!({})),None);
    }
    #[test]
    #[ignore="requires local original world exports"]
    fn every_exported_world_has_one_surface_value_per_collision_face() {
        let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
        let mut checked=0;
        for entry in std::fs::read_dir(&root).unwrap().flatten() {
            let name=entry.file_name().to_string_lossy().into_owned();
            let Some(world)=name.strip_suffix(".collision.surfaces.json") else {continue};
            let faces=Surfaces::parse(&std::fs::read_to_string(entry.path()).unwrap());
            let obj=std::fs::read_to_string(root.join(format!("{world}.collision.obj"))).unwrap();
            assert_eq!(faces.len(),obj.lines().filter(|line|line.starts_with("f ")).count(),"{world}");
            assert!(faces.iter().all(|flags|*flags<=10),"{world}: unexpected SurfaceFlags");
            checked+=1;
        }
        assert!(checked>=29,"{checked} worlds");
    }
    #[test]
    fn cues_are_2d_3d_or_out_of_range() {
        let listener=Some(Vec3::ZERO);
        // A player-only sound and a cutscene voice (a source position without falloff) are 2D cues at full volume, improved option or not.
        assert_eq!(classify(None,listener,false),(None,None,1.0));
        for improved in [false,true] {
            let (radius,distance,level)=classify(Some((Vec3::new(900.0,0.0,0.0),UNBOUNDED)),listener,improved);
            assert_eq!((radius,distance),(None,None));assert!(level>0.999,"{level}");
        }
        // A 3D sound keeps its retail radius and falls off linearly (the baseline) to silence at it.
        let (radius,distance,level)=classify(Some((Vec3::new(320.0,0.0,0.0),640.0)),listener,false);
        assert_eq!((radius,distance),(Some(640.0),Some(320.0)));assert!((level-0.5).abs()<1e-6);
        assert_eq!(classify(Some((Vec3::new(700.0,0.0,0.0),640.0)),listener,false).2,0.0,"beyond the radius nothing is heard");
        assert_eq!(classify(Some((Vec3::ONE,0.0)),listener,false).2,0.0,"radius 0");
        // Without a listener (menus, tests) a positional sound is heard in full.
        assert_eq!(classify(Some((Vec3::X,640.0)),None,false),(Some(640.0),None,1.0));
    }
    #[test]
    fn the_cue_record_counts_what_was_heard() {
        let mut cues=Cues::default();
        cues.push(Cue {path:"audio/sounds/weapons/Glock_s.wav".into(),radius:Some(640.0),distance:Some(10.0),heard:true});
        cues.push(Cue {path:"audio/sounds/weapons/glock_s.wav".into(),radius:Some(640.0),distance:Some(900.0),heard:false});
        assert_eq!((cues.heard("glock_s"),cues.heard("GLOCK"),cues.heard("sig")),(1,1,0));
        for i in 0..Cues::CAP+10 {cues.push(Cue {path:format!("x{i}"),radius:None,distance:None,heard:true});}
        assert!(cues.list.len()<=Cues::CAP,"the record is bounded");
        assert_eq!(cues.list.last().map(|c|c.path.as_str()),Some("x4105"));
    }
    #[test]
    fn surface_export_is_parsed_and_paths_resolve() {
        assert_eq!(Surfaces::parse(r#"{"format":"x","faces":[0,2,3,6]}"#),vec![0,2,3,6]);
        assert!(Surfaces::parse("nonsense").is_empty());
        assert_eq!(asset_path("sounds\\weapons\\ryko0.wav"),"audio/sounds/weapons/ryko0.wav");
        assert_eq!(asset_path("audio/sounds/hero/KROK1.WAV"),"audio/sounds/hero/KROK1.WAV");
    }
}
