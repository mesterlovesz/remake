"""Headless captures of the retail level Kiskina (chinatown.dat) in the current remake, for docs/cut-content.html.

Runs the ready level-viewer exe one hidden, silent instance at a time (never two at once; COMMON.md rules): MESTER_SILENT=1, capture arguments,
scratch MESTER_USER_DIR so the quick-save the hidden run makes does not touch the owner's saves. Usage:
  python -m tools.capture_kiskina [name ...]      (default: every view)       --list prints the views
Environment: LEVEL_VIEWER_EXE overrides the exe path. Output: docs/cut-content-media/shots/<name>.png (gitignored, retail-derived).
"""
from __future__ import annotations

import math
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXE = Path(os.environ.get("LEVEL_VIEWER_EXE", "C:/Users/mannin/AppData/Local/mester-fix-target/debug/level-viewer.exe"))
SHOTS = ROOT / "docs" / "cut-content-media" / "shots"


def look(eye: tuple[float, float, float], target: tuple[float, float]) -> tuple[float, float, float, float]:
    """MESTER_SPAWN value from an eye position and a point on the ground plane (yaw convention of walk_probe.rs face_heading)."""
    yaw = math.atan2(-(target[0] - eye[0]), -(target[1] - eye[2]))
    return (eye[0], eye[1], eye[2], yaw)


# name -> (spawn x,y,z,yaw native units/radians, game seconds before the picture, caption id for the HTML)
VIEWS: dict[str, tuple[tuple[float, float, float, float] | None, float, dict[str, str]]] = {}


def view(name: str, eye: tuple[float, float, float] | None, target: tuple[float, float] | float | None = None, seconds: float = 4.0, world: str = "chinatown", **env: str) -> None:
    """eye None = the retail StartPoint pose (no MESTER_SPAWN); target = ground point to face, or a raw yaw in radians; env = extra variables."""
    VIEWS[name] = (None if eye is None else look(eye, target) if isinstance(target, tuple) else (*eye, float(target)), seconds, {"_world": world, **env})


def capture(name: str) -> bool:
    spawn, seconds, extra = VIEWS[name]
    extra = dict(extra)
    SHOTS.mkdir(parents=True, exist_ok=True)
    target = SHOTS / f"{name}.png"
    user = Path(tempfile.mkdtemp(prefix="kiskina-user-"))
    (user / "autoexec.cfg").write_text('"CAutoQuickSave" "0"\r\n')  # no "Gyorsmentés létrehozva." banner over the picture
    env = dict(os.environ, MESTER_SILENT="1", MESTER_WINDOW="1280x720", MESTER_USER_DIR=str(user))
    env.pop("MESTER_SPAWN", None)
    env.pop("MESTER_TEST_SCENARIO", None)
    world = extra.pop("_world", "chinatown")
    env.update(extra)  # e.g. MESTER_TEST_SCENARIO=dialogue for the dialogue panel pictures
    if spawn:
        env["MESTER_SPAWN"] = ",".join(f"{v:.4f}" for v in spawn)
    done = subprocess.run([str(EXE), world, "../../output", str(target), str(seconds)], cwd=ROOT / "crates" / "level-viewer", env=env, timeout=120, capture_output=True)
    ok = done.returncode == 0 and target.exists()
    if ok:  # the remake's "Kattints a játékhoz" help line (the hidden window never grabs the mouse) is not part of the game picture
        from PIL import Image
        Image.open(target).convert("RGB").crop((0, 0, 1280, 640)).save(target)
    print(("ok   " if ok else "FAIL ") + name, flush=True)
    return ok


def main(argv: list[str]) -> int:
    from tools.kiskina_views import register  # the view table lives next to the captions
    register(view)
    if "--list" in argv:
        for name, (spawn, seconds, _) in VIEWS.items():
            print(name, spawn and [round(v, 2) for v in spawn], seconds)
        return 0
    names = [a for a in argv if not a.startswith("-")] or list(VIEWS)
    return 0 if all(capture(n) for n in names) else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
