# Retail gunfire presentation (cshell.dll)

Evidence for `crates/level-viewer/src/gunfire.rs`. Addresses are cshell.dll
virtual addresses (base `0x10000000`); `python -m tools.inspect_retail START END`
disassembles a range. Lengths are native units (1 unit = 1 cm in the remake).

## Engine rules

- Every effect goes through `EffectMgr::Spawn` (0x1004e520). Per frame
  (0x10054440): `age += dt`, removed after its lifetime; a fading effect loses
  `dt/lifetime` alpha and RGB dims with it; growth is `scale *= 1 + g·dt`.
- Moving effects (0x10052290): `pos += vel·dt`; gravity mode 1/2/3 subtracts
  640/240/96 units/s² from `vel.y`. Types 0x0c, 0x1f and 0x20 turn their up axis
  along the velocity (streaks).
- Blending (Lithtech.exe 0x53af65): sprites are additive unless the spawner asks
  for normal alpha or multiply.
- Sprite size (Lithtech.exe 0x53da90): corners at `pos ± right·texW·s ± up·texH·s`,
  so a sprite is `2·texW·s` units wide.
- A global detail switch (0x100b2490) gates smoke, sparks, tracers, debris,
  decals and the impact light; the remake keeps it on.

## Player shot (0x10003f90)

| Effect | Rule |
|---|---|
| Muzzle flash | `sprite0..2` at view-model sockets `socket_blik0..2`, scale `skala_sprite0..2`, 0.12 s, camera-facing, additive, follows the gun (0x10053170). `ile_sprite0 N` picks `…0`..`…N-1` variants at random (0x1000440c). |
| Muzzle light | 0.07 s, radius 192, colour (0.95, 0.85, 0.45), at the last flash (0x1000519b). |
| Smoke | `ilosc_dymu_po_strzale_gracza` puffs of `dym_po_strzale_gracza` at the sprite0 flash, velocity `(F + a·R + b·U)·256` with `a,b ∈ ±0.25`, lifetime `czas_…`, scale `skala_…`, growth `zwiekszanie_…`, fades (0x100044af). |
| Muzzle sparks | `iskry_po_strzale_gracza` × `sprites\iskra.spr` from the muzzle, velocity `(F + a·R + b·U)·predkosc` with `a,b ∈ ±0.125`, lifetime `czas_iskier…`, scale `skala_iskier…`, streak along the velocity, fades (0x100047af). |
| Shake | `shake_screen` is computed (0x100051df) but nothing reads it: no visible shake. |
| Tracer | `sprites\smuga.spr`, scale (0.12, 40), 4000 units/s, 0.6 s, only if the hit is over 320 units away, starting at `eye + dir·1400`. |
| Casing | `models\misc\luska.ltb` (`luska_do_obrzyna` for `kul_na_raz ≥ 2`) at `socket_luska_strzal0`, scale 0.6, 10 s, gravity 640, velocity `F·(48..96) + R·(64..128) + U·(32..64)`, bounce `vy *= −0.35` with `sounds\weapons\luska1.wav` (0x10009ea0, 0x100527ff). The case leaves when the first shoot clip ends (the shotgun pump: 0.666 s after the shot) or when the next shot interrupts that clip, not at the trigger pull (0x100101ef, 0x10004029); a weapon change or reload before that loses it. |

## Enemy shot (0x100462e0)

Muzzle = the held pickup model's `socket_lezacy_strzal`. Flash
`sprite_lezacy_oko_0` × `skala_sprite_lezacy_oko_0` (1.0 if 0), 0.25 s, stays
where spawned. Light 0.07 s radius 128. Smoke `dym_po_strzale_wroga`, velocity
`(gunF + 0.25·Y)·64`, growth 0.5/s. Sparks as above with `…_wroga` values.
Tracer only if the hit is over 128 units away, from `muzzle + 0.4·distance`.
The bullet uses the same trace as the player's, so impacts are identical.
Casings from `socket_lezacy_luska_strzal`, scale 0.75, 5 s.

## Impact (trace 0x10005ce0)

- `hit_sprite` × `hit_sprite_skala`, 0.6 s, at the hit point, camera-facing,
  additive; world and world-model hits only (0x10006c6d).
- Ricochet: 75 % chance of `sounds\weapons\ryko0..4.wav`, radius 1280, on every
  hit (0x10007e1c). The item's `hit_sound` is used only by melee (0x1000fd25).
- Light 0.1 s, radius 32, colour (0.25, 0.2, 0.15).
- Debris by polygon SurfaceFlags (or `iskry/gruz/drewno_przy_trafieniu`
  volumes, 0x1002c870): 0 → 4 rubble, 6 → 6 rubble + smoke, 2 → 5 sparks,
  3 → 5 wood splinters. Sparks: `dir = norm((r1−r2)/8, r3/16 − r4/32, (r5−r6)/8)`,
  at `hit + 2·dir`, velocity `80·dir`, 0.5 s, scale (0.1, 0.2). Rubble
  (`models\levelowe\kawalki\gruz01..04.ltb`) and splinters (`drewienko01..03`):
  velocity `160·dir`, gravity 240, scale 0.2..0.6, 0.6..0.9 s.
- `hit_smuga` streaks of `sprites\ogon.spr` on plain world hits, scale
  (0.05, 0.5), velocity `10·d` with d ∈ ±32 / −16..32 / ±32, 0.3..0.6 s, gravity 240 (0x1000887f).
- Bullet hole (0x10008a55): SurfaceFlags 6 → random `dziura0..2.dtx`, 12×12
  units; otherwise `slad.dtx`, 6×6; none on SurfaceFlags 1. In the wall plane,
  `0.4..0.65` units off it, lives 5 s.

## Blood

- NPC hit (0x100053f0): `krew1/2/3.spr` puffs, 1 s, scale 0.05, growth 1.2/s,
  velocity `0.75·(±16, 0..16, ±16)`, gravity 96, normal alpha 0.8 (constant: the fade argument is 0, round 3 correction);
  `krew4.spr` with 1/16 chance, 16 units above, no motion/growth/gravity. A bullet into the lower part of a corpse: docs/retail-blast.md.
- Gore within 72 units of the player: 22 × `krew_dodatki\1..11.spr`, scale 0.27,
  gravity 240, 0.8..1.4 s; half leave `krewpodloga1..4` on floors (30 s, scale 0.03).
- Wall splat (0x10009300): ray 32..256 units past the hit (±16 jitter), vertical
  walls only, 1.4..1.65 off the wall, alpha 0.9. `duzy_bryzg` guns (FN, M-14):
  scale 0.2, `bryzg/bryzg2` (60 s) or `bryzgmaly/1/2` (120 s); others 0.08,
  `bryzgmaly/1/2`, 120 s.
- Dead NPC (0x10042edc, 0x100543b0): `sladkrwipoziom.spr` flat at the feet,
  hidden for 4 s, then scale `0.04·(1 + √(age − 4))` until 14 s, lives 180 s.
- Player hit: `sounds\speech\hero\wcialo.wav`, the same puffs, and
  `sprites\krewmonitor.spr` 8 units in front of the camera, scale 0.04, rolled
  towards the attacker, fading over 1 s (0x10033a64, 0x1003b100).

## Remake implementation

`src/fx.rs` is the EffectMgr stand-in (sprite/model particles: age, `dt/lifetime` fade, `scale *= 1+g·dt`,
gravity, velocity-aligned streaks, bounce with `luska1.wav`, delayed/growing pools, view-space particles on
RenderLayers 3). `src/gunfire.rs` spawns everything from the retail item keys (`output/retail_weapons.json`
`commands`, NPC `weapon_asset.commands`) and the exported sprites/textures/models (`retail_effects.json`).
Bullet holes and debris use the per-face SurfaceFlags of `<world>.collision.surfaces.json` (`audio::Surfaces`;
doors use the dominant flag of their world model). `plama_krwi` (cshell 0x10006a16, 0x10042e56) gates the wall
splat and the dead-NPC pool, `nie_krwaw` the puffs. Sounds are positional (`sound.rs`, linear `1 - d/R`).

Deliberate approximations: the view-space flash follows the drawn weapon socket including the weapon bob;
the muzzle light and world-space smoke/sparks use the camera transform of that point; tracers do not fade
(Spawn's fade argument is 0 in the tracer call); the debris/spark direction is mirrored into the hit
surface's hemisphere; gore velocity, the `bryzg` choice odds (50 % big/small for `duzy_bryzg`) and the casing
spin are not in the evidence; the player-hit `krewmonitor` overlay is a UI image (about 1.5 screen widths,
rolled with the attacker's bearing) instead of a sprite 8 units ahead.

The revolver's six reload casings (`socket_luska_reload0..5`) do exist: cshell 0x1000a1d0 spawns one `luska.ltb` per socket when the first
reload clip (`reload1`) ends, velocity (32..64, -(1 + 32..64), 32..64) in world axes, 15 s, like the other casings (`gunfire::reload_casings`;
the axes of that vector are read from the code, the socket frame it is expressed in is not proven).

Headless check: `MESTER_SILENT=1 MESTER_TEST_SCENARIO=gunfire level-viewer rh1-wiezienie2 ../../output out.png t1,t2`
faces `MESTER_TEST_NPC` (name, or `auto` = a hostile with a wall 60..160 units behind it) at
`MESTER_TEST_NPC_DISTANCE`, fires `MESTER_TEST_GUN` (default Glock) every half second from t=2.5 s and lets the NPC
fight back; `MESTER_TEST_SLOW="factor,from,to"` slows game time in that window so a capture catches the 0.12 s
flash, `MESTER_TEST_HOLD_FIRE=1` only watches the enemy, `MESTER_TEST_HIT=right|left|behind` adds a wound from that
side at t=2 s, `MESTER_TEST_END` sets the run length and `MESTER_TEST_SURFACE=<flags>` (impact scenario) forces every
face's SurfaceFlags (6 tile crater, 2 sparks, 3 splinters).
