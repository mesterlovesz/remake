# Retail first-person camera (cshell.dll)

Evidence for the camera in `crates/level-viewer/src/view.rs`. The client shell
pointer is at 0x10b3cd4c; the player controller is at shell+4, the CCamera at
shell+8. Addresses are cshell.dll virtual addresses.

| Topic | Retail behaviour | Address |
|---|---|---|
| Eye height | collision centre + `eyeOfs`; 46 standing, 16 crouched, sliding 250 units/s | 0x100553d0, 0x1005546b |
| FOV | fovX 81° fixed; fovY = fovX × 1.1 × h/w (66.82° at 4:3) | 0x100560e0 |
| Scope | `alt_zoom` weapons (only the M-14): both FOVs × 0.15 at 5 rad/s per axis, both snap when either arrives; mouse × 0.25 | 0x10056180, 0x100037e6 |
| Mouse | 0.006625 rad per count × sensitivity (0.5..1.5, default 1.0); no smoothing, no acceleration | 0x10054bf0 |
| Pitch | clamped to ±1.5393804; yaw wraps to ±π | 0x10054bf0 |
| Bob | (offsets are along the displayed image's right/up: with the display flip the camera's right offset and the roll take `mirror::SIGN`, see docs/retail-visual.md mirror audit) phase += 11.5·dt (× 1.2 running upright), wraps at 4π; s1 = 1.3·sin φ, s2 = 1.3·sin(φ/2); camera up × s1 × (4 running, else 3), right × 1.5·s2, roll −0.005·s2; only while a move key is held on the ground; after stopping the phase runs on to 2π or 4π | 0x1005567d–0x10055cbb |
| Weapon bob | relative to the camera: up −0.26·sin φ (walk) / −0.39 (run), right 0.845·sin(φ/2) / 1.3 (`CWeaponBob 1`) | 0x10055cbe |
| Strafe roll | ±0.04 rad at 0.1 rad/s while strafing on the ground, back to 0 at 0.1 rad/s | 0x100555ac |
| Recoil | none: `shake_screen` is decayed but never read | 0x10055fa4 |
| Damage | no camera motion; blood sprite on the screen (see retail-gunfire.md) | 0x10033a64 |
| Landing | no dip; fall damage `(takeoff_y − landing_y − 196) × 0.15` after 60 frames in the level, `sounds\speech\hero\spad.wav` | 0x1006102f |
| Death | eye falls 192/s to 16 above the box bottom; yaw +1 rad/s up to the absolute 7π/4 (cap 0x5c), pitch +3 rad/s (down) up to level 0 (cap 0x58), roll +1 rad/s up to π/2 (cap 0x60 = roll + π/2); the 3π/4 stored at 0x64 is the strafe roll, unused here. Fields: yaw 0x50, pitch 0x4c, roll 0x54 | 0x10055130, 0x100600f4 |
| Drunk | alcohol 0..100 decays 0.5/s; FOV × (1 + sin·alc·0.0035), view wobble cos·alc·0.005 | 0x10055e5b, 0x100563a7 |

Retail bindings (`scripts/keys.cfg` with the `AddAction` list in `autoexec.cfg`): W/S/A/D (and arrows) move, right mouse jumps, left mouse fires,
middle mouse or Alt is alternate fire, Ctrl crouches, Shift runs, Q toggles running, 1..8 select weapons, Space or E is use, R reloads.
The complete command table (slots, handlers, addresses) is in retail-input.md.

**Owner's deliberate deviation (quality of life, not retail):** jump is Space,
the right mouse button is the alternate fire / scope (the middle mouse button stays as the second
binding; Alt falls out because a slot holds two ids) and E is the only use / open key. Retail put jump on
the right mouse button, which made scoping impossible.
