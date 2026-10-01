# Sound parity: every retail sound cue against the remake (2026-09-30)

Owner request: "check again that everything is really fine; make it perfect". This is the master table of every sound the retail game can produce:
file, trigger, reach (3D radius or 2D), loop, and whether the remake plays it the same way. Evidence is disassembly of `cshell.dll` (the whole
game: client and most of the logic), `object.lto` (server classes) and `Lithtech.exe` (engine); the extra background is in docs/retail-audio.md.
Nothing here was listened to: the remake side is verified by the non-audible cue net (below), unit tests and offline decoding only.

## How the enumeration was done (reproducible)

`python -m tools.audio_sites` disassembles the modules and lists every `PlaySound` site with its wrapper function and every caller of each wrapper
with the file name pushed before the call; `python -m tools.audio_audit` checks the sound paths the data references against the install.

The engine sound manager is called at **eight places** in `cshell.dll` and **three** in `object.lto`; everything else goes through these wrappers
(`PlaySoundInfo`: flags, file name, radius, volume byte; the switches are the globals `0x100b23f4` Music, `0x100b23f5` Speech, `0x100b23f6` Sound
filled from the console variables of the options pages):

| Wrapper | Flags | What | Switch | Callers |
|---|---|---|---|---|
| S3D `0x100597f0(file, pos*, radius)` | 0x2 (3D, no handle) | positional one-shot | Sound | 18 call sites (table rows marked S3D) |
| S2D `0x100598b0(file)` | 0x200 (local/2D) | player-only one-shot | Sound | 7 call sites (row marks S2D) |
| generic 2D `0x1001c0e0(file)` | -> S2D | hero / pickup sounds | Sound | jump x2, landing, footsteps, pickup x2 |
| music `0x10059950(file or 0)` | 0x254 (2D, loop 0x4, handle 0x10, volume 0x40) | the one music track; a call first kills the old handle | Music | 8 call sites |
| speech 3D `0x10019350(file, pos*, 4096)` | 0x1012 | dialogue line at the speaker | Speech | 1 (0x1001985b) |
| speech 2D `0x10019420(file)` | 0x1210 | dialogue line without speaker, chosen answers | Speech | 5 |
| scene sound (inline, 0x10012ee4..0x10013053) | **0x1210 (2D)** | cutscene phase `glos` | Sound? (none) | scene start |
| prop use (`0x1002e520(obj, file, loop)`) | 0x412 (0x416 looping), radius 1536, volume byte 0x50 ignored | `sound0/1/01/10` of an `o_obiekt`, handle kept at object +0x1f8 | Sound | 6 |
| server message 0x38 -> `0x1005b299` | S3D | `(file, pos, radius)` sent by object.lto `0x10011e10` | Sound | door-like classes, radius 640 |
| object.lto direct `0x100046a0` | 0x2, radius 640 | `Glos_otwierany` / `Glos_zamykany` | none (server side) | `b_door` family |
| object.lto `0x1001cf66` | ambient / 3D | generic `Sound` class (Filename, Inner/OuterRadius, Volume, Priority, Ambient) | - | **no world instantiates it** |

PlaySound flag bits (the engine's `PlaySound`, Lithtech.exe 0x4ac000: `test cl,3` decides the 3D parameters, `test cl,0x40` the volume byte, `test al,0x80`
the pitch, `test byte [esi],0x10` the handle): **bits 0x1 / 0x2 = 3D** (position and radii are only read then), 0x4 = loop, 0x10 = keep the handle,
0x40 = use the volume byte (cleared when it is 100), 0x80 = use the pitch, 0x200 = local to the client, 0x400 / 0x1000 = handle / speech class (not needed here).
So S3D (0x2), the speaker line (0x1012) and the prop use sound (0x412) are 3D; S2D (0x200), music (0x254), the 2D speech line (0x1210) and the **cutscene
sound (0x1210)** are not. The volume byte is 100 (`0x100b23f0`, set once at 0x1001aba7) in every wrapper and only the music sets 0x40, so there is no per-cue
volume: the 80 the prop use sound carries is ignored (no 0x40). The engine reports `Max samples` 255 (SndDrv.dll 0x10004fb0), priority 9 for the music
and 0 for the rest: no voice culling in practice. The engine has no sound pause (no such string or call anywhere).

## Master table

Legend (status columns: *before* = main ac99f47, *after* = this branch): **P** plays as retail, **D** differs, **M** missing. "Silent" rows are cues
retail never plays (verified by the absence of any reference) and the remake does not play either (P). Radius in native units; "2D" = local.

### Hero

| # | Cue | Retail: file, where, when | Reach | Before | After |
|---|---|---|---|---|---|
| H1 | jump | `sounds\speech\hero\skok.wav`, 0x10060c08 / 0x10060c43 (action 5 when standing, latch clear, not exhausted; action 0x14 has no key) | 2D | P (`audio::jump_sound` from the impulse) | P |
| H2 | landing | `speech\hero\spad.wav`, 0x10061064: touch down, level frame > 60, (takeoff - y - 196) > 0, together with the fall damage; **also when already dead** | 2D | D (not when dead) | P (`view::update`) |
| H3 | footsteps | `hero\KROK{K,M,D,}{1,2}.wav`, 0x10061404: every 0.4 s running and moving; surface flags 0,6 K / 2,4 M / 3 D / other; first step variant 2 | 2D | P | P |
| H4 | heartbeat | `speech\mason\serducho.wav`, 0x10061ceb: health crosses 50, then 20 | 2D | P | P |
| H5 | wound by a bullet | `speech\hero\wcialo.wav`, 0x10006a65 (the trace's hit object is the player) | **3D 640 at the hit** | D (2D) | P (`gunfire::Grunt::Bullet`) |
| H6 | wound by a shove (`odepchnij_gracza`) | `weapons\wcialo.wav` (another file), 0x1004248b | 2D | D (hero file, 3D path) | P (`Grunt::Shove`) |
| H7 | wound by a bite (`gryzie`) | none (0x100422e2: health -10 and blood only) | - | D (grunt played) | P (`Grunt::Silent`) |
| H8 | item pickup | item `pickup_sound`, 0x1001c2fa / 0x1001c38c (ammo at its cap: refused, no sound) | 2D | P | P |
| H9 | chosen dialogue answer | `answerNsnd`, 0x10018d65 / 0x10018dc9 / 0x10018e2d / 0x10018e91 via the 2D speech wrapper | 2D | D (3D at the speaker) | P (`dialogue::present`) |
| H10 | silent: eating / using an item, death, weapon draw / holster, empty-magazine click, menu, inventory, level-up | no reference in any module | - | P | P |

### Player weapons (item keys `sound_shoot +0x19d8`, `sound_reload +0x1a58`, `hit_sound +0x1ad8`)

| # | Cue | Retail | Reach | Before | After |
|---|---|---|---|---|---|
| W1 | melee swing | `sound_shoot` (`swist.wav`), 0x10003f83 at camera + 64 x forward | S3D 640 | P | P |
| W2 | shot | `sound_shoot`, 0x100051c1 at the eye plus the weapon's view offset (`przes_right/up/forward`, 0x10005033..0x100050b2: a few units from the camera, not 64 ahead like the melee swing) | S3D 640 | D (camera + 64 x forward) | P (`gunfire::shot_position`) |
| W3 | reload start | `sound_reload`, 0x10003df0, magazine weapons (not `shotgun`), when the reload starts (trigger on an empty magazine or R) | S2D | P | P |
| W4 | shotgun shell | `sound_reload` per inserted shell, 0x100100b7 | S2D | P | P |
| W5 | melee hit on an actor | `hit_sound` (`wcialo.wav`), 0x1000fd46 at the hit point | S3D 640 | P | P |
| W6 | melee on a wall | `palka_sciana.wav`, 0x1000ffa5 | S3D 640 | P | P |
| W7 | ricochet | `weapons\ryko0..4.wav`, 75 % of every bullet hit, 0x10007e91 | S3D 1280 | P | P |
| W8 | grenade pin | `zawleka.wav` is the grenade's `sound_shoot`; **nothing reads it for a grenade** (0x10003f90 sends it to 0x10003e20) | never | D (played at the prime) | P (silent) |
| W9 | grenade bounce | `grt_ryko.wav`, 0x100520c1, once per contact | S3D 640 | P | P |
| W10 | blast | `rock_lup.wav`, 0x10053ba4 (grenade and prop explosions) | S3D 1280 | P | P |
| W11 | casing bounce | `luska1.wav`, 0x10052871, `vy` bounce of a casing | S3D 640 | P | P |
| W12 | P90 laser / M-14 zoom | `alt_sound_on/off` keys are not read (the parser wants `sound_alt_on/off`, which nothing reads); the files `laser_on/off.wav` are not in the install | never | P (silent) | P (silent) |

### NPCs (postacie.txt phase keys)

| # | Cue | Retail | Reach | Before | After |
|---|---|---|---|---|---|
| N1 | NPC shot | `sound_shoot` of the held weapon, 0x10046777 | S3D `glosnosc`, x4 while it sees the player or in a cutscene | P | P |
| N2 | footsteps | `glos_buta1` every `odstep_glosow_buta` > 0.09 s, 0x1004abac (`glos_buta0` never selected); muted in cutscenes | S3D 640 / 2048 seen | P | P |
| N3 | alert ("halt") | `sound_on_kontakt` (`sounds_on_kontakt N` picks `haltN`), 0x10049b1d | S3D 1280 / 2048 seen | P | P |
| N4 | death scream | `deadN.wav` (N = rand % 8, `graj_dzwiek_smierci`) in the halt folder, 0x10042e4c; macaroni has no files: silent | S3D 4024 | P | P |
| N5 | phase `sound` | once when the phase starts, 0x10042177 | S3D 2048 | P | P |
| N6 | `sounds\speech\enemies\*` (80 files) | no reference anywhere | - | P (silent) | P (silent) |

### World objects

| # | Cue | Retail | Reach | Before | After |
|---|---|---|---|---|---|
| O1 | doors, drawers, lift doors | `Glos_otwierany` / `Glos_zamykany` on a state change at the object origin, object.lto 0x100046a0 / `0x10011e10` -> message 0x38 | 640 | P | P |
| O2 | detector `Dzwiek` | wiez_wn1 DOMOFON.WAV, 0x1002cb99 (3D `Zasieg_dzwieku`) or 0x1002cba7 (2D when `Dzwiek_3D` 0) on first trigger | as data | P | P |
| O3 | prop use | `sound01` (open) / `sound10` (close), `0x1002e520`: **a new use sound kills the previous sound of the same prop** (handle at +0x1f8) | 3D 1536 | D (old one kept playing) | P (`audio::SoundOwner`) |
| O4 | prop destroyed | `death_sound`, 0x1002e79c | S3D 960 | P | P |
| O5 | prop idle loops `sound0` / `sound1` | 0x1002d25d, 0x1002d283 (looping 0x416); **no object defines them** | - | P (unused) | P (unused) |
| O6 | ambient `d_odglos` (59) | only written to `scripts\cs\dzwienki.txt`: silent | - | P | P |
| O7 | elevator button `Dzwiek` | message 0x38 (0x1000248b / 0x100024f7); all 80 instances have an empty `Dzwiek` | - | P (silent) | P (silent) |
| O8 | generic `Sound` object class | object.lto 0x1001cf66; 0 instances in the 29 worlds | - | P (unused) | P (unused) |
| O9 | rain / weather | `sounds\deszcz\*` unreferenced | - | P (silent) | P (silent) |

### Music (`sounds\muza`, one 2D looping track, volume 100)

| # | Cue | Retail | Before | After |
|---|---|---|---|---|
| U1 | main menu | `menu.wav` on frame 2 of the menu, 0x1005a382 | P | P |
| U2 | level | gameai `muza` on frame 78 (0x4e) after the load, 0x1005a5ad; table in docs/retail-audio.md | P | P |
| U3 | outro / credits | `outro.wav` 0x100131ae, `credits.wav` 0x10031a30 | P | P |
| U4 | stop | every load stops the track (0x1005a180 -> 0x1005ae92): hard cut, no crossfade, no fade | P | P |
| U5 | no calm / action switch | `MuzaSpok*`, `MuzaAtak*`, `wlaczmuze` are never read | P (silent) | P (silent) |
| U6 | `Music` switch | the per-frame check 0x10059230: switch off stops the track **and clears the remembered name**; switch on plays the remembered name after frame 100, which is empty: music stays off until the next level / menu start. A start while the switch is off plays and remembers nothing | D (muted and resumed mid-track) | P (`music::Retail`) |
| U7 | menus | nothing stops or pauses the track when a menu opens (only loads do); the engine has no sound pause. (Remake courtesy, not retail: the music pauses while the window has lost the focus, `Session::unfocused`) | D (paused behind the in-game menu) | P (`settings::holds`) |

### Speech and cutscenes

| # | Cue | Retail | Reach | Before | After |
|---|---|---|---|---|---|
| V1 | dialogue line | `titlesnd` at the speaker, 0x1001985b; without a speaker 0x10019865; the line's length replaces the node delay (0x1001987a); a dead speaker's voice stops (0x10018b7f); `empty.wav` tracks are silent | 3D 4096 / 2D | P | P |
| V2 | cutscene phase sound (`glos`, e.g. `scenes\intro\01.wav`) | 0x10013012: flags **0x1210 = local**; the position and radius 1280 it also fills in are unused | **2D** | D (3D, radius 1280 at `socket_glos`) | P (`audio::play_voice`; with "Javított 3D hangzás" it pans from `socket_glos`, no falloff) |
| V3 | cutscene speech (subtitle key) | 2D | 2D | P | P |
| V4 | cutscene NPC weapon | 0x10046777 with the cutscene flag | S3D `glosnosc` x4 | P | P |
| V5 | start-up videos | play1.exe, outside the game | - | P | P |
| V6 | `gadka` key (0x10012423) | scene start, 2D if Speech is on; no data line uses it | - | P (unused) | P (unused) |

### Mixer

| # | Rule | Retail | Before | After |
|---|---|---|---|---|
| X1 | switches | Music / Speech / Sound decide whether a **new** sound starts (the wrappers return early); playing ones are never touched except the music (U6) | D (sinks of a switched-off class are muted instead; same result unless a switch changes while a sound plays) | D (unchanged, documented) |
| X2 | volume | no sliders in retail (fixed 100); the remake's master slider multiplies every sink | P (owner addition) | P |
| X3 | falloff / pan | `1 - d/R`, software pan, 25 % quieter behind (Lithtech.exe 0x480940); "Javított 3D hangzás" is the optional extension | P | P |

### Counts

| | Plays (P) | Differs (D) | Missing (M) | Rows |
|---|---|---|---|---|
| before (main ac99f47) | 41 | 12 | 0 | 53 |
| after | 52 | 1 (X1: switch semantics, deliberate) | 0 | 53 |

The eleven fixed differences: H2 (landing sound when dead), H5 / H6 / H7 (the three wounds), H9 (answers 2D), W2 (shot position), W8 (grenade pin), O3 (prop use sounds replace each other),
U6 / U7 (music switch and menus), V2 (cutscene voices 2D). Rows counted P that are "silent" or "unused" are cues retail never plays. Nothing was missing:
every retail cue already had a path, including the two the movement audit listed (jump and landing were already played, `audio::jump_sound` and `view::update`;
the audit note was stale).

## Files referenced but absent (retail too)

`python -m tools.audio_audit`: of 608 distinct `sounds\...wav` references (scripts, scenes, module strings) only `sounds\enemies\macaroni\halt0.wav` (and the
`dead0..7.wav` derived from it) and `sounds\weapons\laser_on.wav` / `laser_off.wav` are absent from the install. The first is the `macaroni` character
(`sound_on_kontakt`, `graj_dzwiek_smierci`): retail asks the engine for a file that does not exist, the engine logs "Missing sound file" and plays
nothing; the remake asks through the same path (`CUE` logged) and `play_world` drops it because the file is not exported. The laser files belong to the
`alt_sound_on/off` keys nothing reads, so they are not even requested. The 79 paths hard-coded in cshell.dll / object.lto all exist. The exported
cutscene tracks `scenes\intro\001.wav`, `intro1/1..3.wav`, `outro/00..17.wav` are all-zero in the install (22 files): silent in retail too.

## Verification without listening

* `audio::Cues` records every sound the game asks for (file, reach, distance, whether it is within its radius); `MESTER_CUE_LOG=1` prints them as
  `CUE 2d <file>`, `CUE 3d r=<radius> d=<distance> <file>`, `CUE far ...` (beyond the radius) and `CUE music play|stop <track>`.
  `audio::classify` (unit-tested) decides 2D / 3D / out of range; `play_world` calls `note_cue` before the silent/missing-file rules, so silent capture runs
  record everything.
* `MESTER_TEST_SCENARIO=sounds` (`sound_probe.rs`, label `sounds` of `tools/run_probes.py`) drives the real systems in rh1-wiezienie2 and asserts:
  level music start (`music play wiezienie1_spokoj`), jump `skok.wav`, a 650-unit fall -> `spad.wav` and the fall damage heartbeat, two heartbeats on
  45 / 15 health, a running footstep, the three wounds (bullet 3D 640 / shove 2D `weapons\wcialo.wav` / bite silent), the Music switch (stop, and
  the quirk: still off after switching on), then for each of the eight guns its retail `sound_shoot` and `sound_reload`, the nightstick `swist.wav`,
  the grenade blast `rock_lup.wav` (and that `zawleka` is never requested).
* `tools/run_probes.py` now sets `MESTER_CUE_LOG=1` and every existing scenario with sound carries a cue expectation (`CUES`): campaign (shot, ricochet,
  speech), retail (pickups, grenade bounce, death scream 4024), menu (menu music), lever (prop use 1536, door 640), door, pickup, arsenal (all eight guns
  and the blast), noise (alert 1280), revolver, move / input (jump, steps), hud (heartbeat), gunfire (grunt 3D, scream, ricochet), dialogue (speech 4096),
  ai-fight (alert 2048, NPC shot 5120), melee (swing, wall, hit, shove). `EVERYWHERE_NO` makes any `zawleka` cue fail every scenario.
* Unit tests: `audio::tests` (classification, cue record), `music::tests` (the retail handle machine: off/on quirk, start with the switch off, replacement,
  no pause behind menus), plus the existing ones (footstep pairs, halt / death paths, heartbeat, jump, detector, surfaces).
* The offline decode (`every_exported_sound_decodes_with_the_game_decoder`, ignored) and the `MESTER_AUDIO_TRACE` runs of docs/retail-audio.md are unchanged.

## What to listen for (owner, on his PC)

1. **Hit grunt** when a guard's bullet strikes you: the grunt now comes from where the bullet hit (a little to the side it came from) and is quiet if it hits
   far from you; a dog bite has no grunt at all; a club shove gives the other `wcialo` (shorter, from the weapons folder).
2. **Cutscene voices** (intro, prison escape, rat scenes) are the same volume wherever the camera is: no fading with distance. With "Javított 3D hangzás" on
   they still come from the speaker's side without getting quieter.
3. **Grenade**: no "pin" click when you ready it (retail has none); the bounce and blast as before.
4. **Choosing a dialogue answer**: your line is in the middle of the head, not from the NPC.
5. **Music**: it keeps playing behind the pause menu and the main menu options; switching Music off in the menu and on again brings no music back until
   the next level load (retail quirk); the level track starts about 4 s after the load and loops.
6. **Levers / switchable props**: pulling the lever again while its sound still plays cuts the first sound.

## Still unknown

* Whether the engine keeps playing one-shots behind the pause menu: nothing in cshell stops them, the remake still pauses them (the music is not paused).
* Which door class uses the direct server PlaySound (0x100046a0, ignores the Sound switch) and which the message path (0x10011e10, honours it).
* The engine's own voice culling (priority / `Max samples` 255): not reproduced, never reached in the game's scenes.
