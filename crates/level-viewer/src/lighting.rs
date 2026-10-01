//! Original scene lights for moving objects and the isolated weapon camera.
use bevy::{camera::visibility::RenderLayers, prelude::*};
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct SourceLight {
    pub position: Vec3,
    pub color: Vec3,
    pub radius: f32,
    pub brightness: f32,
    pub shadows: bool,
    pub spot: Option<(Quat, f32)>,
    /// Name of the LightGroup that switches this light, if any.
    pub group: String,
    pub enabled: bool,
    /// Colour multiplier of the LightGroup (black while the group is off or in the dark half of its flicker).
    pub scale: Vec3,
}

#[derive(Resource, Default)]
pub struct SourceLights {pub lights: Vec<SourceLight>}
#[derive(Component)]
pub(crate) struct SourceLightEntity(usize);
#[derive(Component)]
pub struct WeaponLight;
/// Marks a first-person weapon camera whose ambient light follows the level lights.
#[derive(Component)]
pub struct WeaponAmbient;

impl SourceLights {
    pub fn from_scene(scene: &Value) -> Self {
        let mut lights=Vec::new();
        let Some(objects)=scene["objects"].as_array() else{return Self {lights}};
        for object in objects {
            let kind=object["kind"].as_str().unwrap_or("");
            if !matches!(kind,"Light"|"ObjectLight"|"DirLight") {continue;}
            let properties=&object["properties"];
            if kind!="ObjectLight" && properties["LightObjects"].as_u64()==Some(0) {continue;}
            // server.dll 0x1003f7ed / 0x1003f86c: a light that has `FastLightObjects 1` creates no model-light record (docs/retail-lighting-research.md).
            if kind!="ObjectLight" && properties["FastLightObjects"].as_u64()==Some(1) {continue;}
            let Some(pos)=properties["Pos"].as_array() else{continue};
            let Some(rgb)=properties[if kind=="DirLight" {"InnerColor"}else{"LightColor"}].as_array() else{continue};
            if pos.len()<3 || rgb.len()<3 {continue;}
            let vec3=|values:&Vec<Value>,factor:f32|Vec3::new(
                values[0].as_f64().unwrap_or(0.0) as f32*factor,
                values[1].as_f64().unwrap_or(0.0) as f32*factor,
                values[2].as_f64().unwrap_or(0.0) as f32*factor);
            let radius=properties["LightRadius"].as_f64().unwrap_or(0.0) as f32;
            if radius<=0.0 {continue;}
            let spot=if kind=="DirLight" {
                let angles=properties["Rotation"].as_array();
                let angle=|index:usize|angles.and_then(|a|a.get(index)).and_then(Value::as_f64).unwrap_or(0.0) as f32;
                Some((Quat::from_euler(EulerRot::YXZ,angle(1),angle(0),angle(2)),
                    (properties["FOV"].as_f64().unwrap_or(70.0) as f32*0.5).to_radians()))
            }else{None};
            lights.push(SourceLight {
                position:vec3(pos,1.0),color:vec3(rgb,1.0/255.0),radius,
                brightness:properties["BrightScale"].as_f64().unwrap_or(1.0) as f32
                    *properties["ObjectBrightScale"].as_f64().unwrap_or(1.0) as f32,
                shadows:properties["CastShadows"].as_u64()==Some(1),spot,
                group:properties["LightGroup"].as_str().unwrap_or("").into(),enabled:true,scale:Vec3::ONE,
            });
        }
        Self {lights}
    }

    pub fn sample(&self, position: Vec3, world: &retail_movement::CollisionWorld) -> Vec3 {
        let mut color=Vec3::splat(0.22);
        for light in self.lights.iter().filter(|light|light.enabled) {
            let ray=light.position-position;
            let distance=ray.length();
            if distance>=light.radius || distance<0.001 {continue;}
            let direction=ray/distance;
            if let Some((rotation,half_angle))=light.spot {
                if (rotation*Vec3::Z).dot(-direction)<half_angle.cos() {continue;}
            }
            let origin=position+direction*3.0;
            let native=|v:Vec3|retail_movement::Vec3::new(v.x,v.y,v.z);
            if world.raycast(native(origin),native(direction),distance-3.0)
                .is_some_and(|(hit,_)|hit<distance-6.0) {continue;}
            let fade=1.0-(distance/light.radius).powi(2);
            color+=light.color*light.scale*light.brightness*fade*fade;
        }
        color.min(Vec3::splat(1.8))
    }

    /// A LightGroup toggled by its activation chain switches its member lights.
    pub fn set_group(&mut self, group: &str, on: bool) {self.set_group_level(group,if on {Vec3::ONE}else{Vec3::ZERO});}
    /// A LightGroup colour (object.lto 0x10010200 sends it to the engine as SFX 0x67): black switches the members off.
    pub fn set_group_level(&mut self, group: &str, level: Vec3) {
        for light in self.lights.iter_mut().filter(|light| light.group==group) { light.scale=level;light.enabled=level.max_element()>0.0; }
    }

    pub fn nearest(&self, position: Vec3, count: usize) -> Vec<usize> {
        let mut scored=self.lights.iter().enumerate().filter(|(_,light)|light.enabled).filter_map(|(index,light)|{
            let distance=light.position.distance(position);
            if distance>=light.radius {return None;}
            let fade=1.0-(distance/light.radius).powi(2);
            Some((index,light.brightness*fade*fade))
        }).collect::<Vec<_>>();
        scored.sort_by(|a,b|b.1.total_cmp(&a.1));
        scored.into_iter().take(count).map(|(index,_)|index).collect()
    }
}

pub fn setup_world(commands:&mut Commands,scene:&Value) {
    let lights=SourceLights::from_scene(scene);
    info!("Gyári objektumfények: {}",lights.lights.len());
    for (index,source) in lights.lights.iter().enumerate() {
        let color=Color::srgb(source.color.x,source.color.y,source.color.z);
        let transform=Transform::from_translation(source.position*crate::SCALE);
        if let Some((rotation,outer_angle))=source.spot {
            commands.spawn((crate::WorldGeometry,SourceLightEntity(index),Visibility::Hidden,
                SpotLight {color,intensity:20000.0*source.brightness,range:source.radius*crate::SCALE,
                    inner_angle:outer_angle*0.7,outer_angle,shadows_enabled:false,..default()},
                // LithTech spots face local +Z, Bevy spots shine along -Z (same flip as the camera).
                transform.with_rotation(rotation*Quat::from_rotation_y(std::f32::consts::PI)),RenderLayers::layer(0)));
        } else {
            commands.spawn((crate::WorldGeometry,SourceLightEntity(index),Visibility::Hidden,
                PointLight {color,intensity:20000.0*source.brightness,range:source.radius*crate::SCALE,
                    shadows_enabled:false,..default()},transform,RenderLayers::layer(0)));
        }
    }
    commands.insert_resource(lights);
}

pub fn sync(
    time:Res<Time>,mut elapsed:Local<f32>,lights:Res<SourceLights>,walking:Res<crate::Walking>,
    mut sources:Query<(&SourceLightEntity,&mut Visibility,Option<&mut PointLight>,Option<&mut SpotLight>),Without<WeaponLight>>,
    mut weapon:Query<&mut AmbientLight,With<WeaponAmbient>>,
    mut weapon_light:Query<&mut PointLight,(With<WeaponLight>,Without<SourceLightEntity>)>,
) {
    *elapsed+=time.delta_secs();
    if *elapsed<0.08 {return;}
    *elapsed=0.0;
    let position=Vec3::new(walking.player.position.x,walking.player.position.y+40.0,walking.player.position.z);
    let nearest=lights.nearest(position,12);
    for (source,mut visibility,point,spot) in &mut sources {
        let rank=nearest.iter().position(|&index|index==source.0);
        *visibility=if rank.is_some() {Visibility::Visible}else{Visibility::Hidden};
        let shadow=rank.is_some_and(|index|index<2) && lights.lights[source.0].shadows;
        let tint=lights.lights[source.0].color*lights.lights[source.0].scale;let tint=Color::srgb(tint.x,tint.y,tint.z);
        if let Some(mut point)=point {point.shadows_enabled=shadow;point.color=tint;}
        if let Some(mut spot)=spot {spot.shadows_enabled=shadow;spot.color=tint;}
    }
    let color=lights.sample(position,&walking.world);
    let peak=color.max_element().max(0.01);
    if let Ok(mut ambient)=weapon.single_mut() {
        ambient.color=Color::srgb(color.x/peak,color.y/peak,color.z/peak);
        ambient.brightness=(90.0*peak).max(35.0);
    }
    for mut point in &mut weapon_light {
        point.color=Color::srgb(color.x/peak,color.y/peak,color.z/peak);
        point.intensity=(12000.0*peak).max(3500.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retail_light_properties_select_object_lights_and_spot_color() {
        let scene=serde_json::json!({"objects":[
            {"kind":"Light","properties":{"Pos":[0,100,0],"LightRadius":500,"LightColor":[255,128,32],"LightObjects":1,"CastShadows":1,"BrightScale":1.5}},
            {"kind":"Light","properties":{"Pos":[0,0,0],"LightRadius":500,"LightObjects":0}},
            {"kind":"DirLight","properties":{"Pos":[100,100,0],"Rotation":[0,1.57,0],"FOV":70,"LightRadius":700,"InnerColor":[20,40,80],"LightObjects":1}}
        ]});
        let lights=SourceLights::from_scene(&scene);
        assert_eq!(lights.lights.len(),2);
        assert_eq!(lights.lights[0].color,Vec3::new(1.0,128.0/255.0,32.0/255.0));
        assert!(lights.lights[0].shadows);
        assert!((lights.lights[0].brightness-1.5).abs()<0.001);
        assert!(lights.lights[1].spot.is_some());
    }

    #[test]
    fn wall_blocks_weapon_probe_light() {
        let scene=serde_json::json!({"objects":[{"kind":"Light","properties":{"Pos":[0,100,0],"LightRadius":300,"LightColor":[255,0,0],"LightObjects":1}}]});
        let lights=SourceLights::from_scene(&scene);
        let clear=retail_movement::CollisionWorld::from_obj("v 900 0 900\nv 920 0 900\nv 900 0 920\nf 1 2 3\n").unwrap();
        let wall=retail_movement::CollisionWorld::from_obj("v -20 50 -20\nv 20 50 -20\nv 20 50 20\nv -20 50 20\nf 1 2 3\nf 1 3 4\n").unwrap();
        let open=lights.sample(Vec3::ZERO,&clear);
        let blocked=lights.sample(Vec3::ZERO,&wall);
        assert!(open.x>blocked.x+0.2,"open={open:?}, blocked={blocked:?}");
    }

    #[test]
    fn nearest_lights_prioritise_effective_brightness_within_radius() {
        let mut lights=SourceLights::default();
        for (x,radius,brightness) in [(1.0,10.0,1.0),(2.0,10.0,3.0),(50.0,10.0,99.0)] {
            lights.lights.push(SourceLight {position:Vec3::X*x,color:Vec3::ONE,radius,brightness,shadows:false,spot:None,group:String::new(),enabled:true,scale:Vec3::ONE});
        }
        assert_eq!(lights.nearest(Vec3::ZERO,2),vec![1,0]);
    }
}
