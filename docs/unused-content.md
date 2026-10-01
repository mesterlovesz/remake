# Retail install vs remake: what is unused, what is missing (inventory 2026-09-30)

Generated and checked with `python -m tools.inventory_unused ../GYARI output` (writes `output/unused-inventory.json`, ~1 min, read-only on GYARI; `--list` prints every unused / missing file).
Unit tests: `python -m unittest tests.test_inventory_unused` (helpers, fast) and `INVENTORY_FULL=1 python -m unittest tests.test_inventory_unused` (whole run). Companion documents written in the same round:
`docs/backlog.md` (107 approximations / open items of the remake), `docs/retail-binary-features.md` (cshell.dll / object.lto feature audit, console variables, dead code),
`docs/retail-ancillary-files.md` (every non-asset file of the install), `docs/retail-gameai-semantics.md` (gameai parser / executor semantics and the action-order finding), older: `docs/cut-content.md` (Kiskína page), `docs/retail-assets.md` (asset audit, `tools/audit_assets.py`).

## Összefoglaló (magyarul)

- A gyári telepítés **5743 fájl / 1147 MB**. A statikus leltár szerint **4226 fájl (74 %) van élő gyári hivatkozás alatt és exportálva a remake-ben**, **216 motor-belső** (DLL, editor-maradék, render-stílus forrás), **12 olyan, amit a gyári játék hivatkozik, de a remake nem exportál** (mind apróság: látható felület nélküli DAT-textúra-bejegyzések, egy `drapacz.spr`), és **1289 fájl (200 MB) a gyári játékban sem használt**: kivágott tartalom, nem lerakott definíciók, régi másolatok.
- A kivágott tartalom nagy része jól körülhatárolható: **Kiskína** (`chinatown.dat`, 17 NPC, 45 beszédfájl, 28 textúra), két további DAT (`nic` teszt-pálya, `katscena` v70 prototípus; `outro.dat` a befejezés világa, de a kampány nem tölti be), **11 ki nem használt zeneszám** (19,8 perc: `*_akcja` / `*_spokoj` párok, `czapel`, `lab`, `wiezowiec`, `Capper`...) és 9 soha nem beállított / vizsgált `Muza*Wlaczona` változó = a **félbehagyott dinamikus (nyugodt/akció) zene**, **337 beszédfájl / 14,8 perc** (Mason 119, `enemies` 80, burmistrz 22, wiezienie1 20, Stella 18 ...) amelyre semmilyen szövegkulcs nem mutat, egy valószínűleg tervezett **Marloni-bár átvezető** (`bar1.ltb`, `marloni.ltb`, `sounds/speech/marloni`), két patkány-átvezető (`szczur na strychu/w piwnicy`, `sepia`), kivágott fegyverek (**Remote bomb** üres definícióval, proximity bomb, obrzyn = lefűrészelt puska, `*_low` LOD modellek), 44 soha le nem rakott `o_obiekt` definíció (`*_bucha` = összetört bútor/autó változatok, kapcsolók, zegar...), hősi holttest-modellek (`bohater_dead*.ltb`), 4 nem használt `b_winda*` liftosztály.
- **Új, ellenőrzött szerkezeti eltérés:** a gyári játék a szint `action`-jeit **fordított sorrendben** futtatja (a script utolsó actionje először: `0x10015be0` a lista elejére szúr, `0x1001a898` az elejéről jár), a remake `Mission::tick` a script sorrendjében; 13 szintszakaszban flag-kaszkád 1 frame késéssel, és egyidejű igaz feltételnél a script-beli korábbi action nyer (pl. prológus `HostilePrologAtakuja`). Egysoros javítás: `actions.iter().rev()` (részletek: 7a. szakasz, `docs/retail-gameai-semantics.md`). Az `endifs`, az ismeretlen `if` változó és az üres `ifactionhostile` kezelése egyezik.
- **A fő hiányosságok (a gyári játék csinálja, a remake nem):** NPC-fegyverejtés minden felfegyverzett halottnál (a remake egy gyárilag nem létező `drop_weapon` kulcshoz köti), `obrot_postrzalu` (75 fázis), `aware` jelző, globális/egyedi `hostileattack`, rejtett konzolváltozók (`God`, `FullStamina`, `Invisible`, `DrawKilledCount` az `autoexec.cfg`-ből), prológus útháló kulcsai (`nie_sprawdzaj_drzwi`, `gestosc_sciezek`), limuzin-lámpák, `miecho` húsdarabok, NPC lézer / zseblámpa, modell-árnyék, fej-követés. Rangsorolt lista: lásd "Ranked overlooked features" lent.
- **Gyári szkript-hibák**, amelyeket a remake-nek **nem szabad "javítania"**: 6 akció, amely soha nem fut le (`Cywil1/2/3Nuda`, `HostileUBurmistrzaOstrzegli` sosem áll be, `BurmistrzSieBoi` nincs deklarálva), 4 nem létező fázisra mutató `on_*` / `default_faza`, 1 nem létező szereplőre mutató `setfaza`; a szkript `drop_weapon`, `nie_wspinaj_sie`, `ignoruj_markery_niechodzenia`, `anim_glowa`, `alt_sound_on/off` kulcsait a gyári elemző nem ismeri.
- Fontos melléklet: a gyári `cipher.exe` 2026-09-28-án lefutott a `GYARI` mappán (122 szkript betűpár-cseréje, fájlidők 06:38:35), tehát a `GYARI/scripts` már nem az eredeti telepítési állapot; a `read_script` mindkét alakot olvassa.

## 1. Method and status definitions

Every file of GYARI (5743, `rglob`) gets: category, size, the set of things that refer to it (first three shown in the JSON `referenced_by`), a status and a note.

| status | meaning |
|---|---|
| **USED-IN-REMAKE** | a live retail reference exists (script block that some level places / names, world DAT string, hard-coded string of cshell.dll / object.lto / server.dll / Lithtech.exe, sprite frame of a live sprite, `sprintf`-pattern or resolution prefix) **and** the remake's `output/` has the exported counterpart (`audit_assets.exported`, world material list, `decor/`, `env_textures/`, `ui/`, `hud/`). "Exported" is not "proved rendered": what the runtime does with it is covered by the tests of the respective area |
| **USED-BY-RETAIL-BUT-MISSING-IN-REMAKE** | live reference but no exported counterpart (12 files, all trivial, see section 4) |
| **UNUSED-IN-RETAIL** | nothing live refers to it: commented out, defined but never placed (characters, items, objects, scenes), only in worlds off the campaign route, locale-0 twin of a `_l` file, stray editor file, orphan |
| **ENGINE-INTERNAL** | executables / DLLs, `DirType*` markers, `ClassIcons` (editor), `TextureEffectGroups`, `LightAttenuationPresets`, render-style sources `rs/*.lta|.vsh|.ash` |

Reference sources: all scripts (block-aware: `postac` / `item` / `object` / `scena` blocks are live only when their definition is placed by a level or named by a running scene), the path-like strings of the 31 world DATs, cshell.dll / object.lto / server.dll / Lithtech.exe strings (incl. `%d` patterns and name prefixes such as `save_` + `1024.pcx`), `ile_skinow N` skin variants, `sound_on_kontakt <dir>\halt0.wav` + `sounds_on_kontakt N` (+ `dead0..7.wav` of the same folder), bare names searched by file name, `misc/<dir>_l` localised twins (shipped `autoexec.cfg` has `locale 1`, so the `_l` copy wins and the non-`_l` twin is only the locale-0 fallback). Limits: static string matching, no data-flow (a path built from pieces in code that has no string is invisible), liveness of hard-coded strings is assumed (dead code is listed separately by `docs/retail-binary-features.md`: the DLLs have no orphan functions).

## 2. Totals per category

| category | files | USED-IN-REMAKE | USED-BY-RETAIL-BUT-MISSING | UNUSED-IN-RETAIL | ENGINE-INTERNAL | MB total | MB unused |
|---|---:|---:|---:|---:|---:|---:|---:|
| world (.dat/.pth) | 63 | 54 | 0 | 8 | 1 | 158 | 6.4 |
| model (.ltb) | 479 | 375 | 0 | 96 | 8 | 53 | 7.9 |
| skin (skins/*.dtx) | 485 | 420 | 0 | 57 | 8 | 286 | 36.7 |
| texture (textures/, misc/ dtx) | 1605 | 1185 | 11 | 346 | 63 | 360 | 62.4 |
| sprite (.spr) | 125 | 91 | 1 | 25 | 8 | 0 | 0 |
| render style (rs/) | 51 | 9 | 0 | 5 | 37 | 0 | 0 |
| picture (misc/*.pcx, UI art) | 1625 | 1458 | 0 | 151 | 16 | 58 | 12.4 |
| sound effects (sounds/, ambient, enemies, lewelowe, obiekty ...) | 329 | 173 | 0 | 143 | 13 | 38 | 19.1 |
| music (sounds/muza) | 28 | 17 | 0 | 11 | 0 | 25 | 8.3 |
| speech (sounds/speech) | 695 | 356 | 0 | 337 | 2 | 133 | 45.2 |
| cutscene audio (sounds/scenes) | 63 | 56 | 0 | 7 | 0 | 17 | 1.5 |
| script (scripts/) | 133 | 32 | 0 | 101 | 0 | 1 | 0 |
| save | 2 | 0 | 0 | 2 | 0 | 0.3 | 0.3 |
| engine / root (exe, dll, cfg, avi, editor dirs) | 60 | 0 | 0 | 0 | 60 | 19 | 0 |
| **total** | **5743** | **4226** | **12** | **1289** | **216** | **1147** | **200** |

Byte-identical duplicates: 191 groups, 246 redundant files, 45 MB (JSON `duplicate_groups`; e.g. `skins/postacie/policjant{,0,1,2}.dtx`, `sounds/muza/Fear_the_Sniper_s.wav` == `menu.wav`, `chinatown_akcja.wav` and `wiezowiec_wn.wav` have the same size and duration but different bytes, 14 `empty.wav` placeholders of 88602 bytes: `sounds/ambient/empty.wav`, `sounds/speech/empty.wav` and 12 `tormiens` / other speech files that are silent stand-ins).

## 3. (a) Worlds and levels (31 DAT + 31 PTH)

27 worlds are on the linked campaign route (26 playable + the prologue `rh3-miasteczko0`; `output/campaign-inventory.json` from the `Skok_do_levelu` / `startlevel` / `runworld` graph). All 31 are exported (`output/<world>.scene.json`); `katscena` only objects (DAT v70).

| world | DAT MB | status | what it is | evidence |
|---|---:|---|---|---|
| `chinatown` (Kiskína) | 3.9 | UNUSED-IN-RETAIL (cut content) | full authored level: 17 NPC (6 Chinese soldiers, 4 civilians, 7 others), 18 pickups, 310 objects, 24 AI actions, 30 dialogue blocks, 45 speech files, 2 music tracks, loading picture `chinatown.pcx`; no inbound transition (podziemia1c `hujjj` goes to `chinatown2`); the remake loads it from the menu | `docs/cut-content.md` |
| `nic` | 1.4 | UNUSED-IN-RETAIL | small test level (13 NPC, 14 pickups, 41 props, 7 doors); `runworld` is a retail console variable with default `worlds\nic` (cshell 0x10059230: the engine's start world, never the game) | `docs/retail-ancillary-files.md` |
| `katscena` | 0.5 | UNUSED-IN-RETAIL | DAT **v70** prototype (2 NPC, 20 sprites, 46 objects, 7 lights, cutscene models `bohater_intro` / `policjant_intro`); geometry not decoded; no `.pth` content | `docs/retail-visual.md` |
| `outro` | 0.3 | UNUSED-IN-RETAIL (as a world) | 21 objects (4 props, 7 sprites, 1 `o_cutscene outro`); the ending plays inside `rh12-lab2` through scenki `outro`, `c_outromgr` shows slides/credits (`endgame.rs`); nothing loads `outro.dat` | `docs/retail-scenes.md` |
| `test2`, `pudlo`, `test`, `boks` | - | script only | `level` blocks in gameai.txt (`pure_shooter`, `load_d Level testowy`), no DAT shipped | gameai lines 45-110 |

Counts of the 27 route worlds (characters / emitters / pickups / objects) are in the JSON `worlds`. Two more facts: `rh3-miasteczko0` carries the level-header keys `gestosc_sciezek 4` and `nie_sprawdzaj_drzwi` that the remake skips (overlooked-feature list, #12).

## 4. (b) Assets

Notable unused groups (full lists: `python -m tools.inventory_unused --list`, JSON `files` / `unused_groups`).

### Models, skins, textures, sprites

| group | files | what | evidence / note |
|---|---:|---|---|
| `models/Levelowe` | 59 | props with `object` definitions in objects.txt that no level places: `*_bucha` (= "broken" variants of furniture, cars, lamps: `lamborghini`, `lotus`, `trabant`, `limuzyna`, `sedes`, `zlew`, `krzeslo`, `kanapa`, `papyrus`...), `przelacznik*`, `przycisk*`, `wajcha*` (switches / lever), `zegar`, `taczki`, `drzewo1`, `para` (steam), 6 bottle chunks `flaszka_01-06`, 4 `lampaN_rozbita` | 44 of the 229 `object` definitions are never placed (list in JSON `definitions.objects`); `flaszka*` / `lampa*_rozbita` are not referenced by cshell's chunk list either |
| `models/bronie`, `models/bronie_post`, `models/pikapy` | 17 | cut / superseded weapon models: `proximity_bomb`, `mossberg`, `fnp90_stary`, `*_low`, `*_low3.1` LOD meshes (glock, M14, mossberg, sig, S&W), `obrzyn_low` (sawn-off), `gloc_low`; skins `bronie/remote_bomb`, `proximity_bomb`, `pikapy/obrzyn_low`, `mossberg` | items.txt has `item Remote bomb` with an **empty body** (line 911), HUD art `ramka_ammo_remote` / `ramka_pick_remote`, `misc/Items/remote_bomb.pcx`, `obrzyn.pcx`, `palka.pcx`: the remote bomb, proximity bomb and sawn-off shotgun were cut. The shotgun that shipped is `FN shotgun` |
| `models/cutscenes` | 5 | `bar1`, `marloni`, `korytarz`, `strych`, `limuzyna` | `strych` / `korytarz` belong to the two rat scenes (scenki `szczur na strychu`, `szczur w piwnicy`, no `o_cutscene` places them); **`bar1` + `marloni` + `sounds/speech/marloni` (9 files) + `barman` (6) = an unfinished bar cutscene** |
| `models/postacie` | 14 | `bohater_dead`, `bohater_dead_w` (hero corpse), `cywil2/5/6/8`, `wiezien`, `cutsceny/bohater_intro`, `policjant_intro` (katscena only), `glock_outro`, `ingram_outro`, `laska_outro_*`, `strzykawa` | `cywil2` is placed only in `chinatown`; the hero corpse models are not used by cshell strings; the `*_outro` props of the ending are not placed by the scenki `outro` |
| `skins/postacie` | 20 | `china_oficer_glowa_{d,g1,g2,s}`, `china_zolnierz_glowa_{d,g1,g2,s}` (alternative head skins: d = dead?, g = gory?, s = ?), `cywil2/5/6/8` (+ `_glowa`), `wlosy`, 3 cutscene car skins | numbered variant skins (`policjant0-3`, `wloski_oficer_glowa0-5` ...) are live through `ile_skinow` and are not counted here |
| `textures/*` | 346 | per-level leftovers: `chapel` 24, `chinatown` 28 (`food2-3`, `hotel_1-3`, `film*`, `neon/restroom_*`), `sprajty` 212 (muzzle / blast / lens sprites: `blik_wybuch*`, `blik_rej`...), `sprzety` 34, `podlogi_schody` 10, `okna_drzwi` 6, ... | not named by any world DAT, sprite or script. `textures/chinatown/DirTypeTextures` is a 174 KB DTX (old copy of `wall_street_bar.dtx`), not a marker |
| `sprites` | 25 | `blikblue`, `bliklampa2`, `blikwoda`, `flare1/blik6*`, `zachod`, `neon_bak/food2,food3,hotel,restroom`, `weapons/dymshotguna`, `systemfala` | `sprites/mortyr.spr` (3 `d_sprite` of wiezowiec3 / wiez_wn1) and 9 `flare1/*` parts are **missing in retail too** (`audit_assets.KNOWN_MISSING_IN_RETAIL`) |
| render styles | 5 + 37 engine | `rs/*.ltb` actually named: `szklotex`, `okulary`, `specular`, `cien`, `przez_z_maska*`, `metal_z_maska`, `automat_cola`: the remake picks a blend state by name (`npcs.rs`, `props.rs`), the `.ltb` is not parsed (USED-IN-REMAKE, approximated) | `rs/*.lta`, `.vsh`, `.ash` are renderer sources (ENGINE-INTERNAL) |
| `USED-BY-RETAIL-BUT-MISSING` | 12 (1 sprite + 11 textures) | `sprites/drapacz.spr` + 4 frame textures (`drapacz_a/b/d/e`: only a raw string of `wiez_wn3.dat`, not a placed `d_sprite`), `textures/detail/envmap012`, `shst0001`, `ogolne/Invisible` (intentionally invisible), `niebo_niebieskie`, `tormies/szachownica`, `sprajty/blik_lampa2/3` (frames of `blikpionowy1.spr` / `blipionowy2.spr`, which only `katscena` uses) | texture-name entries of a DAT table whose polygons the world export does not render, or katscena-only art; worth one look, no known visual gap |

### Pictures, fonts, HUD

* 151 unused pictures: locale-0 twins (`misc/panel/HUD/*`, 40 of which differ from the `panel_l` copy; shipped `autoexec.cfg` has `locale 1`), `misc/outro/*_{640,800,1024}` of `demo_outro`, `credits`, `credits_l` (the game's `c_outromgr` uses `outro1..4` + the credit text), `misc/Items/{fnp90,fnp90_ammo,granat,hkg8,hkg8_ammo,ingram,ingram_ammo,obrzyn,palka,remote_bomb,shellz}.pcx` (icons without the resolution suffix; items.txt uses `icon640/800/1024`), `panel/additional/{painkiller,powerup}.dtx`, `ammo/lithtechwatermark256.dtx`, `butsila/butstam/butzdrow/pasek/podsila/podstam/podzdrow/suwak.pcx`, stray `fonts/sub/*/Untitled-1.pcx` and `uszy.pcx`.
* Resolution variants (640 / 800 / 1024): retail picks by `screenwidth`; the remake uses the 1024 art only (owner-approved modernisation).
* Font glyphs: cshell.dll loads the `info` and `cyfry` fonts glyph by glyph (digits, `D<letter>`, `m<letter>`, 19 special names `apostrof dollar procent nawiasprawy nawiaslewy minus plus gwiazdka slash backslash cudzyslow maupa wykrzyknik pytajnik krzyzyk dwukropek przecinek kropka spacja`). The shipped files are named `nawiasL.pcx` / `nawiasP.pcx` (+ `srednik`, `hash`), so **retail asks for `nawiaslewy/prawy.pcx`, `apostrof`, `dollar`, `procent` that do not exist in those folders**: the glyph is absent in retail; `tools/export_inventory.py` maps `nawiasL` to "(" (JSON `font_glyphs`). UNVERIFIED: how the retail loader treats a missing file (blank / fallback) and whether the remake draws parentheses where retail draws nothing (low impact). The `Un` and `sub` fonts are not loaded by a cshell path.

### Sounds, music, speech

| group | unused | total | minutes | note |
|---|---:|---:|---:|---|
| music `sounds/muza` | 11 | 28 | 19.8 | `burmistrz_spokoj`, `Capper_s`, `CapperB_s`, `czapel`, `Fear_the_Sniper_s` (== `menu.wav`), `knajpa1`, `lab`, `miasteczko2`, `wiezienie1_akcja`, `wiezienie2_akcja`, `wiezowiec`. Together with `*_spokoj` / `*_akcja` pairs that are live (chinatown, wiezienie) and **9 `Muza*Wlaczona` bool variables that are declared but never set or tested** (`MuzaPrologWlaczona`, `MuzaSpokChinioleWlaczona`, `MuzaAtakChinioleWlaczona` ...) this is the **unfinished calm/action music switching**; retail plays one looping track per level (`docs/retail-audio.md`) |
| speech `sounds/speech/mason` | 119 | 120 | 4.9 | no text key names them; `dialogi_mason.txt` is empty; only the `Fight01-`keys (`speech/fight`) exist |
| speech `enemies` | 80 | 80 | 1.9 | voice files with no text key at all (barks / shouts, no script refers to them) |
| speech `burmistrz` 22, `wiezienie1` 20, `stella` 18, `tormiens` 12, `hostile` 10, `marloni` 9, `wiezowiec` 8, `cutscenes-hero` 7, `misc` 7, `barman` 6, `cutscene-other` 5, `chinatown` 7 of 45, `monastery` 2, `tavern` 2, `hero` 2 | 147 | | 8 | cut dialogue lines (`>Tavrn12S`, `>China07S` keys exist but nothing references the keys) |
| ambient / level sounds | 143 | 329 | 7.1 | duplicates under `ambient/`, `lewelowe/drzwi`, `obiekty`: `BAR_DOR`, `MET_DOR*`, `SAMO_DR*`, `DRZWI_C1` (note the world DATs name `sounds\ambient\Drzwi_cl.wav`, which does not exist: digit 1 vs letter l), `GAWRON*`, `kura*`, `pies`, `techno/*`, `deszcz/0-4`, `hero/KROKL1-2`, `pickup/NOAMMO PRZCISK TAK`, `weapons/ing_s1 sig_s1` |
| cutscene audio `sounds/scenes` | 7 | 63 | 0.3 | `intro/10a`, `random_cutscene/*` (Cut1_Cop1, Endcut_swat, STELLAendcut1-3, Person060_text026): the "random cutscene" = unfinished ending variants |

Numbers of played sounds and their retail parameters: `docs/retail-audio.md`. The `sounds/enemies/<type>/haltN.wav` / `deadN.wav` files are live through the `sound_on_kontakt` sequence rule (cshell 0x10049a9d, death scream `dead<rand % 8>.wav`).

### Videos and other binaries' media

`1.avi` (galaxy, 14.2 s), `2.avi` (Mirage logo, 4 s), `3.avi` (tunnel, 17.8 s): played by `play1.exe`; the remake plays them from `output/videos` (`video.rs`; its comments call 1.avi "Cenega" and 3.avi "Lithtech", the pictures do not match: see `docs/retail-ancillary-files.md`). `sniper.ico` (64x64) and the window title "A mesterlövész v 2.33" (`scripts/app_name.txt`) are not used by the remake.

## 5. (c) Scripts

### Vocabulary: who knows which key

JSON `keywords` (422 key/context rows): for every key used in the shipped scripts the number of uses, which retail binary contains the keyword (prefix match noted) and whether the remake's Rust / exporter sources contain it.

**Parsed by retail, used in the scripts, not implemented by the remake** (overlooked-feature candidates):

| key | uses | retail (cshell.dll) | remake | note |
|---|---:|---|---|---|
| `obrot_postrzalu0/1` | 75 phases / 58 characters | parse 0x100400a3, use 0x100489f0 | missing | torso twist when shot / dying (B014) |
| `glowa_do_gracza_dist/kat` | 2 | 0x10018682 (+0xfcc/0xfc8) | missing | head tracking |
| `skala_blik0-3`, `blik0-3`, `socket_blik0-3` | 1 character (`i_limuzyna`) | 0x1003eab0 | missing | intro limousine tail lights (B006) |
| `obracaj_ganem_w_pionie` | 1 | parsed | missing | weapon held vertically |
| `malutki` | 1 | parsed | missing | "tiny" flag |
| `podmieniany_skin0` / `podmieniany_rs0` | 2 objects | parsed | not read (see `docs/retail-props.md` for `death_podmien`) | |
| scenki `sepia`, `anim_glowa_postac0` | 2 / 1 | 0x1001368d | missing | both only in the two rat scenes that no level triggers: unreachable |
| level header `gestosc_sciezek 4`, `nie_sprawdzaj_drzwi` | rh3-miasteczko0 | 0x1003c650 / 0x1003ceb0, 0x1003ca80 | skipped by `mission-runtime` | path-node density and "no door flags" for the prologue |
| `pure_shooter` | 1 (test2) | parsed, disables the mission tick | skipped | unshipped level |
| `nie_wspinaj_sie` | 3 | only the prefix `nie_wspinaj` exists in cshell | not read | probably matches by prefix in retail (strncmp): UNVERIFIED |

**Used in the scripts but unknown to the retail parser** (dead lines: the remake must not give them meaning unless retail does): `drop_weapon` (55 lines; **the remake implements it and uses it as the only trigger of NPC weapon pickups**, retail `Kill` 0x10042ca0 drops every armed NPC's weapons unless `nie_zostawiaj_gana`), `ignoruj_markery_niechodzenia` (2), `anim_glowa` (1), `alt_sound_on/off` (the cshell keys are `sound_alt_on/off`; the named laser sounds do not exist either), `endifs` (197 lines: an unknown word, ignored by the parser; every action is still one AND-chain of conditions followed by its effects, so the remake's `endifs` handling is equivalent: `docs/retail-gameai-semantics.md`).

**Known to the retail parser, never used by a shipped script:** gameai `print`, `wlaczmuze` (stores a value nothing reads), `ifgraczdalejniz`, `iflicznikmniejszyniz`, `include` (all 35 `//include` lines are commented out; `scripts/ai/*.txt` are 0-byte stubs), `outro`; scenki `black`, `koniec`; `mission-runtime` returns an error for these, which is irrelevant for the shipped data.

### Definitions never placed / never referenced

| what | count | list |
|---|---:|---|
| characters (`postac`) never placed by a route level | 3 of 92 | `szczur_c`, `hero` (only the unused rat scenes), `cywil2` (only chinatown); the 20 cutscene actors (`i_limuzyna`, `bohater_outro`, `laska_outro`, `assault*_outro` ...) are live through running scenes |
| items (`item`) | 1 of 39 | `Remote bomb` (empty definition) |
| objects (`object`) | 44 of 229 | see "Models": `*_bucha`, `przelacznik*`, `przycisk*`, `wajcha`, `zegar`, `drzewo1`, `para`, `kwiat_czerwony_bucha`, ... |
| scenes (`scena`) | 2 of 6 | `szczur na strychu`, `szczur w piwnicy` |
| dialogue nodes | 4 of 229 | `BarmanKrzyczy`, `MasonStartGame`, `Burmistrz22`, `Miasteczko1` |
| text keys | 15 of 1106 | `>Tavrn12T/S` (bartender "Ahogy akarod, haver"), `>China07S`, `>Outro34` (slide 3 repeats `Outro32`, a retail bug already mirrored), launcher / installer strings `>launch1-7`, `>Gametitle`, `>Loading`, `>IStext1-2` (read by launcher.exe) |
| mission variables never set and never tested | 13 | all `Muza*Wlaczona` (9), `PolicjanciWiezienie1Atakuja`, `PierwszyDialogSaid`, `LicznikLaski`, `BurmistrzDialog3` |
| object classes registered by object.lto, placed by no level | 7 | `b_winda`, `b_winda_drzwi1/2`, `b_winda_przycisk`, `d_cien_chmur`, `o_detektor_konca_levelu`, `o_marker_rozprysk` (`docs/retail-objects.md`, `SkyPointer`, `AmbientLight` are engine classes) |

### Script defects of the retail game (JSON `script_defects`; retail has these bugs, keep them)

| where | defect |
|---|---|
| gameai lines 306 (`miasteczko0` action `cywil1`), 2439 / 2452 / 2465 / 2477 (`chinatown` `cywil1-4`) | `if Cywil1Nuda` / `Cywil2Nuda` / `Cywil3Nuda` / `Cywil4Nuda` are declared but never set: these actions can never fire |
| line 2068 (`burmistrz1` `JakZobaczyWrogBezGana1`) | `if HostileUBurmistrzaOstrzegli` never set |
| line 2157 (`burmistrz1` `CzyBurmistrzSieBoi`) | `if BurmistrzSieBoi`: variable never declared with `bool`: the lookup 0x10015ac0 returns 0, the condition fails in retail and in the remake: dead in both (`docs/retail-gameai-semantics.md` section 5) |
| line 877 (`rh1-wiezienie2` `straznik5`) | `setfaza chowa_sie straznik_przy_celi`: the definition is `straznik przy celi` (spaces): lookup 0x100424c0 fails, no-op in retail, same in the remake |
| postacie lines 1512, 1580, 1620, 2482 | `on_death pada` of `wiezien egzekucja` / the two escape buses, `on_koniec_widzi2 stoi` of `cywil1p` point to phases that do not exist (retail prints "Nie znaleziono fazy ") |

### Object classes and properties

JSON `classes` (37 placed kinds + the registered ones) and `object_properties` (459 class/property rows with instances, levels, distinct values, `in_object_lto`, `read_by_remake`). Facts: every property the 31 levels set is registered in object.lto except `WorldProperties.Mgla_Liniowa`, `Light/DirLight.OuterColor` and `.Time` (11 + 3 instances, dead); object.lto reads but never uses `b_door.Nazwa_portalu` (309 doors) and the door tints `Kolor1/2`. The `read_by_remake` flag is a string-literal heuristic: lens-flare parts, spark emitter, rain / snow and `d_odglos` rows are false negatives (handled by `decorations.rs` with built names, or inert in retail: `d_emiter_iskier`, `d_odglos` are silent, `docs/retail-objects.md`).

## 6. (d) cshell.dll / object.lto / Lithtech.exe and the non-asset files

Details, addresses and evidence: `docs/retail-binary-features.md`, `docs/retail-ancillary-files.md`. Summary:

| item | what it is | remake |
|---|---|---|
| console variables polled by cshell every frame (0x1005a640 -> 0x10059230) | `Music`, `Speech`, `Sound`, `locale`, `ModelShadow_proj_enable`, `CInvertMouse`, `CWeaponBob`, `CAutoQuickSave` (implemented) and the hidden developer variables `God` (health := max on every change, 0x10061bb4; `autoexec.cfg` already has `"God" "0"`), `FullStamina` (0x10061f19), `Invisible` (NPCs neither see nor hear the player, 0x10043720 / 0x10043860 / 0x10045df0 / 0x10049950), `DrawKilledCount` (HUD kill number, 0x1003ae60), `DrawPaths`, `DrawPostacPaths` (path debug lines, 0x1003d9f0 / 0x1004ae30), `runworld` (default `worlds\nic`) | **missing** (only preserved in `autoexec.cfg`) |
| key handlers | `OnCommandOn` handles only Menu 70, Quit 250, QuickSave 251, QuickLoad 252; everything else is polled (`docs/retail-input.md` is complete); no cheat keys; `"dupowaty"` is a dummy dialogue argument; dead: slot 20 super jump (0x10060c1b), a free-fly flag (0x100b2475), F/G weapon-cycle stub (0x1005ab50) | implemented, dead parts intentionally absent |
| SFX messages | all 20 ids object.lto sends have a client handler; 0x2d, 0x37, 0x6d are never sent | implemented |
| dead code | no orphan function in either DLL (every function reachable from a call, vtable or the class table) | - |
| `launcher.exe` / `lanuchp.txt` / `play1.exe` | launcher builds `lithtech.exe -rez . -rez engine.rez +screenwidth .. +bitdepth .. +MaxTextureSize ..` into `lanuchp.txt` (the misspelling is in the binaries) and starts `play1.exe` (3 videos) or the game; options: resolution x depth, texture cap 128/256/512, skip movies; no hidden options; CD check stubbed | engine-internal |
| `cipher.exe` | dialog tool that swaps paired letters/digits in 122 hard-coded script files; already run on GYARI on 2026-09-28 | `read_script` reads both forms |
| `classhlp.but`, `ClassIcons/`, `TextureEffectGroups/`, `LightAttenuationPresets/` | editor help, editor icons; `pan.tfg` / `pan_szybszy.tfg` UV-pan groups (used by 5 worlds, animated by the remake); `Default.lta` attenuation preset (coefs 1 0 19, exps 0 0 -2, baked into 2634 DAT lights) | the light falloff formula of the engine is not recovered: lights use an approximation (backlog) |
| `Engine.REZ`, `clientfx.fxd`, `SndDrv.dll`, `dx8.snd`, `cdaudio.dll`, `l3codeca.acm`, `mss32.dll`, `ltmsg.dll`, `CRes.dll`, `sres.dll`, MFC/MSVC debug DLLs | engine resources and runtimes; `clientfx.fxd` is a DLL type library (15 FX types) nothing references | engine-internal |
| `save/quick.sav` (+ `.srv`) | retail layout: SYSTEMTIME, `RatHunt save game.`, 7 floats of 100.0, world name, 320x240 thumbnail, variable records | the remake's JSON saves are not compatible; import of retail saves missing (optional) |
| `autoexec.cfg` | written by the retail engine (console dump), variables explained in `docs/retail-ancillary-files.md` | remake keeps its own copy in `output/` |

## 7. Ranked overlooked features (retail does it, the remake does not)

Ranking: player-visible impact x confidence of the evidence; effort S = under a day, M = several hours to a day or two, L = larger. Owners are the parallel worktrees (wt-ai2 combat / AI / missions, wt-pres effects / presentation, wt-snd2 sound, wt-cut Kiskína / cut content). Nothing in this list was implemented by the inventory worktree (the hot files `npcs.rs`, `campaign.rs`, `activation.rs` belong to the other agents).

| # | behaviour | retail evidence | remake today | impact | effort | owner |
|--:|---|---|---|---|---|---|
| 1 | **NPC weapon drop on death for every armed actor, including the second weapon** (`socket_weapon1`: 5 definitions) | `Kill` cshell 0x10042ca0 (0x10042eef..0x10043000), item create 0x10042f74: throws both weapon objects (+0x208, +0x20c: the carried weapon models, released with a downward velocity of -64 and a 300.0 field) for every dead actor unless the +0xfbe flag (`nie_zostawiaj_gana`) is set, independent of the death phase; the parser does not know `drop_weapon`. (Whether a released model is a pickup is what the remake's pickup system already assumes for the 55 `drop_weapon` phases.) | `campaign.rs:128` drops only when the entered phase carries `drop_weapon`; `straznik przy celi`, `straznik na klatce`, `policjant z pala` and all second weapons never drop | M (missing pickups) | S | wt-ai2 |
| 2 | **gameai actions run in REVERSE script order** (the last action of a level section first): the node allocator 0x10015be0 pushes every node to the front of the level list, the driver 0x1001a898 walks it from the head | cshell 0x10015be0, 0x1001a898, executor 0x1001a140, parser 0x10015cc0 (`endifs` is an unknown word and ignored; every other structure of the 213 actions is equivalent, verified by model over ~1.3 M random runs: `docs/retail-gameai-semantics.md`, models in `tools/gameai_model/`) | `Mission::tick` (`mission-runtime/src/lib.rs` line ~212) iterates in script order and its comment states the opposite | **H** for 13 of 32 level sections: flag cascades arrive one frame later; when two actions become true in the same frame retail lets the script-EARLIER one win a dialogue / latch (prologue `HostilePrologAtakuja`: retail suppresses `atakuja`, remake suppresses `czyatak`, `policaj`, `policaj1`, `dziwka1`; rh2-wiezienie1 `czygadaja0..3`; burmistrz1 / chinatown / chinatown2 / knajpa hostile latches; chapel `PierwszeSpotkanie`) | **S**: `for action in actions.iter().rev()`; effects of one action in retail's fixed kind order (dialog, startlevel, cutscene, hostileattack, tnijitems, set, unset, setfaza, setallfaza, print, zdrowie, outro; 35 actions differ in emission order only). **Applied on agent/ai2** (`Mission::tick` iterates `actions.iter().rev()`, effects in the executor's kind order; tests adapted, `actions_run_in_reverse_script_order` added; mission-runtime `--ignored` incl. the walkthrough solver pass) | wt-ai2 |
| 3 | `obrot_postrzalu0/1` death / hit twist | use 0x100489f0 (0x10048a2f), parse 0x100400a3 | not implemented (75 phases, 58 characters) | M (corpse pose) | M | wt-ai2 (B014) |
| 4 | `aware` flag (+0x128): `ifplayerseenby` marks the actor, blocks `patrol` entry | 0x1001a060, 0x10041cbe | not ported; a talking NPC can still patrol | M | M | wt-ai2 (B011) |
| 5 | `hostileattack` from a dialogue node turns only its speaker; script `hostileattack` global | 0x10019811, 0x1001a52d | approximated (per-speaker event, global flag) | M | M | wt-ai2 (B016) |
| 6 | hidden console variables `God`, `FullStamina`, `Invisible`, `DrawKilledCount` (+ `DrawPaths`, `DrawPostacPaths`) from `autoexec.cfg` | 0x10059230; 0x10061bb4, 0x10061f19, 0x10043720 / 0x10043860 / 0x10045df0 / 0x10049950, 0x1003ae60, 0x1003d9f0, 0x1004ae30 | not read (lines preserved) | L for players who edit `autoexec.cfg` | S (M for path overlays) | any |
| 7 | intro limousine tail lights (`blik0-3`, `skala_blik*`, `blikred.spr`) | parse 0x1003eab0 | `opening.rs` does not draw them | L (intro only) | S | wt-pres (B006) |
| 8 | hit-blood chunks `miecho01-03` and the full grenade blast (5x `systemdymduzy`, `systemwybmortyr`, `k1-k4` chunks, light, debris) | 0x10005b83, 0x10053e90..0x10054383 | exported assets, never spawned / 2 of the layers | M | S / M | wt-pres (B005, B010) |
| 9 | NPC M-14 laser ribbons; enemy flashlight beams (`latarkawrogow.spr`) | 0x1002130a; 0x1001d78a / 0x1001d919 | missing; `enemy lights` option only stores a value | M | M | wt-pres (B007, B013) |
| 10 | projected model shadows (`ModelShadow_proj_enable`, `textures/cien.dtx`) | object.lto 0x1000a044, option 0x1002a9b0 | option stored, nothing drawn | M | M | wt-pres (B015) |
| 11 | head tracking `glowa_do_gracza_dist/kat` | 0x10018682 (+0xfcc/0xfc8) | missing (2 characters) | L | M | wt-ai2 |
| 12 | prologue path graph: `nie_sprawdzaj_drzwi` (no door flags), `gestosc_sciezek 4` (node density 4.0 instead of 2.0) | 0x1003ca80 (level +0x388), 0x1003c650 / 0x1003ceb0 (level +0x384) | both keys skipped | L (prologue NPC paths) | S / M | wt-ai2 (B024) |
| 13 | `static` phases skip the spawn ground snap (88 uses) | 0x10043520 / 0x100432e0 | approximated | L | S | wt-ai2 (B037) |
| 14 | per-phase `show_weapon` attach gate instead of a sticky flag | 0x10049e80 (0x10049f38, 0x1004a20b), 0x10041ed1 | UNVERIFIED difference | L | S | wt-ai2 |
| 15 | display gamma ramp | Lithtech.exe 0x4f6ed0, cshell 0x1005a21d | shader pass relative to 1.15, not the ramp | M (whole picture) | needs owner decision | owner (B001) |
| 16 | light falloff: `Default.lta` AttCoefs (1 0 19) / AttExps (0 0 -2) + enum Linear / Quartic / D3D for 2634 lights | `LightAttenuationPresets/Default.lta`, per-light DAT properties | ignored; approximate falloff | M (lamp halo look) | M (formula not recovered) | wt-pres |
| 17 | retail save format (`save/quick.sav`: 16-byte SYSTEMTIME, text header, floats, 320x240 thumbnail) | `savefile.rs` is JSON | not compatible, no import | L | M, optional | wt-ux |
| 18 | window title "A mesterlövész v 2.33" and `sniper.ico` | `scripts/app_name.txt`, `sniper.ico` | title `EDITION / world`, no icon | L | S | any |
| 19 | `locale 0` UI set (`misc/*` non-`_l` twins: 40 HUD frames differ) and the `locale` variable | cshell 0x100591b0 | Hungarian `_l` art only | L | S | wt-ux |
| 20 | single-character quirks: `obracaj_ganem_w_pionie`, `malutki`, `skala_blik*`, `nie_wspinaj_sie` (prefix), `podmieniany_rs0` | parsed in cshell.dll | not read | L | S each | wt-ai2 |

### 7a. Ready patch for #2 (not committed: the auto-mode permission layer refused the last edit round, so the worktree keeps the old behaviour)

* `crates/mission-runtime/src/lib.rs`: in `Mission::tick` replace `for action in actions.iter() {` by `for action in actions.iter().rev() {` and the comment above it by the retail statement (actions newest first, cshell 0x10015be0 / 0x1001a898). **The file has CRLF line endings: edit it in a CRLF-preserving way** (a Python read/write without `newline=''` rewrote all 1071 lines in this worktree's attempt, which was reverted).
* Tests that encode the old order and must tick more than once (retail cascades one frame later): `crates/mission-runtime/tests/prison.rs` `weapon_drawing_activates_police_but_unarmed_start_is_peaceful` (line ~88: collect the events of two ticks) and the ignored `complete_original_prison_scripts_parse_and_first_riot_runs` (line ~181: `BuntWybuchl && PolicjantZabil` after 4 ticks). Observed in the attempt: with the one-line change only these two tests failed among those that ran (the ignored run stopped at the failing `prison` binary, so `walkthrough.rs` `every_script_gated_level_can_be_left` was not re-run with the change: do that first); the suggested multi-tick edits were prepared but not run.
* New regression test (passes with the change): two actions `first` (`ifnot Latch` / `set Seen`) and `second` (`set Latch`) must leave `Latch` set and `Seen` unset after one tick, and `a` (`set A`) / `b` (`if A` / `set B`) must set `B` only on the second tick.
* The level-viewer side (`campaign.rs`, `activation.rs`) only consumes the event list; the 27-level walk bot (`MESTER_WALK_*`, `docs/probes.md`) should be re-run once after the change because dialogue starts change frame.

Dead in retail as well (do not port): `print` / `wlaczmuze` / `ifgraczdalejniz` / `iflicznikmniejszyniz` / `include`, the free-fly flag, super jump slot 20, SFX 0x2d / 0x37 / 0x6d, `d_emiter_iskier`, `d_odglos`, scenki `sepia` (unreachable), the `launcher.exe` CD check, `save\auto.sav` relaunch hook (never create `save\auto.sav` next to the retail exes).

## 8. Backlog

The audit of every doc and source comment is `docs/backlog.md` (107 rows B001..B107: 7 OPEN, 11 MISSING, 57 APPROX, 32 UNVERIFIED; 4 high / 27 medium / 76 low visible impact, sorted by impact and effort, with the suggested owner) plus a list of 22 places where a doc and the code disagree. This document adds the retail-side view: items 1-5, 12-14 and 16 above are new or sharpened relative to that backlog (the backlog lists `obrot_postrzalu`, `aware`, `gestosc_sciezek`; it did not know that weapon drops are tied to a non-retail key, nor the hidden console variables, the locale twin, the retail save layout or the script defects).

## 9. What could not be determined

* Whether `God` also blocks instant-kill or fall damage (only the per-frame health refresh was followed); the key that opens the retail engine console (inside Lithtech.exe).
* The pre-`cipher.exe` state of `GYARI/scripts` (InstallShield cabinets not available); `dialogs_real.txt` is still in cipher form while `dialogs.txt` (a runtime dump of all dialogue lines, written by cshell 0x10017d90) is plain.
* The engine falloff formula for `AttCoefs` / `AttExps`; the render effect of `sepia`; exact use of `gestosc_sciezek` beyond the two functions named; `showframerate 2`, `ServerProps`, `c:\s_pov.bin`; the layout of `quick.sav` after the thumbnail and the purpose of `quick.srv`.
* Whether retail hides an NPC weapon in phases without `show_weapon`; whether `nie_wspinaj_sie` matches by prefix in retail.
* The meaning of the alternative head skins `china_*_glowa_{d,g1,g2,s}` (no script names them; no model attaches them).
* How the retail font loader handles a glyph file that does not exist (`nawiaslewy.pcx`...), and whether `misc/fonts/Un` / `sub` are loaded by the engine font manager.
* The classification is static: a retail feature that builds a path from pieces in code without a matching string, or an asset used by an exported-but-never-loaded JSON, is invisible to it. The 12 USED-BY-...-MISSING rows and the 4226 USED-IN-REMAKE rows are therefore lower / upper bounds, not proofs.
