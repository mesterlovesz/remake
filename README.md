# A Mesterlövész: Újratöltve

A 2002-es **A Mesterlövész** (*Sniper: Path of Vengeance*, magyar 2.33-as kiadás) újraírása Rustban és Bevyben, az eredeti játékélményt követve.

**[Weboldal: sniper.gay](https://sniper.gay/)** · **[Kiskína: a kihagyott pálya](https://sniper.gay/kiskina.html)** · **[Képek és lejátszható hangok](https://sniper.gay/archive.html)**

## Mit tartalmaz?

- Az eredeti 27 kampánypálya betöltése és küldetésszkriptjei.
- Gyári szereplők, fegyverek, párbeszédek, feliratok, átvezetők, hangok és zene.
- Az eredeti menük, beállítások, mentés és töltés, gyorsmentés és gyorstöltés.
- Az eredeti pályafájlokból kinyert hálók, textúrák és lightmapek.
- A kampányból kihagyott **Kiskína** a **Bónusz** menüből indítható, külön mentéssel.
- Dokumentált visszafejtés: a kiolvasott értékek, kódcímek, ellenőrzött és közelítő részek a [docs](docs/) mappában vannak.

A cél az eredeti működés 1:1-es reprodukálása. Ez a visszafejtett szabályokat követő Rust motor- és játékrendszer-újraírás; nem teljes, binárisan egyező C/C++ dekompiláció egyszerű átfordítása. A teljes képpontos és viselkedési azonosságot még nem igazoltuk. Az automata kampánybejárás eredményei a kutatási dokumentumokban olvashatók.

## Egyszerű játékoscsomag

A tervezett folyamat: **ISO a mappába → dupla kattintás az indítóra → játék**, fejlesztői programok telepítése nélkül.

Ez a csomag még nem készült el. A mostani tároló forrásból futtatható; ehhez az alábbi előkészítés kell.

## Jelenlegi indítás Windows 10/11-en

### 1. A projekt letöltése

Ezen az oldalon kattints a **Code → Download ZIP** gombra, és csomagold ki a ZIP-et egy tetszőleges mappába. Például: `D:\Mesterlovesz\projekt`.

### 2. A fejlesztői eszközök telepítése

- **[Rust](https://rustup.rs/)**: futtasd a telepítőt. Fogadd el a **Visual Studio C++ Build Tools** telepítését is, ha felajánlja. Legalább Rust 1.89 szükséges.
- **[Python 3](https://www.python.org/downloads/)**: legalább 3.10. A telepítőben pipáld be az **Add python.exe to PATH** lehetőséget.
- Naprakész videokártya-illesztő és Vulkan-képes videokártya.

A telepítések után nyiss új ablakot, vagy indítsd újra a gépet, ha a telepítő ezt kéri.

### 3. Az eredeti magyar játék ISO-ja

A letöltési gomb a [weboldal indítási részében](https://sniper.gay/#inditas) található. Az **Archive.org-os magyar ISO-t** használd.

1. Kattints duplán az ISO-ra a Fájlkezelőben. Ha nem csatlakozik, jobb klikk → **Csatlakoztatás**.
2. A virtuális DVD-n nyisd meg az **A mesterlövész** mappát, majd indítsd a **setup.exe** fájlt.
3. Telepítési célként a projekt mappája melletti **GYARI** mappát add meg. Például: `D:\Mesterlovesz\GYARI`.
4. Az eredeti játékot nem kell elindítani, javítócsomag sem kell. A remake az eredeti fájlokat csak olvassa.

Az ISO önmagában még nem a telepített játék. Akkor jó a mappa, ha közvetlenül benne van a `Lithtech.exe` és a `cshell.dll`:

```text
Mesterlovesz/
├── GYARI/
│   ├── Lithtech.exe
│   ├── cshell.dll
│   ├── worlds/
│   ├── models/
│   └── ...
└── projekt/
    ├── Indit.cmd
    ├── crates/
    └── tools/
```

### 4. Dupla kattintás: Indit.cmd

Az indító ellenőrzi a szükséges programokat, telepíti a Python-segédcsomagokat, kinyeri az adatokat, lefordítja és elindítja a játékot. Az első alkalom sokáig tarthat és internetet igényel. Hagyd nyitva az ablakot.

A későbbi indításkor a kész lépések kimaradnak. Ha hiba történik, az ablak kiírja a teendőt; a megjavítása után újraindítható.

Részletes leírás: [docs/SETUP-hu.md](docs/SETUP-hu.md).

## Irányítás

| Gomb | Művelet |
|---|---|
| Egér, WASD | Körbenézés, mozgás |
| Shift, Ctrl | Futás, guggolás |
| Szóköz | Ugrás |
| Bal egér, R | Lövés, újratöltés |
| Jobb egér | Másodlagos fegyverfunkció |
| E | Használat, tárgyfelvétel, ajtó, beszélgetés |
| 1–8 | Fegyver elővétele |
| C, Z, X | Felszerelés, karakterinformáció, tulajdonságok |
| F5, F9 | Gyorsmentés, gyorstöltés |
| Esc | Játékmenü |

## Hibaelhárítás

- **Python nem található**: ellenőrizd a Python telepítésénél a PATH jelölőnégyzetet; indíts új ablakot.
- **link.exe / fordító nem található**: telepítsd a Visual Studio C++ Build Tools C++ eszközeit.
- **GYARI hiányzik**: közvetlenül a `GYARI/Lithtech.exe` legyen meg, ne egy további almappában.
- **Nem indul a megjelenítés**: frissítsd a videokártya-illesztőt; Vulkan-támogatás szükséges.
- **Első indítás lassú**: az adatok feldolgozása és a teljes fordítás egyszeri, hosszabb lépés. A részletes naplók az `output/.setup/logs/` mappába kerülnek.

## Kiskína és a kutatási archívum

A Kiskína a gyári telepítésben megmaradt, de a fő kampány nem vezet hozzá. Az archívum 17 szereplőt, párbeszédszkripteket, beszédhangokat, útvonaladatokat és a pálya kijáratát mutatja be. Külön jelöljük a közvetlenül bizonyított tényeket, a következtetéseket és a nyitott kérdéseket. A „teljesen elveszett, soha be nem tölthető pálya” állítását nem kezeljük bizonyított tényként.

A [webes archívumban](https://sniper.gay/archive.html) a hangok egyenként lejátszhatók, a képek és a kutatáshoz válogatott fájlok elérhetők. A játék teljes telepített adatállományát és az ISO-t ez a tároló nem tartalmazza.

## A weboldal

A statikus weboldal és a Kiskína médiaarchívuma [külön tárolóban](https://github.com/mesterlovesz/mesterlovesz.github.io) van. A játék forrásának letöltéséhez ezek nem szükségesek. A webes publikálási workflow GitHub Pages-re tölti fel az oldalt, a megosztásokhoz Open Graph előnézeteket készít.

## Megjegyzés

Nem hivatalos rajongói projekt. Az eredeti játék és gyári anyagai a jogtulajdonosoké.

Ezt az egész projektet egy LLM írta. Mindent [mannin1337](https://www.mannin.hu/) promptolt, emberi kód nincs a projektben.
