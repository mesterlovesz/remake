# Items, inventory, consumables and character progression (cshell.dll audit, round 2)

Addresses are cshell.dll virtual addresses (`python -m tools.inspect_retail START END`). Code: `inventory.rs` (grid), `character.rs` (stats, health function, timed
effects), `panels.rs` (C / Z / X screens), `pickups.rs` (pickup rules), `retail_state.rs` (weapon manager), unit tests in the same files. Earlier notes:
`retail-inventory.md` (panel layout, pickups, level-up), `retail-hud.md` (HUD boxes and fades), `retail-weapons-audit.md`, `retail-input.md`.

## Global player state (0x100b2390.., initialised by 0x1001aa80 = new game, 0x10024bf9 = character screen Start)
health 0x100b2390 / max 0x100b2394 (100), stamina 0x100b2398 / max 0x100b239c (100), 0x100b23a0 and 0x100b23a4 = 100 (dexterity current/max, only displayed on Z),
strength 0x100b23a8 (100, +1 per X click, only displayed and used for the Z "Maradt" = strength - trunc(weight)), power-up 0x100b23ac, painkiller 0x100b23b0,
alcohol 0x100b23b4 (all start 0), weapon skills 0x100b23b8..0x100b23d4 (8 floats, index = experience_index-1), experience 0x100b23d8 (float, 0), level threshold 0x100b23dc
(1000), free points 0x100b23e0 (30), level 0x100b23e4 (2), kills 0x100b23ec (0). Nothing reads strength, dexterity or level for damage, accuracy, speed or carrying:
they are display values (only the weapon skill changes the spread, 0x10005fb7; alcohol, painkiller and power-up are below).

## Verified vs approximated

| Rule | Evidence | Status |
|---|---|---|
| Item use (right mouse in the panel 0x10035488, F1..F4 0x100348e0) | `eaten` items only; health goes through 0x10061b90, then painkiller += `pain_killer`, power-up += `power_up`, alcohol += `alcohol`; the stack count drops by one (panel: removed when the count was 1; belt: decrement to 0 first, so n+1 uses); no sound, no cooldown, used even at full health; the belt works with the panel closed | OK (`Items::use_in_panel`, `use_belt`, `Campaign::apply`) |
| Health-change function 0x10061b90 (items, script health line 0x1001a71a, falls 0x10061058, blasts 0x1005fd20, death zones 0x1002ca2c, enemy bullets 0x1005fe50) | ignored while the level is younger than 60 frames (0x10061ba7) and once dead; a gain is added as is; a loss is scaled by (100 - min(painkiller,100)) * 0.01; clamp 0..max; arms the health-bar timer (2 s) for any non-zero change; heartbeat sound when crossing 50 / 20; death (0x10060070) when health + power-up <= 0 | FIXED: it was applied per call site; items and script events ignored the 60-frame rule, falls and death zones ignored the painkiller. Now `Campaign::change_health` / `character::health_after` (unit test) |
| Timed effects 0x10061d20 (every player update) | painkiller and power-up fall 1 per second, alcohol 0.5 per second, floor 0; no stack limit and no upper cap on any of them; the HUD boxes print `%d` (retail-hud.md) | OK |
| Power-up | not added to health: death needs health + power-up <= 0, so at 0 health the power-up keeps the player alive while it lasts; the health bar shows (power-up + health) / max | OK (`Campaign::dead`) |
| Painkiller | only reduces losses (0..100 %), no effect on healing | OK |
| Alcohol | its only gameplay effect is the drunk view (0x10055e5b, 0x100563a7, `view::drunk`, capped at 100 there); Z and HUD show the raw value | FIXED: the view kept its own alcohol counter, so drinking never made the view drunk; the view now reads `Campaign::alcohol`. The Z panel's 99 cap was invented and is removed |
| Death penalty (0x10060070) | painkiller, power-up and alcohol are set to 0, the weapon is put away (0x10003660); no experience, item, skill or max-health loss | FIXED: `Campaign::death_penalty` |
| Regeneration | none for health (0x10061b90 with 0 is only the death check, called every frame at 0x100612a6); stamina: -4/s running, +0.5/s walking or crouched, +5.5/s standing (0x10061e00, movement crate) | OK |
| Stack limits, capacity | none: identical item ids merge (count in the cell corner), backpack rows 0..2 are unbounded columns scrolled by the arrows, holster 8 cells, belt 4; weight is display only (0x1001bb50 is called only by the C and Z panels) | OK |
| Pick-up (0x10022470, 0x10022360, 0x1001c100) | by touch only (player box or the 64-unit cube ahead of the eye), no use key; ammunition refused while its pool is at the cap (table 0x10066248 = 0x100660d8: 170, 60, 150, 250, 300, 60, 200, 20, 12, 8) and clamped to it after adding (0x10003d10); a weapon: first copy loaded with `ammo_amount` and placed in the first free holster cell (else the backpack), later copies only add rounds; anything else stacks or takes the first free backpack cell; the item's `pickup_icon` shows 1.5 s (0x10034570) and its `pickup_sound` cue plays (`sounds\pickup\general.wav` for every item; never played in tests) | OK |
| A pick-up never selects a weapon | the only caller of the select function 0x100034a0 is the weapon-key loop 0x10060c6a..0x10060c8f; the save loader 0x1004cd45 selects through 0x10003540 | FIXED: `Inventory::acquire` no longer draws the weapon (the remake did) |
| Weapon cycling | none: F/G (slots 31/32) call the stub 0x1005ab50 (`ret`); the wheel and Tab are not read by cshell | REMOVED (main already; `Inventory::cycle`, `Items::holster_step`, `Controls::cycle` deleted) |
| Level change (0x1005adf0, then 0x1001b880) | the weapon manager is reset (0x10003660): every level starts with empty hands; carried items (state 1), ammunition pools, health, timed effects, stats and skills are kept; items lying in the world or thrown are deleted; a full clear (0x1001b8b0) happens on new game (0x1002858e) and on load (0x1004c30d); `tnijitems` (0x1001a558) sets health to max, zeroes the ammo pools and flags 0x1005a3d5 to drop every carried item at level frame 37 (used once, at the end of the prologue action `ChceWyjsc2`) | FIXED: `Inventory::empty_hands()` in `campaign::load_world` |
| New game defaults | 0x1001aa80 / 0x10024bf9: values above; the character screen adds stamina, health and skills and leaves the rest as free points; difficulty factor 0.33 / 0.67 / 1.0 (0x10028564) | OK (`Stats::new_game`) |
| Experience | +`exp_gained` (postacie.txt, per character: 10 x6, 100 x4, 150 x5, 200 x4, 250 x6, 450 x2, 500 x7, 1000 x2, 1500 x2) and kills +1 only when a player BULLET or PELLET kills (0x10006b49, after 0x10043090 at 0x10006b30); melee kills (0x1000fcfb), grenade kills and scripts give nothing; `expgained N` in dialogues (0x10019a94, 7 in dialogi.txt) only prints "Sikeresen szereztél N tapasztalat pontot." for 5 s; there is no mission or discovery experience (0x100b23d8 has one writer) | OK |
| Level-up (0x10006baa) | when old < threshold <= new: free points +1, level +1, threshold += trunc(sqrt(new level) * 1000) (2732, 4732, 6968...), text `GameShell3` for 7 s (0x100b2304), 64x64 level-up icon at (0,0) for 3 s (0x10034560, 0x10037e10; paused while the X or Z screen is open); a gain crossing two thresholds levels once and then never again (retail bug, kept) | OK |
| Weapon skill | +0.1 per bullet/pellet that hits a living character while the skill < 99.8 (0x10006aa1); spread x (1 - 0.0075 skill); index 0 (nightstick, grenade) reads the alcohol slot but their spread is 0 | OK |
| X screen (0x1003a8e4) | one free point per click edge: skill +1 while < 98.8; max stamina +1 and stamina +1; max health +1 and health +1; strength +1; no way back | OK (`character::spend`) |
| Z screen values (0x10039500..) | health, max, stamina, max, dexterity x2, "Maradt" = strength - trunc(weight), strength, alcohol, power-up, painkiller, all truncated integers | OK |
| Tooltips | title + description (Hungarian item texts, exported in retail_inventory.json) after 0.5 s without cursor movement; icons `icon1024` of items.txt | OK |
| Drag out of the panel (0x1003508a -> 0x1001c3a0) | one unit leaves at eye + 16 * d with velocity 480 * d, refused ("Not enough room!") when the 16-unit cube is blocked; `nie_ruszaj` (Golden cat) cannot be dragged | OK; thrown items are not stored in saves (APPROX) |
| Panel layout at other resolutions | retail multiplies every constant by width/1024; the remake scales by min(w/1024, h/768): identical at 4:3, the whole panel stays visible at 16:9 (retail would crop the 768-high art) | APPROX (captures at 1024x768, 1280x720, 800x600) |
| Saves | `Checkpoint` (campaign.rs, written by savefile.rs) carries the items grid, stats (points, skills, power-up, painkiller), alcohol, kills, experience and the native weapon snapshot including the selected weapon (retail 0x1004cd45 selects it on load); test `stats_and_items_survive_a_save_round_trip` | OK |

## Consumables (GYARI/scripts/items.txt, checked by the ignored test `consumables_and_flags_match_items_txt`)
| item | title | health | power-up | painkiller | alcohol | weight |
|---|---|---|---|---|---|---|
| healing kit | Kötszer. | 25 | | | | 0.25 |
| medpack | Orvosi táska. | 50 | | | | 0.5 |
| an apple | Alma. | 5 | | | | 0.15 |
| cereal box | Kukoricapehely. | 15 | | | | 0.25 |
| Schnickers wafer bar | Csoki szelet. | 10 | | | | 0.1 |
| whiskey bottle / vodka bottle | Whisky. / Vodka. | 40 | | | 40 | 0.75 |
| wine bottle | Bor. | 15 | | | 15 | 0.75 |
| a can of coke | Kóla. | 5 | | | | 0.33 |
| a can of beer | Egy doboz sör. | 15 | | | 6 | 0.33 |
| beef steak / chicken drumstick / burger | Marhasült. / Csirkecomb. / Szendvics. | 20 / 10 / 10 | | | | 0.3 / 0.25 / 0.3 |
| small syringe / horse syringe | Energialöket. / Energia. | | 20 / 40 | | | 0.2 / 0.3 |
| pain silencer / pain killer | Fájdalomcsillapító | | | 20 / 40 | | 0.2 / 0.3 |
| Golden cat. | Macska szobor. (`nie_ruszaj`, quest item) | | | | | 5 |

There is no armour or vest, no cigarette item (`papieros` is an empty weapon record without a player model), and no key or document item: the only quest item is the
Golden cat, everything else is mission variables set by scripts. `receive` (dialogi.txt: the S&W and the M-14 only) spawns the item at the player, who takes it at once (0x10019a04).

## Screenshots (headless, `MESTER_TEST_SCENARIO=inventory`, `MESTER_WINDOW=WxH`)
C screen with the full test pack (weight 16.46, stacks 4 / 3 / 2 / 2, weapons in holster cells 1..5), X screen (33 free points after three level-ups, 17 kills, 5350
experience, next level 6968, Glock 5.50), Z screen (all values 100, Maradt 84, alcohol / power-up / painkiller 0); also 1280x720 (panel scaled to the height) and 800x600.
