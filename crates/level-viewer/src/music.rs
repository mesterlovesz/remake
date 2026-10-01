//! Retail music (docs/retail-audio.md "Music"): one looping 2D track per level from the gameai
//! `muza` key, `menu.wav` in the main menu, `outro.wav` during the outro cutscene and `credits.wav`
//! after it. No crossfade and no calm/combat switching; the volume is fixed at 100 % (only the
//! global preference multiplies it). The tracks are MPEG Layer-3 exported by tools/export_audio.py as
//! `output/audio/sounds/muza/<name>.mp3`.
use bevy::prelude::*;
use crate::{ViewerConfig,frontend::Frontend,opening::Opening,settings::Session,travel::Travel};

/// Level music starts on the 78th frame after a load (cshell 0x1005a583), the menu track on frame 2 (0x1005a382).
pub const START_FRAME:u32=78;
pub const MENU_FRAME:u32=2;

/// gameai `muza` per world (lower-case), as track stems under sounds/muza. Worlds not listed
/// (rh1-wiezienie1 and the unused test worlds) have none.
const TRACKS:[(&str,&str);29]=[
    ("rh3-miasteczko0","prolog"),("rh1-wiezienie2","wiezienie1_spokoj"),("rh1-wiezienie3","tension_s"),
    ("rh2-wiezienie1","wiezienie2_spokoj"),("rh2-wiezienie2","blood_and_glory_s"),("rh3-miasteczko1","miasteczko"),
    ("rh3-miasteczko2","wiezienie2_spokoj"),("chapel_mniejszy","blood_and_glory_s"),("rh7a-tunele","tunele"),
    ("rh9-fabryka","fabrika"),("rh12-lab1","tension_s"),("rh12-lab2","blood_and_glory_s"),
    ("burmistrz1","burmistrz"),("burmistrz2","burmistrz"),("chinatown","chinatown_spokoj"),
    ("chinatown2","chinatown_akcja"),("rh10-wiezowiec1","tension_s"),("rh10-wiezowiec2","tension_s"),
    ("rh10-wiezowiec3","tension_s"),("podziemia1","tunele"),("podziemia1a","tunele"),
    ("podziemia1b","podziemia"),("podziemia1c","chinatown_spokoj"),("wiez_wn1","wiezienie2_spokoj"),
    ("wiez_wn2","wiezowiec_wn"),("wiez_wn3","wiezienie2_spokoj"),("knajpa","knajpa2"),
    ("rh1-wiezienie1",""),("test",""),
];

/// The `muza` track of a world; world names may carry any case (`RH9-fabryka`).
pub fn track_for_world(world:&str)->Option<&'static str> {
    let world=world.to_ascii_lowercase();
    TRACKS.iter().find(|(name,_)|*name==world).map(|(_,track)|*track).filter(|track|!track.is_empty())
}
pub fn asset_path(track:&str)->String {format!("audio/sounds/muza/{}.mp3",track.to_ascii_lowercase())}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Context {Loading,Menu,Outro,Credits,Level}
#[derive(Clone,Debug,PartialEq,Eq)]
pub enum Cue {Menu,Level(&'static str),Outro,Credits}
impl Cue {
    pub fn track(&self)->&'static str {match self {Cue::Menu=>"menu",Cue::Level(track)=>track,Cue::Outro=>"outro",Cue::Credits=>"credits"}}
}
/// What the player is looking at. Loading a world stops the old track (cshell 0x1005a180).
pub fn context(loading:bool,main_menu:bool,outro:bool,after_outro:bool)->Context {
    if loading {Context::Loading}else if main_menu {Context::Menu}else if outro {Context::Outro}else if after_outro {Context::Credits}else{Context::Level}
}
/// The track that should be playing `frames` frames after the context began.
pub fn wanted(context:Context,frames:u32,world:&str)->Option<Cue> {
    match context {
        Context::Loading=>None,
        Context::Menu=>(frames>=MENU_FRAME).then_some(Cue::Menu),
        Context::Outro=>Some(Cue::Outro),
        Context::Credits=>Some(Cue::Credits),
        Context::Level=>if frames>=START_FRAME {track_for_world(world).map(Cue::Level)}else{None},
    }
}

#[derive(Component)] pub struct MusicTrack;

/// What the game does to the music this frame.
#[derive(Debug,Default,PartialEq)]
pub struct Step {pub stop:bool,pub start:Option<Cue>}
/// The retail music handle and remembered track name (`[esi+0x2010]` / `[esi+0x2014]`, cshell 0x10059950). A play call kills the old handle and
/// clears the remembered name with it (0x10059962..0x10059978), starts nothing while the `Music` switch is off (0x10059995) and remembers the new
/// name only when it starts. The per-frame check (0x10059230, called from 0x1005a774) stops the track when the switch goes off and, after frame 100 of
/// the level, plays the remembered name again when the switch is on and nothing plays: but the stop has just cleared that name, so a track
/// switched off and on again stays silent until the next level (or menu) start plays one.
#[derive(Default,Debug)]
pub struct Retail {pub handle:Option<Cue>,pub last:Option<Cue>}
impl Retail {
    /// The retail frame counter (`[esi+0x37c]`, reset by a level load) above which the check restarts a track.
    pub const RESTART_FRAME:u32=100;
    /// A play call with a track, or with none (a stop).
    pub fn play(&mut self,cue:Option<Cue>,on:bool)->Step {
        let stop=self.handle.take().is_some();
        if stop {self.last=None;}
        let Some(cue)=cue.filter(|_|on) else {return Step {stop,start:None}};
        self.last=Some(cue.clone());self.handle=Some(cue.clone());
        Step {stop,start:Some(cue)}
    }
    /// The per-frame check.
    pub fn poll(&mut self,on:bool,frames:u32)->Step {
        if on && self.handle.is_none() && frames>Self::RESTART_FRAME {let last=self.last.clone();return self.play(last,on);}
        if !on && self.handle.is_some() {return self.play(None,on);}
        Step::default()
    }
}

#[derive(Resource,Default)]
pub struct Music {
    /// The cue the level / menu state machine last asked for.
    event:Option<Cue>,retail:Retail,entity:Option<Entity>,context:Option<Context>,frames:u32,since_load:u32,world:String,arrived:u64,outro_seen:bool,
}
impl Music {
    /// The track the retail handle holds, if any.
    pub fn playing(&self)->Option<&Cue> {self.retail.handle.as_ref()}
}

#[allow(clippy::too_many_arguments)]
pub fn tick(mut commands:Commands,assets:Res<AssetServer>,config:Res<ViewerConfig>,front:Res<Frontend>,session:Res<Session>,options:Res<crate::options::Options>,opening:Res<Opening>,travel:Res<Travel>,video:Res<crate::video::Video>,mut music:ResMut<Music>) {
    // Entering another world (or reloading this one) is a hard cut and restarts the frame count.
    if music.world!=config.world || music.arrived!=travel.arrived {
        music.world=config.world.clone();music.arrived=travel.arrived;music.outro_seen=false;music.context=None;
    }
    let outro=opening.outro();
    music.outro_seen|=outro;
    let now=context(front.loading.is_some() || video.active,front.main_active,outro,music.outro_seen);
    if music.context!=Some(now) {music.context=Some(now);music.frames=0;}else{music.frames=music.frames.saturating_add(1);}
    music.since_load=if now==Context::Loading {0}else{music.since_load.saturating_add(1)};
    let (want,on)=(wanted(now,music.frames,&config.world),options.music);
    let step=if want!=music.event {music.event=want.clone();music.retail.play(want,on)}else{let frames=music.since_load;music.retail.poll(on,frames)};
    if step.stop {
        if let Some(entity)=music.entity.take() {commands.entity(entity).try_despawn();}
        commands.queue(|world:&mut World|crate::audio::note_music(world,"stop"));
    }
    if let Some(cue) = step.start {
        let path=asset_path(cue.track());
        let log=crate::audio::logging();
        if log {info!("AUDIO music {cue:?} {path}");}
        let what=format!("{}",cue.track());
        commands.queue(move |world:&mut World|crate::audio::note_music(world,&format!("play {what}")));
        if !log && session.spawns_audio() {
            if config.output.join(&path).is_file() {music.entity=Some(commands.spawn((MusicTrack,AudioPlayer::new(assets.load(path)),PlaybackSettings::LOOP)).id());}
            else {warn!("Zene hiányzik: {path}");}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_level_has_the_gameai_muza_track() {
        for (world,track) in [("rh3-miasteczko0","prolog"),("rh1-wiezienie2","wiezienie1_spokoj"),("rh1-wiezienie3","tension_s"),("rh12-lab1","tension_s"),
            ("rh10-wiezowiec1","tension_s"),("rh10-wiezowiec3","tension_s"),("rh2-wiezienie1","wiezienie2_spokoj"),("rh3-miasteczko2","wiezienie2_spokoj"),
            ("wiez_wn1","wiezienie2_spokoj"),("wiez_wn3","wiezienie2_spokoj"),("rh2-wiezienie2","blood_and_glory_s"),("chapel_mniejszy","blood_and_glory_s"),
            ("rh12-lab2","blood_and_glory_s"),("rh3-miasteczko1","miasteczko"),("rh7a-tunele","tunele"),("podziemia1","tunele"),("podziemia1a","tunele"),
            ("rh9-fabryka","fabrika"),("burmistrz1","burmistrz"),("burmistrz2","burmistrz"),("chinatown","chinatown_spokoj"),("podziemia1c","chinatown_spokoj"),
            ("chinatown2","chinatown_akcja"),("podziemia1b","podziemia"),("wiez_wn2","wiezowiec_wn"),("knajpa","knajpa2")] {
            assert_eq!(track_for_world(world),Some(track),"{world}");
        }
        assert_eq!(track_for_world("rh1-wiezienie1"),None,"the intro has no music");
        assert_eq!(track_for_world("nonexistent"),None);
        assert_eq!(track_for_world("RH9-fabryka"),Some("fabrika"),"exported stems keep their case");
        assert_eq!(track_for_world("Rh7a-Tunele"),Some("tunele"));
        assert_eq!(crate::travel::LEVELS.iter().filter(|(world,_)|track_for_world(world).is_some()).count(),27);
    }
    #[test]
    fn table_matches_the_decoded_gameai_headers() {
        let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output/decoded_scripts/gameai.txt");
        let Ok(text)=std::fs::read_to_string(path) else {return};
        let (mut level,mut found)=(String::new(),0);
        let mut listed=std::collections::BTreeMap::new();
        for line in text.lines().map(str::trim) {
            if let Some(name)=line.strip_prefix("level ") {level=name.trim().trim_start_matches("worlds\\").to_ascii_lowercase();listed.entry(level.clone()).or_insert(None);}
            else if let Some(track)=line.strip_prefix("muza ") {
                let stem=track.trim().rsplit('\\').next().unwrap().trim_end_matches(".wav").trim_end_matches(".WAV").to_ascii_lowercase();
                listed.insert(level.clone(),Some(stem));
            }
        }
        for (world,track) in &listed {
            assert_eq!(track_for_world(world).map(str::to_owned),*track,"{world}");
            found+=track.is_some() as usize;
        }
        assert_eq!(found,27);
    }
    #[test]
    fn menu_and_level_tracks_start_on_the_retail_frames() {
        assert_eq!(wanted(Context::Menu,MENU_FRAME-1,"rh1-wiezienie1"),None);
        assert_eq!(wanted(Context::Menu,MENU_FRAME,"rh1-wiezienie1"),Some(Cue::Menu));
        assert_eq!(wanted(Context::Level,START_FRAME-1,"rh9-fabryka"),None);
        assert_eq!(wanted(Context::Level,START_FRAME,"rh9-fabryka"),Some(Cue::Level("fabrika")));
        assert_eq!(wanted(Context::Level,10_000,"rh1-wiezienie1"),None);
        assert_eq!(wanted(Context::Loading,10_000,"rh9-fabryka"),None,"loading is a hard cut");
        assert_eq!(Cue::Level("fabrika").track(),"fabrika");
    }
    #[test]
    fn outro_and_credits_replace_the_level_track() {
        assert_eq!(context(false,false,false,false),Context::Level);
        assert_eq!(context(false,false,true,true),Context::Outro);
        assert_eq!(context(false,false,false,true),Context::Credits);
        assert_eq!(context(true,false,false,true),Context::Loading);
        assert_eq!(context(false,true,false,true),Context::Menu);
        assert_eq!(wanted(Context::Outro,0,"rh12-lab2"),Some(Cue::Outro));
        assert_eq!(wanted(Context::Credits,0,"rh12-lab2"),Some(Cue::Credits));
        assert_eq!((Cue::Outro.track(),Cue::Credits.track(),Cue::Menu.track()),("outro","credits","menu"));
        assert_eq!(asset_path("Blood_and_Glory_s"),"audio/sounds/muza/blood_and_glory_s.mp3");
    }
    #[test]
    fn music_switched_off_and_on_stays_silent_until_the_next_start() {
        // cshell 0x10059950 / 0x10059230: the stop clears the remembered name, so the restart check after frame 100 finds nothing to play.
        let (mut retail,level)=(Retail::default(),Cue::Level("fabrika"));
        assert_eq!(retail.play(Some(level.clone()),true),Step {stop:false,start:Some(level.clone())});
        assert_eq!(retail.poll(true,500),Step::default(),"it plays on");
        assert_eq!(retail.poll(false,500),Step {stop:true,start:None},"Music 0 stops it");
        assert!(retail.handle.is_none() && retail.last.is_none());
        assert_eq!(retail.poll(true,501),Step::default(),"Music 1 alone brings nothing back");
        assert_eq!(retail.play(Some(level.clone()),true).start,Some(level),"the next level start does");
    }
    #[test]
    fn a_start_with_the_switch_off_plays_and_remembers_nothing() {
        let mut retail=Retail::default();
        assert_eq!(retail.play(Some(Cue::Menu),false),Step::default());
        assert!(retail.last.is_none());
        assert_eq!(retail.poll(true,1000),Step::default(),"the name was never stored");
        // A level without a track (rh1-wiezienie1) stops the old one and plays none.
        assert_eq!(retail.play(Some(Cue::Menu),true).start,Some(Cue::Menu));
        assert_eq!(retail.play(None,true),Step {stop:true,start:None});
        assert_eq!(retail.poll(true,1000),Step::default());
    }
    #[test]
    fn a_new_track_replaces_the_old_one() {
        let mut retail=Retail::default();
        retail.play(Some(Cue::Menu),true);
        assert_eq!(retail.play(Some(Cue::Level("tunele")),true),Step {stop:true,start:Some(Cue::Level("tunele"))});
        assert_eq!(retail.handle,Some(Cue::Level("tunele")));
        assert_eq!(retail.poll(false,10),Step {stop:true,start:None});
    }
    #[test]
    fn the_menu_keeps_the_music_playing() {
        use crate::settings::holds;
        assert!(!holds(true,false,true,false),"the music is not held by a menu");
        assert!(holds(true,true,true,false),"an unfocused window pauses it");
        assert!(holds(true,false,false,false) && !holds(false,false,false,false) && !holds(true,false,false,true) && !holds(true,true,false,true),"one-shots pause with the game, the video sound never");
    }
    #[test]
    fn exported_music_files_exist_and_decode_to_stereo_22khz() {
        use bevy::audio::{AudioSource,Decodable,Source};
        let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
        if !root.join("audio/sounds/muza/prolog.mp3").is_file() {return;}
        for track in ["menu","outro","credits"].into_iter().chain(TRACKS.iter().map(|(_,track)|*track).filter(|track|!track.is_empty())) {
            assert!(root.join(asset_path(track)).is_file(),"{track}");
        }
        // Decoding needs no audio device: count the samples of the shortest retail track.
        let bytes=std::fs::read(root.join(asset_path("credits"))).unwrap();
        let mut decoder=AudioSource {bytes:bytes.into()}.decoder();
        assert_eq!((decoder.channels(),decoder.sample_rate()),(2,22050));
        let samples=decoder.by_ref().count();
        // 622 804 bytes at 56 kbit/s: about 89 s of stereo.
        assert!((3_400_000..4_400_000).contains(&samples),"{samples}");
    }
}
