//! Silent AI probe (`MESTER_TEST_SCENARIO=ai`, capture mode only): plays the level's `hostileattack` action every frame and checks the retail rules of
//! one character headlessly (docs/retail-ai.md):
//! * `MESTER_TEST_AI=fight` (default): the player stands `MESTER_TEST_NPC_DISTANCE` (default 0.6 * odleglosc_kontaktu) in front of an armed character
//!   (`MESTER_TEST_NPC`: instance or definition name; default the first armed one with `on_kontakt`). Expected: contact inside the retail reach, the `on_kontakt`
//!   phase, and a shot rate of `1 / strzal` in the firing phase.
//! * `far`: the same beyond 1.3 * odleglosc_kontaktu (or `MESTER_TEST_NPC_DISTANCE`): no contact may happen.
//! * `patrol`: a patrolling character walks its path graph at the phase `speed` and stays on the links.
//! `MESTER_TEST_END` is the run length (default 8 s). Lines start with `AI PROBE`; a failed check makes the process exit with an error.
use bevy::prelude::*;
use crate::{ViewerConfig,Walking,campaign::Campaign,npcs::NpcRoster,settings::Session};

#[derive(Resource,Default)]
pub struct AiProbe {
    pub active:bool,pub finished:bool,pub failure:Option<String>,
    mode:String,elapsed:f32,stage:usize,npc:String,start_phase:String,log_at:f32,
    notice_at:Option<f32>,reaction_at:Option<(f32,String)>,shots_start:u32,shot_times:Vec<f32>,last_shots:u32,fire_phase:Option<(f32,f32)>,
    walked:f32,last_position:Vec3,walk_start:f32,start_at:f32,last_phase:String,max_off_graph:f32,speed:f32,expected:f32,samples:usize,
}
pub fn setup(mut commands:Commands,config:Res<ViewerConfig>) {
    let mode=std::env::var("MESTER_TEST_SCENARIO").unwrap_or_default();
    commands.insert_resource(AiProbe {active:config.capture.is_some() && mode=="ai",mode:std::env::var("MESTER_TEST_AI").unwrap_or_else(|_|"fight".into()),..default()});
}
fn n(v:Vec3)->retail_movement::Vec3 {retail_movement::Vec3::new(v.x,v.y,v.z)}

/// Stand the player about `distance` in front of the actor (within +-30 degrees of its heading) with a free line to its chest.
fn stand_in_front(walking:&mut Walking,doors:&[&crate::doors::Door],position:Vec3,forward:Vec3,eye:Vec3,distance:f32,at_least:f32)->bool {
    for scale in [1.0,0.9,1.1,0.8,1.2,0.7] {
        for offset in [0.0f32,8.0,-8.0,16.0,-16.0,24.0,-24.0,32.0,-32.0] {
            let direction=Quat::from_rotation_y(offset.to_radians())*forward;
            crate::campaign_probe::place(walking,position+direction*distance*scale,eye);
            let p=walking.player.position;let player=Vec3::new(p.x,p.y,p.z);
            let to=player-position;let flat=Vec3::new(to.x,0.0,to.z);
            if (to.length()/distance-1.0).abs()<0.25 && to.length()>=at_least && to.y.abs()<90.0 && flat.normalize_or_zero().dot(forward)>0.8 && crate::npcs::line_of_sight(&walking.world,eye,player+Vec3::Y*48.0)
                && !doors.iter().any(|d|{let delta=player+Vec3::Y*48.0-eye;d.shot_hit(eye,delta.normalize_or_zero(),delta.length()).is_some()}) {return true;}
        }
    }
    false
}

pub fn tick(mut probe:ResMut<AiProbe>,time:Res<Time>,config:Res<ViewerConfig>,mut walking:ResMut<Walking>,mut session:ResMut<Session>,mut campaign:ResMut<Campaign>,mut roster:ResMut<NpcRoster>,opening:Res<crate::opening::Opening>,doors:Query<&crate::doors::Door>) {
    if !probe.active || probe.finished {return;}
    // The level's opening cutscene freezes the actors; the probe's clock starts when it is over.
    if opening.active {return;}
    // The actors advance by the clamped frame time (npcs::tick), so the probe's clock uses it too: slow debug frames stretch the run, not the actors.
    let dt=time.delta_secs().min(0.05);probe.elapsed+=dt;session.paused=false;
    let end=std::env::var("MESTER_TEST_END").ok().and_then(|v|v.parse::<f32>().ok()).unwrap_or(8.0);
    // The player survives the whole run; the damage is counted from the roster's statistics.
    if campaign.health<40.0 {campaign.health=100.0;}
    // The player controller is frozen for its first 60 ticks (retail): start after that, so the placement holds.
    if probe.elapsed<0.5 || walking.player.frames<62 {return;}
    let wanted=std::env::var("MESTER_TEST_NPC").unwrap_or_default();
    if probe.stage==0 {
        let candidates:Vec<usize>=roster.actors.iter().enumerate().filter(|(_,a)|a.alive() && a.shown() && (a.name==wanted || a.definition_name==wanted || (wanted.is_empty() && match probe.mode.as_str() {
            "patrol"=>a.has_phase_with("patrol") && a.has_phase_with("speed"),
            _=>a.diagnostics().armed && a.has_phase_with("on_kontakt") && (a.has_phase_with("strzal") || a.has_phase_with("strzal_raz")),
        }))).map(|(i,_)|i).collect();
        if candidates.is_empty() {probe.failure=Some(format!("No suitable actor ({wanted:?}, mode {}; {} actors, {} armed, {} with on_kontakt)",probe.mode,roster.actors.len(),roster.actors.iter().filter(|a|a.diagnostics().armed).count(),roster.actors.iter().filter(|a|a.has_phase_with("on_kontakt")).count()));probe.finished=true;return;}
        let door_refs:Vec<&crate::doors::Door>=doors.iter().collect();
        let mut chosen=None;
        for index in candidates {
            let (position,rotation,eye)={let a=&roster.actors[index];(a.position,a.rotation,a.eye())};
            let diagnostics=roster.actors[index].diagnostics();
            let distance=std::env::var("MESTER_TEST_NPC_DISTANCE").ok().and_then(|v|v.parse::<f32>().ok())
                .unwrap_or(if probe.mode=="far" {(diagnostics.reach*1.25).min(9000.0)}else{(diagnostics.reach*0.6).min(400.0)});
            let at_least=if probe.mode=="far" {diagnostics.reach*1.05} else {0.0};
            // Patrol: watch from a few hundred units in front.
            let distance=if probe.mode=="patrol" {std::env::var("MESTER_TEST_NPC_DISTANCE").ok().and_then(|v|v.parse::<f32>().ok()).unwrap_or(260.0)} else {distance};
            if stand_in_front(&mut walking,&door_refs,position,rotation*Vec3::Z,eye,distance,at_least) {chosen=Some(index);break;}
        }
        let Some(index)=chosen else {probe.failure=Some("No standing spot with a free line in front of any candidate".into());probe.finished=true;return;};
        let (name,position)={let a=&roster.actors[index];(a.name.clone(),a.position)};
        let diagnostics=roster.actors[index].diagnostics();
        probe.npc=name.clone();probe.start_phase=diagnostics.phase.clone();
        if probe.mode=="patrol" {
            let phase=roster.actors[index].phases_with("patrol").into_iter().next().unwrap_or_default();
            // Nobody notices anything (the player counts as dead for the AI), and the actor is put into its patrol phase.
            roster.player_dead=true;
            roster.set_phase(&name,&phase);
            let after=roster.actors[index].diagnostics();probe.speed=after.speed.unwrap_or(0.0);
            info!("AI PROBE patrol {name} phase={phase} speed={:?} place={:?}",after.speed,after.place);
            probe.last_position=position;probe.walk_start=probe.elapsed;
        }else{
            let p=walking.player.position;let d=Vec3::new(p.x,p.y,p.z).distance(position);
            info!("AI PROBE {} {name} ({}) reach={} cone={} distance={d:.0} on_kontakt={:?} strzal_phases={:?} start_phase={}",probe.mode,roster.actors[index].definition_name,diagnostics.reach,diagnostics.cone_degrees,
                diagnostics.on_kontakt,roster.actors[index].phases_with("strzal"),diagnostics.phase);
        }
        probe.shots_start=roster.stats.shots;probe.last_shots=roster.stats.shots;probe.stage=1;probe.log_at=probe.elapsed;probe.start_at=probe.elapsed;
        return;
    }
    let Some(index)=roster.actors.iter().position(|a|a.name==probe.npc) else {return};
    // The levels' `hostileattack` action runs every tick: whoever notices the player is provoked.
    if probe.mode!="patrol" {roster.provoke_noticing();}
    let d=roster.actors[index].diagnostics();
    let position=roster.actors[index].position;
    if d.contact && probe.notice_at.is_none() {probe.notice_at=Some(probe.elapsed);info!("AI PROBE t={:.2} contact: seen={} provoked={}",probe.elapsed,d.seen,d.provoked);}
    if probe.reaction_at.is_none() && probe.notice_at.is_some() && d.phase!=probe.start_phase {probe.reaction_at=Some((probe.elapsed,d.phase.clone()));info!("AI PROBE t={:.2} reaction phase={}",probe.elapsed,d.phase);}
    if roster.stats.shots>probe.last_shots {for _ in probe.last_shots..roster.stats.shots {let t=probe.elapsed;probe.shot_times.push(t);}probe.last_shots=roster.stats.shots;}
    if let Some(interval)=d.strzal {if probe.fire_phase.is_none_or(|(_,i)|i!=interval) {probe.fire_phase=Some((probe.elapsed,interval));info!("AI PROBE t={:.2} firing phase {} strzal={interval}",probe.elapsed,d.phase);}}
    if probe.mode=="patrol" {
        let step=position.distance(probe.last_position);
        // A forced phase switch makes a walker jump to its target node (0x10041d5e): not walking, not a rail deviation.
        let switched=probe.last_phase!=d.phase;probe.last_phase=d.phase.clone();
        if step<200.0 && !switched {probe.walked+=step;}
        // What the phases in force promise: their `speed` for every moving frame.
        if d.moving && !switched {probe.expected+=d.speed.unwrap_or(0.0)*dt;}
        probe.last_position=position;
        if d.moving && !switched {if let Some(off)=roster.distance_to_links(position,roster.actors[index].half_extents.y) {probe.max_off_graph=probe.max_off_graph.max(off);probe.samples+=1;}}
    }
    if probe.elapsed-probe.log_at>=0.5 {
        probe.log_at=probe.elapsed;
        let p=walking.player.position;let player=Vec3::new(p.x,p.y,p.z);
        let chest=position+Vec3::Y*roster.actors[index].half_extents.y*0.75;
        let door_shut=doors.iter().any(|d|{let delta=chest-(player+Vec3::Y*48.0);d.shot_hit(player+Vec3::Y*48.0,delta.normalize_or_zero(),delta.length()).is_some()});
        let clear=crate::npcs::line_of_sight(&walking.world,player+Vec3::Y*48.0,chest) && !door_shut;
        info!("AI PROBE t={:.1} {} phase={} contact={} seen={} provoked={} moving={} place={:?} pos=({:.0},{:.0},{:.0}) yaw={:.2} shots={} on_player={} damage={} health={:.0} player=({:.0},{:.0},{:.0}) dist={:.0} clear_to_chest={clear} door_shut={door_shut} player_dead={} progress={:.1}/{:.1} next={:?} path_len={} dialogue={}",probe.elapsed,probe.npc,d.phase,d.contact,d.seen,d.provoked,d.moving,d.place,
            position.x,position.y,position.z,d.yaw,roster.stats.shots-probe.shots_start,roster.stats.bullets_on_player,roster.stats.damage_to_player,campaign.health,player.x,player.y,player.z,player.distance(position),roster.player_dead,d.progress,d.seg_len,d.next,d.path_len,session.dialogue_active);
    }
    if probe.elapsed<end {return;}
    // Verdicts.
    let mut failures=Vec::new();
    match probe.mode.as_str() {
        "patrol"=>{
            let seconds=probe.elapsed-probe.walk_start;let speed=probe.walked/seconds.max(0.001);
            info!("AI PROBE RESULT patrol walked {:.0} units in {seconds:.1} s = {speed:.1} units/s (phase speed {}), max distance from a link {:.2} over {} samples, door requests {}",probe.walked,probe.speed,probe.max_off_graph,probe.samples,roster.stats.door_asks);
            if probe.walked<10.0 {failures.push("the patrol never moved".to_string());}
            if probe.max_off_graph>1.5 {failures.push(format!("left the path graph by {:.2}",probe.max_off_graph));}
            let ratio=probe.walked/probe.expected.max(0.001);info!("AI PROBE RESULT patrol walked {:.0} of the {:.0} units the phase speeds promise = {ratio:.2}",probe.walked,probe.expected);
            if probe.expected>0.0 && !(0.8..=1.05).contains(&ratio) {failures.push(format!("walked {ratio:.2} of the promised distance"));}
        },
        "far"=>{
            info!("AI PROBE RESULT far notice={:?} shots={}",probe.notice_at,roster.stats.shots-probe.shots_start);
            if probe.notice_at.is_some() {failures.push("noticed the player beyond the retail reach".into());}
        },
        _=>{
            let shots=probe.shot_times.len();
            info!("AI PROBE RESULT fight notice={:?} reaction={:?} shots={shots} bullets_on_player={} melee={} damage={} door_asks={}",probe.notice_at,probe.reaction_at,roster.stats.bullets_on_player,roster.stats.melee,roster.stats.damage_to_player,roster.stats.door_asks);
            if probe.notice_at.is_none() {failures.push("never noticed the player inside the retail reach".into());}
            else if probe.notice_at.is_some_and(|t|t-probe.start_at>1.5) {failures.push("noticed too late".into());}
            if probe.reaction_at.is_none() {failures.push("no on_kontakt reaction".into());}
            if let Some((start,interval))=probe.fire_phase {
                let times:Vec<f32>=probe.shot_times.iter().copied().filter(|t|*t>start+0.2).collect();
                if times.len()>=4 {
                    let span=times.last().unwrap()-times.first().unwrap();let measured=span/(times.len()-1) as f32;
                    info!("AI PROBE RESULT fire interval measured {measured:.3} s, strzal {interval} s over {} shots",times.len());
                    // Frame quantisation and reload / relocation pauses lengthen the mean, never shorten it.
                    if measured<interval*0.85 {failures.push(format!("fires faster than strzal {interval}: {measured:.3}"));}
                }
            }
        },
    }
    for failure in &failures {error!("AI PROBE FAILURE {failure}");}
    probe.failure=(!failures.is_empty()).then(||failures.join("; "));
    probe.finished=true;
    let _=(&config,n(Vec3::ZERO));
}
