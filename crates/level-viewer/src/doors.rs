//! Authored hinges, sliding brushes, linked activation, and level exits.
use bevy::prelude::*;
use serde_json::Value;
use std::collections::BTreeMap;
use crate::{ViewerConfig,WorldGeometry,Walking,InspectionCamera,SCALE};

/// Door states of the retail server classes (object.lto 0x10001ac0 / 0x10003c30): -3 closed, 1 opening, 2 wobble after opening, 3 open,
/// -1 closing, -2 wobble after closing.
pub const CLOSED:i32=-3;pub const OPEN:i32=3;
#[derive(Component)] pub struct Door {
    pub name:String,pub destination:String,pub occludes_shots:bool,
    low:Vec3,high:Vec3,components:Vec<(Vec3,Vec3)>,pivot:Vec3,axis:Vec3,offset:Vec3,
    /// `b_door` swings on a hinge with the accelerating state machine of 0x10001ac0; drawers slide linearly.
    hinge:bool,state:i32,
    /// Hinge: signed `Obrot` (rad), `Predkosc_obrotu * 2` (rad/s), swing angle, acceleration term, wobble phase, `Od_gracza`, `Przesuniecie_osi`.
    obrot:f32,rate:f32,ang:f32,acc:f32,phase:f32,od_gracza:bool,pivot_offset:Vec3,
    /// Drawer: `Predkosc` (units/s) and travelled fraction of `Przesuniecie`.
    speed:f32,fraction:f32,
    /// Seconds since the last activation attempt, and `Czas_samozamkniecia` (only zero / non-zero matters).
    timer:f32,close_after:f32,usable:bool,solid:bool,
    /// Object origin and the `Glos_otwierany` / `Glos_zamykany` sounds; `sounded` is the state they last played for.
    origin:Vec3,sounds:[String;2],sounded:bool,
}
#[derive(Component)] pub struct DoorPart(Entity);
/// `request_name` exercises the same aim/range/visibility path as E. Opening a
/// door and its Nast_obiekt chain run through the activation dispatcher.
#[derive(Resource,Default)] pub struct DoorUse {
    pub request_name:Option<String>,
    pub hint:Option<String>,pub target_name:Option<String>,pub consumed:bool,
}
pub fn vector(v:&Value)->Vec3 {Vec3::new(v[0].as_f64().unwrap_or(0.0) as f32,v[1].as_f64().unwrap_or(0.0) as f32,v[2].as_f64().unwrap_or(0.0) as f32)}
fn text(v:&Value)->String {v.as_str().unwrap_or("").into()}
/// Skok_do_levelu as a level id (`worlds\knajpa` becomes `knajpa`); empty for none.
pub fn destination(p:&Value)->String {text(&p["Skok_do_levelu"]).replace('\\',"/").trim_start_matches("worlds/").trim_end_matches(".dat").into()}
fn number(v:&Value)->f32 {v.as_f64().unwrap_or(0.0) as f32}
fn native(v:Vec3)->retail_movement::Vec3 {retail_movement::Vec3::new(v.x,v.y,v.z)}
fn render(v:retail_movement::Vec3)->Vec3 {Vec3::new(v.x,v.y,v.z)}
pub fn attach(config:&ViewerConfig,name:&str,object:&Value,parts:Vec<Entity>,commands:&mut Commands) {
    let kind=object["kind"].as_str().unwrap_or("");
    // b_transparent* keep their pose: solid ones block movement (object flag 0x2000) and, for the `nieprzestrzelny` class, bullets and sight.
    if kind.starts_with("b_transparent") && object["properties"]["Solid"].as_i64()==Some(0) {return;}
    if kind!="b_door" && !kind.starts_with("b_szuflada") && !kind.starts_with("b_transparent") {return;}
    let source=std::fs::read_to_string(config.output.join(format!("world_models/{}/{name}.visual.obj",config.world))).unwrap_or_default();
    let Some(door)=Door::from_model(name,kind,&object["properties"],&source) else{return;};
    let door=commands.spawn((WorldGeometry,door)).id();
    for part in parts {commands.entity(part).insert(DoorPart(door));}
}
impl Door {
    fn from_model(name:&str,kind:&str,properties:&Value,source:&str)->Option<Self> {
        let meshes=level_viewer::read_obj(source).ok()?;
        let components=connected_bounds(&meshes);
        if components.is_empty() {return None;}
        let low=components.iter().fold(Vec3::splat(f32::INFINITY),|a,b|a.min(b.0));
        let high=components.iter().fold(Vec3::splat(f32::NEG_INFINITY),|a,b|a.max(b.1));
        let mut door=Self::from_properties(name,kind,properties,low,high);door.components=components;Some(door)
    }
    fn from_properties(name:&str,kind:&str,p:&Value,low:Vec3,high:Vec3)->Self {
        let transparent=kind.starts_with("b_transparent");let sliding=kind.starts_with("b_szuflada") || transparent;
        Self {name:name.into(),destination:destination(p),
            low,high,components:vec![(low,high)],pivot:vector(&p["Pos"])+vector(&p["Przesuniecie_osi"]),axis:vector(&p["Os_obrotu"]).try_normalize().unwrap_or(Vec3::Y),
            offset:if sliding {vector(&p["Przesuniecie"])}else{Vec3::ZERO},hinge:!sliding,state:CLOSED,
            // Obrot * 0.017453292 and Predkosc_obrotu * 0.034906585 (0x100250c4 / 0x100250c0).
            obrot:if sliding {0.0}else{number(&p["Obrot"]).to_radians()},rate:if sliding {0.0}else{number(&p["Predkosc_obrotu"])*0.034906585},
            ang:0.0,acc:0.0,phase:0.0,od_gracza:p["Od_gracza"].as_i64()==Some(1),pivot_offset:vector(&p["Przesuniecie_osi"]),
            speed:if sliding {number(&p["Predkosc"])}else{0.0},fraction:0.0,
            timer:0.0,close_after:number(&p["Czas_samozamkniecia"]),
            origin:vector(&p["Pos"]),sounds:[text(&p["Glos_otwierany"]),text(&p["Glos_zamykany"])],sounded:false,
            usable:!transparent && p["Gracz_otwiera"].as_i64()!=Some(0),solid:p["Solid"].as_i64()!=Some(0),
            // User flag 0x2 (b_transparent, b_szuflada_przestrzelna) lets bullets and NPC sight through; `nieprzestrzelny` lacks it (cshell 0x100578a0).
            occludes_shots:kind!="b_szuflada_przestrzelna" && kind!="b_transparent" && p["Solid"].as_i64()!=Some(0)}
    }
    /// object.lto 0x100020c0 (b_door) / 0x10003cf0, 0x100046a0 (drawers): `by_player` is the use key or an NPC (a door with
    /// `Gracz_otwiera` 0 refuses those; chains and the mission scripts pass 0). A closed leaf starts opening, an open one closing; while it
    /// is moving the activation is refused (false) and then fires no Nast_obiekt. `activator` is the position `Od_gracza` swings away from.
    /// Every attempt that is not refused for `Gracz_otwiera` restarts the auto-close timer of a hinged door (0x10002158).
    pub(crate) fn activate(&mut self,by_player:bool,activator:Vec3)->bool {
        if !self.usable && by_player {return false;}
        if self.hinge {self.timer=0.0;}
        match self.state {
            CLOSED=>{
                if self.hinge && self.usable && self.od_gracza {
                    let negative=od_gracza_negative(self.pivot_offset,self.origin,activator);
                    self.obrot=if negative {-self.obrot.abs()}else{self.obrot.abs()};
                }
                self.state=1;true
            },
            OPEN=>{self.state=-1;true},
            _=>false,
        }
    }
    /// States 1, 2, -1, -2 (a wobbling hinge counts as busy: about 1.05..1.14 s per swing).
    pub(crate) fn moving(&self)->bool {self.state!=CLOSED && self.state!=OPEN}
    /// Opening or open (the use hint offers "close").
    pub fn is_open(&self)->bool {self.state>0}
    pub(crate) fn state(&self)->(bool,f32) {(self.is_open(),self.fraction())}
    /// Travelled fraction of the swing / slide (a hinge's wobble is folded away).
    fn fraction(&self)->f32 {
        if self.hinge {if self.obrot.abs()>1e-6 {(self.ang/self.obrot.abs()).clamp(0.0,1.0)}else if self.is_open() {1.0}else{0.0}} else {self.fraction}
    }
    /// A saved game restores the settled state (retail toggles doors that disagree with the record, 0x10008d50).
    pub(crate) fn restore(&mut self,open:bool,fraction:f32) {
        self.timer=0.0;self.phase=0.0;self.acc=0.0;
        if self.hinge {self.state=if open {OPEN}else{CLOSED};self.ang=if open {self.obrot.abs()}else{0.0};}
        else {self.fraction=fraction.clamp(0.0,1.0);self.state=if self.fraction>=1.0 {OPEN}else if self.fraction<=0.0 {CLOSED}else if open {1}else{-1};}
    }
    /// Signed swing angle (retail: `sign(Obrot) * ang`, 0x10001cc9).
    fn theta(&self)->f32 {if self.obrot<0.0 {-self.ang}else{self.ang}}
    fn rotation(&self)->Quat {if self.hinge {Quat::from_axis_angle(self.axis,self.theta())}else{Quat::IDENTITY}}
    fn translation(&self)->Vec3 {let rotation=self.rotation();self.pivot-rotation*self.pivot+self.offset*self.fraction}
    /// Does the segment pass through the door's brush in its current pose? (NPC path links, cshell 0x1003ca80)
    pub fn crosses(&self,a:Vec3,b:Vec3)->bool {let delta=b-a;let length=delta.length();length>0.0 && self.ray(a,delta/length,0.0,length).is_some()}
    pub fn pose(&self)->(Quat,Vec3) {(self.rotation(),self.translation())}
    /// Hinged door waiting to close by itself: `Czas_samozamkniecia` set (only zero / non-zero matters), more than 2.0 s since the last
    /// activation attempt, fully open (0x100019a0: the server then asks the client, which closes it once the player is 128 and every
    /// `ruchomy` actor 192 units away from the door origin, cshell 0x1002ff40).
    pub fn wants_close(&self)->bool {self.hinge && self.close_after!=0.0 && self.timer>2.0 && self.state==OPEN}
    /// Drawers with `Czas` set toggle periodically (0x10003f60): timer above Czas in a settled state.
    pub fn wants_toggle(&self)->bool {!self.hinge && self.close_after!=0.0 && self.timer>self.close_after && (self.state==OPEN || self.state==CLOSED)}
    pub fn origin(&self)->Vec3 {self.origin}
    pub fn usable(&self)->bool {self.usable}
    /// Centre of the leaf at its current pose (native units); the walk-through probe aims here.
    pub(crate) fn center(&self)->Vec3 {self.rotation()*((self.low+self.high)*0.5)+self.translation()}
    pub(crate) fn player_opens(&self)->bool {self.usable}
    /// Debug aid for the walk probe: whether the leaf is usable and where a ray from `origin` first meets it.
    pub(crate) fn debug_aim(&self,origin:Vec3,direction:Vec3)->(bool,bool,Option<f32>,Vec3,Vec3) {(self.usable,self.solid,self.ray_component(origin,direction,8.0,190.0).map(|hit|hit.0),self.low,self.high)}
    /// A solid leaf that is not (nearly) open: the walker cannot pass it yet.
    pub(crate) fn blocks(&self)->bool {self.solid && self.fraction()<0.6}
    /// The leaf's bounds at rest (closed).
    pub(crate) fn bounds(&self)->(Vec3,Vec3) {(self.low,self.high)}
    /// Highest world-space point of the leaf at its current pose (brush step-up rule of the door correction pass).
    pub(crate) fn top(&self)->f32 {
        let (rotation,translation)=(self.rotation(),self.translation());
        self.components.iter().flat_map(|&(low,high)|(0..8).map(move|i|Vec3::new(if i&1==0 {low.x}else{high.x},if i&2==0 {low.y}else{high.y},if i&4==0 {low.z}else{high.z}))).map(|corner|(rotation*corner+translation).y).fold(f32::NEG_INFINITY,f32::max)
    }
    pub fn is_hinge(&self)->bool {self.hinge}
    pub fn state_code(&self)->i32 {self.state}
    /// Restarts a periodic drawer toggle.
    pub(crate) fn reset_timer(&mut self) {self.timer=0.0;}
    /// Native-unit distance and world-space normal at the current brush pose.
    /// Grates retain physical collision while permitting shots and sight.
    pub fn shot_hit(&self,origin:Vec3,direction:Vec3,max:f32)->Option<(f32,Vec3)> {
        if !self.occludes_shots {return None;}
        let direction=direction.try_normalize()?;
        let (distance,component)=self.ray_component(origin,direction,0.0,max)?;
        let (low,high)=self.components[component];
        let rotation=self.rotation();
        let point=rotation.conjugate()*(origin+direction*distance-self.translation());
        let mut face=(f32::INFINITY,-direction);
        for axis in 0..3 {
            let mut normal=Vec3::ZERO;normal[axis]=-1.0;
            for (plane,n) in [(low[axis],normal),(high[axis],-normal)] {
                let gap=(point[axis]-plane).abs();if gap<face.0 {face=(gap,rotation*n);}
            }
        }
        Some((distance,face.1))
    }
    fn advance(&mut self,dt:f32) {
        self.timer+=dt;
        if self.hinge {self.advance_hinge(dt);}else{self.advance_slide(dt);}
    }
    /// Drawers: `dist += dt * Predkosc` up to |Przesuniecie|, no acceleration (0x10003c30). Predkosc 0 never finishes.
    fn advance_slide(&mut self,dt:f32) {
        let length=self.offset.length();if length<=0.0 {if self.state==1 {self.state=OPEN;}else if self.state==-1 {self.state=CLOSED;}return;}
        let mut distance=self.fraction*length;
        match self.state {
            1=>{distance+=dt*self.speed;if distance>length {distance=length;self.state=OPEN;}},
            -1=>{distance-=dt*self.speed;if distance<0.0 {distance=0.0;self.state=CLOSED;}},
            _=>{},
        }
        self.fraction=(distance/length).clamp(0.0,1.0);
    }
    /// b_door motion, object.lto 0x10001ac0: the swing accelerates (`|rate| + acc`, acc += 8 dt), clamps at |Obrot|, then a damped
    /// sine wobble (amplitude acc, phase += 16 dt, acc -= 8 acc dt) settles below 0.02. Closing mirrors it towards 0.
    fn advance_hinge(&mut self,dt:f32) {
        let state=self.state;let target=self.obrot.abs();
        match state {
            1|-1=>{
                self.ang+=state as f32*(self.rate.abs()+self.acc)*dt;self.acc+=8.0*dt;
                if state==1 && self.ang>target {self.ang=target;self.state=2;self.acc+=self.rate.abs();self.phase=1.0;}
                else if state==-1 && self.ang<0.0 {self.ang=0.0;self.state=-2;self.acc+=self.rate.abs();self.phase=1.0;}
            },
            2|-2=>{
                let base=if state>0 {target}else{0.0};
                self.ang=base+self.phase.sin()*state as f32*self.acc*dt*0.5;
                self.phase+=16.0*dt;self.acc-=self.acc*dt*8.0;
                if self.acc<=0.02 {self.acc=0.0;if state==2 {self.ang=target;self.state=OPEN;}else{self.ang=0.0;self.state=CLOSED;}}
            },
            _=>{},
        }
    }
    fn ray(&self,origin:Vec3,direction:Vec3,padding:f32,max:f32)->Option<f32> {
        self.ray_component(origin,direction,padding,max).map(|hit|hit.0)
    }
    fn ray_component(&self,origin:Vec3,direction:Vec3,padding:f32,max:f32)->Option<(f32,usize)> {
        let inverse=self.rotation().conjugate();
        let origin=inverse*(origin-self.translation());let direction=inverse*direction;let pad=Vec3::splat(padding);
        ray_box(origin,direction,self.low-pad,self.high+pad,max)?;
        self.components.iter().enumerate().filter_map(|(index,(low,high))|ray_box(origin,direction,*low-pad,*high+pad,max).map(|distance|(distance,index))).min_by(|a,b|a.0.total_cmp(&b.0))
    }
    /// Exact continuous SAT for a player AABB and the current brush OBB.
    /// Initial penetration can be escaped; contact blocks only inward motion.
    pub(crate) fn sweep(&self,position:Vec3,half:Vec3,delta:Vec3)->Option<(f32,Vec3)> {
        if !self.solid {return None;}
        self.components.iter().filter_map(|&(low,high)|self.sweep_component(position,half,delta,low,high)).min_by(|a,b|a.0.total_cmp(&b.0))
    }
    fn sweep_component(&self,position:Vec3,half:Vec3,delta:Vec3,low:Vec3,high:Vec3)->Option<(f32,Vec3)> {
        let rotation=self.rotation();let axes=[rotation*Vec3::X,rotation*Vec3::Y,rotation*Vec3::Z];
        let world_axes=[Vec3::X,Vec3::Y,Vec3::Z];
        let center=rotation*((low+high)*0.5)+self.translation();let extent=(high-low)*0.5;
        let mut candidates=Vec::with_capacity(15);candidates.extend(world_axes);candidates.extend(axes);
        for a in world_axes {for b in axes {if let Some(axis)=a.cross(b).try_normalize() {candidates.push(axis);}}}
        let mut enter=f32::NEG_INFINITY;let mut leave=f32::INFINITY;let mut normal=Vec3::ZERO;let mut depth=f32::INFINITY;
        for axis in candidates {
            let radius=half.dot(axis.abs())+extent.x*axis.dot(axes[0]).abs()+extent.y*axis.dot(axes[1]).abs()+extent.z*axis.dot(axes[2]).abs();
            let distance=(position-center).dot(axis);let speed=delta.dot(axis);
            depth=depth.min(radius-distance.abs());
            if speed.abs()<1e-6 {if distance.abs()>radius {return None;}continue;}
            let a=(-radius-distance)/speed;let b=(radius-distance)/speed;
            if a.min(b)>enter {enter=a.min(b);normal=if speed>0.0 {-axis}else{axis};}
            leave=leave.min(a.max(b));if enter>leave {return None;}
        }
        if depth>0.01 || enter< -0.00001 || leave<0.0 || enter>1.0 || delta.dot(normal)>=-0.00001 {None}else{Some((enter.max(0.0),normal))}
    }
}
/// The original world models can group several disconnected cell grates.
/// Merge triangles sharing authored positions, including material seams,
/// and keep their separate bounds so openings between them stay traversable.
fn connected_bounds(meshes:&[level_viewer::VisualMesh])->Vec<(Vec3,Vec3)> {
    let triangles:Vec<_>=meshes.iter().flat_map(|mesh|mesh.positions.chunks_exact(3)).collect();
    let mut parents:Vec<usize>=(0..triangles.len()).collect();
    let mut vertices=BTreeMap::<[u32;3],usize>::new();
    fn root(parents:&[usize],mut index:usize)->usize {while parents[index]!=index {index=parents[index];}index}
    for (index,triangle) in triangles.iter().enumerate() {for vertex in *triangle {
        let key=vertex.map(|v|if v==0.0 {0}else{v.to_bits()});
        if let Some(&other)=vertices.get(&key) {let a=root(&parents,index);let b=root(&parents,other);parents[a]=b;}
        else {vertices.insert(key,index);}
    }}
    let mut bounds=BTreeMap::<usize,(Vec3,Vec3)>::new();
    for (index,triangle) in triangles.iter().enumerate() {
        let bounds=bounds.entry(root(&parents,index)).or_insert((Vec3::splat(f32::INFINITY),Vec3::splat(f32::NEG_INFINITY)));
        for vertex in *triangle {let point=Vec3::from(*vertex);bounds.0=bounds.0.min(point);bounds.1=bounds.1.max(point);}
    }
    bounds.into_values().collect()
}
/// Reach of the use key (cshell 0x1005faa0 sends the eye and view; the server casts 128 units, object.lto 0x10011880).
pub const USE_RANGE:f32=128.0;
/// `Od_gracza` (object.lto 0x100020c0..0x10002246): when a hinged door opens it picks the sign of `Obrot` from the activator's side so the leaf swings
/// away from them. `p` is `Przesuniecie_osi`, `door` the object origin, `activator` the player (or the NPC / path-node point). The retail matrix is not
/// orthonormal, so the decision line is skewed and uses `+P` (both quirks replicated; verified by running the original x87 code). True: `Obrot := -|Obrot|`.
pub fn od_gracza_negative(p:Vec3,door:Vec3,activator:Vec3)->bool {
    let l=(p.x*p.x+p.z*p.z).sqrt();
    // A zero offset normalises to NaN in retail, the unordered compare then always picks the negative sign.
    if l==0.0 {return true;}
    let (a,b)=(p.z/l,p.x/l);
    let facing=Vec2::new(a*(1.0+l),1.0-l-b*(1.0+l));
    let toward=Vec2::new(activator.x-door.x+p.x,activator.z-door.z+p.z);
    facing.dot(toward)<0.0
}
pub fn ray_box(origin:Vec3,direction:Vec3,low:Vec3,high:Vec3,max:f32)->Option<f32> {
    if !origin.is_finite() || !direction.is_finite() || max<0.0 || !max.is_finite() {return None;}
    let mut enter=0.0f32;let mut leave=max;
    for i in 0..3 {
        if direction[i].abs()<1e-6 {if origin[i]<low[i] || origin[i]>high[i] {return None;}}
        else {let a=(low[i]-origin[i])/direction[i];let b=(high[i]-origin[i])/direction[i];enter=enter.max(a.min(b));leave=leave.min(a.max(b));if enter>leave {return None;}}
    }
    Some(enter)
}
fn nearest_target(origin:Vec3,direction:Vec3,world:&retail_movement::CollisionWorld,doors:&[&Door])->Option<(f32,String,String,bool)> {
    doors.iter().filter(|door|door.usable).filter_map(|door| {
        // The server use ray is a thin segment of exactly 128 units from the eye (object.lto 0x10011880, 0x10025438): no aim padding.
        let (distance,component)=door.ray_component(origin,direction,0.0,USE_RANGE)?;
        let (low,high)=door.components[component];
        // Padding helps aim at small switches, but visibility is checked to
        // the real brush surface, so it cannot extend through an adjacent wall.
        let rotation=door.rotation();let translation=door.translation();
        let local=rotation.conjugate()*(origin+direction*distance-translation);
        let surface=rotation*local.clamp(low,high)+translation;
        let delta=surface-origin;let range=delta.length();
        if range>USE_RANGE {return None;}
        if range>0.01 {
            if world.raycast(native(origin),native(delta),range).is_some_and(|hit|hit.0<range-0.5) {return None;}
            if doors.iter().any(|other|other.name!=door.name && other.solid && other.ray(origin,delta/range,0.0,range).is_some_and(|hit|hit<range-0.5)) {return None;}
        }
        Some((distance,door.name.clone(),door.destination.clone(),door.is_open()))
    }).min_by(|a,b|a.0.total_cmp(&b.0))
}
pub fn tick(mut doors:Query<(Entity,&mut Door)>,mut parts:Query<(&DoorPart,&mut Transform),Without<InspectionCamera>>,mut camera:Single<&mut Transform,(With<InspectionCamera>,Without<DoorPart>)>,mut walking:ResMut<Walking>,session:Res<crate::settings::Session>,intro:Res<crate::opening::Opening>,bind:crate::options::Bindings,time:Res<Time>,mut travel:ResMut<crate::travel::Travel>,mut previous:Local<Option<(u64,Vec3)>>,mut use_door:ResMut<DoorUse>,mut commands:Commands,assets:Res<AssetServer>,mut activation:ResMut<crate::activation::Activation>,props:Res<crate::props::PropWorld>) {
    use_door.hint=None;use_door.target_name=None;use_door.consumed=false;
    if intro.active || session.paused || session.dialogue_active {use_door.request_name=None;return;}
    let origin=camera.translation/SCALE;let direction=*camera.forward();
    let nearest=nearest_target(origin,direction,&walking.world,&doors.iter().map(|(_,door)|door).collect::<Vec<_>>());
    if let Some((_,name,destination,open))=&nearest {
        use_door.target_name=Some(name.clone());
        let key=bind.label(crate::keys_cfg::cmd::ACTION);
        use_door.hint=Some(format!("[{key}] {}",if !destination.is_empty() {"Tovább a következő területre"}else if *open {"Bezárás"}else{"Kinyitás / kapcsoló"}));
    }
    let request=use_door.request_name.take();
    if request.is_some() && std::env::var_os("MESTER_WALK_DOORDEBUG").is_some() {info!("DOOR request {:?} nearest {:?} origin {origin:?} direction {direction:?}",request,nearest.as_ref().map(|n|(n.0,&n.1)));}
    // The use action is level triggered (cshell 0x100605e3 calls 0x1005faa0 every frame while it is down): the server's use ray asks the door every frame, and a
    // door that is moving refuses (object.lto 0x100020c0), so holding the key toggles it again each time it settles.
    let manual=nearest.filter(|(_,name,_,_)|bind.pressed(crate::keys_cfg::cmd::ACTION) || request.as_ref()==Some(name));
    // A level exit jumps at once; any other leaf toggles through the dispatcher.
    if let Some((_,name,destination,_))=manual {use_door.consumed=true;if !destination.is_empty() {travel.pending=Some(destination);}else {activation.manual.push_back((name,None));}}
    let dt=time.delta_secs().min(0.05);
    for (_,mut door) in &mut doors {
        door.advance(dt);
        // Every open/close (use, link or self-closing) plays its sound at the object origin.
        if door.is_open()!=door.sounded {door.sounded=door.is_open();let sound=door.sounds[!door.is_open() as usize].clone();if !sound.is_empty() {crate::audio::play_near(&mut commands,&assets,&sound,door.origin,crate::audio::DOOR_RADIUS);}}
    }
    for (part,mut transform) in &mut parts {if let Ok((_,door))=doors.get(part.0) {transform.rotation=door.rotation();transform.translation=door.translation()*SCALE;}}
    let retail_half=walking.player.half_size();
    // The pass tracks the hull CENTRE: crouching moves the feet by 34 units without the player moving, which would sweep over phantom distance.
    let (center,half,mut velocity)=(render(walking.player.position),render(retail_half),render(walking.player.velocity));
    let mut resolved=center;let field=props.field();let mut supported=false;
    // A probe teleport (Walking::teleported, set by campaign_probe::place, seen by npcs::block_player first) is no movement to correct.
    let teleported=std::mem::take(&mut walking.teleported);
    if let Some((revision,old))=*previous {if revision==travel.arrived && !teleported && (center-old).length()<150.0 {
        let mut position=old;let mut remaining=center-old;
        let needs_correction=doors.iter().any(|(_,door)|door.sweep(position,half,remaining).is_some()) || field.sweep(position,half,remaining).is_some();
        // The retail controller owns static movement. Recheck static surfaces
        // only when a brush actually changes its already-resolved movement.
        for _ in 0..if needs_correction {4}else{0} {
            if remaining.length_squared()<0.000001 {break;}
            // Solid props (objects.txt `solid`/`gravity`: object flags 0x2001, cshell 0x1002c24f) block the player like brushes do.
            let door_hit=doors.iter().filter_map(|(_,door)|door.sweep(position,half,remaining).map(|(t,n)|(t,n,door.top()))).chain(field.sweep(position,half,remaining).map(|(t,n)|(t,n,f32::NEG_INFINITY))).min_by(|a,b|a.0.total_cmp(&b.0));
            // A slide must still respect nearby static walls.
            let world_hit=walking.world.sweep_box(native(position),native(half),native(remaining),0.0).map(|(t,n)|(t,render(n)));
            let hit=door_hit.map(|(t,n,_)|(t,n)).into_iter().chain(world_hit).min_by(|a,b|a.0.total_cmp(&b.0));
            let Some((fraction,normal))=hit else {position+=remaining;break;};
            // Stair rule for brushes: a side contact with a solid whose top is at most one stair (18) above the feet is stepped onto, like the controller does
            // on the static hull (podziemia1c: the threshold plate of the exit door `hujjj` lies 2 units above the floor).
            if let Some((_,_,top))=door_hit.filter(|(t,_,_)|(*t-fraction).abs()<1e-6) {
                let rise=top-(position.y-half.y);
                if normal.y.abs()<0.5 && rise>0.0 && rise<=retail_movement::STAIR_HEIGHT {
                    let up=Vec3::Y*(rise+0.05);
                    let blocked=doors.iter().any(|(_,door)|door.sweep(position,half,up).is_some()) || field.sweep(position,half,up).is_some() || walking.world.sweep_box(native(position),native(half),native(up),0.0).is_some();
                    if !blocked {position+=up;supported=true;continue;}
                }
            }
            // A contact facing up lifts the hull onto a solid: the controller must know it is supported (Player::external_support).
            if normal.y>0.5 {supported=true;}
            position+=remaining*(fraction-0.01/remaining.length()).max(0.0);
            remaining*=1.0-fraction;
            remaining-=normal*remaining.dot(normal).min(0.0);
            velocity-=normal*velocity.dot(normal).min(0.0);
        }
        if needs_correction {resolved=position;}
    }}
    if (resolved-center).length_squared()>0.000001 {
        walking.player.position=native(resolved);
        walking.player.velocity=native(velocity);
        camera.translation+=(resolved-center)*SCALE;
    }
    if supported {walking.player.external_support=true;}
    *previous=Some((travel.arrived,resolved));
}
/// Player distance below which a waiting door stays open, and the radius in which a live `ruchomy` actor keeps it open (cshell 0x1002ff40: 128.0 / 192.0).
pub const HOLD_PLAYER:f32=128.0;pub const HOLD_ACTOR:f32=192.0;
/// NPCs open every closed hinged door within this 3D distance of their point (object.lto 0x10009270).
pub const NPC_OPEN_RANGE:f32=128.0;
/// Client door watcher (cshell 0x1002fee0/0x1002ff40) plus the server's periodic drawer toggle: a door that `wants_close` is closed through the
/// activation dispatcher as soon as the player is 128 and every mobile actor 192 units from its origin.
pub fn auto_close(doors:Query<&Door>,walking:Res<Walking>,roster:Res<crate::npcs::NpcRoster>,session:Res<crate::settings::Session>,mut activation:ResMut<crate::activation::Activation>) {
    if session.paused {return;}
    let p=walking.player.position;let player=Vec3::new(p.x,p.y,p.z);
    for door in &doors {
        if door.wants_toggle() {activation.manual.push_back((door.name.clone(),None));continue;}
        // The client asks the server as the player (msg 0x33 -> byPlayer 1), which a Gracz_otwiera 0 door refuses.
        if !door.wants_close() || !door.usable || player.distance(door.origin)<HOLD_PLAYER {continue;}
        if roster.mobile_near(door.origin,HOLD_ACTOR) {continue;}
        activation.manual.push_back((door.name.clone(),None));
    }
}
/// The door one request point addresses (object.lto 0x10009270, the body of client message 0x34): the chain registry is walked from its head, and
/// the registry adds every new entry at the head (0x10008a40), so the walk runs from the LAST placed object back to the first. The first hinged
/// door (registry kind 1, not a level exit) whose origin is within 128 units ends the search whatever its state: it is opened (0x10002050 ->
/// 0x100020c0 with `byPlayer` 1) only when it is closed and `Gracz_otwiera` is set (0x10009318..0x1000932d); an open or moving door, or one the
/// player may not use, is returned as well and the request then does nothing. A request never closes a door and never reaches a second leaf.
pub fn request_target<'a>(doors:&[&'a Door],point:Vec3)->Option<&'a Door> {
    doors.iter().rev().copied().find(|door|door.hinge && door.destination.is_empty() && door.origin.distance(point)<=NPC_OPEN_RANGE)
}
/// Every living actor on an `estimate_*` path sends its position (+32 up) to the server each update (cshell 0x10042058 -> message 0x34); the server
/// applies `request_target` to each point. The door starts opening through `Door::activate` only: the follow-up activation by registry name
/// (0x10009350 via 0x10008cb0 at 0x1000933a) meets a door that is already moving and is refused, so `Nast_obiekt` (the partner leaf of a double
/// door, a pause, a light) does NOT follow an opening by an actor.
pub fn npc_open(doors:Query<&Door>,roster:Res<crate::npcs::NpcRoster>,session:Res<crate::settings::Session>,mut activation:ResMut<crate::activation::Activation>) {
    if session.paused {return;}
    let list:Vec<&Door>=doors.iter().collect();
    for point in roster.door_openers() {
        let Some(door)=request_target(&list,point) else {continue};
        if door.usable && door.state==CLOSED && !activation.npc_requests.iter().any(|(name,_)|*name==door.name) {
            let name=door.name.clone();info!("Ajtónyitás (NPC): {name} @ {point:?}");activation.npc_requests.push_back((name,point));
        }
    }
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn sweep_catches_thin_door_and_rejects_parallel_miss() {
        assert_eq!(ray_box(Vec3::ZERO,Vec3::X,Vec3::new(2.0,-1.0,-1.0),Vec3::new(2.1,1.0,1.0),4.0),Some(2.0));
        assert!(ray_box(Vec3::new(0.0,3.0,0.0),Vec3::X,Vec3::splat(-1.0),Vec3::ONE,4.0).is_none());
    }

    fn leaf() -> Door {
        Door::from_properties("leaf", "b_door", &serde_json::json!({"Pos":[0,0,0],"Os_obrotu":[0,1,0],"Obrot":90,"Predkosc_obrotu":45,"Gracz_otwiera":1}),Vec3::new(-50.0,-80.0,-2.0),Vec3::new(50.0,80.0,2.0))
    }
    #[test] fn doors_keep_their_authored_open_and_close_sounds_and_origin() {
        let door=Door::from_properties("d","b_door",&serde_json::json!({"Pos":[10,20,30],"Glos_otwierany":"sounds\\psss1.wav","Glos_zamykany":"sounds\\psss.wav"}),Vec3::ZERO,Vec3::ONE);
        assert_eq!(door.sounds,["sounds\\psss1.wav".to_string(),"sounds\\psss.wav".to_string()]);
        assert_eq!(door.origin,Vec3::new(10.0,20.0,30.0));
        assert!(!door.is_open() && !door.sounded,"a state change (open, close or self-close) is what plays the sound");
        assert_eq!(crate::audio::DOOR_RADIUS,640.0);
    }
    #[test] fn original_slider_moves_in_native_units_per_second() {
        let mut door=Door::from_properties("krata1","b_szuflada_przestrzelna",&serde_json::json!({"Przesuniecie":[0,160,0],"Predkosc":16,"Gracz_otwiera":0}),Vec3::ZERO,Vec3::ONE);
        assert!(door.activate(false,Vec3::ZERO),"a chain may open a drawer whose Gracz_otwiera is 0");door.advance(5.0);
        assert!((door.translation().y-80.0).abs()<0.001);
        door.advance(5.1);assert_eq!(door.fraction,1.0);assert_eq!(door.state_code(),OPEN);
        assert!(!door.usable && !door.activate(true,Vec3::ZERO),"the use key is refused");
    }
    #[test] fn rotated_leaf_uses_oriented_collision() {
        let mut door=leaf();door.ang=door.obrot.abs()*0.5;door.state=2;
        // This point is inside the rotated world AABB but outside the leaf.
        assert!(door.sweep(Vec3::new(32.0,0.0,32.0),Vec3::ONE,Vec3::new(1.0,0.0,0.0)).is_none());
        assert!(door.sweep(Vec3::new(0.0,0.0,80.0),Vec3::ONE,Vec3::new(0.0,0.0,-160.0)).is_some());
    }
    #[test] fn touching_leaf_allows_separation_but_blocks_entry() {
        let door=leaf();let pos=Vec3::new(0.0,0.0,3.0);
        assert!(door.sweep(pos,Vec3::ONE,Vec3::Z*10.0).is_none());
        let hit=door.sweep(pos,Vec3::ONE,-Vec3::Z*10.0).unwrap();
        assert!(hit.0<0.001 && hit.1.z>0.99);
    }
    fn leaf_at(name:&str,x:f32,properties:serde_json::Value)->Door {
        let mut p=serde_json::json!({"Pos":[x,0,0],"Os_obrotu":[0,1,0],"Obrot":90,"Predkosc_obrotu":45,"Gracz_otwiera":1});
        for (k,v) in properties.as_object().unwrap() {p[k]=v.clone();}
        Door::from_properties(name,"b_door",&p,Vec3::new(x-20.0,-80.0,-2.0),Vec3::new(x+20.0,80.0,2.0))
    }
    #[test] fn an_actor_request_reaches_only_the_last_placed_door_within_128_units_and_never_closes_it() {
        // object.lto 0x10009270 walks the registry from its head (the newest entry, 0x10008a40) and stops at the first hinged door in range.
        let mut doors=vec![leaf_at("first",0.0,serde_json::json!({})),leaf_at("second",40.0,serde_json::json!({})),leaf_at("far",500.0,serde_json::json!({}))];
        let point=Vec3::new(20.0,32.0,0.0);
        assert_eq!(request_target(&doors.iter().collect::<Vec<_>>(),point).unwrap().name,"second","the double door's later leaf answers; the earlier one is never asked");
        assert!(request_target(&doors.iter().collect::<Vec<_>>(),Vec3::new(2000.0,0.0,0.0)).is_none());
        // Open (or moving) doors keep the search: the request does nothing and the other leaf stays shut.
        doors[1].activate(true,point);
        let target=request_target(&doors.iter().collect::<Vec<_>>(),point).unwrap();
        assert_eq!(target.name,"second");assert!(target.state!=CLOSED,"not closed: npc_open would not send anything");
        // Level exits and drawers are skipped (kind 1 without Skok_do_levelu), a door with Gracz_otwiera 0 still ends the search.
        let exit=leaf_at("exit",60.0,serde_json::json!({"Skok_do_levelu":r"worlds\knajpa"}));
        let locked=leaf_at("locked",50.0,serde_json::json!({"Gracz_otwiera":0}));
        let doors=vec![leaf_at("first",0.0,serde_json::json!({})),locked,exit];
        let target=request_target(&doors.iter().collect::<Vec<_>>(),point).unwrap();
        assert_eq!(target.name,"locked");assert!(!target.usable());
    }
    #[test] fn activation_toggles_a_resting_leaf_but_ignores_a_moving_one() {
        let mut door=leaf();
        assert!(door.activate(true,Vec3::ZERO));assert!(door.moving());
        assert!(!door.activate(false,Vec3::ZERO),"a self-linked pair must not close its first leaf again");
        for _ in 0..200 {door.advance(1.0/60.0);}assert!(!door.moving());assert_eq!(door.state_code(),OPEN);
        assert!(door.activate(true,Vec3::ZERO));assert!(!door.is_open());
    }
    fn hinge(obrot:f32,rate:f32,extra:serde_json::Value)->Door {
        let mut p=serde_json::json!({"Pos":[0,0,0],"Os_obrotu":[0,1,0],"Obrot":obrot,"Predkosc_obrotu":rate,"Gracz_otwiera":1,"Od_gracza":0,"Przesuniecie_osi":[0,0,48]});
        for (key,value) in extra.as_object().into_iter().flatten() {p[key]=value.clone();}
        Door::from_properties("d","b_door",&p,Vec3::splat(-1.0),Vec3::ONE)
    }
    /// Frames until the wobble starts, until the door rests, and the largest overshoot in degrees, at a fixed frame time.
    fn swing(obrot:f32,rate:f32,dt:f32)->(usize,usize,f32) {
        let mut door=hinge(obrot,rate,serde_json::json!({}));assert!(door.activate(true,Vec3::ZERO));
        let (mut wobble,mut rest,mut over)=(0,0,0.0f32);
        for frame in 1..600 {
            door.advance(dt);
            if wobble==0 && door.state==2 {wobble=frame;}
            over=over.max(door.ang-door.obrot.abs());
            if door.state==OPEN {rest=frame;break;}
        }
        (wobble,rest,over.to_degrees())
    }
    #[test] fn retail_hinge_accelerates_and_wobbles_like_the_original_code() {
        // Numbers from running object.lto 0x10001ac0 at 60 fps: a 90 degree door (Predkosc_obrotu 45) reaches its stop on frame 28, rests on frame 67.
        let (wobble,rest,over)=swing(90.0,45.0,1.0/60.0);
        assert_eq!((wobble,rest),(28,67));assert!((over-4.26).abs()<0.15,"overshoot {over}");
        let (wobble,rest,over)=swing(70.0,45.0,1.0/60.0);
        assert_eq!((wobble,rest),(24,63));assert!((over-3.83).abs()<0.15,"overshoot {over}");
        assert!(swing(-70.0,45.0,1.0/60.0).1==63,"the sign of Obrot does not change the timing");
    }
    #[test] fn retail_hinge_closes_as_fast_and_passes_the_frame_first() {
        let mut door=hinge(90.0,45.0,serde_json::json!({}));door.activate(true,Vec3::ZERO);
        for _ in 0..200 {door.advance(1.0/60.0);}
        assert_eq!(door.state_code(),OPEN);assert!((door.ang-90f32.to_radians()).abs()<1e-5);
        assert!(door.activate(true,Vec3::ZERO));assert!(!door.is_open());
        let mut lowest=0.0f32;let mut frames=0;
        while door.state_code()!=CLOSED && frames<300 {door.advance(1.0/60.0);lowest=lowest.min(door.ang);frames+=1;}
        assert_eq!(frames,67);assert!(lowest < -0.05,"the closing wobble swings slightly past closed first: {lowest}");assert_eq!(door.ang,0.0);
    }
    #[test] fn a_door_with_zero_speed_still_moves_because_it_accelerates() {
        let mut door=hinge(90.0,0.0,serde_json::json!({}));door.activate(true,Vec3::ZERO);
        for _ in 0..240 {door.advance(1.0/60.0);}
        assert_eq!(door.state_code(),OPEN);
    }
    #[test] fn od_gracza_swings_the_leaf_away_from_the_activator() {
        let door=Vec3::new(10.0,0.0,10.0);
        // Przesuniecie_osi (0,0,48): the decision line runs along (49,-47), so the activator's side flips the sign of Obrot.
        assert!(!od_gracza_negative(Vec3::new(0.0,0.0,48.0),door,door+Vec3::new(100.0,0.0,0.0)));
        assert!(od_gracza_negative(Vec3::new(0.0,0.0,48.0),door,door+Vec3::new(-100.0,0.0,0.0)));
        assert!(od_gracza_negative(Vec3::ZERO,door,door+Vec3::X*50.0) && od_gracza_negative(Vec3::ZERO,door,door-Vec3::X*50.0),"a hinge at the origin always opens negative");
        let mut leaf=hinge(90.0,45.0,serde_json::json!({"Od_gracza":1}));
        assert!(leaf.activate(true,Vec3::new(-100.0,0.0,0.0)));assert!(leaf.obrot<0.0);
        leaf.state=OPEN;assert!(leaf.activate(true,Vec3::new(100.0,0.0,0.0)));assert!(leaf.obrot<0.0,"the sign persists for the close");
    }
    #[test] fn gracz_otwiera_zero_refuses_the_player_but_not_a_chain_and_auto_close_waits_two_seconds() {
        let mut locked=hinge(90.0,45.0,serde_json::json!({"Gracz_otwiera":0,"Czas_samozamkniecia":6.0}));
        assert!(!locked.activate(true,Vec3::ZERO) && locked.state_code()==CLOSED);assert!(locked.activate(false,Vec3::ZERO));
        for _ in 0..200 {locked.advance(1.0/60.0);}
        assert!(locked.wants_close(),"open, Czas set and more than 2 s since the last attempt");
        let mut fresh=hinge(90.0,45.0,serde_json::json!({"Czas_samozamkniecia":2.0}));fresh.activate(true,Vec3::ZERO);
        for _ in 0..90 {fresh.advance(1.0/60.0);}
        assert!(!fresh.wants_close(),"only 1.5 s since the activation: not yet ({})",fresh.timer);
        for _ in 0..40 {fresh.advance(1.0/60.0);}
        assert!(fresh.wants_close());assert!(!hinge(90.0,45.0,serde_json::json!({"Czas_samozamkniecia":0.0})).wants_close());
        fresh.state=OPEN;fresh.activate(false,Vec3::ZERO);assert!(fresh.state_code()==-1,"closing again restarts nothing else");
    }
    #[test] fn transparent_brushes_follow_the_user_flag_rules() {
        let bounds=(Vec3::splat(-5.0),Vec3::splat(5.0));
        let glass=Door::from_properties("g","b_transparent",&serde_json::json!({"Solid":1,"Alpha":0.5}),bounds.0,bounds.1);
        assert!(glass.solid && !glass.occludes_shots && !glass.usable() && !glass.wants_close() && !glass.moving(),"solid glass blocks movement only");
        assert!(glass.sweep(Vec3::new(30.0,0.0,0.0),Vec3::ONE,Vec3::new(-40.0,0.0,0.0)).is_some());
        assert!(glass.shot_hit(Vec3::new(30.0,0.0,0.0),-Vec3::X,100.0).is_none(),"bullets and NPC sight pass (user flag 0x2)");
        let wall=Door::from_properties("w","b_transparent_nieprzestrzelny",&serde_json::json!({"Solid":1}),bounds.0,bounds.1);
        assert!(wall.occludes_shots && wall.shot_hit(Vec3::new(30.0,0.0,0.0),-Vec3::X,100.0).is_some());
        let ghost=Door::from_properties("x","b_transparent_nieprzestrzelny",&serde_json::json!({"Solid":0}),bounds.0,bounds.1);
        assert!(!ghost.solid && !ghost.occludes_shots && ghost.sweep(Vec3::new(30.0,0.0,0.0),Vec3::ONE,Vec3::new(-40.0,0.0,0.0)).is_none(),"Solid 0: nothing collides");
    }
    #[test] fn a_low_brush_reports_its_top_for_the_stair_rule() {
        // podziemia1c b_transparent3: a flat 64x64 plate 2 units above the feet blocks sideways (sweep) but its top is within one stair (18).
        let plate=Door::from_properties("p","b_transparent",&serde_json::json!({"Solid":1}),Vec3::new(-32.0,-198.0,-32.0),Vec3::new(32.0,-198.0,32.0));
        assert!((plate.top()+198.0).abs()<1e-3);
        let half=Vec3::new(17.0,58.0,17.0);let feet=-200.0;
        assert!(plate.sweep(Vec3::new(0.0,feet+half.y,-60.0),half,Vec3::Z*40.0).is_some(),"the side contact exists");
        assert!(plate.top()-feet>0.0 && plate.top()-feet<=retail_movement::STAIR_HEIGHT,"so the correction pass steps onto it");
    }
    #[test] fn aim_padding_cannot_reach_through_a_wall() {
        let door=leaf();
        let wall=retail_movement::CollisionWorld::from_obj("v -100 -100 4\nv 100 -100 4\nv 100 100 4\nv -100 100 4\nf 1 2 3 4").unwrap();
        assert!(nearest_target(Vec3::Z*10.0,-Vec3::Z,&wall,&[&door]).is_none());
    }
    #[test] fn solid_leaf_stops_shots_but_authored_grate_does_not() {
        let mut door=leaf();
        let hit=door.shot_hit(Vec3::Z*20.0,-Vec3::Z,100.0).unwrap();
        assert!((hit.0-18.0).abs()<0.001 && hit.1.z>0.99);
        door.ang=door.obrot.abs();door.state=OPEN;
        let hit=door.shot_hit(Vec3::X*20.0,-Vec3::X,100.0).unwrap();
        assert!((hit.0-18.0).abs()<0.001 && hit.1.x>0.99);
        let grate=Door::from_properties("gate","b_szuflada_przestrzelna",&serde_json::json!({"Solid":1}),Vec3::splat(-2.0),Vec3::splat(2.0));
        assert!(!grate.occludes_shots);
        assert!(grate.shot_hit(Vec3::Z*20.0,-Vec3::Z,100.0).is_none());
        assert!(grate.sweep(Vec3::Z*20.0,Vec3::ONE,-Vec3::Z*40.0).is_some());
    }
    #[test] fn exported_prison_exit_doors_are_usable_from_probe_poses() {
        let output=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
        for (world,name,position,target,destination) in [
            ("rh1-wiezienie2","b_door21",Vec3::new(-1232.0,-190.0,190.0),Vec3::new(-1232.0,-176.0,80.0),"rh1-wiezienie3"),
            ("rh1-wiezienie3","b_door0",Vec3::new(2500.0,-48.0,-144.0),Vec3::new(2620.0,-48.0,-144.0),"rh2-wiezienie1")
        ] {
            let scene:Value=serde_json::from_str(&std::fs::read_to_string(output.join(format!("{world}.scene.json"))).unwrap()).unwrap();
            let object=scene["objects"].as_array().unwrap().iter().find(|o|o["properties"]["Name"]==name).unwrap();
            let source=std::fs::read_to_string(output.join(format!("world_models/{world}/{name}.visual.obj"))).unwrap();
            let mut low=Vec3::splat(f32::INFINITY);let mut high=-low;
            for mesh in level_viewer::read_obj(&source).unwrap() {for p in mesh.positions {low=low.min(Vec3::from(p));high=high.max(Vec3::from(p));}}
            let door=Door::from_properties(name,"b_door",&object["properties"],low,high);
            let collision=retail_movement::CollisionWorld::from_obj(&std::fs::read_to_string(output.join(format!("{world}.collision.obj"))).unwrap()).unwrap();
            let origin=position+Vec3::Y*40.0;
            let selected=nearest_target(origin,(target-origin).normalize(),&collision,&[&door]).expect(world);
            assert_eq!(selected.1,name);assert_eq!(selected.2,destination);
        }
    }

    #[test] fn first_cell_gap_remains_open_with_authored_compound_grates() {
        let output=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
        let scene:Value=serde_json::from_str(&std::fs::read_to_string(output.join("rh1-wiezienie2.scene.json")).unwrap()).unwrap();
        let world=retail_movement::CollisionWorld::from_obj(&std::fs::read_to_string(output.join("rh1-wiezienie2.collision.obj")).unwrap()).unwrap();
        let mut doors=Vec::new();
        for object in scene["objects"].as_array().unwrap() {
            let kind=object["kind"].as_str().unwrap_or("");if kind!="b_door" && !kind.starts_with("b_szuflada") {continue;}
            let name=object["properties"]["Name"].as_str().unwrap();
            let source=std::fs::read_to_string(output.join(format!("world_models/rh1-wiezienie2/{name}.visual.obj"))).unwrap();
            doors.push(Door::from_model(name,kind,&object["properties"],&source).unwrap());
        }
        let grate=doors.iter().find(|door|door.name=="kratacelagora").unwrap();
        assert!(grate.ray(Vec3::new(80.0,2.0,800.0),-Vec3::Z,0.0,200.0).is_none(),"invisible grate fills the original opening");
        assert!(grate.sweep(Vec3::new(0.0,2.0,800.0),Vec3::new(16.0,58.0,16.0),-Vec3::Z*200.0).is_some(),"actual adjacent grate must remain solid");
        let mut retail=retail_movement::Player::new(native(Vec3::new(124.0,13.0,938.0)));world.place_player(&mut retail);
        for _ in 0..240 {
            let old=render(retail.position)-Vec3::Y*retail.half_size().y;
            let p=render(retail.position);
            let delta=Vec3::new(80.0,p.y,700.0)-p;let yaw=(-delta.x).atan2(-delta.z);
            retail.tick(&world,&retail_movement::Input{forward:1.0,yaw,..default()},1.0/60.0);
            let (feet,half)=(render(retail.position)-Vec3::Y*retail.half_size().y,render(retail.half_size()));
            if let Some((fraction,_))=doors.iter().filter_map(|door|door.sweep(old+Vec3::Y*half.y,half,feet-old)).min_by(|a,b|a.0.total_cmp(&b.0)) {
                let resolved=old+(feet-old)*(fraction-0.001).max(0.0);
                retail.position=native(resolved+Vec3::Y*half.y);retail.velocity=retail_movement::Vec3::ZERO;
            }
        }
        let position=render(retail.position);
        assert!(position.z<720.0,"first cell blocked: position={position:?}");
    }
}

