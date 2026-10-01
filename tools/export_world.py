"""Export a retail LithTech world to portable OBJ geometry and JSON metadata."""

import argparse
from hashlib import sha256
import json
from pathlib import Path

from .lithtech_dat import read_world


# LithTech surface flags (research/blender-lithtech-dat-import/test.h).
# Visibility blockers may be invisible without blocking player movement.
SURF_SOLID = 1 << 0
SURF_PHYSICSBLOCKER = 1 << 17


def _blocks_physics(flags: int) -> bool:
    return bool(flags & (SURF_SOLID | SURF_PHYSICSBLOCKER))


def _surface_kind(normal_y: float) -> str:
    if normal_y >= 0.65:
        return "ground"
    if normal_y <= -0.65:
        return "ceiling"
    return "wall"


def _physics_model(world):
    physics = [model for model in world.models if model.name == "PhysicsBSP" and model.flags == 20]
    if len(physics) != 1:
        raise ValueError(f"expected one static PhysicsBSP, found {len(physics)}")
    return physics[0]


def _texture_flags(model, polygon) -> int:
    flags = model.texture_flags
    return flags[polygon.surface] if polygon.surface < len(flags) else 0


def export_collision_surfaces(source: Path, output: Path, world=None) -> Path:
    """One texture-flags value per collision.obj face, in face order.

    cshell's bullet trace (0x10005ce0) picks debris and holes from
    IntersectInfo.m_SurfaceFlags, which the engine fills from the hit
    polygon's surface texture flags. World models keep their dominant value.
    """
    source = Path(source)
    output = Path(output)
    world = world or read_world(source)
    physics = _physics_model(world)
    faces = []
    for polygon in physics.polygons:
        if not _blocks_physics(physics.surfaces[polygon.surface]):
            continue
        faces.extend([_texture_flags(physics, polygon)] * (len(polygon.vertices) - 2))
    models = {}
    for model in world.models:
        if model is physics or not model.polygons:
            continue
        counts = {}
        for polygon in model.polygons:
            value = _texture_flags(model, polygon)
            counts[value] = counts.get(value, 0) + 1
        models[model.name] = max(sorted(counts), key=counts.get)
    path = output / f"{source.stem}.collision.surfaces.json"
    path.write_text(json.dumps({
        "format": "mesterlovesz-collision-surfaces-v1",
        "source": "DAT surface texture flags (IntersectInfo.m_SurfaceFlags)",
        "collision_obj": f"{source.stem}.collision.obj",
        "faces": faces,
        "world_models": models,
    }, ensure_ascii=False, separators=(",", ":")) + "\n", encoding="utf-8", newline="\n")
    return path


def export_world(source: Path, output: Path) -> tuple[Path, Path]:
    source = Path(source)
    output = Path(output)
    world = read_world(source)
    output.mkdir(parents=True, exist_ok=True)
    if world.version == 70:
        # Objects only (see lithtech_dat.read_world): no collision and no render geometry exist for this world.
        scene_path = output / f"{source.stem}.scene.json"
        scene = {"format": "mesterlovesz-scene-v1", "source_world": source.name, "source_sha256": sha256(source.read_bytes()).hexdigest(),
                 "dat_version": 70, "geometry": "not decoded (DAT v70): objects only",
                 "coordinate_system": "LithTech: right-handed, Y up, original units",
                 "objects": [{"kind": obj.kind, "properties": obj.properties} for obj in world.objects]}
        scene_path.write_text(json.dumps(scene, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        return None, scene_path
    physics = _physics_model(world)
    obj_path = output / f"{source.stem}.collision.obj"
    scene_path = output / f"{source.stem}.scene.json"
    lines = ["# LithTech native coordinates, Y up, 1 OBJ unit = 1 retail unit", "o PhysicsBSP"]
    for x, y, z in physics.points:
        lines.append(f"v {x:.9g} {y:.9g} {z:.9g}")
    polygons = []
    face = 0
    current_kind = None
    for source_index, polygon in enumerate(physics.polygons):
        surface_flags = physics.surfaces[polygon.surface]
        if not _blocks_physics(surface_flags):
            continue
        normal = physics.planes[polygon.plane][:3]
        kind = _surface_kind(normal[1])
        if kind != current_kind:
            lines.append(f"g {kind}")
            current_kind = kind
        for index in range(1, len(polygon.vertices) - 1):
            a, b, c = polygon.vertices[0], polygon.vertices[index], polygon.vertices[index + 1]
            lines.append(f"f {a + 1} {b + 1} {c + 1}")
        count = len(polygon.vertices) - 2
        polygons.append({
            "source_polygon_index": source_index,
            "first_face": face,
            "face_count": count,
            "normal": normal,
            "kind": kind,
            "surface_flags": surface_flags,
        })
        face += count
    obj_path.write_text("\n".join(lines) + "\n", encoding="ascii")
    movable_path = output / f"{source.stem}.movable.collision.obj"
    movable_lines = ["# Authored initial world coordinates; one object per movable world model"]
    movable_models = []
    vertex_base = 0
    movable_face = 0
    for model in world.models:
        if model.flags != 2:
            continue
        movable_lines.append(f"o {model.name}")
        for x, y, z in model.points:
            movable_lines.append(f"v {x:.9g} {y:.9g} {z:.9g}")
        first_face = movable_face
        for polygon in model.polygons:
            if not _blocks_physics(model.surfaces[polygon.surface]):
                continue
            for index in range(1, len(polygon.vertices) - 1):
                a, b, c = polygon.vertices[0], polygon.vertices[index], polygon.vertices[index + 1]
                movable_lines.append(f"f {a + vertex_base + 1} {b + vertex_base + 1} {c + vertex_base + 1}")
                movable_face += 1
        movable_models.append({
            "name": model.name,
            "flags": model.flags,
            "first_face": first_face,
            "face_count": movable_face - first_face,
            "object_indices": [
                index for index, obj in enumerate(world.objects)
                if str(obj.properties.get("Name", "")).casefold() == model.name.casefold()
            ],
        })
        vertex_base += len(model.points)
    movable_path.write_text("\n".join(movable_lines) + "\n", encoding="ascii")
    scene = {
        "format": "mesterlovesz-scene-v1",
        "source_world": source.name,
        "source_sha256": sha256(source.read_bytes()).hexdigest(),
        "coordinate_system": "LithTech: right-handed, Y up, original units",
        "collision_obj": obj_path.name,
        "collision_points": len(physics.points),
        "collision_faces": face,
        "collision_source_polygons": len(physics.polygons),
        "collision_excluded_polygons": len(physics.polygons) - len(polygons),
        "collision_polygons": polygons,
        "movable_collision_obj": movable_path.name,
        "movable_world_models": movable_models,
        "objects": [{"kind": obj.kind, "properties": obj.properties} for obj in world.objects],
    }
    scene_path.write_text(json.dumps(scene, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    export_collision_surfaces(source, output, world)
    return obj_path, scene_path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("world", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--surfaces-only", action="store_true",
                        help="only (re)write <world>.collision.surfaces.json")
    args = parser.parse_args()
    if args.surfaces_only:
        print(export_collision_surfaces(args.world, args.output))
        return
    obj, scene = export_world(args.world, args.output)
    print(obj)
    print(scene)


if __name__ == "__main__":
    main()
