//! Level-wide headless movement harness: scripted movement over the collision data of every exported
//! retail level (static hull plus the closed door/drawer brushes). Needs the shared `output/` exports and
//! skips itself when they are missing. `MOVE_LEVEL=name` restricts to one level, `MOVE_SAMPLES=n` sets the
//! number of stuck-probe spots. Nothing here opens a window or plays a sound.
//!
//! Failure classes: non-finite state, tunnelling (the centre ray crosses a triangle), staying embedded, a spawn
//! that does not settle, a spot that cannot be walked away from, a drop that never lands, a jump with the wrong
//! apex, trouble under low ceilings. Falling off an open edge into the void is reported (`open`) but is level
//! design, not a defect.
mod common;
use common::*;
use retail_movement::{CollisionWorld, Input, Vec3};
use std::f32::consts::TAU;

#[derive(Default, Debug)]
struct Report {
    name: String, triangles: usize, spawn_ok: bool, spawn_settle: f32, compass_min: f32, compass_max: f32,
    stuck_spots: usize, stuck: Vec<Vec3>, fall_fail: Vec<Vec3>, violations: Vec<(String, usize, Violation)>, kinds: [usize; 4],
    steps: usize, max_step: f32, stairs: usize, stairs_fail: Vec<Vec3>, jumps: usize, jump_apex: (f32, f32), low_spots: usize, low_fail: Vec<Vec3>,
}

fn samples() -> usize { std::env::var("MOVE_SAMPLES").ok().and_then(|s| s.parse().ok()).unwrap_or(120) }

fn selected() -> Vec<String> {
    let all = level_names();
    match std::env::var("MOVE_LEVEL") { Ok(one) => all.into_iter().filter(|n| *n == one).collect(), Err(_) => all }
}

fn note(report: &mut Report, tag: &str, watch: Watch) {
    for (tick, v) in watch.violations {
        report.kinds[v.index()] += 1;
        if report.violations.iter().filter(|(_, _, o)| o.index() == v.index()).count() < 3 { report.violations.push((tag.to_owned(), tick, v)); }
    }
}

fn random_input(rng: &mut Rng) -> Input {
    Input { forward: if rng.chance(0.85) { 1.0 } else { -1.0 }, right: if rng.chance(0.2) { rng.range(-1.0, 1.0).signum() } else { 0.0 },
        yaw: rng.range(0.0, TAU), run: rng.chance(0.4), crouch: rng.chance(0.15), jump: rng.chance(0.25) }
}

/// Random walking, jumping, crouching and running; `dt` fixed or spiking to 0.05 s. Returns the visited grounded spots.
fn fuzz(level: &Level, report: &mut Report, home: Vec3, seed: u64, ticks: usize, dt: Option<f32>, tag: &str) -> Vec<Vec3> {
    let mut rng = Rng(0x9E3779B97F4A7C15 ^ (seed * 7919) ^ level.name.len() as u64);
    let mut p = free_player(home); let mut watch = Watch::new(&p);
    let (mut input, mut hold) = (Input::default(), 0);
    let mut visited = Vec::new();
    for tick in 0..ticks {
        if hold == 0 { hold = 10 + (rng.next() % 50) as i32; input = random_input(&mut rng); }
        hold -= 1;
        let (before, was_grounded) = (p.position, p.grounded);
        p.tick(&level.world, &input, dt.unwrap_or(if rng.chance(0.05) { 0.05 } else { DT }));
        watch.check(level, &p);
        // A stair step: grounded before and after, the hull rose in one tick.
        if was_grounded && p.grounded && p.position.y - before.y > 1.5 && !p.events.stance_changed { report.steps += 1; report.max_step = report.max_step.max(p.position.y - before.y); }
        if tick % 40 == 0 && p.grounded && !p.crouched { visited.push(p.position); }
    }
    note(report, tag, watch);
    visited
}

fn run_level(name: &str) -> Report {
    let level = load(name);
    let mut report = Report { name: name.to_owned(), triangles: level.triangles, compass_min: f32::MAX, jump_apex: (f32::MAX, 0.0), ..Default::default() };

    // 1. Spawn: the authored start settles onto a floor and stays put; frozen for the first 60 frames.
    let mut player = spawn_player(&level);
    let mut watch = Watch::new(&player);
    let start = player.position;
    for tick in 0..180 {
        player.tick(&level.world, &Input::default(), DT); watch.check(&level, &player);
        if tick < 58 { assert_eq!(player.position, start, "{name}: the start frames are frozen"); }
    }
    report.spawn_ok = player.grounded && (player.position - level.spawn).length() < 150.0;
    report.spawn_settle = (player.position - level.spawn).length();
    note(&mut report, "spawn", watch);
    let home = player.position;

    // 2. Compass: walk into whatever is around the spawn in 16 directions (walk and run).
    for run in [false, true] {
        for i in 0..16 {
            let mut p = free_player(home); p.grounded = true;
            let mut watch = Watch::new(&p);
            let input = walk_input(i as f32 * TAU / 16.0, run);
            for _ in 0..240 { p.tick(&level.world, &input, DT); watch.check(&level, &p); }
            let d = ((p.position.x - home.x).powi(2) + (p.position.z - home.z).powi(2)).sqrt();
            report.compass_min = report.compass_min.min(d); report.compass_max = report.compass_max.max(d);
            note(&mut report, &format!("compass{i}{}", if run { "r" } else { "" }), watch);
        }
    }

    // 3. Fuzz from the spawn (60 fps with dt spikes), plus fixed 30 and 144 fps runs; grounded positions become probe spots.
    let mut visited = Vec::new();
    for seed in 1..=8u64 { visited.extend(fuzz(&level, &mut report, home, seed, 2400, None, &format!("fuzz{seed}"))); }
    for (dt, tag) in [(1.0 / 30.0, "fuzz30"), (1.0 / 144.0, "fuzz144")] { fuzz(&level, &mut report, home, 11, 2400, Some(dt), tag); }

    // 4. Stuck probe: from many reachable spots at least one direction has to be walkable, and a drop has to land.
    let stride = (visited.len() / samples().max(1)).max(1);
    for (n, &spot) in visited.iter().step_by(stride).enumerate() {
        report.stuck_spots += 1;
        let mut best = 0.0f32;
        for i in 0..8 {
            let mut p = free_player(spot); let mut watch = Watch::new(&p);
            let input = walk_input(i as f32 * TAU / 8.0 + 0.3, true);
            for _ in 0..90 { p.tick(&level.world, &input, DT); watch.check(&level, &p); }
            best = best.max(((p.position.x - spot.x).powi(2) + (p.position.z - spot.z).powi(2)).sqrt());
            note(&mut report, &format!("spot{n}h{i}"), watch);
        }
        if best < 20.0 { report.stuck.push(spot); }
        // As high as the whole hull column stays free.
        let clear = level.world.sweep_box(spot, Vec3::new(24.0, 58.0, 24.0), Vec3::Y * 150.0, 0.1).map_or(150.0, |(t, _)| t * 150.0).max(0.0);
        let mut p = free_player(spot + Vec3::Y * clear); let mut watch = Watch::new(&p);
        for _ in 0..240 { p.tick(&level.world, &Input::default(), DT); watch.check(&level, &p); }
        if !p.grounded { report.fall_fail.push(spot); }
        note(&mut report, &format!("drop{n}"), watch);
    }

    // 5a. Stairs: wherever a flat tread 4..17 units higher lies 44 units from a reachable spot with the hull free above it, walking
    // at it (walk speed) has to climb it, and walking back has to come down again without leaving the ground for good.
    let grid = standable_points(&level, 40.0);
    for &spot in grid.iter().step_by((grid.len() / 900).max(1)) {
        for i in 0..8 {
            let angle = i as f32 * TAU / 8.0;
            let dir = Vec3::new(angle.cos(), 0.0, angle.sin());
            let far = spot + dir * 44.0;
            let Some((down, normal)) = level.world.raycast(Vec3::new(far.x, spot.y + 30.0, far.z), Vec3::NEG_Y, 120.0) else { continue };
            let tread = spot.y + 30.0 - down;
            let rise = tread - (spot.y - 58.0);
            if !(4.0..17.0).contains(&rise) || normal.y.abs() < 0.99 { continue; }
            let top = Vec3::new(far.x, tread + 58.06, far.z);
            let half = Vec3::new(16.0, 58.0, 16.0);
            if level.world.box_overlaps(top, half) || level.world.sweep_box(spot + Vec3::Y * (rise + 0.3), half, dir * 44.0, 0.05).is_some() || level.world.sweep_box(spot, half, Vec3::Y * rise, 0.05).is_some() { continue; }
            // A single riser: the tread 20 units before it is at the start height.
            let near = spot + dir * 20.0;
            if level.world.raycast(Vec3::new(near.x, spot.y + 30.0, near.z), Vec3::NEG_Y, 120.0).is_none_or(|(d, _)| (spot.y + 30.0 - d - (spot.y - 58.0)).abs() > 1.0) { continue; }
            report.stairs += 1;
            let mut p = free_player(spot); p.grounded = true;
            let mut watch = Watch::new(&p);
            let mut best = p.position.y;
            let mut inp = Input { forward: 1.0, yaw: -angle + std::f32::consts::FRAC_PI_2 * 0.0, ..Default::default() };
            // Input yaw convention: forward = (-sin yaw, -cos yaw); face `dir`.
            inp.yaw = (-dir.x).atan2(-dir.z);
            for _ in 0..40 { p.tick(&level.world, &inp, DT); watch.check(&level, &p); best = best.max(p.position.y); }
            let tag = format!("stair{}", report.stairs);
            note(&mut report, &tag, watch);
            if best < spot.y + rise - 0.6 { report.stairs_fail.push(spot); }
        }
    }

    // 5b. Jump apex: from open floor the jump rises 300^2 / 2000 = 45 units plus the discrete-step overshoot.
    for &spot in grid.iter().step_by((grid.len() / 40).max(1)) {
        if level.world.sweep_box(spot, Vec3::new(24.0, 58.0, 24.0), Vec3::Y * 120.0, 0.1).is_some() { continue; }
        let mut p = free_player(spot); settle(&level, &mut p, 3);
        if !p.grounded { continue; }
        let floor = p.position.y;
        let mut apex = floor;
        let mut input = Input { jump: true, ..Default::default() };
        for _ in 0..90 { p.tick(&level.world, &input, DT); input.jump = false; apex = apex.max(p.position.y); }
        report.jumps += 1; report.jump_apex = (report.jump_apex.0.min(apex - floor), report.jump_apex.1.max(apex - floor));
    }

    // 6. Crouching under low ceilings: floor-to-ceiling gaps that fit the crouched hull (48) but not the standing one (116).
    let mut low = 0;
    for &spot in visited.iter() {
        if low >= 24 { break; }
        for i in 0..8 {
            let probe = spot + Vec3::new((i as f32 * TAU / 8.0).cos(), 0.0, (i as f32 * TAU / 8.0).sin()) * (70.0 + 30.0 * (i % 3) as f32);
            let Some((down, _)) = level.world.raycast(probe, Vec3::NEG_Y, 300.0) else { continue };
            let floor = probe.y - down;
            let Some((up, _)) = level.world.raycast(Vec3::new(probe.x, floor + 0.5, probe.z), Vec3::Y, 200.0) else { continue };
            if !(52.0..112.0).contains(&up) { continue; }
            let centre = Vec3::new(probe.x, floor + 24.2, probe.z);
            if level.world.box_overlaps(centre, Vec3::splat(24.0)) { continue; }
            low += 1; report.low_spots += 1;
            // Crouched with the key released: the stand-up rays (108 up from the centre) hit the ceiling, so the hull
            // stays crouched, and it must still be able to walk.
            let mut p = free_player(centre); p.crouched = true; p.grounded = true;
            let mut watch = Watch::new(&p);
            let mut moved = 0.0f32;
            for tick in 0..60 { p.tick(&level.world, &Input { forward: 1.0, yaw: tick as f32 * 0.4, ..Default::default() }, DT); watch.check(&level, &p); moved = moved.max((p.position - centre).length()); }
            if !p.crouched && !level.world.can_stand(p.position) { report.low_fail.push(centre); }
            if p.crouched && moved < 1.0 { report.low_fail.push(centre); }
            note(&mut report, &format!("low{low}"), watch);
            break;
        }
    }
    report
}

#[test]
fn every_level_survives_scripted_movement() {
    let names = selected();
    if names.is_empty() { eprintln!("no exported levels, skipped"); return; }
    let mut failures = Vec::new();
    println!("{:<18} {:>6} {:>5} {:>5} {:>5} {:>5} {:>5} {:>5} {:>6} {:>7} {:>7} {:>5} {:>9} {:>11}", "level", "tris", "spawn", "spots", "stuck", "drop", "steps", "max^", "stairs", "jump lo", "jump hi", "low", "low fail", "open/emb/tun");
    for name in names {
        let r = run_level(&name);
        println!("{:<18} {:>6} {:>5} {:>5} {:>5} {:>5} {:>5} {:>5.1} {:>3}/{:<2} {:>7.1} {:>7.1} {:>5} {:>9} {:>4}/{}/{}", r.name, r.triangles, if r.spawn_ok { "ok" } else { "BAD" }, r.stuck_spots, r.stuck.len(), r.fall_fail.len(), r.steps, r.max_step, r.stairs - r.stairs_fail.len(), r.stairs,
            if r.jumps > 0 { r.jump_apex.0 } else { 0.0 }, r.jump_apex.1, r.low_spots, r.low_fail.len(), r.kinds[1], r.kinds[2], r.kinds[3]);
        let jump_bad = r.jumps > 0 && (r.jump_apex.0 < 40.0 || r.jump_apex.1 > 52.0);
        if !r.spawn_ok || r.kinds[0] + r.kinds[2] + r.kinds[3] > 0 || !r.stuck.is_empty() || !r.fall_fail.is_empty() || !r.low_fail.is_empty() || !r.stairs_fail.is_empty() || jump_bad { failures.push(r); }
    }
    for r in &failures {
        println!("-- {}: spawn_settle {:.1}", r.name, r.spawn_settle);
        for (tag, tick, v) in &r.violations { println!("   {tag} tick {tick}: {v:?}"); }
        for s in r.stuck.iter().take(5) { println!("   stuck at {s:?}"); }
        for s in r.fall_fail.iter().take(5) { println!("   drop failed at {s:?}"); }
        for s in r.low_fail.iter().take(5) { println!("   low ceiling trouble at {s:?}"); }
        for s in r.stairs_fail.iter().take(8) { println!("   stair not climbed near {s:?}"); }
    }
    assert!(failures.is_empty(), "{} level(s) with movement defects", failures.len());
}

// A level-less sanity check of the harness itself: a player dropped into a sealed box never leaves it.
#[test]
fn sealed_box_holds_the_player_at_any_frame_time() {
    let world = CollisionWorld::from_obj("v -300 0 -300\nv -300 0 300\nv 300 0 300\nv 300 0 -300\nv -300 300 -300\nv -300 300 300\nv 300 300 300\nv 300 300 -300\nf 1 2 3 4\nf 8 7 6 5\nf 1 5 6 2\nf 2 6 7 3\nf 3 7 8 4\nf 4 8 5 1").unwrap();
    for dt in [1.0 / 30.0, 1.0 / 60.0, 1.0 / 144.0, 0.05, 0.2] {
        let mut rng = Rng(7);
        let mut p = free_player(Vec3::new(0.0, 100.0, 0.0));
        let mut input = Input::default();
        for tick in 0..6000 {
            if tick % 20 == 0 { input = random_input(&mut rng); }
            p.tick(&world, &input, dt);
            assert!(p.position.x.abs() < 300.0 && p.position.z.abs() < 300.0 && p.position.y > 0.0 && p.position.y < 300.0, "dt {dt} tick {tick}: {:?}", p.position);
        }
    }
}
