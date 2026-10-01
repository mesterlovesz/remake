//! Rooks (gawron-lata) circling over the level, as in the retail game.
//!
//! The original has no bird AI class: each rook is a plain `o_obiekt` whose model
//! `models\Levelowe\gawron-lata.ltb` carries the whole flight in its only animation
//! `anima` (13.333 s): `Bip01` travels a closed ~550-unit circle around the object
//! origin with wing and tail flaps baked in. cshell.dll creates the client model
//! (0x1002c481) and only calls its SetAnimation(name,loop) helper 0x1002e420 with
//! loop=0 when the objects.txt definition has `anim_raz` (0x1002c4c3..0x1002c4de);
//! `gawron-lata` has none (HP 99999, insignificant), so the engine's default tracker
//! loops animation 0 forever from level load. The viewer used to freeze it at t=0.
use bevy::{prelude::*,camera::visibility::NoFrustumCulling};
use std::collections::BTreeMap;
use crate::models::Model;

#[derive(Component)]
pub struct Bird {model:String,animation:String,piece:usize,owned_mesh:bool}

/// Retail models whose looping default animation is a baked flight path.
pub fn is_bird(model:&str)->bool {
    model.replace('\\',"/").rsplit('/').next().is_some_and(|file|file.to_lowercase().starts_with("gawron"))
}

/// Components a spawned prop piece needs to fly; the whole path lives in the skin,
/// so the static spawn-time bounds would cull the bird once it leaves them.
pub fn components(model:&str,animation:&str,piece:usize)->(Bird,NoFrustumCulling) {
    (Bird {model:model.to_owned(),animation:animation.to_owned(),piece,owned_mesh:false},NoFrustumCulling)
}

/// All rooks share the level clock (every o_obiekt starts animation 0 at load).
pub fn advance(clock:f32,dt:f32,paused:bool)->f32 {if paused {clock}else{clock+dt.max(0.0)}}

pub fn fly(mut models:Local<BTreeMap<String,Model>>,mut clock:Local<f32>,config:Res<crate::ViewerConfig>,time:Res<Time>,session:Res<crate::settings::Session>,mut meshes:ResMut<Assets<Mesh>>,mut birds:Query<(&mut Bird,&mut Mesh3d)>) {
    if birds.is_empty() {return;}
    *clock=advance(*clock,time.delta_secs(),session.paused);
    if session.paused {return;}
    for (mut bird,mut handle) in &mut birds {
        if !bird.owned_mesh {
            let Some(mesh)=meshes.get(&handle.0).cloned() else{continue};
            handle.0=meshes.add(mesh);bird.owned_mesh=true;
        }
        let model=models.entry(bird.model.clone()).or_insert_with(||Model::load(&config.output,&bird.model));
        let pose=model.pose(&bird.animation,*clock,true);
        if let Some(mesh)=meshes.get_mut(&handle.0) {model.animate_mesh(bird.piece,&pose,mesh);}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root()->std::path::PathBuf {std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output")}

    #[test]
    fn only_rook_models_fly() {
        assert!(is_bird("models/Levelowe/gawron-lata.ltb") && is_bird("models\\Levelowe\\Gawron-lata.ltb"));
        assert!(!is_bird("models/Levelowe/agawa.ltb") && !is_bird("models/Levelowe/smietnik_kwadratowy.ltb"));
    }

    #[test]
    fn pause_holds_the_shared_flight_clock() {
        assert_eq!(advance(2.0,0.5,true),2.0);
        assert_eq!(advance(2.0,0.5,false),2.5);
        assert_eq!(advance(2.0,-1.0,false),2.0);
    }

    #[test]
    fn retail_rook_definition_loops_its_default_animation() {
        let objects=std::fs::read_to_string(root().join("decoded_scripts/objects.txt")).unwrap();
        let block:Vec<&str>=objects.split("object ").find(|b|b.starts_with("gawron-lata")).expect("gawron-lata definition").lines().map(str::trim).collect();
        assert!(block.iter().any(|l|l.eq_ignore_ascii_case("model models\\levelowe\\gawron-lata.ltb")));
        assert!(!block.iter().any(|l|l.starts_with("anim")),"anim_raz/anim0 would stop the loop: {block:?}");
    }

    #[test]
    fn retail_rook_circles_its_origin_and_wraps_seamlessly() {
        let model=Model::load(&root(),"models/Levelowe/gawron-lata.ltb");
        let bip=|t:f32|model.pose("base",t,true).bones[1].transform_point3(Vec3::ZERO);
        let period=13.333;
        let samples:Vec<Vec3>=(0..8).map(|i|bip(i as f32*period/8.0)).collect();
        for p in &samples {let r=Vec2::new(p.x,p.z).length();assert!((480.0..620.0).contains(&r),"radius {r} at {p:?}");}
        assert!(samples[0].distance(samples[4])>900.0,"half a lap crosses the circle: {samples:?}");
        assert!(bip(1.0).distance(bip(1.0+period))<1.0,"animation loops");
        assert!(bip(period-0.05).distance(bip(0.0))<60.0,"end of lap meets its start");
        let still=model.pose("base",0.0,false);let moving=model.pose("base",3.0,true);
        let mut a=model.mesh(0);model.animate_mesh(0,&still,&mut a);let mut b=model.mesh(0);model.animate_mesh(0,&moving,&mut b);
        assert_ne!(format!("{:?}",a.attribute(Mesh::ATTRIBUTE_POSITION)),format!("{:?}",b.attribute(Mesh::ATTRIBUTE_POSITION)));
    }

    #[test]
    fn authored_rooks_are_exported_as_props() {
        let props:Vec<serde_json::Value>=serde_json::from_str(&std::fs::read_to_string(root().join("chinatown.props.json")).unwrap()).unwrap();
        assert_eq!(props.iter().filter(|p|is_bird(p["model"].as_str().unwrap_or(""))).count(),5);
    }
}
