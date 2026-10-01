//! Silent sound-parity probe (`MESTER_TEST_SCENARIO=sounds`, capture runs only, docs/retail-audio-parity.md): drives the real game systems and asserts on the
//! cues they ask for (`audio::Cues`, also printed with `MESTER_CUE_LOG=1`), never on anything audible: jump, landing, footsteps, heartbeat, the three hero
//! grunts, the level music and its switch, every weapon's shot and reload, the melee swing and the grenade blast. Exit code 1 on any failure.
use bevy::{prelude::*,window::{CursorGrabMode,CursorOptions}};
use crate::{ViewerConfig,Walking,audio::Cues,campaign::Campaign,probe_kit::{Cursor,Step},retail_weapons::NativeArsenal,settings::Session};

#[derive(Resource,Default)] pub struct Probe {pub active:bool,pub finished:bool,pub failure:Option<String>}
pub fn setup(mut commands:Commands,config:Res<ViewerConfig>) {commands.insert_resource(Probe {active:config.capture.is_some() && std::env::var("MESTER_TEST_SCENARIO").as_deref()==Ok("sounds"),..default()});}

/// The guns in the order they are tried (items.txt titles), then the melee weapon and the grenade.
const GUNS:[&str;8]=["Glock","Smith and Wesson m. 625","Sig 551-p/SWAT","MAC-10 Ingram","FN shotgun","M-14","HK G8","P90"];
const FIRST_WEAPON_STAGE:usize=20;
/// Stages per weapon: acquire and draw, shoot, reload, wait for the reload to end.
const PER_WEAPON:usize=4;

#[derive(Default)] pub struct Script {cursor:Cursor,now:f32,base:usize,summary:Vec<String>,/// When the grenade was drawn: the throw starts then.
    drawn:Option<f32>}

/// Bevy systems take at most 16 parameters: the read-mostly handles of the script.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Aux<'w,'s> {
    options:ResMut<'w,crate::options::Options>,music:Res<'w,crate::music::Music>,view:Res<'w,crate::view::ViewState>,effects:Res<'w,crate::gunfire::Effects>,
    camera:Single<'w,'s,&'static Transform,With<crate::InspectionCamera>>,cursor:Single<'w,'s,&'static mut CursorOptions>,
}

#[allow(clippy::too_many_arguments)]
pub fn tick(mut probe:ResMut<Probe>,mut script:Local<Script>,time:Res<Time>,mut keys:ResMut<ButtonInput<KeyCode>>,mut walking:ResMut<Walking>,mut campaign:ResMut<Campaign>,cues:Res<Cues>,mut native:ResMut<NativeArsenal>,
    mut controls:ResMut<crate::gunfire::Controls>,mut aux:Aux,mut commands:Commands,mut session:ResMut<Session>) {
    let Aux {options,music,view,effects,camera,cursor}=&mut aux;
    let (music,view,effects,camera)=(&**music,&**view,&**effects,&**camera);
    if !probe.active || probe.finished || probe.failure.is_some() {return;}
    session.paused=false;cursor.visible=false;cursor.grab_mode=CursorGrabMode::Locked;
    script.now+=time.delta_secs();let now=script.now;
    campaign.max_health=campaign.max_health.max(100.0);
    let (player_grounded,player_vy)=(walking.player.grounded,walking.player.velocity.y);
    let count=|part:&str|cues.heard(part);
    let stage=script.cursor.stage;
    let base=script.base;
    let weapons=FIRST_WEAPON_STAGE+GUNS.len()*PER_WEAPON;
    let step=match stage {
        // The level is live and the player stands on the floor (falls and health changes count only after frame 60, cshell 0x10061026).
        0=>Step::when(player_grounded && view.level_frames()>70,"the player on the ground after frame 70"),
        // The level music starts on frame 78 (cshell 0x1005a583).
        1=>Step::when(count("music:play ")>0 && music.playing().is_some(),"the level music (music:play ...)"),
        2=>{script.base=count("skok.wav");keys.press(KeyCode::Space);Step::Done},
        3=>{if count("skok.wav")>base {keys.release(KeyCode::Space);script.summary.push("jump skok.wav".into());Step::Done}else{Step::Wait("the jump sound skok.wav")}},
        4=>Step::when(player_grounded && player_vy.abs()<1.0,"the landing"),
        // A fall of 700 units: (takeoff - landing - 196) x 0.15 health and spad.wav (cshell 0x1006102f..0x10061064).
        5=>{
            script.base=count("spad.wav");campaign.health=100.0;
            let Some(above)=free_column(&walking) else {return fail(&mut probe,"no floor with 700 free units above it near the player".into())};
            walking.player.position=above;walking.player.velocity=retail_movement::Vec3::ZERO;Step::Done},
        6=>{if count("spad.wav")>base {script.summary.push("landing spad.wav".into());Step::Done}else{Step::Wait("the landing sound spad.wav")}},
        7=>Step::when(player_grounded && player_vy.abs()<1.0,"the landing after the fall"),
        // Health crossing 50 and 20 plays serducho.wav once each (cshell 0x10061ba0).
        8=>{campaign.health=100.0;script.base=count("serducho");Step::Done},
        9=>{campaign.health=45.0;Step::Done},
        10=>{campaign.health=15.0;Step::Done},
        11=>{if count("serducho")>=base+2 {campaign.health=100.0;script.summary.push("heartbeat x2".into());Step::Done}else{Step::Wait("two heartbeats")}},
        // Running on the ground steps every 0.4 s (cshell 0x1006131b).
        12=>{script.base=count("hero/krok");keys.press(KeyCode::KeyW);keys.press(KeyCode::ShiftLeft);Step::Done},
        13=>{if count("hero/krok")>base {keys.release(KeyCode::KeyW);keys.release(KeyCode::ShiftLeft);script.summary.push("footstep KROK*".into());Step::Done}else{Step::Wait("a footstep")}},
        // The three wounds: a bullet is a 3D sound of radius 640 at the hit, a shove a 2D weapons\wcialo.wav, a bite is silent.
        14=>{script.base=count("hero/wcialo.wav");
            let at=Vec3::new(walking.player.position.x+100.0,walking.player.position.y,walking.player.position.z);
            crate::gunfire::player_hit(&mut commands,&effects,&camera,at,crate::gunfire::Grunt::Bullet(at));Step::Done},
        15=>{
            let bullet=cues.list.iter().rev().find(|cue|cue.path.contains("hero/wcialo.wav"));
            if count("hero/wcialo.wav")<=base {Step::Fail("the bullet wound made no speech/hero/wcialo.wav".into())}
            else if !bullet.is_some_and(|cue|cue.radius==Some(crate::gunfire::GRUNT_RADIUS) && cue.heard) {Step::Fail(format!("the bullet wound is not a 3D sound of radius 640: {bullet:?}"))}
            else {script.base=count("weapons/wcialo.wav");
                crate::gunfire::player_hit(&mut commands,&effects,&camera,Vec3::ZERO,crate::gunfire::Grunt::Shove);Step::Done}},
        16=>{
            let shove=cues.list.iter().rev().find(|cue|cue.path.contains("weapons/wcialo.wav"));
            if count("weapons/wcialo.wav")<=base {Step::Fail("the shove made no weapons/wcialo.wav".into())}
            else if shove.is_some_and(|cue|cue.radius.is_some()) {Step::Fail("the shove is not a 2D sound".into())}
            else {script.base=count("wcialo");crate::gunfire::player_hit(&mut commands,&effects,&camera,Vec3::ZERO,crate::gunfire::Grunt::Silent);Step::Done}},
        17=>Step::check(count("wcialo")==base,||"a bite must be silent".to_owned()),
        18=>{script.summary.push("wounds: bullet 3D 640 / shove 2D / bite silent".into());Step::Done},
        // The level music stops with the Music switch and stays silent when it is switched on again (cshell 0x10059950 / 0x10059230).
        19=>{options.music=false;Step::when(music.playing().is_none() && count("music:stop")>0,"the music to stop with the Music switch")},
        // Weapons: FIRST_WEAPON_STAGE.. four stages per gun (draw, shoot, reload, reload over), then the baton, then the grenade.
        s if (FIRST_WEAPON_STAGE..weapons).contains(&s)=>{
            let (gun,sub)=((s-FIRST_WEAPON_STAGE)/PER_WEAPON,(s-FIRST_WEAPON_STAGE)%PER_WEAPON);
            let name=GUNS[gun];
            let slot=native.definitions.iter().position(|d|d.id.eq_ignore_ascii_case(name));
            let Some(slot)=slot else {return fail(&mut probe,format!("no weapon {name}"))};
            let sound=|key:&str|native.definitions[slot].sounds.get(key).cloned().flatten().map(|path|path.rsplit('/').next().unwrap_or("").to_owned());
            match sub {
                0=>{
                    if script.cursor.first() {
                        options.music=true;let ammo=native.definitions[slot].ammo_index;native.acquire(name);native.inventory.add_ammo(ammo,60);native.inventory.select(slot);
                    }
                    Step::when(native.inventory.selected==Some(slot) && native.inventory.equipped && !native.inventory.switching(),"the weapon to be drawn")
                },
                1=>{
                    let Some(file)=sound("shoot") else {return fail(&mut probe,format!("{name} has no shoot sound"))};
                    if script.cursor.first() {script.base=count(&file);}
                    if count(&file)>base {controls.fire=false;controls.held=false;script.summary.push(format!("{name} shoot {file}"));Step::Done}else{controls.fire=true;controls.held=true;Step::Wait("the shot sound")}
                },
                2=>{
                    let Some(file)=sound("reload") else {return fail(&mut probe,format!("{name} has no reload sound"))};
                    if script.cursor.first() {script.base=count(&file);}
                    if count(&file)>base {controls.reload=false;script.summary.push(format!("{name} reload {file}"));Step::Done}else{controls.reload=!native.inventory.reloading();Step::Wait("the reload sound")}
                },
                _=>Step::when(!native.inventory.reloading(),"the reload to end"),
            }
        },
        // The nightstick swings with swist.wav (item sound_shoot, cshell 0x10003eb3).
        s if s==weapons=>{
            if script.cursor.first() {native.acquire("Police nightstick");native.inventory.select(0);script.base=count("swist.wav");}
            if native.inventory.selected!=Some(0) || native.inventory.switching() {Step::Wait("the nightstick to be drawn")}
            else if count("swist.wav")>base {controls.fire=false;controls.held=false;script.summary.push("baton swist.wav".into());Step::Done}
            else {controls.fire=true;controls.held=true;Step::Wait("the swing sound")}
        },
        // The hand grenade is primed silently (its zawleka.wav is never played in retail) and thrown: the blast is rock_lup.wav, a 3D sound of radius 1280.
        s if s==weapons+1=>{
            let grenade=native.definitions.iter().position(|d|d.grenade);
            let Some(grenade)=grenade else {return fail(&mut probe,"no grenade weapon".into())};
            if script.cursor.first() {let ammo=native.definitions[grenade].ammo_index;native.acquire("Hand grenade");native.inventory.add_ammo(ammo,5);native.inventory.select(grenade);script.base=count("rock_lup.wav");}
            if native.inventory.selected!=Some(grenade) || native.inventory.switching() {Step::Wait("the grenade to be drawn")}
            else if count("zawleka")>0 {Step::Fail("the grenade played zawleka.wav: retail never does".into())}
            else if count("rock_lup.wav")>base {controls.fire=false;controls.held=false;campaign.health=100.0;script.summary.push("grenade blast rock_lup.wav".into());Step::Done}
            else {let age=now-*script.drawn.get_or_insert(now);let holding=age<0.6;controls.fire=age<0.1;controls.held=holding;Step::Wait("the grenade blast")}
        },
        // The music stayed off although the switch is on again (a stop clears the remembered track).
        s if s==weapons+2=>Step::check(options.music && music.playing().is_none(),||"Music 1 alone must not restart the level music (cshell 0x10059230)".to_owned()),
        _=>{
            info!("SOUNDS PRÓBA kész: hiba=false [{}]",script.summary.join("; "));
            probe.finished=true;return;
        },
    };
    let mut failure=None;
    let limit=if (FIRST_WEAPON_STAGE..).contains(&stage) {12.0}else{8.0};
    let stopped=script.cursor.apply(step,now,limit,&mut failure);
    if stopped {error!("SOUNDS PRÓBA sikertelen: {failure:?}");probe.failure=failure;}
    else if script.cursor.stage!=stage {info!("SOUNDS PRÓBA {stage}: rendben (cues={})",cues.list.len());}
}
fn fail(probe:&mut Probe,message:String) {error!("SOUNDS PRÓBA sikertelen: {message}");probe.failure=Some(message);}

/// A floor point near the player with at least 700 free units above it, returned 650 units up in the air (a fall of ~650 units).
fn free_column(walking:&Walking)->Option<retail_movement::Vec3> {
    use retail_movement::Vec3 as N;
    let centre=walking.player.position;
    for ring in 0..=20i32 {for i in -ring..=ring {for j in -ring..=ring {
        if i.abs().max(j.abs())!=ring {continue;}
        let top=N::new(centre.x+i as f32*200.0,centre.y+300.0,centre.z+j as f32*200.0);
        let Some((distance,_,_))=walking.world.raycast_face(top,N::NEG_Y,700.0) else {continue};
        let floor=N::new(top.x,top.y-distance,top.z);
        if walking.world.raycast(floor+N::Y*60.0,N::Y,700.0).is_none() {return Some(floor+N::Y*(58.2+650.0));}
    }}}
    None
}
