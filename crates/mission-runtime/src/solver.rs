//! Automatic playthrough of one level's mission script: a breadth-first search over what the player can do (talk to a character,
//! stand in a marker, kill someone, pick a dialogue answer, wait) until the script ends the level (`startlevel`) or a cutscene.
//! The same interpreter the game runs is driven, so a level whose exit flag can never be set shows up as `Stuck`.
use std::{cmp::Reverse,collections::{BTreeMap,BinaryHeap,HashSet,VecDeque}};
use crate::{Context,Event,Mission,NpcState};

#[derive(Clone,Debug,PartialEq)]
pub enum Op {
    /// Stand next to the instance (id), in its sight, and press use.
    Talk(String),
    /// Stand next to the instance and in its sight without using it (proximity dialogues).
    Approach(String),
    Kill(String),
    Choose(u8),
    /// Walk into an `o_marker_*` volume by name.
    Marker(String),
    Wait(f32),
    Draw(bool),
    /// Take a world item of this kind (`Rodzaj_item`), e.g. the quest item `Golden cat.`.
    Pickup(String),
}
impl Op {
    /// Search cost: killing is the last resort, waiting is cheap but not free.
    fn cost(&self)->u32 {match self {Op::Kill(_)=>6,Op::Wait(_)=>2,_=>1}}
}
#[derive(Clone,Debug,Default)]
pub struct MarkerModel {pub name:String,pub set:Vec<String>,pub unset:Vec<String>,/// (marker that owns the once-only key, dialogue id)
    pub dialogs:Vec<(String,String)>}
#[derive(Clone,Debug,Default)]
pub struct World {
    /// Placed characters in spawn order (`name` = definition, `id` = instance).
    pub npcs:Vec<NpcState>,
    /// Definitions with the hostile role.
    pub hostile:Vec<String>,
    /// definition -> phase -> commands, and the phase each definition starts in.
    pub phases:BTreeMap<String,BTreeMap<String,Vec<(String,String)>>>,
    pub default_phase:BTreeMap<String,String>,
    pub markers:Vec<MarkerModel>,
    /// Kinds of the pickup items lying in the level.
    pub items:Vec<String>,
}
impl World {
    /// The solver's view of a level from its exported `<world>.scene.json`, `<world>.gameplay.json` and `<world>.items.json`.
    pub fn from_exports(scene:&serde_json::Value,gameplay:&serde_json::Value,items:&serde_json::Value)->World {
        use serde_json::Value;
        let text=|v:&Value|v.as_str().unwrap_or("").to_owned();
        let mut world=World::default();
        for npc in gameplay["npcs"].as_array().into_iter().flatten() {
            world.npcs.push(NpcState {name:text(&npc["definition_name"]),id:text(&npc["name"]),position:[0,1,2].map(|i|npc["pos"][i].as_f64().unwrap_or(0.0) as f32),alive:true,player_seen:false,contact:false});
        }
        for (name,definition) in gameplay["characters"].as_object().into_iter().flatten() {
            if definition["hostile"].as_bool()==Some(true) {world.hostile.push(name.clone());}
            world.default_phase.insert(name.clone(),text(&definition["default_phase"]));
            let phases=definition["phases"].as_object().map(|p|p.iter().map(|(phase,body)|(phase.clone(),body["commands"].as_array().map(|c|c.iter().map(|pair|(text(&pair[0]),text(&pair[1]))).collect()).unwrap_or_default())).collect()).unwrap_or_default();
            world.phases.insert(name.clone(),phases);
        }
        let objects:BTreeMap<String,&Value>=scene["objects"].as_array().into_iter().flatten().filter_map(|o|Some((o["properties"]["Name"].as_str()?.to_owned(),&o["properties"]))).collect();
        let kinds:BTreeMap<String,String>=scene["objects"].as_array().into_iter().flatten().filter_map(|o|Some((o["properties"]["Name"].as_str()?.to_owned(),text(&o["kind"])))).collect();
        // A marker's effects: its own variables, and the dialogue markers its Nast_obiekt chain reaches.
        for (name,kind) in kinds.iter().filter(|(_,k)|matches!(k.as_str(),"o_marker_zmienna"|"o_marker_dialog"|"o_marker_wykrywacz_postaci")) {
            let mut marker=MarkerModel {name:name.clone(),..Default::default()};
            let p=objects[name];
            if kind=="o_marker_zmienna" {for (key,list) in [("Set",&mut marker.set),("Unset",&mut marker.unset)] {let v=text(&p[key]);if !v.is_empty() {list.push(v);}}}
            if kind=="o_marker_wykrywacz_postaci" && p["Special1"].as_i64()==Some(1) {marker.set.push("LaskaCzapelRespawnowana".into());}
            let (mut stack,mut seen)=(vec![name.clone()],std::collections::BTreeSet::new());
            while let Some(current)=stack.pop() {
                if !seen.insert(current.clone()) {continue;}
                let Some(p)=objects.get(&current) else {continue};
                if kinds[&current]=="o_marker_dialog" && !text(&p["Dialog"]).is_empty() {marker.dialogs.push((current.clone(),text(&p["Dialog"])));}
                let next=text(&p["Nast_obiekt"]);if !next.is_empty() {stack.push(next);}
            }
            if !marker.set.is_empty() || !marker.unset.is_empty() || !marker.dialogs.is_empty() {world.markers.push(marker);}
        }
        world.items=items.as_array().into_iter().flatten().map(|i|text(&i["kind"])).collect();
        world
    }
}
/// A live game state to search from (instead of the level start).
#[derive(Clone,Debug,Default)]
pub struct Live {pub alive:Vec<bool>,pub phase:Vec<String>,pub inventory:Vec<String>,pub draw:bool}
#[derive(Clone,Debug,PartialEq)]
pub enum End {Transition(String),Cutscene(String),Died}
#[derive(Debug)]
pub struct Report {pub end:Option<End>,pub plan:Vec<Op>,pub explored:usize,pub deaths:usize,pub truncated:bool,
    /// Variables set in the state that got furthest (the last one visited when nothing solved).
    pub flags:Vec<String>,pub dialogues:usize}

#[derive(Clone)]
struct Sim {mission:Mission,alive:Vec<bool>,phase:Vec<String>,inventory:Vec<String>,near:Option<usize>,draw:bool,end:Option<End>,dialogues:usize}

impl Sim {
    fn find(&self,world:&World,name:&str)->Option<usize> {
        world.npcs.iter().position(|n|n.id==name).or_else(||world.npcs.iter().position(|n|n.name==name))
    }
    fn context(&self,world:&World,action:Option<usize>)->Context {
        let mut npcs=world.npcs.clone();
        for (i,n) in npcs.iter_mut().enumerate() {n.alive=self.alive[i];n.player_seen=self.near==Some(i) && self.alive[i];n.contact=n.player_seen;}
        let player=self.near.map_or([1.0e6,0.0,0.0],|i|{let p=world.npcs[i].position;[p[0]+60.0,p[1],p[2]]});
        Context {player_position:player,weapon_drawn:self.draw,action_target:action.map(|i|world.npcs[i].name.clone()),action_id:action.map(|i|world.npcs[i].id.clone()),
            npcs,hostile_npcs:world.hostile.clone(),inventory:self.inventory.clone()}
    }
    /// An actor enters one of its own phases: script commands run, `zabij` kills, `on_death` chains on.
    fn enter_phase(&mut self,world:&World,index:usize,phase:&str,queue:&mut VecDeque<Event>) {
        let def=&world.npcs[index].name;
        let Some(commands)=world.phases.get(def).and_then(|p|p.get(phase)) else {return};
        self.phase[index]=phase.to_owned();
        queue.extend(self.mission.phase_enter(&world.npcs[index].id,commands));
        if commands.iter().any(|(k,_)|k=="zabij") && self.alive[index] {self.alive[index]=false;self.death_callback(world,index,phase,queue);}
    }
    fn death_callback(&mut self,world:&World,index:usize,phase:&str,queue:&mut VecDeque<Event>) {
        let def=&world.npcs[index].name;
        let next=world.phases.get(def).and_then(|p|p.get(&self.phase[index])).and_then(|c|c.iter().find(|(k,_)|k.strip_prefix("on_death").is_some_and(|s|s.bytes().all(|b|b.is_ascii_digit()))).map(|(_,v)|v.clone()));
        if let Some(next)=next.filter(|n|n!=phase) {self.enter_phase(world,index,&next,queue);}
    }
    fn handle(&mut self,world:&World,events:Vec<Event>) {
        let mut queue:VecDeque<Event>=events.into();let mut budget=512;
        while let Some(event)=queue.pop_front() {
            budget-=1;if budget==0 {break;}
            match event {
                Event::SetNpcPhase{name,phase}=>if let Some(i)=self.find(world,&name).filter(|i|self.alive[*i]) {self.enter_phase(world,i,&phase,&mut queue);},
                Event::SetAllNpcPhase{name,phase}=>for i in 0..world.npcs.len() {if world.npcs[i].name==name && self.alive[i] && self.near==Some(i) {self.enter_phase(world,i,&phase,&mut queue);}},
                Event::TransitionWorld(w)=>{if self.end.is_none() {self.end=Some(End::Transition(w));}},
                Event::PlayCutscene(name)=>{if self.end.is_none() {self.end=Some(End::Cutscene(name));}},
                Event::GrantItem(item)=>self.inventory.push(item),
                Event::ClearInventory=>self.inventory.clear(),
                Event::HealthDelta(delta)=>if delta<=-100.0 && self.end.is_none() {self.end=Some(End::Died);},
                Event::Dialogue(_)=>self.dialogues+=1,
                _=>{},
            }
        }
    }
    /// Ticks the script at 10 Hz until nothing is pending: the dialogue ended or waits for an answer (or `wait` seconds passed).
    fn settle(&mut self,world:&World,action:Option<usize>,wait:f32) {
        let mut action=action;let mut time=0.0;
        for _ in 0..800 {
            let context=self.context(world,action.take());
            let events=self.mission.tick(0.1,&context);if std::env::var_os("SOLVER_TRACE").is_some() {eprintln!("t={time:.1} events={events:?}");}self.handle(world,events);time+=0.1;
            if self.end.is_some() {return;}
            let busy=self.mission.dialogue().is_some() && !(self.mission.choices_ready() && self.mission.dialogue().is_some_and(|d|!d.choices.is_empty()));
            if if wait>0.0 {time>=wait}else{!busy && time>=0.3} {break;}
        }
        if self.mission.dialogue().is_none() {self.near=None;}
    }
    fn key(&self)->String {
        let snapshot=self.mission.snapshot();
        let bucket:Vec<u32>=snapshot.timers.values().map(|t|(*t/15.0).min(2.0) as u32).collect();
        format!("{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",snapshot.flags,snapshot.markers,self.mission.dialogue().map(|d|&d.id),self.alive,self.phase,self.inventory,self.draw,bucket)
    }
    fn ops(&self,world:&World)->Vec<Op> {
        let mut ops=Vec::new();
        if let Some(dialogue)=self.mission.dialogue() {
            if self.mission.choices_ready() {ops.extend(dialogue.choices.iter().map(|c|Op::Choose(c.index)));}
            ops.push(Op::Wait(20.0));return ops;
        }
        let referenced=self.mission.referenced_npcs();
        for (_,npc) in world.npcs.iter().enumerate().filter(|(i,n)|self.alive[*i] && referenced.contains(&n.name)) {
            ops.push(Op::Talk(npc.id.clone()));ops.push(Op::Approach(npc.id.clone()));
        }
        let mut seen=HashSet::new();
        for (i,npc) in world.npcs.iter().enumerate().filter(|(i,n)|self.alive[*i] && referenced.contains(&n.name)) {if seen.insert(&npc.name) {ops.push(Op::Kill(world.npcs[i].id.clone()));}}
        ops.extend(world.markers.iter().map(|m|Op::Marker(m.name.clone())));
        let wanted=self.mission.referenced_items();
        ops.extend(world.items.iter().filter(|k|wanted.contains(k) && !self.inventory.contains(k)).map(|k|Op::Pickup(k.clone())));
        ops.push(Op::Wait(20.0));ops.push(Op::Draw(!self.draw));
        ops
    }
    fn apply(&mut self,world:&World,op:&Op) {
        match op {
            Op::Talk(id)=>{if let Some(i)=self.find(world,id) {self.near=Some(i);self.settle(world,Some(i),0.0);}},
            Op::Approach(id)=>{if let Some(i)=self.find(world,id) {self.near=Some(i);self.settle(world,None,0.0);}},
            Op::Kill(id)=>{
                if let Some(i)=self.find(world,id) {
                    self.alive[i]=false;let mut queue=VecDeque::new();let phase=self.phase[i].clone();self.death_callback(world,i,&phase,&mut queue);
                    let events:Vec<Event>=queue.into();self.handle(world,events);self.settle(world,None,0.0);
                }
            },
            Op::Choose(n)=>{let events=self.mission.choose(*n);self.handle(world,events);self.settle(world,None,0.0);},
            Op::Marker(name)=>{
                if let Some(marker)=world.markers.iter().find(|m|&m.name==name) {
                    for flag in &marker.set {self.mission.set_flag(flag,true);}
                    for flag in &marker.unset {self.mission.set_flag(flag,false);}
                    for (owner,dialog) in &marker.dialogs {let events=self.mission.trigger_once(owner,dialog);self.handle(world,events);}
                    self.settle(world,None,0.0);
                }
            },
            Op::Pickup(kind)=>{self.inventory.push(kind.clone());self.settle(world,None,0.0);},
            Op::Wait(seconds)=>self.settle(world,None,*seconds),
            Op::Draw(on)=>{self.draw=*on;self.settle(world,None,0.0);},
        }
    }
}

/// Cheapest-first search for the player behaviour that ends the level; `limit` bounds the states explored.
pub fn solve(start:&Mission,world:&World,limit:usize)->Report {
    let mut first=Sim {mission:start.clone(),alive:vec![true;world.npcs.len()],phase:vec![String::new();world.npcs.len()],inventory:Vec::new(),near:None,draw:false,end:None,dialogues:0};
    for i in 0..world.npcs.len() {
        let phase=world.default_phase.get(&world.npcs[i].name).cloned().unwrap_or_default();
        let mut queue=VecDeque::new();first.enter_phase(world,i,&phase,&mut queue);let events:Vec<Event>=queue.into();first.handle(world,events);
    }
    first.settle(world,None,0.0);
    search(first,world,limit)
}
/// Like `solve`, from the state of a running game (its mission, who is alive and in which phase, what the player carries).
pub fn solve_from(mission:&Mission,world:&World,live:&Live,limit:usize)->Report {
    let n=world.npcs.len();
    let first=Sim {mission:mission.clone(),alive:(0..n).map(|i|live.alive.get(i).copied().unwrap_or(true)).collect(),phase:(0..n).map(|i|live.phase.get(i).cloned().unwrap_or_default()).collect(),inventory:live.inventory.clone(),near:None,draw:live.draw,end:None,dialogues:0};
    search(first,world,limit)
}
fn search(first:Sim,world:&World,limit:usize)->Report {
    let mut visited=HashSet::from([first.key()]);
    let mut states=vec![(first,Vec::<Op>::new())];let mut frontier=BinaryHeap::from([Reverse((0u32,0usize))]);
    let (mut explored,mut deaths,mut last)=(0usize,0usize,0usize);
    while let Some(Reverse((cost,index)))=frontier.pop() {
        let (sim,plan)=states[index].clone();
        if let Some(end)=sim.end.clone() {return Report {end:Some(end),plan,explored,deaths,truncated:false,flags:flags_of(&sim),dialogues:sim.dialogues};}
        explored+=1;if explored>limit {let sim=&states[last].0;return Report {end:None,plan,explored,deaths,truncated:true,flags:flags_of(sim),dialogues:sim.dialogues};}
        for op in sim.ops(world) {
            let mut next=sim.clone();next.apply(world,&op);
            let mut path=plan.clone();let step=op.cost();path.push(op);
            match next.end {
                Some(End::Died)=>{deaths+=1;continue;},
                Some(_)=>{states.push((next,path));frontier.push(Reverse((cost+step-1,states.len()-1)));continue;},
                None=>{}
            }
            if visited.insert(next.key()) {states.push((next,path));last=states.len()-1;frontier.push(Reverse((cost+step,states.len()-1)));}
        }
    }
    let sim=&states[last].0;
    Report {end:None,plan:Vec::new(),explored,deaths,truncated:false,flags:flags_of(sim),dialogues:sim.dialogues}
}
fn flags_of(sim:&Sim)->Vec<String> {sim.mission.snapshot().flags.into_iter().filter(|(_,on)|*on).map(|(name,_)|name).collect()}

#[cfg(test)] mod tests {
    use super::*;
    const AI:&str="level worlds\\town\nbool Told\nbool Leave\naction Talk\nifaction elder\nifnot Told\ndialog Ask\nendifs\naction Exit\nif Told\nif Leave\nstartlevel worlds\\next\nendifs";
    const DIALOGUES:&str="dialog Ask\ndelay 1\nontimeexceeded 30\ntitle Well?\nanswer1 Yes\nonchoice1dialog Yes\nanswer2 No\nonchoice2dialog No\ndialog Yes\ndelay 1\ntitle Good\nset Told\ndialog No\ndelay 1\ntitle Pity";
    fn world()->World {
        World {npcs:vec![NpcState {name:"elder".into(),id:"o_postac0".into(),position:[0.0;3],alive:true,player_seen:false,contact:false}],markers:vec![MarkerModel {name:"exit".into(),set:vec!["Leave".into()],..Default::default()}],..Default::default()}
    }
    #[test] fn the_search_finds_talk_answer_and_exit_marker() {
        let mission=Mission::from_sources("town",AI,DIALOGUES,"").unwrap();
        let report=solve(&mission,&world(),2000);
        assert_eq!(report.end,Some(End::Transition("next".into())),"{report:?}");
        assert!(report.plan.contains(&Op::Talk("o_postac0".into())) && report.plan.contains(&Op::Choose(1)) && report.plan.contains(&Op::Marker("exit".into())),"{:?}",report.plan);
    }
    #[test] fn an_exit_flag_nothing_sets_is_reported_stuck() {
        let mission=Mission::from_sources("town",AI,DIALOGUES,"").unwrap();
        let mut world=world();world.markers.clear();
        let report=solve(&mission,&world,2000);
        assert!(report.end.is_none() && !report.truncated,"{report:?}");
    }
}
