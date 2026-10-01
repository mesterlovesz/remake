use bevy::prelude::*;
use serde::Deserialize;
use std::collections::BTreeMap;
use crate::{ViewerConfig,WorldGeometry,InspectionCamera,Walking,SCALE,models::Model,settings::Session,campaign::Campaign,retail_ui::{RetailUi,Catalog},panels::Panels,retail_weapons::NativeArsenal};

#[derive(Clone,Deserialize)] struct Definition {model:String,skins:BTreeMap<String,String>,animation:String,title:String,health:f32,ammo:u32,ammo_for:i32,weapon:bool,scale:f32,sound:String}
#[derive(Deserialize)] struct Placed {name:String,kind:String,pos:[f32;3],rotation:[f32;4]}
/// `half` is the item's collision box (animation dimensions x scale); `flight` is the velocity and age of a thrown item until it lands.
#[derive(Component)] pub struct Pickup {pub(crate) name:String,pub(crate) kind:String,definition:Definition,pub(crate) position:Vec3,parts:Vec<Entity>,pub(crate) half:Vec3,flight:Option<(Vec3,f32)>,/// Seconds until an NPC-dropped weapon may vanish (300 s, only once the player is 1280+ units away, cshell 0x100222c0).
    lifetime:Option<f32>}
#[derive(Resource,Default)] pub struct DropQueue {pub pending:Vec<(String,String,Vec3)>,pub grants:Vec<String>,pub thrown:Vec<(String,Vec3,Vec3)>,thrown_count:u32,world:String,spawned:std::collections::BTreeSet<String>}

pub fn spawn_drops(mut commands:Commands,config:Res<ViewerConfig>,mut queue:ResMut<DropQueue>,assets:Res<AssetServer>,mut meshes:ResMut<Assets<Mesh>>,mut materials:ResMut<Assets<StandardMaterial>>,_walking:Res<Walking>,_supports:Query<&crate::models::PropSupport>) {
    if queue.world!=config.world {queue.world=config.world.clone();queue.spawned.clear();}
    if queue.pending.is_empty() && queue.thrown.is_empty() {return;}
    let definitions:BTreeMap<String,Definition>=serde_json::from_str(&std::fs::read_to_string(config.output.join("retail_items.json")).expect("native dropped item export")).expect("dropped item catalog");
    // Items dragged out of the inventory panel (0x1001c3a0): any item kind, launched from the eye.
    let thrown=std::mem::take(&mut queue.thrown);
    if !thrown.is_empty() {
        let placed:BTreeMap<String,Definition>=std::fs::read_to_string(config.output.join("items.json")).ok().and_then(|s|serde_json::from_str(&s).ok()).unwrap_or_default();
        for (kind,origin,velocity) in thrown {
            let Some(d)=placed.get(&kind).or_else(||definitions.get(&kind)) else{warn!("Missing dropped item art: {kind}");continue;};
            let model=Model::load(&config.output,&d.model);let pose=model.pose(&d.animation,0.0,false);
            let half=model.dimensions(&d.animation)*d.scale;
            let parts=model.pieces.iter().enumerate().map(|(i,piece)| {
                let mut mesh=model.mesh(i);model.animate_mesh(i,&pose,&mut mesh);
                commands.spawn((WorldGeometry,Mesh3d(meshes.add(mesh)),MeshMaterial3d(materials.add(StandardMaterial {base_color_texture:d.skins.get(&piece.texture.to_string()).map(|s|assets.load(level_viewer::hd::path(s))),unlit:true,cull_mode:None,..default()})),Transform::from_translation(origin*SCALE).with_scale(Vec3::splat(SCALE*d.scale)))).id()
            }).collect();
            queue.thrown_count+=1;let name=format!("thrown:{}",queue.thrown_count);
            commands.spawn((WorldGeometry,Pickup {name,kind,definition:d.clone(),position:origin,parts,half,flight:Some((velocity,0.0)),lifetime:None}));
        }
    }
    for (owner,kind,origin) in std::mem::take(&mut queue.pending) {
        if !queue.spawned.insert(owner.clone()) {continue;}
        let Some(d)=definitions.get(&kind) else{warn!("Missing original dropped weapon: {kind}");continue;};
        let model=Model::load(&config.output,&d.model);let pose=model.pose(&d.animation,0.0,false);
        // An NPC's weapon starts at the actor with velocity (0,-64,0), flying, and lands like a thrown item (cshell 0x10042f95..0x1004306c).
        let half=model.dimensions(&d.animation)*d.scale;let position=origin;
        let parts=model.pieces.iter().enumerate().map(|(i,piece)| {
            let mut mesh=model.mesh(i);model.animate_mesh(i,&pose,&mut mesh);
            commands.spawn((WorldGeometry,Mesh3d(meshes.add(mesh)),MeshMaterial3d(materials.add(StandardMaterial {base_color_texture:d.skins.get(&piece.texture.to_string()).map(|s|assets.load(level_viewer::hd::path(s))),unlit:true,cull_mode:None,..default()})),Transform::from_translation(position*SCALE).with_scale(Vec3::splat(SCALE*d.scale)))).id()
        }).collect();
        commands.spawn((WorldGeometry,Pickup {name:format!("drop:{owner}"),kind,definition:d.clone(),position,parts,half,flight:Some((Vec3::new(0.0,-64.0,0.0),0.0)),lifetime:Some(300.0)}));
        info!("Gyári fegyver eldobva: {owner} / {}",d.title);
    }
}
pub fn setup_world(mut commands:Commands,config:Res<ViewerConfig>,assets:Res<AssetServer>,mut meshes:ResMut<Assets<Mesh>>,mut materials:ResMut<Assets<StandardMaterial>>,walking:Res<Walking>,supports:Query<&crate::models::PropSupport>,mut drops:ResMut<DropQueue>,campaign:Res<Campaign>) {
    let collected=campaign.restored_collected();
    drops.world=config.world.clone();drops.spawned.clear();drops.pending.clear();
    let Ok(source)=std::fs::read_to_string(config.output.join(format!("{}.items.json",config.world))) else{return};
    let definitions:BTreeMap<String,Definition>=serde_json::from_str(&std::fs::read_to_string(config.output.join("items.json")).unwrap()).unwrap();
    let placed:Vec<Placed>=serde_json::from_str(&source).unwrap();
    let mut cache=BTreeMap::<String,Vec<(Handle<Mesh>,Handle<StandardMaterial>)>>::new();
    let mut halves=BTreeMap::<String,Vec3>::new();
    for item in placed {
        let Some(d)=definitions.get(&item.kind) else{continue};
        if collected.contains(&item.name) {continue;}
        let handles=cache.entry(item.kind.clone()).or_insert_with(|| {
            let model=Model::load(&config.output,&d.model);let pose=model.pose(&d.animation,0.0,false);
            halves.insert(item.kind.clone(),model.dimensions(&d.animation)*d.scale);
            model.pieces.iter().enumerate().map(|(i,piece)|{let mut mesh=model.mesh(i);model.animate_mesh(i,&pose,&mut mesh);
                (meshes.add(mesh),materials.add(StandardMaterial {base_color_texture:d.skins.get(&piece.texture.to_string()).map(|p|assets.load(level_viewer::hd::path(p))),unlit:true,cull_mode:None,..default()}))}).collect()
        });
        let half=halves[&item.kind];let position=settled_position(&walking.world,Vec3::from(item.pos),half.y,supports.iter());
        let parts=handles.iter().map(|(mesh,material)|commands.spawn((WorldGeometry,Mesh3d(mesh.clone()),MeshMaterial3d(material.clone()),Transform {translation:position*SCALE,rotation:Quat::from_euler(EulerRot::YXZ,item.rotation[1],item.rotation[0],item.rotation[2]),scale:Vec3::splat(SCALE*d.scale)})).id()).collect();
        commands.spawn((WorldGeometry,Pickup {name:item.name,kind:item.kind,definition:d.clone(),position,parts,half,flight:None,lifetime:None}));
    }
}

fn settled_position<'a>(world:&retail_movement::CollisionWorld,origin:Vec3,half_height:f32,supports:impl Iterator<Item=&'a crate::models::PropSupport>)->Vec3 {
    // cshell.dll 10022673 calls 1001ca60 on initial placement: a 640-unit
    // downward segment, then hit Y + animation dimensions Y * item scale.
    // With no hit, retail keeps the segment endpoint instead of the spawn Y.
    let mut surface=world.raycast(native(origin),native(-Vec3::Y),640.0).map(|(distance,_)|origin.y-distance);
    for support in supports {
        let top=support.center.y+support.half_size.y;
        if (origin.x-support.center.x).abs()<=support.half_size.x && (origin.z-support.center.z).abs()<=support.half_size.z
            && top<=origin.y && top>=origin.y-640.0 && surface.is_none_or(|height|top>height) {surface=Some(top);}
    }
    Vec3::new(origin.x,surface.map_or(origin.y-640.0,|height|height+half_height),origin.z)
}
/// Reserve ammunition cap per ammo index (cshell.dll 0x10066248); a full type refuses its pickups.
pub const AMMO_MAX:[u32;10]=[170,60,150,250,300,60,200,20,12,8];
fn ammo_room(native:&NativeArsenal,index:i32)->bool {usize::try_from(index).ok().and_then(|i|AMMO_MAX.get(i)).is_none_or(|max|native.inventory.ammo.get(&index).copied().unwrap_or(0)<*max)}
fn cap_ammo(native:&mut NativeArsenal,index:i32) {if let Some(max)=usize::try_from(index).ok().and_then(|i|AMMO_MAX.get(i)) {if let Some(reserve)=native.inventory.ammo.get_mut(&index) {*reserve=(*reserve).min(*max);}}}
/// Item pickup (0x1001c100): ammunition feeds the reserve pool, a weapon takes a holster cell
/// (a second copy only gives its rounds), everything else stacks or takes a backpack cell.
/// `false` leaves the pickup on the floor because that ammunition is at its cap.
pub fn take(kind:&str,count:u32,campaign:&mut Campaign,native:&mut NativeArsenal,catalog:&Catalog)->bool {
    let Some(d)=catalog.items.get(kind) else {campaign.items.add(kind,count,false);return true};
    if d.ammo_for>=0 {
        if !ammo_room(native,d.ammo_for) {return false;}
        native.inventory.add_ammo(d.ammo_for,d.amount);cap_ammo(native,d.ammo_for);return true;
    }
    if d.weapon {
        let owned=campaign.items.has(kind);
        // A weapon without `ammo_index` (the nightstick) uses pool 0 in retail (0x1001c255).
        let pool=d.ammo_index.max(0);
        if owned && !ammo_room(native,pool) {return false;}
        let selectable=native.definitions.iter().any(|w|w.id.eq_ignore_ascii_case(kind) && w.player_selectable);
        if !owned && selectable {campaign.items.add(kind,1,true);}
        native.acquire(kind);cap_ammo(native,pool);return true;
    }
    campaign.items.add(kind,count,false);true
}
impl Pickup {pub fn in_flight(&self)->bool {self.flight.is_some()}}
fn overlaps(a:Vec3,a_half:Vec3,b:Vec3,b_half:Vec3)->bool {(a-b).abs().cmplt(a_half+b_half).all()}
/// Retail gathers every item touching the player box (0x10022360) and, while the action key is held, the 64-unit cube 64 units ahead of the eye
/// (0x10022470 via 0x1005faa0). Thrown items are ignored until they land.
pub fn tick(mut commands:Commands,items:Query<(Entity,&Pickup)>,camera:Single<&Transform,With<InspectionCamera>>,walking:Res<Walking>,session:Res<Session>,intro:Res<crate::opening::Opening>,mut campaign:ResMut<Campaign>,assets:Res<AssetServer>,mut native_arsenal:ResMut<NativeArsenal>,mut drops:ResMut<DropQueue>,ui:Res<RetailUi>,mut panels:ResMut<Panels>,bind:crate::options::Bindings) {
    // `receive` (0x10019a04) puts the item at the player, who takes it at once.
    for kind in std::mem::take(&mut drops.grants) {if take(&kind,1,&mut campaign,&mut native_arsenal,&ui.catalog) {panels.notify_pickup(&kind);}}
    if session.paused || session.dialogue_active || intro.active || campaign.dead() {return;}
    let point=camera.translation/SCALE+*camera.forward()*64.0;let use_held=bind.pressed(crate::keys_cfg::cmd::ACTION) && !panels.inventory;
    let p=walking.player.position;let (body,half)=(Vec3::new(p.x,p.y,p.z),{let h=walking.player.half_size();Vec3::new(h.x,h.y,h.z)});
    for (entity,item) in &items {
        // The player box (0x10022360) is automatic; the 64-unit cube ahead (0x10022470) is only tested from the action handler 0x1005faa0,
        // which runs while the action key (E / Space, control 0x1b) is held and the inventory panel is closed (0x100605e3, 0x1005faa0).
        let cube=use_held && overlaps(point,Vec3::splat(32.0),item.position,item.half);
        if item.flight.is_some() || !(cube || overlaps(body,half,item.position,item.half)) {continue;}
        let d=&item.definition;
        if !take(&item.kind,1,&mut campaign,&mut native_arsenal,&ui.catalog) {continue;}
        campaign.collected.insert(item.name.clone());panels.notify_pickup(&item.kind);
        if !d.sound.is_empty() {crate::audio::play_2d(&mut commands,&assets,&d.sound);}
        for &part in &item.parts {commands.entity(part).despawn();}commands.entity(entity).despawn();
        info!("Felvétel: {} / {} (gyári fegyver={})",d.title,item.name,d.weapon);
    }
}
/// Thrown items fly with gravity 640 until something stops them, then settle on the floor.
pub fn fly(mut commands:Commands,mut items:Query<(Entity,&mut Pickup)>,mut transforms:Query<&mut Transform,(Without<Pickup>,Without<InspectionCamera>)>,walking:Res<Walking>,session:Res<Session>,time:Res<Time>,supports:Query<&crate::models::PropSupport>) {
    if session.paused {return;}
    let dt=time.delta_secs().min(0.05);let n=|v:Vec3|retail_movement::Vec3::new(v.x,v.y,v.z);
    let p=walking.player.position;let player=Vec3::new(p.x,p.y,p.z);
    for (entity,mut item) in &mut items {
        // A dropped weapon runs down its 300 s lifetime and is removed once it is over and the player is 1280 units away (0x100222c0).
        if let Some(left)=item.lifetime.as_mut() {
            *left-=dt;
            if *left<=0.0 && item.position.distance(player)>=1280.0 {for &part in &item.parts {commands.entity(part).despawn();}commands.entity(entity).despawn();continue;}
        }
        let Some((mut velocity,age))=item.flight else {continue};
        // Retail flight (cshell 0x10021f90): integrate first, gravity afterwards; a blocked step does not move, drops the horizontal speed, and
        // the next blocked step lands the item (no bounce). `age` is only a safety net.
        let step=velocity*dt;let mut landed=age>4.0;
        if walking.world.sweep_box(n(item.position),n(item.half),n(step),0.05).is_some() {
            if velocity.x==0.0 && velocity.z==0.0 {landed=true;}else{velocity.x=0.0;velocity.z=0.0;}
        }else{item.position+=step;velocity.y-=640.0*dt;}
        if landed {let position=item.position;item.position=settled_position(&walking.world,position+Vec3::Y*4.0,item.half.y,supports.iter());item.flight=None;}else{item.flight=Some((velocity,age+dt));}
        for &part in &item.parts {if let Ok(mut t)=transforms.get_mut(part) {t.translation=item.position*SCALE;}}
    }
}
fn native(v:Vec3)->retail_movement::Vec3 {retail_movement::Vec3::new(v.x,v.y,v.z)}

#[cfg(test)] mod tests {
    use super::*;
    fn floor()->retail_movement::CollisionWorld {
        retail_movement::CollisionWorld::from_obj("v -100 0 -100\nv 100 0 -100\nv 100 0 100\nv -100 0 100\nf 1 3 2\nf 1 4 3").unwrap()
    }
    #[test] fn native_drop_uses_scaled_animation_height_and_nearest_support() {
        let world=floor();let origin=Vec3::new(0.0,77.0,0.0);
        assert_eq!(settled_position(&world,origin,4.55,std::iter::empty()).y,4.55);
        let table=crate::models::PropSupport {center:Vec3::new(0.0,20.0,0.0),half_size:Vec3::new(10.0,5.0,10.0)};
        assert_eq!(settled_position(&world,origin,12.0,std::iter::once(&table)).y,37.0);
        assert_eq!(settled_position(&world,Vec3::new(20.0,77.0,0.0),12.0,std::iter::once(&table)).y,12.0);
        assert_eq!(settled_position(&world,Vec3::new(0.0,700.0,0.0),12.0,std::iter::empty()).y,60.0);
    }
    #[test] fn original_cell_food_settles_on_the_bed() {
        let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
        let source=std::fs::read_to_string(root.join("rh1-wiezienie2.collision.obj")).unwrap();
        let world=retail_movement::CollisionWorld::from_obj(&source).unwrap();
        for (model,animation,scale,origin) in [
            ("models/pikapy/japco.ltb","japco",0.65,Vec3::new(176.0,77.0,872.0)),
            ("models/pikapy/corn flakes.ltb","corn flakes",2.0,Vec3::new(170.0,77.0,895.0)),
        ] {
            let half_height=Model::load(&root,model).dimensions(animation).y*scale;
            let settled=settled_position(&world,origin,half_height,std::iter::empty());
            assert!(settled.y<origin.y-30.0,"{model} did not settle: {settled:?}");
            assert!((settled.y-half_height-10.0).abs()<0.001,"Original bed support changed: {settled:?}");
            println!("{model}: source={origin:?} resting={settled:?}, half-height={half_height}");
        }
    }
}
