//! Silent opt-in integration route through real mission markers and exit doors.
use bevy::prelude::*;
use crate::{ViewerConfig,Walking,settings::Session,campaign::Campaign,npcs::NpcRoster,doors::DoorUse,gunfire::Controls,retail_weapons::NativeArsenal,probe_kit::{Cursor,Step,arrived}};
#[derive(Resource,Default)] pub struct Probe {pub active:bool,pub finished:bool,pub failure:Option<String>,mode:String,elapsed:f32,stage:usize,stage_start:f32,shot_at:f32,gun:usize,hit_done:bool,cursor:Cursor,base:u32,aimed:bool,attempt:usize,markers:Vec<serde_json::Value>}
pub fn setup(mut commands:Commands,config:Res<ViewerConfig>) {let mode=std::env::var("MESTER_TEST_SCENARIO").unwrap_or_default();commands.insert_resource(Probe {active:config.capture.is_some() && matches!(mode.as_str(),"campaign"|"cell"|"startpose"|"bus"|"catalog"|"uv"|"npc"|"impact"|"gunfire"|"zmienna"|"lever"|"stella"|"props"|"door"|"pickup"),mode,..default()});}
/// Gunfire-scenario helpers (slow motion, forced surface flags, synthetic wounds) bundled to stay under Bevy's system parameter limit.
#[derive(bevy::ecs::system::SystemParam)] pub struct GunfireProbe<'w,'s> {clock:ResMut<'w,Time<Virtual>>,effects:Res<'w,crate::gunfire::Effects>,surfaces:ResMut<'w,crate::audio::Surfaces>,commands:Commands<'w,'s>}
/// Mission-scenario helpers (chain state, door poses) bundled to stay under Bevy's system parameter limit.
#[derive(bevy::ecs::system::SystemParam)] pub struct MissionProbe<'w,'s> {activation:ResMut<'w,crate::activation::Activation>,doors_state:Query<'w,'s,&'static crate::doors::Door>,front:Res<'w,crate::frontend::Frontend>}
pub(crate) fn place(walking:&mut Walking,mut position:Vec3,target:Vec3) {
    // Select a real nonpenetrating standing position near the route waypoint.
    let requested=position;
    'search: for radius in [0.0,32.0,64.0,96.0,128.0,192.0] {for i in 0..16 {
        let angle=i as f32*std::f32::consts::TAU/16.0;let p=requested+Vec3::new(angle.cos()*radius,18.0,angle.sin()*radius);
        let origin=retail_movement::Vec3::new(p.x,p.y,p.z);
        if let Some((distance,normal))=walking.world.raycast(origin,retail_movement::Vec3::NEG_Y,300.0) {if normal.y.abs()>0.65 {
            let candidate=origin-retail_movement::Vec3::Y*distance+retail_movement::Vec3::Y*58.2;
            if !walking.world.box_overlaps(candidate,retail_movement::Vec3::new(25.0,58.0,25.0)) {position=Vec3::new(candidate.x,candidate.y,candidate.z);break 'search;}
        }}
    }}
    walking.player.position=retail_movement::Vec3::new(position.x,position.y,position.z);walking.player.velocity=retail_movement::Vec3::ZERO;
    let direction=(target-(position+Vec3::Y*40.0)).normalize();walking.yaw=(-direction.x).atan2(-direction.z);walking.pitch=direction.y.asin();
}
/// True when nothing solid (wall or closed door) lies between `eye` and `target`.
pub(crate) fn clear_line(walking:&Walking,doors:Option<&Query<&crate::doors::Door>>,eye:Vec3,target:Vec3)->bool {
    let delta=target-eye;let length=delta.length();let d=delta/length.max(0.01);
    !walking.world.raycast(retail_movement::Vec3::new(eye.x,eye.y,eye.z),retail_movement::Vec3::new(d.x,d.y,d.z),length).is_some_and(|h|h.0<length-4.0)
        && !doors.is_some_and(|doors|doors.iter().any(|door|door.shot_hit(eye,d,length).is_some()))
}
/// A standing spot with a clear bullet line to `target` (`attempt` picks the n-th such spot, so a retry after the game moved the
/// player out of a bad one - closed doors push - tries another).
pub(crate) fn place_with_clear_shot(walking:&mut Walking,target:Vec3,doors:&Query<&crate::doors::Door>,attempt:usize)->bool {place_with_clear_shot_at(walking,target,&[80.0,120.0,170.0,230.0,300.0],65.0,Some(doors),attempt)}
pub(crate) fn place_with_clear_shot_at(walking:&mut Walking,target:Vec3,radii:&[f32],max_dy:f32,doors:Option<&Query<&crate::doors::Door>>,attempt:usize)->bool {
    let mut spots=Vec::new();
    for &radius in radii {for i in 0..24 {
        let angle=i as f32*std::f32::consts::TAU/24.0;
        let origin=target+Vec3::new(angle.cos()*radius,20.0,angle.sin()*radius);
        let ray=retail_movement::Vec3::new(origin.x,origin.y,origin.z);
        let Some((distance,normal))=walking.world.raycast(ray,retail_movement::Vec3::NEG_Y,160.0) else {continue};
        if normal.y<0.65 {continue;}
        let center=origin-Vec3::Y*distance+Vec3::Y*58.2;
        if (center.y-target.y).abs()>max_dy || walking.world.box_overlaps(retail_movement::Vec3::new(center.x,center.y,center.z),retail_movement::Vec3::new(25.0,58.0,25.0)) {continue;}
        if !clear_line(walking,doors,center+Vec3::Y*46.0,target) {continue;}
        spots.push(center);
    }}
    if spots.is_empty() {return false;}
    let center=spots[attempt%spots.len()];
    walking.player.position=retail_movement::Vec3::new(center.x,center.y,center.z);
    walking.player.velocity=retail_movement::Vec3::ZERO;
    let direction=(target-(center+Vec3::Y*46.0)).normalize();
    walking.yaw=(-direction.x).atan2(-direction.z);walking.pitch=direction.y.asin();
    true
}
pub fn tick(mut probe:ResMut<Probe>,time:Res<Time>,config:Res<ViewerConfig>,mut walking:ResMut<Walking>,mut session:ResMut<Session>,mut campaign:ResMut<Campaign>,mut roster:ResMut<NpcRoster>,mut doors:ResMut<DoorUse>,mut controls:ResMut<Controls>,mut native:ResMut<NativeArsenal>,volume:Res<GlobalVolume>,mut keys:ResMut<ButtonInput<KeyCode>>,opening:Res<crate::opening::Opening>,mut travel:ResMut<crate::travel::Travel>,extra:GunfireProbe,mission:MissionProbe) {
    let GunfireProbe {mut clock,effects,mut surfaces,mut commands}=extra;let MissionProbe {mut activation,doors_state,front}=mission;
    if !probe.active || probe.finished || matches!(probe.mode.as_str(),"props"|"door"|"pickup") {return;}
    probe.elapsed+=time.delta_secs();session.paused=false;
    // MESTER_TEST_SLOW="factor,from,to" slows game time inside that window so a capture can catch 0.1 s effects.
    let window:Vec<f32>=std::env::var("MESTER_TEST_SLOW").unwrap_or_default().split(',').filter_map(|v|v.parse().ok()).collect();
    clock.set_relative_speed(if window.len()==3 && (window[1]..window[2]).contains(&probe.elapsed) {window[0]}else{1.0});
    // MESTER_TEST_LEVER=<o_obiekt name>: stand in front of a lever, press E, then list what opened.
    if probe.mode=="lever" {
        let wanted=std::env::var("MESTER_TEST_LEVER").unwrap_or_default();
        if probe.stage==0 {
            let scene:serde_json::Value=serde_json::from_str(&std::fs::read_to_string(config.output.join(format!("{}.scene.json",config.world))).unwrap()).unwrap();
            match scene["objects"].as_array().unwrap().iter().find(|o|o["properties"]["Name"]==wanted.as_str()) {
                Some(object)=>{if object["kind"]!="o_obiekt" {activation.queue.push_back(wanted.clone());}
                    let target=crate::doors::vector(&object["properties"]["Pos"]);if !place_with_clear_shot_at(&mut walking,target,&[40.0,60.0,80.0,100.0,120.0],150.0,None,0) {info!("LEVER PROBE no standing spot in front of {wanted}: activating it directly");activation.queue.push_back(wanted.clone());}},
                None=>probe.failure=Some(format!("Missing lever {wanted}")),
            }
            probe.stage=1;
        }
        if probe.stage==1 && probe.elapsed>=1.5 {keys.press(KeyCode::KeyE);probe.stage=2;info!("LEVER PROBE hint={:?}",doors.hint);}
        if probe.stage==2 && probe.elapsed>=1.6 {keys.release(KeyCode::KeyE);}
        if probe.elapsed>=5.0 {
            let open:Vec<_>=doors_state.iter().filter(|d|d.state().0 || d.state().1>0.0).map(|d|d.name.clone()).collect();
            let mut on:Vec<_>=activation.states.iter().filter(|(_,on)|**on).map(|(name,_)|name.clone()).collect();on.sort();
            info!("LEVER PROBE {wanted}: open doors={open:?} toggled={on:?} travel={:?} failure={:?}",travel.pending,probe.failure);
            if open.is_empty() && on.is_empty() && travel.pending.is_none() {probe.failure.get_or_insert(format!("{wanted} opened and toggled nothing"));}
            probe.finished=true;
        }
        return;
    }
    // Chapel: enter the Special1 detector, talk to the spawned "laska czapel" and answer until she is "zagadana".
    if probe.mode=="stella" {
        let stella=roster.actors.iter().find(|a|a.definition_name=="laska czapel");
        let now=probe.elapsed;let p=&mut *probe;
        let step=match p.cursor.stage {
            0=>{if p.cursor.first() {walking.player.position=retail_movement::Vec3::new(-1622.0,-1010.0,1078.0);walking.player.velocity=retail_movement::Vec3::ZERO;}
                if let Some(actor)=stella {info!("STELLA PROBE spawned at {:?} phase {} flag={:?}",actor.position,actor.phase,campaign.mission.as_ref().map(|m|m.flag("LaskaCzapelRespawnowana")));Step::Done}else{Step::Wait("Stella to appear")}},
            // Her first line ("Fordulj meg!") plays out, then the player stands in front of her.
            1=>{if p.cursor.age(now)<4.0 {Step::Wait("the first line")}else{
                if let Some(actor)=stella {let target=actor.position+Vec3::Y*20.0;let position=actor.position+actor.rotation*Vec3::Z*70.0;place(&mut walking,position,target);}
                Step::Done}},
            2=>{
                // Take E on her (the first dialogue may still be running), then pick answer 1 every time choices appear.
                if let Some(actor)=stella {let target=actor.eye();let pl=walking.player.position;let d=(target-Vec3::new(pl.x,pl.y+40.0,pl.z)).normalize_or_zero();walking.yaw=(-d.x).atan2(-d.z);walking.pitch=d.y.asin();}
                let step=(p.cursor.age(now)/0.5) as u32;
                if step%6==0 {keys.press(KeyCode::KeyE);}else{keys.release(KeyCode::KeyE);}
                if campaign.dialogue.as_ref().is_some_and(|d|!d.choices.is_empty()) && step%3==1 {if let Some(m)=campaign.mission.as_mut() {let _=m.choose(1);}}
                Step::when(campaign.mission.as_ref().is_some_and(|m|m.flag("LaskaChapelZagadana")),"the conversation to complete (LaskaChapelZagadana)")
            },
            _=>{
                let m=campaign.mission.as_ref();
                info!("STELLA PROBE zagadana={:?} obsluzona={:?} dialogues={}",m.map(|m|m.flag("LaskaChapelZagadana")),m.map(|m|m.flag("LaskaCzapelObsluzona")),campaign.dialogue_count);
                p.finished=true;return;
            },
        };
        if p.cursor.apply(step,now,if p.cursor.stage==2 {120.0}else{15.0},&mut p.failure) {p.finished=true;}
        return;
    }
    // Every o_marker_zmienna of the level: stand in it and see its mission variables change.
    if probe.mode=="zmienna" {
        // The scene JSON is parsed once (it is megabytes), the markers are kept in `probe.markers`.
        if probe.markers.is_empty() {
            let scene:serde_json::Value=serde_json::from_str(&std::fs::read_to_string(config.output.join(format!("{}.scene.json",config.world))).unwrap()).unwrap();
            probe.markers=scene["objects"].as_array().unwrap().iter().filter(|o|o["kind"]=="o_marker_zmienna").map(|o|o["properties"].clone()).collect();
            if probe.markers.is_empty() {info!("ZMIENNA PROBE markers=0 failure=None");probe.finished=true;return;}
        }
        let markers=probe.markers.clone();
        let (index,phase)=(probe.stage/2,probe.stage%2);
        if index>=markers.len() {info!("ZMIENNA PROBE markers={} failure={:?}",markers.len(),probe.failure);probe.finished=true;return;}
        let p=&markers[index];
        if phase==0 {
            let center=crate::doors::vector(&p["Pos"]);
            walking.player.position=retail_movement::Vec3::new(center.x,center.y,center.z);walking.player.velocity=retail_movement::Vec3::ZERO;
            probe.stage_start=probe.elapsed;probe.stage+=1;return;
        }
        // The frame time is clamped, so allow several real seconds for the 0.2 s repeat timer.
        let mission=campaign.mission.as_ref();
        let wrong:Vec<String>=[("Set",true),("Unset",false)].into_iter().filter_map(|(key,want)|{let name=p[key].as_str().unwrap_or("");(!name.is_empty() && mission.map(|m|m.flag(name))!=Some(want)).then(||format!("{key} {name} is not {want}"))}).collect();
        if wrong.is_empty() || probe.elapsed-probe.stage_start>6.0 {
            if !wrong.is_empty() {probe.failure=Some(format!("{}: {wrong:?} (player {:?})",p["Name"],walking.player.position));}
            info!("ZMIENNA PROBE {} Set={} Unset={}",p["Name"],p["Set"],p["Unset"]);probe.stage+=1;
        }
        return;
    }
    if probe.mode=="impact" {
        if let Some(flags)=std::env::var("MESTER_TEST_SURFACE").ok().and_then(|v|v.parse().ok()) {surfaces.force(flags);}
        let now=probe.elapsed;let p=&mut *probe;
        let step=match p.cursor.stage {
            0=>{
                let wanted=std::env::var("MESTER_TEST_NPC").unwrap_or_else(|_|"o_postac18".into());
                if let Some(actor)=roster.actors.iter().find(|actor|actor.name==wanted) {
                    place(&mut walking,actor.position+actor.rotation*Vec3::Z*180.0,actor.eye());
                    let pl=walking.player.position;
                    let eye=retail_movement::Vec3::new(pl.x,pl.y+35.0,pl.z);
                    let surface=(0..16).filter_map(|i|{
                        let angle=i as f32*std::f32::consts::TAU/16.0;
                        let dir=retail_movement::Vec3::new(angle.cos(),0.0,angle.sin());
                        walking.world.raycast(eye,dir,400.0).filter(|(distance,normal)|*distance>65.0 && normal.y.abs()<0.4).map(|(distance,_)|(distance,dir))
                    }).min_by(|a,b|a.0.total_cmp(&b.0));
                    if let Some((distance,dir))=surface {
                        let target=Vec3::new(eye.x+dir.x*distance,eye.y+dir.y*distance,eye.z+dir.z*distance);
                        place(&mut walking,Vec3::new(pl.x,pl.y,pl.z),target);
                        info!("IMPACT PROBE wall distance={distance:.1}");
                        native.acquire_and_draw("Glock");Step::Done
                    }else{Step::Fail("no nearby impact wall".into())}
                }else{Step::Fail(format!("impact actor absent: {wanted}"))}
            },
            // Fire until the first shot is counted (a pulse whenever the trigger is free); it must hit the wall.
            1=>{
                if native.shots==0 {if native.inventory.selected.is_some_and(|s|native.inventory.weapons[s].cooldown<=0.0) {controls.fire=true;}Step::Wait("the first Glock shot")}
                else {Step::check(native.hits>=1,||format!("the shot hit nothing (shots={} hits={})",native.shots,native.hits))}
            },
            2=>Step::when(p.cursor.age(now)>=0.5,"the impact effect to play"),
            _=>{info!("IMPACT PROBE shots={} hits={} failure={:?}",native.shots,native.hits,p.failure);p.finished=true;return;},
        };
        if p.cursor.apply(step,now,10.0,&mut p.failure) {p.finished=true;}
        return;
    }
    if probe.mode=="gunfire" {
        // Faces an actor (MESTER_TEST_NPC, default o_postac22) at MESTER_TEST_NPC_DISTANCE: it fights back while the Glock shoots it every half second.
        campaign.health=100.0;
        if probe.stage==0 {
            let mut wanted=std::env::var("MESTER_TEST_NPC").unwrap_or_else(|_|"o_postac22".into());
            // "auto": a hostile actor with a vertical wall 60..160 units behind it (as seen from the player), for the blood splat check.
            if wanted=="auto" {
                let n=|v:Vec3|retail_movement::Vec3::new(v.x,v.y,v.z);
                wanted=roster.actors.iter().filter(|actor|actor.hostile).find(|actor|{
                    let behind=-(actor.rotation*Vec3::Z);
                    walking.world.raycast(n(actor.position+Vec3::Y*20.0),n(behind),300.0).is_some_and(|(distance,normal)|normal.y.abs()<0.3 && (60.0..160.0).contains(&distance))
                }).map(|actor|actor.name.clone()).unwrap_or_default();
                info!("GUNFIRE PROBE auto NPC={wanted}");
            }
            let distance=std::env::var("MESTER_TEST_NPC_DISTANCE").ok().and_then(|value|value.parse::<f32>().ok()).unwrap_or(180.0);
            if let Some(actor)=roster.actors.iter().find(|actor|actor.name==wanted) {let (position,eye)=(actor.position+actor.rotation*Vec3::Z*distance,actor.eye());place(&mut walking,position,eye);}
            else {probe.failure=Some(format!("NPC not found: {wanted}"));}
            let gun=std::env::var("MESTER_TEST_GUN").unwrap_or_else(|_|"Glock".into());
            native.acquire_and_draw(&gun);probe.gun=native.definitions.iter().position(|d|d.id==gun).unwrap_or(1);roster.hostiles_attacking=true;probe.stage=1;
        }
        // MESTER_TEST_HIT=right|left|behind: a synthetic wound from that side at t=2 (blood overlay roll check).
        if probe.stage>=1 && probe.elapsed>=2.0 && !probe.hit_done {probe.hit_done=true;if let Ok(side)=std::env::var("MESTER_TEST_HIT") {
            let p=walking.player.position;let eye=Vec3::new(p.x,p.y+40.0,p.z);
            let camera=Transform::from_translation(eye*crate::SCALE).with_rotation(Quat::from_euler(EulerRot::YXZ,walking.yaw,walking.pitch,0.0));
            let offset=match side.as_str() {"left"=>-crate::mirror::right(&camera),"behind"=>*camera.back(),_=>crate::mirror::right(&camera)};
            crate::gunfire::player_hit(&mut commands,&effects,&camera,eye+offset*200.0,crate::gunfire::Grunt::Bullet(eye));
        }}
        // The levels' `hostileattack` action runs every tick: whoever notices the player is provoked.
        if probe.stage>=1 {roster.provoke_noticing();}
        if probe.stage==1 && probe.elapsed>=1.0 {controls.native_slot=Some(probe.gun);probe.stage=2;}
        if probe.stage==2 && probe.elapsed>=2.5 && probe.elapsed-probe.shot_at>=0.5 && std::env::var_os("MESTER_TEST_HOLD_FIRE").is_none() {controls.fire=true;controls.held=true;probe.shot_at=probe.elapsed;}
        if (probe.elapsed*2.0).floor()!=((probe.elapsed-time.delta_secs())*2.0).floor() {
            let wanted=std::env::var("MESTER_TEST_NPC").unwrap_or_else(|_|"o_postac22".into());
            if let Some(actor)=roster.actors.iter().find(|actor|actor.name==wanted) {info!("GUNFIRE PROBE t={:.1} npc phase={} hp={:.0} hostile={} attacking={} health={:.0}",probe.elapsed,actor.phase,actor.hp,actor.hostile,roster.hostiles_attacking,campaign.health);}
        }
        if probe.elapsed>=std::env::var("MESTER_TEST_END").ok().and_then(|v|v.parse().ok()).unwrap_or(9.0) {info!("GUNFIRE PROBE shots={} hits={} failure={:?}",native.shots,native.hits,probe.failure);
            if std::env::var_os("MESTER_TEST_HOLD_FIRE").is_none() && (native.shots<3 || native.hits<1) {probe.failure.get_or_insert(format!("the Glock did not shoot or hit (shots={} hits={})",native.shots,native.hits));}
            probe.finished=true;}
        return;
    }
    if probe.mode=="npc" {
        if probe.stage==0 {
            let wanted=std::env::var("MESTER_TEST_NPC").unwrap_or_else(|_|"cywil1p".into());
            if let Some(actor)=roster.actors.iter().find(|actor|actor.definition_name==wanted || actor.name==wanted) {
                let side=std::env::var("MESTER_TEST_NPC_SIDE").ok().and_then(|value|value.parse::<f32>().ok()).unwrap_or(1.0).signum();
                let distance=std::env::var("MESTER_TEST_NPC_DISTANCE").ok().and_then(|value|value.parse::<f32>().ok()).unwrap_or(180.0);
                place(&mut walking,actor.position+actor.rotation*Vec3::Z*(distance*side),actor.eye());
            }else {probe.failure=Some(format!("NPC not found: {wanted}"));}
            probe.stage=1;
        }
        if probe.stage==1 && probe.elapsed>=1.0 {
            if let Some(actor)=roster.actors.iter().find(|actor|actor.name==std::env::var("MESTER_TEST_NPC").unwrap_or_default()) {
                let player=Vec3::new(walking.player.position.x,walking.player.position.y,walking.player.position.z);
                info!("NPC PROBE name={} phase={} hostile={} attack={} seen={} dot={:.3}",actor.name,actor.phase,actor.hostile,roster.hostiles_attacking,actor.player_seen(&walking.world,player),(actor.rotation*Vec3::Z).dot((player-actor.position).with_y(0.0).normalize_or_zero()));
            }
            probe.stage=2;
        }
        if probe.elapsed>=5.0 {
            if let Some(actor)=roster.actors.iter().find(|actor|actor.name==std::env::var("MESTER_TEST_NPC").unwrap_or_default()) {
                info!("NPC PROBE LATE name={} phase={} health={:.1}",actor.name,actor.phase,campaign.health);
            }
            probe.finished=true;
        }
        return;
    }
    // Retail start pose (docs/retail-start-view.md): the level begins on its StartPoint facing the `Kierunek` compass direction, never the Rotation.
    if probe.mode=="startpose" {
        let Some((_,pos,kierunek))=crate::start_view::STARTS.iter().find(|s|s.0.eq_ignore_ascii_case(&config.world)) else {probe.failure.get_or_insert(format!("{} is not in start_view::STARTS",config.world));return};
        if probe.elapsed>=0.5 {
            let (forward,expected)=(Vec2::new(-walking.yaw.sin(),-walking.yaw.cos()),crate::start_view::forward(kierunek));
            let p=walking.player.position;
            info!("STARTPOSE PROBE world={} position=({:.0},{:.0},{:.0}) forward=({:.2},{:.2}) expected=({:.0},{:.0}) {kierunek}",config.world,p.x,p.y,p.z,forward.x,forward.y,expected.x,expected.y);
            if (forward-expected).length()>0.01 {probe.failure.get_or_insert(format!("{}: facing ({:.2},{:.2}) but Kierunek {kierunek} is ({},{})",config.world,forward.x,forward.y,expected.x,expected.y));}
            if (p.x-pos[0]).abs()>2.0 || (p.z-pos[2]).abs()>2.0 || (p.y-pos[1]).abs()>60.0 {probe.failure.get_or_insert(format!("{}: start position ({:.0},{:.0},{:.0}) is not the StartPoint {pos:?}",config.world,p.x,p.y,p.z));}
            probe.finished=true;
        }
        return;
    }
    if probe.mode=="uv" {
        if probe.stage==0 {
            place(&mut walking,Vec3::new(-4780.0,0.0,-172.0),Vec3::new(-4688.0,112.0,-172.0));
            probe.stage=1;
        }
        if probe.stage==1 && probe.elapsed>=3.0 {
            place(&mut walking,Vec3::new(-5750.0,0.0,128.0),Vec3::new(-5947.0,440.0,128.0));
            probe.stage=2;
        }
        if probe.elapsed>=5.0 {info!("UV PROBE finished");probe.finished=true;}
        return;
    }
    // Every level of the list, one after the other: the mission script binds, the actors load and the loop runs a moment.
    // MESTER_TEST_FROM / MESTER_TEST_TO (indices into travel::LEVELS, TO exclusive) run a slice, so a run stays inside its timeout.
    if probe.mode=="catalog" {
        campaign.health=100.0;
        let from:usize=std::env::var("MESTER_TEST_FROM").ok().and_then(|v|v.parse().ok()).unwrap_or(0);
        let to:usize=std::env::var("MESTER_TEST_TO").ok().and_then(|v|v.parse().ok()).unwrap_or(crate::travel::LEVELS.len()).min(crate::travel::LEVELS.len());
        let now=probe.elapsed;let p=&mut *probe;
        let index=from+p.cursor.stage;
        if index>=to {session.paused=true;session.page=1;info!("CATALOG FINISHED levels={} failure={:?}",to.saturating_sub(from),p.failure);p.finished=true;return;}
        let (id,_)=crate::travel::LEVELS[index];
        if p.cursor.first() && !config.world.eq_ignore_ascii_case(id) {travel.pending=Some(id.into());}
        let ready=arrived(&config,&front,&campaign,id) && p.cursor.age(now)>=0.5;
        let step=if !ready {Step::Wait("the level to load")}else if campaign.mission.is_none() && !id.eq_ignore_ascii_case("rh1-wiezienie1") {Step::Fail(format!("mission script unavailable: {id}"))}
            else {info!("CATALOG {}: mission={} npcs={}",config.world,campaign.mission.is_some(),roster.actors.len());Step::Done};
        if p.cursor.apply(step,now,40.0,&mut p.failure) {p.finished=true;}
        return;
    }
    // The retail bus: the o_cutscene marker starts the bus ride into the town, its exits lead on to the mayor (starts on rh2-wiezienie2).
    if probe.mode=="bus" {
        campaign.health=100.0;
        let now=probe.elapsed;let p=&mut *probe;
        let scene_position=|config:&ViewerConfig,name:&str|->Option<Vec3> {
            let scene:serde_json::Value=serde_json::from_str(&std::fs::read_to_string(config.output.join(format!("{}.scene.json",config.world))).ok()?).ok()?;
            scene["objects"].as_array()?.iter().find(|o|o["properties"]["Name"]==name).map(|o|crate::doors::vector(&o["properties"]["Pos"]))
        };
        let step=match p.cursor.stage {
            0=>{if p.cursor.first() {
                    if config.world!="rh2-wiezienie2" {p.failure=Some(format!("the bus probe starts on rh2-wiezienie2, not {}",config.world));p.finished=true;return;}
                    place(&mut walking,Vec3::new(1040.0,-506.0,472.0),Vec3::new(1040.0,-506.0,600.0));
                }
                Step::when(opening.active,"the bus marker to start the cutscene")},
            1=>Step::when(arrived(&config,&front,&campaign,"rh3-miasteczko1") && !opening.active,"the town level after the bus ride"),
            2|4=>Step::check(campaign.mission.is_some() && !roster.actors.is_empty(),||format!("no mission or actors in {}",config.world)),
            3|5=>{
                let (wanted,next)=if p.cursor.stage==3 {("koniec","rh3-miasteczko2")}else{("wrota3","burmistrz1")};
                if p.cursor.first() {
                    let Some(target)=scene_position(&config,wanted) else {p.failure=Some(format!("missing exit object: {wanted}"));p.finished=true;return;};
                    place(&mut walking,target+Vec3::Z*100.0,target);doors.request_name=Some(wanted.into());
                }
                Step::when(arrived(&config,&front,&campaign,next),"the exit to lead to the next level")
            },
            _=>{
                p.finished=true;session.paused=true;session.page=1;
                if config.world!="burmistrz1" || campaign.mission.is_none() || volume.volume!=bevy::audio::Volume::Linear(0.0) {p.failure=Some(format!("mayor level / silence failure: {}",config.world));}
                info!("BUSZPRÓBA: world={} failure={:?}",config.world,p.failure);return;
            },
        };
        if p.cursor.apply(step,now,if p.cursor.stage==1 {90.0}else{30.0},&mut p.failure) {p.finished=true;}
        return;
    }
    if probe.mode.starts_with("cell") {
        // Walk from the real StartPoint with ordinary movement and collision (rh1-wiezienie2).
        // No teleport, health override or forced mission flag in this regression.
        keys.release(KeyCode::KeyW);
        let pl=walking.player.position;let position=Vec2::new(pl.x,pl.z);
        let now=probe.elapsed;let p=&mut *probe;
        // MESTER_TEST_TRACE=<definition name>: keep the run going to MESTER_TEST_END s (default 30) and log that actor's world position and phase twice a second.
        let trace=std::env::var("MESTER_TEST_TRACE").ok();
        if let Some(name)=&trace {
            if (now*2.0).floor()!=((now-time.delta_secs())*2.0).floor() {if let Some(a)=roster.actors.iter().find(|a|&a.definition_name==name||&a.name==name) {info!("CELL TRACE t={now:.1} {} pos=({:.0},{:.0},{:.0}) phase={} player=({:.0},{:.0},{:.0})",a.name,a.position.x,a.position.y,a.position.z,a.phase,pl.x,pl.y,pl.z);}}
        }
        let trace_end=trace.as_ref().map_or(0.0,|_|std::env::var("MESTER_TEST_END").ok().and_then(|v|v.parse().ok()).unwrap_or(30.0));
        let step=match p.cursor.stage {
            0=>{let delta=Vec2::new(80.0,700.0)-position;
                if delta.length()<12.0 {Step::Done}else{walking.yaw=(-delta.x).atan2(-delta.y);walking.pitch=0.0;keys.press(KeyCode::KeyW);Step::Wait("the cell exit at (80,700)")}},
            // Stop after crossing and the authored warning; waiting idle here deliberately provokes the guard's original baton attack.
            _=>{
                if campaign.health<=0.0 {Step::Fail("the guard killed the player before the warning".into())}
                else if campaign.dialogue_count>0 && campaign.mission.as_ref().is_some_and(|m|m.flag("IntroOdpalone")) && now>=trace_end {
                    session.paused=true;p.finished=true;
                    if pl.z>=735.0 {p.failure=Some(format!("cell exit: position={pl:?} did not cross z=735"));}
                    info!("CELLAPRÓBA: mode={} position={pl:?} health={} dialogues={} failure={:?}",p.mode,campaign.health,campaign.dialogue_count,p.failure);return;
                }else{Step::Wait("the authored warning dialogue (IntroOdpalone)")}
            },
        };
        if p.cursor.apply(step,now,20.0,&mut p.failure) {p.finished=true;}
        return;
    }
    // This is an interaction/transition probe, not a survival walkthrough: restore health while inspecting stationary
    // actors and distant exits. The separate cell regression above exercises unmodified live health. World: rh1-wiezienie2.
    campaign.health=100.0;session.dialogue_active=campaign.dialogue.as_ref().is_some_and(|d|!d.choices.is_empty());
    let now=probe.elapsed;let p=&mut *probe;
    let guard=roster.actors.iter().find(|a|a.definition_name=="straznik przy celi");
    let step=match p.cursor.stage {
        0=>{if p.cursor.first() {place(&mut walking,Vec3::new(80.0,-3.0,768.0),Vec3::new(80.0,25.0,672.0));}
            Step::when(campaign.dialogue_count>0 && campaign.mission.as_ref().is_some_and(|m|m.flag("IntroOdpalone")),"the original start marker and the guard dialogue (IntroOdpalone)")},
        1=>{if let Some(guard)=guard {place(&mut walking,guard.position+guard.rotation*Vec3::Z*170.0,guard.eye());Step::Done}else{Step::Fail("no guard at the cell".into())}},
        2=>{native.acquire_and_draw("Glock");p.base=native.shots;Step::Done},
        // The (walking) guard is tracked while the shot is fired, so the check does not depend on his pace.
        3=>{
            if native.shots>p.base {Step::Done} else {
            let Some(guard)=guard.filter(|a|a.hp>0.0) else {p.failure=Some("the cell guard was already dead".into());p.finished=true;return;};
            // Stand where a bullet line to him is free (the cell door may be closed; closed doors also push the player out of a
            // spot that looks free), verify it from the real camera position each frame, then fire while tracking him.
            let pl=walking.player.position;let eye=Vec3::new(pl.x,pl.y+46.0,pl.z);
            if p.aimed && !clear_line(&walking,Some(&doors_state),eye,guard.eye()) {p.aimed=false;p.attempt+=1;}
            if !p.aimed {let found=place_with_clear_shot(&mut walking,guard.eye(),&doors_state,p.attempt);p.aimed=found;Step::Wait("a clear shot at the cell guard")}
            else {
                let direction=(guard.eye()-eye).normalize_or_zero();
                if direction!=Vec3::ZERO {walking.yaw=(-direction.x).atan2(-direction.z);walking.pitch=direction.y.asin();}
                if native.shots==p.base {if native.inventory.selected.is_some_and(|s|native.inventory.weapons[s].cooldown<=0.0) && !native.inventory.switching() && p.cursor.age(now)>0.3 {controls.fire=true;}Step::Wait("the Glock shot")}else{Step::Done}
            }
            }
        },
        4=>{
            if guard.is_none_or(|a|a.hp<=0.0) {Step::Done}
            else if p.cursor.age(now)>3.0 {Step::Fail(format!("the Glock shot did not kill the original guard (phase {} hp {})",guard.map_or("?",|g|g.phase.as_str()),guard.map_or(0.0,|g|g.hp)))}
            else {Step::Wait("the guard to fall")}
        },
        5=>{if p.cursor.first() {place(&mut walking,Vec3::new(-1232.0,-190.0,190.0),Vec3::new(-1232.0,-176.0,80.0));doors.request_name=Some("b_door21".into());}
            Step::when(arrived(&config,&front,&campaign,"rh1-wiezienie3") && roster.actors.len()>=60,"the first exit to load rh1-wiezienie3 (60+ NPCs)")},
        // The kitchen prisoner speaks when he sees the player: the second dialogue of the route.
        6=>{if p.cursor.first() {if let Some(npc)=roster.actors.iter().find(|a|a.definition_name=="wiezien_kuchnia") {place(&mut walking,npc.position+npc.rotation*Vec3::Z*170.0,npc.eye());}}
            Step::when(campaign.dialogue_count>=2,"the second dialogue (wiezien_kuchnia)")},
        7=>{if p.cursor.first() {place(&mut walking,Vec3::new(2500.0,-48.0,-144.0),Vec3::new(2620.0,-48.0,-144.0));doors.request_name=Some("b_door0".into());}
            Step::when(arrived(&config,&front,&campaign,"rh2-wiezienie1"),"the second exit to load rh2-wiezienie1")},
        8=>{if p.cursor.first() {place(&mut walking,Vec3::new(832.0,-432.0,-400.0),Vec3::new(952.0,-432.0,-400.0));doors.request_name=Some("sector2b00".into());}
            Step::when(arrived(&config,&front,&campaign,"rh2-wiezienie2"),"the third exit to load rh2-wiezienie2")},
        _=>{
            p.finished=true;session.paused=true;session.page=1;
            if campaign.dialogue_count<2 || volume.volume!=bevy::audio::Volume::Linear(0.0) {p.failure=Some("asset or silence failure".into());}
            info!("KAMPÁNYPRÓBA: world={} dialogues={} shots={} npcs={} failure={:?}",config.world,campaign.dialogue_count,native.shots,roster.actors.len(),p.failure);return;
        },
    };
    if p.cursor.apply(step,now,20.0,&mut p.failure) {p.finished=true;}
}
