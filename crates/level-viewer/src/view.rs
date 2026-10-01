//! Retail first-person view: eye height, bob, strafe roll, FOV and zoom, death
//! and drunk camera, the `CWeaponBob` weapon offset and fall damage.
//! Evidence and addresses (cshell.dll): docs/retail-camera.md.
//! LithTech yaw/pitch grow to the right/down. The displayed image is flipped (mirror.rs), so with the mirror Bevy's yaw grows to the right
//! too (without it the Bevy angles were negated); pitch keeps the negated sign, and a retail roll (positive tilts the camera's up towards
//! the screen's left) or a right offset takes `mirror::SIGN`.
use bevy::{camera::{CameraProjection,SubCameraView},math::Vec3A,prelude::*};
use std::f32::consts::{PI,TAU};
use crate::{SCALE,Walking,InspectionCamera,campaign::Campaign,settings::Session,retail_weapons::{NativeArsenal,NativeCamera}};

pub const EYE_STAND:f32=46.0;
pub const EYE_CROUCH:f32=16.0;
/// Radians per mouse count at sensitivity 1.0 (0x10054bf0).
pub const MOUSE_RADIANS:f32=0.006625;
pub const PITCH_LIMIT:f32=1.5393804;
pub const FOV_X_DEGREES:f32=81.0;
const FALL_SOUND:&str="audio/sounds/speech/hero/spad.wav";

/// What the controller did this frame; flags are key state, not measured speed.
#[derive(Clone,Copy,Default)] pub struct Intent {pub moving:bool,pub strafe_left:bool,pub strafe_right:bool,pub run:bool}

/// 0x10055130 with the caps stored at 0x100600f4 (Bevy angles = negated LithTech yaw/pitch; roll keeps its sign).
/// Retail keeps the three Euler fields: yaw 0x50, pitch 0x4c (mouse code 0x10054bf0), roll 0x54 (unused when alive).
#[derive(Clone,Copy,Debug,PartialEq)] pub struct Death {pub yaw:f32,pub pitch:f32,pub roll:f32}
impl Death {
    pub fn new(yaw:f32,pitch:f32)->Self {Self {yaw,pitch,roll:0.0}}
    /// Yaw +1 rad/s up to the absolute 7π/4 (cap 0x5c), pitch +3 rad/s (down) up to level 0 (cap 0x58), roll +1 rad/s up to
    /// π/2 (cap 0x60 = roll at death + π/2). The 3π/4 stored at 0x64 is the strafe roll, which this camera ignores.
    pub fn step(self,dt:f32)->Self {Self {yaw:(self.yaw-dt).max(-1.75*PI),pitch:(self.pitch-3.0*dt).max(0.0),roll:(self.roll+dt).min(0.5*PI)}}
}

#[derive(Resource)]
pub struct ViewState {
    pub eye:f32,pub bob_phase:f32,pub strafe_roll:f32,
    /// `CWeaponBob` offset of the first-person weapon, native units, Bevy view space.
    pub weapon_offset:Vec3,
    /// Switch dip v (0..10): the weapon drops v units and pitches 0.05·v rad (0x100106ca); `Inventory::dip` drives it.
    pub weapon_dip:f32,weapon_drop:Vec3,
    /// Set by the alternate fire of `alt_zoom` weapons (M-14).
    pub scoped:bool,
    /// The weapon's alt flag on an `alt_zoom` weapon slows the mouse (0x10054cee); a reload clears it while `scoped` stays.
    pub mouse_slow:bool,
    /// Q: toggled run, combined with Shift by XOR; nothing clears it.
    pub run_toggle:bool,
    pub(crate) intent:Intent,
    alcohol:f32,drunk:[f32;2],rng:u32,fov:[f32;2],frames:u32,death:Option<Death>,
    /// Debug/capture aid (MESTER_DEATH_FRAME): kills the player on that frame of the level.
    pub debug_death:Option<u32>,
}
impl Default for ViewState {
    fn default()->Self {Self {eye:EYE_STAND,bob_phase:0.0,strafe_roll:0.0,weapon_offset:Vec3::ZERO,weapon_dip:0.0,weapon_drop:Vec3::ZERO,scoped:false,mouse_slow:false,run_toggle:false,
        intent:Intent::default(),alcohol:0.0,drunk:[0.0;2],rng:1,fov:[f32::NAN;2],frames:0,death:None,debug_death:None}}
}
impl ViewState {
    /// A new level resets the camera; alcohol, the drunk phases and the run toggle are global in retail.
    pub fn enter_level(&mut self) {*self=Self {alcohol:self.alcohol,drunk:self.drunk,rng:self.rng,run_toggle:self.run_toggle,debug_death:self.debug_death,..Self::default()};}
    pub fn alcohol(&self)->f32 {self.alcohol}
    /// Retail stores any amount, decays it 0.5/s and caps its effect at 100.
    pub fn set_alcohol(&mut self,value:f32) {self.alcohol=value.max(0.0);}
    /// Frames since the level started (retail shell +0x37c); health changes are ignored for the first 60 (0x10061ba7).
    pub fn level_frames(&self)->u32 {self.frames}
    pub fn mouse_scale(&self)->f32 {if self.mouse_slow {0.25}else{1.0}}
    /// Applies bob and switch dip to a weapon's base view transform.
    pub fn weapon(&self,base:Transform)->Transform {
        Transform {translation:base.translation+(self.weapon_offset+self.weapon_drop)*SCALE,rotation:Quat::from_rotation_x(-0.05*self.weapon_dip)*base.rotation,scale:base.scale}
    }
    fn random(&mut self)->f32 {self.rng=self.rng.wrapping_mul(214013).wrapping_add(2531011);((self.rng>>16)&0x7fff) as f32%1000.0*0.001}
}

/// 0x1005546b: the eye slides linearly, 250 units/s, between 16 and 46 above the box centre.
pub fn slide_eye(eye:f32,crouched:bool,dt:f32)->f32 {if crouched {(eye-250.0*dt).max(EYE_CROUCH)}else{(eye+250.0*dt).min(EYE_STAND)}}
/// 0x10055130: when dead the eye falls 192 units/s to 16 above the box bottom.
pub fn death_eye(eye:f32,half_height:f32,dt:f32)->f32 {(eye-192.0*dt).max(16.0-half_height)}
/// (-π, π]
pub fn wrap(angle:f32)->f32 {PI-(PI-angle).rem_euclid(TAU)}
/// 0x10054bf0: raw counts, no smoothing, acceleration or frame-time scaling.
pub fn look(yaw:f32,pitch:f32,delta:Vec2,sensitivity:f32,scale:f32)->(f32,f32) {look_axes(yaw,pitch,delta,sensitivity,scale,scale)}
/// The look step under a dialogue: by default (the owner's choice) plain full-speed look, the answers are picked with the wheel / arrows; with "Gyári egeres választás" (`retail`)
/// a node with answers scales the pitch by 0.25 (0x10054d2d) and yaw stays live.
pub fn dialogue_look(yaw:f32,pitch:f32,delta:Vec2,sensitivity:f32,scale:f32,choices:bool,retail:bool)->(f32,f32) {
    look_axes(yaw,pitch,delta,sensitivity,scale,scale*if choices && retail {0.25}else{1.0})
}
/// The same with separate factors: a dialogue with answers scales only the pitch by 0.25 (0x10054d2d).
pub fn look_axes(yaw:f32,pitch:f32,delta:Vec2,sensitivity:f32,yaw_scale:f32,pitch_scale:f32)->(f32,f32) {
    let k=MOUSE_RADIANS*sensitivity;
    // The displayed image is flipped (mirror.rs): moving the mouse right turns toward Bevy's left, LithTech's +yaw.
    (wrap(yaw-delta.x*k*yaw_scale*crate::mirror::SIGN),(pitch-delta.y*k*pitch_scale).clamp(-PITCH_LIMIT,PITCH_LIMIT))
}
/// 0x100555ac: ±0.1 rad/s while a strafe key is held on the ground, clamped to ±0.04; back to 0 at 0.1 rad/s.
pub fn strafe_roll(roll:f32,left:bool,right:bool,dt:f32)->f32 {
    let step=0.1*dt;
    if !left && !right {return if roll.abs()<=step {0.0}else{roll-step*roll.signum()};}
    (roll+step*(left as i32-right as i32) as f32).clamp(-0.04,0.04)
}
/// 0x1005567d..0x10055cbb: returns the phase whose offsets apply this frame (None at rest) and the next phase.
/// After stopping the phase runs on until it passes 4π or lands within one step past 2π.
pub fn bob_step(phase:f32,moving:bool,run_upright:bool,dt:f32)->(Option<f32>,f32) {
    let step=11.5*dt;
    if !moving && (phase==0.0 || phase>2.0*TAU || (phase>=TAU && phase-TAU<=step)) {return (None,0.0);}
    let next=phase+step*if run_upright {1.2}else{1.0};
    (Some(phase),if next>2.0*TAU {0.0}else{next})
}
#[derive(Clone,Copy,Debug,PartialEq)] pub struct BobPose {pub up:f32,pub right:f32,pub roll:f32,pub weapon:Vec2}
/// Camera offset along its own up/right axes and roll; the weapon (x right, y up) moves
/// by up·s1·(3.7|2.8) + right·s2·(1|0.65) + right·1.5·s2, so relative to the camera as below (0x10055cbe).
pub fn bob_pose(phase:f32,run_upright:bool)->BobPose {
    let (s1,s2)=(1.3*phase.sin(),1.3*(phase*0.5).sin());
    let (camera,weapon,side)=if run_upright {(4.0,3.7,1.0)}else{(3.0,2.8,0.65)};
    BobPose {up:s1*camera,right:1.5*s2,roll:-0.005*s2,weapon:Vec2::new(s2*side,s1*(weapon-camera))}
}
/// 0x100560e0: [fovX, fovY] in radians; fovY = fovX·1.1·h/w at 4:3 and kept on wider screens (Hor+).
pub fn retail_fov(fov_x_degrees:f32)->[f32;2] {let x=fov_x_degrees.to_radians();[x,x*1.1*0.75]}
/// 0x10056180: `alt_zoom` scales both axes by 0.15 at 5 rad/s each; both snap when either arrives.
pub fn zoom_step(fov:[f32;2],normal:[f32;2],scoped:bool,dt:f32)->[f32;2] {
    let (step,zoom)=(5.0*dt,normal.map(|v|v*0.15));let mut fov=fov;
    // Unscoped at or beyond the normal FOV (also after the preference was lowered).
    if !scoped && fov[0]>=normal[0] && fov[1]>=normal[1] {return normal;}
    for i in 0..2 {
        if scoped && fov[i]>zoom[i] {fov[i]-=step;if fov[i]<zoom[i] {return zoom;}}
        if !scoped && fov[i]<normal[i] {fov[i]+=step;if fov[i]>normal[i] {return normal;}}
    }
    fov
}
/// 0x100562cf: phase += (3 + rand%1000/1000)·0.25·factor·dt; past π it restarts at −π.
pub fn drunk_phase(phase:f32,random:f32,factor:f32,dt:f32)->f32 {let p=phase.max(-PI)+(3.0+random)*0.25*factor*dt;if p>PI {-PI}else{p}}
/// 0x10055e5b, 0x100563a7: alcohol (capped at 100) turns the view by cos·a·0.005 about the
/// camera's up/right axes ([yaw, pitch], LithTech signs) and adds sin·a·0.0035 rad to fovX/fovY.
pub fn drunk(alcohol:f32,phase:[f32;2])->([f32;2],[f32;2]) {
    let a=alcohol.min(100.0);
    if a<=0.0 {return ([0.0;2],[0.0;2]);}
    (phase.map(|p|p.cos()*a*0.005),phase.map(|p|p.sin()*a*0.0035))
}

/// Retail sets fovX and fovY independently, so its 4:3 pixels are not square; Bevy derives the
/// horizontal extent from the aspect ratio, so the clip X axis is rescaled to restore tan(fovX/2)
/// at 4:3 (Hor+ beyond it).
#[derive(Debug,Clone)] pub struct RetailProjection {pub perspective:PerspectiveProjection,pub x_scale:f32,/// The retail [fovX, fovY] (radians, 4:3) the projection was last set to.
    pub fov:[f32;2]}
impl RetailProjection {
    pub fn new(fov:[f32;2],near:f32)->Self {let mut p=Self {perspective:PerspectiveProjection {near,..default()},x_scale:1.0,fov};p.set(fov);p}
    pub fn set(&mut self,fov:[f32;2]) {self.fov=fov;self.perspective.fov=fov[1];self.x_scale=(fov[1]*0.5).tan()/(0.75*(fov[0]*0.5).tan());}
}
impl CameraProjection for RetailProjection {
    fn get_clip_from_view(&self)->Mat4 {Mat4::from_scale(Vec3::new(self.x_scale,1.0,1.0))*self.perspective.get_clip_from_view()}
    fn get_clip_from_view_for_sub(&self,sub_view:&SubCameraView)->Mat4 {Mat4::from_scale(Vec3::new(self.x_scale,1.0,1.0))*self.perspective.get_clip_from_view_for_sub(sub_view)}
    fn update(&mut self,width:f32,height:f32) {self.perspective.update(width,height);}
    fn far(&self)->f32 {self.perspective.far}
    fn get_frustum_corners(&self,z_near:f32,z_far:f32)->[Vec3A;8] {self.perspective.get_frustum_corners(z_near,z_far).map(|c|c*Vec3A::new(1.0/self.x_scale,1.0,1.0))}
}
/// Converts a perspective camera (keeping its planes) or updates an existing retail projection.
pub fn apply_fov(projection:&mut Projection,fov:[f32;2]) {
    if let Projection::Custom(custom)=projection {if let Some(retail)=custom.get_mut::<RetailProjection>() {retail.set(fov);return;}}
    let base=if let Projection::Perspective(p)=projection {p.clone()}else{default()};
    let mut retail=RetailProjection {perspective:base,x_scale:1.0,fov};retail.set(fov);
    *projection=Projection::custom(retail);
}

/// The camera's current retail [fovX, fovY] in radians (4:3 horizontal, vertical), zoom and drunk wobble included; for
/// effects that scale with the view. Also accepts a plain perspective projection.
pub fn camera_fov(projection:&Projection)->Option<[f32;2]> {
    match projection {
        Projection::Custom(custom)=>custom.get::<RetailProjection>().map(|retail|retail.fov),
        Projection::Perspective(p)=>Some([2.0*((p.fov*0.5).tan()/0.75).atan(),p.fov]),
        _=>None,
    }
}

/// Runs right after the controller: places the camera and weapon view, applies fall damage.
#[allow(clippy::too_many_arguments)]
pub fn update(mut view:ResMut<ViewState>,options:Res<crate::options::Options>,walking:Res<Walking>,mut campaign:ResMut<Campaign>,session:Res<Session>,opening:Res<crate::opening::Opening>,time:Res<Time>,native:Res<NativeArsenal>,assets:Res<AssetServer>,mut commands:Commands,
    mut camera:Single<(&mut Transform,&mut Projection),(With<InspectionCamera>,Without<NativeCamera>)>,mut weapon_camera:Single<&mut Projection,(With<NativeCamera>,Without<InspectionCamera>)>) {
    let dead=campaign.health<=0.0;
    if opening.active || session.paused || (session.dialogue_active && !dead) {return;}
    let dt=time.delta_secs().min(0.05);let view=&mut *view;let player=&walking.player;
    view.frames+=1;
    if view.debug_death==Some(view.frames) {campaign.health=0.0;}
    // The drunk level is the character's alcohol value (cshell 0x100b23b4): items add to it, character::tick decays it, death clears it.
    view.alcohol=campaign.alcohol;
    for (i,factor) in [0.9,0.99].into_iter().enumerate() {let random=view.random();view.drunk[i]=drunk_phase(view.drunk[i],random,factor,dt);}
    // (takeoff − landing − 196) × 0.15 health after 60 frames in the level, the takeoff being the jump apex (cshell 0x1006102f): retail_movement::Player.
    let damage=player.events.fall_damage;
    // `spad.wav` (2D) follows the health change whether or not it still counts: 0x10061b90 ignores a dead player, 0x10061064 plays regardless.
    if damage>0.0 && view.frames>60 {
        if !dead {campaign.change_health(-damage,view.frames);info!("Esési sérülés: {damage:.1}");}
        crate::audio::play_2d(&mut commands,&assets,FALL_SOUND);
    }
    let run_upright=view.intent.run && !player.crouched;
    let (turn,fov_add)=drunk(view.alcohol(),view.drunk);
    let (rotation,bob)=if dead {
        let death=view.death.get_or_insert(Death::new(walking.yaw,walking.pitch));*death=death.step(dt);
        view.eye=death_eye(view.eye,player.half_size().y,dt);
        // The spin and the roll are retail's on screen: mirrored about the yaw the player died with.
        let (yaw,roll)=if crate::mirror::MIRRORED {(2.0*walking.yaw-death.yaw,-death.roll)}else{(death.yaw,death.roll)};
        (Quat::from_euler(EulerRot::YXZ,yaw,death.pitch,roll),None)
    }else {
        view.death=None;
        view.eye=slide_eye(view.eye,player.crouched,dt);
        view.strafe_roll=strafe_roll(view.strafe_roll,view.intent.strafe_left,view.intent.strafe_right,dt);
        let (shown,next)=bob_step(view.bob_phase,view.intent.moving,run_upright,dt);view.bob_phase=next;
        let pose=shown.map(|phase|bob_pose(phase,run_upright));
        (Quat::from_euler(EulerRot::YXZ,walking.yaw-crate::mirror::SIGN*turn[0],walking.pitch-turn[1],crate::mirror::SIGN*(view.strafe_roll+pose.map_or(0.0,|p|p.roll))),pose)
    };
    // Bob follows the undisturbed yaw/pitch axes, so it tilts with pitch but not with roll.
    let axes=Quat::from_euler(EulerRot::YXZ,walking.yaw,walking.pitch,0.0);
    let p=player.position;
    camera.0.translation=(Vec3::new(p.x,p.y+view.eye,p.z)+bob.map_or(Vec3::ZERO,|b|axes*Vec3::new(b.right*crate::mirror::SIGN,b.up,0.0)))*SCALE;
    camera.0.rotation=rotation;
    // The weapon bob option only moves the weapon model (0x10055cbe): off keeps it at its rest pose.
    view.weapon_offset=bob.filter(|_|options.weapon_bob).map_or(Vec3::ZERO,|b|b.weapon.extend(0.0));
    // Retail lowers the old weapon for a third of a second, swaps, then raises the new one (0x10010125, 0x1001015d); the counter
    // lives in the inventory because it also blocks firing and reloading.
    view.weapon_dip=native.inventory.dip();
    view.weapon_drop=rotation.inverse()*Vec3::NEG_Y*view.weapon_dip;
    let normal=retail_fov(session.preferences.fov);
    if !view.fov[0].is_finite() {view.fov=normal;}
    view.fov=zoom_step(view.fov,normal,view.scoped,dt);
    apply_fov(&mut camera.1,[view.fov[0]+fov_add[0],view.fov[1]+fov_add[1]]);
    // One retail camera draws the weapon too; its model offsets are authored for the fixed 81°.
    let weapon=retail_fov(FOV_X_DEGREES);
    apply_fov(&mut weapon_camera,[weapon[0]+fov_add[0],weapon[1]+fov_add[1]]);
}

#[cfg(test)] mod tests {
    #[test] fn the_dialogue_look_is_plain_by_default_and_retail_slows_only_the_pitch() {
        let d=Vec2::new(40.0,30.0);let full=look(0.3,0.1,d,1.0,1.0);
        // Default (owner): the mouse look is not locked and not slowed while a choice list shows.
        assert_eq!(dialogue_look(0.3,0.1,d,1.0,1.0,true,false),full);
        assert_eq!(dialogue_look(0.3,0.1,d,1.0,1.0,false,true),full,"no list: retail mode changes nothing either");
        // Retail mouse: yaw live, pitch x0.25.
        let (yaw,pitch)=dialogue_look(0.3,0.1,d,1.0,1.0,true,true);
        assert_eq!(yaw,full.0);assert!(((0.1-pitch)-(0.1-full.1)*0.25).abs()<1e-6);
    }
    use super::*;
    fn close(a:f32,b:f32)->bool {(a-b).abs()<1e-4}
    #[test] fn eye_slides_between_retail_heights() {
        assert!(close(slide_eye(46.0,true,0.02),41.0));
        assert!(close(slide_eye(20.0,true,0.1),16.0));
        assert!(close(slide_eye(16.0,false,0.04),26.0));
        assert!(close(slide_eye(40.0,false,0.1),46.0));
        let mut eye=46.0;let mut t=0.0;while eye>16.0 {eye=slide_eye(eye,true,0.01);t+=0.01;}
        assert!((t-0.12_f32).abs()<0.011,"30 units at 250/s: {t}");
    }
    #[test] fn death_eye_stops_16_above_the_box_bottom() {
        assert!(close(death_eye(46.0,58.0,0.1),26.8));
        assert!(close(death_eye(0.0,58.0,1.0),-42.0));
        assert!(close(death_eye(16.0,24.0,1.0),-8.0));
    }
    #[test] fn death_camera_levels_the_view_spins_to_7pi_over_4_and_rolls_a_quarter_turn() {
        let mut d=Death::new(0.3,0.2);
        d=d.step(0.05);assert!(close(d.yaw,0.25) && close(d.pitch,0.05) && close(d.roll,0.05));
        // Looking down (Bevy pitch below zero) snaps to level at once.
        assert_eq!(Death::new(0.0,-0.4).step(0.01).pitch,0.0);
        for _ in 0..1000 {d=d.step(0.05);}
        assert!(close(d.yaw,-1.75*PI) && close(d.pitch,0.0) && close(d.roll,0.5*PI));
    }
    #[test] fn mouse_uses_raw_counts_clamps_pitch_and_wraps_yaw() {
        let (yaw,pitch)=look(0.0,0.0,Vec2::new(100.0,-10.0),1.0,1.0);
        assert!(close(yaw,-0.6625*crate::mirror::SIGN) && close(pitch,0.06625));
        let (_,pitch)=look(0.0,0.0,Vec2::new(0.0,1000.0),1.5,1.0);assert_eq!(pitch,-PITCH_LIMIT);
        let (yaw,_)=look(3.1,0.0,Vec2::new(-10.0*crate::mirror::SIGN,0.0),1.0,1.0);assert!(close(yaw,3.16625-TAU));
        let (yaw,_)=look(0.0,0.0,Vec2::new(40.0,0.0),0.5,0.25);assert!(close(yaw,-0.033125*crate::mirror::SIGN));
        assert!(close(wrap(-PI),PI) && close(wrap(PI),PI) && close(wrap(7.0),7.0-TAU) && close(wrap(-4.0),TAU-4.0));
    }
    /// Mirror audit: the mouse, the strafe keys and the retail right offsets all follow the RIGHT OF THE DISPLAYED IMAGE (mirror::right), whichever way the flip is set.
    #[test] fn mouse_strafe_and_bob_follow_the_displayed_right() {
        let yaw=0.7;let shown=|yaw:f32|Transform::from_rotation(Quat::from_euler(EulerRot::YXZ,yaw,0.0,0.0));
        let right=crate::mirror::right(&shown(yaw));
        let (turned,_)=look(yaw,0.0,Vec2::new(10.0,0.0),1.0,1.0);
        assert!(shown(turned).forward().dot(right)>0.0,"mouse right must turn the view to the displayed right");
        let f=shown(yaw).forward();let heading=(-f.x).atan2(-f.z);let (s,c)=heading.sin_cos();
        // retail_movement::Player::tick: velocity += (-s*forward + c*right, 0, -c*forward - s*right); D presses right = SIGN.
        let strafe=Vec3::new(c,0.0,-s)*crate::mirror::SIGN;
        assert!(strafe.dot(right)>0.999,"D must strafe to the displayed right");
        // the bob's right offset and the strafe roll (left strafe leans the view left = up towards the displayed left)
        let bob=Quat::from_euler(EulerRot::YXZ,yaw,0.0,0.0)*Vec3::new(1.0*crate::mirror::SIGN,0.0,0.0);
        assert!(bob.dot(right)>0.999);
        let leaned=Quat::from_euler(EulerRot::YXZ,yaw,0.0,crate::mirror::SIGN*0.04)*Vec3::Y;
        assert!(leaned.dot(-right)>0.0,"positive retail roll tilts the camera's up towards the displayed left");
    }
    #[test] fn strafe_roll_leans_clamps_and_recovers() {
        assert!(close(strafe_roll(0.0,true,false,0.1),0.01));
        assert!(close(strafe_roll(0.035,true,false,0.1),0.04));
        assert!(close(strafe_roll(-0.035,false,true,0.1),-0.04));
        assert!(close(strafe_roll(0.02,true,true,0.1),0.02));
        assert!(close(strafe_roll(0.04,false,false,0.1),0.03));
        assert!(close(strafe_roll(-0.04,false,false,0.1),-0.03));
        assert_eq!(strafe_roll(0.005,false,false,0.1),0.0);
        assert_eq!(strafe_roll(-0.01,false,false,0.1),0.0);
    }
    #[test] fn bob_advances_runs_faster_upright_and_resets_past_4pi() {
        let (shown,next)=bob_step(0.0,true,false,0.1);assert!(shown==Some(0.0) && close(next,1.15));
        assert!(close(bob_step(1.0,true,true,0.1).1,1.0+1.38));
        assert_eq!(bob_step(2.0*TAU-0.1,true,false,0.02).1,0.0);
        assert_eq!(bob_step(0.0,false,false,0.1),(None,0.0));
    }
    #[test] fn bob_runs_on_after_stopping_until_2pi_or_4pi() {
        let (shown,next)=bob_step(1.0,false,false,0.1);assert_eq!(shown,Some(1.0));assert!(close(next,2.15));
        assert_eq!(bob_step(TAU+0.1,false,false,0.1),(None,0.0));
        let (shown,next)=bob_step(TAU+0.2,false,false,0.01);assert_eq!(shown,Some(TAU+0.2));assert!(close(next,TAU+0.315));
        assert_eq!(bob_step(2.0*TAU+0.01,false,false,0.1),(None,0.0));
        let mut phase=0.5;let mut frames=0;while phase!=0.0 {phase=bob_step(phase,false,false,1.0/60.0).1;frames+=1;}
        assert!(frames<40,"walk-stop settles at 2π: {frames}");
    }
    #[test] fn bob_offsets_match_retail_amplitudes() {
        let quarter=bob_pose(PI/2.0,false);
        assert!(close(quarter.up,3.9) && close(quarter.right,1.5*1.3*(PI/4.0).sin()) && close(quarter.roll,-0.005*1.3*(PI/4.0).sin()));
        assert!(close(quarter.weapon.y,-0.26) && close(quarter.weapon.x,0.845*(PI/4.0).sin()));
        let run=bob_pose(PI/2.0,true);assert!(close(run.up,5.2) && close(run.weapon.y,-0.39) && close(run.weapon.x,1.3*(PI/4.0).sin()));
        let side=bob_pose(PI,false);assert!(close(side.up,0.0) && close(side.right,1.95) && close(side.weapon.x,0.845));
    }
    #[test] fn retail_fov_is_81_by_66_82_at_4_3() {
        let [x,y]=retail_fov(FOV_X_DEGREES);
        assert!(close(x,1.4137167) && (y.to_degrees()-66.825).abs()<0.01);
        let p=RetailProjection::new([x,y],0.1);
        let clip=p.get_clip_from_view();
        // At 4:3 the horizontal half-extent is tan(40.5°), as the retail fovX says.
        let mut q=p.clone();q.update(1024.0,768.0);let m=q.get_clip_from_view();
        assert!(((1.0/m.x_axis.x)-(x*0.5).tan()).abs()<1e-5 && ((1.0/m.y_axis.y)-(y*0.5).tan()).abs()<1e-5);
        assert!(clip.is_finite());
        assert_eq!(camera_fov(&Projection::custom(p.clone())),Some([x,y]));
        let mut plain=Projection::Perspective(PerspectiveProjection {fov:y,..default()});
        assert!((camera_fov(&plain).unwrap()[0]-2.0*((y*0.5).tan()/0.75).atan()).abs()<1e-6);
        apply_fov(&mut plain,[1.0,0.8]);assert_eq!(camera_fov(&plain),Some([1.0,0.8]));
        // Hor+: 16:9 keeps fovY and widens the horizontal extent by the aspect ratio.
        q.update(1280.0,720.0);let wide=q.get_clip_from_view();
        assert!(((1.0/wide.x_axis.x)-(x*0.5).tan()*(16.0/9.0)/(4.0/3.0)).abs()<1e-5 && close(wide.y_axis.y,m.y_axis.y));
    }
    #[test] fn zoom_takes_both_axes_to_15_percent_and_back() {
        let normal=retail_fov(FOV_X_DEGREES);
        let mut fov=normal;let mut frames=0;
        while fov!=normal.map(|v|v*0.15) {fov=zoom_step(fov,normal,true,0.01);frames+=1;assert!(frames<100);}
        assert!((20..=26).contains(&frames),"1.2 rad at 5 rad/s: {frames}");
        while fov!=normal {fov=zoom_step(fov,normal,false,0.01);frames+=1;assert!(frames<200);}
        assert_eq!(zoom_step(retail_fov(110.0),normal,false,0.01),normal);
        assert_eq!(zoom_step(normal,normal,false,0.01),normal);
    }
    #[test] fn drunk_phases_wrap_to_minus_pi_and_effects_cap_at_100() {
        assert!(close(drunk_phase(0.0,0.5,0.9,1.0),3.5*0.25*0.9));
        assert_eq!(drunk_phase(3.1,0.9,0.99,1.0),-PI);
        assert!(close(drunk_phase(-5.0,0.0,0.9,0.0),-PI));
        assert_eq!(drunk(0.0,[1.0,2.0]),([0.0;2],[0.0;2]));
        let (turn,fov)=drunk(250.0,[0.0,PI/2.0]);
        assert!(close(turn[0],0.5) && close(turn[1],0.0) && close(fov[0],0.0) && close(fov[1],0.35));
    }
    #[test] fn view_state_keeps_global_values_across_levels() {
        let mut view=ViewState {eye:20.0,bob_phase:3.0,run_toggle:true,..default()};view.set_alcohol(40.0);view.death=Some(Death::new(0.0,0.0));
        view.enter_level();
        assert_eq!((view.eye,view.bob_phase,view.run_toggle,view.alcohol(),view.death),(EYE_STAND,0.0,true,40.0,None));
        view.set_alcohol(-3.0);assert_eq!(view.alcohol(),0.0);
    }
    #[test] fn weapon_view_applies_bob_and_switch_dip() {
        let view=ViewState {weapon_offset:Vec3::new(1.0,-0.5,0.0),weapon_dip:10.0,weapon_drop:Vec3::new(0.0,-10.0,0.0),..default()};
        let moved=view.weapon(Transform::from_xyz(0.1,-0.2,-0.3));
        assert!((moved.translation-Vec3::new(0.11,-0.305,-0.3)).length()<1e-6);
        // Muzzle (view −Z) dips down by 0.5 rad.
        assert!(close((moved.rotation*Vec3::NEG_Z).y,-(0.5_f32).sin()));
    }
}
