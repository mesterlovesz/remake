//! The retail player controller (cshell.dll 0x10060160 input, 0x10060dd0 frame, 0x100618f0 move) in native units.
//! The controller integrates its own velocity (cshell never reads it back from the engine), the engine only
//! turns `velocity·dt` into a collision-limited displacement; this file mirrors that split.
use parry3d::math::Vec3;
use crate::{CollisionWorld, GROUND_NORMAL, STAIR_HEIGHT};

/// The swept box is inflated by this much, so contacts stop 0.05 away from a surface.
const SKIN: f32 = 0.05;
/// Extra separation along the contact normal after a hit; keeps sliding contacts clear of float noise.
const BACKOFF: f32 = 0.03;
/// How far below the hull a walkable surface still counts as standing on it.
const GROUND_PROBE: f32 = 0.3;
/// cshell 0x10061906: movement is frozen (velocity zeroed) while the level has run fewer than 60 frames.
pub const FREEZE_FRAMES: u32 = 60;
/// cshell 0x10061a30: a frame's displacement is clamped to clamp(0.75 x largest half extent, 4, 16); 16 for this hull.
const MOVE_CLAMP: f32 = 16.0;
/// The same clamp before the first stance change: cshell's copy of the box size (controller+0xb0..0xb8, written only by
/// 0x1005f5f0, in .bss) is still zero, so retail limits a frame to 4 units (run 240 u/s at 60 fps, walk 120 u/s at 30 fps).
const MOVE_CLAMP_UNSET: f32 = 4.0;

#[derive(Default, Clone, Copy)]
pub struct Input { pub forward: f32, pub right: f32, pub yaw: f32, pub run: bool, pub crouch: bool, pub jump: bool }

/// What the last `tick` did, for sound cues and health (nothing here plays a sound).
#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub struct Events {
    /// A jump started (`sounds\speech\hero\skok.wav`, cshell 0x10060c08).
    pub jumped: bool,
    /// The hull came to rest on a surface after being airborne.
    pub landed: bool,
    /// Health to subtract for this landing: (takeoff - landing - 196) x 0.15 (cshell 0x1006102f), 0 when none.
    pub fall_damage: f32,
    /// Crouched or stood up this tick.
    pub stance_changed: bool,
}

pub struct Player {
    pub position: Vec3, pub velocity: Vec3, pub grounded: bool, pub crouched: bool,
    pub stamina: f32, pub max_stamina: f32, pub footstep: bool,
    pub events: Events,
    /// Ticks since the level began (cshell shell+0x37c); the first 60 are frozen and free of fall damage.
    pub frames: u32,
    /// Set by a collision-correction system (doors, props, NPCs) that lifted the hull onto a dynamic solid; the standing state
    /// of the engine's standing-on-object (Lithtech.exe 0x4ab1c0) for the next tick. Lifting a falling hull is also detected.
    pub external_support: bool,
    /// Reproduce the retail frame-rate quirk of `MOVE_CLAMP_UNSET`; off by default because retail's MaxFPS 160 hides it.
    pub initial_clamp_quirk: bool,
    /// The hidden console variable `FullStamina` (cshell 0x10061f19): the stamina is refilled at the end of every stamina update.
    pub full_stamina: bool,
    stance_changed: bool, jump_latched: bool, exhausted: bool, step_timer: f32, takeoff: f32, was_standing: bool, expected: Vec3, support_hold: u8,
}

impl Player {
    pub fn new(position: Vec3) -> Self {
        // cshell 0x1005b759: the takeoff height starts at the start point's y; the footstep timer at 0 (0x1005f7ab).
        Self { position, velocity: Vec3::ZERO, grounded: false, crouched: false, stamina: 110.0, max_stamina: 110.0, footstep: false, events: Events::default(),
            frames: 0, external_support: false, initial_clamp_quirk: false, full_stamina: false, stance_changed: false, jump_latched: false, exhausted: false, step_timer: 0.0, takeoff: position.y, was_standing: false, expected: position, support_hold: 0 }
    }
    /// Half extents of the collision box: 16/58/16 until the first stance change (0x1005f7ba), then 24/58/24 or 24/24/24 (0x1005f5f0).
    pub fn half_size(&self) -> Vec3 {
        if self.crouched { Vec3::splat(24.0) } else { Vec3::new(if self.stance_changed { 24.0 } else { 16.0 }, 58.0, if self.stance_changed { 24.0 } else { 16.0 }) }
    }
    /// Resting on a walkable surface (the engine's standing-on state, cshell 0x100614e0).
    fn supported(&self, world: &CollisionWorld) -> bool { world.ground(self.position, self.half_size(), GROUND_PROBE, SKIN).is_some() }
    pub fn exhausted(&self) -> bool { self.exhausted }

    /// cshell 0x10061e00: jump cost, then run drain -4/s (running on the ground, not exhausted), +0.5/s while moving on
    /// the ground, +5.5/s otherwise; exhausted below 10, recovered above 15.
    fn stamina_step(&mut self, cost: f32, dt: f32, running: bool, ground_moving: bool) {
        let rate = if running && ground_moving && !self.exhausted { -4.0 } else if ground_moving { 0.5 } else { 5.5 };
        self.stamina = (self.stamina + cost + dt * rate).clamp(0.0, self.max_stamina);
        if self.stamina < 10.0 { self.exhausted = true; } else if self.stamina > 15.0 { self.exhausted = false; }
        if self.full_stamina { self.stamina = self.max_stamina; }
    }

    pub fn tick(&mut self, world: &CollisionWorld, input: &Input, dt: f32) {
        self.footstep = false; self.events = Events::default();
        if !dt.is_finite() || dt <= 0.0 { return; }
        self.frames = self.frames.saturating_add(1);
        // A correction after the last tick that lifted the falling hull is a dynamic solid under it. The support is kept for four
        // ticks; the last of them lets gravity act once more, and the next lift renews it (so the hull does not flicker between
        // standing and falling, while walking off the solid ends the support within four ticks).
        if self.external_support || (self.position.y - self.expected.y > 0.001 && self.velocity.y < 0.0) { self.support_hold = 4; }
        self.external_support = false;
        let static_support = self.supported(world);
        // The engine's standing-on state left by the last move; a rising hull is not standing (jump takeoff).
        let start_standing = (self.grounded || self.velocity.y <= 0.0) && (static_support || self.support_hold > 0);
        let test_fall = !static_support && self.support_hold == 1;
        self.support_hold = self.support_hold.saturating_sub(1);
        // 0x100606a2..0x100606eb: each stance change adds 64 to the vertical velocity, also in the air.
        let mut stance_moved = false;
        if input.crouch != self.crouched && (input.crouch || world.can_stand(self.position)) && self.change_stance(world, input.crouch) {
            stance_moved = true; self.events.stance_changed = true; self.velocity.y += 64.0;
        }
        // 0x100601b5..0x10060210, 0x10060633..0x10060716: speed limit 180, run 180+120x(stamina share+.2), crouch 90.
        let running = input.run && !self.exhausted && !self.crouched;
        let limit = if self.crouched { 90.0 } else if running { 180.0 + 120.0 * (self.stamina / self.max_stamina.max(1.0e-3) + 0.2).min(1.0) } else { 180.0 };
        let moving = input.forward != 0.0 || input.right != 0.0;
        let ground_moving = moving && start_standing;
        // 0x10060737..: 9000 u/s^2 along the yaw-only frame, x0.05 in the air (the standing state of the frame start).
        let (s, c) = input.yaw.sin_cos();
        self.velocity += Vec3::new(-s * input.forward + c * input.right, 0.0, -c * input.forward - s * input.right) * (9000.0 * dt * if start_standing { 1.0 } else { 0.05 });
        // The stance move cleared the engine's standing state, so no jump and no ground friction this frame.
        let standing = start_standing && !stance_moved;
        // 0x10060ba0: +300 (cap 450) from the ground, not exhausted, not still rising; costs 5 stamina.
        if input.jump && standing && !self.jump_latched && !self.exhausted {
            self.velocity.y = (self.velocity.y + 300.0).min(450.0);
            self.jump_latched = true; self.takeoff = -1.0e8; self.events.jumped = true;
            self.stamina_step(-5.0, dt, running, ground_moving);
        }
        // 0x1006102f..0x1006108d: fall damage on touching down after 60 frames, takeoff re-armed when leaving the ground.
        if !self.was_standing && standing {
            self.events.landed = true;
            let fall = self.takeoff - self.position.y - 196.0;
            if self.frames > FREEZE_FRAMES && fall > 0.0 { self.events.fall_damage = fall * 0.15; }
        }
        if self.was_standing && !standing { self.takeoff = self.position.y; }
        self.was_standing = standing;
        // 0x1006108d..0x10060fd: gravity 1000 u/s^2 in the air, floor at 0 on the ground, terminal -640.
        if !standing || test_fall { self.velocity.y -= 1000.0 * dt; } else if self.velocity.y < 0.0 { self.velocity.y = 0.0; }
        self.velocity.y = self.velocity.y.max(-640.0);
        // 0x100610dd: the jump latch clears when the rise ends and the takeoff height becomes the apex.
        if self.velocity.y <= 0.0 { if self.jump_latched { self.takeoff = self.position.y; } self.jump_latched = false; }
        // 0x10061112..0x100611eb: without input on the ground the horizontal velocity is scaled by (.95-dt)*.45 per frame.
        let mut horizontal = Vec3::new(self.velocity.x, 0.0, self.velocity.z);
        if standing && !moving { horizontal *= (0.95 - dt) * 0.45; }
        let speed = horizontal.length();
        if speed < 1.0 { horizontal = Vec3::ZERO; } else if speed > limit { horizontal *= limit / speed; }
        self.velocity.x = horizontal.x; self.velocity.z = horizontal.z;
        self.stamina_step(0.0, dt, running, ground_moving);
        // 0x1006131b: running footsteps every 0.4 s while moving on the ground.
        if running && ground_moving { self.step_timer -= dt; if self.step_timer < 0.0 { self.footstep = true; self.step_timer = 0.4; } }
        // 0x100618f0: frozen for the first 60 frames and across a frame time above 1.5 s.
        if self.frames < FREEZE_FRAMES || dt > 1.5 { self.velocity = Vec3::ZERO; }
        else {
            let mut delta = self.velocity * dt;
            let length = delta.length();
            if length > 0.001 {
                let clamp = if self.initial_clamp_quirk && !self.stance_changed { MOVE_CLAMP_UNSET } else { MOVE_CLAMP };
                if length > clamp { delta *= clamp / length; }
                self.move_world(world, delta, start_standing);
            }
        }
        self.grounded = self.supported(world);
        self.expected = self.position;
    }

    /// The engine's box move: sweep, slide along the contacts, step up ledges of at most the stair height.
    fn move_world(&mut self, world: &CollisionWorld, delta: Vec3, may_step: bool) {
        let half = self.half_size();
        let mut position = self.position;
        // An embedded hull (level start, a closing door, a stance change) is pushed out before it moves.
        if world.box_overlaps(position, half) { if let Some(free) = world.depenetrate(position, half, 48.0) { position = free; } }
        let mut remaining = delta;
        let (mut planes, mut count) = ([Vec3::ZERO; 6], 0);
        for _ in 0..10 {
            if remaining.length_squared() < 1.0e-8 { break; }
            let Some(hit) = world.sweep(position, half, remaining, SKIN) else { position += remaining; break };
            position += remaining * hit.time + hit.normal * BACKOFF;
            let tail = remaining * (1.0 - hit.time);
            if may_step && hit.normal.y < GROUND_NORMAL && !world.no_step(&hit) {
                if let Some((raised, advance)) = step_up(world, half, position, tail) {
                    position = raised;
                    remaining = Vec3::new(tail.x * (1.0 - advance), tail.y, tail.z * (1.0 - advance));
                    continue;
                }
            }
            if count < planes.len() { planes[count] = hit.normal; count += 1; }
            remaining = clip(tail, &planes[..count]);
        }
        self.position = position;
    }

    /// cshell 0x1005f5f0: crouching shrinks the box in place; standing lifts it 32 first and then grows it into the
    /// free space around it (Lithtech.exe 0x451c40/0x452090). Returns false when the box does not fit.
    fn change_stance(&mut self, world: &CollisionWorld, crouch: bool) -> bool {
        let old_half = self.half_size();
        let (target, start) = if crouch { (Vec3::splat(24.0), self.position) } else { (Vec3::new(24.0, 58.0, 24.0), self.position + Vec3::Y * 32.0) };
        // Standing is a MoveObject +32 (as far as it is free) followed by the resize.
        let mut position = start;
        if !crouch { if let Some(hit) = world.sweep(self.position, old_half, Vec3::Y * 32.0, SKIN) { position = self.position + Vec3::Y * (32.0 * hit.time); } }
        let mut half = old_half;
        for axis in 0..3 {
            if target[axis] > half[axis] {
                // Growth by g per side needs 2g of free travel along the axis around the centre (0x452090 moves -g, then +2g and
                // centres the box in that span; here both sides are measured, so a wall on either side shifts the box away).
                let g = target[axis] - half[axis];
                let mut unit = Vec3::ZERO; unit[axis] = 1.0;
                let free_low = world.sweep(position, half, -unit * 2.0 * g, SKIN).map_or(2.0 * g, |h| h.time * 2.0 * g);
                let free_high = world.sweep(position, half, unit * 2.0 * g, SKIN).map_or(2.0 * g, |h| h.time * 2.0 * g);
                if free_low + free_high < 2.0 * g - 0.25 { return false; }
                let (lo, hi) = (g - free_low, free_high - g);
                position += unit * if lo <= hi { 0.0f32.clamp(lo, hi) } else { (lo + hi) * 0.5 };
                half[axis] = target[axis];
            } else { half[axis] = target[axis]; }
        }
        if world.box_overlaps(position, half) && world.depenetrate(position, half, 8.0).map(|free| position = free).is_none() { return false; }
        self.crouched = crouch; self.stance_changed = true; self.position = position;
        true
    }
}

/// The engine's stair step: up by at most the stair height, forward over the tread, down onto a walkable, steppable surface.
/// Returns the new position and the fraction of the horizontal tail that was consumed.
fn step_up(world: &CollisionWorld, half: Vec3, position: Vec3, tail: Vec3) -> Option<(Vec3, f32)> {
    let horizontal = Vec3::new(tail.x, 0.0, tail.z);
    let length = horizontal.length();
    if length < 1.0e-4 { return None; }
    let rise = world.sweep(position, half, Vec3::Y * STAIR_HEIGHT, SKIN).map_or(STAIR_HEIGHT, |h| h.time * STAIR_HEIGHT);
    if rise < 0.5 { return None; }
    let raised = position + Vec3::Y * rise;
    let advance = world.sweep(raised, half, horizontal, SKIN).map_or(1.0, |h| h.time);
    if advance * length < 0.25 { return None; }
    let over = raised + horizontal * advance;
    let reach = rise + 0.5;
    // Any contact on the way down must be the tread itself: a steeper surface in the path means no step.
    let floor = world.sweep(over, half, -Vec3::Y * reach, SKIN)?;
    if floor.normal.y < GROUND_NORMAL || world.no_step(&floor) { return None; }
    let landed = over - Vec3::Y * (floor.time * reach) + floor.normal * BACKOFF;
    (landed.y > position.y + 0.02).then_some((landed, advance))
}

/// Quake-style clip of the remaining motion against every contact plane of this move, sliding along a crease when two planes fight.
fn clip(velocity: Vec3, planes: &[Vec3]) -> Vec3 {
    for (i, plane) in planes.iter().enumerate() {
        let candidate = velocity - *plane * velocity.dot(*plane).min(0.0);
        if planes.iter().enumerate().all(|(j, other)| j == i || candidate.dot(*other) >= -1.0e-4) { return candidate; }
    }
    if let [a, b, ..] = planes {
        let crease = a.cross(*b);
        if crease.length_squared() > 1.0e-8 { let crease = crease.normalize(); return crease * crease.dot(velocity); }
    }
    Vec3::ZERO
}
