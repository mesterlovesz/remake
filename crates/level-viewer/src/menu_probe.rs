//! Silent opt-in scripts that drive the real menu input: MESTER_TEST_SCENARIO=menu_pages walks every options page with a synthetic
//! pointer, MESTER_TEST_SCENARIO=menu_saves saves and loads through the menu, F5 and F9 in a level. They need a scratch profile
//! (MESTER_USER_DIR=<empty dir>) so the option and save files can be checked without touching the real ones.
use bevy::prelude::*;
use crate::{ViewerConfig,Walking,frontend::Frontend,menu::MenuState,menu_layout::*,options::{Options,RetailOptions},settings::Session,campaign::Campaign,keys_cfg::cmd,savefile};

#[derive(Resource,Default)]
pub struct Probe {pub active:bool,pub finished:bool,pub failure:Option<String>,steps:Vec<Step>,at:usize,wait:f32,scenario:String,mark:f32}
#[derive(Clone)]
enum Step {Wait(f32),Move(Vec2),Down,Up,Press(KeyCode),Health(f32),Shift(f32),Mark,/// Stand in front of the exit door b_door0 of the bonus level and use it (repeated: the first use may miss the aim).
    Exit,Check(&'static str,fn(&Ctx,f32)->bool),Dir(&'static str,fn(&std::path::Path)->bool)}
pub struct Ctx<'a> {/// (text entities the layer should show, text entities that really have their glyphs and are visible).
    drawn:(usize,usize),/// Actors of the loaded level.
    actors:usize,feedback:&'a crate::character::Feedback,menu:&'a MenuState,front:&'a Frontend,options:&'a Options,session:&'a Session,campaign:&'a Campaign,walking:&'a Walking,config:&'a ViewerConfig}

fn click(v:&mut Vec<Step>,x:f32,y:f32) {v.extend([Step::Move(Vec2::new(x,y)),Step::Wait(0.15),Step::Down,Step::Up,Step::Wait(0.2)]);}
fn row(page:u8,i:usize)->(f32,f32) {(500.0,LIST_Y+list_pitch(page)*i as f32+22.0)}
fn click_row(v:&mut Vec<Step>,page:u8,i:usize) {let (x,y)=row(page,i);click(v,x,y);}
fn esc(v:&mut Vec<Step>) {v.extend([Step::Press(KeyCode::Escape),Step::Wait(0.3)]);}
fn near(a:f32,b:f32,tolerance:f32)->bool {(a-b).abs()<=tolerance}

fn pages_script()->Vec<Step> {
    use Step::*;let mut v=vec![Wait(2.5),Check("the main menu is up",|c,_|c.front.main_active && c.front.page==MAIN)];
    click_row(&mut v,MAIN,3);v.push(Check("row 4 opens the misc page",|c,_|c.front.page==MISC));
    v.extend([Move(Vec2::new(600.0,250.0)),Wait(0.1),Down,Wait(0.1),Move(Vec2::new(700.0,250.0)),Wait(0.2),Up,Wait(0.2)]);
    v.push(Check("dragging the first slider sets the mouse sensitivity",|c,_|near(c.options.keys.mouse_sensitivity,(700.0-570.0)/256.0+0.5,0.01)));
    v.extend([Move(Vec2::new(600.0,376.0)),Wait(0.1),Down,Wait(0.1),Move(Vec2::new(570.0+100.0,376.0)),Wait(0.2),Up,Wait(0.2)]);
    v.push(Check("the third slider picks hud colour 3",|c,_|c.options.keys.hud_color==3));
    esc(&mut v);v.push(Check("Esc returns to the main list",|c,_|c.front.page==MAIN));
    click_row(&mut v,MAIN,4);v.push(Check("row 5 opens controls #1",|c,_|c.front.page==CONTROLS1));
    let cell=cell_center(4,1);click(&mut v,cell.x,cell.y);v.push(Check("a click on a cell waits for a key",|c,_|c.menu.capture==Some((4,1))));
    v.extend([Press(KeyCode::KeyJ),Wait(0.2)]);
    v.push(Check("the captured key becomes the second jump binding",|c,_|c.options.keys.bound(cmd::JUMP)==[30,10] && c.menu.capture.is_none()));
    let cell=cell_center(0,0);click(&mut v,cell.x,cell.y);esc(&mut v);
    v.push(Check("Esc cancels a capture and stays on the page",|c,_|c.front.page==CONTROLS1 && c.menu.capture.is_none() && c.options.keys.bound(cmd::FORWARD)==[23,42]));
    let next=NEXT_BUTTON.center();click(&mut v,next.x,next.y);v.push(Check("the arrow opens controls #2",|c,_|c.front.page==CONTROLS2));
    esc(&mut v);v.push(Check("Esc on controls #2 returns to #1",|c,_|c.front.page==CONTROLS1));esc(&mut v);
    click_row(&mut v,MAIN,5);v.push(Check("row 6 opens the display list",|c,_|c.front.page==PERF));
    click_row(&mut v,PERF,5);click_row(&mut v,PERF,4);click_row(&mut v,PERF,0);
    v.push(Check("the toggles change inversion, subtitles and debris",|c,_|c.options.invert_mouse && !c.options.keys.subtitles && !c.options.keys.shot_debris));
    click_row(&mut v,PERF,10);v.push(Check("the remake row opens the video page",|c,_|c.front.page==VIDEO));
    click_row(&mut v,VIDEO,4);v.push(Check("the field of view steps by five",|c,_|near(c.session.preferences.fov,crate::view::FOV_X_DEGREES+5.0,0.1)));
    esc(&mut v);v.push(Check("Esc from the video page returns to the display list",|c,_|c.front.page==PERF));esc(&mut v);
    click_row(&mut v,MAIN,6);v.push(Check("row 7 opens the sound list",|c,_|c.front.page==SOUND));
    click_row(&mut v,SOUND,0);v.push(Check("the music toggle turns music off",|c,_|!c.options.music));
    v.extend([Move(Vec2::new(570.0+10.0,VOLUME_Y+8.0)),Wait(0.1),Down,Wait(0.1),Move(Vec2::new(570.0+64.0,VOLUME_Y+8.0)),Wait(0.2),Up,Wait(0.2)]);
    v.push(Check("the volume slider follows the pointer",|c,_|near(c.session.preferences.volume,0.25,0.02)));
    click_row(&mut v,SOUND,5);v.push(Check("the improved 3D audio row switches it off (on by default)",|c,_|!c.options.spatial_audio));
    esc(&mut v);click_row(&mut v,MAIN,7);v.push(Check("Kilépés asks first",|c,_|c.front.page==CONFIRM));
    click_row(&mut v,CONFIRM,1);v.push(Check("Vissza leaves the confirmation",|c,_|c.front.page==MAIN));
    v.push(Wait(0.5));
    v.push(Dir("the option files were written",|dir|{
        let loaded=RetailOptions::load(dir);
        loaded.from_files && loaded.keys.bound(cmd::JUMP)==[30,10] && loaded.keys.hud_color==3 && loaded.invert_mouse && !loaded.music && !loaded.spatial_audio && !loaded.keys.subtitles && near(loaded.keys.mouse_sensitivity,1.0078,0.01)
    }));
    v
}
/// The retail death flow: the shell prints GameShell4 and F9 loads the quick save the level start wrote.
fn death_script()->Vec<Step> {
    use Step::*;
    vec![Wait(6.5),Check("the level start quick-saved by itself",|c,_|c.config.persist && c.front.in_game),
        Dir("save/quick.sav exists",|dir|savefile::read(&dir.join("save").join("quick.sav"),true).is_ok_and(|f|f.thumbnail.is_some())),
        Health(0.0),Wait(1.5),
        Check("the shell says GameShell4",|c,_|c.feedback.message.starts_with("Meghalt") && c.campaign.dead()),
        Press(KeyCode::F9),Wait(9.0),
        Check("F9 brings the player back alive",|c,_|!c.campaign.dead() && c.campaign.health>40.0)]
}
fn saves_script()->Vec<Step> {
    use Step::*;let mut v=vec![Wait(7.0),Check("a level runs",|c,_|c.front.in_game && !c.front.main_active && c.campaign.mission.is_some())];
    v.push(Press(KeyCode::F5));v.push(Wait(1.5));
    v.push(Dir("F5 wrote save/quick.sav",|dir|savefile::read(&dir.join("save").join("quick.sav"),true).is_ok_and(|f|f.thumbnail.is_some() && f.payload["player"].is_object() && !f.header.world.is_empty())));
    esc(&mut v);v.push(Wait(0.3));v.push(Check("Esc opens the menu",|c,_|c.front.main_active && c.front.page==MAIN));
    click_row(&mut v,MAIN,2);v.push(Check("the save row opens the save page",|c,_|c.front.page==SAVE && c.menu.slots.with_new));
    let action=ACTION_BUTTON.center();click(&mut v,action.x,action.y);v.push(Wait(0.5));
    v.push(Check("the new-save entry closes the menu",|c,_|!c.front.main_active));
    v.push(Dir("the menu wrote save/save0.sav",|dir|savefile::read(&dir.join("save").join("save0.sav"),true).is_ok_and(|f|f.thumbnail.is_some() && f.header.health>0.0)));
    v.extend([Mark,Health(37.0),Shift(900.0),Wait(0.3),Press(KeyCode::F9),Wait(9.0)]);
    v.push(Check("F9 restores health and position",|c,mark|c.campaign.health>40.0 && near(c.walking.player.position.x,mark,60.0)));
    esc(&mut v);click_row(&mut v,MAIN,1);v.push(Check("the load row opens the load page",|c,_|c.front.page==LOAD && c.menu.slots.len()>=2));
    v.extend([Health(15.0)]);
    let action=ACTION_BUTTON.center();click(&mut v,action.x,action.y);v.push(Wait(9.0));
    v.push(Check("loading the slot restores the saved state",|c,_|c.campaign.health>40.0 && !c.front.main_active));
    v
}
/// Opens the sound list and stays there (captures of the page: `MESTER_TEST_SCENARIO=menu_sound`).
/// The owner-requested bonus page: main menu -> Bónusz -> Kiskína, F5 / death / F9 stay in the bonus slot, the exit door returns to the main menu.
fn bonus_script()->Vec<Step> {
    use Step::*;
    let mut v=vec![Wait(2.5),Check("the main menu draws the retail rows and the bonus row below them",|c,_|c.front.main_active && c.front.page==MAIN && c.drawn.0==19 && c.drawn.0==c.drawn.1)];
    v.extend([Move(Vec2::new(row(MAIN,7).0,row(MAIN,7).1)),Wait(0.3),Check("Kilépés keeps its retail row",|c,_|c.menu.hover==Some(7)),Move(Vec2::new(row(MAIN,8).0,row(MAIN,8).1)),Wait(0.4),Check("the pointer lights the bonus row",|c,_|c.menu.hover==Some(8) && c.drawn.0==c.drawn.1)]);
    click_row(&mut v,MAIN,8);v.push(Check("row 9 opens the bonus page",|c,_|c.front.page==BONUS));
    esc(&mut v);v.push(Check("Esc returns to the main list",|c,_|c.front.page==MAIN));
    click_row(&mut v,MAIN,8);click_row(&mut v,BONUS,1);v.push(Check("the back row returns to the main list",|c,_|c.front.page==MAIN));
    click_row(&mut v,MAIN,8);v.extend([Move(Vec2::new(row(BONUS,0).0,row(BONUS,0).1)),Wait(1.0),Check("the first entry is lit with its description drawn",|c,_|c.menu.hover==Some(0) && c.drawn.0==c.drawn.1 && c.drawn.0>=7)]);
    click_row(&mut v,BONUS,0);v.push(Wait(0.5));v.push(Check("Kiskína is loading as a fresh bonus game",|c,_|c.front.bonus.as_deref()==Some("chinatown") && c.front.loading.is_some() && c.campaign.health>=100.0));
    v.push(Wait(11.0));
    v.push(Check("Kiskína runs: world, 17 actors, the player at the StartPoint",|c,_|c.config.world=="chinatown" && c.front.in_game && !c.front.main_active && c.front.loading.is_none() && c.campaign.mission.is_some() && c.actors==17 && near(c.walking.player.position.x,7520.0,80.0) && near(c.walking.player.position.z,-1136.0,80.0)));
    v.extend([Press(KeyCode::F5),Wait(1.5)]);
    v.push(Dir("F5 and the level start write save/bonus.sav of chinatown, never quick.sav",|dir|savefile::read(&dir.join("save").join("bonus.sav"),true).is_ok_and(|f|f.header.world=="chinatown" && f.thumbnail.is_some()) && !dir.join("save").join("quick.sav").exists()));
    esc(&mut v);v.push(Wait(0.5));v.push(Check("Esc opens the menu",|c,_|c.front.main_active && c.front.page==MAIN));
    click_row(&mut v,MAIN,2);v.push(Check("the save row does nothing in a bonus run",|c,_|c.front.page==MAIN));
    esc(&mut v);v.push(Check("Esc resumes the bonus level",|c,_|!c.front.main_active && c.front.bonus.is_some()));
    v.extend([Health(0.0),Wait(1.5),Check("death: the shell says GameShell4",|c,_|c.feedback.message.starts_with("Meghalt") && c.campaign.dead()),Press(KeyCode::F9),Wait(9.0),
        Check("F9 loads bonus.sav: alive, same level, still a bonus run",|c,_|!c.campaign.dead() && c.campaign.health>40.0 && c.config.world=="chinatown" && c.front.bonus.is_some() && c.actors==17)]);
    v.extend([Wait(1.0),Exit,Wait(0.4),Exit,Wait(0.4),Exit,Wait(0.4),Exit,Wait(6.0)]);
    v.push(Check("the exit door leads back to the main menu, not to A Templom",|c,_|c.front.main_active && c.front.page==MAIN && !c.front.in_game && c.front.bonus.is_none() && c.front.loading.is_none() && c.config.world!="chinatown" && c.config.world!="chinatown2"));
    v.push(Dir("the campaign quick.sav was never touched",|dir|!dir.join("save").join("quick.sav").exists()));
    v.push(Wait(1.0));
    v
}
fn sound_script()->Vec<Step> {
    use Step::*;let mut v=vec![Wait(2.5)];
    click_row(&mut v,MAIN,6);v.push(Check("row 7 opens the sound list",|c,_|c.front.page==SOUND));v.push(Wait(30.0));
    v
}
/// The label entities of the menu layer: how many the scene wants (ghost copy + label per row, title...) and how many are really drawn (glyph children built, visible).
fn drawn_texts(menu:&MenuState,layer:&crate::menu::MenuLayer,texts:&Query<(&ChildOf,&crate::retail_ui::BitmapText,Option<&Children>,&InheritedVisibility)>)->(usize,usize) {
    let wanted=menu.shown().iter().filter(|i|matches!(i,Item::Text {text,..}|Item::Ghost {text,..} if !text.trim().is_empty())).count();
    let drawn=texts.iter().filter(|(parent,text,children,visible)|parent.parent()==layer.0 && !text.text.trim().is_empty() && children.is_some_and(|c|!c.is_empty()) && visible.get()).count();
    (wanted,drawn)
}
/// Regression of the owner's "the buttons disappear when I move the mouse": the pointer sweeps over the rows frame by frame, leaves the list, comes back;
/// after every move the rows must still be drawn. Capture times: 2.4 (before the move), 6 (after).
fn hover_script()->Vec<Step> {
    use Step::*;let mut v=vec![Wait(2.5),Check("the main menu draws its rows",|c,_|c.front.main_active && c.drawn.0>=16 && c.drawn.0==c.drawn.1)];
    // one move per frame: down the whole list, out to the art, back in on another row
    // the frame right after each move is checked: every text entity must still carry its glyphs (a rebuilt layer is empty for one frame)
    for i in 0..8 {let (x,y)=row(MAIN,i);v.extend([Move(Vec2::new(x,y)),Check("no blank frame after the pointer changed rows",|c,_|c.drawn.0>=16 && c.drawn.0==c.drawn.1)]);}
    for x in (500..900).step_by(40) {v.extend([Move(Vec2::new(x as f32,300.0)),Wait(0.0)]);}
    v.extend([Move(Vec2::new(10.0,10.0)),Wait(0.0),Move(row(MAIN,2).into()),Wait(0.5)]);
    v.push(Check("the rows are still drawn after the pointer sweep",|c,_|c.menu.hover==Some(2) && c.drawn.0>=16 && c.drawn.0==c.drawn.1));
    v.extend([Move(Vec2::new(800.0,700.0)),Wait(0.5),Check("and with the pointer off the list",|c,_|c.menu.hover.is_none() && c.drawn.0>=16 && c.drawn.0==c.drawn.1),Wait(2.0)]);
    v
}
/// Probe helpers bundled to stay under Bevy's system parameter limit.
#[derive(bevy::ecs::system::SystemParam)] pub struct Extra<'w,'s> {feedback:Res<'w,crate::character::Feedback>,layer:Res<'w,crate::menu::MenuLayer>,roster:Res<'w,crate::npcs::NpcRoster>,doors:Query<'w,'s,&'static crate::doors::Door>,door_use:ResMut<'w,crate::doors::DoorUse>}
pub fn setup(mut commands:Commands,config:Res<ViewerConfig>) {
    let scenario=std::env::var("MESTER_TEST_SCENARIO").unwrap_or_default();
    let steps=match scenario.as_str() {"menu_pages"=>pages_script(),"menu_saves"=>saves_script(),"menu_death"=>death_script(),"menu_sound"=>sound_script(),"menu_hover"=>hover_script(),"bonus"=>bonus_script(),_=>Vec::new()};
    commands.insert_resource(Probe {active:config.capture.is_some() && !steps.is_empty(),steps,scenario,..default()});
}
pub fn tick(mut probe:ResMut<Probe>,time:Res<Time>,mut menu:ResMut<MenuState>,mut buttons:ResMut<ButtonInput<MouseButton>>,mut keys:ResMut<ButtonInput<KeyCode>>,front:Res<Frontend>,options:Res<Options>,session:Res<Session>,mut campaign:ResMut<Campaign>,mut walking:ResMut<Walking>,config:Res<ViewerConfig>,extra:Extra,mut window:Single<&mut Window>,scale:Res<UiScale>,texts:Query<(&ChildOf,&crate::retail_ui::BitmapText,Option<&Children>,&InheritedVisibility)>) {
    let Extra {feedback,layer,roster,doors,mut door_use}=extra;
    if !probe.active || probe.finished {return;}
    if probe.wait>0.0 {probe.wait-=time.delta_secs();return;}
    let Some(step)=probe.steps.get(probe.at).cloned() else {probe.finished=true;info!("MENÜPRÓBA {}: kész, hiba={:?}",probe.scenario,probe.failure);return};
    probe.at+=1;
    match step {
        Step::Wait(seconds)=>probe.wait=seconds,
        Step::Move(pos)=>{
            // Like a real pointer: the window reports its cursor position (the design-space `fake` only stands in when it does not, `menu::input`).
            if std::env::var_os("MESTER_PROBE_REAL_POINTER").is_some() {let (size,s,f)=(Vec2::new(window.width(),window.height()),scale.0,window.scale_factor());window.set_physical_cursor_position(Some(((pos*s+(size-Vec2::new(1024.0,768.0)*s)*0.5)*f).as_dvec2()));}
            else {menu.fake=Some(pos);}
        },
        Step::Down=>buttons.press(MouseButton::Left),Step::Up=>buttons.release(MouseButton::Left),
        Step::Press(key)=>{keys.press(key);keys.release(key);},
        Step::Health(value)=>campaign.health=value,
        Step::Shift(units)=>walking.player.position.x+=units,
        Step::Mark=>probe.mark=walking.player.position.x,
        Step::Exit=>if front.bonus.is_some() && config.world=="chinatown" {
            let attempt=probe.at;let target=doors.iter().find(|d|d.name=="b_door0").map(|d|d.center());
            if target.is_some_and(|target|crate::campaign_probe::place_with_clear_shot_at(&mut walking,target,&[60.0,90.0,130.0,180.0,240.0],130.0,None,attempt)) {door_use.request_name=Some("b_door0".into());}else{probe.failure.get_or_insert("no standing spot with a clear line to b_door0".to_owned());}
        },
        Step::Check(name,test)=>{let ctx=Ctx {actors:roster.actors.len(),drawn:drawn_texts(&menu,&layer,&texts),feedback:&feedback,menu:&menu,front:&front,options:&options,session:&session,campaign:&campaign,walking:&walking,config:&config};let ok=test(&ctx,probe.mark);info!("MENÜPRÓBA {}: {name}: {}",probe.scenario,if ok {"rendben"}else{"HIBA"});if !ok {probe.failure.get_or_insert(name.to_owned());}},
        Step::Dir(name,test)=>{let ok=test(&config.user);info!("MENÜPRÓBA {}: {name}: {}",probe.scenario,if ok {"rendben"}else{"HIBA"});if !ok {probe.failure.get_or_insert(name.to_owned());}},
    }
}
