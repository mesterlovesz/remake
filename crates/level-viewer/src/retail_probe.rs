//! Silent regression using real exported pickups, NPC drops, weapon state across levels and the grenade (world rh1-wiezienie2).
//! Retail has no use key: an item is taken when it touches the player's box or the cube ahead (0x10022470, 0x10022360), so the
//! script teleports the player onto an item and waits until it is gone. Every step waits for game state (see probe_kit.rs).
use bevy::prelude::*;
use crate::{ViewerConfig,Walking,settings::Session,campaign::Campaign,gunfire::Controls,retail_weapons::NativeArsenal,pickups::Pickup,frontend::Frontend,probe_kit::{Cursor,Step,arrived}};
#[derive(Resource,Default)] pub struct Probe {pub active:bool,pub finished:bool,pub failure:Option<String>,elapsed:f32,cursor:Cursor,base:u32,mark:u32,reserve:u32,action:f32,target:String}
pub fn setup(mut commands:Commands,config:Res<ViewerConfig>) {commands.insert_resource(Probe {active:config.capture.is_some() && std::env::var("MESTER_TEST_SCENARIO").as_deref()==Ok("retail"),..default()});}
/// Puts the player's box on the first grounded item of that kind; returns its name.
fn touch(items:&Query<&Pickup>,kind:&str,walking:&mut Walking)->Option<String> {
    let item=items.iter().find(|i|i.kind==kind && !i.in_flight())?;
    walking.player.position=retail_movement::Vec3::new(item.position.x,item.position.y,item.position.z);walking.player.velocity=retail_movement::Vec3::ZERO;
    Some(item.name.clone())
}
fn count(items:&Query<&Pickup>,kind:&str)->u32 {items.iter().filter(|i|i.kind==kind).count() as u32}
fn gone(items:&Query<&Pickup>,name:&str)->bool {items.iter().all(|i|i.name!=name)}
fn slot(native:&NativeArsenal,id:&str)->usize {native.definitions.iter().position(|d|d.id==id).unwrap_or_else(||panic!("weapon {id} missing from the catalog"))}
fn ammo(native:&NativeArsenal,id:&str)->(u32,u32) {let s=slot(native,id);(native.inventory.weapons[s].magazine,native.inventory.reserve(&native.definitions,s))}
fn holding(native:&NativeArsenal,id:&str)->bool {native.inventory.equipped && !native.inventory.switching() && native.inventory.selected==Some(slot(native,id))}
/// Retail draws a weapon only from its number key (0x10060c8f), never on pickup: the key of the holster cell that holds `kind`.
fn key_of(campaign:&Campaign,kind:&str)->Option<usize> {campaign.items.entries.iter().find(|e|e.item==kind && e.row>=crate::inventory::HOLSTER_ROW && e.row<crate::inventory::BELT_ROW).map(|e|(e.column+crate::inventory::COLUMNS*(e.row-crate::inventory::HOLSTER_ROW)) as usize)}
/// Presses the number key of `kind` until it is in the hand; false when the weapon is not in a holster cell.
fn draw(controls:&mut Controls,campaign:&Campaign,native:&NativeArsenal,kind:&str)->Option<bool> {
    if holding(native,kind) {return Some(true);}
    if !native.inventory.switching() {controls.native_slot=Some(key_of(campaign,kind)?);}
    Some(false)
}
/// One trigger pulse per frame while the weapon is free, until the shot counter passes `target`.
fn pulse(controls:&mut Controls,native:&NativeArsenal,target:u32)->bool {
    if native.shots>=target {return true;}
    if native.inventory.selected.is_some_and(|s|native.inventory.weapons[s].cooldown<=0.0) && !native.reloading() && !native.inventory.switching() {controls.fire=true;}
    false
}
/// Ammunition a pickup type adds, capped like the game (0x10066248).
fn capped(index:usize,amount:u32)->u32 {amount.min(crate::pickups::AMMO_MAX[index])}
#[allow(clippy::too_many_arguments)]
pub fn tick(mut probe:ResMut<Probe>,time:Res<Time>,mut controls:ResMut<Controls>,mut walking:ResMut<Walking>,mut session:ResMut<Session>,mut campaign:ResMut<Campaign>,mut native:ResMut<NativeArsenal>,items:Query<&Pickup>,roster:Res<crate::npcs::NpcRoster>,mut doors:ResMut<crate::doors::DoorUse>,config:Res<ViewerConfig>,front:Res<Frontend>,volume:Res<GlobalVolume>,shields:Query<&crate::doors::Door>) {
    if !probe.active || probe.finished {return;}
    // Focused interaction test, not a survival walkthrough: prevent NPC damage from obscuring weapon assertions.
    campaign.health=100.0;session.paused=false;probe.elapsed+=time.delta_secs();*controls=Controls::default();
    let now=probe.elapsed;let p=&mut *probe;
    let step=match p.cursor.stage {
        // 0..1: the shotgun lies in the cell block; touching it takes it and puts it in the hand.
        0=>{if p.cursor.first() {match touch(&items,"FN shotgun",&mut walking) {Some(name)=>p.target=name,None=>{p.failure=Some("no FN shotgun pickup in the level".into());p.finished=true;return;}}}
            Step::when(gone(&items,&p.target),"the FN pickup to be taken")},
        1=>{
            let (magazine,_)=ammo(&native,"FN shotgun");
            if native.inventory.selected.is_some() || native.inventory.equipped {Step::Fail("touching the FN drew it (retail draws only from the number key)".into())}
            else if magazine!=8 {Step::Fail(format!("FN magazine after the pickup: {magazine}"))}
            else {Step::Done}
        },
        // The number key of its holster cell draws it; then the player goes to the guard.
        2=>match draw(&mut controls,&campaign,&native,"FN shotgun") {
            None=>Step::Fail("the FN is in no holster cell".into()),
            Some(false)=>Step::Wait("the FN to be drawn"),
            Some(true)=>if let Some(guard)=roster.actors.iter().find(|a|a.name=="o_postac22") {
                // A spot with a clear bullet line (no wall, no closed door): the blast must reach him.
                if crate::campaign_probe::place_with_clear_shot(&mut walking,guard.eye(),&shields,0) {p.base=native.shots;Step::Done}else{Step::Wait("a clear shot at the guard o_postac22")}
            }else{Step::Fail("guard o_postac22 missing".into())},
        },
        // 3..5: one shotgun blast kills the guard, whose death phase drops his Glock; touching it selects the Glock.
        3=>Step::when(pulse(&mut controls,&native,p.base+1),"the FN shot"),
        4=>Step::when(items.iter().any(|i|i.kind=="Glock" && i.name.starts_with("drop:")),"the guard's dropped Glock"),
        // The dropped weapon falls first (an item in flight is not taken): touch it once it lies.
        5=>{if p.cursor.first() {p.target.clear();}if p.target.is_empty() {p.target=touch(&items,"Glock",&mut walking).unwrap_or_default();}
            Step::when(!p.target.is_empty() && gone(&items,&p.target),"the dropped Glock to be taken")},
        // The pickup leaves the FN in the hand; the Glock's number key draws it.
        6=>{let (magazine,_)=ammo(&native,"Glock");
            if p.cursor.first() && holding(&native,"Glock") {Step::Fail("touching the Glock drew it (retail draws only from the number key)".into())}
            else if magazine!=17 {Step::Fail(format!("Glock magazine after the pickup: {magazine}"))}
            else {match draw(&mut controls,&campaign,&native,"Glock") {None=>Step::Fail("the Glock is in no holster cell".into()),Some(ready)=>Step::when(ready,"the Glock to be drawn")}}},
        // 6..7: two Glock shots (the second waits for the 0.3 s cooldown).
        7=>{if p.cursor.first() {p.base=native.shots;}Step::when(pulse(&mut controls,&native,p.base+1),"the first Glock shot")},
        8=>Step::when(pulse(&mut controls,&native,p.base+2),"the second Glock shot"),
        // 8..9: Glock ammunition lies in pairs: touching one takes what touches the player; each item is one magazine (17 rounds).
        9=>{let (magazine,_)=ammo(&native,"Glock");
            if magazine!=15 {Step::Fail(format!("Glock magazine after two shots: {magazine}"))}
            else {if p.cursor.first() {p.mark=count(&items,"Glock ammo");p.target=touch(&items,"Glock ammo",&mut walking).unwrap_or_default();}
                Step::when(!p.target.is_empty() && gone(&items,&p.target),"the Glock ammunition to be taken")}},
        10=>{let taken=p.mark-count(&items,"Glock ammo");let (_,reserve)=ammo(&native,"Glock");p.reserve=reserve;p.base=native.reloads;
            Step::check(taken>=1 && reserve==capped(0,17*taken),||format!("Glock reserve {reserve} after taking {taken} ammunition items"))},
        // 10..12: the reload starts, does not advance while the game is paused, then completes.
        11=>{if native.reloads==p.base {controls.reload=true;}Step::when(native.reloads>p.base,"the Glock reload to start")},
        12=>{
            if p.cursor.first() {p.action=native.inventory.action.elapsed;}
            if p.cursor.age(now)<1.0 {session.paused=true;Step::Wait("the pause window")}
            else {Step::check(native.inventory.action.elapsed==p.action,||format!("the reload advanced while paused: {} -> {}",p.action,native.inventory.action.elapsed))}
        },
        13=>{let (magazine,reserve)=ammo(&native,"Glock");
            if native.reloading() {Step::Wait("the Glock reload to finish")}else{Step::check((magazine,reserve)==(17,p.reserve-2),||format!("Glock magazine/reserve after the reload: ({magazine},{reserve}), expected (17,{})",p.reserve-2))}},
        // 13..15: the shotgun's Mossberg ammunition (20 rounds each) and its shell-by-shell reload of the one missing shell.
        14=>{
            if p.cursor.first() {p.mark=count(&items,"Mossberg ammo");p.reserve=ammo(&native,"FN shotgun").1;p.target=touch(&items,"Mossberg ammo",&mut walking).unwrap_or_default();}
            let drawn=draw(&mut controls,&campaign,&native,"FN shotgun").unwrap_or(false);
            Step::when(!p.target.is_empty() && gone(&items,&p.target) && drawn,"the Mossberg ammunition to be taken with the FN drawn by its key")},
        15=>{let taken=p.mark-count(&items,"Mossberg ammo");let (_,reserve)=ammo(&native,"FN shotgun");p.base=native.reloads;
            if taken>=1 && reserve==capped(5,p.reserve+20*taken) {p.reserve=reserve;Step::Done}else{Step::Fail(format!("FN reserve {reserve} after taking {taken} Mossberg items (was {})",p.reserve))}},
        16=>{if native.reloads==p.base {controls.reload=true;}Step::when(native.reloads>p.base,"the FN reload to start")},
        17=>{let (magazine,reserve)=ammo(&native,"FN shotgun");
            if native.reloading() {Step::Wait("the FN reload to finish")}else{Step::check((magazine,reserve)==(8,p.reserve-1),||format!("FN magazine/reserve after the reload: ({magazine},{reserve}), expected (8,{})",p.reserve-1))}},
        // 17..19: the weapons survive both exits (rh1-wiezienie3, then rh2-wiezienie1).
        18=>{if p.cursor.first() {crate::campaign_probe::place(&mut walking,Vec3::new(-1232.0,-190.0,190.0),Vec3::new(-1232.0,-176.0,80.0));doors.request_name=Some("b_door21".into());}
            Step::when(arrived(&config,&front,&campaign,"rh1-wiezienie3"),"the first exit to load rh1-wiezienie3")},
        19=>{
            let (magazine,_)=ammo(&native,"Glock");
            if magazine!=17 || !native.inventory.weapons[slot(&native,"FN shotgun")].owned {Step::Fail(format!("the level change lost a weapon (Glock magazine {magazine})"))}
            else {crate::campaign_probe::place(&mut walking,Vec3::new(2500.0,-48.0,-144.0),Vec3::new(2620.0,-48.0,-144.0));doors.request_name=Some("b_door0".into());Step::Done}
        },
        20=>Step::when(arrived(&config,&front,&campaign,"rh2-wiezienie1"),"the second exit to load rh2-wiezienie1"),
        // 20..23: six SIGs lie side by side; the first taken gives the weapon (30 rounds) and selects it, the others add rounds only.
        21=>{if p.cursor.first() {p.mark=count(&items,"Sig 551-p/SWAT");p.target=touch(&items,"Sig 551-p/SWAT",&mut walking).unwrap_or_default();}
            Step::when(!p.target.is_empty() && gone(&items,&p.target),"the SIG pickup to be taken")},
        22=>{let taken=p.mark-count(&items,"Sig 551-p/SWAT");let (magazine,reserve)=ammo(&native,"Sig 551-p/SWAT");p.base=native.shots;
            if p.cursor.first() && holding(&native,"Sig 551-p/SWAT") {Step::Fail("touching the SIG drew it (retail draws only from the number key)".into())}
            else if !(taken>=1 && magazine==30 && reserve==capped(2,30*(taken-1))) {Step::Fail(format!("SIG after touching {taken} copies: magazine={magazine} reserve={reserve}"))}
            else {match draw(&mut controls,&campaign,&native,"Sig 551-p/SWAT") {None=>Step::Fail("the SIG is in no holster cell".into()),Some(ready)=>Step::when(ready,"the SIG to be drawn")}}},
        23=>Step::when(pulse(&mut controls,&native,p.base+1),"the SIG shot"),
        24=>{let (magazine,reserve)=ammo(&native,"Sig 551-p/SWAT");
            if magazine!=29 {Step::Fail(format!("SIG magazine after one shot: {magazine}"))}
            else if reserve==0 {Step::Done}
            else {p.base=native.reloads;p.reserve=reserve;Step::Done}},
        25=>{let (_,reserve)=ammo(&native,"Sig 551-p/SWAT");
            if reserve==0 {Step::Done} else {
                if native.reloads==p.base {controls.reload=true;}
                if native.reloads>p.base && !native.reloading() {let (magazine,after)=ammo(&native,"Sig 551-p/SWAT");Step::check((magazine,after)==(30,p.reserve-1),||format!("SIG magazine/reserve after the reload: ({magazine},{after}), expected (30,{})",p.reserve-1))}
                else {Step::Wait("the SIG reload")}}},
        // 25..29: a grenade (touching one takes the weapon and readies it), the pin, one second held, the throw and the blast.
        26=>{if p.cursor.first() {p.target=touch(&items,"Hand grenade",&mut walking).unwrap_or_default();}
            Step::when(!p.target.is_empty() && gone(&items,&p.target),"the grenade pickup to be taken")},
        27=>{
            match draw(&mut controls,&campaign,&native,"Hand grenade") {
                None=>Step::Fail("the grenade is in no holster cell".into()),
                Some(false)=>Step::Wait("the grenade to be drawn"),
                Some(true)=>{if !native.inventory.armed_grenade() {controls.fire=true;}controls.held=true;Step::when(native.inventory.armed_grenade(),"the grenade pin to be pulled")}
            }
        },
        28=>{controls.held=true;Step::when(p.cursor.age(now)>=1.0,"the grenade to be held")},
        29=>Step::when(native.grenade_throws>=1,"the grenade throw (trigger released)"),
        30=>Step::when(native.explosions>=1,"the grenade blast after its fuse"),
        _=>{
            p.finished=true;session.paused=true;
            let summary=(native.shots,native.reloads,native.grenade_throws,native.explosions);
            if summary!=(5,3,1,1) || native.hits<3 || native.pose_updates<20 || volume.volume!=bevy::audio::Volume::Linear(0.0) {p.failure=Some(format!("weapon totals (shots,reloads,throws,blasts)={summary:?} expected (5,3,1,1); hits={} poses={}",native.hits,native.pose_updates));}
            info!("GYÁRI PRÓBA kész: {} shots={} hits={} reloads={} failure={:?}",config.world,native.shots,native.hits,native.reloads,p.failure);return;
        },
    };
    if p.cursor.apply(step,now,20.0,&mut p.failure) {p.finished=true;}
}
