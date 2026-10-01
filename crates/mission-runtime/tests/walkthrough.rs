//! Runs the original mission script of every campaign level through the solver: what must the player do to leave the level?
//! `cargo test --test walkthrough -- --ignored --nocapture` prints the per-level plan (needs the local export).
use mission_runtime::{solver::{solve,End,Op,World},Mission};
use serde_json::Value;
use std::path::Path;

/// The 27 campaign levels in retail order with the level each one is expected to hand over to when the script (not a door) ends it.
pub const ORDER:[&str;27]=["rh3-miasteczko0","rh1-wiezienie1","rh1-wiezienie2","rh1-wiezienie3","rh2-wiezienie1","rh2-wiezienie2","rh3-miasteczko1","rh3-miasteczko2","burmistrz1","burmistrz2","chapel_mniejszy","knajpa","rh7a-tunele","podziemia1","podziemia1a","podziemia1b","podziemia1c","chinatown2","rh9-fabryka","rh10-wiezowiec1","rh10-wiezowiec2","rh10-wiezowiec3","wiez_wn1","wiez_wn2","wiez_wn3","rh12-lab1","rh12-lab2"];

fn text(v:&Value)->String {v.as_str().unwrap_or("").to_owned()}
fn stem(output:&Path,id:&str)->String {
    std::fs::read_dir(output).unwrap().flatten().filter_map(|e|e.file_name().into_string().ok()).find(|n|n.to_ascii_lowercase()==format!("{id}.scene.json"))
        .map(|n|n[..n.len()-".scene.json".len()].to_owned()).unwrap_or_else(||id.to_owned())
}
/// Builds the solver's view of a level from its exported scene and gameplay files.
pub fn world_of(output:&Path,id:&str)->World {
    let stem=stem(output,id);
    let read=|name:String|serde_json::from_str::<Value>(&std::fs::read_to_string(output.join(name)).unwrap_or_else(|_|"[]".into())).unwrap();
    World::from_exports(&read(format!("{stem}.scene.json")),&read(format!("{stem}.gameplay.json")),&read(format!("{stem}.items.json")))
}

#[test] #[ignore = "requires local export"]
fn every_script_gated_level_can_be_left() {
    let output=Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
    let sources:Value=serde_json::from_str(&std::fs::read_to_string(output.join("gameplay_scripts.json")).unwrap()).unwrap();
    let mut failures=Vec::new();
    for id in ORDER {
        let mission=Mission::from_sources(id,sources["gameai"].as_str().unwrap(),sources["dialogues"].as_str().unwrap(),sources["text_keys"].as_str().unwrap()).unwrap();
        let world=world_of(&output,id);
        if !mission.ends_level() {println!("{id}: the level ends at a door (no startlevel/cutscene in its script)");continue;}
        let report=solve(&mission,&world,30000);
        let plan:Vec<String>=report.plan.iter().map(|op|match op {Op::Talk(i)=>format!("talk {i}"),Op::Approach(i)=>format!("near {i}"),Op::Kill(i)=>format!("kill {i}"),Op::Choose(n)=>format!("answer {n}"),Op::Marker(m)=>format!("marker {m}"),Op::Wait(s)=>format!("wait {s}"),Op::Draw(d)=>format!("draw {d}"),Op::Pickup(k)=>format!("pickup {k}")}).collect();
        println!("{id}: {:?} explored={} deaths={} truncated={} dialogues={} plan={plan:?}",report.end,report.explored,report.deaths,report.truncated,report.dialogues);
        // Levels ended by doors have no script exit; only the seven `startlevel` levels must solve.
        let scripted=matches!(id,"rh3-miasteczko0"|"burmistrz1"|"burmistrz2"|"chapel_mniejszy"|"knajpa"|"chinatown2"|"rh9-fabryka");
        if scripted && !matches!(report.end,Some(End::Transition(_))) {failures.push(format!("{id}: {:?} flags={:?}",report.end,report.flags));}
    }
    assert!(failures.is_empty(),"{failures:#?}");
}
