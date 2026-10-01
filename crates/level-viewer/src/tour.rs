//! Capture aid (visual QA, never active in normal play): `MESTER_TOUR="stop;stop;..."` teleports the player to the next stop 0.3 s
//! after each capture time, so one headless run (`level-viewer <world> ../../output <png> t0,t1,..`) shoots every stop; stop i is seen
//! by capture i. A stop is `x,y,z[,yaw[,pitch]]` (native units, radians; no yaw = along the longest open line of sight) or
//! `t:x,y,z,distance` (stand `distance` units from the target with a clear view and look at it).
use bevy::prelude::*;
use retail_movement::Vec3 as NativeVec3;
use crate::{ViewerConfig,Walking};

#[derive(Clone,Copy)]
enum Stop {Fixed(Vec3,Option<f32>,f32),Orbit(Vec3,f32)}

#[derive(Resource,Default)]
pub struct Tour {stops:Vec<Stop>,shown:Option<usize>}

impl Tour {
    pub fn from_env()->Self {
        let numbers=|text:&str|text.split(',').filter_map(|n|n.trim().parse::<f32>().ok()).collect::<Vec<_>>();
        let stops=std::env::var("MESTER_TOUR").ok().map(|text|text.split(';').filter_map(|stop|{
            if let Some(target)=stop.strip_prefix("t:") {
                let v=numbers(target);
                return (v.len()>=4).then(||Stop::Orbit(Vec3::new(v[0],v[1],v[2]),v[3]));
            }
            let v=numbers(stop);
            (v.len()>=3).then(||Stop::Fixed(Vec3::new(v[0],v[1],v[2]),v.get(3).copied(),v.get(4).copied().unwrap_or(0.0)))
        }).collect()).unwrap_or_default();
        Self {stops,shown:None}
    }
}

fn native(v:Vec3)->NativeVec3 {NativeVec3::new(v.x,v.y,v.z)}

/// Free distance (up to `cap`) of the horizontal ray from `from` along the yaw.
fn free_distance(walking:&Walking,from:Vec3,yaw:f32,cap:f32)->f32 {
    let direction=Quat::from_rotation_y(yaw)*Vec3::NEG_Z;
    walking.world.raycast(native(from),native(direction),cap).map_or(cap,|(d,_)|d)
}

/// The yaw (Bevy convention) whose horizontal ray from the eye travels farthest.
fn open_yaw(walking:&Walking,eye:Vec3)->f32 {
    (0..32).map(|i|i as f32*std::f32::consts::TAU/32.0).map(|yaw|(yaw,free_distance(walking,eye,yaw,20000.0)))
        .max_by(|a,b|a.1.total_cmp(&b.1)).map_or(0.0,|(yaw,_)|yaw)
}

/// A tour looks at the level, it does not play it: health is restored after the campaign tick (drowning, gunfire) every frame.
pub fn god(tour:Res<Tour>,mut campaign:ResMut<crate::campaign::Campaign>) {
    if !tour.stops.is_empty() {campaign.health=campaign.max_health;}
}

pub fn tick(mut tour:ResMut<Tour>,config:Res<ViewerConfig>,time:Res<Time>,mut walking:ResMut<Walking>,mut frames:Local<(f32,u32)>) {
    if tour.stops.is_empty() || config.capture.is_none() {return;}
    frames.0+=time.delta_secs();frames.1+=1;
    let elapsed=time.elapsed_secs();
    let index=config.capture_times.iter().rposition(|t|elapsed>*t+0.3).map_or(0,|i|i+1).min(tour.stops.len()-1);
    if tour.shown==Some(index) {return;}
    // Frame time of the stop just left (the first interval includes the level load).
    if tour.shown.is_some() && frames.1>0 {info!("Túra kockaidő {}: {:.1} ms ({} kocka)",tour.shown.unwrap_or(0),frames.0/frames.1 as f32*1000.0,frames.1);}
    *frames=(0.0,0);
    tour.shown=Some(index);
    let (position,yaw,pitch,target)=match tour.stops[index] {
        Stop::Fixed(position,yaw,pitch)=>(position,yaw,pitch,None),
        Stop::Orbit(target,distance)=>{
            // Stand at eye height on the side with the most room, then look at the target.
            let best=(0..24).map(|i|i as f32*std::f32::consts::TAU/24.0).map(|yaw|(yaw,free_distance(&walking,target,yaw,distance+40.0)))
                .fold((0.0,-1.0f32),|best,candidate|if candidate.1>best.1+1.0 {candidate}else{best});
            let direction=Quat::from_rotation_y(best.0)*Vec3::NEG_Z;
            let stand=target+direction*(best.1-40.0).clamp(20.0,distance);
            (Vec3::new(stand.x,target.y-46.0,stand.z),None,0.0,Some(target))
        }
    };
    walking.player.position=native(position);
    walking.player.velocity=NativeVec3::ZERO;
    let Walking {world,player,..}=&mut *walking;
    world.place_player(player);
    let eye=Vec3::new(player.position.x,player.position.y+46.0,player.position.z);
    if let Some(target)=target {
        let toward=target-eye;
        walking.yaw=(-toward.x).atan2(-toward.z);
        walking.pitch=toward.y.atan2(Vec2::new(toward.x,toward.z).length());
    }else{
        walking.yaw=yaw.unwrap_or_else(||open_yaw(&walking,eye));
        walking.pitch=pitch;
    }
    info!("Túra {index}: {:.0},{:.0},{:.0} yaw {:.2}",eye.x,eye.y,eye.z,walking.yaw);
}

/// MESTER_PICK="px,py;px,py" (pixels of a 1280x720 capture): once, 5 s into the run, logs what the level camera sees there (entity,
/// texture, mesh triangle, surface UV) so a defect in a capture can be traced to its polygon.
pub fn pick(mut ray_cast:bevy::picking::mesh_picking::ray_cast::MeshRayCast,camera:Single<(&Camera,&GlobalTransform),With<crate::InspectionCamera>>,materials:Query<&MeshMaterial3d<crate::retail_world::RetailWorld>>,worlds:Res<Assets<crate::retail_world::RetailWorld>>,assets:Res<AssetServer>,meshes:Query<&Mesh3d>,mesh_assets:Res<Assets<Mesh>>,time:Res<Time>,mut done:Local<bool>) {
    let Some(points)=std::env::var("MESTER_PICK").ok() else{return};
    if *done || time.elapsed_secs()<5.0 {return;}
    *done=true;
    for point in points.split(';') {
        let v:Vec<f32>=point.split(',').filter_map(|n|n.trim().parse().ok()).collect();
        if v.len()<2 {continue;}
        let Ok(ray)=camera.0.viewport_to_world(camera.1,Vec2::new(v[0],v[1])) else{continue};
        let settings=bevy::picking::mesh_picking::ray_cast::MeshRayCastSettings::default().with_visibility(bevy::picking::mesh_picking::ray_cast::RayCastVisibility::Any).with_filter(&|_|true);
        // Backfaces are skipped by the default culling of the cast, so report both sides.
        let hits=ray_cast.cast_ray(ray,&settings);
        for (entity,hit) in hits.iter().take(3) {
            let texture=materials.get(*entity).ok().and_then(|m|worlds.get(&m.0)).and_then(|m|m.base.as_ref()).and_then(|h|assets.get_path(h.id())).map(|p|p.to_string()).unwrap_or_default();
            let uv=meshes.get(*entity).ok().and_then(|m|mesh_assets.get(&m.0)).zip(hit.triangle_index).and_then(|(mesh,index)|{
                let bevy::mesh::VertexAttributeValues::Float32x2(uvs)=mesh.attribute(Mesh::ATTRIBUTE_UV_0)? else{return None};
                let b=hit.barycentric_coords;
                let corner=|i:usize|Vec2::from(uvs[index*3+i]);
                Some(corner(0)*b.x+corner(1)*b.y+corner(2)*b.z)
            });
            info!("Kiválasztás {:?}: {:?} {:.0} egység, pont {:.0},{:.0},{:.0}, normál {:.2},{:.2},{:.2}, textúra {texture}, uv {uv:?}",point,entity,hit.distance*100.0,hit.point.x*100.0,hit.point.y*100.0,hit.point.z*100.0,hit.normal.x,hit.normal.y,hit.normal.z);
        }
    }
}
