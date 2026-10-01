//! Retail alternate fire, the laser sight, the sniper scope overlay and the nightstick.
//! Evidence and addresses (cshell.dll): docs/retail-weapons-alt.md. Native units throughout, render = native*SCALE.
use bevy::{asset::RenderAssetUsages,camera::visibility::NoFrustumCulling,mesh::{Indices,PrimitiveTopology},prelude::*,render::render_resource::{Extent3d,TextureDimension,TextureFormat},window::{CursorGrabMode,CursorOptions}};
use retail_movement::CollisionWorld;
use serde::Deserialize;
use crate::{ViewerConfig,Walking,InspectionCamera,SCALE,fx::{self,Particle,SpriteAsset},gunfire::Effects,retail_weapons::NativeArsenal,settings::Session,view::ViewState,npcs::NpcRoster,doors::Door,campaign::Campaign};

/// 0x1000ed00: the laser trace is 6400 units long; the ribbon is 0.2 wide (0x1000e670), the dot sprite scale 0.1, the dust 0.02.
const LASER_RANGE:f32=6400.0;
const BEAM_HALF_WIDTH:f32=0.1;
const DOT_SCALE:f32=0.1;
const DUST_SCALE:f32=0.02;
/// 0x10045cf0 called from 0x1000ee32: the dot is a visual stimulus of this radius for the AI.
pub const STIMULUS_RADIUS:f32=640.0;
/// 0x1000f4c2..0x1000f6e8: the nightstick probes these distances along the camera forward from the eye.
pub const MELEE_REACH:[f32;5]=[24.0,32.0,48.0,56.0,64.0];
/// 0x1000b1a0: the flashlight traces 1600 units, its light sits 16 units short of the hit with radius 24+0.175·d and fades to
/// nothing at 1600; the colour is (0.95,0.98,1.0) (0x10003860).
const FLASH_RANGE:f32=1600.0;
const FLASH_COLOR:[f32;3]=[0.95,0.98,1.0];
/// 0x10036340: the reticle quarter shows u 0.3..0.995 and v 0.45..0.995 of the 512x512 texture.
const SCOPE_U:[f32;2]=[0.3,0.995];
const SCOPE_V:[f32;2]=[0.45,0.995];

// ---- alternate-fire state machine (0x10060ce4 -> 0x100037a0) ----
/// `alt` is the weapon's flag (weapon object +0x660), `zoom` the camera flag (shell +0x20), `down` the edge latch (+0x100).
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub struct AltFlags {pub alt:bool,pub zoom:bool,down:bool}
impl AltFlags {
    /// The action is edge-triggered on the combined middle mouse / Alt state and has no weapon-state guard; only a missing weapon
    /// (0x10009e70) stops the toggle, and the latch is set either way. `weapon` is the current weapon's `alt_zoom`.
    pub fn action(&mut self,pressed:bool,weapon:Option<bool>)->bool {
        if !pressed {self.down=false;return false;}
        if self.down {return false;}
        self.down=true;
        let Some(alt_zoom)=weapon else {return false};
        self.toggle(alt_zoom);true
    }
    /// 0x100037a0: flips the flag; an `alt_zoom` weapon copies it to the camera zoom flag.
    pub fn toggle(&mut self,alt_zoom:bool) {self.alt=!self.alt;if alt_zoom {self.zoom=self.alt;}}
    /// 0x100034f0 (select) and 0x10003540 (holster): leaving a weapon toggles once if its flag is set. A stale camera flag
    /// (after a reload) is therefore not cleared here.
    pub fn leave(&mut self,alt_zoom:bool) {if self.alt {self.toggle(alt_zoom);}}
    /// 0x10003e0f, 0x10010274: a reload clears only the weapon flag of an `alt_zoom` weapon; the camera stays zoomed.
    pub fn reload(&mut self,alt_zoom:bool) {if alt_zoom {self.alt=false;}}
    /// 0x10054cee: the mouse slows to a quarter by the weapon's flag, not the camera's.
    pub fn slow_mouse(&self,alt_zoom:bool)->bool {self.alt && alt_zoom}
    /// The crosshair is hidden by the weapon flag of an `alt_zoom` weapon; the overlay follows the camera flag.
    pub fn crosshair_hidden(&self,alt_zoom:bool)->bool {self.alt && alt_zoom}
}

/// The item keys that matter here (retail_weapons.json `commands`).
#[derive(Clone,Debug,Default,PartialEq)]
pub struct AltWeapon {pub alt_zoom:bool,pub laser:Option<String>,pub alt_laser:Option<String>,pub flashlight:Option<String>}
impl AltWeapon {
    pub fn from_commands(commands:&[(String,String)])->Self {
        let value=|key:&str|commands.iter().rev().find(|(k,_)|k==key).map(|(_,v)|v.clone());
        Self {alt_zoom:commands.iter().any(|(k,_)|k=="alt_zoom"),laser:value("socket_laser"),alt_laser:value("socket_alt_laser"),flashlight:value("socket_alt_latarka")}
    }
    /// 0x1000e8a1: the alternate flag uses `socket_alt_laser`, otherwise `socket_laser`.
    pub fn laser_socket(&self,alt:bool)->Option<&str> {(if alt {&self.alt_laser}else{&self.laser}).as_deref()}
}

// ---- laser geometry (0x1000e870, 0x1000e670) ----
/// L = length - 8; 4 dust sprites if L > 128, 3 if > 96, 2 if > 64, 1 if > 8.
pub fn dust_count(length:f32)->usize {let l=length-8.0;if l>128.0 {4}else if l>96.0 {3}else if l>64.0 {2}else if l>8.0 {1}else{0}}
/// Distance along the beam of one dust sprite: 8 + roll·min(L,128), roll in 0..1, re-rolled every frame.
pub fn dust_distance(length:f32,roll:f32)->f32 {8.0+roll*(length-8.0).min(128.0)}
/// The untextured ribbon: `start ± 0.1·R`, `end ± 0.1·R` with R the camera right.
pub fn beam_corners(start:Vec3,end:Vec3,right:Vec3)->[Vec3;4] {let r=right*BEAM_HALF_WIDTH;[start-r,start+r,end+r,end-r]}
/// Strictly inside the box (0x1000fa60).
pub fn inside_box(point:Vec3,center:Vec3,half:Vec3)->bool {let d=(point-center).abs();d.x<half.x && d.y<half.y && d.z<half.z}
/// 0x1000b5e8..0x1000b75c: (light distance along the beam, radius, brightness) for a hit `distance` units from the socket.
pub fn flash_light(distance:f32)->(f32,f32,f32) {(if distance>16.0 {distance-16.0}else{distance},24.0+0.175*distance,(1.0-distance/FLASH_RANGE).max(0.0))}
/// 0x1000f4c2: the five probe points of a nightstick swing.
pub fn melee_points(eye:Vec3,forward:Vec3)->[Vec3;5] {MELEE_REACH.map(|reach|eye+forward*reach)}

// ---- scope overlay (0x10036340) ----
/// One screen quadrant of the overlay: its top-left corner as a fraction of the screen and the mirroring of the reticle quarter.
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct ScopeQuad {pub left:f32,pub top:f32,pub flip_x:bool,pub flip_y:bool}
/// Top-left, top-right, bottom-left, bottom-right quarters of the screen.
pub fn scope_quads()->[ScopeQuad;4] {
    [ScopeQuad {left:0.0,top:0.0,flip_x:false,flip_y:false},ScopeQuad {left:0.5,top:0.0,flip_x:true,flip_y:false},
     ScopeQuad {left:0.0,top:0.5,flip_x:false,flip_y:true},ScopeQuad {left:0.5,top:0.5,flip_x:true,flip_y:true}]
}
/// Texture coordinate at fraction (fx,fy) across a quadrant from its own top-left corner (what `ImageNode.rect` and the flips draw).
#[cfg(test)]
pub fn scope_uv(quad:&ScopeQuad,fx:f32,fy:f32)->Vec2 {
    let (fx,fy)=(if quad.flip_x {1.0-fx}else{fx},if quad.flip_y {1.0-fy}else{fy});
    Vec2::new(SCOPE_U[0]+(SCOPE_U[1]-SCOPE_U[0])*fx,SCOPE_V[0]+(SCOPE_V[1]-SCOPE_V[0])*fy)
}
/// The 4:3 area of the screen (x,y,w,h as fractions) the reticle covers. Retail stretches the quadrants over any resolution; a
/// pillarboxed 4:3 area keeps the circle round on wider windows.
pub fn scope_area(aspect:f32)->[f32;4] {
    if aspect>=4.0/3.0 {let w=4.0/3.0/aspect;[(1.0-w)*0.5,0.0,w,1.0]}else{let h=aspect*0.75;[0.0,(1.0-h)*0.5,1.0,h]}
}
/// Black bars around the area: left, right, top, bottom (x,y,w,h).
pub fn scope_bars([x,y,w,h]:[f32;4])->[[f32;4];4] {[[0.0,0.0,x,1.0],[x+w,0.0,1.0-x-w,1.0],[x,0.0,w,y],[x,y+h,w,1.0-y-h]]}
/// The reticle quarter in texels of a `width`x`height` texture.
pub fn scope_rect(width:f32,height:f32)->Rect {Rect::new(SCOPE_U[0]*width,SCOPE_V[0]*height,SCOPE_U[1]*width,SCOPE_V[1]*height)}

// ---- resources and components ----
#[derive(Deserialize)] struct RawCatalog {weapons:Vec<RawWeapon>}
#[derive(Deserialize)] struct RawWeapon {#[serde(default)] commands:Vec<(String,String)>}
#[derive(Deserialize)] struct RawSprite {sprite:String,frames:Vec<String>,width:f32,height:f32,#[serde(default)] fps:f32}
#[derive(Deserialize)] struct RawScope {texture:String,width:f32,height:f32}
#[derive(Deserialize)] struct RawMelee {wall_sound:Option<String>}
#[derive(Deserialize)] struct RawAltFire {laser_dot:RawSprite,scope:RawScope,melee:RawMelee}

#[derive(Resource)] pub struct AltFire {
    pub weapons:Vec<AltWeapon>,pub flags:AltFlags,
    last:Option<usize>,arrived:u64,swing:Option<f32>,
    beam:Handle<Mesh>,cone:Handle<Mesh>,pool:Handle<StandardMaterial>,wall_sound:Option<String>,
    /// Where the laser dot is this frame (native units): the visual stimulus for the AI.
    pub dot:Option<Vec3>,
    /// Counters for the headless probe.
    pub swings:u32,pub melee_npc_hits:u32,pub wall_hits:u32,pub laser_frames:u32,pub laser_length:f32,pub dust:usize,pub socket:Vec3,pub flash_frames:u32,
}
impl AltFire {
    /// The nightstick swing started (0x10003e60); the hit test follows when `do_uderza` ends.
    pub fn begin_swing(&mut self) {self.swing=Some(0.0);self.swings+=1;}
    /// The current weapon (selected and equipped), if any.
    fn current(&self,native:&NativeArsenal)->Option<usize> {native.inventory.selected.filter(|_|native.inventory.equipped && !native.inventory.switching())}
}
#[derive(Component)] pub struct LaserBeam;
#[derive(Component)] pub struct LaserSprite(usize);
#[derive(Component)] pub struct FlashPart(usize);
#[derive(Component)] pub struct ScopeOverlay;
#[derive(Component)] pub struct ScopeBox;
#[derive(Component)] pub struct ScopeBar(usize);

pub fn setup(mut commands:Commands,config:Res<ViewerConfig>,assets:Res<AssetServer>,mut effects:ResMut<Effects>,mut meshes:ResMut<Assets<Mesh>>,mut images:ResMut<Assets<Image>>,mut materials:ResMut<Assets<StandardMaterial>>) {
    let read=|name:&str|std::fs::read_to_string(config.output.join(name)).unwrap_or_else(|_|panic!("{name} missing; run tools.export_weapons / tools.export_altfire"));
    let catalog:RawCatalog=serde_json::from_str(&read("retail_weapons.json")).expect("original weapon export format");
    let weapons:Vec<AltWeapon>=catalog.weapons.iter().map(|w|AltWeapon::from_commands(&w.commands)).collect();
    let altfire:RawAltFire=serde_json::from_str(&read("retail_altfire.json")).expect("retail altfire export format");
    let quad=effects.lib.quad.clone();
    let mut beam=Mesh::new(PrimitiveTopology::TriangleList,RenderAssetUsages::default());
    beam.insert_attribute(Mesh::ATTRIBUTE_POSITION,vec![[0.0f32;3];4]);
    beam.insert_attribute(Mesh::ATTRIBUTE_NORMAL,vec![[0.0f32,0.0,1.0];4]);
    beam.insert_attribute(Mesh::ATTRIBUTE_UV_0,vec![[0.0f32;2];4]);
    beam.insert_indices(Indices::U32(vec![0,1,2,0,2,3]));
    let beam=meshes.add(beam);
    // (85,0,0) additive, depth-tested, no jitter (0x1000e670). Retail adds in gamma space; the same number as a linear add gives
    // a comparable brightness on the mid-dark walls (added as sRGB it would hardly show).
    let red=materials.add(StandardMaterial {base_color:Color::linear_rgb(85.0/255.0,0.0,0.0),unlit:true,cull_mode:None,alpha_mode:AlphaMode::Add,fog_enabled:false,..default()});
    commands.spawn((LaserBeam,Mesh3d(beam.clone()),MeshMaterial3d(red),Transform::IDENTITY,Visibility::Hidden,NoFrustumCulling));
    // The dot and the four dust sprites are persistent fx particles (additive billboards) that the laser system moves every frame;
    // they are not WorldGeometry, so they survive level changes.
    let dot=&altfire.laser_dot;
    effects.lib.sprites.insert(fx::key(&dot.sprite),SpriteAsset {frames:dot.frames.iter().map(|f|assets.load(f.clone())).collect::<Vec<_>>().into(),fps:dot.fps.max(1.0),size:Vec2::new(dot.width,dot.height)});
    for i in 0..5 {
        if let Some(mut particle)=Particle::sprite(&effects.lib,&dot.sprite,Vec3::ZERO) {particle.life=f32::MAX;particle.alpha=0.0;commands.spawn((LaserSprite(i),particle,Transform::default(),Visibility::Hidden));}
    }
    // Flashlight: a soft additive cone from the socket, a light pool on the hit surface and a light for the lit models. The static
    // level is unlit vertex colour, so the pool sprite carries the illumination there.
    let mut cone=Mesh::new(PrimitiveTopology::TriangleList,RenderAssetUsages::default());
    cone.insert_attribute(Mesh::ATTRIBUTE_POSITION,vec![[0.0f32;3];6]);
    cone.insert_attribute(Mesh::ATTRIBUTE_NORMAL,vec![[0.0f32,0.0,1.0];6]);
    cone.insert_attribute(Mesh::ATTRIBUTE_UV_0,vec![[0.0f32;2];6]);
    cone.insert_attribute(Mesh::ATTRIBUTE_COLOR,vec![[FLASH_COLOR[0],FLASH_COLOR[1],FLASH_COLOR[2],0.0];6]);
    // Left edge, axis and right edge at the lamp (0..3) and at the lit spot (3..6): transparent edges, brighter axis.
    cone.insert_indices(Indices::U32(vec![0,1,4,0,4,3,1,2,5,1,5,4]));
    let cone=meshes.add(cone);
    let gradient:Vec<u8>=(0..64*64).flat_map(|i|{let (x,y)=((i%64) as f32-31.5,(i/64) as f32-31.5);let fall=(1.0-(x*x+y*y).sqrt()/32.0).clamp(0.0,1.0);
        [(FLASH_COLOR[0]*255.0) as u8,(FLASH_COLOR[1]*255.0) as u8,(FLASH_COLOR[2]*255.0) as u8,(fall*fall*255.0) as u8]}).collect();
    let pool_image=images.add(Image::new(Extent3d {width:64,height:64,depth_or_array_layers:1},TextureDimension::D2,gradient,TextureFormat::Rgba8UnormSrgb,RenderAssetUsages::default()));
    let pool=materials.add(StandardMaterial {base_color_texture:Some(pool_image),unlit:true,cull_mode:None,alpha_mode:AlphaMode::Add,fog_enabled:false,..default()});
    let shaft=materials.add(StandardMaterial {unlit:true,cull_mode:None,alpha_mode:AlphaMode::Add,fog_enabled:false,..default()});
    commands.spawn((FlashPart(0),Mesh3d(cone.clone()),MeshMaterial3d(shaft),Transform::IDENTITY,Visibility::Hidden,NoFrustumCulling));
    commands.spawn((FlashPart(1),Mesh3d(quad.clone()),MeshMaterial3d(pool.clone()),Transform::IDENTITY,Visibility::Hidden,NoFrustumCulling));
    commands.spawn((FlashPart(2),PointLight {color:Color::srgb(FLASH_COLOR[0],FLASH_COLOR[1],FLASH_COLOR[2]),intensity:0.0,range:1.0,shadows_enabled:false,..default()},Transform::IDENTITY,Visibility::Hidden));
    // Four mirrored quadrants of the reticle quarter, under the rest of the HUD (0x10036340 is drawn first).
    let texture=assets.load(altfire.scope.texture.clone());let rect=scope_rect(altfire.scope.width,altfire.scope.height);
    commands.spawn((ScopeOverlay,Visibility::Hidden,GlobalZIndex(-1),Node {position_type:PositionType::Absolute,width:percent(100),height:percent(100),..default()})).with_children(|root| {
        for i in 0..4 {root.spawn((ScopeBar(i),BackgroundColor(Color::BLACK),Node {position_type:PositionType::Absolute,..default()}));}
        root.spawn((ScopeBox,Node {position_type:PositionType::Absolute,..default()})).with_children(|area| {
            for quad in scope_quads() {
                area.spawn((ImageNode {image:texture.clone(),rect:Some(rect),flip_x:quad.flip_x,flip_y:quad.flip_y,..default()},
                    Node {position_type:PositionType::Absolute,left:percent(quad.left*100.0),top:percent(quad.top*100.0),width:percent(50),height:percent(50),..default()}));
            }
        });
    });
    commands.insert_resource(AltFire {weapons,flags:AltFlags::default(),last:None,arrived:0,swing:None,beam,cone,pool,wall_sound:altfire.melee.wall_sound,
        dot:None,swings:0,melee_npc_hits:0,wall_hits:0,laser_frames:0,laser_length:0.0,dust:0,socket:Vec3::ZERO,flash_frames:0});
}

/// Alternate-fire input, weapon change and reload rules, and the camera flags.
#[allow(clippy::too_many_arguments)]
pub fn tick(mut alt:ResMut<AltFire>,mut view:ResMut<ViewState>,native:Res<NativeArsenal>,session:Res<Session>,bind:crate::options::Bindings,cursor:Single<&CursorOptions>,opening:Res<crate::opening::Opening>,travel:Res<crate::travel::Travel>,probe:Res<Probe>) {
    if travel.arrived!=alt.arrived {alt.arrived=travel.arrived;alt.flags=AltFlags::default();alt.last=None;alt.swing=None;alt.dot=None;}
    if opening.active || session.paused || session.dialogue_active {return;}
    let current=alt.current(&native);
    if current!=alt.last {
        if let Some(old)=alt.last {let alt_zoom=alt.weapons[old].alt_zoom;alt.flags.leave(alt_zoom);}
        alt.last=current;
    }
    let alt_zoom=current.map(|c|alt.weapons[c].alt_zoom);
    if native.reloading() {if let Some(zoom)=alt_zoom {alt.flags.reload(zoom);}}
    let live=cursor.grab_mode!=CursorGrabMode::None || probe.active;
    // keys.cfg command 7 (the owner's table: right and middle mouse buttons; retail: middle button and Alt).
    let pressed=live && bind.pressed(crate::keys_cfg::cmd::ALT_FIRE);
    alt.flags.action(pressed,alt_zoom);
    view.scoped=alt.flags.zoom;
    view.mouse_slow=alt_zoom.is_some_and(|zoom|alt.flags.slow_mouse(zoom));
}

/// Hides the crosshair while the weapon's alt flag zooms and shows the scope overlay while the camera is zoomed.
#[allow(clippy::too_many_arguments)]
pub fn overlay(alt:Res<AltFire>,native:Res<NativeArsenal>,opening:Res<crate::opening::Opening>,window:Single<&Window>,mut scope:Single<&mut Visibility,With<ScopeOverlay>>,mut crosshair:Single<&mut Visibility,(With<crate::hud::Crosshair>,Without<ScopeOverlay>)>,
    mut area:Single<&mut Node,(With<ScopeBox>,Without<ScopeBar>)>,mut bars:Query<(&ScopeBar,&mut Node),Without<ScopeBox>>) {
    **scope=if alt.flags.zoom && !opening.active {Visibility::Visible}else{Visibility::Hidden};
    if alt.current(&native).is_some_and(|c|alt.flags.crosshair_hidden(alt.weapons[c].alt_zoom)) {**crosshair=Visibility::Hidden;}
    let rect=scope_area(window.width()/window.height().max(1.0));
    let place=|node:&mut Node,[x,y,w,h]:[f32;4]| {node.left=percent(x*100.0);node.top=percent(y*100.0);node.width=percent(w*100.0);node.height=percent(h*100.0);};
    place(&mut area,rect);
    for (bar,mut node) in &mut bars {place(&mut node,scope_bars(rect)[bar.0]);}
}

/// Retail draws the laser and the flashlight in weapon states 0, 3 and 4 only: not while reloading or switching, and not in
/// menus, the opening or after death.
fn gun_ready(native:&NativeArsenal,campaign:&Campaign,opening:&crate::opening::Opening,session:&Session)->bool {
    !(opening.active || session.paused || campaign.health<=0.0 || native.reloading() || native.inventory.switching())
}
/// The view model's socket `name` in world native units: `NativeArsenal::socket_view` gives it in the view space of the weapon
/// camera (Bevy axes), the main camera carries it to the world like the render pass does.
fn socket_position(native:&NativeArsenal,view:&ViewState,camera:&Transform,slot:usize,name:&str)->Option<Vec3> {
    Some(crate::mirror::to_world(camera,native.socket_view(slot,name,view)?)/SCALE)
}
/// First hit of the world, door brushes and living actors along a ray, with the surface normal for world hits.
fn trace_hit(world:&CollisionWorld,doors:&Query<&Door>,npcs:&NpcRoster,origin:Vec3,direction:Vec3,max:f32)->Option<(f32,Option<Vec3>)> {
    let n=|v:Vec3|retail_movement::Vec3::new(v.x,v.y,v.z);
    let mut hit=world.raycast(n(origin),n(direction),max).map(|(d,normal)|(d,Some(Vec3::new(normal.x,normal.y,normal.z))));
    let limit=hit.map_or(max,|h|h.0);
    let other=doors.iter().filter_map(|door|door.shot_hit(origin,direction,limit).map(|h|h.0)).chain(npcs.ray_distance(origin,direction,limit)).min_by(f32::total_cmp);
    if let Some(d)=other {hit=Some((d,None));}
    hit
}
/// First hit of the world, door brushes and living actors along a ray; `max` when nothing is in the way.
fn trace(world:&CollisionWorld,doors:&Query<&Door>,npcs:&NpcRoster,origin:Vec3,direction:Vec3,max:f32)->f32 {
    let n=|v:Vec3|retail_movement::Vec3::new(v.x,v.y,v.z);
    let mut hit=world.raycast(n(origin),n(direction),max).map_or(max,|(d,_)|d);
    for door in doors {if let Some((d,_))=door.shot_hit(origin,direction,hit) {hit=hit.min(d);}}
    npcs.ray_distance(origin,direction,hit).map_or(hit,|d|hit.min(d))
}

/// The laser sight: ribbon from the view model's socket, dot and dust sprites, and the AI stimulus.
#[allow(clippy::too_many_arguments)]
pub fn laser(mut alt:ResMut<AltFire>,native:Res<NativeArsenal>,view:Res<ViewState>,walking:Res<Walking>,mut npcs:ResMut<NpcRoster>,doors:Query<&Door>,campaign:Res<Campaign>,opening:Res<crate::opening::Opening>,session:Res<Session>,
    camera:Single<&Transform,(With<InspectionCamera>,Without<LaserBeam>)>,mut meshes:ResMut<Assets<Mesh>>,mut beam:Single<(&mut Visibility,&mut Transform),(With<LaserBeam>,Without<InspectionCamera>)>,mut sprites:Query<(&LaserSprite,&mut Particle)>) {
    npcs.visual_stimuli.clear();
    let socket=(|| {
        if !gun_ready(&native,&campaign,&opening,&session) {return None;}
        let slot=alt.current(&native)?;let name=alt.weapons[slot].laser_socket(alt.flags.alt)?;
        Some((slot,socket_position(&native,&view,&camera,slot,name)?))
    })();
    let Some((slot,start))=socket else {alt.dot=None;*beam.0=Visibility::Hidden;for (_,mut p) in &mut sprites {p.alpha=0.0;}return};
    let eye=camera.translation/SCALE;let forward=*camera.forward();
    // Scoped (alt flag of an alt_zoom weapon), the eye ray decides where the beam ends (0x1000ec30).
    let aim=if alt.flags.alt && alt.weapons[slot].alt_zoom {eye+forward*trace(&walking.world,&doors,&npcs,eye,forward,LASER_RANGE)}else{start+forward*LASER_RANGE};
    let toward=(aim-start).try_normalize().unwrap_or(forward);
    let length=trace(&walking.world,&doors,&npcs,start,toward,(aim-start).length().min(LASER_RANGE));
    let end=start+toward*length;
    // The ribbon lives around its start: transparent surfaces are sorted by their origin, and the one at the world origin was
    // drawn before (and hidden by) the bullet marks it flies in front of.
    let corners=beam_corners(start,end,*camera.right()).map(|c|((c-start)*SCALE).to_array());
    if let Some(mesh)=meshes.get_mut(&alt.beam) {mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION,corners.to_vec());}
    beam.1.translation=start*SCALE;*beam.0=Visibility::Visible;
    let count=dust_count(length);
    let rolls:[f32;4]=std::array::from_fn(|_|fx::rnd());
    for (sprite,mut particle) in &mut sprites {
        let (position,scale)=match sprite.0 {
            // The dot sits at the hit, pulled back a unit so it is not clipped by the wall it lands on.
            0=>(end-toward*length.min(1.0),DOT_SCALE),
            i=>(start+toward*dust_distance(length,rolls[i-1]),DUST_SCALE),
        };
        particle.pos=position;particle.scale=Vec2::splat(scale);
        particle.alpha=if sprite.0==0 || sprite.0<=count {1.0}else{0.0};
    }
    alt.socket=start-eye;alt.dot=Some(end);alt.laser_frames+=1;alt.laser_length=length;alt.dust=count;
    npcs.visual_stimuli.push(end);
}

/// SIG 551 and P90 alternate fire (0x10003860, 0x1000b1a0): a light 16 units short of where the beam meets a surface.
#[allow(clippy::too_many_arguments)]
pub fn flashlight(mut alt:ResMut<AltFire>,native:Res<NativeArsenal>,view:Res<ViewState>,walking:Res<Walking>,mut npcs:ResMut<NpcRoster>,doors:Query<&Door>,campaign:Res<Campaign>,opening:Res<crate::opening::Opening>,session:Res<Session>,
    camera:Single<&Transform,(With<InspectionCamera>,Without<FlashPart>)>,mut meshes:ResMut<Assets<Mesh>>,mut materials:ResMut<Assets<StandardMaterial>>,mut parts:Query<(&FlashPart,&mut Visibility,&mut Transform,Option<&mut PointLight>)>) {
    let socket=(|| {
        if !gun_ready(&native,&campaign,&opening,&session) || !alt.flags.alt {return None;}
        let slot=alt.current(&native)?;let name=alt.weapons[slot].flashlight.as_deref()?;
        socket_position(&native,&view,&camera,slot,name)
    })();
    let Some(start)=socket else {for (_,mut v,_,_) in &mut parts {*v=Visibility::Hidden;}return};
    let forward=*camera.forward();
    let hit=trace_hit(&walking.world,&doors,&npcs,start,forward,FLASH_RANGE);
    let distance=hit.map_or(FLASH_RANGE,|h|h.0);
    let (along,radius,brightness)=flash_light(distance);
    let position=start+forward*along;
    // A soft shaft from the socket to the lit spot: narrow and brighter at the lamp, wide and faint at the wall.
    let right=*camera.right();let (near,far)=(0.5,(0.25*distance).clamp(0.5,120.0));
    let at=|t:f32,half:f32,sign:f32|((forward*t+right*half*sign)*SCALE).to_array();
    if let Some(mesh)=meshes.get_mut(&alt.cone) {
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION,vec![at(0.0,near,-1.0),at(0.0,near,0.0),at(0.0,near,1.0),at(distance,far,-1.0),at(distance,far,0.0),at(distance,far,1.0)]);
        let rgba=|a:f32|[FLASH_COLOR[0],FLASH_COLOR[1],FLASH_COLOR[2],a];
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR,vec![rgba(0.0),rgba(0.07),rgba(0.0),rgba(0.0),rgba(0.03*brightness),rgba(0.0)]);
    }
    if let Some(m)=materials.get_mut(&alt.pool) {m.base_color=Color::linear_rgba(1.0,1.0,1.0,0.7*brightness);}
    for (part,mut visible,mut transform,light) in &mut parts {
        match part.0 {
            0=>{transform.translation=start*SCALE;*visible=Visibility::Visible;}
            1=>{
                *visible=if hit.is_some() {Visibility::Visible}else{Visibility::Hidden};
                // On a wall the pool lies flat on it, otherwise it faces the camera.
                let normal=hit.and_then(|h|h.1).map(|n|if n.dot(forward)>0.0 {-n}else{n});
                let rotation=normal.map_or(camera.rotation,|n|Quat::from_rotation_arc(Vec3::Z,n));
                let lifted=normal.map_or(-forward*2.0,|n|n*0.6);
                transform.translation=(start+forward*distance+lifted)*SCALE;transform.rotation=rotation;transform.scale=Vec3::new(radius*1.6*SCALE,radius*1.6*SCALE,1.0);
            }
            _=>{
                *visible=if hit.is_some() {Visibility::Visible}else{Visibility::Hidden};
                transform.translation=position*SCALE;
                if let Some(mut light)=light {light.intensity=30000.0*brightness;light.range=radius*SCALE;}
            }
        }
    }
    if hit.is_some() {npcs.visual_stimuli.push(position);}
    alt.flash_frames+=1;
}

/// Nightstick: the hit test runs once when `do_uderza` ends (0x1000f420): five probe points from the eye, 40 damage to every
/// actor box that holds one, `palka_sciana.wav` when the 64-unit ray meets the world, blood and `wcialo.wav` on an actor.
#[allow(clippy::too_many_arguments)]
pub fn melee(mut commands:Commands,mut alt:ResMut<AltFire>,native:Res<NativeArsenal>,time:Res<Time>,session:Res<Session>,opening:Res<crate::opening::Opening>,camera:Single<&Transform,With<InspectionCamera>>,walking:Res<Walking>,mut npcs:ResMut<NpcRoster>,doors:Query<&Door>,assets:Res<AssetServer>,effects:Res<Effects>) {
    if opening.active || session.paused || session.dialogue_active {return;}
    let Some(elapsed)=alt.swing else {return};
    let Some(slot)=alt.current(&native).filter(|&s|native.definitions[s].melee) else {alt.swing=None;return};
    let d=&native.definitions[slot];
    let delay=d.animations.shoot.first().and_then(|clip|d.durations.get(clip)).copied().unwrap_or(0.1);
    let elapsed=elapsed+time.delta_secs().min(0.05);
    if elapsed<delay {alt.swing=Some(elapsed);return;}
    alt.swing=None;
    let eye=camera.translation/SCALE;let forward=*camera.forward();let points=melee_points(eye,forward);
    let n=|v:Vec3|retail_movement::Vec3::new(v.x,v.y,v.z);
    let wall=walking.world.raycast(n(eye),n(forward),MELEE_REACH[4]).map(|(t,_)|t).into_iter().chain(doors.iter().filter_map(|door|door.shot_hit(eye,forward,MELEE_REACH[4]).map(|(t,_)|t))).min_by(f32::total_cmp);
    if let Some(distance) = wall {
        alt.wall_hits+=1;
        if let Some(path)=&alt.wall_sound {crate::sound::play_at(&mut commands,&assets,path,eye+forward*distance,640.0);}
    }
    // Props: every prop whose box holds the 32-unit point, else the 64-unit point, takes the weapon's `sila_strzalu` (cshell 0x1000f829..0x1000fa25).
    {
        let mut field=effects.props.field();let mut struck=Vec::new();
        for point in [points[1],points[4]] {for index in field.strike(point) {if !struck.contains(&index) {struck.push(index);field.hit(index,d.damage,point);crate::gunfire::prop_impact(&mut commands,&effects,effects.weapon(&d.id),point,-forward,field.props[index].effects);}}}
    }
    // The nightstick's damage is the item's `sila_strzalu`, without difficulty scaling.
    let hits=npcs.melee_hits(&points,d.damage);
    if hits.is_empty() {return;}
    alt.melee_npc_hits+=hits.len() as u32;
    if let Some(Some(path))=d.sounds.get("hit") {crate::sound::play_at(&mut commands,&assets,path,points[4],640.0);}
    // 0x1000fd58: krew1.spr at the 32-unit point (1.5 s, scale 0.15·(1.2+0.2·(rand&5)), normal alpha, fading), then 15..22
    // krew_dodatki bits (0x1000fe0a) with velocity (±96, 0..96, ±96), gravity 240, 0.8..1.4 s, scale 0.15.
    fx::spawn(&mut commands,Particle::sprite(&effects.lib,"sprites/krew1.spr",points[1]).map(|p|p.scale(0.15*(1.2+0.2*(fx::pick(8) & 5) as f32)).life(1.5).normal_alpha().fade()));
    for _ in 0..15+fx::pick(8) {
        let velocity=Vec3::new(fx::between(0.0,96.0)-fx::between(0.0,96.0),fx::between(0.0,96.0),fx::between(0.0,96.0)-fx::between(0.0,96.0));
        fx::spawn(&mut commands,Particle::sprite(&effects.lib,&format!("sprites/krew_dodatki/{}.spr",1+fx::pick(11)),points[1]).map(|p|p.scale(0.15).life(fx::between(0.8,1.4)).vel(velocity).gravity(240.0).normal_alpha()));
    }
}

// ---- headless probe ----
#[derive(Resource,Default)] pub struct Probe {pub active:bool,pub finished:bool,pub failure:Option<String>,elapsed:f32,/// Game time (frame time clamped like the simulation): the arsenal scenario runs on it.
    clock:f32,mark:f32,stage:usize,mode:String,/// Free-form state of a scenario (the noise scenario keeps the guard's first phase).
    note:String}
pub fn probe_setup(mut commands:Commands,config:Res<ViewerConfig>) {
    let mode=std::env::var("MESTER_TEST_SCENARIO").unwrap_or_default();
    commands.insert_resource(Probe {active:config.capture.is_some() && matches!(mode.as_str(),"altfire"|"melee"|"flash"|"arsenal"|"noise"|"revolver"),mode,..default()});
}
/// Yaw (Bevy, as `place` uses it) of the longest clear sight line at eye height from the player.
fn longest_sight(walking:&Walking)->(f32,f32) {
    let p=walking.player.position;let eye=retail_movement::Vec3::new(p.x,p.y+46.0,p.z);
    (0..72).map(|i|{let yaw=i as f32*std::f32::consts::TAU/72.0;
        let d=walking.world.raycast(eye,retail_movement::Vec3::new(-yaw.sin(),0.0,-yaw.cos()),LASER_RANGE).map_or(LASER_RANGE,|h|h.0);(yaw,d)}).max_by(|a,b|a.1.total_cmp(&b.1)).unwrap()
}
/// The `arsenal` scenario: every weapon in turn (select, wait out the 1/3 s lowering and raising, one shot), with the weapon change
/// at t = 1.0 + 1.4·i (`MESTER_TEST_SCENARIO=arsenal ... 0.4,2.05,..`: see `ARSENAL_SHOTS`). Logs the magazine and reserve of every step.
pub const ARSENAL_START:f32=1.0;
pub const ARSENAL_STEP:f32=1.4;
fn arsenal(commands:&mut Commands,config:&ViewerConfig,probe:&mut Probe,walking:&mut Walking,native:&mut NativeArsenal,controls:&mut crate::gunfire::Controls,roster:&NpcRoster,view:&ViewState) {
    const ORDER:[&str;10]=["Police nightstick","Glock","Smith and Wesson m. 625","Sig 551-p/SWAT","MAC-10 Ingram","FN shotgun","M-14","HK G8","P90","Hand grenade"];
    let now=probe.clock;
    let shoot=|commands:&mut Commands,name:&str| {
        let Some(path)=&config.capture else {return};
        let base=std::path::PathBuf::from(path);
        let target=base.with_file_name(format!("{}-{name}.png",base.file_stem().unwrap().to_string_lossy()));
        commands.spawn(bevy::render::view::screenshot::Screenshot::primary_window()).observe(bevy::render::view::screenshot::save_to_disk(target));
    };
    if probe.stage==0 && now>=0.3 {
        probe.stage=1;
        let (yaw,distance)=longest_sight(walking);walking.yaw=yaw;walking.pitch=0.0;
        for id in ORDER {native.acquire(id);}
        for index in 0..8 {native.inventory.add_ammo(index,60);}
        info!("ARSENAL PRÓBA: irány {yaw:.2} rad, szabad látótávolság {distance:.0}");
        for (slot,d) in native.definitions.iter().enumerate().filter(|(_,d)|d.player_selectable) {info!("ARSENAL {} tár={} tartalék={}",d.id,native.inventory.weapons[slot].magazine,native.inventory.reserve(&native.definitions,slot));}
    }
    if probe.stage==0 {return;}
    let index=((now-ARSENAL_START)/ARSENAL_STEP).floor();
    if index<0.0 {return;}
    if index>=ORDER.len() as f32 {
        // Everything has been used: the run ends a little later.
        if probe.stage<100 && native.explosions>0 {probe.stage=100;probe.mark=now;info!("ARSENAL robbanás t={now:.2}");}
        if probe.stage==100 && now>=probe.mark+0.3 {probe.stage=101;shoot(commands,"explosion");}
        return;
    }
    let index=index as usize;let phase=now-ARSENAL_START-index as f32*ARSENAL_STEP;
    let short=["baton","glock","snw","sig","ingram","fn","m14","hk","p90","grenade"][index];
    // The change starts at the beginning of the step, the shot comes 0.8 s later.
    let step_stage=2+index*4;
    if probe.stage<=step_stage && phase>=0.0 {
        probe.stage=step_stage+1;
        let accepted=index==0 || native.inventory.select(index);
        // The grenade is thrown at a wall about 250 units away so its explosion is in view.
        if index==9 {
            let p=walking.player.position;let eye=retail_movement::Vec3::new(p.x,p.y+46.0,p.z);
            let best=(0..72).filter_map(|i|{let yaw=i as f32*std::f32::consts::TAU/72.0;let d=walking.world.raycast(eye,retail_movement::Vec3::new(-yaw.sin(),0.0,-yaw.cos()),600.0).map(|h|h.0)?;(d>=150.0).then_some((yaw,d))}).min_by(|a,b|(a.1-250.0).abs().total_cmp(&(b.1-250.0).abs()));
            if let Some((yaw,distance))=best {walking.yaw=yaw;walking.pitch=0.0;info!("ARSENAL gránát: fal {distance:.0} egységre, irány {yaw:.2}");}
        }
        info!("ARSENAL váltás {} elfogadva={accepted} kézben={:?}",ORDER[index],native.inventory.selected.map(|s|native.definitions[s].id.clone()));
    }
    if probe.stage==step_stage+1 && phase>=0.18 {
        probe.stage=step_stage+2;
        info!("ARSENAL {} váltás közben dip={:.2} kézben={:?}",ORDER[index],native.inventory.dip(),native.inventory.selected.map(|s|native.definitions[s].id.clone()));
        if index==1 || index==5 {shoot(commands,&format!("lower-{short}"));}
    }
    if probe.stage==step_stage+2 && phase>=0.8 {
        probe.stage=step_stage+3;
        let before=native.inventory.selected.map(|s|native.inventory.weapons[s].magazine);
        controls.fire=true;controls.held=true;
        // The screenshot request renders the frame that also spawns the muzzle flash.
        if index!=9 {shoot(commands,&format!("flash-{short}"));}
        info!("ARSENAL {} lövés előtt tár={before:?} dip={:.2} zaj={} dip_view={:.2}",ORDER[index],native.inventory.dip(),roster.noises.len(),view.weapon_dip);
    }
    if probe.stage==step_stage+3 && phase>=0.86 {
        probe.stage=step_stage+4;
        let slot=native.inventory.selected;
        info!("ARSENAL {} lövés után tár={:?} tartalék={:?} lövések={}",ORDER[index],slot.map(|s|native.inventory.weapons[s].magazine),slot.map(|s|native.inventory.reserve(&native.definitions,s)),native.shots);
        if index!=9 {shoot(commands,&format!("shot-{short}"));}
    }
    // The grenade is held for one second, then thrown.
    if index==9 && probe.stage>=step_stage+3 && phase>=0.8 && phase<1.9 {controls.held=true;}
}

/// The `revolver` scenario: the S&W is fired once and reloaded; the six casings fall when the first reload clip ends (`reload1`, 1.1 s).
fn revolver(commands:&mut Commands,config:&ViewerConfig,probe:&mut Probe,walking:&mut Walking,native:&mut NativeArsenal,controls:&mut crate::gunfire::Controls) {
    let now=probe.clock;
    let shoot=|commands:&mut Commands,name:&str| {
        let Some(path)=&config.capture else {return};
        let base=std::path::PathBuf::from(path);
        let target=base.with_file_name(format!("{}-{name}.png",base.file_stem().unwrap().to_string_lossy()));
        commands.spawn(bevy::render::view::screenshot::Screenshot::primary_window()).observe(bevy::render::view::screenshot::save_to_disk(target));
    };
    // MESTER_TEST_WEAPON=<item id> fires that weapon once instead (casing timing: captures 0.2, 0.5, 0.8 and 1.1 s after the shot).
    let other=std::env::var("MESTER_TEST_WEAPON").ok();
    if probe.stage==0 && now>=0.3 {probe.stage=1;let (yaw,_)=longest_sight(walking);walking.yaw=yaw;walking.pitch=-0.35;native.acquire_and_draw(other.as_deref().unwrap_or("Smith and Wesson m. 625"));for index in 0..8 {native.inventory.add_ammo(index,30);}}
    if let Some(weapon)=&other {
        if probe.stage==1 && now>=0.9 {probe.stage=2;controls.fire=true;controls.held=true;info!("REVOLVER PRÓBA {weapon}: lövés");}
        for (stage,delay) in [(2,0.2),(3,0.5),(4,0.8),(5,1.1)] {
            if probe.stage==stage && now>=0.9+delay {probe.stage=stage+1;shoot(commands,&format!("shot-{delay}"));}
        }
        return;
    }
    if probe.stage==1 && now>=0.9 {probe.stage=2;controls.fire=true;controls.held=true;}
    if probe.stage==2 && now>=1.6 {probe.stage=3;controls.reload=true;info!("REVOLVER PRÓBA újratöltés tár={:?}",native.inventory.selected.map(|s|native.inventory.weapons[s].magazine));}
    // The first clip lasts 1.1 s; the casings appear when it ends.
    if probe.stage==3 && now>=1.6+1.1+0.12 {probe.stage=4;shoot(commands,"casings-a");}
    if probe.stage==4 && now>=1.6+1.1+0.45 {probe.stage=5;shoot(commands,"casings-b");}
    if probe.stage==5 && now>=1.6+1.1+1.6 {probe.stage=6;info!("REVOLVER PRÓBA vége tár={:?} újratöltések={}",native.inventory.selected.map(|s|native.inventory.weapons[s].magazine),native.reloads);}
}

/// The `noise` scenario: a hostile guard (MESTER_TEST_NPC, default o_postac22) that cannot see the player, who stands 400 units behind
/// it. Retail (docs/retail-ai.md, "Noise"): a player's gunshot lives ONE frame, so contact by hearing it can never be provoked (0x10049a57 clears
/// the provocation as soon as the noise is gone); what alerts an idle guard is a two-frame stimulus: the bullet's impact or the wound it causes.
/// Default: the Glock is fired at the guard's back, the wound (192 near / 1024 heard, two frames) must wake it. `MESTER_TEST_NOISE=away`: the shot
/// flies away from it and must NOT change its phase. A run without shooting (MESTER_TEST_HOLD_FIRE=1) is the control: nothing may happen.
fn noise(probe:&mut Probe,walking:&mut Walking,native:&mut NativeArsenal,controls:&mut crate::gunfire::Controls,roster:&mut NpcRoster) {
    let now=probe.clock;
    let wanted=std::env::var("MESTER_TEST_NPC").unwrap_or_else(|_|"o_postac22".into());
    let away=std::env::var("MESTER_TEST_NOISE").is_ok_and(|v|v=="away");
    let phase=roster.actors.iter().find(|a|a.name==wanted).map(|a|a.phase.clone());
    // The levels' `hostileattack` action runs every tick: whoever notices the player (or the shot) is provoked (docs/retail-ai.md).
    if probe.stage>=1 {roster.provoke_noticing();}
    if probe.stage==0 && now>=0.3 {
        probe.stage=1;
        let Some(actor)=roster.actors.iter().find(|a|a.name==wanted) else {probe.failure=Some(format!("nincs {wanted}"));return};
        let behind=-(actor.rotation*Vec3::Z);
        // A spot behind the guard (out of its 90 degree contact cone) with a clear line to it.
        let mut found=false;
        'search: for radius in [300.0,400.0,500.0,250.0] {for step in 0..17 {
            let angle=(step as f32-8.0)*0.17;let dir=Quat::from_rotation_y(angle)*behind;
            let stand=actor.position+dir*radius;
            crate::campaign_probe::place(walking,stand,if away {stand+dir*100.0}else{actor.position+Vec3::Y*20.0});
            let p=walking.player.position;let eye=Vec3::new(p.x,p.y+46.0,p.z);
            if eye.distance(actor.position)<radius+80.0 && crate::npcs::line_of_sight(&walking.world,eye,actor.position) && !actor.player_seen(&walking.world,eye) {found=true;break 'search;}
        }}
        if !found {probe.failure=Some("nincs rálátásos hely a gárda mögött".into());}
        native.acquire_and_draw("Glock");roster.hostiles_attacking=true;probe.note=actor.phase.clone();
        info!("NOISE PRÓBA: {wanted} fázis={:?} játékos={:?}",actor.phase,walking.player.position);
    }
    if probe.stage==1 && now>=1.5 {probe.stage=2;info!("NOISE PRÓBA lövés előtt: fázis={phase:?} zajok={:?}",roster.noises);if let Some(actor)=roster.actors.iter().find(|a|a.name==wanted) {let p=walking.player.position;let eye=Vec3::new(p.x,p.y+46.0,p.z);info!("NOISE PRÓBA npc pos={:?} hostile={} attacking={} hp={} táv={:.0} rálátás={}",actor.position,actor.hostile,roster.hostiles_attacking,actor.hp,eye.distance(actor.position),crate::npcs::line_of_sight(&walking.world,eye,actor.position));}}
    if probe.stage==2 && now>=1.6 {
        probe.stage=3;
        if std::env::var_os("MESTER_TEST_HOLD_FIRE").is_none() {controls.fire=true;controls.held=true;}
    }
    // The guard's reaction has its own delay: wait for the phase change (up to 8 s after the shot); the control run waits 0.6 s.
    let held_fire=std::env::var_os("MESTER_TEST_HOLD_FIRE").is_some();
    if probe.stage==3 && (now>=if held_fire {2.2}else{9.6} || (!held_fire && native.shots>0 && phase.as_deref()!=Some(probe.note.as_str()) && now>=1.8)) {
        probe.stage=4;
        info!("NOISE PRÓBA lövés után: fázis={phase:?} lövések={}",native.shots);
        let held=std::env::var_os("MESTER_TEST_HOLD_FIRE").is_some();
        let changed=phase.as_deref()!=Some(probe.note.as_str());
        if !held && native.shots==0 {probe.failure=Some("nem dördült el a lövés".into());}
        // A shot that wounds the guard must alert it; a lone shot into the distance or no shot at all must change nothing.
        if changed!=(!held && !away) {probe.failure=Some(format!("a lövés hatása hibás: változott={changed}, kontroll={held}"));}
        info!("NOISE PRÓBA kész: fázis megváltozott={changed} (kontroll={held})");
    }
}

/// `altfire`: M-14 unscoped, scoped, the reload quirk and back. `melee`: the nightstick against the nearest actor.
#[allow(clippy::too_many_arguments)]
pub fn probe(mut commands:Commands,mut probe:ResMut<Probe>,time:Res<Time>,config:Res<ViewerConfig>,mut walking:ResMut<Walking>,mut session:ResMut<Session>,mut campaign:ResMut<Campaign>,mut native:ResMut<NativeArsenal>,mut controls:ResMut<crate::gunfire::Controls>,mut buttons:ResMut<ButtonInput<MouseButton>>,alt:Res<AltFire>,view:Res<ViewState>,mut roster:ResMut<NpcRoster>,mut exit:MessageWriter<AppExit>) {
    if !probe.active || probe.finished {return;}
    campaign.health=100.0;session.paused=false;probe.elapsed+=time.delta_secs();probe.clock+=time.delta_secs().min(0.05);*controls=crate::gunfire::Controls::default();
    let end=config.capture_times.last().copied().unwrap_or(3.0);
    if probe.elapsed>end+2.0 {
        probe.finished=true;
        info!("ALTFIRE PRÓBA {}: swings={} npc_hits={} wall_hits={} laser_frames={} length={:.0} dust={} failure={:?}",probe.mode,alt.swings,alt.melee_npc_hits,alt.wall_hits,alt.laser_frames,alt.laser_length,alt.dust,probe.failure);
        if probe.failure.is_some() {exit.write(AppExit::error());}
        return;
    }
    // The nightstick is drawn with the 1/3 s raise and cannot swing meanwhile: the trigger stays held through both swing windows.
    if probe.mode=="melee" && ((1.6..2.5).contains(&probe.elapsed) || (3.0..4.0).contains(&probe.elapsed)) {controls.held=true;}
    if probe.mode=="revolver" {return revolver(&mut commands,&config,&mut probe,&mut walking,&mut native,&mut controls);}
    if probe.mode=="noise" {return noise(&mut probe,&mut walking,&mut native,&mut controls,&mut roster);}
    if probe.mode=="arsenal" {return arsenal(&mut commands,&config,&mut probe,&mut walking,&mut native,&mut controls,&roster,&view);}
    let at:&[f32]=match probe.mode.as_str() {"melee"=>&[0.5,1.5,1.6,2.6,3.0,4.2],"flash"=>&[0.5,1.5,3.0,3.2,4.0],_=>&[0.5,1.5,3.0,3.2,6.0,6.2,9.0,9.2,9.4,9.6,9.8,11.0]};
    if probe.stage>=at.len() || probe.elapsed<at[probe.stage] {return;}
    let stage=probe.stage;probe.stage+=1;
    let fail=|probe:&mut Probe,message:String| {error!("{message}");probe.failure.get_or_insert(message);};
    if probe.mode=="melee" {
        match stage {
            0=>{
                let wanted=std::env::var("MESTER_TEST_NPC").unwrap_or_default();
                let Some(actor)=roster.actors.iter().find(|a|a.alive() && a.visible && a.half_extents.y>20.0 && (wanted.is_empty() || a.name==wanted || a.definition_name==wanted)) else {return fail(&mut probe,"Nincs élő NPC a bunkópróbához".into())};
                // 40 units in front of the actor's box centre, looking at its chest.
                let side=actor.rotation*Vec3::Z*40.0;
                crate::campaign_probe::place(&mut walking,actor.position+side,actor.position+Vec3::Y*actor.half_extents.y*0.3);
                native.acquire_and_draw("Police nightstick");
            },
            1=>{let name=native.inventory.selected.map(|s|native.definitions[s].id.clone());if name.as_deref()!=Some("Police nightstick") {fail(&mut probe,format!("Nem a gumibot van kézben: {name:?}"));}},
            2=>{controls.held=true;controls.fire=true;},
            3=>{
                if alt.swings==0 || alt.melee_npc_hits==0 {fail(&mut probe,format!("A gumibot nem ütött: swings={} hits={}",alt.swings,alt.melee_npc_hits));}
                // Then against the nearest wall: put it 45 units in front of the eye.
                let p=walking.player.position;let eye=retail_movement::Vec3::new(p.x,p.y+46.0,p.z);
                let nearest=(0..72).filter_map(|i|{let yaw=i as f32*std::f32::consts::TAU/72.0;let d=retail_movement::Vec3::new(-yaw.sin(),0.0,-yaw.cos());
                    walking.world.raycast(eye,d,300.0).map(|h|(h.0,yaw,d))}).min_by(|a,b|a.0.total_cmp(&b.0));
                match nearest {
                    Some((distance,yaw,d))=>{walking.player.position=retail_movement::Vec3::new(p.x+d.x*(distance-45.0),p.y,p.z+d.z*(distance-45.0));walking.yaw=yaw;walking.pitch=0.0;},
                    None=>fail(&mut probe,"Nincs fal a bunkópróbához".into()),
                }
            },
            4=>{controls.held=true;controls.fire=true;},
            _=>{if alt.wall_hits==0 {fail(&mut probe,"A gumibot nem ütötte a falat".into());}},
        }
        return;
    }
    let check=|probe:&mut Probe,ok:bool,what:&str| {if !ok {fail(probe,format!("Altfire hiba: {what}"));}};
    if probe.mode=="flash" {
        match stage {
            0=>{
                let wanted=std::env::var("MESTER_TEST_WEAPON").unwrap_or_else(|_|"P90".into());
                native.acquire_and_draw(&wanted);
                let (yaw,distance)=longest_sight(&walking);let turn=std::env::var("MESTER_TEST_TURN").ok().and_then(|t|t.parse::<f32>().ok()).unwrap_or(0.0);walking.yaw=yaw+turn;walking.pitch=0.0;info!("ALTFIRE PRÓBA: {wanted}, irány {yaw:.2} rad, szabad látótávolság {distance:.0}");
            },
            1=>check(&mut probe,alt.flash_frames==0,"a zseblámpa magától bekapcsolt"),
            2=>buttons.press(MouseButton::Middle),
            3=>buttons.release(MouseButton::Middle),
            _=>check(&mut probe,alt.flags.alt && !alt.flags.zoom && alt.flash_frames>2,"a zseblámpa nem gyulladt ki"),
        }
        return;
    }
    match stage {
        0=>{
            native.acquire_and_draw("M-14");
            if let Some(slot)=native.inventory.selected {let index=native.definitions[slot].ammo_index;native.inventory.add_ammo(index,40);}
            let (yaw,distance)=longest_sight(&walking);let turn=std::env::var("MESTER_TEST_TURN").ok().and_then(|t|t.parse::<f32>().ok()).unwrap_or(0.0);walking.yaw=yaw+turn;walking.pitch=0.0;info!("ALTFIRE PRÓBA: irány {yaw:.2} rad, szabad látótávolság {distance:.0}");
        },
        1=>{check(&mut probe,native.inventory.selected.is_some_and(|s|native.definitions[s].id=="M-14"),"nincs M-14");controls.fire=true;},
        2=>buttons.press(MouseButton::Middle),
        3=>buttons.release(MouseButton::Middle),
        4=>{
            check(&mut probe,alt.flags.alt && alt.flags.zoom && view.scoped && view.mouse_scale()==0.25,"alt bekapcsolás nem szkóp");
            check(&mut probe,alt.laser_frames>10 && alt.dot.is_some(),"nincs lézer");
            info!("ALTFIRE PRÓBA szkóp: pont={:?} hossz={:.1} por={} zoom={} zsák={:?}",alt.dot,alt.laser_length,alt.dust,view.scoped,alt.socket);
            controls.reload=true;
        },
        5=>{},
        6=>{
            // Retail quirk: the reload cleared the weapon flag only; the camera stays zoomed.
            check(&mut probe,!alt.flags.alt && alt.flags.zoom && view.scoped && view.mouse_scale()==1.0,"újratöltés után nem maradt zoom");
            buttons.press(MouseButton::Middle);
        },
        7=>buttons.release(MouseButton::Middle),
        8=>{check(&mut probe,alt.flags.alt && alt.flags.zoom,"az újrafegyverzés nem tartotta a zoomot");buttons.press(MouseButton::Middle);},
        9=>buttons.release(MouseButton::Middle),
        _=>check(&mut probe,!alt.flags.alt && !alt.flags.zoom && !view.scoped,"a második alt nem oldotta a zoomot"),
    }
}

#[cfg(test)] mod tests {
    use super::*;
    use crate::models::Model;
    fn close(a:f32,b:f32)->bool {(a-b).abs()<1e-4}
    #[test] fn alt_press_is_edge_triggered_and_needs_only_a_weapon() {
        let mut f=AltFlags::default();
        assert!(!f.action(true,None) && !f.alt,"no weapon, no toggle");
        assert!(!f.action(true,Some(true)),"the latch was set even without a weapon");
        assert!(!f.action(false,Some(true)) && f.action(true,Some(true)) && f.alt && f.zoom);
        assert!(!f.action(true,Some(true)) && f.alt,"held: no repeat");
        f.action(false,Some(true));assert!(f.action(true,Some(true)) && !f.alt && !f.zoom);
    }
    #[test] fn only_alt_zoom_weapons_move_the_camera_flag() {
        let mut f=AltFlags::default();
        f.action(true,Some(false));assert!(f.alt && !f.zoom,"Glock: the flag flips, nothing else");
        assert!(!f.slow_mouse(false) && !f.crosshair_hidden(false));
        f.action(false,Some(false));f.action(true,Some(false));assert!(!f.alt && !f.zoom);
    }
    #[test] fn switching_away_unzooms_while_the_flag_is_set() {
        let mut f=AltFlags::default();f.action(true,Some(true));
        assert!(f.slow_mouse(true) && f.crosshair_hidden(true));
        f.leave(true);assert_eq!(f,AltFlags {down:true,..default()});
    }
    #[test] fn reload_clears_the_weapon_flag_but_the_camera_stays_zoomed() {
        let mut f=AltFlags::default();f.action(true,Some(true));
        f.reload(true);
        assert!(!f.alt && f.zoom,"quirk: zoomed overlay with the weapon flag off");
        assert!(!f.slow_mouse(true) && !f.crosshair_hidden(true),"mouse back to x1 and crosshair back");
        // Leaving now does not toggle, so the stale zoom survives a weapon change ...
        f.leave(true);assert!(f.zoom);
        // ... the next press re-arms it (still zoomed), the one after releases it.
        f.action(false,Some(true));assert!(f.action(true,Some(true)) && f.alt && f.zoom);
        f.action(false,Some(true));assert!(f.action(true,Some(true)) && !f.alt && !f.zoom);
        // A reload of a weapon without alt_zoom leaves its flag alone.
        let mut g=AltFlags::default();g.action(true,Some(false));g.reload(false);assert!(g.alt);
    }
    #[test] fn dust_count_and_positions_follow_the_length() {
        for (length,count) in [(0.0,0),(16.0,0),(16.1,1),(72.0,1),(72.1,2),(104.0,2),(104.1,3),(136.0,3),(136.1,4),(6400.0,4)] {assert_eq!(dust_count(length),count,"{length}");}
        assert!(close(dust_distance(6400.0,0.5),8.0+64.0) && close(dust_distance(50.0,1.0),50.0) && close(dust_distance(50.0,0.0),8.0));
    }
    #[test] fn beam_is_a_camera_facing_ribbon_two_tenths_wide() {
        let c=beam_corners(Vec3::new(1.0,2.0,3.0),Vec3::new(1.0,2.0,-97.0),Vec3::X);
        assert_eq!(c[0],Vec3::new(0.9,2.0,3.0));assert_eq!(c[1],Vec3::new(1.1,2.0,3.0));assert_eq!(c[2],Vec3::new(1.1,2.0,-97.0));assert_eq!(c[3],Vec3::new(0.9,2.0,-97.0));
    }
    #[test] fn scope_quadrants_mirror_the_reticle_around_the_screen_centre() {
        let quads=scope_quads();
        // The screen centre is (1,1) of the top-left quadrant, (0,1) of the top-right one, ...
        let centre=[(1.0,1.0),(0.0,1.0),(1.0,0.0),(0.0,0.0)];
        for (quad,(fx,fy)) in quads.iter().zip(centre) {
            let uv=scope_uv(quad,fx,fy);assert!(close(uv.x,0.995) && close(uv.y,0.995),"{quad:?} {uv}");
        }
        // The screen corners show the far corner of the quarter.
        let corner=[(0.0,0.0),(1.0,0.0),(0.0,1.0),(1.0,1.0)];
        for (quad,(fx,fy)) in quads.iter().zip(corner) {
            let uv=scope_uv(quad,fx,fy);assert!(close(uv.x,0.3) && close(uv.y,0.45),"{quad:?} {uv}");
        }
        let r=scope_rect(512.0,512.0);assert!(close(r.min.x,153.6) && close(r.min.y,230.4) && close(r.max.x,509.44) && close(r.max.y,509.44));
        assert_eq!(quads.map(|q|(q.left,q.top)),[(0.0,0.0),(0.5,0.0),(0.0,0.5),(0.5,0.5)]);
    }
    #[test] fn scope_area_keeps_the_reticle_at_four_by_three() {
        assert_eq!(scope_area(4.0/3.0),[0.0,0.0,1.0,1.0]);
        let [x,y,w,h]=scope_area(16.0/9.0);assert!(close(x,0.125) && y==0.0 && close(w,0.75) && h==1.0);
        let [x,y,w,h]=scope_area(1.0);assert!(x==0.0 && close(y,0.125) && w==1.0 && close(h,0.75));
        let bars=scope_bars(scope_area(16.0/9.0));
        assert_eq!(bars[0],[0.0,0.0,0.125,1.0]);assert!(close(bars[1][0],0.875) && close(bars[1][2],0.125) && bars[2][3]==0.0 && close(bars[3][1],1.0));
    }
    #[test] fn flashlight_sits_sixteen_units_short_of_the_hit_and_dims_with_distance() {
        let (along,radius,brightness)=flash_light(100.0);assert!(close(along,84.0) && close(radius,41.5) && close(brightness,0.9375));
        let (along,radius,brightness)=flash_light(1600.0);assert!(close(along,1584.0) && close(radius,304.0) && brightness==0.0);
        assert_eq!(flash_light(10.0).0,10.0,"closer than 16 units the light sits on the hit");
    }
    #[test] fn nightstick_probes_five_points_and_boxes_are_strict() {
        let p=melee_points(Vec3::new(10.0,20.0,30.0),Vec3::NEG_Z);
        assert_eq!(p.map(|v|v.z),[6.0,-2.0,-18.0,-26.0,-34.0]);
        assert!(p.iter().all(|v|v.x==10.0 && v.y==20.0));
        let centre=Vec3::new(10.0,20.0,-2.0);let half=Vec3::new(14.0,50.0,10.0);
        assert!(inside_box(p[1],centre,half),"the 32-unit point is at the centre");
        assert!(!inside_box(Vec3::new(24.0,20.0,-2.0),centre,half),"on the face is outside");
        assert!(inside_box(p[0],centre,half) && !inside_box(p[2],centre,half) && !inside_box(p[4],centre,half));
    }
    #[test] fn catalog_flags_match_the_retail_items() {
        let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
        let catalog:RawCatalog=serde_json::from_str(&std::fs::read_to_string(root.join("retail_weapons.json")).unwrap()).unwrap();
        let full:serde_json::Value=serde_json::from_str(&std::fs::read_to_string(root.join("retail_weapons.json")).unwrap()).unwrap();
        let by_id=|id:&str|{let i=full["weapons"].as_array().unwrap().iter().position(|w|w["id"]==id).unwrap();AltWeapon::from_commands(&catalog.weapons[i].commands)};
        let m14=by_id("M-14");assert!(m14.alt_zoom && m14.laser_socket(false)==Some("laser") && m14.laser_socket(true)==Some("laser") && m14.flashlight.is_none());
        let p90=by_id("P90");assert!(!p90.alt_zoom && p90.laser_socket(false)==Some("laser") && p90.flashlight.as_deref()==Some("latarka"));
        let sig=by_id("Sig 551-p/SWAT");assert!(sig.laser.is_none() && sig.flashlight.as_deref()==Some("latarka"));
        for id in ["Glock","Smith and Wesson m. 625","Police nightstick","Hand grenade"] {assert_eq!(by_id(id),AltWeapon::default(),"{id}");}
    }
    #[test] fn m14_laser_socket_is_ahead_right_and_below_the_eye() {
        let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
        let catalog:crate::retail_state::Catalog=serde_json::from_str(&std::fs::read_to_string(root.join("retail_weapons.json")).unwrap()).unwrap();
        for id in ["M-14","P90"] {
            let d=catalog.weapons.iter().find(|d|d.id==id).unwrap();let model=Model::load(&root,&d.model);let pose=model.pose(&d.animations.base,0.0,true);
            let view=ViewState::default().weapon(crate::retail_weapons::view_transform(d));
            let s=view.transform_point(model.socket(&pose,"laser").transform_point3(Vec3::ZERO))/SCALE;
            assert!(s.z< -10.0 && s.z> -80.0 && s.x>0.0 && s.y<0.0,"{id}: {s:?}");
        }
    }
}
