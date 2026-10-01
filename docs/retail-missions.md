# Mission objects and character AI: what the retail binaries do

Addresses are `cshell.dll` unless marked `object.lto`. Everything below is implemented in
`crates/level-viewer/src/activation.rs`, `npcs.rs`, `campaign.rs` and `crates/mission-runtime`.

## Scene markers (client special effects)
The server objects (`object.lto`) only forward their properties to the client in an SFX message; the volume test
against the player (position +- hull) and all effects run in `cshell.dll` every frame.

| object | SFX id | client update | behaviour |
|---|---|---|---|
| `o_marker_zmienna` | 0x26 | 0x1002cf00 | inside the volume a timer counts up; at >= 0 it is set to -0.2, `Set` is set, `Unset` is unset (0x10015b30 / 0x10015b60) and the object is activated (message 0x29). So the variables refresh every 0.2 s while the player stands inside. |
| `o_marker_wykrywacz_postaci` | 0x2b | 0x1002ca40 | timer runs down always. Inside: `Jednokrotny` and already triggered -> nothing; `Czestosc_wykrycia` > 0.01 -> waits for the timer (then resets it to the period). Triggering plays `Dzwiek`, activates `Nast_obiekt`, and with `Special1` runs 0x1002c5d0. |
| `o_marker_dialog` | | 0x1002cc30 | once: StartDialog (0x100194d0) and activation. A chain reaching the marker (0x1002f5bd) starts the dialogue too, once. |
| `o_marker_death` | | 0x1002c930 | health change of `-Sila * dt` while inside (`Sila0`, `Sila1` after being toggled). |
| `o_marker_niechodzenia_postaci` / `_nierespawnu_postaci` | 0x2f / 0x30 | | zones for characters; the remake stops walkers entering the first and does not respawn emitters inside the second. |

`Special1` (chapel only): 0x1002c5d0 creates the character `laska czapel` 128 units behind the player and raises
`LaskaCzapelRespawnowana`. `LaskaChapelZagadana` (needed by the chapel exit) is set by the dialogue `Chapel10`.

## Object chains
`Nast_obiekt` is followed synchronously by the server registry (`object.lto` 0x1002cc08 / 0x10009350); a door still moving
ignores an activation and then ends the chain, `o_pauza` fires after `Pauza` seconds, beams switch off after
`Dlugosc_dzialania`. The dispatcher in `activation.rs` mirrors this. `Death_nast_obiekt` fires when an `o_obiekt` dies
(client message 0x27 -> registry 0x10009730). 38 of the 44 props that carry it are unbreakable bulbs (`HP 99999999`) naming lights that mostly do not exist; two pairs are live:
rh3-miasteczko1 `o_obiekt70` -> `o_obiektsam08` and rh3-miasteczko2 `o_obiekt20` -> `o_obiekt22huj` (cars, HP 200) - see docs/retail-props.md.

## Characters
* `setfaza` acts on the first placed instance of the definition (0x1001a5b7 loop), never on a corpse; `setallfaza` on every
  living instance (0x1001a6a2). `ifaction` (0x10019b80) tests whether the point 72 units ahead of the eye lies inside a
  living actor's box grown by 32; corpses are skipped, so `ifaction X` + `ifdead X` never fires for a corpse.
* `estimate_*` (parse 0x1004154e..): flags 0x49d do_gracza, 0x49e kryjowka, 0x49f od_gracza, 0x4a0 do_strzalu, 0x4a1 kluczy;
  dispatch at 0x10047ca0.
  * kryjowka (0x10047490): the nearest node (Manhattan distance from the actor's own node) that has a free link slot and
    whose segment to the player's eye (y+55) is blocked (0x10047070); else it falls back to do_gracza.
  * od_gracza (0x10047680): the point straight away from the player at 1024..2048 units.
  * do_strzalu (0x100477e0): a slot chain turning towards the player until a node sees him (see docs/retail-ai.md).
  * do_gracza (0x10046f30): a point 24 units short of the player, nothing when farther than 2048.
* `ucieka_jak_mniej_niz` (header, +0xfa8): below it the actor takes an `on_hurt` branch (0x10049bb8 -> 0x10045950 picks a
  numbered on_hurt slot) once its non-looping phase has ended; a bullet hit itself triggers nothing (docs/retail-ai.md).
* `on_reload`: taken when the clip (`max_ammo` of the weapon) is spent.
* `nie_respawnuj` (+0xfba): only read by the corpse clean-up at 0x10049d80 (30 s after death, farther than 320 units and out
  of sight the corpse is removed unless the flag is set). Emitters have no "already alive" check, only a 0.5 s throttle.
* `exp_gained` (+0xfb4) is added to the player's experience when the player kills the actor (0x10006b66); the dialogue
  `expgained` (+0xa9c) only prints "Sikeresen szereztel N tapasztalat pontot." (0x10019a8e).
* `mod_y` (+0x4ac) lifts a flyer above its path nodes (helicopter: 480). `mod_obrotu` (+0x4a8) is the turn-rate multiplier of the phase (pi * mod_obrotu rad/s, docs/retail-ai.md).

## Verified vs approximated (audit round: aware tag, speakers, door requests)
| Rule | Status |
|---|---|
| Per-tick clearing of the `aware` tags (0x1001a878), tags set by `ifaction`, `ifactionhostile`, `ifplayerseenby`, `ifhostileblizejniz`, kept through a dialogue, patrol phases refused while tagged (0x10041cbe) | ported (`Mission::aware`, `NpcRoster::set_aware`, `begin_phase`), unit tests in `npcs/tests.rs` and `mission-runtime` |
| StartDialog: speaker = first tagged actor, reset to `default_faza` with the tag cleared meanwhile, turns to the player (+0x158); end without a follow-up node releases every tagged actor and re-enters its phase (forced) | ported (`reset_speaker`, `Event::Released`, `release_aware`) |
| Dialogue node `hostileattack` provokes only the speaker (+0x127 = 1, +0x128 = 0, 0x10019811); the mission action of that name provokes every actor with contact (0x1001a52d) | ported (`provoke_speaker`, `provoke_noticing`); the contact test of the next actor pass still decides whether `on_kontakt` starts |
| `ifaction` probe: first living actor in list order with the point 72 ahead in its box grown by 32, no line-of-sight test, no `insignificant` filter; the key is level triggered | ported; the list order is unknown (see docs/retail-ai.md "Actor list order"), overlapping boxes are resolved by distance |
| `ifseenbyhostile` does not tag (0x10019f70) | already so |
| Registry order: doors are asked newest first, `Nast_obiekt` by name resolves the newest duplicate (0x10008b10 walks from the head) | doors ported; duplicate names: last wins (already) |
| "First instance" rules (`setfaza`, `ifalive`, `ifdead`, `ifgraczblizejniz`) walk the actor list from its head = the last created actor | **unknown** which placed instance that is (server update order not decoded); the remake keeps the first placed one |
| `hostile <name>` script command | used by no script |

| Actions run in REVERSE script order, effects of one action in the executor's fixed kind order (0x10015be0, 0x1001a898, 0x1001a140) | ported (`Mission::tick`) |

## Approximations that remain
* The character AI is a port of the retail actor update; what is still approximated is listed in docs/retail-ai.md (verified vs approximated).
* Head models of `laska czapel`, and the `o_obiekt` death animations (`death_podmien`) are not reproduced.
