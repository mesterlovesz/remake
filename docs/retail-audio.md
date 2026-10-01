# Retail audio and weather (cshell.dll, object.lto, Lithtech.exe)

Evidence for music, positional sound, footsteps and rain. object.lto is the
server game module, cshell.dll the client, Lithtech.exe the engine. The master
table of every cue (file, trigger, reach, remake status, addresses) is
docs/retail-audio-parity.md; this file keeps the detail of the individual systems.

## Music

`muza` in each gameai level header (parsed at 0x10016192) names a track under
`sounds\muza\`. It starts on the 78th frame of a level load (0x1005a583 →
10059950): looping, 2D, volume 100, hard cut (no crossfade, no fade); entering a world
stops the old track (0x1005a180 → 0x1005ae92). There is no calm/combat switching (MuzaSpok*,
MuzaAtak* and `wlaczmuze` are never read). Main menu: `menu.wav` (0x1005a382, frame 2);
outro: `outro.wav` (0x100131ae); credits: `credits.wav` (0x10031a2c). Options
are on/off toggles only; volume is fixed at 100. The files are MPEG Layer-3
inside WAV (format tag 0x55), 22050 Hz stereo.

The handle (`[this+0x2010]`) and the remembered track name (`[this+0x2014]`): every play call
(0x10059950) first kills the old handle and clears the name, returns when the `Music` switch
(0x100b23f4) is off, else remembers the name and plays. The per-frame check 0x10059230 (called from
0x1005a774) stops the track when the switch is off and, when it is on, nothing plays and the level frame
counter (`[this+0x37c]`) is above 100, plays the remembered name again; but the stop has just cleared it, so **Music off
and on again leaves the music silent until the next level / menu start**. A start while the switch is
off plays and remembers nothing. Nothing in cshell pauses the track when a menu opens, and the engine
has no sound pause, so it plays on behind the pause and options menus. Implemented in `music.rs`
(`Retail`, unit-tested) and `settings::holds`.

| Track | Levels |
|---|---|
| prolog.wav | rh3-miasteczko0 |
| wiezienie1_spokoj.WAV | rh1-wiezienie2 |
| Tension_s.wav | rh1-wiezienie3, rh12-lab1, rh10-wiezowiec1..3 |
| wiezienie2_spokoj.wav | rh2-wiezienie1, rh3-miasteczko2, wiez_wn1, wiez_wn3 |
| Blood_and_Glory_s.wav | rh2-wiezienie2, chapel_mniejszy, rh12-lab2 |
| miasteczko.wav | rh3-miasteczko1 |
| tunele.wav | rh7a-tunele, podziemia1, podziemia1a |
| fabrika.wav | rh9-fabryka |
| burmistrz.wav | burmistrz1, burmistrz2 |
| chinatown_spokoj.wav | chinatown, podziemia1c |
| chinatown_akcja.wav | chinatown2 |
| podziemia.wav | podziemia1b |
| wiezowiec_wn.wav | wiez_wn2 |
| knajpa2.wav | knajpa |
| none | rh1-wiezienie1 |

## Positional sound

Engine falloff (Lithtech.exe 0x480940, 0x4804a0): the game always passes inner
radius 0, so gain = 1 − d/R (0 beyond R). Software panning: pan = 64·(1 −
dir·right) with dir = normalise(listener − source); sources behind the listener
are up to 25 % quieter (1 − 0.25·dot(dir, forward)). The position is fixed at
play time. SndDrv.dll maps volume to DirectSound attenuation
`dB = −100·(1 − ((127/126)(1 − 1/v))^√127)` with v = clamp(1.27·vol, 1, 127)
(vol 50 → −8.6 dB, 25 → −24 dB, 10 → −56 dB); the far channel is cut by up to
15.7 dB at full pan.

| Sound | Radius |
|---|---|
| Player shot (at the eye plus the weapon's view offset, 0x100051c1) and melee swing (camera + 64·forward), melee hit, body hit, baton on wall, casing, grenade bounce, doors and drawers (`Glos_otwierany/zamykany`, at the door origin), elevator button | 640 |
| Ricochet ryko0..4, rocket | 1280 |
| Destructible object `death_sound` | 960 |
| NPC weapon | its `glosnosc` (×4 while the NPC sees the player or in a cutscene) |
| Hero wound by a bullet (`speech\hero\wcialo.wav`, 0x10006a65, at the hit point) | 640 |
| Prop use sound `sound01` / `sound10` (0x1002e520, flags 0x412; the prop's previous sound is killed first) | 1536 |
| NPC footsteps (`glos_buta1`, every `odstep_glosow_buta` > 0.09 s) | 640, 2048 when it sees the player; muted in cutscenes |
| NPC "halt" (`sound_on_kontakt`, `sounds_on_kontakt N` picks haltN at random) | 1280, 2048 when it sees the player |
| NPC death scream deadN.wav (N = rand % 8, needs `graj_dzwiek_smierci`) | 4024 |
| Phase `sound` key | 2048 |
| Detector `Dzwiek` (only wiez_wn1 DOMOFON.WAV) | `Zasieg_dzwieku`, 2D if `Dzwiek_3D` = 0 |

2D sounds: player footsteps, jump (`speech\hero\skok.wav`), fall
(`spad.wav`, also when dead), heartbeat, pickups, reload, dialogue lines without a speaker and
every chosen answer, cutscene voices (PlaySound flags 0x1210: local; the scene code also fills in a
position and radius 1280, which the engine ignores), the shove wound (`weapons\wcialo.wav`), music.
A dog bite makes no sound. The grenade's `sound_shoot` (`zawleka.wav`) is never played. The heartbeat `speech\mason\serducho.wav` plays once when health drops
below 50 and again below 20 (0x10061bcb).

`d_odglos` objects (Glos, Zasieg, Czas_powtarzania) are only appended to
`scripts\cs\dzwienki.txt` by the server (0x1000ef10); no code plays them, so
they are silent in retail.

## Player footsteps

The surface under the player (ray +32 → −256, 0x100614e0) selects the sound
(0x10061366); variants 1/2 alternate; steps play every 0.4 s only while running
and moving (walking and crouching are silent).

| SurfaceFlags | Sound |
|---|---|
| 0, 6 | KROKK1/2 |
| 2, 4 | KROKM1/2 |
| 3 | KROKD1/2 |
| other | KROK1/2 |

## SurfaceFlags

Each world DAT surface is `u32 engine flags, u16 texture index, u16 texture
flags`; the u16 texture flags are the game's SurfaceFlags (a ray hit reports
them at +0x28). Meanings by texture name: 0 concrete/brick/plaster, 1 sky,
2 metal, 3 wood, 4 water, 5 soft (grass, carpet, beds), 6 tiles/marble,
7 misc, 10 one detail texture. chinatown's physics uses one flag-7 texture.

## Rain

Only rh3-miasteczko0..2 (`d_emiter_opadu`, identical properties). The server
sends one client effect after 3.1 s (0x1000c19d). Client (create 0x100305c0,
update 0x10031110, draw 0x10030d90): 64 drops respawned around the camera
(x, z ± 512; y + 0..512); a drop lives only if a ray 10240 units straight up
hits a SurfaceFlags-1 (sky) polygon, otherwise it waits 1 s; ground from a ray
down to −1280; fall speed 0.75·|Kierunek.y| = 1230 units/s; respawn beyond
1024 units on any axis. Each drop is a vertical additive streak 30 units tall
and 0.8 wide, colour top rgb(7,17,12) a7, bottom rgb(15,35,25) a15; on landing
a `kregi.spr` ripple, scale 0.4, 0.5 s, 2 units above the ground. There is no
rain sound code (`sounds\deszcz` is unused).

## Remake implementation (crates/level-viewer/src/audio.rs, music.rs)

Verification without listening: `MESTER_CUE_LOG=1` prints every sound the game asks for (`CUE 2d|3d|far ...`, `CUE music play|stop`) in any silent run and
`MESTER_TEST_SCENARIO=sounds` asserts the cue classes (docs/retail-audio-parity.md). `MESTER_SILENT=1 MESTER_AUDIO_LOG=1` makes every sound
log an `AUDIO 2d|near r=<radius> <file>` line (and each music cue `AUDIO music ...`)
instead of playing; `MESTER_AUDIO_PROBE=1` (capture runs) additionally steps on each
distinct floor surface near the spawn, drops health through 50 and 20 and jumps.
Silent runs never create an AudioPlayer for music or these sounds.

- Music: `tools/export_audio.py` writes the MP3 data chunk of each muza WAV to
  `output/audio/sounds/muza/<lowercase name>.mp3` (Bevy's `mp3` feature decodes it).
  `music::tick` follows the retail state: loading stops the track, the menu track
  starts on frame 2, the level track (table above, lower-cased world name) on frame
  78 after the load, the outro cutscene switches to `outro`, and `credits` follows the
  outro. Like every other sink the music is driven by `settings::update` (`Session::drive`):
  it follows the volume preference and is never paused (it plays on behind every menu like retail); the
  `Music` switch stops and forgets the track as in 0x10059230 (docs/retail-audio-parity.md U6).
- Player footsteps: `move_camera` calls `audio::player_step` on `Player::footstep`
  (retail-movement fires it every 0.4 s only while running and moving on the ground).
  The surface is the per-face SurfaceFlags of the collision face hit by the ray
  +32 → −256 from the player centre (`raycast_face`, `<world>.collision.surfaces.json`);
  a miss reads flags 0. The first step of a level is variant 2, then 1, 2, ...
- Jump `skok.wav` is detected from the vertical impulse leaving the ground (the retail
  controller also plays it for the 1500-unit action 0x14, which does not exist here).
- Heartbeat: 0x10061ba0 plays `serducho.wav` once per health update, when old health
  was ≥ 50 and the new one is < 50, otherwise when ≥ 20 and now < 20.
- NPCs (all through `npcs::pending_sounds` → `audio::play_near`):
  - footsteps 0x1004ab2b: a phase with `odstep_glosow_buta` > 0.09 counts a timer down
    every frame, plays and reloads when it passes zero; always `glos_buta1` (the byte
    that would select `glos_buta0` is never written in cshell), whether or not the
    NPC actually moves;
  - halt 0x10049a9d: `sound_on_kontakt` with `sounds_on_kontakt N` > 1 replaces the
    digit before ".wav" with rand % N (rand only consumed then);
  - death 0x10042d40: a `graj_dzwiek_smierci` character takes the last
    `sound_on_kontakt` path it defines, overwrites its last nine characters
    ("halt0.wav") with `dead<rand % 8>.wav` and plays it at 4024. macaroni has no
    halt/dead files, so it is silent in retail too.
- Doors and drawers: every open/close state change (use, link, self-closing) plays
  `Glos_otwierany` / `Glos_zamykany` at the object `Pos`, radius 640.
- Detector `Dzwiek` (wiez_wn1 DOMOFON.WAV): played when the marker first triggers.

Everything retail plays is listed in docs/retail-audio-parity.md with its status; `audio::play_near` /
`sound::play_at` (positional, linear falloff to the retail radius), `audio::play_2d`, `audio::play_voice`
(retail-2D sound with a source position, pans only in the improved mode) and `audio::play_near_owned` (a new sound
of the same owner stops the old one) are the only entry points, and each records a `Cue` (MESTER_CUE_LOG=1).

## Diagnostic: "nincs hang" (2026-09-29)

Root cause, verified without any audible output: **the sound went to the DualSense pad's speaker.** The Windows default playback
device (console and communications role, read-only Core Audio query) of the owner's PC is `Hangszórók (2 - DualSense Wireless
Controller)`, 4 channels, 48 kHz, 100 % and not muted. The other active endpoints are `SPDIF illesztő (Realtek USB Audio)`, `Hangszórók
(Steam Streaming Microphone)` (Steam's virtual sink), `Hangszórók (Focusrite USB Audio)` (at 96 %: the studio speakers) and the
`VG258QM` monitor (HDMI). Bevy 0.18's AudioPlugin opens `rodio::OutputStream::try_default()`, i.e. exactly the default device, and
cannot be told otherwise (the trace runs before the fix logged `Hangkimenet: Hangszórók (2 - DualSense Wireless Controller) (4 csatorna, 48000 Hz, F32)`).
`cargo test --bin level-viewer output_device_report -- --ignored --nocapture` lists the devices, the default and the order the game
tries them in (it opens nothing; `MESTER_PROBE_OPEN=1` also opens the first one, silence only).

Fix (output.rs, replaces Bevy's AudioPlugin; same rodio `Sink`s under the same `AudioPlayer` / `AudioSink` components):
- `Preferences::output_device` (settings.json, empty = automatic) names the device; the pause menu ("Hangkimenet" row) and the retail
  sound page ("Hangkimenet váltása") cycle automatic and every device at run time, the sounds playing restart on the new one.
- Automatic: the system default unless its name says game pad (DualSense, DualShock, Wireless Controller, Xbox, gamepad); then the
  first real speaker device (names starting "Speakers"/"Hangszórók"/... before digital outs and displays), the pad and Steam's
  streaming sink last (a pad still beats silence; while Steam streams its sink is the default and stays). On the owner's PC this
  picks `Hangszórók (Focusrite USB Audio)`. The chosen device and why is logged at start: `Hangkimenet: ... helyett`.
- Silent capture runs open no device at all (Bevy used to open the default one in every capture run); a MESTER_AUDIO_TRACE run opens it.
- Bevy unwraps the decoder result and would panic on a file that does not decode; the new player logs the error and drops the sound.

What was checked and is NOT the cause:
- The pipeline up to the device works in a normal run: MESTER_AUDIO_TRACE menu run: `menu.mp3` player, sink 4 frames after the spawn,
  playing, never paused, intended volume 0.60 (the saved preference; `output/settings.json` was written by the owner's volume
  buttons: 1.0 - 4 x 0.1). Level runs: KROK*/skok/SERDUCHO/psss (footsteps, jump, heartbeat, door), glock_s/ryko0..4/luska1 (shots,
  ricochets, casings) all get a sink 0-1 frames after the spawn with the right distance-scaled volume; 0 load failures.
- Nothing mutes a normal run: `Session::silent` is only `MESTER_SILENT` or a capture run; no `MESTER_*` variable exists in the
  Windows user or machine environment; `Mesterlovesz-Ujrairva.cmd` / `Palyanezo.cmd` start the debug exe with `menu|<world> ..\..\output`
  and no third argument (not a capture run). The launcher's exe (built 12:11) contains the audio commits.
- The debug profile is fast enough: `audio_thread_load_in_the_debug_profile` (ignored probe): decoding the level music costs the audio
  thread 4 % of real time, the music plus 16 looping effects 19 %. No underruns, no need for an opt-level override.
- Every exported sound decodes with the game's decoder (`every_exported_sound_decodes_with_the_game_decoder`, ignored, 3 min in the
  debug profile): 1128 files, 0 failures. 22 are all zero: `scenes/intro/001.wav`, `intro1/1..3.wav`, `outro/00..17.wav`, byte-identical
  (md5) in the retail install, so those cutscene voice tracks are silent in retail too. Every sound path in the exported data exists in
  `output/audio`, except `sounds/enemies/macaroni/halt0.wav` and `sounds/weapons/laser_on|off.wav`, which the retail install lacks.

For the owner: the game now lists the device it plays on in the log (`Hangkimenet: ...`) and in the option screens. If the sound is
still not where he listens, pick the device with the "Hangkimenet" buttons (saved in `output/settings.json`), or set the Windows
default output device (Settings > System > Sound) to the Focusrite speakers.

Non-audible tools: `MESTER_SILENT=1 MESTER_AUDIO_TRACE=1 timeout 120 level-viewer.exe <world|menu> ../../output <capture.png> 3,14`
(capture runs only) keeps the whole pipeline live (players, asset loads, sinks, pause, despawn, volume) but replaces every sound by
silence and mutes every sink; the log has one `AUDIOTRACE` line per second and per sound (sink appeared after N frames, frames playing or
paused, intended volume, `NEVER GOT A SINK`, `DOES NOT DECODE`) and a `FINAL` summary. `MESTER_PROBE_DEVICES=Focusrite,DualSense cargo
test --bin level-viewer switching_the_device -- --ignored` opens those two devices with silence and checks the run-time switch.

## Improved 3D audio ("Javított 3D hangzás", 2026-09-29)

Owner request: sounds come from the direction of their source (dialogue from the speaker), switchable in the settings.

Option: `RetailOptions::spatial_audio`, console variable `"CSpatialAudio" "0|1"` in autoexec.cfg (written with the other sound switches),
default ON. It is the sixth row of the sound page ("Javított 3D hangzás be/ki"; the page is shared by the main menu and the in-game menu;
the volume slider moved from y 545 to 572 below it). It acts live: sounds started afterwards use the setting, a spatialised sound that is
still playing glides back to the baseline within ~20 ms when it is switched off.

Retail baseline (option OFF, unchanged from before, docs above): the engine 3D sound (Lithtech.exe 0x480940 / 0x4804a0) has inner
radius 0 and a per-sound outer radius R (table under "Positional sound"), gain 1 - d/R, software pan 64·(1 - dir·right) and up to 25 %
less behind the listener, position fixed at play time. The remake baseline keeps the linear falloff (`sound::gain`, sink volume via
`Gain`) and plays the mono file to both ears without pan. The engine code also keeps the source velocity, but the game only ever
plays static one-shots, so there is no Doppler.

Improved mode (ON), `spatial.rs`:
- The listener is the camera (`Listener::from_camera`): position, right, forward, up. **Handedness**: mirror.rs shows the image left/right
  flipped (LithTech is left-handed), so a Bevy camera's own right vector points to the LEFT of the picture; with `DISPLAY_MIRRORED` the ear
  axis is negated (`mirror::MIRRORED`), so a source on the right of the screen sounds in the right ear (unit test with a camera facing +Z and
  one turned to +X; with the flag off the same source is on the left).
- Distance: same retail radius R as the hard limit, but `distance_gain`: 1 up to 0.1 R, then inverse distance with rolloff 0.35
  (`0.1R / (0.1R + 0.35 (d - 0.1R))`), cosine-faded to exactly 0 from 0.6 R to R (no pop at the limit). d = 0.25 R: 0.66 (linear 0.75),
  0.5 R: 0.42 (0.5), 0.75 R: 0.21 (0.25). It is the sink volume (`sound::level` in `sound::update`, `audio::play_world`, dialogue `speak`).
- Per ear (`mix`, targets in `SpatialState`, written every frame by `spatial::drive`): equal-power pan on the lateral component
  `dir·right` (elevation therefore centres a source overhead; `L²+R²=1`, centre 0.707); rear cut 1 - 0.25·behind (retail's 25 %);
  interaural time difference on the far ear, Woodworth `0.7 (a/c)(θ + sin θ)`, a = 8.75 cm, c = 343 m/s, 0.46 ms at 90 degrees; one-pole
  low-pass per ear from 20 kHz down to 2.5 kHz: 0.9·behind + 0.6·head shadow (far ear only) + a mild term for sources more than ~17 degrees
  above/below the head. Closer than 60 cm all cues fade to the centre (no direction, no NaN).
- The effect is `Spatialized<S>`, a rodio `Source` wrapper in the sink chain built by `output::play_queued` (decoder -> `Spatialized` ->
  the sink's own volume/pause/speed): any source is downmixed to mono, then delayed (fractional delay line), filtered and scaled per ear
  and emitted as stereo. Gains, delays and cut-offs slew per sample with a 20 ms time constant (turning never clicks) and start at the
  real pan of the sound (no ramp-in). A mono device gets one channel (average of the ears). Devices with more than two channels get the
  front pair exactly as the baseline does (rodio pads the rest with zeros): the extra channels of a game pad are haptics/headphones and of an
  audio interface other outputs, so the rear channels are never used.

Spatialised (everything that carries `sound::Positional`, i.e. goes through `audio::play_near` / `sound::play_at`): NPC speech at the speaker
(dialogue titlesnd, r 4096), NPC weapons, footsteps, "halt" shouts, death screams, phase sounds, player shots and the ricochets (they are
world positions), melee hits, casings, door/drawer sounds, prop use sounds and destroyed-prop death sounds, the detector sound, and (new)
the grenade bounce (r 640) and the grenade blast (r 1280) which used to play 2D (docs table: rocket 1280, grenade bounce 640; this also
changes the OFF baseline: they now fall off with distance like retail). Not touched: music, the video and cutscene voices, menu sounds, player
footsteps, jump, landing, heartbeat, hit grunt, pickups, reload and the player's own weapon sounds, and dialogue speech without a speaker (all 2D).

Verification without listening:
- `cargo test --bin level-viewer spatial` and `output::`: the pan/ITD/roll-off/distance math (front, left, right, behind, above, head, mirror),
  the offline `Spatialized` render (left source: left channel RMS > 20x the right one, ITD of 10 samples exact, no click when the pan flips
  under a DC signal), the real `play_queued` + sink chain without a device (`spatial_run`: left/right/front/behind, 4-channel and mono
  device, 2D sound untouched, option off = identical channels, switched off while playing glides to equal channels).
- `MESTER_SILENT=1 MESTER_AUDIO_TRACE=1 MESTER_AUDIO_PROBE=1 level-viewer.exe rh2-wiezienie1 ../../output x.png 3,14`: five ricochets 300 units
  from the camera (volume 0.6 x distance gain 0.68 = 0.408), the trace prints their ears:

| Source | ears L / R | ITD | low-pass |
|---|---|---|---|
| left | 1.000 / 0.000 | R 459 us | R 5743 Hz |
| right | 0.000 / 1.000 | L 459 us | L 5743 Hz |
| ahead | 0.707 / 0.707 | 0 | 20000 Hz |
| behind | 0.530 / 0.530 | 0 | 3078 Hz |
| above | 0.707 / 0.707 | 0 | 11892 Hz |

- `MESTER_TEST_SCENARIO=dialogue MESTER_TEST_PERSON=barman MESTER_TEST_DIALOG=BarmanBar1 ... level-viewer.exe knajpa ...` (speaker 110 units ahead):
  `AUDIOTRACE end speech audio/sounds/speech/tavern/19.wav ... SPATIAL ears L=0.707 R=0.707 ...`, i.e. the speech sink is spatialised at the speaker.
- `MESTER_TEST_SCENARIO=menu_pages` clicks the new row (off, written to autoexec.cfg as `"CSpatialAudio" "0"`); `menu_sound` holds the sound page for a capture.

For the owner (not audible to me): with the option ON, a speaking NPC or a shot to your left should be clearly in the left ear/speaker, turning
towards it moves the sound to the centre smoothly, sounds behind you are duller and a little quieter, and near sounds are louder than before while
the far ones still end at the same distances. OFF is the old mono-in-both-ears behaviour. On headphones the effect is strongest; on speakers the
time difference matters little and the pan/roll-off carry it.
