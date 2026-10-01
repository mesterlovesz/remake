//! Original campaign mission rules, without renderer or audio dependencies.
use std::{collections::BTreeMap,sync::Arc};
use serde::{Deserialize, Serialize};

pub mod solver;

#[derive(Clone, Debug, Default)]
pub struct NpcState {
    /// Character definition name used by the scripts.
    pub name: String,
    pub position: [f32; 3],
    pub alive: bool,
    /// The actor's `seen` flag (cshell +0x149): the player's eye sees its chest. `ifplayerseenby` and `ifhostileblizejniz` read it.
    pub player_seen: bool,
    /// The actor's contact flag (+0x126, only maintained while its phase has `on_kontakt`): what `ifseenbyhostile` reads.
    pub contact: bool,
    /// Scene instance name; empty in callers that do not track instances.
    pub id: String,
}

#[derive(Clone, Debug, Default)]
pub struct Context {
    pub player_position: [f32; 3],
    pub weapon_drawn: bool,
    pub action_target: Option<String>,
    /// Instance picked by the use probe (the NPC `ifaction` tags as speaker).
    pub action_id: Option<String>,
    /// NPCs in original spawn order: first-instance conditions depend on it.
    pub npcs: Vec<NpcState>,
    /// Names of NPC instances with the source hostile role, supplied by the game.
    /// This is independent of whether a hostile is currently attacking.
    pub hostile_npcs: Vec<String>,
    /// Exact original inventory item identifiers (including punctuation).
    pub inventory: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    pub index: u8,
    pub text: String,
    pub speech: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DialogueView {
    pub id: String,
    /// The node's question line; it stays on screen through the answer stage.
    pub title: String,
    /// The sound of this event: the question's speech, or, in the answer stage, the chosen answer's speech.
    pub speech: Option<String>,
    pub person: Option<String>,
    pub choices: Vec<Choice>,
    /// Answer stage (cshell.dll 0x10018cfb): the chosen answer, shown as the player's own line under the question.
    #[serde(default)] pub answered: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// `setfaza`: the first instance of a definition (or a named instance), skipped when dead.
    SetNpcPhase { name: String, phase: String },
    /// `setallfaza`: every living instance that currently sees the player.
    SetAllNpcPhase { name: String, phase: String },
    HostileAttack,
    /// A dialogue node's `hostileattack` (0x10019811): only its speaker turns to attack; the game maps it as it likes.
    SpeakerAttack(Option<String>),
    /// A dialogue ended without a follow-up node (StartDialog with no node, 0x10019500..0x10019590): every actor still tagged (`aware`, +0x128) is
    /// untagged and re-enters its current phase with the forced flag. Carries the instance ids; only sent when there were any.
    Released(Vec<String>),
    /// StartDialog resets its tagged speaker to `default_faza` (0x10019762 -> SetPhase 0x10041c60); carries the instance id.
    SpeakerDefaultPhase(String),
    Dialogue(DialogueView),
    EndDialogue,
    TransitionWorld(String),
    GrantItem(String),
    /// Dialogue `expgained`: cshell.dll 0x10019a8e only shows the HUD message;
    /// experience itself is awarded for the player's own kills.
    Experience(u32),
    ClearInventory,
    HealthDelta(f32),
    PlayCutscene(String),
    NpcCommand { name: String, command: String, value: String },
    Warning(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Command { key: String, value: String, line: usize }

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ActiveDialogue {
    view: DialogueView,
    elapsed: f32,
    /// Node `delay` (default 0.2), replaced by the speech length when it plays.
    delay: f32,
    timeout: Option<(f32, String)>,
    distance: Option<(f32, String)>,
    choices: BTreeMap<u8, (String, f32)>,
    /// Speaking instance (scene name); resolved from `person` on the next tick if empty.
    #[serde(default)] speaker: Option<String>,
    #[serde(default)] answer: bool,
    /// The choice list is on screen (retail `+0x3ff4e`): the delay passed or a click skipped it.
    #[serde(default)] shown: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub world: String,
    pub flags: BTreeMap<String,bool>,
    pub timers: BTreeMap<String,f32>,
    pub markers: Vec<String>,
    active: Option<ActiveDialogue>,
    callbacks: BTreeMap<String,Vec<Command>>,
}

#[derive(Clone)]
pub struct Mission {
    world: String,
    source: Arc<str>,
    actions: Arc<Vec<Vec<Command>>>,
    dialogues: Arc<BTreeMap<String, Vec<Command>>>,
    keys: Arc<BTreeMap<String, String>>,
    flags: BTreeMap<String, bool>,
    timers: BTreeMap<String, f32>,
    active: Option<ActiveDialogue>,
    markers: Vec<String>,
    callbacks: BTreeMap<String,Vec<Command>>,
    /// NPCs matched by this frame's conditions (the original's +0x128 tag).
    tagged: Vec<(usize,NpcState)>,
    hostile: Vec<String>,
    /// Level-scope keywords of the current world's section (`nie_sprawdzaj_drzwi`, `gestosc_sciezek`, `pure_shooter`, `blyski_postaci`, `load_*`, `muza`): key -> argument.
    level_keys: BTreeMap<String,String>,
}
impl Mission {
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {world:self.world.clone(),flags:self.flags.clone(),timers:self.timers.clone(),
            active:self.active.clone(),markers:self.markers.clone(),callbacks:self.callbacks.clone()}
    }
    pub fn restore(&mut self, snapshot: Snapshot) -> Result<Vec<Event>,String> {
        if snapshot.timers.values().any(|v|!v.is_finite() || *v<0.0) {return Err("Invalid saved mission timers".into());}
        self.enter_world(&snapshot.world)?;
        self.flags=snapshot.flags;self.timers=snapshot.timers;self.markers=snapshot.markers;
        self.callbacks=snapshot.callbacks;self.active=snapshot.active;
        // Display state is restored without executing grants or other dialogue side effects again.
        Ok(self.active.as_ref().map(|a|vec![Event::Dialogue(a.view.clone())]).unwrap_or_default())
    }

    /// The caller tests the authored marker volume against the player hull.
    pub fn trigger_once(&mut self, marker: &str, dialog: &str) -> Vec<Event> {
        let key=format!("{}:{marker}",self.world);
        if self.markers.contains(&key) {return Vec::new();}
        self.markers.push(key);
        self.trigger_dialog(dialog)
    }

    /// Call once when an NPC enters a phase. Use scene-object identity for name so
    /// multiple instances of a character keep separate animation callbacks.
    /// NpcCommand forwards physical/presentation directives to the caller explicitly.
    pub fn phase_enter(&mut self, name: &str, pairs: &[(String,String)]) -> Vec<Event> {
        let mut callbacks=Vec::new();let mut result=Vec::new();
        for (index,(key,value)) in pairs.iter().enumerate() {
            let mut cmd=Command {key:key.clone(),value:value.clone(),line:index+1};
            if key.starts_with("on_") {callbacks.push(cmd);continue;}
            if key.strip_prefix("setfaza").is_some_and(|suffix|suffix.chars().all(|c|c.is_ascii_digit())) {cmd.key="setfaza".into();}
            if matches!(cmd.key.as_str(),"set"|"unset"|"setfaza"|"dialog"|"hostileattack"|"startlevel"|"receive"|"expgained"|"tnijitems"|"zdrowie"|"cutscene") {
                self.execute(&cmd,&mut result);
            } else {
                result.push(Event::NpcCommand {name:name.into(),command:key.clone(),value:value.clone()});
            }
        }
        self.callbacks.insert(name.into(),callbacks);
        result
    }

    /// Fire an authored callback such as on_death, on_koniec_anim, on_kontakt,
    /// on_closer_widzi, or on_further. Caller supplies a deterministic/random branch
    /// number; repeated numbered entries preserve the source's branch weighting.
    pub fn phase_event(&self, name: &str, trigger: &str, branch: usize) -> Vec<Event> {
        let Some(commands)=self.callbacks.get(name) else {return Vec::new();};
        let candidates:Vec<_>=commands.iter().filter(|c|c.key.strip_prefix(trigger)
            .is_some_and(|suffix|suffix.chars().all(|c|c.is_ascii_digit()))).collect();
        if candidates.is_empty() {return Vec::new();}
        vec![Event::SetNpcPhase {name:name.into(),phase:candidates[branch%candidates.len()].value.clone()}]
    }
    pub fn from_sources(world: &str, gameai: &str, dialogues: &str, text_keys: &str) -> Result<Self, String> {
        let mut nodes = BTreeMap::new();
        let mut id = String::new();
        for cmd in commands(dialogues) {
            if cmd.key == "dialog" { id = cmd.value; nodes.entry(id.clone()).or_insert_with(Vec::new); }
            else if !id.is_empty() { nodes.get_mut(&id).unwrap().push(cmd); }
        }
        let keys = text_keys.lines().filter_map(|line| {
            let line = line.trim();
            if !line.starts_with('>') { return None; }
            let (key, value) = split(line);
            Some((key.to_owned(), value.to_owned()))
        }).collect();
        let mut m = Self {world:String::new(),source:gameai.into(),actions:Arc::new(Vec::new()),dialogues:Arc::new(nodes),keys:Arc::new(keys),
            flags:BTreeMap::new(),timers:BTreeMap::from([("CzasOdGadki".into(),0.0)]),active:None,
            markers:Vec::new(),callbacks:BTreeMap::new(),tagged:Vec::new(),hostile:Vec::new(),level_keys:BTreeMap::new()};
        m.enter_world(world)?;
        Ok(m)
    }

    /// All distances use native LithTech coordinates. Omit ticks while paused.
    pub fn tick(&mut self, dt: f32, context: &Context) -> Vec<Event> {
        if !dt.is_finite() || dt <= 0.0 { return Vec::new(); }
        // cshell.dll 0x1001a834: every counter advances, during dialogues too.
        for timer in self.timers.values_mut() { *timer += dt; }
        let mut result = Vec::new();
        self.tick_dialogue(dt, context, &mut result);
        // 0x1001a864: no action runs while a dialogue node is current, so a
        // condition that stays true cannot restart its own speech every frame.
        if self.active.is_some() { return result; }
        self.hostile = context.hostile_npcs.clone();
        self.tagged.clear();
        // Retail runs the actions of a level NEWEST FIRST, i.e. in reverse script order: the node allocator 0x10015be0 links every new node at the head of the level
        // list (+0x380, next +0xaa8) and the driver 0x1001a898 walks it from the head. A flag an action sets is therefore seen in the same frame only by actions
        // written EARLIER in the script (a cascade arrives one frame later), and when two actions hold in the same frame the script-earlier one wins a dialogue or a latch.
        let actions = Arc::clone(&self.actions);
        for action in actions.iter().rev() {
            let mut allowed = true;
            let mut effects: Vec<Command> = Vec::new();
            for cmd in action.iter().cloned() {
                if cmd.key == "endifs" { allowed=true; }
                // The first failing condition stops the rest (and their speaker tags).
                else if cmd.key.starts_with("if") { if allowed { let mut tags=std::mem::take(&mut self.tagged); allowed=self.condition(&cmd,context,&mut tags); self.tagged=tags; } }
                else if allowed { effects.push(cmd); }
            }
            // The effects of one action run in the executor's fixed kind order (0x1001a4ad..0x1001a71f), not in the order they are written.
            effects.sort_by_key(|cmd| effect_rank(&cmd.key));
            for cmd in &effects { self.execute(cmd,&mut result); }
        }
        result
    }

    /// cshell.dll 0x10018ab0 (per frame, `elapsed` is the node clock `+0x218`):
    /// * the choice list appears once `elapsed > delay` (a click shows it earlier, see `click`);
    /// * a node moves on when `elapsed > ontimeexceeded && elapsed > delay` (defaults 0.2 s / end) unless its
    ///   `ontimeexceeded` is below 0.1, which only a click can satisfy; the answer stage skips this and the distance rule;
    /// * or when the player is farther than `onfurtherthan` (default 320, end) from the speaker; no speaker means distance 0.
    fn tick_dialogue(&mut self, dt: f32, context: &Context, result: &mut Vec<Event>) {
        let Some(active) = &mut self.active else { return; };
        active.elapsed += dt;
        if !active.answer && !active.view.choices.is_empty() && active.elapsed > active.delay { active.shown = true; }
        let mut next = None;
        if let Some((seconds, target)) = &active.timeout {
            let click_only = !active.answer && *seconds < 0.1;
            if !click_only && active.elapsed > *seconds && active.elapsed > active.delay { next = Some(target.clone()); }
        }
        if !active.answer && next.is_none() {
            if let (Some((distance, target)), Some(npc)) = (&active.distance, speaker(active, context)) {
                if *distance > 1.0 && distance_between(context.player_position, npc.position) > *distance { next = Some(target.clone()); }
            }
        }
        if let Some(target) = next { self.finish(target, result); }
    }
    fn finish(&mut self, target: String, result: &mut Vec<Event>) {
        self.active = None;
        result.push(Event::EndDialogue);
        if !target.is_empty() { result.extend(self.trigger_dialog(&target)); } else { self.release(result); }
    }
    /// The end of a dialogue with nothing following (0x10019550..0x10019585): every tagged actor loses its `aware` flag and has its current phase re-entered.
    fn release(&mut self, result: &mut Vec<Event>) {
        let ids = self.aware();
        self.tagged.clear();
        if !ids.is_empty() { result.push(Event::Released(ids)); }
    }
    /// The actors the last evaluation tagged (the `aware` flag, cshell +0x128): it is cleared for everyone at the start of every tick without a current
    /// dialogue (0x1001a878) and set by `ifaction`, `ifactionhostile`, `ifplayerseenby` and `ifhostileblizejniz`; it survives a dialogue (no action
    /// runs meanwhile). `SetPhase` refuses `patrol` phases of a tagged actor (0x10041cbe). Instance ids, in list order.
    pub fn aware(&self) -> Vec<String> {
        let mut tagged: Vec<&(usize, NpcState)> = self.tagged.iter().filter(|(_,n)| !n.id.is_empty()).collect();
        tagged.sort_by_key(|(i,_)| *i);tagged.dedup_by_key(|(i,_)| *i);
        tagged.into_iter().map(|(_,n)| n.id.clone()).collect()
    }

    /// The left mouse button (engine command 0x54, edge) with `selected` highlighted (cshell.dll 0x10018bb0..0x10018d34):
    /// * choices not on screen yet: the click shows them at once and is used up;
    /// * choices on screen and `elapsed > delay`: the highlighted answer is taken;
    /// * answer stage: skips the rest of `answerNdelay`;
    /// * a plain node with `ontimeexceeded` below 0.1 ends (`elapsed > delay` still required).
    pub fn click(&mut self, selected: usize) -> Vec<Event> {
        let Some(active) = &mut self.active else { return Vec::new(); };
        let mut result = Vec::new();
        if active.answer {
            let target = active.timeout.as_ref().map(|t| t.1.clone()).unwrap_or_default();
            self.finish(target, &mut result);
        } else if !active.view.choices.is_empty() && !active.shown { active.shown = true; }
        else if !active.view.choices.is_empty() {
            if active.elapsed > active.delay { if let Some(choice) = active.view.choices.get(selected) { let index = choice.index; return self.choose(index); } }
        } else if let Some((seconds, target)) = active.timeout.clone() {
            if seconds < 0.1 && active.elapsed > seconds && active.elapsed > active.delay { self.finish(target, &mut result); }
        }
        result
    }

    /// The node's speech (or answer speech) started: its length replaces `delay`.
    pub fn set_speech_length(&mut self, seconds: f32) {
        if let Some(active) = &mut self.active { if seconds.is_finite() && seconds > 0.0 { active.delay = seconds; } }
    }
    /// Answers are offered once the question's delay (its speech) has passed or a click skipped it.
    pub fn choices_ready(&self) -> bool { self.active.as_ref().is_some_and(|a| a.shown && !a.answer) }
    /// The node clock (`+0x218`), for the caller's probes.
    pub fn dialogue_elapsed(&self) -> f32 { self.active.as_ref().map_or(0.0, |a| a.elapsed) }

    /// Whether pressing use on `target` now would run an authored action:
    /// some action block gated by `ifaction`/`ifactionhostile` on it passes
    /// all its conditions and has a command to execute. Changes nothing.
    pub fn responds_to_action(&self, target: &str, context: &Context) -> bool {
        let mut context = context.clone();
        context.action_target = Some(target.to_owned());
        self.actions.iter().any(|action| {
            let (mut allowed, mut gated) = (true, false);
            action.iter().any(|cmd| {
                if cmd.key == "endifs" { allowed = true; gated = false; false }
                else if cmd.key.starts_with("if") {
                    if matches!(cmd.key.as_str(), "ifaction" | "ifactionhostile") { gated = true; }
                    allowed = allowed && self.condition(cmd, &context, &mut Vec::new()); false
                }
                else { allowed && gated }
            })
        })
    }

    pub fn trigger_dialog(&mut self, id: &str) -> Vec<Event> {
        let Some(node) = self.dialogues.get(id).cloned() else {
            // StartDialog with an unknown name ends the current node.
            let mut result = vec![Event::Warning(format!("Original dialogue is undefined: {id}"))];
            if self.active.take().is_some() { result.push(Event::EndDialogue); self.release(&mut result); }
            return result;
        };
        let mut result = Vec::new();
        // The parser (0x173a0..) keeps one value per key: a repeated line overwrites the earlier one.
        let field = |key: &str| node.iter().rev().find(|c|c.key==key).map(|c|c.value.as_str());
        let title = field("title").map(|s|self.resolve(s)).unwrap_or_default();
        let speech = field("titlesnd").map(|s|self.resolve(s).replace('\\',"/"));
        let person = field("person").map(str::to_owned);
        let mut choices = Vec::new();
        let mut targets = BTreeMap::new();
        // Four answer slots only (`answer1`..`answer4`, 0x1001768b..0x10017a73).
        for index in 1..=4u8 {
            if let Some(text) = field(&format!("answer{index}")) {
                choices.push(Choice {index,text:self.resolve(text),
                    speech:field(&format!("answer{index}snd")).map(|s|self.resolve(s).replace('\\',"/"))});
                targets.insert(index,(field(&format!("onchoice{index}dialog")).unwrap_or("").to_owned(),
                    field(&format!("answer{index}delay")).and_then(|v|v.parse().ok()).unwrap_or(0.2)));
            }
        }
        // Flag-only marker dialogues execute immediately without hiding ongoing speech.
        // StartDialog (cshell.dll 0x100194d0) replaces or restarts any current node;
        // CzasOdGadki is left to the scripts' own set/unset.
        let shows = !title.is_empty() || speech.is_some() || !choices.is_empty();
        if shows {
            if self.active.take().is_some() { result.push(Event::EndDialogue); }
            let tagged = self.tagged_speaker();
            let speaker = match (&tagged, person.as_deref()) { (Some(id), _) => Some(id.clone()), (None, Some("hostile")) => Some(String::new()), _ => None };
            if let Some(id) = tagged.filter(|id| !id.is_empty()) { result.push(Event::SpeakerDefaultPhase(id)); }
            let view = DialogueView {id:id.into(),title,speech,person,choices,answered:None};
            let delay = field("delay").and_then(|v|v.parse().ok()).unwrap_or(0.2);
            self.active = Some(ActiveDialogue {view:view.clone(),elapsed:0.0,delay,
                timeout:Some(field("ontimeexceeded").and_then(number_target).unwrap_or((0.2,String::new()))),
                distance:Some(field("onfurtherthan").and_then(number_target).unwrap_or((320.0,String::new()))),
                choices:targets,speaker,answer:false,shown:false});
            result.push(Event::Dialogue(view));
        }
        for (at,cmd) in node.iter().enumerate() {
            if matches!(cmd.key.as_str(),"person"|"delay"|"title"|"titlesnd"|"ontimeexceeded"|"onfurtherthan")
                || cmd.key.starts_with("answer") || cmd.key.starts_with("onchoice") || node[at+1..].iter().any(|c|c.key==cmd.key) { continue; }
            if cmd.key == "dialog" { result.push(Event::Warning("Nested dialogue command is unsupported".into())); }
            else if cmd.key == "hostileattack" {
                // 0x10019811: the speaker is provoked (+0x127) and loses its `aware` tag (+0x128).
                let speaker = self.active.as_ref().and_then(|a|a.speaker.clone()).filter(|s|!s.is_empty());
                if let Some(id) = &speaker { self.tagged.retain(|(_,n)| n.id != *id); }
                result.push(Event::SpeakerAttack(speaker));
            }
            else { self.execute(cmd,&mut result); }
        }
        result
    }

    /// The highlighted answer (or the answer number) is taken: the node's question stays, the answer is shown and
    /// spoken for `answerNdelay` (its speech length when it plays), then `onchoiceNdialog` starts (0x10018cfb..0x10018f26).
    pub fn choose(&mut self, index: u8) -> Vec<Event> {
        let Some(active) = &self.active else { return Vec::new(); };
        if active.answer || !active.shown || active.elapsed <= active.delay { return Vec::new(); }
        let Some((target,delay)) = active.choices.get(&index).cloned() else { return Vec::new(); };
        let Some(answer)=active.view.choices.iter().find(|c|c.index==index).cloned() else {return Vec::new();};
        let question=active.view.title.clone();
        let view=DialogueView {id:format!("{}/answer/{index}",active.view.id),title:question,speech:answer.speech,person:None,choices:Vec::new(),answered:Some(answer.text)};
        self.active=Some(ActiveDialogue {view:view.clone(),elapsed:0.0,delay,
            timeout:Some((0.0,target)),distance:None,choices:BTreeMap::new(),speaker:Some(String::new()),answer:true,shown:false});
        vec![Event::EndDialogue,Event::Dialogue(view)]
    }

    /// The current node's speaker: the tagged instance id (empty = none by design) or, unresolved, the `person` name
    /// whose first placed instance speaks.
    pub fn speaker(&self) -> Option<(Option<&str>, Option<&str>)> {
        let a = self.active.as_ref()?;
        Some((a.speaker.as_deref().filter(|s| !s.is_empty()), a.view.person.as_deref().filter(|p| *p != "hostile" && a.speaker.is_none())))
    }
    /// Character definitions the level's actions test (`ifalive`, `ifdead`, `ifaction`, `ifplayerseenby`, `ifgraczblizejniz`).
    pub fn referenced_npcs(&self) -> Vec<String> {
        let mut names: Vec<String> = self.actions.iter().flatten().filter_map(|c| match c.key.as_str() {
            "ifalive"|"ifdead"|"ifaction"|"ifplayerseenby" => Some(c.value.clone()),
            "ifgraczblizejniz" => number_target(&c.value).map(|(_,name)| name),
            _ => None,
        }).collect();
        names.sort();names.dedup();names
    }
    /// Quest items the level's actions ask for with `ifplayerhas`.
    pub fn referenced_items(&self) -> Vec<String> {
        let mut items: Vec<String> = self.actions.iter().flatten().filter(|c| c.key == "ifplayerhas").map(|c| c.value.clone()).collect();
        items.sort();items.dedup();items
    }
    /// Whether some action ends the level or starts a cutscene by itself (otherwise the level ends at a door).
    pub fn ends_level(&self) -> bool {self.actions.iter().flatten().any(|c| matches!(c.key.as_str(), "startlevel"|"cutscene"))}
    pub fn dialogue(&self) -> Option<&DialogueView> { self.active.as_ref().map(|a|&a.view) }
    /// cshell.dll 0x10015b30/0x10015b60: on a `licznik` both set and unset restart it.
    pub fn set_flag(&mut self, name: &str, value: bool) {
        if let Some(timer)=self.timers.get_mut(name) { *timer=0.0; return; }
        self.flags.insert(name.into(),value);
    }
    pub fn flag(&self, name: &str) -> bool { self.flags.get(name).copied().unwrap_or(false) }

    /// Preserve mission flags and inventory in the caller across authored map handoffs.
    pub fn enter_world(&mut self, world: &str) -> Result<(), String> {
        let world=normalize_world(world);
        let mut selected=false;
        let mut found=false;
        let mut actions:Vec<Vec<Command>>=Vec::new();
        let mut in_action=false;
        let mut timers=Vec::new();
        let mut level_keys=BTreeMap::new();
        for cmd in commands(&self.source) {
            if cmd.key=="level" {
                selected=normalize_world(&cmd.value)==world;
                found |= selected;in_action=false;continue;
            }
            if !selected { continue; }
            match cmd.key.as_str() {
                "action"=>{ actions.push(Vec::new());in_action=true; },
                "bool"=>{},
                "licznik"=>{timers.push(cmd.value);},
                "muza"|"load_c"|"load_i"|"load_o"|"load_w"|"load_d"|"gestosc_sciezek"|
                "nie_sprawdzaj_drzwi"|"pure_shooter"|"blyski_postaci"=>{level_keys.insert(cmd.key.clone(),cmd.value.clone());},
                _ if in_action=>{
                    if !supported_action(&cmd.key) {
                        return Err(format!("Unsupported original mission command '{}' at gameai line {} ({world})",cmd.key,cmd.line));
                    }
                    if cmd.key=="iflicznikwiekszyniz" {
                        if let Some((_,name))=number_target(&cmd.value) {timers.push(name);}
                    }
                    actions.last_mut().unwrap().push(cmd);
                },
                _=>{}, // Level presentation metadata (loading pictures/music), not executable rules.
            }
        }
        if !found {return Err(format!("No original mission script for world {world}"));}
        for timer in timers { self.timers.entry(timer).or_insert(0.0); }
        self.actions=Arc::new(actions);self.active=None;self.world=world;self.callbacks.clear();self.level_keys=level_keys;
        Ok(())
    }
    /// A level-scope keyword of the current world's section (`nie_sprawdzaj_drzwi` skips the door flags of the path links, cshell 0x1003ca80; `gestosc_sciezek` is the
    /// density used when a path graph is generated, 0x1003c650 / 0x1003ceb0): its argument, `Some("")` for a flag without one.
    pub fn level_setting(&self, key: &str) -> Option<&str> { self.level_keys.get(key).map(String::as_str) }

    /// Conditions tag the instance they matched: it becomes the dialogue speaker.
    fn condition(&self, cmd:&Command, context:&Context, tags:&mut Vec<(usize,NpcState)>)->bool {
        // First placed instance in spawn order, alive or dead (cshell.dll 0x1001a40c).
        let first=|name:&str| context.npcs.iter().find(|n|n.name==name);
        let hostile=|n:&NpcState| n.alive && context.hostile_npcs.contains(&n.name);
        let mut tag=|index:usize| {tags.push((index,context.npcs[index].clone()));true};
        let used=|n:&NpcState| n.alive && context.action_target.as_deref()==Some(n.name.as_str())
            && context.action_id.as_deref().is_none_or(|id|id.is_empty() || n.id.is_empty() || n.id==id);
        match cmd.key.as_str() {
            "if"=>self.flag(&cmd.value),
            "ifnot"=>!self.flag(&cmd.value),
            "ifweapondrawn"=>context.weapon_drawn,
            "ifweaponhidden"=>!context.weapon_drawn,
            // With no instance both are false.
            "ifalive"=>first(&cmd.value).is_some_and(|n|n.alive),
            "ifdead"=>first(&cmd.value).is_some_and(|n|!n.alive),
            // Any living instance that sees the player (0x1001a060).
            "ifplayerseenby"=>context.npcs.iter().position(|n|n.name==cmd.value && n.alive && n.player_seen).is_some_and(&mut tag),
            // The use probe only picks living NPCs, so a corpse is never "used" (0x10019b80).
            "ifaction"=>context.action_target.as_deref()==Some(cmd.value.as_str())
                && context.npcs.iter().position(|n|n.name==cmd.value && used(n)).is_none_or(&mut tag),
            "ifseenbyhostile"=>context.npcs.iter().filter(|n|hostile(n)).any(|n|n.contact),
            // The nearest living hostile whose `seen` flag is set, within the distance (0x10019fc0: distance <= argument).
            "ifhostileblizejniz"=>cmd.value.parse::<f32>().ok().filter(|d|d.is_finite() && *d>=0.0).is_some_and(|d| {
                context.npcs.iter().enumerate().filter(|(_,n)|hostile(n) && n.player_seen)
                    .map(|(i,n)|(i,distance_between(context.player_position,n.position))).filter(|(_,distance)|*distance<=d)
                    .min_by(|a,b|a.1.total_cmp(&b.1)).is_some_and(|(i,_)|tag(i))
            }),
            "ifactionhostile"=>context.npcs.iter().position(|n|hostile(n) && used(n)).is_some_and(&mut tag),
            "ifplayerhas"=>context.inventory.contains(&cmd.value),
            // First instance; a distance of 1 or less never passes.
            "ifgraczblizejniz"=>number_target(&cmd.value).is_some_and(|(d,name)|d>1.0 && first(&name).is_some_and(|n|distance_between(context.player_position,n.position)<d)),
            "iflicznikwiekszyniz"=>number_target(&cmd.value).is_some_and(|(t,name)|self.timers.get(&name).copied().unwrap_or(0.0)>t),
            _=>false,
        }
    }
    /// The speaker StartDialog picks (0x10019737 / 0x1001979f): the first tagged actor in list order, whatever
    /// `person` says; without any tag a named person is the first placed instance (resolved on the next tick)
    /// and `hostile` has no speaker.
    fn tagged_speaker(&self)->Option<String> {
        self.tagged.iter().min_by_key(|(i,_)|*i).map(|(_,n)|n.id.clone())
    }

    fn execute(&mut self, cmd:&Command, result:&mut Vec<Event>) {
        match cmd.key.as_str() {
            "set"=>self.set_flag(&cmd.value,true),
            "unset"=>self.set_flag(&cmd.value,false),
            "setfaza"=>{let (phase,name)=split(&cmd.value);result.push(Event::SetNpcPhase{name:name.into(),phase:phase.into()});},
            "setallfaza"=>{let (phase,name)=split(&cmd.value);result.push(Event::SetAllNpcPhase{name:name.into(),phase:phase.into()});},
            "dialog"=>result.extend(self.trigger_dialog(&cmd.value)),
            "hostileattack"=>{if !result.contains(&Event::HostileAttack) {result.push(Event::HostileAttack);}},
            "startlevel"=>result.push(Event::TransitionWorld(normalize_world(&cmd.value))),
            "receive"=>result.push(Event::GrantItem(cmd.value.clone())),
            "expgained"=>match cmd.value.parse() {Ok(x)=>result.push(Event::Experience(x)),Err(_)=>result.push(Event::Warning(format!("Invalid experience at line {}",cmd.line)))},
            "tnijitems"=>result.push(Event::ClearInventory),
            "zdrowie"=>match cmd.value.parse::<f32>() {
                Ok(delta) if delta.is_finite()=>result.push(Event::HealthDelta(delta)),
                _=>result.push(Event::Warning(format!("Invalid health change at line {}",cmd.line))),
            },
            "cutscene"=>result.push(Event::PlayCutscene(cmd.value.clone())),
            _=>result.push(Event::Warning(format!("Unsupported original command '{}' at line {}",cmd.key,cmd.line))),
        }
    }
    fn resolve(&self, value:&str)->String {self.keys.get(value).cloned().unwrap_or_else(||value.into())}
}

/// Position of an effect keyword in the retail executor (0x1001a140): dialog, startlevel, cutscene, hostileattack, tnijitems, set, unset, setfaza, setallfaza,
/// print, zdrowie, outro. Keywords the executor does not know keep their place at the end.
fn effect_rank(key:&str)->u8 {
    match key {"dialog"=>0,"startlevel"=>1,"cutscene"=>2,"hostileattack"=>3,"tnijitems"=>4,"set"=>5,"unset"=>6,"setfaza"=>7,"setallfaza"=>8,"print"=>9,"zdrowie"=>10,"outro"=>11,_=>12}
}

fn supported_action(key:&str)->bool {
    matches!(key,"if"|"ifnot"|"ifweapondrawn"|"ifweaponhidden"|"ifalive"|"ifdead"|"ifplayerseenby"|
        "ifaction"|"ifgraczblizejniz"|"iflicznikwiekszyniz"|"endifs"|"set"|"unset"|"setfaza"|"setallfaza"|
        "dialog"|"hostileattack"|"startlevel"|"receive"|"expgained"|"tnijitems"|"zdrowie"|"cutscene"|
        "ifseenbyhostile"|"ifhostileblizejniz"|"ifactionhostile"|"ifplayerhas")
}
fn split(line:&str)->(&str,&str) {
    line.split_once(char::is_whitespace).map(|(a,b)|(a,b.trim())).unwrap_or((line,""))
}
fn commands(source:&str)->Vec<Command> {
    source.lines().enumerate().filter_map(|(line,text)| {
        let text=text.split("//").next().unwrap_or("").trim();
        if text.is_empty() {return None;}
        let (key,value)=split(text);
        Some(Command{key:key.into(),value:value.into(),line:line+1})
    }).collect()
}
fn number_target(value:&str)->Option<(f32,String)> {
    let (number,target)=split(value);
    let number=number.parse::<f32>().ok()?;
    if !number.is_finite() || number<0.0 {return None;}
    Some((number,target.into()))
}
fn normalize_world(world:&str)->String {
    world.replace('\\',"/").trim_start_matches("worlds/").trim_end_matches(".dat").to_ascii_lowercase()
}
/// Speaker instance of a node; `Some("")` means the node has none.
fn speaker<'a>(active:&ActiveDialogue, context:&'a Context)->Option<&'a NpcState> {
    match active.speaker.as_deref() {
        Some("")=>None,
        Some(id)=>context.npcs.iter().find(|n|n.id==id),
        None=>active.view.person.as_deref().filter(|p|*p!="hostile").and_then(|p|context.npcs.iter().find(|n|n.name==p)),
    }
}
fn distance_between(a:[f32;3],b:[f32;3])->f32 {
    a.into_iter().zip(b).map(|(a,b)|(a-b)*(a-b)).sum::<f32>().sqrt()
}
