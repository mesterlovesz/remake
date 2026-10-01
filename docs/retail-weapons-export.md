# Original carried weapon assets and behavior

`python -m tools.export_weapons` reads `../GYARI`, exports to `output`, and
refuses output anywhere inside the original installation. Original scripts,
models, textures, sprites and audio are read only. Tests do not launch a game
or open an audio device.

## Exports

- `retail_weapons.json`: eleven carried definitions, exact source commands,
  original LTB model/skin references, animation names/durations, Hungarian names,
  native camera offsets and nonuniform scales, damage, ammo, latency, spread,
  HUD panels, sound paths and source hashes.
- `retail_items.json`: ten player weapon pickups and all eight ammunition
  pickups, including weapons not placed in the first four prison worlds. Uses
  the existing `items.json` schema, allowing original NPC weapon drops.
- 28 model JSONs, 74 distinct textures/HUD/sprite frames, and 28 audio files.
  Sprite sequences retain repeated frame references and source ordering.

Regular selection order follows the original experience slots: baton, Glock,
S&W, SIG, Ingram, FN shotgun, M14, HK G8, P90. Grenade follows these. `heli_bron`
is retained as an NPC-only carried definition (`player_selectable=false`).
The cigarette object has a `weapon` flag but no carried model and is not
converted into a player weapon.

`max_ammo` is magazine capacity, not an invented reserve cap. `ammo_index`
identifies eight firearm ammunition pools plus grenade index 8. The ordered
commands retain duplicate records; Ingram ammo's final `amount 50` overrides
its earlier `amount 30`. FN shotgun has two `descr` records and no `title`:
the exporter resolves its existing `>IdShotgunT` text, retaining both raw lines.

## Source inconsistencies retained

The original baton references a nonexistent `reload` animation. Glock and
S&W reference alternate-fire animations not contained in their carried models.
P90 references two missing laser toggle sound files. These appear in manifest
warnings and `missing_animations`; no replacement animations or audio are
invented. Every firearm's primary and reload clips exists.

## Binary behavior evidence

Addresses below are virtual addresses in the installed `cshell.dll`, image
base `0x10000000`. Its SHA-256 is recorded in the manifest. A local disassembly
is saved at `output/retail-cshell-disassembly.txt`. No binary was patched.

### Primary fire and animations

- `0x10060c9a..0x10060ccf` reads held action 6 and calls `0x10003f90` every
  active frame. Unlike adjacent alternate-fire action 7, it has no pressed-edge
  latch. `automatic=true` therefore means repeating held primary for **every
  non-grenade native weapon**, including pistols, not a real-world gun category.
- Item `shot_latency` is stored at `+0x8fc` (`0x10062a02`) and checked by
  `0x1000400d..0x1000401e`. A shot may restart the shoot clip before the entire
  previous animation sequence finishes.
- `anim_shoot0` at `+0xbd8` then optional `anim_shoot1` at `+0xc58` are
  **sequential stages**, followed by looping `anim_base` at `+0xb58`.
  See `0x100101dd..0x10010255`; melee uses the same sequence at
  `0x1000f442..0x1000f4b5`.
- Offsets `przes_right/up/forward` are stored at `+0xb38/+0xb3c/+0xb40`
  and applied in the camera basis by `0x10010635..0x100106c5`. Model vertices,
  animation binding translations and native scales remain unmodified. The
  original first-person camera FOV has not been established by this export.

### Player bullet spread

`0x10005dde..0x100061a0` uses:

```
range = 640 + 24 * max(256 - raw_spread, 0)
spread = raw_spread * (1 - skill[experience_index] * 0.75 * 0.01)
endpoint = origin + forward * range + independent_xyz_jitter
jitter_axis = (rand() % 1000 - rand() % 1000) * spread * 0.001
```

The player does not receive the NPC distance cap or difficulty spread factor.
Zero weapon skill uses raw spread. M14 has no `rozrzut` command, giving zero
spread and a 6784-unit ray. FN shotgun emits `kul_na_raz 10` pellets; heli emits
two. Full character skill progression remains a separate runtime concern.

### FN shotgun reload

`0x1000ffb0..0x100100bc` is used for a shotgun in reload state 1 or 2.
When `reload0` finishes, primary held or full/no-reserve ends with `reload2`.
Otherwise reserve decreases and magazine increases **before** `reload1` plays.
At each subsequent completion the same checks repeat. A shell that fills the
magazine still plays its `reload1` before the final `reload2`.

Thus N inserted shells take `0.5 + N * 0.453 + 0.5` seconds, with the first
ammo increment at 0.5 seconds. Holding primary interrupts through the closing
clip and retains already inserted shells. Weapon switch `0x100034a0` rejects
reload states 1/2 and transition states 9/10. It does not reject armed-grenade
states 5..8; that original cancellation behavior must not be silently patched.

### Grenade timing and physics

- Primary begins `zawleczka` (0.7s) and consumes a grenade. After that animation
  completes, the in-hand five-second fuse starts. Held primary cooks it; release
  starts `sam_rzut1` (0.3s), with cooking continuing. On completion, the object
  spawns and `sam_rzut2` (0.18s) plays. In-hand expiry explodes. See
  `0x1000ad40..0x1000b105`.
- **Original quirk:** type-7 constructor `0x1004f2c7..0x1004f2cd` discards the
  passed cook time and sets a fresh **3.0s** projectile lifetime. The in-hand
  limit remains 5.0s. Exported `fuse_seconds` and `thrown_fuse_seconds` are
  intentionally different.
- Throw velocity is `forward * 640 + world_up * 256` native units/s.
  Airborne gravity is 640 native units/s² (`0x10052113`). The original grenade
  pickup model renders at scale 2 and receives collision dimensions (8,4,8).
- Ground probe in `0x10051e50` is down `16 - vy * dt` units. Contact multiplies
  velocity by `30 / (630 * max(dt, 0.05))`. On first contact, if resulting
  `vy < -16`, `vy *= -0.3` followed by `velocity *= 0.65`; bounce sound is
  `sounds/weapons/grt_ryko.wav`. LithTech's subsequent physical object movement
  and wall collision remain engine behavior; these constants alone do not
  establish complete collision parity.
- NPC blast (`0x10043180`) uses radius 640, maximum damage 500 with linear
  falloff and an occlusion ray from explosion Y+16. Player blast
  (`0x1005fd20`) uses radius 640, maximum damage 100 with linear falloff and
  explosion Y+32 for its occlusion ray.

### Original grenade visuals

Manifest `grenade_effect` contains `sound`, `impact_sound`, and `layers` with
ordered image paths, source dimensions, fps, start, duration and native scale.
`0x10053ab0..0x10053c21` supplies:

| Layer | Original sprite | Start | Lifetime | Scale |
| --- | --- | ---: | ---: | ---: |
| Flash | systemblikwybuch.spr, 11 frames at 15fps | 0 | .45s | 4 |
| Explosion | systemwybduzy1.spr, 31 frames at 20fps | .1s | 1.45s | 1.1 |

Sound is `sounds/weapons/rock_lup.wav`. Secondary sparks/debris and dynamic
light behavior are not reconstructed by this export.

## Verification

`python -m unittest tests.test_weapons`: six passing tests. They validate all
carried and pickup assets, referenced animation availability (including known
missing source references), original values and ordering, localized labels,
hash metadata, rejection of output inside GYARI, ordered sprite frames and
the distinct in-hand/thrown grenade fuse values. These checks establish export
integrity; gameplay/renderer verification is performed by the viewer separately.
