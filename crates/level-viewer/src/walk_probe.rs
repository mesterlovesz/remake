//! Automated headless walk-through: MESTER_TEST_SCENARIO=walk drives the real game (movement, collision, doors, markers, dialogues, travel)
//! through consecutive campaign levels. Per level it (1) asks the mission solver which player behaviour the level script needs, (2) walks
//! there along the level's navigation graph with the ordinary player controller, (3) leaves through the exit the campaign links to the next
//! level, and logs `WALK n level: PASS | BLOCKED <what>`. MESTER_WALK_COUNT=n levels starting at the world given on the command line,
//! MESTER_WALK_LEVELS=a,b,c an explicit list, MESTER_WALK_BUDGET=game seconds per level (default 110), MESTER_WALK_SPEED=game seconds per wall second while walking (default 1; with MESTER_NOVSYNC=1 a hidden window can run 3-4x), MESTER_WALK_TIMEOUT=game seconds for the whole run, MESTER_WALK_ENDING=1 also plays credits -> main menu -> new game. The player is kept at full health.
use bevy::{ecs::system::SystemParam,prelude::*};
use mission_runtime::solver::{self,End,Live,Op};
use serde_json::Value;
use std::collections::{BTreeMap,VecDeque};
use crate::{ViewerConfig,Walking,settings::Session,campaign::Campaign,npcs::NpcRoster,doors::{Door,DoorUse},gunfire::Controls,retail_weapons::NativeArsenal,travel::Travel,opening::Opening,frontend::Frontend};

/// The 27 campaign levels in retail order (docs/campaign-research.md).
pub const ORDER:[&str;27]=["rh3-miasteczko0","rh1-wiezienie1","rh1-wiezienie2","rh1-wiezienie3","rh2-wiezienie1","rh2-wiezienie2","rh3-miasteczko1","rh3-miasteczko2","burmistrz1","burmistrz2","chapel_mniejszy","knajpa","rh7a-tunele","podziemia1","podziemia1a","podziemia1b","podziemia1c","chinatown2","rh9-fabryka","rh10-wiezowiec1","rh10-wiezowiec2","rh10-wiezowiec3","wiez_wn1","wiez_wn2","wiez_wn3","rh12-lab1","rh12-lab2"];
/// Levels whose exit is a cutscene object instead of a door or a script command.
const CUTSCENE_EXIT:[(&str,&str);2]=[("rh2-wiezienie2","ucieczka z 2 wiezienia"),("rh12-lab2","outro")];

#[derive(Clone,Debug)]
enum Goal {Marker(String,Vec3,Vec3),Pickup(String,Vec3),Talk(String),Approach(String),Kill(String),Choose(u8),Wait(f32),Door(String,String),Cutscene(String,Vec3),Skip}
impl Goal {
    fn label(&self)->String {match self {Goal::Marker(n,..)=>format!("marker {n}"),Goal::Pickup(k,_)=>format!("pickup {k}"),Goal::Talk(i)=>format!("talk {i}"),Goal::Approach(i)=>format!("near {i}"),Goal::Kill(i)=>format!("kill {i}"),
        Goal::Choose(n)=>format!("answer {n}"),Goal::Wait(s)=>format!("wait {s}"),Goal::Door(n,d)=>format!("door {n} -> {d}"),Goal::Cutscene(n,_)=>format!("cutscene {n}"),Goal::Skip=>"skip the cutscene".into()}}
}
enum Step {Working,Done,Failed(String)}
#[derive(PartialEq,Eq,Clone,Copy,Debug,Default)] enum Stage {#[default] Boot,Run,Done}

/// One step of a route: where to stand, whether crouched, whether the step into it is a jump.
type Waypoint=(Vec3,bool,bool);
/// Route over the collision world with the player controller's own step rules (`CollisionWorld::plan_walk`).
fn plan_walk(world:&retail_movement::CollisionWorld,start:Vec3,goal:Vec3,limit:usize,portals:&[(retail_movement::Vec3,retail_movement::Vec3)],props:&[(Vec3,Vec3)])->(Vec<Waypoint>,bool,usize) {
    let native=|p:Vec3|retail_movement::Vec3::new(p.x,p.y,p.z);
    let mut cells=Vec::new();let debug=std::env::var("MESTER_WALK_DEBUG").ok();
    let avoid:Vec<(retail_movement::Vec3,retail_movement::Vec3)>=props.iter().map(|(c,h)|(native(*c),native(*h))).collect();
    let (route,reached,expanded)=world.plan_walk_avoiding(native(start),native(goal),limit,portals,&avoid,debug.as_ref().map(|_|&mut cells));
    if let (false,Some(path)) =(reached,debug) {if path.ends_with(".json") {let dump:Vec<[f32;3]>=cells.iter().map(|p|[p.x,p.y,p.z]).collect();let _=std::fs::write(path,serde_json::to_string(&dump).unwrap());}}
    (route.into_iter().map(|(p,crouch,jump)|(Vec3::new(p.x,p.y,p.z),crouch,jump)).collect(),reached,expanded)
}

#[derive(Default)] struct DoorInfo {position:Vec3,destination:String,player_opens:bool}
#[derive(Default)] struct Data {portals:Vec<(retail_movement::Vec3,retail_movement::Vec3)>,doors:BTreeMap<String,DoorInfo>,markers:BTreeMap<String,(Vec3,Vec3)>,items:Vec<(String,String,Vec3)>,cutscenes:Vec<(String,Vec3)>,preds:BTreeMap<String,Vec<String>>,kinds:BTreeMap<String,String>}
fn vec3(v:&Value)->Vec3 {Vec3::new(v[0].as_f64().unwrap_or(0.0) as f32,v[1].as_f64().unwrap_or(0.0) as f32,v[2].as_f64().unwrap_or(0.0) as f32)}

#[derive(Resource,Default)]
pub struct Probe {
    pub active:bool,speed:f32,pub finished:bool,pub failure:Option<String>,pub timeout:f32,
    levels:Vec<String>,at:usize,stage:Stage,clock:f32,level_clock:f32,budget:f32,frames:u32,arrived:u64,
    data:Data,goals:VecDeque<Goal>,goal_clock:f32,attempts:u32,goal_label:String,route:Vec<Waypoint>,route_goal:Option<Vec3>,route_i:usize,route_reached:bool,route_progress:(usize,f32),
    watch:(Vec3,f32),stuck:u32,teleports:Vec<String>,notes:Vec<String>,results:Vec<String>,taps:Vec<(KeyCode,f32)>,replans:u32,walked:f32,last_pos:Option<Vec3>,
    carry:Option<(Vec<String>,String)>,last_frame:Option<std::time::Instant>,fps_frames:u32,damage:f32,npc_moved:f32,npc_noticed:usize,npc_last:Vec<Vec3>,hitches:u32,gaps:u32,worst:f32,credits_seen:bool,ending:u8,ending_clock:f32,ending_arrived:u64,door_tap:BTreeMap<String,f32>,vantage:Option<Vec3>,
}

pub fn setup(mut commands:Commands,config:Res<ViewerConfig>) {
    let active=config.capture.is_some() && std::env::var("MESTER_TEST_SCENARIO").as_deref()==Ok("walk");
    let number=|key:&str,default:f32|std::env::var(key).ok().and_then(|v|v.parse::<f32>().ok()).unwrap_or(default);
    let levels:Vec<String>=match std::env::var("MESTER_WALK_LEVELS") {
        Ok(list)=>list.split(',').map(|s|s.trim().to_ascii_lowercase()).filter(|s|!s.is_empty()).collect(),
        Err(_)=>{let first=ORDER.iter().position(|l|l.eq_ignore_ascii_case(&config.world)).unwrap_or(0);ORDER.iter().skip(first).take(number("MESTER_WALK_COUNT",1.0) as usize).map(|s|s.to_string()).collect()}
    };
    commands.insert_resource(Probe {active,speed:number("MESTER_WALK_SPEED",1.0),levels,budget:number("MESTER_WALK_BUDGET",110.0),timeout:number("MESTER_WALK_TIMEOUT",115.0),..default()});
}

/// Everything the walker touches, bundled to stay under Bevy's system parameter limit.
#[derive(SystemParam)]
pub struct Ctx<'w,'s> {
    time:Res<'w,Time>,config:Res<'w,ViewerConfig>,walking:ResMut<'w,Walking>,session:ResMut<'w,Session>,campaign:ResMut<'w,Campaign>,roster:ResMut<'w,NpcRoster>,
    doors:ResMut<'w,DoorUse>,controls:ResMut<'w,Controls>,native:ResMut<'w,NativeArsenal>,keys:ResMut<'w,ButtonInput<KeyCode>>,buttons:ResMut<'w,ButtonInput<MouseButton>>,
    opening:Res<'w,Opening>,travel:ResMut<'w,Travel>,front:ResMut<'w,Frontend>,leaves:Query<'w,'s,&'static Door>,speed:ResMut<'w,Time<Virtual>>,props:Res<'w,crate::props::PropWorld>,camera:Single<'w,'s,&'static Transform,With<crate::InspectionCamera>>,
}
fn bevy(v:retail_movement::Vec3)->Vec3 {Vec3::new(v.x,v.y,v.z)}
fn flat(a:Vec3,b:Vec3)->f32 {Vec2::new(a.x-b.x,a.z-b.z).length()}
impl Ctx<'_,'_> {
    fn position(&self)->Vec3 {bevy(self.walking.player.position)}
    fn look_at(&mut self,target:Vec3) {
        let eye=self.camera.translation/crate::SCALE;let d=(target-eye).normalize_or_zero();
        if d!=Vec3::ZERO {self.walking.yaw=(-d.x).atan2(-d.z);self.walking.pitch=d.y.asin();}
    }
    fn face_heading(&mut self,target:Vec3) {
        let p=self.position();let d=Vec2::new(target.x-p.x,target.z-p.z).normalize_or_zero();
        if d!=Vec2::ZERO {self.walking.yaw=(-d.x).atan2(-d.y);self.walking.pitch=0.0;}
    }
}

impl Probe {
    fn tap(&mut self,c:&mut Ctx,key:KeyCode) {c.keys.press(key);self.taps.push((key,0.12));}
    fn level(&self)->&str {self.levels.get(self.at).map(String::as_str).unwrap_or("")}
    /// The cut level `chinatown` (Kiskína) is not on the campaign route, but its own door b_door0 jumps to `chinatown2` (docs/cut-content.md).
    fn next_level(&self)->Option<&'static str> {if self.level()=="chinatown" {return Some("chinatown2");}let i=ORDER.iter().position(|l|*l==self.level())?;ORDER.get(i+1).copied()}
    fn log(&self,text:String) {info!("WALK {:>2} {}: {text}",self.at+1,self.level());}

    /// Reads the level's exports and decides the goals: the script's behaviour plan, then the exit.
    fn plan(&mut self,c:&mut Ctx)->Result<(),String> {
        let stem=crate::travel::exported_stem(&c.config.output,self.level()).unwrap_or(self.level().to_owned());
        let read=|name:String|serde_json::from_str::<Value>(&std::fs::read_to_string(c.config.output.join(name)).unwrap_or_else(|_|"[]".into())).unwrap_or_default();
        let (scene,gameplay,items)=(read(format!("{stem}.scene.json")),read(format!("{stem}.gameplay.json")),read(format!("{stem}.items.json")));
        let mut data=Data::default();
        for object in scene["objects"].as_array().into_iter().flatten() {
            let (p,kind)=(&object["properties"],object["kind"].as_str().unwrap_or(""));
            let Some(name)=p["Name"].as_str() else {continue};
            data.kinds.insert(name.to_owned(),kind.to_owned());
            if let Some(next)=p["Nast_obiekt"].as_str().filter(|n|!n.is_empty()) {data.preds.entry(next.to_owned()).or_default().push(name.to_owned());}
            match kind {
                "b_door"|"b_szuflada"|"b_szuflada_przestrzelna"=>{data.doors.insert(name.to_owned(),DoorInfo {position:vec3(&p["Pos"]),destination:crate::doors::destination(p).to_ascii_lowercase(),player_opens:p["Gracz_otwiera"].as_i64()!=Some(0)});},
                k if k.starts_with("o_marker_")=>{data.markers.insert(name.to_owned(),(vec3(&p["Pos"]),vec3(&p["Promien"])));},
                "o_cutscene"=>data.cutscenes.push((p["Rodzaj_cuts"].as_str().unwrap_or("").to_owned(),vec3(&p["Pos"]))),
                _=>{}
            }
        }
        for item in items.as_array().into_iter().flatten() {data.items.push((item["name"].as_str().unwrap_or("").to_owned(),item["kind"].as_str().unwrap_or("").to_owned(),vec3(&item["pos"])));}
        data.portals=c.leaves.iter().filter_map(|d|{let (low,high)=d.bounds();retail_movement::CollisionWorld::portal_of(retail_movement::Vec3::new(low.x,low.y,low.z),retail_movement::Vec3::new(high.x,high.y,high.z))}).collect();
        if std::env::var_os("MESTER_WALK_DOORDEBUG").is_some() {for d in c.leaves.iter() {let (lo,hi)=d.bounds();info!("WALK leaf {} blocks {} usable {} bounds {} .. {}",d.name,d.blocks(),d.player_opens(),fmt(lo),fmt(hi));}}
        self.goals.clear();
        if self.level()=="rh1-wiezienie1" {self.goals.push_back(Goal::Skip);self.data=data;return Ok(());}
        // 1. What the mission script needs before it ends the level itself.
        let mut script_exit=false;
        if let Some(mission)=c.campaign.mission.clone() {
            script_exit=mission.ends_level();
            if script_exit {
                let world=solver::World::from_exports(&scene,&gameplay,&items);
                let live=Live {alive:world.npcs.iter().map(|n|c.roster.actors.iter().find(|a|a.name==n.id).is_none_or(|a|a.alive())).collect(),
                    phase:world.npcs.iter().map(|n|c.roster.actors.iter().find(|a|a.name==n.id).map(|a|a.phase.clone()).unwrap_or_default()).collect(),inventory:c.campaign.items.names(),draw:false};
                let report=solver::solve_from(&mission,&world,&live,40000);
                match report.end {
                    Some(End::Transition(_)|End::Cutscene(_))=>{
                        self.log(format!("script plan ({} states): {:?}",report.explored,report.plan));
                        for op in &report.plan {match op {
                            Op::Talk(id)=>self.goals.push_back(Goal::Talk(id.clone())),Op::Approach(id)=>self.goals.push_back(Goal::Approach(id.clone())),Op::Kill(id)=>self.goals.push_back(Goal::Kill(id.clone())),
                            Op::Choose(n)=>self.goals.push_back(Goal::Choose(*n)),Op::Wait(s)=>self.goals.push_back(Goal::Wait(*s)),Op::Draw(_)=>{},
                            Op::Marker(name)=>{let (centre,half)=data.markers.get(name).copied().unwrap_or_default();self.goals.push_back(Goal::Marker(name.clone(),centre,half));},
                            Op::Pickup(kind)=>if let Some((name,_,at))=data.items.iter().find(|i|&i.1==kind) {self.goals.push_back(Goal::Pickup(name.clone(),*at));},
                        }}
                    },
                    _=>return Err(format!("SCRIPT DEADLOCK: no player behaviour ends the level (explored {}, flags {:?})",report.explored,report.flags)),
                }
            }
        }
        // 2. The exit the campaign links to the next level.
        if !script_exit {
            let next=self.next_level().unwrap_or("").to_owned();
            if let Some((_,scene_name))=CUTSCENE_EXIT.iter().find(|(l,_)|*l==self.level()) {
                let (_,at)=data.cutscenes.iter().find(|(n,_)|n==scene_name).cloned().ok_or(format!("no o_cutscene '{scene_name}' in the level"))?;
                self.goals.push_back(Goal::Cutscene(scene_name.to_string(),at));
            } else {
                let start=bevy(c.walking.player.position);
                let mut exits:Vec<(&String,&DoorInfo)>=data.doors.iter().filter(|(_,d)|d.destination==next).collect();
                exits.sort_by(|a,b|a.1.position.distance(start).total_cmp(&b.1.position.distance(start)));
                let Some((name,door))=exits.first().copied() else {return Err(format!("the level has no door to {next}"))};
                if !door.player_opens {
                    // Chain-only exit: walk into the marker (or reach the lever) that fires it.
                    let mut roots=Vec::new();let mut stack=vec![name.clone()];let mut seen=std::collections::BTreeSet::new();
                    while let Some(x)=stack.pop() {for q in data.preds.get(&x).into_iter().flatten() {if seen.insert(q.clone()) {stack.push(q.clone());if data.kinds.get(q).is_some_and(|k|k.starts_with("o_marker_")) {roots.push(q.clone());}}}}
                    let Some(root)=roots.first() else {return Err(format!("exit door {name} cannot be opened by the player and no marker fires it"))};
                    let (centre,half)=data.markers.get(root).copied().unwrap_or_default();self.goals.push_back(Goal::Marker(root.clone(),centre,half));
                    self.goals.push_back(Goal::Wait(3.0));
                } else {self.goals.push_back(Goal::Door(name.clone(),door.destination.clone()));}
            }
        }
        self.data=data;Ok(())
    }

    /// Walks toward `target` along the planned route; true once within `radius` (horizontally). Where the planner finds no walkable
    /// link the walker goes to the closest reachable point and steps over the gap (a logged teleport).
    fn go(&mut self,c:&mut Ctx,target:Vec3,radius:f32,dt:f32)->bool {
        let p=c.position();
        if flat(p,target)<radius && (p.y-target.y).abs()<170.0 {self.release_walk(c);return true;}
        if self.route_goal.is_none_or(|g|g.distance(target)>100.0) || self.route.is_empty() {
            let props=solid_props(c);let (route,reached,expanded)=plan_walk(&c.walking.world,p,target,400000,&self.data.portals,&props);
            if !reached {self.notes.push(format!("no walkable route from {} to {} ({expanded} cells searched); closest {}",fmt(p),fmt(target),fmt(route.last().unwrap().0)));
                // Is a solid prop what closes the way? Plan again ignoring them and name the props on that route.
                if !props.is_empty() {let (free,ok,_)=plan_walk(&c.walking.world,p,target,400000,&self.data.portals,&[]);
                    if ok {let mut names=Vec::new();for pair in free.windows(2) {let (a,b)=(pair[0].0,pair[1].0);let n=((a.distance(b)/12.0).ceil() as usize).max(1);
                        for i in 0..=n {let q=a.lerp(b,i as f32/n as f32);for prop in c.props.field().props.iter().filter(|x|x.solid && !x.dead) {if (q-prop.center).abs().cmplt(prop.half+Vec3::new(17.0,58.0,17.0)).all() && !names.contains(&prop.name) {names.push(prop.name.clone());}}}}
                        self.notes.push(format!("the route exists only through solid props {names:?}"));}}}
            self.route=route;self.route_reached=reached;self.route_goal=Some(target);self.route_i=0;
        }
        let last=self.route.len()-1;
        while self.route_i<last && flat(p,self.route[self.route_i].0)<26.0 && (p.y-self.route[self.route_i].0.y).abs()<80.0 {self.route_i+=1;}
        // The end of a route that never reached the target: cross the gap.
        if self.route_i==last && !self.route_reached && flat(p,self.route[last].0)<80.0 {
            // A target the player keeps being pushed off (a prop on the spot) would replan and teleport forever: after three gap steps the goal counts as reached.
            self.gaps+=1;if self.gaps>3 {self.notes.push(format!("gave up reaching {} after {} gap steps",fmt(target),self.gaps-1));self.release_walk(c);return true;}
            self.teleports.push(format!("gap {}->{}",fmt(p),fmt(target)));
            crate::campaign_probe::place(&mut c.walking,target,target+Vec3::Z*100.0);
            self.route.clear();self.route_goal=None;self.release_walk(c);return false;
        }
        let (waypoint,crouch,_)=self.route[self.route_i];
        let here=flat(p,waypoint);
        c.face_heading(waypoint);c.keys.press(KeyCode::KeyW);c.keys.press(KeyCode::ShiftLeft);
        let want_crouch=crouch || self.route.get(self.route_i+1).is_some_and(|n|n.1 && here<48.0);
        if want_crouch {c.keys.press(KeyCode::ControlLeft);}else {c.keys.release(KeyCode::ControlLeft);}
        // Jump: the step into the next point rises or crosses a gap; jump when close to the edge.
        let jump_now=self.route.get(self.route_i).is_some_and(|w|w.2) && here<70.0 || (waypoint.y-p.y>20.0 && here<60.0);
        if jump_now {c.buttons.press(MouseButton::Right);}else if self.stuck!=2 {c.buttons.release(MouseButton::Right);}
        // Doors: open a closed leaf ahead before walking into it (E on it, or on the player-usable door whose chain opens it).
        let heading=(waypoint-p).with_y(0.0).normalize_or_zero();
        let ahead=c.leaves.iter().filter(|d|d.blocks()).map(|d|(d.center().distance(p),d)).filter(|(distance,d)|*distance<130.0 && (d.center()-p).with_y(0.0).normalize_or_zero().dot(heading)>0.35).min_by(|a,b|a.0.total_cmp(&b.0)).map(|(_,d)|(d.name.clone(),d.player_opens()));
        if let Some((name,usable)) = ahead {
            let opener=if usable {Some(name.clone())} else {self.opener_of(&name,c)};
            match opener {
                Some(opener) if self.door_tap.get(&opener).is_none_or(|t|self.level_clock-*t>1.5) => {
                    if let Some(centre)=c.leaves.iter().find(|d|d.name==opener).map(|d|d.center()) {
                        // Stand still, aim from the camera, and press E once the camera has settled (a small switch is a 12-unit box).
                        c.keys.release(KeyCode::KeyW);c.look_at(centre);
                        let still=self.last_pos.is_some_and(|l|l.distance(p)<0.3);
                        if !still {return false;}
                        if !self.door_tap.contains_key(&opener) {self.notes.push(format!("E on {opener} for the closed {name} ahead at {}",fmt(p)));}
                        self.door_tap.insert(opener.clone(),self.level_clock);
                        if std::env::var_os("MESTER_WALK_DOORDEBUG").is_some() {info!("WALK E on {opener}: player {} centre {} target {:?}",fmt(p),fmt(centre),c.doors.target_name);}
                        c.doors.request_name=Some(opener);
                    }
                },
                None if self.door_tap.get(&name).is_none() => {self.door_tap.insert(name.clone(),self.level_clock);self.notes.push(format!("closed door {name} ahead has no player-usable opener within reach"));},
                _=>{}
            }
        }
        // Watchdog: the route index has not advanced for 8 s (sliding back and forth on a slope, a prop in the way): step to the waypoint.
        if self.route_progress.0!=self.route_i {self.route_progress=(self.route_i,self.level_clock);}
        else if self.level_clock-self.route_progress.1>8.0 && c.campaign.dialogue.is_none() {
            self.route_progress.1=self.level_clock;self.teleports.push(format!("stalled {}->{}",fmt(p),fmt(waypoint)));
            crate::campaign_probe::place(&mut c.walking,waypoint,waypoint+Vec3::Z*100.0);self.route_i=(self.route_i+1).min(last);
        }
        // Stuck: jump, then try the nearest closed door ahead, then step the player to the waypoint (logged).
        self.watch.1+=dt;
        if self.watch.1>=0.5 {
            let moved=flat(p,self.watch.0);self.watch=(p,0.0);self.stuck=if moved<5.0 {self.stuck+1} else {0};
            if self.stuck==2 {c.buttons.press(MouseButton::Right);}
            if (3..10).contains(&self.stuck) {
                let heading=(waypoint-p).with_y(0.0).normalize_or_zero();
                let door=c.leaves.iter().filter(|d|d.player_opens() && d.state().1<0.6).map(|d|(d.center().distance(p),d)).filter(|(distance,d)|*distance<320.0 && (d.center()-p).with_y(0.0).normalize_or_zero().dot(heading)>-0.2).min_by(|a,b|a.0.total_cmp(&b.0));
                if let Some((_,door))=door {let (name,centre)=(door.name.clone(),door.center());c.look_at(centre);c.doors.request_name=Some(name);}
            }
            if self.stuck>=10 {
                self.stuck=0;
                // What is in the way? (doors, props, hull) for the report.
                let ahead=(waypoint-p).with_y(0.0).normalize_or_zero()*12.0;let half=c.walking.player.half_size();let half=Vec3::new(half.x,half.y,half.z);
                let mut blockers:Vec<String>=c.leaves.iter().filter(|d|d.sweep(p,half,ahead).is_some()).map(|d|d.name.clone()).collect();
                {let field=c.props.field();blockers.extend(field.props.iter().filter(|x|x.solid && !x.dead && crate::props::sweep_box(p,half,ahead,x.center,x.half).is_some()).map(|x|x.name.clone()));}
                if c.walking.world.sweep_box(retail_movement::Vec3::new(p.x,p.y,p.z),retail_movement::Vec3::new(half.x,half.y,half.z),retail_movement::Vec3::new(ahead.x,ahead.y,ahead.z),0.0).is_some() {blockers.push("hull".into());}
                self.teleports.push(format!("stuck {}->{} by {blockers:?}",fmt(p),fmt(waypoint)));
                crate::campaign_probe::place(&mut c.walking,waypoint,waypoint+Vec3::Z*100.0);self.route_i=(self.route_i+1).min(last);
            }
        }
        false
    }
    /// A player-usable door within reach whose Nast_obiekt chain ends at `target` (the gate row of wiez_wn1: E on the first opens all).
    fn opener_of(&self,target:&str,c:&Ctx)->Option<String> {
        let mut stack=vec![target.to_owned()];let mut seen=std::collections::BTreeSet::new();let p=c.position();
        while let Some(x)=stack.pop() {
            for q in self.data.preds.get(&x).into_iter().flatten() {
                if !seen.insert(q.clone()) {continue;}
                stack.push(q.clone());
                if let Some(d)=c.leaves.iter().find(|d|&d.name==q) {if d.player_opens() && d.center().distance(p)<170.0 {return Some(q.clone());}}
            }
        }
        None
    }
    fn release_walk(&mut self,c:&mut Ctx) {c.keys.release(KeyCode::KeyW);c.keys.release(KeyCode::ControlLeft);c.buttons.release(MouseButton::Right);}

    fn goal_step(&mut self,c:&mut Ctx,goal:&Goal,dt:f32)->Step {
        match goal {
            Goal::Skip=>{
                // MESTER_WALK_NOSKIP=1 lets the whole intro play (78 s + `intro zwei`) instead of holding the use key.
                if c.opening.active {if std::env::var_os("MESTER_WALK_NOSKIP").is_none() {c.keys.press(KeyCode::KeyE);}Step::Working}
                else {c.keys.release(KeyCode::KeyE);if self.goal_clock>3.0 {Step::Done}else{Step::Working}}
            },
            Goal::Wait(seconds)=>{self.release_walk(c);if self.goal_clock>=*seconds {Step::Done}else{Step::Working}},
            Goal::Marker(name,centre,half)=>{
                // Inside the volume for a second (the variable marker repeats every 0.2 s).
                let radius=(half.x.min(half.z)*0.5).clamp(10.0,60.0);
                // The game fires a marker as soon as the player's box touches its volume (activation.rs): no need to reach the centre (a limousine can stand in the way).
                let hull={let h=c.walking.player.half_size();Vec3::new(h.x,h.y,h.z)};
                let inside=(c.position()-*centre).abs().cmple(*half+hull).all();
                if !inside && !self.go(c,*centre,radius,dt) {self.goal_clock=0.0;return Step::Working;}
                self.release_walk(c);
                if self.goal_clock>1.3 {Step::Done}else if self.goal_clock>10.0 {Step::Failed(format!("marker {name} unreachable"))}else{Step::Working}
            },
            Goal::Pickup(name,at)=>{
                if c.campaign.collected.contains(name) {c.keys.release(KeyCode::KeyE);self.release_walk(c);return Step::Done;}
                // An item on a table or a shelf is taken with the held action key: the 64-unit cube 64 units ahead of the eye (pickups.rs, cshell 0x10022470).
                let eye=c.camera.translation/crate::SCALE;
                if eye.distance(*at)<70.0 && flat(c.position(),*at)>18.0 {
                    if self.attempts==0 {self.attempts=1;self.goal_clock=0.0;}
                    self.release_walk(c);c.look_at(*at);c.keys.press(KeyCode::KeyE);if std::env::var_os("MESTER_WALK_DOORDEBUG").is_some() && (self.goal_clock*4.0) as u32!=((self.goal_clock-dt)*4.0) as u32 {info!("WALK use-hold at {} eye {:?} item {} dialogue {} pressed {}",fmt(c.position()),c.camera.translation/crate::SCALE,fmt(*at),c.session.dialogue_active,c.keys.pressed(KeyCode::KeyE));}
                    // A dialogue that starts on the spot freezes the player: the clock only counts while it is not running.
                    if c.session.dialogue_active {self.goal_clock=0.0;}
                    if self.goal_clock>4.0 {c.keys.release(KeyCode::KeyE);self.notes.push(format!("{} was not taken with the held use key from {}",self.goal_label,fmt(c.position())));return Step::Failed("the use cube did not take the item".into());}
                    return Step::Working;
                }
                c.keys.release(KeyCode::KeyE);
                if !self.go(c,*at,18.0,dt) {return Step::Working;}
                self.release_walk(c);if self.goal_clock>0.8 {Step::Done}else{Step::Working}
            },
            Goal::Talk(id)|Goal::Approach(id)=>{
                let Some(actor)=c.roster.actors.iter().find(|a|&a.name==id).map(|a|(a.position,a.eye(),a.alive())) else {return Step::Failed(format!("character {id} is not in the level"))};
                if !actor.2 {return Step::Failed(format!("{id} is dead"));}
                if !self.go(c,actor.0,70.0,dt) {self.goal_clock=0.0;return Step::Working;}
                self.release_walk(c);c.look_at(actor.1);
                if matches!(goal,Goal::Approach(_)) {return if self.goal_clock>0.8 {Step::Done}else{Step::Working};}
                if c.campaign.dialogue.is_some() {return Step::Done;}
                if self.goal_clock>self.attempts as f32*1.5 && self.attempts<6 {self.attempts+=1;self.tap(c,KeyCode::KeyE);}
                if self.attempts>=6 && self.goal_clock>10.0 {Step::Failed(format!("{id} did not start a dialogue on use"))}else{Step::Working}
            },
            Goal::Choose(n)=>{
                self.release_walk(c);
                let ready=c.campaign.mission.as_ref().is_some_and(|m|m.choices_ready());
                let Some(dialogue)=c.campaign.dialogue.clone().filter(|d|!d.choices.is_empty()) else {
                    return if self.goal_clock>8.0 {Step::Failed(format!("no dialogue offers answer {n}"))}else{Step::Working};
                };
                if !ready {return Step::Working;}
                if !dialogue.choices.iter().any(|ch|ch.index==*n) {return Step::Failed(format!("dialogue {} has no answer {n}",dialogue.id));}
                if self.attempts==0 {self.attempts=1;self.goal_clock=0.0;
                    // The dialogue takes its answers with the mouse (dialogue.rs); the probe takes answer n straight from the mission, like a click on that row.
                    let events=c.campaign.mission.as_mut().map(|m|m.choose(*n)).unwrap_or_default();c.campaign.queue(events);}
                if self.goal_clock>0.5 {Step::Done}else{Step::Working}
            },
            Goal::Kill(id)=>{
                let Some(actor)=c.roster.actors.iter().find(|a|&a.name==id).map(|a|(a.position,a.eye(),a.alive())) else {return Step::Failed(format!("character {id} is not in the level"))};
                if !actor.2 {return Step::Done;}
                // The M-14 (the prologue's sniper rifle) for far targets, else the Glock.
                let far=c.position().distance(actor.0)>1500.0;let gun=if far {"M-14"} else {"Glock"};
                // Keys 1..8 draw the holster cell (retail_weapons.rs): make sure the gun sits in the item grid (a real player got it by dialogue/pickup) and press its key.
                c.native.acquire(gun);
                let key=|c:&Ctx|c.campaign.items.entries.iter().find(|e|e.item.eq_ignore_ascii_case(gun) && e.row>=crate::inventory::HOLSTER_ROW && e.row<crate::inventory::BELT_ROW).map(|e|(e.column+crate::inventory::COLUMNS*(e.row-crate::inventory::HOLSTER_ROW)) as usize);
                if key(c).is_none() {c.campaign.items.add(gun,1,true);self.notes.push(format!("{gun} was not in the holsters: added it for the shot"));}
                c.controls.native_slot=key(c);
                let eye=c.position()+Vec3::Y*40.0;
                let in_sight=crate::npcs::line_of_sight(&c.walking.world,eye,actor.1);
                // No line: walk to the nearest reachable spot that has one.
                if !in_sight && self.vantage.is_none() && self.attempts==0 {
                    self.attempts=1;
                    match vantage(&c.walking.world,c.position(),actor.1,&self.data.portals) {
                        Some(spot)=>{self.notes.push(format!("{id} is not in sight from {}: shooting from {}",fmt(c.position()),fmt(spot)));self.vantage=Some(spot);},
                        None=>return Step::Failed(format!("no walkable spot with a line to {id}")),
                    }
                }
                if !in_sight {
                    if let Some(spot)=self.vantage {if !self.go(c,spot,30.0,dt) {self.goal_clock=0.0;return Step::Working;}}
                    self.release_walk(c);
                    if self.goal_clock>60.0 {return Step::Failed(format!("{id} is out of sight"));}
                    if self.vantage.is_none() {return Step::Working;}
                }
                self.release_walk(c);c.look_at(actor.1);
                if self.goal_clock>1.0 && (self.goal_clock*2.0) as u32!=((self.goal_clock-dt)*2.0) as u32 {c.controls.fire=true;c.controls.held=true;}
                if self.goal_clock>40.0 {Step::Failed(format!("{id} survived 40 s of shooting"))}else{Step::Working}
            },
            Goal::Door(name,destination)=>{
                let Some(leaf)=c.leaves.iter().find(|d|&d.name==name).map(|d|d.center()) else {return Step::Failed(format!("door {name} has no leaf"))};
                if !self.go(c,leaf,105.0,dt) {self.goal_clock=0.0;return Step::Working;}
                self.release_walk(c);c.look_at(leaf);c.doors.request_name=Some(name.clone());
                if (self.goal_clock*2.0) as u32!=((self.goal_clock-dt)*2.0) as u32 {let p=c.position();info!("WALK door {name}: player {} leaf {} distance {:.0} hint {:?} target {:?} camera {} forward {:?} yaw {:.2} pitch {:.2}",fmt(p),fmt(leaf),p.distance(leaf),c.doors.hint,c.doors.target_name,fmt(c.camera.translation/crate::SCALE),*c.camera.forward(),c.walking.yaw,c.walking.pitch);
                if let Some(d)=c.leaves.iter().find(|d|&d.name==name) {info!("WALK door aim {:?}",d.debug_aim(c.camera.translation/crate::SCALE,*c.camera.forward()));for other in c.leaves.iter().filter(|o|&o.name!=name) {let aim=other.debug_aim(c.camera.translation/crate::SCALE,*c.camera.forward());if aim.2.is_some() {info!("WALK door aim also hits {} {:?}",other.name,aim);}}}}
                if self.goal_clock>6.0 {let p=c.position();let near:Vec<String>=c.leaves.iter().filter(|d|d.center().distance(p)<400.0).map(|d|format!("{} open {:?} blocks {} usable {}",d.name,d.state(),d.blocks(),d.player_opens())).collect();Step::Failed(format!("door {name} to {destination} did not respond ({:?}); leaves near: {near:?}",c.doors.hint))}else{Step::Working}
            },
            Goal::Cutscene(name,at)=>{
                if c.opening.active {self.release_walk(c);return Step::Working;}
                if !self.go(c,*at,50.0,dt) {return Step::Working;}
                self.release_walk(c);if self.goal_clock>8.0 {Step::Failed(format!("cutscene {name} did not start at its object"))}else{Step::Working}
            },
        }
    }
}
/// Living solid props (boxes the player cannot walk through; the static hull does not know them).
fn solid_props(c:&Ctx)->Vec<(Vec3,Vec3)> {c.props.field().props.iter().filter(|p|p.solid && !p.dead).map(|p|(p.center,p.half)).collect()}
fn fmt(v:Vec3)->String {format!("({:.0},{:.0},{:.0})",v.x,v.y,v.z)}

/// The reachable standing spot closest to `from` that has a clear line to `target` (a sniper's vantage point).
fn vantage(world:&retail_movement::CollisionWorld,from:Vec3,target:Vec3,portals:&[(retail_movement::Vec3,retail_movement::Vec3)])->Option<Vec3> {
    let native=|p:Vec3|retail_movement::Vec3::new(p.x,p.y,p.z);
    let mut cells=Vec::new();
    world.plan_walk(native(from),native(target),250000,portals,Some(&mut cells));
    let mut spots:Vec<Vec3>=cells.iter().map(|p|Vec3::new(p.x,p.y,p.z)).collect();
    spots.sort_by(|a,b|a.distance(from).total_cmp(&b.distance(from)));
    spots.into_iter().find(|s|s.distance(target)<9000.0 && crate::npcs::line_of_sight(world,*s+Vec3::Y*40.0,target))
}

pub fn tick(mut p:ResMut<Probe>,mut c:Ctx) {
    if !p.active || p.finished {return;}
    let dt=c.time.delta_secs().min(0.05);let real=c.time.delta_secs();
    p.clock+=real;
    // Wall-clock frame time: Bevy clamps `real`, so a multi-second hitch would otherwise be invisible in the level clock.
    p.fps_frames+=1;
    let now=std::time::Instant::now();if let Some(last)=p.last_frame.replace(now) {let wall=now.duration_since(last).as_secs_f32();if wall>p.worst {p.worst=wall;}if wall>0.5 {p.hitches+=1;let at=c.position();let goal=p.goal_label.clone();p.log(format!("HITCH frame took {wall:.2} s wall at {} goal '{goal}'",fmt(at)));}}
    // Release finished key taps.
    let mut taps=std::mem::take(&mut p.taps);for (key,left) in &mut taps {*left-=real;if *left<=0.0 {c.keys.release(*key);}}taps.retain(|(_,left)|*left>0.0);p.taps=taps;
    // MESTER_WALK_DEATH=1: F5 at 2 s, death at 5 s (the shell message and default phases), F9 at 9 s, verdict at 14 s.
    if std::env::var_os("MESTER_WALK_DEATH").is_some() {
        if c.front.loading.is_none() && c.session.paused && !c.front.main_active {c.session.paused=false;}
        let t=p.clock;
        if (2.0..2.0+real*1.5).contains(&t) {p.tap(&mut c,KeyCode::F5);}
        if t>5.0 && t<5.0+real*1.5 {c.campaign.health=0.0;}
        if t>9.0 && t<9.0+real*1.5 {p.tap(&mut c,KeyCode::F9);}
        if t>=14.0 {p.log(format!("death probe: health {:.0} dead {} world {} dialogue {}",c.campaign.health,c.campaign.dead(),c.config.world,c.session.dialogue_active));p.finished=true;}
        let taps=std::mem::take(&mut p.taps);let mut kept=Vec::new();for (key,left) in taps {if left-real<=0.0 {c.keys.release(key);}else{kept.push((key,left-real));}}p.taps=kept;
        return;
    }
    // Regression metrics: what the merged AI did to the (invulnerable) bot, whether characters walk and whether they noticed the player.
    p.damage+=(c.campaign.max_health-c.campaign.health).max(0.0);
    {let now:Vec<Vec3>=c.roster.actors.iter().map(|a|if a.alive() {a.position}else{Vec3::splat(f32::MAX)}).collect();
     if now.len()==p.npc_last.len() {let moved:f32=now.iter().zip(&p.npc_last).map(|(a,b)|a.distance(*b)).filter(|d|*d<300.0).sum();p.npc_moved+=moved;}
     p.npc_last=now;p.npc_noticed=p.npc_noticed.max(c.roster.actors.iter().filter(|a|a.noticed_player()).count());}
    c.campaign.health=c.campaign.max_health;
    if c.front.loading.is_none() && c.session.paused && !c.front.main_active {c.session.paused=false;}
    // Cutscenes and the credits run at 3x game time (a scenario frame is a fixed 0.05 s of game time, probe_kit.rs).
    let fast=c.opening.active || c.opening.end.is_some();
    c.speed.set_relative_speed(if fast {3.0} else {p.speed});
    if p.at>=p.levels.len() {p.finished=true;return;}
    // A level change: judge the level just left.
    if c.travel.arrived!=p.arrived && p.ending==0 {
        let arrived_at=c.config.world.to_ascii_lowercase();p.arrived=c.travel.arrived;
        if p.stage==Stage::Run {
            let expected=p.next_level().unwrap_or("");
            let ok=arrived_at==expected || (expected=="rh1-wiezienie1" && arrived_at=="rh1-wiezienie1");
            let carried=match &p.carry {Some((items,_))=>{let now=c.campaign.items.names();if *items==now {"carry ok".to_owned()}else{format!("carry changed {items:?} -> {now:?}")}},None=>String::new()};
            let line=format!("{} in {:.1} s, {:.0} units walked, {} teleports {:?}, {} hitches, worst frame {:.2} s, damage taken {:.0}, npc movement {:.0}, npcs that noticed {}{}{}",if ok {"PASS"} else {"BLOCKED"},p.level_clock,p.walked,p.teleports.len(),p.teleports,p.hitches,p.worst,p.damage,p.npc_moved,p.npc_noticed,
                if p.notes.is_empty() {String::new()} else {format!(", notes {:?}",p.notes)},if carried.is_empty() {String::new()} else {format!(", {carried}")});
            p.log(if ok {line} else {format!("BLOCKED left to {arrived_at} instead of {expected}: {line}")});
            let level=p.level().to_owned();p.results.push(format!("{level}: {}",if ok {"PASS".to_owned()} else {format!("BLOCKED (went to {arrived_at})")}));
            if !ok {p.failure=Some(format!("{level} left to {arrived_at}"));}
            p.at+=1;
            if p.at>=p.levels.len() {p.finish(&mut c);return;}
        }
        p.stage=Stage::Boot;p.frames=0;
    }
    match p.stage {
        Stage::Boot=>{
            p.frames+=1;
            let ready=c.front.loading.is_none() && c.travel.pending.is_none() && c.travel.staged.is_none() && p.frames>30 && (c.campaign.mission.is_some() || c.config.world=="rh1-wiezienie1") && c.campaign.world==c.config.world;
            if !ready {return;}
            let want=p.level().to_owned();
            if !c.config.world.eq_ignore_ascii_case(&want) {c.travel.pending=Some(want);p.frames=0;return;}
            p.level_clock=0.0;p.hitches=0;p.worst=0.0;p.damage=0.0;p.npc_moved=0.0;p.npc_noticed=0;p.npc_last.clear();p.teleports.clear();p.notes.clear();p.walked=0.0;p.route.clear();p.route_goal=None;p.stuck=0;p.replans=0;p.last_pos=Some(c.position());p.watch=(c.position(),0.0);p.goal_label.clear();
            p.carry=Some((c.campaign.items.names(),String::new()));
            match p.plan(&mut c) {
                Ok(())=>{let labels:Vec<String>=p.goals.iter().map(Goal::label).collect();p.log(format!("start at {} health {:.0}: goals {labels:?}",fmt(c.position()),c.campaign.health));p.stage=Stage::Run;p.goal_clock=0.0;p.attempts=0;}
                Err(reason)=>p.block(&mut c,reason),
            }
        },
        Stage::Run=>{
            p.level_clock+=real;p.goal_clock+=dt;
            if (p.level_clock/5.0) as u32!=((p.level_clock-real)/5.0) as u32 {let (i,n,r)=(p.route_i,p.route.len(),p.route_reached);let fps=std::mem::take(&mut p.fps_frames) as f32/(5.0);p.log(format!("t={:.0} fps {fps:.0} at {} goal '{}' route {i}/{n} reached={r} stuck={} dialogue={} paused={}",p.level_clock,fmt(c.position()),p.goal_label,p.stuck,c.campaign.dialogue.as_ref().map(|d|d.id.clone()).unwrap_or_default(),c.session.paused));}
            let here=c.position();if let Some(last)=p.last_pos {let step=here.distance(last);if step<200.0 {p.walked+=step;}}p.last_pos=Some(here);
            // Credits: the level's job is done once the sequence starts.
            if let Some(end)=&c.opening.end {
                if !p.credits_seen {p.credits_seen=true;p.log("the outro cutscene ended; credits sequence started".into());}
                if end.screen==crate::endgame::Screen::Last && p.ending==0 {
                    p.log("outro, three story slides, credits and the last slide played".into());
                    // MESTER_WALK_ENDING=1: also prove the way back (the last slide ends in the main menu, a new game starts from there).
                    if std::env::var_os("MESTER_WALK_ENDING").is_some() {p.ending=1;p.ending_clock=0.0;p.ending_arrived=c.travel.arrived;}
                    else {let level=p.level().to_owned();p.log("PASS: credits sequence".into());p.results.push(format!("{level}: PASS (credits)"));p.finish(&mut c);}
                }
                return;
            }
            if p.ending>0 {
                p.ending_clock+=real;
                let fail=|p:&mut Probe,c:&mut Ctx,why:String|{p.log(format!("BLOCKED ending: {why}"));p.results.push(format!("{}: BLOCKED ending ({why})",p.level()));p.failure=Some(why);p.finish(c);};
                if p.ending==1 {
                    let menu=c.front.main_active && !c.front.in_game && c.opening.end.is_none() && c.travel.arrived!=p.ending_arrived && c.front.loading.is_none() && c.config.world=="rh1-wiezienie1";
                    if menu && p.ending_clock>3.0 {
                        p.log(format!("main menu after the last slide: world {} paused {} page {} in_game {} arrived {}",c.config.world,c.session.paused,c.front.page,c.front.in_game,c.travel.arrived));
                        // The New game button of the new-player page (frontend.rs "start").
                        c.front.character_started=true;c.campaign.reset_requested=true;c.travel.pending=Some("rh3-miasteczko0".into());p.ending=2;p.ending_clock=0.0;
                    } else if p.ending_clock>30.0 {let why=format!("no main menu 30 s after the last slide (main_active {} in_game {} end {} world {})",c.front.main_active,c.front.in_game,c.opening.end.is_some(),c.config.world);fail(&mut p,&mut c,why);}
                } else {
                    let running=c.config.world=="rh3-miasteczko0" && c.front.loading.is_none() && !c.front.main_active && c.front.in_game && c.campaign.mission.is_some() && !c.session.paused && c.campaign.world==c.config.world;
                    if running && p.ending_clock>3.0 {
                        let level=p.level().to_owned();p.log(format!("PASS: credits, main menu and a new game from there (health {:.0}, items {:?})",c.campaign.health,c.campaign.items.names()));p.results.push(format!("{level}: PASS (credits -> main menu -> new game)"));p.finish(&mut c);
                    } else if p.ending_clock>60.0 {let why=format!("new game did not start (world {} loading {} menu {} mission {})",c.config.world,c.front.loading.is_some(),c.front.main_active,c.campaign.mission.is_some());fail(&mut p,&mut c,why);}
                }
                return;
            }
            if p.level_clock>p.budget {let reason=format!("time budget of {:.0} s exhausted at goal '{}'",p.budget,p.goal_label);p.block(&mut c,reason);return;}
            if c.travel.pending.is_some() || c.travel.staged.is_some() || c.front.loading.is_some() {return;}
            // A dialogue that offers answers freezes the player: answer 1 unless the plan asks for a specific answer.
            let goal=p.goals.front().cloned();
            if c.session.dialogue_active && !matches!(goal,Some(Goal::Choose(_))) && c.campaign.mission.as_ref().is_some_and(|m|m.choices_ready()) {
                if p.goal_clock>1.0 && c.campaign.dialogue.as_ref().is_some_and(|d|!d.choices.is_empty()) && !p.taps.iter().any(|(k,_)|*k==KeyCode::Digit1) {p.tap(&mut c,KeyCode::Digit1);}
                return;
            }
            let Some(goal)=goal else {
                // Every goal is done: the level should end by itself; give the script a few seconds, then re-plan from the live state.
                p.release_walk(&mut c);
                if p.goal_clock>6.0 {
                    if p.replans>=14 {let reason="every planned goal is done but the level did not end".to_owned();p.block(&mut c,reason);return;}
                    p.replans+=1;p.goal_clock=0.0;
                    match p.plan(&mut c) {Ok(())=>{let labels:Vec<String>=p.goals.iter().map(Goal::label).collect();p.log(format!("re-plan {}: {labels:?}",p.replans));},Err(reason)=>p.block(&mut c,reason)}
                }
                return;
            };
            if p.goal_label!=goal.label() {p.goal_label=goal.label();p.goal_clock=0.0;p.attempts=0;p.gaps=0;p.vantage=None;p.route.clear();p.route_goal=None;p.log(format!("goal {} at {} (t={:.1})",p.goal_label,fmt(here),p.level_clock));
                if matches!(goal,Goal::Door(..)) {p.carry=Some((c.campaign.items.names(),String::new()));}}
            match p.goal_step(&mut c,&goal,dt) {
                Step::Working=>{},
                Step::Done=>{
                    p.goals.pop_front();p.goal_label.clear();
                    // Ambient dialogues and timing can take the script another way than the plan assumed: after an answer, plan again from the live state.
                    if matches!(goal,Goal::Choose(_)) && p.replans<12 {p.replans+=1;match p.plan(&mut c) {Ok(())=>{let labels:Vec<String>=p.goals.iter().map(Goal::label).collect();p.log(format!("re-plan after the answer: {labels:?}"));},Err(reason)=>p.block(&mut c,reason)}}
                },
                Step::Failed(reason)=>{
                    let reason=format!("goal '{}': {reason}",goal.label());
                    if p.replans<12 {p.replans+=1;p.log(format!("{reason}; planning again"));match p.plan(&mut c) {Ok(())=>{p.goal_label.clear();p.goal_clock=0.0;},Err(error)=>p.block(&mut c,format!("{reason}; then {error}"))}}
                    else {p.block(&mut c,reason);}
                },
            }
        },
        Stage::Done=>{},
    }
}
impl Probe {
    /// The level cannot be finished: log why, record it and (when more levels follow) jump to the next one.
    fn block(&mut self,c:&mut Ctx,reason:String) {
        let level=self.level().to_owned();
        self.log(format!("BLOCKED {reason} (after {:.1} s, {} teleports {:?}, {} hitches, worst frame {:.2} s, damage taken {:.0}, npc movement {:.0}, npcs that noticed {}, notes {:?})",self.level_clock,self.teleports.len(),self.teleports,self.hitches,self.worst,self.damage,self.npc_moved,self.npc_noticed,self.notes));
        self.results.push(format!("{level}: BLOCKED {reason}"));self.failure=Some(format!("{level}: {reason}"));
        self.at+=1;
        if self.at>=self.levels.len() {self.finish(c);return;}
        // Continue with the next level from its own start so one blocker does not hide the rest.
        self.stage=Stage::Boot;self.frames=0;c.travel.pending=Some(self.levels[self.at].clone());
        self.arrived=c.travel.arrived+1;
    }
    fn finish(&mut self,c:&mut Ctx) {
        self.release_walk(c);c.keys.release(KeyCode::ShiftLeft);
        info!("WALK SUMMARY ({} levels): {:#?}",self.results.len(),self.results);
        self.finished=true;self.stage=Stage::Done;
    }
}
