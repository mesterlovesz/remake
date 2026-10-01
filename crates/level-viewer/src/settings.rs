use bevy::{audio::{AudioSinkPlayback, Volume}, prelude::*, window::{CursorGrabMode,CursorOptions}};
use serde::{Serialize,Deserialize};
use crate::{ViewerConfig,options};

#[derive(Serialize,Deserialize,Clone)]
pub struct Preferences {pub sensitivity:f32,pub fov:f32,pub volume:f32,#[serde(default="default_difficulty")]pub difficulty:u8,#[serde(default)]pub version:u32,
    /// Name of the sound output device (output.rs); empty is automatic.
    #[serde(default)]pub output_device:String,
    /// Dark outline and faint strip behind the on-screen texts (retail_ui.rs `Aid`); on by default.
    #[serde(default="yes")]pub text_aid:bool}
fn yes()->bool {true}
fn default_difficulty()->u8 {2} // Original cshell difficulty initializer is 1.0 (third choice).
/// 1: sensitivity is the retail slider (× 0.006625 rad per count), fov the 4:3 horizontal FOV.
const PREFERENCES_VERSION:u32=1;
impl Default for Preferences {fn default()->Self {Self {sensitivity:1.0,fov:crate::view::FOV_X_DEGREES,volume:1.0,difficulty:default_difficulty(),version:PREFERENCES_VERSION,output_device:String::new(),text_aid:true}}}
impl Preferences {
    pub(crate) fn sanitize(&mut self) {
        // Version 0 stored radians per count (about 0.003) and a 90° vertical FOV.
        if self.version<PREFERENCES_VERSION {self.sensitivity=1.0;self.fov=crate::view::FOV_X_DEGREES;self.version=PREFERENCES_VERSION;}
        self.sensitivity=((self.sensitivity*100.0).round()/100.0).clamp(0.5,1.5);
        // Steps of 5° from the retail 81°.
        self.fov=self.fov.clamp(61.0,111.0);self.volume=self.volume.clamp(0.0,1.0);self.difficulty=self.difficulty.min(2);
    }
}
#[derive(Resource)]
pub struct Session {
    pub preferences:Preferences,pub paused:bool,
    /// Capture and MESTER_SILENT runs: nothing may be audible and no file is written.
    pub silent:bool,
    /// MESTER_AUDIO_TRACE capture run (audio_trace.rs): the audio pipeline is live, but every sink is muted and every sample silenced.
    pub trace:bool,pub suppress_fire:u8,
    pub notice:String,pub dialogue_active:bool,/// The dialogue choice list is on screen: the fire button belongs to it and the pitch runs at a quarter (dialogue.rs).
    pub dialogue_choices:bool,/// The choice list is showing (answers selectable): the arrow keys belong to it, not to walking.
    pub dialogue_list:bool,pub page:u8,
    /// The sound device the game opened (output.rs); empty before that and in silent runs.
    pub output:String,
    /// The window has lost the keyboard focus (never in capture runs): the music then pauses so it does not play behind other programs (remake courtesy).
    pub unfocused:bool,
}
impl Session {
    pub fn new(preferences:Preferences,silent:bool,trace:bool)->Self {Self {preferences,paused:false,silent,trace,suppress_fire:0,notice:String::new(),dialogue_active:false,dialogue_choices:false,dialogue_list:false,page:0,output:String::new(),unfocused:false}}
    /// AudioPlayers exist in audible runs and in trace runs; every other silent run creates none.
    pub fn spawns_audio(&self)->bool {!self.silent || self.trace}
    /// Master volume of the run: the preference, 0 in silent runs.
    pub fn master(&self)->f32 {if self.silent {0.0}else{self.preferences.volume}}
    /// Drives a sink to the preference times its distance `gain`. A silent run mutes it instead of lowering it, so the volume
    /// it would have stays readable (`AudioSink::volume`) while nothing is audible.
    pub fn drive(&self,sink:&mut AudioSink,gain:f32) {
        if self.silent {if !sink.is_muted() {sink.mute();}}else if sink.is_muted() {sink.unmute();}
        sink.set_volume(Volume::Linear(self.preferences.volume*gain));
    }
}
/// Whether a sink is held (paused) while the game is `paused`: the video sound never is and neither is the music: the engine's sound manager has no
/// pause and nothing in cshell stops the track when the in-game or main menu opens (only a level load does), so it plays on behind the menu. A window
/// that lost the focus (`unfocused`, remake courtesy) pauses the music too.
pub fn holds(paused:bool,unfocused:bool,music:bool,video:bool)->bool {if video {false}else if music {unfocused}else{paused}}
pub fn setup(mut commands:Commands,config:Res<ViewerConfig>) {
    let mut preferences=std::fs::read_to_string(config.user.join("settings.json")).ok()
        .and_then(|s|serde_json::from_str::<Preferences>(&s).ok()).unwrap_or_default();
    preferences.sanitize();
    // Capture runs use the built-in defaults so probes stay deterministic; MESTER_USER_DIR loads a scratch profile instead.
    let mut options=if config.capture.is_some() && !config.persist {options::RetailOptions::default()}else{options::RetailOptions::load(&config.user)};
    // The remake's earlier settings.json held the mouse sensitivity; it seeds a first run that has no keys.cfg yet.
    if !options.from_files {options.keys.mouse_sensitivity=preferences.sensitivity;}
    commands.insert_resource(options::Options(options));
    let silent=config.capture.is_some() || std::env::var_os("MESTER_SILENT").is_some();
    let trace=config.capture.is_some() && crate::audio_trace::requested();
    commands.insert_resource(GlobalVolume::new(Volume::Linear(if silent {0.0}else{preferences.volume})));
    commands.insert_resource(Session::new(preferences,silent,trace));
}

fn resume(session:&mut Session,cursor:&mut CursorOptions) {
    session.paused=false;session.suppress_fire=2;cursor.visible=false;cursor.grab_mode=CursorGrabMode::Locked;
}

/// Alt-tab away from a game that owns the mouse (locked, or a panel on the free OS cursor) pauses it; hidden capture windows are never focused.
pub fn pause_on_focus_loss(focused:bool,capture:bool,grabbed:bool,panel_mouse:bool)->bool {!focused && !capture && (grabbed || panel_mouse)}

/// Pause state and pointer while a menu, a loading screen or a lost window focus holds the game (the menu pages are menu.rs; Esc opens it there).
#[allow(clippy::too_many_arguments)]
pub fn controls(buttons:Res<ButtonInput<MouseButton>>,mut session:ResMut<Session>,mut cursor:Single<&mut CursorOptions>,window:Single<&Window>,front:Res<crate::frontend::Frontend>,mut menu:ResMut<crate::menu::MenuState>,panels:Res<crate::panels::Panels>,config:Res<ViewerConfig>,mut commands:Commands) {
    session.suppress_fire=session.suppress_fire.saturating_sub(1);
    session.unfocused=!window.focused && config.capture.is_none();
    if front.loading.is_some() {session.paused=true;cursor.visible=false;cursor.grab_mode=CursorGrabMode::None;return;}
    if front.main_active || menu.opening>0 {session.paused=true;cursor.visible=true;cursor.grab_mode=CursorGrabMode::None;return;}
    // Losing the window (alt-tab) pauses a game that owns the mouse, including one with the inventory / X screen open on the free OS cursor,
    // and opens the menu so there is something to click when the player comes back. Hidden capture windows are never focused, so headless runs are exempt.
    if pause_on_focus_loss(window.focused,config.capture.is_some(),cursor.grab_mode!=CursorGrabMode::None,panels.mouse_mode()) {
        session.paused=true;cursor.visible=true;cursor.grab_mode=CursorGrabMode::None;
        if front.in_game {crate::menu::request_open(&mut menu,&mut commands);}
    }else if !session.paused && buttons.just_pressed(MouseButton::Left) && cursor.visible && !panels.mouse_mode() {
        resume(&mut session,&mut cursor);
    }
    if session.paused {cursor.visible=true;cursor.grab_mode=CursorGrabMode::None;}
}

/// Master volume and every sink: the preference times the distance gain times the retail switches (`Music`, `Speech`, `Sound` console
/// variables; the video sound has none). A paused game pauses the sinks, except the video sound and the music.
pub fn update(session:Res<Session>,options:Res<options::Options>,mut volume:ResMut<GlobalVolume>,mut sinks:Query<(&mut AudioSink,Option<&crate::sound::Gain>,Has<crate::music::MusicTrack>,Has<crate::video::VideoAudio>,Has<crate::campaign::Speech>)>) {
    volume.volume=Volume::Linear(session.master());
    for (mut sink,gain,music,video,speech) in &mut sinks {
        let switch=if video {true}else if music {options.music}else if speech {options.speech}else{options.sound};
        session.drive(&mut sink,if switch {gain.map_or(1.0,|gain|gain.0)}else{0.0});
        if holds(session.paused,session.unfocused,music,video) {sink.pause();}else{sink.play();}
    }
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn alt_tab_pauses_a_game_that_owns_the_mouse_including_an_open_panel() {
        assert!(pause_on_focus_loss(false,false,true,false),"locked mouse look");
        assert!(pause_on_focus_loss(false,false,false,true),"inventory / X screen on the free cursor");
        assert!(!pause_on_focus_loss(true,false,true,true),"focused windows are left alone");
        assert!(!pause_on_focus_loss(false,false,false,false),"click-to-play screen: nothing to pause");
        assert!(!pause_on_focus_loss(false,true,true,true),"hidden capture windows are never focused");
    }
    #[test] fn old_settings_migrate_to_retail_units() {
        let mut old:Preferences=serde_json::from_str(r#"{"sensitivity":0.003,"fov":90.0,"volume":0.3,"difficulty":1}"#).unwrap();old.sanitize();
        assert_eq!((old.sensitivity,old.fov,old.version,old.difficulty),(1.0,81.0,1,1));assert!((old.volume-0.3).abs()<1e-6);
        let mut new:Preferences=serde_json::from_str(r#"{"sensitivity":1.2,"fov":96.0,"volume":1.0,"difficulty":2,"version":1}"#).unwrap();new.sanitize();
        assert_eq!((new.sensitivity,new.fov),(1.2,96.0));
        let mut wild=Preferences {sensitivity:1.2300001,fov:200.0,..default()};wild.sanitize();assert_eq!((wild.sensitivity,wild.fov),(1.23,111.0));
        let mut low=Preferences {sensitivity:0.1,fov:10.0,..default()};low.sanitize();assert_eq!((low.sensitivity,low.fov),(0.5,61.0));
    }
    /// The saved sound device (output.rs) is optional in settings.json: the owner's file from before it existed still loads.
    #[test] fn the_output_device_choice_is_saved_and_old_files_still_load() {
        let old:Preferences=serde_json::from_str(r#"{"sensitivity":1.0,"fov":91.0,"volume":0.5999999,"difficulty":1,"version":1}"#).unwrap();
        assert_eq!(old.output_device,"","no choice means automatic");
        let chosen=Preferences {output_device:"Hangszórók (Focusrite USB Audio)".into(),..old};
        let back:Preferences=serde_json::from_str(&serde_json::to_string_pretty(&chosen).unwrap()).unwrap();
        assert_eq!(back.output_device,"Hangszórók (Focusrite USB Audio)");
        let mut sane=back;sane.sanitize();assert_eq!(sane.output_device,"Hangszórók (Focusrite USB Audio)","sanitize keeps the choice");
    }
}
