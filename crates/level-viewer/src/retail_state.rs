//! Original carried-weapon inventory, independent of rendering and movement.
//!
//! Source clips run in their numbered order. Primary fire may restart a shooting
//! sequence after shot_latency; its remaining animation is not a fire lock.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize)]
pub struct Catalog {
    pub schema_version: u32,
    pub weapons: Vec<Definition>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Definition {
    pub id: String,
    pub title: String,
    pub model: String,
    pub skins: BTreeMap<String, String>,
    pub hud: String,
    pub offset: [f32; 3],
    pub scale: [f32; 3],
    pub melee: bool,
    pub automatic: bool,
    #[serde(default)]
    pub grenade: bool,
    #[serde(default)]
    pub shotgun: bool,
    #[serde(default = "one_pellet")]
    pub pellets: u32,
    #[serde(default = "selectable")]
    pub player_selectable: bool,
    pub ammo_index: i32,
    pub ammo_amount: u32,
    pub capacity: u32,
    pub damage: f32,
    pub shot_latency: f32,
    pub spread: f32,
    #[serde(default = "grenade_fuse")]
    pub fuse_seconds: f32,
    pub animations: Animations,
    pub durations: BTreeMap<String, f32>,
    pub sounds: BTreeMap<String, Option<String>>,
}

fn one_pellet() -> u32 { 1 }
fn selectable() -> bool { true }
fn grenade_fuse() -> f32 { 5.0 }

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Animations {
    pub base: String,
    #[serde(default)]
    pub shoot: Vec<String>,
    #[serde(default)]
    pub reload: Vec<String>,
    #[serde(default)]
    pub hold: String,
}

#[derive(Clone, Debug, Default)]
pub struct WeaponState {
    pub owned: bool,
    pub magazine: u32,
    pub cooldown: f32,
    selectable: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ActionKind {
    #[default]
    Idle,
    Shoot,
    Reload,
}

#[derive(Clone, Debug, Default)]
pub struct Action {
    pub kind: ActionKind,
    pub elapsed: f32,
    pub duration: f32,
}

#[derive(Clone, Debug)]
struct Stage {
    clip: usize,
    duration: f32,
    insert_shell: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GrenadePhase { Pin, Hold, Throw, Recover }

/// Weapon change (cshell 0x100034a0, 0x100100c0): the old weapon lowers (state 9), then the new one rises (state 10).
/// Both run a 0..10 counter at 30 per second, i.e. one third of a second each; nothing fires or reloads meanwhile.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Switch { Lower { target: usize, progress: f32 }, Raise { progress: f32 } }
/// Counter speed and depth of the lowering (0x1006621c = 30, 0x10066170 = 10); the view drops that many units.
pub const SWITCH_SPEED: f32 = 30.0;
pub const SWITCH_DEPTH: f32 = 10.0;

#[derive(Clone, Debug)]
struct GrenadeAction {
    phase: GrenadePhase,
    elapsed: f32,
    fuse: f32,
}

#[derive(Clone, Debug)]
pub struct Inventory {
    pub weapons: Vec<WeaponState>,
    pub selected: Option<usize>,
    pub equipped: bool,
    pub ammo: BTreeMap<i32, u32>,
    pub action: Action,
    stages: Vec<Stage>,
    idle_elapsed: f32,
    grenade: Option<GrenadeAction>,
    switch: Option<Switch>,
    /// The view-model dip (+0x6f0) left behind when a reload interrupted a weapon change: retail keeps drawing the weapon that low until the next change.
    frozen_dip: f32,
    casing_pending: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Snapshot {
    pub weapons: Vec<SavedWeapon>,
    pub ammo: BTreeMap<i32, u32>,
    pub selected: Option<String>,
    pub equipped: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SavedWeapon {
    pub id: String,
    pub owned: bool,
    pub magazine: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TickOutcome {
    pub shot: Option<usize>,
    pub reload_started: bool,
    pub reload_completed: bool,
    pub grenade_primed: bool,
    pub grenade_fuse: Option<f32>,
    pub grenade_in_hand: bool,
    /// Shotgun shells that went into the magazine this tick (each plays `sound_reload`, cshell 0x100100a0).
    pub shells_inserted: u32,
    /// The empty case of the last shot leaves the weapon now: retail ejects it when the first shoot clip ends or when the next shot
    /// interrupts that clip (0x100101ef, 0x10004029).
    pub eject_casing: bool,
    /// A revolver-style reload passed its first clip: the cylinder is open and the six casings fall (0x1000a1d0, called at 0x10010293).
    pub eject_casings: bool,
    /// The last grenade was thrown: the item leaves the inventory (0x1000b142..0x1000b16c).
    pub grenade_spent: Option<usize>,
}

impl Inventory {
    pub fn new(definitions: &[Definition]) -> Self {
        Self {
            weapons: definitions.iter().map(|d| WeaponState {
                selectable: d.player_selectable, ..Default::default()
            }).collect(),
            selected: None,
            equipped: false,
            ammo: BTreeMap::new(),
            action: Action::default(),
            stages: Vec::new(),
            idle_elapsed: 0.0,
            grenade: None,
            switch: None,
            frozen_dip: 0.0,
            casing_pending: false,
        }
    }

    /// A weapon pickup carries exactly ammo_amount rounds, not an additional
    /// magazine plus ammo_amount. Earlier ammo pickups remain in the pool.
    pub fn acquire(&mut self, definitions: &[Definition], id: &str) -> bool {
        let Some(slot) = definitions.iter().position(|d| d.id.eq_ignore_ascii_case(id)) else {
            return false;
        };
        let definition = &definitions[slot];
        if !definition.player_selectable { return false; }
        let Some(weapon) = self.weapons.get_mut(slot) else { return false; };
        let first = !weapon.owned;
        weapon.owned = true;
        let loaded = if first && !definition.melee {
            definition.ammo_amount.min(definition.capacity)
        } else { 0 };
        weapon.magazine = weapon.magazine.saturating_add(loaded);
        self.add_ammo(definition.ammo_index, definition.ammo_amount - loaded);
        // Retail never draws a picked-up weapon: the only caller of the select function is the weapon-key loop (cshell 0x10060c8f).
        true
    }

    pub fn add_ammo(&mut self, index: i32, amount: u32) {
        if index < 0 || amount == 0 { return; }
        let reserve = self.ammo.entry(index).or_default();
        *reserve = reserve.saturating_add(amount);
    }

    pub fn reserve(&self, definitions: &[Definition], slot: usize) -> u32 {
        definitions.get(slot)
            .and_then(|d| self.ammo.get(&d.ammo_index)).copied().unwrap_or(0)
    }

    /// Key 1..8 / wheel (0x100034a0): refused while reloading or changing weapon; with a weapon in hand the change starts by
    /// lowering it, with empty hands the new one is drawn at once.
    pub fn select(&mut self, slot: usize) -> bool {
        if self.reloading() || self.switch.is_some() || !self.weapons.get(slot).is_some_and(|w| w.owned && w.selectable) { return false; }
        let holding = self.equipped && self.selected.is_some_and(|s| self.weapons.get(s).is_some_and(|w| w.owned));
        if holding && self.selected == Some(slot) { return true; }
        self.cancel_action();
        self.frozen_dip = 0.0;
        if holding {
            self.switch = Some(Switch::Lower { target: slot, progress: 0.0 });
        } else {
            self.selected = Some(slot);
            self.equipped = true;
            self.switch = Some(Switch::Raise { progress: SWITCH_DEPTH });
        }
        true
    }

    /// True while the weapon is lowered or raised.
    pub fn switching(&self) -> bool { self.switch.is_some() }

    /// How far the view model is lowered (0..10 units; cshell 0x100106ca): it drops that many units and pitches 0.05 rad per unit.
    pub fn dip(&self) -> f32 {
        match self.switch { Some(Switch::Lower { progress, .. }) | Some(Switch::Raise { progress }) => progress, None => self.frozen_dip }
    }

    /// One frame of state 9 / state 10 (0x10010120, 0x10010158): the swap happens when the counter passes 10.
    fn advance_switch(&mut self, dt: f32) {
        match self.switch {
            Some(Switch::Lower { target, progress }) => {
                let progress = progress + dt * SWITCH_SPEED;
                if progress > SWITCH_DEPTH {
                    self.cancel_action();
                    self.selected = Some(target);
                    self.equipped = true;
                    self.switch = Some(Switch::Raise { progress });
                } else {
                    self.switch = Some(Switch::Lower { target, progress });
                }
            }
            Some(Switch::Raise { progress }) => {
                let progress = progress - dt * SWITCH_SPEED;
                self.switch = if progress <= 0.0 { None } else { Some(Switch::Raise { progress }) };
            }
            None => {}
        }
    }

    pub fn holster(&mut self) {
        if self.reloading() { return; }
        self.equipped = !self.equipped && self.selected
            .and_then(|slot| self.weapons.get(slot)).is_some_and(|w| w.owned);
        self.cancel_action();
        // The remake's H key: drawing the weapon again raises it like a fresh selection.
        if self.equipped { self.switch = Some(Switch::Raise { progress: SWITCH_DEPTH }); }
    }

    /// Leaving a level (cshell 0x1005adf0 -> 0x10003660 -> 0x10003540(0)) deselects the weapon: the next level starts with empty hands.
    pub fn empty_hands(&mut self) {
        self.selected = None;
        self.put_away();
    }

    pub fn put_away(&mut self) {
        self.equipped = false;
        self.cancel_action();
        self.frozen_dip = 0.0;
    }

    pub fn reloading(&self) -> bool { self.action.kind == ActionKind::Reload }

    pub fn armed_grenade(&self) -> bool {
        self.grenade.as_ref().is_some_and(|g| g.phase != GrenadePhase::Recover)
    }

    pub fn cancel_action(&mut self) {
        self.action = Action::default();
        self.stages.clear();
        self.idle_elapsed = 0.0;
        self.grenade = None;
        self.switch = None;
        self.casing_pending = false;
    }

    fn start_action(&mut self, definition: &Definition, kind: ActionKind, shells: u32) {
        let clips = if kind == ActionKind::Reload {
            &definition.animations.reload
        } else { &definition.animations.shoot };
        let shell_reload = kind == ActionKind::Reload && definition.shotgun && clips.len() >= 3;
        self.stages.clear();
        for (clip, name) in clips.iter().enumerate() {
            let duration = definition.durations.get(name).copied()
                .filter(|v| v.is_finite() && *v > 0.0).unwrap_or(0.001);
            let repeats = if shell_reload && clip == 1 { shells } else { 1 };
            for repeat in 0..repeats {
                // Retail inserts the first shell after opening, then starts the
                // middle clip. Even the final inserted shell gets this clip.
                let insert_shell = shell_reload && (clip == 0 || (clip == 1 && repeat + 1 < shells));
                self.stages.push(Stage { clip, duration, insert_shell });
            }
        }
        self.action = Action { kind, elapsed: 0.0, duration: self.stages.iter().map(|s| s.duration).sum() };
        self.idle_elapsed = 0.0;
    }

    fn transfer_ammo(&mut self, definition: &Definition, slot: usize, limit: u32) {
        let reserve = self.ammo.entry(definition.ammo_index).or_default();
        let amount = definition.capacity.saturating_sub(self.weapons[slot].magazine)
            .min(*reserve).min(limit);
        self.weapons[slot].magazine += amount;
        *reserve -= amount;
    }

    fn grenade_tick(&mut self, definition: &Definition, slot: usize, dt: f32,
        held_fire: bool) -> TickOutcome {
        let mut outcome = TickOutcome::default();
        let Some(mut grenade) = self.grenade.take() else { return outcome; };
        let clip_duration = |name: &str| definition.durations.get(name).copied().unwrap_or(0.0).max(0.0);
        let pin_duration = clip_duration(&definition.animations.hold);
        let throw_duration = definition.animations.shoot.first().map(|s| clip_duration(s)).unwrap_or(0.0);
        let return_duration = definition.animations.shoot.get(1).map(|s| clip_duration(s)).unwrap_or(0.0);
        let mut remaining = dt;
        // At most pin -> hold -> throw -> recovery -> idle can occur in one tick.
        for _ in 0..5 {
            if grenade.phase == GrenadePhase::Hold && !held_fire {
                grenade.phase = GrenadePhase::Throw;
                grenade.elapsed = 0.0;
            }
            let duration = match grenade.phase {
                GrenadePhase::Pin => pin_duration,
                GrenadePhase::Hold => f32::INFINITY,
                GrenadePhase::Throw => throw_duration,
                GrenadePhase::Recover => return_duration,
            };
            let step = remaining.min((duration - grenade.elapsed).max(0.0));
            if matches!(grenade.phase, GrenadePhase::Hold | GrenadePhase::Throw) {
                if grenade.fuse <= step {
                    outcome.shot = Some(slot);
                    outcome.grenade_in_hand = true;
                    outcome.grenade_fuse = Some(0.0);
                    self.cancel_action();
                    self.transfer_ammo(definition, slot, 1);
                    return outcome;
                }
                grenade.fuse -= step;
            }
            grenade.elapsed += step;
            remaining -= step;
            if grenade.elapsed < duration { break; }
            match grenade.phase {
                GrenadePhase::Pin => {
                    grenade.phase = GrenadePhase::Hold;
                    grenade.elapsed = 0.0;
                }
                GrenadePhase::Hold => break,
                GrenadePhase::Throw => {
                    outcome.shot = Some(slot);
                    // Original projectile construction overwrites the cooked
                    // fuse with a fresh three seconds. Preserve that quirk.
                    outcome.grenade_fuse = Some(3.0);
                    grenade.phase = GrenadePhase::Recover;
                    grenade.elapsed = 0.0;
                }
                GrenadePhase::Recover => {
                    self.cancel_action();
                    self.transfer_ammo(definition, slot, 1);
                    // Nothing left to ready: the grenade item is removed and the hands are empty.
                    if self.weapons[slot].magazine == 0 {
                        self.weapons[slot].owned = false;
                        self.selected = None;
                        self.equipped = false;
                        outcome.grenade_spent = Some(slot);
                    }
                    return outcome;
                }
            }
            if remaining <= 0.0 { break; }
        }
        self.action.elapsed = grenade.elapsed;
        self.action.duration = match grenade.phase {
            GrenadePhase::Pin | GrenadePhase::Hold => pin_duration,
            GrenadePhase::Throw => throw_duration,
            GrenadePhase::Recover => return_duration,
        };
        self.grenade = Some(grenade);
        outcome
    }

    pub fn tick(&mut self, definitions: &[Definition], dt: f32, held_fire: bool,
        fresh_fire: bool, reload: bool) -> TickOutcome {
        let mut outcome = TickOutcome::default();
        if !dt.is_finite() || dt <= 0.0 { return outcome; }
        for weapon in &mut self.weapons { weapon.cooldown = (weapon.cooldown - dt).max(0.0); }
        self.idle_elapsed += dt;
        if self.switch.is_some() {
            // 0x10003d40 checks no weapon state: the reload key during a change starts the reload of the weapon in hand (the old one while lowering, the
            // new one while rising) and overwrites state 9 / 10, so the change never finishes and the dip counter (+0x6f0) keeps its value.
            if reload && self.equipped {
                if let Some(slot) = self.selected.filter(|&s| self.weapons.get(s).is_some_and(|w| w.owned)) {
                    let dip = self.dip();
                    if self.start_reload(definitions, slot) {
                        self.switch = None; self.frozen_dip = dip;
                        outcome.reload_started = true;
                        return outcome;
                    }
                }
            }
            self.advance_switch(dt); return outcome;
        }
        if !self.equipped { return outcome; }
        let Some(slot) = self.selected.filter(|&s| self.weapons.get(s).is_some_and(|w| w.owned)) else {
            return outcome;
        };
        let Some(definition) = definitions.get(slot) else { return outcome; };

        if self.grenade.is_some() { return self.grenade_tick(definition, slot, dt, held_fire); }

        if self.action.kind != ActionKind::Idle {
            let previous = self.action.elapsed;
            self.action.elapsed += dt;
            let mut end = 0.0;
            let mut inserted = 0;
            let mut interrupt_at = None;
            for stage in &self.stages {
                end += stage.duration;
                if definition.shotgun && self.reloading() && held_fire && stage.clip < 2
                    && previous < end && self.action.elapsed >= end {
                    interrupt_at = Some(end);
                    break;
                }
                if stage.insert_shell && previous < end && self.action.elapsed >= end { inserted += 1; }
                if self.action.kind == ActionKind::Shoot && self.casing_pending && stage.clip == 0 && previous < end && self.action.elapsed >= end {
                    outcome.eject_casing = true;
                    self.casing_pending = false;
                }
                if self.reloading() && !definition.shotgun && stage.clip == 0 && self.stages.len() > 1 && previous < end && self.action.elapsed >= end { outcome.eject_casings = true; }
            }
            if inserted > 0 { self.transfer_ammo(definition, slot, inserted); outcome.shells_inserted = inserted; }
            if let Some(end) = interrupt_at {
                if let Some(closing) = self.stages.last().cloned() {
                    self.action.elapsed -= end;
                    self.action.duration = closing.duration;
                    self.stages = vec![closing];
                }
            }
            if self.action.elapsed >= self.action.duration {
                // The rounds of a magazine reload moved when it started (0x10003da3); only a shotgun feeds shells during it.
                if self.reloading() { outcome.reload_completed = true; }
                self.cancel_action();
            }
        }
        // Grenades are individual items: the next one is readied after the
        // throw, without inventing a magazine reload animation.
        if definition.grenade && self.action.kind == ActionKind::Idle && self.weapons[slot].magazine == 0 {
            self.transfer_ammo(definition, slot, 1);
        }
        // The reload key is level triggered and polled after the trigger (0x10060c9a, 0x10060d08: `fire` then `reload` every frame): held, it starts the
        // reload right after a shot and restarts a shotgun's shell cycle every frame (the gun cannot finish while R is down); a full magazine refuses it.
        if self.reloading() {
            if reload && self.start_reload(definitions, slot) { outcome.reload_started = true; }
            return outcome;
        }
        let trigger = fresh_fire || (held_fire && (definition.automatic || definition.melee));
        // Firing an empty magazine only restarts the shot timer and reloads if it can (0x1000403a..0x10004056).
        if trigger && self.weapons[slot].cooldown <= 0.0 && !definition.melee && !definition.grenade && self.weapons[slot].magazine == 0 {
            self.weapons[slot].cooldown = definition.shot_latency.max(0.001);
            if self.start_reload(definitions, slot) { outcome.reload_started = true; }
            return outcome;
        }
        if trigger && self.weapons[slot].cooldown <= 0.0
            && (definition.melee || self.weapons[slot].magazine > 0) {
            if !definition.melee { self.weapons[slot].magazine -= 1; }
            self.weapons[slot].cooldown = definition.shot_latency.max(0.001);
            if definition.grenade {
                self.action = Action { kind: ActionKind::Shoot, elapsed: 0.0,
                    duration: definition.durations.get(&definition.animations.hold).copied().unwrap_or(0.0) };
                self.grenade = Some(GrenadeAction { phase: GrenadePhase::Pin,
                    elapsed: 0.0, fuse: definition.fuse_seconds });
                outcome.grenade_primed = true;
                return outcome;
            }
            // A shot that interrupts the first shoot clip ejects the casing of the previous one first.
            if self.action.kind == ActionKind::Shoot && self.casing_pending { outcome.eject_casing = true; }
            self.start_action(definition, ActionKind::Shoot, 0);
            self.casing_pending = !definition.melee;
            outcome.shot = Some(slot);
        }
        if reload && self.start_reload(definitions, slot) { outcome.reload_started = true; }
        outcome
    }

    /// Reload key or an empty trigger (0x10003d40): needs reserve rounds and a magazine that is not full. A magazine weapon moves
    /// min(missing, reserve) rounds at once and runs the animation afterwards; a shotgun (`shotgun` item flag) feeds shells later.
    fn start_reload(&mut self, definitions: &[Definition], slot: usize) -> bool {
        let definition = &definitions[slot];
        let missing = definition.capacity.saturating_sub(self.weapons[slot].magazine);
        let reserve = self.reserve(definitions, slot);
        if definition.melee || definition.grenade || missing == 0 || reserve == 0 || definition.animations.reload.is_empty() { return false; }
        if !definition.shotgun { self.transfer_ammo(definition, slot, u32::MAX); }
        // A reload before the case left loses it (0x10004029 is the only place that ejects it).
        self.casing_pending = false;
        self.start_action(definition, ActionKind::Reload, missing.min(reserve));
        true
    }

    pub fn pose<'a>(&'a self, definitions: &'a [Definition]) -> Option<(&'a str, f32, bool)> {
        if !self.equipped { return None; }
        let slot = self.selected?;
        if !self.weapons.get(slot)?.owned { return None; }
        let definition = definitions.get(slot)?;
        if let Some(grenade) = &self.grenade {
            let clip = match grenade.phase {
                GrenadePhase::Pin | GrenadePhase::Hold => &definition.animations.hold,
                GrenadePhase::Throw => definition.animations.shoot.first()?,
                GrenadePhase::Recover => definition.animations.shoot.get(1)?,
            };
            let time = if grenade.phase == GrenadePhase::Hold {
                definition.durations.get(clip).copied().unwrap_or(0.0)
            } else { grenade.elapsed };
            return Some((clip, time, false));
        }
        if self.action.kind != ActionKind::Idle {
            let clips = if self.reloading() { &definition.animations.reload } else { &definition.animations.shoot };
            let mut time = self.action.elapsed;
            for stage in &self.stages {
                if time < stage.duration {
                    return clips.get(stage.clip).map(|name| (name.as_str(), time, false));
                }
                time -= stage.duration;
            }
        }
        Some((&definition.animations.base, self.idle_elapsed, true))
    }

    pub fn snapshot(&self, definitions: &[Definition]) -> Snapshot {
        Snapshot {
            weapons: definitions.iter().zip(&self.weapons).map(|(definition, weapon)| SavedWeapon {
                id: definition.id.clone(), owned: weapon.owned, magazine: weapon.magazine,
            }).collect(),
            ammo: self.ammo.clone(),
            selected: self.selected.and_then(|slot| definitions.get(slot)).map(|d| d.id.clone()),
            equipped: self.equipped,
        }
    }

    pub fn restore(&mut self, definitions: &[Definition], snapshot: &Snapshot) {
        *self = Self::new(definitions);
        self.ammo = snapshot.ammo.iter().filter(|(index, _)| **index >= 0)
            .map(|(&index, &amount)| (index, amount)).collect();
        let mut restored = BTreeSet::new();
        for saved in &snapshot.weapons {
            let Some(slot) = definitions.iter().position(|d| d.id == saved.id) else { continue; };
            if !restored.insert(slot) { continue; }
            self.weapons[slot].owned = saved.owned && definitions[slot].player_selectable;
            self.weapons[slot].magazine = if self.weapons[slot].owned {
                saved.magazine.min(definitions[slot].capacity)
            } else { 0 };
        }
        self.selected = snapshot.selected.as_ref().and_then(|id| definitions.iter().position(|d| &d.id == id))
            .filter(|&slot| self.weapons[slot].owned);
        self.equipped = snapshot.equipped && self.selected.is_some();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Inventory {
        /// Test stand-in for "pick the weapon up, then press its number key".
        fn take_and_draw(&mut self, definitions: &[Definition], id: &str) -> bool {
            let taken = self.acquire(definitions, id);
            if let Some(slot) = definitions.iter().position(|d| d.id.eq_ignore_ascii_case(id)) { self.select(slot); }
            taken
        }
    }

    fn definitions() -> Vec<Definition> {
        serde_json::from_value(serde_json::json!([
            {"id":"baton","title":"Baton","model":"baton","skins":{},"hud":"baton.png","offset":[0,0,0],"scale":[1,1,1],"melee":true,"automatic":false,"ammo_index":-1,"ammo_amount":0,"capacity":0,"damage":40,"shot_latency":0.3,"spread":0,"animations":{"base":"idle","shoot":["strike","return"],"reload":[]},"durations":{"idle":1,"strike":0.2,"return":0.3},"sounds":{}},
            {"id":"glock","title":"Glock","model":"glock","skins":{},"hud":"glock.png","offset":[0,0,0],"scale":[1,1,1],"melee":false,"automatic":false,"ammo_index":0,"ammo_amount":3,"capacity":3,"damage":30,"shot_latency":0.3,"spread":64,"animations":{"base":"idle","shoot":["fire","recover"],"reload":["open","load"]},"durations":{"idle":1,"fire":0.1,"recover":0.2,"open":0.4,"load":0.6},"sounds":{}},
            {"id":"sig","title":"SIG","model":"sig","skins":{},"hud":"sig.png","offset":[0,0,0],"scale":[1,1,1],"melee":false,"automatic":true,"ammo_index":2,"ammo_amount":3,"capacity":3,"damage":25,"shot_latency":0.05,"spread":128,"animations":{"base":"idle","shoot":["fire"],"reload":["reload"]},"durations":{"idle":1,"fire":0.1,"reload":0.8},"sounds":{}},
            {"id":"grenade","title":"Grenade","model":"grenade","skins":{},"hud":"grenade.png","offset":[0,0,0],"scale":[1,1,1],"melee":false,"automatic":false,"grenade":true,"ammo_index":8,"ammo_amount":1,"capacity":1,"damage":0,"shot_latency":0.6,"spread":0,"animations":{"base":"idle","shoot":["throw","return"],"reload":[],"hold":"pin"},"durations":{"idle":1,"throw":0.3,"return":0.3,"pin":0.7},"sounds":{}},
            {"id":"shotgun","title":"Shotgun","model":"shotgun","skins":{},"hud":"shotgun.png","offset":[0,0,0],"scale":[1,1,1],"melee":false,"automatic":false,"shotgun":true,"ammo_index":5,"ammo_amount":3,"capacity":3,"damage":30,"shot_latency":1,"spread":192,"animations":{"base":"idle","shoot":["fire"],"reload":["open","shell","close"]},"durations":{"idle":1,"fire":0.2,"open":0.2,"shell":0.3,"close":0.2},"sounds":{}}
        ])).unwrap()
    }

    /// Runs a pending weapon change (lowering, then raising) to its end.
    fn settle(inventory: &mut Inventory, defs: &[Definition]) {
        while inventory.switching() { inventory.tick(defs, 1.0, false, false, false); }
    }

    fn fire(inventory: &mut Inventory, defs: &[Definition]) -> TickOutcome {
        settle(inventory, defs);
        inventory.tick(defs, 1.0, true, true, false)
    }

    #[test]
    fn ammunition_before_weapon_is_retained_and_duplicate_adds_one_grant() {
        let defs = definitions();
        let mut inventory = Inventory::new(&defs);
        inventory.add_ammo(0, 7);
        assert!(inventory.take_and_draw(&defs, "glock"));
        assert_eq!(inventory.weapons[1].magazine, 3);
        assert_eq!(inventory.reserve(&defs, 1), 7);
        assert!(inventory.equipped);
        assert_eq!(inventory.selected, Some(1));
        assert!(inventory.take_and_draw(&defs, "glock"));
        assert_eq!(inventory.reserve(&defs, 1), 10);
        assert_eq!(inventory.weapons.iter().filter(|w| w.owned).count(), 1);
        assert!(!inventory.take_and_draw(&defs, "unknown"));
    }

    #[test]
    fn a_picked_up_weapon_is_never_drawn_until_its_key_is_pressed() {
        let defs = definitions();
        let mut inventory = Inventory::new(&defs);
        assert!(inventory.acquire(&defs, "baton"));
        assert!(inventory.acquire(&defs, "glock"));
        assert_eq!((inventory.selected, inventory.equipped, inventory.switching()), (None, false, false), "cshell 0x100034a0 is only called by the key loop 0x10060c8f");
        assert!(inventory.weapons[1].owned && inventory.weapons[1].magazine == 3);
        assert!(inventory.select(1));
        assert_eq!((inventory.selected, inventory.equipped), (Some(1), true));
        assert!(inventory.acquire(&defs, "sig"));
        assert_eq!(inventory.selected, Some(1), "a second pickup leaves the weapon in hand alone");
    }

    #[test]
    fn a_magazine_reload_moves_the_rounds_when_it_starts_and_then_plays_its_stages() {
        let defs = definitions();
        let mut inventory = Inventory::new(&defs);
        inventory.take_and_draw(&defs, "glock");
        inventory.add_ammo(0, 5);
        fire(&mut inventory, &defs);
        assert_eq!(inventory.weapons[1].magazine, 2);
        let outcome = inventory.tick(&defs, 0.01, false, false, true);
        assert!(outcome.reload_started);
        assert_eq!(inventory.pose(&defs), Some(("open", 0.0, false)));
        assert_eq!(inventory.weapons[1].magazine, 3, "cshell 0x10003da3 refills at once");
        assert_eq!(inventory.reserve(&defs, 1), 4);
        inventory.tick(&defs, 0.5, true, true, false);
        assert_eq!(inventory.pose(&defs).unwrap().0, "load");
        assert_eq!(inventory.tick(&defs, 0.3, true, true, false).shot, None, "no shot while reloading");
        let finished = inventory.tick(&defs, 0.51, false, false, false);
        assert!(finished.reload_completed);
        assert_eq!((inventory.weapons[1].magazine, inventory.reserve(&defs, 1)), (3, 4));
    }

    #[test]
    fn the_casing_leaves_when_the_first_shoot_clip_ends_or_the_next_shot_interrupts_it() {
        let defs = definitions();
        let mut inventory = Inventory::new(&defs);
        inventory.take_and_draw(&defs, "sig");
        settle(&mut inventory, &defs);
        let first = inventory.tick(&defs, 0.1, true, true, false);
        assert!(first.shot.is_some() && !first.eject_casing, "nothing leaves at the trigger pull");
        let mut single = Inventory::new(&defs);
        single.take_and_draw(&defs, "sig");
        settle(&mut single, &defs);
        assert!(single.tick(&defs, 0.1, true, true, false).shot.is_some());
        assert!(!single.tick(&defs, 0.05, false, false, false).eject_casing, "the clip `fire` lasts 0.1 s");
        assert!(single.tick(&defs, 0.06, false, false, false).eject_casing, "it ends: the case leaves");
        assert!(!single.tick(&defs, 1.0, false, false, false).eject_casing, "once per shot");
        // A second shot 0.05 s after the first interrupts the running clip and ejects the first case.
        let mut burst = Inventory::new(&defs);
        burst.take_and_draw(&defs, "sig");
        settle(&mut burst, &defs);
        assert!(burst.tick(&defs, 0.1, true, true, false).shot.is_some());
        let second = burst.tick(&defs, 0.06, true, false, false);
        assert!(second.shot.is_some() && second.eject_casing, "held fire: the next shot ejects the previous case");
        // Switching away with a case still pending loses it, and the nightstick never has one.
        let mut lost = Inventory::new(&defs);
        lost.take_and_draw(&defs, "baton");
        settle(&mut lost, &defs);
        lost.take_and_draw(&defs, "sig");
        settle(&mut lost, &defs);
        assert!(lost.tick(&defs, 0.1, true, true, false).shot.is_some());
        assert!(lost.select(0));
        assert!(!lost.tick(&defs, 2.0, false, false, false).eject_casing);
        let mut stick = Inventory::new(&defs);
        stick.take_and_draw(&defs, "baton");
        settle(&mut stick, &defs);
        assert!(stick.tick(&defs, 0.1, true, true, false).shot.is_some());
        assert!(!stick.tick(&defs, 1.0, false, false, false).eject_casing);
    }

    #[test]
    fn a_two_clip_magazine_reload_ejects_the_casings_when_its_first_clip_ends() {
        let defs = definitions();
        let mut inventory = Inventory::new(&defs);
        inventory.take_and_draw(&defs, "glock");
        inventory.add_ammo(0, 5);
        fire(&mut inventory, &defs);
        inventory.tick(&defs, 0.01, false, false, true);
        assert!(!inventory.tick(&defs, 0.3, false, false, false).eject_casings, "the cylinder is still opening");
        assert!(inventory.tick(&defs, 0.2, false, false, false).eject_casings, "clip `open` (0.4 s) has ended");
        assert!(!inventory.tick(&defs, 0.6, false, false, false).eject_casings, "only once per reload");
        let mut single = Inventory::new(&defs);
        single.take_and_draw(&defs, "sig");
        fire(&mut single, &defs);single.add_ammo(2, 5);
        single.tick(&defs, 0.01, false, false, true);
        assert!(!single.tick(&defs, 0.9, false, false, false).eject_casings, "a one-clip reload ejects nothing");
    }

    #[test]
    fn empty_full_and_blocked_reload_do_not_create_ammo() {
        let defs = definitions();
        let mut inventory = Inventory::new(&defs);
        inventory.take_and_draw(&defs, "glock");
        inventory.add_ammo(0, 1);
        settle(&mut inventory, &defs);
        assert!(!inventory.tick(&defs, 0.01, false, false, true).reload_started, "a full magazine does not reload");
        for _ in 0..3 { assert_eq!(fire(&mut inventory, &defs).shot, Some(1)); }
        // Pulling the trigger on an empty magazine reloads by itself when there are rounds left (0x10004056).
        let empty = fire(&mut inventory, &defs);
        assert_eq!(empty.shot, None);
        assert!(empty.reload_started);
        assert_eq!((inventory.weapons[1].magazine, inventory.reserve(&defs, 1)), (1, 0));
        assert_eq!(inventory.tick(&defs, 0.5, true, true, true).shot, None);
        inventory.tick(&defs, 0.51, false, false, false);
        assert_eq!((inventory.weapons[1].magazine, inventory.reserve(&defs, 1)), (1, 0));
        assert_eq!(fire(&mut inventory, &defs).shot, Some(1));
        assert!(!inventory.tick(&defs, 0.01, false, false, true).reload_started, "no reserve, no reload");
        let dry = fire(&mut inventory, &defs);
        assert!(dry.shot.is_none() && !dry.reload_started, "an empty gun without reserve only clicks");
    }

    #[test]
    fn a_reload_during_a_weapon_change_keeps_the_old_weapon_and_leaves_the_view_model_lowered() {
        // 0x10003d40 checks no weapon state and sets state 1: the change (9 / 10) never finishes, +0x6f0 keeps its value (used by 0x100106ca in every state).
        let defs = definitions();
        let mut inventory = Inventory::new(&defs);
        inventory.take_and_draw(&defs, "baton");
        settle(&mut inventory, &defs);
        inventory.take_and_draw(&defs, "glock");
        settle(&mut inventory, &defs);
        inventory.add_ammo(0, 4);
        fire(&mut inventory, &defs);
        assert!(inventory.select(0));
        inventory.tick(&defs, 0.1, false, false, false);
        let dip = inventory.dip();
        assert!(dip > 2.9 && dip < 3.1, "{dip}");
        let outcome = inventory.tick(&defs, 0.01, false, false, true);
        assert!(outcome.reload_started && inventory.reloading() && !inventory.switching());
        assert_eq!(inventory.selected, Some(1), "the swap to the baton never happens");
        assert!((inventory.dip() - dip).abs() < 1e-4, "the dip is frozen");
        inventory.tick(&defs, 3.0, false, false, false);
        assert!(!inventory.reloading() && (inventory.dip() - dip).abs() < 1e-4, "still lowered after the reload");
        assert!(inventory.select(0));
        assert_eq!(inventory.dip(), 0.0, "the next change starts from zero again");
    }

    #[test]
    fn the_held_reload_key_runs_after_the_trigger_in_the_same_frame() {
        // 0x10060c9a (fire) precedes 0x10060d0f (reload): a shot and the reload of the same frame both happen.
        let defs = definitions();
        let mut inventory = Inventory::new(&defs);
        inventory.take_and_draw(&defs, "glock");
        settle(&mut inventory, &defs);
        inventory.add_ammo(0, 4);
        let outcome = inventory.tick(&defs, 1.0, true, true, true);
        assert_eq!(outcome.shot, Some(1));
        assert!(outcome.reload_started && inventory.reloading(), "the magazine is not full after the shot");
        assert_eq!(inventory.weapons[1].magazine, 3, "min(missing, reserve) moved at the start");
    }

    #[test]
    fn a_held_reload_key_restarts_a_shotgun_reload_every_frame_but_not_a_magazine_one() {
        let defs = definitions();
        let mut inventory = Inventory::new(&defs);
        inventory.take_and_draw(&defs, "shotgun");
        settle(&mut inventory, &defs);
        inventory.add_ammo(5, 4);
        fire(&mut inventory, &defs);
        assert!(inventory.tick(&defs, 1.0, false, false, true).reload_started);
        let (first, second) = (inventory.tick(&defs, 0.1, false, false, true), inventory.tick(&defs, 0.1, false, false, true));
        assert!(first.reload_started && second.reload_started && inventory.action.elapsed < 0.15, "the shell cycle never gets past its first clip while R is down");
        let mut inventory = Inventory::new(&defs);
        inventory.take_and_draw(&defs, "glock");
        settle(&mut inventory, &defs);
        inventory.add_ammo(0, 4);
        fire(&mut inventory, &defs);
        assert!(inventory.tick(&defs, 0.01, false, false, true).reload_started);
        assert!(!inventory.tick(&defs, 0.1, false, false, true).reload_started, "the magazine is full since the start: nothing to restart");
    }

    #[test]
    fn switch_cycle_and_holster_are_blocked_during_source_reload_states() {
        let defs = definitions();
        let mut inventory = Inventory::new(&defs);
        inventory.take_and_draw(&defs, "baton");
        settle(&mut inventory, &defs);
        inventory.take_and_draw(&defs, "glock");
        settle(&mut inventory, &defs);
        inventory.add_ammo(0, 4);
        fire(&mut inventory, &defs);
        inventory.tick(&defs, 0.01, false, false, true);
        inventory.tick(&defs, 0.5, false, false, false);
        assert!(!inventory.select(0));
        inventory.holster();
        assert!(inventory.equipped);
        assert!(inventory.reloading());
        assert_eq!(inventory.selected, Some(1));
        assert_eq!(inventory.weapons[1].magazine, 3, "the rounds moved when the reload began");
        assert_eq!(inventory.reserve(&defs, 1), 3);
        inventory.tick(&defs, 0.51, false, false, false);
        assert_eq!(inventory.weapons[1].magazine, 3);
        assert_eq!(inventory.reserve(&defs, 1), 3);
        assert!(inventory.select(0));
        assert_eq!(inventory.selected, Some(1), "the old weapon is still in hand while it lowers");
        inventory.holster();
        assert!(!inventory.equipped && !inventory.switching());
        inventory.holster();
        assert!(inventory.equipped && inventory.switching(), "drawing it again raises it");
    }

    #[test]
    fn a_weapon_change_lowers_for_a_third_of_a_second_then_raises_and_blocks_shooting() {
        let defs = definitions();
        let mut inventory = Inventory::new(&defs);
        inventory.take_and_draw(&defs, "baton");
        inventory.take_and_draw(&defs, "glock");
        assert!(inventory.switching(), "the first weapon is drawn at once, rising from the depth of 10");
        assert_eq!((inventory.selected, inventory.dip()), (Some(0), 10.0));
        settle(&mut inventory, &defs);
        assert_eq!(inventory.dip(), 0.0);
        assert!(inventory.select(1));
        assert_eq!((inventory.selected, inventory.dip()), (Some(0), 0.0));
        assert!(!inventory.select(0), "a change in progress refuses another");
        inventory.tick(&defs, 0.1, true, true, true);
        assert!((inventory.dip() - 3.0).abs() < 1e-4);
        assert_eq!(inventory.tick(&defs, 0.2, true, true, true).shot, None, "no shot while lowering");
        assert_eq!(inventory.selected, Some(0), "still lowering at 9");
        assert!((inventory.dip() - 9.0).abs() < 1e-4);
        inventory.tick(&defs, 0.05, false, false, false);
        assert_eq!(inventory.selected, Some(1), "swapped once the counter passed 10");
        assert!((inventory.dip() - 10.5).abs() < 1e-4);
        assert_eq!(inventory.tick(&defs, 0.1, true, true, false).shot, None, "no shot while raising");
        assert!((inventory.dip() - 7.5).abs() < 1e-4);
        inventory.tick(&defs, 0.3, false, false, false);
        assert!(!inventory.switching() && inventory.dip() == 0.0);
        assert!(fire(&mut inventory, &defs).shot.is_some());
    }

    #[test]
    fn acquiring_during_reload_grants_once_without_interrupting_current_weapon() {
        let defs = definitions();
        let mut inventory = Inventory::new(&defs);
        inventory.take_and_draw(&defs, "glock");
        inventory.add_ammo(0, 3);
        fire(&mut inventory, &defs);
        inventory.tick(&defs, 0.01, false, false, true);
        assert!(inventory.take_and_draw(&defs, "sig"));
        assert_eq!(inventory.selected, Some(1));
        assert_eq!(inventory.weapons[2].magazine, 3);
        assert!(inventory.reloading());
    }

    #[test]
    fn npc_only_weapons_cannot_be_acquired_selected_or_restored_as_owned() {
        let mut defs = definitions();
        defs[2].player_selectable = false;
        let mut inventory = Inventory::new(&defs);
        assert!(!inventory.take_and_draw(&defs, "sig"));
        assert_eq!(inventory.reserve(&defs, 2), 0);
        inventory.take_and_draw(&defs, "glock");
        inventory.weapons[2].owned = true;
        assert!(!inventory.select(2));
        assert_eq!(inventory.selected, Some(1));
        let snapshot = inventory.snapshot(&defs);
        inventory.restore(&defs, &snapshot);
        assert!(!inventory.weapons[2].owned);
    }

    #[test]
    fn held_trigger_repeats_only_automatic_weapons() {
        let defs = definitions();
        let mut inventory = Inventory::new(&defs);
        inventory.take_and_draw(&defs, "glock");
        assert_eq!(fire(&mut inventory, &defs).shot, Some(1));
        assert_eq!(inventory.tick(&defs, 1.0, true, false, false).shot, None);
        inventory.take_and_draw(&defs, "sig");
        assert_eq!(fire(&mut inventory, &defs).shot, Some(2));
        assert_eq!(inventory.tick(&defs, 0.06, true, false, false).shot, Some(2));
        assert_eq!(inventory.tick(&defs, 0.001, true, false, false).shot, None);
    }

    #[test]
    fn held_trigger_repeats_nightstick_swings_every_shot_latency_without_ammo() {
        let defs = definitions();
        let mut inventory = Inventory::new(&defs);
        inventory.take_and_draw(&defs, "baton");
        assert_eq!(fire(&mut inventory, &defs).shot, Some(0));
        assert_eq!(inventory.tick(&defs, 0.1, true, false, false).shot, None);
        assert_eq!(inventory.tick(&defs, 0.25, true, false, false).shot, Some(0));
        assert_eq!(inventory.tick(&defs, 0.1, false, false, false).shot, None);
        assert_eq!(inventory.weapons[0].magazine, 0);
    }

    #[test]
    fn shotgun_inserts_each_shell_before_its_authored_middle_clip() {
        let defs = definitions();
        let mut inventory = Inventory::new(&defs);
        inventory.take_and_draw(&defs, "shotgun");
        inventory.add_ammo(5, 2);
        fire(&mut inventory, &defs);
        fire(&mut inventory, &defs);
        assert!(inventory.tick(&defs, 0.01, false, false, true).reload_started);
        inventory.tick(&defs, 0.21, false, false, false);
        assert_eq!(inventory.weapons[4].magazine, 2);
        assert_eq!(inventory.reserve(&defs, 4), 1);
        assert_eq!(inventory.pose(&defs).unwrap().0, "shell");
        inventory.holster();
        assert_eq!(inventory.weapons[4].magazine, 2);
        assert_eq!(inventory.reserve(&defs, 4), 1);
    }

    #[test]
    fn held_fire_interrupts_shotgun_reload_at_clip_boundary_without_extra_shell() {
        let defs = definitions();
        let mut inventory = Inventory::new(&defs);
        inventory.take_and_draw(&defs, "shotgun");
        inventory.add_ammo(5, 2);
        fire(&mut inventory, &defs);
        fire(&mut inventory, &defs);
        inventory.tick(&defs, 0.01, false, false, true);
        inventory.tick(&defs, 0.21, false, false, false);
        assert_eq!(inventory.weapons[4].magazine, 2);
        assert_eq!(inventory.tick(&defs, 0.31, true, true, false).shot, None);
        assert_eq!(inventory.pose(&defs).unwrap().0, "close");
        assert_eq!(inventory.weapons[4].magazine, 2);
        assert_eq!(inventory.reserve(&defs, 4), 1);
        assert!(inventory.tick(&defs, 0.21, false, false, false).reload_completed);
        assert_eq!(inventory.weapons[4].magazine, 2);
    }

    #[test]
    fn grenade_pin_hold_and_release_use_source_clips_before_emitting_projectile() {
        let defs = definitions();
        let mut inventory = Inventory::new(&defs);
        inventory.take_and_draw(&defs, "grenade");
        inventory.take_and_draw(&defs, "grenade");
        let prime = fire(&mut inventory, &defs);
        assert!(prime.grenade_primed);
        assert_eq!(prime.shot, None);
        assert_eq!(inventory.weapons[3].magazine, 0);
        assert_eq!(inventory.reserve(&defs, 3), 1);
        assert_eq!(inventory.pose(&defs).unwrap().0, "pin");
        inventory.tick(&defs, 0.7, true, false, false);
        inventory.tick(&defs, 1.0, true, false, false);
        assert_eq!(inventory.pose(&defs).unwrap().0, "pin");
        assert_eq!(inventory.tick(&defs, 0.1, false, false, true).shot, None);
        assert_eq!(inventory.pose(&defs).unwrap().0, "throw");
        let released = inventory.tick(&defs, 0.21, false, false, false);
        assert_eq!(released.shot, Some(3));
        assert_eq!(released.grenade_fuse, Some(3.0));
        assert!(!released.grenade_in_hand);
        assert_eq!(inventory.pose(&defs).unwrap().0, "return");
        assert_eq!(inventory.tick(&defs, 0.4, false, false, false).grenade_spent, None, "another grenade is readied");
        assert_eq!(inventory.weapons[3].magazine, 1);
        assert_eq!(inventory.reserve(&defs, 3), 0);
        // The next throw uses the last one: afterwards the item is gone and the hands are empty.
        fire(&mut inventory, &defs);
        inventory.tick(&defs, 0.7, true, false, false);
        inventory.tick(&defs, 0.1, false, false, false);
        inventory.tick(&defs, 0.4, false, false, false);
        let spent = inventory.tick(&defs, 0.4, false, false, false);
        assert_eq!(spent.grenade_spent, Some(3));
        assert!(!inventory.weapons[3].owned && !inventory.equipped && inventory.selected.is_none());
    }

    #[test]
    fn cooked_grenade_expires_in_hand_after_pin_clip_and_five_seconds() {
        let defs = definitions();
        let mut inventory = Inventory::new(&defs);
        inventory.take_and_draw(&defs, "grenade");
        fire(&mut inventory, &defs);
        assert_eq!(inventory.tick(&defs, 0.7, true, false, false).shot, None);
        assert_eq!(inventory.tick(&defs, 4.9, true, false, false).shot, None);
        let exploded = inventory.tick(&defs, 0.11, true, false, false);
        assert_eq!(exploded.shot, Some(3));
        assert!(exploded.grenade_in_hand);
        assert_eq!(exploded.grenade_fuse, Some(0.0));
        assert_eq!(inventory.weapons[3].magazine, 0);
        assert_eq!(inventory.tick(&defs, 1.0, true, false, false).shot, None);
    }

    #[test]
    fn source_allows_cancelling_an_armed_grenade_by_switching() {
        let defs = definitions();
        let mut inventory = Inventory::new(&defs);
        inventory.take_and_draw(&defs, "baton");
        settle(&mut inventory, &defs);
        inventory.take_and_draw(&defs, "grenade");
        fire(&mut inventory, &defs);
        assert!(inventory.armed_grenade());
        assert!(inventory.select(0));
        assert!(!inventory.armed_grenade());
        assert_eq!(inventory.weapons[3].magazine, 0);
        assert_eq!(inventory.tick(&defs, 10.0, false, false, false).shot, None);
    }

    #[test]
    fn exported_catalog_loads_and_all_player_weapons_equip_with_source_ammo() {
        let root = option_env!("CARGO_MANIFEST_DIR").map(std::path::PathBuf::from)
            .map(|p| p.join("../../output"))
            .unwrap_or_else(|| std::path::PathBuf::from("output"));
        let catalog: Catalog = serde_json::from_slice(&std::fs::read(root.join("retail_weapons.json")).unwrap()).unwrap();
        assert_eq!(catalog.schema_version, 1);
        let mut inventory = Inventory::new(&catalog.weapons);
        for (slot, definition) in catalog.weapons.iter().enumerate().filter(|(_, d)| d.player_selectable) {
            settle(&mut inventory, &catalog.weapons);
            assert!(inventory.take_and_draw(&catalog.weapons, &definition.id));
            settle(&mut inventory, &catalog.weapons);
            assert_eq!(inventory.selected, Some(slot));
            assert_eq!(inventory.weapons[slot].magazine, definition.ammo_amount.min(definition.capacity));
            assert!(definition.durations.contains_key(&definition.animations.base));
            assert!(root.join(format!("{}.json", definition.model)).is_file());
            for sound in definition.sounds.values().flatten() { assert!(root.join(sound).is_file()); }
            for skin in definition.skins.values() { assert!(root.join(skin).is_file()); }
            assert!(root.join(&definition.hud).is_file());
        }
        assert_eq!(catalog.weapons.iter().filter(|d| d.player_selectable).count(), 10);
    }

    #[test]
    fn snapshots_preserve_ammo_and_restore_by_id_validating_ownership() {
        let defs = definitions();
        let mut inventory = Inventory::new(&defs);
        inventory.take_and_draw(&defs, "glock");
        inventory.add_ammo(0, 9);
        fire(&mut inventory, &defs);
        let serialized = serde_json::to_string(&inventory.snapshot(&defs)).unwrap();
        let snapshot = serde_json::from_str(&serialized).unwrap();
        let mut reordered = defs.clone();
        reordered.swap(0, 1);
        let mut restored = Inventory::new(&reordered);
        restored.restore(&reordered, &snapshot);
        assert_eq!(restored.selected, Some(0));
        assert_eq!(restored.weapons[0].magazine, 2);
        assert_eq!(restored.reserve(&reordered, 0), 9);
        assert!(restored.equipped);
        assert!(!restored.select(1));
    }

    #[test]
    fn invalid_snapshot_cannot_equip_unowned_guns_or_overfill_magazines() {
        let defs = definitions();
        let mut inventory = Inventory::new(&defs);
        inventory.take_and_draw(&defs, "glock");
        let mut snapshot = inventory.snapshot(&defs);
        snapshot.weapons[1].magazine = 999;
        snapshot.weapons[2].magazine = 999;
        snapshot.selected = Some("sig".into());
        snapshot.ammo.insert(-1, 10);
        inventory.restore(&defs, &snapshot);
        assert_eq!(inventory.weapons[1].magazine, 3);
        assert_eq!(inventory.weapons[2].magazine, 0);
        assert_eq!(inventory.selected, None);
        assert!(!inventory.equipped);
        assert!(!inventory.ammo.contains_key(&-1));
    }
}
