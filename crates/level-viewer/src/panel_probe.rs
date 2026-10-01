//! Silent capture aid: MESTER_TEST_SCENARIO=inventory fills the pack, then shows the C, X and Z screens
//! (MESTER_PANEL_SET=basic gives apple + nightstick + Glock, the owner's 1.28 screenshot).
use bevy::{prelude::*,window::CursorOptions};
use crate::{ViewerConfig,Walking,InspectionCamera,SCALE,settings::Session,campaign::Campaign,pickups::{DropQueue,Pickup},panels::Panels,retail_ui::RetailUi};
#[derive(Resource,Default)] pub struct Probe {pub active:bool,pub finished:bool,pub failure:Option<String>,elapsed:f32,stage:usize,apples:u32,grabbed:bool,base_weight:f32}
pub fn setup(mut commands:Commands,config:Res<ViewerConfig>) {commands.insert_resource(Probe {active:config.capture.is_some() && std::env::var("MESTER_TEST_SCENARIO").as_deref()==Ok("inventory"),..default()});}
pub fn tick(mut probe:ResMut<Probe>,time:Res<Time>,mut keys:ResMut<ButtonInput<KeyCode>>,mut campaign:ResMut<Campaign>,mut drops:ResMut<DropQueue>,mut session:ResMut<Session>,panels:Res<Panels>,ui:Res<RetailUi>,camera:Single<&Transform,With<InspectionCamera>>,items:Query<&Pickup>,mut walking:ResMut<Walking>,mut cursor:Single<&mut CursorOptions>) {
    if !probe.active || probe.finished {return;}
    // A focused UI check, not a survival run: nothing in the cell may interrupt the screens.
    campaign.health=campaign.max_health;session.paused=false;probe.elapsed+=time.delta_secs();
    // Leveling accepts one threshold per gain, so experience arrives in steps like real kills.
    campaign.experience=match probe.elapsed {t if t>5.0=>5350,t if t>3.0=>3000,t if t>1.0=>1200,_=>0};
    let at=[1.0,2.0,4.0,5.0,7.0,8.0,10.0,11.0,12.5,14.0,15.0,16.0,17.0,18.0];
    if probe.stage>=at.len() || probe.elapsed<at[probe.stage] {return;}
    let stage=probe.stage;probe.stage+=1;
    for key in [KeyCode::KeyC,KeyCode::KeyX,KeyCode::KeyZ,KeyCode::Escape] {keys.release(key);}
    let basic=std::env::var("MESTER_PANEL_SET").as_deref()==Ok("basic");
    let error=match stage {
        0=>{
            // The click-to-play lock, so the release / lock-again hand-over below has something to restore.
            cursor.visible=false;cursor.grab_mode=bevy::window::CursorGrabMode::Locked;probe.base_weight=campaign.items.weight(&ui.catalog);
            let set:&[(&str,u32)]=if basic {&[("an apple",1),("Police nightstick",1),("Glock",1)]}else{&[("an apple",1),("Police nightstick",1),("Glock",1),("chicken drumstick",3),("cereal box",2),("small syringe",4),("Sig 551-p/SWAT",1),("MAC-10 Ingram",1),("FN shotgun",1),("vodka bottle",2),("medpack",1),("Golden cat.",1),("burger",1),("pain killer",2),("beef steak",1),("a can of coke",1)]};
            for (item,count) in set {for _ in 0..*count {drops.grants.push((*item).into());}}
            campaign.kills=17;campaign.stats.skills[0]=5.5;campaign.stats.skills[1]=0.8;None
        },
        // A hidden headless window may refuse the grab; the lock-again check only applies when it took.
        1=>{probe.grabbed=cursor.grab_mode==bevy::window::CursorGrabMode::Locked;keys.press(KeyCode::KeyC);None},
        // The cell's own cereal box and apple are picked up on entry and count too (apple + nightstick + Glock alone weigh 1.28).
        2=>{let expected=if basic {1.28+probe.base_weight}else{29.38+probe.base_weight};let weight=campaign.items.weight(&ui.catalog);
            (!panels.inventory || (basic && (weight-expected).abs()>0.01)).then(||format!("Inventory panel closed or weight {weight} != {expected}"))
                // The inventory takes the real OS cursor: shown and released, the drawn virtual one is gone.
                .or_else(||(!panels.mouse_mode() || !cursor.visible || cursor.grab_mode!=bevy::window::CursorGrabMode::None).then(||format!("Inventory open but the OS cursor is not free: mode={} visible={} grab={:?}",panels.mouse_mode(),cursor.visible,cursor.grab_mode)))},
        3=>{keys.press(KeyCode::KeyC);keys.press(KeyCode::KeyX);None},
        4=>(!panels.attributes).then(||"X screen did not open".into()),
        5=>{keys.press(KeyCode::KeyX);keys.press(KeyCode::KeyZ);None},
        6=>(!panels.char_info).then(||"Z screen did not open".into()),
        // Drag-out: one apple leaves at eye + 16*d with velocity 480*d and must land as a pickup.
        7=>{keys.press(KeyCode::KeyZ);probe.apples=campaign.items.count("an apple");
            let cell=campaign.items.entries.iter().find(|e|e.item=="an apple").map(|e|(e.column,e.row));
            cell.and_then(|cell|campaign.items.take_one(cell)).map(|_|{let d=*camera.forward();drops.thrown.push(("an apple".into(),camera.translation/SCALE+d*16.0,d*480.0));None}).unwrap_or(Some("No apple to throw".into()))},
        8=>{let count=campaign.items.count("an apple");let thrown=items.iter().find(|i|i.name.starts_with("thrown:"));
            if thrown.is_some_and(|i|i.in_flight()) {probe.stage-=1;return;}
            match thrown {Some(item) if count+1==probe.apples => {walking.player.position=retail_movement::Vec3::new(item.position.x,item.position.y+24.0,item.position.z);None},_=>Some(format!("Thrown apple missing: apples left {count}, pickup {}",thrown.is_some()))}},
        9=>{let count=campaign.items.count("an apple");(count!=probe.apples).then(||format!("Thrown apple was not taken back: {count} of {}",probe.apples))
                .or_else(||(panels.mouse_mode() || (probe.grabbed && cursor.visible)).then(||format!("All panels closed but the cursor was not locked again: mode={} visible={}",panels.mouse_mode(),cursor.visible)))},
        // Escape closes one panel per press (inventory, then character info) before it may open the menu; character info alone leaves the mouse look alone.
        10=>{keys.press(KeyCode::KeyC);keys.press(KeyCode::KeyZ);None},
        11=>{keys.press(KeyCode::Escape);(!panels.inventory || !panels.char_info || !panels.mouse_mode()).then(||"Inventory + character info were not both open".into())},
        12=>{keys.press(KeyCode::Escape);(panels.inventory || !panels.char_info || panels.mouse_mode() || (probe.grabbed && cursor.visible)).then(||format!("First Escape must close the inventory only (inventory={} char_info={} mode={} visible={})",panels.inventory,panels.char_info,panels.mouse_mode(),cursor.visible))},
        _=>{probe.finished=true;(panels.inventory || panels.char_info || panels.attributes || panels.mouse_mode()).then(||"Second Escape must close the character info".into())},
    };
    if let Some(error)=error {error!("{error}");probe.failure.get_or_insert(error);}
    info!("PANELPRÓBA {stage}: inventory={} attributes={} char_info={} items={:?} failure={:?}",panels.inventory,panels.attributes,panels.char_info,campaign.items.names(),probe.failure);
}
