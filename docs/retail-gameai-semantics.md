# Retail `gameai.txt` action semantics (cshell.dll) versus the remake mission interpreter

**Összefoglaló (magyarul)**
1. A retail parser (cshell 0x10015cc0) soronként csak az első szót hasonlítja össze; az `endifs` ismeretlen szó, ezért egyszerűen figyelmen kívül marad. Egy `action` csomópontláncot (+0x518) épít: minden `if*` sor új csomópont (ÉS kapcsolat), az effekt-sorok az utolsó csomópontba kerülnek.
2. A végrehajtó (0x1001a140) a láncon addig lép tovább, amíg a csomópont feltétele teljesül és van következő; a megállási csomópont effektjei futnak le (fix sorrendben: dialog, startlevel, cutscene, hostileattack, tnijitems, set, unset, setfaza, setallfaza, print, zdrowie, outro). A szállított scriptben ez pontosan "minden feltétel igaz -> effektek"; vegyes (feltétel+effekt) csomópont, duplikált effekt, ismeretlen szó az `endifs`-en kívül nincs.
3. Mind a 213 action külön-külön egyenértékű a remake interpreterével (véletlen állapotokon ellenőrizve); az egyetlen eltérés a `Bunt3` (rh1-wiezienie2) effektjeinek kibocsátási sorrendje (setfaza/dialog), ennek nincs gyakorlati hatása.
4. EGY valódi szerkezeti eltérés van: a retail a szint action-jeit FORDÍTOTT sorrendben futtatja (a script utolsó actionje először, mert a csomópontlista elejére szúrnak be), a remake `tick`-je a script sorrendjében. Ez 1 frame késést okoz a flag-kaszkádokban, és ha két action ugyanabban a frame-ben igaz, a retailben a script-beli korábbi győz (dialog, latch-flag), a remake-ben a későbbi/korábbi fordítva. Javaslat: `actions.iter().rev()` a `tick`-ben + fix effekt-sorrend.
5. Nem deklarált változó (`if BurmistrzSieBoi`), üres `ifactionhostile`, nem létező karakter `setfaza`-ja: a retailben mind hatástalan/hamis, a remake-ben is (lásd alább), tehát nincs eltérés.

## 1. Verdict

| Question | Answer |
|---|---|
| Does the remake evaluate each of the 213 action blocks like retail, one block at a time? | Yes. 212 blocks give identical events and identical variable/counter state on every tested input (6000 random flag/counter/world assignments per block, ~1.3 M runs); `Bunt3` (rh1-wiezienie2, line 529) emits `setfaza` before `dialog` in the remake and `dialog` before `setfaza` in retail (order only, same set of effects, same state). |
| Is the remake equivalent as a whole? | **No, one structural difference**: retail iterates the actions of a level newest-first, i.e. in *reverse script order* (section 4). Everything else is equal for this script. |
| Anything the shipped script relies on that the remake would get wrong? | Only through the execution order (section 4); plus minor items in section 6. |

Method: parser (0x10015cc0..0x100172c3), node allocator 0x10015be0, variable lookup 0x10015ac0, executor 0x1001a140 and its driver 0x1001a750 were read in the disassembly; a Python model of the retail node structure (`tools/gameai_model/retail.py`, `sim.py` class `Retail`) parses `output/decoded_scripts/gameai.txt` exactly the way retail does and a second model (`sim.py` class `Remake`) ports `tick`/`condition`/`execute`/`endifs` of `crates/mission-runtime/src/lib.rs`. Both use a shared oracle for the world predicates (seen, alive, distance, ...), so only the structural semantics are compared. Reproduce with `cd tools/gameai_model && python cmp.py 3000` (per action), `python order2.py 3000` (whole level, retail node semantics in script order versus remake: 0 set/state differences in 96 000 frames), `python order.py 3000` (retail reverse order versus remake).

## 2. Parser (cshell.dll 0x10015cc0, called once from 0x10059572 at client start, after 0x10015990 cleared the lists)

* File is opened `"rt"` (mode string 0x10069710), read line by line with the decoding `fgets` clone 0x100580b0 (swap cipher 0x10057f50; the shipped copy in GYARI is already toggled to plain text, see docs/retail-ancillary-files.md H3). Text mode removes the CR.
* Line handling: `first word` = characters up to the first **space** (0x100581a0, max 127, exact case-sensitive compare; a TAB is not a separator, a leading space makes the first word empty so the line matches nothing). `argument` = 0x10058230(line): leading spaces dropped, trailing spaces dropped, first word dropped, following spaces dropped; the rest of the line (spaces kept, so `ifalive stara dziwka` works) is copied into a fixed node field. No `//` comment stripping except that a line that starts with `//` has the first word `//...` which matches nothing. The shipped script has no tabs, no leading/trailing spaces and no trailing comments on keyword lines (checked).
* It is **not** an else-if chain: every line is compared against all keywords in turn (only one can match). Lines that match no keyword (here only `endifs`, 197 lines, plus blanks and comments) are silently ignored and do not touch any parser state.
* State: `+0x200` variable list head (global for the whole game, newest first), `+0x208` current `level` node (list newest first via +0x390), `+0x3ff58` = **S**, the "open chain tail" (0 = none). `level` and `action` set S = 0.
* Node = 0xaac bytes zero-filled, allocated by 0x10015be0 and **pushed to the front** of the current level's list (`level+0x380` = head; node `+0xaa8` = next (older), `+0xaa4` = previous). Chain link of one action: node `+0x518`.

Keyword table (compare address -> what is stored):

| Keyword | Compare | Stored |
|---|---|---|
| `include` | 0x10015d60 | opens the file recursively (only `//include` lines exist, all commented out) |
| `bool` | 0x10015df2 | new variable node (0x10015a30, 0x90 bytes, list `+0x200`): name at +0, `+0x80`=1 (bool), `+0x82` value |
| `licznik` | 0x10015e7e | variable node with `+0x81`=1 (counter), `+0x84` float value |
| `level` | 0x10015ed8 | new level node (0x10015b90, 0x394 bytes), S=0 |
| `action` | 0x10015f5d | `0x10015be0` new node, name copied to +0 of the new head, S=0 |
| `pure_shooter` (level +0x389), `load_c/i/o/w/d` (level +0x180 for `load_c`, +0x80 `load_i`, +0x100 `load_o`, ...), `muza` (level +0x300), `gestosc_sciezek` (+0x384), `nie_sprawdzaj_drzwi` (+0x388), `blyski_postaci` (global byte +0x3ff78) | 0x10015fe8..0x10016215, 0x10015d43 | level-scope fields; they never read or write S |
| condition keywords | see table 3 | field of the chain node (below) |
| effect keywords | see table 4 | field of the chain node |
| `endifs` | - | **not compared anywhere**: ignored |
| `receive`, `expgained`, `estimate_*` ... | - | not action keywords in retail (would be ignored); none occurs in `gameai.txt` actions |

### 2.1 How conditions and effects are attached (this is the whole grouping logic)

Every condition line (0x10016238 `if`, 0x100162c5 `ifnot`, ... 0x10016b13 `ifhostileblizejniz`) does:

```
if S != 0:  N = new node (pushed to level list head);  S.next(+0x518) = N
head = level list head;  S = head;  head.<condition field> = argument
```
so the first condition of an action goes into the `action` node itself (S was 0, head = action node); each further condition (also of the same type, e.g. two `ifnot`) opens a **new** node linked behind the previous one. Conditions are therefore ANDed; a later condition never overwrites an earlier one.

Every effect line (0x10016b88 `dialog` ... 0x1001724b `zdrowie`) does:

```
if S != 0:  N = new node;  S.next = N
S = 0;  head.<effect field> = argument
```
so the first effect opens a node behind the last condition and closes the chain (S = 0); **following effect lines, and following condition lines, go into the same head node** (S is 0, `head` = that node). Consequences:
* several effects in an action share one node; same-type effects overwrite each other (last wins), different types coexist;
* a condition written after an effect lands in the *effect node* (it is not a new link before the effect) and then S = that node again (see the trap below).

`endifs` does nothing, so it does not close groups. In `gameai.txt` it appears only as the last line of an action (197 times; the other 16 actions simply have none), so the ignored word changes nothing.

## 3. Executor (0x1001a140, per action head node, `ecx` = game-AI object)

Driver 0x1001a750 (per frame, called from 0x1005a795): needs `+0x3ff68` set, a player object, a current level (`+0x3ff5c`), `[0x10071978] == 0` and `level+0x389` (pure_shooter) == 0; advances every counter (`+0x81`) by the frame time (0x1001a834, also during dialogs); if a dialog node is current (`+0x3ff60`) it only ticks the dialog and returns (0x1001a864 - same as the remake: no action runs during a dialog); otherwise clears the per-actor `+0x128` tag on all actors, then loops

```
for node in level list (head = +0x380, next = +0xaa8):      // 0x1001a898..0x1001a8b7
    if node.name[0] != 0:  executor(node)                  // only `action` heads have a name
```
Level list order is **newest first = reverse script order** (allocator 0x10015c08/0x10015c32). Chain nodes have an empty name and are reached only through `+0x518`.

Executor per node (0x1001a14a..):

```
loop:
  for each condition field set in the node:          // order in table 3; a node holds at most one
      if the condition HOLDS and node.next != 0:  node = node.next;  goto loop
      (otherwise fall through to the next field check)
  run the effects of `node` in fixed order (table 4); return
```
So the chain is walked while conditions hold; it stops at the first node whose condition does not hold (or at the last node) and **that node's effects are executed** - effects are not gated by "all conditions of the action". For the shipped script this is exactly "all conditions hold -> run the effect node", because condition nodes never carry effects. The chain end (`+0x518 == 0`) with a holding condition falls through to that node's own effects.

### 3.1 Condition fields (executor address -> helper; "holds" = advance)

| Keyword | node field | executor | holds when |
|---|---|---|---|
| `if V` | +0x80 (name) | 0x1001a14a | V declared (0x10015ac0) **and** flag +0x82 set; name empty or V undeclared -> not holding |
| `ifnot V` | +0x100 | 0x1001a17e | V declared **and** flag clear; **undeclared V -> not holding** (not "true") |
| `ifseenbyhostile` | +0x401 | 0x1001a1b2 -> 0x10019f70 | player +0xcc clear and an actor with +0x9b, contact flag +0x126, alive (argument ignored) |
| `ifactionhostile` | +0x400 | 0x1001a1d8 -> 0x10019db0 | use key pressed, first actor whose box contains the probe point and whose *definition* is hostile (+0xfbd), alive. **Takes no argument**: the word after it is stored nowhere (all 10 uses have an empty argument). A dead hostile whose box contains the point makes it false at once (0x10019f53), whereas `ifaction` skips corpses (0x10019c84) |
| `ifhostileblizejniz D` | +0x40c | 0x1001a1fe -> 0x10019fc0 | only evaluated if D > 1.0 (const 0x100660bc) |
| `iflicznikwiekszyniz T C` | +0x490 / name +0x498 | 0x1001a236 | only if T > 0.001 (0x10066244); counter node found and value(+0x84) > T |
| `iflicznikmniejszyniz T C` | +0x494 / +0x498 | 0x1001a27f | value < T (unused in the script) |
| `ifplayerseenby N` | +0x300 | 0x1001a2c8 -> 0x1001a060 | living actor of definition N with `seen` flag +0x149 |
| `ifplayerhas I` | +0x380 | 0x1001a2f5 -> 0x1001a0f0 | inventory lookup |
| `ifweapondrawn` / `ifweaponhidden` | +0x402 / +0x403 | 0x1001a322 / 0x1001a346 | `[0x1006f608]` != 0 / == 0 |
| `ifgraczblizejniz D N` | +0x404 / name +0x410 | 0x1001a36a | only if D > 1.0; first actor named N exists (0x10042ae0) and distance < D |
| `ifgraczdalejniz D N` | +0x408 / +0x410 | 0x1001a3bb | distance > D (unused) |
| `ifalive N` | +0x180 | 0x1001a40c | first actor named N exists and `+0x144 == 0` |
| `ifdead N` | +0x200 | 0x1001a446 | first actor named N exists and `+0x144 != 0` |
| `ifaction N` | +0x280 | 0x1001a480 -> 0x10019b80 | use key + probe point in the box of the first living actor, whose definition name is N |

A field whose argument is empty or numerically below its guard is "not set", i.e. the check is skipped and counts as **not holding** (the walk stops there). All thresholds in the script (96..640, counters 1..30) are above the guards.

### 3.2 Effect fields, executed in this order inside one node

| # | Keyword | field | address | action |
|---|---|---|---|---|
| 1 | `dialog X` | +0x51c | 0x1001a4ad -> StartDialog 0x100194d0 | also runs the dialog node's own single `set` (+0x614, 0x10015b30) and then its single `unset` (+0x694, 0x10015b60) immediately |
| 2 | `startlevel W` | +0x59c | 0x1001a4c5 -> 0x1005af20 | |
| 3 | `cutscene C` | +0x99f | 0x1001a4e1 | |
| 4 | `hostileattack` | +0x61c | 0x1001a523 | for every actor with +0x126: +0x128 = 0, +0x127 = 1 |
| 5 | `tnijitems` | +0x61d | 0x1001a558 | |
| 6 | `set V` | +0x61f | 0x1001a587 -> 0x10015b30 | undeclared V: no-op. bool: flag = 1; **counter: value reset to 0** |
| 7 | `unset V` | +0x69f | 0x1001a59f -> 0x10015b60 | bool: flag = 0; counter: value = 0 |
| 8 | `setfaza P N` | +0x71f (P, cut at first space) / +0x79f (N) | 0x1001a5b7 | N = `hostile`: all actors with +0x9b and +0x128, **using the `setallfaza` phase field +0x81f (retail quirk; unused by the script)**; else first actor of definition N in the actor list (0x100424c0 -> list scan), 0x10041c60 ignores dead actors; unknown N -> nothing |
| 9 | `setallfaza P N` | +0x81f / +0x89f | 0x1001a671 | every living actor of definition N with `seen` +0x149 set (docs already mirror this through `seen_player`) |
| - | `wlaczmuze` | +0x91f | - | parsed, never read (no-op; not in script) |
| 10 | `print` | +0xa1f | 0x1001a6d2 | console print (not in script) |
| 11 | `zdrowie H` | +0xaa0 | 0x1001a6f2 -> 0x10061b90 | only when H != 0 |
| 12 | `outro` | +0x61e | 0x1001a71f | (not in script) |

## 4. The one real difference: execution order of the actions

* Retail: the actions of a level run **last action of the level section first, first action last**, every frame. A flag set by an action is seen in the same frame only by actions that are *earlier* in the script.
* Remake (`tick`, lib.rs line 212): `for action in actions.iter()` = script order, and its comment "flags set above are visible to later actions" states the opposite of retail. `responds_to_action` is order independent (any()).
* Same-frame speaker tagging (`tagged`, retail +0x128 flags set by `ifaction`/`ifplayerseenby`/`ifhostileblizejniz`/`ifactionhostile` and consumed by StartDialog 0x10019737/0x1001979f = "first tagged actor in actor-list order") also accumulates in the reverse order in retail.

Practical effect (nothing else differs): (a) a cascade "action A sets V, action B reads V" is seen in the same frame by B in one engine and one frame (about 16 ms) later in the other; 140 action pairs in 8 levels have such a dependency. (b) If two actions become true in the **same frame**, the outcome depends on the order: which `dialog` ends up current (StartDialog replaces the node; retail: the script-earliest of the simultaneous ones, remake: the script-latest), and which one wins a latch (an action that `set`s V while another is gated by `ifnot V`). Static over-approximation (conditions assumed simultaneously satisfiable, contradictory literals removed; `tools/gameai_model/latch.py`):

| level | latch flag | set by | gated (remake suppresses / retail suppresses) |
|---|---|---|---|
| rh3-miasteczko0 | `HostilePrologAtakuja` | atakuja@177, czyatak@229 | remake suppresses czyatak@229, policaj@243, policaj1@260, dziwka1@277; retail suppresses atakuja@177 |
| rh1-wiezienie2 | `Wiezienie1Walka` | CzyGeneralAttack@455, Bunt00@499 | remake: Bunt00, ZagadujePolicjantaZabija@784, ...Dead@797; retail: CzyGeneralAttack |
| rh1-wiezienie2 | `BuntWybuchl`, `Wiezien7/8/9Schowany`, `WiezienPalaczPowital` | Bunt0/1@472,486, Wiezien7/8/9Chowa@582..608, powitanie0@885 | remake: straznik13@928, Zagadujewieznia7a/8a/9a@716..760; retail: straznik3@858 |
| rh2-wiezienie1 | `WiezienEgzekucjaDialogOdpalony`, `UsmazylTrojce` | czygadaja0..3@1130..1172, smazenie@1188 | with several `czygadaja*` true at once the remake shows TrojcaGadka of the first, retail of the last (TrojcaGadka3); `smazenie` suppresses czygadaja* only in retail |
| burmistrz1 | `HostileUBurmistrzaAtakuja`, `GadalZBurmistrzem` | CzyWyciagnalGana@1953, ZagadalBurmistrza7@2169, BurmistrzZagaduje*@2120,2137 | remake suppresses HostileZagaduje1/2@2015,2032, BurmistrzZagaduje1@2137; retail additionally JakZobaczyWrogBezGana/1@2050,2063; ChceWyjsc1@1976 only in retail |
| chinatown, chinatown2 | `ChinioleAtakuja` | tongpo1@2509/2897, CzyWyciagnalGana@2522/2910 | remake suppresses ChiniolZauwaza1-3, JakZobaczyWrogBezGana1, wybranychiniol1-3, ChiniolePrzekraczaBrame; retail suppresses Automat1, tongpo1 |
| chinatown2 | `NiesieKota` | CzyNiesieKota@2836 | remake suppresses ChceWyjsc1@2847 |
| knajpa | `HostileWKnajpieAtakuja`, `GadalZDziwkaKibel`, `PrzedstawionyNico` | JakZobaczyWrogZGanem@4035, JakMarloniDead@4058, JakMarloniPogonil@4069, DziwkaKibelWybranaPrzezGraczaZywa1@4165, LaskaZKrzeslem6@4312 | remake suppresses JakZobaczyWrogBezGana1/2 (@4081,4095) after a hostile latch; retail suppresses DziwkaKibel1@4148, BarmanBarWybranyZywy@3902, LaskaZKrzeslem2@4251 |
| chapel_mniejszy, rh9-fabryka, rh3-miasteczko1 | (dialog priority only) | - | PierwszeSpotkanie/ChceWyjsc/zagadana*/martwa@1451..1515 and cywil2/3 actions: final dialog differs when they fire in the same frame |

The table lists possibilities, not measured frequencies: each needs the two conditions to become true in the same 16 ms frame. Random-state frame simulation (`order.py`, 3000 frames per level, states not restricted to reachable ones) shows differing same-frame outcomes in 13 of the 32 level sections (rh3-miasteczko0, rh1-wiezienie2/3, rh2-wiezienie1, rh3-miasteczko1, chapel_mniejszy, rh9-fabryka, burmistrz1/2, chinatown, chinatown2, rh10-wiezowiec1, knajpa); the remaining 19 sections (including test2, pudlo, rh1-wiezienie1, rh2-wiezienie2, rh3-miasteczko2, rh7a, lab1/2, wiezowiec2/3, podziemia*, boks, wiez_wn*) are order independent.

**Fix (report only, crates/ not touched):** in `Mission::tick` iterate `actions.iter().rev()`; fix the comment; make `execute` of one action run the effect kinds in the retail order of table 3.2 (dialog, startlevel, cutscene, hostileattack, tnijitems, set, unset, setfaza, setallfaza, print, zdrowie, outro) with one value per kind (last wins) instead of script order. The second part only changes emission order in 35 actions (`set,dialog` x3, `setfaza,set` x11 + `Bunt3`, `dialog,zdrowie,set` x2, `unset,dialog` x18); no state differs because dialog-side set/unset target the same variables in both orders.

## 5. The specific questions

| Case | Retail | Remake | Same? |
|---|---|---|---|
| `endifs` | unknown word, ignored, no state change; grouping is only by chain nodes | resets `allowed = true` | Same for this script: every `endifs` is the last line of its action, no action has two, nothing follows it |
| Several conditions of one type (two `ifnot`) | each opens its own node, ANDed | each evaluated in turn, ANDed | Same (163 `ifnot`, 127 `if` in the script) |
| Conditions vs effects order | conditions after an effect would be attached to the effect node; with `holds && next` the effects of that node are skipped when its condition holds and run when it fails (inverted). Nothing like that exists in the script (0 mixed nodes) | a failing condition disables all later effects until `endifs` | Same for the shipped script; would differ for such a script |
| Two effects of the same kind in one action | last wins | both run | none exists in the script |
| Actions that start with an effect (no condition): 23 (`general` x14 + `GeneralAttack` x9, each `hostileattack` + `endifs`) | node without condition fields: runs every frame | `allowed` stays true: runs every frame | Same (the script has 23 such actions, not 29) |
| `action FlagaWiezienia` = `ifnot CzyWiezienie`, `set CzyWiezienie`, `endifs`, then `muza ...` | `muza` (line 1008...) is a level field: sets the level music, does not touch S | `muza` dropped while in an action | Same |
| `if BurmistrzSieBoi` (burmistrz1 `CzyBurmistrzSieBoi`@2155, variable never declared by `bool`) | lookup 0x10015ac0 returns 0 -> condition not holding; the action never fires. `set`/`unset` of an undeclared name would be a no-op | flag() = false -> never fires (and no dialog `set`s it: checked) | Same: dead action in both. (`ifnot` of an undeclared variable would differ - retail false, remake true - but no `ifnot` names an undeclared variable, checked) |
| `ifactionhostile` without argument (10 uses, e.g. burmistrz1 `Wybranyhostile1`@2078) | correct and complete: the function has no argument | argument unused | Same |
| `setfaza chowa_sie straznik_przy_celi` (rh1-wiezienie2 `straznik5`@870) | definition "straznik_przy_celi" does not exist (the character is `straznik przy celi` with spaces; the `.ltb` model file is `straznik_przy_celi.ltb`) -> 0x100424c0 returns 0 -> nothing happens. So `straznik5` (weapon drawn -> hide guard) is a no-op in retail | emits `SetNpcPhase{name:"straznik_przy_celi"}`; `roster.set_phase` finds nobody | Same result (campaign.rs 145 / solver.rs 120 ignore unknown names) |
| Counters | `set C` and `unset C` both reset the counter to 0; counters count from client start in every level that is in gameai.txt | `set_flag` resets timers; timers exist only from the first entry of a level that declares/uses them | Same in a normal playthrough (see 6) |

## 6. Minor differences and open points (none needs action for the shipped script)

* Counter start value: retail counters (`Licznik1`, `Licznik2`, `LicznikLaski`, `CzasOdGadki`) are created at client start and counted through every frame of the running game; remake timers start at 0 when the first level that declares/uses them is entered (`Licznik1`/`2` are declared before the first `level` line, so the remake collects them only through `iflicznikwiekszyniz`). Identical when the campaign starts in prolog (it uses `Licznik1/2` first) and `CzasOdGadki` is seeded at 0; a level entered directly from a save or level select would have a larger counter in retail.
* `ifactionhostile` (0x10019db0): the first actor in the list whose box contains the probe point *and* whose definition is hostile decides, a corpse blocks; the remake tests `hostile(n) && used(n)` over living NPCs. Only matters when a corpse of a hostile definition overlaps the probe point.
* `setfaza hostile` uses the `setallfaza` phase field in retail (+0x81f); the script never uses `hostile` as a setfaza target.
* Not determined: the meaning of the gate `[0x10071978] != 0` in 0x1001a750 (actions and counters do not run while it is set) and of `+0x3ff68`/`+0x3ff6c` (frame-delta handling at 0x1001a7f5..0x1001a830); the remake assumes they are "game running".
* Model limits: the world predicates (`seen`, alive, distances, use probe, inventory) are abstract random booleans shared by both models; their own semantics are covered by the earlier notes (docs/retail-missions.md, retail-ai.md). Dialog nodes are treated as instantaneous (only their `set`/`unset`/`hostileattack` lines are modelled). States were random, not restricted to reachable game states.
