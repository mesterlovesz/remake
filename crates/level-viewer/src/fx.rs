//! Retail sprite/model particles. `EffectMgr::Spawn` (cshell 0x1004e520) creates every effect, the per-frame update
//! (0x10054440) ages it, fades it by dt/lifetime and grows it by `scale *= 1+g*dt`, the motion update (0x10052290) adds
//! `vel*dt` and subtracts gravity 640/240/96 from vel.y. A sprite quad is `2*tex*scale` units (Lithtech.exe 0x53da90).
//! Positions are native units (1 unit = 1 cm); `view` particles live in the first-person camera's space on RenderLayers 3.
use bevy::{camera::visibility::RenderLayers,prelude::*};
use std::{collections::BTreeMap,sync::{atomic::{AtomicU32,Ordering},Arc}};
use crate::{InspectionCamera,SCALE,WorldGeometry,settings::Session};

static SEED:AtomicU32=AtomicU32::new(0x2545_f491);
/// Uniform 0..1 from a shared xorshift stream (visual variation only).
pub fn rnd()->f32 {let mut x=SEED.load(Ordering::Relaxed);x^=x<<13;x^=x>>17;x^=x<<5;SEED.store(x,Ordering::Relaxed);(x>>8) as f32/16777216.0}
pub fn between(low:f32,high:f32)->f32 {low+(high-low)*rnd()}
pub fn signed(range:f32)->f32 {between(-range,range)}
pub fn pick(count:usize)->usize {((rnd()*count as f32) as usize).min(count.saturating_sub(1))}
/// Library key of a retail path ("sprites\\glock\\main.spr" -> "sprites/glock/main.spr").
pub fn key(path:&str)->String {path.replace('\\',"/").to_lowercase()}
/// `ile_sprite0 N` variants: "sprites/sig/s0.spr" -> "sprites/sig/s2.spr" (cshell 0x1000440c).
pub fn variant(path:&str,index:usize)->String {
    let path=key(path);
    match path.strip_suffix("0.spr") {Some(stem)=>format!("{stem}{index}.spr"),None=>path}
}
/// Streak/impact-spark direction: `(r1-r2)/8, r3/16-r4/32, (r5-r6)/8` (cshell 0x1002c870).
pub fn debris_direction()->Vec3 {
    let r=|n:f32|(rnd()*n).floor();
    Vec3::new((r(8.0)-r(8.0))/8.0,r(16.0)/16.0-r(16.0)/32.0,(r(8.0)-r(8.0))/8.0).try_normalize().unwrap_or(Vec3::Y)
}
/// Sprite alpha of a fading effect: it loses dt/lifetime per frame from `start`.
pub fn faded(start:f32,age:f32,life:f32)->f32 {(start-age/life.max(0.001)).clamp(0.0,1.0)}
/// Dead-NPC pool (cshell 0x100543b0): scale `base*(1+sqrt(age-4))`, growth stops at 14 s.
pub fn pool_scale(base:f32,age:f32)->f32 {base*(1.0+(age.min(14.0)-4.0).max(0.0).sqrt())}
/// Full quad size in native units.
pub fn quad_size(texture:Vec2,scale:Vec2)->Vec2 {texture*scale*2.0}
/// Sprite animation frame at `age`.
pub fn frame_at(age:f32,fps:f32,count:usize)->usize {((age*fps) as usize).min(count.saturating_sub(1))}

#[derive(Clone)] pub struct SpriteAsset {pub frames:Arc<[Handle<Image>]>,pub fps:f32,pub size:Vec2}
#[derive(Clone)] pub struct ModelAsset {pub parts:Arc<[(Handle<Mesh>,Handle<StandardMaterial>)]>}
pub struct Library {pub quad:Handle<Mesh>,pub sprites:BTreeMap<String,SpriteAsset>,pub models:BTreeMap<String,ModelAsset>}
impl Library {
    pub fn sprite(&self,path:&str)->Option<&SpriteAsset> {self.sprites.get(&key(path))}
    pub fn model(&self,path:&str)->Option<&ModelAsset> {self.models.get(&key(path))}
}

#[derive(Clone,Copy,PartialEq)] pub enum Blend {Add,Alpha,Multiply}
#[derive(Clone,Copy)] pub enum Orient {Billboard,Streak,Fixed(Quat)}
#[derive(Clone)] pub struct Bounce {pub restitution:f32,pub sound:Option<&'static str>,rested:bool}
#[derive(Component,Clone)] pub struct Particle {
    quad:Handle<Mesh>,sprite:Option<SpriteAsset>,model:Option<ModelAsset>,
    pub pos:Vec3,pub vel:Vec3,pub gravity:f32,pub age:f32,pub life:f32,pub delay:f32,
    pub scale:Vec2,pub growth:f32,pub alpha:f32,pub fade:bool,pub blend:Blend,pub orient:Orient,pub view:bool,
    pub color:[f32;3],pub follow:Option<String>,pub bounce:Option<Bounce>,pub spin:Vec3,pub rot:Quat,pub pool:Option<f32>,pub leaves_pool:bool,
    frame:usize,shown_alpha:f32,visible:bool,
}
impl Particle {
    fn new(quad:Handle<Mesh>,pos:Vec3)->Self {
        Self {quad,sprite:None,model:None,pos,vel:Vec3::ZERO,gravity:0.0,age:0.0,life:1.0,delay:0.0,scale:Vec2::ONE,growth:0.0,alpha:1.0,fade:false,
            blend:Blend::Add,orient:Orient::Billboard,view:false,color:[1.0;3],follow:None,bounce:None,spin:Vec3::ZERO,rot:Quat::IDENTITY,pool:None,leaves_pool:false,frame:usize::MAX,shown_alpha:-1.0,visible:false}
    }
    /// A missing sprite export yields `None`: effects degrade to nothing instead of panicking.
    pub fn sprite(lib:&Library,path:&str,pos:Vec3)->Option<Self> {
        let sprite=lib.sprite(path)?.clone();let mut particle=Self::new(lib.quad.clone(),pos);particle.sprite=Some(sprite);Some(particle)
    }
    pub fn model(lib:&Library,path:&str,pos:Vec3)->Option<Self> {
        let model=lib.model(path)?.clone();let mut particle=Self::new(lib.quad.clone(),pos);particle.model=Some(model);Some(particle)
    }
    pub fn scale(mut self,scale:f32)->Self {self.scale=Vec2::splat(scale);self}
    pub fn scale2(mut self,x:f32,y:f32)->Self {self.scale=Vec2::new(x,y);self}
    pub fn life(mut self,life:f32)->Self {self.life=life;self}
    pub fn delay(mut self,delay:f32)->Self {self.delay=delay;self}
    pub fn vel(mut self,vel:Vec3)->Self {self.vel=vel;self}
    pub fn gravity(mut self,gravity:f32)->Self {self.gravity=gravity;self}
    pub fn growth(mut self,growth:f32)->Self {self.growth=growth;self}
    pub fn alpha(mut self,alpha:f32)->Self {self.alpha=alpha;self}
    pub fn fade(mut self)->Self {self.fade=true;self}
    pub fn normal_alpha(mut self)->Self {self.blend=Blend::Alpha;self}
    /// Multiplicative sprite (flags2 bit 4, D3D ZERO / SRCCOLOR, Lithtech.exe 0x53d5b0): the smoke of the grenade blast.
    pub fn multiply(mut self)->Self {self.blend=Blend::Multiply;self}
    /// Constant spin about Y in rad/s (Spawn arg 8, rotated about the world Y axis 0x10b39870 = (0,1,0) by 0x10052f10).
    pub fn spinning(mut self,rate:f32)->Self {self.spin=Vec3::new(0.0,rate,0.0);self}
    pub fn streak(mut self)->Self {self.orient=Orient::Streak;self}
    pub fn facing(mut self,normal:Vec3)->Self {self.orient=Orient::Fixed(Quat::from_rotation_arc(Vec3::Z,normal)*Quat::from_rotation_z(between(0.0,std::f32::consts::TAU)));self}
    pub fn flat(mut self,rotation:Quat)->Self {self.orient=Orient::Fixed(rotation);self}
    /// SetObjectColor rgb (the cigarette smoke is (0.1, 0.15, 0.2), cshell 0x10045097 call setup).
    pub fn color(mut self,color:[f32;3])->Self {self.color=color;self}
    pub fn in_view(mut self)->Self {self.view=true;self}
    pub fn following(mut self,socket:&str)->Self {self.follow=Some(socket.into());self}
    pub fn bouncing(mut self,restitution:f32,sound:Option<&'static str>)->Self {self.bounce=Some(Bounce {restitution,sound,rested:false});self}
    pub fn tumbling(mut self,spin:f32)->Self {
        self.rot=Quat::from_euler(EulerRot::XYZ,between(0.0,6.28),between(0.0,6.28),between(0.0,6.28));
        self.spin=Vec3::new(signed(spin),signed(spin),signed(spin));self
    }
    pub fn pooling(mut self,base:f32)->Self {self.pool=Some(base);self}
    pub fn leaving_pool(mut self)->Self {self.leaves_pool=true;self}
}
/// Queues the particle; its mesh/material are built next frame by `realize`.
pub fn spawn(commands:&mut Commands,particle:Option<Particle>)->Option<Entity> {
    let particle=particle?;let view=particle.view;
    let mut entity=commands.spawn((WorldGeometry,particle,Transform::default(),Visibility::Hidden));
    if view {entity.insert(RenderLayers::layer(3));}
    Some(entity.id())
}
#[derive(Component)] pub struct FxLight {pub remaining:f32}
/// Short point light (muzzle 0.07 s r192, impact 0.1 s r32); intensity scales like the level lights (lighting.rs).
pub fn light(commands:&mut Commands,pos:Vec3,color:[f32;3],radius:f32,life:f32) {
    commands.spawn((WorldGeometry,FxLight {remaining:life},PointLight {color:Color::srgb(color[0],color[1],color[2]),intensity:30000.0,range:radius*SCALE,shadows_enabled:false,..default()},Transform::from_translation(pos*SCALE)));
}

/// sRGB transfer curve (gamma value -> linear light), for the multipliers of additive sprites.
fn to_linear(value:f32)->f32 {if value<=0.04045 {value/12.92}else{((value+0.055)/1.055).powf(2.4)}}
fn tint(blend:Blend,alpha:f32,color:[f32;3])->Color {
    // Additive sprites dim through alpha (Bevy premultiplies), normal ones through SetObjectColor-style (a,a,a,a).
    // Direct3D adds the gamma values `tex * colour` (and `tex * a` while fading) to the frame buffer; the linear pipeline adds linear light, so the multipliers go through the
    // sRGB curve to land on the same displayed value over black (docs/retail-visual.md "Additive effects").
    if blend==Blend::Add {return Color::linear_rgba(to_linear(color[0]),to_linear(color[1]),to_linear(color[2]),to_linear(alpha));}
    match blend {Blend::Add=>Color::linear_rgba(color[0],color[1],color[2],alpha),Blend::Alpha=>Color::linear_rgba(alpha*color[0],alpha*color[1],alpha*color[2],alpha),Blend::Multiply=>Color::linear_rgba(color[0],color[1],color[2],1.0)}
}
fn to_bevy(v:retail_movement::Vec3)->Vec3 {Vec3::new(v.x,v.y,v.z)}
fn to_native(v:Vec3)->retail_movement::Vec3 {retail_movement::Vec3::new(v.x,v.y,v.z)}

/// Builds the render components of freshly spawned particles.
pub fn realize(mut commands:Commands,mut materials:ResMut<Assets<StandardMaterial>>,fresh:Query<(Entity,&Particle),Added<Particle>>) {
    for (entity,particle) in &fresh {
        let layers=particle.view.then(||RenderLayers::layer(3)).unwrap_or_default();
        if let Some(sprite)=&particle.sprite {
            let mode=match particle.blend {Blend::Add=>AlphaMode::Add,Blend::Alpha=>AlphaMode::Blend,Blend::Multiply=>AlphaMode::Multiply};
            let material=materials.add(StandardMaterial {base_color:tint(particle.blend,particle.alpha,particle.color),base_color_texture:sprite.frames.first().cloned(),unlit:true,cull_mode:None,alpha_mode:mode,fog_enabled:false,..default()});
            commands.entity(entity).insert((Mesh3d(particle.quad.clone()),MeshMaterial3d(material),layers));
        } else if let Some(model) = &particle.model {
            commands.entity(entity).with_children(|parent| {
                for (mesh,material) in model.parts.iter() {parent.spawn((Mesh3d(mesh.clone()),MeshMaterial3d(material.clone()),Transform::IDENTITY,layers.clone()));}
            });
        }
    }
}

fn orientation(particle:&Particle,camera:&Transform)->Quat {
    match particle.orient {
        Orient::Fixed(rotation)=>rotation,
        Orient::Billboard=>if particle.view {Quat::IDENTITY}else{camera.rotation},
        Orient::Streak=>{
            let up=particle.vel.try_normalize().unwrap_or(Vec3::Y);
            let eye=if particle.view {Vec3::ZERO}else{camera.translation/SCALE};
            let right=up.cross(eye-particle.pos).try_normalize().unwrap_or(camera.rotation*Vec3::X);
            Quat::from_mat3(&Mat3::from_cols(right,up,right.cross(up)))
        }
    }
}

/// Ages, moves, fades and draws every particle. Frozen while the game is paused.
#[allow(clippy::too_many_arguments)]
pub fn tick(mut commands:Commands,session:Res<Session>,time:Res<Time>,walking:Option<Res<crate::Walking>>,assets:Res<AssetServer>,mut materials:ResMut<Assets<StandardMaterial>>,
    camera:Single<&Transform,(With<InspectionCamera>,Without<Particle>,Without<FxLight>)>,library:Option<Res<crate::gunfire::Effects>>,
    mut lights:Query<(Entity,&mut FxLight)>,
    mut particles:Query<(Entity,&mut Particle,&mut Transform,&mut Visibility,Option<&MeshMaterial3d<StandardMaterial>>),Without<InspectionCamera>>) {
    if session.paused {return;}
    let dt=time.delta_secs().min(0.05);
    for (entity,mut light) in &mut lights {light.remaining-=dt;if light.remaining<=0.0 {commands.entity(entity).despawn();}}
    for (entity,mut p,mut transform,mut visibility,material) in &mut particles {
        p.age+=dt;
        if p.age>=p.life {
            if p.leaves_pool {if let (Some(walking),Some(library))=(&walking,&library) {
                if let Some((distance,normal))=walking.world.raycast(to_native(p.pos),to_native(Vec3::NEG_Y),120.0) {
                    if normal.y>0.5 {crate::gunfire::floor_pool(&mut commands,library,p.pos-Vec3::Y*distance.min(120.0),30.0,0.03);}
                }
            }}
            commands.entity(entity).despawn();continue;
        }
        let shown=p.age>=p.delay;
        if p.vel!=Vec3::ZERO || p.gravity!=0.0 {
            p.vel.y-=p.gravity*dt;
            let step=p.vel*dt;
            let mut position=p.pos+step;
            if let (Some(walking),true)=(&walking,p.bounce.as_ref().is_some_and(|b|!b.rested)) {
                let length=step.length();
                if length>0.0001 {if let Some((distance,normal))=walking.world.raycast(to_native(p.pos),to_native(step/length),length+0.25) {
                    let normal=to_bevy(normal);let speed=p.vel.length();
                    position=p.pos+step/length*(distance-0.2).max(0.0);
                    let restitution=p.bounce.as_ref().map_or(0.0,|b|b.restitution);
                    let along=p.vel.dot(normal);p.vel=(p.vel-normal*along)*0.6-normal*along*restitution;
                    if let Some(path)=p.bounce.as_ref().and_then(|b|b.sound).filter(|_|speed>60.0) {crate::sound::play_at(&mut commands,&assets,path,position,640.0);}
                    if p.vel.length()<24.0 {p.vel=Vec3::ZERO;p.gravity=0.0;p.spin=Vec3::ZERO;if let Some(b)=p.bounce.as_mut() {b.rested=true;}}
                }}
            }
            p.pos=position;
        }
        if p.spin!=Vec3::ZERO {let rotation=Quat::from_scaled_axis(p.spin*dt);p.rot=rotation*p.rot;}
        if p.growth!=0.0 {let factor=1.0+p.growth*dt;p.scale*=factor;}
        let scale=match p.pool {Some(base)=>Vec2::splat(pool_scale(base,p.age)),None=>p.scale};
        let alpha=if p.fade {faded(p.alpha,p.age,p.life)}else{p.alpha};
        if let (Some(sprite),Some(material))=(p.sprite.clone(),material) {
            let frame=frame_at(p.age,sprite.fps,sprite.frames.len());
            if frame!=p.frame || (alpha-p.shown_alpha).abs()>0.004 {
                if let Some(target)=materials.get_mut(&material.0) {target.base_color=tint(p.blend,alpha,p.color);if let Some(image)=sprite.frames.get(frame) {target.base_color_texture=Some(image.clone());}}
                p.frame=frame;p.shown_alpha=alpha;
            }
            let size=quad_size(sprite.size,scale)*SCALE;
            // The level image is shown flipped (mirror.rs): a quad of the level camera is mirrored too so its art reads left to right; view-space quads are not.
            *transform=Transform {translation:p.pos*SCALE,rotation:orientation(&p,&camera),scale:Vec3::new(size.x*if p.view {1.0}else{crate::mirror::QUAD_HAND},size.y,1.0)};
        } else {
            *transform=Transform {translation:p.pos*SCALE,rotation:p.rot,scale:Vec3::splat(scale.x*SCALE)};
        }
        if shown!=p.visible {p.visible=shown;*visibility=if shown {Visibility::Inherited}else{Visibility::Hidden};}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sprite_quad_is_twice_texture_times_scale() {
        // cshell 0x53da90: corners at pos +- right*texW*s +- up*texH*s; smuga.spr 64px at scale (0.12,40) is 15.36 x 5120 units.
        assert_eq!(quad_size(Vec2::new(256.0,256.0),Vec2::splat(0.05)),Vec2::splat(25.6));
        let tracer=quad_size(Vec2::splat(64.0),Vec2::new(0.12,40.0));
        assert!((tracer.x-15.36).abs()<1e-4 && (tracer.y-5120.0).abs()<1e-3);
    }
    /// Mirror audit: a camera-facing sprite's texture reads left to right on the displayed image (U grows towards `mirror::right`).
    #[test]
    fn billboard_art_is_not_mirrored_on_the_displayed_image() {
        let camera=Transform::from_xyz(1.0,2.0,3.0).with_rotation(Quat::from_rotation_y(0.9));
        let quad=Transform {rotation:orientation(&{let mut p=Particle::new(Handle::default(),Vec3::ZERO);p.orient=Orient::Billboard;p},&camera),scale:Vec3::new(2.0*crate::mirror::QUAD_HAND,2.0,1.0),..default()};
        let u_axis=quad.transform_point(Vec3::new(0.5,0.0,0.0))-quad.transform_point(Vec3::new(-0.5,0.0,0.0));
        assert!(u_axis.normalize().dot(crate::mirror::right(&camera))>0.999,"the right edge of the texture must lie on the displayed right");
    }
    #[test]
    fn ile_sprite_variants_replace_the_trailing_zero() {
        assert_eq!(variant("sprites\\sig\\s0.spr",2),"sprites/sig/s2.spr");
        assert_eq!(variant("sprites\\weapons\\ingram0.spr",1),"sprites/weapons/ingram1.spr");
        assert_eq!(variant("sprites\\glock\\main.spr",2),"sprites/glock/main.spr");
    }
    #[test]
    fn fading_effect_loses_dt_over_lifetime_and_pool_grows_after_four_seconds() {
        assert_eq!(faded(1.0,0.0,0.5),1.0);assert!((faded(1.0,0.25,0.5)-0.5).abs()<1e-6);assert_eq!(faded(0.8,1.0,1.0),0.0);
        assert!((faded(0.8,0.5,1.0)-0.3).abs()<1e-6);
        assert_eq!(pool_scale(0.04,3.0),0.04);assert!((pool_scale(0.04,13.0)-0.04*4.0).abs()<1e-6);assert_eq!(pool_scale(0.04,50.0),pool_scale(0.04,14.0));
    }
    #[test]
    fn frames_advance_at_the_sprite_rate_and_hold_the_last() {
        assert_eq!(frame_at(0.0,15.0,11),0);assert_eq!(frame_at(0.2,15.0,11),3);assert_eq!(frame_at(9.0,15.0,11),10);assert_eq!(frame_at(1.0,15.0,0),0);
    }
    #[test]
    fn debris_direction_is_a_unit_vector() {
        for _ in 0..50 {assert!((debris_direction().length()-1.0).abs()<1e-4);}
        let r=rnd();assert!((0.0..1.0).contains(&r));
    }
}
