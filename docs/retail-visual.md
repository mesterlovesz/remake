# Retail world rendering (LithTech Jupiter D3D renderer in Lithtech.exe, cshell.dll, DAT v83/v85)

Evidence for `crates/level-viewer/src/{retail_world.rs,retail_world.wgsl,mirror.rs,tour.rs}` and `tools/{lightmaps,export_visual,render_dat,visual_tour}.py`.
Addresses are the module's own virtual addresses (Lithtech.exe image base 0x400000, cshell.dll 0x10000000). The public LithTech Jupiter
sources (research/d3d_renderworld.cpp, github.com/jsj2008/lithtech `runtime/render_a/src/sys/d3d`) name the shaders; the retail
exe contains the same cvar set (Lithtech.exe 0x591a00..0x592400 strings, initialisers 0x4b9800..0x4bbb40).

## What a render block section is (tools/render_dat.py `_node`)
Per render node: sections (base texture, secondary texture, **shader**, triangle count, **texture effect** file, lightmap width/height/RLE data),
44-byte vertices (position, UV, lightmap UV = always 0, colour, normal), 16-byte triangles (a, b, c, poly id), sky portals, occluders, light groups.
- shader 1 = Gouraud (vertex colour), shader 4 = lightmap-texture (base pass), shader 2 = lightmap (its own pass and **own vertices**, the
  lightmap UV lives in `RenderVertex.uv`, section texture `LightAnim_BASE`). Every shader 4 triangle has exactly one shader 2 twin with the same
  vertex positions and winding (verified on all 28 worlds: 100 % matched, 0 duplicate keys), so the exporter matches them by position.
- lightmap data: RGB, rows top to bottom, run-length coded: control byte with bit 7 set = repeat the next RGB (c & 127) + 1 times, else (c + 1)
  literal RGB pixels. All 4,000+ retail lightmaps decode to exactly width*height pixels. Sizes are 128 or 256 wide, 16..256 high.
  The lightmap UV is already texel-centred (u = (x + 0.5) / w); the base texture pass carries no colour.
- **Light groups** (`_read_light_groups`) exist only in podziemia1c (24 nodes, all named `LightGroup0`). A group holds per-section lightmap frames (an
  intensity byte per texel) and a per-vertex intensity stream (`0 n` = n + 1 vertices at 0, any other byte = one vertex; decodes to exactly the node's
  vertex count in all 24 nodes). `SetLightGroupColor` (Lithtech.exe 0x516a60 rebuilds each section lightmap from the stored data) adds `int(intensity * colour)`
  per channel, saturating at 255; the colour is set at run time by the `LightGroup` object (object.lto 0x10010200 sends `StartColor / 255` while the group is
  on and, with `CzestoscMigania` > 0, in the odd half periods, the phase starts dark; activation.rs already drives it). `tools/export_light_groups.py` writes
  `lightmap_base.png` (without groups), `lightgroup.png` (summed intensity, same atlas layout; base + int(intensity x authored colour) reproduces the historical
  baked `lightmap.png` exactly), `<stem>.visual.lightgroup.bin` (u8 per OBJ vertex) and a `light_group` entry in `lightmap.json`. The viewer draws the base atlas and
  adds `group intensity x colour` in the shader (lightmapped surfaces from the group atlas, Gouraud surfaces from the vertex stream carried in the second UV set),
  the colour follows `Activation::group_level` (`retail_world::sync_light_groups`). Before this the group was baked at its authored colour (1, .97, .88), i.e.
  permanently at full strength instead of dark / 27 % grey flicker. The vertex-stream rule (colour added like the lightmap) is by analogy: the code that consumes it
  was not disassembled.
- DTX header flags (materials.json `dtx_flags`): bit 0 = DTX_FULLBRITE, bit 3 = sections fixed, bit 7 = 32-bit copy; `dtx_user_flags` = surface type
  (0 stone, 1 sky, 2 metal, 3 wood, 4 water, 5 soft, 6 tiles). DTX alpha is data (fullbright mask, envmap reflectivity), not transparency.

## Shader selection and blend states (research/d3d_renderworld.cpp `CD3D_RenderWorld::AllocShader`, d3d_rendershader_lightmap.cpp)
- `Draw` sets `D3DRS_CULLMODE = D3DCULL_CCW` (clockwise faces are the front faces) and `D3DRS_ALPHAFUNC = GREATER`.
- shader 4 + `LightMap` cvar on = **Lightmap_Texture**: pass 1 (shader 2 = `CRenderShader_Lightmap`) writes the lightmap texture unblended (clamped),
  pass 2 (shader 4) draws the base texture with `SRCBLEND = DESTCOLOR, DESTBLEND = ZERO` and **fog colour white** (0xFFFFFF, `Saturate` 0):
  result = lightmap * texture. The vertex colour is not used by the lightmapped shaders (`SVertex_Lightmap_Texture` has none).
- shader 1 = **Gouraud_Texture**: texture * vertex colour (D3DTOP_MODULATE, 1x).
- A section whose base texture is missing: shader 4 returns no shader (not drawn), shader 2 still draws the lightmap alone, shader 1 falls back to plain
  Gouraud. The viewer's white fallback texture reproduces all three (chapel_mniejszy roofs, wiezienie1 `textures!!!` facades are lightmap-only).
- **Fullbright** (`DTX_FULLBRITE`): drawn after the lit surface with `SRCALPHA / INVSRCALPHA`, i.e. the texture alpha is the mask: lit window glass is
  fullbright while the plaster around it (alpha 0) keeps its lightmap. 294 fullbright world materials.
- Nothing is computed in linear space: Direct3D 8 shades the stored values and writes them to the screen. The viewer therefore samples the textures as
  plain unorm, does the maths on gamma values and outputs their linear equivalent to the sRGB target (which encodes them back). Feeding the vertex
  colours straight into Bevy's linear pipeline made every dark surface about 2.3x too bright (a 0.2 vertex colour drew as 0.48).

### The `lightmaps 0` line of autoexec.cfg does nothing
autoexec.cfg has `"lightmaps" "0"`. The engine's cvar is `LightMap` (Lithtech.exe 0x592264, default 1.0, object 0x5a84b8): a different name, and no
instruction in the exe reads its value (the only reference to 0x5a84bc is the initialiser 0x4bbaa7; the same holds for `DrawFlat`, `LightmapsOnly`,
`DetailTextures`, `EnvMapEnable`). The dev typed the wrong name; retail always draws lightmaps. The old exporter dropped all shader 2 sections and
rendered shader 4 with vertex colours (the `LightMap == 0` fallback of the source), which flattened every level and lost all light pools and shadows.

## Per-level render settings (cshell 0x1005b460, object.lto WorldProperties)
object.lto 0x1001ad60 reads exactly Widocznosc, Mgla_Wlaczona, Kolor_Mgly, Start_Mgly, Koniec_Mgly, Mgla_Nieba, Start_Mgly_Nieba, Koniec_Mgly_Nieba
(`Mgla_Liniowa`, present in chinatown, is never read). The client turns them into console commands `FarZ`, `FogEnable`, `FogNearZ`, `FogFarZ`,
`FogR/G/B`, `SkyFogEnable`, `SkyFogNearZ`, `SkyFogFarZ`.
- Fog: linear table fog (cvar `TableFog` 1) on the view depth: `f = clamp((end - z) / (end - start))`, `colour = mix(fogColour, colour, f)`. Lightmapped
  surfaces fog each pass separately (lightmap towards the fog colour, texture towards white), the product is in the shader.
- `FarZ` (default 10000, WorldProperties `Widocznosc`) is a hard clip plane; Bevy's reverse-Z projection has no far plane, so the shader discards beyond it.
- **The clear colour is the fog colour**, fog on or off: cshell 0x1005a8e0 calls `ClearScreen(0, 3, &fog colour)` on the bytes the world-properties
  message stored at `this+0x2114..0x2116` (B, G, R). A level without a WorldProperties object (rh1-wiezienie1, nic, wiez_wn1) clears black with no fog.
  Sky world brushes (DemoSkyWorldModel) use the sky fog (`Mgla_Nieba`, `Start/Koniec_Mgly_Nieba`), far 10000.
- Texture filtering cvars (defaults read from the initialisers): `Bilinear` 1, `Trilinear` 0, `Anisotropic` 0, `MipMapBias` 0, `Dither` 1: bilinear with the
  nearest mip level. The PNG exports carry no mip levels, so the viewer rebuilds the chain with a box filter and samples mag/min linear, mip nearest.
- Texture effects: the section's effect string names a `TextureEffectGroups/*.tfg`; its script (`UVPan.txt`: `Mat02 = SpeedX * Time`, `Mat12 = SpeedY * Time`)
  translates the base texture coordinates. Used by `pan.tfg` (0.002, 0.004 per second: woda_ciem/woda in miasteczko0 and fabryka) and
  `pan_szybszy.tfg` (0.002, 0.006: niebo_noc in wiez_wn1). Parameters are the floats after the script path in the .tfg (tools/lightmaps.py).

## Handedness: the levels were displayed mirrored (mirror.rs)
LithTech is left-handed (+X right, +Z forward), Bevy right-handed. Keeping LithTech's numbers and turning the camera to face +Z made every frame the
horizontal mirror of the retail view: levels were swapped left/right, model text was mirrored, and the world's `1 - u` correction only fixed surfaces
whose U axis is screen-horizontal (a sideways-mapped menu board in knajpa read upside down). Proof: rh3-miasteczko0's start view now reproduces
research/retail-reference/retail-rain-street-lamps.webp (lamps and kerb on the left, dim lights on the right, cobbles), it was the opposite before.
The level cameras render into an offscreen image shown flipped by a window camera; world numbers, physics and AI are untouched. Input follows the
displayed image: mouse yaw (`view::look`), strafe (`right` input), strafe/bob roll, death spin, blood overlay side, clock hands and rotators (no more negated
angles) use `mirror::SIGN`. The weapon camera and HUD draw unflipped. `level_viewer::DISPLAY_MIRRORED = false` restores the old behaviour.
World culling follows the mirrored winding: Bevy's front (counter-clockwise) faces are LithTech's clockwise ones, so `cull Back` equals D3DCULL_CCW.

## DTX texture commands: EnvMap, EnvMapAlpha, DetailTex (Lithtech.exe, round 2)
The effect comes from the DTX header CommandString (128 bytes at offset 36; `dtx_command` in the materials json), not from a flag: token 0 `DetailTex` = 1,
`EnvMap` = 2, `EnvMapAlpha` = 3 (parse 0x465080..0x4652dd), token 1 the second texture (a plain 2D DTX). Detail scale = header float at 30, angle int16 at 34
(trawa_duza 34, beton 15, angle 0). `tools/export_env_effects.py` writes `env_effects.json` (keyed by the base texture path) and `env_textures/*.png`.
- **`AllocShader` (0x4bf830..0x4bfa8a) decides what is drawn, and it disagrees with the old belief that the ~330 `EnvMapAlpha` materials reflect.** A lightmapped
  section (shader 4) with `EnvMapAlpha` falls back to plain Lightmap_Texture (`mov edi, 0xc` at 0x4bf91a; same in the public source
  research/d3d_renderworld.cpp 992-1060): window glass, elevator doors and every other lightmapped `EnvMapAlpha` surface (149 materials, +2 fullbright) are NOT reflective
  in retail. `EnvMapAlpha` reflects only on Gouraud sections (shader 1: 35 materials, mostly window panes of the miasteczko levels), `EnvMap` on both (ptica_blacha on the
  `ruracz` pipes of rh2-wiezienie2 / RH9-fabryka, 4 materials). `DetailTex` needs its file: `\textures\detail\detail 1` (the 26 beton materials) does not exist in the
  install, the shader validates false and draws plain; only trawa_duza -> `trawa_szara.dtx` (chinatown2, miasteczko0: 4 materials) is drawn.
- Passes (PreFlush 0x526b80 lightmap env, 0x52cbe0 Gouraud env, 0x525f90 lightmap detail, 0x52adf0 Gouraud detail): the base texture is stage 0, the second texture stage 1.
  EnvMap and DetailTex use `ADDSIGNED` (`base + second - 0.5`, cvars `EnvMapAdd` / `DetailTextureAdd` default 1, else MODULATE); `EnvMapAlpha` on Gouraud uses
  `MODULATEALPHA_ADDCOLOR` (`base + base.a * env`, the DTX alpha is the reflectivity mask). Lightmapped: result = lightmap x that (SRC DESTCOLOR, DEST ZERO, fog colour white);
  Gouraud: that x vertex colour.
- Env texcoords (0x5408a0): `TCI_CAMERASPACEREFLECTIONVECTOR`, 2D texture matrix `A x M` with A = scale -0.5/EnvScale (cvar, default 1) and offset 0.5 and M the camera
  rotation (0x5ab34c, set up at 0x535080), i.e. the world space reflection `R = E - 2 (E.N) N` of the eye ray about the vertex normal, `u = 0.5 - 0.5 R.x`,
  `v = 0.5 - 0.5 R.y`, wrap addressing, per vertex in retail, per pixel here. The rotation matrix identity is inferred from how the struct is built (if a view of the reflection
  turns wrongly with the camera, M is the inverse). Detail texcoords (0x525e90): `uv1 = 0.2 (cvar DetailTextureScale) x header scale x rotate(angle) uv0`.
- Not covered: models (LTB skins carry the same commands: `drzwi_winda` elevator doors are a model skin with `EnvMapAlpha dtl0009`), which use another renderer.
- Viewer: `retail_world.rs::effect_for` + `retail_world.wgsl` (binding 5/6 second texture, `effect` uniform); test `env_and_detail_effects_follow_the_retail_shader_choice`.
  Checked on chinatown2 (grass now has blade detail, before a blurry smear); the env pass is subtle by design (mid-grey environment textures).

## Mirror audit (round 2): every left/right sensitive thing
The world is LithTech numerics (x right, z forward) shown mirrored back through the offscreen flip (mirror.rs), so everything that carries LithTech numbers (world, models, NPCs,
props, doors, clocks, rotors, cutscene cameras, sky, pan) is correct as is, while everything built in Bevy's own right-handed camera frame (`camera.right()`, the weapon camera's view
space, Bevy quads and textures, UI) needs the flip accounted for. New helpers: `mirror::right(camera)` = the right of the displayed image as a world direction, `mirror::to_world(camera, view)`
= a point of the (unflipped) weapon view space as a world point, `mirror::QUAD_HAND` = scale.x sign of a camera-facing quad.

| Thing | Status | Evidence |
|---|---|---|
| Mouse yaw / pitch | ok | `view::look`; test `mouse_strafe_and_bob_follow_the_displayed_right` (mouse right turns the view towards `mirror::right`) |
| Strafe A / D | ok | movement formula `retail_movement` (velocity += (-s f + c r, -c f - s r)) with `right = SIGN x axis`; same test; D goes to the displayed right |
| Strafe roll, bob roll, death roll / spin | ok | `SIGN` on the roll; the test asserts a positive retail roll tilts the camera's up towards the displayed left (LithTech left-handed roll about +Z) |
| Head-bob sideways offset | fixed | it moved along Bevy's right (the displayed LEFT); now `b.right x SIGN` |
| Drunk view yaw wobble | fixed | applied with the LithTech sign of yaw, which is inverted in the mirrored frame; now `yaw - SIGN x turn` |
| Weapon view model handedness / position | ok | `view_transform` reflects Z only; weapon on the right of the window in all arsenal captures (captures/audit/ars-*.png); test `original_glock_muzzle_is_right_of_camera...` |
| Weapon bob (right offset) | ok | the weapon camera is unflipped, offset x is displayed right = retail right |
| Muzzle flash sprite | ok | view-space particle (unflipped camera) on the socket; captures |
| Muzzle smoke, sparks, muzzle light, casing position | fixed | were mapped through `camera.rotation x view`: the displayed left of the gun. Now `mirror::to_world` (captures: smoke and light rise from the gun, ars-sheet.png) |
| Ejected casing side | fixed | velocity used `camera.right()` (flew out of the LEFT of the screen); now `mirror::right` (ingram capture: the casing leaves to the right) |
| Revolver reload casings, grenade throw (8 units right), grenade hand socket | fixed | same helpers |
| Laser / flashlight sockets | fixed | `weapons_alt::socket_position` through `to_world`; laser dot and flashlight cone start at the gun (fl-af.png) |
| Scope reticle quads, weapon HUD, damage blood overlay | ok | UI is drawn to the window unflipped; `overlay_roll` (attacker on the right -> blood at the right edge, capture gf17-002.15.png, test `blood_overlay_rolls_toward_the_attacker`); the retail angle is the sign of `a.z b.x - a.x b.z` (cshell 0x10043b10) |
| NPC models (face, badge side, gun hand) | ok | not mirrored: the police badge sits on the wearer's left chest exactly like the skin texture (npc17-crop.png, pol0.png); aiming NPCs hold the pistol in the right hand (gf17-crops.png) |
| Billboards: d_sprite glows, flares, smoke, fx particles, blast sprites | fixed | quad art reads left to right on the flipped image only if the quad is mirrored: `sprite_step` uses `mirror::right`, `fx::tick` / explosions `scale.x = QUAD_HAND` (test `billboard_art_is_not_mirrored_on_the_displayed_image`) |
| World text, posters, signs, model text | ok | captures readable: clock face "ZIDGER", shop signs and licence plates in the intro, menu board in knajpa |
| Clock hands | ok | rh2-wiezienie1 at 22:14 local / :14 UTC shows the hour hand at about 10 and the minute hand at about 3 (captures/audit/clock1.png): clockwise, correct time |
| Door / rotator swing sense | ok | rotations are `Quat::from_axis_angle` / Euler with the retail numbers; LithTech's yaw and pitch formulas equal glam's numerically, no angle is negated anywhere. Capture captures/audit/door2-sheet.png: burmistrz2 `b_door1` (`Od_gracza`) swings open away from the player, hinge on the leaf's side |
| Cutscene cameras | ok | intro scenes (`story` capture): correct handedness, readable signs |
| Sky, clouds, texture pan (UVPan) | ok | same numerics; the UV pan sign is already handled for the mirror (round 1); the sky camera copies the main camera rotation |
| Sound panning | ok | the new 3D audio (spatial.rs, main) builds its ear axis with `Listener::from_camera(camera, mirror::MIRRORED)`, which flips Bevy's right like `mirror::right`; the old volume-only sound has no panning |
| Compass / minimap | n/a | none in the game |
| Symmetric Bevy quads (tracers, rain streaks) | ok | nothing to mirror |

## Lamp halos (round 2 finding: no defect proven)
The sprite maths matches the engine: colour `(a, a, a, a)` from the object Alpha (cshell 0x10002a30 -> object colour call), additive = ONE / ONE with the fog colour black
(0x53d5b0), z test on and z write off (0x4f6d60 sets ZWRITE 0, no ZENABLE change), texture RGB only. The old note (halos dimmer than
research/retail-reference/retail-rain-street-lamps.webp) compared radial profiles at equal screen radius, but the retail camera stands about 1.3 x closer to the lamp, so
its halo covers more pixels. What really differs near the bulb is the lamp fixture: retail shows a bright white bulb and a fixture that blends with the haze, ours a dim bulb
(Gouraud vertex colours 0.37..0.41 on `zielona blacha`) and a black housing (vertex colours 0). The video also carries no display ramp (see "Display gamma" below). After the ramp fix the street start frame and the reference agree closely (lamp heads, halo colour and size, fog); what still differs is the bulb itself (peak pixel of the bulb: remake (52,93,86), retail video (193,234,229)) and the sky gradient: the remake darkens towards the top of the frame (13,21,18 at the top row against the flat fog colour 24,37,33 from 22 % of the height down), the retail frame is nearly flat (20,30,27 at the top). Both are open (table at the end).

## Display gamma (round 3, verified against the engine; the old "calibrated at 1.15" premise was wrong)
* Lithtech.exe 0x4f6ed0 `SetGammaRamp` builds three 256-entry tables `pow(i/255, 1/g) * 65535` and calls `IDirect3DDevice8::SetGammaRamp` (vtable +0x48); `0x4f82a0` re-applies it whenever the cvars GammaR/G/B (0x5a8acc/0x5a8ab4/0x5a8a9c) change; the engine default of all three is 1.0 (identity). cshell 0x1005a21d copies keys.cfg `gamma` (shipped 1.15, menu slider 1..2) to `gammar/gammag/gammab`. Nothing checks windowed mode; autoexec.cfg has `windowed 0`.
* The reference videos are screen recordings: the ramp lives in the display hardware, so they show the picture BEFORE it. Measured: the retail menu frame is 0.96 x the source picture (`output/ui/menu/main_menu_1024.png`, 396 blocks; a 1.15 ramp would give 1.2 x), the street video's sky is (22,36,33) for the fog/clear colour (24,37,33) (a ramp would give (33,47,43)). So the two videos do NOT disagree: both are raw.
* Consequence: the retail player's screen was `pow(raw, 1/1.15)`: mid-tones +13 %, dark fog colours +30..40 %. The remake now applies the same ramp (`option_effects.rs`: one full-screen pass after the UI, in gamma space: linear -> sRGB -> `pow(x, 1/gamma)` -> linear, the target re-encodes). `RETAIL_GAMMA = 1.0`: the slider value IS the retail value (1.0 = identity, default 1.15). Menu 74 -> 87, street fog (24,37,33) -> (33,47,43) measured in captures.
* The earlier pass was a no-op: Bevy 0.18 draws bevy_ui in a `UiPass` AFTER `EndMainPassPostProcessing`, and the composite and HUD cameras shared one main texture (the ramp ran twice on the level, never on the UI). Now one `GammaEffect` on the HUD camera only, node edges `UiPass -> Gamma -> Upscaling`, `sub_graph = Core2d`.
* Owner decision point: if the picture should match the screen recordings instead of the real screen, set `RETAIL_GAMMA` to 1.15 (the slider default then leaves the picture raw).

## Additive effects (round 3)
Direct3D adds gamma values (`tex * colour`, `tex * a` while fading) to the frame buffer; the linear pipeline adds linear light. `fx::tint` therefore puts colour and fade alpha of additive sprites through the sRGB curve, which reproduces the displayed value over black exactly (bright backgrounds still add less than in D3D; decorations bake for the fog colour instead). The retail object colour alpha is unused by ONE/ONE: the cigarette smoke (0x10045097, colour (0.1,0.15,0.2), alpha 0.1) no longer carries `alpha(0.1)` (it was multiplied in twice).

## Model fog
Models (props, NPCs, pickups) use Bevy's `DistanceFog`, which mixes the fog colour into the LINEAR light of the fragment; retail blends it into the shaded gamma values (as the
world shader does). Round 2 patches the pbr fog library once it has loaded (`retail_world::patch_model_fog` rewrites the three `mix` sites of `bevy_pbr::fog` to convert to
gamma, mix and convert back), so a surface of gamma 0.35 at 50 % fog towards (24, 37, 33) is 0.225 as in retail (Bevy's own mix gave 0.28). Effect in the dark fogged levels
is small (mean change 0.2/255 on the miasteczko0 NPC captures); a warning is logged if a Bevy update changes the library text. Still approximate: Bevy's fog amount uses the radial
distance, retail's the view depth (a few percent at the screen edges).

## Worlds exported additively (round 2)
`outro` is exported completely (scene, collision, visual OBJ, lightmaps, props: 4 `o_obiekt`, `o_cutscene` `outro`). `katscena` is a DAT v70 file (an earlier prototype of the intro
cutscene: `o_cutscene` `intro`, models `Levelowe/cutsceny/glinowoz`, `drugi_do_intra`, `postacie/cutsceny/policjant_intro`, `bohater_intro`, 20 `d_sprite`, 11 `Light`): the
objects are exported (`katscena.scene.json`, `dat_version` 70, `geometry` "not decoded"), the world tree and models of v70 are not decoded, so it has no collision and no
render geometry. Every other retail world already existed (29 exported: the 28 supported + `nic`). `tools.export_world` reads v70 objects through `lithtech_dat.read_world`.

## Tools
- `python -m tools.export_visual <world.dat> ../GYARI <out>` (also used by `export_model_worlds`) now adds: `<stem>.visual.lightmap.{png,json,bin}` (atlas, 6 f32
  per exported face), `fullbright` and `texture_effect` fields in `<stem>.visual.materials.json`. The OBJ and all existing files are unchanged.
- `python -m tools.visual_tour <world> [stops] [--run] [--objects kind,name] [--texture text] [--tag x]`: headless tour (MESTER_TOUR, tour.rs): teleports to
  k-means stops over the walkable collision, or looks at objects/textures, shoots each stop and writes `captures/<world>/` sheets and a log with frame times.
  `MESTER_PICK="px,py;..."` logs the polygon/texture/UV under a pixel (pixels of the level image BEFORE the display flip: use x = width - x of the capture); `MESTER_WINDOW=WxH` sets the capture size; god mode keeps the tour alive.

## Performance
Release build, hidden 1280x720 window, tour stops (frame time per stop in `captures/<world>/<world>perf.log`): rh3-miasteczko0 (largest: 3 M lightmap pixels, rain,
68 sprites), chapel_mniejszy, RH9-fabryka, rh10-wiezowiec2, rh3-miasteczko2 all hold 16.6-17 ms (the 60 Hz cap; `MESTER_NOVSYNC` did not lift it on the hidden
window), only the first stop (level load) is 19-21 ms. The debug build runs 55-90 ms. The world is one mesh per material for the whole level, so Bevy's AABB
culling removes nothing, the GPU draws every triangle (30-70 k per level) and back-face culling halves the fill; no frame-time problem was found, so no
portal/BSP culling was added (retail's render-block occlusion is unnecessary at this size).

## Per-level status (captures: `captures/<world>/<world>-sheet0..1.png`, 4 stops each, tour stop 0 = start point; positions in `<world>.stops.txt`)
Defects found and fixed for every level: flat vertex-colour lighting (now lightmaps), too-bright dark surfaces (gamma), grey clear colour instead of the fog
colour, missing fullbright windows/signs, mirrored levels and text, poster/board textures with sideways mappings, unmasked fullbright plaster, missing far clip.
Round 2 (below) added the DTX texture commands, the display mirror audit, the light group layer and the additive brush blend (ONE / ONE with the object colour, the
texture alpha is not read, Lithtech.exe 0x53d5b0). The tour of round 2 (release build, 8 stops per level, `captures/<world>/*-sheet0..1.png`) shows every level without
regressions; stops that fall inside solid geometry (flat fog colour or black) are the k-means artefacts noted in round 1.

| Level | Checked (stops) | Result |
|---|---|---|
| rh1-wiezienie1 | start cell, 4 street/roof stops, glass orbit (cut by the opening cutscene) | lit windows glass-only, lamp pools, black sky (no WorldProperties); start faces the calendar wall (Kierunek) |
| rh1-wiezienie2 / 3 | start + 7 stops | cell corridors lit by lightmaps, plausible; start faces the doorway |
| rh2-wiezienie1 / 2 | start + 7 stops | ok; rh2-wiezienie2 start faces a plant/wall (Kierunek) |
| rh3-miasteczko0 | start (matches research/retail-reference/retail-rain-street-lamps.webp), water orbit, 3 aspect ratios; round 2: 8-stop tour, 4:3 comparison stops | fog/clear colour, lamp halos (see "Lamp halos": no defect proven, the fixture bulb is dimmer than retail's), water is fog-coloured plane at fog 1..4000; the black road sign at the start is real vertex data (`blacha 3` vertex colours 0) |
| rh3-miasteczko1 / 2 | start + 7 stops | ok (fog 100..4000 / 200..2800, rain) |
| chinatown / chinatown2 | start + 7 stops | ok; chinatown start faces a close wall (dark); chinatown2 grass has its `trawa_szara` detail texture (round 2) |
| knajpa | start (poster, phone, arcade cabinet), mirror/phone orbit | text fixed by the display mirror; cabinet body dark by lightmap (data) |
| burmistrz1 / 2 | start + 7 stops | ok (no fog, grey 127 clear); the "black slab" of burmistrz2 at (755,-80,-490) is the wall telephone (`telefon_stolowy`, textured, seen edge-on from its dark side): no prop piece of any of the 28 levels lacks its skin |
| chapel_mniejszy | start + 7 stops | ok; roofs with missing `Textures\GFX\*` are lightmap-only like retail |
| podziemia1 / 1a / 1b | start + 7 stops | ok; podziemia1b start = tunnel |
| podziemia1c | start + 7 stops (round 2 tour) | LightGroup0 now dynamic: lightmaps without the group + intensity layer at the LightGroup colour (dark first period, 27 % grey flicker every 3 s), Gouraud vertex stream applied |
| RH9-fabryka | start + 7 stops, water orbit | ok; lens flare/wave decorations by decorations.rs; `ruracz` pipes get the `ptica_blacha` EnvMap |
| Rh7a-Tunele | start + 7 stops | ok |
| nic | start + 7 stops | ok |
| rh10-wiezowiec1 / 2 / 3, wiez_wn3 | start + 7 stops each | sky world skyline with sky fog; the skyline is upright but its bases sit 64 units above the sky camera (SkyPointer y -1680, `InnerPercentY` 0, so the camera never moves vertically): buildings look like they float above the cloud band, per the data (not verified against retail) |
| rh12-lab1 / 2 | start + 7 stops, frosted glass orbit | translucent glass blends correctly |
| wiez_wn1 / 2 | start + 7 stops | niebo_noc pans (pan_szybszy); wn2 start = lift lobby |
| Back-face culling check | start point and a mid NPC position of 11 levels, culled vs double-sided | 0-0.8 % pixel difference (blood overlays); differences appear only at k-means stops that lie inside solid geometry |

Round 2 exported `outro` (full) and `katscena` (objects only, DAT v70), see "Worlds exported additively"; every retail world dat now has an export.

## Round 3 status table (presentation audit, agent/pres)
Legend: **ported** = implemented from retail code/data with evidence and a test or capture; **identical** = proved the remake already matches (or retail does nothing); **dead** = retail code/data never reaches it; **approx** = still approximate; **open** = not resolved.

| # | Item | Result | Evidence / where |
|---|---|---|---|
| 1 | Grenade blast layers | ported | type 8 controller + embers + chunks + multiply smoke: docs/retail-blast.md, `retail_weapons.rs`, test `the_blast_controller_fires_its_stages_once_and_never_reaches_the_fourth_smoke`, captures `captures/pres/blast-*.png` |
| 1 | Hit-blood chunks (`miecho`) | ported | corpse lower-body hits (0x10005920); gore/puff vectors exact (0x10005360); blood puffs do not fade, `krew4` is static (arg 17 = 0) |
| 2 | Limousine `blik0..3` | ported | `opening.rs` `Light`: additive billboards on `socket_blik0..3` (front `blik1.spr` 0.2, rear `blikred.spr` 0.1, white, always on, follow the car: cshell 0x100448bc.., 0x1004a4c0); capture `captures/pres/limo-003.00.png` (headlight glow and red tail light) |
| 3 | Enemy flashlight / M-14 laser | dead (static) | `[w+0x104]` never set nonzero (docs/retail-lighting-research.md); the spec is recorded |
| 3 | Snow `snieg` | dead | `Snieg 0` in every `d_emiter_opadu` (3 worlds) |
| 4 | Lamp fixtures / halos | approx | ramp fix brought the start frame close to the reference; bulb peak (52,93,86) vs retail (193,234,229) and the dark top gradient of the sky remain open |
| 5 | Model skin EnvMap / EnvMapAlpha / DetailTex | identical | retail ignores DTX commands on models (only world `AllocShader` reads them); reflections on models come from render styles (`lustro`, `heli_lata`, `fn_shotgun`, `zegar`: unported, docs/retail-lighting-research.md) |
| 6 | Model fog | ported | gamma-space mix (round 2) + view-depth distance (`patch_model_fog`: `distance = -view_z`) |
| 7 | Cigarette smoke blend | identical + fixed | additive (flags2 2), colour (0.1,0.15,0.2); the extra `alpha(0.1)` (premultiplied twice) removed |
| 7 | Head clips `gada` / `mruga` | ported | own looping clock; SetPhase restarts the clip when `animacja_glowa` is non-empty (0x10041ea4); dialogue: node start `gada`, answer list / node end `mruga` (0x10019b39, 0x10018c60, 0x1001950f) |
| 8 | Display gamma | ported (decision) | see "Display gamma": the ramp exists in retail, the videos are pre-ramp, the old slider was a no-op; now exact |
| 9 | podziemia1c light groups | ported | timer drops the flip remainder, server dt clamp 0.2 s (Lithtech.exe 0x46cd80, 0x476784); test extended |
| 10 | Non-finite UVs | explained + fixed | the DAT carries inf/NaN UVs for sections whose texture the compiler could not find (chapel_mniejszy 5708 faces, wiez_wn1 94, ...); retail draws the lightmap pass only; `tools/export_visual.py` writes 0, the viewer notice is debug level |
| 10 | "Camera order ambiguities" | not reproduced | six captures (street, sky world, menu, story, lab, prison) show no such warning |
| 11 | Policeman brightness | open | the skin is dark blue-grey (46,48,58); no x2 gain exists in the model path (default style = MODULATE 1x, lights saturate at 1 before the texture, Lithtech.exe 0x507f70, 0x538a00): retail's hall simply lights him more; model lighting is unported (next row) |
| 11 | Model lighting | unported | ambient grid, FastLightObjects filter (now applied), D3D attenuation, 4 lights per model: docs/retail-lighting-research.md |
| 11 | Projected model shadows | dead by default | `ModelShadow_proj_enable` 0 in autoexec.cfg, only the options menu writes it |
| - | Window title | ported | `A mesterlövész v 2.33` (scripts/app_name.txt) before the edition name; `sniper.ico` not applied (bevy_winit has no icon API here) |
| - | `locale 0` HUD set | identical | retail's locale 0 fallback dirs are incomplete in this install (docs/retail-binary-features.md), not used |
| - | Font glyph gaps | open | retail asks for `nawiaslewy/prawy`, `apostrof`, `dollar`, `procent` files that do not exist; 0x100329d0 creates the glyph node with a null surface and uninitialised width: behaviour unverifiable |
