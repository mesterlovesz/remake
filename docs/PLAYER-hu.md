# Játék indítása — három lépés

**Windows 10/11, 64 bit · Vulkan-képes videokártya · 5 GB szabad hely**

## 1. Töltsd le és csomagold ki a játékoscsomagot

[Mesterlövész Újratöltve — Windows ZIP](https://github.com/mesterlovesz/remake/releases/latest/download/Mesterlovesz-Ujratoltve-Windows.zip)

Jobb kattintás a ZIP-en → **Az összes kibontása**. Válassz például egy `D:\Mesterlovesz` mappát. A teljes csomagot bontsd ki; az indító mellett a `.player` mappára is szükség van.

## 2. Tedd az ISO-t az indító mellé

A [weboldal Játékfájlok gombjával](https://sniper.gay/#inditas) töltsd le az Archive.org-os magyar ISO-t. Másold abba a mappába, ahol az **Indit.bat** van. A fájl neve maradhat az eredeti; egy ISO legyen ebben a mappában.

```text
Mesterlovesz/
├── Indit.bat
├── A_mesterlovesz.iso
├── .player/
└── OLVASS-EL.txt
```

## 3. Dupla kattintás az Indit.bat-ra

Az indító kinyeri a játék adatait a lemezképből, és előkészíti a pályákat, modelleket, hangokat és átvezetőket. Az első alkalom néhány perc; hagyd nyitva az ablakot. Internet ekkor még kell a médiafeldolgozó automatikus, körülbelül 31 MB-os letöltéséhez.

Ezután megjelenik a játék főmenüje. A további indításokhoz már csak az **Indit.bat** kell, internet nélkül is. Az ISO az első sikeres előkészítés után eltávolítható. Az `output`, `GYARI` és `.player` mappát hagyd az indító mellett.

**Rustot, Pythont vagy az eredeti játék telepítőjét nem kell telepítened. Rendszergazdai jog sem szükséges.**

## Ha valami elakad

- **Hiányzik az ISO:** tedd közvetlenül az `Indit.bat` mellé. A letöltés fejeződjön be.
- **Több ISO van:** ebből a mappából csak A Mesterlövész ISO-ja maradjon ott.
- **Megszakadt az előkészítés:** indítsd újra; a már kész lépések kimaradnak.
- **Nem tud letölteni:** ellenőrizd az internetkapcsolatot, és indítsd újra.
- **Nem indul a játék:** frissítsd a videokártya illesztőjét. A kártyának Vulkan-támogatás kell.
- **Hibajelentés:** a hiba szövege és az `inditas.log`, illetve az `output/.setup/logs` naplója segít. [GitHub hibajelentés](https://github.com/mesterlovesz/remake/issues).

A letölthető ZIP előre lefordított játékot és indítót tartalmaz. Az eredeti játék fájljait az ISO-ból készíti elő; a lemezképet és a gyári fájlokat a remake csak olvassa. A mentések és beállítások az `output` mappában vannak.

Fejlesztőknek a forrásból indítás külön leírása: [SETUP-hu.md](SETUP-hu.md).
