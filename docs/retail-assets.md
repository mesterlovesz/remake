# Retail asset completeness (audit, the missing head of `stara dziwka`, cigarette smoke)

Owner report: "does this woman have a head now at the start? until now she did not really have one" - the old prostitute
(`postac stara dziwka`, `o_postac2` of rh3-miasteczko0 at (532,-76,601), dialogue `Prolog11`). Evidence addresses are cshell.dll.

## Root cause: a stale export, not a renderer bug
`postacie.txt` gives her `glowa_specjalna`, `socket_glowa glowa`, `model_glowa models\postacie\stara_dziwka_glowa.ltb`,
`skin_glowa skins\postacie\stara_dziwka_glowa.dtx`. The runtime already attached such heads (commit b56a12e, `npcs.rs`), but that commit
made `tools/export_gameplay.py` write `head_asset` **and the head model/skin** into `<world>.gameplay.json`, and the exporter was only
re-run for the three worlds that had a head-carrying character (burmistrz1, chapel_mniejszy, knajpa). Every other level kept the older
`gameplay.json` without `head_asset` (rh3-miasteczko0's file was written 06:55, the commit landed 12:06), so the head model
`stara_dziwka_glowa.ltb.json` and its skin `stara_dziwka_glowa.dtx.png` never existed and the runtime skipped the head silently.
Fix: `python -m tools.export_gameplay ../GYARI output --worlds <all 28 worlds with a gameplay.json>` (22 s). The re-export is purely
additive (checked key by key against a backup: only `characters/*/head_asset` appeared, plus 2 models `stara_dziwka_glowa`, `cywil2` and 3
skins in the report). Chinatown, Tunele, fabryka, wiezowiec*, lab*, wn* ... had the same latent gap for every `glowa_specjalna` /
`model_glowa` character (cigarettes and ties of the Italian soldiers, the China officers' heads, ...).

## What the woman looks like now (headless captures, `MESTER_SPAWN` in front of her)
`C:/Temp/mester-assets-shots/`: `d2.png` (head with the seeded pre-fix binary and the fixed data: only the export was
missing), `e2.png` and `e2_crop.png` (final build, 2.6x brighter crop): long blonde hair, face, lace gloves, the raised hand and a thin
smoke streak rising from the fingers. Head orientation/position are those of the other head characters (burmistrz, barman): the
`glowa` socket transform of the body, so no floating or rotated head; the display mirror (`mirror.rs`) flips the whole image like
every other character, the face reads the right way round.
The dress is dark red (`stara_dziwka.dtx.png`) and only looks black in the unlit alley of the level lightmaps.

### The cigarette
* `socket_papieros dymek` is **the smoke socket** (cshell 0x1003f46d stores it at character +0xd9c; 0x1004476c resolves it to an
  engine socket handle at actor +0x23c), not a model socket. `socket_weapon bron` (+0xe1c, actor +0x240) names a socket her body model
  does not have (sockets: `glowa`, `papieros`, `dymek`), so retail draws **no 3D cigarette** for her (the remake logs
  "csatlakozópont hiányzik" and draws nothing, like retail). The other cigarette users carry `papieros.ltb` as their head asset.
* Smoke, `0x10044f00` (called for every actor of the normal update loop, 0x1004add5): a phase with `dym_papierosa` (phase +0x184), the
  character's `socket_papieros`, and the player within 640 units (0x10043a70 vs `[0x100661a0]`); an actor timer (+0x270) is
  advanced by the frame time and, at >= 0.05 s, reset and one `EffectMgr::Spawn` type 0x0d effect is created at the socket:
  `sprites\papieros.spr` (56 frames, 20 fps, 16x16), lifetime 2.4 s, velocity (0,12,0), scale vector (0.02,0.2,0.02), colour
  (0.1,0.15,0.2), alpha 0.1, no gravity/fade flag. Type 0x0d turns the sprite up-axis along its velocity (a streak, 0x100528de..0x10052ad4).
  Implemented in `npcs.rs::tick`, `fx.rs` got `Particle::color`; `sprites/papieros.spr` is exported by `export_effects.py`.
* Head clips: phases carry `animacja_glowa` (`gada` talking, `mruga` blinking); the head model now plays that clip on the phase clock
  (approximation: retail starts the head clip with the phase, we reuse the body's `elapsed` and looping flag). 25 phases in postacie.txt.

## Audit: `tools/audit_assets.py` (`python -m tools.audit_assets ../GYARI output`)
Reads every reference and checks it against GYARI (case-insensitively; bare names such as `prolog.pcx` or `szum.wav` are searched by name)
and against the exporters' output conventions (`ltb -> models/<p>.json`, `dtx -> model_textures/<p>.png`, `misc/**.pcx -> ui/<dir>/..png`,
`misc/panel/hud/*.dtx -> hud/`, `wav/mp3 -> audio/<p>`, `spr ->` every frame texture in `model_textures/` or `decor/`).
Sources: all `scripts/**/*.txt` (8 891 paths, `key value` lines keep spaces and `&`), the path strings of cshell.dll / object.lto /
Lithtech.exe / server.dll, every string property of the `o_*`/`d_*`/`b_*` objects in `output/*.scene.json`, the 28 world
`missing_textures` lists, plus (separately) 216 character definitions of the `gameplay.json` files (body / skins / weapon / head models,
`glowa_specjalna` => `head_asset` + `socket_glowa` present on the body, `socket_papieros` present) and 6 343 output-relative paths inside
the exported JSON (`gameplay`, `props`, `items`, `retail_*`, ...) that must be files. It writes `output/assets-audit.json` and exits 1 on
an unexplained (a) or (b?) reference.
Tests that keep this from coming back: `tests/test_assets_audit.py` (`python -m unittest tests.test_assets_audit`, 42 s) and the ignored
level-viewer tests `npcs::tests::every_character_of_every_level_gets_its_body_skins_head_and_sockets` (loads every model, asserts
skins, head model + skin + `glowa` socket, smoke/weapon sockets for all 216 definitions), `the_prologue_woman_has_her_head_hair_skin_and_cigarette_smoke`
and `the_asset_audit_reports_no_unexported_or_unexplained_missing_asset`.

Classes: **a** exporter bug (retail has it, output lacks it), **b** missing in retail too, **c** unused / superseded.

### Result of the run (8 936 references incl. the world texture lists)
| class | model | skin | texture | sprite | sound | picture | render style |
|---|---|---|---|---|---|---|---|
| ok | 4 694 | 630 | 83 | 1 019 | 1 977 | 169 | - |
| a (fixed) | 7 (`miecho01-03`, `k1-k4`) + 1 head (`stara_dziwka_glowa`) | 1 (`kx`) + 1 head skin | 5 (`latarka`, `snieg`, `system/celownik`, `cien`, `kursor`) | 11 | - | - | - |
| b | 4 | 2 | 47 | 30 | 6 | - | 1 |
| c | 6 | - | 2 | 21 | 5 | 119 | 121 |
| a (open) | 0 | 0 | 0 | 0 | 0 | 0 | 0 |

**(a) exporter bugs, fixed by `export_effects.py` (additive)**
| Reference | Where it is named | Note |
|---|---|---|
| `stara_dziwka_glowa.ltb` + skin (and every other `head_asset` of the 28 levels) | postacie.txt | stale gameplay export, see above |
| `models/levelowe/kawalki/miecho01-03.ltb` (skin `udko_fin.dtx`) | cshell 0x1004f41e (type 0x24) | drumstick chunks |
| `models/misc/k1-k4.ltb` (skin `kx.dtx`) | cshell 0x1004f5c0 (type 0x0b), 0x10053ea9 (grenade blast) | chunks |
| `sprites/latarkawrogow.spr`, `textures/sprajty/latarka.dtx` | cshell 0x1001d78a / 0x1001d919 | enemy flashlight |
| `sprites/snieg.spr`, `textures/sprajty/snieg.dtx` | cshell 0x10030954 / 0x10030a85 | snow |
| `sprites/weapons/systemdymduzy.spr`, `systemwybmortyr.spr`, `systemwybred.spr` | cshell 0x10053fda.., object.lto 0x10010c17 | grenade blast layers |
| `sprites/krew.spr`, `sprites/sig/main.spr`, `weapons/ingram.spr`, `ingramwrogow.spr` | object.lto 0x10010b58..0x10010dd4 (server precache list) | |
| `sprites/blikred.spr` | postacie.txt `i_limuzyna` (`blik2/3`) | intro limousine tail lights |
| `sprites/papieros.spr` | cshell 0x10045036 | cigarette smoke |
| `textures/cien.dtx`, `textures/sprajty/system/celownik.dtx`, `misc/menu/kursor.dtx` | object.lto 0x1000a044, cshell 0x10010b7a / 0x10024002 | blob shadow, sniper reticle, menu pointer |

**(b) missing in retail too** (all in `KNOWN_MISSING_IN_RETAIL`, the game shows/plays nothing for them in retail either)
| Reference | Named by | Reason |
|---|---|---|
| `sounds/enemies/macaroni/halt0.wav` | postacie.txt (6 characters) | folder typo: retail has halt0.wav for china/civilian/italiano/police/russian only |
| `sounds/weapons/laser_on.wav`, `laser_off.wav` | items.txt 532/533 | not shipped |
| `rs/metal_z_maska.dtx` | items.txt 596 | typo, the render style is `.ltb` |
| `misc/panel/hud/ramka_pick_healingkit.dtx` | items.txt 1085 | typo, shipped frame is `ramka_pick_helingkit` |
| `sprites/para.spr` | objects.txt 664 | not shipped |
| `sprites/mortyr.spr` | d_sprite of rh10-wiezowiec3, wiez_wn1 | not shipped (docs/retail-decorations.md) |
| `sprites/flare1/centrum, przed1-5, za1-3.spr` | lens flare of 6 levels / object.lto | not shipped, those parts draw nothing |
| `models/default.ltb`, `renderstyles/default.ltb`, `skins/default.dtx`, `sprites/default.spr`, `textures/default_texture.dtx` | Lithtech.exe / server.dll | engine fallback names, never shipped |
| 47 world textures | world DATs: chapel_mniejszy (`Textures\GFX\*` 33 + `ceg?y jasne`), knajpa + nic (`textures\sprzety\drzwi.dtx`, exists as `okna_drzwi\drzwi.dtx`), podziemia1c / rh1-wiezienie1 / rh3-miasteczko2 (`ceg?y jasne.dtx`, the shipped file is `ceg_jasna.dtx`), rh1-wiezienie1 (`textures!!!\...` 2), rh3-miasteczko1/2 (`ogolne\asfalt_mokry.dtx`), wiez_wn1 (`lastryko_srodek_braz`, `marmur_braz_sredni`) | the engine finds no texture either: lightmap-only surfaces (docs/retail-visual.md) |
| d_odglos sounds `szum.wav` (rh2-wiezienie2), `wnt1-3.wav` (rh1-wiezienie3) | scene properties | no such file anywhere in the install |

**(c) unused / superseded**: 86 `..640.pcx` / `..800.pcx` resolution variants (only the 1024 art is remade), 31 cshell `misc\menu\x.pcx` /
`misc\panel\x.pcx` strings whose shipped file is in the localised `misc\menu_l` / `panel_l` folder (also referenced and exported there),
121 render-style references (`rs\*.ltb`: additive, przez_z_maska, szklotex, okulary, cien, specular, ... are engine blend-state files, chosen by
name in `npcs.rs` / `props.rs`, never loaded), 5 commented-out sounds, `katscena.dat` (DAT v70 prototype) models/sprites, `d_emiter_iskier`'s
`blik.dtx` (inert in retail). Retail files that nothing references and that stay unexported: `cywil5/6/8`, `wiezien.ltb`, `bohater_dead*`,
`glock_outro/ingram_outro/strzykawa/laska_outro_*`, `karalec` (misc), `models/bronie_post/*_low*`, `models/pikapy/*_low`, `flaszka_01-06`,
`lampa1-4_rozbita`, `obrazek_pion/poziom`, `wajcha3`, `sufit_wierzowiec`, `skins/postacie/china_*_glowa_d/g1/g2/s`, `wlosy.dtx`, 15 outro `640/800`
pictures, 1 009 bitmap font pcx (`misc/fonts` other sizes), ClassIcons, 11 unused `sprites/*_bak.spr` / `neon_bak`. (No script, DLL string or level names them.)

## Open items found on the way (round 3: resolved)
* Grenade blast layers and hit-blood `miecho` chunks: ported, docs/retail-blast.md.
* `i_limuzyna` `socket_blik0..3` lights: ported (opening.rs `Light`).
* Enemy flashlight (`latarkawrogow.spr`, `latarka.dtx`) and M-14 laser: probably dead in retail (`[w+0x104]` never set); snow `snieg`: dead (`Snieg 0`). docs/retail-lighting-research.md.
* Render styles are matched by the substring `maska` only (`npcs.rs::material_handle`); additive / glass / specular styles are approximated.
* The smoke's blend mode is taken as additive (retail passes the effect flags 0x1|0x8|0x200 to CreateObject); colour (0.1,0.15,0.2) x alpha 0.1
  makes one streak faint, the column comes from 48 overlapping streaks.
