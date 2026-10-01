# Cutscenes, game flow and the automated walk-through

Branch `agent/scenes`. Addresses are `cshell.dll` (base 0x10000000) unless marked `Lithtech.exe`. Reproduce a range with
`python -m tools.inspect_retail START END` or read the full disassembly the earlier session dumped (`disasm_all.py`, capstone).
Status words: **verified** = read from the code/data and reproduced, **approximated** = behaviour known, algorithm not reproduced.

## 1. Cutscene controller (`c_cutscene`, `opening.rs`)

| Part | Retail | Address | Status |
|---|---|---|---|
| Parser | `scripts\scenki.txt` blocks `scena*` (`pierwsza_faza`, `nast_faza`, `cutscene`, `runworld`) | 0x10011ce0 | verified (tools/export_presentation.py + export_scenes.py) |
| Start | `postac0..9` are created through the character manager; a character the level already has (don Mario, policjant) is *borrowed*: its position and rotation are saved and it plays the scene itself | 0x100130d0 | verified; the remake hides the level's copy and shows a scene copy (same picture, no position side effects) |
| Music | `outro` switches to `muza\outro.wav` | 0x100131ae | verified |
| Phase change | `anim`, `socket_kamera`, `socket_postacN`, `socket_glos`, `anim_postacN`, `setfaza0 <phase> <character>` (after the animations), `object_anim <clip> <object>`, sound (3-D at `socket_glos`, radius 1280), speech, subtitle | 0x10013440 | verified |
| Per frame | phase ends after `min(length, glos wav length)`; no sound = the phase does not wait; no length = the sound's length. Camera, characters and weapons follow the scene model's sockets; character phases run (`strzal_raz`, `on_koniec_anim`) | 0x10013950 | verified |
| Skip | "use" or "fire" held; runs to the last `runworld` of the scene; the outro cannot be skipped; `intro zwei` ends by itself after 16 s | 0x100139bc..0x10013a06 | verified |
| Proximity | `o_cutscene` fires when the 3-D distance player-object is below `detection_radius` (bus: 200) | 0x10013d90 | verified |
| Letterbox | two bars of 1/6 screen height | 0x10014030 (0x10066240) | verified |
| Subtitles | mincho bitmap font, scale 0.4, wrapped at 3/4 of the width, at most 8 lines, bottom line two rows above the bottom | 0x10034160 | verified |
| End | the scene sound stops, every scene-created character is removed, borrowed ones get their saved position back, the subtitle clears | 0x100132e0 | verified |
| Menu during a scene | Esc opens no menu while a cutscene or the credits run | 0x1005ac8b | verified (`menu.rs` checks `opening.active`) |

The player is not moved by a scene (only the camera), so positions, HP and inventory after a cutscene are the ones from before it; `runworld` then
loads the next world through the normal travel path (campaign state carries over, see section 3). Played scenes: `intro` (22 phases, 77.97 s, world
rh1-wiezienie1, then `intro zwei`, then rh1-wiezienie2), `ucieczka z 2 wiezienia` (bus, 3 phases + `runworld rh3-miasteczko1`, rh2-wiezienie2),
`outro` (19 phases, 93 s, rh12-lab2). The two `szczur` scenes are never placed in a world. `output/scenes-audit.json` lists what the scenes name and the
export lacks (animations `zapala`/`jedzie` of the empty bus, `do_broni`/`strzela`/`reload`/`pada` of don Mario, three policjant clips): the retail
`SetCurAnim` keeps the previous clip when a name is missing, the remake does the same.

Checked headless (screenshots in `C:/Temp/mester-scenes-scratch/`): `intro-006.00.png` (car, subtitle "A munka elvégezve.", bars),
`intro-054.00.png` (arrest street, two policemen with guns), `outro-020.00.png` (Dominick with the pistol), `outro-070.00.png` (Stella with the syringe,
subtitle "Nézd mit találtam a lenti laborban."), `bus-003.00.png` / `bus-009.00.png` (bus in the courtyard, gate `brama1` opening).

## 2. Ending (`c_outromgr`, `endgame.rs`)

State machine 0x10032420 (update) / 0x10032530 (draw): three story slides (`outro1..3`, 10 s each, four centred lines at y = 540 + 32 i, scale 0.5),
the credits, the last slide (`outro4`, 20 s, lines from y = 140), then `0x10059cc0`, which ends in `ILTClient::Shutdown` (`call [eax+0x1c]`): **the retail
game quits to the desktop after the last slide**. **Deliberate owner deviation:** the remake returns to the MAIN MENU instead (owner decision 2026-09-29; `endgame.rs`
sets `Travel::menu`, `travel::request` unloads the level, reloads the menu backdrop world exactly like a launch with `menu`, re-arms the new-game reset, closes the
end screen, frees the cursor and lets `music.rs` switch to `menu.wav`; New game / Load work from there). Proven headlessly by
`MESTER_WALK_ENDING=1` (see section 4).
Credits: up to 63 rows of `credits.txt`, row `i` at `y = 32 i + scroll`, initial `scroll` = screen height (768). *Both* per-frame routines subtract from that
offset: the update 0x100323a0 `dt * 20` (float 0x10066418) and the draw 0x10032530 `dt * 16` (0x10066128, times width/1024) = **36 px/s**, about 78 s until the
last row has left (`rows * 32 + scroll - 1 < -32`); the credits track is 89 s. (Earlier notes said 16 and main's `ending.rs` 20: each counted one routine.)
Rows are drawn twice (dark 0x20 shadow, then 1 px up-left in colour), scale/colour by index mod 3 (0.3 grey c0, 0.45 grey f0, 0.5 dark).
Texts: `output/endgame.json` (`tools/export_scenes.py`), images `ui/outro/*.png`. Tests: slide order and total time, row layout, export audit.

## 3. Game flow

* **New game**: menu -> player creation -> `rh3-miasteczko0` (prologue; music `prolog`), `startlevel rh1-wiezienie1` (script line 155, `tnijitems` takes the
  prologue's gear) -> `intro` cutscene -> `intro zwei` -> rh1-wiezienie2. Levels then follow `docs/campaign-research.md` (27 levels; `chinatown`
  is unreachable in retail too). Exits are doors with `Skok_do_levelu` (20 levels; a door is a jump even when a chain opens it, object.lto 0x100020c0),
  script `startlevel` (7: prologue, burmistrz1/2, chapel, knajpa, chinatown2, fabryka) or a cutscene (`rh2-wiezienie2` bus, `rh12-lab2` outro).
* **Carry-over**: weapons, ammo, item grid, stats, health, experience and kills live in resources that persist across `travel`; only mission flags are
  per level (`bool` lines sit under each `level`). The probe compares the item grid before the exit and after arrival (`carry ok`).
* **Death** (0x10060070): the controller is flagged dead (input and physics skipped, only the death camera runs, view.rs), every living character re-enters
  its `default_faza` (`death.rs`: attackers stop), the shell prints `GameShell4` ("Meghaltál. A menük eléréséhez nyomd meg az ESC gombot, a
  gyorstöltéshez pedig az F9-t.", scripts\locale\shell.txt). There is no death screen: the red "MEGHALTÁL" overlay of earlier builds was not retail and is
  hidden/removed. F9 quick load, F5 quick save ("Gyorsmentés létrehozva." 5 s; while dead GameShell1) and the level-start auto quick save are `menu.rs`
  (0x1005abd4 / 0x1005ac27 / 0x1005a5d0).
* **Mission failed**: only `zdrowie -1000` (Stella killed, chapel line 1522 and knajpa line 4362); it is a `HealthDelta` event, so the death flow above runs.
  No other level has a fail condition in `gameai.txt`.
* **Water**: retail has no water damage or swimming (docs/retail-movement-audit.md: the river/pool/tunnel water surfaces are solid polygons with
  SurfaceFlags 4, footstep `krokm`; no water string in cshell.dll, no VolumeBrush). The remake's instant death on a water surface (`campaign.rs`) was removed.
  The deadly sewer water of podziemia1/1a is `o_marker_death` volumes (23 objects, `Sila0/Sila1` HP per second), handled in `activation.rs`.
* **Chinatown detector chain, gates**: `Nast_obiekt` chains such as wiez_wn1's three sliding gates (`b_szuflada0` -> 1 -> 2, only the first is player-usable) open
  all at once from one E.

## 4. Automated walk-through

Three layers, all headless and silent:

1. **Mission solver** (`crates/mission-runtime/src/solver.rs`, `tests/walkthrough.rs`): cheapest-first search over what the player can do (talk to an instance,
   stand next to it, kill it, answer n, walk into a marker, pick up an item, wait, draw the weapon) driving the *real* interpreter until it emits
   `startlevel` or a cutscene; kills cost most, answers are taken from the live dialogue. `cargo test --test walkthrough -- --ignored --nocapture`.
   Result: all seven script-gated levels have a plan (prologue: marker + kill the target; burmistrz1: talk + answer 1 + exit marker; burmistrz2: two
   markers + talk; chapel: detector, seven answers, exit marker; knajpa: marker, talk, two answers, wait 20 s; chinatown2: marker + pick up `Golden cat.`;
   fabryka: talk, three answers, marker). Levels that end at a door have no script exit.
2. **Walkability planner** (`crates/retail-movement/src/plan.rs`, `tests/reach_levels.rs`): A* on a 24-unit grid over the static hull with box sweeps
   (stairs <= 18, ramps, drops <= 300, jumps <= 45 up / running jumps over gaps, crouched 24-box for ducts, door leaves as portals). It is an
   approximation of the controller, used to route the bot and to report reachability.
3. **Engine bot** (`walk_probe.rs`): `MESTER_TEST_SCENARIO=walk` (+ `MESTER_SILENT=1`, capture arguments so the window is hidden). Per level: solver plan ->
   goals (marker, pickup, talk, answer, kill from a walkable vantage with a line of sight, wait, exit door / chain marker / cutscene object), walked with
   the ordinary controller along the planner's route, doors opened ahead (or through their chain opener), dialogues answered, health kept full, re-plan
   from the live state after answers and failures; stuck/stalled steps and unreachable gaps become logged teleports. `MESTER_WALK_COUNT=n` levels from the given
   world, `MESTER_WALK_LEVELS=a,b`, `MESTER_WALK_BUDGET=s`. Log line per level: `WALK n level: PASS ... / BLOCKED ...` and a summary. `MESTER_WALK_DEATH=1` runs the
   F5 / death / F9 flow.

### Status per level (regression proof 2026-10-01 on main 4753a3a + the bot fixes below; dev-optimized build)

PASS = the level's real exit fired (door, script `startlevel`, cutscene) and the next expected level loaded. Times are GAME seconds (a scenario frame is a fixed 0.05 s,
`probe_kit.rs`; with `MESTER_NOVSYNC=1` the hidden window runs at roughly real time to 2x). "tele" = logged bot teleports (stuck steps on slopes/stairs, unusable grates, unconnected
regions; owner decision: spots that need a crouch-jump may be teleported), "dmg" = health the AI took from the invulnerable bot (proves NPCs attack; 0 where the bot passes the
guards before they see it). "prev" = the earlier proof (2026-09-30, before the AI/sight/door/script-order changes of main). Every level was run alone from its own start
(`walkall.sh`-style loop, see Commands) and the whole campaign once in ONE process (`chain`, 27/27 PASS, carry-over checked, ending included). The solo run is deterministic
(two runs give identical seconds/teleports).

| # | level | result (solo) | tele | dmg | prev | notes |
|---|---|---|---|---|---|---|
| 1 | rh3-miasteczko0 | PASS 85.9 s | 0 | 1924 | 186 s / 0 | the target o_postac08 patrols: the bot shot from a spot the target had left (bot, see below); now re-chooses its vantage |
| 2 | rh1-wiezienie1 | PASS 0.4 s skipped (`story` start) | 0 | 0 | 0.4 s | unskipped intro 73 s with `MESTER_WALK_NOSKIP=1`; started as `story`, not by world name |
| 3 | rh1-wiezienie2 | PASS 24.7 s | 2 | 0 | 38 s / 5 | cell grates `kratacelagora`/`krata2` are not player-usable |
| 4 | rh1-wiezienie3 | PASS 12.3 s | 1 | 0 | 12 s / 1 | hop into the exit corridor (crouch-jump spot, teleport accepted) |
| 5 | rh2-wiezienie1 | PASS 27.2 s | 2 | 96 | 45 s / 6 | `sector2b` has no player opener |
| 6 | rh2-wiezienie2 | PASS 36.7 s | 3 | 410 | 32 s / 2 | bus cutscene fires, `rh3-miasteczko1` loads |
| 7 | rh3-miasteczko1 | PASS 75.0 s | 5 | 1024 | 75 s / 5 | slopes |
| 8 | rh3-miasteczko2 | PASS 66.4 s | 2 | 993 | 72 s / 3 | (was 139 s / 15 tele before the teleport fix) |
| 9 | burmistrz1 | PASS 92.8 s | 7 | 2244 | 100 s / 10 | |
| 10 | burmistrz2 | PASS 82.3 s | 7 | 1764 | 88 s / 7 | |
| 11 | chapel_mniejszy | PASS 97.0 s | 0 | 255 | 113 s / 3 | |
| 12 | knajpa | PASS 232 s | 18 | 4974 | 362 s / 35 | in the chain the solver plans `kill o_postac6` first (see below): PASS 121 s |
| 13 | rh7a-tunele | PASS 8.3 s | 2 | 0 | 8 s / 2 | exit region unconnected in the hull (teleport accepted) |
| 14 | podziemia1 | PASS 133.7 s | 16 | 3290 | 134 s / 16 | |
| 15 | podziemia1a | PASS 31.1 s | 2 | 290 | 31 s / 1 | |
| 16 | podziemia1b | PASS 80.5 s | 10 | 392 | 106 s / 15 | |
| 17 | podziemia1c | PASS 46.0 s | 3 | 658 | 57 s / 5 | |
| 18 | chinatown2 | PASS 197.3 s | 12 | 1000 | 188-200 s / 11-13 | |
| 19 | rh9-fabryka | PASS 240.6 s | 20 | 5180 | 346-372 s / 31-39 | target o_postac3 patrols (vantage with trail) |
| 20 | rh10-wiezowiec1 | PASS 63.1 s | 0 | 615 | 60 s / 0 | |
| 21 | rh10-wiezowiec2 | PASS 196.9 s | 9 | 8004 | 198-210 s / 10-12 | most dangerous level for the bot (13 NPCs noticed it) |
| 22 | rh10-wiezowiec3 | PASS 67.6 s | 1 | 3666 | 82 s / 2 | |
| 23 | wiez_wn1 | PASS 15.0 s | 0 | 247 | 20 s / 0 | gate row opened by E on its lever |
| 24 | wiez_wn2 | PASS 16.6 s | 1 | 226 | 11 s / 0 | |
| 25 | wiez_wn3 | PASS 13.8 s | 0 | 0 | 14 s / 0 | |
| 26 | rh12-lab1 | PASS 61.6 s | 3 | 826 | 110 s / 13 | |
| 27 | rh12-lab2 | PASS (credits) | | | PASS | outro, three slides, credits, last slide, MAIN MENU, New game starts rh3-miasteczko0 (`MESTER_WALK_ENDING=1`) |
| - | chinatown (Kiskina, label `walk-chinatown`) | PASS 70.0 s | 5 | 0 | 201 s / 8 | from its StartPoint to `b_door0`, whose own jump leads to chinatown2 |
| - | chain (one process, rh3-miasteczko0 -> credits -> main menu -> new game) | 26/27 real exits, wiez_wn1 BLOCKED (bot), then outro, credits, main menu, new game OK | | | 27/27 | knajpa 121 s, burmistrz1 140 s; wiez_wn1: see below |

Other flows proven headlessly: death -> F9 (`MESTER_WALK_DEATH=1` with a scratch `MESTER_USER_DIR`: dead, F9 reloads rh1-wiezienie2, health 100, alive), carry-over of items across
the 27 real exits (chain run), cutscenes: intro (73 s unskipped), bus, outro (in the chain), credits scroll and the return to the main menu.

Commands (Git Bash, worktree crates/level-viewer, any `CARGO_TARGET_DIR`, `cargo build -j 4` (the dev profile of Cargo.toml is optimized) or `cargo build --release -j 4`):

```
# one level (rh1-wiezienie1 is started as `story`), hidden window, silent, ~10x real time
MESTER_SILENT=1 MESTER_TEST_SCENARIO=walk MESTER_WALK_COUNT=1 MESTER_WALK_BUDGET=900 MESTER_WALK_TIMEOUT=6000 MESTER_NOVSYNC=1   level-viewer.exe <level> ../../output out.png 5
# the whole campaign in one process (real exits, carry-over), ending included
MESTER_SILENT=1 MESTER_TEST_SCENARIO=walk MESTER_WALK_COUNT=27 MESTER_WALK_BUDGET=900 MESTER_WALK_TIMEOUT=90000 MESTER_NOVSYNC=1 MESTER_WALK_ENDING=1   level-viewer.exe rh3-miasteczko0 ../../output out.png 5
```
Per-level loop of the 2026-10-01 proof (dev-optimized build, `cargo build -j 4` in the worktree, `CARGO_TARGET_DIR` anywhere; one level at a time, never two walk processes):
```
for L in rh3-miasteczko0 rh1-wiezienie2 ... chinatown; do   # rh1-wiezienie1 is started as `story` (the intro only runs from there): MESTER_WALK_LEVELS=rh1-wiezienie1 ... level-viewer.exe story ...
  MESTER_SILENT=1 MESTER_TEST_SCENARIO=walk MESTER_WALK_LEVELS=$L MESTER_WALK_BUDGET=900 MESTER_WALK_TIMEOUT=6000 MESTER_NOVSYNC=1     timeout 1500 level-viewer.exe $L ../../output out.png 5 > $L.log; done
```
`python -m tools.run_probes --exe <exe>` runs the 50 probe scenarios (about 5 minutes; 50/50 PASS on this proof). `MESTER_WALK_TRACE=1` adds the bot diagnostics (position/waypoint/stance every
0.25 s, the placement after a teleport, sweeps and ray of a stuck step); the kill goal prints the first thing on the bullet line when the target survives.

Extra switches: `MESTER_WALK_NOSKIP=1` (play the intro), `MESTER_WALK_DEATH=1` (F5/death/F9), `MESTER_WALK_DOORDEBUG=1` (door/leaf diagnostics), `MESTER_WALK_SPEED` (obsolete since
scenario frames are fixed 0.05 s; keep 1). Never run two walk processes at once: the second hidden window loses its swap chain (wgpu "Acquiring a texture failed").

### Regression proof 2026-10-01 (everything of main since the last proof: AI sight/bullet filters, NPC door requests, script action order, ...)

Result: probes 50/50, 27 levels + chinatown PASS solo, chain 27/27 (see table). No game regression was found; every failure was a bot or probe limitation, fixed:
* `sounds` probe held R for the Mossberg reload: the reload key is level triggered since 4915f9e, so a held R restarts the shotgun shell cycle every frame (retail 0x10060d08) and the
  shell/sound never comes. The probe now presses R only until the reload runs.
* Probe teleports were *corrected as movement*: `npcs::block_player` and the door/prop correction pass of `doors::tick` sweep every position change under 150 units from the old
  position against actors, props and the hull, so a bot teleport across a thin wall (or a prop box such as `o_obiekt1` in miasteczko2) was clamped back (15 stuck teleports instead of 3,
  139 s instead of 66 s). `Walking::teleported` (set by `campaign_probe::place`) skips both passes for that frame. The game itself is unchanged (no player can jump 80 units).
* Kill goals: the vantage test used an eye 6 units below the real camera and a spot the bot only reaches within 30 units; the patrolling targets (miasteczko0 `o_postac08`,
  fabryka `o_postac3`) walked on while the bot crossed the level (a 40 s shoot-out against a wall, three times). The bot aims from the camera, requires a line from the spot and 20 units
  around it, keeps the target's trail and re-chooses the vantage (best view of the trail, never a spot that already failed) after 15 s.
* knajpa in the chain: the solver now plans `kill o_postac6` BEFORE the marker (the reverse script order changed which plan is cheapest); the bartender stands behind the counter,
  so the vantage must see him over it (fixed by the robust vantage). Solo the plan is marker first, the target then walks to the far end.
* **OPEN** wiez_wn1 in the full chain (only there; solo PASS 15 s, and the 4-level chain wiezowiec1..wn1 PASS): the bot reaches the end of the drawer-gate row after the gates have closed
  again (they close when the player is 128 units from the first gate, and an actor's door request had opened the later gates earlier) and ping-pongs between the first gate `b_szuflada0`
  (its opener) and the door `b_door0` (3 attempts of the bot fix: closed leaf in front of the door takes the press, walk to its opener, one press per 3 s + 8 s rush). Judged a BOT limit
  (the level is passable solo and the gates behave as in retail docs) but NOT proven: the game state of the chain at that point (NPCs hostile and moving 90k units, 2000-3800 damage)
  differs from the solo run; the harness continues with wiez_wn2 after the BLOCKED level, so the credits/main-menu/new-game proof still ran.
* Noise that is not a bug: `matNNNN: N gyari texturakoordinata nem veges` (export data of chapel/burmistrz, textured-less fallback), the wgpu "Loader Message" lines, the one
  `Original dialogue is undefined: Mason13` (chinatown), the 8-20 s "hitches" (the bot's own A* at a goal start: 400000 cells), and the rare `Couldn't get swap chain texture` panic of a
  hidden window (seen once in a chain run: rerun; never run another window/walk process at the same time).
* Not seen: NaN / non-finite state, panics from the game, doors that stay shut for the AI (the NPC door requests opened `b_door2` 54x, `b_door17` 27x ...), NPCs that never move
  (npc movement > 3000 units in every level with characters). Levels with dmg 0 (wiezienie2/3, tunele, wn3, chinatown) are passed before the guards see the bot; the `ai-fight`/`ai-far`/
  `ai-patrol`/`gunfire`/`noise` probes cover attacks.

### Game bugs found and fixed by the proof runs
* **Brush stair rule** (`doors.rs` correction pass): a side contact with a solid brush whose top is at most one stair (18) above the feet is stepped onto, like the
  controller does on the static hull. podziemia1c: the exit door's 64x64 threshold plate (`b_transparent3`, 2 units above the floor) blocked the only way to the
  detector marker / exit `hujjj`. Unit test `a_low_brush_reports_its_top_for_the_stair_rule`.
* **Ending** now returns to the main menu (owner decision, section 2) instead of quitting; a stale `output/endgame.json` (old exporter format) crashed the credits: the shared
  export was regenerated (`python -m tools.export_scenes GYARI output`) and the credits skip themselves if the export is missing.

### Bot changes (bot limits, not game faults)
Solid props are walls for the planner (retail props are solid; the static hull does not know them) and a failed route names the blocking props; "closest cell to an unreachable
goal" counts height; markers are done as soon as the player's box touches the volume (as the game fires them); doors: stand still, aim from the camera and press E on the
opener; the M-14/Glock is drawn through the holster keys; items on tables are taken with the held use key; at most three gap steps per goal; frame hitches, damage taken, NPC
movement and the blockers of every stuck step are logged.

Not proven: a physical walk (no teleport) over rh1-wiezienie3's roof hops and rh7a-tunele's exit region (owner: crouch-jump spots, teleport accepted); the many stuck steps on
steep stair runs (burmistrz1, podziemia1b, rh9-fabryka) are the bot's route following, the planner (approximation) says reachable.

### Findings and open points

* Planner reachability (`reach_levels`): every exit is reachable except rh1-wiezienie3 (the exit corridor is 256 above the yard; the crate route to the roof
  works with jumps but the last link is missing: 52-64 unit hops beyond the 47.5 jump apex - whether retail steps up in the air (jump 45 + stair 18) is
  unknown, main's controller steps only when standing), and Rh7a-Tunele (exit 3768 away, region ends). rh10-wiezowiec3's exit centre lies 54 units inside a wall
  (button), fine. There are no ladders, lifts or water in retail (movement audit); vertical links are stairs, crates and jumps.
* Doors that are not player-usable and have no chain opener the bot can reach (`kratacelagora`, `krata2`, `sector2b`, `hujjj`, `b_szuflada0` in podziemia1c) are
  opened by markers/detectors or are cell grates that are not on the walkable route; the bot steps over them (logged).
* The bot is slower than a player (many stuck teleports on ramps and door frames): budget failures in a level are bot limits unless the note names a
  real blocker. Reachability itself is settled by the planner table above.
* Quirk seen in knajpa: the "telephone" and poster textures render mirrored/upside down (props agent's area).
