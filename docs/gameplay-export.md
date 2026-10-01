# Original prison gameplay export and character runtime

The exporter reads the installed game directory without modifying it. It uses
the actual merged `scripts/postacie.txt`, `scripts/items.txt`,
`scripts/ai/gameai.txt`, `scripts/ai/dialogi.txt` and `scripts/text_keys.txt`.
Most separate per-level AI files in this installation are empty stubs.

## Reproduce

From the remake directory, using the available Python runtime:

```text
python -m tools.export_gameplay ../GYARI output --worlds rh1-wiezienie1 rh1-wiezienie2 rh1-wiezienie3 rh2-wiezienie1 rh2-wiezienie2
```

The existing scene exports for those worlds are prerequisites. The exporter
uses the existing LTB and DTX decoders and writes only to the output directory.
Audio files are copied; export and verification do not play audio.

| World | Placed characters | Inactive emitters | Navigation nodes |
|---|---:|---:|---:|
| rh1-wiezienie1 | 15 | 1 | 509 |
| rh1-wiezienie2 | 34 | 4 | 1,764 |
| rh1-wiezienie3 | 67 | 6 | 4,913 |
| rh2-wiezienie1 | 25 | 8 | 2,467 |
| rh2-wiezienie2 | 45 | 1 | 6,317 |

The current export contains 186 placed characters, 20 authored emitter
definitions, 41 models including weapon attachments, 56 decoded textures and
50 copied speech/sound files. `gameplay-export.report.json` records the paths,
asset summaries and original-data warnings.

## Portable files

`<world>.gameplay.json` has format `mesterlovesz-gameplay-v1`:

- `characters`: keyed by the exact semantic `postac` name used by mission
  scripts. Includes model, skins and original skin variants, source header,
  HP, explicit hostile flag, weapon definition, default phase, animation
  metadata, bounds and every named phase.
- `npcs`: every original placed `o_postac`, linked by exact world `Name`,
  original object index, semantic character name, position and rotation.
- `emitters`: original `o_emiter_postaci` definitions, kept separate from
  initially placed actors. `properties.Nast_obiekt` preserves trigger chains.
- `markers` and `interactables`: original marker, door, drawer, item and
  cutscene properties; no synthetic objective or kill gate is introduced.
- `ai`: original world header and ordered action command arrays.
- `dialogues`: reachable original dialogue nodes, resolved Hungarian text,
  original speech reference, copied audio path, choices and ordered commands.
- `navigation`: original `.pth` node IDs, positions, real positions and
  neighbor links, with source provenance.
- `sources`: original relative filenames and SHA-256 hashes.

Each entity keeps its complete original `properties` map. `property_metadata`
also retains the DAT property type and flags, which the older scene JSON did
not expose. Command arrays contain pairs in original order and retain repeated
and numbered commands. Semantic names can contain spaces; do not split them
into individual words or confuse them with world instance names.

`gameplay_scripts.json` includes the complete decoded `gameai`, `dialogues`
and `text_keys` strings. The source text is also available under
`decoded_scripts/gameai.txt` and `decoded_scripts/dialogi.txt`; existing
`decoded_scripts/text_keys.txt` and `postacie.txt` remain usable. The mission
runtime parses complete original text, rather than reconstructing logic from
the convenience JSON summaries.

## Coordinates and models

Positions use original LithTech coordinates, Y up. Renderer scale is 0.01.
Object rotation is the original four-number property; the renderer consumes
its first three Euler values as Y-X-Z.

Model JSON adds `bounds.min`/`bounds.max` from bind-pose vertex positions.
`collision_half_extents` and each animation's `dimensions` are the original
LTB centered object half extents. They are **not** feet coordinates or full
width/height. Each animation retains its exact root `translation`, bone and
vertex animation data. The renderer applies that translation before skeleton
and socket composition. Characters commonly have root translation around
`[0,-57,0]` and half-height 57.

The runtime keeps each authored X/Z spawn coordinate. It treats the DAT Y as
the collision-hull center during movement and raises a spawned actor only if
the original static floor cuts into that hull. Treating this center as a foot
coordinate previously dropped walking characters roughly half a body into
the ground. A local audit covers 922 placed actors and emitters across 28
exports; the seated mayor and one cutscene prop do not have standing feet.
Some source actors stand on props absent from the static collision OBJ. Their
patrol movement retains the authored elevation when the next static floor is
more than a 24-unit step below them; it no longer drops to that floor at once.

Third-person weapons use the original item `mesh`, `texN` and `rsN`, attached
to the character's original `socket_weapon`. They do not use first-person
weapon offsets. `policjant z pala` has a weapon definition but no attachment
socket in its original header; no additional socket is invented.

## Runtime integration

`crates/level-viewer/src/npcs.rs` supplies:

- `NpcRoster` resource with public active `actors` and `hostiles_attacking`.
- `setup_world`: prepares original bodies and attachments, caching parsed
  models. Called by the existing world setup after old world geometry removal.
- `tick`: phase animation, bounded navigation, source contact callbacks and
  player-targeting strikes. Menu pause and opening stop this runtime; choice
  dialogue pauses AI while its poses continue animating.
- `block_player`: solid, living NPC hull collision after controller movement.
  It corrects both retail and IW4 feet/velocity and the camera together; IW4
  weapon, stance and timing state is preserved.
- `set_phase(instance_or_semantic_name, phase)`: changes every matching
  instance and queues its original phase commands. The return is informational.
- `drain_commands()`: returns `(world_instance_name, ordered_commands)` once
  per entry. Pass these to mission `phase_enter`; do not also apply the
  `set_phase` return value. The NPC adapter handles `on_death` and animation
  completion callbacks internally, including the subsequent queued phase.
- `activate_emitter(name)`: activates a prepared original emitter actor once.
  Before activation it is absent from the active actor roster and invisible.
  `emitter_next(name)` exposes its exact authored next target; the campaign
  handles marker/door/emitter chains with a visited set.
- `hit_scan`, `nearest_target`, `drain_damage`, plus per-character `alive`,
  `eye`, `player_seen` and `player_seen_with_doors`. Hit/interaction results use world instance names;
  mission `ifaction` conditions use that instance's semantic definition name.

Ordinary hostile actors contact the player after the mission's original
`hostileattack`. Special guards still execute explicitly requested attack
phases. Contact vision uses source distance/cone fields and a static-world
ray. Current moving door transforms also block contact, damage and visibility
branches; source shoot-through grates permit shots but retain physical collision.
Strikes require explicit player-targeting phase commands and clear line of
sight. Gunshots then trace the individually scattered ray against the active
retail or IW4 stance hull, the static world, and current moving doors. A miss
still consumes the original weapon interval and plays the authored gunshot.
Thus the riot policeman's `zabija2` shot is not redirected at the
player. Hits stop at static walls and select only the nearest intersected live
actor. Hit hulls use original animation dimensions.
HP-unspecified actors (including source vehicles) still return hits while
retaining unlimited HP. `NpcHit.bleeds` follows the original `nie_krwaw` flag;
the API does not itself render blood particles.

Direct phase `sound`, `sound_on_kontakt`, and weapon `sound_shoot` references
play their copied original WAVs. The riot gunshot remains audible without
fabricating player damage. Silent probes/captures do not create audio players.
Weapon cooldown survives phase changes; original 0.05-second SIG and
0.06-second helicopter intervals are not rounded up to 0.1 seconds.

## Recovered NPC gunshot math

The original `cshell.dll` NPC call at `0x10046c0d` passes zero as the final
argument to shared shot routine `0x10005ce0`; the player wrapper passes one.
The NPC branch uses the weapon's `rozrzut` and `sila_wroga` fields:

- Base ray length is `640 + 24 * max(256 - rozrzut, 0)`
  (`0x10005dde`–`0x10005e0d`). The NPC path caps it at 1.5 times the distance
  from the shot origin to the player reference position
  (`0x10005e14`–`0x10005ea4`).
- Effective spread is `rozrzut * (2 - D)` (`0x10005fd2`–`0x10005fe6`). Each
  world-axis endpoint component gets an independent offset
  `(rand() % 1000 - rand() % 1000) * effective_spread * 0.001`
  (`0x10005fec`–`0x1000619e`). This is endpoint scatter, not a percentage
  chance to hit or an angular cone. Zero spread consumes no random values.
- Player hit damage is `trunc(sila_wroga * D * 1.3)`
  (`0x10006c26`–`0x10006c68`). Original difficulty choices map to
  `D = 0.33, 0.67, 1.0` (`0x1002854e`–`0x10028587`). The original global
  initializer is 1.0 (`0x1001ab7f`), also the adapter's default.

For example the SIG retains spread 128, enemy strength 10 and a 0.05-second
interval. Its successful hit deals 4, 8 or 13 points according to difficulty;
its hard-difficulty ray length before the distance cap is 3,712 native units.
Weapon strength and interval are not reduced to compensate for missing spread.
The previous unconditional hit damage has been removed.

The adapter uses the 15-bit MSVCRT `rand` sequence
(`state = state * 214013 + 2531011`, wrapping 32 bits; return bits 16–30),
initialized to seed 1 when the roster is created. Its local stream persists
across world loads. Original seed and whole-game random-call history remain
unverified, so individual shots are reproducible without claiming identical
original-game random outcomes. No audio is played by these tests.

## Fidelity limits

This delivers original actors and script phases, not a complete recovered
LithTech character engine. Current approximations are explicit:

- Character AI (phase machine, rail movement on the `.pth` graph with the greedy path search, contact/provocation, stimuli, aim, ammo/reload,
  pellets, NPC-to-NPC bullets, melee, hit/death rules) is a port of the retail actor update: see docs/retail-ai.md, which lists what is
  verified and what is still approximated (door opening rule, bounding boxes instead of model hit tests, the `aware` flag).
  The scripted riot shot remains visual animation and source audio without a fabricated player target.
- Source `show_weapon` and `drop_weapon` control attachments; a dropped weapon
  is hidden rather than spawned as a collectible by this module. Physical
  knockback, bone-specific look offsets, particles, footstep audio and all
  original render styles are not fully implemented here. NPC audio currently
  uses the game's shared volume without the original spatial attenuation.
- Emitters currently activate at most once per authored emitter per world
  load. Repeating wave/respawn policy has not been recovered.
- NPC motion and vision use the static CollisionWorld and current moving-door
  shapes. Door contact stops motion; full alternate path planning around a
  closed door and mutual NPC collision are not yet recovered.
- Some original phases reference missing clips, such as `sniper/nuda`,
  several `policjant/reload` phases and the bus's `jedzie`. Source names remain
  intact in JSON; rendering picks a known stable clip. All 25 export warnings
  are in the report, including one missing sound and three undefined guard
  dialogue references. These are not silently replaced with invented scripts.

The five-world export establishes the authored prison sequence and assets.
It does not establish a measured 10–20 minute, end-to-end fidelity claim.

## Verification

Independent export checks confirmed all 186 placed names exist in their
source world, all referenced models/textures/copied dialogue audio exist,
every animation retains translation and all 31 non-null character attachment
socket definitions resolve across the five worlds.

NPC unit tests cover static wall hit occlusion, nearest hits, death and
animation-end command queues, semantic matching of multiple instances,
source contact vision, moving-door occlusion, single-strike damage, continuous
fire, original weapon latency across phase changes, scripted execution
isolation, emitter activation and player/NPC body sweeps. Gunshot tests cover
the original CRT endpoint offsets, zero and nonzero spread, source difficulty
damage, finite range, and a scattered ray blocked by a wall or moving barrier
despite clear center aim. An optional local-assets test
loads every original character animation and checks finite bone/socket poses:

```text
cd crates/level-viewer
cargo test --offline npcs::tests -- --include-ignored
```

No verification command needs audio playback or interaction with the original
game installation.
