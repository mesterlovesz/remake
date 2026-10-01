//! The retail menu (cshell.dll 0x10023000..0x1002b000): every page is drawn in the 1024x768 design space and hit-tested with
//! the retail rectangles of [`crate::menu_layout`]; this module owns the state, the mouse / keyboard handling and the entities.
//! Pages: main list, new player (frontend.rs), misc sliders, controls #1/#2 with key capture, display / sound toggle lists,
//! the remake's video list, load / save slot pages with thumbnails and the quit confirmation.
use bevy::{prelude::*,ecs::system::SystemParam,asset::RenderAssetUsages,input::mouse::AccumulatedMouseScroll,window::{CursorGrabMode,CursorOptions},
    render::render_resource::{Extent3d,TextureDimension,TextureFormat},render::view::screenshot::{Screenshot,ScreenshotCaptured}};
use crate::{ViewerConfig,Walking,frontend::{Frontend,MenuCanvas,CharacterDraft},menu_layout::{self as ml,*},options::Options,retail_ui::{RetailUi,BitmapText,label},savefile,settings::Session,
    campaign::Campaign,travel::Travel,opening::Opening,npcs::NpcRoster,retail_weapons::NativeArsenal,character::Feedback,keys_cfg};

/// Seconds the message after a save stays on screen (0x10029ed4: 5.0 at 0x100b2304).
pub const MESSAGE_SECONDS:f32=5.0;

#[derive(Resource,Default)]
pub struct MenuState {
    /// Pointer in design-space pixels (the canvas is centred in the window and scaled by `UiScale`).
    pub mouse:Vec2,/// Scripted pointer of the menu probes and captures.
    pub fake:Option<Vec2>,pub hover:Option<usize>,pub slider:Option<usize>,pub capture:Option<(usize,usize)>,
    pub slots:SlotList,pub labels:Vec<String>,pub details:[String;2],pub thumb:Option<Handle<Image>>,
    /// The game picture grabbed when the menu opened (320x240 RGB565), the thumbnail of the "new save" entry (0x1004b550).
    pub snapshot:Option<Vec<u8>>,
    /// Frames until a requested open really shows the menu (the grab must not contain the menu itself).
    pub opening:u8,scroll:Option<(bool,f32)>,knob_drag:bool,shown:Vec<Item>,/// The entity of every shown item (a `Thumb` without a picture has none), for the in-place text edits of `render`.
    spawned:Vec<Option<Entity>>,
    /// The page a load/save list was built for, and its selection as of the last refresh.
    listed:Option<(u8,usize)>,
}
impl MenuState {/// The items the layer was last built from (the menu probes count them against the drawn text entities).
    pub fn shown(&self)->&[Item] {&self.shown}}
#[derive(Resource)] pub struct MenuLayer(pub Entity);
#[derive(Component)] pub struct GhostText {x:f32,row_y:f32,size:Vec2}
/// Everything the menu asks of the running game.
#[derive(SystemParam)]
pub struct Game<'w> {
    pub campaign:ResMut<'w,Campaign>,pub travel:ResMut<'w,Travel>,pub walking:Res<'w,Walking>,pub roster:Res<'w,NpcRoster>,pub native:Res<'w,NativeArsenal>,
    pub opening:Res<'w,Opening>,pub feedback:ResMut<'w,Feedback>,pub ui:Res<'w,RetailUi>,pub config:Res<'w,ViewerConfig>,
    pub video:Res<'w,crate::video::Video>,pub panels:Res<'w,crate::panels::Panels>,
}
impl Game<'_> {
    pub fn loc(&self,key:&str)->String {if key.starts_with("Remake") {ml::remake_text(key).to_owned()}else{self.ui.text(key)}}
    /// The retail condition of the save row (0x1002a61d..): a running game, alive, not kneeling, no cutscene.
    pub fn can_save(&self,front:&Frontend)->bool {front.in_game && front.bonus.is_none() && !self.campaign.dead() && !self.walking.player.crouched && !self.opening.active && self.campaign.mission.is_some()}
}

pub fn setup(mut commands:Commands,canvas:Single<Entity,With<MenuCanvas>>) {
    let layer=commands.spawn(Node {position_type:PositionType::Absolute,left:px(0),top:px(0),width:px(1024),height:px(768),..default()}).id();
    commands.entity(*canvas).add_child(layer);
    commands.insert_resource(MenuLayer(layer));commands.init_resource::<MenuState>();
}

fn fake_mouse()->Option<Vec2> {
    let text=std::env::var("MESTER_MENU_MOUSE").ok()?;let mut parts=text.split(',').filter_map(|v|v.trim().parse::<f32>().ok());
    Some(Vec2::new(parts.next()?,parts.next()?))
}
/// Window pixel -> design-space pixel of the centred, `UiScale`d canvas.
pub fn design_pos(window:&Window,scale:f32)->Option<Vec2> {
    let cursor=window.cursor_position()?;let size=Vec2::new(window.width(),window.height());
    Some((cursor-(size-Vec2::new(1024.0,768.0)*scale)*0.5)/scale)
}
fn resume(session:&mut Session,cursor:&mut CursorOptions) {session.paused=false;session.suppress_fire=2;cursor.visible=false;cursor.grab_mode=CursorGrabMode::Locked;}

/// Asks for the grab of the game picture (the menu thumbnail) and shows the menu two frames later.
pub fn request_open(menu:&mut MenuState,commands:&mut Commands) {
    menu.opening=2;
    commands.spawn(Screenshot::primary_window()).observe(|shot:On<ScreenshotCaptured>,mut menu:ResMut<MenuState>| {
        let image=shot.image.clone();
        if let Ok(dynamic)=image.try_into_dynamic() {let rgba=dynamic.to_rgba8();menu.snapshot=savefile::thumbnail_from_pixels(rgba.width() as usize,rgba.height() as usize,rgba.as_raw(),false);}
    });
}
/// Opens the menu page from a running game without a thumbnail grab (probes, captures).
pub fn open_now(front:&mut Frontend,menu:&mut MenuState,page:u8) {menu.labels.clear();front.main_active=true;front.page=page;menu.hover=None;menu.capture=None;menu.slider=None;}

fn thumbnail_image(images:&mut Assets<Image>,thumb:&[u8])->Handle<Image> {
    images.add(Image::new(Extent3d {width:savefile::THUMB_W as u32,height:savefile::THUMB_H as u32,depth_or_array_layers:1},TextureDimension::D2,savefile::rgba_from_thumbnail(thumb),TextureFormat::Rgba8UnormSrgb,RenderAssetUsages::default()))
}
fn save_dir(config:&ViewerConfig)->std::path::PathBuf {config.user.join("save")}
fn slot_label(header:&savefile::Header)->String {if header.title.is_empty() {header.world.clone()}else{header.title.clone()}}

/// Builds the list of a load / save page (0x10024560 / 0x10024890). Returns false when a load page has nothing to show.
fn open_slots(menu:&mut MenuState,game:&Game,save_page:bool)->bool {
    let slots=savefile::list(&save_dir(&game.config));
    if !save_page && slots.is_empty() {return false;}
    let mut labels:Vec<String>=Vec::new();
    if save_page {labels.push(game.loc("SGNewSave"));}
    labels.extend(slots.iter().map(|slot|slot_label(&slot.header)));
    menu.slots=SlotList::new(slots,save_page);menu.labels=labels;menu.listed=None;true
}
/// Details lines and thumbnail of the selected row (0x1002a2f6..0x1002a418).
fn refresh_slot(menu:&mut MenuState,game:&Game,images:&mut Assets<Image>,page:u8) {
    let key=(page,menu.slots.index());
    if menu.listed==Some(key) {return;}
    menu.listed=Some(key);
    let (created,at,health)=(game.loc("SGCreated"),game.loc("SGAt"),game.loc("SGPlayerHealth"));
    match menu.slots.selected() {
        Some(None)=>{
            menu.details=[format!("{created} {}",game.loc("SGNow")),String::new()];
            menu.thumb=menu.snapshot.as_ref().map(|t|thumbnail_image(images,t));
        },
        Some(Some(slot))=>{
            menu.details=[savefile::created_line(&created,&at,&slot.header.time),savefile::health_line(&health,slot.header.health)];
            let thumb=savefile::read(&slot.path,false).ok().and_then(|f|f.thumbnail);
            menu.thumb=thumb.map(|t|thumbnail_image(images,&t));
        },
        None=>{menu.details=Default::default();menu.thumb=None;},
    }
}

/// What a page does on Esc (0x1002ad10) and for the back button: sub pages return to the list, the main list resumes a running game.
fn back(front:&mut Frontend,menu:&mut MenuState,session:&mut Session,cursor:&mut CursorOptions) {
    menu.capture=None;menu.slider=None;menu.knob_drag=false;menu.scroll=None;
    match escape_target(front.page) {
        Some(page)=>front.page=page,
        None if front.in_game=>{front.main_active=false;resume(session,cursor);},
        None=>{},
    }
}

/// Mouse and keyboard of the menu; runs before the game systems of the frame.
#[allow(clippy::too_many_arguments)]
pub fn input(mut menu:ResMut<MenuState>,mut front:ResMut<Frontend>,mut options:ResMut<Options>,mut session:ResMut<Session>,mut keys:ResMut<ButtonInput<KeyCode>>,buttons:Res<ButtonInput<MouseButton>>,
    window:Single<&Window>,scale:Res<UiScale>,mut cursor:Single<&mut CursorOptions>,mut game:Game,time:Res<Time>,wheel:Res<AccumulatedMouseScroll>,mut images:ResMut<Assets<Image>>,
    mut exit:MessageWriter<AppExit>,mut commands:Commands) {
    if front.loading.is_some() || game.video.active {return;}
    // Esc in a running game opens the menu (0x1005abd4: not in cutscenes); the open waits for the picture grab.
    if !front.main_active {
        if menu.opening>0 {menu.opening-=1;if menu.opening==0 {open_now(&mut front,&mut menu,MAIN);session.paused=true;}return;}
        let panel_open=game.panels.inventory || game.panels.attributes || game.panels.char_info;
        if keys.just_pressed(KeyCode::Escape) && front.in_game && !game.opening.active && !panel_open {
            keys.clear_just_pressed(KeyCode::Escape);session.paused=true;cursor.visible=true;cursor.grab_mode=CursorGrabMode::None;request_open(&mut menu,&mut commands);
        }
        return;
    }
    // Headless captures have no pointer: MESTER_MENU_MOUSE="x,y" (design pixels) stands in for it.
    let previous=menu.mouse;
    let pos=design_pos(&window,scale.0).or(menu.fake).or_else(fake_mouse).unwrap_or(Vec2::splat(-1.0e4));
    menu.mouse=pos;let pointer_moved=pos!=previous;
    let page=front.page;
    if (page==LOAD || page==SAVE) && menu.labels.is_empty() {open_slots(&mut menu,&game,page==SAVE);}
    let dt=time.delta_secs();
    let down=buttons.pressed(MouseButton::Left);let clicked=buttons.just_pressed(MouseButton::Left);
    let can_save=game.can_save(&front);
    let loc_owned=|key:&str|game.loc(key);
    let rows=list_rows(page,&options,&session.preferences,&session.output,&loc_owned);
    let is_list=matches!(page,MAIN|PERF|SOUND|VIDEO|CONFIRM|BONUS);let pitch=list_pitch(page);
    // ---- key capture on the controls pages (0x10028a0d: keys below 70 and the mouse buttons)
    if let Some((row,column)) = menu.capture {
        if keys.just_pressed(KeyCode::Escape) {keys.clear_just_pressed(KeyCode::Escape);menu.capture=None;return;}
        if let Some(action)=keys_cfg::captured_action(&keys,&buttons) {
            let command=if page==CONTROLS1 {keys_cfg::CONTROLS1_ROWS[row]}else{keys_cfg::CONTROLS2_ROWS[row]};
            options.keys.assign(command,column,action);options.dirty=true;menu.capture=None;
        }
        return;
    }
    // ---- Esc and the back button
    let ball=inside(BACK_BUTTON,pos);
    if keys.just_pressed(KeyCode::Escape) || (clicked && ball && page!=CHAR) {
        keys.clear_just_pressed(KeyCode::Escape);
        if page==LOAD || page==SAVE {menu.listed=None;}
        back(&mut front,&mut menu,&mut session,&mut cursor);
        return;
    }
    // ---- list pages: hover follows the pointer (0x10027c30) and the keyboard (remake extension: Up / Down / Enter)
    if is_list {
        if pointer_moved || menu.hover.is_none() {menu.hover=list_hover_at(pos,pitch).filter(|row|*row<rows.len());}
        let count=rows.len();
        if keys.just_pressed(KeyCode::ArrowDown) && count>0 {menu.hover=Some(menu.hover.map_or(0,|h|(h+1)%count));}
        if keys.just_pressed(KeyCode::ArrowUp) && count>0 {menu.hover=Some(menu.hover.map_or(count-1,|h|(h+count-1)%count));}
        let activate=clicked.then(||list_row_at(pos,pitch)).flatten().filter(|row|*row<count).or_else(||keys.just_pressed(KeyCode::Enter).then_some(menu.hover).flatten());
        // The volume slider the remake adds under the sound toggles.
        if page==SOUND && clicked {if slider_at(pos,&[VOLUME_Y]).is_some() {menu.slider=Some(4);}}
        if let Some(row)=activate.filter(|_|menu.slider.is_none()) {
            let click=click_row(page,row,&mut options,&mut session.preferences,can_save);
            run_click(click,&mut front,&mut menu,&mut game,&mut session,&mut exit);
            return;
        }
    }else{menu.hover=None;}
    // ---- sliders (misc page and the volume slider): press grabs, dragging follows x, release writes the files
    if page==MISC && clicked {menu.slider=slider_at(pos,&SLIDER_Y);}
    if let Some(slider)=menu.slider {
        if down {
            let mut cfg=options.keys.clone();let mut volume=session.preferences.volume;
            if drag_slider(slider,pos.x,&mut cfg,&mut volume) {if cfg!=options.keys {options.keys=cfg;options.dirty=true;}
                if slider==4 && (volume-session.preferences.volume).abs()>1e-4 {session.preferences.volume=volume;options.dirty=true;}}
        }else{menu.slider=None;}
    }
    // ---- controls pages
    if page==CONTROLS1 || page==CONTROLS2 {
        if clicked {
            let count=if page==CONTROLS1 {keys_cfg::CONTROLS1_ROWS.len()}else{keys_cfg::CONTROLS2_ROWS.len()};
            if let Some(cell)=cell_at(pos,count) {menu.capture=Some(cell);}
            else if inside(NEXT_BUTTON,pos) {front.page=if page==CONTROLS1 {CONTROLS2}else{CONTROLS1};}
        }
    }
    // ---- load / save pages
    if page==LOAD || page==SAVE {
        let count=menu.slots.len();
        menu.slots.row=menu.slots.row.min(menu.slots.window().len().saturating_sub(1));
        if clicked {
            if let Some(row)=menu.slots.row_at(pos) {menu.slots.row=row;}
            else if inside(ACTION_BUTTON,pos) {slot_action(page,&mut front,&mut menu,&mut session,&mut cursor,&mut game);return;}
            else if inside(SCROLL_UP,pos) {menu.scroll=Some((true,SCROLL_STEP));menu.slots.scroll(-1);}
            else if inside(SCROLL_DOWN,pos) {menu.scroll=Some((false,SCROLL_STEP));menu.slots.scroll(1);}
            else if pos.x>SCROLL_X && pos.x<SCROLL_X+16.0 && pos.y>SCROLL_TOP && pos.y<SCROLL_TOP+SCROLL_TRAVEL {menu.knob_drag=true;}
        }
        if !down {menu.scroll=None;menu.knob_drag=false;}
        if let Some((up,left))=menu.scroll {
            let left=left-dt;if left<=0.0 {menu.slots.scroll(if up {-1}else{1});menu.scroll=Some((up,SCROLL_STEP));}else{menu.scroll=Some((up,left));}
        }
        if menu.knob_drag {menu.slots.drag_to(pos.y);}
        if wheel.delta.y!=0.0 {menu.slots.scroll(if wheel.delta.y>0.0 {-1}else{1});}
        // Keyboard: Up / Down move the selection through the visible window, Enter runs the page action.
        if keys.just_pressed(KeyCode::ArrowDown) {if menu.slots.row+1<menu.slots.window().len() {menu.slots.row+=1;}else{menu.slots.scroll(1);}}
        if keys.just_pressed(KeyCode::ArrowUp) {if menu.slots.row>0 {menu.slots.row-=1;}else{menu.slots.scroll(-1);}}
        if keys.just_pressed(KeyCode::Enter) && count>0 {slot_action(page,&mut front,&mut menu,&mut session,&mut cursor,&mut game);return;}
        refresh_slot(&mut menu,&game,&mut images,page);
    }
    // ---- the option files are written when a change is finished (the retail menu writes keys.cfg when a page closes)
    if options.dirty && menu.slider.is_none() && menu.capture.is_none() && !down && game.config.persist {
        let root=game.config.user.clone();
        if let Err(error)=options.save(&root) {warn!("Beállítások mentése: {error}");}
        if let Ok(text)=serde_json::to_string_pretty(&session.preferences) {let _=std::fs::write(root.join("settings.json"),text);}
    }
}

/// The shell request behind a click on a list row.
#[allow(clippy::too_many_arguments)]
fn run_click(click:Request,front:&mut Frontend,menu:&mut MenuState,game:&mut Game,session:&mut Session,exit:&mut MessageWriter<AppExit>) {
    match click {
        Request::None=>{},
        Request::Goto(page)=>{front.page=page;menu.hover=None;},
        Request::NewPlayer=>{front.draft=CharacterDraft::default();front.page=ml::CHAR;},
        Request::Load=>{if open_slots(menu,game,false) {front.page=LOAD;}},
        Request::Save=>{if open_slots(menu,game,true) {front.page=SAVE;}},
        Request::Quit=>{exit.write(AppExit::Success);},
        // A fresh game on the level: the new-game defaults of "Új játék" with the default character, the new-player page skipped.
        Request::Bonus(i)=>if let Some(item)=BONUS_ITEMS.get(i) {
            front.draft=CharacterDraft::default();front.character_started=true;session.preferences.difficulty=front.draft.difficulty;
            game.campaign.reset_requested=true;game.campaign.clear_restore();front.bonus=Some(item.world.into());game.travel.pending=Some(item.world.into());
        },
    }
}

/// The load / save action button (0x10029750 / 0x10029eb2).
fn slot_action(page:u8,front:&mut Frontend,menu:&mut MenuState,session:&mut Session,cursor:&mut CursorOptions,game:&mut Game) {
    let dir=save_dir(&game.config);
    let Some(selected)=menu.slots.selected() else {return};
    if page==LOAD {
        let Some(slot)=selected else {return};
        let Ok(file)=savefile::read(&slot.path,true) else {return};
        let Some(world)=game.campaign.request_payload(file.payload) else {warn!("A mentés nem tölthető: {}",slot.path.display());return};
        front.bonus=None;game.travel.pending=Some(world);front.main_active=false;front.in_game=true;
        return;
    }
    // Save: the "new save" entry writes the lowest free file, a slot overwrites its own (a legacy save is replaced by a numbered one).
    let path=match selected {Some(slot) if slot.kind!=savefile::Kind::Legacy=>slot.path.clone(),_=>savefile::next_free(&dir)};
    let payload=match game.campaign.snapshot_json(&game.config.world,&game.walking,&game.roster,&game.native) {Ok(p)=>p,Err(error)=>{warn!("{error}");return}};
    let health=game.campaign.health;let world=game.config.world.clone();
    let file=savefile::SaveFile {header:savefile::Header {time:savefile::SaveTime::now(),title:crate::travel::level_title(&world).to_owned(),world,health},payload,thumbnail:menu.snapshot.clone()};
    if game.config.persist {if let Err(error)=savefile::write(&path,&file) {warn!("Mentés: {error}");return;}}
    game.feedback.message=game.ui.text("SGMOk");game.feedback.message_time=MESSAGE_SECONDS;
    front.main_active=false;menu.listed=None;resume(session,cursor);
}

/// Rebuilds the drawn items whenever the scene changed, and animates the ghost labels. A change that only re-words or re-colours the texts of an
/// otherwise identical scene (the hover highlight: every pointer move over the list) edits the existing text entities in place: `draw_text` then swaps
/// the glyphs of just those labels in one command flush. Despawning and respawning the whole layer left every label empty for a frame (the new
/// entities get their glyph children one frame later), which showed as the rows blinking out whenever the pointer changed rows.
#[allow(clippy::too_many_arguments)]
pub fn render(mut commands:Commands,mut menu:ResMut<MenuState>,layer:Res<MenuLayer>,children:Query<&Children>,front:Res<Frontend>,options:Res<Options>,session:Res<Session>,ui:Res<RetailUi>,assets:Res<AssetServer>,mut texts:Query<(&mut BitmapText,&mut Node),Without<GhostText>>) {
    if !front.main_active {return;}
    let loc=|key:&str|if key.starts_with("Remake") {ml::remake_text(key).to_owned()}else{ui.text(key)};
    let capture=menu.capture;
    let scroll_held=menu.scroll.map(|(up,_)|up);
    let items=scene(&PageView {page:front.page,mouse:menu.mouse,in_game:front.in_game,hover:menu.hover,capture,scroll_held,options:&options,prefs:&session.preferences,opened:&session.output,volume:session.preferences.volume,
        slots:&menu.slots,slot_labels:&menu.labels,details:&menu.details,has_thumb:menu.thumb.is_some(),loc:&loc});
    if items==menu.shown {return;}
    if same_shape(&menu.shown,&items) && menu.spawned.len()==items.len() {
        for ((item,old),entity) in items.iter().zip(&menu.shown).zip(&menu.spawned) {
            let (Item::Text {text,pos,scale,rgb},Item::Text {..}) = (item,old) else {continue};
            let Some(Ok((mut bitmap,mut node)))=entity.map(|e|texts.get_mut(e)) else {continue};
            let color=Color::srgb_u8(rgb[0],rgb[1],rgb[2]);
            if bitmap.text!=*text {bitmap.text.clone_from(text);}
            if bitmap.color!=color {bitmap.color=color;}
            if bitmap.scale!=*scale {bitmap.scale=*scale;}
            if node.left!=px(pos.x) {node.left=px(pos.x);}
            if node.top!=px(pos.y) {node.top=px(pos.y);}
        }
        menu.shown=items;return;
    }
    if let Ok(old)=children.get(layer.0) {for child in old.iter() {commands.entity(child).despawn();}}
    let thumb=menu.thumb.clone();let mut spawned=Vec::with_capacity(items.len());
    commands.entity(layer.0).with_children(|root| {
        for item in &items {
            spawned.push(match item {
                Item::Text {text,pos,scale,rgb}=>Some(root.spawn(label("mincho",text.clone(),*scale,Color::srgb_u8(rgb[0],rgb[1],rgb[2]),pos.x,pos.y)).id()),
                Item::Ghost {text,x,row,pitch}=>{
                    let size=Vec2::new(cell_width(text,LABEL_SCALE),64.0*LABEL_SCALE);let row_y=LIST_Y+pitch*(*row as f32);let at=crate::frontend::ghost_pose(*x,row_y,size,0.0);
                    Some(root.spawn((GhostText {x:*x,row_y,size},label("mincho",text.clone(),LABEL_SCALE*1.2,Color::srgb_u8(GHOST_COLOR[0],GHOST_COLOR[1],GHOST_COLOR[2]),at.x,at.y))).id())
                },
                Item::Image {path,pos,size,crop}=>{
                    Some(root.spawn((ImageNode {image:assets.load(*path),rect:*crop,..default()},Node {position_type:PositionType::Absolute,left:px(pos.x),top:px(pos.y),width:px(size.x),height:px(size.y),..default()})).id())
                },
                Item::Thumb {pos,size}=>{
                    thumb.as_ref().map(|handle|root.spawn((ImageNode::new(handle.clone()),Node {position_type:PositionType::Absolute,left:px(pos.x),top:px(pos.y),width:px(size.x),height:px(size.y),..default()})).id())
                },
            });
        }
    });
    menu.shown=items;menu.spawned=spawned;
}
/// True when only the words / colours / places of the `Text` items differ: same length, same kind at every index, everything else equal.
fn same_shape(old:&[Item],new:&[Item])->bool {
    old.len()==new.len() && old.iter().zip(new).all(|(a,b)|match (a,b) {(Item::Text {..},Item::Text {..})=>true,_=>a==b})
}
/// The menu clock of 0x1002ae45 drives every ghost label.
pub fn ghosts(mut front:ResMut<Frontend>,time:Res<Time>,mut ghosts:Query<(&GhostText,&mut Node)>) {
    if !front.main_active || front.loading.is_some() {return;}
    front.clock=(front.clock+time.delta_secs())%std::f32::consts::TAU;
    for (ghost,mut node) in &mut ghosts {let at=crate::frontend::ghost_pose(ghost.x,ghost.row_y,ghost.size,front.clock);if node.left!=px(at.x) {node.left=px(at.x);}if node.top!=px(at.y) {node.top=px(at.y);}}
}

// ---- quick save / quick load / auto quick save (cshell 0x1005abd4 F5, 0x1005ac27 F9, 0x1005a5d0 level start)
/// The header of a save of the running game.
fn header_of(game:&Game)->savefile::Header {
    let world=game.config.world.clone();savefile::Header {time:savefile::SaveTime::now(),title:crate::travel::level_title(&world).to_owned(),world,health:game.campaign.health}
}
/// Writes `save\quick.sav` once the picture grab of this frame arrives (the thumbnail is the game picture, 0x1004b550).
fn write_after_grab(commands:&mut Commands,path:std::path::PathBuf,file:savefile::SaveFile) {
    let mut pending=Some((path,file));
    commands.spawn(Screenshot::primary_window()).observe(move |shot:On<ScreenshotCaptured>| {
        let Some((path,mut file))=pending.take() else {return};
        if let Ok(dynamic)=shot.image.clone().try_into_dynamic() {let rgba=dynamic.to_rgba8();file.thumbnail=savefile::thumbnail_from_pixels(rgba.width() as usize,rgba.height() as usize,rgba.as_raw(),false);}
        if let Err(error)=savefile::write(&path,&file) {warn!("Gyorsmentés: {error}");}
    });
}
/// Quick save of the running game; the retail message "Gyorsmentés létrehozva." shows for 5 s.
/// The quick save file of the running game: a bonus run never touches the campaign's `quick.sav`.
fn quick_path(config:&ViewerConfig,bonus:bool)->std::path::PathBuf {save_dir(config).join(if bonus {"bonus.sav"}else{"quick.sav"})}
fn quick_save(game:&mut Game,commands:&mut Commands,bonus:bool)->bool {
    let payload=match game.campaign.snapshot_json(&game.config.world,&game.walking,&game.roster,&game.native) {Ok(p)=>p,Err(error)=>{warn!("{error}");return false}};
    let file=savefile::SaveFile {header:header_of(game),payload,thumbnail:None};
    if game.config.persist {write_after_grab(commands,quick_path(&game.config,bonus),file);}
    game.feedback.message=game.ui.text("GameShell2");game.feedback.message_time=MESSAGE_SECONDS;true
}
#[derive(Default)] pub struct QuickLocal {seen_arrival:Option<u64>,timer:f32}
/// F5 saves, F9 loads `quick.sav`, F10 asks to quit, and every new level start saves by itself when the option is on.
#[allow(clippy::too_many_arguments)]
pub fn quick(keys:Res<ButtonInput<KeyCode>>,mut front:ResMut<Frontend>,mut menu:ResMut<MenuState>,options:Res<Options>,mut session:ResMut<Session>,mut game:Game,time:Res<Time>,mut local:Local<QuickLocal>,mut commands:Commands) {
    if front.loading.is_some() || game.video.active {return;}
    let playing=front.in_game && !front.main_active && menu.opening==0 && !game.opening.active;
    // F10 (the retail Quit action) leads to the confirmation.
    if keys.just_pressed(KeyCode::F10) && !game.opening.active {open_now(&mut front,&mut menu,CONFIRM);session.paused=true;}
    if keys.just_pressed(KeyCode::F5) && playing && !game.campaign.dead() {
        // 0x1005abd4: a kneeling player may not save, the shell says so instead.
        if game.walking.player.crouched {game.feedback.message=game.ui.text("GameShell1");game.feedback.message_time=MESSAGE_SECONDS;}
        else {quick_save(&mut game,&mut commands,front.bonus.is_some());}
    }
    if keys.just_pressed(KeyCode::F9) && front.in_game && !game.opening.active {
        let path=quick_path(&game.config,front.bonus.is_some());
        if let Some(world)=savefile::read(&path,true).ok().and_then(|file|game.campaign.request_payload(file.payload)) {
            game.travel.pending=Some(world);front.main_active=false;
        }
    }
    // Level start (0x1005a5d0): CAutoQuickSave, not entered from a save, not during a cutscene.
    if local.seen_arrival!=Some(game.travel.arrived) {
        if front.in_game && !session.paused {local.timer+=time.delta_secs();}
        if local.timer>=0.75 && playing {
            local.seen_arrival=Some(game.travel.arrived);local.timer=0.0;
            if options.auto_quick_save && !game.campaign.from_save && !game.campaign.dead() {quick_save(&mut game,&mut commands,front.bonus.is_some());}
        }
    }
}

#[cfg(test)] mod tests {
    use super::*;
    fn window(width:f32,height:f32,cursor:Vec2)->Window {let mut w=Window::default();w.resolution.set(width,height);w.set_cursor_position(Some(cursor));w}
    /// The canvas is 1024x768 design pixels scaled by `UiScale` and centred: the pointer maps back to design space.
    #[test] fn the_pointer_maps_into_the_centred_scaled_canvas() {
        // 1280x720: scale 0.9375, the canvas is 960x720 with 160 px bars left and right.
        let scale=(720.0f32/768.0).min(1280.0/1024.0);
        assert_eq!(design_pos(&window(1280.0,720.0,Vec2::new(160.0,0.0)),scale),Some(Vec2::ZERO));
        let centre=design_pos(&window(1280.0,720.0,Vec2::new(640.0,360.0)),scale).unwrap();assert!((centre-Vec2::new(512.0,384.0)).length()<1e-3);
        let corner=design_pos(&window(1280.0,720.0,Vec2::new(1119.0,719.0)),scale).unwrap();assert!((corner-Vec2::new(959.0/0.9375,719.0/0.9375)).length()<1e-3,"{corner:?}");
        // 4:3 windows have no bars: 1024x768 maps 1:1, 2048x1536 at scale 2.
        assert_eq!(design_pos(&window(1024.0,768.0,Vec2::new(340.0,300.0)),1.0),Some(Vec2::new(340.0,300.0)));
        assert_eq!(design_pos(&window(2048.0,1536.0,Vec2::new(680.0,600.0)),2.0),Some(Vec2::new(340.0,300.0)));
        assert_eq!(Window::default().cursor_position(),None);
    }
    /// The hover highlight only re-colours texts: that edit must keep the entities (a respawned layer is blank for a frame), a page change must not.
    #[test] fn a_recoloured_scene_is_edited_in_place_and_a_different_one_is_rebuilt() {
        let text=|t:&str,rgb:[u8;3]|Item::Text {text:t.into(),pos:Vec2::ZERO,scale:0.3,rgb};
        let ghost=|t:&str|Item::Ghost {text:t.into(),x:340.0,row:0,pitch:73.0};
        let (a,b)=(vec![ghost("Új"),text("Új",[1,2,3])],vec![ghost("Új"),text("Új",[255,224,0])]);
        assert!(same_shape(&a,&b));
        assert!(!same_shape(&a,&[ghost("Más"),text("Új",[1,2,3])]),"another ghost is another page");
        assert!(!same_shape(&a,&a[..1]) && !same_shape(&a,&[text("Új",[1,2,3]),ghost("Új")]));
    }
    #[test] fn slot_labels_use_the_level_title_and_fall_back_to_the_world() {
        let mut header=savefile::Header {world:"knajpa".into(),title:"A Bullseye Kocsma".into(),..default()};
        assert_eq!(slot_label(&header),"A Bullseye Kocsma");header.title.clear();assert_eq!(slot_label(&header),"knajpa");
    }
}
