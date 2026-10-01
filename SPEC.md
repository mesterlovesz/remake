# Mesterlövész remake: specification and research baseline

## Objective

Rebuild the Hungarian retail release of *A Mesterlövész* (Sniper: Path of Vengeance)
from the user's installed copy. The first playable milestone covers the opening
prison missions. The data and runtime architecture must allow the remaining game
to be added without replacing the world or collision representation.

## Fidelity contract

- `../GYARI/` and the ISO are read-only reference inputs. Never patch the retail
  executable, DLLs, world files, or assets.
- Preserve observed retail behavior, including bugs. The bleeding bus stays at
  its authored position. Jump plus crouch flight remains part of the original
  movement mode.
- Do not claim 1:1 behavior from asset inspection alone. Compare the remake with
  a retail playthrough before marking a mission or mechanic complete.
- Generated assets remain local and are not packaged with source code.

## Verified retail facts

- Installed game: version 2.33 (`GYARI/scripts/app_name.txt`), LithTech Jupiter,
  32-bit Windows executable. The manual targets DirectX 8.1.
- 31 compiled `.dat` worlds, mostly DAT version 85; 31 `.pth` path files.
- `cshell.dll` names `worlds\\rh1-wiezienie1` as an initial world. World object
  transitions prove `rh1-wiezienie2` -> `rh1-wiezienie3` ->
  `rh2-wiezienie1` -> `rh2-wiezienie2`.
- `rh1-wiezienie1.dat` has 327 objects and 52 world models. Its `PhysicsBSP`
  contains 7,649 polygons; its start point is `(4028, 125, 698)` in native
  LithTech coordinates.
- `rh3-miasteczko1.dat` contains `o_obiekt27` with model
  `models\\Levelowe\\autobus.ltb` at `(1600, -260, 1384.16)`. This is an authored
  bus placement. A reference image shows a bloody bus; the exact material and
  event behavior still need in-game comparison.

## Capability map and order

| Module | Responsibility | Depends on |
|---|---|---|
| `asset-import` | Read DAT v85, DTX, LTB, scripts and export a portable scene | retail input |
| `world-query` | Identify ground/walls and query contact on imported triangles | `asset-import` |
| `retail-movement` | Reproduce acceleration, jumping, crouching and known flight bug | `world-query` |
| `presentation` | Render the original scene, UI, audio and cinematics | `asset-import` |
| `mission-runtime` | Run entities, dialogue, AI, combat, triggers and saves | all above |

The presentation and mission modules can be developed per mission after their
shared formats are understood. Movement modes are replaceable through one
controller boundary; the default stays retail-accurate.

## First implementation slice

Read the first three prison DATs, export static collision as OBJ and all world
objects as JSON. Export movable world-model collision separately, linked to
the corresponding entities; do not bake opening doors or moving grates into
static collision. Preserve native coordinates and raw flags. Verify the source
counts, start point and bus placement against the retail files. A Rust world
query module will accept triangles independent of the rendering engine.

This slice is a data and collision foundation, not a playable remake. The first
playable milestone additionally requires textured render geometry, LTB models,
DTX images, collision with movable objects, retail movement measurements,
combat, AI, dialogue, doors, cutscenes and mission transitions.

## Current evidence and limits

### Playable presentation milestone (2026-09-28)

- `Jatek.cmd` starts the authored 77.97-second two-part opening, original WAV
  speech/scene audio and Hungarian subtitles, then loads `rh1-wiezienie2`.
- Native LTB meshes, skeletons, animation channels and sockets now drive the
  opening actors/cameras. Vertex animation is supported for the animated can.
- Walking, running, jumping, crouching, stamina, floor/wall sweeps and 18-unit
  stair stepping work on the static triangle mesh. Retail movement constants,
  stance impulses and centre lift were recovered from `cshell.dll`.
- Localized HUD images and the original crosshair are displayed. Generic
  running footsteps play on the recovered .4-second cadence.
- The first three worlds now also render104 separate world models (doors,
  bars, glass) and341 authored environment props. Their runtime collision,
  interaction and gameplay scripts remain unimplemented.
- Full opening execution and transition were smoke-tested;28 tests passed
  across Python importers, Rust geometry, movement and OBJ readers.28 original
  world SHA-256 values still match the pre-existing export manifests.
- This milestone is not a completed mission or a verified1:1 remake. Combat,
  gameplay NPCs/AI, dialogue interaction, saves and remaining transitions are
  still pending. See `docs/retail-movement.md` and `docs/presentation.md` for
  measured values, provisional values and visual limitations.

- All 28 DAT v85 worlds have static collision, separate authored movable-model
  collision, and entity JSON exports. The three remaining worlds use DAT v83 or
  v70 and require separate format readers.
- Render geometry parses on all 28 DAT v85 worlds. Textured OBJ/MTL/PNG exports
  exist for the three opening prison worlds and `rh3-miasteczko1` for bus study.
- The DTX reader handles uncompressed BGRA images. DXT1/DXT5, embedded world
  lightmaps, decals and sky are not represented yet. An all-zero alpha channel is treated as opaque in this
  diagnostic export; game-specific alpha behavior still needs verification.
- The first prison world contains 36 referenced nonfinite UV values in its
  retail render vertices. The exported OBJ retains them. The diagnostic viewer
  marks affected materials magenta and reports them; retail rendering behavior
  still needs comparison. The bus world has 96 such UV records.
- Some authored texture references do not resolve in the installed tree:
  `textures!!!\\elewacje\\elewacja5.dtx`,
  `textures!!!\\ogolne\\blacha 4.dtx`, and
  `textures\\sciany_sufity\\cegły jasne.dtx` in the first world, plus
  `textures\\ogolne\\asfalt_mokry.dtx` in the bus world. The first two have
  apparent counterparts under `textures\\`; no automatic alias is applied.
- The authored bus object and location are verified, but whether that exact
  instance produces the bleeding-bus screenshot is unverified. The bus base
  skin contains no visible blood; the blood may come from an in-game effect.
- The Rust query finds ground at Y=72, 53 units below the first-world start
  point. This tests raw triangle contact only, not retail player physics.
- Five retail script catalogs use a reversible adjacent-character substitution;
  decoded research copies are in `output/decoded_scripts/`. The original scripts
  are unchanged. The decoded cutscene catalog exposes models, sockets, phases,
  timing and sounds needed for later mission reconstruction.
- `level-viewer` now includes walking/HUD and opening playback. It is not yet
  a completed mission or a 1:1 gameplay build.
  The three opening prison exports were smoke-tested on this Windows machine
  with the Vulkan backend; DirectX 12 lost the GPU device during a hidden test.

## Commands and layout

- Import/test: bundled Python 3.12, `python -m unittest discover -s tests -v`
- Export: `python -m tools.export_world ../GYARI/worlds/rh1-wiezienie1.dat output`
- Visual export: `python -m tools.export_visual ../GYARI/worlds/rh1-wiezienie1.dat ../GYARI output`
- Decode script catalogs: `python -m tools.decode_scripts ../GYARI output/decoded_scripts`
- Viewer: from `crates/level-viewer`, `cargo run -- rh1-wiezienie1 ../../output`
- `tools/`: retail readers and scene export; `tests/`: integration checks against
  the user's local files; `output/`: generated portable data; `docs/`: research.
- Rust runtime will live in `crates/` and consume exported data.

## Boundaries and verification

- Always validate binary offsets and indices before using them. Verify each
  exported level against the corresponding DAT and original game view.
- Ask before distributing retail assets or replacing the retail movement mode.
- Never alter the installed game or silently correct retail quirks.
- Phase 1 acceptance: parsers work on the first three levels; collision OBJ and
  entity JSON are produced; bus position regression check passes; source files
  have unchanged SHA-256 hashes.
- Full-game acceptance: every mission can be completed in sequence and its
  observable gameplay, presentation and quirks match the retail baseline.

## Research references

- Retail manual: `../disc/A mesterlövész/kézikönyv.pdf`.
- Retail files: `../GYARI/`.
- Format cross-check: https://github.com/leoschur/blender-lithtech-dat-import
  and its Kaitai DAT v85 schema. The research clone in `../research/` is not a
  runtime dependency and is not copied into this project.
