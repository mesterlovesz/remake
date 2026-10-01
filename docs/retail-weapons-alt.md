# Retail laser, scope, alternate fire and melee (cshell.dll)

Evidence for the laser, scope and nightstick in the remake. Addresses are
cshell.dll virtual addresses; LT marks Lithtech.exe.

## Item fields

`melee` +0xb18, `grenade` +0xb1c, `alt_zoom` +0xb20, `sila_strzalu` +0xb28
(atoi, integer), `socket_alt_latarka` +0x1658, `socket_alt_laser` +0x16d8,
`socket_laser` +0x1758, `socket_niesiony_laser` +0x1958. The parser keys for
sounds are `sound_alt_on` / `sound_alt_off`, which nothing reads; the P90's
`alt_sound_on/off` lines don't match them, so no weapon plays a laser or zoom
sound. Index 0 of the array at 0x100b23b4 is the alcohol level; weapon skill is
`[0x100b23b4 + 4·experience_index]` for indices 1..8.

## Laser

- Built on weapon select when the item has `socket_laser` (0x100035e3,
  0x10003a40); the alt toggle (0x100037a0) rebuilds it from `socket_alt_laser`
  or `socket_laser`. It is therefore always on for the P90 and the M-14, and
  hidden while reloading or switching (weapon state not 0, 3 or 4).
- Beam (0x1000e870, drawn at 0x1000e670 after the world): from the view model's
  `laser` socket; unscoped along the camera forward for 6400 units; scoped, the
  eye ray's hit point becomes the end. The trace hits the world, world models
  and NPCs.
- Drawn as an untextured quad `start ± 0.1·R`, `end ± 0.1·R` (R = camera
  right): a camera-facing ribbon 0.2 units wide, colour (85, 0, 0), additive,
  depth-tested, no jitter.
- `sprites\weapons\pyleklasera.spr` (`pylek_lasera.dtx`, 32×32 red spot),
  additive, camera-facing: one at scale 0.1 at the hit point (the dot), and up
  to four at scale 0.02 as dust along the first `min(L, 128)` units of the beam
  (L = length − 8; 4 if L > 128, 3 if > 96, 2 if > 64, 1 if > 8), re-rolled
  every frame at `socket + forward·(8 + rand·min(L,128))`.
- The dot is a visual stimulus of radius 640 for the AI every frame (0x10045cf0).
- NPCs holding an M-14 draw two crossed ribbons 0.4 wide, colour (45, 0, 0),
  from their gun's `laser` socket to the aim point (0x1002130a); no dot.

## Scope (M-14 `alt_zoom`)

- Alternate fire (middle mouse / Alt) is edge-triggered and toggles alt mode
  (0x10060ce4 → 0x100037a0) with no state guard. With `alt_zoom` it sets the
  camera zoom flag: FOV × 0.15 at 5 rad/s per axis, both snap when either
  arrives; mouse × 0.25 (0x10054cee). Drunk FOV wobble still applies.
- Overlay (0x10036340, drawn first by the HUD): `misc\panel\celownik\Snajper_celownik.dtx`,
  512×512, one quarter of the reticle centred at its bottom-right corner, drawn as
  four mirrored screen quadrants with u 0.3→0.995, v 0.45→0.995, alpha-blended,
  white. Black outside radius 256 texels, a thin rim, a dithered grey ring,
  four grey cross bars from radius 128 to 240, a nearly clear centre. The rest
  of the HUD draws on top.
- The crosshair `textures\sprajty\system\celownik.dtx` (half-size W/1024·16 px)
  is hidden only while scoped. The view model stays visible (rendered with its
  own FOV). No tint in the code.
- Switching weapons un-zooms. Retail quirk: reload clears only the weapon's alt
  flag, so the camera stays zoomed with the overlay while the mouse returns to
  ×1 and the crosshair and unscoped laser path return; the next alt press
  re-arms it and a second one un-zooms.
- Other weapons: Glock/S&W alt only flips the flag (no effect); SIG 551 and P90
  alt switch a flashlight (light colour (0.95, 0.98, 1.0), 0x10003860,
  0x1000e770).

## Nightstick (melee)

- Fire (0x10003e60): no ammo; every `shot_latency` (0.3 s) while held. Plays
  `do_uderza` (0.1 s) and `swist.wav` about 64 units ahead (radius 640).
- When `do_uderza` ends (0x1000f420) the hit test runs once: points 24, 32, 48,
  56 and 64 units along the camera forward from the eye. Every NPC whose box
  contains one of them takes 40 damage (0x10043090); a world ray to the 64-unit
  point plays `palka_sciana.wav` at the hit (0x1000ff90). The world does not
  block NPC hits. Then `uderza` (0.3 s) and back to `trzyma`.
- On an NPC hit (0x1000fd25): `hit_sound` (`wcialo.wav`) at the 64-unit point;
  `krew1.spr` at the 32-unit point, 1.5 s, scale 0.15·(1.2 + 0.2·(rand & 5)),
  normal alpha, fading; 15..22 `krew_dodatki\1..11.spr`, 0.8..1.4 s, velocity
  (±96, 0..96, ±96), gravity 240, scale 0.15.

## Spread and skill

- Player trace (0x10005ce0): range `640 + 24·max(256 − rozrzut, 0)`;
  `s = rozrzut·(1 − skill·0.0075)` (0x10005fb7); each axis of the end point gets
  `(rand%1000 − rand%1000)·s·0.001`. No crouch, movement or alt modifiers.
- Skill += 0.1 per NPC hit while below 99.8 (0x10006abe).
- Shotgun: `kul_na_raz` independent traces with their own jitter.

## In the remake (`crates/level-viewer/src/weapons_alt.rs`)

- Assets: `python -m tools.export_altfire` writes `retail_altfire.json` (the 32x32 `pyleklasera.spr` frame, the 512x512 reticle
  texture, `palka_sciana.wav`); the blood sprites are the `krew1`/`krew_dodatki` entries of `retail_effects.json`.
- Alt state machine (`AltFlags`): the toggle, the leave-toggle of a weapon change and the reload rule are the three cshell paths
  above. The reload quirk is kept, so a reload followed by a weapon change leaves the camera zoomed until an `alt_zoom` weapon
  is toggled twice. `ViewState.scoped` is the camera flag, `ViewState.mouse_slow` the weapon flag (mouse x0.25).
- Laser: one camera-facing ribbon in world space (main camera, depth-tested, additive) from the socket, so the scoped view
  shows it entering from the lower right exactly as the real camera would; the ribbon is added as (85,0,0) in linear space,
  which is what the gamma-space blend of the original looks like on the mid-dark levels. Dot and dust are five persistent
  `fx::Particle` sprites (additive billboards, scale 0.1 and 0.02) that the laser system moves every frame. `NpcRoster.visual_stimuli` carries the dot (and the flashlight spot) to hostile actors: within
  640 units inside a 60 degree horizontal cone with a clear line, they get their contact phase like on seeing the player (an earlier "within 16
  units" rule was a misreading: the other two radii of the node are the bit pattern 0x10, zero as a float; docs/retail-weapons-audit.md section 4).
- Scope overlay: four UI quadrants share one texture (`ImageNode.rect` = u 0.3..0.995, v 0.45..0.995, mirrored). Retail stretches
  them over any resolution; here the reticle keeps 4:3 and black bars fill wider or taller windows so the circle stays round.
- Nightstick: `Inventory::tick` repeats a held melee trigger every `shot_latency`; `AltFire.swing` waits for `do_uderza`
  (0.1 s), then `NpcRoster::melee_hits` tests the five probe points against the actor boxes (item damage, no difficulty scaling).
  `swist.wav` comes from the shared `gunfire::shot_sound` (64 units ahead, radius 640); `wcialo.wav` at the 64-unit point and
  `palka_sciana.wav` at the wall hit are `sound::play_at` with radius 640. Blood is `fx` particles (`krew1` puff, 15..22 bits).
- Flashlight (SIG 551, P90): light 16 units short of the 1600-unit hit, radius 24+0.175d, dimming to 0 at 1600. The static level is
  unlit vertex colour, so a soft additive pool on the hit surface stands in for the illumination and a `PointLight` lights the models;
  the shaft is a simple three-strip additive ribbon (the retail cone quads were not decoded).
- Headless check: `MESTER_SILENT=1 MESTER_TEST_SCENARIO=altfire|melee|flash level-viewer.exe rh1-wiezienie2 ../../output out.png 2.5,5.5`
  (`MESTER_TEST_NPC`, `MESTER_TEST_WEAPON` and `MESTER_TEST_TURN` tune it).
