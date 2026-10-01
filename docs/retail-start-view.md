# Retail start view and the "policeman on the other side" report

Question (owner, from the 2007 retail video): in the prison level right after the prologue the policeman stands somewhere else than in
the remake and takes the player to the common room the other way. Different game version, or a remake bug? Round of 2026-09-30, branch
`agent/start`. Verdict: **no bug in the current tree, no version difference**; the video frame is reproduced with the display mirror on and the
StartPoint `Kierunek` yaw, both are proven by two independent retail frames, and regression tests pin them.

## 1. Which level and which moment
* The level is **rh1-wiezienie2** ("A Lázadó"), not rh1-wiezienie1. rh1-wiezienie1 is the intro cutscene level (`intro`, then `intro zwei`,
  whose last phase runs `worlds\rh1-wiezienie2`, scenki.txt); the player wakes in his cell in rh1-wiezienie2 (StartPoint0 (124,13,938)).
* The subtitle of the video frame is **`A fenébe! Ki kell jutnom innen.`** = text key `Fight02T` (text_keys.txt line 584; the low-resolution
  frame reads like "A főnök: ... jutnod", but no retail text contains "jutnod" or a speaker "A főnök"). It is played by `o_marker_dialog1`
  (MasonStart1) at **(80,-3,768)**, extents 32, which the player crosses when he leaves the cell (gameai.txt, first-missions-research.md).
  So the frame is: the hero just stepped out of the cell into the barred walkway, looking at the hall; the blur/tilt is the drugged start
  effect. The policeman on the right is the cell guard `o_postac14` (`straznik przy celi`) at (-80,-3,672), holding the nightstick.
* The cell guard's whole retail behaviour (postacie.txt, gameai.txt 815-870): `nuda` (stands, `stoi_z_pala`); once he sees the player and
  more than 3 s passed: dialogue `StraznikPrzyCeliOstrzega` (`Őr: Menj a közös terembe, mielőtt szétrúgom a segged!`), then phase `do_gracza`
  (`estimate_do_gracza`, speed 48, stop 24) **towards the player**, then `atakuje` / `atakuje1` (`odepchnij_gracza` = a 40 damage blow, cshell
  0x10042460 has no impulse, no dragging). **The policeman never leads the player anywhere**; "go to the common room" is only the spoken
  warning. The common room (`wiezien bije` fight, (872,-222,-751)) and the stair guard `straznik na klatce` (726,-5,622) are on the +X side.

## 2. The 2x2 test {display mirror on/off} x {Rotation yaw / Kierunek yaw}
Mirror off is the horizontal flip of the mirror-on capture (the mirror is exactly a final-image flip, mirror.rs).
* `captures/start/grid-start-2x2.png` (rh1-wiezienie2 start, 3 s): top row mirror on (left Kierunek yaw = current, right Rotation yaw = old), bottom
  row the flips. Kierunek `poludnie` faces -Z straight at the cell door (hall and the distant policeman in the doorway, bed and table symmetric):
  a designed opening shot; the Rotation yaw (150 deg) looks at the corner beside the door.
* `captures/start/compare-video-new-old.png`: video frame | current build at (100,13,700) yaw 0.7 | the stale pre-mirror build of the coordinator
  seed (`mester-start-seed.exe`, older than commit 42abde7) at the same pose. Video and current build: tall windows and railing on the left, cell
  block, ceiling lamp and the policeman on the right. The pre-mirror build shows the mirror image (windows right, policeman left).
  `captures/start/montage4.png`: six poses around the marker, all with the policeman on the right and the hall on the left.
* `captures/start/grid-miasteczko0-2x2.png`, the second frame (research/retail-reference/retail-rain-street-lamps.webp, the rh3-miasteczko0
  start): only {mirror on, Kierunek yaw} shows the kerb running from the lower left with the lamps on the left and dim lights on the right;
  the Rotation yaw (0 = north) faces a black wall, mirror off puts the kerb on the right. **Both retail frames select the same combination**
  (and the second one also proves the Kierunek yaw, which the cell frame cannot, because the player turns freely there).
* The pick aid confirms the mapping numerically: at (80,13,768) looking along -Z (LithTech right = -X) the pixel at the displayed left maps to
  x = +239, the displayed right to x = -79.

Conclusion for suspects (a) and (b): the display mirror (LithTech is left-handed, +X right, +Z forward) and the `Kierunek` yaw are both right.
The owner's mismatch is the mirror image of the retail view: a build older than the display mirror (2026-09-29 15:24, commit 42abde7) shows the
level flipped left-right (policeman left, route mirrored), and one older than the Kierunek yaw (14:24, 6860489) starts with the Rotation yaw.
The current tree (main cf46114 and later) has the retail sides. Suspect (c) (NPC route/AI) is not a cause, see section 4.

## 3. Evidence for the start yaw
* cshell 0x1005b6df: the start message carries the position and one integer; `1 -> pi/2`, `2 -> pi`, `3 -> 3pi/2`, otherwise 0 is stored at
  controller+0x58 (yaw); the StartPoint `Rotation` (read by object.lto 0x1001146a through `[edx+0x28]`) is not sent.
  object.lto 0x100114a2..0x10011548 maps the strings `wschod` = 1, `poludnie` = 2, `zachod` = 3 (`polnoc` = 0).
* Level data: of 30 exported levels 24 have Rotation 0 (and `Kierunek` set, except katscena) (the property the designers used). Where both are set they agree
  in podziemia1 (180 / poludnie), rh1-wiezienie3 (181 / poludnie) and rh2-wiezienie1 (91 / wschod), which fixes the sign and the quarter mapping
  (LithTech yaw 0 faces +Z, +pi/2 faces +X). The three levels where a leftover Rotation disagrees are rh1-wiezienie1 (150 / polnoc),
  rh1-wiezienie2 (150 / poludnie) and rh2-wiezienie2 (339 / wschod); the retail code never reads it.
* The 10 levels whose first view changed (start position, `Kierunek`, facing): podziemia1b (324,-368,1116) zachod -X; rh1-wiezienie1 (4028,125,698)
  polnoc +Z; rh1-wiezienie2 (124,13,938) poludnie -Z; rh10-wiezowiec1 (1616,-1329,-1800) zachod -X; rh2-wiezienie2 (-2714,-446,-1144) wschod +X;
  rh3-miasteczko0 (-6045,3,268) wschod +X (the street of the second frame); rh3-miasteczko1 (1484,-304,1356) zachod -X; wiez_wn1 (-1778,-224,-456)
  wschod +X; wiez_wn2 (-1000,-56,352) wschod +X; wiez_wn3 (2316,408,116) poludnie -Z.

## 4. The policeman's route in the remake
`MESTER_TEST_SCENARIO=cell MESTER_TEST_TRACE=o_postac14 MESTER_TEST_END=25` logs the guard twice a second (real start, ordinary movement to the
marker, no cheats): he stands at (-80,1,672) `nuda` until the warning at 7 s, then walks **+X along z = 672** (phase `do_gracza`, about 48 u/s:
-73, -49, -27, -3, 16), stops (`nuda1`) and from t = 12 s strikes (`atakuje`, `atakuje1`) the player waiting at (79,2,695). This is the retail
phase script; there is no path-graph route (`estimate_do_gracza` is a straight point towards the player), no marker sequence and no escort.
Seen from the cell exit (facing -Z) the guard comes from the displayed right and the common room lies to the displayed left; a mirrored display
swaps both, which is the "other direction" of the report.

## 5. Regression net
* `cargo test` (level-viewer): `start_views_of_the_changed_levels_face_their_compass_heading` (all 10 levels: look vector and displayed right),
  `cell_guard_is_on_the_displayed_right_at_the_marker` (LithTech right vector and `mirror::right`), and the existing
  `start_point_direction_sets_the_yaw_and_ignores_the_rotation`; `cargo test -- --ignored start_table_matches_the_export` checks
  `start_view::STARTS` against the local export.
* Probe `startpose` (`python -m tools.run_probes start-<world>`, 10 runs of 0.5 s game time): the real level load must put the player on the
  StartPoint facing the `Kierunek` heading (forward vector compared, y within 60 units for the settling), including rh1-wiezienie1 with its opening.

## Open
* The video policeman looks lighter blue with white gloves, ours dark navy with flesh-coloured hands (skin `policjant.dtx`); the frame is
  over-exposed (walls washed out, green floor pale), so this is most likely the video, not a texture difference. Not verified further.
* The exact camera pose of the video frame is unknown (the player turns freely); the composition, the marker subtitle and the policeman / hall
  sides are reproduced, not the pixel position.
