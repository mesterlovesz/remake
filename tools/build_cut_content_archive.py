"""Build the local, browsable archive docs/cut-content.html about the cut retail level Kiskina (chinatown.dat) and the content around it.

  python -m tools.capture_kiskina          # the screenshots (headless, silent; needs the ready level-viewer exe), once per game version
  python -m tools.build_cut_content_archive

The retail installation (../GYARI) is read-only. Retail-derived media (speech, music, the screenshots of the game, the loading pictures, a plan drawn
from the level data) is copied / converted into docs/cut-content-media, which is GITIGNORED: only this script, the HTML page and the Markdown text
are tracked. The page is a local document: open it from the file system; the audio players work without a server.
Every claim carries an evidence level: proved (bizonyitott: read from the retail files or shown by a run), inferred (kovetkeztetett), unknown (ismeretlen).
"""

from __future__ import annotations

import hashlib
import html
import json
import os
import re
import shutil
import struct
import time
from pathlib import Path

from PIL import Image

from tools import kiskina_data as kd

ROOT = kd.ROOT
SOURCE = kd.SOURCE
OUTPUT = kd.OUTPUT
DOCS = ROOT / "docs"
MEDIA = DOCS / "cut-content-media"
SHOTS = MEDIA / "shots"
B = kd.B
TODAY = "2026. szeptember 30."

esc = html.escape


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def extract_mp3_from_wave(source: Path, target: Path) -> None:
    """Rewrap a source MPEG Layer III WAVE; never recompress its audio."""
    data = source.read_bytes()
    if data[:4] != b"RIFF" or data[8:12] != b"WAVE":
        raise ValueError(f"Not WAVE: {source}")
    position, codec, payload = 12, None, None
    while position + 8 <= len(data):
        kind = data[position : position + 4]
        size = struct.unpack_from("<I", data, position + 4)[0]
        body = data[position + 8 : position + 8 + size]
        if kind == b"fmt ":
            codec = struct.unpack_from("<H", body)[0]
        if kind == b"data":
            payload = body
        position += 8 + size + (size & 1)
    if codec != 85 or not payload or payload[:1] != b"\xff":
        raise ValueError(f"Expected MPEG Layer III WAVE: {source}")
    target.write_bytes(payload)


EV = {"p": ("bizonyított", "ev-p"), "i": ("következtetett", "ev-i"), "u": ("ismeretlen", "ev-u")}


def ev(level: str) -> str:
    text, css = EV[level]
    return f'<span class="ev {css}">{text}</span>'


# ---------------------------------------------------------------------------------------------------------------------------------------
# the picture gallery: view name -> (caption, evidence). The positions of the views live in tools/kiskina_views.py.
GALLERY = [
    ("A kezdőépület (kelet)", "A pálya keleti végén, a 6400–7400 közötti x-tartományban áll egy gyűrű alakú épület; itt indul a játékos, és itt vannak az első katonák, a segélycsomag és a Glock.", [
        ("a01-start-retail", "A gyári kezdőpont (7520, −1088, −1136) a StartPoint `Kierunek` = észak irányával: a játékos egy sötét falra néz. A remake az eredeti kezdőpozíciót és irányt használja, ez a pálya tényleges nyitóképe (a docs/retail-visual.md is „close wall (dark)”-ként rögzíti).", "p"),
        ("a02-start-west", "Ugyanez a pont nyugat felé (a pálya többi része felé) fordulva: szűk előtér két ajtóval és falra szerelt dobozokkal. A világháló falain csak a szürke lemeztextúra van; a színt a lightmap és a köd adja.", "p"),
        ("a03-soldier-o_postac1", "Folyosó a kezdőépületben ablakokkal. A folyosó végén áll `o_postac1` (kínai katona, Glock, alapfázis `stoi`). Jobbra lent az első segélycsomag (`medpack`, +50 élet) és egy gabonapehelydoboz (`cereal box`, +15).", "p"),
        ("a04-soldier-o_postac3", "`o_postac3`, a második Glockos katona. A `china zolnierz` modellnek négy bőrváltozata van (`ile_skinow 4`): ez a példány lila kabátot kapott.", "p"),
        ("a07-ring-corridor", "A gyűrűfolyosó másik nézete: katona, pad, ablakok és az asztalon a gyógycsomag.", "p"),
    ]),
    ("Az átjáró és a fő utca", "A kezdőépületből egy lefelé vezető átjáró nyílik a központi utcanegyedre (x 2400–5950). Ezt a tartományt szűk, ködös utcák, lépcsők, korlátok, papírlámpások és kirakatok töltik ki.", [
        ("b01-passage-mason13", "Az átjáró a nyugati kijárat felé (x ≈ 6300). Ennél a helynél, (5950, −1131, −1112) körül áll az `o_marker_dialog2`, amely a `Mason13` nevű párbeszédet hívná; ilyen párbeszéd a gyári szkriptekben nincs (lásd lejjebb).", "p"),
        ("b02-street-civilian", "A fő utca keleti bejárata: lámpás épületek, fakorlát, köd (kék-zöld `Kolor_Mgly` 67,89,90, látótávolság 1600). Az előtérben `o_postac5`, az első civil (`cywil1`) áll.", "p"),
        ("b03-street-market", "Az utca közepe: fakorlát, papírlámpások a falakon, a távolban a köd.", "p"),
        ("b04-street-back-east", "Visszatekintés kelet felé: erkélyes házak, fakorlát és lámpák a ködben.", "p"),
        ("b05-civilian-cywil1", "`o_postac5` (cywil1, 50 HP, fegyvertelen) a korlát mögött; a civil a `Cywil1Nuda` jelzőt állítja, és elfut, ha meglátja a játékost előhúzott fegyverrel.", "p"),
        ("b06-civilian-cywil2", "`o_postac7` (cywil2) lámpások alatt a korlátnál.", "p"),
        ("b07-civilian-cywil3", "`o_postac8` (cywil3) egy fedett bejáratban, mellette a fali telefon (`telefon_wiszacy`) és egy kuka.", "p"),
        ("b08-civilian-cywil4", "`o_postac6` (cywil4) az utca déli oldalán, lámpások alatt.", "p"),
        ("b09-newspaper-gazeta", "Az utca közepe lépcsőkkel; a földön apró sötét foltok: a négy szétszórt `gazeta` (újságlap) objektum. A távolban egy álló alak.", "i"),
        ("b10-qra-o_postac21", "`o_postac21` (`qra`): kis, szárnyas jószág (csirkeszerű; a modell neve `qra`) az udvar kövezetén. A `qra` HP 10, `stoi` → `biegnie` (fut, sebesség 24).", "i"),
        ("b12-tv-room", "Zsákutca az utca déli oldalán: rádió és egy zöld, régi készülék a kövön (a helyen `radio`, `telewizor`, `czajnik`, `konserwa`, `fajki` kellékek állnak).", "p"),
    ]),
    ("A nyugati épület, a folyosó és a kijárat", "Nyugat felé (x 1000–1950) egy újabb épületkomplexum következik három őrrel, majd hosszú folyosó vezet a kijárati helyiséghez (x 0–260).", [
        ("c01-west-corridor", "A nyugatra vezető folyosó: falilámpák, a végén nyílás. Itt halad át az út a második (kisebb) zónába.", "p"),
        ("c02-marker-ChinioleOstrzezenie", "A `ChinioleOstrzezenie` marker (2333, −1092, −1236) közelében: ha a játékos belép, a hős gondolata hallható („Szerintem ez az út vezet a Maffia belső területére.”), és a `PrzekroczylPierwszaStrefe` jelző beáll.", "p"),
        ("c03-guard-o_postac10", "`o_postac10`, HK G8 géppisztolyos katona (`china zolnierz2`, alapfázis `nuda`) sötét kabátban, a nyugati épület folyosóján.", "p"),
        ("c04-guard-o_postac11", "`o_postac11`, a második HK G8-as katona sötétpiros öltözékben, hosszú fegyverrel, a nyugati épület folyosóján.", "p"),
        ("c05-guard-o_postac9", "`o_postac9`, Glockos katona a folyosón; a padlón piros üdítősdobozok (`a can of coke`).", "p"),
        ("c06-west-shop", "Két üdítő- és kávéautomata (`automat do coli`, `automat_kawa`) a nyugati épületben és három üdítősdoboz a padlón (+5 élet darabonként).", "p"),
        ("c08-exit-door", "A kijárati helyiség: lépcső és a kétszárnyú ajtó (`b_door0` + `b_door1`). A `b_door0` `Skok_do_levelu` értéke `worlds\\chinatown2`: ez viszi a játékost A Templom pályára.", "p"),
    ]),
    ("Háztetők és varjak", "Ezek a nézőpontok nem játékospozíciók: a kamera a háztetők fölé van helyezve (MESTER_SPAWN), hogy látszódjon a felső réteg és a repülő madarak. A szabad kamera nem bizonyítja, hogy a háztetők járhatóak.", [
        ("d01-roof-rook", "Nézet a tetők fölül: víztorony és a ködbe vesző házak, fent a pálya felső lezárása. A kép felső részén egy repülő varjú (`gawron-lata`) apró sziluettje látszik.", "p"),
        ("d02-roof-watertower", "Víztornyos tető lámpával a homlokzaton; a varjú a kép jobb felső részén látszik (nagyítása lejjebb).", "p"),
        ("d04-roof-east", "Keleti tetők, a ház oldalán világító ablak; a varjú a bal felső részen.", "p"),
    ]),
]

CAPTION_DIALOG = [
    ("e01-dialog-Chiniole12", "`Chiniole12` a remake párbeszédpaneljében (a `dialogue` próbával idézve): a kínai katona figyelmeztet, a játékos négy válasz közül választ. A szöveg a gyári text_keys-ből jön."),
    ("e02-dialog-Chiniole13", "`Chiniole13`, a „második zóna” kapuőrének kérdése három válasszal (a hír Tong Pau testvéréről beengedi, a másik két válasz támadást vált ki)."),
    ("e03-dialog-Chiniole8", "`Chiniole8`: a katona az „O'Hara” kapcsolattartóról kérdez; a két válasz közül a második (O'Hara és Han halott) a hírvivő állapotot állítja be."),
    ("e04-dialog-Chiniole20", "`Chiniole20`: az „Automat1” akció hívja, ha a játékos hírt visz és túl sokáig ácsorog egy katona közelében."),
]

COMPARE = [
    ("f01-templom-start", "A Templom (chinatown2) kezdőpontja: lila alkonyég, pagoda, kőburkolat. A világ 76 anyaggal, az egész pálya más hangulat."),
    ("f02-templom-gate", "A Templom kapuja a `ChinioleBrama` marker (1124, −187, 1120) közelében: kertfal, lámpa és őr."),
    ("f03-templom-tongpo", "Tong Pau (`tongpo`, P90) a Templom folyosóján; Kiskínában nincs ilyen szereplő."),
]


def prepare_media() -> dict[str, dict]:
    """Copy / convert everything the page links; returns the manifest entries."""
    (MEDIA / "img").mkdir(parents=True, exist_ok=True)
    manifest: list[dict] = []

    def record(kind: str, file: str, source: str, path: Path, note: str = "") -> None:
        entry = {"kind": kind, "file": file, "source": source, "sha256": sha256(path)}
        if note:
            entry["note"] = note
        manifest.append(entry)

    for name in ("chinatown", "temple"):
        source = SOURCE / "misc" / "loading_l" / f"{name}.pcx"
        target = MEDIA / "img" / f"loading-{name}.png"
        Image.open(source).convert("RGB").save(target)
        record("picture", f"img/{target.name}", str(source.relative_to(SOURCE)), target, "gyári töltőkép PCX-ből PNG-be alakítva")
    # plan drawn from the exported level data
    plan = MEDIA / "img" / "plan.png"
    kd.plan(kd.load(), plan)
    record("picture", "img/plan.png", "output/chinatown.{collision.obj,gameplay.json,scene.json}", plan, "felülnézeti térkép a kinyert pályaadatokból")
    # the remake screenshots, as JPEG
    for shot in sorted(SHOTS.glob("*.png")):
        target = MEDIA / "img" / f"{shot.stem}.jpg"
        Image.open(shot).convert("RGB").save(target, quality=88)
        record("picture", f"img/{target.name}", f"docs/cut-content-media/shots/{shot.name}", target, "a remake fejetlen felvétele (tools/capture_kiskina.py)")
    # rook close-up cut from the roof picture
    roof = SHOTS / "d02-roof-watertower.png"
    if roof.exists():
        crop = Image.open(roof).convert("RGB").crop((660, 200, 780, 280)).resize((480, 320), Image.LANCZOS)
        target = MEDIA / "img" / "d02-rook-closeup.jpg"
        crop.save(target, quality=90)
        record("picture", "img/d02-rook-closeup.jpg", "d02-roof-watertower kivágás", target, "a varjú nagyítása (4x, nem élesebb a felvételnél)")
    speech_root = SOURCE / "sounds" / "speech" / "chinatown"
    for source in sorted(speech_root.glob("*.wav")):
        target = MEDIA / f"speech-{source.name.lower()}"
        shutil.copyfile(source, target)
        record("speech", target.name, str(source.relative_to(SOURCE)), target)
    for source in sorted((SOURCE / "sounds" / "enemies" / "china").glob("*.wav")):
        if source.stem.startswith(("halt", "dead")):
            target = MEDIA / f"china-{source.name.lower()}"
            shutil.copyfile(source, target)
            record("sfx", target.name, str(source.relative_to(SOURCE)), target)
    for mood in ("spokoj", "akcja"):
        source = SOURCE / "sounds" / "muza" / f"chinatown_{mood}.wav"
        target = MEDIA / f"chinatown_{mood}.mp3"
        extract_mp3_from_wave(source, target)
        record("music", target.name, str(source.relative_to(SOURCE)), target, "az MP3 bitfolyam az eredeti WAV konténerből, újrakódolás nélkül kiemelve")
    (MEDIA / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return {e["file"]: e for e in manifest}


# ---------------------------------------------------------------------------------------------------------------------------------------
def analysis() -> dict:
    """Everything the tables need, computed from the retail files."""
    level = kd.load("chinatown")
    templom = kd.load("chinatown2")
    keys = kd.text_keys()
    blocks = kd.dialog_blocks()
    reach_k, starters_k = kd.reachable_dialogs("chinatown", blocks)
    reach_t, starters_t = kd.reachable_dialogs("chinatown2", blocks)
    return {"level": level, "templom": templom, "keys": keys, "blocks": blocks, "reach_k": reach_k, "reach_t": reach_t, "starters_k": starters_k, "starters_t": starters_t}


def wav_name(value: str) -> str:
    return value.replace("\\", "/").split("/")[-1].lower()


def speaker_of(text: str, role: str) -> str:
    if role == "answer":
        return "Hős (a játékos): válasz"
    if text.startswith("Kínai katona:"):
        return "Kínai katona"
    if text.startswith("Egy ember:"):
        return "Civil (egy ember)"
    return "Hős (a játékos): gondolat"


def strip_speaker(text: str) -> str:
    for prefix in ("Kínai katona:", "Egy ember:"):
        if text.startswith(prefix):
            return text[len(prefix) :].strip()
    return text


def transcript_rows(a: dict) -> tuple[list[dict], list[dict]]:
    """(rows with a voice file, rows whose sound key is empty.wav). One row per (speech file, text key, dialogue block) use."""
    keys, blocks = a["keys"], a["blocks"]
    uses: dict[str, list[dict]] = {}
    silent: list[dict] = []
    for name, block in blocks.items():
        cmds = block["commands"]
        title = next((v for c, v in cmds if c == "title"), None)
        titlesnd = next((v for c, v in cmds if c == "titlesnd"), None)
        nxt = {c: v for c, v in cmds}
        entries = []
        if title:
            entries.append((title, titlesnd, "title", ""))
        for n in range(1, 5):
            answer = next((v for c, v in cmds if c == f"answer{n}"), None)
            if answer:
                snd = next((v for c, v in cmds if c == f"answer{n}snd"), None)
                entries.append((answer, snd, "answer", nxt.get(f"onchoice{n}dialog", "")))
        for text_key, snd, role, goes in entries:
            text, _ = keys.get(text_key, ("?", 0))
            if snd and snd.startswith(">"):
                sound_key = snd
                path = keys.get(sound_key, ("", 0))[0]
                how = f"kulcs {sound_key}"
            elif snd:
                path, how = snd, "közvetlen fájl a blokkban (titlesnd)"
            else:
                path, how = "", "nincs hangkulcs"
            row = {"block": name, "line": block["line"], "text_key": text_key, "text": text, "role": role, "goes": goes, "how": how, "path": path}
            if wav_name(path) == "empty.wav" or not path:
                silent.append(row)
            else:
                uses.setdefault(wav_name(path), []).append(row)
    order = sorted(uses, key=lambda f: (int(f[:-4]), len(f)))
    files = []
    for f in order:
        files.append({"file": f, "uses": uses[f]})
    return files, silent


def status_of(file: str, uses: list[dict], a: dict) -> tuple[str, str, str]:
    """(label, css, note) for a speech file."""
    in_k = [u for u in uses if u["block"] in a["reach_k"]]
    in_t = [u for u in uses if u["block"] in a["reach_t"]]
    if in_k and in_t:
        return "Kiskína + A Templom", "st-both", ""
    if in_k:
        return "csak Kiskína", "st-k", "A Templom szkriptje a blokkot nem éri el."
    if in_t:
        return "csak A Templom", "st-t", "Kiskínában a blokk nem indulhat (hiányzik a marker / jelző)."
    return "egyik pályán sem", "st-none", ""


def seconds_of(path: Path) -> str:
    try:
        return f"{kd.wav_seconds(path):.2f}".replace(".", ",") + " s"
    except Exception:
        return "?"


def section_transcript(a: dict) -> str:
    files, silent = transcript_rows(a)
    speech_dir = SOURCE / "sounds" / "speech" / "chinatown"
    used_files = {f["file"] for f in files}
    orphan = sorted((p.name.lower() for p in speech_dir.glob("*.wav") if p.name.lower() not in used_files), key=lambda f: (int(f[:-4]), len(f)))
    rows = []
    for item in files:
        f, uses = item["file"], item["uses"]
        label, css, note = status_of(f, uses, a)
        player = f'<audio controls preload="none" src="cut-content-media/speech-{esc(f)}"></audio>'
        spec = seconds_of(speech_dir / f)
        first = True
        for use in uses:
            sp = speaker_of(use["text"], use["role"])
            goes = f' → <code>{esc(use["goes"])}</code>' if use["goes"] else ""
            lead = f'<td rowspan="{len(uses)}"><code>{esc(f)}</code><br><small>{spec}</small></td><td rowspan="{len(uses)}">{player}</td>' if first else ""
            tail = f'<td rowspan="{len(uses)}"><span class="st {css}">{label}</span><br><small>{esc(note)}</small></td>' if first else ""
            how = "" if use["how"].startswith("kulcs") else f'<br><small class="warn">{esc(use["how"])}</small>'
            rows.append(f'<tr>{lead}<td><code>{esc(use["text_key"])}</code><br><small>{esc(use["block"])} · dialogi.txt {use["line"]}. sor</small>{how}</td><td>{esc(sp)}</td><td>{esc(strip_speaker(use["text"]))}{goes}</td>{tail}</tr>')
            first = False
    orphan_rows = []
    for f in orphan:
        note = {
            "07.wav": "A `China07T` kulcs hangja lenne, de a `Chiniole4` blokk `titlesnd` sora közvetlenül a `04.wav`-ra mutat: a sor szövege a „És miért akarna Tong Pau találkozni veled?”, a hallható hang a `04.wav`.",
            "22.wav": "A `China22S` kulcs a `06.wav`-ra mutat (a `China06T` és `China22T` szövege csaknem azonos: „Bandaháború(ban)…”); a 22.wav feltehetően a `China22T` saját felvétele lenne.",
        }.get(f, "Civil-beszéd; a `China24T`…`China28T` kulcsok hangfájlja `empty.wav`, ezért a fájl sehonnan sem hívódik meg. Az összerendelés a számozáson alapul (nem hallgattuk meg).")
        level = "i"
        orphan_rows.append(f'<tr><td><code>{esc(f)}</code><br><small>{seconds_of(speech_dir / f)}{", 22,05 kHz" if f in ("22.wav", "24.wav", "25.wav", "26.wav", "27.wav", "28.wav") else ""}</small></td><td><audio controls preload="none" src="cut-content-media/speech-{esc(f)}"></audio></td><td>{esc(note).replace("`", "")}</td><td>{ev(level)}</td></tr>')
    silent_rows = []
    for use in silent:
        silent_rows.append(f'<tr><td><code>{esc(use["text_key"])}</code><br><small>{esc(use["block"])} · {use["line"]}. sor</small></td><td>{esc(speaker_of(use["text"], use["role"]))}</td><td>{esc(strip_speaker(use["text"]))}</td><td><code>{esc(wav_name(use["path"]) or "—")}</code></td></tr>')
    return f"""
<h2 id="atirat">Beszédhangok átirata: fájl → szövegkulcs → beszélő</h2>
<p>A táblázat a <code>text_keys.txt</code> szövegkulcsaiból és a <code>dialogi.txt</code> <code>Chiniole*</code> blokkjaiból készült; a „beszélő” a szöveg előtagjából (Kínai katona / Egy ember) vagy a hiányából (a hős saját gondolata és válaszai) adódik. A <b>Kiskína / A Templom</b> oszlop a két pálya szkriptjének jelzőkövetéséből számolt elérhetőség ({ev("p")}): egy blokk akkor „elérhető”, ha egy marker, egy akció vagy egy elért blokk válasza elindíthatja, és az akció feltételül szabott jelzői beállíthatók. A hangokat nem hallgattuk meg; a beszélő azonosítása a szöveg alapján történik.</p>
<table class="wide"><thead><tr><th>Fájl</th><th>Lejátszás</th><th>Szövegkulcs · blokk</th><th>Beszélő</th><th>Szöveg (→ következő blokk)</th><th>Hol hangzik el</th></tr></thead><tbody>{"".join(rows)}</tbody></table>
<h3>Kiskína hangfájljai, amelyekre egyetlen hangkulcs sem mutat</h3>
<table class="wide"><thead><tr><th>Fájl</th><th>Lejátszás</th><th>Megfigyelés</th><th>Szint</th></tr></thead><tbody>{"".join(orphan_rows)}</tbody></table>
<h3>Hang nélküli sorok (a hangkulcs <code>empty.wav</code>)</h3>
<table><thead><tr><th>Szövegkulcs</th><th>Beszélő</th><th>Szöveg</th><th>Hangfájl</th></tr></thead><tbody>{"".join(silent_rows)}</tbody></table>
"""


def section_dialogs(a: dict) -> str:
    blocks = a["blocks"]
    rows = []
    for name, block in blocks.items():
        cmds = block["commands"]
        title = next((v for c, v in cmds if c == "title"), None)
        text = a["keys"].get(title, ("", 0))[0] if title else ""
        sets = [f"+{v}" for c, v in cmds if c == "set"] + [f"−{v}" for c, v in cmds if c == "unset"] + [f"XP {v}" for c, v in cmds if c == "expgained"]
        k = "; ".join(a["starters_k"].get(name, [])) or "—"
        t = "; ".join(a["starters_t"].get(name, [])) or "—"
        rows.append(f'<tr><td><code>{esc(name)}</code><br><small>{block["line"]}. sor</small></td><td>{esc(text)}</td><td>{esc(", ".join(sets))}</td><td>{esc(k)}</td><td>{esc(t)}</td></tr>')
    return f"""
<h3 id="blokkok">A {len(blocks)} párbeszédblokk és ki indítja</h3>
<table class="wide"><thead><tr><th>Blokk</th><th>Cím (kimondott szöveg)</th><th>Hatás (+ beállít, − töröl)</th><th>Kiskína</th><th>A Templom</th></tr></thead><tbody>{"".join(rows)}</tbody></table>
<p><small>A „—” azt jelenti: az adott pálya szkriptje a blokkot nem indíthatja el. {ev("p")}</small></p>
"""


def section_actors(a: dict) -> str:
    g = a["level"]["game"]
    nav = g["navigation"]["nodes"]

    def nearest(x: float, z: float) -> str:
        n = min(nav, key=lambda n: (n["pos"][0] - x) ** 2 + (n["pos"][2] - z) ** 2)
        return f'g{n["graph"]}#{n["id"]}' + (" (pontosan rajta)" if abs(n["pos"][0] - x) < 1 and abs(n["pos"][2] - z) < 1 else "")

    notes = {
        "china zolnierz": "kínai katona (Glock, 17 töltény); `stoi` → `on_kontakt` riasztás, 4 bőrváltozat",
        "china zolnierz2": "kínai katona (HK G8, 50 töltény); `nuda` → `do_broni`, 4 bőrváltozat",
        "qra": "kis szárnyas állat (csirkeszerű), 10 HP, nem néz a játékosra",
        "gazeta": "szélfújta újságlap: `lezy` → `leci` (sebesség 32) → `obraca`; nonsolid, nem vérzik",
    }
    rows = []
    for n in g["npcs"]:
        ch = g["characters"][n["definition_name"]]
        links = ", ".join(l["action"] for l in n["script_links"]) or "—"
        head = {h[0]: h[1] for h in ch["header"]}
        skins = head.get("ile_skinow", "0")
        note = notes.get(n["definition_name"], "civil: elfut, ha fegyvert lát (`cywilN` akció), beszélgetésre válaszol")
        rows.append(
            f'<tr><td><code>{esc(n["name"])}</code></td><td>{esc(n["definition_name"])}<br><small>{esc(Path(n["model"]).name)}</small></td><td>{esc(n["weapon"] or "—")}</td>'
            f'<td>{esc(n["default_phase"])}</td><td>{n["hp"]:.0f}</td><td>{esc(head.get("exp_gained", "—"))}</td><td>{esc(head.get("ucieka_jak_mniej_niz", "—"))}</td>'
            f'<td>({n["pos"][0]:.0f}; {n["pos"][2]:.0f})<br><small>y {n["pos"][1]:.0f} · nav {nearest(n["pos"][0], n["pos"][2])}</small></td><td>{esc(links)}</td><td>{esc(note)}</td></tr>'
        )
    phases = []
    for name, c in g["characters"].items():
        phases.append(f'<tr><td><code>{esc(name)}</code></td><td>{esc(", ".join(c["phases"]))}</td></tr>')
    return f"""
<h2 id="szereplok">A 17 szereplő</h2>
<p>Forrás: a DAT <code>o_postac</code> objektumai, a <code>postacie.txt</code> definíciók és a pálya <code>gameai</code> blokkja ({ev("p")}). A „nav” a legközelebbi csomópont (gráf#sorszám) a <code>chinatown.pth</code> útvonalhálóban (2289 csomópont; a sorszámok gráfonként ismétlődnek), az objektumok többsége pontosan egy csomóponton áll: az objektumnak külön útvonal-tulajdonsága nincs, a járőrözés (`patrol`, sebesség 48) a háló élein történik, ahogy a többi pályán is. A kínai katonák <code>odleglosc_kontaktu</code> értéke 3000, <code>kat_kontaktu</code> 89: a kapcsolatfelvételt a megfelelő AI-szabályok adják (docs/retail-ai.md).</p>
<table class="wide"><thead><tr><th>Objektum</th><th>Definíció · modell</th><th>Fegyver</th><th>Alapfázis</th><th>HP</th><th>XP</th><th>Menekül &lt;</th><th>Hely (x; z)</th><th>Szkript</th><th>Megjegyzés</th></tr></thead><tbody>{"".join(rows)}</tbody></table>
<details><summary>A nyolc karakterdefiníció fázisai</summary><table><thead><tr><th>Definíció</th><th>Fázisok</th></tr></thead><tbody>{"".join(phases)}</tbody></table></details>
"""


def section_script(a: dict) -> str:
    header, actions = kd.level_script("chinatown")
    _, actions_t = kd.level_script("chinatown2")
    by_t = {x["name"]: x for x in actions_t}
    same = [x["name"] for x in actions if x["name"] in by_t and by_t[x["name"]]["body"] == x["body"]]
    only_k = [x["name"] for x in actions if x["name"] not in by_t]
    only_t = [x["name"] for x in actions_t if x["name"] not in {y["name"] for y in actions}]
    differ = [x["name"] for x in actions if x["name"] in by_t and x["name"] not in same]
    bool_lines = [i + 1 for i, l in enumerate(kd.read_cp1250(SOURCE / "scripts" / "ai" / "gameai.txt")) if 2399 <= i + 1 <= 2430 and l.strip().startswith("bool")]
    rows = []
    for act in actions:
        body = "<br>".join(esc(l) for l in act["body"])
        status = "azonos A Templomban" if act["name"] in same else ("csak Kiskínában" if act["name"] in only_k else "csak a nevek kis-/nagybetűjében tér el")
        rows.append(f'<tr><td><code>{esc(act["name"])}</code><br><small>gameai.txt {act["line"]}. sor</small></td><td><code>{body}</code></td><td>{status}</td></tr>')
    flags = "".join(f"<li><code>{esc(l)}</code></li>" for l in header if l.startswith("bool"))
    return f"""
<h2 id="szkript">A küldetésszkript</h2>
<p>A <code>gameai.txt</code> 2399–2805. sora (<code>level worlds\\chinatown</code>). A pálya betöltőképe <code>chinatown.pcx</code>, a címe az <code>&gt;LNChinatown1</code> kulcs („Kiskína”), a zenéje <code>chinatown_spokoj</code>. A blokk egyetlen <code>startlevel</code> vagy <code>endlevel</code> parancsot sem tartalmaz; <b>a pálya kilépését nem a szkript, hanem a DAT ajtaja adja</b> ({ev("p")}). Az <code>//include scripts\\ai\\ai_chinatown.txt</code> és a többi <code>ai_*.txt</code> fájl a telepítésben 0 bájtos (a tartalom a <code>gameai.txt</code>-be van olvasztva) {ev("p")}.</p>
<h3>Jelzők (<code>bool</code>)</h3><ul class="cols">{flags}</ul>
<p>A <code>MuzaSpokChinioleWlaczona</code> és <code>MuzaAtakChinioleWlaczona</code> jelzőt sem a blokk, sem a dialógusok nem állítják és nem olvassák; a zene nem vált ({ev("p")}, docs/retail-audio.md).</p>
<h3>A {len(actions)} akció</h3>
<table class="wide"><thead><tr><th>Akció</th><th>Feltételek → hatás (a forrássorok)</th><th>A Templom blokkjában</th></tr></thead><tbody>{"".join(rows)}</tbody></table>
<div class="box"><p><b>Kiskína és A Templom szkriptje:</b> {len(same)} akció szó szerint azonos a két blokkban, {len(differ)} csak a nevek kis-/nagybetűjében tér el ({esc(", ".join(differ))}: <code>TongPo</code> / <code>tongpo</code>). Csak Kiskínában van: {esc(", ".join(only_k))} (a négy civil elfut, ha fegyvert lát). Csak A Templomban: {esc(", ".join(only_t))} (az arany macska és a kijárat). A <code>ChinioleAtakuja</code>, <code>PrzekroczylBrame</code>, <code>NiesieWiadomoscOHanie</code> stb. jelzők <b>csak a Kiskína-blokkban vannak deklarálva</b> ({bool_lines[0]}–{bool_lines[-1]}. sor), A Templom blokkja ezekre hivatkozik, de nem deklarálja őket újra {ev("p")}. A Templom blokkja a <code>MuzaSpokChiniole2Wlaczona</code> / <code>MuzaAtakChiniole2Wlaczona</code> jelzőket deklarálja, a zene itt sem vált. Ebből az következik, hogy a két blokk egy közös forrásszkript két példánya {ev("i")}; melyik volt előbb, a szövegből nem dönthető el, a DAT fájlok mentési verziója és ideje (lásd alább) viszont Kiskínát tekinti a korábbinak.</p></div>
"""


def section_items(a: dict) -> str:
    g = a["level"]
    items = json.loads((OUTPUT / "chinatown.items.json").read_text(encoding="utf-8"))
    effect = {"a can of beer": "+15 élet, +6 alkohol", "a can of coke": "+5 élet", "Schnickers wafer bar": "+10 élet", "cereal box": "+15 élet", "an apple": "+5 élet", "medpack": "+50 élet", "Glock": "fegyver (17 töltényes tár)", "Glock ammo": "17 töltény"}
    groups: dict[str, list[dict]] = {}
    for it in items:
        groups.setdefault(it["kind"], []).append(it)
    rows = []
    for kind, lst in sorted(groups.items(), key=lambda kv: -len(kv[1])):
        where = "; ".join(f'({i["pos"][0]:.0f}; {i["pos"][2]:.0f})' for i in lst)
        rows.append(f"<tr><td>{esc(kind)}</td><td>{len(lst)}</td><td>{esc(effect.get(kind, ''))}</td><td><small>{esc(where)}</small></td></tr>")
    return f"""
<h3>Felvehető tárgyak (18 darab, `o_item_podnoszony`)</h3>
<table><thead><tr><th>Tárgy</th><th>db</th><th>Hatás (items.txt)</th><th>Helyek (x; z)</th></tr></thead><tbody>{"".join(rows)}</tbody></table>
<p><small>Kulcs, küldetéstárgy vagy lőszertömeg nincs: csak az élelmiszerek, egy gyógycsomag és egy Glock a hozzá tartozó tárral. A tárgyak érintésre vehetők fel. {ev("p")}</small></p>
""".replace("`", "")


def section_objects(a: dict) -> str:
    lv = a["level"]
    lights = {k: len(kd.kind(lv, k)) for k in ("Light", "ObjectLight", "DirLight")}
    models: dict[str, int] = {}
    for o in kd.kind(lv, "o_obiekt"):
        models[kd.model_name(o)] = models.get(kd.model_name(o), 0) + 1
    models_rows = "".join(f"<tr><td><code>{esc(m)}</code></td><td>{c}</td></tr>" for m, c in sorted(models.items(), key=lambda kv: (-kv[1], kv[0])))
    wp = kd.kind(lv, "WorldProperties")[0]["properties"]
    rooks = [o for o in kd.kind(lv, "o_obiekt") if kd.model_name(o).startswith("gawron")]
    rook_txt = "; ".join(f'({o["properties"]["Pos"][0]:.0f}; {o["properties"]["Pos"][1]:.0f}; {o["properties"]["Pos"][2]:.0f})' for o in rooks)
    rot = kd.kind(lv, "b_rotator")[0]["properties"]
    doors = kd.kind(lv, "b_door")
    door_rows = "".join(f'<tr><td><code>{esc(o["properties"]["Name"])}</code></td><td>({o["properties"]["Pos"][0]:.0f}; {o["properties"]["Pos"][1]:.0f}; {o["properties"]["Pos"][2]:.0f})</td><td>{esc(o["properties"].get("Skok_do_levelu", "") or "—")}</td><td>{o["properties"].get("Obrot")}°, {o["properties"].get("Predkosc_obrotu")}°/s, önzár {o["properties"].get("Czas_samozamkniecia")} s, játékos nyitja: {o["properties"].get("Gracz_otwiera")}</td></tr>' for o in doors)
    return f"""
<h2 id="objektumok">Objektumok, ajtók, fények, madarak</h2>
<h3>A 310 objektum fajtái</h3>
<p>{", ".join(f'<code>{esc(k)}</code> {n}' for k, n in sorted(((k, sum(1 for o in lv["objects"] if o["kind"] == k)) for k in {o["kind"] for o in lv["objects"]}), key=lambda kv: -kv[1]))}. Nincs benne zónajelző (<code>o_marker_zmienna</code>), detektor, karakterkibocsátó (<code>o_emiter_postaci</code>), hangforrás vagy kulcs/zár: a pálya szerkezete egyszerűbb, mint A Templomé ({ev("p")}).</p>
<h3>Ajtók és kijárat</h3>
<table><thead><tr><th>Ajtó</th><th>Hely</th><th>Pályaváltás (<code>Skok_do_levelu</code>)</th><th>Paraméterek</th></tr></thead><tbody>{door_rows}</tbody></table>
<p>A <code>b_door1</code> a <code>b_door0</code> párja (<code>Nast_obiekt</code>), a kétszárnyú ajtó két fele; az ugrást a <code>b_door0</code> végzi, a portálneve <code>portal1</code>. A játékos kulcs nélkül, használattal nyithatja ({ev("p")}). A kijárat előtt áll a <code>ChinioleAtak</code> marker (128; −922).</p>
<h3>Fények és köd</h3>
<p>{lights["Light"]} <code>Light</code>, {lights["ObjectLight"]} <code>ObjectLight</code>, {lights["DirLight"]} <code>DirLight</code>; 37 <code>d_sprite</code> (fénycsóva-sprite-ok); ködszín ({wp["Kolor_Mgly"][0]:.0f}, {wp["Kolor_Mgly"][1]:.0f}, {wp["Kolor_Mgly"][2]:.0f}), ködtartomány {wp["Start_Mgly"]:.0f}–{wp["Koniec_Mgly"]:.0f}, látótávolság {wp["Widocznosc"]:.0f} ({ev("p")}). A világháló lightmapje 4096×202 képpontos atlasz, az exportált 13 184 háromszögből 11 950 lightmapelt.</p>
<h3>Varjak és forgó tárgy</h3>
<p>Öt <code>gawron-lata</code> objektum repül a fő utca fölött (középpontjaik: {esc(rook_txt)}). A modell egyetlen animációja egy ≈550 egység sugarú, 13,333 s-os zárt kör a szárnycsapásokkal együtt; az objektumdefinícióban nincs <code>anim_raz</code>, ezért a motor végtelenül ismétli (docs és <code>birds.rs</code>, {ev("p")}). A <code>b_rotator0</code> ({rot["Pos"][0]:.1f}; {rot["Pos"][1]:.1f}; {rot["Pos"][2]:.1f}) a Z tengely körül {rot["ObrotZ"]:.0f}°/s-mal forog: az utca mellett egy forgó szerkezet; a DAT {len(a["level"]["scene"]["movable_world_models"])} mozgatható világmodellje: a forgó szerkezet, a nyolc <code>b_transparent</code> és a két ajtó {ev("p")}. A forgó szerkezet mibenléte (ventilátor vagy cégér) {ev("u")}.</p>
<details><summary>A 65 modell-prop jegyzéke</summary><table><thead><tr><th>Modell</th><th>db</th></tr></thead><tbody>{models_rows}</tbody></table></details>
"""


def section_compare(a: dict) -> str:
    k, t = a["level"], a["templom"]
    def count(lv: dict, kind: str) -> int:
        return sum(1 for o in lv["objects"] if o["kind"] == kind)
    def weapons(lv: dict) -> str:
        w = {}
        for n in lv["game"]["npcs"]:
            w[n["weapon"] or "—"] = w.get(n["weapon"] or "—", 0) + 1
        return ", ".join(f"{v}× {k}" for k, v in sorted(w.items(), key=lambda kv: -kv[1]))
    rep_k = json.loads((OUTPUT / "chinatown.visual.report.json").read_text(encoding="utf-8"))
    rep_t = json.loads((OUTPUT / "chinatown2.visual.report.json").read_text(encoding="utf-8"))
    def stat(path: str) -> str:
        p = SOURCE / "worlds" / path
        ts = time.strftime("%Y-%m-%d", time.localtime(p.stat().st_mtime))
        version = struct.unpack("<I", p.read_bytes()[:4])[0]
        return f"{p.stat().st_size / 1e6:.2f} MB · DAT v{version} · {ts}"
    rows = [
        ("Fájl", stat("chinatown.dat"), stat("chinatown2.dat")),
        ("Cím / töltőkép / zene", "Kiskína · chinatown.pcx · chinatown_spokoj", "A Templom · temple.pcx · chinatown_akcja"),
        ("Hangulat", "éjszakai, ködös kínai negyed; kék-zöld köd", "kert és templomudvar, lila alkonyég, pagoda"),
        ("Világháló anyagai", f'{rep_k["materials"]} (mindkettő `blacha szara.dtx`)', f'{rep_t["materials"]}'),
        ("Háromszög / lightmapelt", f'{rep_k["triangles"]} / {rep_k["lightmapped_faces"]}', f'{rep_t["triangles"]} / {rep_t["lightmapped_faces"]}'),
        ("Objektumok", str(len(k["objects"])), str(len(t["objects"]))),
        ("Szereplők", f'{len(k["game"]["npcs"])}: {weapons(k)}', f'{len(t["game"]["npcs"])}: {weapons(t)}'),
        ("Civil / állat / újság", "4 civil, 3 `qra`, 4 `gazeta`", "nincs"),
        ("Tong Pau (`tongpo`)", "nincs, pedig a `tongpo1` akció hivatkozik rá", "van (P90), az `o_postac25`"),
        ("Felvehető tárgyak", f'{count(k, "o_item_podnoszony")}: italok, édesség, 1 gyógycsomag, Glock + tár', f'{count(t, "o_item_podnoszony")}: főleg gyógykészletek és fájdalomcsillapítók, a `Golden cat.`'),
        ("Dialógus-markerek", "3: `ChinioleOstrzezenie`, `ChinioleAtak`, `Mason13` (nincs ilyen blokk)", "4: `ChinioleBrama`, `Chiniole18`, `Chiniole29`, `ChinioleAtak`"),
        ("Zónajelző / karakterkibocsátó", "nincs", f'{count(t, "o_marker_zmienna")} `o_marker_zmienna`, {count(t, "o_emiter_postaci")} `o_emiter_postaci`, 1 tiltómarker'),
        ("Kilépés", "`b_door0` → `worlds\\chinatown2`", "szkript: `startlevel worlds\\rh9-fabryka` (ha nála van az arany macska)"),
    ]
    trs = "".join(f"<tr><th>{esc(n)}</th><td>{esc(x).replace('`', '')}</td><td>{esc(y).replace('`', '')}</td></tr>" for n, x, y in rows)
    return f"""
<h2 id="templom">Mi egyedi Kiskínában? Összevetés A Templommal (chinatown2)</h2>
<table class="wide compare"><thead><tr><th></th><th>Kiskína (<code>chinatown</code>)</th><th>A Templom (<code>chinatown2</code>)</th></tr></thead><tbody>{trs}</tbody></table>
<div class="box">
<p><b>Amit a két pálya közösen használ:</b> ugyanazt a kínai katona/őr-rendszert (kapuőr-kérdések, figyelmeztetés, „Tong Pau testvéréről hozok hírt” kártya), ugyanazt a <code>Chiniole*</code> párbeszédkészletet, ugyanazt a dialógus-fát. {ev("p")}</p>
<p><b>Ami egyedi Kiskínában:</b> (1) a <b>mindennapi élet</b>: négy civil, csirkeszerű állatok, szélfújta újságlapok, lámpások, utcai telefonok, automaták; (2) a <b>két hang, amelyet csak Kiskína szkriptje érhet el</b>: a <code>Chiniole10</code> figyelmeztetés („…szent helyhez…”) és a <code>ChinioleOstrzezenie</code> hősgondolat, mert A Templomban nincs megfelelő marker; (3) a civilek négy <code>cywilN</code> elfutó akciója; (4) a hosszú, három zónás út a keleti kezdőépülettől a nyugati kijáratig; (5) a világháló <b>két</b> anyaga és nyers, placeholder-szerű textúrázása. {ev("p")}</p>
<p><b>Ami Kiskínából hiányzik A Templomhoz képest:</b> Tong Pau, az arany macska, a kapu (<code>ChinioleBrama</code> marker), a szobor-párbeszéd (<code>Chiniole29</code>), a karakterkibocsátók, a <code>China2ChceWyjsc</code> kijárati jelző és a tisztek/sörétes lövegek. A <code>PrzekroczylBrame</code> soha nem állítódik be Kiskínában, így a <code>Chiniole19</code> („Nem hiszem el, hogy ennyire okos vagyok.”, <b>+1000 XP</b>) és a hozzá tartozó <code>201.wav</code> Kiskínában elérhetetlen; A Templomban a <code>ChinioleBrama</code> marker adja. {ev("p")}</p>
<p><b>Mit jelent ez?</b> A fájlverzió (v83, 2002. május 21.) alapján a pálya a „májusi szerkesztő”-korszak terméke, amelyet A Templom (v85, 2002. december 23.) váltott le; Kiskína szkriptje sablonja lehetett A Templom szkriptjének, és a Kiskína-blokk szolgál a közös jelzők deklarációjául. Hogy a fejlesztők szándékosan hagyták-e ki, vagy csak a végső szerkesztőváltás után nem mentették újra, nem tudható. {ev("i")}</p>
</div>
<div class="grid">{"".join(f'<figure><img src="cut-content-media/img/{n}.jpg" alt=""><figcaption>{esc(c).replace("`", "")}</figcaption></figure>' for n, c in COMPARE)}<figure><img src="cut-content-media/img/loading-chinatown.png" alt="Kiskína töltőkép"><figcaption>Kiskína töltőképe (<code>chinatown.pcx</code>, gyári kép).</figcaption></figure><figure><img src="cut-content-media/img/loading-temple.png" alt="A Templom töltőkép"><figcaption>A Templom töltőképe (<code>temple.pcx</code>, gyári kép).</figcaption></figure></div>
"""


def gallery_html() -> str:
    parts = []
    for title, lead, shots in GALLERY:
        figs = []
        for name, caption, level in shots:
            if not (MEDIA / "img" / f"{name}.jpg").exists():
                continue
            figs.append(f'<figure><img loading="lazy" src="cut-content-media/img/{name}.jpg" alt="{esc(name)}"><figcaption>{esc(caption).replace("`", "")} {ev(level)}<br><small>{esc(name)}</small></figcaption></figure>')
        if title.startswith("Háztetők") and (MEDIA / "img" / "d02-rook-closeup.jpg").exists():
            figs.append('<figure><img src="cut-content-media/img/d02-rook-closeup.jpg" alt="varjú nagyítás"><figcaption>A varjú nagyítása a <code>d02</code> képből (4×): szárnyait kitárva kering a háztetők fölött. A madár pontos fajtája nem azonosítható; a modell neve <code>gawron-lata</code> („repülő varjú”).</figcaption></figure>')
        parts.append(f"<h3>{esc(title)}</h3><p>{esc(lead)}</p><div class=\"grid\">{''.join(figs)}</div>")
    dialog = "".join(f'<figure><img loading="lazy" src="cut-content-media/img/{n}.jpg" alt=""><figcaption>{esc(c).replace("`", "")} {ev("p")}</figcaption></figure>' for n, c in CAPTION_DIALOG if (MEDIA / "img" / f"{n}.jpg").exists())
    parts.append(f'<h3>Párbeszédpanel</h3><div class="grid">{dialog}</div>')
    return "".join(parts)


def section_playability() -> str:
    return f"""
<h2 id="jatszhato">Játszható-e a remake-ben?</h2>
<p>Igen, <b>a pálya betölthető, végigjátszható, és a kijárata átvisz A Templomba</b> (2026. szeptember 30., agent/cut). A mérések és próbák ({ev("p")}):</p>
<table><thead><tr><th>Próba</th><th>Eredmény</th></tr></thead><tbody>
<tr><td><code>catalog</code> (a 28. pálya, <code>chinatown</code>)</td><td>PASS: a gyári küldetésszkript betölt, 17 NPC megjelenik (<code>CATALOG chinatown: mission=true npcs=17</code>).</td></tr>
<tr><td><code>menu</code> (főmenüpróba)</td><td>PASS (ma újrafuttatva): „Új játék” után a pálya a <code>travel.pending</code> kéréssel betöltődik, 17 NPC-vel, és a kezdőpontból lehet járni (<code>FŐMENÜPRÓBA: world=chinatown npcs=17</code>). A próba a menü <i>kódjából</i> kéri a pályát, nem egy menüpontból.</td></tr>
<tr><td><code>walk</code> (bejáró bot, <code>MESTER_WALK_LEVELS=chinatown</code>)</td><td><b>PASS</b>: a bot a kezdőpontból a <code>b_door0</code>-ig sétált, az ajtót használta (E), és A Templom kezdőpontján érkezett meg (−480; −404; −1984). 201,1 játékmásodperc, 8999 egység, 0 sebzés, az egyetlen figyelmeztetés az <code>Original dialogue is undefined: Mason13</code>. A bot <b>8 rövid segített ugrást</b> (40–190 egység) tett, ha elakadt: kettőt a kezdőépületben (egy hull-elakadás és egy a katona megszólítása alatt), hármat az <code>o_postac10</code> őr mellett (1415; −947), kettőt a két automatánál (532; −1200) és egyet a kijárati helyiségben (174; −1096). Az okok (az őr teste, az automaták) a helyekből következtetettek {ev("i")}; a folyosók átjárhatóságát emberi játékossal nem próbáltuk. A bot a <code>Chiniole12</code> kérdést is megkapta (a kezdőépület katonája megszólította, nagyjából 20 játékmásodperc, amíg az időtúllépés lejárt).</td></tr>
<tr><td><code>ai</code> `patrol`</td><td>PASS: <code>o_postac5</code> (civil) a hálón 210 egységet sétált 4,9 s alatt, a fázissebesség 99%-ával.</td></tr>
<tr><td><code>ai</code> `fight` / `far`</td><td><b>Nem állapítható meg:</b> a próba öt katona mellett nem talált szabad lövésvonalú állóhelyet („No standing spot…”), a hatodiknál (<code>o_postac11</code>, vándorló őr) a saját „noticed too late” elvárásán bukott (a katona 5,15 s-nál észrevett, és <code>do_broni</code>-ba váltott, de nem lőtt). A katonák tényleges tűzharca Kiskínában tehát nem mért.</td></tr>
</tbody></table>
<h3>Mi hiányzik, mi akadályoz?</h3>
<ul>
<li><b>Nincs bejövő átmenet</b> a kampányból: egyetlen gyári szkript, DAT-ajtó vagy kampánygráf-él sem mutat a <code>worlds\\chinatown</code> világra ({ev("p")}; a <code>cshell.dll</code>, <code>Lithtech.exe</code>, <code>object.lto</code> fájlokban sincs „chinatown” szöveg). A remake menüjében <b>nincs pályaválasztó</b> (csak Új játék, Betöltés, Mentés és beállítások); a pályát a <code>Palyanezo.cmd chinatown</code> indítja közvetlenül, vagy a próbák a <code>travel.pending</code> kéréssel. Korábbi dokumentáció („a remake menüjéből betölthető”) pontatlan volt.</li>
<li>A világ belép a kampányállapotba <b>új karakterrel</b>, nem az előző pálya felszerelésével; a kampányban Kiskína nincs a <code>walk_probe</code> <code>ORDER</code> listáján; a minimális módosítás: a <code>next_level()</code> a <code>chinatown</code> után <code>chinatown2</code>-t ad (a pálya saját ajtaja ezt teszi), és a <code>tools/run_probes.py</code> új címkéje a <code>walk-chinatown</code>.</li>
<li>A <code>Mason13</code> marker nem vált ki semmit (a gyári szkriptben nincs ilyen párbeszéd); a remake ezt naplózza és nem áll le ({ev("p")}).</li>
<li>Kiskína civiljeinek sorai a gyári kulcsokban <code>empty.wav</code>-ra mutatnak, ezért hangjuk nincs; a meglévő <code>24–28.wav</code> fájlok nincsenek bekötve (gyári állapot, nem remake-hiba).</li>
<li>Hogy a gyári motor a pályát egyáltalán hibátlanul betöltötte volna, nem bizonyított ({ev("u")}).</li>
</ul>
"""


def section_other() -> str:
    return f"""
<h2 id="egyeb">Egyéb ki nem használt vagy nehezen elérhető állományok</h2>
<p>Teljes, rendszeres leltár a <b>docs/unused-content.md</b> fájlban készül (a <code>GYARI</code> összes fel nem használt fájlja); itt csak az, ami a Kiskína-kutatás közben előkerült.</p>
<table class="wide"><thead><tr><th>Állomány</th><th>Megállapítás</th><th>Szint</th></tr></thead><tbody>
<tr><td><code>nic.dat</code> (1,37 MB, v85, 2002-12-16)</td><td>Kocsma-jelenet: 13 NPC (<code>pijak</code>, <code>dziwka_kibel</code>, <code>barman</code>, <code>laska</code> és kilenc <code>karalec</code>), 14 tárgy, 7 ajtó, 63 anyag. <b>Nincs</b> <code>level worlds\\nic</code> blokk a <code>gameai.txt</code>-ben, és a kampány nem hivatkozik rá. A szereplők a <code>knajpa</code> pálya kocsmarészének szereplői (ellenségek nélkül); a neve („semmi”) teszt- vagy ideiglenes világra utal.</td><td>{ev("p")} / {ev("i")}</td></tr>
<tr><td><code>outro.dat</code> (0,28 MB, v83, 2002-05-12)</td><td>Kis színpad (két asztal, két automata, egy <code>o_cutscene</code> „outro”, 7 fény). A befejező jelenetet a játékban a <code>rh12-lab2</code> saját <code>o_cutscene</code> objektuma indítja; az <code>outro.dat</code>-ra <b>nem mutat semmi</b> (a <code>scenki.txt</code> <code>runworld</code> sorai csak <code>rh3-miasteczko1</code> és <code>rh1-wiezienie2</code>). Kiskínával azonos DAT-verzió (v83) és hónap (2002. május).</td><td>{ev("p")} / {ev("i")}</td></tr>
<tr><td><code>katscena.dat</code> (0,47 MB, <b>v70</b>, 2002-03-13)</td><td>A legrégebbi világfájl (minden más v83 vagy v85); a DAT-olvasó csak az objektumokat fejti vissza, a geometriát nem. Tartalma: 2 szereplő (<code>policjant_intro</code>, <code>bohater_intro</code>), egy <code>intro</code> <code>o_cutscene</code>, 20 sprite, 11 fény, füstkibocsátó. Az <code>intro</code> jelenetet a játék a <code>rh1-wiezienie1</code>-ben játssza; a <code>katscena</code> tehát az intro korai, külön világa lehetett.</td><td>{ev("p")} / {ev("i")}</td></tr>
<tr><td><code>sounds/enemies/china/mosb_m_old.wav</code></td><td>423 KB-os fájl a kínai katonák hangjai között (2002. július); sem a szkriptekben, sem a <code>cshell.dll</code>-ben nem szerepel a neve. A <code>Engine.REZ</code>-ben való hivatkozást nem vizsgáltuk.</td><td>{ev("p")} / {ev("u")}</td></tr>
<tr><td>Ki nem kötött hangok a Kiskína-mappában</td><td><code>07.wav</code> és <code>22.wav</code> (hivatkozatlan), <code>24–28.wav</code> (civilek, a kulcsok <code>empty.wav</code>-ra mutatnak). Lásd a fenti átiratot.</td><td>{ev("p")}</td></tr>
<tr><td>Hiányzó hivatkozás a szkriptben</td><td>A <code>Mason13</code> párbeszéd nincs definiálva; a <code>ChinioleBrama</code> / <code>Chiniole18</code> / <code>Chiniole29</code> / <code>Chiniole30</code> blokkok csak A Templomból érhetők el.</td><td>{ev("p")}</td></tr>
<tr><td>A <code>tongpo1</code> akció <code>tongpo</code> nevű szereplőre hivatkozik</td><td>Kiskínában nincs ilyen nevű szereplő; a feltétel soha nem teljesül.</td><td>{ev("p")}</td></tr>
<tr><td>Egyéb szkriptblokkok DAT nélkül</td><td><code>test2</code>, <code>pudlo</code>, <code>test</code>, <code>boks</code>: a <code>gameai.txt</code>-ben van blokkjuk, de a telepített <code>worlds</code> mappában nincs azonos nevű DAT.</td><td>{ev("p")}</td></tr>
</tbody></table>
<p>További információ: <code>docs/unused-content.md</code> (rendszeres leltár), <code>docs/campaign-research.md</code> (kampánygráf).</p>
"""


CSS = """
:root{color-scheme:dark}*{box-sizing:border-box}body{margin:0;background:#0b090a;color:#e9e0d9;font:16px/1.5 system-ui,sans-serif}
a{color:#efb79f}header{padding:3rem max(1rem,calc((100vw - 1150px)/2));background:#2a1014;border-bottom:2px solid #9b2f35}
main{max-width:1150px;margin:auto;padding:1rem}h1{font-size:clamp(2rem,5vw,3.5rem);margin:0}h2{color:#f0b49a;margin-top:2.6rem;border-bottom:1px solid #4d3638;padding-bottom:.3rem}h3{color:#e3c0ad;margin-top:1.8rem}
.lead{font-size:1.15rem;max-width:58rem}.tag{color:#ffb77d;text-transform:uppercase;letter-spacing:.16em;font-weight:700}
.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(320px,1fr));gap:1rem}figure{margin:0;background:#1b191a;border:1px solid #57444a}figure img{width:100%;display:block}figcaption{padding:.7rem 1rem;font-size:.92rem}
.box{background:#191617;border:1px solid #4d3638;padding:1rem;margin:1rem 0}nav.toc{display:flex;flex-wrap:wrap;gap:.4rem 1.2rem;margin:1rem 0}
table{border-collapse:collapse;width:100%;margin:1rem 0;font-size:.92rem}th,td{border:1px solid #3b3030;padding:.4rem .6rem;vertical-align:top;text-align:left}th{background:#231c1d}table.compare th:first-child{width:14rem}
audio{width:min(100%,300px);height:34px}.audio{display:flex;align-items:center;gap:.8rem;border-bottom:1px solid #3b3030;padding:.35rem .1rem}.audio span{min-width:5rem}
.audio-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(340px,1fr));gap:0 1.5rem}code{color:#f7bea5}small{color:#b8aaa9}li{margin:.35rem 0}ul.cols{columns:3;font-size:.9rem}
.ev{display:inline-block;padding:0 .45rem;border-radius:.6rem;font-size:.75rem;font-weight:700;text-transform:uppercase;letter-spacing:.04em}.ev-p{background:#1f4a2b;color:#b8f0c2}.ev-i{background:#4a3f1a;color:#f3de9a}.ev-u{background:#4a2020;color:#f5b5b5}
.st{font-weight:700;font-size:.8rem}.st-both{color:#9fd8ff}.st-k{color:#f3de9a}.st-t{color:#d8b2ff}.st-none{color:#ff9b9b}.warn{color:#ffb77d}
details{margin:1rem 0}summary{cursor:pointer;color:#f0b49a}
@media(max-width:800px){ul.cols{columns:1}}
"""


def build_page(media: dict[str, dict]) -> str:
    a = analysis()
    speech_dir = SOURCE / "sounds" / "speech" / "chinatown"
    barks = "".join(
        f'<div class="audio"><span>{esc(Path(f).stem.replace("china-", ""))}</span><audio controls preload="none" src="cut-content-media/{esc(f)}"></audio></div>'
        for f in sorted(media) if f.startswith("china-")
    )
    lv = a["level"]
    summary_rows = [
        ("A `chinatown.dat` külön pálya, amelyet a fő kampány nem érint.", "Semmilyen gyári szkript, DAT-ajtó vagy kampányél nem mutat rá; a `podziemia1c` kijárata közvetlenül `chinatown2`-re vezet. A DAT saját kijárata (`b_door0`) viszont `chinatown2`-re mutat.", "p"),
        ("A pálya teljes: szkript, 17 szereplő, 18 tárgy, dialógusok, zene, töltőkép.", f"{len(a['blocks'])} párbeszédblokk ({len(a['reach_k'])} elérhető Kiskínában), 45 beszédfájl, `chinatown_spokoj`, `chinatown.pcx`.", "p"),
        ("A pálya világhálója nyers: két anyag, mindkettő a szürke `blacha szara` textúra.", "A többi világban 20–183 anyag van, A Templomnál 76. A szereplők, tárgyak és kellékek viszont teljesen textúráltak, a világ lightmapelt. A régi dokumentum „szürke szoba” képe a régi remake hiányos megjelenítéséből (lightmap, köd, fények nélkül) adódott.", "p"),
        ("Kiskína régebbi fájlverzió, mint A Templom.", "`chinatown.dat`: DAT v83, 2002-05-21; `chinatown2.dat`: v85, 2002-12-23. A kampány többi pályája v85. Hogy ez a pálya „korai változat” lenne, következtetés.", "i"),
        ("A pálya szkriptje A Templomé majdnem változatlan előképe.", "19 akció szó szerint azonos; a közös jelzők csak Kiskínában vannak deklarálva. Tong Pau, a kapu és az arany macska hiányzik.", "p"),
        ("A gyári játékban valaha betölthető volt-e / játszották-e.", "Nem találtunk erre bizonyítékot, sem ellene; kódban (`cshell.dll`) nincs „chinatown” szöveg, de egy parancssori világbetöltés lehetséges lett volna.", "u"),
        ("A remake-ben betölthető és végigjátszható.", "A bejáró bot 201 s alatt eljut a `b_door0`-ig és át A Templomba (8 segített ugrással); a menüben nincs hozzá pályaválasztó.", "p"),
    ]
    summary = "".join(f"<tr><td>{esc(c).replace('`', '')}</td><td>{esc(t).replace('`', '')}</td><td>{ev(l)}</td></tr>" for c, t, l in summary_rows)
    zones = f"""
<ol>
<li><b>Kezdőépület (x 6400–7700, z −1620…−660).</b> StartPoint (7520; −1088; −1136), irány: észak. Három fegyveres katona (<code>o_postac1</code> Glock, <code>o_postac3</code> Glock, <code>o_postac2</code> HK G8), gyógycsomag (7082; −734), italok és gabonapehely, valamint a Glock és tára a déli szárnyban (6999; −1553).</li>
<li><b>Átjáró (x 5950–6400).</b> Itt áll a <code>Mason13</code> marker (5950; −1112), amelynek párbeszédét a gyári szkript nem definiálja.</li>
<li><b>Fő utca (x 2450–5900, z −1750…−460).</b> Négy civil, négy újságlap, három csirkeszerű állat, öt varjú az égen, tíz lampion, kilenc kuka, öt fali telefon; a déli oldalon kis szobák (rádió, TV, főzőedények).</li>
<li><b>Összekötő folyosó, az első zónahatár (x 1950–2450, marker x = 2333).</b> A <code>ChinioleOstrzezenie</code> marker: „Szerintem ez az út vezet a Maffia belső területére.”; ettől kezdve a katonák a <code>Chiniole10</code> figyelmeztetéssel küldenek el.</li>
<li><b>Nyugati épület (x 1000–1950, z −750…−1710).</b> Három őr (<code>o_postac9</code> Glock, <code>o_postac10</code> és <code>o_postac11</code> HK G8), 12 felvehető tárgy (italok, édesség), két automata, kanapé.</li>
<li><b>Hosszú folyosó (x 260–980) és a kijárati helyiség (x 0–260).</b> Két automata (416/480; −1160), a <code>ChinioleAtak</code> marker (128; −922: „Itt óvatosnak kell lennem, Tong Pau emberei mindenütt ott vannak.”) és a <code>b_door0</code>/<code>b_door1</code> kétszárnyú ajtó (96/160; −638) a <code>worlds\\chinatown2</code> felé.</li>
</ol>
"""
    music = f"""
<h2 id="zene">Zene</h2>
<div class="audio"><span>Nyugalom</span><audio controls preload="none" src="cut-content-media/chinatown_spokoj.mp3"></audio><small>chinatown_spokoj: Kiskína és a <code>podziemia1c</code> pálya zenéje</small></div>
<div class="audio"><span>Akció</span><audio controls preload="none" src="cut-content-media/chinatown_akcja.mp3"></audio><small>chinatown_akcja: A Templom zenéje</small></div>
<p>A gyári játék egy pályához egyetlen <code>muza</code> sávot rendel (a 78. képkockán indul), nyugodt/harci váltás <b>nincs</b>: az <code>MuzaSpok…</code> / <code>MuzaAtak…</code> jelzőket sem a szkript, sem a <code>cshell.dll</code> nem használja ({ev("p")}, docs/retail-audio.md). Kiskínában tehát végig a <code>chinatown_spokoj</code> szól, támadás közben is. A kínai katonák kiáltásai (<code>sounds/enemies/china/haltN.wav</code>, <code>deadN.wav</code>; <code>on_kontakt</code> és <code>pada</code> fázis) mindkét pályán közösek.</p>
<details><summary>A kínai katonák 16 kiáltása</summary><div class="audio-grid">{barks}</div></details>
<small>A zene az eredeti WAV konténerből újrakódolás nélkül kiemelt MP3 bitfolyam, a beszédhangok változatlan gyári WAV másolatok.</small>
"""
    body = f"""
<header><div class="tag">Gyári fájlok • kutatási archívum • {TODAY}</div><h1>Kiskína: a kihagyott pálya</h1>
<p class="lead">A <code>chinatown.dat</code> (címe „Kiskína”) a gyári telepítésben megmaradt, teljes szkripttel, 17 szereplővel, 45 beszédfájllal és saját töltőképpel, de a kampány nem vezet hozzá. A következő pálya, „A Templom” (<code>chinatown2.dat</code>), külön fájl. Ez az oldal a jelenlegi remake felvételeivel és a gyári adatokból rekonstruált tartalommal mutatja be, mi van benne.</p></header>
<main>
<nav class="toc"><a href="#osszefoglalo">Összefoglaló</a><a href="#terkep">Térkép</a><a href="#kepek">Képek</a><a href="#szereplok">Szereplők</a><a href="#szkript">Szkript</a><a href="#atirat">Átirat és hangok</a><a href="#zene">Zene</a><a href="#objektumok">Objektumok</a><a href="#templom">Összevetés</a><a href="#jatszhato">Játszhatóság</a><a href="#egyeb">Egyéb</a><a href="cut-content.md">cut-content.md</a></nav>
<h2 id="osszefoglalo">Összefoglaló</h2>
<p>Bizonyossági szintek: {ev("p")} közvetlenül a gyári fájlokból vagy egy futásból olvasható; {ev("i")} a bizonyítékokból következik, de nem közvetlen; {ev("u")} a rendelkezésre álló adatból nem dönthető el.</p>
<table class="wide"><thead><tr><th>Állítás</th><th>Amit tudunk</th><th>Szint</th></tr></thead><tbody>{summary}</tbody></table>
<div class="box"><p><b>Helyesbítés a 2026. szeptember 28-i változathoz.</b> Az akkori képek a <i>régi</i> remake-et mutatták (szürke, textúra- és lightmap nélküli terek, NPC-viselkedés nélkül, tükrözés előtt), és az a következtetés, hogy „a pálya nagy része szürke”, a megjelenítés hiányosságából is adódott. A gyári adat két része ma is áll: a világháló mindössze két anyagot használ, és mindkettő a <code>textures\\ogolne\\blacha szara.dtx</code> szürke-barna lemeztextúra ({ev("p")}; <code>chinatown.visual.materials.json</code>). Ami megváltozott: a remake azóta a gyári lightmapeket (11 950 lightmapelt lap), a gyári ködöt és fényeket, a gamma-szabályokat, a tükrözést és az NPC-viselkedést is használja, így a nyers falak a gyári baked fényekkel (meleg lámpafény, hideg köd) jelennek meg. A pálya tehát <i>nyers textúrázású, de megvilágított</i>; a szereplők, tárgyak és kellékek teljes textúrájúak. A tömör szürke szoba képe elavult.</p></div>
<h2 id="terkep">A pálya térképe</h2>
<figure><img src="cut-content-media/img/plan.png" alt="A pálya felülnézeti térképe"><figcaption>A pálya felülnézeti rajza a kinyert ütközésháló, a navigációs háló és az objektumlista alapján (nem gyári kép). A kezdőpont jobbra (kelet), a kijárat balra fent (nyugat). {ev("p")}</figcaption></figure>
{zones}
<h2 id="kepek">Képek a jelenlegi remake-ből</h2>
<p>Minden kép fejetlen, néma felvétel a jelenlegi játékkal (<code>tools/capture_kiskina.py</code>, 1280×720-ból vágva, a nézőpontok: <code>tools/kiskina_views.py</code>). A szabad kamerás nézetek a <code>MESTER_SPAWN</code> segédváltozóval készültek, nem a játékos útvonaláról. A szürke lemeztextúra ({ev("p")}) és a lightmapek együtt adják a képek hangulatát; a szereplők, lámpások és tárgyak a gyári modellek.</p>
{gallery_html()}
{section_actors(a)}
{section_script(a)}
{section_dialogs(a)}
{section_transcript(a)}
{music}
{section_objects(a)}
{section_items(a)}
{section_compare(a)}
{section_playability()}
{section_other()}
<h2>Fájlok</h2><ul>
<li><a href="../../GYARI/worlds/chinatown.dat">chinatown.dat</a> · <a href="../../GYARI/worlds/chinatown.pth">chinatown.pth</a> · <a href="../../GYARI/worlds/chinatown2.dat">chinatown2.dat</a></li>
<li><a href="../output/chinatown.scene.json">scene.json</a> · <a href="../output/chinatown.gameplay.json">gameplay.json</a> · <a href="../output/chinatown.items.json">items.json</a> · <a href="../output/chinatown.visual.obj">látható háló OBJ</a> · <a href="../output/chinatown.collision.obj">ütközésháló OBJ</a></li>
<li><a href="../../GYARI/scripts/ai/gameai.txt">gameai.txt</a> (2399–2805. sor) · <a href="../../GYARI/scripts/ai/dialogi.txt">dialogi.txt</a> (2359–2805. sor) · <a href="../../GYARI/scripts/text_keys.txt">text_keys.txt</a> (1826–2112. sor)</li>
<li><a href="cut-content-media/manifest.json">a média jegyzéke (SHA-256)</a> · <a href="cut-content.md">cut-content.md</a> · <a href="unused-content.md">unused-content.md</a></li></ul>
<p><small>Az oldal és a média a saját telepítés helyi kutatásához készült; a gyári könyvtár fájljait a készítés során nem módosítottuk. A generált média (<code>docs/cut-content-media</code>) nincs a git tárolóban; <code>python -m tools.capture_kiskina</code> majd <code>python -m tools.build_cut_content_archive</code> újra előállítja.</small></p>
</main>"""
    body = re.sub(r"`([^`\n<>]+)`", r"<code>\1</code>", body)
    return f'<!doctype html>\n<html lang="hu"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>A Mesterlövész – Kiskína, a kihagyott pálya</title><style>{CSS}</style></head><body>{body}</body></html>\n'


def main() -> None:
    media = prepare_media()
    page = build_page(media)
    (DOCS / "cut-content.html").write_text(page, encoding="utf-8")
    speech = sum(1 for e in media.values() if e["kind"] == "speech")
    pictures = sum(1 for e in media.values() if e["kind"] == "picture")
    print(f"Wrote cut-content.html: {speech} speech tracks, 2 music tracks, {pictures} pictures, {len(page) // 1024} KB")


if __name__ == "__main__":
    main()
