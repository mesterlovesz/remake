# Opening presentation evidence

Read-only sources: decoded `scenki.txt`, `text_keys.txt`, `postacie.txt`, DAT
entities, original LTB models, DTX textures and WAVs.

The exporter follows `pierwsza_faza`, `nast_faza`, `cutscene`, `runworld`:
22 records,77.97seconds of authored durations, then `rh1-wiezienie2`.
Speech (`gadka`) and scene audio (`glos`) are separate; Hungarian subtitle
keys come from the installed release.

The independent LTB reader handles PC container9/model23–25, rigid/skinned/
vertex-animated geometry, hierarchy, channels and sockets. Raw weights remain
in JSON. Runtime skinning normalizes their sums; ten original car/body
vertices total.5. Exact engine handling still needs comparison.

Format fields cross-checked with primary source repositories:

- https://github.com/haekb/io_scene_lithtech/blob/master/src/reader_ltb_pc.py
- https://github.com/jsj2008/lithtech/blob/master/runtime/model/src/model_load.cpp
- https://github.com/jsj2008/lithtech/blob/master/runtime/render_a/src/sys/d3d/d3dmeshrendobj_vertanim.cpp
- https://github.com/leoschur/blender-lithtech-dat-import

Research copies are outside `remake`; they are not runtime dependencies.

## Animation placement correction (2026-09-28)

The first implementation omitted LTB animation bindings after the socket table.
These contain dimensions and an animation-specific root translation. Their
absence put the street camera behind facade geometry and the medical scene at
the wrong height. All 74 existing model exports were regenerated from original
files. The runtime now adds the binding translation to the root channel before
parent/child and socket composition, matching `TransformMaker::InitTransform`.

- Street rig: `[-700, -25, 0]` for all animations except `scena17`, which is
  authored as `[0, 0, 0]`; the exception is retained.
- Medical rig: `[-48, 200, 420]` for all three animations.
- Regression tests cover the source offsets and composition with a rotated
  root and an interpolated camera child. Both reproduced the omission before
  the correction.

Primary implementation references:

- https://github.com/jsj2008/lithtech/blob/master/runtime/model/src/transformmaker.cpp
- https://github.com/jsj2008/lithtech/blob/master/runtime/shared/src/objectmgr.cpp

Capture mode accepts comma-separated times as its fourth argument, allowing
one playback to verify multiple shots and the final world transition.

World-model translucency now reads the DAT `Alpha` and `Additive` properties.
The street light cones are authored with alpha 0.09; treating them as alpha-cut
geometry had created opaque white noise cones. Missing texture references stay
missing and use vertex color instead of the importer's magenta marker, following
the Gouraud fallback in `CD3D_RenderWorld::AllocShader` with lightmaps disabled.
This does not invent replacement texture paths or alter invalid source UVs.

- https://github.com/jsj2008/lithtech/blob/master/runtime/render_a/src/sys/d3d/d3d_renderworld.cpp

The corrected build completed the full opening and transitioned into
`rh1-wiezienie2` on 2026-09-28 without a runtime error. Ten captures from one
playback are in `output/intro-build-*.png` (6, 16, 24, 32, 44, 54, 64, 69,
75 and 82 seconds). The 54-second arrest scene now shows actors and cars in
the street where `output/camera-before.png` showed only a facade. The
64-second medical shot shows the protagonist on the bed; the 82-second shot
shows the destination world and HUD. The street cones retain their authored
translucency. The Python suite passed all 14 tests, the viewer passed all five
tests, and the final executable rebuilt successfully.

These checks verify the placement correction and execution, not complete retail fidelity.
FOV, sky, lighting, render styles, animation transitions, bitmap font,
spatialization and interpolation need further comparison. Material styles
currently approximate opaque/blended/masked surfaces; special effects remain.

HUD images use `misc/panel_l/HUD` from the localized release. Readable subtitles
currently use the local Windows `consolab.ttf`. Props use authored placement
and a baked default pose, without their gameplay scripts or collision.
