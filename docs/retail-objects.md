# Retail object classes: what the game does, and what the remake does

Every class registered by `object.lto` (35 game classes + the engine light classes) and every kind that occurs in the 29 exported levels
(`output/<world>.scene.json`; the two other worlds, `outro` and `katscena`, hold at most 4 `o_obiekt`, 7 `d_sprite`, 7 lights and one `o_cutscene`, and `katscena` is a DAT v70 file the
exporter does not read). `output/<world>.gameplay.json` only repeats subsets of these kinds (`o_postac`, `o_emiter_postaci`, the `o_marker_*`, `b_door`, `b_szuflada*`, `o_cutscene`, `o_item_podnoszony`).
Addresses: `object.lto` server class = `L`, `cshell.dll` client = `C`. Companion documents: docs/retail-props.md (o_obiekt), docs/retail-missions.md (chains, markers, NPC rules),
docs/retail-decorations.md (d_* classes, clock hands, rotors), docs/retail-inventory.md (item grid), docs/retail-audio.md.
Status: **Y** implemented like retail, **P** partial (what is missing is listed), **-** nothing to implement (retail does nothing / never instantiated), **other** owned by another worktree.

## How the classes talk to the client
The server classes mostly do not draw anything. Three channels exist: (1) engine objects created in `PreCreate` (world models `b_*`, sprites); (2) text files the server appends to and the
client re-reads at fixed level frames (`scripts\cs\ajtemsy.txt` items at frame 0x25 [C 0x1001bd80], `sprajty.txt` + `beamsy.txt` at 0x3d [C 0x100027b0 / 0x100012f0], `obdrzekty.txt` props [C 0x1002bb24],
`facety.txt`, `emitersy_facetuf.txt`, `lensflare.txt`, `dialogs`; the client truncates all of them at level load, C 0x1005af60, and numbers are written with 2 decimals, L 0x1001b680);
(3) SFX messages (8-bit id + payload) dispatched by C 0x1005b1f0: 0x23 cutscene, 0x26 zmienna, 0x27 kill by id, 0x28 lens flare, 0x29 toggle by id, 0x2a marker_death, 0x2b detector, 0x2c pauza,
0x2e dialog, 0x2f/0x30 no-walk / no-respawn, 0x33 door watcher, 0x38 positional sound, 0x65 world props, 0x66 level change, 0x67 light group colour, 0x6d/0x6f/0x70/0x75 effects.
Client-to-server: 0x21 use, 0x27 prop died, 0x29 / 0x33 activate by id, 0x32 position, 0x34 NPC point (opens doors).

## Class table
| class | instances / levels | retail behaviour (server L / client C) | remake | status |
|---|---|---|---|---|
| `o_obiekt` | 3287 / 29 | client prop manager: solid models (block the player and NPCs, stop bullets), bullet + melee damage and shove, death sound / wreck / explosion / debris, per-piece render styles, use-key animations (`sound01/10` radius 1536), chains (docs/retail-props.md) | `props.rs`, `models.rs`, `activation.rs`, `weapons_alt.rs` (melee) | Y |
| `o_item_podnoszony` | 617 / 26 | server resolves `Rodzaj_item` against items.txt, dumps it to ajtemsy.txt and deletes itself; client item manager: box pickup, action-key cube, ammo caps, flight (below) | `pickups.rs`, `inventory.rs` | Y |
| `b_door` | 309 / 28 | hinged door, accelerating swing + wobble, `Od_gracza`, auto-close watcher, NPC opening, level exits (below) | `doors.rs` | Y |
| `b_szuflada` | 96 / 17 | sliding leaf (drawer, barrier, lift door), linear `Predkosc` units/s, solid while moving | `doors.rs` | Y |
| `b_szuflada_przestrzelna` | 14 / 6 | same movement; user flag 0x2 lets bullets and NPC sight through; never usable by the use ray (registry kind 9) | `doors.rs` | Y |
| `b_transparent` | 384 / 28 | static brush, `Alpha`, `Kolor` tint (`Uzyj_kolor`), `Additive`; solid when `Solid`; bullets and NPC sight pass (user flag 6) | `main.rs` material, `doors.rs` static solid | Y |
| `b_transparent_nieprzestrzelny` | 82 / 14 | same, but blocks bullets and NPC sight (user flag 4); `Solid 0` = ignored by everything | `main.rs`, `doors.rs` | Y |
| `b_rotator` | 7 / 4 | absolute rotation / slide by `sin(phase)`, solid (flags 0x2101) | `decorations.rs` | P (not solid; angle signs follow the mirrored render) |
| `b_wskazowka_*` | 12 / 4 | clock hands, absolute rotation from the system clock | `decorations.rs` | Y |
| `b_winda`, `b_winda_drzwi1/2`, `b_winda_przycisk` | 0 in all levels | elevator platform + two sliding doors + call button (state machine L 0x10005be0; `winda1/2` in lab levels are plain `b_szuflada`) | - | - (never placed; spec in "Unused classes") |
| `d_sprite` | 809 / 25 | sprites, 769 become client nodes | `decorations.rs` | Y |
| `d_lens_flare`, `d_emiter_dymu`, `d_emiter_opadu`, `d_emiter_iskier`, `d_fala3d` | 6, 3, 3, 2, 1 | see docs/retail-decorations.md (`d_emiter_iskier` is inert in retail) | `decorations.rs` | Y |
| `d_odglos` | 59 / 6 | **silent**: the server writes `scripts\cs\dzwienki.txt` and deletes itself after 0.1 s; the client only truncates that file, nothing reads it (C 0x1005b00a is the only reference) | none needed | - |
| `d_cien_chmur` | 0 | cloud shadow: deletes itself after 3.1 s, sends nothing | - | - |
| `o_postac`, `o_emiter_postaci`, `o_marker_niechodzenia/nierespawnu_postaci` | 800, 135, 65, 6 | NPC actors, emitters, no-walk / no-respawn zones | `npcs.rs` | other (wt-ai) |
| `o_marker_wykrywacz_postaci`, `o_marker_dialog`, `o_marker_zmienna` | 81, 36, 18 | player-volume detector / dialogue / variable markers (docs/retail-missions.md) | `activation.rs` | Y |
| `o_marker_death` | 23 / 2 | damage zone `-Sila*dt` through ChangeHealth (painkiller factor applies, C 0x1002ca2c) | `activation.rs` | Y |
| `o_marker_rozprysk` | 0 | splash marker; the server parses `Rodzaj_efektu` and discards it, no client code | - | - |
| `o_detektor_konca_levelu` | 0 | radius detector that sends the 0x66 level change every 0.1 s; unused | - | - |
| `o_promien_energii` | 5 / 2 | energy beam: strands of jagged additive ribbons, starts off, toggled by chain, `Dlugosc_dzialania` self-off, no damage, no sound | `activation.rs` | P (flash sprite `Blik_emitera` and the exact jitter rule are approximate) |
| `o_pauza` | 2 / 1 | countdown that fires `Nast_obiekt` once | `activation.rs` | Y |
| `o_cutscene` | 4 / 3 | starts a scenki.txt cutscene | `opening.rs` | other (wt-scenes) |
| `Light`, `ObjectLight`, `DirLight`, `StaticSunLight` | 2022, 526, 65, 19 | editor stubs (base handler only); the baked world lighting is in the DAT, `ObjectLight` / lights with `LightObjects` light models | `lighting.rs` | P (model lights only; world lighting is baked, wt-visual) |
| `LightGroup` | 3 / 3 | `StartOn`, `StartColor`, `CzestoscMigania` flicker (hard square wave, dark first), toggled by chain, killed by `Death_nast_obiekt` (below) | `activation.rs`, `lighting.rs` | Y |
| `StartPoint`, `WorldProperties`, `DemoSkyWorldModel` | 29, 26, 7 | spawn, fog / sky settings, sky worlds | `main.rs`, `decorations.rs` | Y |

## b_door (L vtable 0x10025080, message fn 0x100015f0)
* PreCreate: world model, flags 0x2001 (visible | solid), FLAGS2 0x40 (translucent so `Alfa` works). `Obrot` (deg -> rad), `Predkosc_obrotu * 0.034906585` = **twice** the value in degrees per second, `Gracz_otwiera`, `Od_gracza`, `Przesuniecie_osi`, `Os_obrotu`, `Alfa`, `Czas_samozamkniecia`, sounds, `Skok_do_levelu`, `Nast_obiekt`. `Nazwa_portalu` is read and never used; `Kolor1/2` only with `Uzyj_kolor` (0 in all data).
* Motion (L 0x10001ac0, per server frame, no clamp of dt): opening `ang += (|rate| + acc) * dt; acc += 8 dt` until `ang > |Obrot|`, then a damped sine wobble (`ang = |Obrot| + sin(phase) * acc * dt`, `phase += 16 dt`, `acc -= 8 acc dt`) until `acc <= 0.02`; closing mirrors it and swings slightly past closed. States -3 closed, 1 opening, 2 wobble, 3 open, -1 closing, -2 wobble. A 90 degree door with `Predkosc_obrotu` 45 reaches its stop on frame 28 and rests on frame 67 at 60 fps (1.12 s; 0.47 s to the stop), overshoot 4.3 degrees. A speed of 0 still moves (acceleration). The door stays solid throughout. Rotation about the world pivot `Pos + Przesuniecie_osi`.
* Activation (L 0x100020c0, byPlayer flag): refused while moving; refused for the use key / NPCs when `Gracz_otwiera` is 0 (chains and scripts always pass); `Skok_do_levelu` set: sends message 0x66 (level change, loading image) and never moves; closed -> opening, open -> closing; every non-refused attempt resets the auto-close timer.
* `Od_gracza` (249 of 309 doors): on closed -> open the sign of `Obrot` is chosen from the activator's side so the leaf swings away (formula with the original quirks in `doors::od_gracza_negative`, verified by running the x87 code; 13 doors with a zero hinge offset always open negative).
* Auto-close: server side only sets a request once the door is open, `Czas_samozamkniecia` is non-zero (the value does not matter) and more than 2.0 s passed since the last activation attempt; the client watcher (C 0x1002ff40) fires the close when the player is >= 128 and no living `ruchomy` actor is < 192 units from the door origin. A door with `Gracz_otwiera` 0 never closes by itself.
* NPCs (C 0x10042058 -> msg 0x34 -> L 0x10009270): every living actor whose phase runs an `estimate_*` search sends its position + 32 up each update, and on a node arrival next to a door link (C 0x100485d1). The server walks its chain registry from the head (0x10008a40 inserts new entries at the head: **newest placed object first**), skips everything but `b_door` (kind 1) and level exits (`Skok_do_levelu`), and stops at the FIRST door whose origin is within 128 units of the point. That door opens only if it is closed and has `Gracz_otwiera` (0x10002050 -> 0x100020c0 with byPlayer 1, activator = the point); if it is open, moving or locked the request does nothing - **a request never closes a door and never reaches the second leaf of a double door** (the partner 40 units away is never the first match). The follow-up activation by registry name (0x10009350, arg 1) meets the moving door and is refused, so `Nast_obiekt` does **not** follow an opening by an actor (70 doors of 16 levels have a chain, e.g. every leaf of the burmistrz1 double doors names its partner). Drawers are never opened by NPCs. Port: `doors::request_target`, `doors::npc_open`, `Activation::npc_requests`.
* Use ray (L 0x10011880): thin segment of 128 units from the eye, no padding; only `b_door` and `b_szuflada` are usable. Level exits: 18 doors, 3 of them (`secret`, `hujjj`, `hujek00`) have `Gracz_otwiera` 0 and are reached by chains only.
* Remake: `doors.rs` (`Door`, `advance_hinge`, `activate`, `wants_close`, `auto_close`, `npc_open`), dispatcher in `activation.rs` (`manual` queue = byPlayer). Tests: timing (28 / 67 frames), close symmetry, `Od_gracza`, refusal rules, auto-close, level-exit probes. Capture: `MESTER_TEST_SCENARIO=door MESTER_TEST_DOOR=b_door12 MESTER_TEST_ANGLE=0|180` (leaf swings away from either side).

## Drawers, barriers, lift doors (`b_szuflada*`, L 0x100038e0 / 0x10004370)
Linear: `dist += dt * Predkosc` (native units per second) up to `|Przesuniecie|`, no acceleration or wobble, solid while moving, duration `|P| / Predkosc` (160 / 16 = 10 s, 100 / 96 = 1.04 s). 7 drawers have `Predkosc` 0 and never finish. `Czas` (0 in all data) would toggle periodically. Same `Gracz_otwiera` rule; no `Od_gracza`.

## Transparent brushes (`b_transparent*`, L 0x10004c90 / 0x10005250)
Flags 0x2000 only when `Solid`; no Update handler (static). Bullet / NPC sight filters skip objects with user flag 0x2 (C 0x100578a0, 0x10057940, 0x100579f0): `b_transparent` (flag 6) and `b_szuflada_przestrzelna` are shot through, `nieprzestrzelny` (flag 4) blocks. 75 of the 384 `b_transparent` have `Solid` 0 (pure visuals). Windows and glass are these brushes: retail glass cannot be shot to pieces.

## Pickups (`o_item_podnoszony`)
* Server: reads `Rodzaj_item`, finds the block in items.txt (1-based ordinal = id), appends id + position + quaternion to ajtemsy.txt and deletes itself; 22 items in rh10-wiezowiec3 have an empty kind and resolve to a non-existing id (dead objects, skipped). No respawn, no client-to-server message.
* Client rules (C 0x10022610 every frame, 0x10022360, 0x10022470, 0x1001c100): the player box overlaps the item box -> taken automatically; the 64-unit cube 64 units ahead of the eye is tested **only while the action key (E / Space, control 0x1b) is held** and the inventory panel is closed (0x1005faa0 <- 0x100605e3). Thrown items cannot be taken in flight.
* Take: ammo refused (silently, the item stays) while the pool is at its cap (170, 60, 150, 250, 300, 60, 200, 20, 12, 8); a carried weapon only takes the rounds (also refused at the cap; a melee weapon without `ammo_index` uses pool 0, so a second nightstick is refused while Glock ammo is 170); everything else stacks or takes a grid cell (no carry limit). Pickup sound `pickup_sound` (2D), HUD icon for 1.5 s; no text message. Items do not rotate, bob or glow.
* Flight (thrown from the panel or dropped by an NPC, C 0x10021f90): integrate first, gravity 640 afterwards, a blocked step drops the horizontal speed, the next blocked step lands the item on the floor (640 ray); no bounce. An NPC's weapon starts at the actor with velocity (0,-64,0) (C 0x10042f95..0x1004306c), lives 300 s and is removed once that is over and the player is 1280 units away (C 0x100222c0).
* 39 item blocks; amounts: ammo 17 / 20 / 30 / 50 / 50 / 20 / 50 / 20; health 5..50 (`medpack` 50, `healing kit` 25, apple 5 ...), alcohol / power-up / painkiller values per item (see items.txt); `Golden cat` cannot be dragged. Never placed: item 11 (Remote bomb), 38 (heli_bron), 39 (papieros).
* Remake: `pickups.rs`. Probe: `MESTER_TEST_SCENARIO=pickup MESTER_TEST_ITEM=medpack [MESTER_TEST_HOLD_E=0|1]` (item stays without E, is taken with E).

## LightGroup (L 0x1000ff90, apply 0x10010200)
`StartColor` / 255 is sent to the engine as SFX 0x67 while the group is on (with `CzestoscMigania` > 0 only in odd half periods; the phase starts dark). Activation toggles `on` (a dead group still returns 1 so the chain continues), death (`Death_nast_obiekt`) switches it off for good. Only 3 groups exist (burmistrz2 `LightGroup10` white, podziemia1 `LightGroup0` green, podziemia1c `LightGroup0` grey, flicker 3 s). World lighting is baked into the DAT, so in the remake the groups colour the model lights (`SourceLights::set_group_level`).

## Unused classes (0 instances in the 29 levels; implemented as "nothing" on purpose)
* `b_winda` (L 0x10005be0): platform sliding `Przesuniecie` at `Predkosc`, `Opoznienie_startu` delay, children (button, two doors) found by name, state machine -3 rest, call -> -2 doors forced closed and toggled -> after the delay -1 moving -> -4 doors -> -5 wait -> 3; `b_winda_drzwi1/2` sliding leaves, `b_winda_przycisk` use-ray button (plays `Dzwiek`). The lab levels' "winda" are `b_szuflada` chains.
* `d_cien_chmur`, `o_marker_rozprysk`, `o_detektor_konca_levelu` as in the table.

## Probes (all headless: `MESTER_SILENT=1`, capture arguments, `timeout 120`)
`MESTER_TEST_SCENARIO=props MESTER_TEST_PROP=<instance|definition> [MESTER_TEST_GUN, _DISTANCE, _INTERVAL, _WALK=1]` shoots (or walks into) a prop and logs hit points / position; `=door MESTER_TEST_DOOR=<name> [_ANGLE, _WALK=1]`; `=pickup MESTER_TEST_ITEM=<kind>`; all in `props_probe.rs`.
