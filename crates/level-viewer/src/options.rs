//! The retail option set: scripts\keys.cfg (bindings, hud colour, mouse and detail sliders) plus the console variables the
//! menus write into autoexec.cfg (`CInvertMouse`, `CWeaponBob`, `CAutoQuickSave`, `Music`, `Speech`, `Sound`, ...).
//! Both files use the retail formats and live in the export folder next to the game data.
use std::path::{Path,PathBuf};
use bevy::{prelude::*,ecs::system::SystemParam};
use crate::keys_cfg::KeysCfg;

/// The live option set (keys.cfg + autoexec cvars) every system reads: bindings, mouse, sound switches, detail switches.
#[derive(Resource,Deref,DerefMut)] pub struct Options(pub RetailOptions);
/// The binding table with the devices: `pressed(cmd::FIRE)` is true while any key or button bound to the retail command is held.
#[derive(SystemParam)] pub struct Bindings<'w> {pub options:Res<'w,Options>,pub keys:Res<'w,ButtonInput<KeyCode>>,pub mouse:Res<'w,ButtonInput<MouseButton>>,pub ui:Res<'w,crate::retail_ui::RetailUi>}
impl Bindings<'_> {
    pub fn pressed(&self,command:usize)->bool {self.options.keys.pressed(command,&self.keys,&self.mouse)}
    pub fn pressed_without(&self,command:usize,skip:&[KeyCode])->bool {self.options.keys.pressed_without(command,&self.keys,&self.mouse,skip)}
    pub fn just_pressed(&self,command:usize)->bool {self.options.keys.just_pressed(command,&self.keys,&self.mouse)}
    /// The retail key name of a command's first binding for hints ("E", "Space", "Egér 1"); empty when unbound.
    pub fn label(&self,command:usize)->String {self.options.keys.label(command,&self.ui)}
}
impl KeysCfg {
    /// The Hungarian retail key name (text_keys `CMD*`) of the first bound action of a command.
    pub fn label(&self,command:usize,ui:&crate::retail_ui::RetailUi)->String {
        self.bound(command).iter().find_map(|a|crate::keys_cfg::action_text_key(*a)).map(|key|ui.text(&key)).unwrap_or_default()
    }
}

/// Window modes offered on the display page; the retail launcher chose `screenwidth`/`screenheight` from a list like this.
pub const RESOLUTIONS:[(u32,u32);8]=[(640,480),(800,600),(1024,768),(1280,720),(1280,1024),(1600,900),(1920,1080),(2560,1440)];
pub const FPS_CAPS:[u32;5]=[60,120,160,240,0];

#[derive(Clone,Debug,PartialEq)]
pub struct RetailOptions {
    pub keys:KeysCfg,
    pub invert_mouse:bool,pub weapon_bob:bool,pub auto_quick_save:bool,pub music:bool,pub speech:bool,pub sound:bool,
    /// "Javított 3D hangzás" (spatial.rs; remake option, the `CSpatialAudio` console variable): on = per-ear panning, time difference and roll-off of every positional sound.
    pub spatial_audio:bool,
    /// "HD textúrák" (lib.rs `hd`, docs/retail-hd-textures.md; remake option, the `CHdTextures` console variable): on = load the Real-ESRGAN upscaled textures of `textures_hd/` where they exist. Off by default (retail textures).
    pub hd_textures:bool,
    /// "Gyári egeres választás" (remake option, owner request, the `CRetailChoiceMouse` console variable): off (default) = during a dialogue choice list the mouse
    /// look works normally and the answers are picked with the mouse wheel, Up / Down and Enter / click; on = exact retail (mouse motion scrolls the list, yaw live, pitch x0.25).
    /// `dialogue_hint_seen` (`CDialogHintSeen`): the one-time controls hint of the first choice list has been shown; a new game clears it. See docs/retail-dialogue.md.
    pub retail_choice_mouse:bool,pub dialogue_hint_seen:bool,
    pub model_shadow:bool,pub show_framerate:u32,pub max_fps:u32,pub windowed:bool,pub screen:[u32;2],
    /// The hidden retail console variables of autoexec.cfg, polled by cshell 0x10059230 and default off (0x1001ab34): `God` (every health change refills the health, 0x10061bb4),
    /// `FullStamina` (0x10061f19), `Invisible` (characters never see the player or hear a stimulus, 0x10043720, 0x10043860, 0x10045df0, 0x100499ba), `DrawKilledCount` (the kill
    /// counter top left, 0x1003ae60) and the path overlays `DrawPaths` / `DrawPostacPaths` (0x1003d9f0, 0x1004ae30; read, the debug overlays are not drawn). No menu edits them.
    pub god:bool,pub full_stamina:bool,pub invisible:bool,pub draw_killed_count:bool,pub draw_paths:bool,pub draw_postac_paths:bool,
    /// autoexec.cfg lines this game does not interpret (AddAction/rangebind/other console variables): written back untouched.
    extra:Vec<String>,
    /// A slider, toggle or binding changed and the files have to be written (the retail menu writes keys.cfg when it closes).
    pub dirty:bool,
    /// The options were loaded from files (a file the menu wrote), not built from the defaults alone.
    pub from_files:bool,
}
impl Default for RetailOptions {
    /// Retail initial values (cshell.dll 0x1001aa80..): mouse Y normal, weapon bob and auto quick save on, all sound on. The window
    /// defaults stay the remake's 1280x720 window; the shipped autoexec.cfg asks the launcher's 1024x768 full screen. The frame cap is off (the owner's modernisation: the presentation follows the monitor's refresh rate through vsync; retail asked MaxFPS 160).
    fn default()->Self {Self {keys:KeysCfg::default(),invert_mouse:false,weapon_bob:true,auto_quick_save:true,music:true,speech:true,sound:true,spatial_audio:true,hd_textures:false,retail_choice_mouse:false,dialogue_hint_seen:false,model_shadow:false,show_framerate:0,max_fps:0,windowed:true,screen:[1280,720],god:false,full_stamina:false,invisible:false,draw_killed_count:false,draw_paths:false,draw_postac_paths:false,extra:Vec::new(),dirty:false,from_files:false}}
}
pub fn keys_path(root:&Path)->PathBuf {root.join("scripts").join("keys.cfg")}
pub fn autoexec_path(root:&Path)->PathBuf {root.join("autoexec.cfg")}
/// Splits `"key" "value"` (the engine's own console-variable line) into its two words.
fn quoted_pair(line:&str)->Option<(String,String)> {
    let mut parts=line.trim().split('"').filter(|p|!p.trim().is_empty());
    let key=parts.next()?.trim().to_owned();let value=parts.next()?.trim().to_owned();
    (line.trim_start().starts_with('"') && parts.next().is_none()).then_some((key,value))
}
fn flag(value:&str)->bool {value.trim().parse::<f64>().map(|v|v!=0.0).unwrap_or(false)}
impl RetailOptions {
    pub fn load(root:&Path)->Self {
        let mut options=Self::default();
        if let Ok(bytes)=std::fs::read(keys_path(root)) {options.keys=KeysCfg::parse(&bytes.iter().map(|b|*b as char).collect::<String>());options.from_files=true;}
        if let Ok(text)=std::fs::read_to_string(autoexec_path(root)) {options.parse_autoexec(&text);options.from_files=true;}
        options
    }
    pub fn parse_autoexec(&mut self,text:&str) {
        self.extra.clear();
        for line in text.lines() {
            let Some((key,value))=quoted_pair(line) else {if !line.trim().is_empty() {self.extra.push(line.trim_end().to_owned());}continue};
            match key.to_ascii_lowercase().as_str() {
                "cinvertmouse"=>self.invert_mouse=flag(&value),"cweaponbob"=>self.weapon_bob=flag(&value),"cautoquicksave"=>self.auto_quick_save=flag(&value),
                "music"=>self.music=flag(&value),"speech"=>self.speech=flag(&value),"sound"=>self.sound=flag(&value),"cspatialaudio"=>self.spatial_audio=flag(&value),"chdtextures"=>self.hd_textures=flag(&value),"cretailchoicemouse"=>self.retail_choice_mouse=flag(&value),"cdialoghintseen"=>self.dialogue_hint_seen=flag(&value),"cdialoglock"=>{}, // the superseded view-lock option is dropped
                "modelshadow_proj_enable"=>self.model_shadow=flag(&value),
                "showframerate"=>self.show_framerate=value.trim().parse().unwrap_or(0),"maxfps"=>self.max_fps=value.trim().parse().unwrap_or(0),
                "windowed"=>self.windowed=flag(&value),"screenwidth"=>self.screen[0]=value.trim().parse().unwrap_or(1280).clamp(320,7680),"screenheight"=>self.screen[1]=value.trim().parse().unwrap_or(720).clamp(240,4320),
                // The hidden cheat / debug variables keep their line (no menu writes them) and set the flag the retail poll would.
                "god"|"fullstamina"|"invisible"|"drawkilledcount"|"drawpaths"|"drawpostacpaths"=>{
                    match key.to_ascii_lowercase().as_str() {"god"=>self.god=flag(&value),"fullstamina"=>self.full_stamina=flag(&value),"invisible"=>self.invisible=flag(&value),
                        "drawkilledcount"=>self.draw_killed_count=flag(&value),"drawpaths"=>self.draw_paths=flag(&value),_=>self.draw_postac_paths=flag(&value)}
                    self.extra.push(line.trim_end().to_owned());
                },
                _=>self.extra.push(line.trim_end().to_owned()),
            }
        }
    }
    /// autoexec.cfg text: the console variables first, then the untouched lines.
    pub fn autoexec_text(&self)->String {
        let b=|v:bool|(v as u8).to_string();
        let mut out=String::new();
        for (key,value) in [("windowed",b(self.windowed)),("screenwidth",self.screen[0].to_string()),("screenheight",self.screen[1].to_string()),("MaxFPS",self.max_fps.to_string()),("showframerate",self.show_framerate.to_string()),
            ("CInvertMouse",b(self.invert_mouse)),("CWeaponBob",b(self.weapon_bob)),("CAutoQuickSave",b(self.auto_quick_save)),("Music",b(self.music)),("Speech",b(self.speech)),("Sound",b(self.sound)),("CSpatialAudio",b(self.spatial_audio)),("CHdTextures",b(self.hd_textures)),("CRetailChoiceMouse",b(self.retail_choice_mouse)),("CDialogHintSeen",b(self.dialogue_hint_seen)),("ModelShadow_proj_enable",b(self.model_shadow))] {
            out.push_str(&format!("\"{key}\" \"{value}\"\r\n"));
        }
        for line in &self.extra {out.push_str(line);out.push_str("\r\n");}
        out
    }
    pub fn save(&mut self,root:&Path)->Result<(),String> {
        std::fs::create_dir_all(root.join("scripts")).map_err(|e|e.to_string())?;
        // keys.cfg is a Windows text file in a Central European code page: the values are ASCII, so bytes are written as they are.
        std::fs::write(keys_path(root),self.keys.write().as_bytes()).map_err(|e|format!("keys.cfg: {e}"))?;
        std::fs::write(autoexec_path(root),self.autoexec_text()).map_err(|e|format!("autoexec.cfg: {e}"))?;
        self.dirty=false;self.from_files=true;Ok(())
    }
    /// Next entry after `current` in a list of choices (wraps); an unknown current value starts from the first entry.
    pub fn next_in<T:PartialEq+Copy>(list:&[T],current:T)->T {list.iter().position(|v|*v==current).map_or(list[0],|i|list[(i+1)%list.len()])}
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn the_hidden_console_variables_are_read_and_written_back() {
        let mut options=RetailOptions::default();
        assert!(!options.god && !options.full_stamina && !options.invisible && !options.draw_killed_count,"all off by default (0x1001ab34)");
        options.parse_autoexec("\"God\" \"1\"\r\n\"fullstamina\" \"1\"\r\n\"Invisible\" \"0\"\r\n\"DrawKilledCount\" \"2\"\r\n\"DrawPaths\" \"1\"\r\n");
        assert!(options.god && options.full_stamina && !options.invisible && options.draw_killed_count && options.draw_paths && !options.draw_postac_paths);
        let text=options.autoexec_text();
        assert!(text.contains("\"God\" \"1\"") && text.contains("\"DrawKilledCount\" \"2\""),"the lines survive a rewrite untouched");
    }
    #[test] fn the_shipped_autoexec_cfg_is_read_and_its_other_lines_survive_a_rewrite() {
        let Ok(text)=std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../GYARI/autoexec.cfg")) else {return};
        let mut options=RetailOptions::default();options.parse_autoexec(&text);
        assert!(options.weapon_bob && options.auto_quick_save && !options.invert_mouse && options.music && options.speech && options.sound && !options.model_shadow);
        assert_eq!((options.max_fps,options.show_framerate,options.windowed,options.screen),(160,2,false,[1024,768]));
        let rewritten=options.autoexec_text();
        assert!(rewritten.contains("\"CWeaponBob\" \"1\"") && rewritten.contains("AddAction QuickSave 251") && rewritten.contains("rangebind \"##keyboard\" \"F5\" 0.000000 0.000000 \"QuickSave\""));
        assert!(rewritten.contains("\"MaxTextureSize\" \"512*512\"") && rewritten.contains("\"bitdepth\" \"32\""),"unknown console variables are kept");
        let mut again=RetailOptions::default();again.parse_autoexec(&rewritten);assert_eq!(again.autoexec_text(),rewritten);
        assert_eq!(again.keys,options.keys);
    }
    #[test] fn options_round_trip_through_the_two_files() {
        let dir=std::env::temp_dir().join(format!("mester-options-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
        let mut options=RetailOptions::default();
        options.invert_mouse=true;options.weapon_bob=false;options.music=false;options.spatial_audio=false;options.hd_textures=true;options.retail_choice_mouse=true;options.dialogue_hint_seen=true;options.max_fps=60;options.windowed=false;options.screen=[1920,1080];options.keys.assign(crate::keys_cfg::cmd::JUMP,1,30);options.keys.gamma=1.5;options.keys.subtitles=false;
        options.save(&dir).unwrap();
        let back=RetailOptions::load(&dir);
        assert_eq!(back.keys,options.keys);assert!(back.invert_mouse && !back.weapon_bob && !back.music && back.speech && !back.windowed && !back.spatial_audio && back.hd_textures && back.retail_choice_mouse && back.dialogue_hint_seen);assert_eq!((back.max_fps,back.screen),(60,[1920,1080]));assert!(back.from_files);
        assert!(std::fs::read_to_string(keys_path(&dir)).unwrap().starts_with("Generated by RatHunt.\r\nDo NOT modify!\r\n \r\n"));
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test] fn missing_files_give_the_retail_defaults_and_choices_wrap() {
        let options=RetailOptions::load(Path::new("Z:/does/not/exist"));
        assert!(!options.from_files && options.spatial_audio && !options.hd_textures && !options.retail_choice_mouse && !options.dialogue_hint_seen && options.weapon_bob && options.auto_quick_save && !options.invert_mouse && options.keys==KeysCfg::default());
        assert_eq!(RetailOptions::next_in(&FPS_CAPS,240),0);assert_eq!(RetailOptions::next_in(&FPS_CAPS,0),60);assert_eq!(RetailOptions::next_in(&FPS_CAPS,77),60);
        assert_eq!(quoted_pair("\"Music\" \"1\""),Some(("Music".into(),"1".into())));assert_eq!(quoted_pair("AddAction A 1"),None);assert_eq!(quoted_pair("scale \"##mouse\" \"##y-axis\" 0.0"),None);
    }
}
