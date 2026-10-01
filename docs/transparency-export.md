# Source transparency export

The previous OBJ exporter discarded the rendering properties of named DAT
objects. All MTLs declared `d 1`, and the viewer used the texture path alone.
This made the transparent background pixels of authored grates visible.

`export_model_worlds` now joins render model names to DAT object names. Each
export writes a `<name>.visual.materials.json` object keyed by MTL material ID.
It records `alpha_mode`, `alpha_factor`, `alpha_cutoff`, source class, texture,
shader, DTX flags, user flags, and command string. Material identity includes
the section shader. The MTL remains an ordinary portable texture reference;
the viewer must consume the sidecar for render state.

Retail evidence from `rh1-wiezienie1.dat`:

- `b_transparent1`: class `b_transparent`, Alpha 1, Additive 0;
  `textures/ogolne/kratka.dtx` has 52,224 zero-alpha pixels and 12,288 alpha-255
  pixels. The remaining 1,024 pixels have alpha 1.
- `krataceladol`, `kratacelagora`, `krata1`, `krata2`: class
  `b_szuflada_przestrzelna`, Alfa 1; `textures/sprzety/krata.dtx` contains 48,384
  zero-alpha pixels and authored edge coverage. These use blending, preserving
  edge alpha rather than inventing a cutoff.
- Glass `b_transparent25` uses Alpha approximately 0.09 and
  `textures/detail/dtl0010.dtx`; its texture alpha ranges from 0 through 255.
- Ordinary `b_door25` remains opaque at Alfa 1.

Transparent brush classes, shoot-through sliding gates, opacity below one,
and Additive are explicit render state inputs. Class rendering interpretation
is based on original object classes/properties and texture content; the retail
server DLL implementation was not disassembled to verify every class flag.

DTX alone cannot determine surface transparency: alpha also stores reflection
and fullbright information. DTX user flags are surface types, not alpha modes.
The [LithTech texture loader](https://github.com/jsj2008/lithtech/blob/master/runtime/render_a/src/sys/d3d/d3d_texture.cpp)
reads the `AlphaRef` command for cutout state; none of these retail DTX files
contains that command. The
[render block source](https://github.com/jsj2008/lithtech/blob/master/runtime/render_a/src/sys/d3d/d3d_renderblock.cpp)
defines source shaders 1 as Gouraud, 2 as lightmap and 4 as lightmap texture.
None of these source shader IDs independently means transparency.

PNG conversion preserves all authored alpha bytes for transparent materials,
including an all-zero image. The existing all-zero-to-opaque treatment remains
for ordinary textures with unused alpha. No RGB colors are replaced, no mesh
faces are removed, and collision exports are unchanged.

Regenerated 28 supported worlds, 885 named world models, and 3,869 material
records: 3,285 opaque, 552 blend, 32 additive. Detailed world names are in
`output/transparency-export.report.json`. Missing source textures remain listed
in each existing visual export report; no replacement texture is invented.

Validation: eight tests across `test_transparency`, `test_visual`, and
`test_render` pass, including decoded PNG alpha, zero-alpha policy, exact
authored opacity, additive and reflection metadata, section identity, and
unchanged world triangle counts. Full Python discovery additionally exposed
unrelated weapon export failures because that exporter double-decodes the
plaintext retail scripts; reported to the integration owner.
