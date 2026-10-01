//! Retail world surfaces: lightmaps, gamma-space shading, table fog, far clip, texture pan and mip chains.
//! Evidence and formulas: docs/retail-visual.md (LithTech render blocks, Lithtech.exe/cshell.dll addresses).
use bevy::{
    asset::{embedded_asset, RenderAssetUsages},
    image::{ImageAddressMode, ImageFilterMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor},
    mesh::{MeshVertexBufferLayoutRef, PrimitiveTopology},
    pbr::{Material, MaterialPipeline, MaterialPipelineKey, MaterialPlugin},
    prelude::*,
    render::render_resource::{AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError},
    shader::ShaderRef,
};
use level_viewer::read_obj_lightmapped;
use std::{collections::BTreeMap, fs};
use crate::{ViewerConfig, SCALE};

/// WorldProperties as the client shell applies them (cshell 0x1005b460): FarZ = Widocznosc, FogEnable/FogNearZ/FogFarZ =
/// Mgla_Wlaczona/Start_Mgly/Koniec_Mgly, FogR/G/B = Kolor_Mgly. The same colour clears the screen (cshell 0x1005a8e0 calls
/// ClearScreen(0, 3, &fog colour) every frame). A level without WorldProperties keeps the engine defaults: FarZ 10000
/// (Lithtech.exe cvar), no fog, black clear colour.
#[derive(Resource, Clone, Copy, Debug)]
pub struct WorldFog {
    pub enabled: bool,
    pub color: [f32; 3],
    /// Render units.
    pub start: f32,
    pub end: f32,
    pub far: f32,
    /// SkyFogEnable/NearZ/FarZ (Mgla_Nieba, Start/Koniec_Mgly_Nieba): the fog of the sky world, which the level fog does not touch.
    pub sky: (bool, f32, f32),
}

impl Default for WorldFog {
    fn default() -> Self { Self { enabled: false, color: [0.0; 3], start: 0.0, end: 0.0, far: 10000.0 * SCALE, sky: (false, 0.0, 0.0) } }
}

impl WorldFog {
    pub fn from_scene(scene: &serde_json::Value) -> Self {
        let Some(properties) = scene["objects"].as_array().and_then(|objects| objects.iter().find(|o| o["kind"] == "WorldProperties")).map(|o| &o["properties"]) else { return Self::default() };
        let number = |key: &str, fallback: f64| properties[key].as_f64().unwrap_or(fallback) as f32;
        Self {
            enabled: properties["Mgla_Wlaczona"].as_u64() == Some(1),
            color: [0, 1, 2].map(|i| properties["Kolor_Mgly"][i].as_f64().unwrap_or(0.0) as f32 / 255.0),
            start: number("Start_Mgly", 0.0) * SCALE,
            end: number("Koniec_Mgly", 0.0) * SCALE,
            far: number("Widocznosc", 10000.0) * SCALE,
            sky: (properties["Mgla_Nieba"].as_u64() == Some(1), number("Start_Mgly_Nieba", 0.0) * SCALE, number("Koniec_Mgly_Nieba", 0.0) * SCALE),
        }
    }
    /// The fog the sky world brushes are drawn with (SkyFarZ defaults to 10000).
    pub fn for_sky(&self) -> Self { Self { enabled: self.sky.0, start: self.sky.1, end: self.sky.2, far: 10000.0 * SCALE, ..*self } }
    pub fn clear_color(&self) -> Color { Color::srgb(self.color[0], self.color[1], self.color[2]) }
}

#[derive(Clone, Copy, Debug, ShaderType)]
pub struct RetailParams { surface: Vec4, fog_color: Vec4, fog_range: Vec4, pan: Vec4, effect: Vec4, group: Vec4 }

/// One world surface (texture + shading kind) drawn like the retail render block shaders.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct RetailWorld {
    #[uniform(0)] pub params: RetailParams,
    #[texture(1)] #[sampler(2)] pub base: Option<Handle<Image>>,
    #[texture(3)] #[sampler(4)] pub lightmap: Option<Handle<Image>>,
    /// Second texture of a DTX `EnvMap` / `EnvMapAlpha` / `DetailTex` command (see `EffectKind`).
    #[texture(5)] #[sampler(6)] pub effect: Option<Handle<Image>>,
    /// Light group intensity atlas (same layout as the lightmap atlas), see `WorldLightGroup`.
    #[texture(7)] #[sampler(8)] pub group_atlas: Option<Handle<Image>>,
    pub alpha_mode: AlphaMode,
}

/// Bit 0: lightmap (else vertex colour); bit 1: the texture alpha masks fullbright pixels over the lit surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shading { Gouraud = 0, Lightmap = 1, GouraudFullbright = 2, LightmapFullbright = 3 }

/// The texture effect a DTX header command gives a world surface (Lithtech.exe `AllocShader` 0x4bf830..0x4bfa8a, PreFlush 0x526b80 / 0x52cbe0 / 0x525f90).
/// The shader picks the pass by the section's shader: `EnvMapAlpha` only draws on Gouraud sections (its lightmapped variant falls back to the plain
/// Lightmap_Texture, so glass and elevator doors with it are NOT reflective in retail); `EnvMap` draws on both.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EffectKind { None = 0, EnvMap = 1, EnvMapAlpha = 2, Detail = 3 }

/// `env_effects.json` entry of a base texture -> the effect the section gets (`shader` is the section's source shader, 1 Gouraud, 4 lightmapped).
pub fn effect_for(entry: &serde_json::Value, shader: u64) -> (EffectKind, f32) {
    let file = entry["texture"].as_str().is_some();
    match (entry["kind"].as_str(), shader) {
        (Some("envmap"), 1 | 4) if file => (EffectKind::EnvMap, 0.0),
        (Some("envmapalpha"), 1) if file => (EffectKind::EnvMapAlpha, 0.0),
        // DetailTextureScale cvar 0.2 times the DTX header scale (0x525e90).
        (Some("detail"), 1 | 4) if file => (EffectKind::Detail, 0.2 * entry["scale"].as_f64().unwrap_or(0.0) as f32),
        _ => (EffectKind::None, 0.0),
    }
}

impl RetailWorld {
    pub fn new(fog: &WorldFog, shading: Shading, alpha_mode: AlphaMode, alpha: f32, pan: Vec2, base: Option<Handle<Image>>, lightmap: Option<Handle<Image>>, effect: (EffectKind, f32, Option<Handle<Image>>), group_atlas: Option<Handle<Image>>) -> Self {
        let cutoff = if let AlphaMode::Mask(cutoff) = alpha_mode { cutoff } else { 0.0 };
        let additive = matches!(alpha_mode, AlphaMode::Add);
        Self {
            params: RetailParams {
                surface: Vec4::new(alpha, cutoff, shading as u32 as f32, additive as u32 as f32),
                fog_color: Vec4::new(fog.color[0], fog.color[1], fog.color[2], fog.enabled as u32 as f32),
                fog_range: Vec4::new(fog.start, fog.end, fog.far, matches!(alpha_mode, AlphaMode::Opaque | AlphaMode::Mask(_)) as u32 as f32),
                pan: Vec4::new(pan.x, pan.y, 0.0, 0.0),
                effect: Vec4::new(effect.0 as u32 as f32, effect.1, 0.0, 0.0),
                group: Vec4::new(0.0, 0.0, 0.0, group_atlas.is_some() as u32 as f32),
            },
            base, lightmap, effect: effect.2, group_atlas, alpha_mode,
        }
    }
}

impl Material for RetailWorld {
    fn fragment_shader() -> ShaderRef { "embedded://level_viewer/retail_world.wgsl".into() }
    fn alpha_mode(&self) -> AlphaMode { self.alpha_mode }
    fn specialize(_: &MaterialPipeline, descriptor: &mut RenderPipelineDescriptor, _: &MeshVertexBufferLayoutRef, _: MaterialPipelineKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
        // CD3D_RenderWorld::Draw sets D3DCULL_CCW: retail draws clockwise (front) faces only. The world is rendered mirrored (LithTech
        // is left-handed), which turns those into Bevy's counter-clockwise front faces. MESTER_DOUBLE_SIDED=1 restores the old both-sides look.
        if std::env::var_os("MESTER_DOUBLE_SIDED").is_none() {descriptor.primitive.cull_mode = Some(bevy::render::render_resource::Face::Back);}
        else {descriptor.primitive.cull_mode = None;}
        Ok(())
    }
}

/// World textures waiting for their mip chain. The DTX files carry mip levels; the PNG exports do not, so they are rebuilt with
/// the box filter the retail tools use. Retail draws them bilinear with the nearest mip level (Bilinear 1, Trilinear 0).
#[derive(Resource, Default)]
pub struct MipQueue(Vec<Handle<Image>>);

/// The DAT light group of the loaded world (`<stem>.visual.lightmap.json` `light_group`, only podziemia1c has one): the LightGroup object of the same
/// name (activation.rs) drives its colour, the world lightmaps hold the stored data and add `intensity * colour` (`SetLightGroupColor`, Lithtech.exe 0x516a60).
#[derive(Resource, Default, Clone, Debug)]
pub struct WorldLightGroup { pub name: Option<String> }

/// Feeds the current colour of the world's light group into every material that carries the group layer (only when it changes).
pub fn sync_light_groups(group: Res<WorldLightGroup>, activation: Res<crate::activation::Activation>, mut materials: ResMut<Assets<RetailWorld>>, mut last: Local<Option<Vec3>>) {
    let Some(name) = &group.name else { return };
    let colour = activation.group_level(name);
    if *last == Some(colour) { return; }
    *last = Some(colour);
    for (_, material) in materials.iter_mut() {
        if material.params.group.w > 0.5 { material.params.group = colour.extend(1.0); }
    }
}

/// Bevy fogs models (props, NPCs, pickups: StandardMaterial) by mixing the fog colour into the LINEAR light of the fragment, while Direct3D 8 mixes it into the shaded
/// gamma values, like the world shader here does. The pbr fog library (`bevy_pbr::fog`, three mix sites) is patched once it has loaded so that every mix converts to gamma,
/// mixes and converts back (a surface of gamma 0.35 at 50 % fog towards (24, 37, 33) is 0.225 in retail, 0.28 with Bevy's own mix). The fog amount (radial distance in
/// Bevy, view depth in retail) follows the retail view depth through `patch_model_fog_depth`.
pub fn patch_model_fog(assets: Res<AssetServer>, mut shaders: ResMut<Assets<Shader>>, mut done: Local<bool>, mut depth_done: Local<bool>) {
    // Table fog of Direct3D is linear in the view depth z (Lithtech.exe TableFog), not in the eye distance `length(view_to_world)` Bevy uses.
    if !*depth_done {
        let functions: Handle<Shader> = assets.load("embedded://bevy_pbr/render/pbr_functions.wgsl");
        if let Some(shader) = shaders.get_mut(&functions) {
            *depth_done = true;
            if let bevy::shader::Source::Wgsl(source) = &shader.source {
                let radial = "let distance = length(view_to_world);";
                if source.contains(radial) {
                    let patched = source.replace(radial, "let distance = max(-view_transformations::position_world_to_view(fragment_world_position).z, 0.0001);");
                    shader.source = bevy::shader::Source::Wgsl(patched.into());
                } else { warn!("bevy_pbr::pbr_functions changed: models keep the radial fog distance"); }
            }
        }
    }
    if *done { return; }
    let handle: Handle<Shader> = assets.load("embedded://bevy_pbr/render/fog.wgsl");
    let Some(shader) = shaders.get_mut(&handle) else { return };
    *done = true;
    let bevy::shader::Source::Wgsl(source) = &shader.source else { return };
    let mix = "return vec4<f32>(mix(input_color.rgb, fog_color.rgb, fog_color.a), input_color.a);";
    if !source.contains(mix) { warn!("bevy_pbr::fog changed: models keep Bevy's linear-light fog"); return; }
    let helpers = "
fn fog_to_gamma(c: vec3<f32>) -> vec3<f32> { return select(1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055, max(c, vec3<f32>(0.0)) * 12.92, c <= vec3<f32>(0.0031308)); }
fn fog_from_gamma(c: vec3<f32>) -> vec3<f32> { return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045)); }
fn fog_mix(input_color: vec4<f32>, fog_color: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(fog_from_gamma(mix(fog_to_gamma(input_color.rgb), fog_to_gamma(fog_color.rgb), fog_color.a)), input_color.a);
}
";
    let patched = source.replace(mix, "return fog_mix(input_color, fog_color);");
    let at = patched.find("fn scattering_adjusted_fog_color").unwrap_or(0);
    let patched = format!("{}{}{}", &patched[..at], helpers, &patched[at..]);
    shader.source = bevy::shader::Source::Wgsl(patched.into());
}

pub struct RetailWorldPlugin;
impl Plugin for RetailWorldPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "retail_world.wgsl");
        app.add_plugins(MaterialPlugin::<RetailWorld>::default()).init_resource::<MipQueue>().init_resource::<WorldFog>().init_resource::<WorldLightGroup>().add_systems(Update, (build_mips, hd_skin_mips, sync_light_groups, patch_model_fog));
    }
}

fn build_mips(mut queue: ResMut<MipQueue>, mut images: ResMut<Assets<Image>>, assets: Res<AssetServer>) {
    queue.0.retain(|handle| {
        if assets.load_state(handle.id()).is_failed() { return false; }
        let Some(image) = images.get(handle) else { return true };
        if image.texture_descriptor.mip_level_count > 1 || image.data.is_none() || image.texture_descriptor.format != bevy::render::render_resource::TextureFormat::Rgba8Unorm { return false; }
        let Some(image) = images.get_mut(handle) else { return true };
        add_mips(image);
        false
    });
}

/// HD model skins (`textures_hd/model_textures/skins/..`, 1024 px) would shimmer without a mip chain the 256 px retail skins do not need: the box-filtered chain
/// is appended when they finish loading (world textures go through `MipQueue`; the retail skins keep their single level).
fn hd_skin_mips(mut events: MessageReader<AssetEvent<Image>>, mut images: ResMut<Assets<Image>>, assets: Res<AssetServer>) {
    for event in events.read() {
        let AssetEvent::LoadedWithDependencies { id } = event else { continue };
        if !assets.get_path(*id).is_some_and(|path| path.path().starts_with("textures_hd/model_textures")) { continue; }
        let Some(image) = images.get_mut(*id) else { continue };
        if image.texture_descriptor.mip_level_count == 1 && matches!(image.texture_descriptor.format, bevy::render::render_resource::TextureFormat::Rgba8Unorm | bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb) { add_mips(image); }
    }
}

/// Box-filtered mip chain appended to an RGBA8 image (level 0 first, as wgpu expects).
pub fn add_mips(image: &mut Image) {
    let (mut width, mut height) = (image.width() as usize, image.height() as usize);
    let Some(data) = image.data.as_mut() else { return };
    if data.len() != width * height * 4 || (width == 1 && height == 1) { return; }
    let mut levels = 1;
    let (mut start, mut size) = (0usize, width * height * 4);
    while width > 1 || height > 1 {
        let (next_width, next_height) = ((width / 2).max(1), (height / 2).max(1));
        let mut next = vec![0u8; next_width * next_height * 4];
        for y in 0..next_height {
            for x in 0..next_width {
                let (x0, x1, y0, y1) = ((x * 2).min(width - 1), (x * 2 + 1).min(width - 1), (y * 2).min(height - 1), (y * 2 + 1).min(height - 1));
                for c in 0..4 {
                    let sum: u32 = [(x0, y0), (x1, y0), (x0, y1), (x1, y1)].iter().map(|&(sx, sy)| data[start + (sy * width + sx) * 4 + c] as u32).sum();
                    next[(y * next_width + x) * 4 + c] = ((sum + 2) / 4) as u8;
                }
            }
        }
        start += size;
        size = next.len();
        data.extend_from_slice(&next);
        (width, height) = (next_width, next_height);
        levels += 1;
    }
    image.texture_descriptor.mip_level_count = levels;
}

fn texture_settings(repeat: bool) -> impl Fn(&mut ImageLoaderSettings) + Send + Sync + 'static {
    move |settings: &mut ImageLoaderSettings| {
        // Direct3D shades the stored values, so the textures are read as plain unorm and converted once in the shader.
        settings.is_srgb = false;
        let address = if repeat { ImageAddressMode::Repeat } else { ImageAddressMode::ClampToEdge };
        settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: address, address_mode_v: address,
            mag_filter: ImageFilterMode::Linear, min_filter: ImageFilterMode::Linear, mipmap_filter: ImageFilterMode::Nearest,
            ..default()
        });
    }
}

/// The exported visual OBJ of a world (`stem`) or of one of its movable models, drawn with the retail shaders.
/// Returns the spawned parts.
pub fn spawn_visual(stem: &str, appearance: (AlphaMode, Color), config: &ViewerConfig, fog: &WorldFog, commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<RetailWorld>, mips: &mut MipQueue, asset_server: &AssetServer) -> Vec<Entity> {
    let mut entities = Vec::new();
    let obj = fs::read_to_string(config.output.join(format!("{stem}.visual.obj"))).expect("read visual OBJ");
    let mtl = fs::read_to_string(config.output.join(format!("{stem}.visual.mtl"))).expect("read visual MTL");
    let texture_paths: BTreeMap<String, String> = level_viewer::read_mtl(&mtl);
    let material_data: serde_json::Value = fs::read_to_string(config.output.join(format!("{stem}.visual.materials.json"))).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default();
    // Lightmap atlas and the atlas UV of every face (tools/lightmaps.py).
    let lightmap: Option<(serde_json::Value, Vec<f32>)> = fs::read_to_string(config.output.join(format!("{stem}.visual.lightmap.json"))).ok().and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok()).and_then(|info| {
        let bytes = fs::read(config.output.join(info["uvs"].as_str()?)).ok()?;
        let values: Vec<f32> = bytes.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
        (values.len() == info["faces"].as_u64()? as usize * 6).then_some((info, values))
    });
    let effects: serde_json::Value = fs::read_to_string(config.output.join("env_effects.json")).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default();
    // A world with a light group (podziemia1c) draws its lightmaps without the group and adds the group layer at the group's current colour.
    let group_info = lightmap.as_ref().map(|(info, _)| &info["light_group"]).filter(|group| group["base"].is_string());
    let group_intensity: Option<Vec<u8>> = group_info.and_then(|group| fs::read(config.output.join(group["vertices"].as_str()?)).ok());
    let group_atlas: Option<Handle<Image>> = group_info.and_then(|group| group["intensity"].as_str()).map(|path| asset_server.load_with_settings(path.to_owned(), texture_settings(false)));
    if stem == config.world { commands.insert_resource(WorldLightGroup { name: group_info.and_then(|group| group["names"][0].as_str()).map(str::to_owned) }); }
    let atlas: Option<Handle<Image>> = lightmap.as_ref().and_then(|(info, _)| group_info.and_then(|group| group["base"].as_str()).or(info["atlas"].as_str())).map(|path| asset_server.load_with_settings(path.to_owned(), texture_settings(false)));
    let parts = read_obj_lightmapped(&obj, lightmap.as_ref().map(|(_, values)| values.as_slice())).expect("parse visual OBJ");
    let mut textures: BTreeMap<String, Handle<Image>> = BTreeMap::new();
    for part in parts {
        let data = material_data.get(&part.material);
        let appearance = crate::exported_material(data, appearance);
        let broken_uv = part.invalid_uvs > 0;
        // The DAT carries inf/NaN UVs for sections whose texture the retail compiler could not find (tools/export_visual.py now writes 0 there): retail draws only the lightmap pass, so this is data, not a fault.
        if broken_uv { debug!("{}: {} gyári textúrakoordináta nem véges; textúra nélküli megjelenítés", part.material, part.invalid_uvs); }
        let lightmapped = atlas.is_some() && data.and_then(|d| d["source_shader"].as_u64()) == Some(4) && !part.lightmap_uvs.is_empty() && part.lightmap_uvs.iter().all(|uv| uv[0].is_finite() && uv[1].is_finite());
        let fullbright = data.and_then(|d| d["fullbright"].as_bool()).unwrap_or(false);
        let shading = match (lightmapped, fullbright) { (false, false) => Shading::Gouraud, (true, false) => Shading::Lightmap, (false, true) => Shading::GouraudFullbright, (true, true) => Shading::LightmapFullbright };
        // UVPan (TextureEffectGroups/UVPan.txt): the texture matrix translates by Speed * Time (X pans the other way only when read_obj mirrors U).
        let effect = data.map(|d| &d["texture_effect"]).filter(|e| e["script"] == "UVPan");
        let pan = effect.map_or(Vec2::ZERO, |e| Vec2::new((if level_viewer::DISPLAY_MIRRORED {1.0} else {-1.0}) * e["params"]["SpeedX"].as_f64().unwrap_or(0.0) as f32, e["params"]["SpeedY"].as_f64().unwrap_or(0.0) as f32));
        let source = data.and_then(|d| d["source_texture"].as_str()).map(|t| t.replace('\\', "/").to_lowercase()).unwrap_or_default();
        let (kind, detail_scale) = effects.get(&source).map_or((EffectKind::None, 0.0), |entry| effect_for(entry, data.and_then(|d| d["source_shader"].as_u64()).unwrap_or(0)));
        let effect_texture = (kind != EffectKind::None).then(|| effects[&source]["texture"].as_str().map(|path| textures.entry(path.to_owned()).or_insert_with(|| { let handle = asset_server.load_with_settings(path.to_owned(), texture_settings(true)); mips.0.push(handle.clone()); handle }).clone())).flatten();
        let positions = part.positions.into_iter().map(|[x, y, z]| [x * SCALE, y * SCALE, z * SCALE]).collect::<Vec<_>>();
        let vertices = positions.len();
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, crate::mirror_padded_u(part.uvs, data));
        // Gouraud surfaces carry their per-vertex light group intensity (0..1) in U of the second UV set.
        let vertex_group: Vec<[f32; 2]> = match &group_intensity { Some(bytes) => part.source_vertices.iter().map(|&v| [bytes.get(v as usize).copied().unwrap_or(0) as f32 / 255.0, 0.0]).collect(), None => vec![[0.0, 0.0]; vertices] };
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, if lightmapped { part.lightmap_uvs } else { vertex_group });
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, part.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, part.colors);
        let base = if broken_uv { None } else { texture_paths.get(&part.material).map(|path| textures.entry(path.clone()).or_insert_with(|| { let handle = asset_server.load_with_settings(level_viewer::hd::path(path), texture_settings(true)); mips.0.push(handle.clone()); handle }).clone()) };
        entities.push(commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(RetailWorld::new(fog, shading, appearance.0, appearance.1.alpha(), pan, base, if lightmapped { atlas.clone() } else { None }, (kind, detail_scale, effect_texture), group_atlas.clone()))),
            crate::WorldGeometry,
        )).id());
    }
    entities
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_properties_set_fog_far_clip_and_clear_colour() {
        let scene = serde_json::json!({"objects": [{"kind": "WorldProperties", "properties": {"Widocznosc": 1600.0, "Mgla_Wlaczona": 1, "Kolor_Mgly": [67.0, 89.0, 90.0], "Start_Mgly": 1.0, "Koniec_Mgly": 1600.0}}]});
        let fog = WorldFog::from_scene(&scene);
        assert!(fog.enabled && (fog.far - 16.0).abs() < 1e-5 && (fog.end - 16.0).abs() < 1e-5 && (fog.color[2] - 90.0 / 255.0).abs() < 1e-6);
        // Fog off still clears to the property colour (cshell 0x1005a8e0), and a level without properties keeps the engine defaults.
        let plain = serde_json::json!({"objects": [{"kind": "WorldProperties", "properties": {"Widocznosc": 1000.0, "Mgla_Wlaczona": 0, "Kolor_Mgly": [127.0, 127.0, 127.0]}}]});
        assert!(!WorldFog::from_scene(&plain).enabled && WorldFog::from_scene(&plain).color[0] > 0.49);
        let none = WorldFog::from_scene(&serde_json::json!({"objects": []}));
        assert!(!none.enabled && none.color == [0.0; 3] && (none.far - 100.0).abs() < 1e-5);
    }

    #[test]
    fn env_and_detail_effects_follow_the_retail_shader_choice() {
        let entry = |kind: &str, scale: f64| serde_json::json!({"kind": kind, "texture": "env_textures/x.png", "scale": scale});
        // AllocShader: EnvMapAlpha on a lightmapped section is the plain Lightmap_Texture; on Gouraud it is Gouraud_Alpha_EnvMap.
        assert_eq!(effect_for(&entry("envmapalpha", 0.0), 4).0, EffectKind::None);
        assert_eq!(effect_for(&entry("envmapalpha", 0.0), 1).0, EffectKind::EnvMapAlpha);
        assert_eq!(effect_for(&entry("envmap", 0.0), 4).0, EffectKind::EnvMap);
        let (kind, scale) = effect_for(&entry("detail", 34.0), 4);
        assert_eq!(kind, EffectKind::Detail);
        assert!((scale - 6.8).abs() < 1e-5, "trawa_duza tiles its detail 0.2 * 34 times");
        // A command whose file is missing (beton: `\\textures\\detail\\detail 1`) validates false and draws plain.
        assert_eq!(effect_for(&serde_json::json!({"kind": "detail", "texture": null, "scale": 15.0}), 4).0, EffectKind::None);
    }

    #[test]
    fn mip_chain_halves_down_to_one_texel_with_box_filter() {
        let mut data = Vec::new();
        for value in [0u8, 100, 200, 60] { data.extend([value, value, value, 255]); }
        let mut image = Image::new(bevy::render::render_resource::Extent3d { width: 2, height: 2, depth_or_array_layers: 1 }, bevy::render::render_resource::TextureDimension::D2, data, bevy::render::render_resource::TextureFormat::Rgba8Unorm, RenderAssetUsages::default());
        add_mips(&mut image);
        assert_eq!(image.texture_descriptor.mip_level_count, 2);
        let bytes = image.data.as_ref().unwrap();
        assert_eq!(bytes.len(), 16 + 4);
        assert_eq!(&bytes[16..20], &[90, 90, 90, 255]);
    }
}
