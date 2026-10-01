//! Original first-person models and combat; movement remains an independent choice.
use bevy::{camera::visibility::{NoFrustumCulling,RenderLayers},prelude::*};
use std::collections::BTreeMap;
use serde::Deserialize;
use crate::{ViewerConfig,Walking,InspectionCamera,SCALE,models::Model,retail_state::{Catalog,Definition,Inventory,Snapshot,ActionKind},settings::Session,gunfire::{Controls,Effects}};

#[derive(Deserialize)] struct ItemGrant {ammo:u32,ammo_for:i32}
#[derive(Resource)] pub struct NativeArsenal {
    pub definitions:Vec<Definition>,pub inventory:Inventory,models:Vec<Model>,items:BTreeMap<String,ItemGrant>,
    pub shots:u32,pub hits:u32,pub reloads:u32,pub pose_updates:u32,pub grenade_throws:u32,pub explosions:u32,rng:u32,
    grenade_parts:Vec<(Handle<Mesh>,Handle<StandardMaterial>)>,
}
impl NativeArsenal {
    pub fn acquire(&mut self,id:&str)->bool {
        if self.inventory.acquire(&self.definitions,id) {return true;}
        if let Some(item)=self.items.get(id) {self.inventory.add_ammo(item.ammo_for,item.ammo);}
        false
    }
    /// Probe helper only: retail never draws a picked-up weapon (the number key does, cshell 0x10060c8f).
    pub fn acquire_and_draw(&mut self,id:&str)->bool {
        let taken=self.acquire(id);
        if let Some(slot)=self.definitions.iter().position(|d|d.id.eq_ignore_ascii_case(id)) {self.inventory.select(slot);}
        taken
    }
    pub fn reset(&mut self) {self.inventory=Inventory::new(&self.definitions);}
    /// The weapon left the holster (dropped): no longer owned, deselected.
    pub fn release(&mut self,id:&str) {
        if let Some(slot)=self.definitions.iter().position(|d|d.id.eq_ignore_ascii_case(id)) {
            self.inventory.weapons[slot].owned=false;self.inventory.weapons[slot].magazine=0;
            if self.inventory.selected==Some(slot) {self.inventory.selected=None;self.inventory.equipped=false;self.inventory.cancel_action();}
        }
    }
    pub fn snapshot(&self)->Snapshot {self.inventory.snapshot(&self.definitions)}
    pub fn restore(&mut self,saved:&Snapshot) {self.inventory.restore(&self.definitions,saved);}
    pub fn reloading(&self)->bool {self.inventory.action.kind==ActionKind::Reload}
    /// Bevy view-space point (render units) of a socket on the drawn first-person weapon, for muzzle flashes and casings.
    pub fn socket_view(&self,slot:usize,socket:&str,view:&crate::view::ViewState)->Option<Vec3> {
        let (name,time,looping)=self.inventory.pose(&self.definitions)?;
        let model=self.models.get(slot)?;
        let matrix=view.weapon(view_transform(self.definitions.get(slot)?)).to_matrix()*model.try_socket(&model.pose(name,time,looping),socket)?;
        Some(matrix.transform_point3(Vec3::ZERO))
    }
    fn random_offset(&mut self,spread:f32)->f32 {
        let mut next=|| {self.rng=self.rng.wrapping_mul(214013).wrapping_add(2531011);((self.rng>>16)&0x7fff)%1000};
        (next() as f32-next() as f32)*spread*0.001
    }
}
#[derive(Component)] pub struct NativeModel {slot:usize,piece:usize}
#[derive(Component)] pub struct NativeCamera;
#[derive(Component)] pub struct NativeHud;
#[derive(Component)] pub struct NativeNumber(bool);
#[derive(Component)] pub struct NativeStatus;
#[derive(Component)] pub struct Grenade {position:Vec3,velocity:Vec3,fuse:f32}
#[derive(Deserialize)] struct BlastDefinition {sound:String,impact_sound:String}
#[derive(Resource)] pub struct BlastAssets {definition:BlastDefinition}
/// The blast effect objects of EffectMgr (docs/retail-blast.md): the type 8 controller (cshell 0x10053ab0), its type 9 flying embers (0x100541e0) and their type 0x0a puffs (0x10054280).
#[derive(Component)] pub struct Blast {pos:Vec3,age:f32,flash:bool,fire:u8,smoke:u8}
#[derive(Component)] pub struct BlastEmber {pos:Vec3,vel:Vec3,age:f32,life:f32,timer:f32}
#[derive(Component)] pub struct BlastPuff {pos:Vec3,scale:f32,age:f32,flash:bool,smoke:bool}

pub fn setup(mut commands:Commands,config:Res<ViewerConfig>,assets:Res<AssetServer>,mut meshes:ResMut<Assets<Mesh>>,mut materials:ResMut<Assets<StandardMaterial>>) {
    let catalog:Catalog=serde_json::from_str(&std::fs::read_to_string(config.output.join("retail_weapons.json")).expect("original weapon export missing; run tools.export_weapons")).expect("original weapon export format");
    assert_eq!(catalog.schema_version,1);
    let raw:serde_json::Value=serde_json::from_str(&std::fs::read_to_string(config.output.join("retail_weapons.json")).unwrap()).unwrap();
    let blast:BlastDefinition=serde_json::from_value(raw["grenade_effect"].clone()).expect("original explosion export");
    commands.insert_resource(BlastAssets {definition:blast});
    let mut models=Vec::new();
    for (slot,d) in catalog.weapons.iter().enumerate() {
        let model=Model::load(&config.output,&d.model);let pose=model.pose(&d.animations.base,0.0,true);
        for (piece_index,piece) in model.pieces.iter().enumerate() {
            let mut mesh=model.mesh(piece_index);model.animate_mesh(piece_index,&pose,&mut mesh);
            commands.spawn((NativeModel {slot,piece:piece_index},Mesh3d(meshes.add(mesh)),MeshMaterial3d(materials.add(StandardMaterial {
                base_color_texture:d.skins.get(&piece.texture.to_string()).map(|p|assets.load(level_viewer::hd::path(p))),unlit:true,cull_mode:None,..default()})),
                Transform::default(),Visibility::Hidden,RenderLayers::layer(3),NoFrustumCulling));
        }
        models.push(model);
    }
    let grenade_model=Model::load(&config.output,"models/pikapy/granat_low.ltb");
    let grenade_parts=grenade_model.pieces.iter().enumerate().map(|(i,_)|(meshes.add(grenade_model.mesh(i)),materials.add(StandardMaterial {
        base_color_texture:Some(assets.load(level_viewer::hd::path("model_textures/skins/pikapy/granat_low.dtx.png"))),unlit:true,cull_mode:None,..default()}))).collect();
    let inventory=Inventory::new(&catalog.weapons);
    let mut items:BTreeMap<String,ItemGrant>=serde_json::from_str(&std::fs::read_to_string(config.output.join("items.json")).expect("item export")).expect("item grants");
    items.extend(serde_json::from_str::<BTreeMap<String,ItemGrant>>(&std::fs::read_to_string(config.output.join("retail_items.json")).expect("native item export")).expect("native item grants"));
    commands.insert_resource(NativeArsenal {definitions:catalog.weapons,inventory,models,items,shots:0,hits:0,reloads:0,pose_updates:0,grenade_throws:0,explosions:0,rng:1,grenade_parts});
    commands.spawn((NativeCamera,Camera3d::default(),Camera {order:1,is_active:false,clear_color:ClearColorConfig::None,..default()},
        Projection::Perspective(PerspectiveProjection {fov:65f32.to_radians(),near:0.001,..default()}),bevy::core_pipeline::tonemapping::Tonemapping::None,
        Transform::IDENTITY,RenderLayers::layer(3)));
    let orange=Color::srgb(1.0,0.65,0.13);let font=assets.load("hud/subtitles.ttf");
    commands.spawn((NativeHud,ImageNode {color:orange,..default()},Node {position_type:PositionType::Absolute,right:px(12),bottom:px(5),width:px(256),height:px(128),..default()},Visibility::Hidden,GlobalZIndex(20)))
        .with_children(|panel| {
            for (reserve,top) in [(false,8.0),(true,51.0)] {panel.spawn((NativeNumber(reserve),Text::new(""),TextFont {font:font.clone(),font_size:28.0,..default()},TextColor(orange),Node {position_type:PositionType::Absolute,right:px(12),top:px(top),..default()}));}
            panel.spawn((NativeStatus,Text::new(""),TextFont {font:font.clone(),font_size:12.0,..default()},TextColor(orange),Node {position_type:PositionType::Absolute,left:px(5),bottom:px(10),..default()}));
        });
}

fn sound(commands:&mut Commands,assets:&AssetServer,d:&Definition,key:&str) {
    if let Some(Some(path))=d.sounds.get(key) {crate::audio::play_2d(commands,assets,path);}
}
pub(crate) fn view_transform(d:&Definition)->Transform {
    Transform {translation:Vec3::new(d.offset[0],d.offset[1],-d.offset[2])*SCALE,
        rotation:Quat::IDENTITY,scale:Vec3::from(d.scale)*Vec3::new(1.0,1.0,-1.0)*SCALE}
}
/// Player progression handles `tick` needs (bundled: Bevy systems take at most 16 parameters).
#[derive(bevy::ecs::system::SystemParam)] pub struct Progress<'w> {campaign:ResMut<'w,crate::campaign::Campaign>,ui:Res<'w,crate::retail_ui::RetailUi>,panels:Res<'w,crate::panels::Panels>,alt:ResMut<'w,crate::weapons_alt::AltFire>}
pub fn tick(mut commands:Commands,mut native:ResMut<NativeArsenal>,view:Res<crate::view::ViewState>,surfaces:Res<crate::audio::Surfaces>,session:Res<Session>,controls:Res<Controls>,time:Res<Time>,walking:Res<Walking>,camera:Single<&Transform,With<InspectionCamera>>,effects:Res<Effects>,assets:Res<AssetServer>,opening:Res<crate::opening::Opening>,mut npcs:ResMut<crate::npcs::NpcRoster>,doors:Query<(Entity,&crate::doors::Door)>,progress:Progress) {
    let Progress {mut campaign,ui,panels,mut alt}=progress;
    if opening.active || session.paused || session.dialogue_active {return;}
    // Keys 1..8 pick the holster cell of that number (0x1001c060); there is no other way to draw a weapon.
    let choose=|native:&mut NativeArsenal,id:Option<String>|{if let Some(slot)=id.and_then(|id|native.definitions.iter().position(|d|d.id.eq_ignore_ascii_case(&id))) {native.inventory.select(slot);}};
    if let Some(key)=controls.native_slot.filter(|k|*k<8) {choose(&mut native,campaign.items.holster(key as u32,&ui.catalog).map(str::to_owned));}
    // No shot while the inventory or the attribute screen owns the mouse (0x10060caa).
    let (held,fire)=if panels.blocks_fire() {(false,false)}else{(controls.held,controls.fire)};
    let outcome={let NativeArsenal {definitions,inventory,..}=&mut *native;inventory.tick(definitions,time.delta_secs().min(0.05),held,fire,controls.reload)};
    // A magazine weapon plays `sound_reload` when the reload starts, a shotgun once per shell (cshell 0x10003dcc, 0x100100a0).
    if outcome.reload_started {native.reloads+=1;if let Some(slot)=native.inventory.selected {if !native.definitions[slot].shotgun {sound(&mut commands,&assets,&native.definitions[slot],"reload");}info!("Gyári újratöltés: {}",native.definitions[slot].id);}}
    if let Some(slot)=native.inventory.selected {for _ in 0..outcome.shells_inserted {sound(&mut commands,&assets,&native.definitions[slot],"reload");}}
    // The grenade's `sound_shoot` (zawleka.wav) is never played: cshell reads +0x19d8 only for the melee swing (0x10003eb3), the muzzle of a shot (0x100051a6)
    // and an NPC weapon (0x10046729), and a grenade takes neither path (0x10003f90 sends it to 0x10003e20).
    if outcome.eject_casing {if let Some(slot)=native.inventory.selected {crate::gunfire::player_casing(&mut commands,&effects,&native,slot,&camera,&view);}}
    if outcome.eject_casings {if let Some(slot)=native.inventory.selected {crate::gunfire::reload_casings(&mut commands,&effects,&native,slot,&camera,&view);}}
    // The last grenade is gone from the holster and the inventory grid once thrown (0x1001bf40).
    if let Some(slot)=outcome.grenade_spent {let id=native.definitions[slot].id.clone();campaign.items.remove_item(&id);}
    let Some(slot)=outcome.shot else{return};native.shots+=1;
    let d=native.definitions[slot].clone();let direction=*camera.forward();let origin=camera.translation;
    if !d.grenade {crate::gunfire::shot_sound(&mut commands,&effects,&d,&camera);}
    if d.melee {alt.begin_swing();return;}
    // Every trace call raises a gunshot stimulus at the shooter: 128 units through walls, `glosnosc` with a clear line, one frame for the player (0x10005dd9).
    if !d.grenade {npcs.add_noise(crate::npcs::Stimulus::player_gunshot(origin/SCALE,crate::gunfire::item_number(&effects,&d.id,"glosnosc").unwrap_or(0.0)));}
    if !d.grenade && !d.melee {crate::gunfire::player_shot(&mut commands,&effects,&native,slot,&camera,&view);}
    if d.grenade {
        native.grenade_throws+=1;
        // Thrown from 52 units ahead and 8 to the right of the eye (0x1000afd9..0x1000b076); a wall on the way makes it go off at once
        // (0x1000b07a..0x1000b08e). Held too long it explodes at the hand socket (0x1000adc6).
        let eye=origin/SCALE;let target=eye+direction*52.0+crate::mirror::right(&camera)*8.0;
        let n=|v:Vec3|retail_movement::Vec3::new(v.x,v.y,v.z);
        let blocked=walking.world.raycast(n(eye),n((target-eye).normalize_or_zero()),(target-eye).length()).is_some()
            || doors.iter().any(|(_,door)|door.shot_hit(eye,(target-eye).normalize_or_zero(),(target-eye).length()).is_some());
        let hand=native.socket_view(slot,"granat",&view).map(|point|crate::mirror::to_world(&camera,point)/SCALE);
        let position=if outcome.grenade_in_hand {hand.unwrap_or(target)}else{target};
        let in_hand=outcome.grenade_in_hand || blocked;
        let velocity=if in_hand {Vec3::ZERO}else{direction*640.0+Vec3::Y*256.0};
        let mut entity=commands.spawn((crate::WorldGeometry,Grenade {position,velocity,fuse:if in_hand {0.0}else{outcome.grenade_fuse.unwrap_or(3.0)}},Transform::from_translation(position*SCALE),Visibility::Inherited));
        entity.with_children(|parent| {for (mesh,material) in &native.grenade_parts {parent.spawn((Mesh3d(mesh.clone()),MeshMaterial3d(material.clone()),Transform::from_scale(Vec3::splat(SCALE*2.0))));}});
    }else {
        let range=if d.melee {110.0}else{640.0+24.0*(256.0-d.spread).max(0.0)};
        // The weapon skill narrows the player's spread and grows by 0.1 per pellet that hits a character (0x10005fb7, 0x10006aa1).
        let skill=ui.catalog.items.get(&d.id).and_then(|i|i.experience_index).unwrap_or(-1);let spread_scale=campaign.stats.spread(1.0,skill);
        for pellet in 0..d.pellets.max(1) {
            let scatter=d.spread*spread_scale;
            let spread=if d.melee {Vec3::ZERO}else{Vec3::new(native.random_offset(scatter),native.random_offset(scatter),native.random_offset(scatter))};
            let ray=(direction*range+spread).normalize_or_zero();
            let shot=crate::gunfire::shoot_outcome(&mut commands,&walking.world,&mut npcs,&doors,&surfaces,&effects,&d.id,origin,ray,range,&|_|d.damage,!d.melee && pellet==0);
            if shot.hit || shot.world_hit {native.hits+=1;}
            if shot.hit {campaign.stats.record_hit(skill);}
            // Experience is the victim's `exp_gained` (postacie.txt), paid when the player's shot kills it.
            if shot.killed {let exp=npcs.actors.iter().find(|a|a.name==shot.name && !a.alive()).map_or(0,|a|a.exp_gained());campaign.experience+=exp;campaign.kills+=1;}
        }
    }
    info!("Gyári lövés: {} tár={} tartalék={} hits={}",d.id,native.inventory.weapons[slot].magazine,native.inventory.reserve(&native.definitions,slot),native.hits);
}

pub fn present(mut native:ResMut<NativeArsenal>,view:Res<crate::view::ViewState>,opening:Res<crate::opening::Opening>,assets:Res<AssetServer>,mut meshes:ResMut<Assets<Mesh>>,mut models:Query<(&NativeModel,&Mesh3d,&mut Transform,&mut Visibility),Without<NativeHud>>,mut camera:Single<&mut Camera,With<NativeCamera>>,mut panel:Single<(&mut ImageNode,&mut Visibility),(With<NativeHud>,Without<NativeModel>)>,mut numbers:Query<(&NativeNumber,&mut Text),Without<NativeStatus>>,mut status:Single<&mut Text,(With<NativeStatus>,Without<NativeNumber>)>) {
    let active=native.inventory.equipped && native.inventory.selected.is_some() && !opening.active;
    camera.is_active=active;*panel.1=if active {Visibility::Visible}else{Visibility::Hidden};
    let selected=native.inventory.selected;
    let sample=native.inventory.pose(&native.definitions).map(|(name,t,looping)|(name.to_string(),t,looping));
    for (part,handle,mut transform,mut visible) in &mut models {
        *visible=if active && selected==Some(part.slot) {Visibility::Visible}else{Visibility::Hidden};
        if !active || selected!=Some(part.slot) {continue;}
        let d=&native.definitions[part.slot];
        // LithTech's left-handed view (+Z forward) becomes Bevy's right-handed
        // view (-Z forward). Reflect Z without mirroring the authored right hand.
        *transform=view.weapon(view_transform(d));
        if let (Some((name,t,looping)),Some(mesh))=(&sample,meshes.get_mut(&handle.0)) {let model=&native.models[part.slot];model.animate_mesh(part.piece,&model.pose(name,*t,*looping),mesh);}
    }
    if active {native.pose_updates+=1;let slot=selected.unwrap();let d=&native.definitions[slot];
        panel.0.image=assets.load(d.hud.clone());
        for (kind,mut text) in &mut numbers {let count=if d.melee {1}else if kind.0 {native.inventory.reserve(&native.definitions,slot)}else{native.inventory.weapons[slot].magazine};text.0=if kind.0 {format!("{count:03}")}else{format!("{count:02}")};}
        status.0=if native.reloading() {"ÚJRATÖLTÉS".into()}else{d.title.clone()};
    }
}

pub fn projectiles(mut commands:Commands,session:Res<Session>,opening:Res<crate::opening::Opening>,time:Res<Time>,walking:Res<Walking>,mut grenades:Query<(Entity,&mut Grenade,&mut Transform)>,mut npcs:ResMut<crate::npcs::NpcRoster>,doors:Query<(Entity,&crate::doors::Door)>,mut campaign:ResMut<crate::campaign::Campaign>,blast:Res<BlastAssets>,assets:Res<AssetServer>,mut native:ResMut<NativeArsenal>,view:Res<crate::view::ViewState>) {
    if opening.active || session.paused || session.dialogue_active {return;}
    let dt=time.delta_secs().min(0.05);
    for (entity,mut grenade,mut transform) in &mut grenades {
        grenade.fuse-=dt;
        let p=grenade.position;let n=|v:Vec3|retail_movement::Vec3::new(v.x,v.y,v.z);
        let ground=walking.world.raycast(n(p),n(-Vec3::Y),(16.0-grenade.velocity.y*dt).max(0.0)).is_some();
        if ground {grenade.velocity*=30.0/(630.0*dt.max(0.05));if grenade.velocity.y< -16.0 {grenade.velocity.y*= -0.3;grenade.velocity*=0.65;
            crate::audio::play_near(&mut commands,&assets,&blast.definition.impact_sound,p,640.0);
        }}else{grenade.velocity.y-=640.0*dt;}
        let step=grenade.velocity*dt;let half=Vec3::new(8.0,4.0,8.0);
        let wall=walking.world.sweep_box(n(p),n(half),n(step),0.05).map(|(t,n)|(t,Vec3::new(n.x,n.y,n.z)));
        let door=doors.iter().filter_map(|(_,door)|door.sweep(p,half,step)).min_by(|a,b|a.0.total_cmp(&b.0));
        if let Some((fraction,normal))=wall.into_iter().chain(door).min_by(|a,b|a.0.total_cmp(&b.0)) {
            grenade.position+=step*fraction;
            // The portable collision adapter clips the native physical box at
            // walls; the source ground bounce above remains frame dependent.
            let into=grenade.velocity.dot(normal);if into<0.0 {grenade.velocity-=normal*into;}
        }else{grenade.position+=step;}
        transform.translation=grenade.position*SCALE;
        if grenade.fuse>0.0 {continue;}
        let center=grenade.position;let radius=640.0;
        npcs.add_noise(crate::npcs::Stimulus::explosion(center));
        npcs.blast(center,&walking.world,|origin,target| {let delta=target-origin;doors.iter().any(|(_,door)|door.shot_hit(origin,delta.normalize_or_zero(),delta.length()).is_some())});
        let player=walking.player.position;let target=Vec3::new(player.x,player.y,player.z);let distance=target.distance(center);let origin=center+Vec3::Y*32.0;let delta=target-origin;
        if distance<radius && walking.world.raycast(n(origin),n(delta.normalize_or_zero()),delta.length()).is_none() && !doors.iter().any(|(_,door)|door.shot_hit(origin,delta.normalize_or_zero(),delta.length()).is_some()) {campaign.change_health(-(radius-distance)*0.15625,view.level_frames());}
        spawn_blast(&mut commands,&blast,&assets,center);
        native.explosions+=1;info!("Gyári gránát robbanás: {center:?}");commands.entity(entity).despawn();
    }
}

/// A grenade-class blast at a native-unit position: the type 8 controller object (cshell 0x1005b120 spawns it; its update 0x10053ab0 does everything else).
pub fn spawn_blast(commands:&mut Commands,_blast:&BlastAssets,_assets:&AssetServer,center:Vec3) {
    commands.spawn((crate::WorldGeometry,Blast {pos:center,age:0.0,flash:false,fire:0,smoke:0}));
}

#[derive(Debug,PartialEq,Clone,Copy)] pub enum BlastEvent {Flash,Fire,SecondFire,Smoke(usize)}
/// Smoke puffs of the controller: (rise speed, scale, growth per second), stages 0..2 (0x10053fb2, 0x10054040, 0x100540ce; a fourth at 0x1005415f is unreachable).
const BLAST_SMOKE_STAGES:[(f32,f32,f32);3]=[(48.0,1.9,0.2),(32.0,2.1,0.1),(16.0,2.3,0.0)];
impl Blast {
    /// The stage tests of 0x10053ab0 against the age BEFORE this frame's advance, in the order of the code; each fires once.
    fn events(&mut self)->Vec<BlastEvent> {
        let age=self.age;let mut events=Vec::new();
        if age>0.0 && !self.flash {self.flash=true;events.push(BlastEvent::Flash);}
        if age>0.1 && self.fire==0 {self.fire=1;events.push(BlastEvent::Fire);}
        if age>0.6 && self.fire==1 {self.fire=2;events.push(BlastEvent::SecondFire);}
        if age>0.3 && self.fire==2 {self.fire=3;}
        for (limit,stage) in [(0.9,0u8),(0.6,1),(0.8,2),(1.1,3)] {
            if age>limit && self.smoke==stage {self.smoke=stage+1;if let Some(_)=BLAST_SMOKE_STAGES.get(stage as usize) {events.push(BlastEvent::Smoke(stage as usize));}}
        }
        events
    }
}
fn triangular(scale:f32)->f32 {(crate::fx::rnd()-crate::fx::rnd())*scale}
const BLAST_FLASH:&str="sprites/weapons/systemblikwybuch.spr";const BLAST_FIRE:&str="sprites/weapons/systemwybduzy1.spr";const BLAST_SMOKE:&str="sprites/weapons/systemdymduzy.spr";const BLAST_EMBER:&str="sprites/weapons/systemwybmortyr.spr";

/// EffectMgr type 8 (0x10053ab0): the controller lives 1.0 s (Spawn 0x100514d7) and its stages test the age BEFORE it is advanced, so the `age > 1.1` smoke is never reached.
/// 0: flash (additive, 0.45 s, scale 4) + light (0.2 s, r256, (.85,.95,.99)) + sound; 0.1: fireball (1.1) + 12 embers (type 9) + 32 chunks (type 0x0b); 0.6: second fireball (1.9, 32 above);
/// 0.9 (and the 0.6 / 0.8 tests that follow at once): three multiplicative smoke puffs rising 48 / 32 / 16 u/s.
#[allow(clippy::too_many_arguments)]
pub fn explosions(mut commands:Commands,session:Res<Session>,time:Res<Time>,assets:Res<AssetServer>,blast:Res<BlastAssets>,effects:Option<Res<Effects>>,
    mut blasts:Query<(Entity,&mut Blast)>,mut embers:Query<(Entity,&mut BlastEmber)>,mut puffs:Query<(Entity,&mut BlastPuff)>) {
    use crate::fx::{self,Particle};
    if session.paused || session.dialogue_active {return;}
    let Some(effects)=effects.as_deref() else {return};let lib=&effects.lib;let dt=time.delta_secs().min(0.05);
    for (entity,mut b) in &mut blasts {
        let center=b.pos;
        for event in b.events() {match event {
            BlastEvent::Flash=>{
                fx::spawn(&mut commands,Particle::sprite(lib,BLAST_FLASH,center).map(|p|p.scale(4.0).life(0.45)));
                fx::light(&mut commands,center,[0.85,0.95,0.99],256.0,0.2);
                // The rocket / grenade blast is a 3D sound of radius 1280 (docs/retail-audio.md: ricochet and rocket).
                crate::audio::play_near(&mut commands,&assets,&blast.definition.sound,center,1280.0);
            },
            BlastEvent::Fire=>{
                fx::spawn(&mut commands,Particle::sprite(lib,BLAST_FIRE,center).map(|p|p.scale(1.1).life(1.45)));
                for _ in 0..12 {
                    let offset=Vec3::new(triangular(48.0),fx::rnd()*32.0+4.0,triangular(48.0));
                    commands.spawn((crate::WorldGeometry,BlastEmber {pos:center+offset,vel:offset*10.0,age:0.0,life:0.5+fx::rnd()*0.2,timer:0.0}));
                }
                for _ in 0..32 {
                    let offset=Vec3::new(triangular(32.0),fx::rnd()*24.0,triangular(32.0));let life=fx::rnd()*0.8+0.9;let (position,velocity)=(center+offset,offset*16.0);
                    let chunk=format!("models/misc/k{}.ltb",1+fx::pick(4));
                    fx::spawn(&mut commands,Particle::model(lib,&chunk,position).map(|p|p.scale(2.0).life(life).vel(velocity).gravity(240.0).spinning(18.8496)));
                    // The chunk's own `ogon.spr` streak (0x1004f80f): additive, along the velocity, lives and falls with it.
                    fx::spawn(&mut commands,Particle::sprite(lib,"sprites/ogon.spr",position).map(|p|p.scale2(0.25,1.6).life(life).vel(velocity).gravity(240.0).streak()));
                }
            },
            BlastEvent::SecondFire=>{fx::spawn(&mut commands,Particle::sprite(lib,BLAST_FIRE,center+Vec3::Y*32.0).map(|p|p.scale(1.9).life(1.45)));},
            BlastEvent::Smoke(stage)=>{
                let (rise,scale,growth)=BLAST_SMOKE_STAGES[stage];
                fx::spawn(&mut commands,Particle::sprite(lib,BLAST_SMOKE,center).map(|p|p.scale(scale).life(2.5).vel(Vec3::Y*rise).growth(growth).multiply()));
            },
        }}
        b.age+=dt;
        if b.age>1.0 {commands.entity(entity).despawn();}
    }
    for (entity,mut e) in &mut embers {
        // 0x100541e0: every 0.03 s of accumulated frame time a type 0x0a puff, sized by the remaining life; then gravity mode 2 (240) and the motion step (0x10052290).
        e.timer+=dt;
        if e.timer>0.03 {e.timer=0.0;commands.spawn((crate::WorldGeometry,BlastPuff {pos:e.pos,scale:(e.life-e.age)*0.24,age:0.0,flash:false,smoke:false}));}
        e.vel.y-=240.0*dt;let step=e.vel*dt;e.pos+=step;
        e.age+=dt;if e.age>e.life {commands.entity(entity).despawn();}
    }
    for (entity,mut puff) in &mut puffs {
        // 0x10054280: an additive `systemwybmortyr` flash at once (scale + 0.02, 1.15 s) and a multiplicative smoke after 0.3 s (scale + 0.09, 2.5 s); the puff object lives 1.0 s.
        let (age,pos,scale)=(puff.age,puff.pos,puff.scale);
        if age>0.0 && !puff.flash {puff.flash=true;fx::spawn(&mut commands,Particle::sprite(lib,BLAST_EMBER,pos).map(|p|p.scale(scale+0.02).life(1.15)));}
        if age>0.3 && !puff.smoke {puff.smoke=true;fx::spawn(&mut commands,Particle::sprite(lib,BLAST_SMOKE,pos).map(|p|p.scale(scale+0.09).life(2.5).multiply()));}
        puff.age+=dt;if puff.age>1.0 {commands.entity(entity).despawn();}
    }
}

/// `MESTER_TEST_BLAST=<distance>`: capture aid, sets the blast off that far in front of the camera (on the floor there) two seconds into the level.
pub fn blast_probe(mut commands:Commands,time:Res<Time>,camera:Single<&Transform,With<InspectionCamera>>,walking:Res<Walking>,blast:Res<BlastAssets>,assets:Res<AssetServer>,mut done:Local<bool>) {
    let Some(distance)=std::env::var("MESTER_TEST_BLAST").ok().and_then(|v|v.parse::<f32>().ok()) else {return};
    if *done || time.elapsed_secs()<2.0 {return;}
    *done=true;let n=|v:Vec3|retail_movement::Vec3::new(v.x,v.y,v.z);
    let eye=camera.translation/SCALE;let forward=camera.rotation*Vec3::NEG_Z;let flat=Vec3::new(forward.x,0.0,forward.z).normalize_or_zero();
    let spot=eye+flat*distance;let floor=walking.world.raycast(n(spot),n(Vec3::NEG_Y),400.0).map_or(spot.y-eye.y.min(40.0),|(d,_)|spot.y-d);
    spawn_blast(&mut commands,&blast,&assets,Vec3::new(spot.x,floor+16.0,spot.z));info!("Próba robbanás: {:?}",spot);
}

/// The audited weapon table: (item id, sila_strzalu, sila_wroga, glosnosc, rozrzut, shot_latency, max_ammo, ammo_amount, ammo_index, kul_na_raz,
/// experience_index) exactly as scripts/items.txt of the retail install spells them (docs/retail-weapons-audit.md).
#[cfg(test)] const RETAIL_TABLE:[(&str,f32,f32,f32,f32,f32,u32,u32,i32,u32,i32);11]=[
    ("Police nightstick",40.0,0.0,0.0,0.0,0.3,0,0,-1,1,0),
    ("Glock",30.0,15.0,1280.0,64.0,0.3,17,17,0,1,1),
    ("Smith and Wesson m. 625",60.0,30.0,1640.0,96.0,0.6,6,6,1,1,2),
    ("Sig 551-p/SWAT",25.0,10.0,1280.0,128.0,0.05,30,30,2,1,3),
    ("MAC-10 Ingram",20.0,12.0,1640.0,180.0,0.03,50,50,3,1,4),
    ("FN shotgun",30.0,8.0,1640.0,192.0,1.0,8,8,5,10,5),
    ("M-14",120.0,60.0,1640.0,0.0,0.6,20,20,7,1,6),
    ("HK G8",25.0,20.0,1640.0,128.0,0.06,50,50,6,1,7),
    ("P90",20.0,20.0,640.0,64.0,0.04,50,50,4,1,8),
    ("Hand grenade",0.0,0.0,0.0,0.0,0.6,1,1,8,1,0),
    ("heli_bron",10.0,10.0,1280.0,160.0,0.06,50,250,6,2,7),
];
#[cfg(test)] mod tests {
    use super::*;
    /// The type 8 controller lives 1.0 s and tests its stages before ageing: flash on the second update, fireball after 0.1 s, second fireball after 0.6 s,
    /// the three smoke puffs at once after 0.9 s, and the `age > 1.1` puff never happens (0x10053ab0, 0x10054440, Spawn 0x100514d7).
    #[test] fn the_blast_controller_fires_its_stages_once_and_never_reaches_the_fourth_smoke() {
        for step in [1.0/60.0,0.05,0.1] {
            let mut blast=Blast {pos:Vec3::ZERO,age:0.0,flash:false,fire:0,smoke:0};let mut log=Vec::new();
            while blast.age<=1.0 {let age=blast.age;for event in blast.events() {log.push((age,event));}blast.age+=step;}
            let kinds:Vec<_>=log.iter().map(|(_,e)|*e).collect();
            assert_eq!(kinds.iter().filter(|e|**e==BlastEvent::Flash).count(),1,"step {step}");
            assert_eq!(kinds.iter().filter(|e|**e==BlastEvent::Fire).count(),1);assert_eq!(kinds.iter().filter(|e|**e==BlastEvent::SecondFire).count(),1);
            assert_eq!(kinds.iter().filter(|e|matches!(e,BlastEvent::Smoke(_))).count(),3,"step {step}: {kinds:?}");
            assert!(log.iter().find(|(_,e)|*e==BlastEvent::Flash).unwrap().0>0.0,"nothing happens on the first update (age 0)");
            assert!(log.iter().find(|(_,e)|*e==BlastEvent::Fire).unwrap().0>0.1);assert!(log.iter().find(|(_,e)|*e==BlastEvent::Smoke(0)).unwrap().0>0.9);
        }
    }
    fn key(commands:&[serde_json::Value],name:&str)->Option<f32> {commands.iter().rev().find(|pair|pair[0]==name).and_then(|pair|pair[1].as_str()?.split_whitespace().next()?.parse().ok())}
    #[test] fn exported_weapon_table_matches_the_audited_retail_values() {
        let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
        let raw:serde_json::Value=serde_json::from_str(&std::fs::read_to_string(root.join("retail_weapons.json")).unwrap()).unwrap();
        let weapons=raw["weapons"].as_array().unwrap();
        assert_eq!(weapons.len(),RETAIL_TABLE.len());
        for (id,damage,enemy,loudness,spread,latency,max_ammo,amount,ammo_index,pellets,skill) in RETAIL_TABLE {
            let w=weapons.iter().find(|w|w["id"]==id).unwrap_or_else(||panic!("{id} missing"));
            let commands=w["commands"].as_array().unwrap();
            assert_eq!((w["damage"].as_f64().unwrap() as f32,w["spread"].as_f64().unwrap() as f32,w["shot_latency"].as_f64().unwrap() as f32),(damage,spread,latency),"{id}");
            assert_eq!((w["capacity"].as_u64().unwrap() as u32,w["ammo_amount"].as_u64().unwrap() as u32,w["ammo_index"].as_i64().unwrap() as i32,w["pellets"].as_u64().unwrap() as u32,w["experience_index"].as_i64().unwrap() as i32),(max_ammo,amount,ammo_index,pellets,skill),"{id}");
            assert_eq!(key(commands,"sila_wroga").unwrap_or(0.0),enemy,"{id} sila_wroga");
            assert_eq!(key(commands,"glosnosc").unwrap_or(0.0),loudness,"{id} glosnosc");
        }
    }
    /// Reads the retail item file itself (needs the local install next to the remake): the table above and the export must both match it.
    #[test] #[ignore="needs the retail install (../GYARI)"] fn audited_table_matches_gyari_items_txt() {
        let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../GYARI/scripts/items.txt");
        let text=String::from_utf8_lossy(&std::fs::read(path).unwrap()).into_owned();
        for (id,damage,enemy,loudness,spread,latency,max_ammo,amount,ammo_index,pellets,skill) in RETAIL_TABLE {
            let section=text.split("
item ").skip(1).find(|s|s.lines().next().is_some_and(|name|name.trim()==id)).unwrap_or_else(||panic!("{id} not in items.txt"));
            let body=section.split("
item ").next().unwrap();
            let value=|name:&str,default:f32|body.lines().filter_map(|l|{let l=l.trim();let rest=l.strip_prefix(name)?;rest.starts_with(' ').then(||rest.trim().split_whitespace().next().and_then(|v|v.parse::<f32>().ok()))?}).last().unwrap_or(default);
            assert_eq!((value("sila_strzalu",0.0),value("sila_wroga",0.0),value("glosnosc",0.0),value("rozrzut",0.0),value("shot_latency",0.0)),(damage,enemy,loudness,spread,latency),"{id}");
            assert_eq!((value("max_ammo",0.0) as u32,value("ammo_amount",0.0) as u32,value("ammo_index",-1.0) as i32,value("kul_na_raz",1.0) as u32,value("experience_index",0.0) as i32),(max_ammo,amount,ammo_index,pellets,skill),"{id}");
        }
    }
    /// Damage a player takes from one enemy bullet at the three difficulties: trunc(sila_wroga * D * 1.3) (0x10006c26).
    #[test] fn enemy_bullet_damage_per_difficulty_follows_the_retail_formula() {
        let hit=|strength:f32,factor:f32|(strength*factor*1.3).trunc();
        assert_eq!([0.33,0.67,1.0].map(|d|hit(15.0,d)),[6.0,13.0,19.0],"Glock");
        assert_eq!([0.33,0.67,1.0].map(|d|hit(10.0,d)),[4.0,8.0,13.0],"SIG (docs/gameplay-export.md)");
        assert_eq!([0.33,0.67,1.0].map(|d|hit(60.0,d)),[25.0,52.0,78.0],"M-14 sniper");
        assert_eq!([0.33,0.67,1.0].map(|d|hit(8.0,d)*10.0),[30.0,60.0,100.0],"an NPC shotgun blast with every pellet on target");
    }
    /// The trace length and the player's spread (0x10005dde..0x10005fb7): range 640+24*max(256-rozrzut,0), spread scaled by (1-0.0075*skill).
    #[test] fn trace_range_and_skill_spread_follow_the_retail_formula() {
        let range=|spread:f32|640.0+24.0*(256.0-spread).max(0.0);
        assert_eq!([range(64.0),range(0.0),range(180.0),range(300.0)],[5248.0,6784.0,2464.0,640.0]);
        let stats=crate::character::Stats {skills:[0.0,0.0,50.0,0.0,0.0,0.0,0.0,99.8],..Default::default()};
        assert_eq!(stats.spread(128.0,3),128.0*(1.0-0.0075*50.0));
        assert!((stats.spread(64.0,8)-64.0*(1.0-0.0075*99.8)).abs()<1e-4);
        assert_eq!(stats.spread(192.0,1),192.0);
    }
    #[test] fn original_glock_muzzle_is_right_of_camera_and_ahead_of_ejection_socket() {
        let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
        let catalog:Catalog=serde_json::from_str(&std::fs::read_to_string(root.join("retail_weapons.json")).unwrap()).unwrap();
        let d=catalog.weapons.iter().find(|d|d.id=="Glock").unwrap();let model=Model::load(&root,&d.model);let pose=model.pose(&d.animations.base,0.0,false);
        let world=view_transform(d);let muzzle=world.transform_point(model.socket(&pose,"blik_lufa").transform_point3(Vec3::ZERO));
        let ejector=world.transform_point(model.socket(&pose,"luska").transform_point3(Vec3::ZERO));
        assert!(muzzle.x>0.0 && muzzle.z<ejector.z && ejector.z<0.0,"Mirrored or reversed native gun: muzzle={muzzle:?}, ejector={ejector:?}");
    }
}
