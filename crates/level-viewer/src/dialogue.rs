//! The retail conversation panel, mouse selection and speech (cshell.dll dialogue object 0x10072398; evidence, addresses and the
//! verified-versus-approximated table in docs/retail-dialogue.md). The script side (nodes, timing, conditions) is mission-runtime.
//!
//! Screen (render 0x10019010, everything times `s = width / 1024`, text = Mincho `table` font cells 32x64 drawn at 0.3 s):
//! the question at (16 s, 32 s) in (173,237,221); once the list is up the answers at (32 s, (32 i + 72) s), the highlighted one
//! in (253,246,198), the others (99,113,160); after taking one the player's own line "Dominick: ..." at (16 s, 80 s) in (253,246,192);
//! every line has a black copy 3 s to the lower right. No panel, no portrait.
use bevy::{prelude::*,input::mouse::{AccumulatedMouseMotion,AccumulatedMouseScroll,MouseScrollUnit},window::CursorGrabMode};
use std::path::Path;
use crate::{ViewerConfig,InspectionCamera,SCALE,settings::Session,campaign::{Campaign,Speech},npcs::NpcRoster,retail_ui::{aided_label,Aid,BitmapText,RetailUi,wrap},character::Feedback};

/// Glyph size of the dialogue text (`0x100151c0(s * 0.3)` at 0x10019085).
pub const FONT_SCALE:f32=0.3;
/// Shadow offset in units of `s` (0x10019071).
pub const SHADOW:f32=3.0;
/// Positional speech radius when the node has a speaker (0x10019852: 4096.0); without one the sound is 2D (0x10019420).
pub const SPEECH_RADIUS:f32=4096.0;
pub const TITLE_COLOR:[u8;3]=[173,237,221];
pub const SELECTED_COLOR:[u8;3]=[253,246,198];
pub const OTHER_COLOR:[u8;3]=[99,113,160];
pub const ANSWERED_COLOR:[u8;3]=[253,246,192];
/// Line pitch of a wrapped line in glyph heights and the gap kept between two blocks, in units of `s`. Retail cuts a line at the screen edge (three
/// nodes have lines wider than 1024); the remake wraps it and pushes the rows below down only as far as needed, so the retail positions stay
/// whenever the text fits.
pub const WRAP_PITCH:f32=0.9;
pub const ROW_GAP:f32=6.0;

/// Top-left of the question, the answer `i` (0-based) and the chosen-answer line, in units of `s`.
pub fn title_at()->Vec2 {Vec2::new(16.0,32.0)}
pub fn answer_at(i:usize)->Vec2 {Vec2::new(32.0,32.0*i as f32+72.0)}
pub fn answered_at()->Vec2 {Vec2::new(16.0,80.0)}

/// Mouse selection (cshell 0x10054d4d): the accumulator moves by `sensitivity * dy * 8` where `dy` is the engine axis, i.e. counts x 0.006625;
/// it is clamped to 0 ..= count-1 (no wrap). The highlighted slot is its integer part (0x10018b13).
pub fn scroll(acc:f32,sensitivity:f32,counts_y:f32,count:usize)->f32 {
    if count==0 {return 0.0;}
    (acc+sensitivity*counts_y*crate::view::MOUSE_RADIANS*8.0).clamp(0.0,(count-1) as f32)
}
pub fn selected(acc:f32,count:usize)->usize {(acc.max(0.0) as usize).min(count.saturating_sub(1))}
/// Remake addition (owner request): the mouse wheel and the Up / Down arrow keys move the highlight `delta` answers (negative = up), clamped like the mouse
/// accumulator (no wrap); the accumulator lands on the integer slot so retail-mode mouse motion continues from there.
pub fn step(acc:f32,delta:i32,count:usize)->f32 {
    if count==0 {return 0.0;}
    (selected(acc,count) as i32+delta).clamp(0,count as i32-1) as f32
}
/// Whether mouse MOTION scrolls the list: only with "Gyári egeres választás" (retail 0x10054d4d) while a list with answers is up; by default it only looks around.
pub fn motion_selects(retail:bool,ready:bool,count:usize)->bool {retail && ready && count>0}
/// Arrow keys as a step.
pub fn key_step(acc:f32,up:bool,down:bool,count:usize)->f32 {step(acc,down as i32-up as i32,count)}
/// Wheel notches of one frame as a signed step: wheel up (positive y) moves the highlight up. A notch is one line; pixel-unit wheels (touchpads) count 40 px per step.
pub fn wheel_steps(y:f32,unit:MouseScrollUnit)->i32 {
    if y==0.0 || !y.is_finite() {return 0;}
    let notches=match unit {MouseScrollUnit::Line=>y.abs(),MouseScrollUnit::Pixel=>y.abs()/40.0}.round().max(1.0) as i32;
    -notches*y.signum() as i32
}
/// The keys that confirm the highlighted answer besides the left button.
pub const CONFIRM_KEYS:[KeyCode;2]=[KeyCode::Enter,KeyCode::NumpadEnter];
/// The one-line controls hint of the first choice list of a game (owner request); with the retail mouse the retail sentence (IngameText1) is used instead.
pub const CONTROLS_HINT:&str="Görgő vagy fel/le nyíl: választás, Enter vagy kattintás: megerősítés";
/// Whether this list prints the hint: only the first of a game (`shown` = saved flag or the options mirror), and only with the retail "subtitles" option (`[0x100b247d]`).
pub fn hint_due(shown:bool,subtitles:bool)->bool {!shown && subtitles}

/// Length of a PCM/ADPCM WAV from its header (data bytes / byte rate); StartDialog replaces the node's `delay` by it (0x1001987a).
pub fn wav_seconds(path:&Path)->Option<f32> {
    use std::io::Read;
    let mut head=Vec::new();std::fs::File::open(path).ok()?.take(4096).read_to_end(&mut head).ok()?;
    if head.get(..4)!=Some(b"RIFF") || head.get(8..12)!=Some(b"WAVE") {return None;}
    let (mut at,mut rate)=(12,0u32);
    while at+8<=head.len() {
        let size=u32::from_le_bytes(head[at+4..at+8].try_into().ok()?);
        match &head[at..at+4] {
            b"fmt " => rate=u32::from_le_bytes(head.get(at+16..at+20)?.try_into().ok()?),
            b"data" => return (rate>0).then(||size as f32/rate as f32),
            _=>{}
        }
        at+=8+size as usize+(size as usize&1);
    }
    None
}

#[derive(Resource)]
pub struct Dialogue {
    /// Lower edge of the panel text in UI pixels (0 while nothing is drawn); the HUD message (the hint) moves below it.
    pub bottom:f32,
    /// Selection accumulator (`0x100725cc` / `+0x234`), zeroed whenever the list appears.
    pub acc:f32,
    seen:u32,shown:bool,
    /// The character whose head plays the dialogue clips (the node's speaker).
    speaker:Option<String>,
    /// Question shadow + text, four answers, the chosen-answer line (shadow + text each).
    lines:[[Entity;2];6],
}
impl Dialogue {
    pub fn slot(&self,count:usize)->usize {selected(self.acc,count)}
}
impl Default for Dialogue {fn default()->Self {Self {bottom:0.0,acc:0.0,seen:0,shown:false,speaker:None,lines:[[Entity::PLACEHOLDER;2];6]}}}

fn rgb(c:[u8;3])->Color {Color::srgb_u8(c[0],c[1],c[2])}

pub fn setup(mut commands:Commands) {
    let mut dialogue=Dialogue::default();
    for row in dialogue.lines.iter_mut() {
        for (n,entity) in row.iter_mut().enumerate() {
            // The shadow copy carries the faint strip (under everything, z 25); the text on top has the outline (z 26).
            *entity=commands.spawn((aided_label("mincho","",FONT_SCALE,if n==0 {Color::BLACK}else{Color::WHITE},0.0,0.0,if n==0 {Aid::STRIP}else{Aid::OUTLINE}),GlobalZIndex(25+n as i32),Visibility::Inherited)).id();
        }
    }
    commands.insert_resource(dialogue);
}

/// Mouse wheel of the choice list and the left button (engine command 0x54, edge). Runs before `campaign::tick`, which drains the events.
pub fn input(mut dialogue:ResMut<Dialogue>,mut campaign:ResMut<Campaign>,mut session:ResMut<Session>,opening:Res<crate::opening::Opening>,panels:Res<crate::panels::Panels>,
    mouse:Res<AccumulatedMouseMotion>,wheel:Res<AccumulatedMouseScroll>,buttons:Res<ButtonInput<MouseButton>>,keys:Res<ButtonInput<KeyCode>>,bind:crate::options::Bindings,cursor:Single<&bevy::window::CursorOptions>,probes:Res<crate::campaign_probe::Probe>) {
    session.dialogue_list=false;
    if session.paused || opening.active || campaign.dead() || campaign.dialogue.is_none() {return;}
    let count=campaign.dialogue.as_ref().map_or(0,|d|d.choices.len());
    let ready=campaign.mission.as_ref().is_some_and(|m|m.choices_ready());
    // Default: mouse motion only looks around; "Gyári egeres választás" gives it the retail job of scrolling the list (0x10054d4d).
    if motion_selects(bind.options.retail_choice_mouse,ready,count) && (cursor.grab_mode!=CursorGrabMode::None || probes.active) && !panels.mouse_mode() {
        dialogue.acc=scroll(dialogue.acc,bind.options.keys.mouse_sensitivity,mouse.delta.y,count);
    }
    // Wheel and arrow keys (remake addition): the highlight steps; Enter confirms like the left button. While the list is up the arrows do not walk (main.rs).
    let confirm=ready && count>0 && !session.paused && !panels.mouse_mode() && CONFIRM_KEYS.iter().any(|k|keys.just_pressed(*k));
    if ready && count>0 && !panels.mouse_mode() {
        session.dialogue_list=true;
        let (up,down)=(keys.just_pressed(KeyCode::ArrowUp),keys.just_pressed(KeyCode::ArrowDown));
        if up || down {dialogue.acc=key_step(dialogue.acc,up,down,count);}
        let notches=wheel_steps(wheel.delta.y,wheel.unit);
        if notches!=0 {dialogue.acc=step(dialogue.acc,notches,count);}
    }
    if confirm {
        let slot=dialogue.slot(count);
        if let Some(mission)=&mut campaign.mission {let events=mission.click(slot);campaign.queue(events);session.suppress_fire=2;}
    }
    // The click that opens the inventory / takes the mouse back is not a dialogue click.
    if buttons.just_pressed(MouseButton::Left) && session.suppress_fire==0 && !panels.mouse_mode() && (cursor.grab_mode!=CursorGrabMode::None || probes.active) {
        let slot=dialogue.slot(count);
        if let Some(mission)=&mut campaign.mission {let events=mission.click(slot);campaign.queue(events);session.suppress_fire=2;}
    }
}

fn speech_target<'a>(campaign:&Campaign,roster:&'a NpcRoster)->Option<&'a crate::npcs::Npc> {
    let (id,person)=campaign.mission.as_ref()?.speaker()?;
    id.and_then(|id|roster.actors.iter().find(|a|a.name==id))
        .or_else(||person.and_then(|p|roster.actors.iter().find(|a|a.definition_name==p)))
}

/// Starts the sound of a node or answer: positional at the speaker (radius 4096) or plain 2D, like 0x10019350 / 0x10019420.
fn speak(commands:&mut Commands,assets:&AssetServer,path:&str,at:Option<Vec3>) {
    let (path,assets)=(path.to_owned(),assets.clone());
    commands.queue(move |world:&mut World| {
        let asset=crate::audio::asset_path(&path);
        let distance=at.and_then(|position|world.query_filtered::<&Transform,With<InspectionCamera>>().iter(world).next().map(|t|(t.translation/SCALE).distance(position)));
        let level=distance.map_or(1.0,|distance|crate::sound::level(crate::spatial::enabled(world),distance,SPEECH_RADIUS));
        crate::audio::note_cue(world,&asset,at.map(|_|SPEECH_RADIUS),distance,level>0.0);
        if level<=0.0 {return;}
        if crate::audio::logging() {info!("AUDIO {} r={} {asset}",if at.is_some() {"near"}else{"2d"},SPEECH_RADIUS);return;}
        if world.get_resource::<Session>().is_none_or(|s|!s.spawns_audio()) {return;}
        if world.get_resource::<ViewerConfig>().is_some_and(|c|!c.output.join(&asset).is_file()) {return;}
        let handle=assets.load::<bevy::audio::AudioSource>(asset);
        let mut sound=world.spawn((Speech,crate::WorldGeometry,AudioPlayer::new(handle),PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(level))));
        if let Some(position)=at {sound.insert((crate::sound::Positional {position,radius:SPEECH_RADIUS},crate::sound::Gain(level)));}
    });
}

/// Panel text, speech, the hint and the mode flags. Runs after `campaign::tick`.
pub fn present(mut dialogue:ResMut<Dialogue>,mut campaign:ResMut<Campaign>,mut session:ResMut<Session>,mut roster:ResMut<NpcRoster>,config:Res<ViewerConfig>,assets:Res<AssetServer>,mut commands:Commands,
    opening:Res<crate::opening::Opening>,window:Single<&Window>,scale:Res<UiScale>,ui:Res<RetailUi>,mut feedback:ResMut<Feedback>,mut options:ResMut<crate::options::Options>,
    speech:Query<Entity,With<Speech>>,mut texts:Query<(&mut BitmapText,&mut Node)>,view:Res<crate::view::ViewState>) {
    let view_now=campaign.dialogue.clone().filter(|_|!opening.active && !campaign.dead());
    let target=speech_target(&campaign,&roster).map(|npc|(npc.position,npc.hp>0.0));
    let speaker=speech_target(&campaign,&roster).map(|npc|npc.name.clone());
    // A node (or an answer) began: play its sound; the sound's length replaces the node delay (0x1001987a).
    if campaign.dialogue_count!=dialogue.seen {
        dialogue.seen=campaign.dialogue_count;dialogue.shown=false;dialogue.acc=0.0;
        // StartDialog (0x10019b39): the speaker's head talks (`gada`, looping from 0) until the answer list appears.
        if let Some(name)=&speaker {roster.set_head_clip(name,"gada");}
        dialogue.speaker=speaker.clone();
        if let Some(path)=campaign.dialogue.as_ref().and_then(|d|d.speech.clone()) {
            let file=config.output.join(crate::audio::asset_path(&path));
            if file.is_file() {
                if let (Some(seconds),Some(mission))=(wav_seconds(&file),campaign.mission.as_mut()) {mission.set_speech_length(seconds);}
                // A dead speaker's sound is stopped in the next tick (0x10018b7f); it never starts here.
                // The question is a 3D sound at the speaker (0x1001985b), the chosen answer the local 2D sound (0x10018d65..0x10018e91 call 0x10019420).
                let answering=campaign.dialogue.as_ref().is_some_and(|d|d.answered.is_some());
                if target.is_none_or(|(_,alive)|alive) {speak(&mut commands,&assets,&path,target.filter(|_|!answering).map(|(p,_)|p));}
            }
        }
    }
    // The speech ends with a dead speaker.
    if target.is_some_and(|(_,alive)|!alive) {for entity in &speech {commands.entity(entity).despawn();}}
    let ready=view_now.is_some() && campaign.mission.as_ref().is_some_and(|m|m.choices_ready());
    // A new game clears the options mirror of the one-time hint flag.
    if campaign.hint_reset {campaign.hint_reset=false;if options.dialogue_hint_seen {options.dialogue_hint_seen=false;options.dirty=true;}}
    if ready && !dialogue.shown {
        dialogue.acc=0.0;
        // The list appears (0x10018c60): the speaker's head goes back to `mruga`.
        if let Some(name)=&dialogue.speaker {roster.set_head_clip(name,"mruga");}
        // The first choice list of the game prints the controls hint once (HUD message; the next node end clears it); afterwards never again.
        if hint_due(campaign.hint_shown || options.dialogue_hint_seen,options.keys.subtitles) {
            feedback.message=if options.retail_choice_mouse {ui.text("IngameText1")}else{CONTROLS_HINT.to_owned()};feedback.message_time=1.0e6;
        }
        if !campaign.hint_shown || !options.dialogue_hint_seen {campaign.hint_shown=true;options.dialogue_hint_seen=true;options.dirty=true;}
    }
    dialogue.shown=ready;
    // EndDialog (0x1001950f): the head blinks again (restarted at 0).
    if view_now.is_none() {if let Some(name)=dialogue.speaker.take() {roster.set_head_clip(&name,"mruga");}}
    // Mode flag of the retail intent (`[0x100b22f8]` && count): a node with answers (or its answer stage) owns the fire button and the pitch.
    session.dialogue_choices=view_now.as_ref().is_some_and(|d|!d.choices.is_empty() || d.answered.is_some());

    // Layout in retail pixels (`s = width / 1024`); UI units are `UiScale` pixels.
    let s=window.width()/1024.0;let k=s/scale.0.max(0.01);
    let mut plan:Vec<(usize,String,Vec2,[u8;3])>=Vec::new();
    // The first 60 frames of a level draw nothing (0x10019039).
    if let Some(d)=&view_now {if view.level_frames()>=60 {
        let font=ui.fonts.get("mincho");
        // Wrapped text: the glyph advance is 32 * 0.3 = 9.6 s; lines end 8 s before the right edge.
        let fit=|text:&str,x:f32|->String {
            let room=(1024.0-x-8.0)/(32.0*FONT_SCALE);let text=text.to_owned();
            if font.is_none() || text.chars().count() as f32<=room {return text;}
            wrap(&text,room.max(8.0) as usize).join("\n")
        };
        let height=|text:&str|->f32 {let n=text.split('\n').count() as f32;64.0*FONT_SCALE*(1.0+(n-1.0)*WRAP_PITCH)};
        let title=fit(&d.title,title_at().x);let mut floor=title_at().y+height(&title)+ROW_GAP;
        plan.push((0,title,title_at(),TITLE_COLOR));
        if ready {
            let slot=dialogue.slot(d.choices.len());
            let mut y=answer_at(0).y.max(floor);
            for (i,c) in d.choices.iter().enumerate() {
                let text=fit(&c.text,answer_at(i).x);let h=height(&text);
                let at=Vec2::new(answer_at(i).x,y);y=(y+32.0).max(y+h+ROW_GAP);floor=at.y+h+ROW_GAP;
                plan.push((1+i,text,at,if i==slot {SELECTED_COLOR}else{OTHER_COLOR}));
            }
        }
        if let Some(text)=&d.answered {
            let text=fit(&format!("{} {text}",ui.text("IngameText4").trim_end()),answered_at().x);
            let at=Vec2::new(answered_at().x,answered_at().y.max(if ready {floor}else{title_at().y+height(&plan[0].1)+ROW_GAP}));floor=at.y+height(&text)+ROW_GAP;
            plan.push((5,text,at,ANSWERED_COLOR));
        }
        dialogue.bottom=(floor-ROW_GAP)*k;
    }else{dialogue.bottom=0.0;}}else{dialogue.bottom=0.0;}
    for (row,entities) in dialogue.lines.iter().enumerate() {
        let entry=plan.iter().find(|p|p.0==row);
        for (n,entity) in entities.iter().enumerate() {
            let Ok((mut text,mut node))=texts.get_mut(*entity) else {continue};
            match entry {
                Some((_,string,at,color))=>{
                    let shift=if n==0 {SHADOW}else{0.0};
                    text.set(string.clone());text.color=if n==0 {Color::BLACK}else{rgb(*color)};text.scale=FONT_SCALE*k;text.pitch=WRAP_PITCH;
                    node.left=px((at.x+shift)*k);node.top=px((at.y+shift)*k);
                },
                None=>text.set(""),
            }
        }
    }
}

/// Silent capture aid: `MESTER_TEST_SCENARIO=dialogue MESTER_TEST_DIALOG=<node> MESTER_TEST_PERSON=<character definition>` stands the player 110 units
/// in front of that character at `MESTER_TEST_START` (default 1) s and starts the node; `MESTER_TEST_SCRIPT="6:scroll=1.4,9:click,14:click"` then moves the highlight or clicks
/// at those times (seconds of run time); `key=up|down|enter` presses the arrow keys / Enter, `wheel=up|down` turns the wheel one notch. It logs what the panel would show.
pub fn probe(mut stage:Local<usize>,mut done:Local<Vec<usize>>,mut pressed:Local<Vec<KeyCode>>,mut keys:ResMut<ButtonInput<KeyCode>>,mut wheel:ResMut<AccumulatedMouseScroll>,time:Res<Time>,config:Res<ViewerConfig>,mut campaign:ResMut<Campaign>,mut walking:ResMut<crate::Walking>,roster:Res<NpcRoster>,mut dialogue:ResMut<Dialogue>) {
    if config.capture.is_none() || std::env::var("MESTER_TEST_SCENARIO").as_deref()!=Ok("dialogue") {return;}
    for key in pressed.drain(..) {keys.release(key);}
    let now=time.elapsed_secs();
    let start=std::env::var("MESTER_TEST_START").ok().and_then(|v|v.parse::<f32>().ok()).unwrap_or(1.0);
    if *stage==0 && now>=start {
        let person=std::env::var("MESTER_TEST_PERSON").unwrap_or_default();
        if let Some(actor)=roster.actors.iter().find(|a|a.definition_name==person) {
            let target=actor.eye();crate::campaign_probe::place(&mut walking,actor.position+actor.rotation*Vec3::Z*110.0,target);walking.pitch=0.0;
        }
        let id=std::env::var("MESTER_TEST_DIALOG").unwrap_or_default();
        if let Some(mission)=&mut campaign.mission {let events=mission.trigger_dialog(&id);campaign.queue(events);}
        info!("DIALOGUE PROBE started {id} with {person}");*stage=1;
    }
    let script=std::env::var("MESTER_TEST_SCRIPT").unwrap_or_default();
    for (n,step) in script.split(',').enumerate() {
        let Some((at,action))=step.split_once(':') else {continue};
        if at.parse::<f32>().map_or(true,|at|now<at) || done.contains(&n) {continue;}
        done.push(n);
        let count=campaign.dialogue.as_ref().map_or(0,|d|d.choices.len());
        if let Some(value)=action.strip_prefix("scroll=") {dialogue.acc=value.parse().unwrap_or(0.0);}
        else if let Some(name)=action.strip_prefix("key=") {
            // Injected like a real key press: `dialogue::input` (next system in the chain) sees it as just pressed this frame.
            let key=match name {"up"=>KeyCode::ArrowUp,"down"=>KeyCode::ArrowDown,_=>KeyCode::Enter};keys.press(key);pressed.push(key);
        }
        else if let Some(direction)=action.strip_prefix("wheel=") {wheel.unit=MouseScrollUnit::Line;wheel.delta=Vec2::new(0.0,if direction=="up" {1.0}else{-1.0});}
        else if action=="click" {let slot=dialogue.slot(count);if let Some(mission)=&mut campaign.mission {let events=mission.click(slot);campaign.queue(events);}}
        info!("DIALOGUE PROBE t={now:.1} {action}: dialogue={:?} ready={:?} acc={}",campaign.dialogue.as_ref().map(|d|(d.id.clone(),d.answered.clone())),campaign.mission.as_ref().map(|m|m.choices_ready()),dialogue.acc);
    }
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn arrow_keys_step_the_highlight_clamped_and_reset_the_fraction() {
        assert_eq!(key_step(0.0,false,true,3),1.0);assert_eq!(key_step(1.0,false,true,3),2.0);
        assert_eq!(key_step(2.0,false,true,3),2.0,"no wrap at the bottom");
        assert_eq!(key_step(0.0,true,false,3),0.0,"no wrap at the top");
        assert_eq!(key_step(1.7,true,false,4),0.0,"the mouse fraction snaps to its slot first");
        assert_eq!(key_step(1.7,false,true,4),2.0);assert_eq!(key_step(0.0,true,true,4),0.0,"both keys cancel");
        assert_eq!(key_step(5.0,false,false,0),0.0);
        // the mouse continues from the key's slot
        assert_eq!(selected(scroll(key_step(0.0,false,true,3),1.22,1.0,3),3),1);
    }
    #[test] fn the_wheel_steps_one_answer_per_notch_clamped_without_wrap() {
        use MouseScrollUnit::{Line,Pixel};
        assert_eq!((wheel_steps(1.0,Line),wheel_steps(-1.0,Line),wheel_steps(0.0,Line)),(-1,1,0),"wheel up = highlight up");
        assert_eq!(wheel_steps(3.0,Line),-3,"several notches in a frame");assert_eq!(wheel_steps(-12.0,Pixel),1,"a touchpad nudge is at least one step");
        assert_eq!(wheel_steps(f32::NAN,Line),0);
        let mut acc=0.0;
        acc=step(acc,1,3);assert_eq!(acc,1.0);acc=step(acc,1,3);acc=step(acc,1,3);assert_eq!(acc,2.0,"clamped at the last answer");
        acc=step(acc,-1,3);assert_eq!(acc,1.0);acc=step(acc,-5,3);assert_eq!(acc,0.0,"clamped at the first answer, no wrap");
        assert_eq!(step(1.6,1,4),2.0,"a retail-mode mouse fraction snaps to its slot first");assert_eq!(step(3.0,1,0),0.0);
    }
    #[test] fn mouse_motion_selects_only_with_the_retail_mouse() {
        assert!(!motion_selects(false,true,3),"default: motion never changes the highlight");
        assert!(motion_selects(true,true,3));assert!(!motion_selects(true,false,3) && !motion_selects(true,true,0));
    }
    #[test] fn the_hint_is_due_only_until_the_first_list_of_the_game_and_with_the_subtitles_option() {
        assert!(hint_due(false,true));assert!(!hint_due(true,true),"afterwards never again");assert!(!hint_due(false,false));
        assert!(CONTROLS_HINT.contains("Görgő") && CONTROLS_HINT.contains("nyíl") && CONTROLS_HINT.contains("Enter") && CONTROLS_HINT.chars().count()<75,"one short line naming wheel, arrows, Enter");
    }
    #[test] fn enter_and_the_numpad_enter_confirm() {assert!(CONFIRM_KEYS.contains(&KeyCode::Enter) && CONFIRM_KEYS.contains(&KeyCode::NumpadEnter));}
    #[test] fn wav_length_comes_from_the_data_chunk_and_byte_rate() {
        let mut wav=Vec::new();wav.extend(b"RIFF\0\0\0\0WAVEfmt ");wav.extend(16u32.to_le_bytes());wav.extend([1,0,1,0]);wav.extend(22050u32.to_le_bytes());
        wav.extend(44100u32.to_le_bytes());wav.extend([2,0,16,0]);wav.extend(b"data");wav.extend(88200u32.to_le_bytes());wav.extend(vec![0u8;100]);
        let path=std::env::temp_dir().join(format!("mester-wav-{}.wav",std::process::id()));std::fs::write(&path,&wav).unwrap();
        assert!((wav_seconds(&path).unwrap()-2.0).abs()<1e-4);std::fs::remove_file(path).unwrap();
    }
    #[test] fn the_list_scrolls_by_eight_axis_units_per_row_without_wrapping() {
        // sensitivity 1.22 (the shipped keys.cfg): 8 * 0.006625 * 1.22 = 0.0647 rows per mouse count, about 15.5 counts per row.
        let one_row=1.0/(1.22*crate::view::MOUSE_RADIANS*8.0);
        assert!((one_row-15.46).abs()<0.05,"{one_row}");
        let mut acc=0.0;
        for _ in 0..16 {acc=scroll(acc,1.22,1.0,3);}
        assert_eq!(selected(acc,3),1);
        for _ in 0..500 {acc=scroll(acc,1.22,1.0,3);}
        assert_eq!((acc,selected(acc,3)),(2.0,2),"clamped to count-1, no wrap");
        for _ in 0..1000 {acc=scroll(acc,1.22,-1.0,3);}
        assert_eq!((acc,selected(acc,3)),(0.0,0),"clamped at the top");
        assert_eq!(selected(7.9,4),3);
    }
    #[test] fn the_panel_rows_sit_where_the_render_code_puts_them() {
        // 0x10019010: title (16 s, 32 s); answer i at x = 32 s, y = ((32 (i+1) + 8) s + 32 s); the chosen line at y = (32 + 16) s + 32 s.
        assert_eq!(title_at(),Vec2::new(16.0,32.0));
        assert_eq!((answer_at(0),answer_at(3)),(Vec2::new(32.0,72.0),Vec2::new(32.0,168.0)));
        assert_eq!(answered_at(),Vec2::new(16.0,80.0));
    }
}
