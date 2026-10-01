//! Retail control rules in native LithTech units, independent of the renderer.
//! Evidence and the verified/approximated table: docs/retail-movement.md, docs/retail-movement-audit.md.
mod world;
mod plan;
mod player;
pub use parry3d::math::Vec3;
pub use world::{CollisionWorld, Hit, SURF_NOTASTEP};
pub use player::{Events, Input, Player};

/// A surface is walkable/steppable when its normal.y exceeds 0.7071 (Lithtech.exe 0x569950, used at 0x416c86).
pub const GROUND_NORMAL: f32 = 0.7071;
/// `SetStairHeight(18)` (cshell 0x1005f836 through the physics vtable slot 3, Lithtech.exe 0x4aa960).
pub const STAIR_HEIGHT: f32 = 18.0;
