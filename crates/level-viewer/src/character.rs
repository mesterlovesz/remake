//! Original character statistics and leveling (cshell.dll globals 0x100b2390..0x100b23ec).
//!
//! Kills are the only experience source in retail: `expgained` in a dialogue only
//! prints a message (0x10019a94). Strength and dexterity are display values; there
//! is no carry limit or encumbrance anywhere in the client.
use bevy::prelude::*;
use serde::{Deserialize,Serialize};

/// Weapon skill slots in panel order, `experience_index` 1..8 in items.txt.
pub const SKILL_NAMES:[&str;8]=["GLOCK","S&W","SiG","INGRAM","FN SHOTGUN","M-14","HK G8","P 90"];

#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
pub struct Stats {
    pub strength:f32,pub dexterity:f32,pub max_stamina:f32,pub skills:[f32;8],
    pub free_points:u32,pub next_level:u32,pub level:u32,
    #[serde(default)] pub power_up:f32,#[serde(default)] pub pain_killer:f32,#[serde(default)] pub last_experience:u32,
}
impl Default for Stats {
    /// 0x1001aa80: strength/dexterity/max stamina 100, skills 0, threshold 1000, 30 points, level 2.
    fn default()->Self {Self {strength:100.0,dexterity:100.0,max_stamina:100.0,skills:[0.0;8],free_points:30,next_level:1000,level:2,power_up:0.0,pain_killer:0.0,last_experience:0}}
}
impl Stats {
    pub fn skill(&self,experience_index:i32)->f32 {usize::try_from(experience_index-1).ok().and_then(|i|self.skills.get(i)).copied().unwrap_or(0.0)}
    /// 0x10006aa1: +0.1 for each bullet or pellet that hits a living character.
    pub fn record_hit(&mut self,experience_index:i32) {
        if let Some(skill)=usize::try_from(experience_index-1).ok().and_then(|i|self.skills.get_mut(i)) {if *skill<99.8 {*skill+=0.1;}}
    }
    /// 0x10005fb7: player spread = rozrzut * (1 - 0.0075 * skill).
    pub fn spread(&self,raw:f32,experience_index:i32)->f32 {raw*(1.0-0.0075*self.skill(experience_index))}
    /// 0x100051f6: view kick strength = shake_screen * max(0, 100 - skill) * 0.01.
    pub fn kick(&self,shake_screen:f32,experience_index:i32)->f32 {shake_screen*(100.0-self.skill(experience_index)).max(0.0)*0.01}
    /// Level-up only when the threshold lies in (old, new]; one level per change.
    /// A single gain crossing two thresholds stops leveling for good, as in retail.
    pub fn observe_experience(&mut self,experience:u32)->bool {
        let old=std::mem::replace(&mut self.last_experience,experience);
        if !(self.next_level>old && experience>=self.next_level) {return false;}
        self.free_points+=1;self.level+=1;self.next_level+=((self.level as f64).sqrt()*1000.0) as u32;
        true
    }
    /// 0x100b23b0: damage is scaled by (100 - min(painkiller, 100)) * 0.01.
    pub fn pain_factor(&self)->f32 {(100.0-self.pain_killer.min(100.0))*0.01}
    /// Start (0x100284c0): allocated stamina and skills; unspent points stay free.
    pub fn new_game(front:&crate::frontend::Frontend)->Self {
        if !front.character_started {return Self::default();}
        let d=&front.draft;
        Self {max_stamina:d.stamina as f32,skills:d.skills.map(|s|s as f32),free_points:d.points as u32,..Self::default()}
    }
    /// Saves from before the character sheet: keep the threshold in step with the experience.
    pub fn legacy(experience:u32,max_stamina:Option<f32>)->Self {
        let mut stats=Self {free_points:0,max_stamina:max_stamina.unwrap_or(100.0),..Self::default()};
        while experience>=stats.next_level {stats.level+=1;stats.next_level+=((stats.level as f64).sqrt()*1000.0) as u32;}
        stats.last_experience=experience;stats
    }
    /// 0x10061d20: per-second decay of the timed item effects.
    pub fn decay(&mut self,dt:f32) {self.pain_killer=(self.pain_killer-dt).max(0.0);self.power_up=(self.power_up-dt).max(0.0);}
}

/// Player-attribute screen (0x1003a8e4): one free point per click, no way back.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Spend {MaxStamina,MaxHealth,Strength,Skill(usize)}
pub fn spend(stats:&mut Stats,health:&mut f32,max_health:&mut f32,stamina:&mut f32,choice:Spend)->bool {
    if stats.free_points==0 {return false;}
    match choice {
        Spend::MaxStamina=>{stats.max_stamina+=1.0;*stamina+=1.0;},
        Spend::MaxHealth=>{*max_health+=1.0;*health+=1.0;},
        Spend::Strength=>stats.strength+=1.0,
        Spend::Skill(i)=>{if stats.skills.get(i).is_none_or(|s|*s>=98.8) {return false;}stats.skills[i]+=1.0;},
    }
    stats.free_points-=1;true
}

/// Pure form of the health-change function (0x10061b90): no change during the first 60 level frames, a loss is scaled by the painkiller
/// factor, a gain is not, the result stays within 0..max.
pub fn health_after(health:f32,max_health:f32,pain_factor:f32,delta:f32,level_frames:u32)->f32 {
    if level_frames<60 {return health;}
    (health+if delta<0.0 {delta*pain_factor}else{delta}).clamp(0.0,max_health)
}

/// Does a health-change call re-arm the HUD health bar (0x10061cad)? Any non-zero amount past the level-start gate does, whatever it does to the health.
pub fn arms_health_bar(delta:f32,level_frames:u32)->bool {level_frames>=60 && delta!=0.0}

/// Transient HUD feedback owned by the character systems.
#[derive(Resource,Default)]
pub struct Feedback {pub level_up:f32,pub message:String,pub message_time:f32,pub experience_message:Option<u32>}

impl crate::campaign::Campaign {
    /// A power-up keeps the player alive at zero health until it wears off (0x10061b90).
    pub fn dead(&self)->bool {self.health<=0.0 && self.stats.power_up<=0.0}
    /// The health-change function (cshell 0x10061b90), used by items, script health events, falls, blasts and traps: ignored during the
    /// first 60 frames of a level (0x10061ba7) or once dead; a loss is scaled by (100 - min(painkiller,100)) * 0.01 (0x10061c21), a gain is not;
    /// the result is clamped to 0..max.
    pub fn change_health(&mut self,delta:f32,level_frames:u32) {
        if self.dead() {return;}
        // 0x10061cad: any non-zero amount past the level-start gate arms the health bar for two seconds, even when the clamp leaves the health as it was
        // (healing at full health, a loss the painkiller absorbs completely); a zero amount does not.
        // `God` (0x10061bb4): after the level-start gate every health change just refills the health; nothing else happens, the bar is not armed.
        if self.god && level_frames>=60 {self.health=self.max_health;return;}
        if arms_health_bar(delta,level_frames) {self.health_events=self.health_events.wrapping_add(1);}
        self.health=health_after(self.health,self.max_health,self.stats.pain_factor(),delta,level_frames);
    }
    /// One consumed `eaten` item (0x100348e0 belt, 0x10035488 panel): health goes through the function above (an item is still used up at
    /// full health), the timed effects are simply added (painkiller 0x100b23b0, power-up 0x100b23ac, alcohol 0x100b23b4).
    pub fn apply(&mut self,effect:crate::inventory::Effect,level_frames:u32) {
        if self.dead() {return;}
        self.change_health(effect.health,level_frames);
        self.stats.power_up+=effect.power_up;self.stats.pain_killer+=effect.pain_killer;self.alcohol+=effect.alcohol;
    }
    /// Death (0x10060070) zeroes the painkiller, power-up and alcohol values; nothing else is lost (no experience or item penalty exists).
    pub fn death_penalty(&mut self) {self.stats.pain_killer=0.0;self.stats.power_up=0.0;self.alcohol=0.0;}
}

pub fn tick(time:Res<Time>,session:Res<crate::settings::Session>,opening:Res<crate::opening::Opening>,panels:Res<crate::panels::Panels>,mut campaign:ResMut<crate::campaign::Campaign>,mut feedback:ResMut<Feedback>,ui:Res<crate::retail_ui::RetailUi>,mut walking:ResMut<crate::Walking>) {
    // Max stamina lives with the character sheet; the controller only reads it.
    if walking.player.max_stamina!=campaign.stats.max_stamina {walking.player.max_stamina=campaign.stats.max_stamina;walking.player.stamina=walking.player.stamina.min(campaign.stats.max_stamina);}
    if let Some(amount)=feedback.experience_message.take() {feedback.message=format!("{} {amount} {}",ui.catalog.text("IngameText2"),ui.catalog.text("IngameText3"));feedback.message_time=5.0;}
    // Death (0x1005ac..): the shell prints GameShell4 - Esc opens the menus, F9 loads the quick save.
    if campaign.dead() && !opening.active {feedback.message=ui.text("GameShell4");feedback.message_time=2.0;}
    if session.paused || opening.active {return;}
    let dt=time.delta_secs().min(0.05);
    if campaign.dead() {campaign.death_penalty();}
    campaign.alcohol=(campaign.alcohol-0.5*dt).max(0.0);
    campaign.stats.decay(dt);
    let experience=campaign.experience;
    if campaign.stats.observe_experience(experience) {
        feedback.level_up=3.0;feedback.message=ui.catalog.text("GameShell3");feedback.message_time=7.0;
        info!("Szintlépés: szabad pont={} következő szint={}",campaign.stats.free_points,campaign.stats.next_level);
    }
    // The level-up icon timer pauses while the inventory or X panel is open (0x10037e10).
    if !panels.inventory && !panels.attributes {feedback.level_up=(feedback.level_up-dt).max(0.0);}
    feedback.message_time=(feedback.message_time-dt).max(0.0);
    if feedback.message_time<=0.0 {feedback.message.clear();}
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn the_health_bar_is_armed_by_any_non_zero_amount_after_the_level_start() {
        assert!(arms_health_bar(-5.0,60) && arms_health_bar(25.0,1000),"a loss (even one the painkiller absorbs) and a gain at full health");
        assert!(!arms_health_bar(0.0,1000),"a zero-size health event does nothing");
        assert!(!arms_health_bar(-5.0,59),"nothing during the first 60 frames of a level");
    }
    #[test] fn owner_screenshot_thresholds_follow_the_square_root_rule() {
        let mut stats=Stats::default();let mut seen=Vec::new();
        for exp in (0..=22000).step_by(50) {if stats.observe_experience(exp) {seen.push(stats.next_level);}}
        assert_eq!(&seen[..8],&[2732,4732,6968,9417,12062,14890,17890,21052]);
        let mut stats=Stats::default();for exp in [0,1200,3000,5350] {stats.observe_experience(exp);}
        assert_eq!((stats.free_points,stats.next_level),(33,6968),"5350 experience: 3 levels, next 6968");
    }
    #[test] fn a_gain_across_two_thresholds_stops_leveling_like_retail() {
        let mut stats=Stats::default();stats.observe_experience(2800);
        assert_eq!(stats.next_level,2732);assert!(!stats.observe_experience(2900));assert!(!stats.observe_experience(9000));
    }
    #[test] fn points_buy_one_unit_and_skills_stop_near_one_hundred() {
        let mut stats=Stats {free_points:3,..default()};let (mut health,mut max,mut stamina)=(40.0,100.0,50.0);
        assert!(spend(&mut stats,&mut health,&mut max,&mut stamina,Spend::MaxHealth));assert_eq!((health,max),(41.0,101.0));
        assert!(spend(&mut stats,&mut health,&mut max,&mut stamina,Spend::MaxStamina));assert_eq!((stats.max_stamina,stamina),(101.0,51.0));
        stats.skills[0]=98.8;assert!(!spend(&mut stats,&mut health,&mut max,&mut stamina,Spend::Skill(0)));
        assert!(spend(&mut stats,&mut health,&mut max,&mut stamina,Spend::Strength));assert_eq!(stats.strength,101.0);
        assert!(!spend(&mut stats,&mut health,&mut max,&mut stamina,Spend::Strength),"no points left");
    }
    #[test] fn hits_train_the_weapon_skill_which_narrows_spread_and_kick() {
        let mut stats=Stats::default();for _ in 0..55 {stats.record_hit(1);}
        assert!((stats.skill(1)-5.5).abs()<1e-3,"owner screenshot: GLOCK 5.50");
        assert!((stats.spread(64.0,1)-64.0*(1.0-0.0075*stats.skill(1))).abs()<1e-4);
        assert_eq!(stats.kick(5.0,6),5.0);stats.record_hit(0);stats.record_hit(9);assert_eq!(stats.skills.iter().filter(|s|**s>0.0).count(),1);
    }
    #[test] fn painkiller_scales_damage_and_timed_effects_decay() {
        let mut stats=Stats {pain_killer:40.0,power_up:1.0,..default()};
        assert!((stats.pain_factor()-0.6).abs()<1e-6);stats.decay(2.0);assert_eq!((stats.pain_killer,stats.power_up),(38.0,0.0));
        stats.pain_killer=150.0;assert_eq!(stats.pain_factor(),0.0);
    }
    #[test] fn health_changes_follow_the_retail_function() {
        assert_eq!(health_after(50.0,100.0,1.0,-20.0,59),50.0,"ignored during the first 60 frames of a level");
        assert_eq!(health_after(50.0,100.0,1.0,-20.0,60),30.0);
        assert!((health_after(50.0,100.0,0.6,-20.0,100)-38.0).abs()<1e-4,"painkiller 40 scales a loss by 0.6");
        assert_eq!(health_after(50.0,100.0,0.6,25.0,100),75.0,"a gain ignores the painkiller");
        assert_eq!(health_after(90.0,100.0,1.0,50.0,100),100.0,"capped at the maximum");
        assert_eq!(health_after(10.0,100.0,1.0,-50.0,100),0.0);
    }
    #[test] fn stats_and_items_survive_a_save_round_trip() {
        let mut stats=Stats {power_up:12.0,pain_killer:30.0,last_experience:2900,free_points:7,level:4,next_level:4732,..default()};stats.skills[2]=41.5;
        let back:Stats=serde_json::from_str(&serde_json::to_string(&stats).unwrap()).unwrap();assert_eq!(back,stats);
        let mut items=crate::inventory::Items::default();items.add("an apple",3,false);items.add("Glock",1,true);items.move_item((0,0),(2,5));
        let back:crate::inventory::Items=serde_json::from_str(&serde_json::to_string(&items).unwrap()).unwrap();assert_eq!(back,items);
    }
    fn default<T:Default>()->T {T::default()}
}
