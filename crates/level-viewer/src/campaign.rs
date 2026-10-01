//! Connects the original mission interpreter to world actors, speech and UI.
use bevy::{prelude::*,ecs::system::SystemParam};
use serde::{Deserialize,Serialize};
use mission_runtime::{Mission,Event,Context,NpcState,DialogueView,Snapshot};
use crate::{ViewerConfig,Walking,InspectionCamera,SCALE,settings::Session,npcs::NpcRoster};
use std::{collections::{BTreeMap,BTreeSet,VecDeque},path::Path};

#[derive(Deserialize)] struct Scripts {gameai:String,dialogues:String,text_keys:String}
#[derive(Clone,Serialize,Deserialize)] pub(crate) struct SavedPlayer {pub position:[f32;3],pub yaw:f32,pub pitch:f32,pub stamina:f32,pub max_stamina:f32}
#[derive(Serialize,Deserialize)] struct SavedActor {name:String,position:[f32;3],hp:Option<f32>,phase:String,visible:bool}
/// World state beyond the mission script that a manual save keeps: object chains, doors, dropped weapons.
#[derive(Clone,Default,Serialize,Deserialize)] pub(crate) struct Aux {
    #[serde(default)] pub activation:crate::activation::Saved,#[serde(default)] pub doors:BTreeMap<String,(bool,f32)>,
    #[serde(default)] pub drops:Vec<(String,String,[f32;3])>,#[serde(default)] pub hostiles_attacking:bool,
}
#[derive(Serialize,Deserialize)] struct Checkpoint {world:String,mission:Snapshot,health:f32,#[serde(default="default_health")]max_health:f32,inventory:Vec<String>,experience:u32,#[serde(default)]native:Option<crate::retail_state::Snapshot>,#[serde(default)]player:Option<SavedPlayer>,#[serde(default)]actors:Vec<SavedActor>,#[serde(default)]collected:Vec<String>,#[serde(default)]items:Option<crate::inventory::Items>,#[serde(default)]stats:Option<crate::character::Stats>,#[serde(default)]alcohol:f32,#[serde(default)]kills:u32,#[serde(default)]aux:Aux,/// The one-time dialogue controls hint was shown (a save from before it existed counts as shown).
    #[serde(default="yes")]hint_shown:bool}
fn yes()->bool {true}
fn default_health()->f32 {100.0}
#[derive(Resource)] pub struct Campaign {
    sources:Scripts,pub mission:Option<Mission>,pub health:f32,pub max_health:f32,pub inventory:Vec<String>,pub experience:u32,
    /// Live world state for saves (refreshed by activation::capture) and a save's state waiting to be applied after a load.
    pub(crate) aux:Aux,pending_aux:Option<Aux>,
    pub reset_requested:bool,/// The running level was entered from a save slot (retail skips the level-start quick save then, 0x1005a5d9).
    pub from_save:bool,/// The one-time controls hint of the first dialogue choice list was shown (saved; a new game clears it, dialogue.rs mirrors it into the options).
    pub hint_shown:bool,/// A new game began: dialogue.rs clears the options mirror of `hint_shown`.
    pub hint_reset:bool,restore:Option<Checkpoint>,pub dialogue:Option<DialogueView>,
    warnings:BTreeSet<String>,
    events:VecDeque<Event>,pub world:String,pub collected:BTreeSet<String>,pub dialogue_count:u32,pub transition_count:u32,
    pub items:crate::inventory::Items,pub stats:crate::character::Stats,pub alcohol:f32,pub kills:u32,/// Health-change calls that got past the gates of 0x10061b90 with a non-zero amount: each one re-arms the HUD health bar (0x10061cad), whatever it did to the health.
    pub health_events:u32,/// The hidden console variable `God` (set from the options every frame).
    pub god:bool,
}
#[derive(Component)] pub struct Speech;
#[derive(Component)] pub struct Prompt;
impl Campaign {
    /// Pickups already taken in a manual save; they must not respawn on load.
    pub(crate) fn restored_collected(&self)->Vec<String> {self.restore.as_ref().filter(|c|c.player.is_some()).map(|c|c.collected.clone()).unwrap_or_default()}
    pub(crate) fn clear_restore(&mut self) {self.restore=None;}
    pub(crate) fn take_aux(&mut self)->Option<Aux> {self.pending_aux.take()}
    /// Events produced outside the mission clock (a click on the choice list) join the queue the next tick drains.
    pub(crate) fn queue(&mut self,events:Vec<Event>) {self.events.extend(events);}
    /// An o_marker_dialog volume (or a chain reaching it) starts its dialogue once per level.
    pub(crate) fn marker_dialog(&mut self,name:&str,dialog:&str) {
        if let Some(mission)=&mut self.mission {let events=mission.trigger_once(name,dialog);self.events.extend(events);}
    }
    /// o_marker_zmienna: the client sets/unsets mission variables (cshell.dll 0x10015b30 / 0x10015b60).
    pub(crate) fn set_variable(&mut self,name:&str,value:bool) {if let Some(mission)=&mut self.mission {mission.set_flag(name,value);}}
    pub(crate) fn saved_player(&self)->Option<&SavedPlayer> {self.restore.as_ref()?.player.as_ref()}
    /// The whole restorable game state as a save payload: mission variables, player, actors, weapons and ammo, item grid, stats, world objects.
    pub(crate) fn snapshot_json(&self,world:&str,walking:&Walking,roster:&NpcRoster,native:&crate::retail_weapons::NativeArsenal)->Result<serde_json::Value,String> {
        let mission=self.mission.as_ref().ok_or("Ezen a ponton még nincs menthető küldetés.")?;
        if self.health<=0.0 {return Err("Halál után nem lehet menteni.".into());}
        let p=walking.player.position;
        let save=Checkpoint {world:world.into(),mission:mission.snapshot(),health:self.health,max_health:self.max_health,inventory:self.inventory.clone(),experience:self.experience,aux:self.aux.clone(),hint_shown:self.hint_shown,
            native:Some(native.snapshot()),
            player:Some(SavedPlayer {position:[p.x,p.y,p.z],yaw:walking.yaw,pitch:walking.pitch,stamina:walking.player.stamina,max_stamina:walking.player.max_stamina}),
            actors:roster.actors.iter().map(|actor|SavedActor {name:actor.name.clone(),position:actor.position.to_array(),hp:actor.hp.is_finite().then_some(actor.hp),phase:actor.phase.clone(),visible:actor.visible}).collect(),collected:self.collected.iter().cloned().collect(),
            items:Some(self.items.clone()),stats:Some(self.stats.clone()),alcohol:self.alcohol,kills:self.kills};
        serde_json::to_value(&save).map_err(|error|error.to_string())
    }
    /// Arms a save payload (a slot's content); the level named inside has to be loaded next. `None` for damaged or dead-player saves.
    pub(crate) fn request_payload(&mut self,payload:serde_json::Value)->Option<String> {
        let save:Checkpoint=serde_json::from_value(payload).ok()?;
        if !save.health.is_finite() || save.health<=0.0 || save.player.is_none() {return None;}
        let world=save.world.clone();self.restore=Some(save);Some(world)
    }
}
pub fn init(mut commands:Commands,config:Res<ViewerConfig>,assets:Res<AssetServer>) {
    let sources=serde_json::from_str(&std::fs::read_to_string(config.output.join("gameplay_scripts.json")).expect("original gameplay scripts export")).expect("gameplay script format");
    commands.insert_resource(Campaign {sources,mission:None,health:100.0,max_health:100.0,inventory:Vec::new(),experience:0,reset_requested:false,from_save:false,hint_shown:false,hint_reset:false,restore:None,dialogue:None,warnings:BTreeSet::new(),events:VecDeque::new(),world:String::new(),collected:BTreeSet::new(),dialogue_count:0,transition_count:0,items:default(),stats:default(),alcohol:0.0,kills:0,health_events:0,god:false,aux:Aux::default(),pending_aux:None});
    commands.spawn((Prompt,Text::new(""),TextFont {font:assets.load("hud/subtitles.ttf"),font_size:17.0,..default()},TextColor(Color::WHITE),TextShadow {offset:Vec2::splat(1.5),color:Color::srgba(0.0,0.0,0.0,0.95)},
        Node {position_type:PositionType::Absolute,bottom:percent(17),left:percent(20),width:percent(60),..default()},TextLayout::new_with_justify(Justify::Center),GlobalZIndex(24)));
}
pub fn load_world(mut campaign:ResMut<Campaign>,config:Res<ViewerConfig>,front:Res<crate::frontend::Frontend>,mut session:ResMut<Session>,mut native:ResMut<crate::retail_weapons::NativeArsenal>,mut roster:ResMut<NpcRoster>,mut door_use:ResMut<crate::doors::DoorUse>,ui:Res<crate::retail_ui::RetailUi>) {
    // Door triggers queued during the previous level must not fire on same-named objects here.
    *door_use=crate::doors::DoorUse::default();
    campaign.from_save=false;campaign.collected.clear();campaign.world=config.world.clone();campaign.dialogue=None;session.dialogue_active=false;campaign.events.clear();
    if campaign.reset_requested {campaign.mission=None;campaign.max_health=if front.character_started {front.draft.health as f32}else{100.0};campaign.health=campaign.max_health;campaign.inventory.clear();campaign.experience=0;campaign.reset_requested=false;native.reset();
        campaign.items.clear();campaign.stats=crate::character::Stats::new_game(&front);campaign.alcohol=0.0;campaign.kills=0;campaign.hint_shown=false;campaign.hint_reset=true;}
    // Leaving a level puts the weapon away (0x1005adf0): every level starts with empty hands until a number key draws one.
    native.inventory.empty_hands();
    // A save or checkpoint restore must not stay armed for a later level.
    if config.world=="rh1-wiezienie1" {campaign.mission=None;campaign.restore=None;return;}
    roster.skip_door_flags=false;
    let result=if let Some(mission)=&mut campaign.mission {mission.enter_world(&config.world)}else{
        Mission::from_sources(&config.world,&campaign.sources.gameai,&campaign.sources.dialogues,&campaign.sources.text_keys).map(|m|campaign.mission=Some(m))
    };
    if let Err(error)=result {session.notice=format!("A küldetés nem tölthető: {error}");warn!("{}",session.notice);campaign.mission=None;campaign.restore=None;return;}
    roster.skip_door_flags=campaign.mission.as_ref().is_some_and(|m|m.level_setting("nie_sprawdzaj_drzwi").is_some());
    let manual=campaign.restore.as_ref().is_some_and(|c|c.player.is_some());campaign.from_save=manual;
    campaign.collected=campaign.restore.as_ref().filter(|_|manual).map(|c|c.collected.iter().cloned().collect()).unwrap_or_default();
    if let Some(checkpoint)=campaign.restore.take() {
        campaign.health=checkpoint.health;campaign.max_health=checkpoint.max_health;campaign.experience=checkpoint.experience;
        campaign.items=checkpoint.items.clone().unwrap_or_else(||crate::inventory::Items::from_legacy(&checkpoint.inventory,&ui.catalog));campaign.inventory=checkpoint.inventory;
        campaign.stats=checkpoint.stats.clone().unwrap_or_else(||crate::character::Stats::legacy(campaign.experience,checkpoint.player.as_ref().map(|p|p.max_stamina)));campaign.alcohol=checkpoint.alcohol;campaign.kills=checkpoint.kills;campaign.hint_shown=checkpoint.hint_shown;
        if manual {campaign.pending_aux=Some(checkpoint.aux);}
        for saved in &checkpoint.actors {roster.restore_actor(&saved.name,Vec3::from_array(saved.position),saved.hp.unwrap_or(f32::INFINITY),&saved.phase,saved.visible);}
        // A manual save already contains the effects of the starting phase commands.
        if manual {let _=roster.drain_commands();}
        native.reset();
        if let Some(saved)=&checkpoint.native {native.restore(saved);}else {for item in &campaign.inventory {native.acquire(item);}}
        match campaign.mission.as_mut().unwrap().restore(checkpoint.mission) {Ok(events)=>campaign.events.extend(events),Err(e)=>warn!("Pályakezdés visszaállítása: {e}")};
    }
    info!("Eredeti küldetésszkript betöltve: {}",config.world);
}

#[derive(SystemParam)] pub struct Input<'w,'s> {
    bind:crate::options::Bindings<'w>,door_use:ResMut<'w,crate::doors::DoorUse>,doors:Query<'w,'s,&'static crate::doors::Door>,
    native:ResMut<'w,crate::retail_weapons::NativeArsenal>,
    drops:ResMut<'w,crate::pickups::DropQueue>,feedback:ResMut<'w,crate::character::Feedback>,view:Res<'w,crate::view::ViewState>,
}
pub fn tick(mut commands:Commands,mut campaign:ResMut<Campaign>,mut roster:ResMut<NpcRoster>,walking:Res<Walking>,camera:Single<&Transform,With<InspectionCamera>>,time:Res<Time>,mut input:Input,mut session:ResMut<Session>,mut opening:ResMut<crate::opening::Opening>,mut prompt:Single<&mut Text,With<Prompt>>,mut travel:ResMut<crate::travel::Travel>,config:Res<ViewerConfig>) {
    if opening.active {prompt.0.clear();return;}
    if session.paused {return;}
    roster.player_dead=campaign.dead();
    campaign.god=input.bind.options.god;roster.invisible=input.bind.options.invisible;
    if campaign.dead() {
        // The original mission halts on death; nothing may transition or overwrite the checkpoint.
        // The retail shell prints GameShell4 on death (character.rs); the menu and F9 do the rest.
        session.dialogue_active=true;prompt.0.clear();return;
    }
    let p=walking.player.position;let position=Vec3::new(p.x,p.y,p.z);
    let target=roster.use_target(camera.translation/SCALE,*camera.forward(),&walking.world);
    // `ifaction` reads the action key level triggered (0x10019b80 asks 0x10011960 every mission tick) and does not compete with the doors (0x1005faa0 sends the
    // use message and runs the prop / pickup code whatever the actor probe finds): holding the key at a character starts the next dialogue when one ends.
    let action=if input.bind.pressed(crate::keys_cfg::cmd::ACTION) && !session.dialogue_active {target.as_ref().and_then(|name|roster.actors.iter().find(|a|&a.name==name)).map(|a|a.definition_name.clone())}else{None};
    // Retail scripts read the actors' flags: `seen` for ifplayerseenby / ifhostileblizejniz, contact for ifseenbyhostile.
    let states:Vec<NpcState>=roster.actors.iter().map(|npc| {
        NpcState {name:npc.definition_name.clone(),position:npc.position.to_array(),alive:npc.alive(),player_seen:npc.seen_player(),contact:npc.noticed_player(),id:npc.name.clone()}
    }).collect();
    let dt=time.delta_secs().min(0.05);
    // Every armed character lets its weapon(s) fall when it dies (cshell `Kill` 0x10042eec..0x10043085), whatever phase it dies in; `drop_weapon` in a phase
    // is not a keyword of the retail parser and only hides the carried model.
    for drop in roster.drain_drops() {input.drops.pending.push((drop.owner,drop.kind,drop.position));}
    for (name,pairs) in roster.drain_commands() {
        if let Some(mission)=&mut campaign.mission {let events=mission.phase_enter(&name,&pairs);campaign.events.extend(events);}
    }
    let inventory=campaign.items.names();
    let target_definition=target.as_ref().and_then(|name|roster.actors.iter().find(|a|&a.name==name)).map(|a|a.definition_name.clone());
    let action_id=target.clone().filter(|_|action.is_some());
    let mut context=Context {player_position:position.to_array(),weapon_drawn:input.native.inventory.equipped,action_target:None,action_id,npcs:states,hostile_npcs:roster.actors.iter().filter(|a|a.hostile).map(|a|a.definition_name.clone()).collect(),inventory};
    // The use prompt is shown only when the original script would react to E.
    let talkable=target_definition.is_some_and(|name|campaign.mission.as_ref().is_some_and(|m|m.responds_to_action(&name,&context)));
    context.action_target=action;
    if let Some(mission)=&mut campaign.mission {
        let events=mission.tick(dt,&context);
        // The `aware` tags of this tick's conditions (+0x128): the actor update that follows refuses their patrol phases.
        roster.set_aware(&mission.aware());
        campaign.events.extend(events);
    }
    let mut budget=256;
    while let Some(event)=campaign.events.pop_front() {
        budget-=1;if budget==0 {error!("Mission event cycle detected");campaign.events.clear();break;}
        match event {
            Event::SetNpcPhase{name,phase}=>{roster.set_phase(&name,&phase);},
            Event::SetAllNpcPhase{name,phase}=>{let seen:Vec<bool>=roster.actors.iter().map(|npc|npc.seen_player()).collect();roster.set_phase_seeing(&name,&phase,&seen);},
            Event::HostileAttack=>roster.provoke_noticing(),
            Event::Dialogue(dialogue)=>{
                info!("Párbeszéd {}: {}",dialogue.id,dialogue.title);campaign.dialogue_count+=1;campaign.dialogue=Some(dialogue);
            },
            // Every node end wipes the HUD message (the hint, an experience note): cshell 0x10018cb2 / 0x10018f74 / 0x10018fd7 -> HUD SetMessage("").
            Event::EndDialogue=>{campaign.dialogue=None;input.feedback.message.clear();input.feedback.message_time=0.0;},
            Event::SpeakerDefaultPhase(id)=>{if let Some(phase)=roster.default_phase_of(&id) {roster.reset_speaker(&id,&phase);}},
            // A dialogue node's `hostileattack` provokes only its speaker (0x10019811); the mission action of the same name is `provoke_noticing`.
            Event::SpeakerAttack(speaker)=>if let Some(id)=speaker {roster.provoke_speaker(&id);},
            // The tags of the ended dialogue are released; those actors re-enter their current phase (0x10019550..0x10019585).
            Event::Released(ids)=>roster.release_aware(&ids),
            Event::TransitionWorld(world)=>{campaign.transition_count+=1;travel.pending=Some(world);},
            // `receive` drops the item at the player, who takes it by touch (0x10019a04).
            Event::GrantItem(item)=>{campaign.inventory.push(item.clone());input.drops.grants.push(item);},
            // Retail `expgained` only prints a message; no experience is added (0x10019a94).
            Event::Experience(amount)=>input.feedback.experience_message=Some(amount),
            Event::HealthDelta(amount)=>campaign.change_health(amount,input.view.level_frames()),
            Event::PlayCutscene(name)=>opening.pending_scene=Some(name),
            // `tnijitems` (0x1001a558): health to max, ammo emptied, every carried item taken.
            Event::ClearInventory=>{campaign.inventory.clear();input.native.reset();campaign.items.clear();campaign.health=campaign.max_health;},
            Event::NpcCommand{name:_,command:_,value:_}=>{}, // Executed physically by the source phase adapter.
            Event::Warning(message)=>{if campaign.warnings.insert(message.clone()) {warn!("Küldetésszkript: {message}");}},
        }
    }

    let damage=roster.drain_damage();
    // Retail ignores every health change during the first 60 frames of a level (cshell 0x10061ba7).
    campaign.change_health(-damage,input.view.level_frames());
    // No drowning: retail water surfaces are solid floor (docs/retail-movement-audit.md, docs/retail-scenes.md); the deadly sewer water of podziemia1/1a is o_marker_death volumes (activation.rs).
    // Only death halts the game now: a conversation leaves walking, yaw and weapons live (dialogue.rs); the choice list only blocks the fire button.
    session.dialogue_active=false;
    prompt.0=if campaign.dead() {String::new()}else if talkable {format!("[{}] Beszélgetés / használat",input.bind.label(crate::keys_cfg::cmd::ACTION))}else {String::new()};
    if campaign.dead() {session.dialogue_active=true;}
}

pub fn hint(doors:Res<crate::doors::DoorUse>,session:Res<Session>,intro:Res<crate::opening::Opening>,mut prompt:Single<&mut Text,With<Prompt>>) {
    if !session.paused && !session.dialogue_active && !intro.active {if let Some(hint)=&doors.hint {prompt.0=hint.clone();}}
}

#[cfg(test)] mod save_tests {
    use super::*;
    /// A save written before the inventory grid, character sheet and alcohol level existed still loads.
    #[test] fn old_checkpoints_without_the_new_fields_stay_loadable() {
        let full=Checkpoint {world:"rh1-wiezienie2".into(),mission:Snapshot::default(),health:80.0,max_health:100.0,inventory:vec!["Glock".into(),"an apple".into()],experience:30,native:None,player:None,actors:Vec::new(),collected:Vec::new(),
            items:Some(default()),stats:Some(default()),alcohol:12.0,kills:3,aux:default(),hint_shown:false};
        let mut value=serde_json::to_value(&full).unwrap();
        for key in ["items","stats","alcohol","kills","native","player","actors","collected","max_health","aux","hint_shown"] {value.as_object_mut().unwrap().remove(key);}
        let old:Checkpoint=serde_json::from_value(value).expect("old save format");
        assert!(old.items.is_none() && old.stats.is_none() && old.alcohol==0.0 && old.kills==0 && old.max_health==100.0);
        assert!(old.aux.doors.is_empty() && old.aux.drops.is_empty() && !old.aux.hostiles_attacking,"saves from before the world state existed load with a fresh world");
        assert!(old.hint_shown,"a save from before the one-time hint counts as shown");assert_eq!(old.inventory,vec!["Glock","an apple"]);
    }
    #[test] fn doors_drops_and_the_hostile_flag_survive_a_save_round_trip() {
        let mut aux=Aux::default();aux.doors.insert("b_door3".into(),(true,0.5));aux.drops.push(("o_postac7".into(),"Glock".into(),[1.0,2.0,3.0]));aux.hostiles_attacking=true;
        let back:Aux=serde_json::from_str(&serde_json::to_string(&aux).unwrap()).unwrap();
        assert_eq!(back.doors["b_door3"],(true,0.5));assert_eq!(back.drops,aux.drops);assert!(back.hostiles_attacking);
    }
    #[test] fn the_item_grid_and_stats_survive_a_save_round_trip() {
        let mut items=crate::inventory::Items::default();items.add("an apple",2,false);items.add("Glock",1,true);
        let mut stats=crate::character::Stats::default();stats.skills[0]=5.5;stats.free_points=31;
        let text=serde_json::to_string(&(&items,&stats)).unwrap();
        let (items2,stats2):(crate::inventory::Items,crate::character::Stats)=serde_json::from_str(&text).unwrap();
        assert_eq!(items2,items);assert_eq!(stats2,stats);
    }
}
