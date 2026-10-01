//! Retail in-game panels of the HUD object (cshell.dll 0x10a0b420): inventory on C
//! (0x10034a00), character info on Z (0x10038f2d), player attributes on X (0x1003a2d4),
//! the pickup notice (0x10038390) and the level-up icon (0x10037e10).
//! Coordinates are the original 1024x768 values; none of the panels pauses the game.
use bevy::{prelude::*,ecs::system::SystemParam,window::{CursorGrabMode,CursorOptions}};
use crate::{SCALE,InspectionCamera,Walking,ViewerConfig,settings::Session,campaign::Campaign,retail_ui::{RetailUi,BitmapText,label,aided_label,Aid,wrap},
    inventory::BACKPACK_ROWS,character::{Spend,Feedback}};

const SLOT_X:[f32;4]=[116.0,188.0,260.0,332.0];
const SLOT_Y:[f32;6]=[137.0,209.0,281.0,411.0,483.0,603.0];
const SLOT:f32=64.0;
/// SetSurfaceAlpha(panel, 0.8) at 0x1003583f; the cursor and a dragged icon stay opaque.
const ALPHA:f32=0.8;
const RED:Color=Color::srgb(1.0,0.0,0.0);
const YELLOW:Color=Color::srgb(1.0,1.0,0.0);
fn hud_color(alpha:f32)->Color {let [r,g,b]=crate::hud::hud_rgb();Color::srgba(r,g,b,alpha)}

#[derive(Resource,Default)]
pub struct Panels {
    pub inventory:bool,pub char_info:bool,pub attributes:bool,pub scroll:u32,pub cursor:Vec2,pub drag:Option<(u32,u32)>,
    pub pickup:Option<(String,f32)>,pub uses:u32,pub drops:u32,still:f32,moved:bool,arrows:[bool;2],world:String,hover_button:Option<usize>,
    /// The OS cursor is released for the inventory / attribute screen (retail drew its own cursor there, 0x100358fd).
    free:bool,relock:bool,skip_read:u8,
}
impl Panels {
    /// Fire is suppressed while the inventory or the X screen is open (0x10060caa).
    pub fn blocks_fire(&self)->bool {self.inventory || self.attributes}
    fn close(&mut self) {self.inventory=false;self.char_info=false;self.attributes=false;self.drag=None;self.arrows=[false;2];}
    /// Only the inventory and the attribute screen take the mouse; character info is a passive sheet (mouse look and fire stay live, 0x10054c98, 0x10060caa).
    pub fn wants_cursor(&self)->bool {self.inventory || self.attributes}
    /// The real OS cursor is free and drives the panels; nothing may treat a click as "click to play".
    pub fn mouse_mode(&self)->bool {self.free}
    /// Escape closes one panel per press, inventory first, then character info, then attributes, and only then opens the menu (0x1005ac3f).
    fn close_top(&mut self)->bool {
        if self.inventory {self.inventory=false;self.drag=None;self.arrows=[false;2];}else if self.char_info {self.char_info=false;}else if self.attributes {self.attributes=false;}else{return false;}
        true
    }
    pub fn notify_pickup(&mut self,item:&str) {self.pickup=Some((item.to_owned(),1.5));}
}
fn ui_size(window:&Window,scale:f32)->Vec2 {Vec2::new(window.width(),window.height())/scale.max(0.01)}
fn cell_at(p:Vec2)->Option<(u32,u32)> {
    for (c,x) in SLOT_X.iter().enumerate() {for (r,y) in SLOT_Y.iter().enumerate() {
        if p.x>=*x && p.x<x+SLOT && p.y>=*y && p.y<y+SLOT {return Some((c as u32,r as u32));}
    }}
    None
}
fn item_cell(scroll:u32,(c,r):(u32,u32))->(u32,u32) {if r<BACKPACK_ROWS {(c+scroll,r)}else{(c,r)}}
/// X-screen plus buttons (16x32): three statistics, then the eight weapon skills.
fn attribute_buttons()->Vec<(Spend,Vec2)> {
    let mut buttons=vec![(Spend::MaxStamina,Vec2::new(384.0,320.0)),(Spend::MaxHealth,Vec2::new(384.0,384.0)),(Spend::Strength,Vec2::new(384.0,448.0))];
    buttons.extend((0..8).map(|i|(Spend::Skill(i),Vec2::new(912.0,144.0+64.0*i as f32))));buttons
}

#[derive(Resource)]
pub struct PanelUi {
    inventory:Entity,icons:[[Entity;6];4],counts:[[Entity;6];4],arrows:[Entity;2],weight:Entity,drag:Entity,cursor:Entity,tooltip:[Entity;4],
    character:Entity,char_values:Vec<(Entity,&'static str)>,attributes:Entity,attr_values:Vec<Entity>,attr_buttons:Vec<Entity>,
    pickup:Entity,level_up:Entity,message:[Entity;3],
}

pub fn setup(mut commands:Commands,mut ui:ResMut<RetailUi>,assets:Res<AssetServer>) {
    let strings=ui.catalog.text.clone();let t=|key:&str|strings.get(key).cloned().unwrap_or_else(||key.to_owned());
    let (inv_title,backpack,holster,belt,weight)=(t("IPInventory"),t("IPBackpack"),t("IPHolster"),t("IPBelt"),t("IPWeight"));
    let panel_image=ui.panel(&assets,"PanelInv1024");let arrow_images=[ui.panel(&assets,"InvPrzyciskL1024"),ui.panel(&assets,"InvPrzyciskP1024")];
    let mut icons=[[Entity::PLACEHOLDER;6];4];let mut counts=[[Entity::PLACEHOLDER;6];4];let mut arrows=[Entity::PLACEHOLDER;2];let mut weight_text=Entity::PLACEHOLDER;
    let absolute=|x:f32,y:f32,w:f32,h:f32|Node {position_type:PositionType::Absolute,left:px(x),top:px(y),width:px(w),height:px(h),..default()};
    let inventory=commands.spawn((Node {position_type:PositionType::Absolute,right:px(0),top:px(0),width:px(512),height:px(768),..default()},Visibility::Hidden,GlobalZIndex(40)))
        .with_children(|root| {
            root.spawn((ImageNode {image:panel_image,color:Color::WHITE.with_alpha(ALPHA),..default()},absolute(0.0,0.0,512.0,768.0)));
            for (i,image) in arrow_images.into_iter().enumerate() {arrows[i]=root.spawn((ImageNode {image,color:Color::WHITE.with_alpha(ALPHA),..default()},absolute([41.0,406.0][i],209.0,64.0,64.0),Visibility::Hidden)).id();}
            for c in 0..4 {for r in 0..6 {
                icons[c][r]=root.spawn((ImageNode {color:Color::WHITE.with_alpha(ALPHA),..default()},absolute(SLOT_X[c],SLOT_Y[r],SLOT,SLOT),Visibility::Hidden)).with_children(|cell| {
                    counts[c][r]=cell.spawn(label("info","",1.0,Color::WHITE.with_alpha(ALPHA),1.0,1.0)).id();
                }).id();
            }}
            weight_text=root.spawn(label("cyfry","0.00",1.0,Color::WHITE.with_alpha(ALPHA),180.0,700.0)).id();
            // Mincho labels, red, at screen x-512 (0x10035d8f).
            for (text,x,y,scale) in [(inv_title,324.0,66.0,0.5),(backpack,120.0,110.0,0.3),(holster,120.0,380.0,0.3),(belt,120.0,576.0,0.3),(weight,100.0,680.0,0.3)] {
                root.spawn(label("mincho",text,scale,RED,x,y));
            }
        }).id();
    let drag=commands.spawn((ImageNode::default(),absolute(0.0,0.0,SLOT,SLOT),Visibility::Hidden,GlobalZIndex(44))).id();
    let tooltip=[0,1,2,3].map(|i|commands.spawn((label("mincho","",0.3,if i%2==0 {Color::BLACK}else{YELLOW},0.0,0.0),Visibility::Hidden,GlobalZIndex(45+i as i32%2))).id());
    let cursor_image=ui.panel(&assets,"cursor1024");
    let cursor=commands.spawn((ImageNode::new(cursor_image),absolute(0.0,0.0,32.0,32.0),Visibility::Hidden,GlobalZIndex(60))).id();

    // Z: character information, left half (0x10038f2d..0x10039736).
    let char_image=ui.panel(&assets,"PanelChar1024");
    let labels=[("CSPChars",190.0,60.0,0.5),("CSPHealth",50.0,180.0,0.3),("CSPStamina",50.0,230.0,0.3),("CSPDexterity",50.0,276.0,0.3),("CSPStrenght",50.0,386.0,0.3),
        ("CSPAlcohol",50.0,460.0,0.3),("CSPPowUp",50.0,516.0,0.3),("CSPPainKiller",50.0,562.0,0.3),("CSPCur",210.0,146.0,0.3),("CSPCur",210.0,196.0,0.3),("CSPCur",210.0,246.0,0.3),
        ("CSPCur",320.0,356.0,0.3),("CSPMax",320.0,146.0,0.3),("CSPMax",320.0,196.0,0.3),("CSPMax",320.0,246.0,0.3),("CSPLeft",210.0,356.0,0.3)].map(|(k,x,y,s)|(t(k),x,y,s));
    let mut char_values=Vec::new();
    let character=commands.spawn((Node {position_type:PositionType::Absolute,left:px(0),top:px(0),width:px(512),height:px(768),..default()},Visibility::Hidden,GlobalZIndex(39)))
        .with_children(|root| {
            root.spawn((ImageNode {image:char_image,color:Color::WHITE.with_alpha(ALPHA),..default()},absolute(0.0,0.0,512.0,768.0)));
            for (text,x,y,scale) in labels {root.spawn(label("mincho",text,scale,RED,x,y));}
            for (key,x,y) in [("health",224.0,166.0),("max_health",352.0,166.0),("stamina",224.0,214.0),("max_stamina",352.0,214.0),("dexterity",224.0,262.0),("max_dexterity",352.0,262.0),
                ("left",224.0,377.0),("strength",352.0,377.0),("alcohol",224.0,457.0),("power_up",224.0,505.0),("pain_killer",224.0,553.0)] {
                char_values.push((root.spawn(label("cyfry","",24.0/27.0,Color::WHITE.with_alpha(ALPHA),x,y)).id(),key));
            }
        }).id();

    // X: player attributes, full screen (0x1003a2d4, handler 0x1003a8e4).
    let attr_image=ui.panel(&assets,"player_attrib_1024");let plus=ui.panel(&assets,"player_attrib_button");
    let attr_labels=[("PIPAttrib",60.0,100.0,0.5),("PIPWeapExp",720.0,100.0,0.3),("PIPPods",170.0,210.0,0.3),("PIPMaxStam",50.0,350.0,0.3),("PIPMaxHealth",50.0,410.0,0.3),
        ("PIPStrength",50.0,480.0,0.3),("PIPEnemiesKilled",23.0,560.0,0.3),("PIPCurExp",23.0,630.0,0.3),("PIPNextLev",40.0,690.0,0.3)].map(|(k,x,y,s)|(t(k),x,y,s));
    let mut attr_values=Vec::new();let mut attr_buttons=Vec::new();
    let attributes=commands.spawn((Node {position_type:PositionType::Absolute,width:percent(100),height:percent(100),justify_content:JustifyContent::Center,align_items:AlignItems::Center,..default()},Visibility::Hidden,GlobalZIndex(41)))
        .with_children(|root| {root.spawn(Node {width:px(1024),height:px(768),flex_shrink:0.0,..default()}).with_children(|canvas| {
            canvas.spawn((ImageNode {image:attr_image,color:Color::WHITE.with_alpha(ALPHA),..default()},absolute(0.0,0.0,1024.0,768.0)));
            for (text,x,y,scale) in attr_labels {canvas.spawn(label("mincho",text,scale,RED,x,y));}
            // HUD ammo digits in the HUD colour (default palette 7 = 208,115,16, alpha 196).
            let mut positions=vec![(96.0,200.0),(256.0,328.0),(256.0,392.0),(256.0,456.0),(256.0,552.0),(256.0,616.0),(256.0,680.0)];
            positions.extend((0..8).map(|i|(770.0,154.0+64.0*i as f32)));
            for (x,y) in positions {attr_values.push(canvas.spawn(label("ammo","",1.0,hud_color(196.0/255.0),x,y)).id());}
            for (_,p) in attribute_buttons() {attr_buttons.push(canvas.spawn((ImageNode::new(plus.clone()),absolute(p.x,p.y,16.0,32.0),Visibility::Hidden)).id());}
        });}).id();

    let pickup=commands.spawn((ImageNode::default(),Node {position_type:PositionType::Absolute,right:px(0),top:px(512),width:px(128),height:px(128),..default()},Visibility::Hidden,GlobalZIndex(21))).id();
    let level_image=ui.image(&assets,"hud/level_up.png");
    let level_up=commands.spawn((ImageNode {image:level_image,color:hud_color(0.0),..default()},absolute(0.0,0.0,64.0,64.0),Visibility::Hidden,GlobalZIndex(21))).id();
    let message=[0,1,2].map(|i|commands.spawn((aided_label("mincho","",0.4,Color::WHITE,72.0,8.0+26.0*i as f32,Aid::BACKED),GlobalZIndex(26))).id());
    commands.insert_resource(PanelUi {inventory,icons,counts,arrows,weight:weight_text,drag,cursor,tooltip,character,char_values,attributes,attr_values,attr_buttons,pickup,level_up,message});
}

#[derive(SystemParam)]
pub struct World<'w,'s> {
    config:Res<'w,ViewerConfig>,session:ResMut<'w,Session>,front:Res<'w,crate::frontend::Frontend>,opening:Res<'w,crate::opening::Opening>,
    walking:ResMut<'w,Walking>,camera:Single<'w,'s,&'static Transform,With<InspectionCamera>>,drops:ResMut<'w,crate::pickups::DropQueue>,
    native:ResMut<'w,crate::retail_weapons::NativeArsenal>,campaign:ResMut<'w,Campaign>,ui:Res<'w,RetailUi>,options:Res<'w,crate::options::Options>,view:Res<'w,crate::view::ViewState>,
}
#[derive(SystemParam)]
pub struct Devices<'w,'s> {
    keys:ResMut<'w,ButtonInput<KeyCode>>,buttons:ResMut<'w,ButtonInput<MouseButton>>,
    window:Single<'w,'s,&'static mut Window>,cursor:Single<'w,'s,&'static mut CursorOptions>,scale:Res<'w,UiScale>,time:Res<'w,Time>,
}

/// The two OS-cursor calls the panels make.
#[derive(Debug,PartialEq,Clone,Copy)] pub enum CursorCall {Release,Lock}
/// Cursor hand-over state machine: `(free, relock, wants, grabbed, managed)` -> new `(free, relock)` and the call to make.
/// A panel that takes the mouse releases the grab (and remembers whether to restore it); the last one closing locks it again
/// like `settings::resume`, unless the pause menu, main menu or a level load own the cursor.
pub fn cursor_step(free:bool,relock:bool,wants:bool,grabbed:bool,managed:bool)->(bool,bool,Option<CursorCall>) {
    if wants && !free {return (true,grabbed,Some(CursorCall::Release));}
    if !wants && free {return (false,false,(relock && !managed).then_some(CursorCall::Lock));}
    (free,relock,None)
}

/// Toggles, cursor hand-over, drag and drop, item use and the input the open panels swallow.
pub fn input(mut panels:ResMut<Panels>,mut io:Devices,mut w:World) {
    let scale=io.scale.0.max(0.01);let screen=ui_size(&io.window,scale);
    if w.config.world!=panels.world {panels.close();panels.world=w.config.world.clone();panels.scroll=0;panels.cursor=screen*0.5;}
    // Opening the menu, a level load or a cutscene clears every panel flag (0x100240d0, 0x1004c2fb); death and dialogues take the mouse too.
    let idle=w.front.main_active || w.front.loading.is_some() || w.opening.active || w.campaign.dead() || w.session.dialogue_active || w.session.paused;
    if idle {panels.close();}
    else {
        if io.keys.just_pressed(KeyCode::Escape) && panels.close_top() {io.keys.clear_just_pressed(KeyCode::Escape);}
        if w.options.keys.just_pressed(crate::keys_cfg::cmd::INVENTORY_PANEL,&io.keys,&io.buttons) {panels.inventory=!panels.inventory;panels.drag=None;}
        if w.options.keys.just_pressed(crate::keys_cfg::cmd::CHARACTER_PANEL,&io.keys,&io.buttons) {panels.char_info=!panels.char_info;}
        if w.options.keys.just_pressed(crate::keys_cfg::cmd::PLAYER_PANEL,&io.keys,&io.buttons) {panels.attributes=!panels.attributes;}
        if panels.attributes {panels.inventory=false;panels.char_info=false;panels.drag=None;}
        // F1..F4 use the belt with the panel open or closed (0x100348e0).
        for (slot,key) in [KeyCode::F1,KeyCode::F2,KeyCode::F3,KeyCode::F4].into_iter().enumerate() {
            if io.keys.just_pressed(key) {let effect=w.campaign.items.use_belt(slot as u32,&w.ui.catalog);if let Some(effect)=effect {w.campaign.apply(effect,w.view.level_frames());panels.uses+=1;}}
        }
    }
    // The real OS cursor (the retail golden arrow is its icon, cursor.rs) replaces the old virtual one.
    let managed=w.session.paused || w.front.main_active || w.front.loading.is_some();
    let (free,relock,call)=cursor_step(panels.free,panels.relock,panels.wants_cursor(),io.cursor.grab_mode!=CursorGrabMode::None,managed);
    (panels.free,panels.relock)=(free,relock);
    match call {
        Some(CursorCall::Release)=>{
            io.cursor.visible=true;io.cursor.grab_mode=CursorGrabMode::None;
            // Start where the cursor was last used (the hidden game cursor could be anywhere); the first read after the warp is skipped.
            if w.config.capture.is_none() {let at=panels.cursor*scale;io.window.set_cursor_position(Some(at));}
            panels.skip_read=1;
        },
        Some(CursorCall::Lock)=>{io.cursor.visible=false;io.cursor.grab_mode=CursorGrabMode::Locked;w.session.suppress_fire=3;},
        None=>{}
    }
    if !panels.wants_cursor() {return;}
    let previous=panels.cursor;
    if panels.skip_read>0 {panels.skip_read-=1;}
    else if let Some(at)=io.window.cursor_position() {panels.cursor=(at/scale).clamp(Vec2::ZERO,screen);}
    panels.moved=panels.cursor!=previous;
    let left_down=io.buttons.pressed(MouseButton::Left);let left_edge=io.buttons.just_pressed(MouseButton::Left);let right_edge=io.buttons.just_pressed(MouseButton::Right);
    io.buttons.clear_just_pressed(MouseButton::Left);
    if panels.attributes {
        let origin=(screen-Vec2::new(1024.0,768.0))*0.5;let p=panels.cursor-origin;
        panels.hover_button=attribute_buttons().iter().position(|(_,b)|p.x>=b.x && p.x<b.x+16.0 && p.y>=b.y && p.y<b.y+32.0);
        if left_edge {if let Some(index)=panels.hover_button {
            let Campaign {stats,health,max_health,..}=&mut *w.campaign;
            if crate::character::spend(stats,health,max_health,&mut w.walking.player.stamina,attribute_buttons()[index].0) {w.walking.player.max_stamina=stats.max_stamina;}
        }}
        return;
    }
    io.buttons.clear_just_pressed(MouseButton::Right);
    // The open inventory swallows the use, jump and weapon keys (0x10060ba8, 0x10060c7e, 0x1005faa0); bound keys, so a rebinding is swallowed too.
    for command in [crate::keys_cfg::cmd::ACTION,crate::keys_cfg::cmd::JUMP].into_iter().chain(crate::keys_cfg::cmd::WEAPON1..crate::keys_cfg::cmd::WEAPON1+8) {
        for action in w.options.keys.bound(command) {for key in crate::keys_cfg::keys_of_action(action) {io.keys.clear_just_pressed(key);}}
    }
    let origin=Vec2::new(screen.x-512.0,0.0);let local=panels.cursor-origin;
    let hovered=cell_at(local).map(|cell|item_cell(panels.scroll,cell));
    // Arrows scroll one backpack column on the press edge; left stops at 0 (0x10034f55).
    let on_left=local.x>40.0 && local.x<104.0 && local.y>208.0 && local.y<272.0;
    let on_right=local.x>406.0 && local.x<470.0 && local.y>209.0 && local.y<273.0;
    panels.arrows=[left_down && on_left && panels.drag.is_none(),left_down && on_right && panels.drag.is_none()];
    if left_edge && panels.drag.is_none() {if on_left {panels.scroll=panels.scroll.saturating_sub(1);}else if on_right {panels.scroll+=1;}}
    // Right mouse uses an `eaten` item (0x10035399).
    if right_edge && panels.drag.is_none() {if let Some(cell)=hovered {let effect=w.campaign.items.use_in_panel(cell,&w.ui.catalog);if let Some(effect)=effect {w.campaign.apply(effect,w.view.level_frames());panels.uses+=1;}}}
    // Left button held over a movable item picks it up (0x10034fd9).
    if left_down && panels.drag.is_none() {if let Some(cell)=hovered {
        if w.campaign.items.at(cell.0,cell.1).is_some_and(|e|!w.ui.catalog.items.get(&e.item).is_some_and(|d|d.fixed)) {panels.drag=Some(cell);}
    }}
    if !left_down {if let Some(from)=panels.drag.take() {
        if panels.cursor.x<origin.x {drop_to_world(&mut panels,&mut w,from);}
        else if let Some(to)=hovered {w.campaign.items.move_item(from,to);}
    }}
    panels.still=if panels.moved {0.0}else{panels.still+io.time.delta_secs()};
}

/// Drag to the left half (0x1003508a -> 0x1001c3a0): one unit leaves at eye + 16*d, velocity 480*d.
fn drop_to_world(panels:&mut Panels,w:&mut World,from:(u32,u32)) {
    let Some(item)=w.campaign.items.at(from.0,from.1).map(|e|e.item.clone()) else {return};
    let mut direction=*w.camera.forward();direction.y=direction.y.min(0.0);let direction=direction.normalize_or(Vec3::NEG_Z);
    let eye=w.camera.translation/SCALE;let spawn=eye+direction*16.0;
    let half=Vec3::splat(8.0);let n=|v:Vec3|retail_movement::Vec3::new(v.x,v.y,v.z);
    if w.walking.world.raycast(n(eye),n(direction),16.0+half.x).is_some() || w.walking.world.box_overlaps(n(spawn),n(half)) {warn!("Not enough room!");return;}
    w.campaign.items.take_one(from);
    let d=w.ui.catalog.items.get(&item);
    if d.is_some_and(|d|d.weapon) && !w.campaign.items.has(&item) {w.native.release(&item);}
    w.drops.thrown.push((item.clone(),spawn,direction*480.0));panels.drops+=1;
    info!("Eldobva: {item}");
}

pub fn present(panels:Res<Panels>,pui:Res<PanelUi>,mut ui:ResMut<RetailUi>,assets:Res<AssetServer>,campaign:Res<Campaign>,walking:Res<Walking>,feedback:Res<Feedback>,window:Single<&Window>,scale:Res<UiScale>,
    mut nodes:Query<(&mut Node,&mut Visibility,Option<&mut ImageNode>)>,mut texts:Query<&mut BitmapText>,opening:Res<crate::opening::Opening>,session:Res<Session>,front:Res<crate::frontend::Frontend>,config:Res<ViewerConfig>,dialogue:Res<crate::dialogue::Dialogue>) {
    let screen=ui_size(&window,scale.0);
    let hidden=front.main_active || front.loading.is_some() || opening.active || session.paused;
    let show=|entity:Entity,visible:bool,nodes:&mut Query<(&mut Node,&mut Visibility,Option<&mut ImageNode>)>| {if let Ok((_,mut v,_))=nodes.get_mut(entity) {*v=if visible {Visibility::Inherited}else{Visibility::Hidden};}};
    show(pui.inventory,panels.inventory && !hidden,&mut nodes);show(pui.character,panels.char_info && !hidden,&mut nodes);show(pui.attributes,panels.attributes && !hidden,&mut nodes);
    // The OS cursor is the visible pointer; the drawn one only stands in for it in hidden capture runs (no OS cursor there).
    show(pui.cursor,config.capture.is_some() && panels.wants_cursor() && !hidden,&mut nodes);
    if let Ok((mut node,_,_))=nodes.get_mut(pui.cursor) {node.left=px(panels.cursor.x);node.top=px(panels.cursor.y);}
    // Inventory grid.
    for c in 0..4u32 {for r in 0..6u32 {
        let cell=item_cell(panels.scroll,(c,r));
        let entry=campaign.items.at(cell.0,cell.1).filter(|_|panels.drag!=Some(cell));
        let icon=entry.and_then(|e|ui.icon(&assets,&e.item));
        if let Ok((_,mut v,Some(mut image)))=nodes.get_mut(pui.icons[c as usize][r as usize]) {
            *v=if icon.is_some() {Visibility::Inherited}else{Visibility::Hidden};if let Some(icon)=icon {image.image=icon;}
        }
        if let Ok(mut text)=texts.get_mut(pui.counts[c as usize][r as usize]) {text.set(entry.filter(|e|e.count>1).map(|e|e.count.to_string()).unwrap_or_default());}
    }}
    for (i,arrow) in pui.arrows.iter().enumerate() {show(*arrow,panels.arrows[i],&mut nodes);}
    if let Ok(mut text)=texts.get_mut(pui.weight) {text.set(format_weight(campaign.items.weight(&ui.catalog)));}
    let dragged=panels.drag.and_then(|cell|campaign.items.at(cell.0,cell.1)).and_then(|e|ui.icon(&assets,&e.item));
    show(pui.drag,dragged.is_some() && panels.inventory && !hidden,&mut nodes);
    if let (Some(icon),Ok((mut node,_,Some(mut image))))=(dragged,nodes.get_mut(pui.drag)) {image.image=icon;node.left=px(panels.cursor.x-32.0);node.top=px(panels.cursor.y-32.0);}
    // Tooltip: title and description after 0.5 s without cursor movement (0x10036030).
    let origin=Vec2::new(screen.x-512.0,0.0);
    let hovered=cell_at(panels.cursor-origin).map(|cell|item_cell(panels.scroll,cell)).and_then(|cell|campaign.items.at(cell.0,cell.1));
    let tip=hovered.filter(|_|panels.inventory && panels.drag.is_none() && panels.still>=0.5 && !hidden).and_then(|e|ui.catalog.items.get(&e.item)).map(|d|[d.title.clone(),d.description.clone()]);
    let mincho=ui.fonts.get("mincho").cloned().unwrap_or_default();
    let width=tip.as_ref().map_or(0.0,|lines|lines.iter().map(|l|mincho.width(l,0.3)).fold(0.0,f32::max));
    let (mut x,mut y)=(panels.cursor.x+32.0,panels.cursor.y);if x+width>screen.x {x=screen.x-width-8.0;y+=32.0;}
    for (i,entity) in pui.tooltip.iter().enumerate() {
        let (line,shadow)=(i/2,i%2==0);
        show(*entity,tip.is_some(),&mut nodes);
        if let Ok((mut node,_,_))=nodes.get_mut(*entity) {node.left=px(x+if shadow {2.0}else{0.0});node.top=px(y+16.0*line as f32+if shadow {2.0}else{0.0});}
        if let Ok(mut text)=texts.get_mut(*entity) {text.set(tip.as_ref().map(|l|l[line].clone()).unwrap_or_default());}
    }
    // Z values: integers (cyfry font).
    let p=&walking.player;let s=&campaign.stats;
    for (entity,key) in &pui.char_values {
        let value=match *key {"health"=>campaign.health,"max_health"=>campaign.max_health,"stamina"=>p.stamina,"max_stamina"=>p.max_stamina,"dexterity"|"max_dexterity"=>s.dexterity,
            "left"=>(s.strength-campaign.items.weight(&ui.catalog).trunc()).trunc(),"strength"=>s.strength,"alcohol"=>campaign.alcohol,"power_up"=>s.power_up,"pain_killer"=>s.pain_killer,_=>0.0};
        if let Ok(mut text)=texts.get_mut(*entity) {text.set((value.trunc() as i64).to_string());}
    }
    // X values: HUD digits; skills with two decimals.
    let values=[s.free_points.to_string(),p.max_stamina.trunc().to_string(),campaign.max_health.trunc().to_string(),s.strength.trunc().to_string(),campaign.kills.to_string(),campaign.experience.to_string(),s.next_level.to_string()];
    for (i,entity) in pui.attr_values.iter().enumerate() {
        let value=values.get(i).cloned().unwrap_or_else(||format!("{:.2}",s.skills[i-values.len()]));
        if let Ok(mut text)=texts.get_mut(*entity) {text.set(value);}
    }
    for (i,entity) in pui.attr_buttons.iter().enumerate() {show(*entity,panels.hover_button==Some(i),&mut nodes);}
    // Pickup notice: 1.5 s, full HUD alpha (196) for 0.5 s then fading (0x10038515); hidden under the inventory.
    let notice=panels.pickup.as_ref().filter(|_|!panels.inventory && !hidden).and_then(|(item,time)|Some((ui.catalog.items.get(item)?.pickup_icon.clone()?,*time)));
    show(pui.pickup,notice.is_some(),&mut nodes);
    if let (Some((path,time)),Ok((_,_,Some(mut image))))=(notice,nodes.get_mut(pui.pickup)) {image.image=ui.image(&assets,&path);image.color=hud_color(196.0/255.0*time.min(1.0));}
    // Level-up icon at (0,0): alpha 196 * min(timer, 1), hidden under the inventory or X screen.
    show(pui.level_up,feedback.level_up>0.0 && !panels.inventory && !panels.attributes && !hidden,&mut nodes);
    if let Ok((_,_,Some(mut image)))=nodes.get_mut(pui.level_up) {image.color=hud_color(196.0/255.0*feedback.level_up.min(1.0));}
    let lines=if hidden {Vec::new()}else{wrap(&feedback.message,((screen.x-90.0)/(32.0*0.4)) as usize)};
    // The message (level-up, pickups, saves, the dialogue hint) sits at the top left; while the dialogue panel is up it moves below the panel text.
    let top=if dialogue.bottom>0.0 {dialogue.bottom+10.0}else{8.0};
    for (i,entity) in pui.message.iter().enumerate() {
        if let Ok(mut text)=texts.get_mut(*entity) {text.set(lines.get(i).cloned().unwrap_or_default());}
        if let Ok((mut node,_,_))=nodes.get_mut(*entity) {node.top=px(top+26.0*i as f32);}
    }
}

/// 0x10058990: two decimals, rounded half up.
pub fn format_weight(weight:f32)->String {format!("{:.2}",(weight as f64*100.0+0.5).floor()/100.0)}

pub fn tick_notices(time:Res<Time>,mut panels:ResMut<Panels>,session:Res<Session>) {
    if session.paused {return;}
    if let Some((_,t))=&mut panels.pickup {*t-=time.delta_secs();if *t<=0.0 {panels.pickup=None;}}
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn the_os_cursor_is_released_for_the_mouse_panels_and_locked_again_when_the_last_closes() {
        // Opening from the locked game releases it and remembers to restore it.
        assert_eq!(cursor_step(false,false,true,true,false),(true,true,Some(CursorCall::Release)));
        assert_eq!(cursor_step(true,true,true,false,false),(true,true,None),"steady while open");
        assert_eq!(cursor_step(true,true,false,false,false),(false,false,Some(CursorCall::Lock)));
        // Opened before the game had grabbed the mouse ("click to play" screen): nothing to restore.
        assert_eq!(cursor_step(false,false,true,false,false),(true,false,Some(CursorCall::Release)));
        assert_eq!(cursor_step(true,false,false,false,false),(false,false,None));
        // The pause menu, main menu and level loads own the cursor.
        assert_eq!(cursor_step(true,true,false,false,true),(false,false,None));
        assert_eq!(cursor_step(false,false,false,true,false),(false,false,None),"a game without panels is left alone");
    }
    #[test] fn escape_closes_inventory_then_character_info_then_attributes_and_the_sheet_alone_keeps_the_mouse_look() {
        let mut panels=Panels {inventory:true,char_info:true,..default()};
        assert!(panels.wants_cursor());assert!(panels.close_top() && !panels.inventory && panels.char_info);
        assert!(!panels.wants_cursor(),"character info is a passive sheet");
        assert!(panels.close_top() && !panels.char_info && !panels.close_top(),"nothing left: Escape opens the menu");
        let mut panels=Panels {attributes:true,..default()};assert!(panels.wants_cursor() && panels.close_top() && !panels.attributes);
    }
    #[test] fn slot_rectangles_and_backpack_scroll_match_the_retail_grid() {
        assert_eq!(cell_at(Vec2::new(116.0,137.0)),Some((0,0)));assert_eq!(cell_at(Vec2::new(395.9,666.9)),Some((3,5)));
        assert_eq!(cell_at(Vec2::new(180.5,137.0)),None,"gap between cells");
        assert_eq!(item_cell(2,(1,2)),(3,2));assert_eq!(item_cell(2,(1,3)),(1,3),"holster and belt do not scroll");
        assert_eq!(format_weight(1.28),"1.28");assert_eq!(format_weight(29.375),"29.38");assert_eq!(format_weight(0.0),"0.00");
        assert_eq!(attribute_buttons()[3],(Spend::Skill(0),Vec2::new(912.0,144.0)));
    }
}
