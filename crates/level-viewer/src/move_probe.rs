//! MESTER_MOVE_PROBE=1 (headless capture run): scripted key presses walk, run, jump, crouch and stand up in the loaded level
//! and log what the retail controller did (position, speed, stance, stamina). Nothing is audible or visible on screen.
use bevy::{prelude::*,window::{CursorGrabMode,CursorOptions}};
use crate::{Walking,settings::Session};

#[derive(Resource)] pub struct Probe {pub active:bool}
impl Default for Probe {fn default()->Self {Self {active:std::env::var_os("MESTER_MOVE_PROBE").is_some()}}}

/// What is held at controller frame `f` (a slow debug capture runs at a few frames per second, so frames, not seconds, script it):
/// frozen for the first 60 frames, then walk, run, jump, crouch-walk and stand.
fn script(f:u32)->(bool,bool,bool,bool) {
    let walk=(65..125).contains(&f) || (125..185).contains(&f) || (230..290).contains(&f);
    let run=(125..185).contains(&f);
    let jump=f==190;
    let crouch=(230..262).contains(&f);
    (walk,run,jump,crouch)
}

pub fn tick(probe:Res<Probe>,time:Res<Time>,mut keys:ResMut<ButtonInput<KeyCode>>,walking:Res<Walking>,mut session:ResMut<Session>,mut cursor:Single<&mut CursorOptions>,mut last:Local<u32>,opening:Res<crate::opening::Opening>,front:Res<crate::frontend::Frontend>) {
    if !probe.active {return;}
    let t=time.elapsed_secs();
    session.paused=false;cursor.grab_mode=CursorGrabMode::Locked;
    let (walk,run,jump,crouch)=script(walking.player.frames);
    // Default keys.cfg: W forward, LeftShift run, LeftControl crouch, Space jump (the owner's change).
    for (key,held) in [(KeyCode::KeyW,walk),(KeyCode::ShiftLeft,run),(KeyCode::ControlLeft,crouch),(KeyCode::Space,jump)] {if held {keys.press(key);}else {keys.release(key);}}
    if walking.player.frames>=*last+10 || walking.player.frames<*last {
        *last=walking.player.frames;
        let p=&walking.player;
        info!("MOVE PROBE t={t:.2} pos=({:.1},{:.1},{:.1}) speed={:.1} vy={:.1} grounded={} crouched={} stamina={:.1} frames={} [opening={} dialogue={} menu={} loading={}]",p.position.x,p.position.y,p.position.z,Vec2::new(p.velocity.x,p.velocity.z).length(),p.velocity.y,p.grounded,p.crouched,p.stamina,p.frames,opening.active,session.dialogue_active,front.main_active,front.loading.is_some());
    }
}
