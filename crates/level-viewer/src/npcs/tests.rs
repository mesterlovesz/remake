//! Unit tests of the NPC roster and of the retail AI port (`ai.rs`, docs/retail-ai.md).
use super::*;
use super::ai::{slot_of,heading_slot,yaw_of,slot_list,Estimate,estimate_of};
use std::f32::consts::{PI,FRAC_PI_2,FRAC_PI_4};

// ---------------------------------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------------------------------

/// A phase JSON: (name, animation duration, looping, commands).
fn phases(list:&[(&str,f32,bool,&[(&str,&str)])])->serde_json::Value {
    let mut map=serde_json::Map::new();
    for (name,duration,looping,commands) in list {
        map.insert((*name).into(),serde_json::json!({"animation":"idle","duration":duration,"loop":looping,
            "commands":commands.iter().map(|(k,v)|serde_json::json!([k,v])).collect::<Vec<_>>()}));
    }
    serde_json::Value::Object(map)
}
fn character(flags:&[&str],header:&[(&str,&str)],phases:serde_json::Value,weapon:Option<serde_json::Value>)->Character {
    serde_json::from_value(serde_json::json!({
        "model":"guard","skins":{},"styles":{},"default_phase":"idle","hp":10.0,"hostile":true,
        "collision_half_extents":[2.0,5.0,2.0],"flags":flags,"header":header.iter().map(|(k,v)|serde_json::json!([k,v])).collect::<Vec<_>>(),"weapon_asset":weapon,
        "model_animations":{"idle":{"duration":1.0,"dimensions":[2.0,5.0,2.0]}},"phases":phases
    })).unwrap()
}
fn weapon(spread:f32,strength:f32,extra:&[(&str,&str)])->serde_json::Value {
    let mut commands=vec![serde_json::json!(["rozrzut",spread.to_string()]),serde_json::json!(["sila_wroga",strength.to_string()]),
        serde_json::json!(["shot_latency","0.5"]),serde_json::json!(["sound_shoot","sig_s.wav"]),serde_json::json!(["glosnosc","1280"]),serde_json::json!(["ammo_amount","30"])];
    for (k,v) in extra {commands.push(serde_json::json!([k,v]));}
    serde_json::json!({"model":"weapon","animation":"idle","skins":{},"styles":{},"socket":null,"commands":commands})
}
fn actor(name:&str,definition:&Arc<Character>,pos:[f32;3])->Npc {
    Npc::from_spawn(Spawn {name:name.into(),definition_name:"guard".into(),pos,rotation:[0.0;4],source_object_index:0,properties:default()},definition.clone())
}
/// One "guard" at (0, 5.1, 20) facing +Z, already in phase `idle` with the start of the entry done.
fn roster_with(definition:Character)->NpcRoster {
    let definition=Arc::new(definition);
    let mut roster=NpcRoster {actors:vec![actor("o_postac1",&definition,[0.0,5.1,20.0])],..default()};
    roster.set_phase("guard","idle");roster.drain_commands();roster.pending_sounds.clear();roster
}
fn basic_phases()->serde_json::Value {
    phases(&[("idle",1.0,true,&[("on_death","death")]),("death",0.5,false,&[("set","dead"),("on_koniec_anim0","corpse")]),("corpse",1.0,true,&[("unset","alive")]),
        ("shoot",0.5,false,&[("strzal_raz",""),("obrot_do_gracza","")]),("automatic",0.5,true,&[("strzal","0.05"),("obrot_do_gracza","")]),("execution",0.5,false,&[("strzal_raz","")])])
}
fn fixture()->NpcRoster {roster_with(character(&["ruchomy"],&[],basic_phases(),None))}
fn armed_fixture(spread:f32)->NpcRoster {roster_with(character(&["ruchomy"],&[],basic_phases(),Some(weapon(spread,10.0,&[]))))}
fn world(wall:bool)->CollisionWorld {
    let mut obj="v -2000 0 -2000\nv 2000 0 -2000\nv 2000 0 2000\nv -2000 0 2000\nf 1 3 2\nf 1 4 3\n".to_string();
    if wall {obj.push_str("v -2000 0 10\nv 2000 0 10\nv 2000 100 10\nv -2000 100 10\nf 5 6 7\nf 5 7 8\n");}
    CollisionWorld::from_obj(&obj).unwrap()
}
fn target(player:Vec3)->PlayerTarget {PlayerTarget {position:player,half_extents:Vec3::new(2.0,5.0,2.0)}}
fn sim_with(roster:&mut NpcRoster,world:&CollisionWorld,player:Vec3,dt:f32,difficulty:f32) {
    roster.simulate_with_barriers(world,target(player),difficulty,dt,|_,_|false,|_,_,_|None);
}
fn sim(roster:&mut NpcRoster,world:&CollisionWorld,player:Vec3,dt:f32) {sim_with(roster,world,player,dt,1.0);}
fn run(roster:&mut NpcRoster,world:&CollisionWorld,player:Vec3,seconds:f32,dt:f32) {for _ in 0..(seconds/dt).round() as usize {sim(roster,world,player,dt);}}

/// A path graph: nodes (top position, floor 66 below) and (from, slot, to) links, one graph.
fn graph(points:&[[f32;3]],links:&[(usize,usize,usize)])->Navigation {
    Navigation {nodes:points.iter().enumerate().map(|(id,pos)|{
        let mut node=NavNode::new(id,*pos,Vec::new(),0);
        node.commands=links.iter().filter(|(a,_,_)|*a==id).map(|(_,slot,b)|(format!("idx{slot}"),b.to_string())).collect();
        node.commands.push(("posx".into(),"0".into()));node
    }).collect(),index:default()}.indexed()
}
/// Nodes along +Z, 100 apart, at x = 100, walking slots 0 (forward) and 4 (back).
fn line(count:usize)->Navigation {
    let points:Vec<[f32;3]>=(0..count).map(|i|[100.0,66.0,i as f32*100.0]).collect();
    let mut links=Vec::new();
    for i in 0..count {if i+1<count {links.push((i,0,i+1));}if i>0 {links.push((i,4,i-1));}}
    graph(&points,&links)
}
fn number_of(list:&[(String,String)],key:&str)->Option<f32> {number(list,key)}

// ---------------------------------------------------------------------------------------------------------------------
// Angles, slots, picks
// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn compass_slots_follow_the_original_45_degree_sectors() {
    assert_eq!([Vec3::Z,Vec3::new(1.0,0.0,1.0),Vec3::X,Vec3::new(1.0,0.0,-1.0),Vec3::NEG_Z,Vec3::new(-1.0,0.0,-1.0),Vec3::NEG_X,Vec3::new(-1.0,0.0,1.0)].map(slot_of),[0,1,2,3,4,5,6,7]);
    assert_eq!(slot_of(Vec3::new(0.3,0.0,1.0)),0,"17 degrees off the axis is still the axis slot");
    assert_eq!(slot_of(Vec3::new(0.5,0.0,1.0)),1,"27 degrees is the diagonal");
    assert_eq!(slot_of(Vec3::new(1.0,50.0,1.0)),0,"the direction is normalised in 3D: a nearly vertical vector is on the axis slot");
    assert_eq!(slot_of(Vec3::ZERO),5,"the original's 0/0 lands on slot 5");
}
#[test]
fn a_heading_of_slot_s_maps_to_the_next_slot() {
    for s in 0..7 {assert_eq!(heading_slot(s as f32*FRAC_PI_4),s+1,"slot {s}");}
    assert_eq!(heading_slot(7.0*FRAC_PI_4),7);assert_eq!(heading_slot(0.1),1);assert_eq!(heading_slot(-0.1),7);
}
#[test]
fn yaw_is_measured_from_plus_z_towards_plus_x() {
    assert_eq!(yaw_of(0.0,1.0),0.0);assert!((yaw_of(1.0,0.0)-FRAC_PI_2).abs()<1e-6);assert!((yaw_of(0.0,-1.0)-PI).abs()<1e-6);assert!((yaw_of(-1.0,0.0)-3.0*FRAC_PI_2).abs()<1e-6);
}
#[test]
fn numbered_callbacks_are_consecutive_from_slot_zero_and_picked_uniformly() {
    let commands:Vec<(String,String)>=[("on_koniec0","a"),("on_koniec1","b"),("on_koniec2","c"),("on_koniec4","never"),("on_koniec5","")].iter().map(|(k,v)|(k.to_string(),v.to_string())).collect();
    let list=slot_list(&commands,"on_koniec",8);assert_eq!(list,vec!["a","b","c"],"the gap at slot 3 ends the list");
    let mut roster=NpcRoster::default();let mut seen=std::collections::BTreeSet::new();
    for _ in 0..200 {seen.insert(roster.pick(&list).unwrap());}
    assert_eq!(seen.into_iter().collect::<Vec<_>>(),vec!["a".to_string(),"b".into(),"c".into()]);
    assert_eq!(roster.pick(&["only"]).as_deref(),Some("only"));assert!(roster.pick(&[]).is_none());
}
#[test]
fn estimate_flags_dispatch_in_the_original_order() {
    let c=|keys:&[&str]|keys.iter().map(|k|(k.to_string(),String::new())).collect::<Vec<_>>();
    assert_eq!(estimate_of(&c(&["estimate_kluczy","estimate_do_gracza"])),Some(Estimate::Chase));
    assert_eq!(estimate_of(&c(&["estimate_kluczy","estimate_do_strzalu"])),Some(Estimate::Strafe));
    assert_eq!(estimate_of(&c(&["estimate_od_gracza"])),Some(Estimate::Flee));assert_eq!(estimate_of(&c(&["speed"])),None);
}

// ---------------------------------------------------------------------------------------------------------------------
// Path graph
// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn nodes_resolve_their_compass_slots_and_floor_position() {
    let nav=line(3);
    assert_eq!(nav.nodes[0].slots[0],Some(1));assert_eq!(nav.nodes[1].slots[4],Some(0));assert_eq!(nav.nodes[1].slots[0],Some(2));assert_eq!(nav.nodes[0].slots[4],None);
    assert_eq!(nav.real(1),Vec3::new(100.0,0.0,100.0),"without real_pos the floor is 66 below the top");
    assert_eq!(nav.place_of(Vec3::new(100.0,5.0,190.0)),Some(2));assert_eq!(nav.place_of(Vec3::new(1.0,1.0,1.0)),None,"within 32 of the origin nothing is found");
}
#[test]
fn the_path_search_is_a_greedy_depth_first_search_towards_the_goal() {
    let nav=line(4);
    assert_eq!(nav.path(0,Vec3::new(100.0,0.0,300.0)),vec![0,1,2,3]);
    assert_eq!(nav.path(3,Vec3::new(100.0,0.0,0.0)),vec![3,2,1,0]);
    assert_eq!(nav.path(2,Vec3::new(100.0,0.0,205.0)),vec![2],"the start can be the goal");
    // A detour that first moves away from the goal is never taken, although a route exists.
    let detour=graph(&[[0.0,66.0,0.0],[100.0,66.0,0.0],[100.0,66.0,100.0],[0.0,66.0,100.0]],&[(0,2,1),(1,0,2),(2,6,3),(1,6,0),(2,4,1),(3,2,2)]);
    assert!(detour.path(0,Vec3::new(0.0,0.0,100.0)).is_empty(),"0 -> 1 leaves the goal farther away than 0 itself");
    // 48 nodes is the limit.
    let long=line(60);
    assert!(long.path(0,Vec3::new(100.0,0.0,5500.0)).is_empty());assert_eq!(long.path(0,Vec3::new(100.0,0.0,4700.0)).len(),48);
}
#[test]
fn a_ridge_between_two_floors_stops_the_greedy_search_only_by_distance() {
    // The neighbour must not be farther from the goal than the current node, equal is fine.
    let nav=graph(&[[0.0,66.0,0.0],[0.0,66.0,100.0],[100.0,66.0,100.0]],&[(0,0,1),(1,2,2)]);
    assert_eq!(nav.path(0,Vec3::new(100.0,0.0,100.0)),vec![0,1,2],"1 is nearer to the goal than 0, 2 is the goal");
}

// ---------------------------------------------------------------------------------------------------------------------
// Perception and contact
// ---------------------------------------------------------------------------------------------------------------------

fn watcher(header:&[(&str,&str)])->NpcRoster {
    let mut roster=roster_with(character(&["ruchomy"],header,phases(&[("idle",1.0,true,&[]),("watch",1.0,true,&[("on_kontakt","alarm"),("sound_on_kontakt","sounds\\enemies\\police\\halt0.wav"),("sounds_on_kontakt","8")]),("alarm",1.0,true,&[])]),None));
    roster.set_phase("guard","watch");roster.drain_commands();roster
}
#[test]
fn contact_needs_the_cone_the_distance_and_a_free_line() {
    let front=Vec3::new(0.0,5.0,120.0);
    let mut roster=watcher(&[("odleglosc_kontaktu","640"),("kat_kontaktu","60")]);
    sim(&mut roster,&world(false),front,0.05);
    assert!(roster.actors[0].contact && roster.actors[0].seen,"in front, 100 units away, nothing between");
    let mut wide=watcher(&[("odleglosc_kontaktu","640"),("kat_kontaktu","60")]);
    sim(&mut wide,&world(false),Vec3::new(400.0,5.0,40.0),0.05);
    assert!(!wide.actors[0].contact,"about 87 degrees off the heading, outside the 60 degree cone");
    let mut far=watcher(&[("odleglosc_kontaktu","640"),("kat_kontaktu","60")]);
    sim(&mut far,&world(false),Vec3::new(0.0,5.0,900.0),0.05);assert!(!far.actors[0].contact,"beyond odleglosc_kontaktu");
    let mut blocked=watcher(&[("odleglosc_kontaktu","640"),("kat_kontaktu","60")]);
    sim(&mut blocked,&world(true),Vec3::new(0.0,5.0,-100.0),0.05);assert!(!blocked.actors[0].seen && !blocked.actors[0].contact,"a wall hides the player");
}
#[test]
fn the_player_behind_is_only_noticed_within_48_units() {
    let mut near=watcher(&[("kat_kontaktu","179")]);sim(&mut near,&world(false),Vec3::new(0.0,5.0,-20.0),0.05);assert!(near.actors[0].contact,"30 units behind");
    let mut behind=watcher(&[("kat_kontaktu","179")]);sim(&mut behind,&world(false),Vec3::new(0.0,5.0,-100.0),0.05);assert!(!behind.actors[0].contact,"120 units behind: the cone is not used");
}
#[test]
fn without_a_cone_angle_the_front_is_never_noticed() {
    let mut roster=watcher(&[]);sim(&mut roster,&world(false),Vec3::new(0.0,5.0,80.0),0.05);
    assert!(!roster.actors[0].contact,"kat_kontaktu defaults to 0 in the original");
}
#[test]
fn contact_alone_changes_nothing_until_a_script_provokes_the_actor() {
    let mut roster=watcher(&[("odleglosc_kontaktu","640"),("kat_kontaktu","60")]);let front=Vec3::new(0.0,5.0,120.0);
    sim(&mut roster,&world(false),front,0.05);
    assert_eq!(roster.actors[0].phase,"watch");assert!(roster.actors[0].noticed_player() && !roster.actors[0].provoked);
    roster.provoke_noticing();assert!(roster.hostiles_attacking && roster.actors[0].provoked);
    sim(&mut roster,&world(false),front,0.05);
    assert_eq!(roster.actors[0].phase,"alarm","hostileattack lets on_kontakt start");
    assert_eq!(roster.pending_sounds.len(),1);let (path,_,radius)=&roster.pending_sounds[0];
    assert!(path.starts_with("sounds\\enemies\\police\\halt") && path.ends_with(".wav") && *radius==2048.0,"seen: 2048");
    assert!(roster.noises.iter().any(|s|s.always==128.0 && s.los==640.0 && s.cone==1280.0),"the shout alerts the others");
}
#[test]
fn hostileattack_only_reaches_actors_that_notice_the_player_at_that_moment() {
    let mut roster=watcher(&[("odleglosc_kontaktu","640"),("kat_kontaktu","60")]);
    roster.provoke_noticing();sim(&mut roster,&world(false),Vec3::new(0.0,5.0,120.0),0.05);
    assert_eq!(roster.actors[0].phase,"watch","nobody noticed anything when the command ran");
}
#[test]
fn a_lost_player_clears_contact_and_provocation() {
    let mut roster=watcher(&[("odleglosc_kontaktu","640"),("kat_kontaktu","60")]);
    sim(&mut roster,&world(false),Vec3::new(0.0,5.0,120.0),0.05);roster.actors[0].provoked=true;
    sim_with_phase_reset(&mut roster);
    fn sim_with_phase_reset(roster:&mut NpcRoster) {
        // The player steps behind a wall: no contact, and the pending provocation is gone (0x10049a57).
        sim(roster,&world(true),Vec3::new(0.0,5.0,-200.0),0.05);
        assert!(!roster.actors[0].contact && !roster.actors[0].provoked);
    }
}
#[test]
fn stimuli_are_noticed_inside_their_radii_and_expire() {
    let mut roster=watcher(&[]);let far_behind=Vec3::new(0.0,5.0,-900.0);let at=roster.actors[0].position;
    roster.add_stimulus(at+Vec3::X*100.0,128.0,0.0,0.0,1);
    sim(&mut roster,&world(false),far_behind,0.05);assert!(roster.actors[0].contact,"inside the always radius");
    assert!(roster.noises.is_empty(),"one frame of life");
    sim(&mut roster,&world(false),far_behind,0.05);assert!(!roster.actors[0].contact,"the noise is over");
    roster.add_stimulus(at+Vec3::X*300.0,128.0,0.0,0.0,1);sim(&mut roster,&world(false),far_behind,0.05);assert!(!roster.actors[0].contact,"outside every radius");
    roster.add_stimulus(at+Vec3::X*300.0,128.0,1280.0,0.0,1);sim(&mut roster,&world(false),far_behind,0.05);assert!(roster.actors[0].contact,"a loud shot with a free line");
    roster.add_stimulus(at+Vec3::new(0.0,0.0,-200.0),128.0,1280.0,0.0,1);sim(&mut roster,&world(true),far_behind,0.05);assert!(!roster.actors[0].contact,"a wall blocks the line");
    roster.add_stimulus(at+Vec3::X*500.0,0.0,0.0,640.0,1);sim(&mut roster,&world(false),far_behind,0.05);assert!(roster.actors[0].contact,"the cone radius: any direction with a free line, the 60 degree test never fails");
    roster.add_stimulus(at+Vec3::new(0.0,0.0,2.0),128.0,0.0,0.0,2);sim(&mut roster,&world(false),far_behind,0.05);sim(&mut roster,&world(false),far_behind,0.05);
    assert!(roster.actors[0].contact,"two frames of life");
}
#[test]
fn laser_dot_and_flashlight_are_640_unit_stimuli() {
    let mut roster=watcher(&[]);let at=roster.actors[0].position;
    roster.visual_stimuli.push(at+Vec3::new(0.0,0.0,-300.0));sim(&mut roster,&world(false),Vec3::new(0.0,5.0,-900.0),0.05);assert!(roster.actors[0].contact,"behind the actor, still noticed");
    roster.visual_stimuli=vec![at+Vec3::new(0.0,0.0,700.0)];sim(&mut roster,&world(false),Vec3::new(0.0,5.0,-900.0),0.05);assert!(!roster.actors[0].contact,"beyond 640");
}
#[test]
fn seen_is_cached_for_distance_over_2560_seconds() {
    let mut roster=fixture();roster.set_phase("guard","idle");
    sim(&mut roster,&world(false),Vec3::new(0.0,5.0,1300.0),0.01);
    assert!(roster.actors[0].seen);assert!((roster.actors[0].seen_timer-1280.0/2560.0+0.01*0.0).abs()<0.02,"{}",roster.actors[0].seen_timer);
    // A wall appears: the cached answer holds until the timer runs out.
    sim(&mut roster,&world(true),Vec3::new(0.0,5.0,-1300.0),0.01);assert!(roster.actors[0].seen,"cached");
    run(&mut roster,&world(true),Vec3::new(0.0,5.0,-1300.0),0.6,0.05);assert!(!roster.actors[0].seen,"refreshed");
}
#[test]
fn nie_patrz_na_gracza_never_sees() {
    let mut roster=roster_with(character(&["ruchomy","nie_patrz_na_gracza"],&[],basic_phases(),None));
    sim(&mut roster,&world(false),Vec3::new(0.0,5.0,100.0),0.05);assert!(!roster.actors[0].seen);
}

// ---------------------------------------------------------------------------------------------------------------------
// Phase transitions
// ---------------------------------------------------------------------------------------------------------------------

fn transit(commands:&[(&str,&str)],player:Vec3,wall:bool)->String {
    let mut roster=roster_with(character(&["ruchomy"],&[("ucieka_jak_mniej_niz","0")],phases(&[("idle",1.0,true,&[]),("wait",0.5,false,commands),
        ("near",1.0,true,&[]),("near_seen",1.0,true,&[]),("far",1.0,true,&[]),("far_seen",1.0,true,&[]),("anim",1.0,true,&[]),("hurt",1.0,true,&[]),("estimate",1.0,true,&[])]),None));
    roster.set_phase("guard","wait");roster.drain_commands();
    run(&mut roster,&world(wall),player,0.6,0.05);
    roster.actors[0].phase.clone()
}
#[test]
fn a_finished_phase_picks_its_callback_by_distance_and_sight() {
    let cmds=[("do_gracza","100"),("on_closer0","near"),("on_closer_widzi0","near_seen"),("on_further0","far"),("on_further_widzi0","far_seen")];
    assert_eq!(transit(&cmds,Vec3::new(0.0,5.0,80.0),false),"near_seen","close and seen");
    assert_eq!(transit(&cmds,Vec3::new(0.0,5.0,-120.0),true),"far","farther than do_gracza and unseen (a wall)");
    assert_eq!(transit(&cmds,Vec3::new(0.0,5.0,500.0),false),"far_seen","farther and seen");
    assert_eq!(transit(&[("do_gracza","100"),("on_closer0","near")],Vec3::new(0.0,5.0,50.0),true),"near","closer without a sight variant");
    assert_eq!(transit(&[("do_gracza","100"),("on_closer_widzi0","near_seen")],Vec3::new(0.0,5.0,-50.0),true),"estimate","the sight variant needs sight; nothing else applies: the estimate phase");
}
#[test]
fn distance_callbacks_need_do_gracza_above_one_and_stop_at_4096() {
    assert_eq!(transit(&[("on_closer0","near")],Vec3::new(0.0,5.0,40.0),false),"estimate","do_gracza 0: on_closer never applies");
    assert_eq!(transit(&[("do_gracza","1"),("on_closer0","near")],Vec3::new(0.0,5.0,20.5),false),"estimate","do_gracza must exceed 1");
    assert_eq!(transit(&[("on_further_widzi0","far_seen")],Vec3::new(0.0,5.0,900.0),false),"far_seen","on_further works without do_gracza");
    assert_eq!(transit(&[("on_further_widzi0","far_seen")],Vec3::new(0.0,5.0,4200.0),false),"estimate","not beyond 4096");
}
#[test]
fn animation_end_and_retreat_outrank_the_distance_callbacks() {
    let cmds=[("do_gracza","100"),("on_closer0","near"),("on_koniec_anim0","anim")];
    assert_eq!(transit(&cmds,Vec3::new(0.0,5.0,50.0),false),"anim");
    let mut hurt=roster_with(character(&["ruchomy"],&[("ucieka_jak_mniej_niz","20")],phases(&[("idle",1.0,true,&[]),("wait",0.5,false,&[("on_koniec_anim0","anim"),("on_hurt0","hurt")]),("anim",1.0,true,&[]),("hurt",1.0,true,&[])]),None));
    hurt.set_phase("guard","wait");run(&mut hurt,&world(false),Vec3::new(0.0,5.0,300.0),0.6,0.05);assert_eq!(hurt.actors[0].phase,"hurt","hp 10 < 20 takes the retreat first");
}
#[test]
fn a_phase_that_names_no_way_out_returns_to_the_default_phase_or_the_estimate_phase() {
    assert_eq!(transit(&[],Vec3::new(0.0,5.0,50.0),false),"estimate","a phase called estimate wins");
    let mut roster=roster_with(character(&["ruchomy"],&[],phases(&[("idle",1.0,true,&[]),("wait",0.5,false,&[])]),None));
    roster.set_phase("guard","wait");run(&mut roster,&world(false),Vec3::new(0.0,5.0,50.0),0.6,0.05);assert_eq!(roster.actors[0].phase,"idle");
}
#[test]
fn looping_phases_never_leave_by_themselves() {
    let mut roster=roster_with(character(&["ruchomy"],&[],phases(&[("idle",1.0,true,&[]),("loop",0.5,true,&[("on_koniec_anim0","idle"),("on_closer0","idle"),("do_gracza","500")]),("other",0.5,true,&[])]),None));
    roster.set_phase("guard","loop");run(&mut roster,&world(false),Vec3::new(0.0,5.0,50.0),3.0,0.05);assert_eq!(roster.actors[0].phase,"loop");
}
#[test]
fn a_looping_phase_that_keeps_its_clip_does_not_restart_it() {
    let mut roster=roster_with(character(&["ruchomy"],&[],phases(&[("idle",1.0,true,&[]),("again",1.0,true,&[]),("cut",1.0,false,&[])]),None));
    run(&mut roster,&world(false),Vec3::new(0.0,5.0,500.0),0.5,0.05);let before=roster.actors[0].elapsed;
    roster.set_phase("guard","again");assert!((roster.actors[0].elapsed-before).abs()<1e-6,"same looping clip continues");
    roster.set_phase("guard","cut");assert_eq!(roster.actors[0].elapsed,0.0,"a non-looping phase restarts");
}
#[test]
fn death_then_animation_end_queue_instance_commands_once() {
    let mut roster=fixture();let world=world(false);
    roster.hit_scan(Vec3::new(0.0,5.0,0.0),Vec3::Z,100.0,20.0,&world).unwrap();
    let pending=roster.drain_commands();assert_eq!(pending.len(),1);assert_eq!(pending[0].0,"o_postac1");assert!(pending[0].1.contains(&("set".into(),"dead".into())));
    assert!(!roster.actors[0].alive());
    sim(&mut roster,&world,Vec3::ZERO,0.6);assert!(roster.drain_commands().is_empty(),"a dead actor no longer changes phase (0x10041c60 refuses)");
}

// ---------------------------------------------------------------------------------------------------------------------
// Walking
// ---------------------------------------------------------------------------------------------------------------------

fn patrolling(count:usize,start:usize)->NpcRoster {
    let mut roster=roster_with(character(&["ruchomy"],&[],phases(&[("idle",1.0,true,&[]),
        ("walk",1.0,true,&[("patrol",""),("speed","50"),("on_koniec0","walk"),("on_koniec1","walk"),("on_koniec2","walk")])]),None));
    roster.navigation=line(count);
    roster.actors[0].position=Vec3::new(100.0,5.0,start as f32*100.0+0.0);
    roster.set_phase("guard","walk");roster.drain_commands();roster
}
#[test]
fn a_patrol_walks_the_segment_at_the_phase_speed_and_hops_at_the_node() {
    let mut roster=patrolling(3,1);let world=world(false);let far=Vec3::new(-900.0,5.0,-900.0);
    sim(&mut roster,&world,far,0.05);
    assert!(roster.actors[0].moving && roster.actors[0].place==Some(1));
    run(&mut roster,&world,far,0.95,0.05);
    let a=&roster.actors[0];let delta=a.position-Vec3::new(100.0,5.0,100.0);
    assert!(delta.x.abs()<1e-3 && delta.y.abs()<1e-3 && (delta.z.abs()-50.0).abs()<3.0,"one second at 50 units/s along the link: {:?}",a.position);
    run(&mut roster,&world,far,1.5,0.05);
    let a=&roster.actors[0];assert!(a.place!=Some(1),"arrived at the neighbour and chose the next one");assert!((a.position.y-5.0).abs()<1e-3,"floor + half height");
}
#[test]
fn a_patrol_turns_its_heading_to_the_link_slot() {
    let mut roster=patrolling(3,0);sim(&mut roster,&world(false),Vec3::new(-900.0,5.0,-900.0),0.05);
    assert_eq!(roster.actors[0].yaw_target,0.0,"slot 0 is +Z");
    roster.set_phase("guard","idle");roster.actors[0].position=Vec3::new(100.0,5.0,200.0);roster.actors[0].place=Some(2);roster.actors[0].moving=false;roster.actors[0].next=None;roster.actors[0].yaw_target=0.0;roster.set_phase("guard","walk");
    sim(&mut roster,&world(false),Vec3::new(-900.0,5.0,-900.0),0.05);assert!((roster.actors[0].yaw_target-PI).abs()<1e-5,"only slot 4 is left: -Z");
}
#[test]
fn a_node_taken_by_another_actor_is_not_a_patrol_target() {
    let mut roster=patrolling(2,0);
    let definition=roster.actors[0].definition.clone();
    let mut other=actor("o_postac2",&definition,[100.0,5.0,100.0]);other.place=Some(1);roster.actors.push(other);
    sim(&mut roster,&world(false),Vec3::new(-900.0,5.0,-900.0),0.05);
    assert!(!roster.actors[0].moving,"the only neighbour is occupied");assert_eq!(roster.actors[0].phase,"idle","no way: back to the default phase");
}
#[test]
fn a_patrol_step_needs_a_similar_floor_height_and_no_door() {
    let mut roster=patrolling(2,0);roster.navigation.nodes[1].real[1]=20.0;
    sim(&mut roster,&world(false),Vec3::new(-900.0,5.0,-900.0),0.05);assert_eq!(roster.actors[0].phase,"idle","20 units higher than the place: not within 8");
    let mut roster=patrolling(2,0);roster.navigation.nodes[0].door[0]=true;
    sim(&mut roster,&world(false),Vec3::new(-900.0,5.0,-900.0),0.05);
    assert!(roster.actors[0].moving,"after the door-free tries fail the last resort ignores doors (0x100481c1)");
}
#[test]
fn switching_phase_with_the_snap_flag_jumps_to_the_target_node() {
    let mut roster=patrolling(3,0);let far=Vec3::new(-900.0,5.0,-900.0);
    run(&mut roster,&world(false),far,0.3,0.05);assert!(roster.actors[0].moving);let before=roster.actors[0].position;
    let next=roster.actors[0].next.unwrap();
    roster.set_phase("guard","idle");
    let a=&roster.actors[0];
    assert!(!a.moving && a.place==Some(next),"the walker arrived at its target");
    assert!((a.position+a.vel-before).length()<1e-3,"the model still shows the old spot and glides");
    let glide=a.vel.length();
    sim(&mut roster,&world(false),far,1.0/60.0);assert!(roster.actors[0].vel.length()<glide,"the offset fades");
}
#[test]
fn a_forced_switch_without_the_snap_flag_does_not_move_the_actor() {
    let mut roster=patrolling(3,0);let far=Vec3::new(-900.0,5.0,-900.0);
    run(&mut roster,&world(false),far,0.3,0.05);let before=roster.actors[0].position;
    let _=roster.enter_phase(0,"idle",false);assert_eq!(roster.actors[0].position,before);
}

fn chaser()->NpcRoster {
    let mut roster=roster_with(character(&["ruchomy"],&[],phases(&[("idle",1.0,true,&[]),
        ("chase",1.0,true,&[("estimate_do_gracza",""),("speed","200"),("do_gracza","24"),("on_koniec0","idle")])]),None));
    roster.navigation=line(6);roster.actors[0].position=Vec3::new(100.0,5.0,0.0);roster
}
#[test]
fn the_chase_walks_towards_the_player_and_stops_one_node_short_on_a_fresh_path() {
    let mut roster=chaser();let world=world(false);let player=Vec3::new(100.0,5.0,340.0);
    roster.set_phase("guard","chase");sim(&mut roster,&world,player,0.01);
    assert_eq!(roster.actors[0].path_len,4,"the goal is the node nearest to the point 24 units short of the player: node 3");
    assert_eq!(&roster.actors[0].path_buf[..4],&[Some(0),Some(1),Some(2),Some(3)]);
    run(&mut roster,&world,player,3.0,0.02);
    // The two-node look-ahead reads path[count], an unset entry, which counts as occupied: the walk ends at path[count - 2].
    assert_eq!(roster.actors[0].phase,"idle");assert_eq!(roster.actors[0].place,Some(2));
    assert!((roster.actors[0].position-Vec3::new(100.0,5.0,200.0)).length()<1e-3);
}
#[test]
fn actors_ask_for_doors_at_the_phase_start_and_at_nodes_next_to_a_door_link() {
    let mut roster=chaser();let world=world(false);let player=Vec3::new(100.0,5.0,340.0);
    roster.set_phase("guard","chase");sim(&mut roster,&world,player,0.01);
    assert_eq!(roster.door_requests,vec![Vec3::new(100.0,37.0,0.0)],"after the estimate: the actor's own position + 32 (0x100420b3)");
    run(&mut roster,&world,player,3.0,0.02);
    assert_eq!(roster.stats.door_asks,1,"no door on any link: no arrival request");
    let mut roster=chaser();roster.navigation.nodes[1].door[0]=true;
    roster.set_phase("guard","chase");let mut points=Vec::new();
    for _ in 0..150 {sim(&mut roster,&world,player,0.02);points.extend(roster.door_openers());}
    assert!(points.len()>1,"the link 1 -> 2 has a door");
    for point in &points[1..] {assert!((0..4).any(|n|(roster.navigation.real(n)+Vec3::Y*32.0).distance(*point)<1e-3),"asked at a node's floor + 32: {point:?}");}
}
#[test]
fn a_stale_path_entry_lets_the_walk_reach_the_goal() {
    let mut roster=chaser();let world=world(false);let player=Vec3::new(100.0,5.0,340.0);
    roster.actors[0].path_buf[4]=Some(4);
    roster.set_phase("guard","chase");run(&mut roster,&world,player,3.0,0.02);
    assert_eq!(roster.actors[0].place,Some(3),"the leftover entry of an earlier path is free, so the walk continues to the goal");
}
#[test]
fn the_chase_ends_early_when_the_player_is_within_do_gracza() {
    let mut roster=chaser();let world=world(false);
    roster.set_phase("guard","chase");sim(&mut roster,&world,Vec3::new(100.0,5.0,340.0),0.01);
    run(&mut roster,&world,Vec3::new(100.0,5.0,105.0),1.0,0.02);
    assert_eq!(roster.actors[0].phase,"idle");assert!(roster.actors[0].position.z<150.0,"stopped at the first node");
}
#[test]
fn a_chase_without_a_path_ends_through_on_koniec() {
    let mut roster=chaser();roster.navigation=Navigation::default();roster.actors[0].place=None;
    roster.set_phase("guard","chase");sim(&mut roster,&world(false),Vec3::new(100.0,5.0,340.0),0.01);
    assert_eq!(roster.actors[0].phase,"idle");
}
#[test]
fn a_chase_far_from_the_player_ends_at_once() {
    let mut roster=chaser();roster.set_phase("guard","chase");sim(&mut roster,&world(false),Vec3::new(100.0,5.0,9000.0),0.01);
    assert_eq!(roster.actors[0].phase,"idle");assert!(!roster.actors[0].moving);
}
#[test]
fn fleeing_heads_for_the_node_nearest_the_point_far_behind_the_actor() {
    let mut roster=roster_with(character(&["ruchomy"],&[],phases(&[("idle",1.0,true,&[]),("flee",1.0,true,&[("estimate_od_gracza",""),("speed","100"),("on_koniec0","idle")])]),None));
    roster.navigation=line(14);roster.actors[0].position=Vec3::new(100.0,5.0,100.0);
    roster.set_phase("guard","flee");sim(&mut roster,&world(false),Vec3::new(100.0,5.0,0.0),0.01);
    // Straight away from the player at z = 0: 1024 units further, the node at z = 1100.
    assert_eq!(roster.actors[0].path_buf[roster.actors[0].path_len-1],Some(11));
}
#[test]
fn hiding_picks_the_nearest_node_with_a_free_slot_that_the_players_eye_cannot_see() {
    let mut roster=roster_with(character(&["ruchomy"],&[],phases(&[("idle",1.0,true,&[]),("hide",1.0,true,&[("estimate_kryjowka",""),("speed","100"),("on_koniec0","idle")])]),None));
    // The wall at z = 10 hides the node at z = -100 from a player at z = 60.
    roster.navigation=graph(&[[100.0,66.0,20.0],[150.0,66.0,20.0],[100.0,66.0,-100.0],[100.0,66.0,60.0]],&[(0,0,3),(3,4,0),(0,4,2),(2,0,0),(0,2,1),(1,6,0)]);
    roster.actors[0].position=Vec3::new(100.0,5.0,20.0);roster.set_phase("guard","hide");sim(&mut roster,&world(true),Vec3::new(100.0,5.0,60.0),0.01);
    let a=&roster.actors[0];assert!(a.path_len>=2 && a.path_buf[a.path_len-1]==Some(2),"path {:?}",&a.path_buf[..a.path_len]);
    let mut open=roster_with(character(&["ruchomy"],&[],phases(&[("idle",1.0,true,&[]),("hide",1.0,true,&[("estimate_kryjowka",""),("on_koniec0","idle")])]),None));
    open.navigation=roster.navigation;open.actors[0].position=Vec3::new(100.0,5.0,20.0);open.set_phase("guard","hide");sim(&mut open,&world(false),Vec3::new(100.0,5.0,60.0),0.01);
    assert_ne!(open.actors[0].path_buf[0],Some(2),"without cover nothing is hidden and the actor closes in like a chase");
}
#[test]
fn a_sidestep_chain_that_hides_from_the_player_plays_the_run_clip() {
    let mut roster=roster_with(character(&["ruchomy"],&[],phases(&[("idle",1.0,true,&[]),("hide",1.0,true,&[("estimate_kryjowka",""),("speed","100"),("on_koniec0","idle")])]),None));
    // The player at z = 60 looks over the wall (z = 10) at a chain that runs sideways (slot 2 from an actor that faces slot 0): the nodes at x = 60 and 120 are behind a second wall at x = 30.
    let mut nav=graph(&[[0.0,66.0,-50.0],[60.0,66.0,-50.0],[120.0,66.0,-50.0]],&[(0,2,1),(1,2,2)]);
    nav.nodes[0].real=[0.0,0.0,-50.0];
    roster.navigation=nav;roster.actors[0].position=Vec3::new(0.0,5.0,-50.0);
    roster.actors[0].definition=Arc::new({let mut c=(*roster.actors[0].definition).clone();c.model_animations.insert("bieg_prawa".into(),Clip {dimensions:[2.0,5.0,2.0]});c});
    roster.set_phase("guard","hide");sim(&mut roster,&world(true),Vec3::new(0.0,5.0,60.0),0.01);
    let a=&roster.actors[0];assert_eq!(a.path_len,2,"the far end of the chain, two nodes");assert_eq!(a.path_buf[..2],[Some(1),Some(2)]);
    assert_eq!(a.anim,"bieg_prawa");assert!(a.face_player);
}
#[test]
fn the_zigzag_takes_a_node_two_links_away_at_right_angles_to_the_player() {
    let mut roster=roster_with(character(&["ruchomy"],&[],phases(&[("idle",1.0,true,&[]),("zig",1.0,true,&[("estimate_kluczy",""),("speed","100"),("on_koniec0","idle")])]),None));
    roster.navigation=graph(&[[0.0,66.0,100.0],[100.0,66.0,100.0],[200.0,66.0,100.0],[-100.0,66.0,100.0],[-200.0,66.0,100.0]],&[(0,2,1),(1,2,2),(0,6,3),(3,6,4)]);
    roster.actors[0].position=Vec3::new(0.0,5.0,100.0);
    roster.set_phase("guard","zig");sim(&mut roster,&world(false),Vec3::new(0.0,5.0,600.0),0.01);
    let a=&roster.actors[0];assert_eq!(a.path_len,2);let ends=[a.path_buf[0],a.path_buf[1]];
    assert!(ends==[Some(1),Some(2)] || ends==[Some(3),Some(4)],"{ends:?}");
}
#[test]
fn the_sidestep_for_a_shot_runs_along_a_slot_until_a_node_sees_the_player() {
    let mut roster=roster_with(character(&["ruchomy"],&[],phases(&[("idle",1.0,true,&[]),("side",1.0,true,&[("estimate_do_strzalu",""),("speed","100"),("on_koniec0","idle")])]),None));
    // Chains of four nodes to both sides (slots 2 and 6) of the actor's node; the player far ahead sees them all.
    let mut points=vec![[500.0,66.0,100.0]];let mut links=Vec::new();
    for side in [1.0f32,-1.0] {
        let first=points.len();
        for k in 0..4 {points.push([500.0+side*100.0*(k as f32+1.0),66.0,100.0]);}
        links.push((0,if side>0.0 {2}else{6},first));
        for k in 0..3 {links.push((first+k,if side>0.0 {2}else{6},first+k+1));}
    }
    roster.navigation=graph(&points,&links);roster.actors[0].position=Vec3::new(500.0,5.0,100.0);
    roster.set_phase("guard","side");sim(&mut roster,&world(false),Vec3::new(500.0,5.0,900.0),0.01);
    let a=&roster.actors[0];
    assert_eq!(a.path_len,2,"the first candidate (the second node of the chain) already sees the player");
    assert!(a.path_buf[0]==Some(1) || a.path_buf[0]==Some(5),"the chain's first node: {:?}",a.path_buf[0]);
    assert!(a.moving && a.chase==false);
    // A wall (z = 10) hides every node from a player behind it: no chain qualifies, the actor gives up (on_koniec).
    let mut hidden=roster_with(character(&["ruchomy"],&[],phases(&[("idle",1.0,true,&[]),("side",1.0,true,&[("estimate_do_strzalu",""),("speed","100"),("on_koniec0","idle")])]),None));
    hidden.navigation=roster.navigation;hidden.actors[0].position=Vec3::new(500.0,5.0,100.0);
    hidden.set_phase("guard","side");sim(&mut hidden,&world(true),Vec3::new(500.0,5.0,-500.0),0.01);
    assert_eq!(hidden.actors[0].phase,"idle");
}
#[test]
fn a_walk_can_fire_at_every_other_node() {
    let mut roster=roster_with(character(&["ruchomy"],&[],phases(&[("idle",1.0,true,&[]),
        ("run",1.0,true,&[("estimate_do_gracza",""),("moze_strzelac_w_wezle",""),("speed","400"),("do_gracza","24"),("on_koniec0","idle")])]),Some(weapon(0.0,10.0,&[]))));
    roster.navigation=line(8);roster.actors[0].position=Vec3::new(100.0,5.0,0.0);
    roster.set_phase("guard","run");run(&mut roster,&world(false),Vec3::new(100.0,5.0,700.0),1.6,0.02);
    let shots=roster.pending_shots.len();assert!(shots>=2 && shots<=5,"{shots} shots over the hops");
}

// ---------------------------------------------------------------------------------------------------------------------
// Turning
// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn the_yaw_follows_its_target_in_two_steps_per_frame_at_pi_times_mod_obrotu() {
    let mut roster=roster_with(character(&["ruchomy"],&[],phases(&[("idle",1.0,true,&[]),("face",1.0,true,&[("obrot_do_gracza","")]),("slow",1.0,true,&[("obrot_do_gracza",""),("mod_obrotu","0.5")])]),None));
    roster.set_phase("guard","face");sim(&mut roster,&world(false),Vec3::new(400.0,5.1,20.0),0.05);
    assert!((roster.actors[0].yaw-2.0*PI*0.05).abs()<1e-4,"{}",roster.actors[0].yaw);
    run(&mut roster,&world(false),Vec3::new(400.0,5.1,20.0),1.0,0.05);assert!((roster.actors[0].yaw-FRAC_PI_2).abs()<1e-4,"arrived without overshoot");
    assert!(((roster.actors[0].rotation*Vec3::Z)-Vec3::X).length()<1e-3);
    roster.set_phase("guard","slow");roster.actors[0].yaw=0.0;sim(&mut roster,&world(false),Vec3::new(400.0,5.1,20.0),0.05);
    assert!((roster.actors[0].yaw-PI*0.05).abs()<1e-4,"mod_obrotu 0.5 halves the rate");
}
#[test]
fn the_yaw_turns_the_short_way_round() {
    let mut roster=roster_with(character(&["ruchomy"],&[],phases(&[("idle",1.0,true,&[]),("face",1.0,true,&[("obrot_do_gracza","")])]),None));
    roster.actors[0].yaw=0.1;roster.set_phase("guard","face");
    sim(&mut roster,&world(false),Vec3::new(-100.0,5.1,-100.0),0.05);// target 225 degrees: counter-clockwise is shorter
    assert!(roster.actors[0].yaw<0.1 || roster.actors[0].yaw>6.0,"{}",roster.actors[0].yaw);
}
#[test]
fn a_turn_that_crosses_north_completes_at_once_like_the_original() {
    let mut roster=roster_with(character(&["ruchomy"],&[],phases(&[("idle",1.0,true,&[]),("face",1.0,true,&[("obrot_do_gracza","")])]),None));
    // From 315 degrees to 30 degrees the short way (+75 degrees) crosses the seam: the clamp against the numeric target fires on the first step.
    roster.actors[0].yaw=5.5;roster.set_phase("guard","face");
    let target=Vec3::new(0.5f32.sin(),0.0,0.5f32.cos())*400.0+Vec3::new(0.0,5.1,20.0);
    sim(&mut roster,&world(false),target,0.05);assert!((roster.actors[0].yaw-0.5).abs()<1e-4,"{}",roster.actors[0].yaw);
    // The long way round the other side turns smoothly.
    roster.actors[0].yaw=1.5;let behind=Vec3::new(0.0,5.1,-380.0);sim(&mut roster,&world(false),behind,0.05);assert!((roster.actors[0].yaw-(1.5+2.0*PI*0.05)).abs()<1e-4,"{}",roster.actors[0].yaw);
}
#[test]
fn nie_obracaj_freezes_the_heading() {
    let mut roster=roster_with(character(&["ruchomy","nie_obracaj"],&[],phases(&[("idle",1.0,true,&[]),("face",1.0,true,&[("obrot_do_gracza","")])]),None));
    roster.set_phase("guard","face");run(&mut roster,&world(false),Vec3::new(400.0,5.1,20.0),1.0,0.05);assert_eq!(roster.actors[0].yaw,0.0);
}
#[test]
fn obrot_turns_relative_to_the_current_heading() {
    let mut roster=roster_with(character(&["ruchomy"],&[],phases(&[("idle",1.0,true,&[]),("quarter",1.0,true,&[("obrot","90")])]),None));
    roster.set_phase("guard","quarter");assert!((roster.actors[0].yaw_target-FRAC_PI_2).abs()<1e-5);
    run(&mut roster,&world(false),Vec3::new(0.0,5.0,900.0),1.0,0.05);assert!((roster.actors[0].yaw-FRAC_PI_2).abs()<1e-4);
}
#[test]
fn the_gun_pitch_follows_the_players_elevation_at_one_radian_per_second_only_in_aiming_phases() {
    let mut roster=roster_with(character(&["ruchomy"],&[],phases(&[("idle",1.0,true,&[]),("aim",1.0,true,&[("obrot_pion_do_gracza0","Bip01 Spine1")])]),None));
    roster.set_phase("guard","aim");let above=Vec3::new(0.0,205.0,120.0);
    sim(&mut roster,&world(false),above,0.05);assert!((roster.actors[0].pitch-0.05).abs()<1e-6);
    run(&mut roster,&world(false),above,1.5,0.05);assert!(roster.actors[0].pitch>0.5 && (roster.actors[0].pitch-roster.actors[0].pitch_target).abs()<1e-4);
    roster.set_phase("guard","idle");sim(&mut roster,&world(false),above,0.05);assert_eq!(roster.actors[0].pitch,0.0,"other phases reset it");
}

// ---------------------------------------------------------------------------------------------------------------------
// Weapons and damage
// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn strzal_fires_every_n_seconds_and_strzal_raz_once_at_phase_entry() {
    let mut roster=armed_fixture(0.0);let world=world(false);let player=Vec3::new(0.0,5.0,1000.0);
    roster.set_phase("guard","automatic");
    sim(&mut roster,&world,player,0.01);assert_eq!(roster.pending_shots.len(),0,"the timer starts at 0");
    run(&mut roster,&world,player,0.24,0.01);assert_eq!(roster.pending_shots.len(),4,"0.05 s apart: shots at 0.05, 0.10, 0.15, 0.20");
    let mut once=armed_fixture(0.0);once.set_phase("guard","shoot");sim(&mut once,&world,player,0.01);assert_eq!(once.pending_shots.len(),1);
    run(&mut once,&world,player,2.0,0.05);assert_eq!(once.pending_shots.len(),1,"strzal_raz is a single shot");
}
#[test]
fn the_items_shot_latency_is_not_the_npc_fire_rate() {
    let mut roster=armed_fixture(0.0);roster.set_phase("guard","automatic");
    run(&mut roster,&world(false),Vec3::new(0.0,5.0,1000.0),0.6,0.01);
    assert!(roster.pending_shots.len()>=10,"weapon shot_latency 0.5 does not slow a `strzal 0.05` phase: {}",roster.pending_shots.len());
}
#[test]
fn the_gun_points_along_the_actors_heading_not_at_the_player() {
    let mut roster=armed_fixture(0.0);roster.set_phase("guard","execution");
    // The player stands to the side of the guard, who faces +Z and does not turn in this phase.
    sim(&mut roster,&world(false),Vec3::new(500.0,5.0,20.0),0.01);
    assert_eq!(roster.drain_damage(),0.0);let shot=&roster.pending_shots[0];assert!(shot.endpoint.x.abs()<1.0 && shot.endpoint.z>shot.origin.z,"{:?}",shot.endpoint);
}
#[test]
fn a_bullet_that_hits_the_player_costs_trunc_strength_times_difficulty_times_1_3() {
    for (difficulty,damage) in [(0.33,4.0),(0.67,8.0),(1.0,13.0)] {
        let mut roster=armed_fixture(0.0);roster.set_phase("guard","shoot");
        sim_with(&mut roster,&world(false),Vec3::new(0.0,5.0,1000.0),0.01,difficulty);
        assert_eq!(roster.drain_damage(),damage,"difficulty {difficulty}");
    }
}
#[test]
fn source_spread_can_miss_a_player_in_clear_view() {
    let mut roster=armed_fixture(128.0);roster.set_phase("guard","shoot");
    sim(&mut roster,&world(false),Vec3::new(0.0,5.0,1000.0),0.01);
    assert_eq!(roster.drain_damage(),0.0);assert_eq!(roster.pending_sounds.len(),1,"a miss still fires the weapon");
}
#[test]
fn a_shot_needs_a_clear_line_and_stops_at_walls() {
    let mut roster=armed_fixture(0.0);roster.set_phase("guard","shoot");
    sim(&mut roster,&world(true),Vec3::new(0.0,5.0,-500.0),0.01);
    // The guard faces +Z: the wall (z = 10) is behind it, the player behind that wall: no hit either way.
    assert_eq!(roster.drain_damage(),0.0);
}
#[test]
fn enemy_bullets_hurt_other_characters_in_the_line_of_fire_with_sila_wroga() {
    let mut roster=armed_fixture(0.0);let definition=roster.actors[0].definition.clone();
    let mut between=actor("o_postac2",&definition,[0.0,5.1,300.0]);between.hp=25.0;roster.actors.push(between);
    roster.set_phase("guard","shoot");sim(&mut roster,&world(false),Vec3::new(0.0,5.0,1000.0),0.01);
    assert_eq!(roster.actors[1].hp,15.0,"sila_wroga 10, difficulty does not scale it");assert_eq!(roster.drain_damage(),0.0,"the actor in the way takes the bullet");
}
#[test]
fn pellet_weapons_fire_kul_na_raz_rays_from_one_round() {
    let mut roster=roster_with(character(&["ruchomy"],&[],basic_phases(),Some(weapon(0.0,10.0,&[("kul_na_raz","3")]))));
    roster.set_phase("guard","shoot");sim(&mut roster,&world(false),Vec3::new(0.0,5.0,300.0),0.01);
    assert_eq!(roster.drain_damage(),39.0,"three pellets of 13");assert_eq!(roster.actors[0].ammo[0],29,"one round");
}
#[test]
fn every_pellet_is_its_own_shot_for_the_presentation_and_a_bystander_is_marked_as_the_victim() {
    let mut roster=roster_with(character(&["ruchomy"],&[],basic_phases(),Some(weapon(0.0,10.0,&[("kul_na_raz","3")]))));
    roster.set_phase("guard","shoot");sim(&mut roster,&world(false),Vec3::new(0.0,5.0,300.0),0.01);
    let shots=roster.drain_shots();
    assert_eq!(shots.len(),3,"one EnemyShot per pellet");assert_eq!(shots.iter().filter(|s|!s.extra).count(),1,"the muzzle is shown once");assert!(shots.iter().all(|s|s.hits_player && !s.victim));
    let mut roster=armed_fixture(0.0);let definition=roster.actors[0].definition.clone();
    roster.actors.push(actor("o_postac2",&definition,[0.0,5.1,300.0]));roster.actors[1].hp=25.0;
    roster.set_phase("guard","shoot");sim(&mut roster,&world(false),Vec3::new(0.0,5.0,1000.0),0.01);
    let shot=roster.drain_shots().remove(0);assert!(shot.victim && !shot.hits_player,"the bullet ends on the bystander");
}
#[test]
fn the_seen_flag_needs_no_on_kontakt_phase_but_the_contact_flag_does() {
    // ifplayerseenby / setallfaza / ifhostileblizejniz read `seen` (0x1001a060), ifseenbyhostile the contact flag (0x10019f70).
    let mut roster=fixture();sim(&mut roster,&world(false),Vec3::new(0.0,5.0,300.0),0.05);sim(&mut roster,&world(false),Vec3::new(0.0,5.0,300.0),0.5);
    assert!(roster.actors[0].seen_player(),"the player's eye sees the chest of a character with no contact phase");assert!(!roster.actors[0].noticed_player(),"but there is no contact flag without on_kontakt");
    let mut hidden=fixture();sim(&mut hidden,&world(true),Vec3::new(0.0,5.0,-300.0),0.5);assert!(!hidden.actors[0].seen_player(),"a wall in between");
    let mut blind=fixture();Arc::make_mut(&mut blind.actors[0].definition).flags.push("nie_patrz_na_gracza".into());sim(&mut blind,&world(false),Vec3::new(0.0,5.0,300.0),0.5);
    assert!(!blind.actors[0].seen_player(),"nie_patrz_na_gracza: never seen");
}
#[test]
fn the_stimulus_constructors_carry_the_retail_radii() {
    let at=Vec3::new(0.0,10.0,0.0);let radii=|s:Stimulus|(s.always,s.los,s.cone,s.life);
    assert_eq!(radii(Stimulus::player_gunshot(at,1280.0)),(128.0,1280.0,0.0,1),"the player's shot lives one frame (0x10005dd9, flag set)");
    assert_eq!(radii(Stimulus::gunshot(at,1280.0)),(128.0,1280.0,0.0,2),"an actor's shot two");
    assert_eq!(radii(Stimulus::impact(at)),(200.0,0.0,1024.0,2));
    assert_eq!(radii(Stimulus::wound(at,false)),(192.0,1024.0,0.0,2));assert_eq!(radii(Stimulus::wound(at,true)),(64.0,0.0,1024.0,2),"a melee weapon's wound is seen, not heard (0x10043113)");
    assert_eq!(Stimulus::wound(at,false).pos.y,106.0,"96 above the body");assert_eq!(Stimulus::death(at).pos.y,76.0);
    assert_eq!(radii(Stimulus::death(at)),(256.0,1280.0,2048.0,2));assert_eq!(radii(Stimulus::shout(at)),(128.0,640.0,1280.0,2));
    assert_eq!(radii(Stimulus::explosion(at)),(196.0,2048.0,0.0,1));assert_eq!(radii(Stimulus::light(at)),(0.0,0.0,640.0,1));
    let mut roster=fixture();let at=roster.actors[0].body_center();
    roster.melee_hits(&[at],5.0);
    assert!(roster.noises.iter().any(|s|s.always==64.0 && s.cone==1024.0),"the nightstick's blow makes the melee wound noise");
    roster.noises.clear();roster.hit_scan(at+Vec3::new(0.0,0.0,-80.0),Vec3::Z,200.0,5.0,&world(false)).unwrap();
    assert!(roster.noises.iter().any(|s|s.always==192.0 && s.los==1024.0),"a bullet wound is heard");
}
#[test]
fn the_clip_is_ammo_amount_and_an_empty_one_reloads_on_the_next_trigger_pull() {
    let mut roster=roster_with(character(&["ruchomy"],&[],phases(&[("idle",1.0,true,&[]),("fire",0.5,true,&[("strzal","0.05"),("on_reload","reload")]),("reload",1.0,true,&[])]),
        Some(serde_json::json!({"model":"weapon","animation":"idle","skins":{},"styles":{},"socket":null,"commands":[["rozrzut","0"],["sila_wroga","10"],["ammo_amount","2"],["max_ammo","50"]]}))));
    assert_eq!(roster.actors[0].ammo[0],2);
    roster.set_phase("guard","fire");let far=Vec3::new(0.0,5.0,1000.0);
    run(&mut roster,&world(false),far,0.12,0.01);assert_eq!(roster.pending_shots.len(),2);assert_eq!(roster.actors[0].phase,"fire","both rounds fired, the reload waits for the next pull");
    run(&mut roster,&world(false),far,0.1,0.01);assert_eq!(roster.actors[0].phase,"reload");assert_eq!(roster.pending_shots.len(),2,"the reload pull shoots nothing");
    assert_eq!(roster.actors[0].ammo[0],2,"refilled");
}
#[test]
fn a_shot_makes_noise_that_other_actors_notice() {
    let mut roster=armed_fixture(0.0);roster.set_phase("guard","shoot");sim(&mut roster,&world(false),Vec3::new(0.0,5.0,1000.0),0.01);
    assert!(roster.noises.iter().any(|s|s.always==128.0 && s.los==1280.0 && s.life==1),"128 always, glosnosc with a free line, two frames minus the one that passed");
}
#[test]
fn a_shot_relocates_a_firing_actor_that_cannot_see_the_player() {
    let mut roster=roster_with(character(&["ruchomy"],&[],phases(&[("idle",1.0,true,&[]),("fire",1.0,true,&[("strzal","0.05")]),("do_strzalu",1.0,true,&[])]),Some(weapon(0.0,10.0,&[]))));
    roster.set_phase("guard","fire");
    for _ in 0..40 {sim(&mut roster,&world(true),Vec3::new(0.0,5.0,-500.0),0.01);}
    assert_eq!(roster.actors[0].phase,"do_strzalu","a 1 in 4 chance per frame when the weapon cannot see the player");
    let mut visible=roster_with(character(&["ruchomy"],&[],phases(&[("idle",1.0,true,&[]),("fire",1.0,true,&[("strzal","0.05")]),("do_strzalu",1.0,true,&[])]),Some(weapon(0.0,10.0,&[]))));
    visible.set_phase("guard","fire");for _ in 0..40 {sim(&mut visible,&world(false),Vec3::new(0.0,5.0,500.0),0.01);}
    assert_eq!(visible.actors[0].phase,"fire");
}
#[test]
fn the_bite_costs_10_inside_do_gracza_and_the_baton_blow_40_anywhere() {
    let mut roster=roster_with(character(&["ruchomy"],&[],phases(&[("idle",1.0,true,&[]),("bite",1.0,true,&[("gryzie",""),("do_gracza","30")]),("club",1.0,true,&[("odepchnij_gracza","")])]),None));
    roster.set_phase("guard","bite");sim(&mut roster,&world(false),Vec3::new(0.0,5.0,40.0),0.01);assert_eq!(roster.drain_damage(),10.0);
    assert!(roster.pending_shots.iter().any(|s|s.melee),"the wound is presented");
    let mut far=roster_with(character(&["ruchomy"],&[],phases(&[("idle",1.0,true,&[]),("bite",1.0,true,&[("gryzie",""),("do_gracza","30")])]),None));
    far.set_phase("guard","bite");sim(&mut far,&world(false),Vec3::new(0.0,5.0,80.0),0.01);assert_eq!(far.drain_damage(),0.0,"out of reach");
    roster.set_phase("guard","club");sim(&mut roster,&world(false),Vec3::new(0.0,5.0,400.0),0.01);assert_eq!(roster.drain_damage(),40.0);
}
#[test]
fn hp_is_subtracted_and_the_actor_dies_below_zero_only() {
    let mut roster=fixture();let w=world(false);
    let hit=roster.hit_scan(Vec3::new(0.0,5.0,0.0),Vec3::Z,100.0,10.0,&w).unwrap();
    assert!(!hit.killed && roster.actors[0].hp==0.0 && roster.actors[0].alive(),"10 damage on 10 HP leaves 0: still alive");
    assert!(roster.hit_scan(Vec3::new(0.0,5.0,0.0),Vec3::Z,100.0,0.5,&w).unwrap().killed);assert!(!roster.actors[0].alive());
    let mut blast=fixture();blast.actors[0].hp=(640.0-100.0)*0.78125;blast.actors[0].position=Vec3::new(0.0,20.0,100.0);blast.blast(Vec3::new(0.0,20.0,0.0),&w,|_,_|false);
    assert!(!blast.actors[0].alive(),"an explosion kills at exactly zero");
}
#[test]
fn a_hit_is_not_a_reaction() {
    let mut roster=roster_with(character(&["ruchomy"],&[("ucieka_jak_mniej_niz","0")],phases(&[("idle",1.0,true,&[("on_hurt0","hurt"),("on_kontakt","alarm"),("on_death","death")]),("hurt",1.0,true,&[]),("alarm",1.0,true,&[]),("death",1.0,true,&[])]),None));
    roster.provoke_noticing();
    let hit=roster.hit_scan(Vec3::new(0.0,5.0,80.0),Vec3::NEG_Z,100.0,1.0,&world(false)).unwrap();
    assert!(!hit.killed && roster.actors[0].phase=="idle","no flinch, no on_hurt, no contact phase");
    assert!(roster.noises.iter().any(|s|s.always==192.0 && s.los==1024.0),"but the hit makes a noise");
}
#[test]
fn killing_enters_the_on_death_phase_and_marks_the_body() {
    let mut roster=fixture();roster.actors[0].hp=5.0;roster.actors[0].position=Vec3::new(0.0,20.0,100.0);
    let hit=roster.hit_scan(Vec3::new(0.0,20.0,0.0),Vec3::Z,300.0,50.0,&world(false)).unwrap();
    assert!(hit.killed && !hit.stains);assert_eq!(roster.actors[0].phase,"death");assert!(roster.actors[0].dead);
    assert_eq!(roster.drain_deaths(),vec![(Vec3::new(0.0,20.0,100.0),false)]);assert!(roster.drain_deaths().is_empty());
    assert!(roster.noises.iter().any(|s|s.always==256.0 && s.los==1280.0 && s.cone==2048.0),"a death is noisy");
    let mut stained=fixture();Arc::make_mut(&mut stained.actors[0].definition).flags.push("plama_krwi".into());
    stained.actors[0].hp=5.0;stained.actors[0].position=Vec3::new(0.0,20.0,100.0);
    assert!(stained.hit_scan(Vec3::new(0.0,20.0,0.0),Vec3::Z,300.0,50.0,&world(false)).unwrap().stains);assert!(stained.drain_deaths()[0].1);
}
#[test]
fn a_flyer_hovers_mod_y_above_its_path_position_and_is_hit_there() {
    let mut roster=roster_with(character(&["ruchomy"],&[],phases(&[("idle",1.0,true,&[]),("fly",1.0,true,&[("mod_y","480")])]),None));
    roster.set_phase("guard","fly");
    assert_eq!(roster.actors[0].body_center(),Vec3::new(0.0,485.1,20.0));assert!(roster.actors[0].eye().y>485.0);
    let w=world(false);
    assert!(roster.hit_scan(Vec3::new(0.0,5.0,0.0),Vec3::Z,100.0,5.0,&w).is_none(),"the ground position is empty");
    assert!(roster.hit_scan(Vec3::new(0.0,485.0,0.0),Vec3::Z,100.0,5.0,&w).is_some(),"the hover position is not");
    assert_eq!(roster.actors[0].position.y,5.1,"the logical position stays on the path");
}
#[test]
fn characters_that_are_not_mobile_take_no_damage_and_hp_zero_is_a_living_corpse() {
    let mut rat=roster_with(character(&[],&[],basic_phases(),None));
    let hit=rat.hit_scan(Vec3::new(0.0,5.0,0.0),Vec3::Z,100.0,50.0,&world(false)).unwrap();assert!(!hit.killed && rat.actors[0].hp==10.0,"only `ruchomy` characters take Hit");
    let mut corpse=roster_with({let mut c=character(&["ruchomy"],&[],basic_phases(),None);c.hp=Some(0.0);c});
    assert!(corpse.actors[0].alive(),"HP 0 does not mean dead");
    assert!(corpse.hit_scan(Vec3::new(0.0,5.0,0.0),Vec3::Z,100.0,1.0,&world(false)).unwrap().killed,"the first hit does");
    let mut unspecified=roster_with({let mut c=character(&["ruchomy"],&[],basic_phases(),None);c.hp=None;c});
    assert_eq!(unspecified.actors[0].hp,0.0,"HP defaults to 0 like the zeroed original");let _=&mut unspecified;
}
#[test]
fn a_wounded_actor_heals_one_point_per_second_below_its_retreat_limit() {
    let mut roster=roster_with(character(&["ruchomy"],&[("ucieka_jak_mniej_niz","20")],basic_phases(),None));
    roster.actors[0].hp=5.0;run(&mut roster,&world(false),Vec3::new(0.0,5.0,900.0),2.0,0.05);assert!((roster.actors[0].hp-7.0).abs()<0.01,"{}",roster.actors[0].hp);
    roster.actors[0].hp=25.0;run(&mut roster,&world(false),Vec3::new(0.0,5.0,900.0),1.0,0.05);assert_eq!(roster.actors[0].hp,25.0,"healthy actors do not heal");
}
#[test]
fn corpses_vanish_after_thirty_unseen_seconds_unless_nie_respawnuj() {
    let mut roster=fixture();roster.actors[0].hp=-1.0;roster.actors[0].dead=true;roster.actors[0].dead_for=31.0;
    sim(&mut roster,&world(true),Vec3::new(0.0,5.0,-900.0),0.05);assert!(!roster.actors[0].visible);
    let mut kept=fixture();kept.actors[0].dead=true;kept.actors[0].dead_for=31.0;Arc::make_mut(&mut kept.actors[0].definition).flags.push("nie_respawnuj".into());
    sim(&mut kept,&world(true),Vec3::new(0.0,5.0,-900.0),0.05);assert!(kept.actors[0].visible);
    let mut seen=fixture();seen.actors[0].dead=true;seen.actors[0].dead_for=31.0;
    sim(&mut seen,&world(false),Vec3::new(0.0,5.0,-900.0),0.05);assert!(seen.actors[0].visible,"a corpse in view stays");
}

// ---------------------------------------------------------------------------------------------------------------------
// Roster API (unchanged rules)
// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn personal_umbrella_face_changes_only_the_prologue_head_slot() {
    let original=BTreeMap::from([("0".into(),"body".into()),("1".into(),"head".into()),("2".into(),"umbrella".into())]);
    let mut target=original.clone();
    apply_umbrella_face("rh3-miasteczko0","cywil1p",&mut target);
    assert_eq!(target["0"],"body");assert_eq!(target["2"],"umbrella");assert_eq!(target["1"],"mods/umbrella_face.png");
    let mut other=original.clone();
    apply_umbrella_face("rh3-miasteczko1","cywil1p",&mut other);apply_umbrella_face("rh3-miasteczko0","policjant",&mut other);
    assert_eq!(other,original);
}
#[test]
fn nightstick_damages_each_actor_whose_box_holds_a_probe_point_once() {
    let mut roster=fixture();
    let hits=roster.melee_hits(&[Vec3::new(0.0,5.0,16.0),Vec3::new(0.0,5.0,19.0),Vec3::new(0.0,5.0,21.0),Vec3::new(0.0,5.0,30.0)],4.0);
    assert_eq!(hits.len(),1);assert_eq!(roster.actors[0].hp,6.0);
    assert!(roster.melee_hits(&[Vec3::new(2.0,5.0,20.0),Vec3::new(0.0,10.1,20.0)],4.0).is_empty(),"faces are outside");
    assert!(roster.melee_hits(&[Vec3::new(0.0,5.0,20.0)],40.0)[0].killed);
    assert!(roster.melee_hits(&[Vec3::new(0.0,5.0,20.0)],40.0).is_empty(),"the dead take no more");
}
#[test]
fn laser_ray_meets_the_living_box_without_hurting_it() {
    let roster=fixture();
    assert!((roster.ray_distance(Vec3::new(0.0,5.0,0.0),Vec3::Z,6400.0).unwrap()-18.0).abs()<1e-3);
    assert!(roster.ray_distance(Vec3::new(0.0,5.0,0.0),Vec3::Z,10.0).is_none());assert_eq!(roster.actors[0].hp,10.0);
}
#[test]
fn explosion_damages_each_visible_actor_once_without_a_reaction() {
    let mut roster=fixture();let mut other=fixture().actors.remove(0);
    roster.actors[0].hp=1000.0;roster.actors[0].position=Vec3::new(0.0,20.0,100.0);
    other.name="far".into();other.hp=1000.0;other.position=Vec3::new(0.0,20.0,300.0);other.phase="idle".into();roster.actors.push(other);
    let center=Vec3::new(0.0,20.0,0.0);roster.blast(center,&world(false),|_,_|false);
    assert_eq!(roster.actors[0].hp,1000.0-(640.0-100.0)*0.78125);assert_eq!(roster.actors[1].hp,1000.0-(640.0-300.0)*0.78125);
    assert_eq!(roster.actors[0].phase,"idle");
    let before=roster.actors.iter().map(|a|a.hp).collect::<Vec<_>>();
    roster.blast(center,&world(false),|_,_|true);assert_eq!(before,roster.actors.iter().map(|a|a.hp).collect::<Vec<_>>());
    assert!(roster.noises.iter().any(|s|s.always==196.0 && s.los==2048.0),"an explosion is heard");
}
#[test]
fn shot_cannot_cross_static_wall_or_damage_a_farther_actor() {
    let mut roster=fixture();
    assert!(roster.hit_scan(Vec3::new(0.0,5.0,0.0),Vec3::Z,100.0,20.0,&world(true)).is_none());assert_eq!(roster.actors[0].hp,10.0);
    let hit=roster.hit_scan(Vec3::new(0.0,5.0,0.0),Vec3::Z,100.0,20.0,&world(false)).unwrap();
    assert!(hit.killed);assert_eq!(hit.name,"o_postac1");assert!((hit.distance-18.0).abs()<0.01);
}
#[test]
fn setfaza_reaches_the_first_placed_instance_and_never_a_corpse() {
    let mut roster=fixture();let mut other=fixture().actors.remove(0);other.name="o_postac2".into();roster.actors.push(other);
    roster.set_phase("guard","shoot");let pending=roster.drain_commands();assert_eq!(pending.len(),1);assert_eq!(pending[0].0,"o_postac1");
    roster.set_phase("o_postac2","shoot");assert_eq!(roster.drain_commands()[0].0,"o_postac2","a scene name picks that instance");
    roster.actors[0].dead=true;roster.actors[0].phase="corpse".into();
    roster.set_phase("guard","shoot");assert!(roster.drain_commands().is_empty());assert_eq!(roster.actors[0].phase,"corpse","a dead prisoner must not stand up");
}
#[test]
fn setallfaza_reaches_every_living_instance_that_sees_the_player() {
    let mut roster=fixture();for name in ["o_postac2","o_postac3"] {let mut other=fixture().actors.remove(0);other.name=name.into();roster.actors.push(other);}
    roster.actors[2].dead=true;
    roster.set_phase_seeing("guard","shoot",&[true,false,true]);
    let names:Vec<_>=roster.drain_commands().into_iter().map(|c|c.0).collect();assert_eq!(names,vec!["o_postac1"]);
}
#[test]
fn emitter_is_absent_until_explicit_activation_and_only_fires_once() {
    let mut roster=fixture();let mut dormant=roster.actors.remove(0);dormant.visible=false;
    roster.dormant.insert("o_emiter1".into(),dormant);assert!(roster.actors.is_empty());
    assert!(roster.activate_emitter("o_emiter1"));assert_eq!(roster.actors.len(),1);
    assert!(roster.actors[0].visible);assert_eq!(roster.drain_commands().len(),1);
    assert!(!roster.activate_emitter("o_emiter1"));
}
#[test]
fn use_key_targets_the_living_actor_at_the_72_unit_point() {
    let mut roster=fixture();let w=world(false);
    assert_eq!(roster.use_target(Vec3::new(0.0,5.0,-40.0),Vec3::Z,&w).as_deref(),Some("o_postac1"));
    assert_eq!(roster.use_target(Vec3::new(0.0,5.0,-120.0),Vec3::Z,&w),None,"too far away to be reached");
    assert_eq!(roster.use_target(Vec3::new(0.0,5.0,-40.0),-Vec3::Z,&w),None,"facing away");
    roster.actors[0].dead=true;assert_eq!(roster.use_target(Vec3::new(0.0,5.0,-40.0),Vec3::Z,&w),None,"a corpse is never used");
}
#[test]
fn flagged_character_leaves_no_weapon_pickup() {
    let mut roster=fixture();Arc::make_mut(&mut roster.actors[0].definition).header.push(("weapon".into(),"Glock".into()));
    assert_eq!(roster.actors[0].dropped_weapon(),Some("Glock"));
    Arc::make_mut(&mut roster.actors[0].definition).flags.push("nie_zostawiaj_gana".into());assert_eq!(roster.actors[0].dropped_weapon(),None);
}
#[test]
fn player_sweep_blocks_live_body_but_allows_dead_body() {
    let mut roster=fixture();let actor=&mut roster.actors[0];let half=Vec3::splat(2.0);
    let hit=sweep_actor(Vec3::new(0.0,5.0,0.0),half,Vec3::Z*40.0,actor).unwrap();
    assert!((hit.0-0.4).abs()<0.001 && hit.1==Vec3::NEG_Z);
    assert!(sweep_actor(Vec3::new(0.0,5.0,16.0),half,Vec3::NEG_Z,actor).is_none());
    actor.dead=true;assert!(sweep_actor(Vec3::new(0.0,5.0,0.0),half,Vec3::Z*40.0,actor).is_none());
}
#[test]
fn a_stance_change_that_keeps_the_hull_centre_does_not_push_the_player_and_landing_on_a_body_reports_support() {
    let mut roster=fixture();let w=world(false);let body=roster.actors[0].body_center();let extent=roster.actors[0].half_extents;
    // Beside the actor, the hull the same before and after crouching (the centre does not move; only the feet do).
    let beside=body+Vec3::new(extent.x+30.0,0.0,0.0);
    let same=correct_hull(&roster.actors,&w,beside,beside,Vec3::splat(24.0),Vec3::ZERO);
    assert_eq!(same.centre,beside);assert!(!same.standing_on_actor);
    // Falling onto the actor's box from above: stopped on top, told to keep standing.
    let above=body+Vec3::Y*(extent.y+24.0+30.0);
    let fall=correct_hull(&roster.actors,&w,above,above-Vec3::Y*60.0,Vec3::splat(24.0),Vec3::new(0.0,-300.0,0.0));
    assert!(fall.standing_on_actor,"the top face of the box is a floor");assert!(fall.centre.y>body.y+extent.y+23.0,"stopped on top: {:?}",fall.centre);assert!(fall.velocity.y>-1e-3,"the fall is cancelled");
    // Walking into its side: blocked, but that is no floor.
    let side=correct_hull(&roster.actors,&w,beside,body,Vec3::splat(24.0),Vec3::ZERO);
    assert!(!side.standing_on_actor && (side.centre.x-body.x).abs()>=extent.x+23.0,"blocked at the side: {:?}",side.centre);
    // A corpse is not solid.
    roster.actors[0].dead=true;
    assert_eq!(correct_hull(&roster.actors,&w,beside,body,Vec3::splat(24.0),Vec3::ZERO).centre,body);
}
#[test]
fn unspecified_bleeding_flag_follows_nie_krwaw() {
    let mut roster=fixture();let w=world(false);let origin=Vec3::new(0.0,5.0,0.0);
    roster.actors[0].hp=100.0;
    assert!(roster.hit_scan(origin,Vec3::Z,100.0,20.0,&w).unwrap().bleeds);
    Arc::make_mut(&mut roster.actors[0].definition).flags.push("nie_krwaw".into());
    assert!(!roster.hit_scan(origin,Vec3::Z,100.0,20.0,&w).unwrap().bleeds);
}
#[test]
fn the_snapshot_restore_keeps_hp_dead_and_phase() {
    let mut roster=fixture();roster.restore_actor("o_postac1",Vec3::new(1.0,5.1,2.0),-3.0,"corpse",true);
    assert!(roster.actors[0].dead && roster.actors[0].phase=="corpse" && roster.actors[0].position==Vec3::new(1.0,5.1,2.0));
    roster.restore_actor("o_postac1",Vec3::new(1.0,5.1,2.0),7.0,"idle",true);assert!(!roster.actors[0].dead && roster.actors[0].phase=="idle","the saved hit points decide");
}
#[test]
fn scripted_execution_shots_do_not_hit_the_player_off_axis() {
    let mut roster=fixture();roster.set_phase("guard","execution");
    sim(&mut roster,&world(false),Vec3::new(0.0,5.0,0.0),0.1);assert_eq!(roster.drain_damage(),0.0,"a guard without a weapon fires nothing");
}
#[test]
fn enemy_shot_queues_its_muzzle_presentation_and_a_three_dimensional_sound() {
    let mut roster=armed_fixture(0.0);roster.set_phase("guard","shoot");
    sim(&mut roster,&world(false),Vec3::new(0.0,5.0,1000.0),0.01);
    let shots=roster.drain_shots();assert_eq!(shots.len(),1,"one bullet, one presentation");
    assert!(shots[0].hits_player && !shots[0].melee && shots[0].endpoint.z>shots[0].origin.z);
    assert!(shots[0].commands.iter().any(|(key,_)|key=="sound_shoot"),"item keys travel with the shot");
    // No socket: the muzzle falls back to 24 units along the heading from the eye; `glosnosc` 1280 counts x4 while the NPC sees the player.
    assert!((shots[0].origin.distance(roster.actors[0].eye())-24.0).abs()<0.01);
    assert_eq!(roster.pending_sounds[0].2,1280.0*4.0);assert!(roster.drain_shots().is_empty());
}
#[test]
fn original_crt_endpoint_offsets_are_world_axes_and_difficulty_scaled() {
    let origin=Vec3::new(0.0,100.0,0.0);let target=origin+Vec3::Z*1000.0;let straight=origin+Vec3::Z*1500.0;
    let mut rng=ShotRng::default();
    assert_eq!(enemy_shot_endpoint(origin,target,0.0,1.0,&mut rng),straight);assert_eq!(rng.0,1,"zero spread consumes no random values");
    let spread=enemy_shot_endpoint(origin,target,128.0,1.0,&mut rng)-straight;
    let expected=Vec3::new(41.0-467.0,334.0-500.0,169.0-724.0)*0.128;assert!((spread-expected).length()<0.0002,"{spread:?}");
    let easy=enemy_shot_endpoint(origin,target,128.0,source_difficulty(0),&mut ShotRng::default())-straight;assert!((easy-spread*1.67).length()<0.0002);
    let distant=enemy_shot_endpoint(origin,origin+Vec3::Z*10000.0,128.0,1.0,&mut ShotRng::default());assert!((distant-(origin+Vec3::Z*3712.0+expected)).length()<0.0003);
}
#[test]
fn bullet_uses_hull_surface_and_finite_segment_before_static_or_moving_barrier() {
    let flat=|wall:bool|{let mut obj="v -100 0 -100\nv 100 0 -100\nv 100 0 100\nv -100 0 100\nf 1 3 2\nf 1 4 3\n".to_string();
        if wall {obj.push_str("v -100 0 10\nv 100 0 10\nv 100 100 10\nv -100 100 10\nf 5 6 7\nf 5 7 8\n");}CollisionWorld::from_obj(&obj).unwrap()};
    let origin=Vec3::new(0.0,5.0,20.0);let player=PlayerTarget {position:Vec3::new(0.0,5.0,0.0),half_extents:Vec3::new(2.0,5.0,2.0)};let endpoint=Vec3::new(0.0,5.0,-10.0);
    assert!(bullet_hits_player(&flat(false),origin,endpoint,player,|_,_|false));assert!(!bullet_hits_player(&flat(true),origin,endpoint,player,|_,_|false));
    assert!(!bullet_hits_player(&flat(false),origin,Vec3::new(0.0,5.0,3.0),player,|_,_|false));
}
#[test]
fn navigation_keys_nodes_by_graph() {
    let node=|graph,id,pos:[f32;3],slot0:usize|{let mut n=NavNode::new(id,pos,vec![slot0],graph);n.commands=vec![("idx0".into(),slot0.to_string())];n};
    let nav=Navigation {nodes:vec![node(0,0,[0.0,5.0,0.0],1),node(0,1,[60.0,5.0,0.0],0),node(1,1,[0.0,5.0,300.0],0),node(1,0,[0.0,5.0,360.0],1)],index:default()}.indexed();
    assert_eq!(nav.nodes[2].slots[0],Some(3),"graph 1 resolves its own ids");assert_eq!(nav.nodes[0].slots[0],Some(1));
    assert_eq!(nav.nearest_real(1,Vec3::new(0.0,-61.0,340.0)),Some(3));
}
#[test]
#[ignore="requires local original navigation export"]
fn exported_navigation_resolves_every_slot_inside_its_own_graph() {
    let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
    let data:Gameplay=serde_json::from_str(&std::fs::read_to_string(root.join("rh3-miasteczko0.gameplay.json")).unwrap()).unwrap();
    let nav=data.navigation.indexed();
    assert_eq!(nav.nodes.iter().map(|n|n.graph).max(),Some(10),"the prologue .pth has 11 path graphs");
    assert_eq!(nav.index.len(),nav.nodes.len());
    let linked:usize=nav.nodes.iter().map(|n|n.slots.iter().flatten().count()).sum();let listed:usize=nav.nodes.iter().map(|n|n.neighbors.len()).sum();
    assert_eq!(linked,listed,"every idx record resolves");
    assert!(nav.nodes.iter().all(|n|n.real_pos.is_some() && (30.0..160.0).contains(&(n.pos[1]-n.real[1]))),"the top position is 55..151 above the floor");
}
#[test]
#[ignore="requires local original asset exports"]
fn original_sig_enemy_strength_and_fifty_millisecond_phase_rate() {
    let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
    let game:Gameplay=serde_json::from_str(&std::fs::read_to_string(root.join("rh2-wiezienie1.gameplay.json")).unwrap()).unwrap();
    let weapon=game.characters["assault"].weapon_asset.clone().unwrap();
    assert_eq!(number_of(&weapon.commands,"rozrzut"),Some(128.0));assert_eq!(number_of(&weapon.commands,"sila_wroga"),Some(10.0));assert_eq!(number_of(&weapon.commands,"ammo_amount"),Some(30.0));
    let rates:Vec<f32>=game.characters["assault"].phases.values().filter_map(|p|number(&p.commands,"strzal")).collect();
    assert!(!rates.is_empty() && rates.iter().all(|r|*r>0.0 && *r<=0.5),"the phases carry the rate: {rates:?}");
}
#[test]
#[ignore="requires local original NPC models and DAT collision export"]
fn every_exported_path_link_resolves_and_actors_stand_on_their_places() {
    let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
    let (mut actors,mut placed,mut links)=(0,0,0);
    for path in std::fs::read_dir(&root).unwrap().flatten().map(|e|e.path()).filter(|p|p.to_string_lossy().ends_with(".gameplay.json")) {
        let data:Gameplay=serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let nav=data.navigation.indexed();
        for node in &nav.nodes {links+=node.slots.iter().flatten().count();assert!(node.slots.iter().flatten().count()==node.neighbors.len(),"{} node {}",path.display(),node.id);}
        for spawn in data.npcs.iter().filter(|s|data.characters.get(&s.definition_name).is_some_and(|c|c.flags.iter().any(|f|f=="ruchomy"))) {
            actors+=1;if let Some(place)=nav.place_of(Vec3::from(spawn.pos)) {if Vec3::from(nav.nodes[place].pos).distance(Vec3::from(spawn.pos))<400.0 {placed+=1;}}
        }
    }
    println!("{actors} mobile actors, {placed} within 400 units of a place, {links} links");
    assert!(placed*10>=actors*7,"most mobile actors stand near a node: {placed}/{actors}");
}

// ---------------------------------------------------------------------------------------------------------------------
// Sounds
// ---------------------------------------------------------------------------------------------------------------------

const HALT:&str="sounds\\enemies\\police\\halt";
const DEAD:&str="sounds\\enemies\\police\\dead";
fn voice(flags:&[&str],weapon:Option<serde_json::Value>)->NpcRoster {
    let mut all=vec!["ruchomy"];all.extend_from_slice(flags);
    roster_with(character(&all,&[("kat_kontaktu","89")],phases(&[("idle",1.0,true,&[]),
        ("walk",1.0,true,&[("glos_buta0","sounds\\enemies\\macaroni\\krokk1.wav"),("glos_buta1","sounds\\enemies\\macaroni\\krokk2.wav"),("odstep_glosow_buta","0.4")]),
        ("quick",1.0,true,&[("glos_buta1","sounds\\enemies\\macaroni\\krokk2.wav"),("odstep_glosow_buta","0.05")]),
        ("watch",1.0,true,&[("on_kontakt","idle"),("sound_on_kontakt","sounds\\enemies\\police\\halt0.wav"),("sounds_on_kontakt","8")]),
        ("single",1.0,true,&[("on_kontakt","idle"),("sound_on_kontakt","sounds\\enemies\\police\\halt3.wav")]),
        ("kill",1.0,false,&[("zabij","")]),("gun",0.5,true,&[("strzal","0.05")]),("chatter",1.0,true,&[("sound","sounds\\ambient\\radio.wav")])]),weapon))
}
#[test]
fn npc_footsteps_repeat_every_odstep_and_carry_640_or_2048_when_seen() {
    let player=Vec3::new(0.0,5.0,-100.0);
    let mut hidden=voice(&[],None);hidden.set_phase("guard","walk");run(&mut hidden,&world(true),player,2.0,0.05);
    assert!((5..=6).contains(&hidden.pending_sounds.len()),"{}",hidden.pending_sounds.len());
    assert!(hidden.pending_sounds.iter().all(|(path,_,radius)|path.ends_with("krokk2.wav") && *radius==640.0),"retail plays glos_buta1 only");
    let mut seen=voice(&[],None);seen.set_phase("guard","walk");run(&mut seen,&world(false),Vec3::new(0.0,5.0,100.0),2.0,0.05);
    assert!(!seen.pending_sounds.is_empty() && seen.pending_sounds.iter().all(|(_,_,radius)|*radius==2048.0));
    let mut quick=voice(&[],None);quick.set_phase("guard","quick");run(&mut quick,&world(true),player,1.0,0.05);assert!(quick.pending_sounds.is_empty(),"0.05 is not above 0.09");
    let mut idle=voice(&[],None);run(&mut idle,&world(true),player,1.0,0.05);assert!(idle.pending_sounds.is_empty());
}
#[test]
fn halt_shout_picks_a_random_variant_and_carries_2048_when_seen() {
    let mut roster=voice(&[],None);roster.set_phase("guard","watch");sim(&mut roster,&world(false),Vec3::new(0.0,5.0,50.0),0.05);roster.provoke_noticing();
    roster.pending_sounds.clear();sim(&mut roster,&world(false),Vec3::new(0.0,5.0,50.0),0.05);
    assert_eq!(roster.pending_sounds.len(),1);let (path,_,radius)=&roster.pending_sounds[0];assert_eq!(*radius,2048.0);
    assert!(path.starts_with(HALT) && path.ends_with(".wav") && path.len()==HALT.len()+5 && path.as_bytes()[HALT.len()]<b'8',"{path}");
    let mut single=voice(&[],None);single.set_phase("guard","single");sim(&mut single,&world(false),Vec3::new(0.0,5.0,50.0),0.05);single.provoke_noticing();
    single.pending_sounds.clear();sim(&mut single,&world(false),Vec3::new(0.0,5.0,50.0),0.05);assert_eq!(single.pending_sounds[0].0,format!("{HALT}3.wav"));
    let mut unseen=voice(&[],None);unseen.set_phase("guard","watch");unseen.actors[0].contact=true;unseen.actors[0].provoked=true;
    sim(&mut unseen,&world(true),Vec3::new(0.0,5.0,-15.0),0.05);
    if let Some((_,_,radius))=unseen.pending_sounds.first() {assert_eq!(*radius,1280.0,"not seen: 1280");}
}
#[test]
fn halts_vary_between_characters() {
    let mut picks=std::collections::BTreeSet::new();
    for seed in 0..40 {
        let mut r=voice(&[],None);r.set_phase("guard","watch");sim(&mut r,&world(false),Vec3::new(0.0,5.0,50.0),0.05);r.provoke_noticing();r.sound_rng.0=seed;
        r.pending_sounds.clear();sim(&mut r,&world(false),Vec3::new(0.0,5.0,50.0),0.05);picks.insert(r.pending_sounds[0].0.clone());
    }
    assert!(picks.len()>=6,"{picks:?}");
}
#[test]
fn death_scream_needs_the_flag_and_a_halt_file_and_carries_4024() {
    let shoot=|r:&mut NpcRoster|r.hit_scan(Vec3::new(0.0,5.0,80.0),Vec3::NEG_Z,100.0,20.0,&world(false)).unwrap();
    let mut screamer=voice(&["graj_dzwiek_smierci"],None);assert!(shoot(&mut screamer).killed);
    assert_eq!(screamer.pending_sounds.len(),1);let (path,position,radius)=&screamer.pending_sounds[0];
    assert!(path.starts_with(DEAD) && path.ends_with(".wav") && path.as_bytes()[DEAD.len()]<b'8',"{path}");assert_eq!((*radius,*position),(4024.0,Vec3::new(0.0,5.1,20.0)));
    let mut mute=voice(&[],None);assert!(shoot(&mut mute).killed);assert!(mute.pending_sounds.is_empty());
    let mut scripted=voice(&["graj_dzwiek_smierci"],None);scripted.set_phase("guard","kill");
    assert_eq!(scripted.pending_sounds.len(),1);assert!(scripted.pending_sounds[0].0.contains("dead"));
    scripted.set_phase("guard","kill");assert_eq!(scripted.pending_sounds.len(),1,"only once");
}
#[test]
fn npc_weapon_reaches_glosnosc_and_four_times_that_when_it_sees_the_player() {
    let loud=|l:f32|Some(serde_json::json!({"model":"weapon","animation":"idle","skins":{},"styles":{},"socket":null,"commands":[["rozrzut","0"],["sila_wroga","10"],["sound_shoot","sig_s.wav"],["glosnosc",l.to_string()],["ammo_amount","30"]]}));
    // `nie_patrz_na_gracza`: the actor never counts as seeing the player (the weapon still fires).
    let mut hidden=voice(&["nie_patrz_na_gracza"],loud(1640.0));hidden.set_phase("guard","gun");run(&mut hidden,&world(false),Vec3::new(0.0,5.0,500.0),0.06,0.01);
    assert_eq!(hidden.pending_sounds[0].2,1640.0);
    let mut seen=voice(&[],loud(1640.0));seen.set_phase("guard","gun");run(&mut seen,&world(false),Vec3::new(0.0,5.0,500.0),0.06,0.01);assert_eq!(seen.pending_sounds[0].2,6560.0);
    let mut club=voice(&[],loud(0.0));club.set_phase("guard","gun");run(&mut club,&world(false),Vec3::new(0.0,5.0,500.0),0.06,0.01);assert_eq!(club.pending_sounds[0].2,0.0,"a nightstick makes no shot sound");
}
#[test]
fn phase_sound_key_carries_2048() {
    let mut roster=voice(&[],None);roster.set_phase("guard","chatter");
    assert_eq!(roster.pending_sounds,vec![("sounds\\ambient\\radio.wav".to_string(),Vec3::new(0.0,5.1,20.0),2048.0)]);
}

#[test]
#[ignore="requires local original NPC models"]
fn gun_forward_of_the_hand_socket_follows_the_body_heading_in_firing_poses() {
    let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
    let mut worst=(2.0f32,String::new());let mut report=Vec::new();
    for world in ["rh1-wiezienie2","rh2-wiezienie2","rh3-miasteczko1","rh10-wiezowiec2"] {
        let game:Gameplay=serde_json::from_str(&std::fs::read_to_string(root.join(format!("{world}.gameplay.json"))).unwrap()).unwrap();
        let mut models=BTreeMap::<String,Arc<Model>>::new();
        for (name,definition) in game.characters.iter().filter(|(_,c)|c.weapon_asset.as_ref().is_some_and(|w|w.socket.is_some())) {
            models.entry(definition.model.clone()).or_insert_with(||Arc::new(Model::load(&root,&definition.model)));
            models.entry(definition.weapon_asset.as_ref().unwrap().model.clone()).or_insert_with(||Arc::new(Model::load(&root,&definition.weapon_asset.as_ref().unwrap().model)));
            let definition=Arc::new(definition.clone());
            for (phase_name,phase) in definition.phases.iter().filter(|(_,p)|has(&p.commands,"strzal") || has(&p.commands,"strzal_raz")) {
                let mut actor=Npc::from_spawn(Spawn {name:"t".into(),definition_name:name.clone(),pos:[0.0,60.0,0.0],rotation:[0.0;4],source_object_index:0,properties:default()},definition.clone());
                let _=actor.begin_phase(phase_name,false,&Navigation::default());
                let duration=phase.duration.unwrap_or(0.5);
                for t in [0.0,duration*0.5] {
                    actor.elapsed=t;
                    if let Some((muzzle,_,forward))=weapon_sockets(&models,&actor) {
                        let d=forward.dot(Vec3::Z);
                        report.push(format!("{world} {name} {phase_name} t={t:.2}: forward.z={d:.2} muzzle=({:.0},{:.0},{:.0})",muzzle.x,muzzle.y,muzzle.z));
                        if d<worst.0 {worst=(d,format!("{world} {name} {phase_name}"));}
                    }
                }
            }
        }
    }
    for line in &report {println!("{line}");}
    println!("worst alignment {:.2} at {}",worst.0,worst.1);
    assert!(!report.is_empty());
}

#[test]
#[ignore="requires local original gameplay and collision exports"]
fn audit_every_level_runs_every_actor_for_a_minute_without_errors() {
    let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
    let mut worlds=std::fs::read_dir(&root).unwrap().flatten().map(|e|e.file_name().to_string_lossy().to_string()).filter(|n|n.ends_with(".gameplay.json")).collect::<Vec<_>>();worlds.sort();
    let (mut total_actors,mut total_shots,mut total_hops,mut stuck_total,mut phase_changes)=(0,0u32,0usize,0,0usize);
    for file in worlds {
        let name=file.trim_end_matches(".gameplay.json").to_owned();
        let data:Gameplay=serde_json::from_str(&std::fs::read_to_string(root.join(&file)).unwrap()).unwrap();
        let world=CollisionWorld::from_obj(&std::fs::read_to_string(root.join(format!("{name}.collision.obj"))).unwrap()).unwrap();
        let characters=data.characters.into_iter().map(|(n,c)|(n,Arc::new(c))).collect::<BTreeMap<_,_>>();
        let mut roster=NpcRoster {navigation:data.navigation.indexed(),..default()};
        for spawn in &data.npcs {
            let Some(definition)=characters.get(&spawn.definition_name).cloned() else {continue};
            let mut actor=Npc::from_spawn(spawn.clone(),definition.clone());
            let _=actor.begin_phase(&definition.default_phase,false,&roster.navigation);
            actor.position=lift_interpenetrating_spawn(&world,actor.position,actor.half_extents);
            roster.actors.push(actor);
        }
        total_actors+=roster.actors.len();
        if roster.actors.is_empty() {continue;}
        // The player walks a square around the centroid of the actors, always with a free standing spot under it.
        let centroid=roster.actors.iter().fold(Vec3::ZERO,|a,b|a+b.position)/roster.actors.len() as f32;
        let mut previous:Vec<(String,Vec3)>=roster.actors.iter().map(|a|(a.phase.clone(),a.position)).collect();
        let mut still=vec![0.0f32;roster.actors.len()];
        for step in 0..(60*30) {
            let t=step as f32/30.0;
            let player=centroid+Vec3::new((t*0.4).cos()*400.0,20.0,(t*0.4).sin()*400.0);
            roster.provoke_noticing();
            sim(&mut roster,&world,player,1.0/30.0);
            roster.pending_sounds.clear();roster.pending_commands.clear();roster.pending_deaths.clear();
            total_shots+=roster.pending_shots.len() as u32;roster.pending_shots.clear();roster.pending_damage=0.0;
            for (i,actor) in roster.actors.iter().enumerate() {
                assert!(actor.position.is_finite() && actor.yaw.is_finite() && actor.pitch.is_finite() && actor.vel.is_finite(),"{name} {} {}: {:?}",actor.name,actor.phase,actor.position);
                assert!(actor.definition.phases.contains_key(&actor.phase),"{name} {}: phase {:?} does not exist",actor.name,actor.phase);
                if actor.phase!=previous[i].0 {phase_changes+=1;}
                if actor.moving && actor.position.distance(previous[i].1)<1e-4 {still[i]+=1.0/30.0;}else{still[i]=0.0;}
                assert!(still[i]<3.0 || actor.seg_len==0.0 || actor.progress<actor.seg_len && false,"{name} {} stands still while walking ({}s, phase {})",actor.name,still[i],actor.phase);
                if actor.moving {total_hops+=1;}
                previous[i]=(actor.phase.clone(),actor.position);
            }
        }
        stuck_total+=still.iter().filter(|s|**s>1.0).count();
        println!("{name}: {} actors, {} alive, {} moving at the end",roster.actors.len(),roster.actors.iter().filter(|a|a.alive()).count(),roster.actors.iter().filter(|a|a.moving).count());
    }
    println!("{total_actors} actors, {phase_changes} phase changes, {total_shots} enemy shots, {total_hops} walking actor-frames, {stuck_total} slow");
    assert!(phase_changes>total_actors && total_shots>0);
}

/// The class of bug behind "the old prostitute has no head": a character whose `glowa_specjalna` head, weapon or smoke socket is not in the export.
/// Every character definition of every exported level must find its body, skins, head model + skin + `socket_glowa`, and the cigarette socket.
#[test]
#[ignore="requires local original gameplay and model exports (tools/export_gameplay.py for every world)"]
fn every_character_of_every_level_gets_its_body_skins_head_and_sockets() {
    let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
    let mut models=BTreeMap::<String,Arc<Model>>::new();
    let (mut characters,mut heads,mut problems)=(0,0,Vec::<String>::new());
    let mut paths:Vec<_>=std::fs::read_dir(&root).unwrap().flatten().map(|e|e.path()).filter(|p|p.to_string_lossy().ends_with(".gameplay.json")).collect();paths.sort();
    for path in paths {
        let world=path.file_name().unwrap().to_string_lossy().split('.').next().unwrap().to_string();
        let game:Gameplay=serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        for (name,c) in &game.characters {
            characters+=1;let at=format!("{world}/{name}");
            for (slot,skin) in c.skins.iter().chain(c.weapon_asset.iter().flat_map(|w|w.skins.iter())).chain(c.head_asset.iter().flat_map(|h|h.skins.iter())) {
                if !root.join(skin).is_file() {problems.push(format!("{at}: skin{slot} {skin} is not exported"));}
            }
            if !root.join(format!("{}.json",c.model)).is_file() {problems.push(format!("{at}: body model {} is not exported",c.model));continue;}
            let body=models.entry(c.model.clone()).or_insert_with(||Arc::new(Model::load(&root,&c.model))).clone();
            let first=body.pose(&c.phases.get(&c.default_phase).and_then(|p|p.animation.clone()).unwrap_or_default(),0.0,false);
            if c.flags.iter().any(|f|f=="glowa_specjalna") {
                let Some(head)=&c.head_asset else {problems.push(format!("{at}: glowa_specjalna without head_asset (stale gameplay export?)"));continue;};
                heads+=1;
                if !root.join(format!("{}.json",head.model)).is_file() {problems.push(format!("{at}: head model {} is not exported",head.model));continue;}
                if head.skins.is_empty() {problems.push(format!("{at}: head has no skin"));}
                models.entry(head.model.clone()).or_insert_with(||Arc::new(Model::load(&root,&head.model)));
                if head.socket.as_deref().is_none_or(|s|body.try_socket(&first,s).is_none()) {problems.push(format!("{at}: head socket {:?} is missing on {}",head.socket,c.model));}
            }
            if let Some(socket)=command_value(&c.header,"socket_papieros").filter(|s|!s.is_empty()) {
                if body.try_socket(&first,socket).is_none() {problems.push(format!("{at}: socket_papieros {socket:?} is missing on {}",c.model));}
            }
            // Retail data quirks, known: the cigarette weapon of `stara dziwka` names a `bron` socket her body lacks (nothing is drawn, like retail) and `policjant z pala` names none.
            if let Some(w)=&c.weapon_asset {
                if w.socket.as_deref().is_none_or(|s|body.try_socket(&first,s).is_none()) && !matches!(name.as_str(),"stara dziwka"|"policjant z pala") {
                    problems.push(format!("{at}: weapon socket {:?} is missing on {}",w.socket,c.model));
                }
            }
        }
    }
    assert!(characters>150 && heads>=10,"{characters} definitions, {heads} with a separate head");
    assert!(problems.is_empty(),"{} problems:\n{}",problems.len(),problems.join("\n"));
}
/// `stara dziwka` (the prologue's woman with the cigarette, the owner's bug report) end to end: head on the `glowa` socket, her clips, the smoke socket.
#[test]
#[ignore="requires local original gameplay and model exports"]
fn the_prologue_woman_has_her_head_hair_skin_and_cigarette_smoke() {
    let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
    let game:Gameplay=serde_json::from_str(&std::fs::read_to_string(root.join("rh3-miasteczko0.gameplay.json")).unwrap()).unwrap();
    let c=&game.characters["stara dziwka"];
    let head=c.head_asset.as_ref().expect("head_asset");
    assert_eq!(head.model,"models/postacie/stara_dziwka_glowa.ltb");assert_eq!(head.socket.as_deref(),Some("glowa"));
    assert!(root.join(&head.skins["0"]).is_file(),"the head skin (hair and face) is exported");
    let (body,head_model)=(Model::load(&root,&c.model),Model::load(&root,&head.model));
    let pose=body.pose("gada",0.0,false);
    assert!(body.try_socket(&pose,"glowa").is_some() && body.try_socket(&pose,"dymek").is_some());
    assert_eq!(command_value(&c.header,"socket_papieros"),Some("dymek"));
    for phase in ["nuda","nuda1","nuda2"] {
        let commands=&c.phases[phase].commands;
        assert!(has(commands,"dym_papierosa"),"{phase} smokes");
        let clip=command_value(commands,"animacja_glowa").unwrap();assert!(head_model.animation_duration(clip).is_some(),"head clip {clip}");
    }
    assert!(!has(&c.phases["pada"].commands,"dym_papierosa"),"the corpse does not smoke");
}
/// The export audit (`python -m tools.audit_assets ../GYARI output`, docs/retail-assets.md) must have nothing unexplained: every model / skin /
/// texture / sound / sprite the retail scripts, DLL strings and placed objects name is exported, or missing in retail too and listed with its reason.
#[test]
#[ignore="requires local exports and a fresh `python -m tools.audit_assets ../GYARI output`"]
fn the_asset_audit_reports_no_unexported_or_unexplained_missing_asset() {
    let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
    let audit:serde_json::Value=serde_json::from_str(&std::fs::read_to_string(root.join("assets-audit.json")).expect("run: python -m tools.audit_assets ../GYARI output")).unwrap();
    let bad:Vec<String>=audit["rows"].as_array().unwrap().iter().filter(|r|matches!(r["class"].as_str(),Some("a"|"b?")))
        .map(|r|format!("{} {} <- {}:{}",r["class"],r["path"],r["source"],r["line"])).collect();
    assert!(bad.is_empty(),"{} unexported (a) / unexplained (b?) references:\n{}",bad.len(),bad.join("\n"));
    for key in ["character_problems","exported_path_problems"] {assert!(audit[key].as_array().unwrap().is_empty(),"{key}: {}",audit[key]);}
    assert!(audit["references"].as_u64().unwrap()>8000 && audit["characters_checked"].as_u64().unwrap()>150,"the audit looked at the whole game");
}

// ---------------------------------------------------------------------------------------------------------------------
// Audit round: sight filters, aware tag, speaker attack (docs/retail-ai.md "Line-of-sight filters")
// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn a_segment_hits_a_box_only_from_outside() {
    let (center,half)=(Vec3::new(0.0,0.0,50.0),Vec3::splat(10.0));
    assert_eq!(segment_box_entry(Vec3::ZERO,Vec3::new(0.0,0.0,100.0),center,half),Some(40.0));
    assert!(segment_box_entry(Vec3::ZERO,Vec3::new(0.0,0.0,30.0),center,half).is_none(),"ends short");
    assert!(segment_box_entry(Vec3::new(0.0,0.0,50.0),Vec3::new(0.0,0.0,100.0),center,half).is_none(),"a segment that starts inside never hits the box (0x42b8f2)");
    assert!(segment_box_entry(Vec3::new(30.0,0.0,0.0),Vec3::new(30.0,0.0,100.0),center,half).is_none(),"passes beside it");
}
#[test]
fn another_character_between_the_eye_and_the_chest_blocks_the_sight_but_a_bystander_behind_does_not() {
    // cshell 0x10057940 lets every model block except the player and the actor itself: the actor behind another character is not seen.
    let definition=Arc::new(character(&["ruchomy"],&[],basic_phases(),None));
    let mut roster=fixture();roster.actors.push(actor("o_postac2",&definition,[0.0,5.1,10.0]));
    roster.set_phase("guard","idle");
    sim(&mut roster,&world(false),Vec3::new(0.0,5.0,-200.0),0.05);
    assert!(!roster.actors[0].seen,"o_postac2 (box half 2/5/2) stands on the line from the eye to o_postac1's chest");
    assert!(roster.actors[1].seen,"the front character itself is seen");
    // The same line with the front character dead: a corpse still blocks (user flag 0x80 is not skipped by 0x10057940).
    roster.actors[1].dead=true;roster.actors[0].seen_timer=0.0;
    sim(&mut roster,&world(false),Vec3::new(0.0,5.0,-200.0),0.05);
    assert!(!roster.actors[0].seen,"a corpse blocks the view as well");
    // Off the line it does not.
    roster.actors[1].position=Vec3::new(40.0,5.1,10.0);roster.actors[0].seen_timer=0.0;
    sim(&mut roster,&world(false),Vec3::new(0.0,5.0,-200.0),0.05);
    assert!(roster.actors[0].seen);
}
#[test]
fn the_hide_test_of_a_cover_estimate_ignores_living_characters_but_meets_corpses() {
    // cshell 0x10057820 (estimate_kryjowka, 0x10047130): living characters (user flag 0x10) and glass are skipped, corpses (0x80) block.
    let definition=Arc::new(character(&["ruchomy"],&[],basic_phases(),None));
    let mut roster=fixture();roster.actors.push(actor("o_postac2",&definition,[0.0,5.1,100.0]));
    let world=world(false);
    let boxes=|r:&NpcRoster|r.actors.iter().enumerate().map(|(index,a)|ai::ActorBox {index,alive:a.alive(),center:a.body_center(),half:a.half_extents}).collect::<Vec<_>>();
    let blocked=|_:Vec3,_:Vec3|false;
    let b=boxes(&roster);
    let frame=ai::Frame {world:&world,player:Vec3::new(0.0,-50.0,-100.0),half:Vec3::new(2.0,5.0,2.0),eye:Vec3::new(0.0,-2.0,-100.0),difficulty:1.0,dt:0.05,blocked:&blocked,boxes:&b};
    // o_postac1 stands at z = 20, o_postac2 at z = 100: from the player's eye + 55 to a point behind both.
    assert!(!roster.hidden_from_player(Vec3::new(0.0,5.0,300.0),&frame),"living characters are ignored by the hide test");
    roster.actors[0].dead=true;roster.actors[1].dead=true;let b=boxes(&roster);
    let frame=ai::Frame {boxes:&b,..frame};
    assert!(roster.hidden_from_player(Vec3::new(0.0,5.0,300.0),&frame),"corpses block it");
}
#[test]
fn a_tagged_actor_does_not_start_a_patrol_phase() {
    // 0x10041cbe: SetPhase returns at once for a patrol phase while +0x128 is set; everything else is accepted.
    let mut roster=patrolling(3,1);roster.set_phase("guard","idle");roster.drain_commands();
    roster.set_aware(&["o_postac1".to_string()]);assert!(roster.actors[0].aware);
    roster.set_phase("guard","walk");
    assert_eq!(roster.actors[0].phase,"idle","the patrol phase was refused");
    roster.set_aware(&[]);roster.set_phase("guard","walk");assert_eq!(roster.actors[0].phase,"walk");
}
#[test]
fn a_speaker_is_reset_with_the_tag_cleared_then_faces_the_player_and_is_released_after_the_dialogue() {
    let mut roster=patrolling(3,1);roster.set_phase("guard","idle");roster.drain_commands();
    roster.set_aware(&["o_postac1".to_string()]);
    // StartDialog (0x10019773..0x10019804): the tag is cleared while the default phase is entered, so even a patrolling default phase starts; then the tag returns.
    roster.reset_speaker("o_postac1","walk");
    assert_eq!(roster.actors[0].phase,"walk");assert!(roster.actors[0].aware && roster.actors[0].face_player);
    // The node's hostileattack (0x10019811): provoked, untagged.
    roster.provoke_speaker("o_postac1");assert!(roster.actors[0].provoked && !roster.actors[0].aware);
    // The dialogue ends: the tag is gone and the current phase is re-entered (forced).
    roster.set_aware(&["o_postac1".to_string()]);roster.drain_commands();
    roster.release_aware(&["o_postac1".to_string()]);
    assert!(!roster.actors[0].aware);assert_eq!(roster.actors[0].phase,"walk");
    assert!(roster.drain_commands().iter().any(|(name,_)|name=="o_postac1"),"the phase entry ran again");
    assert!(!roster.actors[0].face_player,"a phase entry clears +0x158");
}
#[test]
fn a_bullet_passes_the_upper_part_of_a_corpse_box_and_stops_in_its_lowest_quarter() {
    // cshell 0x1000686b..0x10006963: hit below `centre - half height / 2` stops the bullet (blood splash), otherwise it flies on.
    let definition=Arc::new(character(&["ruchomy"],&[],basic_phases(),None));
    let mut roster=fixture();roster.actors.push(actor("o_postac2",&definition,[0.0,5.1,100.0]));
    assert!(roster.corpse_stop(Vec3::new(0.0,1.0,0.0),Vec3::Z,500.0).is_none(),"a living character is not a corpse");
    roster.actors[1].dead=true;
    assert_eq!(roster.corpse_stop(Vec3::new(0.0,1.0,0.0),Vec3::Z,500.0),Some(98.0),"low hit: stops at the box face");
    assert!(roster.corpse_stop(Vec3::new(0.0,5.0,0.0),Vec3::Z,500.0).is_none(),"chest height goes through");
    assert!(roster.corpse_stop(Vec3::new(0.0,1.0,0.0),Vec3::Z,50.0).is_none(),"out of range");
    // An enemy bullet at knee height ends on the corpse without damaging anything behind it.
    let player=PlayerTarget {position:Vec3::new(0.0,1.0,300.0),half_extents:Vec3::new(2.0,5.0,2.0)};
    let world=world(false);let blocked=|_:Vec3,_:Vec3|false;let b:Vec<ai::ActorBox>=vec![];
    let frame=ai::Frame {world:&world,player:player.position,half:player.half_extents,eye:player.position+Vec3::Y*48.0,difficulty:1.0,dt:0.05,blocked:&blocked,boxes:&b};
    let (end,hit_player,_)=roster.trace_bullet(0,Vec3::new(0.0,1.0,20.0),Vec3::new(0.0,1.0,320.0),player,10.0,&frame);
    assert!(!hit_player && (end.z-98.0).abs()<0.01,"{end:?}");
}
#[test]
#[ignore="requires local original gameplay and collision exports"]
fn snap_survey_of_the_spawn_positions() {
    let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
    let mut worlds=std::fs::read_dir(&root).unwrap().flatten().map(|e|e.file_name().to_string_lossy().to_string()).filter(|n|n.ends_with(".gameplay.json")).collect::<Vec<_>>();worlds.sort();
    for file in worlds {
        let name=file.trim_end_matches(".gameplay.json").to_owned();
        let data:Gameplay=serde_json::from_str(&std::fs::read_to_string(root.join(&file)).unwrap()).unwrap();
        let world=CollisionWorld::from_obj(&std::fs::read_to_string(root.join(format!("{name}.collision.obj"))).unwrap()).unwrap();
        let characters=data.characters.into_iter().map(|(n,c)|(n,Arc::new(c))).collect::<BTreeMap<_,_>>();
        let (mut rows,mut stat)=(Vec::new(),0);
        for spawn in &data.npcs {
            let Some(definition)=characters.get(&spawn.definition_name).cloned() else {continue};
            let mut actor=Npc::from_spawn(spawn.clone(),definition.clone());
            let _=actor.begin_phase(&definition.default_phase,false,&Navigation::default());
            if has(actor.commands(),"static") {stat+=1;continue;}
            let lifted=lift_interpenetrating_spawn(&world,actor.position,actor.half_extents);
            let snapped=snap_spawn(&world,lifted,actor.half_extents);
            if (snapped.y-lifted.y).abs()>0.5 {rows.push(format!("{} ({}) dy {:+.1} from y {:.1}",actor.name,actor.definition_name,snapped.y-lifted.y,lifted.y));}
        }
        println!("{name}: {} actors, {stat} static, {} would move", data.npcs.len(),rows.len());
        for row in rows.iter().take(12) {println!("    {row}");}
    }
}
#[test]
fn the_spawn_snap_drops_a_hovering_actor_onto_the_floor_but_leaves_big_drops_and_slopes() {
    let w=world(false);let half=Vec3::new(2.0,5.0,2.0);
    assert_eq!(snap_spawn(&w,Vec3::new(10.0,15.0,0.0),half),Vec3::new(10.0,5.0,0.0),"10 units above its resting height: the segment hits the floor, centre = hit + half height");
    assert_eq!(snap_spawn(&w,Vec3::new(10.0,5.0,0.0),half),Vec3::new(10.0,5.0,0.0));
    assert_eq!(snap_spawn(&w,Vec3::new(10.0,100.0,0.0),half).y,100.0,"more than 32 units of drop: the engine would meet something else first (a prop, a world model)");
    assert_eq!(snap_spawn(&w,Vec3::new(10.0,400.0,0.0),half).y,400.0,"no floor within 256 units");
}
#[test]
fn every_armed_character_drops_its_weapons_on_death_unless_nie_zostawiaj_gana() {
    // Kill (cshell 0x10042ca0, 0x10042eec..0x10043085) throws the weapon objects (+0x208, and +0x20c when `socket_weapon1` names a second one, 0x10044e7a) into the
    // world whatever phase the character dies in; `nie_zostawiaj_gana` removes them instead. The remake used to drop only for phases with the unknown key `drop_weapon`.
    let mut roster=roster_with(character(&["ruchomy"],&[("weapon","Glock"),("socket_weapon1","bron2")],basic_phases(),None));
    roster.actors[0].weapon_visible=true;
    roster.kill(0);
    let drops=roster.drain_drops();
    assert_eq!(drops.iter().map(|d|(d.owner.as_str(),d.kind.as_str())).collect::<Vec<_>>(),vec![("o_postac1","Glock"),("o_postac1+1","Glock")]);
    assert!(!roster.actors[0].weapon_visible,"the carried model is gone");
    let mut one=roster_with(character(&["ruchomy"],&[("weapon","Glock")],basic_phases(),None));one.kill(0);assert_eq!(one.drain_drops().len(),1);
    let mut kept=roster_with(character(&["ruchomy","nie_zostawiaj_gana"],&[("weapon","papieros")],basic_phases(),None));kept.actors[0].weapon_visible=true;kept.kill(0);
    assert!(kept.drain_drops().is_empty() && !kept.actors[0].weapon_visible,"removed, not dropped");
    let mut unarmed=fixture();unarmed.kill(0);assert!(unarmed.drain_drops().is_empty());
}
#[test]
fn the_invisible_console_variable_blinds_every_actor_test() {
    // `Invisible` (0x100b2476): 0x10043720 (seen), 0x10043860 (weapon sight), 0x10045df0 (stimuli) and 0x100499ba (contact) answer "no" at once.
    let mut roster=fixture();sim(&mut roster,&world(false),Vec3::new(0.0,5.0,300.0),0.5);
    assert!(roster.actors[0].seen);
    let mut blind=fixture();blind.invisible=true;
    sim(&mut blind,&world(false),Vec3::new(0.0,5.0,300.0),0.5);assert!(!blind.actors[0].seen);
    blind.add_stimulus(Vec3::new(0.0,5.0,25.0),500.0,0.0,0.0,2);
    let frame_world=world(false);let boxes:Vec<ai::ActorBox>=vec![];let no_block=|_:Vec3,_:Vec3|false;
    let frame=ai::Frame {world:&frame_world,player:Vec3::new(0.0,5.0,300.0),half:Vec3::new(2.0,5.0,2.0),eye:Vec3::new(0.0,53.0,300.0),difficulty:1.0,dt:0.05,blocked:&no_block,boxes:&boxes};
    assert!(!blind.notices(0,&frame),"a stimulus right next to it is not heard");
    blind.invisible=false;assert!(blind.notices(0,&frame));
}
