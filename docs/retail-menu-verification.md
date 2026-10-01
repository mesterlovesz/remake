# Főmenü és pályabetöltés: 2026-09-28

> Frissítés: a pályaválasztó és a „Legutóbbi pályakezdés” helyét a gyári mentéslista, az F5 / F9 gyorsmentés és a pályakezdéskori automatikus gyorsmentés vette át; a menü minden oldala és a bizonyítékok: [retail-menus.md](retail-menus.md).

## Ellenőrzött útvonal

- `Jatek.cmd` a gyári grafikával készült főmenübe indít.
- Az „Új játék indítása” gomb az Előszó pályára vezet.
- A „Játék töltése” pályaválasztóban 28 bejegyzés látható: 26 összekapcsolt játszható kampánypálya, a külön Előszó, és a kimaradt Kiskína.
- Kiskína kiválasztásakor a saját gyári töltőképe jelenik meg, majd a `chinatown` pálya töltődik be. A csendes bejárási próba 17 NPC-t, betöltött küldetésszkriptet és működő mozgást igazolt.
- A teljes Előszó lejátszása után a `rh1-wiezienie2` betöltődött. A buszos próba `rh3-miasteczko1` → `rh3-miasteczko2` → `burmistrz1` útvonalon végigment.

## Bizonyíték és határ

Az eredményeket az `output/menu-click-route.log`, `output/intro-menu-regression.log` és `output/bus-menu-regression.log` naplók rögzítik. A megjelenésről az `output/menu-click-route-002.00.png` (főmenü), `output/menu-click-route-007.80.png` (pályaválasztó) és `output/menu-click-route-008.10.png` (Kiskína töltés) képek készültek. A 27 exportteszt, az 57 megjelenítőteszt és a 14 küldetés-runtime teszt sikeres volt. Minden játékpróba néma módban futott.

Az összes pálya betölthetőségét és a küldetésszkriptek értelmezhetőségét ellenőriztük, de a teljes kampány végigjátszását és minden feladat befejezését nem. Kiskína régi DAT állományában csak egy, szürke faltextúra található, ezért a játszható pálya egyes falai szürkék, bár az eredeti részletes töltőkép rendelkezésre áll. A többhelyes, tetszőleges pillanatot tároló mentés még hiányzik.
