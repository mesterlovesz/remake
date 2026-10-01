# Retail dialogue system (cshell.dll dialogue object 0x10072398)

Code: `crates/mission-runtime` (nodes, timing, conditions, effects), `crates/level-viewer/src/dialogue.rs` (panel, mouse, speech, hint,
probe), `campaign.rs` (event glue). Addresses are cshell.dll (`python -m tools.inspect_retail START END`). Scripts: `scripts\ai\dialogi.txt`
(229 nodes; the `//include` lines in it are comments), texts `scripts\text_keys.txt`, HUD strings `scripts\locale\gameai.txt` (IngameText1..4).

## Object layout (this = 0x10072398)
`+0x0/+0x80/+0x100/+0x180` IngameText1..4 (hint, "Sikeresen szereztél", "tapasztalat pontot.", "Dominick: "), `+0x204` node list, `+0x210` screen width,
`+0x218` node clock, `+0x21c` scale s = width/1024 (0x220 = 16 s, 0x224 = 32 s, 0x228 = 48 s, refreshed every tick 0x10018ab0), `+0x22c` highlighted slot,
`+0x234` float selection accumulator, `+0x238` answer count, `+0x23c` title text, answers at `+0xac54 + 0xaa18 i`, chosen-answer text `+0x354b4`,
`+0x3fecc` question stage, `+0x3fecd` answer stage, `+0x3ff4e` list shown, `+0x3ff4f` answered, `+0x3ff50` speaker, `+0x3ff60` current node,
`+0x3ff64` sound handle, `+0x3ff6c` experience-message timer (5 s), `+0x3ff74` hint counter (5).
Node (0xb28 bytes, parser 0x173a0, defaults 0x172d0): person 0x80, delay 0x100 (0.2), title 0x104, titlesnd 0x184, answer1..4 0x204/0x304/0x404/0x504
(+0x80 = answerNsnd, delays 0x604..0x610 default 0.2, onchoice dialog 0x714/0x794/0x814/0x894), set 0x614, unset 0x694, receive 0xa1c, expgained 0xa9c,
hostileattack 0xb20, ontimeexceeded 0x914 (0.2) + target 0x918, onfurtherthan 0x998 (320) + target 0x99c. One value per key (a repeated line overwrites),
only four answers.

## Flow
* Started by the mission tick (0x1001a750; while a node is current no `action` runs, 0x1001a864) or by `o_marker_dialog` (0x1002cc30). StartDialog 0x100194d0:
  set/unset variables, speaker = first actor tagged by this tick's conditions (any name; else the first placed instance of `person`; `hostile` without a tag has none),
  speaker reset to its `default_faza` (0x10019762 -> SetPhase 0x10041c60; a tagged actor never enters a `patrol` phase, 0x10041cbe), `hostileattack` flag on the
  speaker only (0x10019811), speech, texts, `receive` (an item at the player, 0x10019a04), `expgained` (message "IngameText2 N IngameText3" for 5 s, only a message),
  choice list armed if any answer exists. Speaker head model `+0x13c` plays `gada` (0x10019b5d), `mruga` when the node ends (0x1001950f).
* Speech: `titlesnd` positional at the speaker, radius 4096 (0x10019350/0x10019852), 2D without a speaker (0x10019420); its length replaces `delay` (0x1001988b);
  a dead speaker (`+0x144`) stops it (0x10018b7f). Answer speech is 2D and its length replaces `answerNdelay` (0x10018f04).
* Tick 0x10018ab0: list appears at `elapsed > delay` or on a click; a click with the list up and `elapsed > delay` takes the highlighted answer; then the answer stage
  shows "Dominick: <answer>" for `answerNdelay` (a click skips it) and starts `onchoiceNdialog` (empty = end). Plain / question stage: end when `elapsed > ontimeexceeded &&
  elapsed > delay` (an `ontimeexceeded` below 0.1 only ends on a click) or the player is farther than `onfurtherthan` (> 1) from the speaker (no speaker = distance 0,
  0x10043a70). Every end clears the HUD message (0x10018cb2). Nothing about the world pauses: the NPC update (0x1004ad30) has no dialogue test.
* Input: the click is engine command 0x54 (left button, press edge). The list scrolls in the mouse look (0x10054d4d): `acc = clamp(acc + sens * axisY * 8, 0, count-1)`,
  axisY = counts x 0.006625, slot = trunc(acc); no wrap, reset to 0 when the list appears. Pitch x0.25 and fire blocked while `[0x100b22f8] && count` (0x10054d2d, 0x10060cba).
* Hint: the first five lists print IngameText1 ("Mozgasd az egeret fel/le hogy opciót választhass, az egérgomb a kijelölt opciókra érvényes.") when the
  "subtitles" option `[0x100b247d]` is on (0x10018c2f). That option is read nowhere else in the game code besides the menu.

## Screen (render 0x10019010; text object 0x10015100..0x10015500)
Mincho `table` bitmap font (misc\fonts\mincho\table.dtx + table.txt, 16x8 cells of 32x64, fixed advance, unknown characters leave a blank), glyph scale `0.3 s`
(9.6 x 19.2 px at 1024 wide), no wrapping (three lines are wider than 1024 and are cut), no panel or portrait. Every line has a black copy 3 s to the lower right.

| Line | Position | Colour (RGB) |
|---|---|---|
| question (the speaker name is part of the text: "Bárpultos: Mit akarsz, Dom.") | (16 s, 32 s) | 173,237,221 |
| answer i (0-based), only while the list is up | (32 s, (32 i + 72) s) | highlighted 253,246,198, others 99,113,160 |
| chosen answer in the answer stage | (16 s, 80 s) | 253,246,192 |

Nothing is drawn during the first 60 frames of a level (0x10019039). Captures: `output/dialogue-probe/bar.png` (four answers, third highlighted),
`town-009.00.png` / `town-013.00.png` (list, then answer stage), `prison.png` (a plain node).

## Verified versus approximated
| Item | Status |
|---|---|
| Node parser limits, defaults (0.2 / 320 / four answers / last value wins) | verified (0x173a0), unit tests |
| Timing rules, click skip, `ontimeexceeded < 0.1`, answer stage, strict `>` | verified (0x10018ab0), unit tests |
| Speaker choice (first tagged), default-phase reset, per-speaker `hostileattack` | verified in code; the remake raises the one global hostile switch for `hostileattack` |
| Layout, colours, font, shadow, scale | verified (0x10019010), captures above |
| Mouse scroll numbers, click, hint text and count | verified numbers; **the mode flag `[0x100b22f8]` that enables scroll, the x0.25 pitch and the fire block is never set in cshell.dll** (byte scan: only reads and one clear at 0x1005aec0). The remake switches it on while a node with answers (or its answer stage) is current, otherwise answers 2..4 could never be taken. |
| Nothing freezes (walking, yaw, weapons, doors, AI, damage) | verified (no dialogue tests in 0x10060160 / 0x1004ad30); the old remake freeze is gone, only death halts |
| Speech positional r=4096 / 2D, length replaces delay, speaker death stops it | verified; the positional gain is the shared linear falloff (`sound.rs`), not the engine pan/dB curve |
| `expgained` = message only, `receive` = item drop at the player | verified (existing) |
| Digit keys 1..4 / Enter choosing (old remake extra) | removed: retail has only the button |
| Head `gada` / `mruga` animation of the speaker | **open**: heads are static meshes (npcs.rs) |
| `patrol` phases refused for a tagged (talking) actor | **open** |
| Speaker with `+0x98` (deactivated) cancels the node | not implemented (deactivated actors do not exist in the remake) |
| HUD message clear at each node end | implemented for the shared `Feedback` message |
| `subtitles` option gates only the hint (binary); the title always shows | implemented as the binary does; the menus branch's "subtitles off hides the title" for dialogues is dropped |

## Owner-requested deviations (2026-10-01; supersede the arrow-key / view-lock text of 2026-09-30)
All are remake additions, deliberate, not retail behaviour. Retail (see Flow above): the same mouse motion scrolls the list and turns the view (yaw live, pitch x0.25),
the click chooses, there is no wheel, arrow or Enter handling, and IngameText1 is printed for the first five lists.
* **Default (option "Gyári egeres választás" OFF).** The mouse look is completely normal during a choice list: no lock, no x0.25 pitch (`view::dialogue_look`). Mouse motion
  never changes the highlight. The highlight moves with the **mouse wheel** (one answer per notch, wheel up = up) and the **Up / Down arrow keys**; both are clamped at the
  first / last answer (no wrap) and snap the accumulator to the integer slot (`dialogue::step`, `wheel_steps`, `key_step`). Enter, NumpadEnter or the left button confirm
  (`Mission::click`). The wheel and arrows never turn the camera; while the list is up the arrows are removed from walking (`Session::dialogue_list`,
  `KeysCfg::pressed_without`), W / S / A / D still walk. The fire button stays blocked while a node with answers is current (a click chooses).
* **Option "Gyári egeres választás" ON** (display list row 8, `RetailOptions::retail_choice_mouse`, console variable `CRetailChoiceMouse`, default OFF): exact retail selection:
  mouse motion scrolls the list with the retail accumulator (`sens x counts x 0.006625 x 8`), yaw live, pitch x0.25; wheel, arrows and Enter still work on top.
  The old option "Nézet rögzítése választáskor" (`CDialogLock`) is gone; an old `CDialogLock` line in autoexec.cfg is ignored.
* **Hints.** Only the first choice list of a game prints a hint, then never again: one HUD line, "Görgő vagy fel/le nyíl: választás, Enter vagy kattintás: megerősítés"
  (retail mouse ON: the retail sentence IngameText1 instead), still gated by the retail "subtitles" option. The flag is `Campaign::hint_shown` (in the save / checkpoint;
  a save from before it counts as shown) mirrored into `RetailOptions::dialogue_hint_seen` (`CDialogHintSeen`, so it does not return every session); a new game clears both
  (`Campaign::hint_reset`). The separate remake keys line under the answers was removed.
Probe: `MESTER_TEST_SCRIPT="7:key=down,8:wheel=up,9:key=enter"` injects keys and wheel notches (`key=up|down|enter`, `wheel=up|down`).
