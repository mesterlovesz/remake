# Telepítés és első indítás részletesen (A Mesterlövész: Újratöltve)

Ez a leírás a [README](../README.md) lépéseit fejti ki. Ha csak játszani akarsz, a README elég; ide akkor érdemes jönni,
ha valami elakad, vagy tudni akarod, mit csinál pontosan az `Indit.cmd`.

> A tároló **nem tartalmaz játékfájlt**. Szükséged van a magyar *A Mesterlövész* (*Sniper: Path of Vengeance*) Archive.org-on megőrzött
> ISO-jára. A program az eredeti fájlokat csak olvassa.
> A projekt korábbi neve *A Mesterlövész – Újraírva* volt: ugyanaz a projekt.

## 1. A mappaszerkezet (ez a lényeg)

Két mappának kell egymás mellett lennie: a projektnek és az eredeti játéknak, utóbbi neve pontosan `GYARI`.

```
Mesterlovesz\                      <- bármilyen mappa, bárhol (pl. D:\Mesterlovesz)
├── GYARI\                         <- AZ ERETDEI JÁTÉK TELEPÍTETT MAPPÁJA (ezt te hozod létre)
│   ├── Lithtech.exe
│   ├── cshell.dll
│   ├── object.lto
│   ├── server.dll, launcher.exe, play1.exe, cipher.exe ...
│   ├── 1.avi  2.avi  3.avi        (a három logóvideó)
│   ├── autoexec.cfg  ReadMe.txt  sniper.ico
│   ├── scripts\                   (items.txt, postacie.txt, text_keys.txt, ai\, locale\ ...)
│   ├── worlds\                    (31 pálya: *.dat és *.pth)
│   ├── models\  skins\  textures\  sprites\  sounds\  misc\  rs\  save\ ...
│   └── ...
└── projekt\                       <- ez a tároló (a letöltött / kicsomagolt mappa; a neve mindegy)
    ├── Indit.cmd                  <- EZT INDÍTSD
    ├── README.md
    ├── tools\                     (export_all.py, setup_check.py, az exportálók)
    ├── crates\                    (a játék Rust kódja)
    ├── docs\
    └── output\                    <- az Indit.cmd hozza létre: a kinyert adatok (nincs verziókezelésben)
```

- A `GYARI` mappában közvetlenül ott kell lennie a `Lithtech.exe`-nek és a `cshell.dll`-nek. A leggyakoribb hiba, hogy még egy
  mappával lejjebb van (`GYARI\A mesterlövész\Lithtech.exe`): ilyenkor húzd ki a tartalmát egy szinttel feljebb. Az `Indit.cmd` az üzenetében
  kiírja a pontos helyet, ahol a `GYARI` mappát keresi, és felismeri ezt a hibát is.
- Alternatíva: a `GYARI` mappa a projektmappán **belül** is jó (`projekt\GYARI`); ez a mappa a `.gitignore`-ban van, így véletlenül sem kerül a tárolóba.
  A dokumentált, ajánlott hely a projekt melletti `..\GYARI`.
- Másik hely: `Indit.cmd --game D:\valahol\jatek` vagy a `MESTER_GAME` környezeti változó.
- A telepítés ~1,1 GB. Az eredeti játékot **nem kell** elindítani, javítócsomag (patch) sem kell hozzá.

## 2. A programok

| Program | Honnan | Megjegyzés |
|---|---|---|
| Python 3.10 vagy újabb | [python.org/downloads](https://www.python.org/downloads/) | Telepítéskor pipáld be: **Add python.exe to PATH**. A Microsoft Store-os „python” üres parancsikon nem jó. |
| Rust (1.89 vagy újabb) | [rustup.rs](https://rustup.rs) | A `rustup-init.exe` felajánlja a **Visual Studio C++ Build Tools** telepítését: fogadd el (kb. 3-6 GB), enélkül a fordítás `link.exe not found` hibával áll le. A telepítés után nyiss új ablakot. |
| Videokártya-illesztő | a gyártó oldaláról | A játék Vulkan-t használ: naprakész illesztő kell. |
| Python csomagok | automatikus | Az `Indit.cmd` telepíti (`pip install Pillow imageio-ffmpeg`); internet kell hozzá. Az ffmpeg az `imageio-ffmpeg` csomagban van. |

A projektben nincs más külső függés. (A saját fejlesztői gépünkön a Rust egy projekten belüli `_toolchain` mappában van; ha ilyen nincs,
az `Indit.cmd` a normál rustup-telepítést használja.)

## 3. Az eredeti játék megszerzése lemezképből (ISO)

Az Archive.org-os magyar ISO letöltése után a játékot először telepíteni kell. Az eredeti játék lemezképének forrását a
projekt weboldalán találod; a lemezképet nem a projekt terjeszti.

**Amit a lemezképről ellenőriztünk** (a lemezkép fájljait olvastuk, a telepítőt nem futtattuk):

- A lemezkép ISO 9660 + Joliet; egyetlen, `A mesterlövész` nevű mappát tartalmaz (kb. 578 MB), ebben van a telepítő: `setup.exe`, `setup.ini`,
  `setup.inx`, `setup.boot`, `data1.cab`, `data1.hdr`, `data2.cab` (585 MB), `engine32.cab`, `layout.bin`, `autorun.inf`, `sniper.ico`, a
  `kézikönyv.pdf` (a gyári kézikönyv) és az `Adobe Acrobat Reader CE` mappa (ezt nem kell telepíteni). A lemezkép 2010-es összeállítású.
- A telepítő InstallShield (készítő: Cenega), magyar nyelvű, „Complete” és „Custom” telepítéstípussal; a telepítés célmappáját kérdezi.
- **A telepített játék pontosan a mi `GYARI` mappánk.** A telepítő fejlécfájlja (`data1.hdr`) minden fájlhoz tartalmazza annak MD5 összegét
  (a `setup.ini`-ben `CheckMD5=Y`). A mi (2.33-as) `GYARI` mappánk 5521 nemüres fájljából 5492 összege szerepel a lemezkép fejlécében,
  köztük a `Lithtech.exe`, a `cshell.dll`, az `object.lto`, a `scripts\app_name.txt` („A mesterlövész v 2.33”) és mind a 31 pálya.
  A maradék 29 eltérést megmagyarázzuk: 26 szkriptfájlt a gyári `cipher.exe` átírt (a friss telepítés kódolt szkripteket ad; újrakódolva
  pontosan a lemezkép összegeit kapjuk), 3 fájlt (`autoexec.cfg`, `scripts\cs\dialogs.txt`, `scripts\cs\dialogs_real.txt`) pedig a játék futása
  vagy a `cipher.exe` változtatott. Vagyis **a lemezképről telepített játék 2.33-as, javítócsomag nem kell**, és az `Indit.cmd` mindkét
  szkript-állapotot (kódolt, sima) kezeli: a teljes kinyerést kódolt szkriptekkel is végigfuttattuk, az eredmény azonos.
- A telepített játék mérete ~1,1 GB (a gyári ReadMe 1,2 GB szabad helyet kér).

**Telepítés lépésről lépésre (Windows 10/11):**

1. Kattints duplán az `.iso` fájlra (vagy jobb klikk → **Csatlakoztatás**). A Windows virtuális DVD-meghajtót nyit.
   Ha az `.iso` mást nyit meg (pl. archiváló), jobb klikk → *Megnyitás ezzel* → *Windows Intéző*.
2. A meghajtón nyisd meg az **`A mesterlövész`** mappát (a lemez gyökerében csak ez az egy mappa van, innen nem indul automatikusan semmi),
   és indítsd a **`setup.exe`**-t.
3. A telepítő megkérdezi a célmappát. **Add meg közvetlenül a `GYARI` mappát a projekt mellett** (pl. `D:\Mesterlovesz\GYARI`):
   így nem kell semmit másolni. Ha a telepítő alapértelmezett mappájába telepítettél (várhatóan a *Program Files* alá, a pontos alapértelmezett
   utat nem ellenőriztük), másold ki az így létrejött mappa **tartalmát** a `GYARI` mappába (a *Program Files* védett: a másolás rendszergazdai jogot kérhet).
4. A telepítés végén a lemezkép leválasztható (jobb klikk a meghajtón → *Kiadás*). Az eredeti játékot nem kell elindítani; DirectX-et sem kell
   telepíteni. (Ha mégis elindítod, az nem zavar, de ne futtasd a `GYARI\cipher.exe`-t: átírja a szkripteket.)
5. Ellenőrzés: `GYARI\Lithtech.exe` létezik. Utána: `Indit.cmd`.

**Nem ellenőriztük:** a telepítővarázsló pontos lapjait és az alapértelmezett célmappát (a telepítő futtatása a gépen rendszerbeállításokat is
módosítana: nem futtattuk), és hogy a 2010-es InstallShield telepítő hogyan viselkedik egy adott Windows 11 összeállításon. Ha a `setup.exe` nem indul,
próbáld jobb klikk → *Futtatás rendszergazdaként*, vagy jobb klikk → *Tulajdonságok* → *Kompatibilitás* (Windows 7 mód); ez tipp, nem ellenőrzött.

## 4. Mit csinál az `Indit.cmd` (18 lépés)

Az `Indit.cmd` megkeresi a Pythont, majd a `tools\export_all.py`-t futtatja (`--run`: a végén elindítja a játékot). Az időket egy gyors gépen
(Ryzen 7, 8 mag, NVMe, a gépen közben más program is dolgozott) mértük, egy **tiszta**, kódolt szkripteket tartalmazó `GYARI`-ról indulva.

| # | Lépés (azonosító) | Mit csinál | Idő | Eredmény |
|---|---|---|---|---|
| 1 | Ellenőrzés | Python 3.10+, segédcsomagok (pip), `GYARI` mappa és fájljai, a verzió (a `cshell.dll`, `Lithtech.exe`, `object.lto` mérete és MD5-je az ismert 2.33-as értékkel), Rust 1.89+, szabad hely (csak arra, ami még hátra van) | másodpercek | |
| 2 | A gyári szkriptek visszafejtése (`scripts`) | a betűpár-kódolású szkriptek olvasható másolata | 0 mp | `output\decoded_scripts\` |
| 3 | A pályák kinyerése (`worlds`) | mind a 31 `*.dat` pálya: geometria, ütközés, textúrák, fénytérkép, mozgó világmodellek, díszletek (3 párhuzamos folyamat) | ~45 mp | `output\<pálya>.*`, `output\textures\`, `output\world_models\` |
| 4 | HUD és nyitójelenet (`presentation`) | HUD-képek, a nyitójelenet idővonala | ~2 mp | `output\hud\`, `opening.json` |
| 5 | Szereplők, küldetések, párbeszédek, modellek (`campaign`) | 28 pálya NPC-i, küldetésszkriptjei, párbeszédei, tárgyai, átvezetői, modellek és bőrök | ~45 mp | `output\<pálya>.gameplay.json`, `output\models\`, `output\model_textures\` |
| 6 | Átvezetők és a befejezés (`scenes`) | az átvezetők karakterei, a stáblista és a lezáró képek | ~2 mp | `endgame.json`, `output\ui\outro\` |
| 7 | Fegyverek (`weapons`) | a hordott fegyverek, animációk, hangok | ~1 mp | `retail_weapons.json`, `retail_items.json` |
| 8 | Lövés-, vér- és füsteffektek (`effects`) | lövés-, találat-, vér- és füstsprite-ok | ~2 mp | `retail_effects.json` |
| 9 | Lézer, távcső, gumibot (`altfire`) | másodlagos tűz | 0 mp | `retail_altfire.json` |
| 10 | Díszletek (`decorations`) | lámpafény, eső, lens flare sprite-ok | ~2 mp | `output\decor\` |
| 11 | Textúraeffektek (`env_effects`) | környezettükör és részlettextúrák | ~3 mp | `env_effects.json`, `output\env_textures\` |
| 12 | Felszerelés- és karakterképernyők (`inventory`) | panelek, tárgyikonok, betűtípusok | ~3 mp | `retail_inventory.json`, `output\ui\` |
| 13 | Főmenü és betöltőképek (`menu`) | a magyar menügrafika és szövegek | ~3 mp | `retail_ui.json`, `output\ui\` |
| 14 | Egérkurzor (`cursor`) | a gyári arany nyíl | 0 mp | `output\ui\cursor.png` |
| 15 | Tárgydefiníciók (`props_defs`) | a pályák díszítő tárgyai (`objects.txt`) | ~4 mp | `props_defs.json` |
| 16 | Indító logóvideók (`videos`) | `1.avi`, `2.avi`, `3.avi` átalakítása képkockákká + hanggá (ffmpeg) | ~1 mp | `output\videos\` |
| 17 | Hangok és zene (`audio`) | az összes gyári hang másolása, a zene MP3-má alakítása | ~5 mp | `output\audio\` |
| 18 | A játék lefordítása (`build`) | `cargo build --release` a `crates\level-viewer` mappában | ~9 perc (első alkalommal; később csak ha a kód változott) | `crates\level-viewer\target\release\level-viewer.exe` |

Az 1-17. lépés együtt a mérésben ~2 perc volt; a kinyert `output\` mappa ~1,5 GB, ~12 900 fájl. A fordítás helye ~2-3 GB (a letöltött
Rust-csomagok a felhasználói `.cargo` mappában kb. 1 GB-ot tesznek hozzá). Lassabb gépen vagy keveset szabad maggal mindez többszörös idő lehet.

Végül az `Indit.cmd` elindítja a játékot: `level-viewer.exe menu output` (a főmenüvel). A későbbi indítások gyorsak: az `output\.setup\ready.json` jelzi, hogy minden
kész (azonos kódra és kinyerési változatra), ilyenkor a program ellenőrzés és export nélkül azonnal indítja a játékot.

### Folytatás, ismétlés, kapcsolók

- **Megszakadt?** Indítsd újra az `Indit.cmd`-t: a kész lépéseket (jelző: `output\.setup\<lépés>.done`, pályánként `world-<pálya>.done`) kihagyja.
  Egy lépés akkor is újra fut, ha a játékfájlok megváltoztak (a `Lithtech.exe`, `cshell.dll`, `object.lto` mérete alapján), vagy ha a lépés után hiányzik valamelyik várt fájl.
- **Minden újra:** `Indit.cmd --force`, vagy töröld az `output\.setup` mappát.
- **Csak egy lépés:** `python -m tools.export_all --only worlds --force` (azonosítók: `python -m tools.export_all --list`).
- **Csak az export, fordítás nélkül:** `Indit.cmd --skip-build` (a játékot ilyenkor a program nem indítja); egyéb: `--game <mappa>`, `--output <mappa>`, `--jobs <n>`.
- **Naplók:** `output\.setup\logs\<lépés>.log` (a hibás lépés utolsó sorait a konzol is kiírja).
- **Környezeti változók:** `MESTER_GAME` (a játék mappája), `MESTER_JOBS` (a cargo párhuzamos feladatai), `CARGO_TARGET_DIR` (hová fordítson a Rust).
- **Már meglévő kinyerés:** ha az `output\` mappa teljes kinyerést tartalmaz, de nincs benne `.setup` (pl. fejlesztői gép), a program „örökbe fogadja”, és nem írja át.
- **Frissítés után** (új tárolóváltozat): az `Indit.cmd` észleli, ha a Rust kód megváltozott, és újrafordítja; ha az exportok formátuma változott, azokat is újrafuttatja.

## 5. Hibák és javításuk

Az `Indit.cmd` minden hibánál magyarul írja ki az okot és a teendőt, majd megáll (az ablak nyitva marad). A gyakoribbak a [README hibaelhárító táblázatában](../README.md#hibaelhárítás) vannak.
További tudnivalók:

- **Az `output\` mappa a tiéd:** a mentések (`save\`), a beállítások (`settings.json`, `autoexec.cfg`, `scripts\keys.cfg`) is itt vannak. A kinyert adatok újragenerálhatók.
- **Verziófigyelmeztetés:** ha a `cshell.dll`, a `Lithtech.exe` vagy az `object.lto` nem egyezik a 2.33-as magyar kiadással, a program figyelmeztet, de nem áll le. Más kiadással a kinyerés hibázhat vagy hibás lehet.
- **A szkriptek állapota:** a friss telepítés kódolt (betűpár-cserés) szkripteket tartalmaz, a program ezt is, a már „visszafejtett” szkripteket is olvassa. A
  `GYARI\cipher.exe`-t ne futtasd: átírja a szkripteket a mappában.


## 6. Ha kézzel akarod

```
cd projekt
python -m tools.setup_check --install      # ellenőrzés + a Python csomagok telepítése
python -m tools.export_all --skip-build    # csak a kinyerés
cd crates\level-viewer
cargo build --release                      # a játék lefordítása
.\target\release\level-viewer.exe menu ..\..\output
```

A régi indítók (`Forditas.cmd` debug fordítás, `Mesterlovesz-Ujrairva.cmd`, `Jatek.cmd`, `Palyanezo.cmd <pálya>`, `Stella.cmd`) tovább működnek: a release
fordítást használják, ha az `Indit.cmd` már elkészítette, különben a debug fordítást.
