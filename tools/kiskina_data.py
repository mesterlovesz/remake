"""Kiskina (chinatown.dat) data for the cut-content archive: level summary from the exported JSON and a top-down plan image.

Everything is read from output/ (exported from the retail DAT) and GYARI (read-only retail install). No retail file is copied into git:
the plan image goes to docs/cut-content-media (gitignored).
"""
from __future__ import annotations

import json
import re
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "output"
SOURCE = ROOT.parent / "GYARI"
B = chr(92)


def load(world: str = "chinatown") -> dict:
    scene = json.loads((OUTPUT / f"{world}.scene.json").read_text(encoding="utf-8"))
    game = json.loads((OUTPUT / f"{world}.gameplay.json").read_text(encoding="utf-8"))
    return {"scene": scene, "game": game, "objects": scene["objects"]}


def kind(level: dict, name: str) -> list[dict]:
    return [o for o in level["objects"] if o["kind"] == name]


def model_name(obj: dict) -> str:
    return obj["properties"].get("Model", "").split(B)[-1]


def plan(level: dict, target: Path, scale: float = 0.25, world: str = "chinatown") -> None:
    """Top-down plan: collision triangles shaded by height (roofs light), navigation nodes, then every placed object with a legend."""
    verts: list[tuple[float, float, float]] = []
    faces: list[tuple[int, ...]] = []
    for line in (OUTPUT / f"{world}.collision.obj").read_text().splitlines():
        if line.startswith("v "):
            _, x, y, z = line.split()
            verts.append((float(x), float(y), float(z)))
        elif line.startswith("f "):
            faces.append(tuple(int(t.split("/")[0]) - 1 for t in line.split()[1:]))
    xs = [v[0] for v in verts]
    zs = [v[2] for v in verts]
    x0, x1, z0, z1 = min(xs), max(xs), min(zs), max(zs)
    pad = 40
    width, height = int((x1 - x0) * scale) + 2 * pad, int((z1 - z0) * scale) + 2 * pad + 60

    def px(x: float, z: float) -> tuple[float, float]:  # +X right, +Z up (north), like the level editor
        return pad + (x - x0) * scale, pad + (z1 - z) * scale

    image = Image.new("RGB", (width, height), (16, 14, 15))
    draw = ImageDraw.Draw(image)
    try:
        draw.font = ImageFont.truetype("C:/Windows/Fonts/arial.ttf", 13)
    except OSError:
        pass
    tri = []
    for f in faces:
        ys = [verts[i][1] for i in f]
        tri.append((sum(ys) / len(ys), f))
    tri = [t for t in tri if t[0] < -150]  # the flat cap of the world box at y ~ 0 would hide everything
    tri.sort(key=lambda t: t[0])  # low first: roofs are painted over the street
    for ymean, f in tri:
        shade = int(max(0.0, min(1.0, (-1150.0 - ymean) / -1150.0)) * 150) + 55  # y -1150 street .. 0 top
        draw.polygon([px(verts[i][0], verts[i][2]) for i in f], fill=(shade, shade - 6, shade - 12))
    for n in level["game"]["navigation"]["nodes"]:
        x, _, z = n["pos"]
        cx, cy = px(x, z)
        draw.ellipse((cx - 1.2, cy - 1.2, cx + 1.2, cy + 1.2), fill=(95, 170, 190))
    colors = {"o_postac": (255, 90, 70), "o_item_podnoszony": (250, 215, 70), "o_marker_dialog": (255, 120, 255), "b_door": (90, 255, 120), "StartPoint": (255, 255, 255)}
    for o in level["objects"]:
        c = colors.get(o["kind"])
        pos = o["properties"].get("Pos")
        if o["kind"] == "o_obiekt" and model_name(o).startswith("gawron"):
            c = (120, 160, 255)
        if not c or not pos:
            continue
        cx, cy = px(pos[0], pos[2])
        r = 6 if o["kind"] in ("StartPoint", "b_door", "o_marker_dialog") else 4
        draw.rectangle((cx - r, cy - r, cx + r, cy + r), outline=c, width=2) if o["kind"] in ("o_marker_dialog", "b_door", "StartPoint") else draw.ellipse((cx - r, cy - r, cx + r, cy + r), fill=c)
        if o["kind"] == "o_postac":
            draw.text((cx + 6, cy - 14), o["properties"]["Name"].replace("o_postac", "#"), fill=c)
        if o["kind"] == "o_marker_dialog":
            draw.text((cx + 8, cy - 14), o["properties"]["Dialog"], fill=c)
    legend = [((255, 90, 70), "NPC (o_postac, szám = az objektum száma)"), ((250, 215, 70), "felvehető tárgy"), ((255, 120, 255), "párbeszédmarker"), ((90, 255, 120), "ajtó / kijárat"), ((255, 255, 255), "kezdőpont"), ((120, 160, 255), "gawron (varjú) pályaközéppont"), ((95, 170, 190), "navigációs csomópont")]
    lx = pad
    for c, text in legend:
        draw.rectangle((lx, height - 40, lx + 10, height - 30), fill=c)
        draw.text((lx + 14, height - 42), text, fill=(230, 225, 220))
        lx += 14 + int(draw.textlength(text)) + 18
    draw.text((pad, height - 22), "Felülnézet, +X jobbra (kelet), +Z felfelé (észak); a világosabb felület magasabb. Forrás: chinatown.dat ütközésháló, navigációs háló és objektumlista.", fill=(180, 170, 165))
    image.save(target)


# --- retail text / script parsing ------------------------------------------------------------------------------------------------------

def read_cp1250(path: Path) -> list[str]:
    return path.read_bytes().decode("cp1250", errors="replace").splitlines()


def text_keys() -> dict[str, tuple[str, int]]:
    """text key -> (text, line number in scripts/text_keys.txt)."""
    keys: dict[str, tuple[str, int]] = {}
    for number, line in enumerate(read_cp1250(SOURCE / "scripts" / "text_keys.txt"), 1):
        if line.startswith(">"):
            parts = line.split(None, 1)
            keys[parts[0]] = (parts[1].strip() if len(parts) > 1 else "", number)
    return keys


def dialog_blocks() -> dict[str, dict]:
    """Chiniole* / Chinioleostrzezenie blocks of scripts/ai/dialogi.txt: name -> {line, commands[(cmd,arg)]}."""
    blocks: dict[str, dict] = {}
    current = None
    for number, line in enumerate(read_cp1250(SOURCE / "scripts" / "ai" / "dialogi.txt"), 1):
        stripped = line.strip()
        if stripped.startswith("dialog "):
            name = stripped.split(None, 1)[1]
            current = blocks.setdefault(name, {"line": number, "commands": []}) if name.lower().startswith("chiniol") else None
        elif current is not None and stripped and not stripped.startswith("//"):
            cmd = stripped.split(None, 1)
            current["commands"].append((cmd[0], cmd[1] if len(cmd) > 1 else ""))
    return blocks


def level_script(world: str = "chinatown") -> tuple[list[str], list[dict]]:
    """(header lines, actions) of the Kiskina block of scripts/ai/gameai.txt (`level worlds<backslash>chinatown`). An action is {name, line, body[lines]}."""
    lines = read_cp1250(SOURCE / "scripts" / "ai" / "gameai.txt")
    start = next(i for i, l in enumerate(lines) if l.strip().lower() == "level worlds" + B + world)
    end = next(i for i in range(start + 1, len(lines)) if lines[i].strip().lower().startswith("level "))
    header: list[str] = []
    actions: list[dict] = []
    current = None
    for i in range(start, end):
        stripped = lines[i].strip()
        if stripped.lower().startswith("action "):
            current = {"name": stripped.split(None, 1)[1], "line": i + 1, "body": [], "comment": ""}
            actions.append(current)
        elif current is not None:
            if stripped.startswith("//"):
                current["trailing_comment"] = stripped  # belongs to the next action in the source; kept out of the body
            elif stripped and stripped.lower() != "endifs":
                current["body"].append(stripped)
        elif stripped and not stripped.startswith("//"):
            header.append(stripped)
    return header, actions


def wav_seconds(path: Path) -> float:
    import wave
    with wave.open(str(path)) as w:
        return w.getnframes() / w.getframerate()


kiskina_script = level_script


def scene_markers(world: str) -> list[tuple[str, str]]:
    """(marker name, dialog) of the o_marker_dialog objects of a level."""
    scene = json.loads((OUTPUT / f"{world}.scene.json").read_text(encoding="utf-8"))
    return [(o["properties"]["Name"], o["properties"]["Dialog"]) for o in scene["objects"] if o["kind"] == "o_marker_dialog"]


def dialog_index(blocks: dict[str, dict]) -> dict[str, dict]:
    """name -> {sets, unsets, choices{n: next}, timeout_next}: what a dialogue block changes and where it leads."""
    out = {}
    for name, block in blocks.items():
        info = {"sets": [], "unsets": [], "next": [], "speakers": ""}
        for cmd, arg in block["commands"]:
            if cmd == "set":
                info["sets"].append(arg)
            elif cmd == "unset":
                info["unsets"].append(arg)
            elif re.fullmatch(r"onchoice\d+dialog", cmd):
                info["next"].append(arg)
            elif cmd in ("ontimeexceeded", "onfurtherthan") and len(arg.split()) > 1:
                info["next"].append(arg.split()[1])
            elif cmd == "person":
                info["speakers"] = arg
        out[name] = info
    return out


def reachable_dialogs(world: str, blocks: dict[str, dict]) -> tuple[set[str], dict[str, list[str]]]:
    """Fixpoint over the level script: which dialogue blocks can ever start, and what starts each (marker / action names).

    A dialogue starts from an o_marker_dialog of the level, from an action of the level's gameai block, or as the answer/timeout target of a
    reached block. An action needs every positive `if FLAG` precondition to be settable: set by a reached dialogue, by a character phase (the
    `Cywil*Nuda` flags) or not set by any dialogue at all (engine counters, `CzasOdGadki`). Other conditions (distance, sight) are assumed satisfiable."""
    _, actions = level_script(world)
    index = dialog_index(blocks)
    settable_by_dialog = {f for info in index.values() for f in info["sets"]}
    phase_flags = {"Cywil1Nuda", "Cywil2Nuda", "Cywil3Nuda", "Cywil4Nuda"}
    starters: dict[str, list[str]] = {}
    reached: set[str] = set()
    flags: set[str] = set()

    def add(dialog: str, who: str) -> None:
        starters.setdefault(dialog, [])
        if who not in starters[dialog]:
            starters[dialog].append(who)
        reached.add(dialog)

    for marker, dialog in scene_markers(world):
        if dialog in blocks:
            add(dialog, f"marker {marker}")
    changed = True
    while changed:
        changed = False
        before = (len(reached), len(flags))
        for dialog in list(reached):
            flags.update(index[dialog]["sets"])
            for nxt in index[dialog]["next"]:
                if nxt in blocks and nxt not in reached:
                    add(nxt, f"válasz/időtúllépés: {dialog}")
        for act in actions:
            target = next((l.split(None, 1)[1] for l in act["body"] if l.lower().startswith("dialog ")), None)
            if not target or target not in blocks:
                continue
            needs = [l.split(None, 1)[1] for l in act["body"] if re.match(r"if\s+\S+$", l, re.I)]
            if all(f in flags or f in phase_flags or f not in settable_by_dialog for f in needs):
                if target not in reached:
                    changed = True
                add(target, f"akció {act['name']}")
        changed = changed or before != (len(reached), len(flags))
    return reached, starters
