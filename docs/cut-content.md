# A Mesterlövész: fel nem használt és nehezen hozzáférhető tartalom

Kutatási állapot: **2026. szeptember 30.** (az első változat 2026. szeptember 28.). A vizsgálat alapja a felhasználó saját, változatlan `GYARI` telepítése. A „Kiskína” a gyári fájlok **`chinatown.dat`** pályája; „A Templom” a külön **`chinatown2.dat`** pálya. Ezt a kettőt nem szabad az internetes „Chinatown” említések alapján azonosnak venni.

**[Képes, hangokat lejátszó archívum megnyitása](cut-content.html)** (helyi dokumentum: a képek és hangok a `docs/cut-content-media/` mappából jönnek, amely gyári eredetű, ezért **nincs a git tárolóban**; `python -m tools.capture_kiskina` majd `python -m tools.build_cut_content_archive` előállítja) · [a média jegyzéke](cut-content-media/manifest.json) · a fel nem használt fájlok rendszeres leltára: [unused-content.md](unused-content.md)

Bizonyossági szintek (az archívumban is): **bizonyított** = közvetlenül a gyári fájlokból vagy egy futásból olvasható; **következtetett** = a bizonyítékokból következik; **ismeretlen** = az adatból nem dönthető el.

## Rövid válasz: elveszett pálya volt?

| Állítás | Amit az eddigi bizonyíték megenged |
|---|---|
| „A játékban egyáltalán nem lehetett Chinatownig eljutni.” | **Nem igaz.** Egy 2007-es játékos kifejezetten említ egy játszott Chinatown-küldetést és a kínai maffiát; a nyilvános MobyGames-galériában is van Chinatownnak nevezett felvétel. A fő kampány gyári útvonala eléri a `chinatown2` pályát. |
| „A külön `chinatown.dat` pálya normál játékban nem volt elérhető.” | **Erős, de nem végleges következtetés.** Semmilyen gyári szkript (`startlevel`, `runworld`), DAT-ajtó vagy kampányél nem mutat a `worlds\chinatown` világra (az összes `*.scene.json` objektum-tulajdonságát és a teljes `scripts` mappát átnéztük; a `cshell.dll`, `Lithtech.exe`, `object.lto` fájlokban sincs „chinatown” szöveg). A `podziemia1c` kijárata egyenesen `chinatown2`-re mutat. Nem találtunk nyilvános beszámolót, amely fájlnévvel, pályacímmel vagy egyértelmű képpel igazolná a külön `chinatown.dat` futását a gyári játékban. Ez nem bizonyítja, hogy parancssorból, más kiadásban vagy más úton senki sem tölthette be. |
| „A régi kínai pályában NPC-k sem voltak.” | **A gyári adatok szerint hamis.** A `chinatown.dat` 17 NPC-elhelyezést tartalmaz (6 fegyveres katona, 4 civil, 3 csirkeszerű állat, 4 újságlap). Hogy mind aktiválódott volna a gyári motorban, eredeti futtatás nélkül nem mondható. |
| „Teljes lost media.” | **Pontosabban: a gyári telepítésben megőrzött, a fő útvonalból kihagyott pálya.** DAT, útvonalháló, AI-szkript, 34 párbeszédblokk, 45 beszédhang, zene és töltőkép fennmaradt. A gyári játékbeli működése és az esetleges alternatív elérési útja nem tisztázott. |

## Mit tudott erről a nyilvános internet?

1. A [MobyGames eredeti játékhoz tartozó Windows-képgalériája](https://www.mobygames.com/game/9628/sniper-path-of-vengeance/screenshots/) egy felvételt Chinatownként nevez meg. A kép és a felirat egy kínai tematikájú játékrész nyilvános nyomát adja, de nem azonosítja a DAT fájlt. Az archívum alapján önmagában nem dönthető el, hogy `chinatown` vagy `chinatown2` látható.
2. A [GRY-Online játékosfórumán](https://www.gry-online.pl/gry/sniper-path-of-vengeance/komentarze/zd1b4f) egy 2007. október 17-i hozzászóló azt írja, hogy végigjátszotta a játékot, és különösen tetszett neki a Chinatown-küldetés a kínai maffiával. Ez első kézből származó játékosbeszámoló, de pályafájlt nem nevez meg. A `chinatown2` kampányútvonalával összeegyeztethető.
3. Célzottan kerestünk `chinatown.dat`, `Kiskína` és a játék címe együttes említésére, valamint a pálya elérhetőségéről szóló leírásra. **Nem találtunk ellenőrizhető nyilvános beszámolót arról, hogy valaki a külön `chinatown.dat` pályát a gyári kampányban betöltötte és végigjátszotta**, vagy hogy ott NPC nélkül játszott. A keresési eredmény hiánya nem bizonyítja, hogy ilyen beszámoló nem létezik.

**Meddig jutottak a felhasználók?** A nyilvános bizonyíték legalább egy játszott, kínai maffiás Chinatown-küldetésig terjed. A gyári kampánygráf ezt a `chinatown2` / „A Templom” szakaszhoz köti. A külön `chinatown.dat` pálya eredeti játékban történt betöltésének mértékét jelenleg **nem lehet igazolni**.

Megjegyzés: a `rh7a-tunele` pályán a hős egy aktív, a kampányban elhangzó sora („Ezen keresztül talán eljuthatok Kiskínába.”, `Tunne02T`) a „Kiskína” nevet használja. Ez nem a kihagyott pályára utal feltétlenül: a név a kínai negyedet jelölheti, amelynek A Templom is része.

## Amit a gyári állományok elemzésével találtunk

| Bizonyíték | Eredmény | Gyári / kinyert fájl |
|---|---|---|
| Külön pálya, régebbi fájlverzió | `chinatown.dat`: 3,86 MB, **DAT v83, 2002-05-21**; `chinatown2.dat`: 5,48 MB, **v85, 2002-12-23**. A kampány többi pályája v85. A pálya tehát a „májusi” szerkesztőváltozat terméke (következtetett). | [chinatown.dat](../../GYARI/worlds/chinatown.dat), [chinatown2.dat](../../GYARI/worlds/chinatown2.dat) |
| Cím és töltés | Címkulcs `Kiskína` (`>LNChinatown1`), a következőé `A Templom`; saját gyári töltőkép (`chinatown.pcx`, a Templomé `temple.pcx`). | [gameai.txt](../../GYARI/scripts/ai/gameai.txt) 2399–2405. sor, [text_keys.txt](../../GYARI/scripts/text_keys.txt) 127–129. sor |
| Kilépés | A pálya saját kétszárnyú ajtaja (`b_door0` + `b_door1`, (96/160; −638)) `Skok_do_levelu` értéke `worlds\chinatown2`. A szkriptblokk sem `startlevel`, sem más kilépési parancsot nem tartalmaz. | [régi pálya objektumai](../output/chinatown.scene.json) |
| Szereplők | **17** `o_postac`: 3 Glockos (`china zolnierz`) és 3 HK G8-as (`china zolnierz2`) kínai katona, 4 civil (`cywil1`–`cywil4`), 3 `qra` (csirkeszerű kis állat), 4 `gazeta` (szélfújta újságlap). Nincs Tong Pau. | [játékmenet-export](../output/chinatown.gameplay.json) |
| Geometria és kinézet | 310 objektum; 13 184 ütközési háromszög; **a világháló mindössze 2 anyaga ugyanaz a szürke `blacha szara.dtx` lemeztextúra** (A Templomé 76, más pályáké 20–183); a lightmap 4096×202 atlasz, 11 950 lightmapelt lap; köd (67,89,90), 1–1600. A szereplők, tárgyak és kellékek (65 modell) teljesen textúráltak. | [scene.json](../output/chinatown.scene.json), [visual report](../output/chinatown.visual.report.json), [anyagok](../output/chinatown.visual.materials.json) |
| Navigáció | `chinatown.pth`: 2289 csomópont, 14 394 él. StartPoint (7520; −1088; −1136), irány: észak. | [chinatown.pth](../../GYARI/worlds/chinatown.pth) |

### Helyesbítés a 2026. szeptember 28-i változathoz

Az akkori képek a **régi remake-et** mutatták (szürke, textúra- és lightmap nélküli terek, NPC-viselkedés nélkül, tükrözés előtt). Az adatból két dolog ma is áll, egy viszont elavult:

* **Áll:** a világháló két anyaga a szürke lemeztextúra: a pálya falai nyers, „greybox”-szerű textúrázásúak (bizonyított).
* **Elavult:** a „szürke, élettelen szoba” képe. A remake azóta a gyári lightmapeket, köddel, gyári fényekkel, gamma-szabályokkal, a tükrözéssel és a gyári NPC-viselkedéssel jeleníti meg a pályát. A nyers falak így a baked fényekkel (meleg lámpafény a papírlámpások körül, hideg kékeszöld köd) jelennek meg; a szereplők, lámpások, automaták teljes textúrájúak. A nyitókép (kezdőpont, `Kierunek` = észak) egy sötét, közeli falra néz; ez a gyári kezdőpozíció és irány, nem kamerahiba (docs/retail-visual.md).

### A pálya felépítése (keletről nyugatra)

Az archívumban felülnézeti térkép és 30 feletti felvétel van (kezdőépület, utca, civilek, őrök, automaták, ajtó, háztetők és varjak, a párbeszédpanel, összevetés A Templommal).

1. **Kezdőépület (x 6400–7700):** StartPoint, 3 katona (`o_postac1`, `o_postac3` Glock, `o_postac2` HK G8), gyógycsomag, italok, gabonapehely, a Glock és tára.
2. **Átjáró (x 5950–6400):** a `Mason13` marker; **a hozzá tartozó párbeszéd a gyári szkriptekben nincs definiálva** (a remake naplózza: `Original dialogue is undefined: Mason13`; bizonyított).
3. **Fő utca (x 2450–5900):** négy civil, négy újságlap, csirkeszerű állatok, 10 lampion, 5 fali telefon, 5 varjú (`gawron-lata`, 13,333 s-os baked repülőkör, 550 egység sugarú), kis szobák rádióval, TV-vel, főzőedényekkel.
4. **Összekötő folyosó (x 1950–2450):** a `ChinioleOstrzezenie` marker (2333; −1236): a hős gondolata, „Szerintem ez az út vezet a Maffia belső területére.” (`PrzekroczylPierwszaStrefe`).
5. **Nyugati épület (x 1000–1950):** 3 őr (`o_postac9` Glock, `o_postac10`, `o_postac11` HK G8), 12 felvehető tárgy, 2 automata.
6. **Hosszú folyosó és kijárat (x 0–980):** 2 automata, a `ChinioleAtak` marker (128; −922, `PrzekroczylSwietaStrefe`: „Itt óvatosnak kell lennem, Tong Pau emberei mindenütt ott vannak.”), a kétszárnyú ajtó A Templomra.

Felvehető tárgyak (18): 5 sör (+15 élet, +6 alkohol), 5 kóla (+5), 3 Schnickers (+10), 1 gabonapehely (+15), 1 alma (+5), 1 gyógycsomag (+50), 1 Glock és 1 Glock-tár (17 töltény). Kulcs vagy küldetéstárgy nincs. Fények: 90 `Light`, 63 `ObjectLight`, 4 `DirLight`, 37 fénysprite. Nincs zónajelző, karakterkibocsátó, detektor, hangforrás.

### Küldetéslogika (gameai.txt 2399–2805. sor)

* **Nincs tényleges célkitűzés-lánc.** A szkript egy „beszivárgás” szabályrendszer: a katonák figyelmeztetnek (`Chiniole10`), kérdeznek (`Chiniole12`: „Ide nem jöhetsz be!”, 4 válasz), a hős eljátszhatja, hogy hírt hoz Tong Pau testvéréről, Hanról (O'Hara a kapcsolattartó, mindketten halottak: `NiesieWiadomoscOHanie`), vagy támadást vált ki (`ChinioleAtakuja` → `hostileattack`). Két zónát jelöl a két marker; a pálya célja a kijárati ajtó elérése. A kapuőr-kérdések (`Chiniole13/14`) csak a második zóna után, egy katona közelében indulnak; Kiskína kijárati helyiségében nincs katona, így ezek ott csak kóborló katonánál jöhetnek elő (következtetett).
* **24 akció**, ebből 19 szó szerint azonos A Templom szkriptjében, 1 (`tongpo1`) csak a név kis-/nagybetűjében tér el, 4 (a civilek elfutása: `cywil1`–`cywil4`) csak Kiskínában van; A Templom 3 saját akciót ad (arany macska, kijárat).
* **A `ChinioleAtakuja`, `PrzekroczylBrame` stb. jelzők csak a Kiskína-blokkban vannak deklarálva**, A Templom blokkja ezekre hivatkozik. A `tongpo1` akció egy `tongpo` nevű szereplőre hivatkozik, aki Kiskínában nincs.
* Az `ai_*.txt` és `dialogi_*.txt` fájlok a telepítésben 0 bájtosak: a tartalom a `gameai.txt`-be és `dialogi.txt`-be van olvasztva.

### Párbeszédek és hangok

* **34 `Chiniole*` párbeszédblokk** (`dialogi.txt` 2359–2805. sor); a jelzőkövetés szerint Kiskínában 29 indulhat el, A Templomban 32. Mindkét pálya 27 blokkot közösen használ.
* **Csak Kiskínában** hangzhat el: `Chiniole10` (01.wav, „Ez az út vezet a szent helyhez. Kérlek menj innen.”) és `ChinioleOstrzezenie` (02.wav). **Csak A Templomban**: `ChinioleBrama`, `Chiniole18` (20.wav), `Chiniole29` (31.wav), `Chiniole30` (32.wav), és ezzel a `Chiniole19` (201.wav, a hős „Nem hiszem el, hogy ennyire okos vagyok.”, **+1000 XP**), mert a `PrzekroczylBrame` jelzőt csak a `ChinioleBrama` marker állítja, ilyen marker Kiskínában nincs.
* **45 beszédfájl:** 38 hivatkozott (32 közös, 4 csak A Templom, 2 csak Kiskína), **7 sehonnan sem hívott**: `07.wav` (a `Chiniole4` blokk `titlesnd` sora közvetlenül a `04.wav`-ra mutat, így a „És miért akarna Tong Pau találkozni veled?” sorhoz a „Nem bízom benned, kém vagy!” hang tartozik; az eredeti szándék következtetett), `22.wav` (a `China22S` kulcs a `06.wav`-ra mutat), valamint `24.wav`–`28.wav` (civilek, 22,05 kHz; a hozzájuk tartozó `China24T`…`China30T` kulcsok hangfájlja `empty.wav`; a fájlok és a szövegek összerendelése csak a számozáson alapul).
* Az archívumban hangonként lejátszóval ellátott **átirat-táblázat** van: fájl → szövegkulcs → beszélő → szöveg → melyik pályán hangzik el. A hangokat nem hallgattuk meg; a beszélő a szöveg előtagjából adódik.

### Zene

A gyári játék pályánként egyetlen `muza` sávot rendel (`chinatown_spokoj` Kiskínához és a `podziemia1c` pályához, `chinatown_akcja` A Templomhoz); nyugodt/harci váltás nincs: a `MuzaSpokChinioleWlaczona` / `MuzaAtakChinioleWlaczona` jelzőket sem a szkript, sem a `cshell.dll` nem olvassa (docs/retail-audio.md; bizonyított). Kiskínában végig a nyugodt sáv szól.

### Mi egyedi Kiskínában (összevetés A Templommal)

A két pálya közös a katona/őr-rendszerben és a dialógusfában. Kiskínára jellemző: a mindennapi élet (négy civil, csirkeszerű állatok, újságlapok, lámpások, telefonok, automaták), a három zónás út keletről nyugatra, a nyers, placeholder-szerű világtextúra, a két csak ide tartozó hang. Hiányzik belőle: Tong Pau, az arany macska, a kapu (`ChinioleBrama` marker), a karakterkibocsátók (A Templomban 3), a zónajelzők, a tisztek, a sörétes és a P90. A Templom: 76 anyag, lila alkonyég, pagoda, 14 szereplő (HK G8, FN sörétes, Glock, SIG, P90), 15 tárgy (főleg gyógykészlet).

## Játszható-e a remake-ben? (2026-09-30)

**Igen: a pálya betölthető, végigjátszható, a kijárata átvisz A Templomba.** Próbák (a jelenlegi `main`-re épült fejetlen, néma futások):

| Próba | Eredmény |
|---|---|
| `catalog` (28. pálya) | PASS, a gyári szkript betölt, 17 NPC. |
| `menu` | PASS („Új játék” után a pálya `travel.pending` kéréssel betöltődik, 17 NPC, a kezdőpontból lehet járni). |
| `walk` (`MESTER_WALK_LEVELS=chinatown`, új címke: `walk-chinatown`) | **PASS**: a bot a kezdőpontból a `b_door0`-ig ér, az ajtón át A Templom kezdőpontján (−480; −404; −1984) érkezik. 201,1 játékmásodperc, 8999 egység, 0 sebzés, 8 rövid (40–190 egységes) segített ugrással: 2 a kezdőépületben, 3 az `o_postac10` őr mellett, 2 az automatáknál, 1 a kijárati helyiségben. |
| `ai` `patrol` | PASS (`o_postac5`, a fázissebesség 99%-a). |
| `ai` `fight` / `far` | **Nem állapítható meg**: a próba öt katona mellett nem talál szabad állóhelyet, a hatodiknál (`o_postac11`) a saját időelvárásán bukik. A katonák tényleges tűzharca Kiskínában nem mért. |

A `walk_probe.rs` egyetlen sorral bővült (a `chinatown` következő pályája a saját ajtaja szerint `chinatown2`); `tools/run_probes.py` új címkéje: `walk-chinatown`.

**Elérés (2026-10-01):** a főmenü új, kilencedik sora, a **Bónusz** (a tulajdonos kérésére, nem gyári; [retail-menus.md](retail-menus.md)) a „Kiskína (kiadatlan pálya)” bejegyzéssel új játékot indít a pályán (alapértelmezett karakter, gyári töltőkép); a kijárati ajtó (`b_door0`) nem A Templomba, hanem a főmenübe vezet, a gyorsmentés a `save\bonus.sav`-ba megy. A `Palyanezo.cmd chinatown` parancs és a próbák továbbra is működnek. Próba: `bonus` (docs/probes.md).

**Ami akadályoz / hiányzik:** nincs bejövő átmenet a kampányból (a kampány nem folytatódik a pályáról: a Bónusz-menüből indítva a kijárat a főmenübe tér vissza); a világ új karakterrel indul, nem az előző pálya felszerelésével; a `Mason13` marker nem vált ki semmit; a civilek sorai hangtalanok (gyári állapot). Hogy a gyári motor a pályát hibátlanul betöltötte volna: ismeretlen.

## Egyéb, a fő kampánygráfhoz nem kötött állományok

Teljes leltár: [unused-content.md](unused-content.md). Itt csak a Kiskína-kutatás közben előkerült megállapítások:

| Állomány | Megállapítás | Nyitott kérdés |
|---|---|---|
| [`nic.dat`](../../GYARI/worlds/nic.dat) | v85, 2002-12-16, 1,37 MB. Kocsma-jelenet: 13 NPC (`pijak`, `dziwka_kibel`, `barman`, `laska`, 9 `karalec`), 14 tárgy, 7 ajtó, 63 anyag. Nincs `level worlds\nic` blokk a `gameai.txt`-ben, a kampány nem hivatkozik rá; a szereplők a `knajpa` pálya kocsmarészének szereplői ellenségek nélkül. | Teszt- vagy ideiglenes világ (következtetett). |
| [`outro.dat`](../../GYARI/worlds/outro.dat) | v83, 2002-05-12. Kis színpad: két asztal, két automata, egy `outro` `o_cutscene`, 7 fény. A befejező jelenetet a játékban a `rh12-lab2` saját `o_cutscene` objektuma indítja; az `outro.dat`-ra semmi nem mutat (a `scenki.txt` `runworld` sorai csak `rh3-miasteczko1` és `rh1-wiezienie2`). Kiskínával azonos verzió és hónap. | A korai, külön befejező jelenet-világ lehetett (következtetett). |
| [`katscena.dat`](../../GYARI/worlds/katscena.dat) | **v70**, 2002-03-13: a legrégebbi világfájl; a DAT-olvasó csak az objektumait fejti vissza (`geometry: not decoded (DAT v70)`). 2 szereplő (`policjant_intro`, `bohater_intro`), egy `intro` `o_cutscene`, 20 sprite, füstkibocsátó. Az `intro` jelenetet a játék a `rh1-wiezienie1` pályán játssza. | Az intro korai, külön világa lehetett (következtetett). |
| `sounds/enemies/china/mosb_m_old.wav` | 423 KB, 2002-07; sem a szkriptekben, sem a `cshell.dll`-ben nincs hivatkozása. | Az `Engine.REZ` hivatkozásait nem vizsgáltuk. |
| `test2`, `pudlo`, `test`, `boks` | Szkriptblokkok vannak a `gameai.txt`-ben, de azonos nevű DAT nincs a telepített `worlds` mappában. | Más buildhez vagy fejlesztői próbához tartozhattak. |
| `Mason13`, `tongpo` | Hiányzó hivatkozások a Kiskína-szkriptben (lásd fent). | |

A gyári `worlds` könyvtárban összesen **31 DAT** van: 26 összekapcsolt kampánypálya, egy előszóvilág, a külön Kiskína és a három fenti további DAT. Ez fájlleltár, nem 31 végigjátszható küldetés. Lásd a [részletes kampányfelmérést](campaign-research.md).

## A média és a generálás

A `docs/cut-content-media/` (képek, beszédhangok, zene, térkép) **gitignore-olt**, gyári eredetű. A zenefájlok MPEG Layer III adatot tartalmazó WAV konténerek; az archívum a tömörített hangot újrakódolás nélkül `.mp3`-ba csomagolja. A beszédhangok változatlan WAV másolatok. A `GYARI` könyvtárat a dokumentálás nem módosította. Eszközök: `tools/capture_kiskina.py` (a fejetlen felvételek; nézőpontok: `tools/kiskina_views.py`), `tools/kiskina_data.py` (adatkinyerés, térkép), `tools/build_cut_content_archive.py` (a HTML).

## Mi dönthetné el a nyitott kérdést?

Egy gyári motorral készült, azonosítható `Kiskína` című betöltés vagy végigjátszás; egy eredeti mentés, amelyben `worlds\chinatown` szerepel; vagy a kiadott motor/`cshell.dll` pontos `runworld` hívásának visszafejtése. A nyilvános Chinatown-képek és játékosbeszámolók fájlszintű azonosítás nélkül ehhez nem elegendők.
