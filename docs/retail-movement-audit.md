# Retail movement audit

Player movement, physics and collision of the 2002 game, checked against the installed binaries and the exported worlds.
Addresses are virtual addresses: `cshell.dll` (client shell, base 0x10000000), `Lithtech.exe` (engine, base 0x400000, marked
`Lithtech.exe`) and `object.lto` (server objects, base 0x10000000, marked `object.lto`). Reproduce any range with
`python -m tools.inspect_retail START END [--module Lithtech.exe|object.lto]` (disassembly with float immediates annotated).
Earlier notes: `retail-movement.md` (constant table), `retail-camera.md` (eye, bob, roll, death camera).

Status words: **verified** = read from the code and reproduced; **approximated** = behaviour known, algorithm not reproduced;
**absent** = the retail game has no such thing.

## How the retail controller is built

The player is a client object (created at cshell 0x1005f6f0). cshell integrates its own velocity every frame and hands the
engine only `velocity` (physics slot 11, SetVelocity); it **never reads the velocity back** (GetVelocity, slot 10, is not used by
the controller), so a collision does not change it. Per frame:

1. `0x10060160` input: speed limits, stance, accumulate velocity from the keys, jump.
2. `0x10060dd0` frame: fall damage, gravity, friction, speed clamp, stamina, `SetVelocity`, footsteps, then
3. `0x100618f0` move: zero velocity while the level has run fewer than 60 frames (`[shell+0x37c] < 0x3c`, 0x10061906) or the frame
   took more than 1.5 s (0x10061940); else `UpdateMovement` (slot 20, 0x100619ad) returns `velocity x dt`, cshell clamps it to
   `clamp(0.75 x largest box half extent, 4, 16)` (0x10061a30) and calls `MoveObject` (slot 16, 0x10061b85) with the new position.

The engine's part is a box-vs-BSP move (`MoveObject`, Lithtech.exe 0x44eed0; box resize 0x451c40/0x452090). A dead controller
(`[controller+0xc8]`) skips input and physics entirely (0x10060f5f): the body stays where it died, only the death camera runs.

Physics slots used (vtable Lithtech.exe 0x56b868, ILTPhysics client): 3 `SetStairHeight` (0x4aa960, called with 18 at 0x1005f836 and by
every stance change, 0x1005f6d7), 9 `SetObjectDims` (0x4aa4a0), 11 `SetVelocity` (0x4aa1b0), 16 `MoveObject`, 17 `GetStandingOn`
(0x4ab1c0, reads obj+0x12c/0x130), 19 `SetGlobalForce` (called with 0,0,0 at 0x1005f839: no engine gravity), 20 `UpdateMovement`
(0x4aa400 -> 0x44e050: `v x dt + a x dt^2 / 2`, friction only when obj+0xf8 > 0, which stays 0). Gravity is cshell's own.

## Verified against the retail code

| Topic | Retail rule | Address | Remake |
|---|---|---|---|
| Walk speed | 180 u/s (`[controller+0x140]`; +0x144 = 300 is never read) | 0x100601a1 | `player.rs` |
| Run speed | 180 + 120 x min(stamina/max + 0.2, 1); Shift XOR the Q toggle; exhausted or crouched = walk | 0x100601b5..0x10060210, 0x10060633, 0x10060674 | verified |
| Crouch speed | 0.5 x 180 = 90, running flags cleared | 0x100606fe | verified |
| Acceleration | 9000 u/s^2 x dt per key along the yaw-only frame of the camera (forward and strafe add, no normalising), x 0.05 in the air; the air/ground choice is the standing state of the frame start | 0x100607a0, 0x10066564 | verified |
| Direction | the camera object's rotation of the previous frame, levelled (pitch removed): drunk wobble steers the walk | 0x100601ea..0x10060410 | `move_camera` reads the camera transform |
| Ground friction | horizontal velocity x (0.95 - dt) x 0.45 per frame, only standing and with no direction key; below 1 u/s it is zeroed | 0x10061112..0x1006119c | verified (frame dependent like retail) |
| Speed clamp | horizontal magnitude limited to the current limit (walk/run/crouch), also in the air | 0x100611b9 | verified |
| Gravity | -1000 u/s^2 only when not standing; standing clamps vy to >= 0; terminal -640 | 0x1006108d, 0x100610c0 | verified |
| Jump | needs standing (fresh query), no latch, not exhausted: vy + 300, cap 450, stamina -5, `skok.wav`; the jump frame is still standing (no gravity that frame); the latch clears when vy <= 0 | 0x10060ba0..0x10060c08, 0x100610dd | verified: 47.5 u apex at 60 fps |
| Double jump on touch-down | **retail's own**: the jump runs in the input stage on the velocity the last frame left (still downward after a fall of more than 45 units: vy + 300 <= 0), the frame function then clamps a standing body's vy to >= 0 (0x100610a7..0x100610ba) and clears the latch because vy <= 0 (0x100610dd..0x100610f6), so the next frame jumps again for real: two `skok.wav`, 2 x 5 stamina, two frames apart, while the key is held; a short fall jumps once | 0x10060b92..0x10060c08 before 0x10060dd0 | verified (`solver.rs` test); the earlier explanation (the standing query accepting a hull a few units above the floor) is not needed |
| Stance | crouch shrinks the box in place to 24/24/24; standing lifts it 32 and grows it to 24/58/24; each change adds 64 to vy, also in the air; the first change also widens 16 to 24 | 0x100606a2..0x100606eb, 0x1005f5f0, 0x1005f7ba | verified; the resize fit is approximated (below) |
| Stand clearance | five rays 108 up from the centre and from the four corners (+-24) | 0x100616f0 | verified |
| Stamina | drain 4/s running on the ground, +0.5/s moving, +5.5/s otherwise, jump -5 (the frame runs the update twice), exhausted below 10, recovered above 15 | 0x10061e00 | verified |
| Footsteps | every 0.4 s while running and moving on the ground; the timer starts at 0 (first step on the first frame); surface from a ray +32 / -256 | 0x1006131b, 0x100614e0 | verified, no sound played (`Player.footstep`) |
| Fall damage | after 60 frames: (takeoff - landing - 196) x 0.15; takeoff = start y, reset to the height where the ground was left and again at the jump apex (jump start sets -1e8) | 0x1006102f, 0x1005b759 | verified (`Player.events.fall_damage`; `spad.wav` is played by view.rs) |
| Start | 60 frozen frames; the shell message 0x31 carries position and `Kierunek`: yaw 0, pi/2, pi, 3pi/2 for polnoc, wschod, poludnie, zachod; the StartPoint `Rotation` is not used; no StartPoint = (800, 128, 448) | 0x1005b6df, object.lto 0x10011300..0x100117af | verified; changes the initial view of 10 levels (podziemia1b, rh1-wiezienie1, rh1-wiezienie2, rh10-wiezowiec1, rh2-wiezienie2, rh3-miasteczko0/1, wiez_wn1/2/3) |
| Hull | 16/58/16 at creation, 24/58/24 or 24/24/24 after the first stance change (an AABB, no capsule) | 0x1005f7ba, 0x1005f5f0 | verified |
| Stair height | 18 for both stances | 0x1005f836 | verified constant |
| Walkable / steppable plane | normal.y > 0.7071 (about 45 degrees) | Lithtech.exe 0x569950 used at 0x416c86 | verified constant (was 0.65) |
| NotAStep | DAT surface flag 1<<22 refuses stepping onto the polygon | Lithtech.exe 0x416c60 | flags reach the world (`collision_polygons`); 144 faces in 4 levels |
| Per-frame move clamp | 16 for this hull | 0x10061a30 | verified |
| NPC sight target | centre + 16 crouched, + 48 standing | 0x10043626 | for the NPC code, not used here |

## Approximated

| Topic | What is known | Remake |
|---|---|---|
| Box vs BSP solver | engine `MoveObject` walks the PhysicsBSP (0x416820 family, plane tests with the constants above); the exact order of contacts, its tolerances and step-down are not reproduced | exact swept-AABB SAT against the exported triangles, Quake-style plane clipping, step = up/forward/down sweeps of at most 18 |
| Standing on | the engine sets obj+0x12c/0x130 when a move ends on a surface | a 0.3 unit down probe on a walkable normal; standing on a door, prop or corpse is inferred when a correction pass lifts a falling hull (four ticks, one test fall) or `Player.external_support` is set |
| Box resize | growth per axis moves -g then +2g and centres the box in the span (0x452090); it fails when the span is short | both sides are measured, so a wall on either side shifts the box; a failed stance change is refused instead of leaving the state flag and the box out of step |
| Spawn on the floor | the object is created at the StartPoint centre and the resize fit lifts it out of the floor | `place_player` lifts a start that overlaps a floor within 58 units; the 60 frozen frames then let a hovering start fall |
| Unstick | none in cshell; the resize fit is the only push-out | embedded hulls (level start, a closing door) are pushed out along the minimum translation before each move |

Not reproduced on purpose: cshell's own copy of the box size (controller+0xb0..0xb8) is written only by the stance change and lives
in `.bss` (the shell is a static object, 0x10b3abf8), so **until the first crouch the frame clamp is 4 units, not 16**: run 240
u/s at 60 fps, walk 120 u/s at 30 fps. Retail's `MaxFPS 160` hides it; set `MESTER_STEP_CLAMP_QUIRK=1` to reproduce it
(`Player.initial_clamp_quirk`).

## Absent in retail

| Thing | Evidence |
|---|---|
| Ladders, climbing | no string or code in cshell.dll, object.lto or server.dll; no such object class |
| Swimming, wading, buoyancy | no water strings or code; the worlds have no VolumeBrush objects. The water surfaces (rh3-miasteczko0 river, RH9-fabryka pool, Rh7a-Tunele) are **solid** PhysicsBSP polygons (flags 0x67001081, SurfaceFlags 4) and the footstep table plays `krokm` (metal) for SurfaceFlags 4: retail walks on them |
| Water damage / drowning | none; the remake's instant death on a water surface (`campaign.rs`, `water.rs`) is not from the retail code (open question, below) |
| Elevators, moving platforms, conveyors | the class `b_winda` exists in object.lto but no shipped world uses it (the `winda*` objects are sliding `b_szuflada` doors); no conveyor |
| Lean | strafe roll only (retail-camera.md) |
| Surface-dependent speed or friction | the surface flag only selects the footstep sound |
| Noise events for NPC hearing | the footstep is a local 2D sound (0x100598b0, no server message); NPCs see (`kat_kontaktu`, `odleglosc_kontaktu`) and hear stimuli through the weapon and detector paths, not through the player's movement |
| Super jump | command 0x14 adds 1500 to vy (0x10060c14) but keys.cfg binds nothing to it |

## Level data

* **StartPoints**: every level has one `StartPoint0`; all 29 start within 50 units above a floor (the largest fall is rh12-lab1, 50 units,
  no damage) and four overlap the floor by 1..11 units (rh1-wiezienie1, rh1-wiezienie3, rh12-lab2, rh3-miasteczko1).
* **Collision export** (`tools/export_world.py`): the PhysicsBSP polygons flagged SOLID; the 348 excluded polygons all carry
  0x200104 (invisible, no-subdivide, vis-blocker) and are visibility hulls. No polygon uses PHYSICSBLOCKER (1<<17): retail has no
  invisible walls except closed doors. Doors and sliders (`b_door`, `b_szuflada`) are separate models in `*.movable.collision.obj`;
  their doorways are open in the static hull.
* Faces with normal.y between 0.65 and 0.7071 (356 of about 450,000) are no longer walkable (they were with the old 0.65).
* No defect was found in the export by the harness: no hole, sliver or missing floor stopped a probe. Falling off open edges into the
  void exists (rh10-wiezowiec1 scaffolding) and is level design.

## Harness

`crates/retail-movement/tests/levels.rs` loads every exported world (static hull plus the closed door and drawer brushes, headless,
no window, no sound) and runs, per level: the spawn (frozen 60 frames, settles on the floor), 32 compass walks and runs, ten fuzz
runs (random walk, run, jump, crouch; 60 fps with 0.05 s spikes, fixed 30 fps and 144 fps), stuck probes and drops from the visited
spots, every tread 4..17 units high on a 40 unit grid (it has to be climbed), jumps on open floor (apex 40..52) and crouching under
floor-to-ceiling gaps of 52..112. After every tick it checks: finite state, no embedding (deeper than 0.5), no tunnelling (the centre
ray crosses no triangle). `MOVE_LEVEL=name`, `MOVE_SAMPLES=n` restrict it; `cargo test` in `crates/retail-movement` runs it in about
30 s. `tests/solver.rs` holds the synthetic checks (ramps 30/60 degrees, corners, thin walls at 1/240..1 s frames, NotAStep, jump
and ceiling, fall damage and the apex rule, stance fit, speeds and friction, frame clamp, embedding, standing on a dynamic solid).

`MESTER_MOVE_PROBE=1 MESTER_SILENT=1 level-viewer <world> ../../output <capture.png> 55` scripts keys in the real game (frozen
start, walk, run, jump, crouch-walk, stand) and logs the controller; the run showed 180 u/s walking, 300 running with the stamina
draining about 4/s, 90 crouched and the start freeze of 60 frames.

Result of the last full run (spots = probe spots, open drops = falls below the level, both open drops/embedded/tunnelled):

| level | collision tris | spawn | spots probed | stuck | drop fails | stair climbs (ok/found) | step-ups in fuzz (max rise) | jump apex | low ceilings tried | open drops / embedded / tunnelled |
|---|---|---|---|---|---|---|---|---|---|---|
| RH9-fabryka | 38392 | ok | 140 | 0 | 0 | 3/3 | 0 (0.0) | 47.5 | 0 (0 failed) | 0/0/0 |
| Rh7a-Tunele | 25724 | ok | 141 | 0 | 0 | 29/29 | 0 (0.0) | 47.5 | 0 (0 failed) | 0/0/0 |
| burmistrz1 | 12988 | ok | 130 | 0 | 0 | 101/101 | 163 (16.0) | 47.5 | 0 (0 failed) | 0/0/0 |
| burmistrz2 | 8420 | ok | 131 | 0 | 0 | 113/113 | 99 (16.0) | 47.5 | 0 (0 failed) | 0/0/0 |
| chapel_mniejszy | 35652 | ok | 133 | 0 | 0 | 24/24 | 201 (17.6) | 47.5 | 24 (0 failed) | 0/0/0 |
| chinatown | 13208 | ok | 138 | 0 | 0 | 5/5 | 0 (0.0) | 47.5 | 0 (0 failed) | 0/0/0 |
| chinatown2 | 19224 | ok | 125 | 0 | 0 | 6/6 | 169 (18.6) | 47.5 | 0 (0 failed) | 0/0/0 |
| knajpa | 23596 | ok | 127 | 0 | 0 | 6/6 | 0 (0.0) | 47.5 | 0 (0 failed) | 0/0/0 |
| nic | 5004 | ok | 136 | 0 | 0 | 0/0 | 0 (0.0) | 47.5 | 0 (0 failed) | 0/0/0 |
| podziemia1 | 7418 | ok | 141 | 0 | 0 | 0/0 | 0 (0.0) | 47.5 | 0 (0 failed) | 0/0/0 |
| podziemia1a | 5232 | ok | 130 | 0 | 0 | 3/3 | 1 (6.9) | 47.5 | 0 (0 failed) | 0/0/0 |
| podziemia1b | 5700 | ok | 129 | 0 | 0 | 3/3 | 84 (16.0) | 47.5 | 18 (0 failed) | 0/0/0 |
| podziemia1c | 10368 | ok | 137 | 0 | 0 | 10/10 | 0 (0.0) | 47.5 | 0 (0 failed) | 0/0/0 |
| rh1-wiezienie1 | 15924 | ok | 138 | 0 | 0 | 0/0 | 9 (3.1) | 47.5 | 24 (0 failed) | 0/0/0 |
| rh1-wiezienie2 | 12292 | ok | 137 | 0 | 0 | 16/16 | 8 (3.1) | 47.5 | 24 (0 failed) | 0/0/0 |
| rh1-wiezienie3 | 15684 | ok | 143 | 0 | 0 | 5/5 | 0 (0.0) | 47.5 | 0 (0 failed) | 0/0/0 |
| rh10-wiezowiec1 | 16552 | ok | 132 | 0 | 0 | 15/15 | 23 (8.0) | 47.5 | 0 (0 failed) | 1343/0/0 |
| rh10-wiezowiec2 | 20856 | ok | 148 | 0 | 0 | 37/37 | 0 (0.0) | 47.5 | 17 (0 failed) | 0/0/0 |
| rh10-wiezowiec3 | 19320 | ok | 148 | 0 | 0 | 18/18 | 0 (0.0) | 47.5 | 24 (0 failed) | 0/0/0 |
| rh12-lab1 | 18628 | ok | 142 | 0 | 0 | 22/22 | 0 (0.0) | 47.5 | 2 (0 failed) | 0/0/0 |
| rh12-lab2 | 10880 | ok | 142 | 0 | 0 | 0/0 | 0 (0.0) | 47.5 | 3 (0 failed) | 0/0/0 |
| rh2-wiezienie1 | 11800 | ok | 142 | 0 | 0 | 10/10 | 0 (0.0) | 47.5 | 0 (0 failed) | 0/0/0 |
| rh2-wiezienie2 | 13608 | ok | 142 | 0 | 0 | 9/9 | 0 (0.0) | 47.5 | 0 (0 failed) | 0/0/0 |
| rh3-miasteczko0 | 24180 | ok | 140 | 0 | 0 | 1/1 | 56 (16.0) | 47.5 | 0 (0 failed) | 0/0/0 |
| rh3-miasteczko1 | 17308 | ok | 148 | 0 | 0 | 1/1 | 0 (0.0) | 47.5 | 0 (0 failed) | 0/0/0 |
| rh3-miasteczko2 | 20112 | ok | 137 | 0 | 0 | 1/1 | 92 (8.0) | 47.5 | 0 (0 failed) | 0/0/0 |
| wiez_wn1 | 7600 | ok | 144 | 0 | 0 | 0/0 | 1 (12.4) | 47.5 | 0 (0 failed) | 0/0/0 |
| wiez_wn2 | 10516 | ok | 142 | 0 | 0 | 0/0 | 2 (15.9) | 47.5 | 0 (0 failed) | 0/0/0 |
| wiez_wn3 | 10012 | ok | 144 | 0 | 0 | 0/0 | 0 (0.0) | 47.5 | 0 (0 failed) | 0/0/0 |

## Open questions

* The drowning death on water surfaces is not in the retail code (see above); decision for the owner.
* The engine solver is not reproduced instruction by instruction; a retail input recording (positions per frame) would allow a
  numeric comparison of slides, steps and standing-on.
* Solidity of `b_transparent*` models at run time (they are exported as movable brushes and not blocking).
* Doors, props and NPCs still block through their own correction passes (`doors.rs`, `props.rs`, `npcs.rs`) after the controller
  moved; they should set `Player.external_support` when they lift the hull, and track the hull centre rather than its feet
  (a stance change moves the feet by 34 units without the player moving).
* The initial-clamp quirk is per level here (the retail box-size copy survives level changes once a crouch happened).
