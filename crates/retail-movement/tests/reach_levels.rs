//! Can a standing player walk from each campaign level's StartPoint to its exit door, its markers, characters and items?
//! `cargo test --release --test reach_levels -- --ignored --nocapture` (needs the local export). Doors are open passages for this check
//! (their leaves are movable brushes, not in the static collision mesh).
use retail_movement::{CollisionWorld,Vec3};
use serde_json::Value;
use std::path::Path;

const ORDER:[&str;27]=["rh3-miasteczko0","rh1-wiezienie1","rh1-wiezienie2","rh1-wiezienie3","rh2-wiezienie1","rh2-wiezienie2","rh3-miasteczko1","rh3-miasteczko2","burmistrz1","burmistrz2","chapel_mniejszy","knajpa","rh7a-tunele","podziemia1","podziemia1a","podziemia1b","podziemia1c","chinatown2","rh9-fabryka","rh10-wiezowiec1","rh10-wiezowiec2","rh10-wiezowiec3","wiez_wn1","wiez_wn2","wiez_wn3","rh12-lab1","rh12-lab2"];
fn vec(v:&Value)->Vec3 {Vec3::new(v[0].as_f64().unwrap_or(0.0) as f32,v[1].as_f64().unwrap_or(0.0) as f32,v[2].as_f64().unwrap_or(0.0) as f32)}
fn stem(output:&Path,id:&str)->String {
    std::fs::read_dir(output).unwrap().flatten().filter_map(|e|e.file_name().into_string().ok()).find(|n|n.to_ascii_lowercase()==format!("{id}.scene.json")).map(|n|n[..n.len()-".scene.json".len()].to_owned()).unwrap()
}
/// Door leaf bounds from its exported visual model (native units).
fn leaf_bounds(output:&Path,world:&str,name:&str)->Option<(Vec3,Vec3)> {
    let text=std::fs::read_to_string(output.join(format!("world_models/{world}/{name}.visual.obj"))).ok()?;
    let (mut low,mut high)=(Vec3::splat(f32::MAX),Vec3::splat(f32::MIN));
    for line in text.lines().filter_map(|l|l.strip_prefix("v ")) {let v:Vec<f32>=line.split_whitespace().filter_map(|t|t.parse().ok()).collect();if v.len()>=3 {let p=Vec3::new(v[0],v[1],v[2]);low=low.min(p);high=high.max(p);}}
    Some((low,high))
}
fn leaf_centre(output:&Path,world:&str,name:&str)->Option<Vec3> {leaf_bounds(output,world,name).map(|(l,h)|(l+h)*0.5)}
#[test] #[ignore = "requires local export"]
fn every_level_exit_is_walkable_from_the_start() {
    let output=Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
    for (i,id) in ORDER.iter().enumerate() {
        let stem=stem(&output,id);
        let read=|n:String|serde_json::from_str::<Value>(&std::fs::read_to_string(output.join(n)).unwrap_or("[]".into())).unwrap();
        let scene=read(format!("{stem}.scene.json"));
        let world=CollisionWorld::from_obj(&std::fs::read_to_string(output.join(format!("{stem}.collision.obj"))).unwrap()).unwrap();
        let objects=scene["objects"].as_array().unwrap();
        let start=objects.iter().find(|o|o["kind"]=="StartPoint").map(|o|vec(&o["properties"]["Pos"])).unwrap_or(Vec3::ZERO);
        let mut player=retail_movement::Player::new(start);world.place_player(&mut player);let start=player.position;
        let next=ORDER.get(i+1).copied().unwrap_or("");
        let mut line=format!("{id:<18}");
        let mut goals:Vec<(String,Vec3)>=Vec::new();
        for o in objects {
            let p=&o["properties"];let name=p["Name"].as_str().unwrap_or("");
            if o["kind"]=="b_door" && p["Skok_do_levelu"].as_str().is_some_and(|d|d.replace('\\',"/").to_ascii_lowercase().ends_with(next) && !next.is_empty()) {goals.push((format!("exit {name}"),leaf_centre(&output,&stem,name).unwrap_or(vec(&p["Pos"])) ));}
            if o["kind"]=="o_cutscene" {goals.push((format!("cutscene {}",p["Rodzaj_cuts"].as_str().unwrap_or("")),vec(&p["Pos"])));}
        }
        let portals:Vec<(Vec3,Vec3)>=objects.iter().filter(|o|o["kind"].as_str().is_some_and(|k|k=="b_door"||k.starts_with("b_szuflada"))).filter_map(|o|leaf_bounds(&output,&stem,o["properties"]["Name"].as_str()?)).filter_map(|(l,h)|CollisionWorld::portal_of(l,h)).collect();
        let (mut cells,mut expanded)=(Vec::new(),0);
        let (_,_,e)=world.plan_walk(start,Vec3::new(1e6,0.0,1e6),400000,&portals,Some(&mut cells));expanded+=e;
        line+=&format!(" region {expanded:>6} cells;");
        for (name,goal) in &goals {
            let (route,reached,_)=world.plan_walk(start,*goal,400000,&portals,None);
            line+=&format!(" {name}: {} (closest {:.0} away)",if reached {"REACHABLE"} else {"UNREACHABLE"},route.last().map(|p|((p.0.x-goal.x).powi(2)+(p.0.z-goal.z).powi(2)).sqrt()).unwrap_or(0.0));
        }
        // How much of the level's placed content lies in the walkable region.
        let grid:std::collections::HashSet<(i32,i32)>=cells.iter().map(|p|((p.x/48.0).round() as i32,(p.z/48.0).round() as i32)).collect();
        let inside=|p:Vec3|{let (cx,cz)=((p.x/48.0).round() as i32,(p.z/48.0).round() as i32);(-2..=2).any(|dx|(-2..=2).any(|dz|grid.contains(&(cx+dx,cz+dz))))};
        let gameplay=read(format!("{stem}.gameplay.json"));
        let npcs=gameplay["npcs"].as_array().cloned().unwrap_or_default();
        let items=read(format!("{stem}.items.json"));let items=items.as_array().cloned().unwrap_or_default();
        line+=&format!(" | npcs {}/{} items {}/{}",npcs.iter().filter(|n|inside(vec(&n["pos"]))).count(),npcs.len(),items.iter().filter(|n|inside(vec(&n["pos"]))).count(),items.len());
        println!("{line}");
    }
}
