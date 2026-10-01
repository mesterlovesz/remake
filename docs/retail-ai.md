# Retail character AI (cshell.dll)

Evidence for `crates/level-viewer/src/npcs.rs` and `npcs/ai.rs` (the port), `npcs/tests.rs` and `ai_probe.rs`.
All addresses are `cshell.dll` virtual addresses (base `0x10000000`) unless marked `object.lto`. Reproduce a range with
`python -m tools.inspect_retail START END`. The whole AI is one data-driven **phase machine** ("faza" in `scripts/postacie.txt`): the DLL only
implements the per-frame rules below; which phase follows which, speeds, distances and fire rates are read from the phase commands.
There is no separate "alert / search / flee / cover" code: those behaviours are phases of the definition that use the `estimate_*` commands.

## Objects
* Character definition (0xfd0 bytes, parser 0x1003eabf..0x10041c00). Flags: `hostile` +0xfbd, `insignificant` +0xfbf, `nie_krwaw` +0xfb9, `nie_respawnuj` +0xfba,
  `nie_wspinaj` +0xfbb, `nie_obracaj` +0xfbc, `nie_patrz_na_gracza` +0xfb8, `plama_krwi` +0xfc2, `graj_dzwiek_smierci` +0xfc3, `nonsolid` +0xfc4,
  `glowa_specjalna` +0xfc6, `obracaj_ganem_w_pionie` +0xfc0, `malutki` +0xfc1, `nie_zostawiaj_gana` +0xfbe, `ruchomy` +0xfac. Floats: `HP` +0xf9c (**default 0**: the
  definition is zero-filled), `odleglosc_kontaktu` +0xfa0 (default 640.0, set at 0x1003eb0e), `kat_kontaktu` +0xfa4 (default 0; degrees x pi/180), `ucieka_jak_mniej_niz` +0xfa8,
  `exp_gained` +0xfb4. Phase list head +0xd98 (next +0x245c), `default_faza` +0xd18. Unknown keywords (`drop_weapon`, `ignoruj_markery_niechodzenia`, `nie_wspinaj_sie`) do not
  exist in the DLL and are ignored by the parser.
* Phase (0x2460 bytes): animation +0x80, `loop` +0x180, `static` +0x188, `show_weapon` +0x18c, `obrot_do_gracza` +0x194, `set` +0x198, `unset` +0x218, `setfaza0/1` +0x298/+0x398
  (target definition +0x318/+0x418), `strzal_raz` +0x498, `strzal_raz1` +0x499, `zabij` +0x49a, `odepchnij_gracza` +0x49b, `patrol` +0x49c, `estimate_do_gracza/kryjowka/od_gracza/do_strzalu/kluczy`
  +0x49d..+0x4a1, `moze_strzelac_w_wezle` +0x4a2, `gryzie` +0x4a3, `speed` +0x4a4, **`mod_obrotu` +0x4a8 (turn-rate multiplier, default 1.0 written when the phase is created, 0x10041b6a; the
  earlier notes called it an ignored yaw offset)**, `mod_y` +0x4ac, `mod_kier` +0x4b0, `mod_rot` +0x4b4, `strzal` +0x4c0 (seconds between shots), `obrot` +0x4c8, `obrot_pion_do_gracza0/1` +0x4cc/+0x54c,
  `obrot_postrzalu0/1` +0x5cc/+0x64c, `on_kontakt` +0x6cc, `on_koniec0..7` +0x74c step 0x80, `on_koniec_widzi0..7` +0xb4c, `on_koniec_anim0..7` +0xf4c, `do_gracza` +0x134c,
  `on_closer0..3` +0x1350, `on_closer_widzi0..3` +0x1550, `on_hurt0..3` +0x1750, `on_further0..7` +0x1950, `on_further_widzi0..7` +0x1d50, `on_reload` +0x2150, `on_death` +0x21d0,
  `sound_on_kontakt` +0x2250, `sounds_on_kontakt` +0x2454, `glos_buta0/1` +0x22d0/+0x2350, `odstep_glosow_buta` +0x23d0, `sound` +0x23d4.
* Actor (0x2f94 bytes, created 0x10042520; manager at 0x10b35858: stimuli +0, definitions +4, actors +8, frame time +0x20, player +0x24): position +4, half height +0x20, model +0x138, `hostile` +0x9b,
  dead +0x144, current phase +0x204, weapon objects +0x208/+0x20c (ammo at object +0x94, filled from item `ammo_amount` +0xb24, 0x1001bad8), place (node) +0x210, target node +0x214, moving +0x218,
  segment length/progress/direction +0x21c/+0x220/+0x28, HP +0x26c, contact +0x126, provoked +0x127, aware +0x128, seen +0x149, chase +0x14a, yaw target/current +0x170/+0x174, gun pitch
  target/current +0x178/+0x17c, rotation quaternion +0x160, fire timer +0x224, corpse timer +0x22c, path list +0x274 (count +0x2f80, index +0x2f84, per-step door flags +0x267c).
* Item fields used by the actors: `max_ammo` +0x908 (player only), `ammo_amount` +0xb24 (the clip, equal to max_ammo except `heli_bron`/`papieros`: 250), `sila_strzalu` +0xb28 (player damage),
  `sila_wroga` +0xb2c (enemy damage), `glosnosc` +0xb34, `kul_na_raz` +0xb4c (pellets), `rozrzut` +0xb54, `shot_latency` +0x8fc (player fire rate only).

## Per-frame update (manager 0x1004ac20, per actor 0x1004a7e0)
Order for a living actor (dt = frame time):
1. Actors without `ruchomy` are skipped unless a cutscene runs (`[0x10071978]`, the current cutscene object). They are cutscene props (`i_*`, `*_outro`).
2. `seen` (+0x149) = 0x10043720: the segment from the player's eye (centre + 48, crouched + 16, 0x100435b0) to the chest point (position + 0.75 * half height + `mod_y`) must be free.
   `nie_patrz_na_gracza` and a dead player give false. The result is cached and re-tested after `distance * 0.25 * 0.0015625` (= distance / 2560) seconds; a phase entry and every path node arrival force a
   new test. Cutscene: 1.
3. A phase with `strzal` != 0 whose weapon cannot see the player (0x10043860) leaves with probability 1/4 per frame (`rand() & 3 == 0`, no cutscene) for the phase `do_strzalu_patrzy`, else `do_strzalu`,
   else `estimate`, and the rest of the frame is skipped (0x1004a947..0x1004a9c1).
4. Transitions (0x10049b60), only for a **non-looping phase whose animation has ended**, in this order: `on_hurt` when HP < `ucieka_jak_mniej_niz` (0x10049bb8) > `on_koniec_anim` >
   `on_closer_widzi` (`do_gracza` > 1, distance <= do_gracza, fresh `seen`) > `on_closer` (`do_gracza` > 1, distance <= do_gracza) > `on_further_widzi` (distance > do_gracza, < 4096, seen) >
   `on_further` (distance > do_gracza, < 4096, not seen) > otherwise the phase named `estimate` if the definition has one, else `default_faza` (0x10041bf0, **without** the snap flag). Every numbered
   list is picked with `rand() % n` among the consecutive non-empty slots from 0 (0x10045150 counts, 0x10045610 picks); a gap ends the list.
5. Contact (0x10049950, only for a phase with `on_kontakt`, not in cutscenes): contact = (player in front, i.e. d.f > 0, and |horizontal angle| < `kat_kontaktu`, and distance < `odleglosc_kontaktu`, and seen)
   or (player not in front and distance < 48) or a stimulus. It sets +0x126; if there is none, +0x126 and +0x127 are cleared. **The phase only changes when +0x127 is set**, which only the script command
   `hostileattack` (0x1001a52d: every actor with +0x126, every tick, mission `action general`) and `hostile` do. Then: `sound_on_kontakt` (`rand() % sounds_on_kontakt` replaces the digit five
   characters from the end, radius 2048 seen / 1280 not), a stimulus (128 / 640 / 1280, 2 frames), and `SetPhase(on_kontakt)`.
6. Movement step (0x100483c0, `ruchomy` only, not in cutscenes). See below.
7. Turning (0x100489f0): the yaw target +0x170 is the angle to the player (+ `mod_kier` * 45 degrees) in phases with `obrot_do_gracza` (or +0x158, set by the sidestep of `estimate_kryjowka`), else the heading of the
   current path segment; +0x174 approaches it by `pi * mod_obrotu * dt`, **two identical steps per frame**. `nie_obracaj` freezes it. The clamp compares against the numeric target without regard for the
   0/2 pi seam, so a turn whose short way crosses north (facing +Z) completes at once (reproduced). The gun pitch +0x17c follows the player's elevation at 1 rad/s in phases with `obrot_pion_do_gracza0`
   and is 0 otherwise; with `obracaj_ganem_w_pionie` the weapon object is tilted by it (0x10049e80), otherwise the spine bone is.
8. The model is drawn at position + velocity + `mod_y`; velocity *= clamp((1 - dt) * 5, 0, 0.9) per frame (it holds the jump left by a snap, 0x10041d94).
9. HP below `ucieka_jak_mniej_niz` regenerates `dt` per second (0x1004aafb).
10. Step sounds: `glos_buta1` only (the toggle +0x9a is never written) every `odstep_glosow_buta` s when that is above 0.09; radius 2048 seen, 640 not.
11. `strzal N`: every N seconds `Fire` (0x10046270).

## Phase entry (`SetPhase` 0x10041c60)
Refused for a dead actor (and `patrol` phases while +0x128 is set). Resets seen timer, contact, provoked, chase, the sidestep flag; a **forced** switch while walking (script calls and callbacks; not `on_death`
or the `estimate`/default fallback) makes the actor jump to its target node (position = node floor + half height, the model glides from the old spot, 0x10041d5e). A looping phase that keeps the running
clip does not restart it; hull from the animation; `obrot` sets a relative yaw target; the timer of `strzal` restarts. Then: `strzal_raz` / `strzal_raz1` fire (0x10041f23), `patrol` starts a step (falling back to
`default_faza` when there is none), `estimate_*` runs its search, `set`/`unset` variables, `zabij` kills, `odepchnij_gracza` hurts the player for **40** and plays `wcialo.wav` (0x10042460), `sound` (radius 2048),
`setfaza0/1` on the first instance of the named definition, and `gryzie` hurts the player for **10** when he is within `do_gracza` (0x100422a7).

## Movement: actors walk on the path graph
* Nodes (writer 0x1003d700): +0 `pos`, +0xc `realpos` (floor), 8 neighbour slots +0x18 (compass slot 0 = +Z, 1 = +X+Z, 2 = +X ... 7 = -X+Z; classifier 0x1003c230 with 45 degree sectors,
  a 0/0 vector gives 5), enabled +0x70, per-slot door flag +0x61. Door flags come from segments between the floor positions raised 32 that hit a door (0x1003ca80). The `.pth` export keeps `idxN` = slot N.
* An actor's place is the node nearest to its centre by `pos` (0x1003c130). Its height is `realpos.y` + half height; its heading is `slot * 45 degrees + mod_rot * pi`.
* Speed = phase `speed` units per second: progress += speed * dt (0x100483c0); on arrival the actor snaps to the node and the overshoot is lost.
* Patrol (0x10047fc0): up to 17 rounds of three tries: `heading_slot(yaw) + (rand&1) - (rand&1)`, then `+ rand%3 - rand%3`, then `rand & 7`, where `heading_slot` (0x1003c400) is the first k with yaw < k * 45
  degrees (so slot s maps to s + 1); a candidate must exist, be free of other actors (0x10046ec0: place or target of a living actor; a missing node counts as taken), be enabled, within 8 units of floor height,
  and its link must carry no door. Then every slot from the heading without the door test. Arrival: `on_koniec_widzi` (seen and defined) else `on_koniec`.
* Estimates (dispatch 0x10047c60; beyond 8000 units `estimate_do_gracza`/`do_strzalu` end at once): `do_gracza` 0x10046f30: goal point `distance - 24` towards the player (`-16` when closer than 32, nothing
  beyond 2048); `od_gracza` 0x10047680: 1024..2048 straight away, horizontal; `kryjowka` 0x10047490: a 1..4 node chain to the left/right (slots +-2 from the player direction, path without the place) whose far
  end hides from the player, playing `bieg_lewa`/`bieg_prawa`; else the nearest (Manhattan) node with a free slot that the player's eye + 55 cannot see; else the chase; `do_strzalu` 0x100477e0: a slot chain
  (start 90 degrees off, turning towards the player; seven slots one way round, eight the other) until a node at an odd position sees the player, failing that one call in eight a hidden node 128..2000
  (Manhattan) away and >= 192 from the player (0x10047a10); `kluczy` 0x10047160: one node two links away, 90 degrees off the player direction.
  The search (0x1003bdb0) is a depth first search over enabled, unvisited neighbours that are not farther (floor distance) from the goal node than the current node, nearest to the goal first, 48 nodes deep.
  With fewer than 2 nodes or no path the phase ends through `estimate`/default and `on_koniec(_widzi)` (`kluczy` and `strzela` fallbacks exist for zig-zags).
* Following a path (0x100485d1): each arrival refreshes `seen`; the actor stops when the index reaches the count, when a chase is closer than `do_gracza`, or when the next node, a disabled node or the node after
  it is taken. The list is never cleared, and entries beyond the count are read: a fresh actor stops at `path[count - 2]`, one with a longer earlier path may reach the goal (reproduced with a persistent buffer).
  Every arrival costs one extra frame (the node is targeted once more with length 0, heading slot 5). `moze_strzelac_w_wezle` fires at every other node when the weapon sees the player.
* `o_marker_niechodzenia_postaci`: at level start (first update, 0x1003cd60) the nodes inside the volume (grown by 16, from 16 below to 48 above) are disabled; it does not stop actors physically.

## Combat
* `Fire` (0x100462e0): with no rounds left the call enters `on_reload` and refills the clip instead of shooting; otherwise one round per call (also for pellet weapons). The bullet is traced from the muzzle along the
  gun's forward axis (the hand socket's +Z, verified against the exported `bron` sockets: 0.96..1.0 in firing poses), range `640 + 24 * max(256 - rozrzut, 0)` capped at 1.5 x the distance to the player, endpoint
  scatter `rozrzut * (2 - D)` per world axis, `kul_na_raz` pellets. The first thing on the segment takes the bullet: the player `trunc(sila_wroga * D * 1.3)` (0x10006c26), any actor `sila_wroga` through `Hit`
  (so enemies wound each other and executions kill through the bullet), else the world. `D` = 0.33 / 0.67 / 1.0 (0x1002854e) is the only place difficulty enters. There are no head or limb multipliers.
* `Hit` (0x10043090): only `ruchomy` characters; `hp -= damage`; dead below zero; explosions (0x10043180) `(640 - distance) * 0.78125` within 640 with a free line, dead at zero or below. **A hit does not interrupt
  a phase** (no flinch, no on_hurt, no contact): `on_hurt` is only the wounded retreat of the transitions above.
* `Kill` (0x10042ca0): a noise, the current phase's `on_death` (no snap), dead flag, no longer solid, `deadN` scream (`graj_dzwiek_smierci`, 4024), blood pool (`plama_krwi`), weapons dropped unless
  `nie_zostawiaj_gana`. Corpse clean-up (0x10049d80): after 30 s, at least 320 units away and out of the player's sight unless `nie_respawnuj`.
* Stimuli (0x10045cf0 creates, 0x10045df0 tests; noticed inside `always`, else inside `los` or `cone` with a free segment to the actor; the "60 degree cone" compares an arcsine in radians with 60 and never fails):
  player shot (muzzle, 128, `glosnosc`, 0, 1 frame) / enemy shot (2 frames), bullet impact (200, 0, 1024, 2), hit on an actor (pos + 96: 192, 1024, 0, 2), kill (pos + 66: 256, 1280, 2048, 2), contact shout (128, 640, 1280, 2),
  explosion (196, 2048, 0, 1), laser dot and flashlight pool (0, 0, 640, 1).
* Script side: see "Script conditions and the AI flags" below (an earlier version of this line attributed `ifplayerseenby` and `setallfaza` to the contact flag: wrong).
  The same conditions tag the actor *aware* (+0x128), which blocks re-entering `patrol` phases while the tag stands (ported, see the table below).

## Deviations of the earlier approximation, now fixed
| Earlier remake | Retail | Where |
|---|---|---|
| Free walking with sweeps, BFS routes, repath every 0.35 s | Rail movement on the path graph, greedy DFS, node hops | `ai.rs` movement/estimates |
| Callbacks picked by `branch % n`, all numbered keys | `rand() % n` over consecutive slots | `slot_list`, `pick` |
| `on_kontakt` for every noticing hostile once `hostileattack` ran, sight cone `player_seen` | Contact flag, then a per-actor provocation by the script | `contact`, `provoke_noticing` |
| Cone / range defaults 1500 and 90 | 640 and 0; behind = 48 units | `contact` |
| Perfect aim at the player, item `shot_latency` as rate, `max_ammo` clip, reload right after the clip | Gun forward axis + pitch, phase `strzal N`, `ammo_amount`, reload on the next pull, pellets, enemy bullets hit other actors | `fire`, `trace_bullet` |
| Melee: `sila_gryzienia`/5 damage with LOS | `gryzie` 10 within `do_gracza`, `odepchnij_gracza` 40 at phase entry | `run_pending_start` |
| Hit triggered `on_hurt` / `on_kontakt`; dead = HP <= 0; unspecified HP infinite | No reaction; dead flag, HP < 0; HP default 0 | `damage_actor`, `kill` |
| Distance callbacks with an invented 96 default range | `do_gracza` > 1 required, 4096 cap, fallback to `estimate`/default phase | `transitions` |
| `mod_obrotu` ignored, instant turning | 2 x pi x `mod_obrotu` rad/s with the seam quirk | `turn` |
| Restarted the animation on every phase entry | Looping phases keep a running clip | `begin_phase` |
| No-walk volumes blocked actors | They disable path nodes once at level start | `setup_world` |
| No noise model | Stimuli for shots, impacts, deaths, shouts, explosions | `add_stimulus` |

## Verified vs approximated
| Rule | Status |
|---|---|
| Phase machine order, callbacks, defaults, picks, snap flag | verified in the disassembly, unit tests |
| Contact/vision (cone, reach, 48 behind, seen cache, provocation) | verified; the horizontal angle is measured from the actor centre, the original uses the weapon socket when a weapon is attached (0x10043d90) |
| Stimuli, radii, lifetimes | verified; the LOS test of a stimulus uses the world, doors and props (cshell's own ray helper 0x10051c20 takes the ignore flags per caller, see "Line-of-sight filters") |
| Path graph movement, patrol, estimates, path list quirks | verified; the door flags use the same rule as 0x1003ca80 (filter 0x100577c0 = user flag 0x8, i.e. `Gracz_otwiera` doors only, segment floor + 32, both directions); the remake tests the door's brush parts (`Door::crosses`) instead of the engine's polygons |
| Turning (double step, seam clamp) | verified |
| Fire, ammo, reload, pellets, difficulty, damage | verified; muzzle/forward come from the exported sockets and animation, the spine-bone pitch of `obrot_pion_do_gracza` is applied as a gun tilt |
| `Hit`/`Kill`/explosion rules | verified; the engine hits a model by its **dims box** (Lithtech.exe 0x42b390 on obj+4..+0x18), which cshell sets from the animation hull (0x10042b30), so the animation half extents are the retail hit box (see "Line-of-sight filters"); corpses: see there |
| HP regeneration, corpse clean-up, step sounds, sound radii | verified |
| Door opening by actors | **ported 1:1**: message 0x34 -> object.lto 0x10009270 walks the registry from its head = the LAST placed object (registry insert at the head, 0x10008a40) and stops at the first hinged door (kind 1, no `Skok_do_levelu`) whose origin is within 128 units of the request point; it opens only if that door is closed and has `Gracz_otwiera`; an open, moving or locked first door ends the search (a request never closes a door and never reaches the partner leaf of a double door); the `Nast_obiekt` chain does not follow (the registry activation after 0x10002050 meets a moving door, 0x100020c0 refuses). `doors::request_target`, `Activation::npc_requests` |
| `aware` (+0x128) patrol gate and dialogue tags | **ported**: cleared for every actor at the start of each mission tick without a running dialogue (0x1001a878), set by `ifaction`, `ifactionhostile`, `ifplayerseenby`, `ifhostileblizejniz` (0x10019da1, 0x10019f65, 0x1001a04f, 0x1001a0d7), kept through a dialogue (no action runs meanwhile, 0x1001a864); `SetPhase` refuses `patrol` phases of a tagged actor (0x10041cbe); StartDialog resets the first tagged actor to `default_faza` with the tag cleared meanwhile and makes it face the player (+0x158, 0x10019737..0x10019804); the end of a dialogue without a follow-up node clears every tag and re-enters the current phase of each tagged actor, forced (0x10019500..0x10019590). `Mission::aware`, `Event::Released`, `NpcRoster::{set_aware,reset_speaker,release_aware}` |
| Dialogue node `hostileattack` | **ported per speaker** (0x10019811: speaker +0x127 = 1, +0x128 = 0); the mission action `hostileattack` (0x1001a52d) stays `provoke_noticing` (every actor with contact). The old global `hostiles_attacking` switch only survives in save files |
| `hostile <name>` script command | not used by any script |
| Ground snap of non-static actors (0x100432e0) | see "Ground snap" below |
| Head tracking `glowa_do_gracza_dist` / `_kat` (phase +0x4b8 / +0x4bc, consumer 0x100490b0..0x10049580) | ported (2 characters: `laska czapel`, `wloski sierzant`): inside the distance, within kat degrees of the heading and seen, the head yaw follows the player, else 0; 0.9 per frame; the unknown extra test 0x10043930 is not reproduced. `Npc::head_yaw` |
| `obrot_postrzalu0/1` (75 phases) | **inert in retail**: the parser stores bone names (phase +0x5cc / +0x64c), the turning code 0x100489f0 (0x10048a2f..0x10048aab) only resolves their node indices for a DEAD actor, and the node-control callback 0x1003e720 that would rotate them is registered only for `obrot_pion_do_gracza0` of a living one (0x10048fef) and removed again by Kill (0x10042d12, which also zeroes the gun pitch, 0x10042d31). Nothing to port; Kill zeroes the pitch here too |
| Weapon drop on death | ported as retail: every armed character (`weapon` header) lets the weapon fall, a second one when `socket_weapon1` is named (0x10044e7a), unless `nie_zostawiaj_gana` (then both are removed): `Kill` 0x10042eec..0x10043085; `drop_weapon` is not a parser keyword and only hides the model. `NpcRoster::drain_drops` |
| `show_weapon` gate | ported: the weapon object follows the hand only in phases with `show_weapon` (0x10049f38 / 0x1004a20b), the flag that made it visible is sticky (0x10041ed1) |
| Prologue keys | `nie_sprawdzaj_drzwi` ported (no door flags, 0x1003ca80, `Mission::level_setting`); `gestosc_sciezek 4` only matters while a graph is generated from actor positions (0x1003bb80, 0x1003c650, 0x1003ceb0, then written by 0x1003d700) and all 31 worlds ship a `.pth`: moot |
| Hidden console variables `God`, `FullStamina`, `Invisible`, `DrawKilledCount` (+ `DrawPaths`, `DrawPostacPaths` read only) | ported from autoexec.cfg, default off (poll 0x10059230): `RetailOptions`, `Campaign::god`, `Player::full_stamina`, `NpcRoster::invisible`, `hud::killed_count` |
| Random skin choice (`rand() % ile_skinow`) | the earlier deterministic choice by object index stays (both are valid original outcomes) |
| Frame time | the retail update is frame based (speed * dt per frame, overshoot lost); the port uses the same rules with the game's clamped dt |

## Line-of-sight filters and the hit box (audit round, cshell 0x10057750..0x10057ac0, Lithtech.exe 0x42aa10..0x42b390)
Every world query of the game is `ILTClient::IntersectSegment` (client vtable +0xc8) with `m_Flags = 1` (objects; cshell never sets IGNORE_NONSOLID 2, nor "from inside" 8)
and one of seven filter functions. **The engine side** (Lithtech.exe 0x42aa10 sets up the query, 0x42ae30 is the per-object callback): every object is a candidate
(objects without FLAG_SOLID 0x2000 or the ray flag 0x1000 too, because bit 2 of the query is clear); a sphere test (centre obj+0xc4, radius obj+0xb0 = length of the dims + a
constant, written by SetObjectDims) comes first, then the filter, then the test proper: **world models (type 2, 9) polygon by polygon through the BSP callbacks; every other
object, models included, as the axis aligned box obj+4..obj+0x18 (segment against box, 0x42b390)**. A segment that starts inside a box never hits it (0x42b7f2 needs the
query flag 8). So the retail hit box of a character is its dims box; cshell sets the dims from the animation hull at every phase entry (0x10042b30, `SetPhase` "hull from the
animation"), and Kill does not change them (0x10042ca0), so the animation half extents are the retail hit boxes of living characters and of corpses.

User flags (object flag set 2) set by the game: 0x2 shoot-through (`b_transparent` 0x6, `b_szuflada_przestrzelna` 0x202; object.lto 0x10004e18, 0x100045f4), 0x4 marker of the
`b_transparent*` classes (`nieprzestrzelny` has 0x4 only, 0x100053b9) and the `przezroczysty_dla_blikow` props (0x1002c502), 0x8 a door or drawer with `Gracz_otwiera`
(0x10001959, drawers 0x208 at 0x10003b91), 0x100 a closed `b_door` (0x100019d2), 0x200 drawers, 0x10 a living character (0x100444db), 0x80 a dead one (Kill replaces the flags by 0x80
and clears FLAG_SOLID, 0x10042d79..0x10042db9), 0x20 the laser dot / flashlight object (0x10051be9), 0x40 an item (0x1001cf92, thrown items 0x1001d2b2). Type codes: 0 normal, 1 model, 2 world model.

| Filter | Callers | Does **not** block (returns false) |
|---|---|---|
| 0x10057750 | the spawn snap 0x10043413, node links 0x1003ca43 | the ignored object, user flag 0x8 (usable doors) and 0x40 (items), any type but 0/1/2 |
| 0x100577c0 | node door flag 0x1003cc6a / 0x1003ccc6 | (not a blocker: it returns true for user flag 0x8 objects only, which is what "this link crosses a door" means) |
| 0x10057820 | hide test of `estimate_kryjowka` 0x10047130, wall splat 0x100095b0 / 0x10009636, melee/laser 0x1000f7e2 | the player, living characters (0x10), glass (0x2), items (0x40) |
| 0x100578a0 | **bullet trace 0x10005ce0** (every shot of the player and of actors) | the shooter, a world model without FLAG_SOLID, glass (0x2), items (0x40); everything else, **living characters, corpses, props, other models** stop it |
| 0x10057940 | **`seen` 0x10043720, weapon sight 0x10043860** (via 0x100435b0) | the player, the actor itself, a non-solid world model, glass (0x2), items (0x40); **every other character (living or dead) and prop blocks the sight** |
| 0x100579f0 | the ray helper 0x10051c20 (stimuli, explosions, effects): per call it skips two named objects and optionally laser dots (0x20), living (0x10), glass (0x2), dead (0x80) | items (0x40) always |
| 0x10057ac0 | use probe 0x1005fcc6, flying items 0x1001c9e5 | only the player |

Consequences in the port: `Frame::sight` makes other characters (living and corpses, from the frame-start boxes) and living props block `seen` and the weapon sight, the hide
test of a cover estimate meets corpses but skips living characters, explosions stop at other characters, node door flags come from `usable` doors only (`Door::usable` =
user flag 0x8), and the `blocked` callback of the roster includes props. `ifaction` has no sight test at all (0x10019b80 only tests the box at the probe point).
Not reproduced: the exact polygons of world models (the door brush parts stand in), props that are not in the `PropField`, the glass / living / dead variants of 0x100579f0 per caller
(the stimulus rays use world + doors + props), the use probe 0x1005fcc6 being blocked by any model between the eye and a prop.

**Corpses and bullets** (0x10006836..0x10006c6d): the bullet filter lets a dead character's box through to the trace. For a hit on a corpse (object type 1, user flag 0x80) the trace
finds the actor; when the hit point is below `centre.y - half height / 2` (0x1000686b) the bullet **ends there** with the blood splash 0x10005920 (krew1 sprite + six bits), no `Hit`
and no impact stimulus; above that line the bullet flies on from the hit point (0x10006963: 16 units further, or to the living character whose box, grown by 8, contains the point,
0x100068ab..0x100069d7). `NpcRoster::corpse_stop`, `gunfire::shoot_outcome`, `trace_bullet`.

## Ground snap (0x100432e0)
A segment from the actor's centre 256 units straight down (filter 0x10057750); if it hits, the centre becomes the hit point plus the half height (the hit point must be further than
16 from the origin and finite, which is how "no hit" is detected). It runs for an actor whose default phase has no `static` command (0x10043520, called once by the manager update
for every new actor: 0x10043589) and every time the phase named `estimate` is entered (0x10041d0e). Port: `snap_spawn` at the spawn, restricted to a drop of at most 32 units onto a
walkable surface because the engine ray also meets props and world models that are not in the static hull (a larger drop or a slope means something else stands below: a car roof,
a table); the survey test `snap_survey_of_the_spawn_positions` (ignored, needs the local exports) lists what the unrestricted rule would move (hundreds of actors, many by 5..20 units:
they were floating). The re-snap at `estimate` is not repeated: the path nodes carry the floor heights.

## Actor list order (open)
The actor manager links a new actor at the head of its list (0x10042520 stores the old head in +0x2f90), and every "first instance of a definition" rule (`setfaza`, `ifalive`,
`ifdead`, `ifgraczblizejniz`, `ifaction`, StartDialog's first tagged actor) walks from that head, so it finds the actor created LAST. The creation order is the record order of
`scripts\cs\facety.txt`, which each `o_postac` appends to in its first server update (object.lto 0x10017f9b..0x100181fc), i.e. in the server's update order; the object list
order of the LithTech server is not decoded, so whether "first" means the first or the last placed character of a definition is **unknown** (the remake uses the placed order).
The chain registry (0x10008a40 also inserts at the head) is known to be walked newest first, see the door requests above.

## Headless probe
`MESTER_SILENT=1 MESTER_TEST_SCENARIO=ai [MESTER_TEST_AI=fight|far|patrol] [MESTER_TEST_NPC=name] [MESTER_TEST_NPC_DISTANCE=n] [MESTER_TEST_END=8] level-viewer <world> ../../output <capture.png> <t1,t2>`
(from `crates/level-viewer`, `timeout 120`). The probe waits for the opening cutscene, stands the player in front of an armed character (or counts him as dead for `patrol`), plays the mission's
`hostileattack` every frame and prints `AI PROBE` lines; failed checks exit with an error. Example results: `rh1-wiezienie2` `o_postac19` (`policjant`, reach 3000, cone 89) notices the player at 447 units within
0.2 s, follows `alarm0 -> alarm1 -> estimate -> strzela -> strzela0` and shoots once per animation cycle; `rh3-miasteczko0` patrol `o_postac0` walks 46 units/s on a phase speed of 48 and never leaves a link.
`cargo test -- --ignored` also runs `audit_every_level_runs_every_actor_for_a_minute_without_errors` (787 actors, 28 levels, 60 simulated seconds each) and the socket alignment check.

## Reconciliation with the weapons audit (docs/retail-weapons-audit.md)
Both audits decoded the same stimulus node (0x10045cf0). The radii agree; where they differ the AI evidence wins: the life is 1 for a
shot of the player (call flag set, 0x10005cc9) and 2 for a shot of an actor (0x10046c0d), see `Stimulus::player_gunshot` / `gunshot`. **Settled once for all:** 0x10005dd9 computes `life = (flag != 0) ? 1 : 2` (`neg al; sbb eax,eax; add eax,2`); the manager update (0x1004ac20) runs every actor (0x1004ad39..0x1004ade6) and only then ages the list once (0x10045da0 at 0x1004adf1: `life--`, removed at 0). A player shot is therefore seen by exactly ONE actor pass and an actor's shot by two consecutive passes, on whichever side of the pass it was created (the weapons audit's "one or two passes" expressed that uncertainty); a wound by a melee
weapon (item +0xb18) is (64, 0, 1024) instead of (192, 1024, 0) (0x10043113). Explosions hurt through their own loop and raise only the explosion stimulus.
The stimulus creators are: weapon trace (0x10005ce0: shot and impact), laser (0x1000ee32), flashlight (0x1000b7c0), `Hit`, `Kill`, contact shout,
explosion (0x1005b120). **No creator lives in the player movement code: NPCs do not hear footsteps.** The player's eye for every actor test is
hull centre + 48 (crouched + 16, 0x100435b0, taken from `Player::half_size`); the gun aims at centre + 32 / 16.
`block_player` tracks the hull centre (a stance change moves the feet, not the player) and sets `Player.external_support` when the hull lands on an actor's box.

## Script conditions and the AI flags (audit of every condition in `output/decoded_scripts`)
Two different actor flags feed the scripts, and only one of them depends on `on_kontakt`:
* **`seen` (+0x149)**: the player's eye sees the actor's chest (0x10043720), refreshed in every update of a `ruchomy` actor whatever its phase (an actor
  without `nie_patrz_na_gracza`; a cutscene forces 1). No `on_kontakt`, `kat_kontaktu` or `odleglosc_kontaktu` is needed.
* **contact (+0x126)** and provoked (+0x127): written by 0x10049950, which returns at 0x10049999 when the *current phase* has no `on_kontakt` name (`+0x6cc`
  empty), and at 0x100499ac when the player is dead. The disassembly agrees with `npcs/ai.rs::contact`; the `sound_on_kontakt` name check at 0x10049a7f
  (+0x2250) only chooses the shout. So an actor without an `on_kontakt` phase never has a contact flag.

| Condition (uses in scripts) | Evaluator | Reads | Fed by the port |
|---|---|---|---|
| `ifplayerseenby NAME` (19) | 0x1001a060 | some living actor whose definition is NAME (case-sensitive `strcmp`) has `seen`; tags it aware (+0x128) | `Npc::seen_player()` -> `NpcState.player_seen` |
| `setallfaza PHASE NAME` (4) | 0x1001a6a2 | every living actor of the definition with `seen` | `campaign.rs`, `seen_player()` |
| `ifhostileblizejniz N` (19) | 0x10019fc0 | living actors with `hostile` (+0x9b) and `seen`, distance to the player **<= N** (`fcom`; C3 counts), the nearest one is tagged | `player_seen` + `hostile_npcs`, `<=` |
| `ifseenbyhostile` (13) | 0x10019f70 | a living actor with `hostile` and the **contact** flag | `NpcState.contact` = `Npc::noticed_player()` |
| `ifactionhostile` (10), `ifaction` (66) | 0x10019db0, 0x10019b80 | the use probe (a point 72 ahead of the eye inside a living actor's box grown by 32; the action key is asked level triggered, there is no line-of-sight test; tags the actor aware) | `NpcRoster::use_target` |
| `ifalive`, `ifdead` (87, 19) | 0x1001a40c, 0x1001a446 | first placed instance of the name, dead flag (+0x144) | `Npc::alive()` |
| `ifgraczblizejniz N NAME` (27) | 0x1001a36a | first instance, distance actor -> player **< N** and N > 1 | `NpcState.position` |

Before this audit `ifplayerseenby`, `setallfaza` and `ifhostileblizejniz` were fed with the contact flag. Every script that uses them for a character without an
`on_kontakt` phase was dead: `wiezien_kuchnia` (dialogue Wiezien27, mission critical), `wiezien9`, `wiezien_stolowka`, `burmistrz`, `cywil1..4/7/1p`
(`cywil1..4` also through `setallfaza`), `dziwka`. Two names in the scripts match no definition in retail either (`TongPo` differs in case from `tongpo`, `celi` does not
exist), and `dziwka` has `nie_patrz_na_gracza`, so her condition can never be true. The contact flag is only needed by `ifseenbyhostile` (hostile characters
with `on_kontakt`), which was already right.

## Noise: a lone player shot does not alert an idle guard
* Stimulus life: 0x10005dd9 passes `life = 1` when the last argument of the weapon trace (0x10005ce0) is set (the player, 0x10005cc9) and 2 for an actor (0x10046c0d).
  The manager update (0x1004ac20) runs every actor (0x1004a7e0, contact) and only then ages the stimuli (0x10045da0), so a life-1 node is seen by exactly one pass.
* 0x10049a4e..0x10049a65: a noticed actor gets +0x126 = 1; an actor that is *not* noticed gets +0x126 = 0 **and +0x127 = 0** in the same pass. Only the mission's
  `hostileattack` (0x1001a52d, for every actor whose +0x126 is 1 at that moment) sets +0x127, and the reaction needs +0x127 at the *next* pass. A one-pass stimulus is
  therefore gone (contact 0, provoked cleared) before the provocation can be used, whatever the order of the mission tick and the actor pass inside the frame.
* Two-pass stimuli work: enemy shots (2), the bullet impact (0x10007e17, fixed 2: 200 near, seen within 1024 with a free line), a wound (0x10043128, 96 above the feet:
  192 near / 1024 heard, or 64 / seen 1024 for a melee weapon), a death, a shout. So a shot that lands within 1024 units of an idle guard with a free line from the
  impact point, or that wounds someone near him, alerts him; a shot into the distance does not. The weapons audit's claim that "shots alert NPCs" is true for
  the impact and wound stimuli, not for the muzzle noise of the player's own shot (128 near / `glosnosc` heard, one frame): that one only matters for actors that are
  already in contact through another cause (and enemy shots, which live two frames).
* The `noise` probe therefore shoots at the guard (wound = reaction expected) and, with `MESTER_TEST_NOISE=away`, into the distance (no reaction expected).
