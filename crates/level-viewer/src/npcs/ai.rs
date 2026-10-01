//! Retail character AI: a port of the per-frame actor update of cshell.dll (0x1004a7e0 and everything it calls).
//! `docs/retail-ai.md` lists the evidence for every rule; addresses in comments are cshell.dll virtual addresses.
use super::*;
use std::f32::consts::{PI,TAU,FRAC_PI_4};

// ---------------------------------------------------------------------------------------------------------------------
// Angles and compass slots
// ---------------------------------------------------------------------------------------------------------------------

/// 0x1003c230: compass slot of a direction, 0 = +Z, 1 = +X+Z, 2 = +X ... 7 = -X+Z (45 degree sectors around the axes).
pub(super) fn slot_of(v:Vec3)->usize {
    let len=v.length();if len<1e-9 {return 5;} // 0/0: every comparison of the original fails and it falls through to slot 5
    let (x,z)=(v.x/len,v.z/len);let s=(PI/8.0).sin();
    if x> -s && x<s && z>0.0 {return 0;}
    if x> -s && x<s && z<0.0 {return 4;}
    if z> -s && z<s && x>0.0 {return 2;}
    if z> -s && z<s && x<0.0 {return 6;}
    if z>0.0 && x>0.0 {return 1;}
    if z>=0.0 {return 7;}
    if x>0.0 {3}else if x<0.0 {5}else{7}
}
/// 0x1003c490: yaw of a horizontal direction measured from +Z towards +X, in [0, 2 pi).
pub(super) fn yaw_of(x:f32,z:f32)->f32 {if x==0.0 && z==0.0 {return 0.0;}let a=x.atan2(z);if a<0.0 {a+TAU}else{a}}
/// 0x1003c400: the first slot k whose boundary k * 45 degrees lies above the yaw (so a heading of exactly slot s maps to s + 1), 7 at most.
pub(super) fn heading_slot(yaw:f32)->usize {
    let mut yaw=yaw;while yaw<0.0 {yaw+=TAU;}while yaw>=TAU {yaw-=TAU;}
    (0..8).find(|k|yaw<*k as f32*FRAC_PI_4).unwrap_or(7)
}
fn wrap_slot(slot:i32)->usize {slot.rem_euclid(8) as usize}
fn wrap_yaw(yaw:f32)->f32 {let mut y=yaw;while y<0.0 {y+=TAU;}while y>=TAU {y-=TAU;}y}

// ---------------------------------------------------------------------------------------------------------------------
// Phase rules
// ---------------------------------------------------------------------------------------------------------------------

/// Consecutive non-empty numbered entries `prefix0`, `prefix1` ... (the original stores them in fixed slots and stops at the first gap, 0x10045150).
pub(super) fn slot_list<'a>(commands:&'a [(String,String)],prefix:&str,max:usize)->Vec<&'a str> {
    let mut list=Vec::new();
    for n in 0..max {
        let key=format!("{prefix}{n}");
        match commands.iter().rev().find(|(k,_)|*k==key).map(|(_,v)|v.trim()).filter(|v|!v.is_empty()) {Some(v)=>list.push(v),None=>break}
    }
    list
}
/// A single named callback (`on_kontakt`, `on_reload`, `on_death`): empty means undefined.
pub(super) fn single_pub(commands:&[(String,String)],key:&str)->Option<String> {single(commands,key).map(str::to_owned)}
fn single<'a>(commands:&'a [(String,String)],key:&str)->Option<&'a str> {command_value(commands,key).map(str::trim).filter(|v|!v.is_empty())}

#[derive(Clone,Copy,PartialEq,Debug)]
pub(super) enum Estimate {Chase,Hide,Flee,Strafe,Zigzag}
/// The `estimate_*` flag of a phase in the order 0x10047c60 dispatches them.
pub(super) fn estimate_of(commands:&[(String,String)])->Option<Estimate> {
    if has(commands,"estimate_do_gracza") {Some(Estimate::Chase)}
    else if has(commands,"estimate_kryjowka") {Some(Estimate::Hide)}
    else if has(commands,"estimate_od_gracza") {Some(Estimate::Flee)}
    else if has(commands,"estimate_do_strzalu") {Some(Estimate::Strafe)}
    else if has(commands,"estimate_kluczy") {Some(Estimate::Zigzag)}
    else {None}
}

// ---------------------------------------------------------------------------------------------------------------------
// The path graph
// ---------------------------------------------------------------------------------------------------------------------

impl Navigation {
    /// 0x1003c130: the node nearest to a position by its top position (all graphs); nothing for a position within 32 units of the origin.
    pub(super) fn place_of(&self,pos:Vec3)->Option<usize> {
        if pos.length()<32.0 {return None;}
        self.nodes.iter().enumerate().map(|(i,n)|(i,Vec3::from(n.pos).distance(pos))).filter(|(_,d)|*d<1.0e8).min_by(|a,b|a.1.total_cmp(&b.1)).map(|(i,_)|i)
    }
    /// 0x1003bc70: the node of `graph` whose floor position is nearest to a point.
    pub(super) fn nearest_real(&self,graph:usize,point:Vec3)->Option<usize> {
        self.nodes.iter().enumerate().filter(|(_,n)|n.graph==graph).map(|(i,n)|(i,Vec3::from(n.real).distance(point))).min_by(|a,b|a.1.total_cmp(&b.1)).map(|(i,_)|i)
    }
    /// 0x1003bd20 / 0x1003bdb0: depth first search from `start` towards the node nearest to `target`, 48 nodes deep at most. It only steps to enabled,
    /// unvisited neighbours that are not farther (floor distance) from the goal than the current node, the nearest to the goal first.
    pub(super) fn path(&self,start:usize,target:Vec3)->Vec<usize> {
        let Some(goal)=self.nearest_real(self.nodes[start].graph,target) else {return Vec::new()};
        let mut visited=vec![false;self.nodes.len()];let mut path=Vec::new();
        if self.search(start,goal,0,&mut visited,&mut path) {path.reverse();path}else{Vec::new()}
    }
    fn search(&self,cur:usize,goal:usize,depth:usize,visited:&mut [bool],path:&mut Vec<usize>)->bool {
        visited[cur]=true;
        if depth>=48 {return false;}
        if cur==goal {path.push(cur);return true;}
        let goal_pos=self.real(goal);let here=self.real(cur).distance(goal_pos);
        loop {
            let mut best:Option<(usize,f32)>=None;
            for n in self.nodes[cur].slots.iter().flatten() {
                if visited[*n] || !self.nodes[*n].enabled {continue;}
                let d=self.real(*n).distance(goal_pos);
                if d<=here && best.is_none_or(|(_,b)|d<b) {best=Some((*n,d));}
            }
            let Some((next,_))=best else {return false};
            if self.search(next,goal,depth+1,visited,path) {path.push(cur);return true;}
        }
    }
    /// The compass slot of `cur` that leads to `next`.
    pub(super) fn slot_to(&self,cur:usize,next:usize)->Option<usize> {self.nodes[cur].slots.iter().position(|s|*s==Some(next))}
}

// ---------------------------------------------------------------------------------------------------------------------
// Phase entry
// ---------------------------------------------------------------------------------------------------------------------

impl Npc {
    /// The bookkeeping half of `SetPhase` (0x10041c60): reset of the perception flags, the jump to the target node of a walk interrupted by a forced switch,
    /// the animation restart (a looping phase that keeps its clip does not restart it, 0x10041e16), hull, weapon visibility and the `obrot` turn.
    /// The parts that need the world (seen test, `strzal_raz`, path start, melee) wait in `start_pending`.
    pub(super) fn begin_phase(&mut self,name:&str,snap:bool,nav:&Navigation)->Option<PhaseCommands> {
        if self.dead {return None;}
        let source=self.definition.phases.get(name)?;
        let commands=source.commands.clone();
        // 0x10041cbe: a tagged actor (+0x128) does not start a `patrol` phase (the call returns before anything is reset).
        if self.aware && has(&commands,"patrol") {return None;}
        self.contact=false;self.provoked=false;self.chase=false;self.face_player=false;self.seen_timer=0.0;
        if snap && self.moving {
            if let Some(node)=self.next {
                let old=self.position+self.vel;
                self.place=Some(node);self.progress=0.0;self.next=None;self.moving=false;
                self.position=nav.real(node)+Vec3::Y*self.half_extents.y;
                self.vel=old-self.position;
            }
        }
        let animation=source.animation.clone().unwrap_or_default();
        if !(source.looping && self.anim==animation) {self.elapsed=0.0;self.anim=animation;}
        self.phase=name.into();
        if let Some(dimensions)=source.dimensions {self.half_extents=Vec3::from(dimensions).max(Vec3::splat(0.5));}else if let Some(h)=self.definition.collision_half_extents {self.half_extents=Vec3::from(h).max(Vec3::splat(0.5));}
        self.rendered_animation=choose_animation(&self.definition,source.animation.as_deref());
        // SetPhase plays the phase's head clip unconditionally (0x10041ea4); a phase without one leaves the head as it is.
        if let Some(clip)=single(&commands,"animacja_glowa").filter(|c|!c.is_empty()) {self.head_request=Some(clip.to_owned());}
        if has(&commands,"show_weapon") {self.weapon_visible=true;}
        if has(&commands,"drop_weapon") {self.weapon_visible=false;}
        if let Some(degrees)=number(&commands,"obrot").filter(|d|*d!=0.0) {self.yaw_target=self.yaw+degrees.to_radians();}
        self.fire_timer=0.0;self.start_pending=true;
        Some(commands)
    }
}

/// Everything one actor update needs from the outside world.
pub(super) struct Frame<'a> {
    pub world:&'a CollisionWorld,pub player:Vec3,pub half:Vec3,pub eye:Vec3,pub difficulty:f32,pub dt:f32,pub blocked:&'a dyn Fn(Vec3,Vec3)->bool,
    /// The model boxes of the other characters at the start of the frame (the engine tests a model by its dims box, Lithtech.exe 0x42b390).
    pub boxes:&'a [ActorBox],
}
/// One character's hit box: `index` into the roster, alive or a corpse.
#[derive(Clone,Copy,Debug)]
pub(super) struct ActorBox {pub index:usize,pub alive:bool,pub center:Vec3,pub half:Vec3}
impl Frame<'_> {
    /// A clear segment: no static geometry and no closed door between the points.
    fn free(&self,a:Vec3,b:Vec3)->bool {line_of_sight(self.world,a,b) && !(self.blocked)(a,b)}
    /// `free` plus the characters as occluders. The two sight filters differ in who blocks: the player's line to an actor (`seen`, the weapon test: cshell
    /// 0x10057940) stops at every other model, living or dead, except the actor itself; the hide test (0x10057820, `living` false) ignores living
    /// characters and glass and meets corpses (user flag 0x80). A segment that starts inside a box does not hit it (Lithtech.exe 0x42b8f2).
    fn sight(&self,owner:Option<usize>,living:bool,a:Vec3,b:Vec3)->bool {
        self.free(a,b) && !self.boxes.iter().any(|o|Some(o.index)!=owner && (living || !o.alive) && segment_box_entry(a,b,o.center,o.half).is_some())
    }
}

impl NpcRoster {
    /// `SetPhase(actor, name, snap)` (cshell 0x10041c60): the bookkeeping now, the rest at the start of the actor's next update step.
    pub(super) fn enter_phase(&mut self,index:usize,name:&str,snap:bool)->Option<PhaseCommands> {
        let commands=self.actors[index].begin_phase(name,snap,&self.navigation)?;
        let (actor_name,position)=(self.actors[index].name.clone(),self.actors[index].position);
        self.pending_sounds.extend(phase_sounds(&commands,position));
        self.pending_commands.push((actor_name,commands.clone()));
        if has(&commands,"zabij") && self.actors[index].alive() {self.kill(index);}
        Some(commands)
    }
    /// `Kill` (cshell 0x10042ca0): a noise, the current phase's `on_death` phase (no snap), then the body is dead: no longer solid, screams, leaves a blood pool.
    pub(super) fn kill(&mut self,index:usize) {
        if self.actors[index].dead {return;}
        let at=self.actors[index].body_center()+Vec3::Y*66.0;
        self.add_stimulus(at,256.0,1280.0,2048.0,2);
        let phase=self.actors[index].phase.clone();
        let next=single(self.actors[index].commands(),"on_death").map(str::to_owned);
        if let Some(next)=next.filter(|n|*n!=phase) {let _=self.enter_phase(index,&next,false);}
        let actor=&mut self.actors[index];
        // Kill zeroes the gun pitch and its target (0x10042d31) and removes the node control of `obrot_pion_do_gracza0` (0x10042d12).
        actor.pitch=0.0;actor.pitch_target=0.0;
        actor.dead=true;actor.dead_for=0.0;self.stats.kills+=1;actor.hp=actor.hp.min(-1.0e-4);actor.moving=false;actor.next=None;actor.start_pending=false;
        let (definition,position)=(actor.definition.clone(),actor.position);
        self.pending_deaths.push((position,definition.flags.iter().any(|flag|flag=="plama_krwi")));
        // Kill (0x10042eec..0x10043085) removes the carried weapon objects of a `nie_zostawiaj_gana` character and throws the others into the world as pickups
        // (velocity (0,-64,0), 300 s): every armed character, in whatever phase it dies. `socket_weapon1` names a second weapon object of the same item (0x10044e7a).
        let weapon=command_value(&definition.header,"weapon").filter(|w|!w.is_empty()).map(str::to_owned);
        if weapon.is_some() {self.actors[index].weapon_visible=false;}
        if let Some(kind)=self.actors[index].dropped_weapon().map(str::to_owned).filter(|k|!k.is_empty()) {
            let name=self.actors[index].name.clone();
            let two=command_value(&definition.header,"socket_weapon1").is_some_and(|s|!s.is_empty());
            self.pending_drops.push(WeaponDrop {owner:name.clone(),kind:kind.clone(),position});
            if two {self.pending_drops.push(WeaponDrop {owner:format!("{name}+1"),kind,position});}
        }
        if let Some(scream)=death_scream(&definition,position,&mut self.sound_rng) {self.pending_sounds.push(scream);}
    }
    /// The `rand() % n` pick of a numbered callback list (0x10045610).
    pub(super) fn pick(&mut self,list:&[&str])->Option<String> {
        match list.len() {0=>None,1=>Some(list[0].to_owned()),n=>Some(list[self.ai_rng.next() as usize%n].to_owned())}
    }

    // -----------------------------------------------------------------------------------------------------------------
    // The frame
    // -----------------------------------------------------------------------------------------------------------------

    pub(super) fn simulate_with_barriers(&mut self,world:&CollisionWorld,player_target:PlayerTarget,difficulty:f32,dt:f32,
        blocked:impl Fn(Vec3,Vec3)->bool,_sweep:impl Fn(Vec3,Vec3,Vec3)->Option<(f32,Vec3)>) {
        // The player's eye is 48 above the hull centre, 16 when crouched (0x100435b0).
        let eye=player_target.position+Vec3::Y*if player_target.half_extents.y<40.0 {16.0}else{48.0};
        let boxes:Vec<ActorBox>=self.actors.iter().enumerate().filter(|(_,a)|a.shown()).map(|(index,a)|ActorBox {index,alive:a.alive(),center:a.body_center(),half:a.half_extents}).collect();
        let frame=Frame {world,player:player_target.position,half:player_target.half_extents,eye,difficulty,dt,blocked:&blocked,boxes:&boxes};
        self.door_requests.clear();
        for index in 0..self.actors.len() {self.update_actor(index,&frame);}
        for stimulus in &mut self.noises {stimulus.life=stimulus.life.saturating_sub(1);}
        self.noises.retain(|s|s.life>0);
    }

    /// `UpdatePostac` (0x1004a7e0) for one actor, in the original order.
    fn update_actor(&mut self,index:usize,frame:&Frame) {
        if !self.actors[index].shown() {return;}
        let dt=frame.dt;
        self.actors[index].elapsed+=dt;
        let mobile=self.actors[index].definition.flags.iter().any(|f|f=="ruchomy");
        if self.actors[index].dead {self.update_corpse(index,frame);return;}
        if !mobile && !self.cutscene {return;}
        if self.actors[index].place.is_none() && mobile {self.actors[index].place=self.navigation.place_of(self.actors[index].position);}
        self.run_pending_start(index,frame);
        // seen (0x1004a8ff)
        if self.cutscene {self.actors[index].seen=true;}else{self.refresh_seen(index,frame,false);}
        // A firing phase that cannot hit the player relocates (0x1004a91a).
        let strzal=number(self.actors[index].commands(),"strzal").unwrap_or(0.0);
        if strzal!=0.0 && !self.cutscene && !self.weapon_sees_player(index,frame) && self.ai_rng.next()&3==0 {
            let target=if self.actors[index].definition.phases.contains_key("do_strzalu_patrzy") {"do_strzalu_patrzy"}
                else if self.actors[index].definition.phases.contains_key("do_strzalu") {"do_strzalu"}else{"estimate"};
            let _=self.enter_phase(index,target,true);
            return;
        }
        self.transitions(index,frame);
        if self.actors[index].dead || self.actors[index].start_pending {self.run_pending_start(index,frame);}
        if !self.cutscene && mobile {self.contact(index,frame);self.move_step(index,frame);}
        self.run_pending_start(index,frame);
        self.turn(index,frame);
        let actor=&mut self.actors[index];
        // The model glides from the old position after a snap (0x1004aa05..).
        if actor.vel.length()>0.01 {actor.vel*=((1.0-dt)*5.0).clamp(0.0,0.9);}
        // A wounded actor heals one point per second (0x1004aafb).
        let retreat=number(&actor.definition.header,"ucieka_jak_mniej_niz").unwrap_or(0.0);
        if !actor.dead && actor.hp<retreat {actor.hp+=dt;}
        self.step_sound(index,frame);
        self.fire_timer_step(index,frame);
    }

    /// Dead: only the clean-up clock runs (0x10049d80): after 30 s, 320 units or more away and out of the player's sight, the body goes unless `nie_respawnuj`.
    fn update_corpse(&mut self,index:usize,frame:&Frame) {
        let dt=frame.dt;let actor=&mut self.actors[index];
        actor.dead_for+=dt;
        if actor.dead_for>30.0 && !actor.definition.flags.iter().any(|f|f=="nie_respawnuj") && (actor.position+Vec3::Y*64.0).distance(frame.player)>=320.0
            && !frame.free(frame.eye,actor.position+Vec3::Y*64.0) {actor.visible=false;}
    }

    /// The parts of a phase entry that need the world, run at the start of the actor's update (0x10041d50..0x100422a7).
    fn run_pending_start(&mut self,index:usize,frame:&Frame) {
        for _ in 0..8 {
            if !self.actors[index].start_pending || self.actors[index].dead {return;}
            self.actors[index].start_pending=false;
            self.refresh_seen(index,frame,true);
            let commands=self.actors[index].commands().to_vec();
            let phase=self.actors[index].phase.clone();
            if has(&commands,"strzal_raz") {self.fire(index,false,frame);}
            if self.actors[index].phase!=phase {continue;}
            if has(&commands,"strzal_raz1") {self.fire(index,true,frame);}
            if self.actors[index].phase!=phase {continue;}
            if has(&commands,"patrol") {
                self.patrol_start(index,frame);
                if !self.actors[index].moving {
                    let default=self.actors[index].definition.default_phase.clone();
                    if phase!=default {let _=self.enter_phase(index,&default,true);continue;}
                }
            }
            if let Some(kind)=estimate_of(&commands) {
                self.estimate(index,kind,frame);
                // After the estimate the actor asks for a door next to itself (0x100420b3, its position + 32).
                if self.actors[index].place.is_some() {let at=self.actors[index].position+Vec3::Y*32.0;self.door_requests.push(at);self.stats.door_asks+=1;}
                if self.actors[index].phase!=phase || self.actors[index].start_pending {continue;}
            }
            let distance=self.actors[index].position.distance(frame.player);
            if has(&commands,"odepchnij_gracza") {self.hurt_player(index,40.0,false,frame);}
            if has(&commands,"gryzie") && distance<number(&commands,"do_gracza").unwrap_or(0.0) {self.hurt_player(index,10.0,true,frame);}
        }
    }
    /// A blow that needs no aim: the wound and the presentation of the hit (0x10042460 knocks for 40, 0x100422e2 bites for 10).
    fn hurt_player(&mut self,index:usize,damage:f32,bite:bool,_frame:&Frame) {
        self.pending_damage+=damage;self.stats.melee+=1;self.stats.damage_to_player+=damage;
        let origin=self.actors[index].eye();
        self.pending_shots.push(EnemyShot {origin,endpoint:origin,commands:Vec::new(),casing:None,hits_player:true,melee:true,bite,extra:false,victim:false});
    }

    // -----------------------------------------------------------------------------------------------------------------
    // Perception
    // -----------------------------------------------------------------------------------------------------------------

    /// `0x10043720`: is the actor's chest (position + 0.75 * half height + mod_y) visible from the player's eye? The result is cached for `distance / 2560` seconds.
    fn refresh_seen(&mut self,index:usize,frame:&Frame,force:bool) {
        let actor=&mut self.actors[index];
        if force {actor.seen_timer=0.0;}
        if self.invisible || self.player_dead || actor.definition.flags.iter().any(|f|f=="nie_patrz_na_gracza") {actor.seen=false;return;}
        actor.seen_timer-=frame.dt;
        if actor.seen_timer>0.0 {return;}
        let distance=actor.position.distance(frame.player);
        actor.seen_timer=distance*0.25*0.0015625;
        let chest=actor.position+Vec3::Y*(actor.half_extents.y*0.75+number(actor.commands(),"mod_y").unwrap_or(0.0));
        actor.seen=frame.sight(Some(index),true,frame.eye,chest);
    }
    /// `0x10043860`: can the actor's weapon see the player? (its socket, else 32 above the centre, must be visible from the player's eye)
    fn weapon_sees_player(&self,index:usize,frame:&Frame)->bool {
        if self.invisible {return false;}
        let actor=&self.actors[index];
        let point=weapon_sockets(&self.models,actor).map_or(actor.position+Vec3::Y*32.0,|(muzzle,_,_)|muzzle);
        frame.sight(Some(index),true,frame.eye,point)
    }
    /// A stimulus is noticed inside its `always` radius, or inside `los` / `cone` (the original's 60 degree cone test compares a radian value with 60 and never fails)
    /// when the segment from the stimulus to the actor is free (0x10045df0).
    pub(super) fn notices(&self,index:usize,frame:&Frame)->bool {
        let actor=&self.actors[index];
        if self.invisible || self.player_dead {return false;}
        let visual=self.visual_stimuli.iter().map(|p|Stimulus::light(*p));
        self.noises.iter().cloned().chain(visual).any(|s|{
            let distance=s.pos.distance(actor.position);
            distance<s.always
                || (s.los>1.0 && distance<s.los && frame.free(s.pos,actor.position))
                || (s.cone>1.0 && distance<s.cone && frame.free(s.pos,actor.position))
        })
    }
    /// `ContactCheck` (0x10049950): notice the player, then, once a script provoked the actor, take `on_kontakt`.
    fn contact(&mut self,index:usize,frame:&Frame) {
        let actor=&self.actors[index];
        let Some(target)=single(actor.commands(),"on_kontakt").map(str::to_owned) else {return};
        if self.player_dead || self.invisible {return;}
        let header=&actor.definition.header;
        let reach=number(header,"odleglosc_kontaktu").unwrap_or(640.0);
        let cone=number(header,"kat_kontaktu").unwrap_or(0.0).to_radians();
        let forward=Vec3::new(actor.yaw.sin(),0.0,actor.yaw.cos());
        let to_player=frame.player-actor.position;
        let distance=to_player.length();
        // The player counts as "behind" unless d.f > 0 (0x10043930); only the front half plane uses the cone.
        let front=to_player.dot(forward)>0.0;
        let noticed=if front {
            let flat=Vec3::new(to_player.x,0.0,to_player.z);
            let sine=(flat.x*forward.z-flat.z*forward.x)/(flat.length()*forward.length()).max(1.0e-5);
            sine.clamp(-1.0,1.0).asin().abs()<cone && distance<reach && actor.seen
        }else{distance<48.0};
        let noticed=noticed || self.notices(index,frame);
        let actor=&mut self.actors[index];
        if noticed {actor.contact=true;}else{actor.contact=false;actor.provoked=false;}
        if !actor.provoked {return;}
        let position=actor.position;let seen=actor.seen;
        let sound=single(actor.commands(),"sound_on_kontakt").map(str::to_owned);
        let variants=number(actor.commands(),"sounds_on_kontakt");
        if let Some(path)=sound {self.pending_sounds.push(halt_sound(&path,variants,&mut self.sound_rng,position,seen));}
        self.add_stimulus(position,128.0,640.0,1280.0,2);
        let _=self.enter_phase(index,&target,true);
    }

    // -----------------------------------------------------------------------------------------------------------------
    // Phase transitions
    // -----------------------------------------------------------------------------------------------------------------

    /// `0x10049b60`: once the animation of a non-looping phase has ended, choose the next phase.
    fn transitions(&mut self,index:usize,frame:&Frame) {
        let actor=&self.actors[index];
        let Some(phase)=actor.definition.phases.get(&actor.phase) else {return};
        if phase.looping || actor.dead || actor.start_pending {return;}
        if actor.elapsed<phase.duration.unwrap_or(0.0).max(0.04) {return;}
        let commands=phase.commands.clone();
        let header=actor.definition.header.clone();
        let distance=actor.position.distance(frame.player);
        let range=number(&commands,"do_gracza").unwrap_or(0.0);
        let hurt=slot_list(&commands,"on_hurt",4);
        let retreat=number(&header,"ucieka_jak_mniej_niz").unwrap_or(0.0);
        let anim=slot_list(&commands,"on_koniec_anim",8);
        let closer_seen=slot_list(&commands,"on_closer_widzi",4);
        let closer=slot_list(&commands,"on_closer",4);
        let further_seen=slot_list(&commands,"on_further_widzi",8);
        let further=slot_list(&commands,"on_further",8);
        let hp=actor.hp;
        // (next phase, snap flag): the callbacks snap, the fallback of 0x10041bf0 (the phase `estimate`, else the default phase) does not.
        let (next,snap)=if !hurt.is_empty() && hp<retreat {(self.pick(&hurt),true)}
            else if !anim.is_empty() {(self.pick(&anim),true)}
            else if !closer_seen.is_empty() && range>1.0 && distance<=range && {self.refresh_seen(index,frame,false);self.actors[index].seen} {(self.pick(&closer_seen),true)}
            else if !closer.is_empty() && range>1.0 && distance<=range {(self.pick(&closer),true)}
            else if !further_seen.is_empty() && distance>range && distance<4096.0 && self.actors[index].seen {(self.pick(&further_seen),true)}
            else if !further.is_empty() && distance>range && distance<4096.0 && !self.actors[index].seen {(self.pick(&further),true)}
            else {(Some(if self.actors[index].definition.phases.contains_key("estimate") {"estimate".to_owned()}else{self.actors[index].definition.default_phase.clone()}),false)};
        if let Some(next)=next {let _=self.enter_phase(index,&next,snap);}
    }
    /// End of a walk: `on_koniec_widzi` when the player sees the actor and the phase has it, else `on_koniec` (0x10048510).
    fn walk_ended(&mut self,index:usize) {
        let commands=self.actors[index].commands().to_vec();
        let seen_list=slot_list(&commands,"on_koniec_widzi",8);
        let list=if self.actors[index].seen && !seen_list.is_empty() {seen_list}else{slot_list(&commands,"on_koniec",8)};
        if let Some(next)=self.pick(&list) {let _=self.enter_phase(index,&next,true);}
    }

    // -----------------------------------------------------------------------------------------------------------------
    // Walking
    // -----------------------------------------------------------------------------------------------------------------

    /// Is another living actor standing on, or walking to, a node? (0x10046ec0; a missing node counts as taken)
    fn occupied(&self,node:Option<usize>,me:usize)->bool {
        let Some(node)=node else {return true};
        self.actors.iter().enumerate().any(|(i,a)|i!=me && !a.dead && a.shown() && if a.moving {a.next==Some(node)}else{a.place==Some(node)})
    }
    /// A neighbour the patrol may take: free, enabled, within 8 units of floor height (0x10048060..0x10048091).
    fn patrol_candidate(&self,index:usize,place:usize,slot:usize,doors:bool)->Option<usize> {
        let n=self.navigation.nodes[place].slots[slot]?;
        if self.occupied(Some(n),index) || !self.navigation.nodes[n].enabled {return None;}
        if (self.navigation.nodes[n].real[1]-self.navigation.nodes[place].real[1]).abs()>8.0 {return None;}
        if doors && self.navigation.nodes[place].door[slot] {return None;}
        Some(n)
    }
    /// The start of a `patrol` phase (0x10047fc0): up to 17 rounds of three random tries near the heading, then every slot from the heading, then set off.
    fn patrol_start(&mut self,index:usize,_frame:&Frame) {
        let actor=&mut self.actors[index];
        actor.next=None;actor.path_len=0;actor.progress=0.0;actor.seg_len=0.0;actor.moving=false;
        let Some(place)=actor.place else {return};
        let heading=heading_slot(actor.yaw_target) as i32;
        let mut chosen=None;
        'rounds: for _ in 0..17 {
            let first=(self.ai_rng.next()&1) as i32-(self.ai_rng.next()&1) as i32;
            let mut slot=wrap_slot(heading+first);
            if let Some(n)=self.patrol_candidate(index,place,slot,true) {chosen=Some(n);break 'rounds;}
            slot=wrap_slot(slot as i32+self.ai_rng.next()%3-self.ai_rng.next()%3);
            if let Some(n)=self.patrol_candidate(index,place,slot,true) {chosen=Some(n);break 'rounds;}
            slot=(self.ai_rng.next()&7) as usize;
            if let Some(n)=self.patrol_candidate(index,place,slot,true) {chosen=Some(n);break 'rounds;}
        }
        if chosen.is_none() {
            for k in 0..8 {if let Some(n)=self.patrol_candidate(index,place,wrap_slot(heading+k),false) {chosen=Some(n);break;}}
        }
        let Some(node)=chosen else {return};
        self.begin_segment(index,place,node);
    }
    /// Start walking from `place` to `node`: snap to the place, direction, length, heading (0x1004828c).
    fn begin_segment(&mut self,index:usize,place:usize,node:usize) {
        let from=self.navigation.real(place);let to=self.navigation.real(node);
        let phase_rot=number(self.actors[index].commands(),"mod_rot").unwrap_or(0.0);
        let actor=&mut self.actors[index];
        actor.next=Some(node);actor.position=from+Vec3::Y*actor.half_extents.y;
        let delta=to-from;actor.seg_len=delta.length();actor.seg_dir=if actor.seg_len>0.0 {delta/actor.seg_len}else{Vec3::ZERO};
        actor.progress=0.0;actor.moving=true;
        actor.yaw_target=slot_of(delta) as f32*FRAC_PI_4+phase_rot*PI;
    }
    /// `UpdateMove` (0x100483c0): advance along the segment, hop from node to node.
    fn move_step(&mut self,index:usize,frame:&Frame) {
        let actor=&mut self.actors[index];
        if actor.dead || !actor.moving {return;}
        let Some(place)=actor.place else {actor.moving=false;return};
        let speed=number(actor.commands(),"speed").unwrap_or(0.0);
        actor.progress+=speed*frame.dt;
        if actor.progress<actor.seg_len {
            actor.position=self.navigation.real(place)+actor.seg_dir*actor.progress+Vec3::Y*actor.half_extents.y;
            return;
        }
        if actor.path_len==0 {
            // A patrol step ends at its node.
            actor.progress=0.0;
            if let Some(node)=actor.next {actor.place=Some(node);}
            actor.next=None;actor.moving=false;
            if let Some(place)=actor.place {actor.position=self.navigation.real(place)+Vec3::Y*actor.half_extents.y;}
            self.walk_ended(index);
            // A patrol that stopped on a dead end turns around (0x1004854a).
            let actor=&self.actors[index];
            if actor.phase.is_empty() || !has(actor.commands(),"patrol") || actor.moving {return;}
            let slot=heading_slot(actor.yaw);
            if actor.place.is_some_and(|p|self.navigation.nodes[p].slots[slot].is_none()) {self.actors[index].yaw_target=wrap_yaw(self.actors[index].yaw_target+PI);}
            return;
        }
        self.follow_path(index,frame);
    }
    /// The estimate walk (0x100485d1): arrive at the target, refresh `seen`, and take the next node when it is free.
    fn follow_path(&mut self,index:usize,frame:&Frame) {
        {
            let actor=&mut self.actors[index];
            actor.seen_timer=0.0;actor.progress=0.0;
        }
        self.refresh_seen(index,frame,true);
        // Arriving at a node next to a door link (previous link .. the one after next) asks for the door at the node's floor + 32 (0x100485d1).
        if let Some(reached)=self.actors[index].next {
            let old=self.actors[index].path_index;
            if (old.saturating_sub(1)..=old+2).any(|k|self.path_link_has_door(index,k)) {let at=self.navigation.real(reached)+Vec3::Y*32.0;self.door_requests.push(at);self.stats.door_asks+=1;}
        }
        let actor=&mut self.actors[index];
        actor.place=actor.next.or(actor.place);
        actor.path_index+=1;
        let (idx,count)=(actor.path_index,actor.path_len);
        let distance=actor.position.distance(frame.player);
        let range=number(actor.commands(),"do_gracza").unwrap_or(0.0);
        if idx>=count || (actor.chase && distance<range) {self.stop_walk(index);return;}
        let (here,ahead)=(self.actors[index].path_buf[idx],self.actors[index].path_buf.get(idx+1).copied().flatten());
        // A node someone else stands on, a disabled node, or a busy node beyond it (an unset list entry counts as busy) stops the walk here.
        if self.occupied(here,index) || here.is_some_and(|n|!self.navigation.nodes[n].enabled) || self.occupied(ahead,index) {self.stop_walk(index);return;}
        let node=here.unwrap();
        let place=self.actors[index].place.unwrap_or(node);
        // With `moze_strzelac_w_wezle` the actor fires at every other node when its weapon sees the player (0x1004880d).
        if has(self.actors[index].commands(),"moze_strzelac_w_wezle") && self.weapon_sees_player(index,frame) && !self.actors[index].fired_at_node {
            self.fire(index,false,frame);self.actors[index].fired_at_node=true;
        }else{self.actors[index].fired_at_node=false;}
        if self.actors[index].dead {return;}
        self.begin_segment(index,place,node);
    }
    /// Does the link that leads to path entry `k` (from entry `k - 1`) carry a door (the per-step flags +0x267c)?
    fn path_link_has_door(&self,index:usize,k:usize)->bool {
        let buf=&self.actors[index].path_buf;
        let (Some(a),Some(b))=(k.checked_sub(1).and_then(|p|buf.get(p).copied().flatten()),buf.get(k).copied().flatten()) else {return false};
        self.navigation.slot_to(a,b).is_some_and(|slot|self.navigation.nodes[a].door[slot])
    }
    /// The walk ends where the actor stands (0x1004890d / 0x10048970).
    fn stop_walk(&mut self,index:usize) {
        let actor=&mut self.actors[index];
        actor.next=None;actor.moving=false;
        if let Some(place)=actor.place {actor.position=self.navigation.real(place)+Vec3::Y*actor.half_extents.y;}
        self.walk_ended(index);
    }

    // -----------------------------------------------------------------------------------------------------------------
    // Estimates: where to walk next (0x10047c60)
    // -----------------------------------------------------------------------------------------------------------------

    /// Is the point hidden from the player's eye + 55 (0x10047070)?
    pub(super) fn hidden_from_player(&self,point:Vec3,frame:&Frame)->bool {!frame.sight(None,false,frame.player+Vec3::Y*55.0,point)}

    /// Set a walk: search the path to `target` from the actor's place and store it. Returns whether a path exists.
    fn set_target(&mut self,index:usize,target:Vec3)->bool {
        let Some(place)=self.actors[index].place else {self.actors[index].path_len=0;return false};
        let path=self.navigation.path(place,target);
        self.store_path(index,&path);
        !path.is_empty()
    }
    fn store_path(&mut self,index:usize,path:&[usize]) {
        let actor=&mut self.actors[index];
        for (i,n) in path.iter().take(64).enumerate() {actor.path_buf[i]=Some(*n);}
        actor.path_len=path.len().min(64);
    }
    /// The dispatch of `EstimatePath` (0x10047c60) and its consequences.
    fn estimate(&mut self,index:usize,kind:Estimate,frame:&Frame) {
        {
            let actor=&mut self.actors[index];
            actor.next=None;actor.progress=0.0;actor.seg_len=0.0;actor.moving=false;actor.path_len=0;actor.path_index=0;
        }
        let commands=self.actors[index].commands().to_vec();
        if self.actors[index].position.distance(frame.player)>8000.0 && matches!(kind,Estimate::Chase|Estimate::Strafe) {
            // Far away the chase and the sidestep end at once (0x10047cb4).
            let list=slot_list(&commands,"on_koniec",8);
            if let Some(next)=self.pick(&list) {let _=self.enter_phase(index,&next,true);}
            return;
        }
        match kind {
            Estimate::Chase=>self.estimate_chase(index,frame),
            Estimate::Hide=>self.estimate_hide(index,frame),
            Estimate::Flee=>self.estimate_flee(index,frame),
            Estimate::Strafe=>self.estimate_strafe(index,frame),
            Estimate::Zigzag=>self.estimate_zigzag(index,frame),
        }
        let actor=&self.actors[index];
        let zigzag=kind==Estimate::Zigzag;
        let usable=actor.place.is_some() && actor.path_buf[1].is_some() && if zigzag {actor.path_len>0}else{actor.path_len>=2};
        if !usable {
            // 0x10041bf0: the phase `estimate`, else the default phase.
            let has_estimate=self.actors[index].definition.phases.contains_key("estimate");
            let target=if has_estimate {"estimate".to_owned()}else{self.actors[index].definition.default_phase.clone()};
            let _=self.enter_phase(index,&target,false);
            if zigzag {
                if !has_estimate && self.actors[index].definition.phases.contains_key("strzela") {let _=self.enter_phase(index,"strzela",true);}
                return;
            }
            if !has_estimate {self.walk_ended(index);}
            // A definition with a phase called `kluczy` zig-zags after a failed search unless the phase it landed in does already (0x10047dc9).
            if !has(self.actors[index].commands(),"estimate_kluczy") && self.actors[index].definition.phases.contains_key("kluczy") {let _=self.enter_phase(index,"kluczy",true);}
            return;
        }
        // Success: walk to the second node of the path.
        let Some(place)=self.actors[index].place else {return};
        let Some(node)=self.actors[index].path_buf[1] else {return};
        if kind==Estimate::Chase {self.actors[index].chase=true;}
        self.begin_segment(index,place,node);
    }
    /// `estimate_do_gracza` (0x10046f30): a point `distance - 24` towards the player (16 away from it when closer than 32); nothing beyond 2048.
    fn estimate_chase(&mut self,index:usize,frame:&Frame) {
        let actor=&self.actors[index];
        let to=frame.player-actor.position;let distance=to.length();
        if distance>2048.0 {self.actors[index].path_len=0;return;}
        let direction=to.normalize_or_zero();
        let reach=if distance>32.0 {distance-24.0}else{-16.0};
        let target=actor.position+direction*reach;
        self.set_target(index,target);
    }
    /// `estimate_od_gracza` (0x10047680): the point 1024..2048 units straight away from the player (horizontal).
    fn estimate_flee(&mut self,index:usize,frame:&Frame) {
        let actor=&self.actors[index];
        let away=Vec3::new(actor.position.x-frame.player.x,0.0,actor.position.z-frame.player.z);
        let length=away.length();if length<1e-6 {return;}
        let target=actor.position+away/length*length.clamp(1024.0,2048.0);
        self.set_target(index,target);
    }
    /// `estimate_kryjowka` (0x10047490): first a short chain of nodes to the side that hides from the player (playing the run-sideways clip), else the nearest node with
    /// a free slot that the player cannot see, else the chase.
    fn estimate_hide(&mut self,index:usize,frame:&Frame) {
        let Some(place)=self.actors[index].place else {self.actors[index].path_len=0;return};
        let toward=slot_of(frame.player-self.actors[index].position) as i32;
        for side in [-1i32,1] {
            let slot=wrap_slot(toward+side*2);
            if let Some(chain)=self.side_chain(place,slot,4,frame) {
                self.store_path(index,&chain);
                let clip=if side<0 {"bieg_lewa"}else{"bieg_prawa"};
                let actor=&mut self.actors[index];
                actor.face_player=true;
                if actor.definition.model_animations.contains_key(clip) {actor.anim=clip.into();actor.rendered_animation=clip.into();actor.elapsed=0.0;}
                return;
            }
        }
        let from=self.navigation.real(place);let graph=self.navigation.nodes[place].graph;
        let mut candidates:Vec<(usize,f32)>=self.navigation.nodes.iter().enumerate()
            .filter(|(i,n)|*i!=place && n.graph==graph && n.enabled && n.slots.iter().any(|s|s.is_none()))
            .map(|(i,n)|(i,(Vec3::from(n.real)-from).abs().element_sum())).collect();
        candidates.sort_by(|a,b|a.1.total_cmp(&b.1));
        for (node,_) in candidates {
            if self.hidden_from_player(self.navigation.real(node)+Vec3::Y*64.0,frame) {
                let target=self.navigation.real(node);
                self.set_target(index,target);return;
            }
        }
        self.estimate_chase(index,frame);
    }
    /// 0x10047330: the nodes reached by following one slot from the place for up to `max` links (the place itself is not listed); the path ends at the farthest one
    /// that the player cannot see.
    fn side_chain(&self,place:usize,slot:usize,max:usize,frame:&Frame)->Option<Vec<usize>> {
        let mut chain=Vec::new();let mut here=self.navigation.nodes[place].slots[slot]?;
        if !self.navigation.nodes[here].enabled {return None;}
        loop {
            chain.push(here);
            if chain.len()>=max {break;}
            match self.navigation.nodes[here].slots[slot] {Some(n) if self.navigation.nodes[n].enabled=>here=n,_=>break}
        }
        (0..chain.len()).rev().find(|end|self.hidden_from_player(self.navigation.real(chain[*end])+Vec3::Y*64.0,frame)).map(|end|chain[..=end].to_vec())
    }
    /// `estimate_do_strzalu` (0x100477e0): follow a slot, starting 90 degrees off the player's direction and turning towards it, until a node sees the player; the path lists
    /// the nodes after the place. Failing that, one in eight, walk to a hidden node. The slot loop keeps the original's exit test (the unwrapped slot meets the wrapped
    /// start slot), so it tries seven slots one way round and eight the other.
    fn estimate_strafe(&mut self,index:usize,frame:&Frame) {
        let Some(place)=self.actors[index].place else {self.actors[index].path_len=0;return};
        let toward=slot_of(frame.player-self.actors[index].position) as i32;
        let step=if self.ai_rng.next()&1==1 {1}else{-1};
        let stop=wrap_slot(toward+step) as i32;
        let mut slot=step+stop;
        let mut tried=0;
        while tried<8 {
            tried+=1;
            slot=wrap_slot(slot) as i32;
            let mut chain=Vec::new();let mut here=place;
            while chain.len()<0x18 {
                match self.navigation.nodes[here].slots[slot as usize] {Some(n) if self.navigation.nodes[n].enabled=>{chain.push(n);here=n;},_=>break}
            }
            if chain.len()>=3 {
                let mut k=1;
                while k<chain.len()-1 {
                    if !self.hidden_from_player(self.navigation.real(chain[k])+Vec3::Y*64.0,frame) {
                        let path:Vec<usize>=chain[..=k].to_vec();self.store_path(index,&path);return;
                    }
                    k+=2;
                }
            }
            slot+=step;
            if slot==stop {break;}
        }
        self.actors[index].path_len=0;
        if self.ai_rng.next()&7==0 && self.actors[index].position.distance(frame.player)<640.0 {self.estimate_cover(index,frame);}
    }
    /// 0x10047a10: a node 128..2000 (Manhattan) away from the actor and at least 192 from the player that the player cannot see, nearest first; else the hide search.
    fn estimate_cover(&mut self,index:usize,frame:&Frame) {
        let Some(place)=self.actors[index].place else {return};
        let from=self.navigation.real(place);let graph=self.navigation.nodes[place].graph;
        let mut candidates:Vec<(usize,f32)>=self.navigation.nodes.iter().enumerate()
            .filter(|(i,n)|*i!=place && n.graph==graph && n.enabled)
            .map(|(i,n)|(i,(Vec3::from(n.real)-from).abs().element_sum()))
            .filter(|(i,d)|*d>=128.0 && *d<=2000.0 && (self.navigation.real(*i)-frame.player).abs().element_sum()>=192.0).collect();
        candidates.sort_by(|a,b|a.1.total_cmp(&b.1));
        for (node,_) in candidates {
            if self.hidden_from_player(self.navigation.real(node)+Vec3::Y*64.0,frame) {let target=self.navigation.real(node);self.set_target(index,target);return;}
        }
        self.estimate_hide(index,frame);
    }
    /// `estimate_kluczy` (0x10047160): a zig-zag step, two links away, roughly at right angles to the player's direction.
    fn estimate_zigzag(&mut self,index:usize,frame:&Frame) {
        let Some(place)=self.actors[index].place else {self.actors[index].path_len=0;return};
        let toward=slot_of(frame.player-self.actors[index].position) as i32;
        let step=if self.ai_rng.next()&1==1 {1}else{-1};
        let mut slot=wrap_slot(toward+2*step) as i32;
        for _ in 0..8 {
            let s=wrap_slot(slot);
            if let Some(n)=self.navigation.nodes[place].slots[s].filter(|n|self.navigation.nodes[*n].enabled) {
                let mut inner=slot;
                for _ in 0..8 {
                    if let Some(n2)=self.navigation.nodes[n].slots[wrap_slot(inner)].filter(|n2|self.navigation.nodes[*n2].enabled) {
                        let actor=&mut self.actors[index];
                        actor.path_buf[0]=Some(n);actor.path_buf[1]=Some(n2);actor.path_len=2;return;
                    }
                    inner+=step;
                }
            }
            slot+=step;
        }
        self.actors[index].path_len=0;
    }

    // -----------------------------------------------------------------------------------------------------------------
    // Turning, sounds, weapons
    // -----------------------------------------------------------------------------------------------------------------

    /// `0x100489f0`: the yaw follows its target at `pi * mod_obrotu` rad/s in two steps per frame; in a phase with `obrot_pion_do_gracza0` the gun pitch follows the
    /// player's elevation at 1 rad/s (otherwise it is 0).
    fn turn(&mut self,index:usize,frame:&Frame) {
        let actor=&mut self.actors[index];
        if actor.definition.flags.iter().any(|f|f=="nie_obracaj") {return;}
        let (face,kier,rate,vertical)={let c=actor.commands();
            (has(c,"obrot_do_gracza") || actor.face_player,number(c,"mod_kier").unwrap_or(0.0),number(c,"mod_obrotu").unwrap_or(1.0),command_value(c,"obrot_pion_do_gracza0").is_some_and(|v|!v.is_empty()))};
        let dt=frame.dt;
        if face {
            let to=frame.player-actor.position;
            actor.yaw_target=yaw_of(to.x,to.z)+kier*FRAC_PI_4;
        }
        actor.yaw_target=wrap_yaw(actor.yaw_target);actor.yaw=wrap_yaw(actor.yaw);
        // Two identical steps (0x10048c3f, 0x10048d43). The original clamps against the target without regard for the 0/2 pi seam, so a turn whose short way crosses
        // north (facing +Z) completes at once: reproduced.
        for _ in 0..2 {
            let (current,target)=(actor.yaw,actor.yaw_target);
            if current==target {break;}
            let direction=if current>target {if current-PI<target {-1.0}else{1.0}}else if current+PI<target {-1.0}else{1.0};
            let mut yaw=current+direction*PI*rate*dt;
            if yaw<0.0 {yaw+=TAU;}
            if yaw>=TAU {yaw-=TAU;}
            if direction>0.0 {if yaw>target {yaw=target;}}else if yaw<target {yaw=target;}
            actor.yaw=yaw;
        }
        actor.rotation=Quat::from_rotation_y(actor.yaw);
        if vertical {
            let aim=frame.player+Vec3::Y*if frame.half.y<40.0 {16.0}else{32.0};
            let from=actor.position+Vec3::Y*actor.half_extents.y*0.25;
            let to=aim-from;
            actor.pitch_target=to.y.atan2(Vec2::new(to.x,to.z).length().max(1.0e-4));
            let diff=actor.pitch_target-actor.pitch;
            actor.pitch+=diff.signum()*dt.min(diff.abs());
        }else{actor.pitch=0.0;actor.pitch_target=0.0;}
    }
    /// Footsteps: `glos_buta1` every `odstep_glosow_buta` seconds when that is above 0.09, 2048 units away when the player sees the actor, else 640 (0x1004ab2b).
    fn step_sound(&mut self,index:usize,frame:&Frame) {
        let actor=&mut self.actors[index];
        let interval=number(actor.commands(),"odstep_glosow_buta").unwrap_or(0.0);
        if interval<=0.09 {actor.step_timer=0.0;return;}
        actor.step_timer-=frame.dt;
        if actor.step_timer>=0.0 {return;}
        actor.step_timer=interval;
        let (position,seen)=(actor.position,actor.seen);
        if let Some(path)=command_value(self.actors[index].commands(),"glos_buta1") {self.pending_sounds.push((path.into(),position,crate::audio::npc_step_radius(seen)));}
    }
    /// `0x10046270`: a phase with `strzal N` fires every N seconds.
    fn fire_timer_step(&mut self,index:usize,frame:&Frame) {
        let actor=&mut self.actors[index];
        if actor.dead {return;}
        let interval=number(actor.commands(),"strzal").unwrap_or(0.0);
        if interval==0.0 {return;}
        actor.fire_timer+=frame.dt;
        if actor.fire_timer<interval {return;}
        actor.fire_timer=0.0;
        self.fire(index,false,frame);
    }

    /// `Fire` (0x100462e0): consume a round (an empty clip enters `on_reload` and refills instead), then trace the bullets along the gun's forward axis.
    fn fire(&mut self,index:usize,second:bool,frame:&Frame) {
        let slot=second as usize;
        let definition=self.actors[index].definition.clone();
        let Some(weapon)=definition.weapon_asset.as_ref() else {return};
        let has_second=command_value(&definition.header,"socket_weapon1").is_some_and(|s|!s.is_empty());
        if second && !has_second {return;}
        if self.actors[index].ammo[slot]==0 {
            let clip=weapon_clip(&definition);
            if let Some(reload)=single(self.actors[index].commands(),"on_reload").map(str::to_owned) {let _=self.enter_phase(index,&reload,true);}
            self.actors[index].ammo=[clip,if has_second {clip}else{0}];
            return;
        }
        self.actors[index].ammo[slot]-=1;self.stats.shots+=1;
        let spread=number(&weapon.commands,"rozrzut").unwrap_or(0.0);
        let strength=number(&weapon.commands,"sila_wroga").unwrap_or(0.0);
        let pellets=number(&weapon.commands,"kul_na_raz").map_or(1,|n|n.max(1.0) as usize);
        let (muzzle,casing,forward)=self.gun_ray(index,frame);
        let sees=self.actors[index].seen;
        // The noise of the shot, for the player's ears and for other actors (0x10005dd9).
        let loudness=number(&weapon.commands,"glosnosc").unwrap_or(0.0);
        self.noises.push(Stimulus::gunshot(muzzle,loudness));
        if let Some(path)=command_value(&weapon.commands,"sound_shoot") {
            let position=self.actors[index].position;
            self.pending_sounds.push((path.into(),position,crate::audio::npc_weapon_radius(loudness,sees,self.cutscene)));
        }
        let player=PlayerTarget {position:frame.player,half_extents:frame.half};
        let range_cap=1.5*muzzle.distance(frame.player);
        let commands=weapon.commands.clone();
        for pellet in 0..pellets {
            let range=(640.0+24.0*(256.0-spread).max(0.0)).min(range_cap);
            let mut end=muzzle+forward*range;
            if spread!=0.0 {
                let effective=spread*(2.0-frame.difficulty);
                end+=Vec3::new(self.shot_rng.offset(effective),self.shot_rng.offset(effective),self.shot_rng.offset(effective));
            }
            let (end,hit_player,victim)=self.trace_bullet(index,muzzle,end,player,strength,frame);
            if hit_player {let damage=enemy_bullet_damage(strength,frame.difficulty);self.pending_damage+=damage;self.stats.bullets_on_player+=1;self.stats.damage_to_player+=damage;}
            self.pending_shots.push(EnemyShot {origin:muzzle,endpoint:end,commands:commands.clone(),casing:if pellet==0 {casing}else{None},hits_player:hit_player,melee:false,bite:false,extra:pellet>0,victim});
        }
    }
    /// Muzzle, casing socket and the direction the gun points (its socket's forward axis, tilted by the gun pitch); without a model the body's heading.
    fn gun_ray(&self,index:usize,_frame:&Frame)->(Vec3,Option<Vec3>,Vec3) {
        let actor=&self.actors[index];
        let flat=Vec3::new(actor.yaw.sin(),0.0,actor.yaw.cos());
        let (muzzle,casing,forward)=weapon_sockets(&self.models,actor).unwrap_or((actor.eye()+flat*24.0,None,flat));
        let forward=forward.try_normalize().unwrap_or(flat);
        let up=(Vec3::Y-forward*forward.y).try_normalize().unwrap_or(Vec3::Y);
        let p=actor.pitch;
        (muzzle,casing,(forward*p.cos()+up*p.sin()).normalize_or_zero())
    }
    /// One bullet: the first thing along the segment takes it: the player, another actor (`sila_wroga`) or the world. Returns the end point and whether the player was hit.
    pub(super) fn trace_bullet(&mut self,index:usize,from:Vec3,to:Vec3,player:PlayerTarget,strength:f32,frame:&Frame)->(Vec3,bool,bool) {
        let delta=to-from;let length=delta.length();
        let Some(direction)=delta.try_normalize() else {return (to,false,false)};
        // The lowest quarter of a corpse's box takes the bullet like a wall (cshell 0x10006836..0x10006898); the rest of it lets the bullet through.
        let world_wall=frame.world.raycast(native(from),native(direction),length).map_or(length,|(d,_)|d.min(length));
        let wall=self.corpse_stop(from,direction,world_wall).map_or(world_wall,|c|c.min(world_wall));
        let clear=|d:f32|d<=length && d<wall && !(frame.blocked)(from,from+direction*d);
        let player_hit=ray_box(from,direction,player.position,player.half_extents).filter(|d|clear(*d));
        let victim=self.actors.iter().enumerate().filter(|(i,a)|*i!=index && !a.dead && a.shown())
            .filter_map(|(i,a)|ray_box(from,direction,a.body_center(),a.half_extents).filter(|d|clear(*d)).map(|d|(i,d))).min_by(|a,b|a.1.total_cmp(&b.1));
        let (end,hit_player)=match (player_hit,victim) {
            (Some(p),Some((i,v))) if v<p=>{self.damage_actor(i,strength,Hit::Bullet);(from+direction*v,false)},
            (Some(p),_)=>(from+direction*p,true),
            (None,Some((i,d)))=>{self.damage_actor(i,strength,Hit::Bullet);(from+direction*d,false)},
            (None,None)=>(from+direction*wall,false),
        };
        // The impact is heard (cshell 0x10007e17): 200 always, 1024 with a free line, two frames.
        if (wall<length && wall==world_wall) || hit_player || victim.is_some() {self.add_stimulus(end,200.0,0.0,1024.0,2);}
        (end,hit_player,!hit_player && victim.is_some_and(|(_,d)|player_hit.is_none_or(|p|d<p)))
    }
}
