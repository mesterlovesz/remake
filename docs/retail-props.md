# Retail scene props (`o_obiekt`): what cshell.dll / object.lto do

Evidence for `crates/level-viewer/src/props.rs`, `models.rs`, `activation.rs` (o_obiekt branch) and the prop hooks in `gunfire.rs`, `doors.rs`, `npcs.rs`.
Addresses are `cshell.dll` unless marked `object.lto`. Definitions come from `scripts\objects.txt` (229 blocks, `output/decoded_scripts/objects.txt`),
placed instances from `<world>.props.json` / `<world>.scene.json` (3287 `o_obiekt`, 183 distinct definitions in the 29 exported levels).
Art (models, every skin frame, the `.spr` flame skins and the `kawalki` debris models) is exported additively by `python -m tools.export_props definitions <GYARI> output` (`output/props_defs.json`).

## Server side (`o_obiekt`, object.lto vtable 0x100259ec)
Only `Model`, `Gracz_otwiera`, `Nast_obiekt`, `Death_nast_obiekt` and the name are read. The server object dumps itself to `scripts\cs\obdrzekty.txt`
(the client reads that file at level start, 0x1002bb24) and registers in the chain registry. Activation (0x10017580): refused when `Gracz_otwiera` is 0 and the caller is
the player / an NPC (4 instances), refused when dead; otherwise it toggles a state byte and returns 1 so the chain follows. The registry death path (0x10009730) marks the
object dead (0x100175d0) and recurses into its `Death_nast_obiekt`.

## Client prop manager (cshell 0x1002b2d0..0x1002f750)
| step | address | behaviour |
|---|---|---|
| parse objects.txt | 0x1002d496 | keys: `model`, `skin0..7`, `rs0..3`, `podmieniany_skin0..7`, `podmieniany_rs0..3`, `mass` (+0x1288), `HP`, `gravity` (+0xd04), `solid` (+0x1290), `insignificant` (+0x1298, value or 1), `przezroczysty_dla_blikow` (+0x12a4), `iskry/gruz/drewno/papier_przy_trafieniu`, `anim0/1/01/10/anim_raz`, `sound0/1/01/10`, `death_sound` (+0x1188), `death_podmien` (+0x1208), `death_*` mask (+0x1294) |
| create a prop | 0x1002c0b0 | one engine model object per scene prop, flags 0x2001 (visible + solid) when `solid` or `gravity` is set, else 1; the first animation of the model (or `anim0`) loops; `Gracz_otwiera` sets a user flag |
| bullet hit | 0x1002bf50 (called at 0x10007f2a) | damage = the weapon's `sila_strzalu` (item +0xb28, raw, no distance falloff / difficulty). `hp -= damage`; a prop that survives is shoved `damage * 10 / mass` units along `normalize(stored_position - hit_point)` (the stored position is never updated, so death effects and the next push direction use the placed position); a dead prop skips the shove |
| death | 0x1002e690 | sends client message 0x27 with the registry id (server kills the chain target), sets the dead flag unless `death_keepalive` (2), plays `death_sound` at 960 units, spawns the `death_podmien` wreck (fresh definition, HP 9999, position/rotation of the original, `anim_raz` plays once), hides the model, `death_wybuch` (1) runs the explosion 0x1005b120, then the debris table |
| explosion | 0x1005b120 | effect type 8 (sprite set), a record (pos, 196, 2048) appended to the character manager's list by 0x10045cf0 (bullet impacts append 200 / 1024; not modelled), player blast 0x1005fd20 and character blast 0x10043180: every living `ruchomy` character within 640 units and with an unobstructed ray from the blast +16 up takes `(640 - distance) * 0.78125` (500 at the centre), the player takes `(640 - d) * 0.15625` (100) scaled by the painkiller factor. The two calls with the prop / pickup managers (0x1005ade0) are `ret 4` stubs: **explosions never damage other props**, so a chain reaction between props exists only through `Death_nast_obiekt` |
| debris | 0x1002e86f..0x1002f565 | `death_ceramika` 8 pieces (`ceramika01..04`), `_malo` 6 at half scale, `blacha` 8 (0.5), `_malo` 6 (0.2), `_duzo` 8 (1.0), `prety` 8, `_malo` 6, `deski` 8 (`deski_polamane`, 0.7), `_malo` 8 (0.25); offset `(r-r)*32, r*32-r*16, (r-r)*32` from the prop, velocity `offset * 8` (`* 5` for the small ones), gravity 640, 5..15 s, tumbling; the big pieces drag an `ogon.spr` streak |
| hit debris | 0x10007e1c..0x1000887f | ricochet sound (75 %), then 6 sparks (`iskry_`), 8 rubble, 8 wood splinters, 4 paper scraps |
| melee | 0x1000f829..0x1000fa25 | every prop whose box (around its placed position) contains the 32-unit strike point, else the 64-unit point, takes the weapon's `sila_strzalu` through the same hit routine (nightstick 40) |
| use key | 0x1002f750 | E on a prop within 128 units: `Nast_obiekt` chain, `anim01/sound01` (open) or `anim10/sound10` (close), then `anim1` / `anim0` loop |
| object detail | 0x1002d0b0 | option 0x100b2494 (default 0): level 1 hides `insignificant 1`, level 2 every `insignificant` prop |

## Render styles (`rsN`)
`objects.txt` `rs0..rs3` name render style files (`GYARI/rs/*.ltb`, readable sources `*.lta`). N is the **piece's render style index** (mesh header field after the texture indices, exported by
`tools/ltb.py` as `style`), not the texture slot: `samochod_niebieski_bucha` draws its flames (texture 2) with `rs0 przez_z_maska_plomien`, its shadow (texture 1) with `rs1 cien`, the body with no style.
Blend states from the sources: `additive` ADD, `przez_z_maska_plomien` MUL_SRCALPHA_ONE (additive weighted by the texture alpha, z read-only, no cull), `cien` / `okulary` / `przez_z_maska1`
MOD_SRCALPHA (alpha blend), `przez_z_maska` alpha test GREATER 0, `automat_cola` / `metal_z_maska` / `do_ganow` opaque with a second (reflection) texture stage: the DTX alpha of those is the reflection mask,
not transparency. `models::style_alpha` maps them; pieces without a style stay opaque.

## Data facts
* `solid` 100 definitions, `gravity` 47; both make the prop solid for the player and NPCs (the remake blocks both through `PropField::sweep`, wired into `doors::tick` and `npcs::tick`).
* Masses: 1 (papers, plates, shoes, bulbs) .. 1000; a Glock bullet (30) shoves a mass-1 prop 300 units, a mass-100 prop 3 units. The push ignores the floor: a prop hit from above sinks (original quirk).
* HP 99999999 props (bulbs, most lamps, wall lights) never die; the only destructible lamp is `lampka_abazur` (HP 300, explodes).
* Explosive props (`death_wybuch`): cars (limuzyna, trabant_bezsrodka, samochod_niebieski, truck, lotus, lamborghini, autobus), `butla_niebieska` / `butla_czerwona` (gas cylinders, 30 + 24 instances), vending machines (`Automat Cola/kawa`), `lampka_abazur`, `wietrak`, helicopter.
* `Death_nast_obiekt` (44 props): 38 `zarowka` bulbs name lights that do not exist (or, in burmistrz2, LightGroups; only LightGroup10 exists and its bulb has HP 99999999), so nothing happens; two live pairs exist: rh3-miasteczko1 `o_obiekt70` (car, HP 200) -> `o_obiektsam08`, rh3-miasteczko2 `o_obiekt20` (limousine, HP 200) -> `o_obiekt22huj`.
* There is no loot drop from props in retail and no breakable window class: glass is `b_transparent` (see docs/retail-objects.md), which has no damage handler.

## Remake status
Implemented: everything in the table (creation flags, idle animation loops, skins incl. `.spr` flames, chain / use-key activation, bullet damage from `sila_strzalu`, the push, death sound / wreck / explosion / debris, keepalive props, `Death_nast_obiekt`, solid collision for the player and NPCs, bullets stop at props), tested by `props.rs` unit tests and the `MESTER_TEST_SCENARIO=props` capture probe (docs/retail-objects.md).
Approximate: debris `Spawn` model scales / trail sprite, the effect-type-8 sprite set (uses the grenade explosion assets), the 0x10045cf0 list record is not modelled, `sound0/sound1` (unused in the data) are not played.
