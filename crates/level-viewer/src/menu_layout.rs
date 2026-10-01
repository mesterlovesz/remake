//! Geometry and rules of the retail menu pages (cshell.dll 0x10023000..0x1002b000), all in the 1024x768 design space the
//! retail code scales with `screen width / 1024`. Everything here is pure so it can be tested without a window.
use bevy::math::{Vec2,Rect};
use crate::{keys_cfg::KeysCfg,options::{self,RetailOptions},settings::Preferences,savefile};

// Page ids: the retail ids of `[menu + 0x550cb4]` are 0 main, 1 misc, 2 sound, 5/6 controls, 7 char, 9 display, 0xa save, 0xb load;
// the remake keeps the numbers its earlier screens used and adds the missing ones.
pub const MAIN:u8=0;pub const CHAR:u8=1;pub const MISC:u8=2;pub const CONTROLS1:u8=3;pub const PERF:u8=4;pub const SOUND:u8=5;pub const CONTROLS2:u8=6;pub const LOAD:u8=7;pub const SAVE:u8=8;
/// The remake's own screen page (window, resolution, frame cap, field of view): the retail video pages 3 and 4 are unreachable code.
pub const VIDEO:u8=9;
/// The quit confirmation (retail art `confirm_quit_*.pcx` is shipped but cshell never loads it; the remake shows it before leaving).
pub const CONFIRM:u8=10;
/// The owner-requested bonus page (retail has none): one row per [`BONUS_ITEMS`] entry plus the retail back row.
pub const BONUS:u8=11;
/// A bonus entry: text keys of the row and of its description, and the level it starts (fresh game, see `Frontend::bonus`).
pub struct BonusItem {pub label:&'static str,pub info:&'static str,pub world:&'static str}
/// Kiskína (`chinatown.dat`) is the only entry that loads and plays (docs/cut-content.md). `nic` has no mission script or NPC / prop export and `outro` is an empty stage with a cutscene object: both stay out.
pub const BONUS_ITEMS:[BonusItem;1]=[BonusItem {label:"RemakeBonusKiskina",info:"RemakeBonusKiskinaInfo",world:"chinatown"}];
/// The description lines sit under the list (white with the black shadow of the load page details), wrapped to this many Mincho cells.
pub const BONUS_INFO_Y:[f32;2]=[556.0,586.0];pub const BONUS_INFO_SCALE:f32=0.3;pub const BONUS_INFO_CELLS:usize=62;
/// Greedy word wrap into at most `lines` lines of `cells` characters (the rest is cut), always exactly `lines` entries.
pub fn wrap_lines(text:&str,cells:usize,lines:usize)->Vec<String> {
    let mut out:Vec<String>=Vec::new();
    for word in text.split_whitespace() {
        match out.last_mut() {Some(last) if last.chars().count()+1+word.chars().count()<=cells=>{last.push(' ');last.push_str(word);},_=>out.push(word.to_owned())}
    }
    out.resize(lines,String::new());out.truncate(lines);out
}

/// Lines of scripts\locale\main_menu.txt; a slot holds the 1-based line number ("text id") of its label (0x10024230..0x10025158).
pub const TEXT_KEYS:[&str;54]=["MainMenuStartGame","","","MainMenuControls","MainMenuMiscOptions","MainMenuQuitGame","","","","","","","","MenuGoToPrevious","","","","","","","","","","","","","MainMenuPerfAndDisplay","MainMenuShotDebrisOn","MainMenuShotDebrisOff","MainMenuIObjAllOn","MainMenuIObjPartOff","MainMenuInsCharOn","MainMenuInsCharOff","MainMenuEnLOn","MainMenuEnLOff","MainMenuIObjAllOff","","MainMenuLoadGame","MainMenuSaveGame","MainMenuSubtitlesOn","MainMenuSubtitlesOff","MainMenuMouseYInversed","MainMenuMouseYNormal","MainMenuQuickSaveNewLevelYes","MainMenuQuickSaveNewLevelNo","MainMenuWeaponBobOn","MainMenuWeaponBobOff","MainMenuSoundOptions","MainMenuMusicOn","MainMenuMusicOff","MainMenuSpeechOn","MainMenuSpeechOff","MainMenuSoundsOn","MainMenuSoundsOff"];
pub const MAIN_IDS:[u8;8]=[1,38,39,5,4,27,48,6];
pub fn text_key(id:u8)->&'static str {TEXT_KEYS.get((id as usize).wrapping_sub(1)).copied().unwrap_or("")}

// ---- list pages (0x10027aa0 draw, 0x1002a450 click)
pub const LIST_X:f32=340.0;pub const LIST_Y:f32=240.0;pub const LIST_PITCH:f32=52.0;pub const LIST_SLOTS:usize=12;pub const LIST_HOVER_W:f32=256.0;pub const LABEL_SCALE:f32=0.35;
pub const LABEL_COLOR:[u8;3]=[199,194,158];pub const HOVER_COLOR:[u8;3]=[255,224,0];pub const GHOST_COLOR:[u8;3]=[71,66,54];
/// A click counts from x = 340 to the right edge and from y = 240 down (0x1002a4fb..0x1002a55e); the row is `(y - 240) / 52`.
pub fn list_row_at(pos:Vec2,pitch:f32)->Option<usize> {(pos.x>=LIST_X && pos.y>=LIST_Y).then(||((pos.y-LIST_Y)/pitch) as usize).filter(|row|*row<LIST_SLOTS)}
/// The label lights up only inside x 340..596 (0x10027c30) on its own row.
pub fn list_hover_at(pos:Vec2,pitch:f32)->Option<usize> {(pos.x>LIST_X && pos.x<LIST_X+LIST_HOVER_W).then(||list_row_at(pos,pitch)).flatten()}
/// Row pitch of a list page: 52 (`[menu + 0x550c98]`); the display list carries the remake's extra row, so its rows sit closer to fit the panel.
pub fn list_pitch(page:u8)->f32 {if page==PERF || page==VIDEO {46.0}else{LIST_PITCH}}
pub const BACK_BUTTON:Rect=Rect {min:Vec2::new(954.0,653.0),max:Vec2::new(1000.0,699.0)};
pub fn inside(rect:Rect,pos:Vec2)->bool {pos.x>rect.min.x && pos.x<rect.max.x && pos.y>rect.min.y && pos.y<rect.max.y}

/// What a click on a page asks the shell to do.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Request {None,Goto(u8),NewPlayer,Load,Save,Quit,/// Start bonus entry `i` of [`BONUS_ITEMS`].
    Bonus(usize)}
/// The slot labels of a list page; `loc` maps a text key (without '>') to its Hungarian text.
/// `opened` is the sound device the game opened (Session::output, output.rs).
pub fn list_rows(page:u8,options:&RetailOptions,prefs:&Preferences,opened:&str,loc:&dyn Fn(&str)->String)->Vec<String> {
    let k=&options.keys;let by=|id:u8|loc(text_key(id));
    let pick=|on:bool,on_id:u8|by(if on {on_id}else{on_id+1});
    match page {
        // The bonus row sits below the retail ones so every retail row keeps its y (52 px pitch from 240).
        MAIN=>MAIN_IDS.iter().map(|id|by(*id)).chain([loc("RemakeBonus")]).collect(),
        BONUS=>BONUS_ITEMS.iter().map(|item|loc(item.label)).chain([by(14)]).collect(),
        PERF=>vec![pick(k.shot_debris,28),by([30,31,36][k.insignificant_objects.clamp(0,2) as usize]),pick(k.insignificant_characters,32),pick(k.enemy_lights,34),pick(k.subtitles,40),
            pick(options.invert_mouse,42),pick(options.auto_quick_save,44),pick(options.weapon_bob,46),loc(if options.retail_choice_mouse {"RemakeChoiceMouseOn"}else{"RemakeChoiceMouseOff"}),by(14),loc("RemakeVideoPage")],
        SOUND=>vec![pick(options.music,49),pick(options.speech,51),pick(options.sound,53),by(14),format!("{} {}",loc("RemakeOutput"),crate::output::label(&prefs.output_device,opened)),loc(if options.spatial_audio {"RemakeSpatialOn"}else{"RemakeSpatialOff"})],
        VIDEO=>vec![loc(if options.windowed {"RemakeWindowed"}else{"RemakeFullscreen"}),format!("{} {}x{}",loc("RemakeResolution"),options.screen[0],options.screen[1]),
            format!("{} {}",loc("RemakeFrameCap"),if options.max_fps==0 {loc("RemakeUnlimited")}else{options.max_fps.to_string()}),
            loc(if options.show_framerate>0 {"RemakeFramerateOn"}else{"RemakeFramerateOff"}),format!("{} {:.0}",loc("RemakeFov"),prefs.fov),
            loc(if options.model_shadow {"RemakeShadowsOn"}else{"RemakeShadowsOff"}),loc(if prefs.text_aid {"RemakeTextAidOn"}else{"RemakeTextAidOff"}),loc(if options.hd_textures {"RemakeHdOn"}else{"RemakeHdOff"}),by(14)],
        CONFIRM=>vec![by(6),by(14)],
        _=>Vec::new(),
    }
}
/// Handles a click on row `row` of a list page. `can_save` is the retail condition of the "save" row (in a level, alive, not kneeling).
pub fn click_row(page:u8,row:usize,options:&mut RetailOptions,prefs:&mut Preferences,can_save:bool)->Request {
    let mut changed=true;
    let outcome=match (page,row) {
        (MAIN,0)=>Request::NewPlayer,(MAIN,1)=>Request::Load,(MAIN,2)=>{changed=false;if can_save {Request::Save}else{Request::None}},
        (MAIN,3)=>Request::Goto(MISC),(MAIN,4)=>Request::Goto(CONTROLS1),(MAIN,5)=>Request::Goto(PERF),(MAIN,6)=>Request::Goto(SOUND),(MAIN,7)=>Request::Goto(CONFIRM),(MAIN,8)=>Request::Goto(BONUS),
        (BONUS,row) if row<BONUS_ITEMS.len()=>{changed=false;Request::Bonus(row)},(BONUS,row) if row==BONUS_ITEMS.len()=>Request::Goto(MAIN),
        (PERF,0)=>{options.keys.shot_debris^=true;Request::None},(PERF,1)=>{options.keys.insignificant_objects=(options.keys.insignificant_objects+1)%3;Request::None},
        (PERF,2)=>{options.keys.insignificant_characters^=true;Request::None},(PERF,3)=>{options.keys.enemy_lights^=true;Request::None},(PERF,4)=>{options.keys.subtitles^=true;Request::None},
        (PERF,5)=>{options.invert_mouse^=true;Request::None},(PERF,6)=>{options.auto_quick_save^=true;Request::None},(PERF,7)=>{options.weapon_bob^=true;Request::None},
        (PERF,8)=>{options.retail_choice_mouse^=true;Request::None},(PERF,9)=>Request::Goto(MAIN),(PERF,10)=>Request::Goto(VIDEO),
        (SOUND,0)=>{options.music^=true;Request::None},(SOUND,1)=>{options.speech^=true;Request::None},(SOUND,2)=>{options.sound^=true;Request::None},(SOUND,3)=>Request::Goto(MAIN),(SOUND,4)=>{prefs.output_device=crate::output::cycle(&prefs.output_device,1);Request::None},(SOUND,5)=>{options.spatial_audio^=true;Request::None},
        (VIDEO,0)=>{options.windowed^=true;Request::None},
        (VIDEO,1)=>{let now=(options.screen[0],options.screen[1]);let next=RetailOptions::next_in(&options::RESOLUTIONS,now);options.screen=[next.0,next.1];Request::None},
        (VIDEO,2)=>{options.max_fps=RetailOptions::next_in(&options::FPS_CAPS,options.max_fps);Request::None},
        (VIDEO,3)=>{options.show_framerate=if options.show_framerate>0 {0}else{1};Request::None},
        (VIDEO,4)=>{prefs.fov=if prefs.fov+5.0>111.0 {61.0}else{prefs.fov+5.0};prefs.sanitize();Request::None},
        (VIDEO,5)=>{options.model_shadow^=true;Request::None},(VIDEO,6)=>{prefs.text_aid^=true;Request::None},(VIDEO,7)=>{options.hd_textures^=true;Request::None},(VIDEO,8)=>Request::Goto(PERF),
        (CONFIRM,0)=>Request::Quit,(CONFIRM,1)=>Request::Goto(MAIN),
        _=>{changed=false;Request::None},
    };
    // Navigation (a page change, a request to the shell) leaves the option files alone; only toggles and cycles are changes.
    if changed && outcome==Request::None {options.dirty=true;}
    outcome
}
/// The page the Esc key returns to (0x1002ad10): the sub pages go back to the main list, the second controls page to the first.
pub fn escape_target(page:u8)->Option<u8> {match page {MAIN=>None,CONTROLS2=>Some(CONTROLS1),VIDEO=>Some(PERF),_=>Some(MAIN)}}

// ---- misc page sliders (0x10026120 draw, 0x10028e00 drag): knob x for value, x for knob y
pub const SLIDER_X:f32=570.0;pub const SLIDER_W:f32=256.0;pub const SLIDER_Y:[f32;4]=[244.0,306.0,369.0,433.0];pub const KNOB_SIZE:Vec2=Vec2::new(8.0,16.0);
/// The volume slider the remake adds under the sound toggles (the retail sound page has none).
pub const VOLUME_Y:f32=572.0;
/// Which slider a press at `pos` grabs: x 570..826 and a 16 px band from each knob's top.
pub fn slider_at(pos:Vec2,y:&[f32])->Option<usize> {(pos.x>SLIDER_X && pos.x<SLIDER_X+SLIDER_W).then(||y.iter().position(|top|pos.y>*top && pos.y<top+KNOB_SIZE.y)).flatten()}
/// Knob left edge for a slider value (sensitivity 0.5..1.5, smoothness 0..1, hud colour 0..7, gamma 1..2, volume 0..1).
pub fn knob_x(slider:usize,cfg:&KeysCfg,volume:f32)->f32 {
    match slider {
        0=>(cfg.mouse_sensitivity-0.5)*SLIDER_W+SLIDER_X,1=>cfg.mouse_smoothness*SLIDER_W+SLIDER_X,2=>cfg.hud_color as f32*32.0+16.0+SLIDER_X,3=>(cfg.gamma-1.0)*SLIDER_W+SLIDER_X,
        _=>volume.clamp(0.0,1.0)*SLIDER_W+SLIDER_X,
    }
}
/// Dragging: the value for mouse x. The retail code ignores positions outside x 570..826 while dragging.
pub fn drag_slider(slider:usize,mouse_x:f32,cfg:&mut KeysCfg,volume:&mut f32)->bool {
    if !(mouse_x>SLIDER_X && mouse_x<SLIDER_X+SLIDER_W) {return false;}
    let t=(mouse_x-SLIDER_X)/SLIDER_W;
    match slider {
        0=>cfg.mouse_sensitivity=t+0.5,1=>cfg.mouse_smoothness=t,2=>cfg.hud_color=((t*8.0) as i32).clamp(0,7),3=>cfg.gamma=t+1.0,_=>*volume=t,
    }
    *cfg=cfg.clone().sanitized();true
}

// ---- controls pages (0x10025bf0, 0x10025e90 draw; 0x10028790, 0x10028ad0 click)
pub const NEXT_BUTTON:Rect=Rect {min:Vec2::new(362.0,665.0),max:Vec2::new(458.0,713.0)};
pub fn cell_center(row:usize,column:usize)->Vec2 {Vec2::new(616.0+192.0*column as f32,224.0+48.0*row as f32)}
/// Key text top-left y (48 per row from 220) and the row label positions (50 per row from 220, x 320).
pub fn cell_text_y(row:usize)->f32 {220.0+48.0*row as f32}
pub fn row_label_y(row:usize)->f32 {220.0+50.0*row as f32}
/// The binding cell under `pos`: 190 px wide and 30 px high around the centre (0x10028929..0x10028965).
pub fn cell_at(pos:Vec2,rows:usize)->Option<(usize,usize)> {
    (0..rows).flat_map(|r|(0..2).map(move|c|(r,c))).find(|(r,c)|{let center=cell_center(*r,*c);pos.x>center.x-95.0 && pos.x<center.x+95.0 && pos.y>center.y-15.0 && pos.y<center.y+15.0})
}

// ---- load and save pages (0x10026ed0, 0x10027260, 0x10029460, 0x10029c50)
pub const THUMB_POS:Vec2=Vec2::new(345.0,233.0);
pub const DETAIL_POS:[Vec2;2]=[Vec2::new(360.0,556.0),Vec2::new(360.0,586.0)];
pub const SLOT_TEXT_X:f32=714.0;pub const SLOT_TEXT_Y:f32=280.0;pub const SLOT_SCALE:f32=0.3;pub const SLOT_ROWS:usize=10;
/// Row pitch: 1.3 x the height of a 0.3 scale line (64 * 0.3 = 19.2, truncated to 19 by the text size query).
pub const SLOT_PITCH:f32=19.0*1.3;
pub const SLOT_HIT:Rect=Rect {min:Vec2::new(724.0,280.0),max:Vec2::new(884.0,280.0+SLOT_PITCH*SLOT_ROWS as f32)};
pub const SCROLL_X:f32=928.0;pub const SCROLL_UP:Rect=Rect {min:Vec2::new(928.0,257.0),max:Vec2::new(944.0,273.0)};pub const SCROLL_DOWN:Rect=Rect {min:Vec2::new(928.0,544.0),max:Vec2::new(944.0,560.0)};
pub const SCROLL_TOP:f32=275.0;pub const SCROLL_TRAVEL:f32=268.0;
pub const ACTION_BUTTON:Rect=Rect {min:Vec2::new(788.0,693.0),max:Vec2::new(891.0,736.0)};
pub const ACTION_LABEL:Vec2=Vec2::new(810.0,700.0);
/// Seconds a held scroll arrow needs per step (0x10029993).
pub const SCROLL_STEP:f32=0.1;

/// The list of slots on the load and save pages with the retail scrolling rules.
#[derive(Clone,Debug,Default)]
pub struct SlotList {pub slots:Vec<savefile::Slot>,/// The save page starts with a "new save" entry that has no slot yet.
    pub with_new:bool,pub top:usize,/// Selected row inside the visible window (0..10), like `[menu + 0x550cc4]`.
    pub row:usize}
impl SlotList {
    pub fn new(slots:Vec<savefile::Slot>,with_new:bool)->Self {Self {slots,with_new,top:0,row:0}}
    pub fn len(&self)->usize {self.slots.len()+self.with_new as usize}
    pub fn index(&self)->usize {self.top+self.row}
    /// `None` = the "new save" entry (save page, first row).
    pub fn selected(&self)->Option<Option<&savefile::Slot>> {
        let i=self.index();if i>=self.len() {return None;}
        Some(if self.with_new {i.checked_sub(1).map(|j|&self.slots[j])}else{Some(&self.slots[i])})
    }
    pub fn scroll(&mut self,delta:i32) {self.top=(self.top as i32+delta).clamp(0,self.len().saturating_sub(1) as i32) as usize;}
    /// A visible row picked by the mouse; rows past the end of the list are ignored.
    pub fn row_at(&self,pos:Vec2)->Option<usize> {
        if !(pos.x>=SLOT_HIT.min.x && pos.x<SLOT_HIT.max.x && pos.y>=SLOT_HIT.min.y) {return None;}
        let row=((pos.y-SLOT_HIT.min.y)/SLOT_PITCH) as usize;(row<SLOT_ROWS && self.top+row<self.len()).then_some(row)
    }
    /// Knob top for the current scroll position (275 + 268 * top / count).
    pub fn knob_y(&self)->f32 {SCROLL_TOP+SCROLL_TRAVEL*self.top as f32/self.len().max(1) as f32}
    /// Dragging the knob: the entry at the mouse height.
    pub fn drag_to(&mut self,y:f32) {
        if self.len()==0 {return;}
        let index=((y-SCROLL_TOP)/(SCROLL_TRAVEL/self.len() as f32)).max(0.0) as usize;self.top=index.min(self.len()-1);
    }
    pub fn window(&self)->std::ops::Range<usize> {self.top.min(self.len())..(self.top+SLOT_ROWS).min(self.len())}
}


// ---- the scene of a page: everything the renderer draws, in design-space pixels (cshell 0x10027d70 draw dispatcher, 0x10027f40 overlays)
pub const RED:[u8;3]=[255,0,0];pub const WHITE:[u8;3]=[255,255,255];pub const BLACK:[u8;3]=[0,0,0];
/// The cell waiting for a key is drawn green (0x10025b98: colour 0,255,0), an empty cell reads `*****` (0x10025acf).
pub const CAPTURE_COLOR:[u8;3]=[0,255,0];
pub const SLOT_SELECTED:[u8;3]=[255,240,160];pub const SLOT_NORMAL:[u8;3]=[96,176,192];
pub const TITLE_POS:Vec2=Vec2::new(600.0,90.0);
/// Texts of the pages the remake adds to the retail set (the retail files have no strings for them).
pub fn remake_text(key:&str)->&'static str {
    match key {
        "RemakeBonus"=>"Bónusz","RemakeBonusKiskina"=>"Kiskína (kiadatlan pálya)","RemakeBonusKiskinaInfo"=>"A gyári telepítésben megmaradt, a kampányból kihagyott pálya (chinatown.dat)",
        "RemakeVideoPage"=>"Képbeállítások","RemakeWindowed"=>"Ablakos mód","RemakeFullscreen"=>"Teljes képernyő","RemakeResolution"=>"Felbontás","RemakeFrameCap"=>"Képkockakorlát",
        "RemakeUnlimited"=>"korlátlan (monitor Hz)","RemakeFramerateOn"=>"Képkockaszám kijelzése be","RemakeFramerateOff"=>"Képkockaszám kijelzése ki","RemakeFov"=>"Látószög",
        "RemakeShadowsOn"=>"Modellárnyékok be","RemakeShadowsOff"=>"Modellárnyékok ki","RemakeTextAidOn"=>"Olvasható feliratok (körvonal) be","RemakeTextAidOff"=>"Olvasható feliratok (körvonal) ki","RemakeVolume"=>"Hangerő","RemakeOutput"=>"Hangkimenet","RemakeSpatialOn"=>"Javított 3D hangzás be","RemakeSpatialOff"=>"Javított 3D hangzás ki","RemakeHdOn"=>"HD textúrák be (új pályától)","RemakeHdOff"=>"HD textúrák ki","RemakeChoiceMouseOn"=>"Gyári egeres választás be","RemakeChoiceMouseOff"=>"Gyári egeres választás ki",_=>"",
    }
}
/// Background art of a page (misc\menu\<name>1024.pcx exported to png).
pub fn background(page:u8)->&'static str {
    match page {
        CHAR=>"ui/menu/char_create_1024.png",MISC=>"ui/menu/misc_options_1024.png",CONTROLS1=>"ui/menu/controls_menu1_1024.png",CONTROLS2=>"ui/menu/controls_menu2_1024.png",
        PERF|VIDEO|BONUS=>"ui/menu/performance_1024.png",SOUND=>"ui/menu/sound_options_1024.png",LOAD=>"ui/menu/load_1024.png",SAVE=>"ui/menu/save_1024.png",CONFIRM=>"ui/menu/confirm_quit_1024.png",
        _=>"ui/menu/main_menu_1024.png",
    }
}
/// The page's title text key (list pages PERF / SOUND / VIDEO / CONFIRM have none: their draw sets no header object).
pub fn title_key(page:u8)->Option<&'static str> {
    match page {MAIN=>Some("MainMenuHeader"),BONUS=>Some("RemakeBonus"),MISC=>Some("MOMHeader"),CONTROLS1=>Some("C1MControls"),CONTROLS2=>Some("C2MControls"),LOAD=>Some("LGMHeader"),SAVE=>Some("SGMHeader"),_=>None}
}
pub const MISC_LABELS:[&str;4]=["MOMMouseSens","MOMMouseSmooth","MOMHudColor","MOMGamma"];
pub const MISC_LABEL_Y:[f32;4]=[240.0,300.0,360.0,420.0];
pub const CONTROLS1_LABELS:[&str;9]=["C1MForward","C1MBackward","C1MSL","C1MSR","C1MJump","C1MCrouch","C1MRun","C1MToggleRun","C1MAction"];
pub const CONTROLS2_LABELS:[&str;8]=["C2MFire","C2MAlt","C2MReload","C2MNext","C2MPrev","C2MInv","C2MChar","C2MPlayer"];
/// Key text of a binding cell: the retail key name, `*****` for a free slot (0x10025acf).
pub fn cell_text(action:u8,loc:&dyn Fn(&str)->String)->String {crate::keys_cfg::action_text_key(action).map(|key|loc(&key)).filter(|t|!t.is_empty()).unwrap_or_else(||"*****".to_owned())}

#[derive(Clone,Debug,PartialEq)]
pub enum Item {
    Text {text:String,pos:Vec2,scale:f32,rgb:[u8;3]},
    /// The faint enlarged copy behind a list label (0x10027aa0): `x` is the label column, `row` its list row.
    Ghost {text:String,x:f32,row:usize,pitch:f32},
    /// `crop` is a pixel rectangle of the source image.
    Image {path:&'static str,pos:Vec2,size:Vec2,crop:Option<Rect>},
    /// The picture of the selected save slot (or of the screen for the "new save" entry).
    Thumb {pos:Vec2,size:Vec2},
}
pub struct PageView<'a> {
    pub page:u8,pub mouse:Vec2,pub in_game:bool,pub hover:Option<usize>,pub capture:Option<(usize,usize)>,pub scroll_held:Option<bool>,
    pub options:&'a RetailOptions,pub prefs:&'a Preferences,pub opened:&'a str,pub volume:f32,
    pub slots:&'a SlotList,pub slot_labels:&'a [String],pub details:&'a [String;2],pub has_thumb:bool,
    pub loc:&'a dyn Fn(&str)->String,
}
fn text(text:impl Into<String>,x:f32,y:f32,scale:f32,rgb:[u8;3])->Item {Item::Text {text:text.into(),pos:Vec2::new(x,y),scale,rgb}}
/// Text with the retail black drop shadow (2 px right and down).
fn shadowed(items:&mut Vec<Item>,t:String,x:f32,y:f32,scale:f32,rgb:[u8;3]) {items.push(text(t.clone(),x+2.0,y+2.0,scale,BLACK));items.push(text(t,x,y,scale,rgb));}
/// Width of a text in Mincho cells (fixed 32 px advance at scale 1, the retail font table).
pub fn cell_width(text:&str,scale:f32)->f32 {text.chars().count() as f32*32.0*scale}
pub fn scene(s:&PageView)->Vec<Item> {
    let mut items=Vec::new();let loc=s.loc;
    if let Some(key)=title_key(s.page) {items.push(text(loc(key),TITLE_POS.x,TITLE_POS.y,0.5,RED));}
    match s.page {
        MAIN|PERF|SOUND|VIDEO|CONFIRM|BONUS=>{
            for (row,label) in list_rows(s.page,s.options,s.prefs,s.opened,loc).into_iter().enumerate() {
                if label.is_empty() {continue;}
                items.push(Item::Ghost {text:label.clone(),x:LIST_X,row,pitch:list_pitch(s.page)});
                items.push(text(label,LIST_X,LIST_Y+list_pitch(s.page)*row as f32+(list_pitch(s.page)/3.0).trunc(),LABEL_SCALE,if s.hover==Some(row) {HOVER_COLOR}else{LABEL_COLOR}));
            }
            if s.page==BONUS {
                // The description of the row under the pointer / keyboard cursor (the first entry when none): always four items so the hover edit stays in place.
                let info=BONUS_ITEMS.get(s.hover.unwrap_or(0)).map(|item|loc(item.info)).unwrap_or_default();
                for (line,y) in wrap_lines(&info,BONUS_INFO_CELLS,2).into_iter().zip(BONUS_INFO_Y) {shadowed(&mut items,line,LIST_X,y,BONUS_INFO_SCALE,WHITE);}
            }
            if s.page==SOUND {
                items.push(text(loc("RemakeVolume"),LIST_X,VOLUME_Y-5.0,LABEL_SCALE,LABEL_COLOR));
                items.push(Item::Image {path:"ui/menu/misc_options_1024.png",pos:Vec2::new(SLIDER_X,VOLUME_Y+2.0),size:Vec2::new(SLIDER_W,12.0),crop:Some(Rect::new(SLIDER_X,246.0,SLIDER_X+SLIDER_W,258.0))});
                items.push(Item::Image {path:"ui/menu/misc_slider.png",pos:Vec2::new(knob_x(4,&s.options.keys,s.volume),VOLUME_Y),size:KNOB_SIZE,crop:None});
            }
        },
        MISC=>{
            for (label,y) in MISC_LABELS.iter().zip(MISC_LABEL_Y) {items.push(text(loc(label),320.0,y,0.3,RED));}
            for i in 0..4 {items.push(Item::Image {path:"ui/menu/misc_slider.png",pos:Vec2::new(knob_x(i,&s.options.keys,s.volume),SLIDER_Y[i]),size:KNOB_SIZE,crop:None});}
        },
        CONTROLS1|CONTROLS2=>{
            items.push(text(loc("C1MPrim"),560.0,190.0,0.25,RED));items.push(text(loc("C1MSec"),740.0,190.0,0.25,RED));
            let (rows,labels):(&[usize],&[&str])=if s.page==CONTROLS1 {(&crate::keys_cfg::CONTROLS1_ROWS,&CONTROLS1_LABELS)}else{(&crate::keys_cfg::CONTROLS2_ROWS,&CONTROLS2_LABELS)};
            for (row,(command,label)) in rows.iter().zip(labels.iter()).enumerate() {
                items.push(text(loc(label),320.0,row_label_y(row),0.3,RED));
                for column in 0..2 {
                    let t=cell_text(s.options.keys.bound(*command)[column],loc);let width=cell_width(&t,0.42);
                    let rgb=if s.capture==Some((row,column)) {CAPTURE_COLOR}else{WHITE};
                    items.push(text(t,cell_center(row,column).x-(width/2.0).trunc(),cell_text_y(row),0.42,rgb));
                }
            }
            let button=if s.page==CONTROLS1 {"ui/menu/controls_button_next.png"}else{"ui/menu/controls_button_previous.png"};
            if inside(NEXT_BUTTON,s.mouse) {items.push(Item::Image {path:button,pos:NEXT_BUTTON.min,size:NEXT_BUTTON.size(),crop:None});}
        },
        LOAD|SAVE=>{
            let (avail,details,action)=if s.page==LOAD {("LGMAvail","LGMDetails","LGMLoad")}else{("SGMAvail","SGMDetails","SGMSave")};
            items.push(text(loc(avail),700.0,230.0,0.3,RED));items.push(text(loc(details),470.0,630.0,0.3,RED));items.push(text(loc(action),ACTION_LABEL.x,ACTION_LABEL.y,0.4,RED));
            if s.has_thumb {items.push(Item::Thumb {pos:THUMB_POS,size:Vec2::new(320.0,240.0)});}
            if !s.details[0].is_empty() {shadowed(&mut items,s.details[0].clone(),DETAIL_POS[0].x,DETAIL_POS[0].y,0.3,WHITE);}
            if !s.details[1].is_empty() {shadowed(&mut items,s.details[1].clone(),DETAIL_POS[1].x,DETAIL_POS[1].y,0.3,WHITE);}
            for (row,index) in s.slots.window().enumerate() {
                let label=s.slot_labels.get(index).cloned().unwrap_or_default();
                shadowed(&mut items,label,SLOT_TEXT_X,SLOT_TEXT_Y+SLOT_PITCH*row as f32,SLOT_SCALE,if row==s.slots.row {SLOT_SELECTED}else{SLOT_NORMAL});
            }
            items.push(Item::Image {path:"ui/menu/scroll_slider.png",pos:Vec2::new(SCROLL_X,s.slots.knob_y()),size:Vec2::new(16.0,32.0),crop:None});
            match s.scroll_held {
                Some(true)=>items.push(Item::Image {path:"ui/menu/scroll_up.png",pos:SCROLL_UP.min,size:Vec2::splat(16.0),crop:None}),
                Some(false)=>items.push(Item::Image {path:"ui/menu/scroll_down.png",pos:SCROLL_DOWN.min,size:Vec2::splat(16.0),crop:None}),None=>{}
            }
        },
        _=>{},
    }
    // The back button lights up under the pointer; on the main list only while a game runs (0x10027fb0).
    if s.page!=CHAR && (s.page!=MAIN || s.in_game) && inside(BACK_BUTTON,s.mouse) {items.push(Item::Image {path:"ui/menu/Menu_but_1024.png",pos:BACK_BUTTON.min,size:BACK_BUTTON.size(),crop:None});}
    items
}

#[cfg(test)] mod tests {
    use super::*;
    fn loc(key:&str)->String {format!("<{key}>")}
    #[test] fn the_main_list_maps_rows_to_the_retail_texts_and_actions() {
        let o=RetailOptions::default();let p=Preferences::default();
        let rows=list_rows(MAIN,&o,&p,"",&loc);
        assert_eq!(rows,["<MainMenuStartGame>","<MainMenuLoadGame>","<MainMenuSaveGame>","<MainMenuMiscOptions>","<MainMenuControls>","<MainMenuPerfAndDisplay>","<MainMenuSoundOptions>","<MainMenuQuitGame>","<RemakeBonus>"]);
        assert_eq!(text_key(14),"MenuGoToPrevious");assert_eq!(text_key(0),"");assert_eq!(text_key(99),"");assert_eq!(text_key(54),"MainMenuSoundsOff");
        let mut o=RetailOptions::default();let mut p=Preferences::default();
        assert_eq!(click_row(MAIN,2,&mut o,&mut p,false),Request::None);assert_eq!(click_row(MAIN,2,&mut o,&mut p,true),Request::Save);
        assert_eq!(click_row(MAIN,7,&mut o,&mut p,false),Request::Goto(CONFIRM));assert_eq!(click_row(CONFIRM,0,&mut o,&mut p,false),Request::Quit);assert_eq!(click_row(MAIN,4,&mut o,&mut p,false),Request::Goto(CONTROLS1));assert_eq!(click_row(MAIN,9,&mut o,&mut p,false),Request::None);assert_eq!(click_row(MAIN,8,&mut o,&mut p,false),Request::Goto(BONUS));
        assert!(!o.dirty,"navigation alone does not rewrite the option files");
    }
    #[test] fn the_bonus_row_keeps_every_retail_row_where_it_was_and_the_page_lists_the_bonus_table() {
        let (mut o,mut p)=(RetailOptions::default(),Preferences::default());
        assert_eq!(list_rows(MAIN,&o,&p,"",&loc).len(),MAIN_IDS.len()+1,"one row added below Kilépés: rows 0..7 keep y = 240 + 52 row");
        assert_eq!(list_row_at(Vec2::new(500.0,LIST_Y+LIST_PITCH*8.0+10.0),LIST_PITCH),Some(8));assert!(LIST_Y+LIST_PITCH*8.0+(LIST_PITCH/3.0).trunc()+64.0*LABEL_SCALE<=700.0,"the last label ends inside the panel art");
        assert_eq!(list_rows(BONUS,&o,&p,"",&loc),["<RemakeBonusKiskina>","<MenuGoToPrevious>"]);
        assert_eq!(click_row(BONUS,0,&mut o,&mut p,false),Request::Bonus(0));assert_eq!(click_row(BONUS,1,&mut o,&mut p,false),Request::Goto(MAIN));assert_eq!(click_row(BONUS,2,&mut o,&mut p,false),Request::None);
        assert!(!o.dirty,"starting a bonus level changes no option");assert_eq!(escape_target(BONUS),Some(MAIN));
        assert_eq!((BONUS_ITEMS[0].world,BONUS_ITEMS[0].label),("chinatown","RemakeBonusKiskina"));
        assert_eq!((remake_text("RemakeBonus"),remake_text("RemakeBonusKiskina")),("Bónusz","Kiskína (kiadatlan pálya)"));
        assert_eq!(remake_text(BONUS_ITEMS[0].info),"A gyári telepítésben megmaradt, a kampányból kihagyott pálya (chinatown.dat)");
        let slots=SlotList::default();let details=Default::default();
        let texts=|hover:Option<usize>|{let mut v=view(BONUS,Vec2::ZERO,&o,&p,&slots,&[],&details,&loc);v.hover=hover;scene(&v).into_iter().filter_map(|i|match i {Item::Text {text,pos,rgb,..}=>Some((text,pos,rgb)),_=>None}).collect::<Vec<_>>()};
        let with=texts(Some(0));let without=texts(Some(1));
        assert_eq!(with.len(),without.len(),"the description items exist on every hover so the layer is edited in place");
        assert_eq!(with[0],("<RemakeBonus>".to_owned(),TITLE_POS,RED));
        assert_eq!(with.iter().filter(|t|t.2==WHITE).map(|t|t.0.as_str()).collect::<Vec<_>>(),["<RemakeBonusKiskinaInfo>","",],"white foreground lines: the placeholder text fits one line");
        assert!(without.iter().filter(|t|t.2==WHITE).all(|t|t.0.is_empty()),"no description on the back row");
        assert_eq!(wrap_lines(remake_text("RemakeBonusKiskinaInfo"),BONUS_INFO_CELLS,2),["A gyári telepítésben megmaradt, a kampányból kihagyott pálya","(chinatown.dat)"]);
        assert_eq!(wrap_lines("",10,2),["",""]);assert_eq!(wrap_lines("aa bb cc dd ee",5,2),["aa bb","cc dd"]);
    }
    #[test] fn list_hit_areas_follow_the_retail_rectangles() {
        let p=LIST_PITCH;assert_eq!(list_row_at(Vec2::new(339.0,300.0),p),None);assert_eq!(list_row_at(Vec2::new(340.0,239.0),p),None);
        assert_eq!(list_row_at(Vec2::new(900.0,240.0),p),Some(0));assert_eq!(list_row_at(Vec2::new(500.0,240.0+52.0*7.0+51.0),p),Some(7));assert_eq!(list_row_at(Vec2::new(500.0,240.0+52.0*12.0),p),None);
        assert_eq!(list_hover_at(Vec2::new(600.0,250.0),p),None,"the highlight stops at x 596");assert_eq!(list_hover_at(Vec2::new(500.0,250.0),p),Some(0));assert_eq!(list_hover_at(Vec2::new(500.0,240.0+46.0*9.0+5.0),list_pitch(PERF)),Some(9));
        assert!(inside(BACK_BUTTON,Vec2::new(970.0,670.0)) && !inside(BACK_BUTTON,Vec2::new(1001.0,670.0)));
    }

    fn view<'a>(page:u8,mouse:Vec2,options:&'a RetailOptions,prefs:&'a Preferences,slots:&'a SlotList,labels:&'a [String],details:&'a [String;2],loc:&'a dyn Fn(&str)->String)->PageView<'a> {
        PageView {page,mouse,in_game:false,hover:None,capture:None,scroll_held:None,options,prefs,opened:"",volume:1.0,slots,slot_labels:labels,details,has_thumb:false,loc}
    }
    #[test] fn the_main_page_draws_a_ghost_and_a_label_per_row_and_lights_the_hovered_one() {
        let (o,p)=(RetailOptions::default(),Preferences::default());let slots=SlotList::default();let details=Default::default();
        let mut v=view(MAIN,Vec2::new(500.0,300.0),&o,&p,&slots,&[],&details,&loc);v.hover=Some(1);
        let items=scene(&v);
        assert_eq!(items.iter().filter(|i|matches!(i,Item::Ghost{..})).count(),9);
        let texts:Vec<_>=items.iter().filter_map(|i|match i {Item::Text {text,pos,rgb,..}=>Some((text.as_str(),*pos,*rgb)),_=>None}).collect();
        assert_eq!(texts[0],("<MainMenuHeader>",TITLE_POS,RED));
        assert_eq!(texts[1],("<MainMenuStartGame>",Vec2::new(340.0,240.0+17.0),LABEL_COLOR));
        assert_eq!(texts[2].2,HOVER_COLOR,"the row under the pointer is yellow");assert_eq!(texts[2].1.y,240.0+52.0+17.0);
        assert!(!items.iter().any(|i|matches!(i,Item::Image {path,..} if path.contains("Menu_but"))),"no back ball on the main list outside a game");
        let mut in_game=view(MAIN,Vec2::new(970.0,670.0),&o,&p,&slots,&[],&details,&loc);in_game.in_game=true;
        assert!(scene(&in_game).iter().any(|i|matches!(i,Item::Image {path,..} if path.contains("Menu_but"))),"in a game the ball lights up and resumes");
    }
    #[test] fn the_controls_page_shows_key_names_stars_and_the_green_capture_cell() {
        let (o,p)=(RetailOptions::default(),Preferences::default());let slots=SlotList::default();let details=Default::default();
        let mut v=view(CONTROLS2,Vec2::new(0.0,0.0),&o,&p,&slots,&[],&details,&loc);v.capture=Some((1,1));
        let items=scene(&v);
        let cells:Vec<_>=items.iter().filter_map(|i|match i {Item::Text {text,scale,rgb,pos}=>(*scale==0.42).then_some((text.as_str(),*rgb,*pos)),_=>None}).collect();
        assert_eq!(cells.len(),16,"8 rows x 2 columns");
        assert_eq!(cells[0].0,"<CMDMButton0>","fire is the left button");assert_eq!(cells[1].0,"*****","its second slot is free");
        assert_eq!(cells[3],("<CMDMButton2>",CAPTURE_COLOR,cells[3].2),"the cell waiting for a key is green");
        assert_eq!(cells[1].2.y,220.0,"the key text top-left sits at y = 220 + 48 row");
        assert!(items.iter().any(|i|matches!(i,Item::Text {text,pos,..} if text=="<C2MFire>" && *pos==Vec2::new(320.0,220.0))));
    }
    #[test] fn the_load_page_lists_ten_rows_with_shadows_and_a_scroll_knob() {
        let (o,p)=(RetailOptions::default(),Preferences::default());
        let list=SlotList::new(slots(14),false);let labels:Vec<String>=(0..14).map(|i|format!("slot {i}")).collect();let details=["Mentve: 9-29, 2026 at: 15:18".to_owned(),"Játékos egészsége: 88.00".to_owned()];
        let mut v=view(LOAD,Vec2::new(0.0,0.0),&o,&p,&list,&labels,&details,&loc);v.has_thumb=true;
        let items=scene(&v);
        let rows:Vec<_>=items.iter().filter_map(|i|match i {Item::Text {text,scale,rgb,..} if *scale==SLOT_SCALE && text.starts_with("slot")=>Some((text.clone(),*rgb)),_=>None}).collect();
        assert_eq!(rows.len(),20,"ten rows, each drawn black then coloured");assert_eq!(rows[1],("slot 0".to_owned(),SLOT_SELECTED));assert_eq!(rows[3].1,SLOT_NORMAL);assert_eq!(rows[0].1,BLACK);
        assert!(items.iter().any(|i|matches!(i,Item::Thumb {pos,..} if *pos==THUMB_POS)));
        assert!(items.iter().any(|i|matches!(i,Item::Image {path,pos,..} if path.contains("scroll_slider") && *pos==Vec2::new(928.0,275.0))));
        assert!(items.iter().any(|i|matches!(i,Item::Text {text,..} if text=="Mentve: 9-29, 2026 at: 15:18")));
        assert_eq!(background(SAVE),"ui/menu/save_1024.png");assert_eq!(title_key(LOAD),Some("LGMHeader"));assert_eq!(title_key(PERF),None);
    }
    #[test] fn display_and_sound_toggles_switch_the_retail_texts_and_mark_the_options_dirty() {
        let mut o=RetailOptions::default();let mut p=Preferences::default();
        assert_eq!(list_rows(PERF,&o,&p,"",&loc)[..8],["<MainMenuShotDebrisOn>","<MainMenuIObjAllOn>","<MainMenuInsCharOn>","<MainMenuEnLOn>","<MainMenuSubtitlesOn>","<MainMenuMouseYNormal>","<MainMenuQuickSaveNewLevelYes>","<MainMenuWeaponBobOn>"]);
        for row in 0..8 {click_row(PERF,row,&mut o,&mut p,false);}
        click_row(PERF,1,&mut o,&mut p,false);// the object level cycles 0 -> 1 -> 2
        let rows=list_rows(PERF,&o,&p,"",&loc);
        assert_eq!(rows[..8],["<MainMenuShotDebrisOff>","<MainMenuIObjAllOff>","<MainMenuInsCharOff>","<MainMenuEnLOff>","<MainMenuSubtitlesOff>","<MainMenuMouseYInversed>","<MainMenuQuickSaveNewLevelNo>","<MainMenuWeaponBobOff>"]);
        assert!(o.dirty && !o.keys.shot_debris && o.invert_mouse && !o.weapon_bob && !o.auto_quick_save && o.keys.insignificant_objects==2);
        click_row(PERF,1,&mut o,&mut p,false);assert_eq!(list_rows(PERF,&o,&p,"",&loc)[1],"<MainMenuIObjAllOn>");
        assert_eq!(click_row(PERF,9,&mut o,&mut p,false),Request::Goto(MAIN));assert_eq!(click_row(PERF,10,&mut o,&mut p,false),Request::Goto(VIDEO));
        assert!(!o.retail_choice_mouse && list_rows(PERF,&o,&p,"",&loc)[8]=="<RemakeChoiceMouseOff>","the retail choice mouse is off by default");
        click_row(PERF,8,&mut o,&mut p,false);assert!(o.retail_choice_mouse && list_rows(PERF,&o,&p,"",&loc)[8]=="<RemakeChoiceMouseOn>" && o.dirty);
        assert_eq!((remake_text("RemakeChoiceMouseOn"),remake_text("RemakeChoiceMouseOff")),("Gyári egeres választás be","Gyári egeres választás ki"));
        assert_eq!(list_rows(SOUND,&o,&p,"",&loc)[..4],["<MainMenuMusicOn>","<MainMenuSpeechOn>","<MainMenuSoundsOn>","<MenuGoToPrevious>"]);assert!(list_rows(SOUND,&o,&p,"Speakers",&loc)[4].starts_with("<RemakeOutput> Speakers"),"the remake row shows the opened device");
        click_row(SOUND,0,&mut o,&mut p,false);click_row(SOUND,2,&mut o,&mut p,false);
        assert_eq!(list_rows(SOUND,&o,&p,"",&loc)[..3],["<MainMenuMusicOff>","<MainMenuSpeechOn>","<MainMenuSoundsOff>"]);assert!(!o.music && !o.sound && o.speech);
        assert_eq!(click_row(SOUND,3,&mut o,&mut p,false),Request::Goto(MAIN));
        assert!(o.spatial_audio && list_rows(SOUND,&o,&p,"",&loc)[5]=="<RemakeSpatialOn>","the improved 3D audio switch is on by default");
        click_row(SOUND,5,&mut o,&mut p,false);assert!(!o.spatial_audio && list_rows(SOUND,&o,&p,"",&loc)[5]=="<RemakeSpatialOff>" && o.dirty);
        assert_eq!((remake_text("RemakeSpatialOn"),remake_text("RemakeSpatialOff")),("Javított 3D hangzás be","Javított 3D hangzás ki"));
    }
    #[test] fn video_page_cycles_the_window_options_and_the_field_of_view() {
        let mut o=RetailOptions::default();let mut p=Preferences::default();
        assert_eq!(o.screen,[1280,720]);click_row(VIDEO,1,&mut o,&mut p,false);assert_eq!(o.screen,[1280,1024]);
        o.screen=[2560,1440];click_row(VIDEO,1,&mut o,&mut p,false);assert_eq!(o.screen,[640,480]);
        click_row(VIDEO,0,&mut o,&mut p,false);assert!(!o.windowed);assert_eq!(o.max_fps,0);click_row(VIDEO,2,&mut o,&mut p,false);assert_eq!(o.max_fps,60);click_row(VIDEO,3,&mut o,&mut p,false);assert_eq!(o.show_framerate,1);
        p.fov=111.0;click_row(VIDEO,4,&mut o,&mut p,false);assert_eq!(p.fov,61.0);click_row(VIDEO,4,&mut o,&mut p,false);assert_eq!(p.fov,66.0);
        assert_eq!(list_rows(VIDEO,&o,&p,"",&loc).len(),9);assert!(!o.hd_textures && list_rows(VIDEO,&o,&p,"",&loc)[7]=="<RemakeHdOff>","HD textures are off by default");click_row(VIDEO,7,&mut o,&mut p,false);assert!(o.hd_textures && o.dirty && list_rows(VIDEO,&o,&p,"",&loc)[7]=="<RemakeHdOn>");click_row(VIDEO,7,&mut o,&mut p,false);assert!(!o.hd_textures);assert_eq!((remake_text("RemakeHdOn"),remake_text("RemakeHdOff")),("HD textúrák be (új pályától)","HD textúrák ki"));assert!(p.text_aid);click_row(VIDEO,6,&mut o,&mut p,false);assert!(!p.text_aid && list_rows(VIDEO,&o,&p,"",&loc)[6]=="<RemakeTextAidOff>");assert_eq!(click_row(VIDEO,8,&mut o,&mut p,false),Request::Goto(PERF));
        assert_eq!((escape_target(MAIN),escape_target(CONTROLS2),escape_target(CONTROLS1),escape_target(VIDEO),escape_target(LOAD)),(None,Some(CONTROLS1),Some(MAIN),Some(PERF),Some(MAIN)));
    }
    #[test] fn misc_sliders_use_the_retail_ranges_and_knob_positions() {
        let cfg=KeysCfg::default();
        // Shipped values: sensitivity 1.22, smoothness 0.69, hud colour 7, gamma 1.15.
        assert!((knob_x(0,&cfg,1.0)-(0.72*256.0+570.0)).abs()<0.01);assert!((knob_x(1,&cfg,1.0)-(0.69*256.0+570.0)).abs()<0.01);
        assert_eq!(knob_x(2,&cfg,1.0),7.0*32.0+16.0+570.0);assert!((knob_x(3,&cfg,1.0)-(0.15*256.0+570.0)).abs()<0.01);
        let mut cfg=cfg;let mut volume=1.0;
        assert!(drag_slider(0,570.0+128.0,&mut cfg,&mut volume) && (cfg.mouse_sensitivity-1.0).abs()<1e-5);
        assert!(!drag_slider(0,569.0,&mut cfg,&mut volume) && !drag_slider(0,826.0,&mut cfg,&mut volume));
        drag_slider(2,570.0+255.0,&mut cfg,&mut volume);assert_eq!(cfg.hud_color,7);drag_slider(2,571.0,&mut cfg,&mut volume);assert_eq!(cfg.hud_color,0);drag_slider(2,570.0+100.0,&mut cfg,&mut volume);assert_eq!(cfg.hud_color,3);
        drag_slider(3,570.0+256.0*0.5,&mut cfg,&mut volume);assert!((cfg.gamma-1.5).abs()<1e-5);drag_slider(4,570.0+64.0,&mut cfg,&mut volume);assert!((volume-0.25).abs()<1e-5);
        assert_eq!(slider_at(Vec2::new(600.0,250.0),&SLIDER_Y),Some(0));assert_eq!(slider_at(Vec2::new(600.0,376.0),&SLIDER_Y),Some(2));assert_eq!(slider_at(Vec2::new(600.0,300.0),&SLIDER_Y),None);assert_eq!(slider_at(Vec2::new(500.0,250.0),&SLIDER_Y),None);
    }
    #[test] fn binding_cells_are_190_by_30_around_their_centres() {
        assert_eq!(cell_at(Vec2::new(616.0,224.0),9),Some((0,0)));assert_eq!(cell_at(Vec2::new(808.0,224.0+48.0*8.0),9),Some((8,1)));assert_eq!(cell_at(Vec2::new(808.0,224.0+48.0*8.0),8),None);
        assert_eq!(cell_at(Vec2::new(712.0,224.0),9),None,"between the two columns");assert_eq!(cell_at(Vec2::new(616.0,224.0+16.0),9),None);
        assert_eq!(cell_text_y(2),316.0);assert_eq!(row_label_y(8),620.0);
        assert!(inside(NEXT_BUTTON,Vec2::new(400.0,690.0)) && !inside(NEXT_BUTTON,Vec2::new(400.0,660.0)));
    }
    fn slots(n:usize)->Vec<savefile::Slot> {(0..n).map(|i|savefile::Slot {path:format!("save{i}.sav").into(),kind:savefile::Kind::Numbered(i as u8),header:Default::default()}).collect()}
    #[test] fn the_slot_list_scrolls_selects_and_drags_like_the_retail_pointer_list() {
        let mut list=SlotList::new(slots(25),true);// 26 entries with "new save" first
        assert_eq!((list.len(),list.index()),(26,0));assert!(matches!(list.selected(),Some(None)));
        list.row=2;assert!(matches!(list.selected(),Some(Some(s)) if s.kind==savefile::Kind::Numbered(1)));
        list.scroll(3);assert_eq!((list.top,list.index()),(3,5));list.scroll(-9);assert_eq!(list.top,0);list.scroll(99);assert_eq!(list.top,25);
        list.top=0;assert_eq!(list.row_at(Vec2::new(750.0,281.0)),Some(0));assert_eq!(list.row_at(Vec2::new(750.0,280.0+SLOT_PITCH*9.5)),Some(9));assert_eq!(list.row_at(Vec2::new(723.0,281.0)),None);assert_eq!(list.row_at(Vec2::new(750.0,280.0+SLOT_PITCH*10.5)),None);
        assert_eq!(list.knob_y(),275.0);list.top=13;assert!((list.knob_y()-(275.0+268.0*13.0/26.0)).abs()<1e-3);
        list.drag_to(275.0+134.5);assert_eq!(list.top,13);list.drag_to(9999.0);assert_eq!(list.top,25);list.drag_to(0.0);assert_eq!(list.top,0);
        let short=SlotList::new(slots(3),false);assert_eq!(short.row_at(Vec2::new(750.0,280.0+SLOT_PITCH*3.2)),None);assert_eq!(short.window(),0..3);
        let empty=SlotList::new(Vec::new(),false);assert!(empty.selected().is_none() && empty.row_at(Vec2::new(750.0,281.0)).is_none());
    }
}
