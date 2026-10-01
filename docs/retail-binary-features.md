# Retail binary features: what cshell.dll / object.lto implement that the remake does not, and dead code in them

**Összefoglaló (magyarul)**

- A gyári `cshell.dll` minden frissítésnél (`Update` 0x1005a640 -> 0x10059230) beolvas 12 konzolváltozót (és induláskor további 2-t: `locale`, `ModelShadow_proj_enable`) az `autoexec.cfg`-ből / konzolból (`GetConsoleVar` + `atoi`): `Music`, `Speech`, `Sound`, `locale`, `ModelShadow_proj_enable`, `CInvertMouse`, `CWeaponBob`, `CAutoQuickSave` (ezeket a remake is ismeri) és a fejlesztői csalók `God`, `FullStamina`, `Invisible`, `DrawKilledCount`, `DrawPaths`, `DrawPostacPaths` (ezeket a remake nem olvassa, az `autoexec.cfg`-ben csak változatlanul megőrzi). A szállított `autoexec.cfg` már tartalmazza a `"God" "0"` sort: egy szövegszerkesztős átírás elég a gyári csalásokhoz.
- Nincs billentyűs csalókód és nincs `OnKeyDown`: a parancsok lekérdezéssel jönnek (`IsCommandOn`), az `OnCommandOn` (0x1005aba0) csak a Menü (70), Kilépés (250), Gyorsmentés (251), Gyorsbetöltés (252) parancsokat kezeli. A "dupowaty" szöveg nem csalókód, csak egy dummy argumentum a párbeszéd-visszaállításnál (0x1005aeac). Halott fejlesztői maradványok: a kötetlen 20-as parancs (szuperugrás, 0x10060c1b), egy soha be nem állított "repülő / fizika-kikapcsolás" jelző (0x100b2475) és az F/G fegyverváltás üres csonkja (0x1005ab50).
- Az ügyfél 23 SFX-üzenetkezelőt tartalmaz (0x23..0x75); az `object.lto` mind a 20 elküldött üzenetére van kezelő, de 3 kezelőhöz (0x2d, 0x37, 0x6d) a szerver soha nem küld üzenetet (halott; a 0x37 kezelő egy üres `ret 0x14`, a 0x6d a soha nem aktív `d_emiter_iskier` kezelője).
- Árva (sehonnan nem hivatkozott) függvény egyik DLL-ben sincs (a CRT-belépőn kívül): minden kód vtable-ből, osztálytáblából vagy hívásból elérhető; a halott kód viselkedés szintű (jelzők, kulcsszavak, osztályok).
- Script-kulcsszavak: a `postacie.txt` fejlécében/fázisaiban a `drop_weapon` (55 sor), `nie_wspinaj_sie`, `ignoruj_markery_niechodzenia` a gyári elemzőben nem létezik (figyelmen kívül marad), viszont a remake a `drop_weapon`-t valódi parancsként kezeli, és **csak ez váltja ki az NPC fegyverejtést**: a gyári `Kill` (0x10042ca0) minden felfegyverzett halott NPC-nél ejt fegyvert (kivéve `nie_zostawiaj_gana`), így 3 karakternél (`straznik przy celi`, `straznik na klatce`, `policjant z pala`) a remake nem ejt fegyvert.
- A `gestosc_sciezek`, `nie_sprawdzaj_drzwi`, `pure_shooter`, `blyski_postaci` hatása most ki van bontva (lásd 5. fejezet): `nie_sprawdzaj_drzwi` kikapcsolja az útvonal-élek ajtójelzőit az egész szinten (csak `rh3-miasteczko0`), `pure_shooter` (csak a nem szállított `test2`) letiltja a küldetés-szkript futtatását, `blyski_postaci` halott, `gestosc_sciezek` a csomópont-szomszédság ellenőrző doboz/sűrűség szorzója (alap 2.0, prológ: 4.0).
- A 20 legfontosabb hiányzó gyári viselkedés a 7. fejezetben, rangsorolva; a konzol megnyitó billentyűjét a Lithtech.exe-ből nem sikerült azonosítani (a konzol be van kapcsolva: `ConsoleEnable` alapérték 1).

English tables follow. All addresses are virtual addresses of the named module (`C` = cshell.dll, `L` = object.lto, `E` = Lithtech.exe). Status codes:
**USED-IN-REMAKE**, **USED-BY-RETAIL-BUT-MISSING-IN-REMAKE**, **UNUSED-IN-RETAIL (dead code)**, **ENGINE-INTERNAL**. Effort S / M / L. "B0xx" = row of docs/backlog.md.
Method: capstone linear sweep of both DLLs, call / jmp / vtable / relocation cross-reference index (thunks resolved, `_initterm` table and class-definition tables as roots), string xrefs, then the parser bodies were read; scripts were compared by first token with `output/decoded_scripts/*.txt` (identical to the plaintext `GYARI/scripts/ai/gameai.txt` and `dialogi.txt`; the per-level `ai_*.txt` and `postacie/*.txt` files of this install are 0 bytes and every `include` line in `gameai.txt` is commented out with `//`).

## 1. Console variables and commands (cshell.dll, Lithtech.exe)

**How the game reads them.** `C 0x1005a640` (`Update`) calls `C 0x10059230` every frame (call at 0x1005a774): per name `ILTClient::GetConsoleVar` (vtable +0x18c) -> `GetVarValueString` (+0x8c) -> `atoi` -> `setne` into a byte. Any non-zero value is "on"; a variable that does not exist leaves the byte unchanged (constructor default `C 0x1001aa80`). `C 0x100591b0` does the same once for `locale` and `ModelShadow_proj_enable` (called from 0x10059675). **How a player sets them:** plain lines `"Name" "value"` in `autoexec.cfg` (the shipped file has `"God" "0"`, `"Music" "1"`, `"locale" "1"`, `"CWeaponBob" "1"`, ...; the game rewrites it), a `+Name value` command-line argument (launcher `lanuchp.txt` uses `+screenwidth 800 ...`; the engine also knows `-rez`, `-cmdfile`, `display`, `config`, `noinput`, `world`; E 0x57fa4c..), or the engine console. The console exists (`ConsoleEnable`, its default appears to be 1, E variable table at 0x584a2c; `ConsoleBufferLen`, `ConsoleHistoryLen`, `NumConsoleLines`, `ForceConsole`, console texture `console.pcx` E 0x4959c7; commands `Exec`, `World <name>`, `quit`, `Bind`, `RangeBind`, `AddAction`, `EnableDevice`, `ListCommands`, `MoveConsole`, `ShowVersionInfo`, ...). **The key that opens it was not determined** (no console key appears in `autoexec.cfg`; it is handled inside the engine; `NumConsoleLines` is 0 in the shipped autoexec). cshell.dll registers **no console programs of its own** (no `RegisterConsoleProgram`); it only reads the variables below and pushes engine commands as strings (`runworld <name>` 0x10059b6d, `ModelShadow_proj_enable 0|1` 0x1002aa53/0x1002aa7c, `CInvertMouse`/`CAutoQuickSave`/`CWeaponBob` `%d` from the menu callbacks 0x1002a4c0/0x1002a9b0, and `FarZ`, `FogEnable`, `FogNearZ`, `FogFarZ`, `FogR/G/B`, `SkyFogEnable/NearZ/FarZ` from the SFX 0x65 world-properties handler 0x1005b460/0x1005b580 and the camera function 0x1002cdb0).

| Variable (string) | Byte / default | Read at | What it does | Remake | Impact / effort |
|---|---|---|---|---|---|
| `Music` | `[0x100b23f4]`, 1 | C 0x10059251, 0x1005927a -> 0x10059950 | start / stop the music track when it changes; menu option | USED-IN-REMAKE (`options.rs`) | - |
| `Speech` | `[0x100b23f5]`, 0 (autoexec 1) | C 0x10019350, 0x10019420, 0x100135b0, 0x100256c0 | gates dialogue and cutscene speech | USED-IN-REMAKE | - |
| `Sound` | `[0x100b23f6]`, 0 (autoexec 1) | C 0x100597f0, 0x100598b0, 0x1002e520 | gates sound effects | USED-IN-REMAKE | - |
| `locale` | `[0x100b247c]`, 0 (autoexec 1) | C 0x10023ac0.. (every menu / panel loader) | 1 = localised art and texts `misc\menu_l`, `misc\panel_l`, `misc\loading_l`; 0 = fallback dirs `misc\menu` (4 files) and `misc\panel` (5 files) which are incomplete in this install, so 0 is unusable | ENGINE-INTERNAL / not needed (remake always uses the `_l` art; the line is preserved untouched) | none / S |
| `ModelShadow_proj_enable` | `[0x100b23f8]`, 0 | C 0x100591b7, 0x10025010 | projected model shadow; the menu also sends it to the engine (object.lto `cien.dtx`) | option stored only, no shadows (B015, B095) | M / M |
| `CInvertMouse` | `[0x100b247e]`, 0 | C 0x10054d0e (mouse look), 0x10025080 | flips pitch | USED-IN-REMAKE | - |
| `CWeaponBob` | `[0x100b2480]`, 1 | C 0x10055cbe | camera / weapon bob | USED-IN-REMAKE (`view.rs`) | - |
| `CAutoQuickSave` | `[0x100b247f]`, 1 | C 0x1005a5d0 (level start) | autosave to `save\quick.sav` when a level starts | USED-IN-REMAKE (`menu.rs`) | - |
| **`God`** | `[0x100b2474]`, 0 | C 0x10061bb4 (`ChangeHealth` 0x10061b90), 0x100594f3 | every health change is replaced by `health = max` (damage and healing ignored); also clears the kill counter every frame (0x100594fe) | USED-BY-RETAIL-BUT-MISSING-IN-REMAKE (line preserved, not read) | cheat only / S |
| **`FullStamina`** | `[0x100b2470]`, 0 | C 0x10061f19 (tail of the stamina change 0x10061e00) | stamina := max after every change (jump cost, running) | MISSING | cheat only / S |
| **`Invisible`** | `[0x100b2476]`, 0 | C 0x10043720 (actor `seen`), 0x10043860 (weapon line of sight), 0x10045df0 (stimulus test), 0x10049950 (contact) | NPCs never see, hear or contact the player | MISSING | cheat only / S |
| **`DrawKilledCount`** | `[0x100b2473]`, 0 | C 0x1003ae60 (HUD), counter `[0x100b23ec]` (+1 per killing shot 0x10006b5c, reset at new game 0x10024c78) | draws the kill number on the HUD; the counter itself is shown on the Z panel (`PIPEnemiesKilled`) | counter USED-IN-REMAKE, overlay MISSING | debug / S |
| **`DrawPaths`** | `[0x100b2471]`, 0 | C 0x1003d9f0 (called from the frame renderer 0x1005a8b0 at 0x1005a9bb) | debug lines of the path graph | MISSING | debug / M |
| **`DrawPostacPaths`** | `[0x100b2472]`, 0 | C 0x1004ae30 (0x1005a9c5) | debug lines of the actors' current paths | MISSING | debug / M |
| (flag, no variable) | `[0x100b2475]`, 0, **never written after the constructor** | C 0x10060252 (controller entry: skips the physics update and goes straight to the command polling at 0x10060414), 0x1006072f (forward key moves along the full camera vector incl. pitch), 0x100614e0 (standing test returns 0), 0x1005f8f0, 0x10060dd0 | a free-fly / physics-bypass developer mode | UNUSED-IN-RETAIL (dead) | none / - |
| (constants) | `[0x100b23f0]` = 100 (percent), `[0x100b23fa]` = 1, `[0x100b2408]` = 768.0 | sound / HUD code | only written by the constructor; never changed | ENGINE-INTERNAL | none |

The `print` and `wlaczmuze` tokens are **gameai.txt action commands**, not console commands (section 5).

## 2. Command / key handlers of the client shell

The shell has **no `OnKeyDown` / `OnKeyUp`** (the vtable slots 0x100590c0..0x100590f0 are `ret` / `xor eax, eax` stubs) and no text cheat entry. Input is (a) polled level-triggered through `IsPressed(slot)` C 0x10011960 (`ILTClient::IsCommandOn`, used by the controller `C 0x10060160`), and (b) `OnCommandOn(int)` = vtable slot 1 = **C 0x1005aba0**, a switch on `cmd - 0x46` (byte table 0x1005ace0, jump table 0x1005accc).

`OnCommandOn` cases (all others 71..249 and 253..255 only latch a press flag at `[this + 0x1b10 + cmd]`, read by the F1..F4 belt code 0x100604f8..):

| Command | Handler | Behaviour | Remake |
|---|---|---|---|
| 70 Menu | C 0x1005ac3f | closes inventory, else character info, else attributes, else opens the in-game menu (0x10024060) | USED-IN-REMAKE |
| 250 Quit | C 0x1005ac33 -> 0x10059cc0 | shutdown (`Dostalem polecenie quit`) | USED (menus branch) |
| 251 QuickSave | C 0x1005abd4 | writes `save\quick.sav` through the save manager 0x10b358f0 (0x1004b550 / 0x1004b840 / 0x1004b5c0), message text from `scripts\locale\shell.txt` (+0x1c10 / +0x1d10), timer `[0x100b2304] = 5.0` | USED (menus branch) |
| 252 QuickLoad | C 0x1005ac27 | sets `[this+0x2119] = 1`; `Update` loads `save\quick.sav` once the level is older than 0x3c frames (0x1005a652..) | USED (menus branch) |

Polled slots (complete list of `IsPressed` call sites; the table in docs/retail-input.md is correct and complete, rows marked `=` agree with it):

| Slot | Poll at | Command | Remake |
|---|---|---|---|
| 1 / 2 / 3 / 4 | 0x10060722 / 0x100608b0 / 0x100609a4 / 0x10060a9d | forward / backward / strafe | = |
| 5 | 0x10060b92 | jump (retail default: right mouse; remake Space, owner) | = |
| 6 | 0x10060ca1 (+ 0x1000fff9, 0x100100e3 in unidentified functions 0x1000ffb0 / 0x100100c0, 0x100139de in the cutscene update 0x10013950) | fire | = |
| 7 | 0x10060cdb | alternate fire | = |
| 8 / 9 / 10 | 0x10060698 / 0x1006063a / 0x10060601 | crouch / run / toggle run | = |
| 11..18 | 0x10060c75 (loop) | weapon 1..8 | = |
| **20** | 0x10060c1b | vertical velocity += 1500 on the ground (super jump), unbound in every keys.cfg | UNUSED-IN-RETAIL (B035), not implemented |
| 21 / 22 / 33 | 0x10060444 / 0x1006047c / 0x100604bf | inventory / character info / attributes | = |
| 27 | 0x100605ea (+ 0x100139ce in the cutscene update 0x10013950, 0x10019bad / 0x10019dba in the `ifaction` / `ifactionhostile` evaluators) | use | = |
| 30 | 0x10060d0f | reload | = |
| 31 / 32 | 0x10060d5d / 0x10060d29 | previous / next weapon: call the empty `ret` at 0x1005ab50 | = (nothing) |
| raw 80..83 | 0x100604f8..0x100605d2 | F1..F4 belt | = |
| 29 Holster | never polled | - | = |

Cheat / hidden-key features: **none on the keyboard**. The only developer hooks are the console variables of section 1, the unbound slot 20 and the dead flag 0x100b2475. `"dupowaty"` (C 0x1006e224) is the placeholder argument of the dialogue reset call `0x100194d0` in `OnExitWorld` (0x1005aeab), not a code word.

## 3. Client-shell SFX messages (object.lto -> cshell.dll) and client -> server messages

Dispatcher: `OnMessage` C 0x1005b1f0 (byte id, switch table 0x1005c70c / 0x1005c6ac for 0x23..0x75; every other id falls to 0x1005c669 = ignore). Senders are the `push 8 / push id / call [msg+0x24]` (WriteBits) pattern in object.lto (every id is sent from exactly one place, 0x66 from two).

| Id | Client handler | Sent by object.lto at (class) | Remake |
|---|---|---|---|
| 0x23 | C 0x1005c382 | L 0x10012610 (`o_cutscene`) | USED (`opening.rs`) |
| 0x26 | C 0x1005b32a | L 0x10016c50 (`o_marker_zmienna`) | USED (`activation.rs`) |
| 0x27 / 0x29 | C 0x1005bda5 / 0x1005bd79 | L 0x10009730 / 0x10009350 (object registry: kill / toggle by id) | USED |
| 0x28 | C 0x1005c2a4 | L 0x1000e800 (`d_lens_flare`) | USED |
| 0x2a | C 0x1005bcde | L 0x100144d0 (`o_marker_death`) | USED |
| 0x2b | C 0x1005bb62 | L 0x10016520 (`o_marker_wykrywacz_postaci`) | USED |
| 0x2c | C 0x1005ba9d | L 0x10017a80 (`o_pauza`) | USED |
| **0x2d** | C 0x1005ba0b (-> 0x1002b7c0, adds a record to the prop manager 0x10760ca0: id, float, position, rotation) | **never sent** | UNUSED-IN-RETAIL (dead) |
| 0x2e | C 0x1005bc51 | L 0x10014af0 (`o_marker_dialog`) | USED |
| 0x2f / 0x30 | C 0x1005b99a / 0x1005b929 | L 0x10015090 / 0x100155e0 (no-respawn / no-walk markers) | USED |
| 0x31 | C 0x1005b6df | L 0x100111e0 (player start: position + `Kierunek`) | USED (`retail-movement-audit.md`) |
| 0x33 | C 0x1005b418 | L 0x100019a0 (`b_door` auto-close watcher) | USED |
| **0x37** | C 0x1005b2a7 -> 0x100305b0 which is a bare `ret 0x14` | **never sent** | UNUSED-IN-RETAIL (dead stub) |
| 0x38 | C 0x1005b23d | L 0x10011e10 (positional sound helper) | USED |
| 0x65 | C 0x1005b460 (+0x1005b580 sky fog): `FarZ`, `FogEnable`, `FogNearZ`, `FogFarZ`, `FogR/G/B`, `SkyFog*` | L 0x1001ac20 (`WorldProperties`) | USED (fog from the scene data) |
| 0x66 | C 0x1005c63d (level change) | L 0x100020c0 (`b_door` `Skok_do_levelu`), 0x10012c80 (`o_detektor_konca_levelu`, never placed) | USED |
| 0x67 | C 0x1005b3d3 | L 0x10010200 (`LightGroup`) | USED |
| **0x6d** | C 0x1005c458 (string, position, 5 words, 3 vectors -> 0x1001afe0 on manager 0x100b26b8: a spark / particle effect) | **never sent** (the `d_emiter_iskier` class, L 0x1000b610, only reads its properties) | UNUSED-IN-RETAIL (dead; spark emitters inert) |
| 0x6f / 0x70 / 0x75 | C 0x1005c0fa / 0x1005bf8a / 0x1005bdc2 | L 0x1000ab80 (`d_emiter_dymu`), 0x1000bfa0 (`d_emiter_opadu`), 0x1000cbc0 (`d_fala3d`) | USED (`decorations.rs`) |

Sent but unhandled: **none**. Handled but never sent: 0x2d, 0x37, 0x6d.

Client -> server (server handler `L 0x10011bc0`, the player object message function): 0x20 (C 0x1004cd50, loads `<name>.srv` server state, `L 0x10008d50`, fopen "rb"), 0x22 (C 0x1004bb60 serialises the actors, `L 0x10009060`, fopen "wb": the two halves of the retail save game), 0x21 use (C 0x1005faa0 -> L 0x10011880), 0x27 prop died (C 0x1002e690 -> L 0x100096d0), 0x29 / 0x33 activate by id (C 0x1002ca40, 0x1002cc30, 0x1002cf00, 0x1002f750 / 0x1002ff40 -> L 0x10008ce0), 0x32 player position (C 0x10061190), 0x34 NPC door point (C 0x10042060, 0x100483c0 -> L 0x10009270), 0x66 level name (C 0x1005b020 -> L 0x10011b10). The server also accepts **0x35 as a no-op** (L 0x10011be5) that no client code sends. The remake replaces the `.srv` / save protocol by its JSON saves (B096).

## 4. Dead code and unused data in the two DLLs

**Functions.** With corrected function boundaries (16-byte aligned starts after `nop` padding, all call targets, all relocation-referenced code pointers, thunk chains, `_initterm` table and class definition records as roots, vtables live only through a referenced constructor) there is **no orphan function** in cshell.dll (2 100 starts) or object.lto (1 900): the only unreachable bytes are the CRT `DllMain` entries (C 0x10064b58, L 0x100236ac, 328 bytes each, called by the loader) and padding thunks. The linker dropped everything unreferenced; the dead code of these DLLs is therefore **behavioural** (handlers, flags, keywords, classes). A function-level "ranked list" does not exist; the behavioural equivalents are:

| Dead item | Address | Evidence | Guess |
|---|---|---|---|
| free-fly / physics-bypass flag | C 0x100b2475 (readers 0x10060252, 0x1006072f, 0x100614e0, 0x1005f8f0, 0x10060dd0) | only a constructor write 0x1001ab4c | noclip / spectator mode of the programmers |
| command 20 super jump | C 0x10060c1b | no default binding in keys.cfg / autoexec | developer hop |
| F / G weapon cycling | C 0x10060d22 / 0x10060d5d -> 0x1005ab50 (`ret`) | stub | removed feature |
| SFX 0x2d, 0x37, 0x6d | C 0x1005ba0b, 0x1005b2a7, 0x1005c458 | no sender in object.lto (section 3) | removed server features (dynamic prop spawn, particle effects) |
| `blyski_postaci` flag | C `[mgr+0x3ff78]` (0x10015d55 sets, 0x10015866 clears, nothing reads) | only writers; its gameai line is commented out | planned muzzle flashes of characters |
| `wlaczmuze` action field | C 0x10017154 stores to action +0x91f | no reader of +0x91f anywhere; 0 uses in scripts | per-action music switch |
| `black` (scenki) flag | C 0x1001231e stores phase +0x761 | no reader; 0 uses | fade to black |
| `weryfikuj_po_sejwie` | definition +0xfc5 read only by the load-check 0x1004c8e0 | 0 uses in postacie.txt | verify actor position after load |
| classes never placed | `b_winda`, `b_winda_drzwi1/2`, `b_winda_przycisk` (L 0x100055d0..0x100074b0), `d_cien_chmur` (0x10009fb0), `o_marker_rozprysk` (0x100158c0), `o_detektor_konca_levelu` (0x10012b30) | 0 instances in the 31 scene files | elevator set, cloud shadow, splash marker, exit radius detector (B087) |
| inert classes | `d_emiter_iskier` (L 0x1000b610, no message), `d_odglos` (writes `dzwienki.txt`, nothing reads it) | docs/retail-decorations.md, retail-objects.md | unfinished |
| property declared, never read | `b_rotator` `Additive`, `Multiply` (L 0x10002b50 declares, 0x10002fd0 does not read) | string xrefs | editor leftovers |
| property read, never used | `Nazwa_portalu` (`b_door` 71 non-empty values, `b_szuflada*`), `Kolor1/2` (`Uzyj_kolor` is 0 in all 309 doors), `Mgla_Liniowa` is **set by 1 level but never read** (value 0) | L 0x10001010 / 0x10001390; scene data | portal names for a visibility system; door tint |
| property never varied by data | `b_szuflada` `Solid` = 1 and `Czas_samozamkniecia` = 0 everywhere; `b_szuflada_przestrzelna` `Gracz_otwiera` = 0; `d_emiter_dymu` never sets `Nazwa_plika1/2`; `LightGroup.StartOn` = 1 | `props.py` style comparison of every scene object | code paths never exercised by the shipped levels |
| constant-value config | `[0x100b23f0]` = 100 (volume percent), `[0x100b23fa]`, `[0x100b2408]` = 768.0 | constructor only | - |

Properties that the level data sets but that object.lto does not read: only `WorldProperties.Mgla_Liniowa` (plus engine light / world-model fields). Properties read by object.lto that the remake never names (string search in `crates/*/src`, `tools/*.py`): `b_door` `Kolor1/2`, `Nazwa_portalu`; `d_sprite` `Sledz_kamere` (camera-following flag, documented in retail-decorations.md but not read by name); `o_promien_energii` `Blik_emitera` (B048); `d_emiter_opadu` `Szer_emitera/Wys_emitera/Dl_emitera/Snieg` and `d_fala3d` `Czas/Szer_fali/Dl_fali` (the remake derives these from defaults; all constant across the single instances). The lens-flare `Przed_*` / `Za_*` families are read through formatted names and are fine.

## 5. Level-header keys, what they do, and the keyword audit of every retail parser

### 5.1 gameai.txt level-header keys (parser C 0x10015cc0..0x100172c3, conditions evaluated by `C 0x10019980..0x1001a750`)

| Key (uses) | Effect (level record field) | Remake | Status / impact / effort |
|---|---|---|---|
| `gestosc_sciezek 4` (1, `rh3-miasteczko0`) | level +0x384 (float, default 2.0 when 0): multiplier in the path-node neighbour / floor sample test (`C 0x1003c650` half-size = value * 0.5, `C 0x1003ceb0` loop bound `[esp+0x48]`) | parsed and dropped (`mission-runtime` line 412); backlog B024 "never researched" | USED-BY-RETAIL-BUT-MISSING; prologue NPC link test uses 4.0 instead of 2.0; M |
| `nie_sprawdzaj_drzwi` (1, `rh3-miasteczko0`) | level +0x388: `C 0x1003ca80` (door-flag pass over all path links, prints "Checking for doors omitted") returns at once, so **no path link of that level carries a door flag** (NPCs there never stop for / open doors on a link, `patrol` ignores doors) | dropped; remake computes door flags from door bounds for every level (B017) | MISSING; prologue NPC behaviour; S |
| `pure_shooter` (1, `test2`, not shipped) | level +0x389: `C 0x1001a750` (mission tick) returns before running any action | dropped | UNUSED-IN-RETAIL (level absent) |
| `blyski_postaci` (commented) | `[mgr+0x3ff78]`, never read | dropped | dead |
| `muza <wav>` (27) | level record +0x300 music track (parsed at 0x10016192) | `music.rs` has its own table, same result | USED (verify table against the 27 lines: owner of music.rs) |
| `load_c/i/o/w/d` (28) | loading-screen images and text | `export_menu.py` | USED |
| `zdrowie` (2), `tnijitems` (1), `setallfaza`, `startlevel`, `cutscene`, `dialog`, `hostileattack`, `set/unset`, `setfaza`, `receive`, `expgained`, all `if*` conditions used in scripts | as in docs/retail-ai.md and retail-missions.md | `mission-runtime` | USED |
| `endifs` (197) | **not a retail keyword** (the parser skips unknown first tokens; blocks end at the next `action`) | handled as a no-op | USED |
| `print X` (0) | per-action string sent to the engine console (`ILTClient` +0x50, `C 0x1001a6d2`) | none | UNUSED in scripts (debug) |
| `wlaczmuze X` (0) | stored at action +0x91f, never read | none | dead |
| `ifgraczdalejniz` (0), `iflicznikmniejszyniz` (0), `outro` (0), `include` (35, all commented) | parsed, unused by data | none | UNUSED in scripts; effort S each if ever needed |

### 5.2 Keyword audit per parser (first token of every script line against the keys found in the parser body)

| Parser | Keys parsed by retail and used by scripts but ignored by the remake (HIGH) | Parsed by retail, never used by the shipped scripts | Used in scripts but not parsed (dead lines) |
|---|---|---|---|
| **postacie.txt** C 0x1003eab0..0x10041a20 | `obrot_postrzalu0/1` (150 lines, 75 phases; read by the turning code C 0x100489f0 at 0x10048a2f..3f, bone `Bip01 L Thigh`) B014; `glowa_do_gracza_dist/kat` (2) B014; `static` (88: the actor's ground snap at spawn C 0x100432e0 is skipped for a static default phase, C 0x10043551/0x1004358d) B037; `blik0-3`, `socket_blik0-3`, `skala_blik0-3` (4, the intro limousine lights) B006; `show_weapon` per-phase weapon update C 0x10049e80 / 0x10041ed1 (remake keeps a sticky `weapon_visible`) | `def_file`, `weryfikuj_po_sejwie`, `ruch`, `nie_wspinaj`, `mod_kier`, `mod_rot` (present in the parser, 0 uses), `on_hurt1..3`, `on_koniec_widzi4..7`, `on_closer3`, `strzal1`, `obrot_pion_do_gracza1`, `rs0`, `rs3` | **`drop_weapon` (55)**, `nie_wspinaj_sie` (3), `ignoruj_markery_niechodzenia` (2), `anim` / `anim_glowa` (1 each, line 301: the phase `nuda` has no `animacja`), `/rs1` (comment) |
| **items.txt** C 0x10062120..0x10063f60 | none that matter: every used key is read by the export / weapon code | `description` (alias of `descr`), `sound_alt_on/off`, `sprite_smuga_dymu`, `carried_rs0`, `tex1..3` | `alt_sound_on/off` (P90 laser / zoom sounds are never played in retail: they do not match `sound_alt_on/off`; the exporter reads them, the remake plays nothing: correct), `/dym_po_strzale_gracza...` (commented block) |
| **objects.txt** C 0x1002d490..0x1002e420 | none | `death_keepalive`, `death_prety_malo`, `podmieniany_rs1..3`, `podmieniany_skin1..7`, `rs3`, `skin7`, `sound0`, `sound1` | none |
| **scenki.txt** C 0x10011ce0..0x10012a99 (keys with a number are built as `socket` + N, `postac` + N, `anim_postac` + N, `socket_postac` + N, `anim_glowa_postac` + N) | `sepia` (2, in `szczur na strychu` / `szczur w piwnicy`): sets cutscene field +0x24 = 255.0 (C 0x1001368d), render effect not traced; **both scenes are referenced by no level** (only `intro`, `intro zwei`, `ucieczka z 2 wiezienia`, `outro` are triggered), so unreachable | `black` (phase +0x761, never read), `koniec`, `set`, `unset` | none |
| **gameai.txt** | see 5.1 | `blyski_postaci`, `include`, `outro`, `print`, `wlaczmuze`, `ifgraczdalejniz`, `iflicznikmniejszyniz` | `endifs` |
| **dialogi.txt** C 0x100173a0..0x10017cf0 | none | `include` | none |

All parsers compare the first blank-delimited token case-sensitively for equality (C 0x100581a0) and take the rest of the line with C 0x10058230; unknown tokens are silently skipped and later duplicates overwrite earlier ones. The remake reads these files through the exporters (`export_gameplay.py` keeps the ordered command arrays, so nothing is lost on export) and `npcs.rs` / `npcs/ai.rs` / `mission-runtime` consult keys by literal name; the audit of which literals exist was done with `rg` over `crates/*/src` and `tools/*.py` and then confirmed against docs/retail-ai.md.

## 6. Extra remake behaviour that retail does not have (reverse direction)

| Remake | Retail | Note |
|---|---|---|
| `drop_weapon` hides the NPC weapon (`npcs/ai.rs:129`, `opening.rs:224`) and is the **only** trigger of an NPC weapon pickup (`campaign.rs:128`) | parser ignores the key; `Kill` (C 0x10042ca0, code 0x10042eef..0x10043000) throws **both** weapon objects (+0x208, +0x20c) as items at velocity (0,-64,0) for every dead actor unless `nie_zostawiaj_gana` | 55 phases, all targets of `on_death` (`pada`, `pada1`, `kuca_pada`, `pada_kuca`); the armed definitions whose death phase lacks the key never drop in the remake: `straznik przy celi`, `straznik na klatce`, `policjant z pala`; the second weapon (`socket_weapon1`, 5 definitions) is never dropped |

## 7. Ranked list: retail behaviours the remake lacks (top 20)

| # | Behaviour | Retail address | Status | Player-visible impact | Effort |
|---|---|---|---|---|---|
| 1 | NPC weapon drop on death for **every** armed actor, including the second weapon | C `Kill` 0x10042ca0 (0x10042eef..0x10043000), thrown-item create 0x10042f74 | USED-BY-RETAIL-BUT-MISSING (3 definitions + second weapons) | missing pickups from 3 guards | S |
| 2 | `obrot_postrzalu0/1` death / hit twist (150 lines) | C 0x100489f0 (0x10048a2f), parse 0x100400a3 | MISSING (B014) | corpses fall without the torso twist, 75 phases in 58 characters | M |
| 3 | Cheat console variables `God`, `FullStamina`, `Invisible` from `autoexec.cfg` | C 0x10061bb4, 0x10061f19, 0x10043720 / 0x10043860 / 0x10045df0 / 0x10049950 | MISSING | only for players who edit autoexec.cfg | S |
| 4 | `nie_sprawdzaj_drzwi` (no door flags on the prologue path graph) | C 0x1003ca80 (flag at level +0x388) | MISSING (B024) | prologue NPCs never treat doors on a link as obstacles | S |
| 5 | `gestosc_sciezek 4` node-link density for the prologue | C 0x1003c650, 0x1003ceb0 (level +0x384) | MISSING (B024) | prologue NPC path decisions | M |
| 6 | `static` phases skip the spawn ground snap | C 0x10043520 / 0x100432e0 | APPROX (B037) | spawn height of 88 static phases | S |
| 7 | `aware` (+0x128): `ifplayerseenby` marks the actor, blocks `patrol` entry | C 0x1001a060, 0x10041cbe | MISSING (B011) | talking NPCs keep patrolling | M |
| 8 | `hostileattack` from a dialogue hits only the speaker | C 0x10019811 | APPROX (B016) | group aggro | M |
| 9 | Limousine tail lights `blik0-3` in the intro | parse 0x1003eab0 (+ sprites) | MISSING (B006) | missing lights in the intro | S |
| 10 | Head tracking `glowa_do_gracza_dist/kat` | C 0x10018682 (+0xfcc/0xfc8 fields) | MISSING (B014) | two NPCs do not turn the head | M |
| 11 | NPC M-14 laser ribbons and enemy flashlight beams | C 0x1002130a; 0x1001d78a / 0x1001d919 | MISSING (B007, B013) | missing lights on some enemies | M |
| 12 | Projected model shadows (`ModelShadow_proj_enable`) | L 0x1000a044 (`cien.dtx`), option C 0x1002a9b0 | MISSING (B015) | no blob shadows | M |
| 13 | Hit-blood chunks `miecho01-03` and the full grenade blast layers | C 0x10005b83 loop, 0x10053e90..0x10054383 | MISSING (B005, B010) | fewer gore / blast pieces | S / M |
| 14 | Per-phase weapon attach gate (`show_weapon`) instead of a sticky flag | C 0x10049e80 (0x10049f38, 0x1004a20b), 0x10041ed1 | UNVERIFIED difference | a weapon may stay drawn in phases that lack `show_weapon` | S |
| 15 | `DrawKilledCount` HUD counter | C 0x1003ae60 | MISSING | debug only | S |
| 16 | `DrawPaths` / `DrawPostacPaths` path debug overlays | C 0x1003d9f0, 0x1004ae30 | MISSING | debug only | M |
| 17 | Super jump command 20 | C 0x10060c1b | MISSING (B035), unbound in retail | none | S |
| 18 | Free-fly / physics-bypass flag | C 0x100b2475 (0x10060252) | dead in retail | none | - |
| 19 | `scenki` `sepia` tint (+0x24 = 255.0) | C 0x1001368d | dead (scenes never triggered) | none | S |
| 20 | Unused classes `b_winda*`, `d_cien_chmur`, `o_marker_rozprysk`, `o_detektor_konca_levelu`, spark emitter `d_emiter_iskier` / SFX 0x6d | L 0x100055d0.., 0x10009fb0, 0x100158c0, 0x10012b30, 0x1000b610 | dead in retail (B087) | none | M |

## 8. What could not be determined

* The key that opens the engine console (handled inside Lithtech.exe; `ConsoleEnable` default 1 and the history / buffer variables are known, `autoexec.cfg` binds no console key). Live effects of console changes after start-up are only guaranteed for the variables of section 1, which are polled every frame.
* The render effect of the scenki `sepia` flag (field +0x24 of the cutscene object at 0x10071970) and the exact use of `gestosc_sciezek` beyond the two functions named above.
* Whether retail hides the NPC weapon model in phases without `show_weapon` (the update function 0x10049e80 only repositions it; hiding was not traced).
* Dead virtual slots: the class definition tables and the vtables in both DLLs are all referenced, so a slot-level "never invoked" analysis (engine-called methods) was not done.
* The purpose of the `ILTClient` +0x50 call used by `print` (assumed console print), of the constants `[0x100b23f0]` / `[0x100b23fa]`, and of the string-keyed 0x20 / 0x22 messages beyond "server state load / save".
