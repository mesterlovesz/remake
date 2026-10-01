# Original first prison missions — source research and runtime handoff

Research date: 2026-09-28. Original inputs are read only. This documents the
authored sequence; it does not establish a measured 10–20 minute playthrough.
The faithful playable target is the prison progression below, with the actual
dialogue, actors, pickups, doors and consequences. Do not add a kill-all gate,
key hunt, quest giver or synthetic objective that the original data lacks.

## Source files that actually contain the game

- `GYARI/scripts/ai/gameai.txt`: 44,947 bytes, merged game/level action rules.
  `scripts/ai/ai_w2.txt`, `ai_w3.txt`, `ai_wiezienie2_1.txt`,
  `ai_wiezienie2_2.txt` are **empty stubs**. Their commented includes in the
  merged file are not executable includes.
- `GYARI/scripts/ai/dialogi.txt`: 45,888 bytes, complete dialogue graph.
  `dialog_wiezienie1.txt`, `dialog_wiezienie2.txt`, `dialogi_mason.txt` are empty.
- `GYARI/scripts/postacie.txt`: merged character headers and named animation/AI
  phases. The headers' semantic `postac` names are the identifiers referenced by
  mission scripts; world `o_postacNN` names identify placed instances.
- `GYARI/scripts/text_keys.txt`: Hungarian lines and original WAV paths.
- `output/<world>.scene.json`: exact object type, name, position, rotation and
  editable properties. `o_postac.Model` maps to a character header's `model`.
- `output/<world>.gameplay.json`: portable exporter output, with complete ordered
  phase commands, character definitions, placed instances, markers, items,
  interactables, navigation, dialogue nodes and mission action commands.
- `output/gameplay_scripts.json`: full decoded `gameai`, `dialogues`, `text_keys`
  strings. Decode originals as CP1250 through `tools.decode_scripts.decode_text`.
  The three text fields preserve original source line numbers.

The line numbers below refer to decoded originals, with blank lines retained.
Coordinates are native LithTech `(x,y,z)`, Y up; renderer scale is 0.01.

## Authored world order

| World / Hungarian loading title | Entry | Authored exit |
|---|---|---|
| `rh1-wiezienie1` / Bevezetés | Existing 77.97-second opening | Last opening phase runs `rh1-wiezienie2` |
| `rh1-wiezienie2` / A Lázadó | StartPoint0 `(124,13,938)`, yaw `2.616247892` | `b_door20` `(-1328,-176,80)` and `b_door21` `(-1232,-176,80)` both have `Skok_do_levelu=worlds\rh1-wiezienie3` |
| `rh1-wiezienie3` / Keress fedezéket! | StartPoint0 `(512,-337,1563)`, yaw `3.160790920` | `b_door0` `(2620,-48,-144)` has `Skok_do_levelu=worlds\rh2-wiezienie1` |
| `rh2-wiezienie1` / Találj kiutat! | StartPoint0 `(-864,-180,1248)`, yaw `1.581267953` | `sector2b00` `(952,-432,-400)` has `Skok_do_levelu=worlds\rh2-wiezienie2`; paired `sector2a00` is `(952,-432,-496)` |
| `rh2-wiezienie2` / A Menekülés | StartPoint0 `(-2714,-446,-1144)`, yaw `5.913175106` | Further transition needs separate source inspection; do not invent an exit |

Add PI to these authored yaw values for the existing Bevy camera convention.
The door transitions do not have a source script condition requiring every
enemy to die. Preserve door properties: `Gracz_otwiera`, `Obrot`,
`Predkosc_obrotu`, `Os_obrotu`, `Przesuniecie_osi`, `Czas_samozamkniecia`,
`Nast_obiekt`. Paired doors can point back to each other; propagate one activation
with a visited set to prevent infinite recursion.

## `rh1-wiezienie2`: guards, common room, riot, escape

The level's merged rules occupy `gameai` lines 392–978. The original begins
peacefully for the ordinary hostile police. `GeneralAttack` invokes
`hostileattack` only after `Wiezienie1Walka` is set. `CzyGeneralAttack` sets that
flag when the player draws a weapon, so drawing one has a concrete
consequence in the original encounter.

### Opening markers and guards

| Object / semantic NPC | Position | Source behavior |
|---|---|---|
| `o_marker_dialog1` / `MasonStart1` | `(80,-3,768)`, extents `(32,32,32)` | `Fight02T`: “A fenébe! Ki kell jutnom innen.” Sets `IntroOdpalone`; resets `CzasOdGadki` (`dialogi` 1467–1475) |
| `o_postac14` / `straznik przy celi` | `(-80,-3,672)` | Alive, sees player, Intro flag true, more than 3 seconds since dialogue: `StraznikPrzyCeliOstrzega` (`gameai` 815–825) |
| `o_postac23` / `straznik na klatce` | `(726,-5,622)` | Within 192: one warning; within 96: `atakuje` (`gameai` 902–922) |
| `o_marker_dialog2` / `MasonStart2` | `(888,-195,512)`, extents `(128,64,2)` | `Fight03T`: “Ha átjutnék azokon a kapukon....” |
| `o_postac88` / `wiezien9` | `(464,-217.9846,-400)` | After cell guard warning, if prisoner sees player, `Wiezien25` greets them once and sets `WiezienPalaczPowital` |

The cell guard warning is `Mutiny02T`, telling the player to go to the common
room. The stair guard warning is `Mutiny01T`, also directing them there.
The cell guard walks toward the player after warning until the smoker welcomes
the player; at distance under 96 he attacks. Character phases implement
`stoi_z_pala -> bije_pala1 -> bije_pala2 -> nuda`, using `on_koniec_anim0`.
The second strike contains `odepchnij_gracza`. `do_gracza` has authored speed 48,
stopping distance 24. These phase-enter `set`/`unset` flags must reach the mission
engine or the guard's top-level rules repeatedly restart its animation.

### Riot and consequences

| Character | Placed object and native position | Phase change |
|---|---|---|
| `wiezien bije` | `o_postac15` `(872.1858,-221.9846,-751.5270)` | `napad`, animation `napad` |
| `policjant bity` | `o_postac13` `(880,-198,-624)` | `napad`, animation `napad` |
| `policjant zabija` | `o_postac86` `(1052,-198,-772)` | `zabija`, then `zabija2`, `zabija3`, `mierzy` through original animation callbacks |

The trigger is player distance below **360** to `wiezien bije`, unless the
forbidden-door flag is already set (`gameai` 472–481). Crossing
`o_marker_dialog9` at `(1197,-169,-50)`, extents `(64,64,64)`, runs flag-only
dialogue `WiezienieWlazlWDrzwi`, which also triggers the riot and immediately
sets `Wiezienie1Walka` through `Bunt0/Bunt00`.

`Bunt2` starts the assaulted guard phase. `Bunt3` starts the shooting policeman
phase, sets `PolicjantZabil`, and plays `PolicjantZabil` / `Mutiny03T`.
The gunshot occurs through `strzal_raz` in the policeman's `zabija2` phase, not
at initial trigger time. Prisoners 1/2/3/7/8/9/10 then enter `napad` and their
matching `WiezienNSchowany` flags are set. Interacting with them selects their
fearful dialogue nodes; before the riot, prisoners 7/8/9 use Wiezien22/23/24.
Prisoners 4/5/6 always have their own dialogue.

The semantic names and NPC positions are exported. Do not mark every prisoner
hostile merely because its model is in the encounter. Ordinary enemy headers
contain the explicit `hostile` flag; special baton guards are script-driven.

### Original items and progression objects

- Original FN shotgun: `o_item_podnoszony28` near
  `(-1355.4753,-176.7216,697.1891)`, type `FN shotgun`.
- Nearby shotgun ammo: `o_item_podnoszony29` `(-1365,-163,631)`, type
  `Mossberg ammo`. The source name mismatch is intentional evidence; preserve it.
- Glock ammo and healing supplies are in the right-hand guard rooms around
  `(1090,-191,280..320)` and `(-1500,-176,550)`.
- `b_door25` at `(-1014,-164.4588,727)` points to `krataceladol` through
  `Nast_obiekt`; the target is a moving grate. Supporting only hinged doors
  will leave this original activation chain incomplete.
- `o_marker_wykrywacz_postaci7` at `(256,0,352)` with extents `(32,256,240)`
  targets `akcja3`; other detector markers activate authored NPC emitters.
  Several target names have no matching current exported object. Report source
  dangling links; do not spawn invented reinforcements for them.
- Proceed through the left-hand exit door pair to `rh1-wiezienie3`.

## `rh1-wiezienie3`: cover, common room and ventilation route

Merged rules occupy `gameai` lines 979–1080. This map invokes `hostileattack`
unconditionally. It sets `CzyWiezienie` and uses `Tension_s.wav` for music.

Two living prisoners give automatic one-time dialogue when within **256** and
able to see the player:

- `wiezien_stolowka` (`o_postac25`, `(-544,-360,96)`) enters `gada` and plays
  `Wiezien26` / `Mutiny19T`: “Vigyázz haver, a sarok mögött őr áll.”
- `wiezien_kuchnia` (`o_postac28`, `(-768,-424,-1184)`) enters `gada` and plays
  `Wiezien27` / `Mutiny20T`: “Kitörni, mi? Próbáld a szellőzőaknát, az kivezet
  a cellablokkból.” This explicitly identifies the ventilation shaft route.

The authored Mason voice markers are useful progress evidence:

| Marker | Position / extents | Dialogue and meaning |
|---|---|---|
| `o_marker_dialog3` | `(-656,-335,552)` / `(96,32,4)` | MasonStart3 / Fight04, comment about brushing teeth |
| `o_marker_dialog4` | `(-745,-382,-1545)` / `(32,32,32)` | MasonStart4 / Fight05, “Nem hinném, hogy itt kijuthatok.” |
| `o_marker_dialog5` | `(-108,-217,-1507)` / `(32,32,32)` | MasonStart5 / Fight06, “Igen, ide be kéne férnem.” |
| `o_marker_dialog6` | `(-1325,-447,-1627)` / `(32,32,32)` | MasonStart6 / Fight07, outside air again |
| `o_marker_dialog7` | `(-1408,-326,-999)` / `(32,32,64)` | MasonStart7 / Fight08, too quiet |

The exact traversal order among optional side markers is not proved solely by
their identifiers. Player navigation, usable grates and combat must still be
tested on the loaded geometry. `b_door0` is the proved next-map handoff.

## Next prison section: original execution event

`rh2-wiezienie1` rules occupy `gameai` 1081–1222. Police attack unconditionally.
An execution trio is represented by `wiezien egzekucja`,
`policjant prawa egzekucja`, `policjant lewa egzekucja`.

The prisoner group cycles phases that set `WiezienEgzekucjaGada0..3` and unset
`WiezienEgzekucjaDialogOdpalony`. If the player is within 640 and the group has
not been electrocuted, corresponding TrojcaGadka0..3 dialogue plays. The
`Wiezienie2_0` marker dialogue sets `SmazyTrojce`; rule `smazenie` then sets
phase `umieraja` on the prisoner group and flag `UsmazylTrojce`. Its phase has
`setfaza0/1` directives affecting both guards. Preserve ordered numbered
commands; a key-value dictionary that collapses them loses the scene.

Marker `o_marker_trojca` is `(1684,-256,-1344)`, radius `(32,32,32)` and also
targets `o_promien_energii0` through `Nast_obiekt`. The energy beam is an
additional entity behavior, not an implicit kill-all trigger.

`rh2-wiezienie2` (`gameai` 1223–1284) remains an unconditional combat escape map,
with MasonStart10/11 markers. It can extend the playable prison sequence once
its authored geometry, NPCs and route have been exported and verified.

## Portable mission runtime API

`crates/mission-runtime` uses the complete decoded strings:

```rust,ignore
let mut mission = Mission::from_sources(world, &gameai, &dialogues, &text_keys)?;
let events = mission.tick(dt_seconds, &Context {
    player_position: native_player_position,
    weapon_drawn,
    action_target: just_interacted_semantic_npc_name,
    npcs: npc_snapshots, // name = definition_name; alive; player_seen; native position
});
```

- Evaluate authored marker bounds against the player hull in the renderer, then
  call `trigger_once(scene_object_name, dialogue_id)`. State keys include world.
- `trigger_dialog(id)` is also available for repeatable NPC interaction.
- `phase_enter(scene_instance_name, &ordered_command_pairs)` executes quest
  `set/unset/setfaza*`, stores callbacks, and returns all physical/presentation
  directives explicitly as `NpcCommand`. Apply it only on phase entry.
- `phase_event(scene_instance_name, "on_koniec_anim", branch)` returns authored
  phase-change events. It also accepts exact `on_death`, `on_kontakt`,
  `on_closer_widzi`, `on_further`, and other source callback prefixes. The caller
  selects a branch; repeated numbered entries preserve source weighting.
- `choose(1..=9)` observes the dialogue delay, plays the chosen localized answer
  via a Dialogue event for its original `answerNdelay`, then follows the target.
- `enter_world(world)` preserves flags/timers/one-time marker history and clears
  active dialogue/callbacks. The caller reloads world, NPCs and collision.
- `snapshot()` returns a serde-serializable `Snapshot`; `restore(snapshot)`
  restores quest state and display without reapplying grants. The caller saves
  player transform, health, inventory and scene entity state alongside it.
- Events: SetNpcPhase, HostileAttack, Dialogue, EndDialogue, TransitionWorld,
  GrantItem, Experience, ClearInventory, NpcCommand, Warning.
- Unsupported executable mission commands fail parsing with source line.
  Undefined original dialogue IDs yield Warning events rather than fabrication.
  Physical directives are passed explicitly to the integrating runtime; they
  are not claimed implemented by this crate.

The opening map's `cutscene intro` remains owned by the existing opening system.
Its unsupported mission command intentionally does not silently run through the
prison gameplay engine.

## Known uncertainties and original inconsistencies

- Full NPC pathfinding, cover selection, aim accuracy, hit regions, animation
  event timing and melee damage need original-runtime comparison. Parsed phase
  data is evidence of authored intent, not proof of engine semantics.
- `delay` is used as minimum dialogue/choice time and timeout is at least that
  delay. The exact retail interaction of `delay` and `ontimeexceeded` requires
  comparison. Audio files and their content are the originals.
- `CzasOdGadki` is treated as time since the latest spoken dialogue; explicit
  `unset` also resets it. The retail engine's implicit timer semantics need
  binary/runtime confirmation.
- Runtime preserves semantic names exactly: `gameai` straznik5 uses
  `straznik_przy_celi`, whereas the character is `straznik przy celi`. Missing
  `StraznikPrzyCeliMartwy`, `StraznikNaKlatceMartwy`, `StraznikNaKlatce75`
  dialogue definitions are source dangling references. Do not claim fixes to
  these original bugs as fidelity.
- Marker `Promien` is a vector, including very thin slabs (MasonStart2 Z=2).
  A point-only center test can miss them; use player-hull intersection and/or a
  swept test. Exact retail trigger shape rules remain to be confirmed.
- Dialogue choices exist elsewhere in the original graph, but the first two
  gameplay maps' recovered conversations are automatic or single-line nodes.
  Do not invent a choice tree for the kitchen prisoner.

## Verification evidence

The pure crate was built offline and all ten tests passed, including an opt-in
test reading the complete local `gameplay_scripts.json` for all four gameplay
prison worlds. Tests cover original riot radius/one-shot flags, drawn-weapon
hostility, voice-key resolution and guard timer, map rule handoff, zero-time
pause, undefined commands/dialogues, phase callbacks, checkpoint restoration,
and voiced answer timing/grants. No renderer or audio device is used.

```text
cargo test --offline --manifest-path crates/mission-runtime/Cargo.toml --tests -- --include-ignored
```

This verifies the interpreter and source compatibility. It does not certify a
completed prison playthrough or replace integrated collision/combat checks.

## Authored door and gate traversal

### Starting cell: authored open gap

`kratacelagora` is a compound row of six disconnected slabs at Z=734..735,
Y=-55..107. Its X intervals are [-876,-756], [-652,-532], [-428,-308],
[-204,-84], [-76,44], [244,364]. The player's upper panel is already shifted
left from [20,140] to [-76,44]; the initial gap is deliberately open. The
static doorway additionally constrains the walking route to approximately
X=64..112. A single enclosing collider for this whole row falsely sealed
the gap. There is no authored upper-grate activation or guard opening callback;
the guard warning follows the `MasonStart1` marker and its three-second timer.

The static exporter also used to emit visibility-only planes as collision.
It now retains surfaces carrying SOLID (bit0) or PHYSICSBLOCKER (bit17),
including invisible physical blockers. Visibility-only 0x200104 planes are
excluded. Regenerating 28 supported worlds removed 696 spurious triangles,
44 of them in the first playable prison world. Original source files are
unchanged; report: `output/collision-export-verification.json`.

The moving-brush adapter now reads the original two movement families:
`b_door` uses `Obrot`, `Predkosc_obrotu`, `Os_obrotu` and
`Przesuniecie_osi`; `b_szuflada*` uses `Przesuniecie` and `Predkosc`
in native units per second. `Gracz_otwiera=0` requires an authored activation.
The world2 gate controls are:

| Control | Position | Target | Original travel |
|---|---|---|---|
| b_szuflada2 | 1090.5, -206.5, 413 | krata1 | gate rises Y=160 at 16 units/s |
| b_szuflada0 | -1499, -208, 765.5 | krata2 | gate rises Y=160 at 16 units/s |
| b_door25 | -1014, -164.4588, 727 | krataceladol | gate slides X=-112 at 20 units/s |

`Nast_obiekt` is followed through every linked leaf with cycle protection.
World3's `akcja1 -> akcja2 -> wbieganie` must continue into the NPC emitter;
the door adapter returns non-door names in `DoorUse.linked_targets` for the
campaign host to dispatch. Interaction uses E, at most 190 native units, an
aim tolerance for small switches, and visibility to the actual brush surface.
`DoorUse.request_name` exercises that same path for silent integration checks.

Seven door tests passed without opening a renderer or audio device. They cover
linear speed, linked cycles, oriented collision, separation from contact,
thin brushes, rejection of interaction through a wall, and both original exit
targets using real scene, model and static-collision exports. The source poses
were world2 `(-1232,-190,190)` facing b_door21 and world3 `(2500,-48,-144)`
facing b_door0. The viewer compiled offline after these changes. An additional
controller test confirmed brush corrections retain sprint and aim state.

Brush collision is an oriented bounding box of each exported model. This
still approximates non-convex brush shapes and does not implement the retail
door push/crush behavior. The checks above establish the exit interactions,
not an automated walk through every corridor.
