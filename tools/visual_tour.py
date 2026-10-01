"""Visual QA aid: spread camera stops over a level and shoot them with one headless capture run.

    python -m tools.visual_tour <world> [stops] [--run]

Stops are the StartPoint plus k-means centres of the walkable (ground) collision polygons weighted by area,
plus the highest and the lowest walkable point. The level-viewer teleports to each stop (MESTER_TOUR, tour.rs),
looks along the longest open line of sight and writes captures/<world>/<world>-<time>.png plus a montage.
Retail files are never touched; the run is hidden and silent like every capture.
"""

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parent.parent
EXE = Path(os.environ.get("MESTER_EXE") or Path(os.environ.get("CARGO_TARGET_DIR", "C:/Users/mannin/AppData/Local/mester-render-target")) / "debug" / "level-viewer.exe")


def ground_points(world: str, output: Path):
    """Centroid and area of every polygon whose normal points up (walkable), from the collision OBJ."""
    vertices, faces = [], []
    for line in (output / f"{world}.collision.obj").read_text().splitlines():
        if line.startswith("v "):
            vertices.append([float(v) for v in line.split()[1:4]])
        elif line.startswith("f "):
            faces.append([int(p.split("/")[0]) - 1 for p in line.split()[1:]])
    vertices = np.array(vertices)
    centres, areas = [], []
    for face in faces:
        a, b, c = vertices[face[0]], vertices[face[1]], vertices[face[2]]
        n = np.cross(b - a, c - a)
        length = np.linalg.norm(n)
        if length < 1e-6 or abs(n[1] / length) < 0.65:
            continue
        centres.append((a + b + c) / 3)
        areas.append(length / 2)
    return np.array(centres), np.array(areas)


def stops(world: str, count: int, output: Path):
    scene = json.loads((output / f"{world}.scene.json").read_text(encoding="utf-8"))
    start = next((o for o in scene["objects"] if o["kind"] == "StartPoint"), None)
    result = []
    if start:
        x, y, z = start["properties"]["Pos"]
        result.append((x, y, z, None))
    centres, areas = ground_points(world, output)
    if len(centres) == 0:
        return result
    rng = np.random.default_rng(7)
    k = min(count, len(centres))
    pick = centres[rng.choice(len(centres), k, replace=False, p=areas / areas.sum())]
    for _ in range(12):
        distance = ((centres[:, None, :] - pick[None]) ** 2).sum(-1)
        label = distance.argmin(1)
        for i in range(k):
            member = label == i
            if member.any():
                pick[i] = np.average(centres[member], axis=0, weights=areas[member])
    for centre in pick:
        near = ((centres[:, None, [0, 2]] - centre[None, [0, 2]]) ** 2).sum(-1).ravel()
        # snap to a real walkable polygon of similar height
        candidates = np.where(np.abs(centres[:, 1] - centre[1]) < 200)[0]
        index = candidates[near[candidates].argmin()] if len(candidates) else near.argmin()
        x, y, z = centres[index]
        result.append((x, y + 58.1, z, None))
    top, low = centres[centres[:, 1].argmax()], centres[centres[:, 1].argmin()]
    result.extend([(top[0], top[1] + 58.1, top[2], None), (low[0], low[1] + 58.1, low[2], None)])
    return result


def object_stops(world: str, kinds, distance: float, limit: int, output: Path, spread: float = 400.0):
    """`t:` orbit stops (look at the object from `distance` units) for scene objects of these kinds, spread over the level."""
    scene = json.loads((output / f"{world}.scene.json").read_text(encoding="utf-8"))
    picked = []
    for o in scene["objects"]:
        if o["kind"] in kinds or any(k == o["properties"].get("Name", "") for k in kinds):
            position = o["properties"].get("Pos")
            if position and all(sum((a - b) ** 2 for a, b in zip(position, q)) > spread ** 2 for q in picked):
                picked.append(position)
    return [("t", x, y, z, distance) for x, y, z in picked[:limit]]


def texture_stops(world: str, needle: str, distance: float, limit: int, output: Path, spread: float = 500.0, lift: float = 30.0):
    """`t:` orbit stops at surfaces whose texture name contains `needle` (e.g. woda, niebo, kratka)."""
    materials = json.loads((output / f"{world}.visual.materials.json").read_text(encoding="utf-8"))
    wanted = {k for k, v in materials.items() if needle in v["source_texture"].lower()}
    vertices, current, points = [], None, []
    for line in (output / f"{world}.visual.obj").read_text().splitlines():
        if line.startswith("v "):
            vertices.append([float(c) for c in line.split()[1:4]])
        elif line.startswith("usemtl"):
            current = line.split()[1]
        elif line.startswith("f ") and current in wanted:
            idx = [int(p.split("/")[0]) - 1 for p in line.split()[1:4]]
            points.append([sum(vertices[i][k] for i in idx) / 3 for k in range(3)])
    picked = []
    for p in points:
        if all(sum((a - b) ** 2 for a, b in zip(p, q)) > spread ** 2 for q in picked):
            picked.append(p)
    return [("t", x, y + lift, z, distance) for x, y, z in picked[:limit]]


def tour_argument(points):
    return ";".join(f"t:{p[1]:.0f},{p[2]:.0f},{p[3]:.0f},{p[4]:.0f}" if p[0] == "t" else
                    f"{p[0]:.0f},{p[1]:.0f},{p[2]:.0f}" + (f",{p[3]:.3f}" if p[3] is not None else "") for p in points)


def montage(images, target: Path, columns=2, size=(640, 360)):
    from PIL import Image
    rows = (len(images) + columns - 1) // columns
    sheet = Image.new("RGB", (columns * size[0], rows * size[1]))
    for i, path in enumerate(images):
        sheet.paste(Image.open(path).convert("RGB").resize(size), ((i % columns) * size[0], (i // columns) * size[1]))
    sheet.save(target)


def run(world: str, count: int, output: Path, out_dir: Path, points=None, tag=""):
    points = points or stops(world, count, output)
    if len(points) == 1:
        points = points * 2
    times = [4.0 + 2.0 * i for i in range(len(points))]
    out_dir.mkdir(parents=True, exist_ok=True)
    for old in out_dir.glob(f"{world}{tag}-*.png"):
        old.unlink()
    environment = dict(os.environ, MESTER_SILENT="1", MESTER_TOUR=tour_argument(points))
    target = out_dir / f"{world}{tag}.png"
    with open(out_dir / f"{world}{tag}.log", "wb") as log:
        subprocess.run([str(EXE), world, str(output), str(target), ",".join(f"{t:g}" for t in times)],
                       cwd=ROOT / "crates" / "level-viewer", env=environment, timeout=240,
                       stdout=log, stderr=subprocess.STDOUT)
    shots = sorted(out_dir.glob(f"{world}{tag}-0*.png"))
    (out_dir / f"{world}{tag}.stops.txt").write_text("\n".join(f"{i}: {p}" for i, p in enumerate(points)) + "\n")
    return points, shots


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("world")
    parser.add_argument("count", nargs="?", type=int, default=6)
    parser.add_argument("--run", action="store_true")
    parser.add_argument("--objects", help="comma separated object kinds or names: look at those instead of the k-means stops")
    parser.add_argument("--texture", help="look at surfaces whose texture name contains this")
    parser.add_argument("--dist", type=float, default=220.0)
    parser.add_argument("--spread", type=float, default=400.0, help="minimum distance between two looked-at objects")
    parser.add_argument("--tag", default="")
    parser.add_argument("--output", type=Path, default=ROOT / "output")
    parser.add_argument("--out", type=Path, default=ROOT / "captures")
    args = parser.parse_args()
    given = object_stops(args.world, args.objects.split(","), args.dist, args.count, args.output, args.spread) if args.objects else None
    if args.texture:
        given = texture_stops(args.world, args.texture.lower(), args.dist, args.count, args.output, args.spread)
    if not args.run:
        print(tour_argument(given or stops(args.world, args.count, args.output)))
        return
    points, shots = run(args.world, args.count, args.output, args.out / args.world, given, args.tag)
    for i in range(0, len(shots), 4):
        montage(shots[i:i + 4], args.out / args.world / f"{args.world}{args.tag}-sheet{i // 4}.png")
    print(f"{args.world}: {len(points)} stops, {len(shots)} captures", file=sys.stderr)


if __name__ == "__main__":
    main()
