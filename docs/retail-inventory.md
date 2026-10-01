# Retail inventory, character sheet, new-player screen and menu ghost text

Evidence is cshell.dll (Sniper: Path of Vengeance); addresses are virtual addresses, readable with
`python -m tools.inspect_retail START END`. Art, strings and item numbers come from the retail files via
`python -m tools.export_inventory ../GYARI output` (writes `retail_inventory.json`, `ui/panel`, `ui/items`,
`ui/fonts`) and `python -m tools.export_menu ../GYARI output` (ghost/label variants added to `retail_ui.json`).

## Panels (HUD object 0x10a0b420; none of them pauses the game)
| key | panel | handler | placement |
|---|---|---|---|
| C | Felszerelés (inventory) | 0x10034a00 | right half, panel art 512x768, alpha 0.8 |
| Z | Karakter infó (Jellemzők) | 0x10038f2d | left half |
| X | Játékos tul. (attributes) | 0x1003a2d4 / click 0x1003a8e4 | full screen 1024x768 |

Keys come from GYARI/scripts/keys.cfg (C=3, Z=26, X=24, F1..F4 = 80..83). While the inventory or X screen is open
the mouse moves a golden cursor (trunc(counts * 0.006625 * 180) pixels) instead of the view, fire is suppressed
(0x10060caa) and the use, jump and weapon-number keys are swallowed (0x10060ba8, 0x10060c7e, 0x1005faa0); the remake uses the real OS cursor instead of the golden virtual one (retail-input.md).

## Item grid (0x1001bba0..0x1001c3a0)
* One grid, 4 columns. Rows 0..2 are the backpack (unbounded columns; the two triangle arrows scroll one column per press),
  rows 3..4 the holster (slot = column + 4*(row-3), keys 1..8), row 5 the belt (F1..F4). Cells are 64x64 at
  x = 116/188/260/332 and y = 137/209/281 (backpack), 411/483 (holster), 603 (belt), in panel-local pixels.
* A newly carried item first merges with the same id (stackable, count in the cell's top-left corner), weapons take the first free
  holster cell (then the backpack) and never stack: a second copy only gives its rounds. Ammunition never enters the grid.
* No carry limit and no "full" state: strength and dexterity are display values. Weight (0x1001bb50) is the sum of
  count x unit weight (2 decimals, round half up), e.g. apple + nightstick + Glock = 1.28.
* Left button held over a movable cell picks the icon up; releasing on a cell moves it (an occupied cell swaps, stacks never merge);
  releasing on the left half throws one unit from eye + 16*d with velocity 480*d (0x1003508a -> 0x1001c3a0). `nie_ruszaj`
  items (Golden cat) cannot be dragged.
* Right button on an `eaten` item consumes one unit (health capped at max, alcohol/power-up/painkiller added). F1..F4 use the belt cell
  with the panel open or closed (0x100348e0); retail decrements to zero before removing an entry, so a belt stack of n gives n+1 uses.
* Tooltip (title, description) after 0.5 s without cursor movement.
* Number keys 1..8 select the weapon in that holster cell; wheel/Tab walk the occupied cells.

## Pickups (0x10022470, 0x10022360, 0x1001c100)
Every frame each item that overlaps the player's box is taken (0x10022610 -> 0x10022360). The 64-unit cube 64 units ahead of the eye (0x10022470) is only tested while the action key
(E / Space, control 0x1b) is held and the inventory panel is closed (called from 0x1005faa0 <- 0x100605e3), so items on shelves need E.
Ammunition is refused while that type is at its cap (0x10066248: 170, 60, 150, 250, 300, 60, 200, 20, 12, 8 by ammo index); a thrown
item cannot be taken while it flies. `receive` in a mission script spawns the item at the player (taken at once, 0x10019a04);
`tnijitems` (0x1001a558) sets health to max, empties ammo and takes every carried item.

## Character sheet
Globals 0x100b2390..0x100b23ec: max health/stamina, experience (float), threshold (starts 1000), free points (start 30),
level (starts 2), kills, weapon skills (index 1..8 = experience_index in items.txt; index 0 is the alcohol level, decays 0.5/s).
* A player shot that kills adds the victim's `exp_gained` (postacie.txt) and one kill (0x10006b49); `expgained` in a dialogue only
  prints "Sikeresen szereztél N tapasztalat pontot." and adds nothing.
* Level-up when old < threshold <= new: +1 free point, level+1, threshold += trunc(sqrt(level)*1000) (2732, 4732, 6968, ...).
  A single gain crossing two thresholds stops leveling for good (reproduced).
* Each bullet or pellet that hits a character adds 0.1 to the weapon's skill up to 99.8 (0x10006aa1); the player's spread is
  rozrzut * (1 - 0.0075 * skill) (0x10005fb7); the view kick scales with (100 - skill) * 0.01 (0x100051f6, exposed as `Stats::kick`).
* X screen: one free point per click on a plus icon (max stamina, max health, strength, each weapon skill up to 98.8+1).

## New-player screen (`char_create_menu.txt`, draw 0x10026440, Start 0x100284c0)
Positions in 1024x768: title (580,90) scale 0.5 red; labels scale 0.3 red at (420,480) Pontok felosztása, (320,540) Állóképesség,
(320,590) Életerő, (340,650) Nehézség; Start! (800,700) scale 0.4 red; numbers scale 0.4, colour (222,194,120): points (358,470),
stamina (530,548), health (530,596), weapon skills "%.2f" at (820, 260+48i). Hover-only 16x16 arrows: stamina/health minus at x=480,
plus at x=624 (y=560 and 608); weapon rows minus x=768, plus x=912, y=272+48i. Difficulty: only the selected 32x32 chevron is
drawn at (480|544|608, 648): easy, tough, real. Skills cost one point each up to 99; Start copies the added stamina/health/skills and
turns leftover points into free points. The difficulty factor 0x100b2478 becomes 0.33 / 0.67 / 1.0. The round back button is at
(954,653) 46x46 and lights up under the cursor.

## Main-menu ghost text (0x10027aa0, clock 0x1002ae45)
Each of the 12 list slots is drawn twice. Menu clock t (seconds since the menu opened, wrapped at 2*pi), label width w and height h
(integers at scale 0.35: n*11.2 x 22, position x=340, y=240 + 52*i + 17):
ghost = same text at scale 0.35*1.2, colour (71,66,54), top-left (340 + 0.1*w*sin t, 240 + 52*i + h/3 + 0.125*h*sin 2t), drawn first;
label colour (199,194,158), hovered (255,224,0); the header "Főmenü" is red (255,0,0) at scale 0.5.
The same routine draws every list page of the retail menu (slot arrays set at 0x10024230, 0x10025010, 0x100250a5, 0x1002a7e4).

## Not exact / not implemented
* Option pages 2..5 keep the remake's own controls; only the ghost effect (approximate metrics) was added to their labels.
* Grenade kills award no experience; thrown items are not stored in manual saves.
* The Z panel labels/values use coordinates read from the code but there is no retail screenshot of it to compare.
