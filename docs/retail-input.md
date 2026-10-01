# Retail player input, cursor and controls (cshell.dll) and what the remake does with it

Code: `keys_cfg.rs` + `options.rs` (the ONE binding table, written by the menus branch: `KeysCfg`, `options::Bindings`),
`panels.rs` (cursor hand-over), `settings.rs` (pause / focus rules), `main.rs::move_camera` (walking, mouse look), `gunfire.rs::input`,
`weapons_alt.rs::tick`. Addresses are cshell.dll virtual addresses (`python -m tools.inspect_retail START END`). The HUD side is
`retail-hud.md`, the camera / mouse-look numbers `retail-camera.md`, movement `retail-movement.md`.

## How retail reads keys
`scripts\keys.cfg` (0x100116f0 reads it, 0x100114f0 writes it) has 256 command slots with two `AddAction` ids each (`autoexec.cfg`: A=1 .. Z=26,
LeftShift 27, RightShift 28, Alt 29, Space 30, Enter 31, arrows 42..45, LeftControl 67, Esc/Menu 70, Weapon1..8 = 71..78, F1..F4 = 80..83,
MButton0/1/2 = 84/85/86). The input object at 0x1006f748 answers `IsPressed(slot)` (0x10011960): true if either id is on according to the
engine (`ILTClient::IsCommandOn`, vtable +0x1f4), polled **every frame, level triggered**. The player controller update 0x10060160 polls the
slots below; a few handlers latch the press edge in a flag of the controller. `[0x10ab5700]`, `[0x10ab56fc]`, `[0x10ab5704]` are the HUD
object's inventory, character-info and attribute-screen flags (`retail-hud.md`); `[0x100b22f8]` is the dialogue mode.

## Command table (slot, retail default keys, what cshell does, remake)
| Slot | Command | Retail default | Handler and behaviour | Remake (default keys) |
|---|---|---|---|---|
| 1 / 2 | Forward / Backward | W or Up / S or Down | 0x10060722 / 0x100608b0: move; while a stance or exhaustion rule says so the speed is 90 (crouched) or 180 (walk) | same (movement crate) |
| 3 / 4 | Step left / right | A or Left / D or Right | 0x100609a4 / 0x10060a9d | same |
| 5 | Jump | **right mouse** | 0x10060b92: needs `[ctrl+0xec]==0` (not exhausted), inventory closed (0x10060ba8), on the ground (0x100614e0), not latched (`+0xdc`); vertical velocity += 300 (cap 450), stamina -5 (0x10061e00(-5)), `sounds\speech\hero\skok.wav`; latch until landing | **Space** (owner); not while the inventory is open; holding it jumps again on landing like retail |
| 6 | Fire | left mouse | 0x10060c9a: not while the inventory (`[0x10ab5700]`) or attribute screen (`[0x10ab5704]`) is open, and not in dialogue mode unless a choice list is showing (`[0x100725d0]`); weapon fire 0x10003f90 every frame | same (`Panels::blocks_fire`; dialogue picks with the left button in campaign.rs) |
| 7 | Alternate fire | middle mouse or Alt | 0x10060cd4: press edge (latch `+0x100`) toggles the weapon's alt flag (0x100037a0: laser / flashlight / M-14 zoom, copy to the camera zoom flag `[shell+0x20]`) | **right mouse** first, middle mouse second (owner); Alt no longer bound (two ids per slot) |
| 8 | Crouch | LeftControl (both control keys are id 67) | 0x10060691: held; standing up needs head room (0x100616f0) and adds a 64-unit hop; crouched speed 0.5 x 180 | same |
| 9 | Run | LeftShift / RightShift | 0x1006063a: run = (key held) XOR (toggle `+0xd8`); exhausted (`+0xec`) forces walking | same, both shift keys |
| 10 | Toggle run | Q | 0x10060601: press edge flips `+0xd8` | same |
| 11..18 | Weapon 1..8 | digits 1..8 | 0x10060c6a: not while the inventory is open; weapon manager 0x100034a0(n) selects the weapon in holster cell n (same weapon: nothing; busy states 1, 2, 9, 10: nothing) | same (`retail_weapons::tick`; 9 and 0 are unused) |
| 21 | Inventory | C | 0x10060420: edge toggles `[0x10ab5700]` | same |
| 22 | Character info | Z | 0x10060475: edge toggles `[0x10ab56fc]` | same |
| 33 | Player attributes | X | 0x100604b8: edge toggles `[0x10ab5704]`; while it is open the two other flags are cleared every frame (0x1003b4d5) | same |
| 27 | Action / use | Space or E | 0x100605e3: calls 0x1005faa0 every frame while held; nothing while the inventory is open. Each call sends message 0x21 to the server (the 128 unit use ray, object.lto 0x10011880, asks the door or drawer: a moving door refuses, a settled one toggles again), tests the client ray for a prop (0x1002f750: refuses while the prop's own animation runs), the 64 unit item cube (0x10022470) and, in the mission tick, `ifaction` (0x10019b80) - all independently | **E** only (owner); level triggered for doors, props and `ifaction` (`bind.pressed`), no exclusivity between a door and a character |
| 30 | Reload | R | 0x10060d0f: level triggered, called after the fire call of the same frame, no inventory or state check; 0x10003d40 refuses a full magazine or an empty pool | level triggered (`controls.reload = pressed`), after the trigger; see docs/retail-weapons-audit.md section 3 |
| 31 / 32 | Previous / next weapon | F / G | 0x10060d56 / 0x10060d22: edge latch, then call 0x1005ab50 which is an empty stub (`ret`): **no effect in retail** | nothing, like retail: the menus branch removed the weapon cycling (wheel / Tab / F / G) from the main build |
| 20 | (unbound) | none | 0x10060c14: vertical velocity += 1500 on the ground; a developer super jump, not in the shipped keys.cfg | not implemented |
| F1..F4 | Belt slots | raw ids 80..83, not remappable | 0x100604f8..0x100605d2: any of them down and latch `+0x151` clear: `HUD::UseBelt(i)` (0x100348e0); works with the inventory open or closed | same (`panels::input`) |
| 70 | Menu | Esc | 0x1005ac3f: closes the inventory, else character info, else attributes, else opens the in-game menu (0x10024060) | same, one per press |
| 29 | Holster | H (in the shipped keys.cfg) | never polled by cshell: retail has no holster command | not implemented (the menus branch removed the H holster extra) |
| 250 / 251 / 252 | Quit / QuickSave / QuickLoad | F10 / F5 / F9 | menus branch | menus branch |

The mouse wheel and Tab are unknown to cshell (the engine axes `Axis1..3` are read once per frame for x and y only): the wheel / Tab weapon cycling
(`Controls::cycle`, now always 0) and the H holster were remake additions and are gone from the main build.
There is no lean command: the only "lean" is the strafe roll of the camera (`retail-camera.md`).

### Owner's deliberate changes (only deviations from the table above)
`KeysCfg::default()` (keys_cfg.rs) is the shipped keys.cfg (`KeysCfg::retail()`) with three slots changed, in AddAction numbers: jump `[30]`
(Space), alternate fire `[85, 86]` (right, middle mouse), action `[5]` (E). The requests: the right mouse button as jump made the M-14 scope
impossible to hold; Space no longer opens doors. Unit test `the_default_table_is_the_retail_one_with_only_the_owners_three_changes`
(input_probe.rs) pins the difference. Every gameplay system reads `options::Bindings` (`pressed` / `just_pressed` by slot, `label` for the
"[E]" hints); the panels swallow the bound use and weapon keys, so a rebinding follows. A fresh install without a keys.cfg starts from
`KeysCfg::default()`, an existing file (the rebinding menu writes it) wins.

## Mouse look and the cursor (0x10054c98, 0x10054bf0)
* Look is skipped completely while the inventory or the attribute screen is open (`[0x10ab5700]`, `[0x10ab5704]`): the engine axis deltas are
  copied to `0x10ab56f4/f8` and drive retail's own golden cursor (trunc(counts x 0.006625 x 180) pixels), so the raw counts were the source of
  the "uncontrollably fast mouse" the owner saw in the remake's virtual cursor. An active `alt_zoom` scope scales yaw and pitch by 0.25, dialogue mode (`[0x100b22f8]`) scales only the pitch by 0.25
  (0x10054d2d) and lets the vertical mouse motion scroll the choice list (0x10054d4d); `[0x100b247e]` (`CInvertMouse`) flips pitch; sensitivity is `[0x100b2488]` (constructor 1.0, the shipped keys.cfg
  writes 1.22, which `options::Bindings` now feeds to `view::look`; 0.5..1.5 in the menu). The camera orientation is rebuilt from yaw / pitch every frame (a slerp with the keys.cfg
  "smoothness" `[0x100b2484]` is computed at 0x1005504d but its result is overwritten at 0x1005508d, so there is no smoothing).
* Remake, by the owner's request: the inventory and the X screen use the **real OS cursor** (golden arrow icon, `cursor.rs`), released while
  they are open. The character-info screen (Z) is passive: the look and the fire button stay live under it (as in retail, only `[0x10ab5700]`
  and `[0x10ab5704]` matter).

### Cursor hand-over rules (panels.rs `cursor_step`, settings.rs `controls` / `pause_on_focus_loss`, main.rs `move_camera`)
| Event | Result |
|---|---|
| C or X opens while the mouse is locked | cursor released and shown, warped to where the panel cursor was last (screen centre the first time); the first read after the warp is ignored; remembers that the game had the grab |
| last mouse panel closes (key, or Esc: inventory first, then character info, then attributes) | cursor hidden and locked again, fire suppressed for 3 frames, the frame that takes the mouse back never turns the view |
| panel opened before the click-to-play lock | released, nothing to restore afterwards |
| menu, main menu, level load, cutscene, dialogue or death while a panel is open | panels close; only menu / main menu / loading own the cursor themselves, the others lock it again |
| alt-tab (window loses focus) while locked **or** with a panel open | game pauses, cursor free (the pause menu resumes with Esc / "Folytatás") |
| click on the game window with a panel open | ignored (the click-to-play rule needs a hidden, non-panel cursor) |
| hidden capture windows | never focused: the focus rule is exempt so headless runs keep the lock |
Verification: unit tests `cursor_step` / `close_top` / `pause_on_focus_loss` and the headless scenarios `MESTER_TEST_SCENARIO=inventory` (panel probe,
now really locked: it checks the release, the re-lock and the Escape order) and `MESTER_TEST_SCENARIO=input` (`input_probe.rs`: Space jumps, the right
mouse button does not, the right button toggles the M-14 scope, Space does not, X frees the cursor and walking / jumping stay live, the inventory
refuses the jump, Escape closes it without opening the menu, the mouse is locked again). `MESTER_TEST_SCENARIO=menu` (frontend probe) now also asserts that a level started from the main menu (new game) and one loaded from the pause-menu level list run with the mouse locked, hidden and the game unpaused. Not checkable headless: the real OS pointer and alt-tab (the focus rule is a tested pure function).

## Crosshair and HUD scale
* Crosshair (0x10010bd0): only with a weapon in hand, not while the menu (`[0x100c64a8]`) or the paused / cutscene flag (`[0x10071978]`) is set,
  not while an `alt_zoom` weapon is zoomed (`weapon+0xb20` and `[0x1006f618]`); 32x32 at the screen centre, white vertex colour (the texture
  `celownik` is red itself). It stays under the inventory and the X screen and in dialogues. Remake: same (`hud::update`, scope overlay in weapons_alt).
* Scale: every retail draw multiplies its 1024x768 constants by `width / 1024` on both axes, so retail is exact only at 4:3 (640x480,
  800x600, 1024x768); at 16:9 its bottom row would fall below the screen. The remake uses `min(w/1024, h/768)` and anchors the groups to their
  corner: identical at 4:3 (checked at 1024x768, capture `hud-1024x768-010.50.png`: health frame at (16,680), effect boxes at 448/512/576,
  ammo panel at (768,640)), corner-anchored at 1280x720 and 1920x1080.

## Held keys: level or edge triggered? (owner question "holding Space does not jump continuously in the original, I think")
Every command goes through `IsPressed` (0x10011960): `ILTClient::IsCommandOn` of either bound id, i.e. **level triggered**, polled once per controller update (0x10060160). Only the handlers
that store a flag in the controller are edge triggered. Checked against the disassembly:

| Command | Retail (address) | Held key does | Remake |
|---|---|---|---|
| Jump (slot 5) | poll 0x10060b8b/0x10060b92; needs `[ctrl+0xec]==0` (not exhausted), inventory closed (0x10060ba8), on the ground (0x100614e0), latch `[ctrl+0xdc]==0` (0x10060bbb); sets the latch at 0x10060bf7 | **jumps again as soon as it is back on the floor.** The latch is cleared at 0x100610e9..0x100610f6 as soon as the vertical velocity is <= 0, i.e. at the APEX, not on landing: it only stops a second jump while still rising. The key never has to be released. Every hop costs 5 stamina (0x10061e00(-5)), so a held key stops repeating when the stamina drops below 10 (exhausted, `+0xec`) and resumes above 15 | same: `main.rs::move_camera` passes `held(JUMP)` (level), `retail-movement/player.rs` implements the latch. Tests: `input_probe.rs::a_held_key_is_level_triggered_for_pressed_and_edge_triggered_for_just_pressed`, `retail-movement/tests/solver.rs::a_held_jump_key_repeats_on_landing_but_never_in_the_air` |
| Crouch (slot 8) | 0x10060691 level: the key HOLDS the stance, releasing stands up (head room permitting, 0x100616f0) | hold, not a toggle | same (`held(CROUCH)`) |
| Run (slot 9) / toggle run (slot 10) | 0x1006063a level; the toggle has the flag `+0xf0` (0x1006060a..0x1006062d), edge | run while held; toggle flips once per press | same (`held(RUN) != run_toggle`, `just_pressed(TOGGLE_RUN)`) |
| Alternate fire (slot 7) | 0x10060cd4, latch `+0x100` | one toggle per press | `just_pressed` |
| Panels C / Z / X, F/G | 0x10060420.. latch flags | one toggle per press | `just_pressed` |
| Fire (slot 6) | 0x10060c9a level, the weapon decides the rate | the weapon code (0x10003f90, called every frame) decides the rate, `retail-gunfire.md` | `held` + `just_pressed` (`gunfire.rs`) |
| Reload (slot 30) | 0x10060d0f level, no latch: 0x10003d40 is called every frame the key is down and returns at once when the magazine is full (0x10003d89) | reloads when the magazine is short of ammo and the pool has some; a held key re-issues the request every frame | **edge** (`just_pressed`): identical unless R is held through a shot. Kept (the reload state machine `0x100033b0` behind it was ported per press) |
| Use (slot 27) | 0x100605e3 level: 0x1005faa0 runs every frame the key is down (128-unit segment from the eye, then the object/NPC use request 0x1002f750, and the item cube 0x10022470) | pickups: while held, the 64-unit cube in front of the eye takes every item overlapping it (already level triggered in `pickups.rs`, `bind.pressed`). Doors, switches, NPC talk: the request is sent every frame in retail, the target object decides whether it reacts twice | doors / switches / talk use the press edge (`just_pressed`): one request per press. Not verified against the objects' own re-trigger rules (a held E on a lever may toggle it repeatedly in retail) |

So retail DOES bunny-hop with the jump key held; the remake matches it and the owner's belief is not supported by the disassembly (the guess probably comes from the 5-stamina cost that
ends the chain after a few hops, or from the 0.6 s flight time between two hops). If the owner still wants "one jump per press", it is a one-line change in `move_camera`
(`held(JUMP)` -> `just_pressed(JUMP)` latched until landing) but it would deviate from retail.
One retail quirk shows in the trace (`retail-movement` test): the standing query 0x100614e0 also accepts a body a few units above the floor, so on touch-down the controller can fire the jump one tick
early, get flattened by the floor and jump for real on the next tick: two jump events (two `skok.wav` starts and 2 x 5 stamina) 17 ms apart for one hop.

## Open / not checkable here
* The HUD colour option (8 palettes, `retail-hud.md`, `keys_cfg::hud_color`) is not wired to `hud.rs` yet (default palette 7 only).
* Alt is no longer an alternate-fire key (a slot holds two ids). Retail's F/G "previous / next weapon" do nothing (0x1005ab50 is a bare `ret`), and neither do they here.
* Reload is edge triggered in the remake (retail: level triggered, see "Held keys" above).
* Dialogues (see `retail-dialogue.md`): walking, yaw, weapons and the AI stay live in the remake (retail has no dialogue test). Retail scrolls the choice list with the same mouse motion that turns the view (`acc += sens * counts * 0.006625 * 8`, pitch x0.25, left button chooses) through a mode flag that is never set in cshell.dll. Owner-requested default of the remake: normal mouse look, the mouse wheel and Up / Down pick the answer, Enter / click confirm (arrows do not walk while the list is up), a one-line hint only for the first list of a game; the option "Gyári egeres választás" restores the retail mouse selection.
* The real OS pointer speed, focus changes and the golden cursor image can only be judged by the owner on screen.
