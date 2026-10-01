//! Synthetic-geometry tests of the box solver and the retail frame rules (cshell 0x10060160..0x100618f0).
use retail_movement::{CollisionWorld, Input, Player, Vec3, SURF_NOTASTEP};

const DT: f32 = 1.0 / 60.0;

/// Closed axis-aligned boxes (min, max) as an OBJ hull.
fn boxes(list: &[([f32; 3], [f32; 3])]) -> String {
    let mut out = String::new();
    for (n, (lo, hi)) in list.iter().enumerate() {
        for &(x, y, z) in &[(lo[0], lo[1], lo[2]), (hi[0], lo[1], lo[2]), (hi[0], lo[1], hi[2]), (lo[0], lo[1], hi[2]), (lo[0], hi[1], lo[2]), (hi[0], hi[1], lo[2]), (hi[0], hi[1], hi[2]), (lo[0], hi[1], hi[2])] { out += &format!("v {x} {y} {z}\n"); }
        let b = n * 8;
        for f in [[1, 4, 3, 2], [5, 6, 7, 8], [1, 2, 6, 5], [2, 3, 7, 6], [3, 4, 8, 7], [4, 1, 5, 8]] { out += &format!("f {} {} {} {}\n", b + f[0], b + f[1], b + f[2], b + f[3]); }
    }
    out
}
fn world(list: &[([f32; 3], [f32; 3])]) -> CollisionWorld { CollisionWorld::from_obj(&boxes(list)).unwrap() }
/// A big floor slab whose top is y = 0.
const FLOOR: ([f32; 3], [f32; 3]) = ([-4000.0, -100.0, -4000.0], [4000.0, 0.0, 4000.0]);
fn free(position: Vec3) -> Player { let mut p = Player::new(position); p.frames = 60; p }
fn go(world: &CollisionWorld, p: &mut Player, input: &Input, ticks: usize, dt: f32) { for _ in 0..ticks { p.tick(world, input, dt); } }
fn on_floor(x: f32, z: f32) -> Player { let mut p = free(Vec3::new(x, 58.1, z)); p.grounded = true; p }
fn forward(yaw: f32) -> Input { Input { forward: 1.0, yaw, ..Default::default() } }

#[test]
fn ramps_are_climbed_below_45_degrees_and_refused_above() {
    // Walkable ramp: 30 degrees rising towards -Z; steep ramp: 60 degrees. Hulls as wedges are boxes rotated: use triangles directly.
    let wedge = |rise: f32| format!("v -500 0 0\nv 500 0 0\nv 500 0 -400\nv -500 0 -400\nv -500 {rise} -400\nv 500 {rise} -400\nf 1 2 3 4\nf 1 5 6 2\nf 5 4 3 6\nf 1 4 5\nf 2 6 3\nf 4 1 2 3\n");
    for (rise, climbs) in [(230.0, true), (690.0, false)] {
        let mut source = boxes(&[FLOOR]);
        // The wedge vertices are appended after the floor's 8 vertices.
        let base = 8;
        for (i, line) in wedge(rise).lines().enumerate() {
            if let Some(f) = line.strip_prefix("f ") { source += &format!("f {}\n", f.split_whitespace().map(|n| (n.parse::<usize>().unwrap() + base).to_string()).collect::<Vec<_>>().join(" ")); } else { source += line; source.push('\n'); }
            let _ = i;
        }
        let w = CollisionWorld::from_obj(&source).unwrap();
        let mut p = on_floor(0.0, 100.0);
        go(&w, &mut p, &forward(0.0), 100, DT);
        let gained = p.position.y - 58.1;
        if climbs { assert!(gained > 60.0 && p.grounded, "30 degree ramp: {gained} {:?}", p.position); } else { assert!(gained < 20.0, "60 degree ramp climbed by {gained}"); }
    }
}

#[test]
fn concave_corners_neither_stick_nor_leak() {
    let w = world(&[FLOOR, ([-1000.0, 0.0, -1000.0], [-100.0, 300.0, 1000.0]), ([-1000.0, 0.0, -1000.0], [1000.0, 300.0, -100.0])]);
    let mut p = on_floor(0.0, 0.0);
    // Diagonal run into the corner at (-100, -100), then out again.
    go(&w, &mut p, &Input { forward: 1.0, right: -1.0, yaw: 0.0, run: true, ..Default::default() }, 180, DT);
    assert!(p.position.x >= -100.0 + 15.9 && p.position.z >= -100.0 + 15.9, "inside the corner: {:?}", p.position);
    assert!(p.position.x < -70.0 && p.position.z < -70.0, "reached the corner: {:?}", p.position);
    go(&w, &mut p, &Input { forward: 1.0, right: -1.0, yaw: std::f32::consts::PI, run: true, ..Default::default() }, 90, DT);
    assert!(p.position.x > 0.0 && p.position.z > 0.0, "walks out of the corner: {:?}", p.position);
}

#[test]
fn a_thin_wall_holds_at_every_frame_time() {
    let w = world(&[FLOOR, ([-1000.0, 0.0, -101.0], [1000.0, 300.0, -100.0])]);
    for dt in [1.0 / 240.0, DT, 0.05, 0.25, 1.0] {
        let mut p = on_floor(0.0, 0.0);
        go(&w, &mut p, &Input { forward: 1.0, run: true, ..Default::default() }, (10.0 / dt) as usize, dt);
        assert!(p.position.z > -100.0, "dt {dt}: {:?}", p.position);
    }
}

#[test]
fn not_a_step_faces_refuse_the_stair() {
    let step = ([-1000.0, 0.0, -1000.0], [1000.0, 12.0, -100.0]);
    let mut w = world(&[FLOOR, step]);
    let mut p = on_floor(0.0, 0.0);
    go(&w, &mut p, &forward(0.0), 120, DT);
    assert!(p.position.z < -300.0 && p.position.y > 58.0 + 11.0, "a 12 unit step is climbed: {:?}", p.position);
    // Every face of the step brush flagged NotAStep (DAT surface flag 1<<22).
    w.set_surface_flags((0..12).map(|face| if face >= 6 { SURF_NOTASTEP } else { 0 }).collect());
    let mut p = on_floor(0.0, 0.0);
    go(&w, &mut p, &forward(0.0), 120, DT);
    assert!(p.position.z > -100.0 && p.position.y < 60.0, "a NotAStep ledge blocks: {:?}", p.position);
}

/// Retail polls the jump command level-triggered every frame (IsPressed 0x10011960, poll 0x10060b8b). The only guards are on the ground (0x100614e0), stamina and the
/// latch +0xdc, which is set by the jump and cleared once the rise ends (vy <= 0, 0x100610e9..0x100610f6): a HELD key jumps again the frame the body is back on
/// the floor (a bunny hop), never in the air, and every jump costs 5 stamina until the exhaustion rule (below 10) refuses it.
#[test]
fn a_held_jump_key_repeats_on_landing_but_never_in_the_air() {
    let w = world(&[FLOOR]);
    let mut p = on_floor(0.0, 0.0);
    p.tick(&w, &Input::default(), DT);
    let held = Input { jump: true, ..Default::default() };
    // Two seconds with the key held the whole time: one jump per ~0.6 s of flight (0.3 s up, 0.3 s down). A jump event only ever comes from the floor; the doubled event after a long fall
    // is retail's own frame order (next test).
    let (mut starts, mut prev_jumped, mut prev_y, mut air_events) = (0, false, 58.1, 0);
    for _ in 0..120 { p.tick(&w, &held, DT); if p.events.jumped { if !prev_jumped { starts += 1; } if prev_y > 58.1 + 6.0 { air_events += 1; } } prev_jumped = p.events.jumped; prev_y = p.position.y; }
    assert!((3..=4).contains(&starts), "held for 2 s: {starts} jumps");
    assert_eq!(air_events, 0, "a jump started in the air");
    // Released in the air, pressed again once landed: exactly one more jump per press edge.
    for _ in 0..90 { p.tick(&w, &Input::default(), DT); }
    assert!(p.grounded);
    let mut again = 0;
    for _ in 0..10 { p.tick(&w, &held, DT); if p.events.jumped { again += 1; } }
    assert_eq!(again, 1, "one press, one jump within the rise");
}

#[test]
fn jump_apex_takeoff_and_ceiling() {
    let w = world(&[FLOOR]);
    let mut p = on_floor(0.0, 0.0);
    p.tick(&w, &Input::default(), DT);
    p.tick(&w, &Input { jump: true, ..Default::default() }, DT);
    // The jump frame is still standing: +300 without gravity (0x10060bc3), stamina -5.
    assert!(p.events.jumped && (p.velocity.y - 300.0).abs() < 0.01 && p.stamina < 106.0, "{:?} {}", p.velocity, p.stamina);
    let mut apex = 0.0f32;
    for _ in 0..80 { p.tick(&w, &Input::default(), DT); apex = apex.max(p.position.y - 58.1); }
    assert!((apex - 47.5).abs() < 1.5, "jump apex {apex}");
    assert!(p.grounded && p.events.fall_damage == 0.0);
    // A ceiling at 150 stops the head: hull top 116 + rise 34.
    let w = world(&[FLOOR, ([-1000.0, 150.0, -1000.0], [1000.0, 200.0, 1000.0])]);
    let mut p = on_floor(0.0, 0.0);
    p.tick(&w, &Input::default(), DT);
    let mut top = 0.0f32; let mut pressed = 0;
    for tick in 0..90 { p.tick(&w, &Input { jump: tick == 0, ..Default::default() }, DT); top = top.max(p.position.y + 58.0); if p.position.y + 58.0 > 149.0 && p.velocity.y > 0.0 { pressed += 1; } }
    assert!(top < 150.0, "head through the ceiling: {top}");
    // The vertical velocity is not cancelled by the contact (cshell never reads it back), so the head presses for a while.
    assert!(pressed >= 3, "pressed {pressed} ticks");
    assert!(p.grounded);
}

#[test]
fn fall_damage_follows_the_takeoff_apex_rule() {
    let w = world(&[FLOOR]);
    // Frozen start: hovering 460 above the floor for 60 frames, then a fall that counts.
    let mut p = Player::new(Vec3::new(0.0, 458.1, 0.0));
    let mut hurt = 0.0;
    for tick in 0..260 { p.tick(&w, &Input::default(), DT); if tick < 59 { assert_eq!(p.position.y, 458.1); } hurt += p.events.fall_damage; }
    assert!((hurt - (458.1 - 58.1 - 196.0) * 0.15).abs() < 1.0, "{hurt}");
    // Walking off a ledge: 150 units hurt nothing, 250 hurt (250 - 196) x 0.15 (the takeoff is where the hull left the ground).
    for (drop, expect) in [(150.0, 0.0), (250.0, 8.1)] {
        let w = world(&[FLOOR, ([-200.0, -400.0, -200.0], [200.0, drop, 200.0])]);
        let mut p = free(Vec3::new(0.0, drop + 58.1, 0.0)); p.grounded = true;
        p.tick(&w, &Input::default(), DT);
        let mut hurt = 0.0;
        for _ in 0..400 { p.tick(&w, &Input { forward: 1.0, yaw: 0.0, ..Default::default() }, DT); hurt += p.events.fall_damage; }
        assert!((hurt - expect).abs() < 0.2, "drop {drop}: {hurt}");
    }
    // A jump's takeoff is its apex (cshell 0x100610f6): jumping off the ledge edge hurts (apex - landing - 196) x 0.15 with the apex 47.5 above the ledge.
    let w = world(&[FLOOR, ([-200.0, -400.0, -200.0], [200.0, 300.0, 200.0])]);
    let mut p = free(Vec3::new(0.0, 358.1, -170.0)); p.grounded = true;
    p.tick(&w, &Input::default(), DT);
    let mut hurt = 0.0;
    for tick in 0..400 { p.tick(&w, &Input { jump: tick == 0, forward: 1.0, ..Default::default() }, DT); hurt += p.events.fall_damage; }
    let expect = (300.0 + 58.1 + 47.5 - 58.1 - 196.0) * 0.15;
    assert!((hurt - expect).abs() < 1.5, "jump from a ledge: {hurt} vs {expect}");
    let _ = w;
}

#[test]
fn standing_up_fits_the_box_and_is_refused_under_a_low_ceiling() {
    let w = world(&[FLOOR, ([-1000.0, 100.0, -1000.0], [-200.0, 200.0, 1000.0])]);
    // Free space: crouch, wait, stand up: the bottom rests on the floor, nothing is embedded.
    let mut p = on_floor(0.0, 0.0);
    go(&w, &mut p, &Input { crouch: true, ..Default::default() }, 60, DT);
    assert!(p.crouched && (p.half_size().y - 24.0).abs() < 1e-3 && p.position.y < 26.0, "{:?}", p.position);
    go(&w, &mut p, &Input::default(), 2, DT);
    assert!(!p.crouched && (p.half_size() - Vec3::new(24.0, 58.0, 24.0)).length() < 1e-3);
    assert!(!w.box_overlaps(p.position, p.half_size()) && p.position.y - 58.0 > -0.3, "{:?}", p.position);
    go(&w, &mut p, &Input::default(), 60, DT);
    assert!(p.grounded && (p.position.y - 58.05).abs() < 0.3, "{:?}", p.position);
    // Under the 100-high ceiling: the crouched hull (48) fits, the rays (108 up from the centre) refuse to stand.
    let mut p = on_floor(-500.0, 0.0);
    go(&w, &mut p, &Input { crouch: true, ..Default::default() }, 60, DT);
    go(&w, &mut p, &Input { right: -1.0, yaw: 0.0, ..Default::default() }, 1, DT);
    let mut q = free(Vec3::new(-400.0, 24.1, 0.0)); q.crouched = true; q.grounded = true;
    go(&w, &mut q, &Input::default(), 60, DT);
    assert!(q.crouched, "still crouched under the ceiling");
    go(&w, &mut q, &Input { forward: 1.0, yaw: std::f32::consts::FRAC_PI_2 * 3.0, ..Default::default() }, 120, DT);
    let _ = p;
}

#[test]
fn the_first_crouch_widens_the_hull_by_shifting_off_a_wall() {
    // 8 units of room beside the wall: the 16 -> 24 growth pushes the box away instead of embedding it.
    let w = world(&[FLOOR, ([-1000.0, 0.0, 30.0], [1000.0, 300.0, 100.0])]);
    let mut p = on_floor(0.0, 30.0 - 16.0 - 0.06);
    p.tick(&w, &Input { crouch: true, ..Default::default() }, DT);
    assert!(p.crouched && !w.box_overlaps(p.position, p.half_size()), "{:?} {:?}", p.position, p.half_size());
    assert!(p.position.z <= 30.0 - 24.0 + 0.1, "pushed off the wall: {:?}", p.position);
    // Flush against the wall on both sides in a 40-wide slot the growth to 48 fails and the stance does not change.
    let w = world(&[FLOOR, ([-1000.0, 0.0, 30.0], [1000.0, 300.0, 100.0]), ([-1000.0, 0.0, -100.0], [1000.0, 300.0, -10.0])]);
    let mut p = on_floor(0.0, 10.0);
    p.tick(&w, &Input { crouch: true, ..Default::default() }, DT);
    assert!(!p.crouched && !w.box_overlaps(p.position, p.half_size()), "{:?}", p.position);
}

#[test]
fn speeds_friction_and_air_control_follow_the_retail_numbers() {
    let w = world(&[FLOOR]);
    let speed = |p: &Player| Vec3::new(p.velocity.x, 0.0, p.velocity.z).length();
    // Walk 180; run 180 + 120 x min(stamina share + .2, 1) = 300 at full stamina; crouch 90.
    let mut p = on_floor(0.0, 0.0);
    go(&w, &mut p, &forward(0.0), 30, DT); assert!((speed(&p) - 180.0).abs() < 0.5, "{}", speed(&p));
    let mut p = on_floor(0.0, 0.0);
    go(&w, &mut p, &Input { forward: 1.0, run: true, ..Default::default() }, 3, DT); assert!((speed(&p) - 300.0).abs() < 0.5, "{}", speed(&p));
    let mut p = on_floor(0.0, 0.0); p.stamina = 22.0;
    go(&w, &mut p, &Input { forward: 1.0, run: true, ..Default::default() }, 3, DT); assert!((speed(&p) - (180.0 + 120.0 * (22.0f32 / 110.0 + 0.2))).abs() < 1.0, "{}", speed(&p));
    let mut p = on_floor(0.0, 0.0);
    go(&w, &mut p, &Input { forward: 1.0, crouch: true, run: true, ..Default::default() }, 30, DT); assert!((speed(&p) - 90.0).abs() < 0.5, "{}", speed(&p));
    // Releasing the keys on the ground: velocity x (.95 - dt) x .45 per frame, gone below 1 unit/s.
    let mut p = on_floor(0.0, 0.0);
    go(&w, &mut p, &forward(0.0), 30, DT);
    p.tick(&w, &Input::default(), DT); assert!((speed(&p) - 180.0 * (0.95 - DT) * 0.45).abs() < 0.5, "{}", speed(&p));
    go(&w, &mut p, &Input::default(), 5, DT); assert_eq!(speed(&p), 0.0);
    // In the air the acceleration is 5 % (450 u/s^2) and nothing brakes.
    let mut p = free(Vec3::new(0.0, 1000.0, 0.0));
    go(&w, &mut p, &forward(0.0), 10, DT);
    assert!((speed(&p) - 450.0 * 10.0 * DT).abs() < 0.5, "{}", speed(&p));
    go(&w, &mut p, &Input::default(), 10, DT); assert!((speed(&p) - 450.0 * 10.0 * DT).abs() < 0.5);
}

#[test]
fn a_frame_moves_at_most_16_units_and_the_start_is_frozen() {
    let w = world(&[FLOOR]);
    let mut p = free(Vec3::new(0.0, 1000.0, 0.0)); p.velocity.y = -640.0;
    let before = p.position;
    p.tick(&w, &Input::default(), 0.5);
    assert!(((before - p.position).length() - 16.0).abs() < 0.01, "{:?}", p.position);
    let mut p = Player::new(Vec3::new(0.0, 58.1, 0.0));
    for _ in 0..59 { p.tick(&w, &Input { forward: 1.0, run: true, ..Default::default() }, DT); assert_eq!((p.position, p.velocity), (Vec3::new(0.0, 58.1, 0.0), Vec3::ZERO)); }
    p.tick(&w, &Input { forward: 1.0, run: true, ..Default::default() }, DT);
    p.tick(&w, &Input { forward: 1.0, run: true, ..Default::default() }, DT);
    assert!(p.position.z < 0.0);
}

#[test]
fn an_embedded_hull_is_pushed_out_and_can_walk_away() {
    let w = world(&[FLOOR, ([-1000.0, 0.0, 30.0], [1000.0, 300.0, 100.0])]);
    for depth in [0.5, 4.0, 12.0] {
        let mut p = on_floor(0.0, 30.0 - 16.0 + depth);
        assert!(w.box_overlaps(p.position, p.half_size()));
        go(&w, &mut p, &Input { forward: 1.0, yaw: 0.0, ..Default::default() }, 30, DT);
        assert!(!w.box_overlaps(p.position, p.half_size()) && p.position.z < 30.0 - 15.9, "depth {depth}: {:?}", p.position);
    }
}

/// A stand-in for the door/prop/NPC correction passes: lifts the hull out of a dynamic slab after every tick.
fn lift_onto_slab(p: &mut Player, top: f32, x: (f32, f32)) {
    if p.position.x > x.0 && p.position.x < x.1 && p.position.y - 58.0 < top { p.position.y = top + 58.05; }
}

#[test]
fn a_dynamic_solid_under_the_hull_counts_as_ground() {
    // A slab (not in the static hull) 40 units above the floor: the player is put on it by a correction after each tick.
    let w = world(&[FLOOR]);
    let mut p = free(Vec3::new(0.0, 98.1, 0.0));
    let mut hover = 0;
    for _ in 0..120 { p.tick(&w, &Input::default(), DT); lift_onto_slab(&mut p, 40.0, (-100.0, 100.0)); if p.velocity.y < -1.0 { hover += 1; } }
    assert!(p.position.y > 98.0 && p.position.y < 98.2, "{:?}", p.position);
    // Standing on it: full ground acceleration, friction and a jump.
    let speed = |p: &Player| Vec3::new(p.velocity.x, 0.0, p.velocity.z).length();
    p.tick(&w, &Input { right: 1.0, yaw: 0.0, ..Default::default() }, DT);
    assert!(speed(&p) > 100.0, "ground acceleration on the slab: {}", speed(&p));
    lift_onto_slab(&mut p, 40.0, (-100.0, 100.0));
    for _ in 0..30 { p.tick(&w, &Input::default(), DT); lift_onto_slab(&mut p, 40.0, (-100.0, 100.0)); }
    assert!(speed(&p) == 0.0, "friction on the slab");
    let mut jumped = false;
    for _ in 0..8 { p.tick(&w, &Input { jump: true, ..Default::default() }, DT); lift_onto_slab(&mut p, 40.0, (-100.0, 100.0)); jumped |= p.events.jumped; }
    assert!(jumped, "the player can jump from the slab (hover {hover})");
    // An explicit hint from a correction pass works without the lift being noticed.
    let mut p = free(Vec3::new(0.0, 500.0, 0.0)); p.velocity.y = -100.0;
    p.external_support = true; p.grounded = true;
    p.tick(&w, &Input::default(), DT);
    assert!(p.velocity.y == 0.0, "standing state from the hint: {}", p.velocity.y);
}

#[test]
fn the_initial_step_clamp_quirk_limits_a_frame_to_four_units_until_the_first_stance_change() {
    let w = world(&[FLOOR]);
    let mut p = on_floor(0.0, 0.0); p.initial_clamp_quirk = true;
    go(&w, &mut p, &Input { forward: 1.0, run: true, ..Default::default() }, 30, DT);
    let z = p.position.z;
    go(&w, &mut p, &Input { forward: 1.0, run: true, ..Default::default() }, 60, DT);
    assert!((z - p.position.z - 240.0).abs() < 2.0, "240 u/s at 60 fps, not 300: {}", z - p.position.z);
    go(&w, &mut p, &Input { crouch: true, ..Default::default() }, 30, DT);
    go(&w, &mut p, &Input::default(), 60, DT);
    let z = p.position.z;
    go(&w, &mut p, &Input { forward: 1.0, run: true, ..Default::default() }, 60, DT);
    assert!((z - p.position.z - 300.0).abs() < 4.0, "16 units after the first stance change: {}", z - p.position.z);
}

/// The touch-down double jump is retail's own frame structure, not a solver artefact: the jump sits in the input stage (0x10060b92..0x10060c08: vy + 300 on the
/// velocity the frame before left, still negative from the fall), the frame function then clamps a standing body's vy to >= 0 (0x100610a7..0x100610ba) and clears the
/// jump latch because vy <= 0 (0x100610dd..0x100610f6); the next frame jumps again for real. The first jump has cost 5 stamina and played its sound for nothing.
#[test]
fn a_held_jump_key_jumps_twice_on_touching_down_after_a_long_fall_like_the_retail_frame_order() {
    let w = world(&[FLOOR]);
    // From 150 units up the body arrives at about 540 u/s: 540 - 300 is still downward, the clamp flattens it.
    let mut p = free(Vec3::new(0.0, 58.1 + 150.0, 0.0));
    let (mut events, mut first_landing_tick) = (Vec::<(i32, f32)>::new(), None::<i32>);
    for tick in 0..90i32 { let before = p.stamina; p.tick(&w, &Input { jump: tick > 10, ..Default::default() }, DT); if p.events.jumped { events.push((tick, before - p.stamina)); } if p.events.landed && first_landing_tick.is_none() { first_landing_tick = Some(tick); } }
    let landing = first_landing_tick.expect("the body lands");
    let at_landing: Vec<_> = events.iter().filter(|(tick, _)| *tick >= landing.saturating_sub(1) && *tick <= landing + 2).collect();
    assert_eq!(at_landing.len(), 2, "two jump events on consecutive ticks around the landing: {events:?} (landed at {landing})");
    assert_eq!(at_landing[1].0, at_landing[0].0 + 1);
    // A short fall (10 units: 140 u/s, less than the 300 of the jump) jumps once and really takes off.
    let mut p = free(Vec3::new(0.0, 58.1 + 10.0, 0.0));
    let mut count = 0;
    for tick in 0..20i32 { p.tick(&w, &Input { jump: tick > 0, ..Default::default() }, DT); if p.events.jumped { count += 1; } }
    assert_eq!(count, 1, "a short fall does not double");
}
