//! Retail handedness. LithTech is left-handed (+X right, +Z forward); Bevy is right-handed. The exported world keeps LithTech's
//! numbers, so a Bevy camera facing LithTech's forward direction sees +X on its LEFT: every frame was the horizontal mirror of the
//! retail view (levels swapped left and right, model textures and text mirrored). The level cameras therefore render into an
//! offscreen image and a window camera shows that image flipped, which is exactly what LithTech's own left-handed projection
//! displays; world numbers, physics and AI stay untouched. Input follows the displayed image: mouse yaw, strafe, roll and the
//! blood direction use `SIGN`. The weapon camera and the HUD draw straight to the window (the view models are already authored
//! for a right-handed camera). `level_viewer::DISPLAY_MIRRORED` is the single switch (also read by the OBJ reader for U).
use bevy::{camera::{visibility::RenderLayers,RenderTarget,ClearColorConfig},prelude::*,render::render_resource::{Extent3d,TextureFormat},window::PrimaryWindow};

pub const MIRRORED:bool=level_viewer::DISPLAY_MIRRORED;
/// -1 when the displayed image is flipped: multiplies every screen-relative left/right sign of the first-person view.
pub const SIGN:f32=if MIRRORED {-1.0}else{1.0};
/// LithTech's right (the right of the displayed image) as a world direction of a level camera: Bevy's own right is the displayed left.
pub fn right(camera:&Transform)->Vec3 {camera.rotation*Vec3::X*SIGN}
/// A point of the unflipped weapon camera's view space (x right on the window) as a world point of the level camera it belongs to.
pub fn to_world(camera:&Transform,view:Vec3)->Vec3 {camera.translation+camera.rotation*(view*Vec3::new(SIGN,1.0,1.0))}
/// Scale.x of a camera-facing quad drawn by a level camera: its texture reads left to right on the displayed image only when the quad is mirrored too.
pub const QUAD_HAND:f32=SIGN;
const COMPOSITE_LAYER:usize=30;
const OFFSCREEN_ORDER:isize=-10;

#[derive(Resource)] pub struct MirrorTarget(pub Handle<Image>);
#[derive(Component)] pub struct Composite;
#[derive(Component)] struct Redirected;

pub struct MirrorPlugin;
impl Plugin for MirrorPlugin {
    fn build(&self,app:&mut App) {
        if !MIRRORED {return;}
        app.add_systems(Startup,setup).add_systems(Update,(redirect,resize).chain());
    }
}

fn target_size(window:&Window)->Extent3d {Extent3d {width:window.physical_width().max(1),height:window.physical_height().max(1),depth_or_array_layers:1}}

fn setup(mut commands:Commands,mut images:ResMut<Assets<Image>>,window:Single<&Window,With<PrimaryWindow>>) {
    let size=target_size(&window);
    let image=images.add(Image::new_target_texture(size.width,size.height,TextureFormat::bevy_default(),None));
    commands.insert_resource(MirrorTarget(image.clone()));
    commands.spawn((Composite,Camera2d,Camera {order:0,clear_color:ClearColorConfig::Custom(Color::BLACK),..default()},RenderLayers::layer(COMPOSITE_LAYER)));
    commands.spawn((Composite,Sprite {image,flip_x:true,custom_size:Some(Vec2::new(window.width(),window.height())),..default()},RenderLayers::layer(COMPOSITE_LAYER)));
}

/// The level cameras (main and sky world) draw into the offscreen image, before the composite camera (order 0), the weapon and the HUD.
fn redirect(mut commands:Commands,target:Res<MirrorTarget>,mut cameras:Query<(Entity,&mut Camera),(Or<(With<crate::InspectionCamera>,With<crate::decorations::SkyCamera>)>,Without<Redirected>)>,sky:Query<(),With<crate::decorations::SkyCamera>>) {
    for (entity,mut camera) in &mut cameras {
        camera.order=OFFSCREEN_ORDER-if sky.contains(entity) {1}else{0};
        commands.entity(entity).insert((RenderTarget::Image(target.0.clone().into()),Redirected));
    }
}

fn resize(target:Option<Res<MirrorTarget>>,window:Single<&Window,With<PrimaryWindow>>,mut images:ResMut<Assets<Image>>,mut sprite:Query<&mut Sprite,With<Composite>>) {
    let Some(target)=target else {return};
    let size=target_size(&window);
    if let Some(image)=images.get(&target.0) {
        if image.texture_descriptor.size!=size {if let Some(image)=images.get_mut(&target.0) {image.resize(size);}}
    }
    for mut sprite in &mut sprite {sprite.custom_size=Some(Vec2::new(window.width(),window.height()));}
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn weapon_side_is_the_displayed_right_in_the_world() {
        // The camera faces LithTech +Z (yaw pi): the displayed right is +X, the weapon camera's +x must land on it.
        let camera=Transform::from_xyz(1.0,2.0,3.0).with_rotation(Quat::from_rotation_y(std::f32::consts::PI));
        let (right,at)=(right(&camera),to_world(&camera,Vec3::new(0.5,0.0,-1.0)));
        if MIRRORED {assert!((right-Vec3::X).length()<1e-5 && (at-Vec3::new(1.5,2.0,4.0)).length()<1e-5);}
        else {assert!((right+Vec3::X).length()<1e-5 && (at-Vec3::new(0.5,2.0,4.0)).length()<1e-5);}
    }
}
