//! Static world hull: exact swept-box queries against the exported PhysicsBSP triangles.
//! LithTech's own box-vs-BSP solver (Lithtech.exe 0x44eed0 ff.) is not reproduced; these queries
//! reproduce its observable contract (separating motion is free, penetrating motion is blocked).
use parry3d::{math::Vec3, query::{Ray, RayCast}, shape::{Triangle, TriMesh}};

/// A swept-box contact: fraction of the delta that is free, contact normal (pointing at the box) and the OBJ face.
#[derive(Clone, Copy, Debug)]
pub struct Hit { pub time: f32, pub normal: Vec3, pub triangle: u32, pub face: u32 }

/// Surface flag bits of the DAT (research/blender-lithtech-dat-import/test.h).
pub const SURF_NOTASTEP: u32 = 1 << 22;

pub struct CollisionWorld { mesh: TriMesh, faces: Vec<u32>, flags: Vec<u32> }
impl CollisionWorld {
    pub fn from_obj(source: &str) -> Result<Self, String> {
        let input = world_query::Mesh::from_obj_str(source)?;
        // world_query fans each OBJ face into `vertices-2` triangles in order.
        let source_faces = source.lines().filter_map(|line| line.strip_prefix("f ")).enumerate()
            .flat_map(|(face, rest)| std::iter::repeat_n(face as u32, rest.split_whitespace().count().saturating_sub(2)));
        let (mut vertices, mut indices, mut faces) = (Vec::new(), Vec::new(), Vec::new());
        for (t, face) in input.triangles().iter().zip(source_faces) {
            if t.normal().is_none() { continue; }
            let i = vertices.len() as u32;
            vertices.extend([t.a, t.b, t.c].map(|v| Vec3::new(v.x, v.y, v.z)));
            indices.push([i, i + 1, i + 2]); faces.push(face);
        }
        Ok(Self { mesh: TriMesh::new(vertices, indices).map_err(|e| e.to_string())?, faces, flags: Vec::new() })
    }
    /// DAT surface flags per OBJ `f` line (scene.json `collision_polygons`: `first_face`/`face_count`/`surface_flags`).
    pub fn set_surface_flags(&mut self, flags: Vec<u32>) { self.flags = flags; }
    pub fn surface_flags(&self, face: u32) -> u32 { self.flags.get(face as usize).copied().unwrap_or(0) }
    fn triangle_flags(&self, triangle: u32) -> u32 { self.surface_flags(self.faces.get(triangle as usize).copied().unwrap_or(0)) }

    /// Exact continuous sweep of an axis-aligned box (centre `position`, half extents `half`) along `delta`.
    /// The box is inflated by `skin`, so a hit stops `skin` away from the surface. Contacts that only
    /// separate or slide (`delta·normal >= 0`) never block; a box already inside a triangle blocks only inward motion.
    pub fn sweep(&self, position: Vec3, half: Vec3, delta: Vec3, skin: f32) -> Option<Hit> {
        if !(position.is_finite() && delta.is_finite()) { return None; }
        let extent = half + Vec3::splat(skin);
        let bounds = parry3d::bounding_volume::Aabb::new(position.min(position + delta) - extent, position.max(position + delta) + extent);
        let length = delta.length();
        let mut best: Option<Hit> = None;
        for index in self.mesh.bvh().intersect_aabb(&bounds) {
            let triangle = self.mesh.triangle(index);
            let Some((time, normal, _, _)) = box_triangle_interval(position, half, delta, &triangle, skin) else { continue };
            // A wall already touching the hull must not mask the floor in a downward probe:
            // separating and tangent contacts are ignored, every candidate is tested.
            if delta.dot(normal) >= -0.00001 * length { continue; }
            if best.is_none_or(|old| time < old.time) { best = Some(Hit { time, normal, triangle: index, face: self.faces.get(index as usize).copied().unwrap_or(0) }); }
        }
        best
    }
    /// Nearest walkable contact (normal.y above `GROUND_NORMAL`) within `distance` below the box; slopes steeper than that
    /// and walls beside the hull never mask a floor.
    pub fn ground(&self, position: Vec3, half: Vec3, distance: f32, skin: f32) -> Option<Hit> {
        let delta = Vec3::NEG_Y * distance;
        let extent = half + Vec3::splat(skin);
        let bounds = parry3d::bounding_volume::Aabb::new(position.min(position + delta) - extent, position.max(position + delta) + extent);
        let mut best: Option<Hit> = None;
        for index in self.mesh.bvh().intersect_aabb(&bounds) {
            let Some((time, normal, _, _)) = box_triangle_interval(position, half, delta, &self.mesh.triangle(index), skin) else { continue };
            if normal.y < crate::GROUND_NORMAL || delta.dot(normal) >= -0.00001 * distance { continue; }
            if best.is_none_or(|old| time < old.time) { best = Some(Hit { time, normal, triangle: index, face: self.faces.get(index as usize).copied().unwrap_or(0) }); }
        }
        best
    }
    /// True when the surface the contact hit refuses stair steps (DAT `NotAStep`, Lithtech.exe 0x416c60).
    pub fn no_step(&self, hit: &Hit) -> bool { self.triangle_flags(hit.triangle) & SURF_NOTASTEP != 0 }

    /// A native-unit sweep for alternate controllers (doors, NPC blockers). Does not change retail's cast rules.
    pub fn sweep_box(&self, position: Vec3, half: Vec3, delta: Vec3, skin: f32) -> Option<(f32, Vec3)> { self.sweep(position, half, delta, skin).map(|h| (h.time, h.normal)) }

    /// Deepest penetration of the box into the hull: depth and the direction that pushes it out.
    pub fn penetration(&self, position: Vec3, half: Vec3) -> Option<(f32, Vec3)> {
        let bounds = parry3d::bounding_volume::Aabb::new(position - half, position + half);
        let mut best: Option<(f32, Vec3)> = None;
        for index in self.mesh.bvh().intersect_aabb(&bounds) {
            if let Some((_, normal, depth, _)) = box_triangle_interval(position, half, Vec3::ZERO, &self.mesh.triangle(index), 0.0) {
                if depth > 0.0 && best.is_none_or(|(d, _)| depth > d) { best = Some((depth, normal)); }
            }
        }
        best
    }
    /// True only for penetration, so a hull touching the floor can still change stance.
    pub fn box_overlaps(&self, position: Vec3, half: Vec3) -> bool { self.penetration(position, half).is_some_and(|(depth, _)| depth > 0.01) }
    /// Pushes an embedded box out along the minimum translation of its deepest contact (at most `limit` units in total).
    /// Returns the corrected position, or None when it stays embedded (a closed solid).
    pub fn depenetrate(&self, mut position: Vec3, half: Vec3, limit: f32) -> Option<Vec3> {
        let start = position;
        for _ in 0..24 {
            let Some((depth, normal)) = self.penetration(position, half) else { return Some(position) };
            if depth <= 0.01 { return Some(position); }
            position += normal * (depth + 0.02);
            if (position - start).length() > limit { return None; }
        }
        (!self.box_overlaps(position, half)).then_some(position)
    }
    /// Whether penetrations cover the entire path, including between different surfaces.
    pub fn box_sweep_allsolid(&self, position: Vec3, half: Vec3, delta: Vec3) -> bool {
        let bounds = parry3d::bounding_volume::Aabb::new(position.min(position + delta) - half, position.max(position + delta) + half);
        let mut intervals = Vec::new();
        for index in self.mesh.bvh().intersect_aabb(&bounds) {
            if let Some((enter, _, _, leave)) = box_triangle_interval(position, half, delta, &self.mesh.triangle(index), -0.01) { intervals.push((enter, leave.min(1.0))); }
        }
        intervals.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        let mut covered = 0.0_f32;
        for (enter, leave) in intervals {
            if enter > covered { return false; }
            covered = covered.max(leave);
            if covered >= 1.0 { return true; }
        }
        false
    }
    /// First static-world hit, returning native distance and a world-space normal.
    pub fn raycast(&self, origin: Vec3, direction: Vec3, max_distance: f32) -> Option<(f32, Vec3)> {
        if !origin.is_finite() || !direction.is_finite() || !max_distance.is_finite() || max_distance <= 0.0 { return None; }
        let direction = direction.try_normalize()?;
        self.mesh.cast_local_ray_and_get_normal(&Ray::new(origin, direction), max_distance, false).map(|hit| (hit.time_of_impact, hit.normal))
    }
    /// `raycast` plus the zero-based OBJ face (`f` line) that was hit, for per-face surface data.
    pub fn raycast_face(&self, origin: Vec3, direction: Vec3, max_distance: f32) -> Option<(f32, Vec3, usize)> {
        if !origin.is_finite() || !direction.is_finite() || !max_distance.is_finite() || max_distance <= 0.0 { return None; }
        let direction = direction.try_normalize()?;
        let hit = self.mesh.cast_local_ray_and_get_normal(&Ray::new(origin, direction), max_distance, false)?;
        // parry reports back faces as `triangle + triangle_count`.
        let triangle = match hit.feature { parry3d::shape::FeatureId::Face(id) => id as usize % self.faces.len().max(1), _ => 0 };
        Some((hit.time_of_impact, hit.normal, self.faces.get(triangle).copied().unwrap_or(0) as usize))
    }
    /// Retail 0x100616f0: five upward rays, 108 units long, at the centre and the four corners (±24).
    pub fn can_stand(&self, position: Vec3) -> bool {
        [(0.0, 0.0), (-24.0, -24.0), (-24.0, 24.0), (24.0, 24.0), (24.0, -24.0)].iter().all(|&(x, z)|
            self.mesh.cast_local_ray(&Ray::new(position + Vec3::new(x, 0.0, z), Vec3::Y), 108.0, false).is_none())
    }
    /// Places a spawning player: a fresh object is grown into the free space around its start (0x452090), which lifts a
    /// start that overlaps the floor onto it.
    pub fn place_player(&self, player: &mut crate::Player) {
        let ray = Ray::new(player.position + Vec3::Y * 18.0, Vec3::NEG_Y);
        if let Some(hit) = self.mesh.cast_local_ray_and_get_normal(&ray, 94.0, false) {
            if hit.normal.y.abs() > crate::GROUND_NORMAL {
                let floor = ray.origin.y - hit.time_of_impact;
                if player.position.y - floor < 58.1 { player.position.y = floor + 58.1; }
            }
        }
    }
}

// Continuous separating-axis test for a translating axis-aligned box and a static triangle. Exact
// face/edge axes avoid iterative GJK contact drift on large native-level triangles; the mesh BVH keeps
// candidate counts local. Returns (entry time in 0..1, entry normal, penetration depth, exit time).
pub(crate) fn box_triangle_interval(position: Vec3, half: Vec3, delta: Vec3, triangle: &Triangle, skin: f32) -> Option<(f32, Vec3, f32, f32)> {
    let vertices = [triangle.a - position, triangle.b - position, triangle.c - position];
    let edges = [vertices[1] - vertices[0], vertices[2] - vertices[1], vertices[0] - vertices[2]];
    let mut axes = [Vec3::ZERO; 13];
    axes[..3].copy_from_slice(&[Vec3::X, Vec3::Y, Vec3::Z]);
    axes[3] = edges[0].cross(edges[1]);
    for (i, edge) in edges.iter().enumerate() { for (j, basis) in [Vec3::X, Vec3::Y, Vec3::Z].iter().enumerate() { axes[4 + i * 3 + j] = edge.cross(*basis); } }
    let (mut enter, mut leave) = (f32::NEG_INFINITY, f32::INFINITY);
    let mut enter_normal = Vec3::ZERO;
    let mut separation = f32::NEG_INFINITY;
    let mut closest_normal = Vec3::ZERO;
    for axis in axes {
        if axis.length_squared() < 1.0e-12 { continue; }
        let axis = axis.normalize();
        let projected = vertices.map(|v| v.dot(axis));
        let radius = half.dot(axis.abs()) + skin;
        let low = projected[0].min(projected[1]).min(projected[2]) - radius;
        let high = projected[0].max(projected[1]).max(projected[2]) + radius;
        let axis_separation = low.max(-high);
        if axis_separation > separation { separation = axis_separation; closest_normal = if low > -high { -axis } else { axis }; }
        let speed = delta.dot(axis);
        if speed.abs() < 1.0e-8 { if low > 0.0 || high < 0.0 { return None; } continue; }
        let (near, far, normal) = if speed > 0.0 { (low / speed, high / speed, -axis) } else { (high / speed, low / speed, axis) };
        if near > enter { enter = near; enter_normal = normal; }
        leave = leave.min(far);
        if enter > leave { return None; }
    }
    if leave < 0.0 || enter > 1.0 { return None; }
    Some((enter.max(0.0), if enter < 0.0 { closest_normal } else { enter_normal }, -separation, leave))
}
