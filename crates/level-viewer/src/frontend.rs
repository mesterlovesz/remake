//! Retail menu artwork and loading presentation over the existing game controls.
use bevy::prelude::*;
use serde_json::Value;
use std::collections::BTreeMap;
use crate::{ViewerConfig,settings::Session,retail_ui::{RetailUi,BitmapText,label}};

#[derive(Clone,Copy)]
pub struct CharacterDraft {pub points:u8,pub stamina:u8,pub health:u8,pub difficulty:u8,pub skills:[u8;8]}
impl Default for CharacterDraft {fn default()->Self {Self {points:30,stamina:100,health:100,difficulty:1,skills:[0;8]}}}
impl CharacterDraft {
    fn change_stamina(&mut self,delta:i8) {if delta>0 && self.points>0 {self.points-=1;self.stamina+=1;}else if delta<0 && self.stamina>100 {self.stamina-=1;self.points+=1;}}
    /// Weapon skills take one point each up to 99 (cshell.dll 0x10024ccf); the screen shows them as `x.00`.
    fn change_skill(&mut self,index:usize,delta:i8) {
        let Some(skill)=self.skills.get_mut(index) else {return};
        if delta>0 && self.points>0 && *skill<99 {self.points-=1;*skill+=1;}else if delta<0 && *skill>0 {*skill-=1;self.points+=1;}
    }
    fn change_health(&mut self,delta:i8) {if delta>0 && self.points>0 {self.points-=1;self.health+=1;}else if delta<0 && self.health>100 {self.health-=1;self.points+=1;}}
}

#[derive(Resource,Default)]
pub struct Frontend {
    pub main_active:bool,pub return_to_menu:bool,pub loading:Option<String>,/// A level runs behind the menu: the save row works and Esc / the back ball resume it.
    pub in_game:bool,/// Bonus run (docs/retail-menus.md, owner-requested): the world id started from the bonus page. Level exits return to the main menu, saving goes to `saveonus.sav`.
    pub bonus:Option<String>,
    pub page:u8,pub draft:CharacterDraft,pub character_started:bool,
    pub load_started:bool,frames:u32,elapsed:f32,/// Menu clock of cshell.dll 0x1002ae45: seconds, wrapped at 2*pi; drives the ghost labels.
    pub clock:f32,
    pub probe_active:bool,pub probe_finished:bool,pub probe_failure:Option<String>,probe_stage:u8,probe_start:Option<Vec3>,
    data:Value,images:BTreeMap<String,Handle<Image>>,
}
impl Frontend {
    pub fn new(active:bool)->Self {Self{main_active:active,in_game:!active,..default()}}
    pub fn begin_load(&mut self,world:&str) {self.main_active=false;self.in_game=true;self.page=0;self.return_to_menu=false;self.loading=Some(world.into());self.load_started=false;self.frames=0;self.elapsed=0.0;}
    pub fn screen_ready(&self,assets:&AssetServer)->bool {
        self.frames>=3 && self.loading.as_ref().and_then(|w|self.data["loading"][w]["image"].as_str()).and_then(|p|self.images.get(p)).is_none_or(|h|assets.is_loaded_with_dependencies(h.id()))
    }
}
#[derive(Component)] pub(crate) struct MainScreen;
#[derive(Component)] pub(crate) struct LoadingScreen;
#[derive(Component)] pub(crate) struct LoadingPicture;
#[derive(Component)] pub(crate) struct LoadingTitle;
#[derive(Component)] pub(crate) struct LoadingStatus;
#[derive(Component)] pub(crate) struct Notice;
#[derive(Component)] pub(crate) struct MenuCanvas;
#[derive(Component)] pub(crate) struct ScreenPart(u8);
#[derive(Component)] pub(crate) struct PageButton {page:u8,id:&'static str,index:u8}
/// A number on the new-player screen, drawn with the Mincho glyphs (0x100264a2..0x10026656).
#[derive(Component)] pub(crate) enum CharValue {Points,Stamina,Health,Skill(usize)}
/// The bright chevron over the selected difficulty slot (0x10026c41).
#[derive(Component)] pub(crate) struct DifficultyIcon(u8);
/// Ghost pose for menu clock `t`: x drifts by 0.1 label widths, y by 0.125 label heights (twice as fast); the label
/// itself sits pitch/3 below its row while the ghost sits height/3 below it. Pixel values are truncated like ftol.
pub(crate) fn ghost_pose(x:f32,row_y:f32,size:Vec2,t:f32)->Vec2 {
    let (w,h)=(size.x.trunc(),size.y.trunc());
    Vec2::new((x+0.1*w*t.sin()).trunc(),(row_y+h/3.0+0.125*h*(2.0*t).sin()).trunc())
}

fn load_images(value:&Value,assets:&AssetServer,images:&mut BTreeMap<String,Handle<Image>>) {
    match value {
        Value::String(s) if s.ends_with(".png")=>{images.entry(s.clone()).or_insert_with(||assets.load(s.clone()));},
        Value::Array(a)=>for v in a {load_images(v,assets,images)},
        Value::Object(o)=>for v in o.values() {load_images(v,assets,images)},_=>{}
    }
}
fn size(value:&Value,key:&str,default:f32)->f32 {value[key].as_f64().unwrap_or(default as f64) as f32}

pub fn setup(mut commands:Commands,config:Res<ViewerConfig>,assets:Res<AssetServer>,mut front:ResMut<Frontend>,mut session:ResMut<Session>,ui:Res<RetailUi>) {
    front.data=std::fs::read_to_string(config.output.join("retail_ui.json")).ok().and_then(|s|serde_json::from_str(&s).ok()).unwrap_or_default();
    let data=front.data.clone();load_images(&data,&assets,&mut front.images);
    front.probe_active=config.capture.is_some() && std::env::var("MESTER_TEST_SCENARIO").as_deref()==Ok("menu");
    if front.main_active {session.paused=true;}
    // Debug/capture aid: MESTER_FRONT_PAGE=1 opens that main-menu page (1 = new player).
    if let Some(page)=std::env::var("MESTER_FRONT_PAGE").ok().and_then(|p|p.parse::<u8>().ok()) {front.page=page;}
    let font=assets.load("hud/subtitles.ttf");
    commands.spawn((MainScreen,Visibility::Hidden,GlobalZIndex(200),Node {position_type:PositionType::Absolute,width:percent(100),height:percent(100),justify_content:JustifyContent::Center,align_items:AlignItems::Center,..default()},BackgroundColor(Color::BLACK)))
        .with_children(|root| {
            root.spawn((MenuCanvas,ImageNode::new(assets.load(data["main_background"].as_str().unwrap_or("ui/menu/main_menu_1024.png").to_string())),Node {width:px(1024),height:px(768),flex_shrink:0.0,..default()})).with_children(|canvas| {
                let title=&data["menu_title"];
                // The header is red (255,0,0) at half the glyph-cell size (0x10024305); the labels are 0.35 of the cell.
                if let Some(path)=title["red_image"].as_str() {canvas.spawn((ScreenPart(0),Visibility::Inherited,ImageNode::new(assets.load(path.to_string())),Node {position_type:PositionType::Absolute,left:px(size(title,"x",598.0)),top:px(size(title,"y",92.0)),width:px(96),height:px(32),..default()}));}
                else if let Some(path)=title["image"].as_str() {canvas.spawn((ScreenPart(0),Visibility::Inherited,ImageNode::new(assets.load(path.to_string())),Node {position_type:PositionType::Absolute,left:px(size(title,"x",598.0)),top:px(size(title,"y",92.0)),width:px(size(title,"width",160.0)*0.7),height:px(size(title,"height",32.0)*0.7),..default()}));}
                canvas.spawn((ScreenPart(0),Visibility::Inherited,Notice,Text::new(""),TextFont {font:font.clone(),font_size:14.0,..default()},TextColor(Color::srgb(0.9,0.85,0.55)),Node {position_type:PositionType::Absolute,left:px(336),top:px(672),width:px(540),..default()}));
                // Page titles and the new-player screen use the Mincho glyphs: red, positions and sizes from
                // cshell.dll 0x10024cec..0x10024fe1 (title 0.5 at 580,90; labels 0.3; Start 0.4 at 800,700).
                let red=Color::srgb(1.0,0.0,0.0);let tan=Color::srgb_u8(222,194,120);
                canvas.spawn((ScreenPart(1),Visibility::Hidden,label("mincho",ui.text("CCCreatePlayer"),0.5,red,580.0,90.0)));
                for (text,scale,x,y) in [(ui.text("CCPods"),0.3,420.0,480.0),(ui.text("CCMaxStamina"),0.3,320.0,540.0),(ui.text("CCMaxHealth"),0.3,320.0,590.0),(ui.text("CCDiff"),0.3,340.0,650.0),(ui.text("CCStart"),0.4,800.0,700.0)] {
                    canvas.spawn((ScreenPart(1),Visibility::Hidden,label("mincho",text,scale,red,x,y)));
                }
                let mut values=vec![(CharValue::Points,358.0,470.0),(CharValue::Stamina,530.0,548.0),(CharValue::Health,530.0,596.0)];
                values.extend((0..8).map(|i|(CharValue::Skill(i),820.0,260.0+48.0*i as f32)));
                for (value,x,y) in values {canvas.spawn((ScreenPart(1),Visibility::Hidden,value,label("mincho","",0.4,tan,x,y)));}
                // Clickable areas; the small arrows show only under the cursor (0x10026840). Stamina and health
                // arrows sit at x=480 and 624, the weapon skills at 768 and 912.
                let icon=|name:&str|assets.load(format!("ui/menu/{name}.png"));let hidden=Color::srgba(1.0,1.0,1.0,0.0);
                let mut arrows=vec![("stamina_minus",0,"char_button_minus",480.0,560.0),("stamina_plus",0,"char_button_plus",624.0,560.0),("health_minus",0,"char_button_minus",480.0,608.0),("health_plus",0,"char_button_plus",624.0,608.0)];
                for i in 0..8u8 {arrows.push(("skill_minus",i,"char_button_minus",768.0,272.0+48.0*i as f32));arrows.push(("skill_plus",i,"char_button_plus",912.0,272.0+48.0*i as f32));}
                for (id,index,image,x,y) in arrows {
                    canvas.spawn((Button,PageButton {page:1,id,index},ScreenPart(1),Visibility::Hidden,ImageNode {image:icon(image),color:hidden,..default()},Node {position_type:PositionType::Absolute,left:px(x),top:px(y),width:px(16),height:px(16),..default()}));
                }
                for (index,(id,image,x)) in [("difficulty_easy","char_button_easy",480.0),("difficulty_normal","char_button_tough",544.0),("difficulty_hard","char_button_real",608.0)].into_iter().enumerate() {
                    let index=index as u8;
                    canvas.spawn((Button,PageButton {page:1,id,index},ScreenPart(1),Visibility::Hidden,Node {position_type:PositionType::Absolute,left:px(x),top:px(648),width:px(32),height:px(32),..default()}));
                    canvas.spawn((DifficultyIcon(index),ScreenPart(1),Visibility::Hidden,ImageNode::new(icon(image)),Node {position_type:PositionType::Absolute,left:px(x),top:px(648),width:px(32),height:px(32),..default()}));
                }
                canvas.spawn((Button,PageButton {page:1,id:"start",index:0},ScreenPart(1),Visibility::Hidden,Node {position_type:PositionType::Absolute,left:px(788),top:px(692),width:px(102),height:px(43),..default()}));
                canvas.spawn((Button,PageButton {page:1,id:"back",index:0},ScreenPart(1),Visibility::Hidden,ImageNode {image:icon("Menu_but_1024"),color:hidden,..default()},Node {position_type:PositionType::Absolute,left:px(954),top:px(653),width:px(46),height:px(46),..default()}));
            });
        });
    commands.spawn((LoadingScreen,Visibility::Hidden,GlobalZIndex(300),Node {position_type:PositionType::Absolute,width:percent(100),height:percent(100),justify_content:JustifyContent::Center,align_items:AlignItems::Center,..default()},BackgroundColor(Color::BLACK)))
        .with_children(|root| {root.spawn(Node {width:px(1024),height:px(768),flex_shrink:0.0,..default()}).with_children(|canvas| {
            canvas.spawn((LoadingPicture,ImageNode::default(),Node {position_type:PositionType::Absolute,left:px(170.67),top:px(128),width:px(682.67),height:px(512),..default()}));
            canvas.spawn((LoadingTitle,ImageNode::default(),Node {position_type:PositionType::Absolute,top:px(241),left:px(400),..default()}));
            canvas.spawn((LoadingStatus,ImageNode::default(),Node {position_type:PositionType::Absolute,top:px(602),left:px(400),..default()}));
        });});
}

pub fn interact_pages(mut buttons:Query<(&Interaction,&PageButton,Option<&mut ImageNode>),(Changed<Interaction>,With<Button>)>,mut front:ResMut<Frontend>,mut session:ResMut<Session>,mut travel:ResMut<crate::travel::Travel>,mut campaign:ResMut<crate::campaign::Campaign>) {
    if !front.main_active || front.page!=crate::menu_layout::CHAR {return;}
    for (interaction,button,image) in &mut buttons {
        if button.page!=front.page {continue;}
        // Icon buttons appear only while the cursor is over them.
        if let Some(mut image)=image {image.color=Color::srgba(1.0,1.0,1.0,if *interaction==Interaction::None {0.0}else{1.0});}
        if *interaction!=Interaction::Pressed {continue;}
        session.notice.clear();
        match button.id {
            "back"=>front.page=crate::menu_layout::MAIN,
            "skill_plus"=>front.draft.change_skill(button.index as usize,1),
            "skill_minus"=>front.draft.change_skill(button.index as usize,-1),
            "stamina_plus"=>front.draft.change_stamina(1),
            "stamina_minus"=>front.draft.change_stamina(-1),
            "health_plus"=>front.draft.change_health(1),
            "health_minus"=>front.draft.change_health(-1),
            "difficulty_easy"=>front.draft.difficulty=0,
            "difficulty_normal"=>front.draft.difficulty=1,
            "difficulty_hard"=>front.draft.difficulty=2,
            "start"=>{
                front.bonus=None;front.character_started=true;session.preferences.difficulty=front.draft.difficulty;
                campaign.reset_requested=true;travel.pending=Some("rh3-miasteczko0".into());
            },
            _=>{},
        }
    }
}

/// Numbers and the selected chevron of the new-player screen.
pub fn char_page(front:Res<Frontend>,mut values:Query<(&CharValue,&mut BitmapText)>,mut icons:Query<(&DifficultyIcon,&mut Visibility)>) {
    if !front.main_active || front.page!=1 {return;}
    for (value,mut text) in &mut values {
        text.set(match value {CharValue::Points=>front.draft.points.to_string(),CharValue::Stamina=>front.draft.stamina.to_string(),CharValue::Health=>front.draft.health.to_string(),
            CharValue::Skill(i)=>format!("{:.2}",front.draft.skills[*i] as f32)});
    }
    for (icon,mut visibility) in &mut icons {*visibility=if icon.0==front.draft.difficulty {Visibility::Inherited}else{Visibility::Hidden};}
}

pub fn update(mut front:ResMut<Frontend>,mut session:ResMut<Session>,assets:Res<AssetServer>,materials:Res<Assets<StandardMaterial>>,world_materials:Query<&MeshMaterial3d<StandardMaterial>,With<crate::WorldGeometry>>,retail_materials:Res<Assets<crate::retail_world::RetailWorld>>,retail_world:Query<&MeshMaterial3d<crate::retail_world::RetailWorld>,With<crate::WorldGeometry>>,time:Res<Time>,mut roots:Query<(&mut Visibility,Has<MainScreen>,Has<LoadingScreen>),Without<ScreenPart>>,mut images:Query<(&mut ImageNode,&mut Node,Has<LoadingPicture>,Has<LoadingTitle>,Has<LoadingStatus>),Without<MenuCanvas>>,mut notice:Single<&mut Text,With<Notice>>,mut cursor:Single<&mut bevy::window::CursorOptions>,mut canvas:Single<&mut ImageNode,(With<MenuCanvas>,Without<LoadingPicture>,Without<LoadingTitle>,Without<LoadingStatus>)>,mut parts:Query<(&ScreenPart,&mut Visibility),Without<MainScreen>>) {
    if front.loading.is_some() {front.frames+=1;front.elapsed+=time.delta_secs();session.paused=true;}
    if front.load_started && front.frames>=6 && front.elapsed>=1.5 {
        let ready=world_materials.iter().all(|h|materials.get(&h.0).and_then(|m|m.base_color_texture.as_ref()).is_none_or(|h|assets.is_loaded_with_dependencies(h.id())))
            && retail_world.iter().all(|h|retail_materials.get(&h.0).is_none_or(|m|m.base.iter().chain(m.lightmap.iter()).all(|t|assets.is_loaded_with_dependencies(t.id()))));
        if ready || front.elapsed>15.0 {front.loading=None;front.load_started=false;session.paused=false;session.suppress_fire=2;cursor.visible=false;cursor.grab_mode=bevy::window::CursorGrabMode::Locked;}
    }
    for (mut visibility,main,loading) in &mut roots {
        // Every write below is guarded: an unchanged value written anyway still counts as a change and makes Bevy relayout / re-extract the UI every frame.
        let wanted=if main {Some(if front.main_active && front.loading.is_none() {Visibility::Visible}else{Visibility::Hidden})}else if loading {Some(if front.loading.is_some() {Visibility::Visible}else{Visibility::Hidden})}else{None};
        if let Some(wanted)=wanted {if *visibility!=wanted {*visibility=wanted;}}
    }
    if notice.0!=session.notice {notice.0.clone_from(&session.notice);}
    let background=crate::menu_layout::background(front.page);
    let picture=front.images.get(background).cloned().unwrap_or_else(||assets.load(background.to_string()));
    if canvas.image!=picture {canvas.image=picture;}
    for (part,mut visibility) in &mut parts {let wanted=if part.0==front.page {Visibility::Inherited}else{Visibility::Hidden};if *visibility!=wanted {*visibility=wanted;}}
    if let Some(world)=&front.loading {
        let loading=&front.data["loading"][world];
        let status=&front.data["status"][if front.load_started {"assets"}else{"world"}];
        for (mut image,mut node,picture,title,is_status) in &mut images {
            let (path,w,h)=if picture {(loading["image"].as_str(),682.67,512.0)}
                else if title {(loading["title_image"].as_str(),size(loading,"title_width",200.0),size(loading,"title_height",32.0))}
                else if is_status {(status["image"].as_str(),size(status,"width",280.0),size(status,"height",20.0))}else{continue};
            if let Some(handle)=path.and_then(|p|front.images.get(p)) {image.image=handle.clone();node.width=px(w);node.height=px(h);if !picture {node.left=px((1024.0-w)*0.5);}}
        }
    }
}

/// Silent full-screen UI and both original loading routes.
pub fn probe(mut front:ResMut<Frontend>,config:Res<ViewerConfig>,time:Res<Time>,mut campaign:ResMut<crate::campaign::Campaign>,mut travel:ResMut<crate::travel::Travel>,roster:Res<crate::npcs::NpcRoster>,mut session:ResMut<Session>,mut walking:ResMut<crate::Walking>,mut keys:ResMut<ButtonInput<KeyCode>>,mut buttons:Query<(&mut Interaction,&PageButton),With<Button>>) {
    if !front.probe_active || front.probe_finished {return;}
    let elapsed=time.elapsed_secs();
    match front.probe_stage {
        // The pointer-less probe takes the same shell requests the click on "Új játék indítása" and "Start!" make.
        0 if elapsed>=2.5=>{if !front.main_active || !session.silent {front.probe_failure=Some("Az eredeti főmenü vagy a némítás hiányzik".into());}front.draft=CharacterDraft::default();front.page=crate::menu_layout::CHAR;front.probe_stage=1;},
        1 if elapsed>=3.5=>{if front.page!=crate::menu_layout::CHAR {front.probe_failure=Some("Az Új Játékos képernyő nem nyílt meg".into());}front.draft.points=0;front.draft.stamina=115;front.draft.health=115;for (mut interaction,button) in &mut buttons {if button.id=="start" {*interaction=Interaction::Pressed;}}front.probe_stage=2;},
        2 if elapsed>=7.5=>{if config.world!="rh3-miasteczko0" || campaign.mission.is_none() || campaign.max_health!=115.0 {front.probe_failure=Some(format!("Új játék: {} / küldetés={} / élet={}",config.world,campaign.mission.is_some(),campaign.max_health));}session.paused=true;front.probe_stage=3;},
        3 if elapsed>=8.0=>{campaign.reset_requested=true;travel.pending=Some("chinatown".into());front.probe_stage=4;},
        4 if elapsed>=13.0=>{if config.world!="chinatown" || campaign.mission.is_none() || roster.actors.len()!=17 {front.probe_failure=Some(format!("Kiskína: {} / küldetés={} / NPC={}",config.world,campaign.mission.is_some(),roster.actors.len()));}front.probe_start=Some(Vec3::new(walking.player.position.x,walking.player.position.y,walking.player.position.z));walking.yaw=std::f32::consts::FRAC_PI_2;keys.press(KeyCode::KeyW);front.probe_stage=5;},
        5 if elapsed>=16.0=>{keys.release(KeyCode::KeyW);let p=Vec3::new(walking.player.position.x,walking.player.position.y,walking.player.position.z);if front.probe_start.is_some_and(|start|start.distance(p)<10.0) {front.probe_failure=Some(format!("Kiskína kezdőpozícióból nem lehet sétálni: {p:?}"));}front.probe_finished=true;info!("FŐMENÜPRÓBA: world={} npcs={} position={p:?} failure={:?}",config.world,roster.actors.len(),front.probe_failure);},
        _=>{},
    }
}

#[cfg(test)]
mod ghost_tests {
    use super::*;
    /// Measured on the retail screenshots (1024x768 space): at sin(t)=-1 the "Új játék indítása" ghost starts ~27 px
    /// left of its label (x=340) and ~10 px above it; at t=0 it starts on the label column.
    #[test] fn ghost_offsets_follow_the_retail_clock() {
        let size=Vec2::new(17.0*11.2,22.4);
        let left=ghost_pose(340.0,240.0,size,3.0*std::f32::consts::FRAC_PI_2);
        let rest=ghost_pose(340.0,240.0,size,0.0);
        assert_eq!(rest.x,340.0);assert!((left.x-(340.0-19.0)).abs()<1.5,"{left:?}");
        assert!((rest.y-247.0).abs()<1.5,"{rest:?}");
        assert!(257.0-rest.y>8.0 && 257.0-rest.y<12.0,"the ghost sits about 10 px above the label top");
    }
}

#[cfg(test)]
mod character_tests {
    use super::*;

    #[test]
    fn weapon_skills_cost_one_point_each_and_stop_at_99() {
        let mut draft=CharacterDraft::default();
        draft.change_skill(0,1);draft.change_skill(0,1);draft.change_skill(7,1);
        assert_eq!((draft.points,draft.skills[0],draft.skills[7]),(27,2,1));
        draft.change_skill(0,-1);draft.change_skill(3,-1);assert_eq!((draft.points,draft.skills[0],draft.skills[3]),(28,1,0));
        draft.skills[1]=99;draft.change_skill(1,1);assert_eq!((draft.points,draft.skills[1]),(28,99));
        draft.points=0;draft.change_skill(2,1);assert_eq!(draft.skills[2],0,"no points, no skill");
    }
    #[test]
    fn thirty_points_are_allocated_with_source_screen_limits() {
        let mut draft=CharacterDraft::default();
        assert_eq!((draft.points,draft.stamina,draft.health),(30,100,100));
        for _ in 0..15 {draft.change_stamina(1);draft.change_health(1);}
        assert_eq!((draft.points,draft.stamina,draft.health),(0,115,115));
        draft.change_health(1);assert_eq!(draft.health,115);
        draft.change_stamina(-1);assert_eq!((draft.points,draft.stamina),(1,114));
    }
}
