//! World-positioned one-shot sounds. LithTech plays gunfire, ricochets and
//! casings as 3D sounds with an authored radius (e.g. ricochet 1280 at
//! cshell 0x10007e1c, casing 640 at 0x10052860); the remake attenuates the
//! sink volume by listener distance and settings::update multiplies it by the
//! master volume every frame.
use bevy::prelude::*;
use crate::{InspectionCamera,SCALE};

/// Native-unit emitter position and audible radius.
#[derive(Component)] pub struct Positional {pub position:Vec3,pub radius:f32}
/// Current distance gain of a positional sink, 0..1.
#[derive(Component)] pub struct Gain(pub f32);

/// Linear falloff to silence at `radius`.
pub fn gain(distance:f32,radius:f32)->f32 {if radius<=0.0 {0.0}else{(1.0-distance/radius).clamp(0.0,1.0)}}

/// Plays `path` once at a native-unit world position with linear falloff to silence at `radius`; one call into
/// `audio::play_near`, which owns the silent/log/missing-file rules.
pub fn play_at(commands:&mut Commands,assets:&AssetServer,path:&str,position:Vec3,radius:f32) {crate::audio::play_near(commands,assets,path,position,radius);}
/// Plays `path` once without positioning (player-only sounds: hit grunt, reload, pickups).
pub fn play_2d(commands:&mut Commands,assets:&AssetServer,path:&str) {crate::audio::play_2d(commands,assets,path);}

/// The distance gain of the run: the retail linear falloff, or with "Javított 3D hangzás" the inverse-distance curve (spatial.rs) that ends at the same radius.
pub fn level(improved:bool,distance:f32,radius:f32)->f32 {if improved {crate::spatial::distance_gain(distance,radius)}else{gain(distance,radius)}}

pub fn update(camera:Single<&Transform,With<InspectionCamera>>,audio:Res<crate::spatial::SpatialAudio>,mut sounds:Query<(&Positional,&mut Gain)>) {
    let listener=camera.translation/SCALE;
    for (source,mut volume) in &mut sounds {volume.0=level(audio.enabled,listener.distance(source.position),source.radius);}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn falloff_is_linear_and_silent_at_the_radius() {
        assert_eq!(gain(0.0,1280.0),1.0);
        assert!((gain(640.0,1280.0)-0.5).abs()<1e-6);
        assert_eq!(gain(1280.0,1280.0),0.0);assert_eq!(gain(5000.0,1280.0),0.0);assert_eq!(gain(10.0,0.0),0.0);
    }
}
