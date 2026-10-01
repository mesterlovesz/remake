# Retail ancillary (non-asset) files of the install

Kutatási jegyzet a gyári telepítés (`GYARI`) **nem tartalmi** fájljairól: indító (launcher), bevezető-lejátszó, kódoló, konfigurációk, motor-/hang-DLL-ek,
szerkesztői maradványok, mentés, videók. A fájlokat csak adatként elemeztük (Python + pefile/capstone), semmit nem futtattunk.
Legfontosabb tanulságok: (1) a `cshell.dll` **rejtett konzolváltozókat** olvas minden képkockán (`God`, `FullStamina`, `Invisible`, `DrawKilledCount`,
`DrawPaths`, `DrawPostacPaths`, `runworld`, `locale`) - ezek az `autoexec.cfg`-ben is beállíthatók, a remake egyiket sem ismeri; (2) a gyári szkriptek
betöltője a betűpár-kódolást (`cipher.exe` algoritmusa) **menet közben oldja vissza**, vagyis a telepítő kódolt szkripteket vár; a `GYARI/scripts` jelenlegi állapota
a `cipher.exe` 2026-09-28 06:38:35-i futásának nyoma (időbélyeg-bizonyíték lent), a remake `read_script` mindkét alakot elfogadja; (3) a `clientfx.fxd`
a LithTech SDK példa-effektkönyvtára, a játék **semmilyen** modulja vagy pályája nem hivatkozik rá; (4) a gyári mentés (`quick.sav`) teljesen más formátum,
mint a remake JSON-alapú mentése; (5) az indítási lánc: `launcher.exe` -> `lanuchp.txt` -> `play1.exe` (3 videó) -> `lithtech.exe`, a videók bármely billentyűre / kattintásra
egyenként átugorhatók. A `video.rs` megjegyzése a logók nevét rosszul adja meg (lásd lent).

Method: file bytes + PE headers/imports/resources (`pefile`), disassembly (`python -m tools.inspect_retail START END --module X`), string extraction, cross-checks
against `docs/retail-*.md` and the remake sources (`crates/level-viewer/src/{options,keys_cfg,savefile,video,retail_world}.rs`, `tools/*.py`).
Addresses are virtual addresses of the named module. Status legend: USED-IN-REMAKE / USED-BY-RETAIL-BUT-MISSING-IN-REMAKE / UNUSED-IN-RETAIL / ENGINE-INTERNAL.
Another agent's `tools/inventory_unused.py` classifies the whole install heuristically; this note is the curated, evidence-based part for the non-asset files.

## 1. Top findings (HIGH PRIORITY first)

| # | File / subject | Finding | Status | Effort |
|---|---|---|---|---|
| H1 | `cshell.dll` console variables (read by `autoexec.cfg` / engine console) | Hidden debug variables polled every frame at 0x10059230 (called from the client update 0x1005a774): **`God`** (health kept at max, kill counter forced to 0), **`FullStamina`**, **`Invisible`** (NPC "can see player" checks return false), **`DrawKilledCount`** (kill counter on the HUD), **`DrawPaths`**, **`DrawPostacPaths`** (navigation-path debug overlays), `runworld` (world to start), `locale`. Evidence in section 5 | USED-BY-RETAIL-BUT-MISSING-IN-REMAKE | S for God/FullStamina/Invisible/DrawKilledCount, M for the path overlays |
| H2 | Lithtech.exe command line | Switches `-config <f>`, `-display <f>`, `-cmdfile <f>`, `-noinput`, `-rez <p>`, `-world <name>`, `-host`, `-breakonerror`, `-windowtitle`, `-workingdir`, `+sounddll <name>`, `+<cvar> <value>` (0x404ef0, strings at 0x57fa4c..0x57fc50). `-world` / `+runworld` start a level without the menu | ENGINE-INTERNAL (remake has its own `<world> <output>` CLI) | - |
| H3 | `cipher.exe` / `scripts/*.txt` | The game decodes scripts on load with the same swap table (cshell 0x10057f50, object.lto 0x1001b090, launcher 0x4012c0); "Files MUST be coded for the game to run properly!". GYARI's scripts were toggled by the retail `cipher.exe` on 2026-09-28 06:38:35 (122 files in its list, touched in list order within 0.12 s). Retail binaries given the current plain files would read garbage; `scripts/cs/dialogs_real.txt` is (still) encoded, `scripts/cs/dialogs.txt` plain (not in cipher's list) | USED-IN-REMAKE (`tools/decode_scripts.py::read_script` accepts both) | done |
| H4 | `launcher.exe` | Builds the command line, writes `lanuchp.txt`, runs `play1.exe` (movies) or `lithtech.exe` directly ("skip movies"); the only options are resolution x depth (6), max texture size (3), skip movies. Reads `autoexec.cfg` for the defaults, never writes it. CD check is stubbed out (0x4022e0 = `mov al,1; ret`) | ENGINE-INTERNAL / no remake need | - |
| H5 | `1.avi 2.avi 3.avi` + `play1.exe` | DirectShow player; any key or mouse click (WM_KEYDOWN, WM_LBUTTONDOWN, WM_RBUTTONDOWN) skips only the current clip (0x401300). `video.rs` lets Esc skip all three. `video.rs` / `export_videos.py` label 1.avi "Cenega" and 3.avi "Lithtech": the pictures are a galaxy (1), the Mirage Interactive logo (2, four identical frames, 4 s) and a tunnel animation (3) | USED-IN-REMAKE (minor deviation + wrong comment) | S |
| H6 | `save/quick.sav`, `quick.srv` | Retail format: SYSTEMTIME(16) + `RatHunt save game.\n` + 7 floats (100.0) + world name + 320x240x32-bit thumbnail + mission-variable records; remake writes its own JSON container (`savefile.rs`, different layout). No cross-compatibility, no retail-save import | USED-BY-RETAIL-BUT-MISSING-IN-REMAKE (import only) | M, optional |
| H7 | Light properties in every DAT / `LightAttenuationPresets/Default.lta` | The preset (`coefs 1 0 19`, `exps 0 0 -2`) is baked into **every** light of every world (AttCoefs/AttExps/AttType `Default`/`Attenuation` enum `D3D`/`Quartic`/`Linear` on 2634 light objects); the remake ignores it (LightRadius/colour only) | USED-BY-RETAIL-BUT-MISSING-IN-REMAKE | M (formula not verified) |
| H8 | `clientfx.fxd` | A DLL exporting 12 `fx*` functions, 15 FX types, 6 animation names; **no retail module, script or world references it** (no CClientFX / CAttacher object in any of the 31 DATs). The remake's `retail_effects.json`/`fx.rs` are built from cshell's hard-coded sprite effects instead | UNUSED-IN-RETAIL | none |
| H9 | `scripts/app_name.txt`, `sniper.ico` | Retail window/launcher title "A mesterlövész v 2.33" (Lithtech.exe 0x404f3d reads it) and the 64x64 icon; remake title is `EDITION / world`, no icon | USED-BY-RETAIL-BUT-MISSING-IN-REMAKE | S |
| H10 | `TextureEffectGroups/` | Only `pan.tfg` (rh3-miasteczko0, RH9-fabryka) and `pan_szybszy.tfg` (burmistrz2, podziemia1a, wiez_wn1) are referenced by worlds; script `UVPan` only. `huj/pan_automat/warble.tfg`, `UVRotate`, `UVWarble`, `WSPosRotateAroundY` are unused | USED-IN-REMAKE (UVPan, `retail_world.rs`) | done |
| H11 | `textures/chinatown/DirTypeTextures` | Not a marker: a 174,342-byte 256x128 DTX (Wall Street Bar sign), 2002-02-14, an older copy of `wall_street_bar.dtx` (2002-04-18, same size, differs from byte 21); referenced by nothing | UNUSED-IN-RETAIL | none |
| H12 | `save\auto.sav` / `c:\s_pov.bin` | Dead hooks: Lithtech.exe at exit (0x405f8d) restarts `launcher.exe` if `save\auto.sav` exists and the launcher then starts the game immediately (0x40189f) - a restart loop; nothing in the install ever creates `auto.sav`. `c:\s_pov.bin` is a marker written by the launcher (0xFE) and by cshell start (0x10059535, 'a') and deleted at exit (0x10059cb0); nothing reads it. Do **not** create `save\auto.sav` next to the retail exes | UNUSED-IN-RETAIL | - |

## 2. Master table

| Path | Size | What it is | Read by (evidence) | Status | Remake need if missing |
|---|---:|---|---|---|---|
| `launcher.exe` | 909,312 | MFC 4.2 static dialog app "Sniper ver. 1.00.01" (RatHunt Launcher, by Corwin, Mirage Interactive, built 2003-09-05); dialog 102 + 420x420 bitmaps | user; writes `lanuchp.txt`, `c:\s_pov.bin`; starts `play1.exe`/`lithtech.exe` | ENGINE-INTERNAL | - |
| `lanuchp.txt` | 109 | One-line Lithtech command line written by the launcher (mtime 2026-09-30 09:53:15: the launcher was run then) | `play1.exe` (0x4014ce), written by launcher (0x402120) | ENGINE-INTERNAL | - |
| `ReadMe.txt` | 5,433 | Xicat readme 2002-09-05 (requirements, performance tips, default controls) | human | UNUSED-IN-RETAIL | - (facts in section 10) |
| `play1.exe` | 45,056 | DirectShow intro player (built 2002-11-18) | launcher | ENGINE-INTERNAL | `video.rs` replaces it |
| `cipher.exe` | 212,992 | MFC dialog tool "cipher": swaps paired letters/digits of 122 script files in place (built 2002-11-19) | user only | ENGINE-INTERNAL (tool) | - (`decode_text`) |
| `classhlp.but` | 16,611 | DEdit class-help text (ButeMgr-style `[Class]` / `Property = "help"`), 2001-06-12 | DEdit only | UNUSED-IN-RETAIL | - |
| `TextureEffectGroups/` | 16 files | LithTech texture-effect scripts `.txt`, compiled `.tfs`, parameter groups `.tfg`, `TSCompiler.exe`, `!compile pan.bat` | Lithtech.exe (directory string `TextureEffectGroups\` 0x594858, used at 0x533eba); worlds name `.tfg` files | USED-IN-REMAKE (UVPan only) | - |
| `LightAttenuationPresets/Default.lta` | 103 | DEdit attenuation preset | DEdit; values copied into every DAT light | USED-BY-RETAIL-BUT-MISSING-IN-REMAKE | M |
| `ClassIcons/` | 13 x 21,924 | 64x64 32-bit DTX editor icons (DEdit tree) | DEdit only | UNUSED-IN-RETAIL | - |
| `Engine.REZ` | 102,162 | RezMgr v1 archive, 11 entries (console font + 10 built-in render styles) | Lithtech.exe (`engine.rez`, 0x57fa8c) | ENGINE-INTERNAL | - |
| `clientfx.fxd` | 208,896 | ClientFX type library DLL (built 2001-10-02) | nothing | UNUSED-IN-RETAIL | - |
| `SndDrv.dll` | 61,440 | `.snd` sound driver plug-in: DirectSound (DX8), built 2002-10-11 | Lithtech.exe (0x59121c, `SoundSysDesc/SoundSysMake`) | ENGINE-INTERNAL | - |
| `dx8.snd` | 53,248 | Same plug-in + "EAX Hardware" device, built 2002-10-10 | Lithtech.exe enumerates `*.snd` (0x4b5880), `+sounddll` | ENGINE-INTERNAL | - |
| `cdaudio.dll` | 24,576 | LithTech music-DLL (`MusicDLLSetup`, CD audio via MCI), 2001-09-28 | Lithtech.exe (0x40d945 default name) | ENGINE-INTERNAL | - |
| `l3codeca.acm` | 290,816 | Fraunhofer MP3 ACM codec (1999) | `SndDrv.dll`/`dx8.snd` (`acmDriverAddA`, `L3CODECA.ACM`) | ENGINE-INTERNAL | remake decodes MP3-in-WAV itself |
| `mss32.dll` | 347,648 | Miles Sound System (2000-08-17) | **no module imports it** | UNUSED-IN-RETAIL | - |
| `ltmsg.dll` | 24,576 | 17 engine error-message strings | Lithtech.exe (0x57fafc "Unable to load ltmsg.dll") | ENGINE-INTERNAL | - |
| `CRes.dll`, `sres.dll` | 24,576 each | Empty client/server resource DLLs (16-byte `.rsrc`, no strings) | Lithtech.exe (`cres.dll`/`sres.dll`, 0x581..) | ENGINE-INTERNAL | - |
| `MFC42D.DLL`, `MSVCIRTD.DLL`, `MSVCRTD.DLL` | 929,844 / 94,285 / 385,100 | MSVC 6 debug runtime leftovers | imported only by each other | UNUSED-IN-RETAIL | - |
| `MSVCP60.DLL`, `msvcrt.dll` | 401,462 / 295,000 | Release C++/C runtime (2001-08-24) | Lithtech.exe imports both | ENGINE-INTERNAL | - |
| `sniper.ico` | 5,694 | 64x64 8-bit icon (same as the launcher's group icon 145) | installer shortcut | USED-BY-RETAIL-BUT-MISSING-IN-REMAKE | S |
| `autoexec.cfg` | 7,567 | Engine console dump (mtime 2026-09-28 06:37:48) | Lithtech.exe (0x57fa6c default config) | USED-IN-REMAKE (`options.rs`) | - |
| `scripts/keys.cfg` | 1,645 | "Generated by RatHunt" key table | cshell 0x100116f0 (read), 0x100114f0 (write) | USED-IN-REMAKE (`keys_cfg.rs`, round-trips) | - |
| `scripts/app_name.txt` | 23 | Window title text, plain CP1250 | Lithtech.exe 0x404f3d, launcher 0x40194d | USED-BY-RETAIL-BUT-MISSING-IN-REMAKE | S |
| `save/quick.sav`, `quick.srv` | 318,034 / 4 | Retail quick save (2003-09-19 15:18) + 4 zero bytes | cshell `c_savemgr` | USED-BY-RETAIL-BUT-MISSING-IN-REMAKE (import) | M |
| `1.avi 2.avi 3.avi` | 5.12 MB / 3.69 MB / 2.16 MB | Start-up videos | `play1.exe` | USED-IN-REMAKE | - |
| `*/DirType*` (121 files) | 0 bytes (120) | Directory-type markers for DEdit/ModelEdit (section 13) | editor | UNUSED-IN-RETAIL | - |
| `rs/*.lta *.vsh *.ash` | 30 + 5 + 1 | Render-style sources and vertex shaders next to 14 compiled `.ltb` | engine loads `.ltb` | ENGINE-INTERNAL (see `retail-visual.md`) | - |
| `misc/**` sub-directories | - | UI graphics (section 13) | cshell | see section 13 | - |

## 3. Start-up chain: `launcher.exe`, `lanuchp.txt`, `play1.exe`

### launcher.exe
* PE: MFC 4.2 statically linked (no `MFC42D.DLL` import), PE time stamp 2003-09-05, resources in Polish (1045): dialog 102 (932 bytes), six 24-bit bitmaps
  (134: 420x420 background; 136: 133x28; 138/141: 132x128 twin button skins; 142: 26x420; 143: 132x23), icon 1 (64x64), version info: CompanyName *Mirage Interactive*,
  FileDescription *RatHunt Launcher*, Comments *by Corwin*, Copyright 2002, OriginalFilename `launcher.EXE`, ProductVersion 1.0.0.1.
* Dialog (English strings inside the Polish-language resource): title `Sniper ver. 1.00.01`; combo 1001 (resolution): `640x480x16`, `800x600x16`, `1024x768x16`, `640x480x32`,
  `800x600x32`, `1024x768x32`; combo 1002 (max texture size): `128*128`, `256*256`, `512*512`; buttons `Run Sniper now!`, `Exit launcher.`; checkbox `skip movies.`; two hint lines
  (`Increasing resolution and bitdepth may cause slowdowns on older hardware.` / `Any texture size should fit your hardware (scaling down is automatic), though biggest may be sometimes slowing.`).
  At start the dialog replaces those captions with the keys `>launch1`..`>launch7` of `scripts\text_keys.txt` (read with the *decoding* reader 0x401320 -> swap function 0x4012c0; Hungarian texts:
  `Felbontás:`, `A felbontás és bitmélység növelése a régebbi hardvereket lelassíthatja.`, `Textúra méret:`, ..., `Futtasd a Mesterlövészt most!`, `Filmek kihagyása`, `Kilépés a betöltésből.`)
  and the title from `scripts\app_name.txt` (plain reader 0x4011f0).
* `InitDialog` (0x401800): writes the marker `c:\s_pov.bin` (`wb`, one 0xFE byte); if `save\auto.sav` exists it closes the dialog and starts `lithtech.exe` with no arguments (CreateProcess 0x401938) and never shows the UI.
* Constructor (0x401480): reads `autoexec.cfg` line by line (plain reader) and picks up the quoted variables `"screenwidth"`, `"bitdepth"`, `"MaxTextureSize"` (0x42d100..0x42d120) as the combo defaults. It never writes `autoexec.cfg`, and it writes no registry values of its own (the `Reg*` imports belong to statically linked MFC).
* Run button (0x401de0..0x402230): builds `lithtech.exe` + ` -rez . -rez engine.rez` + one of ` +screenwidth 640 +screenheight 480 +bitdepth 16` ... ` +screenwidth 1024 +screenheight 768 +bitdepth 32` + ` +MaxTextureSize NxN`, **always** writes it to `lanuchp.txt` (`wt`, 0x402120), then: if "skip movies" is ticked it starts that command line directly; otherwise it starts `play1.exe` (0x40219a). No other switches.
* The "CD present" check (message `Launcher was unable to detect a Sniper: Path of Vengeance CD. Please insert proper CD into your drive and try again.`) is dead: its test function 0x4022e0 is `mov al,1; ret`.
* Hidden options: none beyond the above. No language selection, no cheat/debug switches. Performance presets the remake lacks: the 640/800/1024 x 16/32-bit list and the 128/256/512 texture cap (modern renderer; the remake deliberately uses the native resolution and HD textures, see `retail-hd-textures.md`).
* `lanuchp.txt` (name misspelt in the binaries themselves: `lanuchp.txt` at launcher 0x42d2c0 and play1.exe 0x40708c) now contains
  `lithtech.exe -rez . -rez engine.rez +screenwidth 800 +screenheight 600 +bitdepth 16 +MaxTextureSize 128*128`, i.e. the launcher was last run with 800x600x16 / 128*128 at 2026-09-30 09:53:15.
  Note: those command-line values override `autoexec.cfg` (1024x768x32, 512*512) for that run.

### play1.exe (45,056 bytes, 2002-11-18)
Tiny Win32 app. Creates a window class `Sniper`/`VideoWindow` (WS_POPUP|WS_VISIBLE, `ShowWindow(SW_MAXIMIZE)`), builds a DirectShow graph (CLSID_FilterGraph, IGraphBuilder, IMediaControl, IVideoWindow, IMediaEventEx; GUIDs at 0x406118..0x406158), renders the wide-character paths `1.avi`, `2.avi`, `3.avi` in this order (0x401050: index 0 -> 0x407054 "1.avi", 1 -> "2.avi", 2 -> "3.avi"),
puts the video window inside the client rect and listens for the completion event (WM_APP+1 = 0x8001). The window procedure (0x401300) treats `WM_KEYDOWN` (0x100), `WM_LBUTTONDOWN` (0x201) and `WM_RBUTTONDOWN` (0x204) as "next clip" and `WM_DESTROY` as quit. After the third clip it opens
`lanuchp.txt` (`rt`), reads the first line (0x401000, max 255 chars) and `CreateProcess`es it (cwd = current directory, flags 0x0C000200); if the file is missing it runs the built-in fallback `lithtech.exe -rez . -rez resource.rez` (0x407064; `resource.rez` does not exist in this install). Error text `!error.txt` is only a CRT string.

### Videos (played by play1.exe only)
| File | Video | Audio | Duration | Content (checked against `output/videos`) |
|---|---|---|---:|---|
| `1.avi` 5,120,000 B, 2003-07-30 | MS MPEG-4 v1 (`mpg4`), 640x360, 25 fps, 355 frames | PCM 44.1 kHz stereo 16-bit | 14.2 s | galaxy/space animation |
| `2.avi` 3,688,960 B, 2002-04-05 | uncompressed 24-bit 640x480, 1 fps, 4 frames, no audio | - | 4.0 s | Mirage Interactive logo (still) |
| `3.avi` 2,160,640 B, 2002-09-10 | MS MPEG-4 v2 (`mp42`), 640x480, 14.634 fps, 260 frames | MP3 22.05 kHz stereo | 17.8 s | tunnel animation |

The remake (`video.rs`, `tools/export_videos.py`) plays them in the same order as JPEG frames + MP3 (3.avi exports 257 of 260 frames). Differences: Esc skips all three (retail: each key/click skips one; there is no Esc-all), and the code comment/docstring names 1.avi "Cenega" and 3.avi "Lithtech" although the pictures do not show those logos.

## 4. `cipher.exe` and the script state of GYARI
* What it is: MFC dialog app (`cipher`, label "Work!" / "Cancel", text `This utility ciphers/deciphers Sniper .txt scripts.`). No command line, no other mode. On click (0x4019d0 -> 0x401410/0x4018f0) it calls 0x401360 for 122 hard-coded relative paths (`scripts\ai\*` 56, `scripts\postacie\*` 41, `scripts\locale\*` 19, `scripts\{items,objects,postacie,scenki,text_keys}.txt`, `scripts\cs\dialogs_real.txt`), each: `fopen(path,"rt")`, read up to 0x40000 (262,144) bytes through the swap function 0x401300, `fopen(path,"wt")`, write back. Missing files are skipped. Finally it shows `(de)coding process finished. Check the results.` and `Files MUST be coded for the game to run properly!`.
* Algorithm (0x401300): `A-Z`, `a-z`, `0-9` are swapped in pairs (`a<->b`, `c<->d` ... `y<->z`, `0<->1` ... `8<->9`; i.e. `offset ^ 1` inside the class); everything else unchanged. It is its own inverse, which is why one button both ciphers and deciphers. `tools/decode_scripts.py::decode_text` is the same function.
* The game reads those files through decoding `fgets` clones built around the same swap function (cshell swap 0x10057f50, readers 0x10057fb0 (1 caller) and 0x100580b0 (78 callers, e.g. `shell.txt` at 0x100595ad); object.lto swap 0x1001b090, readers 0x1001b0f0 (1 caller) and 0x1001b190 (7 callers); launcher swap 0x4012c0, reader 0x401320 for `text_keys.txt`), so the shipped files are expected encoded. The `cs\*.txt` files written with `a+t` are read by object.lto's plain reader 0x1001b150. cshell's writer 0x10058010 is a plain `fputc` loop.
* Timeline found on disk (mtimes): `scripts/cs/dialogs.txt` 06:37:22 and `autoexec.cfg` 06:37:48 (2026-09-28), then **the whole script tree within 06:38:35.66..06:38:35.78**: `ai/gameai.txt` first, `ai/*`, `locale/*`, `postacie/*`, `items`, `objects`, `postacie.txt`, `scenki`, `text_keys.txt` last - exactly the call order of the retail `cipher.exe` list (empty files included, because `wt` touches them). So `cipher.exe` was run on GYARI and toggled those files. `dialogs_real.txt` (in the list) is still encoded, `dialogs.txt` (not in the list) is plain Hungarian, so the pre-run state of at least `dialogs_real.txt` was plain. The original installed state could not be checked (the install cabinets are InstallShield `data1.cab`, no unpacker available).
* 101 of the 133 files under `scripts/` are 0 bytes (all `ai/dialog*`, `ai/ai_*` per-level files, all `postacie/*.txt`, `cs/{ajtemsy,beamsy,dzwienki,emitersy_facetuf,facety,obdrzekty,sprajty}.txt`); the per-character/per-level scripts are folded into `postacie.txt` / `gameai.txt` in this release. The seven `scripts/cs/*.txt` with mtime 2026-09-30 09:53:19 were **created by the game**: object.lto opens them with mode `a+t` (0x100287c4, e.g. 0x1000ee14 `dzwienki.txt`, 0x100181fd `facety.txt`; debug message `Zapisuje do pliku %i`), 4 s after the launcher wrote `lanuchp.txt` - i.e. the retail game was started at 2026-09-30 09:53. Because those reads use the decoding reader, a retail run against the now-plain scripts reads scrambled text.
* Remake: `tools/decode_scripts.py::read_script` detects plain vs encoded per file (first token heuristic), so the remake works in either state. Nothing to implement. Recommendation: never run `cipher.exe` again on GYARI.

## 5. `autoexec.cfg`, `scripts/keys.cfg`, `app_name.txt`, hidden console variables

### autoexec.cfg (7,567 bytes) - written by the retail engine, not by our tooling
mtime 2026-09-28 06:37:48. The remake never writes into GYARI (its `RetailOptions::save` writes `<output>/autoexec.cfg`, which differs: 1920x1080, MaxFPS 240, showframerate 1, 288 bytes). The format is the engine's console dump: `"var" "value"` lines, then `AddAction` (input actions), `enabledevice`, `rangebind` (scancode -> action), `scale`, `ModelAdd`. Every variable:

| Variable (value) | Meaning / evidence | Remake |
|---|---|---|
| `numconsolelines` 0 | LithTech `NumConsoleLines` (Lithtech.exe 0x185228, server.dll): 0 = console hidden | - |
| `alwaysflushlog` 1 | `AlwaysFlushLog` (0x185248): flush `error.log` on every write | - |
| `Speech` 1, `Music` 1, `Sound` 1 | cshell switches polled at 0x10059230: Speech -> `[0x100b23f5]` (0x100592c8), Sound -> `[0x100b23f6]` (0x100592ff), Music (0x10059239) compared with `[0x100b23f4]` and starts/stops the music (0x10059950) | `options.rs` |
| `locale` 1 | cshell `[0x100b247c]`, tested at 23 sites: selects the localised UI set `misc\panel_l`, `menu_l`, `loading_l` (with 0 cshell asks for `misc\panel`, `menu`, `loading`, which are (nearly) absent from this install) | remake always uses the `_l` set |
| `optimizesurfaces` 1 | D3D renderer cvar (Lithtech.exe 0x191cbc), surface optimisation | - |
| `windowed` 0 | `windowed` (0x1916f4) | `options.rs` |
| `saturate` 0 | `Saturate` (0x191ca4), see `retail-visual.md` (fog colour) | - |
| `bitdepth` 32, `screenwidth` 1024, `screenheight` 768 | display mode (0x1851e4/0x18520c/0x1851fc), the launcher reads them as defaults; command-line `+` values override | width/height read |
| `errorlog` 0 | `ErrorLog` (0x185258) | - |
| `MaxFPS` 160 | frame cap (0x18504c) | read; cap off by design |
| `MaxTextureSize` "512*512" | renderer texture cap, `W*H` string (0x191c94); launcher offers 128/256/512 | not applied (HD textures) |
| `CInvertMouse` 0, `CWeaponBob` 1, `CAutoQuickSave` 1 | cshell options (0x100593af/0x100593e6/0x1005941d) | `options.rs` |
| **`God` 0** | cshell hidden cheat variable (see below) | **missing** |
| `enabledevice` `##mouse` / `##keyboard` | input devices (Lithtech.exe 0x182d1c) | - |
| `ModelShadow_proj_enable` 0, `ModelShadow_proj_alpha` 300 | projected model shadows, engine cvars (0x191dc4, 0x191f04); cshell can set them (0x1006aa08); blob texture `textures/cien.dtx` | stored only (`backlog` B015) |
| `lightmaps` 0 | no such cvar (engine's is `LightMap`, 0x592264): no effect (`retail-visual.md`) | n/a |
| `showframerate` 2 | `ShowFrameRate` (0x185238); non-zero shows the counter, meaning of 2 vs 1 not verified | `options.rs` |
| `ServerProps` 0.000000 | no string in any module; a runtime-registered variable of the server shell, meaningless | - |
| `MaxModelLights` 4 | max dynamic lights per model (Lithtech.exe 0x192098) | not modelled |
| `AddAction` x ~110, `rangebind`, `scale "##mouse" "##x-axis" 0.006625` | action ids identical to `keys_cfg.rs`; mouse factor 0.006625 rad/count (`view.rs::MOUSE_RADIANS`) | USED |
| `ModelAdd 0 0 0` | engine console command dump (0x182d34, `ModelAdd %f %f %f`: ambient model light add) | - |

### scripts/keys.cfg
Header `Generated by RatHunt. Do NOT modify!`, then 512 integers (256 command slots x 2 `AddAction` ids) + 9 numbers at the end (`7 1.22 0.69 1 0 1 1 1.15 1`: includes the mouse sensitivity 1.22). Reader 0x100116f0, writer 0x100114f0; `keys_cfg.rs` round-trips the shipped file byte for byte (`docs/retail-input.md`).

### scripts/app_name.txt
`A mesterl?v?sz v 2.33` (CP1250 bytes `0xF6`/`0xE9`: "A mesterlövész v 2.33", 23 bytes, plain). Lithtech.exe 0x404f3d reads it for the window title (`windowtitle` default `LithTech`), launcher 0x40194d for the dialog title.

### Hidden console variables of cshell (HIGH PRIORITY)
String pool at 0x1006df18..0x1006dfbc; reader `CClientShell::UpdateConsoleVars` at 0x10059230 (call at 0x1005a774 in the per-frame update): `GetConsoleVar` ([ecx+0x18c]) -> `atoi` -> byte globals. All are also settable from `autoexec.cfg`, `+name value` on the command line or the engine console.

| Variable | Global | Effect (consumers) | In remake |
|---|---|---|---|
| `God` | 0x100b2474 | at the end of the poll (0x100594f3) the kill counter `[0x100b23ec]` is forced to 0; in the player update 0x10061b90 (`[player+0x37c] >= 0x3c`) health `[0x100b2390]` := max `[0x100b2394]` | missing |
| `FullStamina` | 0x100b2470 | 0x10061f18: stamina `[0x100b2398]` := max `[0x100b239c]` every frame | missing |
| `Invisible` | 0x100b2476 | NPC perception: 0x10043720 (`can see` returns 0), 0x10045df0, 0x100499ba skip their work | missing |
| `DrawKilledCount` | 0x100b2473 | 0x1003ae60: HUD text with the kill counter `[0x100b23ec]` | missing |
| `DrawPaths` | 0x100b2471 | 0x1003d9f9: debug overlay of the level path network | missing |
| `DrawPostacPaths` | 0x100b2472 | 0x1004ae39: debug overlay of the NPC paths | missing |
| `runworld` | `[this+0x278/0x27c]` | 0x10059b6c: name of the world to run at start; default `worlds\nic` (0x1006e090, the empty menu-background world `nic.dat`) | remake has its own CLI |
| `locale` | 0x100b247c | see autoexec table | n/a |
| `Sound`, `Speech`, `Music`, `CInvertMouse`, `CWeaponBob`, `CAutoQuickSave`, `ModelShadow_proj_enable` | 0x100b23f6 / 23f5 / 23f4 / 247e / 2480 / 247f / 23f8 | normal options | USED |

Effort: `God`/`FullStamina`/`Invisible`/`DrawKilledCount` are one-line flags once the option parser (`options.rs::parse_autoexec`, which already keeps unknown lines) gives them a field: S. The two path overlays need the exported path networks (`*.pth`, `patch_nav_graphs.py`) drawn as gizmos: M. Note that the retail `God` does not make the player immune to damage events that do not go through the health refresh (falls/instant kills were not verified).

### Lithtech.exe command line and console (engine-internal)
Command line parsed at 0x404ef0 (strings 0x57fa4c..0x57fc50): `-config <file>` (default `autoexec.cfg`), `-display <file>` (default `display.cfg`, not present in the install), `-noinput`, `-rez <path>` (repeatable; `engine.rez` is added by default), `-world <name>`, `-host`, `-breakonerror`, `-windowtitle <t>`, `-workingdir <dir>`, `-cmdfile <file>` (runs a console command file, 0x57fc14), `+sounddll <name>` (force a `.snd` driver), `+<cvar> <value>` (sets any console variable, e.g. the launcher's `+screenwidth`).
On exit, if `save\auto.sav` exists, it starts `launcher.exe` (0x405f8d).
Stock engine console commands present (0x182d..): `MoveConsole`, `ShowVersionInfo`, `Exec`, `ReadHistory`, `WriteHistory`, `ClearHistory`, `HeapCompact`, `LogTextureInfo`, `RebindTextures`, `ResizeScreen`, `RestartRender`, `RestartConsole`, `ListCommands`, `RCom`/`RenderCommand`, `UpdateServer`, `SSFile`, `AddAction`, `RangeScale`, `Scale`, `Bind`, `RangeBind`, `EnableDevice`, `World <name>`, `ModelAdd`, `serv`, `quit`, `ListInputDevices`, `BlastServer`. Stock cvars of interest: `ConsoleEnable`, `ForceConsole`, `ConsoleAlpha`, `Console_FontTexFile`, `TimeScale`, `Wireframe`, `WireframeModels`, `FogEnable`, `LODScale`, `ShowTicks`, `ShowRunningTime`, `NullRender`, `NewPlayerPhysics`, `Screenshot`. How the console is opened in retail (key binding) is not bound in the shipped `rangebind` list and was not determined.

## 6. `Engine.REZ`
RezMgr v1 archive (`RezMgr Version 1 Copyright (C) 1995 MONOLITH INC.`; 102,162 bytes; directory at 0x18edd, 53 bytes). Entries (resource id, offset, size):
`CONSOLE/CONSOLE_FONT.DTX` (10000000, 65,700 bytes: 128x128 32-bit console font), and the built-in render styles `RENDERSTYLES/` `RS_ALPHABLEND_TEXALPHA.LTB` (621), `RS_ALPHABLEND_TFACTORALPHA.LTB` (621), `RS_ALPHATEST_GREATER.LTB` (621), `RS_SPHEREENVMAP_ADD.LTB` (1727), `RS_SPHEREENVMAP_MODTEXALPHA.LTB` (621), `RS_TWOTEXTURE.LTB` (1727), `RS_VERTEXSPECULAR.LTB` (621), `RS_TOON_VS.LTB` (7762), `RS_HALO_VS.LTB` (8674), `RS_DOT3BUMP_DIR_VS.LTB` (12198). Lithtech.exe always mounts it (`-rez . -rez engine.rez`, `NoDefaultEngineRez`); the console font is only drawn when the engine console is on; the render styles are fall-backs for models whose own `rs\*.ltb` is missing. No shaders/fonts beyond these. The remake implements its own materials (`docs/retail-visual.md`); nothing in the remake reads Engine.REZ. Status ENGINE-INTERNAL.

## 7. `clientfx.fxd`
A PE DLL (built 2001-10-02, exports `SetMasterDatabase fxDelete fxGetAnimName fxGetNum fxGetNumAnims fxGetNumTypes fxGetRef fxGetType fxSetDetail fxSetMultiplay fxSetParams fxSetPlayer`). It is the LithTech SDK effect **type library**, not a bank of named effects: `fxGetNum` returns 15 (0x10005d20), `fxGetNumAnims` 6 (0x10006380). The 15 types (fxGetRef order) with the property names of their strings (grouping by the string-pool order, so approximate):

| FX type | Main parameters |
|---|---|
| `PlayRandomSound` | `Sound`, `NumRand`, `%s%d.wav`, radii, priority, volume |
| `PolyTrail` | `UseMarkers`, `Normal` / `ParentRotate`, `SectionInterval`, `TrailLen`, `SectionLifespan`, `UAdd`, `TrailWidth`, type `CameraFacing/AlongNormal/ParentAlign` |
| `FallingStuff` | `Velocity`, `Radius`, `WindAmount`, `WindDir`, `Stretch`, `StuffLifespan`, `StuffPerEmission`, `EmissionInterval`, `ImpactCreate/ImpactSprite/ImpactScale1/2/ImpactLifespan/ImpactPerturb`, `PlaneDir`, `PerturbType` None/Sine/Pendulum |
| `Null` | none |
| `LTBBouncyChunk`, `BouncyChunk` | `Model`, `Skin`, `ChunkSpread`, `ChunkSpeed`, `ChunkDir`, `Gravity`, `Amount`, `ChunkSound`, `RotateAdd`, `Offset`, `Interpolate`, `UpdatePos` |
| `WonkyFX`, `CamJitter` | shared names `xMultiplier`, `yMultiplier`, `Reps` (split between the two not identified) |
| `PlaySound` | `Sound`, `Loop`, `PlayLocal`, `Priority`, `Volume`, `OuterRadius`, `InnerRadius`, `MultiPlaySound`, `%s.wav` |
| `DynaLight` | not identified (`Flicker` is nearby in the string pool) |
| `LTBModel` | `Shadow`, `FadeTime`, `SmoothFade`, `NoBackfaceCulling`, `EnvironmentMap`, `Facing`, `RenderStyle` None/Alpha/Add/Halo, `Skin1..8`; render styles `rs_alphablend_vertandtexalpha_1tex`, `..._attaddblend_1tex`, `rs_halo_vertandtexalpha_2pass_1texstage_vs` |
| `Sprite` | not identified (sprite file, `Stretch`) |
| `SpriteSystem` | `AType` Norm/Add, `EmissionType` Point/Plane/Sphere/PlaneOut/Cone/Circle, `SpriteLifespan`, `SpritesPerEmission`, `StretchU/V`, `UseParentOrientation` |
| `Lightning` | `Type` Single/MegaZap/Ball, `Texture`, `NumLightning`, `NumBreaks`, `Width`, `LightningLifespan`, `Perturb`, `Direction`, `Flash` |
| `ParticleFlurry` | not identified |

Animation names (6): `Large Spin X/Z`, `Large Spin Y/Z`, `Large Spin X/Y`, `Small Spin X/Z`, `Small Spin Y/Z`, `Small Spin X/Y` (plus `Lightning Ball`, `Lightning Bolt`, `Unknown Animation`). Attach-point enum: `Fixed, Follow, LeftHand, RightHand, LeftFoot, RightFoot, Head, Tail, u1..u10`.
**Which ones the retail game spawns: none.** Evidence: none of `Lithtech.exe`, `cshell.dll`, `object.lto`, `server.dll` contains `clientfx`, `.fxd`, or any `fx*` export name except the common `SetMasterDatabase` (every game DLL exports that); none of the 31 worlds contains a `CClientFX` or `CAttacher` object (`classhlp.but` describes them, the retail `object.lto` does not register them); no script names an FX. The retail effects (muzzle flash, impacts, smoke, blood, rain/snow) are hard-coded in cshell/object.lto and are what `output/retail_effects.json`, `tools/export_effects.py` (sprite/impact tables from `items.txt` + cshell constants), `tools/export_env_effects.py` (DTX CommandString `DetailTex/EnvMap/EnvMapAlpha`, Lithtech.exe 0x465080..0x4652dd) and `crates/level-viewer/src/fx.rs` reproduce. No remake work is derived from the FXD. Status UNUSED-IN-RETAIL.

## 8. Editor leftovers: `classhlp.but`, `ClassIcons/`, `TextureEffectGroups/`, `LightAttenuationPresets/`
* `classhlp.but` (16,611 bytes, 2001-06-12): DEdit class-help file, `[Class]` sections of `Property = "tooltip"` lines (17 classes: BaseClass, DemoSkyWorldModel, DirLight, Light, Brush (28 properties), SkyPointer, OutsideDef, ObjectLight, InsideDef, StartPoint, CWorldPropSvr, StaticSunlight, CBaseObjectSrvr, CPropModel, CClientFX, CAttacher, CWorldModel). Only the first ten exist in the retail `object.lto`; the last seven are stock SDK classes (not registered, absent in every DAT). Worlds really use `DirLight` (12 worlds, 137), `ObjectLight` (23 worlds, 876), `DemoSkyWorldModel` (4 worlds), `StaticSunLight` (19), `StartPoint` (all 31), `WorldProperties` (26), and many `Light`s. Nothing reads the file at run time; it names no property the game uses that is not already known (the remake's exporters read the DAT property names directly).
* `ClassIcons/*.dtx`: 13 x 21,924 bytes, 64x64 32-bit DTX with 4 mips (CAttacher, CClientFX, CPropModel, CWorldModel, CWorldPropSrvr, DemoSkyWorldModel, DirLight, InsideDef, Light, ObjectLight, SkyPointer, StartPoint, Terrain): DEdit object-tree icons. UNUSED-IN-RETAIL.
* `TextureEffectGroups/`: LithTech 2.x texture-effect system. `.txt` scripts (`UVPan`: `Mat02 = SpeedX*Time; Mat12 = SpeedY*Time`; `UVRotate` (RotPerS); `UVWarble` (XFreq,YFreq,XScale,YScale); `WSPosRotateAroundY` (CenterX,CenterZ,ScaleX,ScaleZ,RotPerS)) are compiled by `TSCompiler.exe /file X.txt /out X.tfs` (`!compile pan.bat`: only UVPan) into `.tfs` bytecode; `.tfg` = a group: script path `\TextureEffectGroups\UVPan.tfs` + float parameters (`pan.tfg` 0.002, 0.004 per second; `pan_szybszy.tfg` 0.002, 0.006; `pan_automat.tfg` 0.0153, 0; `huj.tfg` 1,0; `warble.tfg` UVWarble 0.25,0.25,0.15,0.15). Lithtech.exe has the directory string `TextureEffectGroups\` and object.lto the DTX/model property `TextureEffect`. Used by worlds: `pan.tfg` in `RH9-fabryka.dat`, `rh3-miasteczko0.dat`; `pan_szybszy.tfg` in `burmistrz2.dat`, `podziemia1a.dat`, `wiez_wn1.dat`; nothing references `pan_automat/huj/warble`. Remake: `tools/lightmaps.py` exports the groups, `retail_world.rs` (line 271) animates `UVPan` (`docs/retail-visual.md`). Others are unused, no work needed. `DirTypeTextureScripts` (0 bytes) is the DEdit type marker.
* `LightAttenuationPresets/Default.lta`: `( lightattenuationpreset ( coefs 1.0 0.0 19.0 ) ( exps 0.0 0.0 -2.0 ) )`. DEdit expands a preset into per-light properties; every light object of the DATs carries `AttType "Default"`, `AttCoefs (1,0,19)`, `AttExps (0,0,-2)` and the enum `Attenuation` (`D3D` 1619, `Quartic` 857, `Linear` 147 string hits), read by object.lto (0x2b621: `Attenuation Quartic AttExps AttCoefs ... LightAttenuation`). The remake (baked lightmaps + `LightRadius` point lights) does not use them; how the engine turns coefficient/exponent triples into a falloff was not reverse engineered.

## 9. Audio files: `SndDrv.dll`, `dx8.snd`, `cdaudio.dll`, `l3codeca.acm`, `mss32.dll`
* Both `SndDrv.dll` (2002-10-11) and `dx8.snd` (2002-10-10) export `SoundSysDesc`/`SoundSysMake` and describe themselves as `DirectSound ( DirectX 8 )`. Lithtech.exe enumerates `*.snd` (0x4b5880, `Could not find suitable .SND sound driver.`) and also names `SndDrv.dll` (0x59121c); `+sounddll` overrides. Device entries: `SndDrv.dll`: `DirectSound Default/Hardware/Software`; `dx8.snd` adds `EAX Hardware`. Engine option strings: `Max samples`, `Reverb`, `ReverbDelay`, `Reflections`, `RoomRolloffFactor`, `AirAbsorptionHF`, `DecayTime`, `Diffusion`, `EAX environment selection` (Lithtech.exe 0x590d98..). Hard-coded formats: streaming buffer 22,050 Hz, 16-bit, stereo (WAVEFORMATEX built at `dx8.snd` 0x100041f3..0x10004217: nChannels 2, rate 0x5622, align 4, avg 0x15888, 16 bit; same in `SndDrv.dll` 0x10004568) and a default 11,025 Hz, 8-bit, mono format (`dx8.snd` 0x10005700..0x1000572b: rate 0x2b11). MP3-in-WAV (format 0x55, which the retail music/speech files use) is decoded through `MSACM32` with `L3CODECA.ACM` (`Sound System Startup - Missing .mp3 codec`) and `IMAADP32.ACM`. The launcher and `autoexec.cfg` select no driver (the engine's selection logic was not followed). 3D volume/pan uses the engine curve documented in `docs/retail-audio.md`.
* Remake: rodio/Bevy audio at the device rate, its own MP3 decoding and linear falloff (`audio.rs`); the optional "Javított 3D hangzás" approximates the engine pan. Sampling rate/EAX/reverb settings are not reproduced (no EAX in modern Windows); status ENGINE-INTERNAL, no work.
* `cdaudio.dll` (2001, exports `MusicDLLSetup`, imports MFC42/WINMM `mci*`/`aux*`): Lithtech.exe's music DLL (default name at 0x581aa4, used at 0x40d945/0x40e2c4; error codes `LT_MISSINGMUSICDLL`/`LT_INVALIDMUSICDLL`). The game's own music is wav/mp3 via the sound driver (`sounds/muza`, cvar `Music`); CD tracks are not used. `l3codeca.acm` only as above. `mss32.dll` (Miles, 2000-08-17) has no importer (older LithTech sound system): UNUSED.

## 10. `ReadMe.txt` (gameplay-relevant facts only)
* Default controls (table in the file): Forward W/Up, Back S/Down, Step left A/Left, Step right D/Right, **Jump MouseButton1**, Crouch LControl/RControl, Run LShift/RShift, Toggle Run Q, Action/Pick up Space/E, Fire MouseButton0, Alt Weapon MouseButton2/Alt, Reload R, Next Weapon G, Previous Weapon F, Inventory C, Char Info Z, Player Attributes X. (Matches `keys.cfg`; the remake's Space-jump / right-mouse alt fire are owner-requested deviations, `docs/retail-input.md`.)
* Performance menu ("Performance and Display"): `Shot Debris`, `Insignificant objects`, `Insignificant characters`, `Enemy lights` can be turned off (all four exist in the remake's options; `Enemy lights` has no effect, backlog B095).
* Launcher tips: resolution 640x480 / 800x600 / 1024x768, 16/32 bit, max texture 512x512 -> 256 or 128.
* Each saved game is about 300 KB (quick.sav is 318,034 bytes), recommended 100 MB for saves. "Ran out of virtual memory" error note. Windows 95/NT unsupported; DirectX 8.1 required.
* No cheat codes, hidden features or bug list in the file.

## 11. Resource DLLs: `ltmsg.dll`, `CRes.dll`, `sres.dll`
* `ltmsg.dll` string table (LithTech error messages, only reachable when the engine fails):

| Id | Text | Id | Text |
|---:|---|---:|---|
| 1 | Internal LithTech error: %1!d!. | 2 | Missing WorldModel: %1!s!. |
| 3 | Can't load game resources in %1!s!. | 4 | Can't initialize DirectInput. |
| 5 | Unable to restore video mode. | 6 | User canceled. |
| 7 | %1!s! is an invalid world file. | 8 | Missing model file %1!s!. |
| 9 | %1!s! is an invalid model file. | 10 | Missing sprite file %1!s!. |
| 11 | %1!s! is an invalid sprite file. | 12 | No game resources specified. |
| 13 | Missing world file %1!s!. | 14 | Unable to restore object (class %1!s!). |
| 15 | Server error: %1!s!. | 16 | Error loading render DLL %1!s!. |
| 17 | Error copying file %1!s!. | 42 | Unable to create server shell. |
| 66 | Object DLL %1!s! is an invalid version (%2!d!).  Current version is %3!d!. | 76 | Invalid shell DLL %1!s!. |
| 77 | Invalid shell DLL version for %1!s!.  DLL version is %2!d!, current version is %3!d!. | 78 | Missing shell DLL %1!s!. |
| 82 | Can't create client shell. | 89 | Unable to initialize sound system. |
| 92 | Got an invalid server packet. | 93 | Missing class: %1!s!. |
| 94 | Unable to initialize network driver %1!s!. | 95 | Network protocol version mismatch - your version: %1!d!, server's version: %2!d!. |

* `CRes.dll` and `sres.dll` (client/server resource DLLs, 2002-10-14): same code, 16-byte empty `.rsrc`, no dialogs, no strings, no menus. Nothing to extract; the different MD5 is only the build stamp/ordering.
* The game's own messages/menus are not in DLLs: they live in `scripts/locale/*.txt` and `text_keys.txt` (`retail-text.md`). No menu/dialog strings exist in ltmsg/CRes/sres beyond the table above. Remake need: none (the remake reports its own errors).
* Debug runtime `MFC42D.DLL` (2002-04-02), `MSVCIRTD.DLL`, `MSVCRTD.DLL`: only import each other; shipped by mistake from a debug build. Ignore.

## 12. `save/` directory
* `quick.sav` (318,034 bytes): layout found by inspection: `SYSTEMTIME` (8 x u16: 2003-09-19 15:18:21.036, dow 5) at 0x00; the tag `RatHunt save game.\n` at 0x10; a 0x60 byte followed by seven floats of 100.0 (player stat values) from 0x23; a u32-length-prefixed world name `worlds\rh3-miasteczko0` (length at 0x87, text at 0x8b; the quick save was made in the first town level); a 320x240 32-bit (BGRX) thumbnail from about 0xf0 (307,200 bytes); then ~10.6 KB of length-prefixed mission variable names with values (`KnajpaMarkerWyjscia`, `MarloniPogonil`, `PierwszyChiniolZagadal`, ... the `bool` variables of `gameai.txt`), which is what makes a save ~300 KB. cshell strings: `save\quick.sav`, `save\save`, `.sav`, `save\save99.sav`, `>SGPlayerHealth`, `>SGAt`, `>SGCreated`; `c_savemgr` 0x1004b500..0x1004d9a0 (`docs/retail-menus.md`). The remaining layout (NPC/object state, inventory) was not decoded.
* `quick.srv`: 4 zero bytes (server-side companion record, unused here).
* Remake: `savefile.rs` keeps the slot model (`quick.sav`, `save0..99.sav`, time stamp, health, level, 320x240 thumbnail) but stores a JSON checkpoint behind its own magic (16-byte time stamp is **not** written), so retail saves cannot be loaded and remake saves are not retail-compatible. Importing a retail save would need the variable list + world name + player stats parsed (M); not needed for the game itself.

## 13. `misc/` directories and DirType markers
| Directory | Files / bytes | Referenced by (cshell strings) | Remake |
|---|---|---|---|
| `misc/Items` | 119 / 322 KB | `items.txt` inventory icons (`misc\items\*<640|800|1024>.pcx`, 161 script lines) | used (`export_inventory.py`) |
| `misc/fonts/Mincho` | `table.dtx` 1.39 MB (used), `table.txt` (glyph order incl. Hungarian áíéóúÚÖüőöÉÍÜŐÁÓű; cshell 0x1006dfc4 + "table.txt"), `table.pcx` (2002, old), `table.tga` (2003 source) | `misc\fonts\mincho\` | used (dtx + txt) |
| `misc/fonts/cyfry`, `info` | 14 / 75 files per resolution | `misc\fonts\cyfry\<640|800|1024>\`, `info` | used (panels) |
| `misc/fonts/Un` (`Normal`, `Podswietl`, `big`), `misc/fonts/sub` | 77-80 files per resolution and set | no path string in cshell; `export_inventory.py` FONTS exports `un`, `un_highlight`, `un_big`, `sub` | exported; reachability in retail via a built path not verified |
| `misc/loading_l` | 28 PCX, 17.6 MB | `misc\loading_l\` + `loading_phases.txt` | used (loading screens); with `locale 0` cshell would ask `misc\loading\` (absent) |
| `misc/menu` | 3 cursor PCX + `kursor.dtx` | `misc\menu\kursor.dtx` | used (gold cursor) |
| `misc/menu_l` | 58 / 16 MB | `misc\menu_l\*` | used |
| `misc/outro` | 21 PCX (credits/outro 1-4 at 640/800/1024, `demo_outro_*`) | `misc\outro\outro1_`, `credits_`, `credits_l_` | used; `demo_outro_*` (3) not referenced |
| `misc/panel`, `panel_l` | HUD 56 + ammo + fonts + celownik (identical 5.5 MB HUD sets in both) | `misc\panel\HUD\..`, `misc\panel_l\..` | `_l` used |
| `*/DirTypeModels/Sounds/Sprites/Textures/Worlds/RenderStyles/TextureScripts` (121 files) | 0 bytes x 120 | editors only (no binary contains the name) | none |

`DirTypeModels` (8), `DirTypeSounds` (15), `DirTypeSprites` (8), `DirTypeTextures` (78 + 9 lower-case `dirtypetextures`), `DirTypeWorlds`, `DirTypeRenderStyles`, `DirTypeTextureScripts` are the LithTech directory-type markers DEdit/ModelEdit use to decide which file types a folder holds. No runtime module contains the string. The single non-empty exception is `textures/chinatown/DirTypeTextures` (finding H11).

## 14. Other unusual files (not assets)
* `rs/vs_aniso_basetex_3light.ash`: 5 spaces, a leftover of a `.vsh` typo; the `rs/*.lta` sources (30) sit next to 14 compiled `.ltb` and 5 `.vsh` vertex shaders (`vs_basic_3light`, `vs_dot3bump_dir_3light`, `vs_halo`, `vs_normalexapnd`, `vs_toon_3light`) referenced by the `.lta` (covered by `retail-visual.md`).
* `textures/*/dirtypetextures`, `sprites/DirTypeSprites`, `worlds/DirTypeWorlds`: markers as above.
* `MSVCP60.DLL`/`msvcrt.dll` are dated 2001-08-24 and are real dependencies of Lithtech.exe.
* No `display.cfg`, `resource.rez`, `error.log` exist (created/used only by the engine on demand).

## 15. Could not be determined
1. The original (pre-`cipher.exe`) state of `GYARI/scripts` (InstallShield cabinets cannot be unpacked here); which of the 122 files were shipped encoded.
2. Meaning of `showframerate 2` versus 1, the origin of `ServerProps`, and the exact engine falloff formula for `AttCoefs/AttExps` (and whether `Attenuation` enum or the coefficients win).
3. How the engine console is opened in the retail build (no bound key found) and whether `ConsoleEnable`/`ForceConsole` work without the dropped `+` switch.
4. The full binary layout of `quick.sav` after the thumbnail (object/NPC state), and the purpose of `quick.srv`.
5. Exact property-to-type mapping of `clientfx.fxd` (the assignment above follows the string-pool order), and what (if anything) ever reads `c:\s_pov.bin` or creates `save\auto.sav`.
6. Whether `God` also blocks environmental/instant-kill damage (only the health refresh at 0x10061bb4 was followed).
7. Whether the launcher touches the registry (only statically linked MFC imports seen).
8. Reachability of `misc/fonts/Un` and `misc/fonts/sub` in retail code (no literal path string).
