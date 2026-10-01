# Retail HUD: bars, effect boxes, ammo panel, fade rule (cshell.dll)

Code: `crates/level-viewer/src/hud.rs` (draw, timers, unit tests, capture scenario `MESTER_TEST_SCENARIO=hud`) and the pickup / level-up
icons in `panels.rs`. All addresses are cshell.dll virtual addresses (`python -m tools.inspect_retail START END`). The HUD is one global
object at 0x10a0b420; a field `+0xNNNNN` of it is also reachable as the absolute address 0x10a0b420+0xNNNNN
(e.g. `+0xaa2bc` = 0x10ab56dc), which is why the timers show up as bare statics in the disassembly.

## Layout (1024x768 space, scaled by `screen width / 1024` in both axes)
Every draw routine multiplies its constants by `[hud+0x129eec] / 1024` (the client width), so retail is only exact at 4:3. The remake
scales the UI by `min(h/768, w/1024)` (hud.rs `update`) and anchors the groups to their corner, which is identical at 4:3.

| Group | Draw | Frame | Fill / digits |
|---|---|---|---|
| Health | 0x10037740 | 256x32 at (16,680) | bar 216x20 at (48,686): the fill is `min((power_up + health) / max_health, 1)` of the texture width, cropped not stretched (u = fraction) |
| Stamina | 0x10037aa0 | 256x32 at (16,720) | bar 216x20 at (48,726): `min(stamina / max_stamina, 1)` |
| Power-up box | 0x10036880 | 256x64 at (16,448) | value (`%d`, truncated) at (88,458) |
| Painkiller box | 0x1003800e | 256x64 at (16,512) | value at (88,522) |
| Alcohol box | 0x100373bb | 256x64 at (16,576) | value at (88,586) |
| Ammo panel | 0x100385d0 | weapon's HUD image, 256x128 at (768,640) | magazine bar 228x24 at (782,729) (`rounds / capacity`, full for melee and grenades, 0x10038925); magazine digits at (962,655), reserve digits at (952,692) |
| Pickup icon | 0x10038390 | 128x128 at (896,512) | the picked-up item's icon; timer set to 1.5 by 0x10034570 (item pickup), see `panels.rs` |
| Level-up icon | 0x10037e10 | at (0,0) | see `panels.rs` |
| Crosshair | 0x10010bd0 | 32x32 centred, white vertex colour (the texture `celownik` is itself red) | |

## Colour
Every element except the crosshair is drawn with the HUD colour: vertex RGB from `[0x100b23fc..0x100b2404]` (0..255 floats), alpha
byte = `[0x100b2408] * min(timer, 1)` (float to byte with 0x10064990). The constructor (0x1001ac1f..0x1001acf1) fills eight palettes and
selects index 7 (`[0x100b246c] = 7`) with alpha `0x43440000` = 196.0:

| idx | RGB | idx | RGB |
|---|---|---|---|
| 0 | 175,19,19 | 4 | 91,205,204 |
| 1 | 201,78,78 | 5 | 37,133,76 |
| 2 | 131,51,121 | 6 | 232,227,73 |
| 3 | 63,119,194 | **7 (default)** | **208,115,16** |

The options page has an 8-step slider that writes `[0x100b246c]` (0x10029120..0x100291eb) and keys.cfg stores it after the 512 key slots;
`hud::HUD_PALETTE` holds the table (the same numbers as `keys_cfg::HUD_COLORS`); `hud::hud_rgb()` is the colour every HUD element and digit is drawn with each frame (default index 7). The options page work only has to return the selected palette from `hud_rgb()` (the menus branch has `option_effects::hud_rgb()` and `HudTint` for the old HUD; hud.rs draws its own images and needs no marker).

## Fade rule
Six timers live in the HUD object and count down at one per second while the HUD updates (0x1003b462..0x1003b4cf, one `fsub dt` each;
the whole update is skipped while `[0x10071978]` (menu / pause / cutscene) is set, so a paused game does not fade). A group is drawn only
while its timer is > 0, with alpha factor `min(timer, 1)`, so a timer of 1 is one second of fade-out at full start alpha, a timer of 2 is one
second of full alpha followed by the fade-out. The draw functions and the state code (re)arm them:

| Timer | Group | Armed by |
|---|---|---|
| `+0xaa2bc` | Health bar | set to **2.0** by the health add function whenever its argument is non-zero (0x10061cad; damage, healing, any consumed item, even at full health) and every frame the power-up value changes (0x10061daf, 0x10061d20 decays it every frame); pinned to **1.0** every frame the weapon manager has a weapon in hand (0x1001030a in the weapon update, condition 0x10009e70) |
| `+0xaa2b8` | Stamina bar | **0.95** on every frame the stamina changed (0x10061ed5, in the stamina update 0x10061e00: -4/s running while moving, +0.5/s walking or crouched, +5.5/s standing still; retail-movement.md); pinned to **1.0** while `stamina != max_stamina` (0x10037ad5 in the draw) |
| `+0xaa2c0` | Ammo panel | pinned to **1.0** while the weapon object exists (0x10038608, `[0x1006f608] != 0`) |
| `+0xaa2d0` | Power-up box | pinned to **1.0** while `power_up > 0` (0x100368be) |
| `+0xaa2cc` | Painkiller box | pinned to **1.0** while `pain_killer > 0` (0x1003803e) |
| `+0xaa2c8` | Alcohol box | pinned to **1.0** while `alcohol > 0` (0x100373ee) |
| `+0xaa2c4` | Pickup icon | **1.5** by 0x10034570 when an item is taken, 0 when cleared (0x1001c1a3, 0x1001c2c2, 0x1001c374) |

The three effect boxes print their number only while the timer is still exactly 1.0 (0x10036aac), so the digits vanish at once when the
effect ends and the empty frame fades out. All six timers are zeroed by the level reset (0x10033218..0x1003323d).

Hidden regardless of timers: the health, stamina and effect groups while the character-info (`+0xaa2dc`) or attribute (`+0xaa2e4`) screen is
open, the ammo panel while the inventory (`+0xaa2e0`) or attribute screen is open.

### What this means in play
* Standing at full stamina and health without a weapon: no bar is drawn at all.
* Running or being tired: the stamina bar is fully drawn while it is below max and fades over 0.95 s after it is full again.
* Being hurt (or healed, or eating): the health bar shows for one second, then fades for one second.
* A weapon in hand: health bar, ammo panel and crosshair are permanent; putting the weapon away fades them out in one second.
* Alcohol / painkiller / power-up: the box (and for a power-up the health bar) stays while the effect lasts, then fades in one second.

`hud::HudFade` reproduces this; its unit tests pin every number above and the capture scenario (`MESTER_TEST_SCENARIO=hud`, capture at
`2,3.5,4.6,5.5,7.5,10.5`) logs the alpha each group is actually drawn with in the running game.

## Approximations
* "Weapon in hand" is `selected && equipped` of the remake's inventory. Retail tests that the weapon model object exists (0x10009e70: `weapon+0x650` and its handle +0x38);
  the object is replaced inside one call when a change swaps weapons (0x100036a0), so it exists in every frame of the lowering and rising states: the two rules agree.
* Health events: the bar is armed by every health-change call with a non-zero amount that passes the gates of 0x10061b90 (player alive, level older than 60 frames, 0x10061cad),
  also when the clamp leaves the health unchanged (healing at full health, a loss the painkiller absorbs completely), and not by a zero amount. `Campaign::health_events` counts
  exactly those calls (`character::arms_health_bar`); a change of `health` or `power_up` by other means still arms it as well.
* The bar / digit fonts use the exported `ammo` bitmap font at scale 1.0; the retail call (0x100371b0) takes a 32-pixel cell.
