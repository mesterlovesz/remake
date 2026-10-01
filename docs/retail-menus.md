# Retail menus, saves, options and the ending screens (cshell.dll) and what the remake does with them

Code: `menu_layout.rs` (pure geometry, texts, click rules, the per-page scene: unit tested), `menu.rs` (state, pointer / keyboard, entities, F5 / F9 / F10 and the
level-start quick save), `frontend.rs` (the new-player page and the loading screens), `keys_cfg.rs` + `options.rs` (keys.cfg and the autoexec console
variables), `savefile.rs` (save slots), `option_effects.rs` (what the options change in the running game), `ending.rs` (outro pages and credits),
`menu_probe.rs` (scripted pointer tests). Addresses are cshell.dll virtual addresses (`python -m tools.inspect_retail START END`); the menu class lives
at 0x10023000..0x1002b000, the save manager at 0x1004b500..0x1004d9a0, the shell command handler at 0x1005aba0.

## How the retail menu works
* One page id `[menu + 0x550cb4]`: 0 main list, 1 misc (sliders), 2 sound, 3 video options, 4 video modes, 5 controls #1, 6 controls #2, 7 new player, 8 "menu",
  9 display, 0xa save, 0xb load. The image of a page is `<prefix><width>.pcx` with `main_menu_`, `misc_options_`, `sound_options_`, `video_options_`, `video_modes_`,
  `controls_menu1_`, `controls_menu2_`, `char_create_`, `menu`, `performance_`, `save_`, `load_` (0x10024380) from `misc\menu_l\` (localized) or `misc\menu\`,
  width 640 / 800 / 1024. Pages 3 and 4 are reachable only from the Esc handler of page 4, page 8 is never set (0x10025010, 0x1002ad10), and cshell never names
  `confirm_quit_*.pcx`: **retail has no video page and no quit confirmation** (the click on "Kilépés" only sets `[menu+0x10] = 2`, 0x1002a6b4).
* Everything is drawn and hit-tested in 1024 x 768 design pixels multiplied by `screen width / 1024` (the remake: `min(w/1024, h/768)` UiScale, canvas centred).
* Pointer: `[menu+8/0xc]` is the pointer clamped to the screen; the golden arrow is `misc\menu\cursor1024.pcx`. Menu clock `[menu+0x550ca8]` += dt, wrapped at
  2 pi (0x1002ae45) drives the ghost labels. There is **no keyboard navigation in retail**: Esc (command 70) goes back one page (0x1002ad10), key capture on the
  controls pages is the only other key use. The remake adds Up / Down / Enter on list and slot pages (the owner asked for keyboard navigation).
* Retail plays **no click or hover sounds**: cshell names no menu effect file (its only sound strings near the menu are `sounds\muza\menu.wav`, 0x1006e10c, the music
  cue `menu`, and `sounds\muza\credits.wav` / `outro.wav`, cues `credits` / `outro`). The remake keeps the same cues (`music.rs`) and adds none.

## Pages, geometry and evidence (design pixels)
| Page | Retail evidence | Remake |
|---|---|---|
| Main list | rows are the text ids 1, 38, 39, 5, 4, 27, 48, 6 of main_menu.txt (0x10024230): Új játék, Játék töltése, Játék mentése, Vegyes opciók, Irányítás, Lejátszás és megjelenítés, Hang opciók, Kilépés. Header `MainMenuHeader` red at (600, 90) scale 0.5 (0x1002429e). List x = 340, y = 240 + 52 i, label 17 px lower (`pitch / 3`), scale 0.35, colour (199,194,158), hover (255,224,0) inside x 340..596, click from x 340 / y 240 (0x10027aa0, 0x1002a4fb). Ghost copy behind each label: scale 1.2, colour (71,66,54), x + 0.1 w sin t, y + h/3 + 0.125 h sin 2t. Row 3 (save) works only in a game, alive, not kneeling (0x1002a61d); the back ball (954, 653, 46 x 46) resumes a running game and lights up on hover (0x10027fb0) | same rectangles, colours, ghost motion; `menu_layout::scene` is the single source of what is drawn |
| New player | 0x10024bd0 sets 30 points, 100 stamina / health, difficulty; layout 0x10024cec..0x10026656 | unchanged from the earlier remake (frontend.rs): points, stamina, health, 8 weapon skills, difficulty chevron, Start |
| Misc | header `MOMHeader` (600, 90); labels scale 0.3 red at x = 320, y = 240 / 300 / 360 / 420 (0x10025720); knobs `misc_slider` 8 x 16 at x = 570 + 256 v, tops 244 / 306 / 369 / 433 (0x10026120); a press inside x 570..826 grabs the knob, dragging ignores x outside that range (0x10028e00); sensitivity `[0x100b2488]` 0.5..1.5, smoothness `[0x100b2484]` 0..1, hud colour `[0x100b246c]` 0..7 (x = 32 i + 16 + 570), gamma `[0x100b248c]` 1..2 | same; values are keys.cfg's |
| Controls #1 / #2 | header (600, 90), column heads 0.25 red at (560, 190) and (740, 190), row labels 0.3 red at x = 320, y = 220 + 50 i (0x10025160, 0x1002541a); 9 rows (forward, back, step left / right, jump, crouch, run, toggle run, action) and 8 rows (fire, alt, reload, next, previous, backpack, character, player) from the command slots 1 2 3 4 5 8 9 10 27 / 6 7 30 32 31 21 22 33; cell centre (616 + 192 c, 224 + 48 r) 190 x 30, text scale 0.42 white, centred, top y = 220 + 48 r (0x100259e0); a cell waiting for a key is green (0,255,0), a free slot reads `*****` (0x10025acf); keys with an id below 70 and the three mouse buttons can be captured, an action key belongs to one command only (0x10028a0d, 0x10028a46); the arrow (362, 665, 96 x 48) switches pages and lights up on hover | same; Esc cancels a capture; controls #2 goes back to #1 on Esc |
| Display list (9) | rows: shot debris, insignificant objects (3 levels), insignificant characters, enemy lights, subtitles, mouse Y, quick save at a new level, weapon bob, back (0x10025080) | same rows; the remake adds a tenth row "Képbeállítások" (its pitch on this page is 46) |
| Sound list (2) | rows: music, speech, sounds, back (0x100256c0) | same rows; the remake adds the output device row (output.rs) and a master volume slider below |
| Load / Save (0xb / 0xa) | list of nodes with four strings each (file, name, two detail lines; 0x1004d8b0), header / "Elérhető fájlok" (700, 230) / "Részletek" (470, 630) / action label (810, 700, scale 0.4), all red; thumbnail (345, 233) 320 x 240 (0x10027260); detail lines white with a black 2 px shadow at (360, 556) and (360, 586), scale 0.3; ten rows at x = 714, y = 280 + 19 * 1.3 r, scale 0.3, selected (255,240,160) else (96,176,192); scroll arrows at x = 928, y 257 / 544 step every 0.1 s held, knob y = 275 + 268 top / count (0x10029460); action button (788, 693, 103 x 43). Save starts with the "new save" entry (`>SGNewSave`, 0x1002a321) and writes the lowest free `save\saveN.sav` (0x1004d7d0); the order is quick.sav then save99..save0 (0x1004d5e0). Created line `Mentve: m-d, year at: h:m` (0x1004d04b), health line `Játékos egészsége: 100.00`. Saving closes the menu and prints `SGMOk` for 5 s (0x10029eb2..0x10029ede) | same geometry; the row label is the level's Hungarian title (retail: text read from the file, format unproven), the slot file is JSON inside a `RatHunt save game.` container with a 320 x 240 RGB565 thumbnail (savefile.rs) |
| Bonus (**owner-requested, not retail**) | retail has no such page; row 9 of the main list (below Kilépés, so the eight retail rows keep y = 240 + 52 i), label `Bónusz`, same list style, ghosts, hover colour; the page (id 11, art `performance_1024`, red header `Bónusz` at (600, 90)) lists the `BONUS_ITEMS` table (`menu_layout.rs`: label key, description key, world id) plus the retail back row (text id 14); Esc / back row return to the main list; the description of the lit row (first entry when none) is two lines wrapped to 62 cells at (340, 556) and (340, 586), scale 0.3, white with the black 2 px shadow of the load details | a click on an entry starts `Request::Bonus(i)` (below) |
| Confirm quit | not in retail | remake page on the retail art `confirm_quit_1024`: rows "Kilépés a játékból" / "Vissza az előző menühöz" (F10 opens it too) |
| Video | retail pages 3 / 4 unreachable | remake page (window mode, resolution, frame cap, frame counter, field of view, model shadows) on the `performance_` art |

Colours are `SetColor(r, g, b, a)` calls whose arguments are pushed in reverse; sizes are `SetScale(f)` calls on the Mincho table (`misc\fonts\Mincho`, 32 px advance).

## Options: the retail globals and what the remake does with them
| Option | Retail global / file | Remake effect |
|---|---|---|
| Mouse sensitivity | keys.cfg, `[0x100b2488]`, 0.5..1.5, multiplies yaw and pitch (0x10054d25) | `move_camera` (also the golden cursor speed of the panels) |
| Mouse Y inverted | `CInvertMouse`, `[0x100b247e]`, flips the pitch sign (0x10054d17) | `move_camera` |
| Mouse smoothness | keys.cfg, `[0x100b2484]` clamped to 0.02..0.98, handed to a filter object (0x1005f200) whose sample function 0x1005f110 is never referenced | stored and written, **no effect, like retail** |
| HUD colour | keys.cfg, `[0x100b246c]`, 8 palette entries (0x1001ac1f..0x1001acd8) | `hud::hud_rgb()` reads `option_effects::hud_rgb()` every frame |
| Gamma | keys.cfg, `[0x100b248c]` 1..2 copied to the console variables gammar / gammag / gammab (0x1005a21d) | a full-screen pass after the 2D camera (`GammaEffect`, everything on screen incl. menus like a display ramp); applied relative to the shipped 1.15 so the default leaves the calibrated picture untouched |
| Subtitles | `[0x100b247d]`, gates the dialogue title text (0x10018c2f) | dialogue title and cutscene captions off, answers stay |
| Weapon bob | `CWeaponBob`, `[0x100b2480]`, copies the un-bobbed offsets when off (0x10055cbe) | `view::update` weapon offset |
| Shot debris | keys.cfg, `[0x100b2490]`, off skips debris pieces and bullet marks (0x100088b8, 0x10008a55) | `gunfire::impact` (`Effects::debris`) |
| Insignificant objects | keys.cfg, `[0x100b2494]` 0 / 1 / 2; `insignificant [N]` of objects.txt, level 1 hides marked 1, level 2 every marked object (0x1002d0b0, parser 0x1002d634) | props marked in objects.txt get `models::Insignificant`, hidden live |
| Insignificant characters | keys.cfg, `[0x100b2498]`; class flag from postacie.txt `insignificant` (0x1004b070) | prisoners, rats, cockroaches, newspaper: `Npc::hidden` (not drawn, not hit) |
| Enemy lights | keys.cfg, `[0x100b2499]` gates the model light pass (0x1001d840) | stored and written; the remake lights NPC models like all props, no separate pass |
| Auto quick save | `CAutoQuickSave`, `[0x100b247f]` (0x1005a5d0) | level-start quick save (below) |
| Music / speech / sound | `Music`, `Speech`, `Sound` console variables | music sink, dialogue sink, effect sinks (settings.rs `update`) |
| Resolution, window, MaxFPS, showframerate | autoexec.cfg `screenwidth`, `screenheight`, `windowed`, `MaxFPS`, `showframerate` | window size / borderless full screen, frame limiter, fps counter |
| Model shadows | `ModelShadow_proj_enable` | stored and written; the remake draws no projected model shadows |
| Field of view, volume, sound device, difficulty | not retail options (`Preferences`, settings.json) | video page, sound page, new player page |

Files: `scripts\keys.cfg` (three header lines, 256 x 2 action ids, then hud colour, sensitivity, smoothness, debris, objects, characters, lights, gamma, subtitles;
writer 0x100114f0, reader 0x100116f0 - the shipped file round-trips byte for byte in `keys_cfg.rs`) and `autoexec.cfg` (console variables plus `AddAction` / `rangebind`
lines that are kept untouched) live next to the exports (`MESTER_USER_DIR` redirects them). The written files change when a toggle, slider or binding
changes. **One binding table** (`KeysCfg::default()`): the shipped table plus the owner's three changes (jump Space, alternate fire right + middle mouse, action E).

## Bonus menu (owner request, 2026-10-01: "the unreleased level should be viewable from the bonus menu")
Not retail: a deliberate deviation of `main` (README deviation list). The main list has a ninth row **Bónusz** under Kilépés: the retail list geometry (8 rows, pitch 52 from y = 240) is untouched and the new row has its label ending at y = 695, inside the panel. It opens the bonus page (page id 11) with one row per entry of `menu_layout::BONUS_ITEMS` and the retail back row. More entries are one table line plus two `remake_text` strings.
* Entries: **Kiskína (kiadatlan pálya)**, world `chinatown`, description "A gyári telepítésben megmaradt, a kampányból kihagyott pálya (chinatown.dat)". `nic` has no mission block, NPC or prop export and `outro` is an empty stage with a cutscene object (docs/cut-content.md), so they are not offered.
* Start (`Request::Bonus`): the same new game as "Új játék" with the default character (30 unspent points, 100 stamina / health, difficulty 1) without the new-player page: `campaign.reset_requested`, `front.character_started`, `Frontend::bonus = Some(world)`, `travel.pending = world`. Retail loading screen (the level's picture and title, `loading.chinatown`), the level's own mission script and music. The difficulty is written into the in-memory preferences like the Start button does; nothing is saved by the start itself.
* Exit: while `Frontend::bonus` is set, `travel::request` turns every level request other than the bonus world itself into `Travel::menu` (the ending's return to the main menu): b_door0 (towards chinatown2 "A Templom") ends the bonus run at the main menu, a fresh campaign is armed, `bonus` is cleared.
* Save policy (decided here): **a separate slot**, `save\bonus.sav`. F5 and the level-start auto quick save (when the option is on) write it, F9 (also the death flow: GameShell4 still names F9) loads it, and the campaign's `save\quick.sav` and numbered slots are never touched or loaded into the bonus level. The bonus slot is not listed on the load page. The menu's Save row does nothing in a bonus run (`Game::can_save`), loading a slot from the menu, "Új játék" Start or the return to the main menu end the bonus run.
* Death: the normal retail flow (GameShell4, Esc menu, F9 reloads the bonus slot).
* Verified by probe `bonus` (docs/probes.md): menu row and page, pointer and Esc, loading, world / 17 actors / StartPoint, F5 -> bonus.sav only, Save row refused, death + F9, the door b_door0 -> main menu, quick.sav never created.

## Saves, quick save, death
* `save\quick.sav`, `save\save0..99.sav` (0x1004b500..): time stamp, tag `RatHunt save game.`, payload, screen thumbnail grabbed when the menu opens (0x1004b550). The remake
  payload is the whole restorable state (mission variables, player position / stamina, actors, weapons and ammo, item grid, stats, world objects, drops, pickups taken).
* F5 (QuickSave, 0x1005abd4): a kneeling player may not save and gets `GameShell1`; otherwise the picture is grabbed, `save\quick.sav` written and `GameShell2` shown for 5 s.
  F9 (QuickLoad, 0x1005ac27) loads it. F10 (Quit) asks first (remake). The level start saves by itself when `CAutoQuickSave` is on, the level was not entered
  from a save and no cutscene runs (0x1005a5d0).
* Death: retail shows only `GameShell4` ("Meghaltál. A menük eléréséhez nyomd meg az ESC gombot, a gyorstöltéshez pedig az F9-t."); the remake's earlier death overlay is hidden.
  The menu is the same main list (Esc), F9 restores.
* Esc opens the menu (0x1005abd4) unless a cutscene runs; it closes the inventory first (input branch). Losing the window focus opens it too.

## Loading screens
Loading art `misc\loading[_l]\...` (28 levels), the level title at scale 0.5 and the phase text at scale 0.25 (0x10059d80, texts Loading1..4). The remake shows the world phase and
the asset phase (its load has no separate frames for the objects and NPC phases); retail has no tips.

## Ending (0x10031a00..0x10032528)
After the outro cutscene: outro pages 1..3 (image `outroN_1024`, four centred lines at y = 540 + 32 i, scale 0.5, 10 s each), credits (`credits_l_1024`, 63 lines from y = 140, scrolling
up at 36 px/s, see docs/retail-scenes.md), outro page 4 (20 s), then the main menu (**deliberate owner deviation**: retail's 0x10059cc0 calls ILTClient::Shutdown and quits to the desktop; the remake returns to the main menu and a new game can be started again). The texts are `scripts\locale\outro1..4.txt` and `credits.txt` resolved through text_keys.txt
(`tools/export_scenes.py` -> `output/endgame.json`, read by endgame.rs); retail's outro3.txt lists `Outro32` twice and never `Outro34`, kept. Enter / Space / a click / Esc skip a page (remake).

## Status per page
| Page | Layout | Texts | Hover | Click / keys | Persistence | Verified by |
|---|---|---|---|---|---|---|
| Main | retail | retail | yellow, ghosts, ball | all rows, Esc resumes | - | scene test, probe `menu_pages` / `menu_saves`, captures |
| New player | retail | retail | arrows | points, difficulty, Start | - | frontend probe (`MESTER_TEST_SCENARIO=menu`) |
| Misc | retail | retail | - | slider drag | keys.cfg | probe: sensitivity and hud colour drag, files written |
| Controls #1 / #2 | retail | retail | arrow | capture, Esc cancel, arrow | keys.cfg | probe: capture, cancel, page switch, file |
| Display | retail + 1 row | retail | yellow | toggles, cycle | keys.cfg, autoexec | probe |
| Sound | retail + 2 rows | retail | yellow | toggles, slider, device | autoexec, settings.json | probe |
| Load / Save | retail | retail | scroll arrows | select, scroll, knob, action | save\ | probe `menu_saves` (F5, F9, new save, load), scene test |
| Bonus | remake (owner request) | remake strings | yellow | entry starts the level, back row, Esc | `save\bonus.sav` only while playing it | probe `bonus`, scene test |
| Confirm quit | remake | retail strings | yellow | yes / back, F10 | - | probe |
| Video | remake | remake strings | yellow | cycle | autoexec, settings.json | probe (fov) |
| Loading | retail art and texts | retail | - | - | - | captures |
| Death | retail message | retail | - | Esc / F9 | quick.sav | probe `menu_death` |
| Ending | retail | retail | - | skip keys | - | captures of the three page kinds |

## Not verified / approximate
* The ghost animation, the pointer and the 640 x 480 / 800 x 600 art variants (the remake always uses the 1024 art) cannot be compared with a retail run here; geometry and
  colours come from the code, not from screenshots. The retail slot row text is unproven (format of the file, 0x1004cff0); the remake shows the level title.
* Retail has no pressed state for list rows (an action runs on the click); scroll arrows show their `scroll_up` / `scroll_down` art while held.
* Real fullscreen, alt-tab and the OS pointer speed can only be judged on screen. Gamma is a shader pass, not a display ramp.
* The remake's headless tests use a synthetic pointer (`MenuState::fake`); the mapping from a real window position is unit tested (`design_pos`).
