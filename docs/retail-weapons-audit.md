# Weapon and damage model audit (cshell.dll, scripts/items.txt)

Every number below comes from the retail install: `scripts/items.txt` (parsed by cshell.dll at 0x10062180..0x10063f70, field offsets at the
end of this file), the disassembly (`python -m tools.inspect_retail START END`) and the exports (`output/retail_weapons.json`,
`retail_items.json`). Addresses are cshell.dll virtual addresses. The audited values are pinned by tests in
`crates/level-viewer/src/retail_weapons.rs` (`RETAIL_TABLE`; `cargo test -- --ignored` also re-reads `../GYARI/scripts/items.txt`).

**What exists in the retail game.** Player: nightstick, Glock, Smith & Wesson 625, SIG 551, MAC-10 Ingram, FN shotgun, M-14 (the only
scoped rifle), HK G8, P90 and the hand grenade. `heli_bron` is an HK G8 with `kul_na_raz 2` for NPCs only. `Remote bomb` (an empty
record) and `papieros` (a cigarette with the `weapon` flag but no carried model) are not usable weapons. There are **no silenced weapons,
no crossbow or other specials, no turrets and no mounted guns**. NPCs carry (postacie.txt): 6 Ingram, 6 Glock, 5 SIG, 4 nightstick, 4 M-14,
4 FN shotgun, 3 S&W, 3 P90, 1 HK G8, 1 `heli_bron`.

Status legend: **OK** verified against the code and matching, **FIXED** deviation found in this round and fixed (with tests),
**APPROX** deliberate approximation, **OPEN** not done.

## 1. Weapon table (retail value = remake value: the export is generated from items.txt and pinned by the tests)

| Item | title | sila_strzalu (player hit) | sila_wroga (NPC hit) | NPC hit on the player at D 0.33 / 0.67 / 1.0 | glosnosc | rozrzut | trace range | shot_latency (rate) | pellets | experience_index |
|---|---|---|---|---|---|---|---|---|---|---|
| Police nightstick | Gumibot | 40 | 0 | - | 0 | 0 | - | 0.3 s (3.33/s) | 1 | 0 |
| Glock | Glock | 30 | 15 | 6 / 13 / 19 | 1280 | 64 | 5248 | 0.3 s (3.33/s) | 1 | 1 |
| Smith and Wesson m. 625 | Smith & Wesson | 60 | 30 | 12 / 26 / 39 | 1640 | 96 | 4480 | 0.6 s (1.67/s) | 1 | 2 |
| Sig 551-p/SWAT | SiG | 25 | 10 | 4 / 8 / 13 | 1280 | 128 | 3712 | 0.05 s (20.00/s) | 1 | 3 |
| MAC-10 Ingram | Ingram | 20 | 12 | 5 / 10 / 15 | 1640 | 180 | 2464 | 0.03 s (33.33/s) | 1 | 4 |
| FN shotgun | Vadászpuska | 30 | 8 | 3 / 6 / 10 | 1640 | 192 | 2176 | 1 s (1.00/s) | 10 | 5 |
| M-14 | M-14 | 120 | 60 | 25 / 52 / 78 | 1640 | 0 | 6784 | 0.6 s (1.67/s) | 1 | 6 |
| HK G8 | HK-G8 | 25 | 20 | 8 / 17 / 26 | 1640 | 128 | 3712 | 0.06 s (16.67/s) | 1 | 7 |
| P90 | P90 | 20 | 20 | 8 / 17 / 26 | 640 | 64 | 5248 | 0.04 s (25.00/s) | 1 | 8 |
| Hand grenade | Gránát | 0 | 0 | - | 0 | 0 | - | 0.6 s (1.67/s) | 1 | 0 |
| heli_bron | heli_bron | 10 | 10 | 4 / 8 / 13 | 1280 | 160 | 2944 | 0.06 s (16.67/s) | 2 | 7 |

The trace range is `640 + 24·max(256 − rozrzut, 0)` (0x10005dde); the shotgun rows count per pellet. `shake_screen` (Glock 5, S&W 7, SIG 5 (the
earlier `shake_screen 15` is overridden), Ingram 7, P90 2, FN 10, HK 10, M-14 15, heli 20) is only written into a camera vector that is
never applied (0x100051f6, 0x10055fa4): no recoil in retail and none in the remake (**OK**).

| Item | magazine (max_ammo) | rounds on pickup (ammo_amount) | ammo pool (ammo_index) | pool cap | ammo pickup | reload clips (s) | shoot clips (s) | weight |
|---|---|---|---|---|---|---|---|---|
| Police nightstick | 0 | 0 | - | - | - | - (clip `reload` does not exist) | do_uderza 0.1 + uderza 0.3 | 0.5 |
| Glock | 17 | 17 | 0 | 170 | Glock ammo +17 | reload 1.233 | strzal1 0.1 + strzal2 0.433 | 0.63 |
| Smith and Wesson m. 625 | 6 | 6 | 1 | 60 | Smith and Wesson ammo +20 | reload1 1.1 + reload2 1.733 | strzal 0.7 | 0.9 |
| Sig 551-p/SWAT | 30 | 30 | 2 | 150 | Sig ammo +30 | reload 1.433 | strzal 0.1 | 1.4 |
| MAC-10 Ingram | 50 | 50 | 3 | 250 | Ingram ammo +50 | reload 2.033 | strzal 0.1 | 1.4 |
| FN shotgun | 8 | 8 | 5 | 60 | Mossberg ammo +20 | reload0 0.5 + reload1 0.453 + reload2 0.5 | strzal 0.666 + strzal1 0.234 | 1.8 |
| M-14 | 20 | 20 | 7 | 20 | M-14 ammo +20 | reload 1.366 | strzal1 0.066 + strzal2 0.467 | 2.6 |
| HK G8 | 50 | 50 | 6 | 200 | HK G8 ammo +50 | reload 2.2 | strzal 0.1 | 2.4 |
| P90 | 50 | 50 | 4 | 300 | P90 ammo +50 | reload 2.133 | strzal 0.133 | 1.4 |
| Hand grenade | 1 | 1 | 8 | 12 | - | - | zawleczka 0.7 (pin), sam_rzut1 0.3 + sam_rzut2 0.18 | 0.6 |
| heli_bron | 50 | 250 | 6 | 200 | HK G8 ammo +50 | reload 2.2 | strzal 0.1 | 1.4 |

Ammo pickups add their `amount` to the pool (0x10003d10) and are refused while the pool is at its cap (0x1001c187, table 0x10066248 =
170, 60, 150, 250, 300, 60, 200, 20, 12, 8); a weapon picked up for the first time is loaded with `ammo_amount` rounds, a second copy only
adds them to the pool (0x1001c25b). The pools are not emptied by a level change (only by `tnijitems`). **OK** (pickups.rs, retail_state.rs).

| Item | sound_shoot | sound_reload | HUD frame | przes right/up/forward | skala x/y/z | flags |
|---|---|---|---|---|---|---|
| Police nightstick | swist.wav | - | ramka_ammo_pauka.dtx | -1.6 / -9 / 1.8 | 0.46 / 0.4 / 0.36 | melee |
| Glock | glock_s.wav | glock_m.wav | ramka_ammo_glock.dtx | -2.1 / -14 / -0.8 | 0.56 / 0.5 / 0.46 | - |
| Smith and Wesson m. 625 | s&w_s1.wav | s&w_m.wav | ramka_ammo_snw.dtx | -2.2 / -16.4 / -4.7 | 0.7 / 0.63 / 0.57 | - |
| Sig 551-p/SWAT | sig_s.wav | sig_m.wav | ramka_ammo_sig.dtx | -1.6 / -13 / 3.6 | 0.6 / 0.45 / 0.34 | socket_alt_latarka, socket_niesiony_latarka, mnoznik_latarki 2 |
| MAC-10 Ingram | ingram_s.wav | ingram_m.wav | ramka_ammo_ingram.dtx | -2.6 / -10 / 1.5 | 0.56 / 0.5 / 0.46 | - |
| FN shotgun | mosb_s1.wav | mosb_m.wav | ramka_ammo_mossberg.dtx | 4.6 / -11.5 / 1.2 | 0.56 / 0.5 / 0.36 | shotgun, duzy_bryzg |
| M-14 | m14_s.wav | m14_m.wav | ramka_ammo_m14.dtx | 3 / -14 / 2 | 0.46 / 0.4 / 0.36 | duzy_bryzg, alt_zoom, socket_laser, socket_niesiony_laser |
| HK G8 | hk_s.wav | hk_m.wav | ramka_ammo_hkg8.dtx | 4 / -16.5 / 7.2 | 0.66 / 0.6 / 0.26 | - |
| P90 | p90_s.wav | p90_m.wav | ramka_ammo_p90.dtx | 2.6 / -11 / -1.6 | 0.52 / 0.47 / 0.42 | socket_laser, socket_alt_latarka, socket_niesiony_latarka |
| Hand grenade | zawleka.wav | - | ramka_ammo_grenade.dtx | -3.6 / -12.5 / 0.6 | 0.56 / 0.46 / 0.46 | grenade |
| heli_bron | hk_s.wav | hk_m.wav | ramka_ammo_hkg8.dtx | 4.6 / -14.5 / 4.6 | 0.46 / 0.4 / 0.26 | - |

Cue names are the retail `sounds\weapons\*.wav`; the remake never plays them under `MESTER_SILENT=1` (`MESTER_AUDIO_LOG=1` logs them). The
P90's `alt_sound_on/off` lines do not match the parser keys (`sound_alt_on/off`), so no laser or zoom sound exists (**OK**).

## 2. Damage model

| Rule | Retail evidence | Remake | Status |
|---|---|---|---|
| Player bullet damage | `sila_strzalu` (+0xb28, integer) flat: 0x10006b00 -> 0x10043090 subtracts it from HP (+0x26c); the NPC dies when HP < 0 | `Definition.damage` per trace | OK |
| Hit location, distance falloff, material penetration, NPC armour | none: 0x10043090 has no location, distance or armour term; the trace stops at the first object | flat damage, first hit stops | OK |
| Shotgun | `kul_na_raz` (+0xb4c, default 1) independent traces per shot, each the full `sila_strzalu` (0x10005ea4) | 10 pellets x 30 | OK |
| Spread | endpoint jitter `(rand%1000 - rand%1000)·s·0.001` per axis, `s = rozrzut·(1 − 0.0075·skill)`; no crouch, movement or scope terms (0x10005fb7) | same, MSVCRT `rand` LCG | OK |
| Weapon skill | +0.1 per bullet hitting a living character while below 99.8 (0x10006abe) | `Stats::record_hit` | OK |
| Kill experience | `exp_gained` of the victim when the player's shot kills it (0x10006b49) | campaign.rs | OK |
| NPC bullet on the player | `trunc(sila_wroga·D·1.3)`, D = 0.33 / 0.67 / 1.0 (0x10006c26); ray length capped at 1.5x the distance to the player; spread `rozrzut·(2 − D)` | `enemy_bullet_damage`, `enemy_shot_endpoint` | OK |
| NPC shotgun and `heli_bron` pellets | the same trace call loops `kul_na_raz` times (0x10005ea4): 10 pellets for an NPC FN shotgun, 2 for the helicopter gun | one trace per NPC shot | FIXED: every pellet is scattered and traced, one muzzle flash |
| NPC bullet meets another character | the trace stops at the first object; another NPC takes the raw `sila_wroga`, without D or painkiller (0x10006b16) | bullets ignored other actors | FIXED: `simulate_with_barriers` strikes the nearest actor box in front of the player (blood, tracer, wound stimulus) |
| Player health change | 0x10061b90: ignored while the level is younger than 60 frames (shell +0x37c), otherwise `health += d·(100 − min(painkiller,100))·0.01`, clamped to 0..max, hurt timer, heartbeat below 50 and 20 | pain factor and heartbeat existed, no 60-frame rule | FIXED (campaign.rs, `ViewState::level_frames`) |
| Armour | none in the game (`pain killer` items are the only reduction) | - | OK |
| Hit direction | 0x1005fe50 passes the attacker bearing to the HUD (0x10034140); blood puffs, `krewmonitor.spr` rolled toward the attacker, `wcialo.wav` | `gunfire::player_hit`, `BloodOverlay` | OK (APPROX: the overlay is a UI image, docs/retail-gunfire.md) |
| Camera recoil and kick | none (`shake_screen` unused) | none | OK |
| Melee | nightstick 40; hit test 0.1 s after the swing starts at 24/32/48/56/64 units, every NPC box holding a point (0x1000f420) | `NpcRoster::melee_hits` | OK |
| Grenade blast | NPC: radius 640, `(640 − d)·0.78125` (500 at the centre), ray from Y+16 (0x10043180); player: radius 640, `(640 − d)·0.15625` (100), ray from Y+32, painkiller applies (0x1005fd20) | same | OK |
| Grenade fuse | 5.0 s in hand (counted from the end of `zawleczka`), a thrown grenade gets a fresh 3.0 s (0x1004f2c7) | same | OK |
| Grenade throw | leaves 52 units ahead and 8 units to the right of the eye with `forward·640 + up·256`; a wall on that segment makes it explode at once (0x1000afd9..0x1000b08e); held too long it explodes at the `granat` socket (0x1000adc6) | 12 units ahead, no wall test | FIXED |
| Last grenade | after `sam_rzut2`, with magazine and pool empty, the item is removed from the inventory and the hands are empty (0x1000b142..0x1000b173; the call to 0x1005ab50 is a stub) | the item stayed with 0 rounds | FIXED (`grenade_spent` also removes it from the grid) |
| Damage per difficulty | e.g. Glock 6 / 13 / 19, SIG 4 / 8 / 13, M-14 25 / 52 / 78 (table 1) | `RETAIL_TABLE` tests | OK |

## 3. Fire, reload and weapon-change state machine (weapon object at 0x1006efb8)

States (+0x65c): 0 idle, 1 first reload clip, 2 second reload clip, 3 `anim_shoot0`, 4 `anim_shoot1`, 5 grenade pin, 6 grenade held, 7 throw,
8 recover, 9 weapon lowering, 10 weapon raising. The per-frame update is 0x100100c0, fire 0x10003f90, reload 0x10003d40, select 0x100034a0.

| Rule | Retail evidence | Remake | Status |
|---|---|---|---|
| Fire input | held action 6 calls fire every frame, no edge: every non-grenade weapon repeats at `shot_latency`, pistols included (0x10060caa..0x10060ccf) | `automatic` for all | OK |
| Shot timer | the timer (+0x694) must reach `shot_latency` in states 0, 3, 4; it restarts on every attempt, also on an empty magazine (0x1000403a) | cooldown | OK, FIXED for the empty attempt |
| Empty magazine | the trigger starts the reload; without reserve nothing else happens (no click sound, the `noammo`/`strzal_noamo` clips are never referenced) | needed the R key | FIXED (`start_reload` from the trigger) |
| Reload preconditions | reserve > 0 and magazine < `max_ammo` (0x10003d75, 0x10003d89); **no weapon-state check at all**; the key is polled level triggered every frame, after the trigger (0x10060c9a fire, 0x10060d08 reload), and without the inventory check the fire and select keys have | level triggered (`controls.reload = pressed`), evaluated after the trigger in the same tick, allowed in every state | FIXED: a held R starts the reload right after each shot, restarts a shotgun's shell cycle every frame (the real game cannot finish it while R is down) and leaves a full magazine alone |
| Reload during a weapon change | 0x10003d40 sets state 1 over state 9 / 10: the change never finishes (the old weapon stays in hand; while rising, the new one), and the dip counter +0x6f0 keeps its value, which 0x100106ca uses in every state: the view model stays lowered (and pitched) until the next change | same (`Inventory::frozen_dip`, reset by the next `select`) | FIXED (a retail glitch, reproduced on the owner's "perfect" request) |
| Reload ammo transfer | `min(missing, reserve)` moves **when the reload starts** (0x10003da3..0x10003dad); the clip `anim_reload0` (+ `anim_reload1` for the S&W) plays afterwards; no fire during states 1 and 2 | moved when it ended | FIXED |
| Shotgun (`shotgun` flag, +0x20b2) | no transfer at the start; after `reload0` each shell moves one round, then plays `reload1` and `sound_reload`; primary held or a full magazine or empty pool ends with `reload2` (0x1000ffb0) | same, plus a `sound_reload` per shell | OK, FIXED (sound) |
| Reload durations | Glock 1.233 s, S&W 1.1 + 1.733, SIG 1.433, Ingram 2.033, M-14 1.366, HK 2.2, P90 2.133, FN 0.5 + N x 0.453 + 0.5 (table 2) | from the LTB clips | OK |
| Weapon change | keys 1..8 -> holster cell (0x1001c060); refused in states 1, 2, 9, 10; otherwise the old weapon lowers (`progress += 30·dt` until above 10, state 9), the new one rises (`progress -= 30·dt` to 0, state 10): a third of a second each; the view model drops `progress` units and pitches `0.05·progress` rad (0x100106ca); no fire meanwhile (0x10003feb); with empty hands the new weapon is drawn at once from 10 | instant swap, only the raise | FIXED (`Inventory` switch phases; `view.weapon_dip` follows `Inventory::dip`) |
| Shot casing timing | the case leaves `socket_luska_strzal0` when the first shoot clip ends (0x100101ef) or when the next shot interrupts it (0x10004029); a weapon change or reload before that loses it (SIG/Ingram/HK/P90 about 0.1 s, M-14 0.066 s, shotgun 0.666 s after the shot) | ejected at the trigger pull | FIXED (`TickOutcome::eject_casing`, `gunfire::player_casing`) |
| Revolver reload casings | 0x1000a1d0 (called when the first reload clip of a two-clip weapon ends, 0x10010293) ejects six `luska.ltb` from `socket_luska_reload0..5`, 15 s | keys parsed, nothing ejected | FIXED (`gunfire::reload_casings`, `TickOutcome::eject_casings`; velocity axes APPROX) |
| Armed grenade and a change | a change starts from states 5..8 and cancels the throw (0x100034d2 only rejects 1, 2, 9, 10) | same | OK |
| Alt toggle on a change | the alt flag is toggled off when the change starts (0x100034fe) | at the start now; laser and flashlight hidden while switching | OK |
| Weapon pickup | the first copy comes loaded with `ammo_amount`; retail never calls the select function on a pickup (only the key handler 0x10060c8f and the save restore 0x1004cd45 do) | the remake selects a newly picked-up weapon | FIXED (Inventory::acquire does not select; hands are also emptied at every level start, docs/retail-items.md) |
| Wheel and Tab | no weapon-cycling code exists: the only caller of the select function is the loop over the actions Weapon1..Weapon8 (0x10060c6a..0x10060c8f; keys.cfg/autoexec.cfg bind only `1`..`8`); two other actions call 0x1005ab50, a bare `ret` in this build | wheel and Tab cycle the holster | REMOVED from main |

## 4. Noise and stimuli (AI alerting)

Every event creates a node `pos, near, hear, sight, life` (0x10045cf0, list at 0x10b35858) that lives one or two AI passes (0x10045da0). The
contact test of every actor (0x10049950 -> 0x10045df0) notices a node inside `near` (no line needed), inside `hear` with a clear ray
(0x10051c20), or inside `sight` within 60 degrees (horizontal) in front of it with a clear ray. A notice sets the contact flag exactly like
seeing the player (`on_kontakt`, then the "halt" shout). Distances are measured to the actor's position; cutscenes ignore stimuli.

| Event | Address | near | hear | sight |
|---|---|---|---|---|
| Any gunshot (player or NPC) at the muzzle origin | 0x10005dd9 | 128 | `glosnosc` (Glock 1280, S&W 1640, SIG 1280, Ingram 1640, P90 640, FN 1640, M-14 1640, HK 1640, heli 1280) | - |
| Bullet end point (every trace) | 0x10007e17 | 200 | - | 1024 |
| Character wounded by a bullet (96 above its feet) | 0x10043128 | 192 | 1024 | - |
| Character wounded by a melee blow | 0x10043128 | 64 | - | 1024 |
| Character killed (66 above its feet) | 0x10042cc6 | 256 | 1280 | 2048 |
| Character makes contact (halt shout) | 0x10049b38 | 128 | 640 | 1280 |
| Grenade explosion | 0x1005b165 | 196 | 2048 | - |
| Laser dot and flashlight pool | 0x1000ee32, 0x1000b7c0 | 0 | 0 | 640 |

Remake: `npcs::Stimulus` (`NpcRoster::noises`), raised by `retail_weapons::tick` (gunshot, explosion), `gunfire::shoot_outcome` (impact),
`NpcRoster::hit_scan_with`, `melee_hits`, `damage_actor` (wound, death) and the contact branch (shout); NPC gunfire raises its own gunshot.
**FIXED**: before this round only the laser dot existed, its "within 16 units" rule was a misreading (both radii are the bit pattern 0x10,
which is zero as a float), and the player's shots alerted nobody. There is no silent weapon: the quietest (P90) is still heard at 640 units.

## 5. Presentation

| Item | Retail | Remake | Status |
|---|---|---|---|
| Muzzle flash, light, smoke, sparks, tracer, casing, impact, decals, debris, blood | docs/retail-gunfire.md | fx.rs, gunfire.rs | OK (approximations listed there) |
| View model offsets and scales | `przes_*`, `skala_*` (table 3), applied in the camera basis (0x10010635) | exported values | OK |
| Draw and holster animation | section 3 | inventory driven | FIXED |
| Idle, shoot, reload, throw clips | `anim_*` clips run in order, idle loops (`trzyma`) | `Inventory::pose` | OK |
| Sprint pose | none: only the `CWeaponBob` offset (docs/retail-camera.md) | same | OK |
| Sniper scope | `alt_zoom`: FOV x 0.15 at 5 rad/s, mouse x 0.25, reticle overlay, crosshair hidden; no sway, no breath holding, no accuracy bonus | weapons_alt.rs | OK |
| Laser sight and flashlight | docs/retail-weapons-alt.md; `mnoznik_latarki` (SIG 2, default 1) scales the shaft quads (0x1000b861) | shaft approximated | APPROX |
| Crosshair | one texture `celownik.dtx`, hidden only while scoped; no hit marker, no spread indication | hud.rs | OK |
| Ammo HUD | 0x10038640..0x10038d00: magazine as two digits and the pool as three digits, zero padded (a melee weapon shows `01` and `001`); a `pasek_ammo` bar of width 228·s·(magazine / `max_ammo`, melee and grenade full) at (782·s, 729·s) with s = width/1024, bitmap digits `misc\panel\ammo\fonts\0..9.dtx` | drawn by `hud.rs` (input agent: frame, bar, digits; `retail_weapons::NativeHud` stays hidden) | OK (values re-checked here) |
| Grenade blast art | `systemblikwybuch.spr` 0.45 s scale 4 and `systemwybduzy1.spr` 1.45 s scale 1.1 | same | OK |

## 6. Weapons and ammunition placed in the levels (from `<world>.items.json`)

RH9-fabryka: Ingram + 3 Ingram ammo, P90 + 4 P90 ammo. Rh7a-Tunele: nightstick, S&W (+2 ammo), P90 (+6), 2 grenades, 7 SIG ammo, 2 HK ammo, 1
shotgun ammo. burmistrz1: 2 Glock (+5 ammo), 2 S&W, SIG (+4), Ingram (+3), 4 grenades, 2 shotgun ammo. burmistrz2: SIG + 3 ammo.
chapel_mniejszy: Glock, 3 grenades, Ingram and SIG ammo. chinatown: Glock. knajpa: Glock, HK, Ingram, M-14, SIG (+ammo of all), 4 grenades.
podziemia1b: 1 grenade. podziemia1c: 4 Ingram ammo, 4 grenades. rh1-wiezienie2: FN shotgun, 1 shotgun ammo, 4 Glock ammo. rh10-wiezowiec1: FN shotgun,
ammo. rh10-wiezowiec2: M-14 + ammo, 1 grenade. rh2-wiezienie1: Glock, 6 SIG, 5 S&W ammo, 4 grenades. rh3-miasteczko1: Glock, M-14 and SIG ammo.
wiez_wn1: 32 nightstick. The other levels place none; weapons come from `receive` script commands and NPC drops (`drop_weapon`, the `weapon`
header, not with `nie_zostawiaj_gana`). Inventory, pools and the selected weapon carry over between levels; `tnijitems` empties everything. **OK**.

## 7. Field offsets of the item record (0x10062800..0x10063f70)

`shot_latency` +0x8fc, `max_ammo` +0x908, `ammo_index` +0xb0c, `ammo_for` +0xb10 (default 12 = none), `scale` +0xb14, `melee` +0xb18,
`grenade` +0xb1c, `alt_zoom` +0xb20, `ammo_amount` +0xb24, `sila_strzalu` +0xb28, `sila_wroga` +0xb2c, `hit_smuga` +0xb30, `glosnosc` +0xb34
(float), `przes_right/up/forward` +0xb38/0xb3c/0xb40, `mnoznik_latarki` +0xb44 (default 1.0), `shake_screen` +0xb48, `kul_na_raz` +0xb4c (default
1), `ile_sprite0` +0xb50, `rozrzut` +0xb54, `weapon` +0x8d8, `eaten` +0x8e0, `nie_ruszaj` +0x20b0, `duzy_bryzg` +0x20b1, `shotgun` +0x20b2,
`weight` +0x20b4, `skala_x/y/z` +0x20a4/0x20a8/0x20ac, `hit_sprite_skala` +0x209c, animations from +0xb58 (`anim_base`, `anim_shoot0/1` +0xbd8/0xc58,
`anim_reload0/1/2` +0xed8/0xf58/0xfd8, `anim_trzyma` +0x1058), sockets from +0x10d8, `sound_shoot` +0x19d8, `sound_reload` +0x1a58, `hit_sound` +0x1ad8.

## 8. Verification

- Unit tests: `retail_state.rs` (reload timing, auto-reload, switch phases, shotgun, grenade incl. the spent item), `npcs.rs` (stimuli, hearing
  through walls, contact and shout chain, wound and death stimuli, NPC pellets, friendly fire), `retail_weapons.rs` (table pinned against the
  export and `../GYARI/scripts/items.txt`, damage per difficulty, trace range and skill spread).
- Headless probe: `MESTER_SILENT=1 MESTER_TEST_SCENARIO=arsenal level-viewer.exe rh1-wiezienie2 ../../output out.png 62` acquires every weapon,
  selects each in turn on game time, logs magazine, reserve and dip, and writes `out-lower-*`, `out-flash-*`, `out-shot-*`, `out-explosion.png`.

## 9. Open

- Weapon pickup auto-equip and wheel/Tab cycling differ from retail (section 3).
- `mnoznik_latarki` shaft geometry; the flashlight cone is a simple three-strip ribbon.
- Reload during a weapon change: ported (section 3). Corpses: ported, see "Corpses and bullets" in docs/retail-ai.md: the lowest quarter of a dead character's box stops a bullet with a blood splash, the rest lets it through (both for the player's and the enemies' bullets).
- The player-shot stimulus lives exactly one actor pass (an actor's shot two): settled from the code, docs/retail-ai.md "Reconciliation". The "weapon in hand" HUD rule is `selected && equipped`: retail tests that the weapon model object exists (0x10009e70: `weapon+0x650` and its handle), and the object is swapped inside one call (0x10003540 / 0x100036a0) so it exists in every frame of the lowering and rising states too: equal. The health bar is armed by any non-zero health call past frame 60 (0x10061cad): `Campaign::health_events`.
