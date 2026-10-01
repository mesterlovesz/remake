# Optional HD textures (Real-ESRGAN)

A remake option, not a retail feature: the retail albedo textures (mostly 64-256 px) can be replaced by Real-ESRGAN upscales (1024 px and up).
**Default OFF**: without the switch the game loads exactly the retail-exported textures, so the default experience stays the original one.

## Switching it on
- Options menu > display list > remake row > **"HD textúrák be / ki"** (last but one row of the video page; the label says "be (új pályától)": it applies to levels and models
  loaded from then on, not to the level that is already on screen). Persisted as `"CHdTextures" "1"` in `autoexec.cfg` like the other remake options
  (`options.rs`, `RetailOptions::hd_textures`), default `0`.
- `MESTER_HD_TEXTURES=1` forces it on for a run (headless captures); `0` / unset leaves it to the saved option. `MESTER_HD_LOG=1` prints one `HDSWAP <path>` line per swapped load.
- Loader rule (`level_viewer::hd`, lib.rs): with the option on, `hd::path(rel)` returns `textures_hd/<rel>` when that file exists under the export folder, otherwise `rel`
  (fallback per texture: partly generated folders, sprites and everything that has no HD twin stay retail). Used by the world / movable world model textures
  (`retail_world::spawn_visual`, base texture only) and by the model skins (`models.rs` props/NPC instances, `props.rs` debris, `npcs.rs`, `pickups.rs`, `retail_weapons.rs`, `opening.rs`, `gunfire.rs`).
  Lightmaps, light groups, env/detail effect textures (`env_effects.json`), sprites, HUD, menus, cursors and fonts are never swapped.
- Nothing texture dependent changed: materials json (alpha mode, fullbright, effect tables, `content_u` door padding) is keyed by the *original* path, the HD file has the same
  aspect ratio and an exact integer scale (x4 or x2), so UVs and lightmap alignment cannot move.
- Mip chains: world textures go through the existing `MipQueue` (box filter, bilinear with nearest mip = retail cvars, docs/retail-visual.md). The retail model skins have a single
  level; 1024 px skins would shimmer on distant NPCs, so `retail_world::hd_skin_mips` appends the same box-filtered chain to `textures_hd/model_textures/..` images when they finish loading.

## Generating the files (output/ is gitignored: every user runs this themselves)
```
python -m tools.upscale_textures output                    # everything, first levels first, resumable (Ctrl+C is safe)
python -m tools.upscale_textures output --worlds knajpa --no-skins
python -m tools.upscale_textures output --plan             # count only
python -m tools.upscale_textures output --exe D:/tools/realesrgan/realesrgan-ncnn-vulkan.exe --pause 3
```
Needs the Real-ESRGAN ncnn Vulkan binary (github.com/xinntao/Real-ESRGAN-ncnn-vulkan releases, `realesrgan-x4plus` in its `models/` folder next to the exe; default
lookup `%LOCALAPPDATA%/Mesterlovesz2026/tools/realesrgan/`), a Vulkan GPU, Python with Pillow + numpy. The process runs at idle CPU priority and rests `--pause` seconds between
batches of 40 images, so the desktop stays usable. Output: `output/textures_hd/<same relative path>`, `_pool/<hash>.png` (unique data), `manifest.json` (per file source size, scale, key; per
group timings), `upscale.log`. Order: rh3-miasteczko0, model/prop/character skins, rh1-wiezienie2, knajpa, RH9-fabryka, then the other worlds.
`tools/audit_assets.py` ignores `textures_hd/` as a reference target and only checks that what the manifest lists exists with the exact integer scale.

## Recipe (tools/upscale_textures.py)
- Model `realesrgan-x4plus` (same as the 2026-REMAKE `crates/texgen` recipe: x4plus, PNG, tool-chosen tile), always run at x4 on RGB only.
- Scale by source size, integer only: longest side <= 256 px -> **x4** (<= 1024 px); <= 1024 px -> the x4 result is Lanczos-reduced to **x2** (<= 2048 px); larger sources (none of the
  exported albedo textures; the sky strip is not one) would be skipped. This keeps a level at roughly 0.4-1.1 GB of GPU memory (table below) instead of 4 GB+ at a flat x4 of the 512 px sheets.
- **Alpha is never sent through the model**: it is resized separately with bicubic (same padding), so cut-outs (fences, foliage, glass) and the fullbright mask keep clean edges. Textures used with
  blend / mask / add materials (586 files, from the `materials.json` next to each OBJ) first get the colour of transparent texels bled outwards from the opaque ones (no dark halos).
  Fullbright textures are upscaled too (RGB independent of the mask, mask bicubic); water / UV-pan textures likewise.
- World textures tile: the image is wrap-padded by 16 px before the model and cropped afterwards, so opposite edges match (checked: 2x2 tiling shows no seam); skins (clamped) are edge-padded.
- Per-channel mean correction to the source colour (clamped +-16 levels): the model drifts by a few levels, the captures show no colour shift.
- Identical textures across worlds are upscaled once (sha1 of the file + kind + role + recipe); the mirrored names are NTFS hard links (copies where links are impossible).

## Result (full game, 2026-09-30)
4,284 mirrored files from 1,163 unique upscales, **1.3 GB on disk** (logical size 5.1 GB of PNG before hard-link sharing), no failures, whole game in **21 min** on an RX 9070 XT at idle priority
(first level 56 s / 132 unique textures, skins 315 s / 407, rh1-wiezienie2 116 s / 55, knajpa 77 s / 119, RH9-fabryka 44 s / 72; later levels reuse the shared `ogolne` textures
and take 1-90 s; per-group timings are in `manifest.json`). Estimated GPU memory per level if all its HD textures are resident (RGBA8 + mips; Bevy also keeps a CPU copy):

| level | files | x4 / x2 | MB |  | level | files | x4 / x2 | MB |
|---|---|---|---|---|---|---|---|---|
| rh3-miasteczko0 | 208 | 180 / 28 | 857 | | rh1-wiezienie2 | 88 | 76 / 12 | 374 |
| knajpa | 272 | 252 / 20 | 1059 | | RH9-fabryka | 143 | 124 / 19 | 625 |
| rh3-miasteczko2 (largest world) | 245 | 192 / 53 | 1025 | | all skins together | 437 | 309 / 128 | 2104 |

(A level loads every skin its props, NPCs, pickups and weapons use, checked with MESTER_HD_LOG=1 on rh1-wiezienie2: ~300 skin loads, the same models loaded repeatedly share nothing across instances only when their skin list differs.)

## Verification (headless, MESTER_SILENT=1, 1280x720, `captures/hd/`)
Same start view and time, option off vs on (`<level>-off-006.00.png` / `-on-`, crops `cmp-<level>.png`): rh3-miasteczko0, rh1-wiezienie2, knajpa, RH9-fabryka. Geometry, lightmaps, fog,
fullbright and colours are identical; HD surfaces have cleaner, less grainy detail (the knajpa menu board and wall stains, prison plaster, factory sheet metal). Character: ESRGAN x4plus is
photo-oriented, so noisy retail grain becomes smooth painterly detail; some very fine noise patterns (plaster) look softer than the original at a distance. Cut-out gate texture: no fringe
against a green background. No seams at tile borders, no warnings in the logs. Frame time: the headless capture window is vsync-limited to 60 fps, so both settings read 16.7 ms on
rh3-miasteczko0 and rh1-wiezienie2 (knajpa 50-90 ms in both, it is the heavy level and the machine was shared); the visible cost is the load: RH9-fabryka with HD had one 0.8 s frame while
the textures uploaded. Expect a few hundred MB more GPU memory (table above).

Tests: `hd_paths_are_preferred_only_when_on_and_present` (lib.rs: off = retail path, on + present = `textures_hd/`, on + missing = fallback, no double prefix),
`options_round_trip_through_the_two_files` (CHdTextures persists), `missing_files_give_the_retail_defaults...` (default off), `display_and_sound_toggles_switch_the_retail_texts_and_mark_the_options_dirty`
(the menu row toggles and dirties the options).
