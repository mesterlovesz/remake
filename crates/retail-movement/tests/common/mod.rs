//! Shared helpers for the level-wide movement harness: loads the exported collision data of
//! every retail level (headless, no window, no sound) and drives `Player` with scripted input.
#![allow(dead_code)]
use retail_movement::{CollisionWorld, Input, Player, Vec3};
use std::path::{Path, PathBuf};

pub const DT: f32 = 1.0 / 60.0;

pub fn output_dir() -> PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output") }

/// Every exported level (`<world>.collision.obj`, movable brushes excluded), sorted.
pub fn level_names() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(output_dir()).map(|dir| dir.filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".collision.obj")).filter(|n| !n.ends_with(".movable")).map(str::to_owned)).collect()).unwrap_or_default();
    names.sort();
    names
}

pub struct Level { pub name: String, pub world: CollisionWorld, pub spawn: Vec3, pub yaw: f32, pub min: Vec3, pub max: Vec3, pub triangles: usize }

/// Vertices referenced by faces give the true extent of the collision hull.
fn bounds(source: &str) -> (Vec3, Vec3, usize) {
    let mut vertices = Vec::new();
    let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    let mut triangles = 0;
    for line in source.lines() {
        let mut words = line.split_whitespace();
        match words.next() {
            Some("v") => { let v: Vec<f32> = words.take(3).filter_map(|w| w.parse().ok()).collect(); vertices.push(Vec3::new(v[0], v[1], v[2])); }
            Some("f") => { let idx: Vec<usize> = words.filter_map(|w| w.split('/').next()?.parse::<usize>().ok()).collect(); triangles += idx.len().saturating_sub(2);
                for i in idx { let v = vertices[i - 1]; min = min.min(v); max = max.max(v); } }
            _ => {}
        }
    }
    (min, max, triangles)
}

/// Closed door/drawer brushes (b_door, solid b_szuflada) from the movable export, appended to the static hull:
/// the static PhysicsBSP leaves their doorways open, the game fills them with the door models.
fn with_closed_doors(mut source: String, movable: &str, scene: &serde_json::Value) -> String {
    let solid_models: Vec<String> = scene["objects"].as_array().unwrap().iter()
        .filter(|o| { let kind = o["kind"].as_str().unwrap_or(""); (kind == "b_door" || kind.starts_with("b_szuflada")) && o["properties"]["Solid"].as_i64() != Some(0) })
        .filter_map(|o| o["properties"]["Name"].as_str().map(str::to_owned)).collect();
    let base = source.lines().filter(|l| l.starts_with("v ")).count();
    let (mut vertices, mut faces, mut keep, mut offset) = (Vec::<String>::new(), Vec::<Vec<usize>>::new(), false, 0usize);
    let mut local = Vec::<usize>::new(); let mut global = 0usize;
    for line in movable.lines() {
        if let Some(name) = line.strip_prefix("o ") { keep = solid_models.iter().any(|m| m.eq_ignore_ascii_case(name)); offset = vertices.len(); local.clear(); continue; }
        if line.starts_with("v ") { global += 1; if keep { local.push(global); vertices.push(line.to_owned()); } continue; }
        if keep && line.starts_with("f ") {
            let idx: Vec<usize> = line[2..].split_whitespace().map(|w| { let g: usize = w.parse().unwrap(); base + offset + local.iter().position(|&x| x == g).unwrap() + 1 }).collect();
            faces.push(idx);
        }
    }
    for v in vertices { source.push_str(&v); source.push('\n'); }
    // The appended vertices follow the static ones, but faces of the static hull stay valid: OBJ indices are absolute.
    for f in faces { source.push_str(&format!("f {}\n", f.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(" "))); }
    source
}

pub fn load(name: &str) -> Level {
    let dir = output_dir();
    let mut source = std::fs::read_to_string(dir.join(format!("{name}.collision.obj"))).expect("collision obj");
    let scene: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join(format!("{name}.scene.json"))).expect("scene")).expect("scene json");
    if std::env::var_os("MOVE_STATIC_ONLY").is_none() {
        if let Ok(movable) = std::fs::read_to_string(dir.join(format!("{name}.movable.collision.obj"))) { source = with_closed_doors(source, &movable, &scene); }
    }
    let world = CollisionWorld::from_obj(&source).expect("collision world");
    let start = scene["objects"].as_array().unwrap().iter().find(|o| o["kind"] == "StartPoint").expect("StartPoint");
    let pos = start["properties"]["Pos"].as_array().unwrap();
    let spawn = Vec3::new(pos[0].as_f64().unwrap() as f32, pos[1].as_f64().unwrap() as f32, pos[2].as_f64().unwrap() as f32);
    // main.rs: LithTech +Z forward vs Bevy -Z, the player yaw is the start rotation + pi.
    let yaw = start["properties"]["Rotation"][1].as_f64().unwrap_or(0.0) as f32 + std::f32::consts::PI;
    let (min, max, triangles) = bounds(&source);
    Level { name: name.to_owned(), world, spawn, yaw, min, max, triangles }
}

/// Small deterministic generator so failures reproduce.
pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u32 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; (self.0 >> 16) as u32 }
    pub fn unit(&mut self) -> f32 { (self.next() & 0xffffff) as f32 / 16777216.0 }
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 { lo + (hi - lo) * self.unit() }
    pub fn chance(&mut self, p: f32) -> bool { self.unit() < p }
}

/// What a tick may not do, checked after every step.
#[derive(Debug, Clone, PartialEq)]
pub enum Violation { NotFinite, FellOut { y: f32 }, Embedded { depth_probe: Vec3 }, Tunnelled { from: Vec3, to: Vec3 } }

impl Violation { pub fn index(&self) -> usize { match self { Violation::NotFinite => 0, Violation::FellOut { .. } => 1, Violation::Embedded { .. } => 2, Violation::Tunnelled { .. } => 3 } } }

pub struct Watch { pub previous: Vec3, pub violations: Vec<(usize, Violation)>, pub ticks: usize }
impl Watch {
    pub fn new(player: &Player) -> Self { Self { previous: player.position, violations: Vec::new(), ticks: 0 } }
    /// `previous` is the position before the tick, `player` the state after it.
    pub fn check(&mut self, level: &Level, player: &Player) {
        self.ticks += 1;
        let p = player.position;
        let record = |w: &mut Self, v: Violation| { let t = w.ticks; w.violations.push((t, v)); };
        if !(p.is_finite() && player.velocity.is_finite()) { record(self, Violation::NotFinite); self.previous = p; return; }
        if p.y < level.min.y - 300.0 { record(self, Violation::FellOut { y: p.y }); }
        if level.world.box_overlaps(p, player.half_size() - Vec3::splat(0.5)) { record(self, Violation::Embedded { depth_probe: p }); }
        let delta = p - self.previous;
        let distance = delta.length();
        if distance > 0.05 && distance < 400.0 {
            if let Some((t, _)) = level.world.raycast(self.previous, delta / distance, distance) { if t < distance - 0.05 { record(self, Violation::Tunnelled { from: self.previous, to: p }); } }
        }
        self.previous = p;
    }
}

pub fn spawn_player(level: &Level) -> Player {
    let mut p = Player::new(level.spawn);
    level.world.place_player(&mut p);
    p
}

/// A player that is already past the 60 frozen start frames.
pub fn free_player(position: Vec3) -> Player { let mut p = Player::new(position); p.frames = 60; p }

pub fn settle(level: &Level, player: &mut Player, ticks: usize) { for _ in 0..ticks { player.tick(&level.world, &Input::default(), DT); } }

/// Forward/right axes that walk along world direction `angle` (radians, 0 = -Z) with the given yaw convention of `Player::tick`.
pub fn walk_input(angle: f32, run: bool) -> Input { Input { forward: 1.0, right: 0.0, yaw: angle, run, crouch: false, jump: false } }

/// Every standable spot of the level on a grid: the hull fits, a floor lies under the feet.
pub fn standable_points(level: &Level, spacing: f32) -> Vec<Vec3> {
    let mut points = Vec::new();
    let top = level.max.y + 10.0;
    let mut x = level.min.x + spacing * 0.5;
    while x < level.max.x {
        let mut z = level.min.z + spacing * 0.5;
        while z < level.max.z {
            let mut y = top;
            for _ in 0..12 {
                let Some((distance, normal)) = level.world.raycast(Vec3::new(x, y, z), Vec3::NEG_Y, y - level.min.y + 10.0) else { break };
                let floor = y - distance;
                if normal.y > 0.8 {
                    let centre = Vec3::new(x, floor + 58.1, z);
                    if !level.world.box_overlaps(centre, Vec3::new(24.0, 58.0, 24.0)) { points.push(centre); }
                }
                y = floor - 1.0;
                if y < level.min.y { break; }
            }
            z += spacing;
        }
        x += spacing;
    }
    points
}
