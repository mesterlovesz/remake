# Retail movement evidence

Input: installed `GYARI/cshell.dll` (x86, base `0x10000000`), `Lithtech.exe` (base `0x400000`), `object.lto`.
Reproduce a range: `python -m tools.inspect_retail START END [--module Lithtech.exe]` (pefile/capstone in
`../research/python-deps`). Original bytes are read only. The full verified/approximated table, the engine facts and the per-level
harness results are in `retail-movement-audit.md`; the camera is in `retail-camera.md`.

| Address | Observed behavior |
|---|---|
| 100601a1 | Walk 180 native units/s |
| 100601b5–10060210 | Run 180 + 120 x min(stamina/max + .2, 1) |
| 100606fe–10060716 | Crouch 90, clears the running flags |
| 100607xx–10060bxx | Acceleration 9000/s^2 x dt, airborne multiplier .05 |
| 10060ba0–10060bec | Exhaustion blocks the jump; standing jump +300 vy, cap 450 |
| 100606a2–100606eb | Each stance change adds 64 vy, also in the air |
| 1005f5f0–1005f6xx | Crouch half-size 24/24/24; standing 24/58/24 and centre y +32; SetStairHeight(18) |
| 1005f7ba | Initial half-size 16/58/16 before the first stance change |
| 100616f0–100618e9 | Stand clearance: five 108-unit upward rays, centre/corners +-24 |
| 1006108d–100610d3 | Gravity 1000/s^2 (not standing), terminal -640 |
| 10061112–100611eb | Standing without a direction key: horizontal velocity x (.95 - dt) x .45 each frame |
| 10061e00–10061f19 | Jump costs 5; running on the ground -4/s; moving +.5/s; rest +5.5/s |
| 10061edf–10061f19 | Exhaustion below 10, clears above 15 |
| 1006131b–1006135c | Running footsteps every .4 s (timer starts at 0) |
| 1006102f | Fall damage (takeoff - landing - 196) x .15 after 60 frames; takeoff reset at the jump apex |
| 10061906 | Movement frozen for the first 60 frames of a level; 10061940 frame time above 1.5 s |
| 10061a30 | Frame displacement clamped to clamp(.75 x largest half extent, 4, 16) |
| 1005b6df | Start message: position and `Kierunek` (yaw 0, pi/2, pi, 3pi/2), not the StartPoint rotation |
| Lithtech.exe 416c60–416c8c | NotAStep surface flag (1<<22) and walkable plane normal.y > 0.7071 |

The stance impulses, centre lift, frame-dependent friction and the 60 frozen frames are intentional retail behaviour.
Surface-dependent footstep sounds are chosen by `audio.rs` from the ray `0x100614e0`; nothing here plays a sound.

`retail-movement` has no Bevy dependency and keeps native Y-up units: exact swept-AABB (separating axis) queries against the
exported PhysicsBSP triangles, Quake-style plane clipping and an 18 unit step; it is not the decompiled LithTech solver. Skin
0.05, back-off 0.03, ground probe 0.3. Door, prop and NPC blocking is done by their own correction passes after the controller.

Viewer scale 0.01, eye offset 46 standing / 16 crouched, FOV 81, mouse factor .006625 and the bob are documented in
`retail-camera.md`.

Tests: `crates/retail-movement/tests/walking.rs` (10), `solver.rs` (13, synthetic geometry) and `levels.rs` (level-wide harness
over all 29 exported worlds, headless). A matching retail input recording is still needed before claiming numeric 1:1 of slides
and steps.
