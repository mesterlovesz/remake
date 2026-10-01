//! Playable reconstruction: imported retail assets and a portable controller.
mod hud;
mod input_probe;
mod models;
mod props;
mod props_probe;
mod opening;
mod endgame;
mod death;
mod walk_probe;
mod settings;
mod gunfire;
mod fx;
mod sound;
mod spatial;
mod travel;
mod pickups;
mod campaign;
mod dialogue;
mod campaign_probe;
mod npcs;
mod ai_probe;
mod doors;
mod retail_state;
mod retail_weapons;
mod retail_probe;
mod frontend;
mod lighting;
mod birds;
mod activation;
mod decorations;
mod view;
mod weapons_alt;
mod audio;
mod audio_trace;
mod output;
mod music;
mod video;
mod cursor;
mod retail_ui;
mod inventory;
mod character;
mod panels;
mod panel_probe;
mod probe_kit;
mod start_view;
mod tour;
mod retail_world;
mod mirror;
mod move_probe;
mod sound_probe;
mod keys_cfg;
mod options;
mod savefile;
mod menu_layout;
mod menu;
mod menu_probe;
mod option_effects;

use std::{fs, path::PathBuf};

use bevy::{
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use retail_movement::{CollisionWorld, Input as MoveInput, Player, Vec3 as NativeVec3};

const SCALE: f32 = 0.01;
pub(crate) const EDITION: &str = "Mesterlövész Újraírva";

#[derive(Resource)]
struct ViewerConfig {
    output: PathBuf,
    /// Where the player's own files live (keys.cfg, autoexec.cfg, settings.json, save\*.sav): the export folder, or MESTER_USER_DIR.
    user: PathBuf,
    /// Silent capture and test runs write no user files unless MESTER_USER_DIR redirects them to a scratch folder.
    persist: bool,
    world: String,
    capture: Option<String>,
    capture_times:Vec<f32>,
    story:bool,
}

#[derive(Resource)]
struct Walking { world:CollisionWorld, player:Player, yaw:f32, pitch:f32, footstep:usize }

#[derive(Component)]
struct InspectionCamera;
#[derive(Component)]
struct WorldGeometry;
#[derive(bevy::ecs::system::SystemParam)]
struct Probes<'w> {campaign:Res<'w,campaign_probe::Probe>,retail:Res<'w,retail_probe::Probe>,front:Res<'w,frontend::Frontend>,walk:Res<'w,walk_probe::Probe>}

fn main() -> AppExit {
    let arguments: Vec<String> = std::env::args().collect();
    let selection = arguments.get(1).cloned().unwrap_or_else(|| "menu".to_owned());
    let story=selection=="story";
    let main_menu=selection=="menu";
    let world=if story || main_menu {"rh1-wiezienie1".to_owned()}else{selection};
    let output = arguments.get(2).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("../../output"));
    let output = fs::canonicalize(output).expect("export output directory does not exist");
    let user_dir=std::env::var_os("MESTER_USER_DIR").map(PathBuf::from);
    let persist=user_dir.is_some() || !(arguments.get(3).is_some() || std::env::var_os("MESTER_SILENT").is_some());
    let user=user_dir.unwrap_or_else(||output.clone());
    let world=travel::exported_stem(&output,&world).unwrap_or(world);
    // Optional HD textures (lib.rs `hd`): the saved option (not in unpersisted capture runs) or MESTER_HD_TEXTURES=1, before the first level loads.
    level_viewer::hd::set_root(&output);
    level_viewer::hd::set_enabled(level_viewer::hd::forced_by_env() || (persist && options::RetailOptions::load(&user).hd_textures));
    let obj = output.join(format!("{world}.visual.obj"));
    assert!(obj.is_file(), "visual OBJ missing: {}", obj.display());
    App::new()
        // Capture runs are always silent, including when a saved volume is nonzero.
        .insert_resource(bevy::audio::GlobalVolume::new(bevy::audio::Volume::Linear(
            if arguments.get(3).is_some() || std::env::var_os("MESTER_SILENT").is_some() {0.0}else{1.0})))
        .insert_resource(ViewerConfig { output: output.clone(), user, persist, world: world.clone(), capture:arguments.get(3).cloned(),capture_times:capture_times(arguments.get(4).map(String::as_str)),story })
        .insert_resource(frontend::Frontend::new(main_menu))
        // Scenario (probe) runs step the game clock by a fixed amount per frame: reproducible whatever the machine does.
        .insert_resource(probe_kit::time_strategy(&arguments))
        .insert_resource(probe_kit::winit_settings(&arguments))
        .add_plugins(DefaultPlugins
            .set(AssetPlugin { file_path: output.to_string_lossy().into_owned(), ..default() })
            .set(WindowPlugin { primary_window: Some(Window {
                // The retail window title is scripts\app_name.txt (Lithtech.exe 0x404f3d); the edition name follows it. The 64x64 sniper.ico is not applied (bevy_winit does not expose winit::Icon and the crate has no winit dependency).
                title: format!("A mesterlövész v 2.33 / {EDITION} / {world}"),
                // Capture aid: MESTER_WINDOW=1024x768 renders a headless run at that size (default 1280x720).
                resolution: std::env::var("MESTER_WINDOW").ok().and_then(|size|{let (w,h)=size.split_once('x')?;Some((w.parse::<u32>().ok()?,h.parse::<u32>().ok()?))}).unwrap_or((1280,720)).into(),
                // Capture aid: MESTER_NOVSYNC=1 lifts the 60 Hz cap so frame times measure the real cost.
                present_mode: if std::env::var_os("MESTER_NOVSYNC").is_some() {bevy::window::PresentMode::AutoNoVsync}else{default()},
                // Capture/test runs must not pop up or steal focus while the user works.
                visible: arguments.get(3).is_none(),
                focused: arguments.get(3).is_none(),
                ..default()
            }), ..default() }).disable::<bevy::audio::AudioPlugin>())
        .add_plugins(output::Output)
        .add_plugins(spatial::SpatialPlugin)
        .add_plugins(option_effects::GammaPlugin)
        .add_plugins(gunfire::GunfirePlugin)
        .add_plugins(audio::RetailAudio)
        .add_plugins(retail_world::RetailWorldPlugin)
        .add_plugins(mirror::MirrorPlugin)
        .add_plugins(audio_trace::AudioTrace)
        .init_resource::<retail_ui::TextAid>()
        .init_resource::<travel::Travel>()
        .init_resource::<doors::DoorUse>()
        .init_resource::<activation::Activation>()
        .init_resource::<npcs::NpcRoster>()
        .init_resource::<pickups::DropQueue>()
        .init_resource::<panels::Panels>()
        .init_resource::<character::Feedback>()
        .init_resource::<models::PropAnimations>()
        .init_resource::<props::PropWorld>()
        .init_resource::<props::SkinClock>()
        .init_resource::<view::ViewState>()
        .insert_resource(tour::Tour::from_env())
        .init_resource::<move_probe::Probe>()
        .add_systems(Startup, (settings::setup,cursor::setup,retail_ui::setup,panels::setup,campaign::init,dialogue::setup,retail_weapons::setup,setup,hud::setup,opening::setup,gunfire::setup,campaign_probe::setup,retail_probe::setup,weapons_alt::setup,weapons_alt::probe_setup,panel_probe::setup,frontend::setup,menu::setup,(menu_probe::setup,ai_probe::setup,sound_probe::setup,option_effects::framerate_setup),video::setup).chain())
        .add_systems(Last, option_effects::frame_limit)
        .add_systems(Startup, (endgame::setup,walk_probe::setup))
        .add_systems(Update, endgame::tick.after(opening::tick).before(fx::realize))
        .add_systems(Update, death::tick.after(campaign::tick))
        .add_systems(Update, walk_probe::tick.after(campaign_probe::tick).before(move_camera))
        .add_systems(Update, birds::fly.after(models::animate_props))
        .add_systems(Update, tour::tick.before(move_camera))
        .add_systems(Update, tour::god.after(campaign::tick).before(hud::health))
        .add_systems(Update, tour::pick.after(view::update))
        .add_systems(Update, move_probe::tick.after(weapons_alt::probe).before(move_camera))
        .add_systems(Update, (decorations::update,decorations::animate).after(move_camera))
        .add_systems(Update, (video::tick,frontend::probe,menu_probe::tick,menu::input,frontend::interact_pages,panel_probe::tick,input_probe::tick,ai_probe::tick,(panels::input,settings::controls,travel::request,gunfire::input,campaign_probe::tick,retail_probe::tick,weapons_alt::probe,sound_probe::tick,move_camera,view::update,lighting::sync,npcs::block_player,opening::triggers,opening::tick,(option_effects::subtitles,opening::subtitle_fit,option_effects::apply),models::animate_props,doors::tick,activation::use_objects,npcs::tick,npcs::respawn_emitters).chain(),(character::tick,activation::tick,activation::capture,(dialogue::probe,dialogue::input,campaign::tick,dialogue::present).chain(),pickups::spawn_drops,pickups::fly,pickups::tick,retail_weapons::tick,retail_weapons::projectiles,retail_weapons::explosions,retail_weapons::present,hud::update,hud::health,campaign::hint,(menu::quick,option_effects::gamma,option_effects::video,option_effects::framerate,option_effects::frame_log,retail_ui::aid_sync,hud::killed_count),settings::update,panels::present,panels::tick_notices,retail_ui::draw_text,capture).chain(),frontend::update,menu::render,menu::ghosts,frontend::char_page).chain())
        .add_systems(Update, (props::tick,props::animate_skins,props::register_debris).chain().after(retail_weapons::projectiles))
        .add_systems(Update, props_probe::tick.after(campaign_probe::tick).before(retail_probe::tick))
        .add_systems(Update, (doors::npc_open,doors::auto_close).after(doors::tick).before(activation::tick))
        .add_systems(Update, (weapons_alt::tick,weapons_alt::melee,weapons_alt::laser,weapons_alt::flashlight,weapons_alt::overlay).chain().after(hud::update).before(fx::realize).before(capture))
        .run()
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut world_materials: ResMut<Assets<retail_world::RetailWorld>>,
    mut mips: ResMut<retail_world::MipQueue>,
    asset_server: Res<AssetServer>,
    config: Res<ViewerConfig>,
    frontend:Res<frontend::Frontend>,
    mut campaign:ResMut<campaign::Campaign>,
    session:Res<settings::Session>,
    mut view:ResMut<view::ViewState>,
    old:Query<Entity,Or<(With<WorldGeometry>,With<InspectionCamera>)>>,
    prop_world:Res<props::PropWorld>,
) {
    for entity in &old {commands.entity(entity).despawn();}
    let scene = fs::read_to_string(config.output.join(format!("{}.scene.json", config.world))).expect("read scene JSON");
    let scene: serde_json::Value = serde_json::from_str(&scene).expect("parse scene JSON");
    let fog=retail_world::WorldFog::from_scene(&scene);
    commands.insert_resource(fog);
    retail_world::spawn_visual(&config.world,(AlphaMode::Opaque,Color::WHITE),&config,&fog,&mut commands,&mut meshes,&mut world_materials,&mut mips,&asset_server);
    lighting::setup_world(&mut commands,&scene);
    commands.insert_resource(activation::Activation::from_scene(&scene,&fs::read_to_string(config.output.join("decoded_scripts/objects.txt")).unwrap_or_default()));
    let named_path=config.output.join(format!("{}.render_models.json",config.world));
    if named_path.exists() {
        let names:Vec<String>=serde_json::from_str(&fs::read_to_string(named_path).unwrap()).unwrap();
        for name in names {
            let obj=scene["objects"].as_array().unwrap().iter().find(|o|o["properties"]["Name"]==name);
            let alpha=world_material(obj);
            let model_fog=if obj.is_some_and(|o|o["kind"]=="DemoSkyWorldModel") {fog.for_sky()}else{fog};
            let entities=retail_world::spawn_visual(&format!("world_models/{}/{name}",config.world),alpha,&config,&model_fog,&mut commands,&mut meshes,&mut world_materials,&mut mips,&asset_server);
            if let Some(object)=obj {decorations::attach(&name,object,&entities,&mut commands);doors::attach(&config,&name,object,entities,&mut commands);}
        }
    }
    models::spawn_props(&config,&mut commands,&mut meshes,&mut materials,&asset_server,&prop_world);
    decorations::setup_world(&mut commands,&mut meshes,&mut materials,&asset_server,&config,&scene);
    let start = scene["objects"].as_array().and_then(|objects| objects.iter().find(|o| o["kind"] == "StartPoint"));
    let coordinates = start.and_then(|o| o["properties"]["Pos"].as_array());
    let position = if let Some(saved)=campaign.saved_player() {Vec3::from_array(saved.position)*SCALE}else if let Some(coords) = coordinates {
        Vec3::new(coords[0].as_f64().unwrap_or(0.0) as f32,coords[1].as_f64().unwrap_or(0.0) as f32,coords[2].as_f64().unwrap_or(0.0) as f32)*SCALE
    // object.lto 0x10011443: without a StartPoint the server uses (800, 128, 448).
    } else { Vec3::new(800.0,128.0,448.0)*SCALE };
    // LithTech cameras face local +Z; Bevy cameras face local -Z.
    let yaw = campaign.saved_player().map(|saved|saved.yaw).unwrap_or_else(||start_yaw(start));
    // Debug/capture aid: MESTER_SPAWN="x,y,z,yaw" in native units and radians.
    let spawn:Option<Vec<f32>>=std::env::var("MESTER_SPAWN").ok().map(|s|s.split(',').filter_map(|v|v.trim().parse().ok()).collect()).filter(|v:&Vec<f32>|v.len()==4);
    let (position,yaw)=spawn.map_or((position,yaw),|v|(Vec3::new(v[0],v[1],v[2])*SCALE,v[3]));
    // Debug/capture aid: MESTER_ALCOHOL=0..100 starts the level drunk.
    if let Some(alcohol)=std::env::var("MESTER_ALCOHOL").ok().and_then(|a|a.parse().ok()) {campaign.alcohol=f32::max(alcohol,0.0);}
    // Debug/capture aid: MESTER_DEATH_FRAME=N kills the player on frame N of the level.
    view.debug_death=std::env::var("MESTER_DEATH_FRAME").ok().and_then(|f|f.parse().ok());
    let source=fs::read_to_string(config.output.join(format!("{}.collision.obj",config.world))).expect("read collision OBJ");
    let mut world=CollisionWorld::from_obj(&source).expect("build collision BVH");
    world.set_surface_flags(collision_surface_flags(&scene));
    let mut player=Player::new(NativeVec3::new(position.x/SCALE,position.y/SCALE,position.z/SCALE));
    if frontend.character_started {player.max_stamina=frontend.draft.stamina as f32;player.stamina=player.max_stamina;}
    if let Some(saved)=campaign.saved_player() {player.max_stamina=saved.max_stamina;player.stamina=saved.stamina.min(saved.max_stamina);}
    world.place_player(&mut player);
    // MESTER_STEP_CLAMP_QUIRK=1: retail's 4 units per frame before the first crouch (frame-rate dependent, see docs/retail-movement-audit.md).
    player.initial_clamp_quirk=std::env::var_os("MESTER_STEP_CLAMP_QUIRK").is_some();
    commands.insert_resource(Walking {world,player,yaw,pitch:campaign.saved_player().map_or(0.0,|saved|saved.pitch),footstep:0});
    view.enter_level();
    let mut camera=commands.spawn((Camera3d::default(),Camera {clear_color:bevy::camera::ClearColorConfig::Custom(fog.clear_color()),..default()},Projection::custom(view::RetailProjection::new(view::retail_fov(session.preferences.fov),0.1)),
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        Transform::from_translation(position).with_rotation(Quat::from_rotation_y(yaw)),
        AmbientLight {brightness:38.0,..default()},InspectionCamera));
    if let Some(fog)=source_fog(&scene) {camera.insert(fog);}
    commands.run_system_cached(npcs::setup_world);
    commands.run_system_cached(pickups::setup_world);
    commands.run_system_cached(campaign::load_world);
    commands.run_system_cached(activation::restore_aux);
    info!("Pálya: {}. WASD: járás, Shift: futás, Q: futás be/ki, Ctrl: guggolás, Szóköz: ugrás, E: használat, jobb egér: célzás, Esc: menü.", config.world);
}

/// DAT surface flags per collision.obj face, from scene.json `collision_polygons` (`NotAStep` refuses stair steps, Lithtech.exe 0x416c60).
fn collision_surface_flags(scene:&serde_json::Value)->Vec<u32> {
    let mut flags=vec![0u32;scene["collision_faces"].as_u64().unwrap_or(0) as usize];
    for polygon in scene["collision_polygons"].as_array().into_iter().flatten() {
        let (first,count,value)=(polygon["first_face"].as_u64().unwrap_or(0) as usize,polygon["face_count"].as_u64().unwrap_or(0) as usize,polygon["surface_flags"].as_u64().unwrap_or(0) as u32);
        for flag in flags.iter_mut().skip(first).take(count) {*flag=value;}
    }
    flags
}

/// The retail player faces the StartPoint's `Kierunek`, not its Rotation: the server sends polnoc/wschod/poludnie/zachod as 0..3
/// (object.lto 0x100114a3..0x10011548) and the shell turns that into yaw 0, pi/2, pi, 3pi/2 (cshell 0x1005b73a); +pi for Bevy.
fn start_yaw(start:Option<&serde_json::Value>)->f32 {
    let quarter=match start.and_then(|o|o["properties"]["Kierunek"].as_str()).map(str::to_ascii_lowercase).as_deref() {Some("wschod")=>1.0,Some("poludnie")=>2.0,Some("zachod")=>3.0,_=>0.0};
    quarter*std::f32::consts::FRAC_PI_2+std::f32::consts::PI
}

fn source_fog(scene:&serde_json::Value)->Option<DistanceFog> {
    let properties=scene["objects"].as_array()?.iter().find(|o|o["kind"]=="WorldProperties")?.get("properties")?;
    if properties["Mgla_Wlaczona"].as_u64()!=Some(1) {return None;}
    let rgb=properties["Kolor_Mgly"].as_array()?;
    let channel=|index:usize|rgb[index].as_f64().unwrap_or(0.0) as f32/255.0;
    let start=properties["Start_Mgly"].as_f64()? as f32*SCALE;
    let end=properties["Koniec_Mgly"].as_f64()? as f32*SCALE;
    if end<=start {return None;}
    Some(DistanceFog {color:Color::srgb(channel(0),channel(1),channel(2)),falloff:FogFalloff::Linear {start,end},..default()})
}

fn world_material(object:Option<&serde_json::Value>)->(AlphaMode,Color) {
    let Some(object)=object else {return (AlphaMode::Opaque,Color::WHITE);};
    let properties=&object["properties"];
    let alpha=properties["Alpha"].as_f64().or_else(||properties["Alfa"].as_f64()).unwrap_or(1.0) as f32;
    // Uzyj_kolor tints the brush with Kolor / 256 (object.lto 0x10004db2: SetObjectColor(Kolor * 0.00390625, Alpha)).
    let tint=if properties["Uzyj_kolor"].as_i64()==Some(1) {properties["Kolor"].as_array().filter(|c|c.len()>=3).map(|c|Color::srgb(c[0].as_f64().unwrap_or(256.0) as f32/256.0,c[1].as_f64().unwrap_or(256.0) as f32/256.0,c[2].as_f64().unwrap_or(256.0) as f32/256.0))}else{None}.unwrap_or(Color::WHITE);
    let kind=object["kind"].as_str().unwrap_or("");
    let mode=if properties["Additive"].as_u64()==Some(1) {AlphaMode::Add}
        else if kind.starts_with("b_transparent") || kind=="b_szuflada_przestrzelna" || alpha<1.0 {AlphaMode::Blend}else{AlphaMode::Opaque};
    (mode,tint.with_alpha(alpha))
}

fn exported_material(value:Option<&serde_json::Value>,fallback:(AlphaMode,Color))->(AlphaMode,Color) {
    let Some(value)=value else{return fallback};
    let mode=match value["alpha_mode"].as_str() {
        Some("blend")=>AlphaMode::Blend,Some("add")=>AlphaMode::Add,
        Some("mask")=>AlphaMode::Mask(value["alpha_cutoff"].as_f64().unwrap_or(0.5) as f32),
        Some("opaque")=>AlphaMode::Opaque,_=>return fallback,
    };
    (mode,Color::WHITE.with_alpha(value["alpha_factor"].as_f64().unwrap_or(fallback.1.alpha() as f64) as f32))
}

/// Padded door atlases keep their art in `[0,content_u)`. Without the display mirror `read_obj` gives `1-u`, so the mirror must be
/// `content_u-u` or a strip of the flat pad colour lands on the leaf (the maroon edge on the prison doors); with the mirrored display U is
/// authored as is and already inside the art.
fn mirror_padded_u(mut uvs:Vec<[f32;2]>,source:Option<&serde_json::Value>)->Vec<[f32;2]> {
    let content=source.and_then(|data|data["content_u"].as_f64()).unwrap_or(1.0) as f32;
    if content<1.0 && !level_viewer::DISPLAY_MIRRORED {for uv in &mut uvs {uv[0]-=1.0-content;}}
    uvs
}

#[cfg(test)]
mod presentation_tests {
    use super::*;

    #[test]
    fn light_cone_keeps_retail_translucency() {
        let object=serde_json::json!({"kind":"b_transparent","properties":{"Alpha":0.09,"Additive":0,"Uzyj_kolor":0}});
        let (mode,color)=world_material(Some(&object));
        assert_eq!(mode,AlphaMode::Blend);
        assert!((color.alpha()-0.09).abs()<0.00001);
    }

    #[test]
    fn perforated_gate_uses_pixel_alpha_even_at_full_object_opacity() {
        let object=serde_json::json!({"kind":"b_szuflada_przestrzelna","properties":{"Alfa":1.0}});
        assert_eq!(world_material(Some(&object)).0,AlphaMode::Blend);
        let solid=serde_json::json!({"kind":"b_door","properties":{}});
        assert_eq!(world_material(Some(&solid)).0,AlphaMode::Opaque);
    }

    #[test]
    fn sidecar_alpha_is_not_multiplied_twice() {
        let data=serde_json::json!({"alpha_mode":"blend","alpha_factor":0.09});
        let (mode,color)=exported_material(Some(&data),(AlphaMode::Blend,Color::WHITE.with_alpha(0.09)));
        assert_eq!(mode,AlphaMode::Blend);
        assert!((color.alpha()-0.09).abs()<0.00001);
    }
    #[test]
    fn padded_door_atlas_mirrors_inside_its_art() {
        // b_door4 front face: retail U runs 0..0.75 over the 96-texel art; without the display mirror read_obj gives 1-u.
        let data=serde_json::json!({"content_u":0.75});
        let uvs=mirror_padded_u(vec![[1.0,0.0],[0.25,0.0]],Some(&data));
        if level_viewer::DISPLAY_MIRRORED {assert_eq!(uvs,vec![[1.0,0.0],[0.25,0.0]]);}
        else {assert!((uvs[0][0]-0.75).abs()<1e-6 && uvs[1][0].abs()<1e-6);}
        assert_eq!(mirror_padded_u(vec![[0.3,0.1]],None),vec![[0.3,0.1]]);
    }
    #[test]
    fn collision_faces_carry_their_polygon_surface_flags() {
        let scene=serde_json::json!({"collision_faces":5,"collision_polygons":[{"first_face":0,"face_count":2,"surface_flags":1},{"first_face":2,"face_count":3,"surface_flags":4194305}]});
        let flags=collision_surface_flags(&scene);
        assert_eq!(flags,vec![1,1,4194305,4194305,4194305]);
        assert!(flags[2] & retail_movement::SURF_NOTASTEP!=0 && flags[1] & retail_movement::SURF_NOTASTEP==0);
    }
    #[test]
    fn start_point_direction_sets_the_yaw_and_ignores_the_rotation() {
        let facing=|kierunek:&str|start_yaw(Some(&serde_json::json!({"properties":{"Kierunek":kierunek,"Rotation":[0.0,2.6,0.0,0.0]}})));
        let forward=|yaw:f32|Quat::from_rotation_y(yaw)*Vec3::NEG_Z;
        // polnoc is +Z, wschod +X, poludnie -Z, zachod -X (LithTech yaw 0 faces +Z and turns towards +X).
        assert!((forward(facing("polnoc"))-Vec3::Z).length()<1e-5 && (forward(facing("wschod"))-Vec3::X).length()<1e-5);
        assert!((forward(facing("poludnie"))+Vec3::Z).length()<1e-5 && (forward(facing("zachod"))+Vec3::X).length()<1e-5);
        assert!((forward(start_yaw(None))-Vec3::Z).length()<1e-5);
    }
    /// The 10 levels whose start view changed: the camera looks along the compass heading of the table (`start_view::STARTS`), and the displayed
    /// right of that view is LithTech's right, (fz,-fx) for a left-handed +X right / +Z forward frame.
    #[test]
    fn start_views_of_the_changed_levels_face_their_compass_heading() {
        for (world,_,kierunek) in start_view::STARTS {
            let camera=Transform::from_rotation(Quat::from_rotation_y(start_yaw(Some(&serde_json::json!({"properties":{"Kierunek":kierunek}})))));
            let (look,right)=(camera.rotation*Vec3::NEG_Z,mirror::right(&camera));
            let heading=start_view::forward(kierunek);
            assert!((Vec2::new(look.x,look.z)-heading).length()<1e-5,"{world} {kierunek}");
            if level_viewer::DISPLAY_MIRRORED {assert!((Vec2::new(right.x,right.z)-Vec2::new(heading.y,-heading.x)).length()<1e-5,"{world} right");}
        }
    }
    /// Retail video frame (research/retail-reference/retail-video-wiezienie1-cell-view.png, subtitle Fight02T at o_marker_dialog1): standing at the
    /// marker (80,768) and looking at the hall through the bars, the cell guard o_postac14 (-80,672) is on the RIGHT of the displayed image.
    #[test]
    fn cell_guard_is_on_the_displayed_right_at_the_marker() {
        let (marker,guard)=(Vec3::new(80.0,13.0,768.0),Vec3::new(-80.0,-3.0,672.0));
        for yaw in [0.3f32,0.7,0.9] {
            let camera=Transform::from_translation(marker).with_rotation(Quat::from_rotation_y(yaw));
            let (to,look)=(guard-marker,camera.rotation*Vec3::NEG_Z);
            assert!(to.dot(look)>0.0,"guard in front at yaw {yaw}");
            let lt_right=Vec2::new(look.z,-look.x);
            assert!(Vec2::new(to.x,to.z).dot(lt_right)>0.0,"LithTech: guard right of the view at yaw {yaw}");
            if level_viewer::DISPLAY_MIRRORED {assert!(to.dot(mirror::right(&camera))>0.0,"displayed image: guard on the right at yaw {yaw}");}
        }
    }
    /// The table equals the exported StartPoint of each level (needs the local export: `cargo test -- --ignored`).
    #[test]
    #[ignore]
    fn start_table_matches_the_export() {
        for (world,pos,kierunek) in start_view::STARTS {
            let scene:serde_json::Value=serde_json::from_str(&fs::read_to_string(format!("../../output/{world}.scene.json")).unwrap()).unwrap();
            let start=scene["objects"].as_array().unwrap().iter().find(|o|o["kind"]=="StartPoint").unwrap();
            let p=&start["properties"];
            assert_eq!(p["Kierunek"].as_str(),Some(kierunek),"{world}");
            for i in 0..3 {assert!((p["Pos"][i].as_f64().unwrap() as f32-pos[i]).abs()<1e-3,"{world} pos {i}");}
        }
    }
}

fn move_camera(
    cursor: Single<&CursorOptions>,
    mouse: Res<AccumulatedMouseMotion>,
    bind: options::Bindings,
    time: Res<Time>,
    mut walking:ResMut<Walking>,
    panels:Res<panels::Panels>,
    assets:Res<AssetServer>,
    mut commands:Commands,
    opening:Res<opening::Opening>,
    session:Res<settings::Session>,
    probes:Probes,
    mut view:ResMut<view::ViewState>,
    campaign:Res<campaign::Campaign>,
    mut was_looking:Local<bool>,
    camera:Single<&Transform,(With<InspectionCamera>,Without<retail_weapons::NativeCamera>)>,
) {
    // Death halts the mission (dialogue_active) but its camera and physics keep running without input.
    let dead=campaign.health<=0.0;
    if opening.active || session.paused || (session.dialogue_active && !dead) { *was_looking=false; return; }
    let look=!dead && (cursor.grab_mode!=CursorGrabMode::None || probes.campaign.active || probes.retail.active || probes.front.probe_active || probes.walk.active);
    // Retail keeps the walking keys live under the inventory / attribute screens; only the mouse look is replaced by the cursor (0x10054c98).
    let active=look || (!dead && panels.mouse_mode());
    // The frame that takes the mouse back (panel closed, menu resumed) carries the motion of the free cursor: it never turns the view.
    // Retail CInvertMouse flips the pitch sign (0x10054d0e); the sensitivity slider is keys.cfg's mouse value (0x100b2488).
    let delta=if bind.options.invert_mouse {Vec2::new(mouse.delta.x,-mouse.delta.y)}else{mouse.delta};
    if look && *was_looking {(walking.yaw,walking.pitch)=view::dialogue_look(walking.yaw,walking.pitch,delta,bind.options.keys.mouse_sensitivity,view.mouse_scale(),session.dialogue_choices,bind.options.retail_choice_mouse);}
    *was_looking=look;
    if active && bind.just_pressed(keys_cfg::cmd::TOGGLE_RUN) {view.run_toggle=!view.run_toggle;}
    let held=|command:usize| active && bind.pressed(command);
    let [forward,back,left,right]=[keys_cfg::cmd::FORWARD,keys_cfg::cmd::BACKWARD,keys_cfg::cmd::STEP_LEFT,keys_cfg::cmd::STEP_RIGHT].map(held);
    // While the dialogue choice list is up the arrow keys move its highlight (dialogue.rs) instead of walking; W / S and the rest still walk.
    let skip:&[KeyCode]=if session.dialogue_list {&[KeyCode::ArrowUp,KeyCode::ArrowDown]}else{&[]};
    let [forward,back]=[(forward,keys_cfg::cmd::FORWARD),(back,keys_cfg::cmd::BACKWARD)].map(|(down,command)|down && (skip.is_empty() || bind.pressed_without(command,skip)));
    let axis=|plus:bool,minus:bool| plus as i32 as f32-minus as i32 as f32;
    // Movement follows the camera object's rotation of the previous frame, not the raw mouse yaw (cshell 0x10060222 reads
    // GetObjectRotation of the camera at controller+0x4c), so a drunk view wobble steers the walk too.
    let heading={let f=camera.forward();(-f.x).atan2(-f.z)};
    let input=MoveInput {forward:axis(forward,back),right:mirror::SIGN*axis(right,left),yaw:heading,
        run:held(keys_cfg::cmd::RUN)!=view.run_toggle,crouch:held(keys_cfg::cmd::CROUCH),
        // Jump follows the table (owner's default: Space); retail refuses the jump while the inventory is open (0x10060ba8).
        jump:held(keys_cfg::cmd::JUMP) && !panels.inventory};
    let Walking {world,player,..}=&mut *walking;
    // A dead player neither moves nor falls: cshell 0x10060f5f only runs the death camera once the controller is dead.
    player.full_stamina=bind.options.full_stamina;
    if dead {player.events=retail_movement::Events::default();player.footstep=false;} else {player.tick(world,&input,time.delta_secs().min(0.05));}
    let ground=player.grounded;
    view.intent=view::Intent {moving:ground && (forward||back||left||right),strafe_left:ground && left,strafe_right:ground && right,run:input.run};
    if walking.player.footstep {
        walking.footstep+=1;
        audio::player_step(&mut commands,&assets,walking.footstep);
    }
}

fn capture_times(argument:Option<&str>)->Vec<f32> {
    let mut times:Vec<f32>=argument.unwrap_or("3").split(',').filter_map(|s|s.parse::<f32>().ok()).filter(|t|t.is_finite() && *t>=0.0).collect();
    times.sort_by(f32::total_cmp);times.dedup();
    if times.is_empty() {times.push(3.0);}
    times
}

fn capture(mut commands:Commands, config:Res<ViewerConfig>, time:Res<Time>, real:Res<Time<Real>>, mut next:Local<usize>, mut ended:Local<Option<f32>>, mut exit:MessageWriter<AppExit>, frames:Res<bevy::diagnostic::FrameCount>, walk:Res<walk_probe::Probe>, status:probe_kit::ProbeStatus) {
    use probe_kit::Verdict;
    if let Some(path)=&config.capture {
        let shot=|commands:&mut Commands,target:PathBuf| {commands.spawn(bevy::render::view::screenshot::Screenshot::primary_window()).observe(bevy::render::view::screenshot::save_to_disk(target));};
        let last=*config.capture_times.last().unwrap();
        if let Some(&at)=config.capture_times.get(*next) {
            if time.elapsed_secs()>at {
                let target=if config.capture_times.len()==1 {PathBuf::from(path)}else {
                    let base=PathBuf::from(path);
                    base.with_file_name(format!("{}-{at:06.2}.png",base.file_stem().unwrap().to_string_lossy()))
                };
                shot(&mut commands,target);
                *next+=1;
            }
        }
        // The walk-through probe ends the app itself once its levels are done (or its time is up).
        if walk.active {
            if walk.finished {if let Some(reason)=&walk.failure {error!("Végigjátszó próba: {reason}");exit.write(AppExit::error());}else{exit.write(AppExit::Success);}}
            else if time.elapsed_secs()>walk.timeout {error!("Végigjátszó próba: időtúllépés");exit.write(AppExit::error());}
            return;
        }
        let verdict=status.verdict();
        if !matches!(verdict,Verdict::Idle|Verdict::Running) && ended.is_none() {info!("PROBE VERDICT after {} frames, game time {:.1} s",frames.0,time.elapsed_secs());}
        match verdict {
            Verdict::Failed(name,why)=>{error!("{name} sikertelen: {why:?}");exit.write(AppExit::error());},
            // A probe decides when the run ends; the capture times are only the earliest exit.
            Verdict::Running=>if time.elapsed_secs()>last+probe_kit::HARD_LIMIT {error!("A próba nem fejeződött be {} s alatt.",last+probe_kit::HARD_LIMIT);exit.write(AppExit::error());},
            Verdict::Passed=>{
                // The finished probe leaves one more picture (`<name>-end.png`), then the run ends once it was written.
                let at=*ended.get_or_insert_with(||{let base=PathBuf::from(path);shot(&mut commands,base.with_file_name(format!("{}-end.png",base.file_stem().unwrap().to_string_lossy())));real.elapsed_secs()});
                if *next>=config.capture_times.len() && real.elapsed_secs()>at+3.0 {exit.write(AppExit::Success);}
            },
            Verdict::Idle=>if time.elapsed_secs()>last+3.0 {exit.write(AppExit::Success);},
        }
    }
}
