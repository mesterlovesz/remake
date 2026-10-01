//! Silent capture probes for scene objects (docs/retail-objects.md). MESTER_TEST_SCENARIO=props stands in front of a prop
//! (MESTER_TEST_PROP=<instance or definition name>, MESTER_TEST_INDEX, MESTER_TEST_DISTANCE) and shoots it with MESTER_TEST_GUN;
//! =door walks up to a door (MESTER_TEST_DOOR=<name>) and presses E; =pickup stands on an item (MESTER_TEST_ITEM=<kind>).
use bevy::prelude::*;
use crate::{ViewerConfig,Walking,settings::Session,campaign::Campaign,gunfire::Controls,retail_weapons::NativeArsenal,props::PropWorld};

#[derive(Default)] pub struct State {elapsed:f32,stage:usize,shot_at:f32,gun:usize,target:Option<usize>,logged:f32}
fn env<T:std::str::FromStr>(key:&str,default:T)->T {std::env::var(key).ok().and_then(|v|v.parse().ok()).unwrap_or(default)}
fn native(v:Vec3)->retail_movement::Vec3 {retail_movement::Vec3::new(v.x,v.y,v.z)}
/// Free standing spot (feet on a floor, box clear) `radius` from `target` with an unobstructed view of it.
pub fn stand_facing(walking:&mut Walking,target:Vec3,radii:&[f32],max_dy:f32)->bool {
    for &radius in radii {for i in 0..32 {
        let angle=env::<f32>("MESTER_TEST_ANGLE",0.0).to_radians()+i as f32*std::f32::consts::TAU/32.0;
        let origin=target+Vec3::new(angle.cos()*radius,30.0,angle.sin()*radius);
        let Some((distance,normal))=walking.world.raycast(native(origin),retail_movement::Vec3::NEG_Y,400.0) else{continue};
        if normal.y<0.65 {continue;}
        let center=origin-Vec3::Y*distance+Vec3::Y*58.2;
        if (center.y-target.y).abs()>max_dy || walking.world.box_overlaps(native(center),retail_movement::Vec3::new(25.0,58.0,25.0)) {continue;}
        let eye=center+Vec3::Y*46.0;
        if !crate::npcs::line_of_sight(&walking.world,eye,target) {continue;}
        walking.player.position=native(center);walking.player.velocity=retail_movement::Vec3::ZERO;
        let direction=(target-eye).normalize();walking.yaw=(-direction.x).atan2(-direction.z);walking.pitch=direction.y.asin();
        return true;
    }}
    false
}
fn field_half(world:&PropWorld,index:usize)->Vec3 {world.field().props[index].half}
pub fn tick(mut probe:ResMut<crate::campaign_probe::Probe>,time:Res<Time>,config:Res<ViewerConfig>,mut walking:ResMut<Walking>,mut session:ResMut<Session>,mut campaign:ResMut<Campaign>,mut controls:ResMut<Controls>,mut native_arsenal:ResMut<NativeArsenal>,world:Res<PropWorld>,mut keys:ResMut<ButtonInput<KeyCode>>,mut clock:ResMut<Time<Virtual>>,mut state:Local<State>,items:Query<&crate::pickups::Pickup>,options:Res<crate::options::Options>,opening:Res<crate::opening::Opening>,doors:Query<&crate::doors::Door>) {
    let mode=std::env::var("MESTER_TEST_SCENARIO").unwrap_or_default();
    if !probe.active || probe.finished || !matches!(mode.as_str(),"props"|"door"|"pickup") {return;}
    state.elapsed+=time.delta_secs();session.paused=false;campaign.health=100.0;
    let window:Vec<f32>=std::env::var("MESTER_TEST_SLOW").unwrap_or_default().split(',').filter_map(|v|v.parse().ok()).collect();
    clock.set_relative_speed(if window.len()==3 && (window[1]..window[2]).contains(&state.elapsed) {window[0]}else{1.0});
    if mode=="props" {
        let wanted=std::env::var("MESTER_TEST_PROP").unwrap_or_default();let index:usize=env("MESTER_TEST_INDEX",0);
        if state.stage==0 && state.elapsed>=0.5 {
            let field=world.field();
            state.target=field.props.iter().enumerate().filter(|(_,p)|!p.dead && (p.name==wanted || p.def==wanted)).map(|(i,_)|i).nth(index);
            let Some(target)=state.target else{probe.failure=Some(format!("no prop {wanted:?} in {} ({} props)",config.world,field.props.len()));probe.finished=true;return};
            let p=&field.props[target];let center=p.center;
            info!("PROPS PROBE target {} ({}) at {:?} half {:?} hp {} solid {}",p.name,p.def,center,p.half,p.hp,p.solid);
            drop(field);
            let distance:f32=env("MESTER_TEST_DISTANCE",220.0);
            if !stand_facing(&mut walking,center,&[distance,distance*0.7,distance*1.4,distance*0.5,distance*2.0],400.0) {probe.failure=Some(format!("no clear standing spot for {wanted}"));probe.finished=true;return;}
            // MESTER_TEST_DROP=1: fall onto the prop from 150 units above (standing on a solid prop needs Player::external_support).
            if env("MESTER_TEST_DROP",0)==1 {let half=field_half(&world,state.target.unwrap());walking.player.position=native(center+Vec3::Y*(half.y+58.0+150.0));walking.player.velocity=retail_movement::Vec3::ZERO;}
            let gun=std::env::var("MESTER_TEST_GUN").unwrap_or_else(|_|"Glock".into());
            native_arsenal.acquire(&gun);state.gun=native_arsenal.definitions.iter().position(|d|d.id==gun).unwrap_or(1);state.stage=1;
        }
        // The retail controller ignores input for its first 60 frames (docs/retail-movement-audit.md): walk from frame 65 on.
        if env("MESTER_TEST_WALK",0)==1 {if walking.player.frames>=65 {keys.press(KeyCode::KeyW);}}
        else if state.stage==1 && state.elapsed>=1.0 {controls.native_slot=Some(state.gun);state.stage=2;}
        let fire_at:f32=env("MESTER_TEST_FIRE_AT",1.5);
        if state.stage==2 && state.elapsed>=fire_at && state.elapsed-state.shot_at>=env("MESTER_TEST_INTERVAL",0.4) && !world.field().props[state.target.unwrap()].dead {controls.fire=true;controls.held=true;state.shot_at=state.elapsed;}
        if state.stage>=1 && state.elapsed-state.logged>=0.5 {
            state.logged=state.elapsed;let field=world.field();let p=&field.props[state.target.unwrap()];
            let p0=walking.player.position;let eye=Vec3::new(p0.x,p0.y+46.0,p0.z);let ray=field.ray(eye,(p.center-eye).normalize_or_zero(),2000.0).map(|h|(h.0,field.props[h.1].name.clone()));
            let flat=Vec2::new(p.center.x-p0.x,p.center.z-p0.z).length();info!("PROPS PROBE walk_distance={flat:.1} py={:.1} ray={ray:?} t={:.1} {} hp={:.0} dead={} at={:?} shots={} hits={} props={}",p0.y,state.elapsed,p.name,p.hp,p.dead,p.center,native_arsenal.shots,native_arsenal.hits,field.props.len());
        }
        if state.elapsed>=env("MESTER_TEST_END",8.0) {probe.finished=true;}
    }
    if mode=="pickup" {
        let wanted=std::env::var("MESTER_TEST_ITEM").unwrap_or_default();let index:usize=env("MESTER_TEST_INDEX",0);
        if state.stage==0 && state.elapsed>=0.5 {
            let Some(item)=items.iter().filter(|i|i.kind==wanted || i.name==wanted).nth(index) else{probe.failure=Some(format!("no item {wanted:?} in {}",config.world));probe.finished=true;return};
            info!("PICKUP PROBE target {} ({}) at {:?} half {:?}",item.name,item.kind,item.position,item.half);
            if !stand_facing(&mut walking,item.position,&[env("MESTER_TEST_DISTANCE",100.0),80.0,120.0,60.0],400.0) {probe.failure=Some("no standing spot".into());probe.finished=true;return;}
            state.stage=1;
        }
        if state.stage==1 && state.elapsed>=env("MESTER_TEST_FIRE_AT",2.5) {if env("MESTER_TEST_HOLD_E",1)==1 {keys.press(KeyCode::KeyE);}state.stage=2;}
        if state.stage>=1 && state.elapsed-state.logged>=0.5 {state.logged=state.elapsed;let p0=walking.player.position;info!("PICKUP PROBE t={:.1} remaining={} held_e={} e_down={} dialogue={} player={:?}",state.elapsed,items.iter().filter(|i|i.kind==wanted || i.name==wanted).count(),state.stage==2,keys.pressed(KeyCode::KeyE),session.dialogue_active,p0);}
        if state.elapsed>=env("MESTER_TEST_END",5.0) {probe.finished=true;}
    }
    if mode=="door" {
        let wanted=std::env::var("MESTER_TEST_DOOR").unwrap_or_default();
        if state.stage==0 && state.elapsed>=0.5 {
            let scene:serde_json::Value=serde_json::from_str(&std::fs::read_to_string(config.output.join(format!("{}.scene.json",config.world))).unwrap()).unwrap();
            let Some(object)=scene["objects"].as_array().unwrap().iter().find(|o|o["properties"]["Name"]==wanted.as_str()) else{probe.failure=Some(format!("no object {wanted}"));probe.finished=true;return};
            let target=crate::doors::vector(&object["properties"]["Pos"]);let distance:f32=env("MESTER_TEST_DISTANCE",90.0);
            if !stand_facing(&mut walking,target,&[distance,distance*1.5,distance*0.6,distance*2.0],300.0) {probe.failure=Some(format!("no standing spot near {wanted}"));}
            state.stage=1;
        }
        if env("MESTER_TEST_WALK",0)==1 {
            if walking.player.frames>=65 {keys.press(KeyCode::KeyW);}
            if state.elapsed-state.logged>=0.5 {state.logged=state.elapsed;let p=walking.player.position;info!("DOOR PROBE t={:.1} player={:?} w={} paused={} dialogue={} vel={:?} grounded={} forward_bound={} opening={}",state.elapsed,p,keys.pressed(KeyCode::KeyW),session.paused,session.dialogue_active,walking.player.velocity,walking.player.grounded,options.keys.bound(crate::keys_cfg::cmd::FORWARD)[0],opening.active);}
        }
        else if state.stage==1 && state.elapsed>=env("MESTER_TEST_FIRE_AT",1.5) {keys.press(KeyCode::KeyE);state.stage=2;}
        if state.stage==2 && state.elapsed>=env("MESTER_TEST_FIRE_AT",1.5)+0.1 {keys.release(KeyCode::KeyE);state.stage=3;}
        // MESTER_TEST_RETREAT=1: 3 s after the door opened, step 260 units away (auto-close needs the player 128+ away) and log the door state.
        if env("MESTER_TEST_RETREAT",0)==1 {
            let target=doors.iter().find(|d|d.name==wanted).map(|d|d.origin());
            if state.stage==3 && state.elapsed>=env("MESTER_TEST_FIRE_AT",1.5)+3.0 {if let Some(target)=target {stand_facing(&mut walking,target,&[260.0,300.0,220.0],300.0);}state.stage=4;}
            if state.elapsed-state.logged>=0.5 {state.logged=state.elapsed;if let Some(d)=doors.iter().find(|d|d.name==wanted) {info!("DOOR PROBE t={:.1} stage={} state={} open={}",state.elapsed,state.stage,d.state_code(),d.is_open());}}
        }
        if state.elapsed>=env("MESTER_TEST_END",6.0) {probe.finished=true;}
    }
}
