# On-screen text: legibility audit (owner feedback "the subtitle at the top was hard to read")

Fonts, colours, positions and Hungarian wording stay the retail ones; only readability was changed. Code: `crates/level-viewer/src/retail_ui.rs`
(`Aid`, `TextAid`, `crisp_atlas`, `draw_text`), `dialogue.rs`, `opening.rs` (`subtitle_fit`), `panels.rs` (message), `hud.rs` (help line).
Screenshots (headless, `MESTER_SILENT=1`, `MESTER_WINDOW=WxH`): `C:/Temp/mester-text-shots/{before,after}/` (before = main 57beb98, after = this branch);
`off/` = same scene with `MESTER_TEXT_AID=0`.

## Findings (before)
| Text | Problem | Evidence |
|---|---|---|
| Dialogue question/answers (Mincho bitmap, 0.3 s) | 32x64 atlas drawn at 0.3x without mip levels: thin serifs vanish, grey colour bleed under transparent atlas pixels made dark fringes; blue answers (99,113,160) vanish on light walls; three nodes (Burmistrz27 118 chars, Tormiens10, LaskaZKrzeslem3) run off the right edge (retail cuts them too) | before/lab-007.00.png, before/burm-006.00.png |
| Hint IngameText1 (first five lists) | wrapped to two lines at (72,8)/(72,34) and printed over the question at (16,32) | before/bar-006.00.png, before/lab-007.00.png |
| Cutscene subtitles | bug: lines were stacked from the top of a fixed 8-line block, so one line sat at y=512 in the picture (over the car body) instead of on the lower letterbox bar; retail 0x1003450b puts line i of n at `H + (i-n-2) h`, i.e. bottom aligned, last line ends two line heights above the bottom edge. Glyph size ignored the width/1024 factor of 0x10034226 (small at 16:9) | before/story-005.00.png, story-010.00.png |
| Help banner "Kattints a játékhoz · WASD ..." | not in retail (mouse is always captured there). A 2-line Consolas strip at the top left covered the dialogue title and every HUD message ("Gyorsmentés létrehozva.") | before/death-003.00.png, before/dlg-007.00.png |
| "[E] Beszélgetés / használat" prompt, fps counter | plain TTF text, no shadow | before/burm-006.00.png |
| Menu, credits, loading | retail art with retail shadows on dark panels/pictures: readable, unchanged | before/menu.png |

## What changed
* One shared helper, `retail_ui::Aid` on `BitmapText` (`Aid::OUTLINE`, `Aid::BACKED`, `Aid::STRIP`), drawn by `draw_text`: a dark 1-1.5 px outline (a second, dilated copy of the
  atlas drawn under the glyphs, `crisp_atlas`) and/or a faint soft strip (9-sliced 64x64 blob, peak alpha 0.5) behind each line. Global switch `TextAid`
  (display page row "Olvasható feliratok (körvonal) be/ki", `Preferences.text_aid`, default ON; `MESTER_TEXT_AID=0` forces it off for comparisons).
* Mincho atlas rebuilt at start: clean white RGB (no grey bleed), four mip levels with trilinear sampling (no shimmer/blur at 0.3x, 1440p stays crisp: glyph size is
  `0.3 * width/1024` design pixels and the UI scale does the rest).
* Dialogue: lines wider than the screen wrap (`retail_ui::wrap`, 0.9 line pitch) and push the rows below down only as far as needed, so retail positions stay when the text fits.
  Shadow copy (retail +3 s) carries the strip (z 25), the text has the outline (z 26). The HUD message (hint) moves below the panel while a dialogue is up
  (`Dialogue.bottom`).
* Subtitles: bottom aligned like the retail formula, size `0.4 * width/1024` (`subtitle_k`), outline + strip for the >3-line case that grows over the picture.
* HUD messages (level-up, pickups, save, hint): outline + strip. Help banner: one short line "Kattints a játékhoz · Esc: menü" at the bottom centre with a text shadow (it is a
  remake-only element; shown only while the mouse is free). Prompt and fps counter: text shadow.
* Accents: ő/ű/á... come from the retail atlas (cells at row 5-6); capital É/Á/Ő are the retail's shorter accented capitals (they look like small "én"); Ű has no cell and folds to U.
  The TTF (Consolas/subtitles.ttf) covers all Hungarian letters.

## After (checked)
after/lab-007.00.png (1024x768, light wall: 4 answers with strips, hint below the panel), after/1080/lab.png (1920x1080), after/1440/burm.png (2560x1440, 118 char
question wrapped in two lines), after/story-005.00.png / story-010.00.png (subtitle on the lower bar), after/death-003.00.png (message and help line no longer collide).

## Open
* The outline is fixed in atlas pixels (about 1 px at 1024x768, 1.7 px at 1440p); the retail +3 s shadow of dialogue lines gets a doubled look at 1080p+ (retail-faithful).
* Credits/slides and the menu labels were not changed. The TTF prompt does not scale with the UI scale (looks small at 1440p).
* No bright outdoor level starts in a light place (all start scenes are dark); the light-wall lab corridor (rh12-lab1) was the brightest background available for the captures.
