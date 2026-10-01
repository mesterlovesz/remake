# Headless probe scenarios (the end-to-end regression net)

Every probe is one hidden, silent run of the real game: `level-viewer <world|menu> ../../output <png> <times>` with `MESTER_SILENT=1`
and `MESTER_TEST_SCENARIO=<name>` (capture arguments = hidden, unfocused window, no sound). A scenario drives the game through its own
input resources / teleports and asserts on game state; the process exit code is the verdict (0 = pass, 1 = a check failed or the probe never
finished). `tools/run_probes.py` runs all of them and prints PASS/FAIL per scenario.

```
export CARGO_HOME="F:/_VIBECODING/RUST/_toolchain/cargo" RUSTUP_HOME="F:/_VIBECODING/RUST/_toolchain/rustup"; export PATH="/f/_VIBECODING/RUST/_toolchain/cargo/bin:$PATH"
export CARGO_TARGET_DIR=C:/Temp/mester-probes-fast      # any dir; the optimized build lives there
python -m tools.run_probes --build          # first build of a fresh target dir ~25 min (Bevy at opt-level 2), later ~2 min
python -m tools.run_probes                  # all 36 scenarios, ~5 min in total
python -m tools.run_probes retail bus       # some (labels below)   |   --list prints the exact command lines   |   --logs DIR
```
The runner exits 1 if any scenario fails, writes `<label>.log` / `<label>.png` (+ `<label>-end.png`, the picture when the probe finished) to
`<tmp>/mester-probe-logs`, and prints `N frames / T s game time` per run: N and T are identical from run to run (checked over three full runs).

## Why the plain debug build is not used
The dev profile (opt-level 0) renders this game at ~4-5 frames per second, so a probe took 1-2 minutes and a slow run went past `timeout 120`.
`--build` uses cargo's `--config 'profile.dev.package."*".opt-level=2' --config 'profile.dev.opt-level=1'` (no Cargo.toml edit): ~7x faster
(retail probe 80 s -> 11 s). The same profile in `Cargo.toml` would also make the owner's `target\debug` launchers playable; that is the coordinator's call.

## What made the probes flaky, and what is deterministic now
* The game clock of a scenario run advances exactly `probe_kit::STEP` = 0.05 s per frame (`TimeUpdateStrategy::ManualDuration`, only when capture
  arguments and a scenario are given; `MESTER_TEST_REALTIME=1` restores the wall clock for visual captures). Before, the virtual clock advanced by the
  real frame time clamped to 0.25 s while the game systems clamp their own step to 0.05 s: at 4-5 fps the scripts ran up to 5x faster than the game
  they tested (a "wait 1.5 s" was 0.3 s of game). That is why the Glock reload, the thrown apple, the second alt press and the cell guard's shot failed
  depending on machine speed. The hidden window also uses `WinitSettings::continuous()` in those runs.
* Scripts (`probe_kit::Cursor` + `Step`) act once and then wait for the state they expect (item gone, shot counted, reload finished, level arrived,
  dialogue flag set) with a generous per-stage timeout, instead of fixed times. A failed stage records `stage N: ...` and stops the probe at once.
* The capture code (`main.rs::capture` + `probe_kit::ProbeStatus`) ends the run when the probe finishes (or fails at once); capture times are only the
  earliest exit. A probe that never finishes is stopped after 600 game seconds.

## Scenarios (labels of `tools/run_probes.py`; result = last full run on agent/probes after merging main 760bc58)
| Label | Scenario | World | Result | What it checks |
|---|---|---|---|---|
| campaign | campaign | rh1-wiezienie2 | PASS | cell -> guard -> Glock kill -> three level exits by real door names |
| cell | cell | rh1-wiezienie2 | PASS | walk out of the cell from the real StartPoint, no cheats |
| bus | bus | rh2-wiezienie2 | PASS | bus marker cutscene -> miasteczko1 -> miasteczko2 -> burmistrz1 (must start on rh2-wiezienie2) |
| catalog-a | catalog | rh3-miasteczko0 | PASS | levels 0..9 of the list: mission script binds, actors load |
| catalog-b | catalog | rh3-miasteczko0 | PASS | levels 10..19 of the list |
| catalog-c | catalog | rh3-miasteczko0 | PASS | levels 20..27 of the list |
| uv | uv | rh3-miasteczko0 | PASS | two fixed vantage points (texture coordinate check for screenshots) |
| npc | npc | rh1-wiezienie2 | PASS | places the player in front of a named NPC and reports its phase |
| impact | impact | rh1-wiezienie3 | PASS | Glock shot at the nearest wall next to o_postac18 must fire and hit |
| gunfire | gunfire | rh1-wiezienie2 | PASS | Glock against o_postac22 for 9 s (shots and hits must register) |
| zmienna | zmienna | rh3-miasteczko0 | PASS | stands in every o_marker_zmienna and checks the mission variables |
| lever | lever | wiez_wn1 | PASS | lever pulled with E, something must open or toggle |
| stella | stella | chapel_mniejszy | PASS | chapel dialogue with Stella until LaskaChapelZagadana |
| retail | retail | rh1-wiezienie2 | PASS | pickups by touch, NPC drop, Glock/FN/SIG ammo and reloads, pause, level changes keep weapons, grenade |
| inventory | inventory | rh1-wiezienie3 | PASS | C / X / Z screens, weight, level-up steps, thrown item picked up again |
| menu | menu | menu | PASS | main menu -> new player -> Előszó, Kiskína from the level list, walk from its start |
| input | input | rh1-wiezienie2 | PASS | input_probe: Space jumps, right mouse scopes, X frees the cursor, inventory refuses the jump, Escape order |
| hud | hud | rh1-wiezienie2 | PASS | hud fade script (logs the alpha of every HUD group; no assertion) |
| menu_pages | menu_pages | menu | PASS | scripted pointer walks every options page, files written (scratch MESTER_USER_DIR) |
| menu_hover | menu_hover | menu | PASS | 1920x1080, pointer through the window's own cursor position (`MESTER_PROBE_REAL_POINTER=1`) sweeps the main list one row per frame; the frame right after every move must still show every label with its glyphs (was: one blank frame per row change) |
| bonus | bonus | menu | PASS | owner-requested Bónusz menu: main list row 9 -> bonus page (pointer, Esc, back row) -> Kiskína (world chinatown, 17 NPCs, at the StartPoint, bonus run) -> F5 writes only `save/bonus.sav`, the Save row is refused, death + F9 reload the bonus slot -> b_door0 returns to the main menu (not to A Templom), campaign `quick.sav` never created (scratch MESTER_USER_DIR; PNGs at 3.3 / 7.8 / 9.3 / 15 s: main menu with the row, bonus page, loading screen, level) |
| menu_saves | menu_saves | rh1-wiezienie2 | PASS | F5 / F9 quick save, menu save and load (scratch MESTER_USER_DIR) |
| menu_death | menu_death | rh1-wiezienie2 | PASS | death message and F9 reload (scratch MESTER_USER_DIR) |
| sounds | sounds | rh1-wiezienie2 | PASS | sound parity net (docs/retail-audio-parity.md): music start, jump, 650-unit fall, heartbeat, footsteps, the three wounds, the Music switch quirk, every gun's shot and reload, baton, grenade blast; asserts on the `CUE` log lines |
| props | props | rh1-wiezienie2 | PASS | shoots a prop and logs its hit points (capture aid) |
| door | door | rh1-wiezienie2 | PASS | walks up to a door and presses E (capture aid) |
| pickup | pickup | rh1-wiezienie2 | PASS | stands on an item, it is taken by touch (capture aid) |
| arsenal | arsenal | rh1-wiezienie2 | PASS | every weapon acquired, lowered/raised and fired (capture aid) |
| noise | noise | rh1-wiezienie2 | PASS | a shot that wounds a guard from behind wakes him (two-frame wound stimulus); `MESTER_TEST_NOISE=away`: a lone shot into the distance changes nothing (retail: one-frame stimulus cannot provoke); waits up to 8 s |
| revolver | revolver | rh1-wiezienie2 | PASS | S&W fired once and reloaded, six casings |
| move | (MESTER_MOVE_PROBE) | rh1-wiezienie2 | PASS | scripted walk, run, jump, crouch, stand (logs the controller state) |
| dialogue | dialogue | rh1-wiezienie3 | PASS | starts a dialogue node in front of a character and logs the panel state (capture aid) |
| ai-fight | ai | rh1-wiezienie3 | PASS | an armed character in contact range: contact, reaction phase, shot rate 1/strzal (docs/retail-ai.md) |
| ai-far | ai | rh3-miasteczko2 | PASS | the same beyond 1.3 * contact distance: no contact may happen |
| ai-patrol | ai | rh1-wiezienie3 | PASS | a patrolling character walks its path graph at the phase speed |
| walk | walk | rh1-wiezienie2 | PASS | walk-through planner plays one campaign level with the real controller (MESTER_WALK_COUNT levels from the given world; see docs/retail-scenes.md) |
| walk-chinatown | walk | chinatown | PASS (201 s, 8 assisted hops) | the cut level Kiskína (`chinatown.dat`, not on the campaign route): the planner walks from its StartPoint to `b_door0`, whose own jump leads to chinatown2 (`MESTER_WALK_LEVELS=chinatown`; docs/cut-content.md) |
| altfire | altfire | rh1-wiezienie2 | PASS | M-14 scope, laser, reload quirk (its script runs to 11 s: the probe ends at capture time + 2 s) |
| melee | melee | rh1-wiezienie2 | PASS | nightstick against an actor and a wall |
| flash | flash | rh1-wiezienie2 | PASS | flashlight toggled with the middle button |
| start-<world> (10) | startpose | podziemia1b, rh1-wiezienie1/2, rh10-wiezowiec1, rh2-wiezienie2, rh3-miasteczko0/1, wiez_wn1/2/3 | PASS | 0.5 s each: the real level load starts on the StartPoint facing its `Kierunek` (docs/retail-start-view.md); `cell` with `MESTER_TEST_TRACE=<npc>` logs that actor's position twice a second |

## Exact command lines (run from `crates/level-viewer`; `<exe>` = `$CARGO_TARGET_DIR/debug/level-viewer.exe`)
```
MESTER_SILENT=1 MESTER_TEST_SCENARIO=campaign timeout 120 <exe> rh1-wiezienie2 ../../output out.png 3   # campaign
MESTER_SILENT=1 MESTER_TEST_SCENARIO=cell timeout 120 <exe> rh1-wiezienie2 ../../output out.png 3   # cell
MESTER_SILENT=1 MESTER_TEST_SCENARIO=bus timeout 120 <exe> rh2-wiezienie2 ../../output out.png 3   # bus
MESTER_SILENT=1 MESTER_TEST_SCENARIO=catalog MESTER_TEST_FROM=0 MESTER_TEST_TO=10 timeout 120 <exe> rh3-miasteczko0 ../../output out.png 3   # catalog-a
MESTER_SILENT=1 MESTER_TEST_SCENARIO=catalog MESTER_TEST_FROM=10 MESTER_TEST_TO=20 timeout 120 <exe> rh3-miasteczko0 ../../output out.png 3   # catalog-b
MESTER_SILENT=1 MESTER_TEST_SCENARIO=catalog MESTER_TEST_FROM=20 MESTER_TEST_TO=28 timeout 120 <exe> rh3-miasteczko0 ../../output out.png 3   # catalog-c
MESTER_SILENT=1 MESTER_TEST_SCENARIO=uv timeout 120 <exe> rh3-miasteczko0 ../../output out.png 3   # uv
MESTER_SILENT=1 MESTER_TEST_SCENARIO=npc MESTER_TEST_NPC=o_postac22 timeout 120 <exe> rh1-wiezienie2 ../../output out.png 3   # npc
MESTER_SILENT=1 MESTER_TEST_SCENARIO=impact timeout 120 <exe> rh1-wiezienie3 ../../output out.png 3   # impact
MESTER_SILENT=1 MESTER_TEST_SCENARIO=gunfire timeout 120 <exe> rh1-wiezienie2 ../../output out.png 3   # gunfire
MESTER_SILENT=1 MESTER_TEST_SCENARIO=zmienna timeout 120 <exe> rh3-miasteczko0 ../../output out.png 3   # zmienna
MESTER_SILENT=1 MESTER_TEST_SCENARIO=lever MESTER_TEST_LEVER=o_obiekt1 timeout 120 <exe> wiez_wn1 ../../output out.png 3   # lever
MESTER_SILENT=1 MESTER_TEST_SCENARIO=stella timeout 120 <exe> chapel_mniejszy ../../output out.png 3   # stella
MESTER_SILENT=1 MESTER_TEST_SCENARIO=retail timeout 120 <exe> rh1-wiezienie2 ../../output out.png 3   # retail
MESTER_SILENT=1 MESTER_TEST_SCENARIO=inventory timeout 120 <exe> rh1-wiezienie3 ../../output out.png 3   # inventory
MESTER_SILENT=1 MESTER_TEST_SCENARIO=menu timeout 120 <exe> menu ../../output out.png 3   # menu
MESTER_SILENT=1 MESTER_TEST_SCENARIO=input timeout 120 <exe> rh1-wiezienie2 ../../output out.png 12   # input
MESTER_SILENT=1 MESTER_TEST_SCENARIO=hud timeout 120 <exe> rh1-wiezienie2 ../../output out.png 11   # hud
MESTER_SILENT=1 MESTER_TEST_SCENARIO=menu_pages MESTER_USER_DIR=<empty scratch dir> timeout 120 <exe> menu ../../output out.png 3   # menu_pages
MESTER_SILENT=1 MESTER_TEST_SCENARIO=bonus MESTER_USER_DIR=<empty scratch dir> timeout 120 <exe> menu ../../output out.png 3.3,7.8,9.3,15   # bonus
MESTER_SILENT=1 MESTER_TEST_SCENARIO=menu_saves MESTER_USER_DIR=<empty scratch dir> timeout 120 <exe> rh1-wiezienie2 ../../output out.png 3   # menu_saves
MESTER_SILENT=1 MESTER_TEST_SCENARIO=menu_death MESTER_USER_DIR=<empty scratch dir> timeout 120 <exe> rh1-wiezienie2 ../../output out.png 3   # menu_death
MESTER_SILENT=1 MESTER_TEST_SCENARIO=props MESTER_TEST_PROP=o_obiekt0 timeout 120 <exe> rh1-wiezienie2 ../../output out.png 3   # props
MESTER_SILENT=1 MESTER_TEST_SCENARIO=door MESTER_TEST_DOOR=b_door17 MESTER_TEST_RETREAT=1 timeout 120 <exe> rh1-wiezienie2 ../../output out.png 3   # door
MESTER_SILENT=1 MESTER_TEST_SCENARIO=pickup MESTER_TEST_ITEM=medpack timeout 120 <exe> rh1-wiezienie2 ../../output out.png 3   # pickup
MESTER_SILENT=1 MESTER_TEST_SCENARIO=arsenal timeout 120 <exe> rh1-wiezienie2 ../../output out.png 62   # arsenal
MESTER_SILENT=1 MESTER_TEST_SCENARIO=noise timeout 120 <exe> rh1-wiezienie2 ../../output out.png 9   # noise
MESTER_SILENT=1 MESTER_TEST_SCENARIO=revolver timeout 120 <exe> rh1-wiezienie2 ../../output out.png 3   # revolver
MESTER_SILENT=1 MESTER_MOVE_PROBE=1 timeout 120 <exe> rh1-wiezienie2 ../../output out.png 16   # move
MESTER_SILENT=1 MESTER_TEST_SCENARIO=dialogue MESTER_TEST_DIALOG=Wiezien27 MESTER_TEST_PERSON=wiezien_kuchnia timeout 120 <exe> rh1-wiezienie3 ../../output out.png 6   # dialogue
MESTER_SILENT=1 MESTER_TEST_SCENARIO=ai MESTER_TEST_AI=fight timeout 120 <exe> rh1-wiezienie3 ../../output out.png 3   # ai-fight
MESTER_SILENT=1 MESTER_TEST_SCENARIO=ai MESTER_TEST_AI=far timeout 120 <exe> rh3-miasteczko2 ../../output out.png 3   # ai-far
MESTER_SILENT=1 MESTER_TEST_SCENARIO=ai MESTER_TEST_AI=patrol timeout 120 <exe> rh1-wiezienie3 ../../output out.png 3   # ai-patrol
MESTER_SILENT=1 MESTER_TEST_SCENARIO=walk MESTER_WALK_COUNT=1 timeout 120 <exe> rh1-wiezienie2 ../../output out.png 3   # walk
MESTER_SILENT=1 MESTER_TEST_SCENARIO=walk MESTER_WALK_LEVELS=chinatown MESTER_WALK_SPEED=3 MESTER_WALK_BUDGET=300 MESTER_WALK_TIMEOUT=400 timeout 120 <exe> chinatown ../../output out.png 3   # walk-chinatown (needs more than the default 110 s game budget; about 25 s of wall time at speed 3)
MESTER_SILENT=1 MESTER_TEST_SCENARIO=altfire timeout 120 <exe> rh1-wiezienie2 ../../output out.png 10   # altfire
MESTER_SILENT=1 MESTER_TEST_SCENARIO=melee timeout 120 <exe> rh1-wiezienie2 ../../output out.png 3   # melee
MESTER_SILENT=1 MESTER_TEST_SCENARIO=flash timeout 120 <exe> rh1-wiezienie2 ../../output out.png 3   # flash
```
`menu_pages`, `bonus`, `menu_saves`, `menu_death` need `MESTER_USER_DIR=<empty scratch dir>` (the runner creates one per run); the world argument `menu` opens the main menu.

## The failures found on `main` (547c442) and afterwards: stale probe or game bug
| Probe | Verdict | Evidence / fix |
|---|---|---|
| `retail` Glock reload (16,0), shots 4 vs 5 | stale + timing | Pickups are by touch (cshell 0x10022360/0x10022470, docs/retail-inventory.md), not `E`. The probe stood *near* the item, so the Glock ammo (two items 8 units apart, plus two bandages) was never taken: no ammo, reload from 0. The second Glock shot was lost to the 0.3 s cooldown under the clock mismatch. Rewritten: teleport onto the item, wait until it is gone; ammunition expectations derive from the level data (17 per `Glock ammo` item, cap `AMMO_MAX` 0x10066248). After the items merge (main 9d74032) a picked-up weapon is not drawn, the number key of its holster cell draws it (cshell 0x10060c8f); the probe asserts both. It also asserts that a reload does not advance while the game is paused, and the grenade throw/blast. |
| `inventory` thrown apple not taken back | stale (probe environment) | It ran on rh1-wiezienie1, the story level whose opening cutscene is active (`pickups::tick` returns while `opening.active`). Main's version (touch teleport) passes; the probe now runs on rh1-wiezienie3. |
| `impact` | stale | The default NPC `o_postac18` exists only in some levels ("Impact actor absent" elsewhere), the shot count was only logged, never asserted, and acquiring the Glock no longer draws it. Now asserts a fired shot that hit the wall. |
| `menu` Kiskina start not walkable | stale (probe environment), fixed on main | The walk started with the game paused: a hidden capture window is never focused and the alt-tab rule re-paused the game right after every load. Main's `pause_on_focus_loss(.., capture, ..)` exempts capture runs; the menu probe passes on the fixed-step clock. Not a level defect (the start point has free room in every direction). |
| `bus` | stale | Must start on rh2-wiezienie2 (the bus `o_cutscene` marker is there); the run goes miasteczko1 -> miasteczko2 -> burmistrz1. Now refuses another world and waits for every arrival (cutscene included). |
| `campaign` / `cell` | stale | First run on rh1-wiezienie1 (start point (4028,125,698) vs the walk target (80,700)): the cell is rh1-wiezienie2. The guard's shot needs a bullet line free of walls **and closed doors**, verified at the real camera position (a closed door pushes the player out of a spot that looks free; the shot hit the door). |
| `altfire` "second alt did not release the zoom" | stale (timing) | The M-14 reload (retail quirk: the camera stays zoomed) was still running when the script pressed alt again; with the clock fixed the whole 11 s script passes. Needs capture time 10 (the probe ends at capture time + 2 s). |
| `altfire`/`melee`/`flash`/`noise` after the items merge | stale | `NativeArsenal::acquire` no longer draws a weapon; the probes use `acquire_and_draw`. |
| `input` "player did not start on the ground" / "Space did not jump" | stale (timing) | Scripted 0.4 s gaps assumed 5 fps of game time; a jump lasts ~0.4 s. Rewritten: every stage waits for the game's reaction (airborne, scoped, panel open ...). |
| `noise` | fixed (probe expectation, docs/retail-ai.md "Noise") | Retail ignores a lone player shot for an idle guard (life-1 stimulus, provocation cleared in the same pass, 0x10049a57). The probe now shoots the guard (wound stimulus, life 2) and expects a reaction; `MESTER_TEST_NOISE=away` expects none. |
| `campaign` stage 6 (kitchen prisoner) | fixed (game regression) | `ifplayerseenby` / `setallfaza` / `ifhostileblizejniz` read the actor's `seen` flag (+0x149), not the contact flag; the port fed them the contact flag, which needs an `on_kontakt` phase (0x10049999). See docs/retail-ai.md "Script conditions and the AI flags". |

Also fixed on the probe side: the `zmienna` probe parsed the multi-megabyte scene JSON every frame (60 s for 8 frames).

## Adding a probe
1. Write the scenario (a system with an `active/finished/failure` resource; `probe_kit::{Cursor,Step}` for the script).
2. Add one row to `ProbeStatus::verdict` (probe_kit.rs) if it has such a resource; otherwise it must exit with an error itself.
3. Add a row to `SCENARIOS` in `tools/run_probes.py` (label, scenario, world, env, regex of the final log line, note, optional capture time). The runner lists scenario-looking names it does not know.
4. Update the table above by hand (its rows mirror `SCENARIOS`).

## Open
* Capture-aid scenarios without assertions (`props`, `door`, `pickup`, `hud`, `arsenal`, `revolver`, `move`, `dialogue`, `uv`, `npc`) only prove that the run completes and logs; their PNGs/logs are for the eye.
* The optimized dev profile is not in Cargo.toml (coordinator's decision, see above).

## Sound cue net (2026-09-30)
`tools/run_probes.py` runs every scenario with `MESTER_CUE_LOG=1` (the game prints each sound it asks for as `CUE 2d <file>`, `CUE 3d r=<radius> d=<dist> <file>`,
`CUE far ...` beyond its radius, `CUE music play|stop <track>`) and a scenario with an entry in `CUES` fails unless its log contains every listed pattern; `EVERYWHERE_NO`
patterns (the grenade's `zawleka`) fail any scenario that logs them. Nothing is audible: silent runs record the cues before they drop them.
