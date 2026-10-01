# Retail decorations and atmosphere (object.lto, cshell.dll, Lithtech.exe)

Evidence for `crates/level-viewer/src/decorations.rs` (addresses are the module's own virtual addresses).
Sprites are exported by `tools/export_decorations.py` (opaque RGB frames, `output/decor/sprites.json`).

## d_sprite (809 objects in 25 levels)
- Server class object.lto 0x1000f620. PreCreate builds an engine sprite (object type 3): `Sledz_kamere` 0 sets flag 8
  (rotateable), otherwise 0x100; `Maly_w_zblizeniu` 0x20, `Wylacz_z_mgly` 0x80, `W_oku` 0x10, always 4; `Additive` /
  `Multiply` are flags2 2 / 4. InitialUpdate colours it (Alpha, Alpha, Alpha, Alpha) and scales it by `Skala`.
- Update (0x1000f8bc): a sprite with `W_oku` or `Pionowy` is appended to `scripts\cs\sprajty.txt` (name, pos,
  Additive, Multiply, Alpha, W_oku, Maly, Skala xyz, "not tracking", Wylacz, Pionowy) and the server object is
  removed: 769 of 809 sprites become client nodes (cshell loader 0x100027b0 at level frame 0x3d, node create 0x10002440).
  Only 40 (`W_oku` 0) stay plain engine sprites; the 3 `sprites\mortyr.spr` do not exist in the install.
- Client update (0x10002a30): every `dist * 0.25 * 0.00078125` s a segment sprite -> eye is intersected with the world;
  hit: the colour fades to 0 at 4/s and the object is hidden, no hit: instantly back to full. `Pionowy` turns the quad
  about Y to face the eye (2 chinatown `rejuch.spr`).
- Engine quad (Lithtech.exe 0x53e1b0): half size `texW*Skala.x`, `texH*Skala.y` (full width `2*tex*scale`), skipped
  nearer than 7 units; `Maly_w_zblizeniu` scales by `clamp((depth-10)/490,0,1)*1.9+0.1`. Blend states 0x53d5b0: additive =
  ONE/ONE with the fog colour forced to black (so linear fog fades it out), `Wylacz_z_mgly` disables fog, z-test on,
  z-write off. Texture RGB only, alpha is never read.

## Rain (d_emiter_opadu, miasteczko0..2)
See docs/retail-audio.md. Drop visibility: a ray straight up must first meet a SurfaceFlags 1 polygon. Landing ripple
(`kregi.spr`, 26 frames, scale 0.4, 0.5 s, 2 units above the ground) is created only when
`dot(camera position, direction to the landing) > 0` (0x10031000): an original quirk, replicated. Streak colours are
additive with no alpha (the alpha bytes 7/15 are not used by ONE/ONE).

## d_lens_flare (6 levels)
Server 0x1000e270 writes 11 parts to `scripts\cs\lensflare.txt` (Za_centrum3..1, Centrum, Przed_centrum1..6, Blysk; sprite,
Alfa, odl, Skala, additive). Client 0x100229b0 creates one additive sprite per part (scale Skala), 0x10023010 places them:
sun = eye - dir*32, centre = eye + forward*32, `step = (centre - sun)*0.1`; Za parts at `sun - step*odl`, Przed parts at
`sun + step*odl`, Centrum/Blysk at the sun. Colour `sqrt(t*Alfa)`, Blysk `((t-0.75)*4*Alfa)^2`. `t = 1 - |sin|/limit` from
the xz and xy angle sines between the light direction and the view (limit 1.7267 / 0.8634). Hidden when dot(dir, forward)
>= 0 (0x10022e20) or when the 20480-unit ray to the sun does not first hit a SurfaceFlags 1 polygon (so rh1-wiezienie2 and
rh2-wiezienie1, which have none, never show it). Retail lacks `flare1\centrum/przed*/za*.spr`; those parts draw nothing.

## d_emiter_dymu (smoke: podziemia1a, miasteczko2)
Message 0x6f 4.4 s after level start. Every `Czestosc_emisji` a puff spawns at the emitter +- `Rozsiew_startu`
(difference of two 0..0.99 draws) with velocity `Predkosc_pocz`; each frame velocity += `Wiatr * (1 + r1 - r2)`, speed
clamped to `Max_predkosc`, scale *= 1 + dt*`Rozszerzanie`, the sprite animation (dymek.spr 15 fps) plays from spawn. The
object colour alpha is `Alpha` but additive ignores it; the puff vanishes abruptly at `Czas_zycia`.
`d_emiter_iskier` (sparks) sends no message to the client (object.lto 0x1000b610 only reads its properties and sets the
colour): it is inert in retail, nothing is drawn.

## d_fala3d (fabryka pool: it is the water surface)
Message 0x75. The client animates an engine PolyGrid (0x1003e380): height byte `(int)(sin(px)*sin(pz)*127)`,
`px` gains `col*0.2*PI` after every cell, `pz` gains `row*0.2*PI` after every row, both phases advance by `X_Speed`/`Z_Speed`
per second; the water texture pans by `Przes_Tex`. Cell size, `Amplituda`, `SkalaX/Z` texture tiling and the grid centring
on the object are inferred (the engine PolyGrid draw code is not in the research sources).

## b_wskazowka_* and b_rotator
Clock hands (object.lto 0x10007bb0 hour, 0x10008200 minute, 0x10008720 second) set the ABSOLUTE rotation every 1 ms:
hour `(hour%12*60+minute)/720` turns from GetLocalTime, minute `minute/60` and second `second/60` turns from
GetSystemTime (UTC), about X, or about Z when `Na_osi_Z`; `Flip_wskazowek` reverses. The exported world is mirrored
relative to LithTech's handedness, so the angle is negated (verified on the rh10-wiezowiec2 tower clock).
`b_rotator` (0x10002fd0): phase += dt*`Krok_Fali`/30 (0.1 gives 0.0033 rad/s), rotation `sin(phase/2)*Obrot` radians,
slide `sin(phase)*Przesuw`; with `Krok_Fali` 0 (knajpa) it never moves.

## DemoSkyWorldModel (rh10-wiezowiec1..3, wiez_wn3)
object.lto 0x1001fcc0: the first object with `SkyDims` > 0 passes the engine a SkyDef {pos-dims, pos+dims,
pos-dims*Inner, pos+dims*Inner}; its brushes (and "DemoSky drapacze") are sky objects. The sky brushes sit in a far corner
of the map, so they are drawn by an extra camera (order -1) that turns with the eye and slides inside the inner box as the
eye crosses the level; sky fog is `Mgla_Nieba` with `Start/Koniec_Mgly_Nieba` (client SkyFogEnable/NearZ/FarZ).

## Fog
Client console commands from WorldProperties (cshell 0x1005b460): `FarZ` = Widocznosc, `FogEnable/NearZ/FarZ` =
Mgla_Wlaczona/Start_Mgly/Koniec_Mgly, `FogR/G/B` = Kolor_Mgly, `SkyFog*` as above. D3D fog is linear on view depth; Bevy's
DistanceFog is radial, so the edges of a wide view fog slightly earlier than in retail. Additive effects are baked for
D3D's gamma-space addition (`lin(bg+v)-lin(bg)` against the fog colour).
