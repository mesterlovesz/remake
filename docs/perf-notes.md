# Performance notes (owner report 2026-09-30: "28 fps although unlocked")

## Cause
The launchers (`Mesterlovesz-Ujrairva.cmd`, `Palyanezo.cmd`) start `crates\level-viewer\target\debug\level-viewer.exe` and `Forditas.cmd` runs a plain `cargo build`. `Cargo.toml` had
no `[profile.dev]`, so **Bevy, wgpu, taffy, cosmic-text ... were compiled at opt-level 0** (only parry3d, retail-movement and mission-runtime were optimised). The frame cap is off by
default (`max_fps = 0`) and was never the limit; the CPU was.

Measured on this PC (hidden 1280x720 capture window, `MESTER_TEST_REALTIME=1 MESTER_NOVSYNC=1 MESTER_FPS_LOG=1`, nothing else running; a hidden window is held at 60 fps by the compositor,
so "60" means "at least 60"; CPU = process CPU seconds per wall second over 12 / 20 s, all threads):

| Build | Main menu | Level rh1-wiezienie2 |
|---|---|---|
| old `cargo build` (opt-level 0, what the owner ran) | 27-35 ms per frame = **29-37 fps** (the owner saw 28 / 35), 2.7 cores busy | 61-71 ms = **14-16 fps**, 2.0 cores busy |
| new dev profile (below) | 60 fps (capped), 0.6 cores | 60 fps (capped), 1.1 cores |

## Fix
`crates/level-viewer/Cargo.toml` now has Bevy's recommended dev profile (debug assertions and incremental builds stay):
```
[profile.dev]                     opt-level = 1   # our code
[profile.dev.package."*"]         opt-level = 3   # every dependency
```
It applies to the path crates too (retail-movement, mission-runtime keep their explicit 3). Nothing else changes: the launchers and `Forditas.cmd` keep working, they now simply
produce an optimised debug exe. `Mesterlovesz-Ujrairva-Gyors.cmd` (new) builds/starts the full `--release` exe for the last few percent (`target\release`, built on first use).

### What the owner should expect from the build
* First build after this change: **~27 minutes** on this PC (4 jobs, another build running at the same time; the same profile with the dependencies at opt-level 2 took 17 min on a quiet
  machine). It compiles every dependency again, once. The old profile needed 5.5 min.
* Later builds only recompile `level-viewer` (about 1-2 minutes; a change in `retail-movement` / `mission-runtime` a bit more). The `target` folder grows (about 20 GB with debug info on this PC);
  `Forditas.cmd` builds into `crates\level-viewer\target`.
* Do not stop a build in the middle of the last `level-viewer` step: an interrupted incremental compile can leave a broken cache (`LNK2001 ... anon.` at link time). Fix: delete
  `target\debug\incremental\level_viewer-*` and build again.
* Probes / tests use the same profile through `cargo test` / `tools/run_probes.py` (its `--config` speed-up is now what `Cargo.toml` says, dependencies at 3).

## Per-frame waste found and removed (all cheap, but they ran every frame)
* `hud::update` assigned `UiScale` every frame: a `ResMut` write marks the resource changed and Bevy relayouts the whole UI tree (hundreds of nodes with the text aid) for nothing.
  Now written only when the window size changes. The same for the main-menu root visibility, the canvas picture handle, the page part visibility, the notice text and the ghost label positions
  (`frontend::update`, `menu::ghosts`).
* `retail_ui::draw_text` cloned the string of every bitmap text every frame to compare it with the cached one; now compared in place.
* `menu::render` despawned and respawned the whole label layer (about 1000 glyph, halo and strip nodes with the text aid) on every hover change; now only the recoloured labels are edited
  (see below).

## The "buttons disappear when I move the mouse" bug (same report)
Root cause: `menu::render` rebuilt the whole label layer whenever the scene differed, and a hover change (the highlighted row) is a scene change. The new label entities only get their
glyph children one frame later (`retail_ui::draw_text` runs before `menu::render` in the frame), so **every change of the row under the pointer showed a frame with all rows blank**;
the slow debug frames (30-60 ms) made it a visible blink and the screenshot caught it (title art and cursor only). Fix: a scene that only re-words / re-colours its texts is edited in
place (`menu.rs::same_shape`), `draw_text` swaps the glyphs of just those labels in one command flush, so no frame is ever blank. Regression: probe `menu_hover`
(`MESTER_TEST_SCENARIO=menu_hover MESTER_PROBE_REAL_POINTER=1 MESTER_WINDOW=1920x1080`) sweeps the pointer over the rows one row per frame through the window's own cursor position and checks
the frame right after each move (drawn text entities == wanted): it fails on the old code ("no blank frame after the pointer changed rows") and passes now; unit test
`a_recoloured_scene_is_edited_in_place_and_a_different_one_is_rebuilt`.

## Measuring
`MESTER_FPS_LOG=1` logs "FPS n (x ms per frame)" every two seconds; with `MESTER_TEST_REALTIME=1 MESTER_NOVSYNC=1` and a capture command line the run is headless and silent
(`level-viewer.exe menu ../../output x.png 20`). Process CPU: read `TotalProcessorTime` of the process from PowerShell after it exits.
