//! What the options change in the running game. Each switch names the retail global it stands for (cshell.dll data 0x100b247c..0x100b2499);
//! docs/retail-menus.md lists which ones the remake can act on and which are stored only.
use bevy::{prelude::*,asset::uuid_handle,core_pipeline::{core_2d::graph::{Core2d,Node2d},fullscreen_material::{FullscreenMaterial,FullscreenMaterialPlugin}},
    render::{extract_component::ExtractComponent,render_graph::{InternedRenderLabel,InternedRenderSubGraph,RenderLabel,RenderSubGraph},render_resource::ShaderType},shader::{Shader,ShaderRef},window::{MonitorSelection,WindowMode}};
use bevy::ui_render::graph::NodeUi;
use std::sync::atomic::{AtomicU8,Ordering};
use crate::{ViewerConfig,options::Options,npcs::NpcRoster,models::{Insignificant,hidden_by_level},keys_cfg::HUD_COLORS};

/// The HUD colour index the slider picked; panels and the HUD read it through [`hud_rgb`] (retail default 7, amber).
static HUD_INDEX:AtomicU8=AtomicU8::new(7);
pub fn hud_rgb()->[f32;3] {let c=HUD_COLORS[(HUD_INDEX.load(Ordering::Relaxed) as usize).min(7)];[c[0]/255.0,c[1]/255.0,c[2]/255.0]}
/// The display ramp of the retail engine (Lithtech.exe 0x4f6ed0 SetGammaRamp, re-applied whenever GammaR/G/B change, 0x4f82a0): `out = in^(1/gamma)` per channel with
/// gamma = the keys.cfg value (shipped 1.15, cshell 0x1005a21d copies it to gammar/gammag/gammab; the engine's own default is 1.0 = no ramp). The ramp lives in the display hardware,
/// so screen recordings (the reference videos in research/retail-reference/) show the picture BEFORE it: raw texture/lightmap values (measured: menu frame x0.96, street sky 22/36/33
/// for the fog colour 24/37/33). The remake applies the ramp itself, so the slider value is the retail value and 1.0 is the identity. docs/retail-visual.md "Display gamma".
pub const RETAIL_GAMMA:f32=1.0;
/// `ColorGrading`-style gamma of the game cameras for a slider value: the retail engine writes gammar/gammag/gammab = value (0x1005a21d).
pub fn grading_gamma(slider:f32)->f32 {(slider/RETAIL_GAMMA).clamp(0.5,2.0)}

/// Detail switches that act on entities and resources every frame.
pub fn apply(options:Res<Options>,mut roster:ResMut<NpcRoster>,mut props:Query<(&Insignificant,&mut Visibility)>,mut effects:ResMut<crate::gunfire::Effects>) {
    // "Insignificant characters" (0x100b2498): off hides the extras (prisoners, rats, cockroaches, newspaper).
    for actor in &mut roster.actors {let hidden=!options.keys.insignificant_characters && actor.insignificant();if actor.hidden!=hidden {actor.hidden=hidden;}}
    // "Insignificant objects" (0x100b2494): level 1 hides the props marked 1, level 2 every marked prop (0x1002d0b0).
    for (level,mut visibility) in &mut props {
        let wanted=if hidden_by_level(options.keys.insignificant_objects,level.0) {Visibility::Hidden}else{Visibility::Inherited};
        if *visibility!=wanted {*visibility=wanted;}
    }
    if effects.debris!=options.keys.shot_debris {effects.debris=options.keys.shot_debris;}
    // "HD textúrák": read by every texture load, so it takes effect for levels and models loaded from now on.
    level_viewer::hd::set_enabled(options.hd_textures || level_viewer::hd::forced_by_env());
    // The HUD reads the palette through hud::hud_rgb() every frame (bars, frames, digits, notices).
    HUD_INDEX.store(options.keys.hud_color.clamp(0,7) as u8,Ordering::Relaxed);
}
/// The gamma slider is a full-screen pass after everything the 2D camera draws (the retail engine writes a display gamma ramp, so the
/// menu and the HUD change with it). Bevy's tone mapping stage cannot do it: it is skipped for `Tonemapping::None`.
#[derive(Component,ExtractComponent,Clone,Copy,ShaderType,Default)] pub struct GammaEffect {pub gamma:f32}
#[derive(Debug,Hash,PartialEq,Eq,Clone,RenderLabel)] struct GammaLabel;
const GAMMA_SHADER:Handle<Shader>=uuid_handle!("6f2c1d3e-8b1a-4b3a-9a11-2d5f0c9c7a10");
const GAMMA_WGSL:&str="#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;
struct GammaEffect { gamma: f32 }
@group(0) @binding(2) var<uniform> settings: GammaEffect;
fn to_gamma(c: vec3<f32>) -> vec3<f32> { return select(1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3<f32>(0.0031308)); }
fn to_linear(c: vec3<f32>) -> vec3<f32> { return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045)); }
@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    // The display ramp acts on the stored (gamma) values; the target re-encodes the linear value we return.
    let c = textureSample(screen_texture, texture_sampler, in.uv);
    let ramped = pow(clamp(to_gamma(max(c.rgb, vec3<f32>(0.0))), vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(1.0 / settings.gamma));
    return vec4<f32>(to_linear(ramped), c.a);
}";
impl FullscreenMaterial for GammaEffect {
    fn fragment_shader()->ShaderRef {ShaderRef::Handle(GAMMA_SHADER)}
    fn node_edges()->Vec<InternedRenderLabel> {vec![NodeUi::UiPass.intern(),GammaLabel.intern(),Node2d::Upscaling.intern()]}
    fn sub_graph()->Option<InternedRenderSubGraph> {Some(Core2d.intern())}
    fn node_label()->impl RenderLabel {GammaLabel}
}
pub struct GammaPlugin;
impl Plugin for GammaPlugin {
    fn build(&self,app:&mut App) {
        let _=app.world_mut().resource_mut::<Assets<Shader>>().insert(GAMMA_SHADER.id(),Shader::from_wgsl(GAMMA_WGSL,"gamma_effect.wgsl"));
        app.add_plugins(FullscreenMaterialPlugin::<GammaEffect>::default());
    }
}
pub fn gamma(options:Res<Options>,mut commands:Commands,mut cameras:Query<(Entity,Option<&mut GammaEffect>),(With<Camera2d>,Without<crate::mirror::Composite>)>) {
    let gamma=grading_gamma(options.keys.gamma);
    for (entity,effect) in &mut cameras {
        match effect {
            Some(mut effect)=>{if effect.gamma!=gamma {effect.gamma=gamma;}},
            None=>{commands.entity(entity).insert(GammaEffect {gamma});},
        }
    }
}
/// "Alcímek" off (0x100b247d) also blanks the cutscene captions.
pub fn subtitles(options:Res<Options>,mut captions:Query<&mut crate::retail_ui::BitmapText,With<crate::opening::SubtitleLine>>) {
    if options.keys.subtitles {return;}
    for mut text in &mut captions {text.set("");}
}
/// Window mode and size from `windowed` / `screenwidth` / `screenheight`. Full screen is the borderless desktop mode (the export
/// keeps the windowed size for the next start); capture runs keep their fixed window.
pub fn video(options:Res<Options>,config:Res<ViewerConfig>,mut window:Single<&mut Window>,mut applied:Local<Option<(bool,[u32;2])>>) {
    if config.capture.is_some() {return;}
    let want=(options.windowed,options.screen);
    if *applied==Some(want) {return;}
    *applied=Some(want);
    if options.windowed {window.mode=WindowMode::Windowed;window.resolution.set(options.screen[0] as f32,options.screen[1] as f32);}
    else{window.mode=WindowMode::BorderlessFullscreen(MonitorSelection::Current);}
}
/// `MaxFPS`: sleeps away the rest of the frame budget (0 = unlimited).
pub fn frame_limit(options:Res<Options>,config:Res<ViewerConfig>,mut last:Local<Option<std::time::Instant>>) {
    let now=std::time::Instant::now();
    if let (Some(previous),true)=(*last,options.max_fps>0 && config.capture.is_none()) {
        let budget=std::time::Duration::from_secs_f64(1.0/options.max_fps as f64);let spent=now.duration_since(previous);
        if spent<budget {std::thread::sleep(budget-spent);}
    }
    *last=Some(std::time::Instant::now());
}
/// `MESTER_FPS_LOG=1`: one line "FPS n (ms per frame)" per two seconds of real time (perf measurements, see docs/perf-notes.md).
pub fn frame_log(time:Res<Time<Real>>,mut spent:Local<(f32,u32)>,mut on:Local<Option<bool>>) {
    if !*on.get_or_insert_with(||std::env::var_os("MESTER_FPS_LOG").is_some()) {return;}
    spent.0+=time.delta_secs();spent.1+=1;
    if spent.0>=2.0 {info!("FPS {:.1} ({:.1} ms per frame)",spent.1 as f32/spent.0,1000.0*spent.0/spent.1 as f32);*spent=(0.0,0);}
}
#[derive(Component)] pub struct FramerateText;
pub fn framerate_setup(mut commands:Commands,assets:Res<AssetServer>) {
    commands.spawn((FramerateText,Text::new(""),TextFont {font:assets.load("hud/subtitles.ttf"),font_size:16.0,..default()},TextColor(Color::srgb(0.9,0.9,0.4)),TextShadow {offset:Vec2::splat(1.0),color:Color::srgba(0.0,0.0,0.0,0.9)},GlobalZIndex(600),
        Node {position_type:PositionType::Absolute,right:px(10),top:px(6),..default()}));
}
/// `showframerate` (0 off, otherwise the counter of the retail engine).
pub fn framerate(options:Res<Options>,time:Res<Time<Real>>,mut text:Single<&mut Text,With<FramerateText>>,mut smoothed:Local<f32>) {
    if options.show_framerate==0 {if !text.0.is_empty() {text.0.clear();}return;}
    let fps=1.0/time.delta_secs().max(1e-4);*smoothed=if *smoothed==0.0 {fps}else{*smoothed*0.95+fps*0.05};
    text.0=format!("{:.0} fps",*smoothed);
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn the_slider_is_the_retail_display_ramp_exponent() {
        // The shipped keys.cfg 1.15 is applied as pow(x, 1/1.15) like Lithtech.exe 0x4f6ed0; the slider end 1.0 is the identity ramp.
        assert_eq!(grading_gamma(1.15),1.15);assert_eq!(grading_gamma(1.0),1.0);assert_eq!(grading_gamma(2.0),2.0);
        assert_eq!(hud_rgb(),[208.0/255.0,115.0/255.0,16.0/255.0]);
    }
}
