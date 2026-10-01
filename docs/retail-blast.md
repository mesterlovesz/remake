# Grenade blast and hit-blood chunks (cshell.dll EffectMgr, presentation round 3)

Evidence for `crates/level-viewer/src/retail_weapons.rs` (`Blast`, `BlastEmber`, `BlastPuff`, `explosions`), `fx.rs` (multiply blend, Y spin) and `gunfire.rs` (`corpse_leg_blood`).
Addresses are cshell.dll virtual addresses. Tool helpers: `python -m tools.inspect_retail START END`.

## EffectMgr::Spawn argument list (0x1004e520, 19 stack arguments, arg1 = last pushed)
| arg | stack slot | meaning |
|---|---|---|
| 1 | 0x4abc | type (1 sprite, 2 light, 3 bullet-like model, 4/5/6 view-model flashes, 7 grenade, 8 blast controller, 9 ember emitter, 0x0a puff, 0x0b chunk, 0x0d cigarette streak, 0x23 floor-stain gore bit, 0x24 `miecho`) |
| 2 | 0x4ac0 | lifetime in seconds (+0x68; type 8 overwrites it with 1.0, 0x100514d7) |
| 3 | 0x4ac4 | moves flag (+0x74): the motion update 0x10052290 does nothing without it |
| 4 | 0x4ac8 | sprite / model path ("" = the type chooses) |
| 5 / 6 | 0x4acc / 0x4ad0 | position* / velocity* |
| 7 | 0x4ad4 | gravity mode (+0x90): 1 = 640, 2 = 240, 3 = 96 units/s^2 on vel.y |
| 8 | 0x4ad8 | spin rate (+0x88) about the world Y axis `(0,1,0)` = 0x10b39870 (0x10052f10, angle kept mod 2 pi) |
| 9 | 0x4adc | scale vec3* (for type 2 the light radius comes from arg 10's neighbour) |
| 10 | 0x4ae0 | colour vec3* -> `SetObjectColor(r,g,b,a = r)` (alpha is the RED value) |
| 13 | 0x4aec | multiply selector |
| 15 | 0x4af4 | flag 4 of the object (irrelevant for blending) |
| 16 | 0x4af8 | 0 -> flags2 = (arg13 ? 4 multiply : 2 additive); non-zero -> normal alpha (SRCALPHA / INVSRCALPHA, Lithtech.exe 0x53d5b0) |
| 17 | 0x4afc | fade: alpha starts at 1.0 and loses `dt / lifetime` per frame; the colour is then replaced by (a,a,a,a) each frame (0x10054440) |
| 18 | 0x4b00 | growth per second (`scale *= 1 + growth * dt`) |
| 19 | 0x4b04 | rotation quaternion* |

`docs/retail-gunfire.md` mislabelled the fade argument for the blood puffs: `krew1..3` (0x10005511, 0x100055e6, 0x100056bb) and `krew4` (0x10005763) have arg 17 = 0, so they keep alpha 0.8 for their whole second and pop out; `krew4` has no motion, growth or gravity at all.
`python -m tools.spawn_sites [address prefix]` lists all 80 `call 0x1004e520` sites with their decoded arguments (it walks back 19 `push` instructions; nested calls can confuse a row).

## Blast (type 8 controller, update 0x10053ab0, called from 0x10054440 before the age is advanced)
The controller is created by 0x1005b120 (grenade life end, prop explosions) at the grenade position; its life is 1.0 s (0x100514d7), it removes itself when `age > 1.0` AFTER the update, so every test below sees `age <= 1.0`.
| age test | what happens |
|---|---|
| `> 0` (second frame) | flash `systemblikwybuch.spr` (additive, scale 4, 0.45 s, no motion); light type 2 (r 256, colour (.85,.95,.99), 0.2 s); `rock_lup.wav` 3D, radius 1280 (0x100597f0) |
| `> 0.1` | `systemwybduzy1.spr` fireball (additive, scale 1.1, 1.45 s); 12 type 9 embers (life 0.5..0.7, pos = centre + (r1-r2)*0.048, r*0.032+4, (r4-r5)*0.048; vel = offset * 10; gravity 2); 32 type 0x0b chunks (model `models\misc\k1..k4.ltb` by rand%4, skin `kx.dtx`, scale 2, life 0.9..1.7, offset (r-r)*0.032, r*0.024, (r-r)*0.032, vel = offset * 16, gravity 2, spin 18.85 rad/s; each drags an additive `ogon.spr` streak scale (0.25,1.6) along its velocity, 0x1004f80f) |
| `> 0.6` (after the 0.1 stage) | second fireball at +32 up, scale 1.9, 1.45 s |
| `> 0.9`, then `> 0.6`, `> 0.8` in the same call | three `systemdymduzy.spr` smoke puffs, MULTIPLY blend (ZERO / SRCCOLOR), 2.5 s, rising 48 / 32 / 16 u/s, scale 1.9 / 2.1 / 2.3, growth 0.2 / 0.1 / 0 |
| `> 1.1` | a fourth puff (0x1005415f) that can never run: the controller is gone at 1.0 |
Type 9 (0x100541e0): every frame `timer += dt`; above 0.03 s the timer resets and a type 0x0a puff is spawned at the ember with scale `(life - age) * 0.24`; the ember itself only moves (gravity 240).
Type 0x0a (0x10054280, life 1.0): `age > 0` additive `systemwybmortyr.spr` (scale + 0.02, 1.15 s), `age > 0.3` multiplicative `systemdymduzy.spr` (scale + 0.09, 2.5 s).
The smoke sprites are white-background multiply textures (frames 0..27 fade to white); DTX alpha of all these frames is zero (additive/multiply ignore it).
Remake: `BlastEvent` state machine (unit test `the_blast_controller_fires_its_stages_once_and_never_reaches_the_fourth_smoke`), `fx::Blend::Multiply` = Bevy `AlphaMode::Multiply`. Capture aid: `MESTER_TEST_BLAST=<distance>` fires one 2 s into the level (captures/pres/blast-*.png).

## Hit blood (cshell 0x100053f0 NPC hit, 0x10005920 corpse lower body)
* `0x10005360` random vector: y = r/16, x = r/16 - r/16, z = r/16 - r/16, r = rand & 0xff.
* Puffs: velocity vector x 0.75, position + velocity x 0.04; gore bits (22, only with the shot-debris option and within 72 units of the player): velocity vector x 3, life 0.8 + (rand & 255) * 0.6/256, sprite `krew_dodatki\N` N = rand % 11 + 1, scale 0.27, gravity 2; odd rand = the floor-stain type 0x23.
* Corpse: the trace (0x1000683e..0x1000689d) handles objects whose user flags are 0x80 (`SetObjectUserFlags(body, 0x80)` when an actor dies, 0x10042d79) and that are visible. A hit below `actor.y - half_height / 2` calls 0x10005920: one `krew1` puff, six gore bits and three `miecho01..03` chunks (type 0x24: model by rand % 3, skin `udko_fin.dtx`, scale 0.7, life 0.8..1.4, velocity vector x 3 with x and z doubled again in Spawn, gravity 3, spin 6.283), and the bullet stops. A hit above that line continues through the body (the loop at 0x1000689d looks for a live actor box at the point, otherwise the ray goes on 16 units further).
Remake: `gunfire::corpse_leg_blood` (the stop rule is wt-ai2's `NpcRoster::corpse_stop`; the splash there now uses the exact 0x10005920 recipe incl. the miecho chunks). Not captured on a real corpse (no reproducible aim at a lying body in the headless scenarios); the vector ranges are unit tested.
