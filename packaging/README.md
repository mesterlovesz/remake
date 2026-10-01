# Windows játékoscsomag készítése

Ez a leírás a kiadás készítőjének szól. A játékosoknak a [játékos útmutató](../docs/PLAYER-hu.md) három lépése elég.

A játék előre lefordítva kerül a ZIP-be. Az ISO-t a felhasználó adja hozzá. Az indító a csomag saját Pythonjával, PyCdlibbel és Unshielddel olvassa a telepítőadatokat, visszaállítja az eredeti fájlneveket, ellenőrzi a magyar 2.33-as kiadást, és futtatja az exportokat. A médiafeldolgozót az első futáskor közvetlenül a PyPI-ról tölti le; az ellenőrzött kerék SHA-256 értéke a `player_launcher.py` fájlban van. A további indítások működnek internet nélkül.

## Készítés Windows x64-en

A készítő gépén Rust MSVC, Visual Studio C++ Build Tools, Python 3 és Git szükséges. A Windows saját `tar.exe` programja kicsomagolja az Unshieldet. A játékos gépén ezek telepítésére nincs szükség.

```powershell
cargo build --release --locked --manifest-path crates/level-viewer/Cargo.toml
python -m tools.download_player_dependencies --out player-deps
python -m tools.build_player_package --source . --exe crates/level-viewer/target/release/level-viewer.exe --dependencies player-deps --out player-dist/Mesterlovesz-Ujratoltve
```

Ha külön célmappába fordítasz (`CARGO_TARGET_DIR`), az `--exe` értéke az ott készült fájl legyen. A csomagoló új célmappát kér, és ellenőrzi a rögzített függőségek SHA-256 értékeit. A `.player/package.json` tartalmazza a forráspillanat commitazonosítóját, a futtatható fájl és a függőségek ellenőrzőösszegeit. A függőségek licencszövegei és a `THIRD-PARTY-NOTICES.txt` a csomagban vannak.

A `player-dist/Mesterlovesz-Ujratoltve` mappát teljes egészében ZIP-be kell tenni, a rejtett `.player` mappával együtt. ISO, `GYARI`, `output`, személyes kép, mentés vagy weboldal ne kerüljön bele.

## Kiadás előtti próba

1. A végleges ZIP-et bontsd ki új, ékezetes mappanévvel is kipróbált helyre.
2. Tedd mellé az Archive.org magyar ISO-ját.
3. Olyan PATH-tal indítsd, amelyen nincs Rust, Python vagy FFmpeg.
4. `Indit.bat --prepare-only`: minden export sikerrel fejeződjön be.
5. Hang nélküli ellenőrzés: `Indit.bat --capture menu-proba.png`. Ellenőrizd a képet és az `inditas.log` fájlt.
6. Újraindításkor az exportok maradjanak ki. A hiányos ISO, több ISO és megszakított előkészítés hibajelzése legyen érthető.

Az indító célzott tesztjei:

```powershell
python -m unittest discover -s tests -p test_player_launcher.py -v
```

Az első csomagban a motor az `4753a3a343f54b9f22828bbb3af012f0df26d358` helyi forráspillanatból készült. A csomagolás és indító külön fejlesztés; a motor forrását nem módosítja.
