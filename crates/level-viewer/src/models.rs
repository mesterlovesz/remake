//! Native LTB skeleton and vertex animation playback over the portable export.
use bevy::{prelude::*,asset::RenderAssetUsages,mesh::{Indices,PrimitiveTopology}};

/// LithTech lit models with diffuse Gouraud only. Bevy's default PBR material
/// (roughness 0.5, 4% reflectance) added a wet sheen to hair and skin.
pub fn matte()->StandardMaterial {
    StandardMaterial {perceptual_roughness:1.0,reflectance:0.0,specular_tint:Color::BLACK,..default()}
}
use serde::Deserialize;
use std::{collections::BTreeMap,path::Path};

#[derive(Deserialize)]
pub struct Model { pub pieces:Vec<Piece>,nodes:Vec<Bone>,animations:BTreeMap<String,Animation>,sockets:BTreeMap<String,Socket>,#[serde(skip)] first:String }
#[derive(Deserialize)]
struct Bone {parent:i32,bind:[f32;16]}
#[derive(Deserialize)]
struct Animation {times:Vec<f32>,tracks:Vec<Track>,translation:[f32;3],#[serde(default)]dimensions:[f32;3]}
#[derive(Deserialize)]
struct Track {pos:Vec<[f32;3]>,rot:Vec<[f32;4]>,#[serde(default)]morph:Vec<Vec<[f32;3]>>}
#[derive(Deserialize)]
struct Socket {bone:usize,pos:[f32;3],rot:[f32;4],scale:[f32;3]}
#[derive(Deserialize)]
pub struct Piece {pub texture:u32,/** Render style index (objects.txt `rsN`); absent in older exports, where the texture slot stands in. */#[serde(default)]pub style:Option<u32>,positions:Vec<[f32;3]>,normals:Vec<[f32;3]>,uvs:Vec<[f32;2]>,weights:Vec<Vec<(usize,f32)>>,indices:Vec<u32>,morph_node:Option<usize>,#[serde(default)]duplicates:Vec<[usize;2]>}

pub struct Pose {pub bones:Vec<Mat4>,pub skin:Vec<Mat4>,frames:(usize,usize,f32),animation:String}

#[derive(Deserialize)]
struct Prop {#[serde(default)]name:String,#[serde(default)]definition_name:String,model:String,pos:[f32;3],rotation:[f32;4],animation:String,skins:BTreeMap<String,String>,styles:BTreeMap<String,String>}

#[derive(Resource,Default)]
pub struct PropAnimations {
    pub pending:Vec<(String,String)>,
    /// Looping rest animation that takes over once a `pending` one ends: anim1 after anim01, anim0 after anim10 (cshell 0x1002d40a).
    pub rest:BTreeMap<String,String>,
    models:BTreeMap<String,Model>,
}
#[derive(Component)]
pub struct PropAnimation {
    definition:String,name:String,model:String,piece:usize,animation:Option<(String,f32)>,owned_mesh:bool,
    /// Looping default animation: objects.txt `anim0`, else the model's first animation (creation 0x1002d3aa, engine default).
    idle:Option<String>,
}
/// Props farther than this (native units) skip their idle animation; they cannot be seen.
const IDLE_RANGE:f32=4000.0;

/// Resolve script object names against objects.txt definitions (cutscenes) or
/// scene instance names (activated o_obiekt). Each animated instance owns its
/// mesh; untouched static instances keep their shared mesh.
pub fn animate_props(mut animations:ResMut<PropAnimations>,config:Res<crate::ViewerConfig>,time:Res<Time>,session:Res<crate::settings::Session>,mut meshes:ResMut<Assets<Mesh>>,mut props:Query<(&mut PropAnimation,&mut Mesh3d,Option<&Transform>)>,cameras:Query<&Transform,With<crate::InspectionCamera>>,mut clock:Local<f32>) {
    if session.paused {return;}
    *clock+=time.delta_secs();
    for (target,animation) in std::mem::take(&mut animations.pending) {
        for (mut prop,mut handle,_) in &mut props {
            if prop.definition!=target && prop.name!=target {continue;}
            let model=animations.models.entry(prop.model.clone()).or_insert_with(||Model::load(&config.output,&prop.model));
            if !model.animations.contains_key(&animation) {warn!("Unknown original object animation: {target} / {animation}");continue;}
            if !prop.owned_mesh {
                let Some(mesh)=meshes.get(&handle.0).cloned() else{continue};
                handle.0=meshes.add(mesh);prop.owned_mesh=true;
            }
            prop.animation=Some((animation.clone(),0.0));
        }
    }
    let eye=cameras.iter().next().map(|t|t.translation);
    let rest=animations.rest.clone();
    let mut poses=BTreeMap::<String,Pose>::new();
    for (mut prop,mut handle,transform) in &mut props {
        let model_path=prop.model.clone();let piece=prop.piece;
        if prop.animation.is_some() {
            let Some((name,elapsed))=&mut prop.animation else{continue};
            let model=&animations.models[&model_path];
            *elapsed+=time.delta_secs();
            let pose=model.pose(name,*elapsed,false);
            if let Some(mesh)=meshes.get_mut(&handle.0) {model.animate_mesh(piece,&pose,mesh);}
            if *elapsed>=*model.animations[name].times.last().unwrap_or(&0.0) {
                prop.animation=None;
                if let Some(next)=rest.get(&prop.name).or_else(||rest.get(&prop.definition)) {prop.idle=Some(next.clone());}
            }
            continue;
        }
        let Some(idle)=prop.idle.clone() else{continue};
        if let (Some(eye),Some(transform))=(eye,transform) {if transform.translation.distance(eye)>IDLE_RANGE*crate::SCALE {continue;}}
        if !prop.owned_mesh {
            let Some(mesh)=meshes.get(&handle.0).cloned() else{continue};
            handle.0=meshes.add(mesh);prop.owned_mesh=true;
        }
        let model=animations.models.entry(model_path.clone()).or_insert_with(||Model::load(&config.output,&model_path));
        let pose=poses.entry(format!("{model_path}|{idle}")).or_insert_with(||model.pose(&idle,*clock,true));
        if let Some(mesh)=meshes.get_mut(&handle.0) {model.animate_mesh(piece,pose,mesh);}
    }
}

/// Native model collision dimensions; used only to settle newly placed pickups.
#[derive(Component)]
pub struct PropSupport {pub center:Vec3,pub half_size:Vec3}
/// One o_obiekt instance: the box the use key tests; `parts` are its model pieces.
#[derive(Component)]
pub struct PropBody {pub name:String,pub center:Vec3,pub half_size:Vec3}

/// `insignificant [N]` of objects.txt (cshell 0x1002d634: a bare keyword is 1) by model: props the "insignificant objects" option hides.
#[derive(Component,Clone,Copy)] pub struct Insignificant(pub i32);
/// Option level 1 hides the props marked 1, level 2 every marked prop (0x1002d0b0).
pub fn hidden_by_level(option:i32,marked:i32)->bool {match option {1=>marked==1,2=>marked!=0,_=>false}}
/// A flame-style `.spr` skin: the material cycles through `frames` at `fps`.
#[derive(Clone)]
pub struct SkinAnim {pub material:Handle<StandardMaterial>,pub frames:Vec<Handle<Image>>,pub fps:f32}
/// Shared meshes/materials of one (definition, model, pose), the collision half size and whether the pose moves.
pub type Handles=(Vec<(Handle<Mesh>,Handle<StandardMaterial>,Option<SkinAnim>)>,Vec3,bool);
pub struct Instance<'a> {
    pub name:&'a str,pub definition:&'a str,pub model:&'a str,pub pos:Vec3,pub rotation:[f32;4],pub animation:&'a str,
    pub skins:&'a BTreeMap<String,Vec<String>>,pub fps:&'a BTreeMap<String,f32>,pub styles:&'a BTreeMap<String,String>,pub idle:Option<String>,
}
pub struct Spawned {pub parts:Vec<Entity>,pub half:Vec3,pub skins:Vec<SkinAnim>}

/// Builds (or reuses) the render pieces of one prop and spawns them at `pos`; the animation is the pose at t=0.
/// Blend behaviour of a retail render style (`rs\<name>.ltb`, sources in GYARI/rs/*.lta): the flame style is additive weighted by the texture alpha
/// (BLEND_MUL_SRCALPHA_ONE, z read-only), shadow / glasses / soft masks alpha-blend (BLEND_MOD_SRCALPHA), `przez_z_maska` alpha-tests (GREATER 0), and the
/// metal / vending-machine styles are opaque: the texture alpha there is the reflection mask of a second texture stage, not transparency.
pub fn style_alpha(style:&str)->AlphaMode {
    let name=style.replace('\\',"/").to_lowercase();let name=name.rsplit('/').next().unwrap_or("").trim_end_matches(".ltb").to_owned();
    match name.as_str() {
        "additive"|"przez_z_maska_plomien"=>AlphaMode::Add,
        "cien"|"okulary"|"przez_z_maska1"=>AlphaMode::Blend,
        "przez_z_maska"=>AlphaMode::Mask(0.004),
        _=>AlphaMode::Opaque,
    }
}
pub fn spawn_instance(commands:&mut Commands,meshes:&mut Assets<Mesh>,materials:&mut Assets<StandardMaterial>,assets:&AssetServer,config:&crate::ViewerConfig,cache:&mut BTreeMap<String,Handles>,i:&Instance)->Spawned {
    let key=format!("{}|{}|{}",i.definition,i.model,i.animation);
    let (handles,half,moving)=cache.entry(key).or_insert_with(|| {
        let model=Model::load(&config.output,i.model);let pose=model.pose(i.animation,0.0,false);
        let moving=i.idle.as_deref().is_some_and(|name|model.moving(name));
        let handles=model.pieces.iter().enumerate().map(|(index,piece)| {
            let mut mesh=model.mesh(index);model.animate_mesh(index,&pose,&mut mesh);
            let slot=piece.texture.to_string();
            let frames:Vec<Handle<Image>>=i.skins.get(&slot).map(|f|f.iter().map(|s|assets.load(level_viewer::hd::path(s))).collect()).unwrap_or_default();
            // The piece names its render style (rsN); pieces without one (older model exports) use their texture slot.
            let alpha_mode=i.styles.get(&piece.style.unwrap_or(piece.texture).to_string()).map_or(AlphaMode::Opaque,|s|style_alpha(s));
            let material=materials.add(StandardMaterial {base_color_texture:frames.first().cloned(),unlit:matches!(alpha_mode,AlphaMode::Add),cull_mode:None,alpha_mode,..matte()});
            let skin=(frames.len()>1).then(||SkinAnim {material:material.clone(),frames:frames.clone(),fps:i.fps.get(&slot).copied().unwrap_or(15.0)});
            (meshes.add(mesh),material,skin)
        }).collect();
        (handles,model.dimensions(i.animation),moving)
    }).clone();
    let rotation=Quat::from_euler(EulerRot::YXZ,i.rotation[1],i.rotation[0],i.rotation[2]);
    let mut parts=Vec::new();let mut skins=Vec::new();
    for (piece,(mesh,material,skin)) in handles.iter().enumerate() {
        let mut entity=commands.spawn((PropAnimation {definition:i.definition.into(),name:i.name.into(),model:i.model.into(),piece,animation:None,owned_mesh:false,idle:if moving && !crate::birds::is_bird(i.model) {i.idle.clone()}else{None}},Mesh3d(mesh.clone()),MeshMaterial3d(material.clone()),crate::WorldGeometry,
            Transform {translation:i.pos*crate::SCALE,rotation,scale:Vec3::splat(crate::SCALE)}));
        if crate::birds::is_bird(i.model) {entity.insert(crate::birds::components(i.model,i.animation,piece));}
        parts.push(entity.id());
        if let Some(skin) = skin {skins.push(skin.clone());}
    }
    Spawned {parts,half,skins}
}

pub fn spawn_props(config:&crate::ViewerConfig,commands:&mut Commands,meshes:&mut Assets<Mesh>,materials:&mut Assets<StandardMaterial>,assets:&AssetServer,world:&crate::props::PropWorld) {
    world.field().clear();
    let data=crate::props::PropData::load(&config.output);
    let path=config.output.join(format!("{}.props.json",config.world));
    let props:Vec<Prop>=if path.exists() {serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()}else{Vec::new()};
    let mut cache=BTreeMap::<String,Handles>::new();let mut skin_anims=Vec::new();
    for prop in props {
        let def=data.defs.get(&prop.definition_name).cloned().unwrap_or_default();
        // props_defs.json carries every skin frame (and the .spr flames); older exports fall back to the per-level file.
        let exported=data.assets.get(&prop.definition_name);
        let skins:BTreeMap<String,Vec<String>>=exported.filter(|a|!a.skins.is_empty()).map(|a|a.skins.clone()).unwrap_or_else(||prop.skins.iter().map(|(k,v)|(k.clone(),vec![v.clone()])).collect());
        let fps=exported.map(|a|a.fps.clone()).unwrap_or_default();
        let styles=if def.styles.is_empty() {&prop.styles}else{&def.styles};
        // Retail loops `anim0` from creation, else the model's own first animation (cshell 0x1002d3aa, engine default).
        let idle=if def.anim0.is_empty() {None}else{Some(def.anim0.clone())};
        let instance=Instance {name:&prop.name,definition:&prop.definition_name,model:&prop.model,pos:Vec3::from(prop.pos),rotation:prop.rotation,animation:&prop.animation,skins:&skins,fps:&fps,styles,idle:idle.or_else(||Some(prop.animation.clone()))};
        let spawned=spawn_instance(commands,meshes,materials,assets,config,&mut cache,&instance);
        skin_anims.extend(spawned.skins.iter().cloned());
        if def.insignificant!=0 {for &part in &spawned.parts {commands.entity(part).insert(Insignificant(def.insignificant));}}
        let mut entities=spawned.parts.clone();
        let solid=def.solid || def.gravity;
        if solid {entities.push(commands.spawn((crate::WorldGeometry,PropSupport {center:Vec3::from(prop.pos),half_size:spawned.half})).id());}
        entities.push(commands.spawn((crate::WorldGeometry,PropBody {name:prop.name.clone(),center:Vec3::from(prop.pos),half_size:spawned.half})).id());
        world.field().add(crate::props::PropState {name:prop.name.clone(),def:prop.definition_name.clone(),center:Vec3::from(prop.pos),rotation:prop.rotation,half:spawned.half,solid,hp:def.hp,dead:false,effects:crate::props::hit_effects(&def),origin:Vec3::from(prop.pos),mass:def.mass,entities});
    }
    commands.insert_resource(data);
    commands.queue(move |world:&mut World|{let mut clock=world.get_resource_or_insert_with(crate::props::SkinClock::default);clock.0=skin_anims;});
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_styles_map_to_the_retail_blend_states() {
        assert!(matches!(style_alpha("rs/przez_z_maska_plomien.ltb"),AlphaMode::Add) && matches!(style_alpha("rs/additive.ltb"),AlphaMode::Add));
        assert!(matches!(style_alpha("rs/cien.ltb"),AlphaMode::Blend) && matches!(style_alpha("rs/okulary.ltb"),AlphaMode::Blend));
        assert!(matches!(style_alpha("rs/przez_z_maska.ltb"),AlphaMode::Mask(cutoff) if cutoff<0.01));
        for opaque in ["rs/automat_cola.ltb","rs/metal_z_maska.ltb","rs/do_ganow.ltb","unknown"] {assert!(matches!(style_alpha(opaque),AlphaMode::Opaque),"{opaque}");}
    }
    #[test]
    fn insignificant_props_are_hidden_by_the_option_level() {
        assert!(!hidden_by_level(0,2) && hidden_by_level(1,1) && !hidden_by_level(1,2) && hidden_by_level(2,1) && hidden_by_level(2,2) && !hidden_by_level(2,0));
    }
    #[test]
    fn retail_model_preserves_exported_skin_coordinates() {
        let piece=Piece {texture:0,style:None,positions:vec![[0.0,0.0,0.0];3],normals:vec![[0.0,1.0,0.0];3],
            uvs:vec![[0.2,0.4],[0.8,0.4],[0.2,0.9]],weights:vec![vec![(0,1.0)];3],indices:vec![0,1,2],morph_node:None,duplicates:vec![]};
        let model=Model {pieces:vec![piece],nodes:vec![],animations:BTreeMap::new(),sockets:BTreeMap::new(),first:String::new()};
        let mesh=model.mesh(0);
        let Some(bevy::mesh::VertexAttributeValues::Float32x2(uvs))=mesh.attribute(Mesh::ATTRIBUTE_UV_0) else {panic!("UVs missing")};
        assert!((uvs[0][0]-0.2).abs()<0.0001 && (uvs[1][0]-0.8).abs()<0.0001,"{uvs:?}");
        assert_eq!(uvs[0][1],0.4);
    }
    #[test]
    #[ignore="requires local original model export"]
    fn umbrella_mouth_uses_separate_original_teeth_geometry() {
        let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
        let model=Model::load(&root,"models/postacie/cywil1_parasol.ltb");
        let (face,teeth)=model.split_uv_region(1,|[u,v]|u>0.68 && v<0.26);
        let triangles=|mesh:&Mesh|match mesh.indices().unwrap() {Indices::U32(indices)=>indices.len()/3,_=>panic!("unexpected indices")};
        assert_eq!(triangles(&teeth),28);
        assert_eq!(triangles(&face)+triangles(&teeth),430);
    }

    #[test]
    fn animated_skin_rotates_surface_normals_with_the_head() {
        let piece=Piece {texture:0,style:None,positions:vec![[1.0,0.0,0.0];3],normals:vec![[1.0,0.0,0.0];3],
            uvs:vec![[0.0,0.0];3],weights:vec![vec![(0,1.0)];3],indices:vec![0,1,2],morph_node:None,duplicates:vec![]};
        let model=Model {pieces:vec![piece],nodes:vec![],animations:BTreeMap::new(),sockets:BTreeMap::new(),first:String::new()};
        let rotation=Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let pose=Pose {bones:vec![rotation],skin:vec![rotation],frames:(0,0,0.0),animation:String::new()};
        let mut mesh=model.mesh(0);model.animate_mesh(0,&pose,&mut mesh);
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(normals))=mesh.attribute(Mesh::ATTRIBUTE_NORMAL) else {panic!("normals missing")};
        assert!(Vec3::from(normals[0]).distance(Vec3::Y)<0.001,"{normals:?}");
    }

    #[test]
    fn original_bus_gate_animation_is_local_and_pauses() {
        let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
        let props:Vec<Prop>=serde_json::from_str(&std::fs::read_to_string(root.join("rh2-wiezienie2.props.json")).unwrap()).unwrap();
        let gate=props.iter().find(|p|p.definition_name=="brama1").expect("authored bus gate");
        let model=Model::load(&root,&gate.model);let mut mesh=model.mesh(0);
        model.animate_mesh(0,&model.pose(&gate.animation,0.0,false),&mut mesh);
        let mut assets=Assets::<Mesh>::default();let shared=assets.add(mesh);
        let original=format!("{:?}",assets.get(&shared).unwrap().attribute(Mesh::ATTRIBUTE_POSITION));
        let mut clock=Time::<()>::default();clock.advance_by(std::time::Duration::from_secs(3));
        let mut app=App::new();
        app.insert_resource(assets).insert_resource(clock).insert_resource(PropAnimations::default())
            .insert_resource(crate::ViewerConfig {output:root.clone(),user:root,persist:false,world:"rh2-wiezienie2".into(),capture:None,capture_times:Vec::new(),story:false})
            .insert_resource({let mut session=crate::settings::Session::new(default(),true,false);session.paused=true;session})
            .add_systems(Update,animate_props);
        let animated=app.world_mut().spawn((PropAnimation {definition:"brama1".into(),name:String::new(),model:gate.model.clone(),piece:0,animation:None,owned_mesh:false,idle:None},Mesh3d(shared.clone()))).id();
        app.world_mut().spawn((PropAnimation {definition:"unrelated".into(),name:String::new(),model:gate.model.clone(),piece:0,animation:None,owned_mesh:false,idle:None},Mesh3d(shared.clone())));
        app.world_mut().resource_mut::<PropAnimations>().pending.push(("brama1".into(),"otwiera".into()));
        app.update();assert_eq!(app.world().get::<Mesh3d>(animated).unwrap().0,shared,"pause must retain gate pose and request");
        app.world_mut().resource_mut::<crate::settings::Session>().paused=false;app.update();
        let changed=&app.world().get::<Mesh3d>(animated).unwrap().0;
        assert_ne!(changed,&shared,"animated instances must not edit the shared mesh");
        let assets=app.world().resource::<Assets<Mesh>>();
        assert_eq!(format!("{:?}",assets.get(&shared).unwrap().attribute(Mesh::ATTRIBUTE_POSITION)),original);
        assert_ne!(format!("{:?}",assets.get(changed).unwrap().attribute(Mesh::ATTRIBUTE_POSITION)),original,"original opening animation must move gate vertices");
        let old=Model::load(&app.world().resource::<crate::ViewerConfig>().output,"models/postacie/stara_dziwka.ltb");
        assert!(old.try_socket(&old.pose("gada",0.0,false),"bron").is_none(),"retain original missing attachment evidence");
    }

    #[test]
    fn animation_binding_translation_offsets_root_before_socket_composition() {
        let model:Model=serde_json::from_value(serde_json::json!({
            "pieces":[],
            "nodes":[
                {"parent":-1,"bind":Mat4::IDENTITY.to_cols_array()},
                {"parent":0,"bind":Mat4::IDENTITY.to_cols_array()}
            ],
            "animations":{"shot":{
                "times":[0.0,1.0],"translation":[10.0,20.0,30.0],
                "tracks":[
                    {"pos":[[0.0,0.0,0.0]],"rot":[[0.0,1.0,0.0,0.0]]},
                    {"pos":[[1.0,2.0,3.0],[3.0,2.0,3.0]],"rot":[[0.0,0.0,0.0,1.0]]}
                ]
            }},
            "sockets":{"camera":{"bone":1,"pos":[0.0,0.0,0.0],"rot":[0.0,0.0,0.0,1.0],"scale":[1.0,1.0,1.0]}}
        })).unwrap();
        let pose=model.pose("shot",0.5,false);
        let position=model.socket(&pose,"camera").transform_point3(Vec3::ZERO);
        assert!(position.abs_diff_eq(Vec3::new(8.0,22.0,27.0),0.001),"{position:?}");
    }
}
impl Model {
    pub fn load(root:&Path,path:&str)->Self {
        let text=std::fs::read_to_string(root.join(format!("{path}.json"))).expect("read exported LTB");
        let mut model:Self=serde_json::from_str(&text).expect("parse exported LTB");model.first=first_animation_key(&text);model
    }
    /// The animation the engine starts a fresh model object on: index 0 in file order (the export keeps it, BTreeMap does not).
    pub fn first_animation(&self)->&str {
        if self.animations.contains_key(&self.first) {&self.first}else{self.animations.keys().next().map_or("",String::as_str)}
    }
    /// Whether animation `name` ever changes the pose (most props ship a two-key constant clip).
    pub fn moving(&self,name:&str)->bool {
        let name=if self.animations.contains_key(name) {name}else{self.first_animation()};
        let Some(animation)=self.animations.get(name) else{return false};
        let duration=*animation.times.last().unwrap_or(&0.0);
        if duration<=0.0 || animation.times.len()<2 {return false;}
        if animation.tracks.iter().any(|t|!t.morph.is_empty()) {return true;}
        let base=self.pose(name,0.0,false);
        [0.25,0.5,0.75,1.0].iter().any(|f|self.pose(name,duration*f,false).skin.iter().zip(&base.skin).any(|(x,y)|!x.abs_diff_eq(*y,1e-3)))
    }
    /// Length in seconds of a named animation; None when the model has no such clip (SetCurAnim then keeps the old one).
    pub fn animation_duration(&self,name:&str)->Option<f32> {self.animations.get(name).map(|a|*a.times.last().unwrap_or(&0.0))}
    pub fn dimensions(&self,name:&str)->Vec3 {
        let animation=self.animations.get(name).or_else(||self.animations.get(self.first_animation()));
        animation.map(|a|Vec3::from(a.dimensions)).unwrap_or(Vec3::ZERO)
    }
    pub fn pose(&self,name:&str,time:f32,looping:bool)->Pose {
        let name=if self.animations.contains_key(name) {name}else{self.first_animation()};
        assert!(self.animations.contains_key(name),"model animation");
        let animation=&self.animations[name];
        let duration=*animation.times.last().unwrap_or(&0.0);
        let time=if looping && duration>0.0 {time%duration}else{time.min(duration)};
        let b=animation.times.partition_point(|t|*t<time).min(animation.times.len()-1);let a=b.saturating_sub(1);
        let alpha=if a==b {0.0}else{(time-animation.times[a])/(animation.times[b]-animation.times[a]).max(0.0001)};
        let mut bones=Vec::<Mat4>::new();let mut skin=Vec::new();
        for (i,bone) in self.nodes.iter().enumerate() {
            let track=&animation.tracks[i];
            let mut pos=Vec3::from(track.pos[a.min(track.pos.len()-1)]).lerp(Vec3::from(track.pos[b.min(track.pos.len()-1)]),alpha);
            if bone.parent<0 {pos+=Vec3::from(animation.translation);}
            let qa=Quat::from_array(track.rot[a.min(track.rot.len()-1)]).normalize();
            let qb=Quat::from_array(track.rot[b.min(track.rot.len()-1)]).normalize();
            let local=Mat4::from_rotation_translation(qa.slerp(qb,alpha),pos);
            let global=if bone.parent<0 {local}else{bones[bone.parent as usize]*local};
            bones.push(global);
            skin.push(global*Mat4::from_cols_array(&bone.bind).transpose().inverse());
        }
        Pose {bones,skin,frames:(a,b,alpha),animation:name.to_owned()}
    }
    pub fn try_socket(&self,pose:&Pose,name:&str)->Option<Mat4> {
        let socket=self.sockets.get(name)?;
        Some(*pose.bones.get(socket.bone)?*Mat4::from_scale_rotation_translation(Vec3::from(socket.scale),Quat::from_array(socket.rot).normalize(),Vec3::from(socket.pos)))
    }
    pub fn socket(&self,pose:&Pose,name:&str)->Mat4 {
        self.try_socket(pose,name).unwrap_or_else(||panic!("missing socket {name}"))
    }
    pub fn mesh(&self,index:usize)->Mesh {
        let p=&self.pieces[index];
        let mut mesh=Mesh::new(PrimitiveTopology::TriangleList,RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION,p.positions.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL,p.normals.clone());
        // LTB model UVs already address the decoded skin. Flipping U sends the
        // police cap into the hand/skin patch of policjant0.dtx, among others.
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0,p.uvs.clone());
        mesh.insert_indices(Indices::U32(p.indices.clone()));mesh
    }
    pub fn split_uv_region(&self,index:usize,inside:impl Fn([f32;2])->bool)->(Mesh,Mesh) {
        let piece=&self.pieces[index];
        let (mut outside,mut selected)=(Vec::new(),Vec::new());
        for triangle in piece.indices.chunks_exact(3) {
            let destination=if triangle.iter().all(|&vertex|inside(piece.uvs[vertex as usize])) {&mut selected}else{&mut outside};
            destination.extend_from_slice(triangle);
        }
        let mut base=self.mesh(index);base.insert_indices(Indices::U32(outside));
        let mut region=self.mesh(index);region.insert_indices(Indices::U32(selected));
        (base,region)
    }
    pub fn animate_mesh(&self,index:usize,pose:&Pose,mesh:&mut Mesh) {
        let p=&self.pieces[index];let mut output=Vec::with_capacity(p.positions.len());
        let mut normals=Vec::with_capacity(p.normals.len());
        let morph=p.morph_node.and_then(|n|self.animations[&pose.animation].tracks.get(n)).filter(|t|!t.morph.is_empty());
        if let Some(track)=morph {
            let (a,b,t)=pose.frames;
            let va=&track.morph[a.min(track.morph.len()-1)];let vb=&track.morph[b.min(track.morph.len()-1)];
            output=p.positions.clone();
            for i in 0..va.len() { output[i]=Vec3::from(va[i]).lerp(Vec3::from(vb[i]),t).to_array(); }
            for &[src,dst] in &p.duplicates {output[dst]=output[src];}
            for v in &mut output { *v=pose.bones[p.weights[0][0].0].transform_point3(Vec3::from(*v)).to_array(); }
            for normal in &p.normals {
                normals.push(pose.bones[p.weights[0][0].0].transform_vector3(Vec3::from(*normal)).normalize_or_zero().to_array());
            }
        } else {
            for ((v,normal),weights) in p.positions.iter().zip(&p.normals).zip(&p.weights) {
                let mut point=Vec3::ZERO;let mut rotated=Vec3::ZERO;let mut total=0.0;
                for &(bone,weight) in weights {
                    point+=pose.skin[bone].transform_point3(Vec3::from(*v))*weight;
                    rotated+=pose.skin[bone].transform_vector3(Vec3::from(*normal))*weight;
                    total+=weight;
                }
                output.push((point/total.max(0.0001)).to_array());
                normals.push(rotated.normalize_or_zero().to_array());
            }
        }
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION,output);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL,normals);
    }
}

fn first_animation_key(text:&str)->String {
    let Some(start)=text.find("\"animations\":{") else{return String::new()};
    let rest=text[start+14..].trim_start();
    if !rest.starts_with('"') {return String::new();}
    let bytes=rest.as_bytes();let mut end=1;
    while end<bytes.len() {if bytes[end]==b'\\' {end+=2;continue;}if bytes[end]==b'"' {break;}end+=1;}
    rest.get(..=end).and_then(|literal|serde_json::from_str(literal).ok()).unwrap_or_default()
}
