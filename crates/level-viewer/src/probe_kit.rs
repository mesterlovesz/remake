//! Shared plumbing of the headless probes (`MESTER_TEST_SCENARIO=<name> level-viewer <world|menu> <output> <png> <times>`).
//!
//! Two things make a probe reproducible: the game clock of a scenario run advances by exactly `STEP` every frame (so a stage
//! script sees the same frames on a slow and on a fast machine), and a script moves on when the game state says so (item gone,
//! shot counted, level arrived), never after a guessed number of seconds. `docs/probes.md` lists every scenario.
use bevy::{prelude::*,time::TimeUpdateStrategy};
use crate::{ViewerConfig,campaign::Campaign,frontend::Frontend};

/// Seconds the game clock advances per frame in a scenario run; equal to the `min(0.05)` clamp most game systems apply.
pub const STEP:f32=0.05;
/// A scenario run has capture arguments and a scenario name; `MESTER_TEST_REALTIME=1` keeps the wall clock for visual captures.
pub fn scenario_run(arguments:&[String])->bool {arguments.get(3).is_some() && (std::env::var_os("MESTER_TEST_SCENARIO").is_some() || std::env::var_os("MESTER_MOVE_PROBE").is_some()) && std::env::var_os("MESTER_TEST_REALTIME").is_none()}
pub fn time_strategy(arguments:&[String])->TimeUpdateStrategy {
    if scenario_run(arguments) {TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(STEP))}else{TimeUpdateStrategy::Automatic}
}
/// The hidden, unfocused window of a scenario run updates as fast as it can (the default throttles unfocused windows).
pub fn winit_settings(arguments:&[String])->bevy::winit::WinitSettings {if scenario_run(arguments) {bevy::winit::WinitSettings::continuous()}else{bevy::winit::WinitSettings::game()}}
/// A run that never finishes its probe is stopped after this many game seconds past the last capture time.
pub const HARD_LIMIT:f32=600.0;

/// What one stage of a probe script reports each frame.
pub enum Step {
    /// The awaited state is not there yet (what is awaited, for the timeout message).
    Wait(&'static str),
    Done,
    Fail(String),
}
impl Step {
    pub fn when(ready:bool,what:&'static str)->Step {if ready {Step::Done}else{Step::Wait(what)}}
    /// `Done` when `ok`, else a failure with the message.
    pub fn check(ok:bool,message:impl FnOnce()->String)->Step {if ok {Step::Done}else{Step::Fail(message())}}
}
/// Position in a probe script: the current stage, the game time it began and whether its action already ran.
#[derive(Default)] pub struct Cursor {pub stage:usize,began:f32,seen:bool}
impl Cursor {
    /// True once per stage, on its first frame: the place for the stage's action.
    pub fn first(&mut self)->bool {!std::mem::replace(&mut self.seen,true)}
    /// Game seconds since the stage began.
    pub fn age(&self,now:f32)->f32 {now-self.began}
    /// Applies a stage result. `Done` moves to the next stage; `Wait` fails after `limit` game seconds; a failure is recorded
    /// (first one wins) and returns `true`, which means the probe should stop.
    pub fn apply(&mut self,step:Step,now:f32,limit:f32,failure:&mut Option<String>)->bool {
        let message=match step {
            Step::Done=>{self.stage+=1;self.began=now;self.seen=false;return false;},
            Step::Wait(what)=>{if self.age(now)<=limit {return false;}format!("stage {}: still waiting for {what} after {limit:.0} s",self.stage)},
            Step::Fail(message)=>format!("stage {}: {message}",self.stage),
        };
        error!("{message}");failure.get_or_insert(message);true
    }
}
/// Level `world` is loaded and playable: the config names it, the loading screen is gone and the mission script is bound to it.
pub fn arrived(config:&ViewerConfig,front:&Frontend,campaign:&Campaign,world:&str)->bool {
    config.world.eq_ignore_ascii_case(world) && front.loading.is_none() && campaign.world.eq_ignore_ascii_case(world)
}

/// The state of every probe of the binary, for the capture exit rule. Add a new probe's resource here (one line in `rows`).
#[derive(bevy::ecs::system::SystemParam)]
pub struct ProbeStatus<'w> {
    campaign:Res<'w,crate::campaign_probe::Probe>,retail:Res<'w,crate::retail_probe::Probe>,panel:Res<'w,crate::panel_probe::Probe>,
    front:Res<'w,Frontend>,alt:Res<'w,crate::weapons_alt::Probe>,menu:Res<'w,crate::menu_probe::Probe>,ai:Res<'w,crate::ai_probe::AiProbe>,sounds:Res<'w,crate::sound_probe::Probe>,
}
pub enum Verdict {Idle,Running,Passed,Failed(&'static str,Option<String>)}
impl ProbeStatus<'_> {
    pub fn verdict(&self)->Verdict {
        let rows=[
            ("Főmenüpróba",self.front.probe_active,self.front.probe_finished,&self.front.probe_failure),
            ("Gyári fegyverpróba",self.retail.active,self.retail.finished,&self.retail.failure),
            ("Panelpróba",self.panel.active,self.panel.finished,&self.panel.failure),
            ("Kampánypróba",self.campaign.active,self.campaign.finished,&self.campaign.failure),
            ("Altfire-próba",self.alt.active,self.alt.finished,&self.alt.failure),
            ("Menüpróba",self.menu.active,self.menu.finished,&self.menu.failure),
            ("AI-próba",self.ai.active,self.ai.finished,&self.ai.failure),
            ("Hangpróba",self.sounds.active,self.sounds.finished,&self.sounds.failure),
        ];
        let mut state=Verdict::Idle;
        for (name,active,finished,failure) in rows {
            if !active {continue;}
            if failure.is_some() {return Verdict::Failed(name,failure.clone());}
            state=if finished {Verdict::Passed}else{Verdict::Running};
            if !finished {return state;}
        }
        state
    }
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn wait_times_out_and_done_advances() {
        let mut cursor=Cursor::default();let mut failure=None;
        assert!(cursor.first() && !cursor.first());
        assert!(!cursor.apply(Step::Wait("x"),1.0,2.0,&mut failure) && cursor.stage==0);
        assert!(cursor.apply(Step::Wait("x"),2.5,2.0,&mut failure) && failure.as_deref().is_some_and(|f|f.contains("waiting for x")));
        let mut next=Cursor::default();
        assert!(!next.apply(Step::Done,3.0,1.0,&mut None) && next.stage==1 && next.first() && (next.age(4.5)-1.5).abs()<1e-6);
    }
    #[test] fn first_failure_is_kept() {
        let mut cursor=Cursor::default();let mut failure=None;
        assert!(cursor.apply(Step::Fail("a".into()),0.0,1.0,&mut failure) && cursor.apply(Step::Fail("b".into()),0.0,1.0,&mut failure));
        assert_eq!(failure.as_deref(),Some("stage 0: a"));
    }
}
