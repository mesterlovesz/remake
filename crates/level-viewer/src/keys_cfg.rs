//! Retail `scripts\keys.cfg` and the `AddAction` numbers of autoexec.cfg (evidence: docs/retail-menus.md).
//! The writer is cshell.dll 0x100114f0, the reader 0x100116f0: three header lines, 256 command slots with two bindings each (one
//! number per line: the `AddAction` id of the bound key or mouse button, 0 = free), then hud colour index, mouse sensitivity, mouse
//! smoothness, shot debris, insignificant objects, insignificant characters, enemy lights, gamma and subtitles.
use bevy::prelude::{ButtonInput,KeyCode,MouseButton};

pub const SLOTS:usize=256;
/// Command slots of the retail input table used by the menus and the game (0x10025bf0 / 0x10025e90 rows, 0x1001100b defaults).
pub mod cmd {
    pub const FORWARD:usize=1;pub const BACKWARD:usize=2;pub const STEP_LEFT:usize=3;pub const STEP_RIGHT:usize=4;pub const JUMP:usize=5;
    pub const FIRE:usize=6;pub const ALT_FIRE:usize=7;pub const CROUCH:usize=8;pub const RUN:usize=9;pub const TOGGLE_RUN:usize=10;
    /// Weapon slots 1..8 (`Weapon1`..`Weapon8`), cmd 11..=18.
    pub const WEAPON1:usize=11;
    pub const INVENTORY_PANEL:usize=21;pub const CHARACTER_PANEL:usize=22;pub const ACTION:usize=27;pub const HOLSTER:usize=29;
    pub const RELOAD:usize=30;pub const PREVIOUS_WEAPON:usize=31;pub const NEXT_WEAPON:usize=32;pub const PLAYER_PANEL:usize=33;
    pub const MENU:usize=70;pub const QUIT:usize=250;
}
/// Row order of the "Irányítás #1" page (C1MForward..C1MAction) and "Irányítás #2" page (C2MFire..C2MPlayer): command slots.
pub const CONTROLS1_ROWS:[usize;9]=[cmd::FORWARD,cmd::BACKWARD,cmd::STEP_LEFT,cmd::STEP_RIGHT,cmd::JUMP,cmd::CROUCH,cmd::RUN,cmd::TOGGLE_RUN,cmd::ACTION];
pub const CONTROLS2_ROWS:[usize;8]=[cmd::FIRE,cmd::ALT_FIRE,cmd::RELOAD,cmd::NEXT_WEAPON,cmd::PREVIOUS_WEAPON,cmd::INVENTORY_PANEL,cmd::CHARACTER_PANEL,cmd::PLAYER_PANEL];
/// The eight hud colours of the misc page slider (0x1001ac1f..0x1001acd8), 0..255 rgb; the retail default is index 7 (amber).
pub const HUD_COLORS:[[f32;3];8]=[[175.0,19.0,19.0],[201.0,78.0,78.0],[131.0,51.0,121.0],[63.0,119.0,194.0],[91.0,205.0,204.0],[37.0,133.0,76.0],[232.0,227.0,73.0],[208.0,115.0,16.0]];

/// Everything that lives in keys.cfg. Ranges follow what the menu sliders can produce (0x10028e00).
#[derive(Clone,Debug,PartialEq)]
pub struct KeysCfg {
    pub keys:Vec<[u8;2]>,pub hud_color:i32,pub mouse_sensitivity:f32,pub mouse_smoothness:f32,pub shot_debris:bool,pub insignificant_objects:i32,
    pub insignificant_characters:bool,pub enemy_lights:bool,pub gamma:f32,pub subtitles:bool,
}
/// The shipped scripts\keys.cfg (also cshell's own initial table 0x10011386 with E added as the second action key).
const DEFAULT_KEYS:[(usize,u8,u8);29]=[(1,23,42),(2,19,43),(3,1,44),(4,4,45),(5,85,0),(6,84,0),(7,86,29),(8,67,68),(9,27,28),(10,17,0),(11,71,0),(12,72,0),(13,73,0),(14,74,0),(15,75,0),(16,76,0),(17,77,0),(18,78,0),
    (21,3,0),(22,26,0),(23,83,0),(27,30,5),(29,8,0),(30,18,0),(31,6,0),(32,7,0),(33,24,0),(70,70,0),(250,250,0)];
impl KeysCfg {
    /// The shipped scripts\keys.cfg exactly (also cshell's own initial table 0x10011386 with E added as the second action key).
    pub fn retail()->Self {
        let mut keys=vec![[0u8;2];SLOTS];
        for (slot,a,b) in DEFAULT_KEYS {keys[slot]=[a,b];}
        // Values of the shipped file's tail.
        Self {keys,hud_color:7,mouse_sensitivity:1.22,mouse_smoothness:0.69,shot_debris:true,insignificant_objects:0,insignificant_characters:true,enemy_lights:true,gamma:1.15,subtitles:true}
    }
}
impl Default for KeysCfg {
    /// The owner's deliberate deviation from the shipped table (the ONE binding table of the remake): Space jumps (retail: right mouse
    /// button), the right mouse button is the alternate fire / scope next to the middle button (retail: middle button + Alt), and E is
    /// the only use / pick up / open key (retail: Space + E).
    fn default()->Self {
        let mut config=Self::retail();
        config.keys[cmd::JUMP]=[30,0];config.keys[cmd::ALT_FIRE]=[85,86];config.keys[cmd::ACTION]=[5,0];config
    }
}
fn atoi(text:&str)->i64 {
    let text=text.trim();let (negative,digits)=match text.strip_prefix('-') {Some(rest)=>(true,rest),None=>(false,text.strip_prefix('+').unwrap_or(text))};
    let end=digits.find(|c:char|!c.is_ascii_digit()).unwrap_or(digits.len());
    let value=digits[..end].parse::<i64>().unwrap_or(0);if negative {-value}else{value}
}
fn atof(text:&str)->f32 {
    let text=text.trim();let mut end=0;let mut seen_dot=false;
    for (i,c) in text.char_indices() {if c.is_ascii_digit() || (i==0 && (c=='-' || c=='+')) {end=i+1;}else if c=='.' && !seen_dot {seen_dot=true;end=i+1;}else{break;}}
    text[..end].parse::<f32>().unwrap_or(0.0)
}
fn float(value:f32)->String {let text=format!("{value:.2}");let text=text.trim_end_matches('0').trim_end_matches('.').to_owned();if text.is_empty() || text=="-0" {"0".into()}else{text}}
impl KeysCfg {
    /// Reads the file like 0x100116f0: the values are taken one line at a time and a short file keeps the defaults of the rest.
    pub fn parse(text:&str)->Self {
        let mut config=Self::default();
        let mut lines=text.lines().skip(3).map(str::trim).filter(|line|!line.is_empty());
        for slot in 0..SLOTS {for column in 0..2 {match lines.next() {Some(line)=>config.keys[slot][column]=atoi(line).clamp(0,255) as u8,None=>return config.sanitized()}}}
        macro_rules! next {($convert:expr)=>{match lines.next() {Some(line)=>$convert(line),None=>return config.sanitized()}}}
        config.hud_color=next!(|l|atoi(l) as i32);config.mouse_sensitivity=next!(atof);config.mouse_smoothness=next!(atof);config.shot_debris=next!(|l|atoi(l)!=0);
        config.insignificant_objects=next!(|l|atoi(l) as i32);config.insignificant_characters=next!(|l|atoi(l)!=0);config.enemy_lights=next!(|l|atoi(l)!=0);
        config.gamma=next!(atof);config.subtitles=next!(|l|atoi(l)!=0);
        config.sanitized()
    }
    /// Menu slider ranges: hud colour 0..7, sensitivity 0.5..1.5, smoothness 0..1, gamma 1..2, insignificant objects 0..2.
    pub fn sanitized(mut self)->Self {
        self.hud_color=self.hud_color.clamp(0,7);self.mouse_sensitivity=if self.mouse_sensitivity.is_finite() {self.mouse_sensitivity.clamp(0.5,1.5)}else{1.0};
        self.mouse_smoothness=if self.mouse_smoothness.is_finite() {self.mouse_smoothness.clamp(0.0,1.0)}else{0.2};self.gamma=if self.gamma.is_finite() {self.gamma.clamp(1.0,2.0)}else{1.0};
        self.insignificant_objects=self.insignificant_objects.clamp(0,2);self
    }
    /// The file text exactly as 0x100114f0 writes it (CRLF, one value per line).
    pub fn write(&self)->String {
        let mut out=String::from("Generated by RatHunt.\r\nDo NOT modify!\r\n \r\n");
        for pair in &self.keys {for action in pair {out.push_str(&format!("{action}\r\n"));}}
        out.push_str(&format!("{}\r\n{}\r\n{}\r\n{}\r\n{}\r\n{}\r\n{}\r\n{}\r\n{}\r\n",self.hud_color,float(self.mouse_sensitivity),float(self.mouse_smoothness),self.shot_debris as u8,self.insignificant_objects,self.insignificant_characters as u8,self.enemy_lights as u8,float(self.gamma),self.subtitles as u8));
        out
    }
    /// The menu's assignment (0x10028a46): an action key belongs to one command only, so it is removed from every other slot first.
    pub fn assign(&mut self,command:usize,column:usize,action:u8) {
        if command>=SLOTS || column>1 || action==0 {return;}
        for pair in &mut self.keys {for key in pair.iter_mut() {if *key==action {*key=0;}}}
        self.keys[command][column]=action;
    }
    pub fn bound(&self,command:usize)->[u8;2] {self.keys.get(command).copied().unwrap_or([0,0])}
    /// True while any key or mouse button bound to `command` is held.
    pub fn pressed(&self,command:usize,keys:&ButtonInput<KeyCode>,mouse:&ButtonInput<MouseButton>)->bool {self.bound(command).iter().any(|a|action_pressed(*a,keys,mouse,|k,c|k.pressed(c),|m,b|m.pressed(b)))}
    /// `pressed` with some physical keys ignored (the arrow keys belong to the dialogue list while it is up, though keys.cfg also walks with them).
    pub fn pressed_without(&self,command:usize,keys:&ButtonInput<KeyCode>,mouse:&ButtonInput<MouseButton>,skip:&[KeyCode])->bool {
        self.bound(command).iter().any(|a|*a!=0 && (keys_of_action(*a).iter().any(|c|!skip.contains(c) && keys.pressed(*c)) || mouse_of_action(*a).is_some_and(|b|mouse.pressed(b))))
    }
    pub fn just_pressed(&self,command:usize,keys:&ButtonInput<KeyCode>,mouse:&ButtonInput<MouseButton>)->bool {self.bound(command).iter().any(|a|action_pressed(*a,keys,mouse,|k,c|k.just_pressed(c),|m,b|m.just_pressed(b)))}
}
fn action_pressed(action:u8,keys:&ButtonInput<KeyCode>,mouse:&ButtonInput<MouseButton>,key:impl Fn(&ButtonInput<KeyCode>,KeyCode)->bool,button:impl Fn(&ButtonInput<MouseButton>,MouseButton)->bool)->bool {
    if action==0 {return false;}
    keys_of_action(action).iter().any(|c|key(keys,*c)) || mouse_of_action(action).is_some_and(|b|button(mouse,b))
}

/// Physical keys behind an `AddAction` id. The autoexec `rangebind` lines bind both control keys to LeftControl (67), so
/// RightControl (68) has no physical key; Weapon1..8 are the digit keys 1..8, F1..F4 / F5 / F9 / F10 / Esc are 80..83 / 251 / 252 / 250 / 70.
pub fn keys_of_action(action:u8)->Vec<KeyCode> {
    use KeyCode::*;
    const LETTERS:[KeyCode;26]=[KeyA,KeyB,KeyC,KeyD,KeyE,KeyF,KeyG,KeyH,KeyI,KeyJ,KeyK,KeyL,KeyM,KeyN,KeyO,KeyP,KeyQ,KeyR,KeyS,KeyT,KeyU,KeyV,KeyW,KeyX,KeyY,KeyZ];
    const NUMPAD:[KeyCode;10]=[Numpad0,Numpad1,Numpad2,Numpad3,Numpad4,Numpad5,Numpad6,Numpad7,Numpad8,Numpad9];
    const DIGITS:[KeyCode;8]=[Digit1,Digit2,Digit3,Digit4,Digit5,Digit6,Digit7,Digit8];
    match action {
        1..=26=>vec![LETTERS[action as usize-1]],27=>vec![ShiftLeft],28=>vec![ShiftRight],29=>vec![AltLeft],30=>vec![Space],31=>vec![Enter],
        32..=41=>vec![NUMPAD[action as usize-32]],42=>vec![ArrowUp],43=>vec![ArrowDown],44=>vec![ArrowLeft],45=>vec![ArrowRight],46=>vec![Delete],47=>vec![Insert],48=>vec![End],49=>vec![Home],
        50=>vec![PageDown],51=>vec![PageUp],52=>vec![NumpadDecimal],53=>vec![NumpadEnter],54=>vec![NumpadAdd],55=>vec![NumpadSubtract],56=>vec![Backspace],57=>vec![Comma],58=>vec![Period],59=>vec![Slash],
        60=>vec![Semicolon],61=>vec![Quote],62=>vec![BracketLeft],63=>vec![BracketRight],64=>vec![Backslash],65=>vec![Minus],66=>vec![Equal],67=>vec![ControlLeft,ControlRight],
        70=>vec![Escape],71..=78=>vec![DIGITS[action as usize-71]],80=>vec![F1],81=>vec![F2],82=>vec![F3],83=>vec![F4],250=>vec![F10],251=>vec![F5],252=>vec![F9],
        _=>Vec::new(),
    }
}
pub fn mouse_of_action(action:u8)->Option<MouseButton> {match action {84=>Some(MouseButton::Left),85=>Some(MouseButton::Right),86=>Some(MouseButton::Middle),_=>None}}
/// The action a physical key produces, if the retail autoexec binds it.
pub fn action_of_key(code:KeyCode)->Option<u8> {(1u8..=252).find(|a|keys_of_action(*a).contains(&code))}
/// The action captured by the rebinding cell: keys 1..69 (Esc, weapon digits and F-keys are excluded like 0x10028a0d) and the three mouse buttons.
pub fn captured_action(keys:&ButtonInput<KeyCode>,mouse:&ButtonInput<MouseButton>)->Option<u8> {
    keys.get_just_pressed().filter_map(|c|action_of_key(*c)).find(|a|(1..70).contains(a))
        .or_else(||[MouseButton::Left,MouseButton::Right,MouseButton::Middle].into_iter().find(|b|mouse.just_pressed(*b)).and_then(|b|(84u8..=86).find(|a|mouse_of_action(*a)==Some(b))))
}
/// Text key of the key-name table (0x10011015..): `CMDA`.. for 1..=68 and `CMDMButton0..2` for 84..86; other ids have no name.
pub fn action_text_key(action:u8)->Option<String> {
    const NAMES:[&str;67]=["A","B","C","D","E","F","G","H","I","J","K","L","M","N","O","P","Q","R","S","T","U","V","W","X","Y","Z","LeftShift","RightShift","Alt","Space","Enter",
        "Num0","Num1","Num2","Num3","Num4","Num5","Num6","Num7","Num8","Num9","UpArrow","DownArrow","LeftArrow","RightArrow","Delete","Insert","End","Home","PgDown","PgUp","NumDel","NumEnter","NumPlus","NumMinus",
        "BackSpace","Comma","Dot","Slash","Semicolon","Apostrophe","SqBracketLeft","SqBracketRight","BackSlash","Minus","Equals","LeftControl"];
    match action {1..=67=>Some(format!("CMD{}",NAMES[action as usize-1])),68=>Some("CMDRightControl".into()),84..=86=>Some(format!("CMDMButton{}",action-84)),_=>None}
}

#[cfg(test)] mod tests {
    use super::*;
    fn retail_file()->Option<String> {
        let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../GYARI/scripts/keys.cfg");
        std::fs::read(path).ok().map(|bytes|bytes.iter().map(|b|*b as char).collect())
    }
    #[test] fn the_shipped_keys_cfg_round_trips_byte_for_byte() {
        let Some(text)=retail_file() else {return};
        let config=KeysCfg::parse(&text);
        assert_eq!(config,KeysCfg::retail(),"the built-in retail table is the shipped file");
        assert_eq!(config.write(),text,"the writer reproduces the retail file");
        assert_eq!(config.bound(cmd::FORWARD),[23,42]);assert_eq!(config.bound(cmd::ACTION),[30,5]);assert_eq!(config.bound(cmd::JUMP),[85,0]);
        assert_eq!((config.hud_color,config.mouse_sensitivity,config.mouse_smoothness,config.gamma,config.insignificant_objects),(7,1.22,0.69,1.15,0));
    }
    #[test] fn a_short_or_damaged_file_keeps_the_defaults_and_stays_in_range() {
        let short=KeysCfg::parse("Generated by RatHunt.\nDo NOT modify!\n \n0\n0\n23\n");
        assert_eq!(short.bound(1),[23,42]);assert_eq!(short.bound(2),[19,43]);
        let wild=KeysCfg {gamma:7.0,mouse_sensitivity:f32::NAN,hud_color:99,insignificant_objects:-3,..KeysCfg::default()}.sanitized();
        assert_eq!((wild.gamma,wild.mouse_sensitivity,wild.hud_color,wild.insignificant_objects),(2.0,1.0,7,0));
        assert_eq!(KeysCfg::parse(""),KeysCfg::default());
        assert_eq!(atoi(" 12abc"),12);assert_eq!(atof("1.22x"),1.22);assert_eq!(float(1.0),"1");assert_eq!(float(0.69),"0.69");assert_eq!(float(1.2),"1.2");
    }
    #[test] fn rebinding_moves_a_key_to_one_command_like_the_retail_menu() {
        let mut config=KeysCfg::retail();
        config.assign(cmd::STEP_LEFT,1,23);// W was Forward's primary
        assert_eq!(config.bound(cmd::FORWARD),[0,42]);assert_eq!(config.bound(cmd::STEP_LEFT),[1,23]);
        config.assign(cmd::JUMP,0,30);// Space was Action's primary
        assert_eq!(config.bound(cmd::ACTION),[0,5]);assert_eq!(config.bound(cmd::JUMP)[0],30);
        let text=config.write();assert_eq!(KeysCfg::parse(&text),config);
    }
    #[test] fn action_ids_match_the_autoexec_add_action_table() {
        let Ok(text)=std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../GYARI/autoexec.cfg")) else {return};
        let table:std::collections::BTreeMap<String,i32>=text.lines().filter_map(|l|{let mut p=l.strip_prefix("AddAction ")?.split_whitespace();Some((p.next()?.to_owned(),p.next()?.parse().ok()?))}).collect();
        assert_eq!(table["W"],23);assert_eq!(table["MButton1"],85);assert_eq!(table["QuickSave"],251);assert_eq!(table["Menu"],70);
        // Every id with a physical key round-trips, and the key names exist for 1..=68.
        for a in (1u8..=83).chain(250..=252) {for code in keys_of_action(a) {assert!(action_of_key(code).is_some(),"{a}");}}
        assert_eq!(action_of_key(KeyCode::KeyE),Some(5));assert_eq!(action_of_key(KeyCode::ControlRight),Some(67));assert_eq!(action_of_key(KeyCode::F5),Some(251));
        assert_eq!(action_text_key(27).as_deref(),Some("CMDLeftShift"));assert_eq!(action_text_key(68).as_deref(),Some("CMDRightControl"));assert_eq!(action_text_key(86).as_deref(),Some("CMDMButton2"));assert_eq!(action_text_key(70),None);
    }
    #[test] fn only_keys_below_seventy_and_the_mouse_buttons_can_be_captured() {
        let mut keys=ButtonInput::<KeyCode>::default();let mut mouse=ButtonInput::<MouseButton>::default();
        keys.press(KeyCode::Escape);keys.press(KeyCode::Digit3);keys.press(KeyCode::F5);assert_eq!(captured_action(&keys,&mouse),None);
        keys.press(KeyCode::KeyG);assert_eq!(captured_action(&keys,&mouse),Some(7));
        keys.clear();keys.release_all();mouse.press(MouseButton::Middle);assert_eq!(captured_action(&keys,&mouse),Some(86));
    }
}
