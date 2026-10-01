# Original campaign inventory and escape handoff

Read-only source audit, 2026-09-28. Counts describe authored maps, not completed remake playthroughs.

## Count and source classification

There are **26 playable maps on the linked retail main route**, plus the prison introduction world. A further authored playable map, `chinatown`, has a complete gameai block and shipped version-83 DAT, but no inbound transition in the shipped main-route scene/script graph. Thus **27 authored gameplay maps**, **one campaign introduction map**, and **three extra DATs** (`nic`, `katscena`, `outro`) account for all 31 DAT files. The script-only test blocks `test2`, `pudlo`, `test`, and `boks` have no matching shipped DAT.

`rh3-miasteczko0` is the playable prologue, preceding `rh1-wiezienie1`. It cannot be counted as a post-prison mission. Starting after the existing four playable prison maps leaves **21 forward main-route gameplay maps**, plus the skipped prologue (22 other linked maps altogether). The legacy Chinatown map is additional authored content, not proved part of that route.

The first Chinatown DAT contains an outbound `worlds\chinatown2` string, but `podziemia1c`'s authored door `hujjj` goes directly to `chinatown2`; do not silently insert the legacy map into the campaign.

## Linked order, followed by legacy content

| Order | World | Original Hungarian title |
|---|---|---|
| 1 | `rh3-miasteczko0` | Előszó |
| 2 | `rh1-wiezienie1` | Bevezetés |
| 3 | `rh1-wiezienie2` | A Lázadó |
| 4 | `rh1-wiezienie3` | Keress fedezéket! |
| 5 | `rh2-wiezienie1` | Találj kiutat! |
| 6 | `rh2-wiezienie2` | A Menekülés |
| 7 | `rh3-miasteczko1` | Az Álmos Üreg |
| 8 | `rh3-miasteczko2` | Az Utcán |
| 9 | `burmistrz1` | A Polgármesternél |
| 10 | `burmistrz2` | A Kulisszák mögött |
| 11 | `chapel_mniejszy` | Az öreg kolostor |
| 12 | `knajpa` | A Bullseye Kocsma |
| 13 | `Rh7a-Tunele` | A Titkos Átjáró |
| 14 | `podziemia1` | Alagutak |
| 15 | `podziemia1a` | Irány a sötétség |
| 16 | `podziemia1b` | Közel a szabadság |
| 17 | `podziemia1c` | Égő város |
| 18 | `chinatown2` | A Templom |
| 19 | `RH9-fabryka` | A Gyár |
| 20 | `rh10-wiezowiec1` | Az utolsó ítélet |
| 21 | `rh10-wiezowiec2` | A Felhőkarcoló |
| 22 | `rh10-wiezowiec3` | A Küzdelem |
| 23 | `wiez_wn1` | Harcmezők |
| 24 | `wiez_wn2` | Vadászat a vadászra |
| 25 | `wiez_wn3` | >LNIn3 |
| 26 | `rh12-lab1` | Kínzókamrák |
| 27 | `rh12-lab2` | Leszámolás |
| legacy | `chinatown` | Kiskína |

The order follows authored `Skok_do_levelu`, mission `startlevel`, and cutscene `runworld`. Machine-readable source lines, titles and links are in `output/campaign-inventory.json`. The introduction and bus transition live in `scenki.txt`; their links supplement the map/script graph.

## Prison bus escape

Source `scripts/scenki.txt` lines 545–599 defines `ucieczka z 2 wiezienia`. In `rh2-wiezienie2`, `o_cutscene0` is `(1040,-506,472)` at yaw PI. Its original **detection_radius is 200 native units**. No kill-all requirement or extra script flag gates this scene.

| Phase | Seconds | Authored behavior |
|---|---|---|
| 01 | 2 | `scena1`, bus phase `zapala`, camera socket 0, original subtitle/speech 11 |
| 02 | 3 | `scena2`, bus phase `jedzie`, camera socket 1 |
| 03 | 5 | `scena3`, camera socket 2, `object_anim otwiera brama1` |
| 04 | 0 | `runworld worlds\rh3-miasteczko1` |

The actors `autobus_ucieczkowy` and `autobus_ucieczkowy_pusty` follow socket3 `autobus` and socket4 `autobus_pusty`. Both retain their original model transforms and socket offsets; no automatic centering, removal of apparently overlapping bus geometry, or pose correction is authorized. This preserves the source data responsible for the user-reported bus-position bleed, although reproducing the actual retail bug still requires runtime comparison.

After the bus, town1's `koniec` door at `(-2352,-370,1873.1621)` leads to town2. Town2's `wrota3` at `(2656,-489,1796)` leads to `burmistrz1`. Town0 is not part of this escape path.

## Exported content and format

All 27 supported linked campaign worlds now have scene, gameplay, item and prop exports. Gameplay preserves ordered phase commands, dialogue graphs, markers, AI actions, emitters, character definitions and navigation. `items.json` merges new definitions without deleting earlier prison pickups. `campaign_cutscenes.json` maps all six authored scene names to the existing opening timeline schema; `bus_escape.json` is the bus-only view. `actors.json` merges the new actors with opening actors. Models, textures, voiced dialogue and scene audio are exported from the original files.

The current source scripts are plaintext; `read_script` detects that form and also accepts the encoded distribution. `decode_text` remains the original reversible letter-swap operation. No original file is rewritten.

Two source compatibility cases are explicit: original `.lta` emitter references resolve to the shipped `.ltb` character model; sprite skins on environment props export the first authored DTX frame. Animated sprite playback remains a renderer concern; first-frame export is not claimed equivalent to retail animation.

## Runtime integration requirements

Town1/2 use the existing interpreter command set. Later/prologue scripts also need `ifseenbyhostile`, `ifhostileblizejniz R`, `ifactionhostile`, `ifplayerhas Golden cat.`, and `zdrowie -1000`. The last command is the original Stella-death failure in chapel (line 1522) and tavern (line 4362). Predicate meanings are inferred from names and warning dialogue context; binary semantics are not independently verified.

Asset export success does not establish a full campaign playthrough. Dialogue choices, original links, cutscene triggers and special action effects still require integrated verification. Known dangling source references are retained in the export report.
