import sys
import unittest
import json
import tempfile
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from tools.lithtech_dat import read_world, World, Model, Polygon, WorldObject
from tools.export_world import export_world


class RetailWorldTests(unittest.TestCase):
    def test_first_prison_world_geometry_and_spawn(self):
        world = read_world(ROOT / "GYARI/worlds/rh1-wiezienie1.dat")
        self.assertEqual(world.version, 85)
        self.assertEqual(len(world.models), 52)
        self.assertEqual(len(world.objects), 327)
        physics = next(model for model in world.models if model.name == "PhysicsBSP")
        self.assertEqual(len(physics.polygons), 7649)
        start = next(obj for obj in world.objects if obj.kind == "StartPoint")
        self.assertEqual(start.properties["Pos"], (4028.0, 125.0, 698.0))

    def test_bleeding_bus_authored_placement(self):
        world = read_world(ROOT / "GYARI/worlds/rh3-miasteczko1.dat")
        buses = [obj for obj in world.objects if obj.properties.get("Model", "").lower().endswith("autobus.ltb")]
        self.assertEqual(len(buses), 1)
        self.assertEqual(buses[0].properties["Pos"], (1600.0, -260.0, 1384.162109375))

    def test_exported_geometry_and_entities_keep_retail_data(self):
        source = ROOT / "GYARI/worlds/rh3-miasteczko1.dat"
        world = read_world(source)
        physics = next(model for model in world.models if model.name == "PhysicsBSP")
        with tempfile.TemporaryDirectory() as directory:
            obj_path, scene_path = export_world(source, Path(directory))
            faces = sum(line.startswith("f ") for line in obj_path.read_text().splitlines())
            colliders = [poly for poly in physics.polygons if physics.surfaces[poly.surface] & 0x20001]
            self.assertEqual(faces, sum(len(poly.vertices) - 2 for poly in colliders))
            scene = json.loads(scene_path.read_text(encoding="utf-8"))
            bus = next(obj for obj in scene["objects"] if str(obj["properties"].get("Model", "")).lower().endswith("autobus.ltb"))
            self.assertEqual(bus["properties"]["Pos"], [1600.0, -260.0, 1384.162109375])
            self.assertEqual(len(scene["collision_polygons"]), len(colliders))

    def test_movable_world_models_are_separate_and_linked_to_entities(self):
        source = ROOT / "GYARI/worlds/rh1-wiezienie1.dat"
        world = read_world(source)
        with tempfile.TemporaryDirectory() as directory:
            _, scene_path = export_world(source, Path(directory))
            scene = json.loads(scene_path.read_text(encoding="utf-8"))
            movable = Path(directory) / scene["movable_collision_obj"]
            self.assertTrue(movable.is_file())
            models = scene["movable_world_models"]
            self.assertEqual(len(models), 50)
            self.assertEqual(
                sum(line.startswith("f ") for line in movable.read_text().splitlines()),
                sum(len(poly.vertices) - 2 for model in world.models if model.flags == 2 for poly in model.polygons
                    if model.surfaces[poly.surface] & 0x20001),
            )
            door = next(model for model in models if model["name"] == "b_door25")
            self.assertEqual(door["flags"], 2)
            self.assertEqual(len(door["object_indices"]), 1)
            self.assertEqual(scene["objects"][door["object_indices"][0]]["kind"], "b_door")

    def test_surface_flags_filter_collision_without_losing_face_or_source_indices(self):
        points = ((0., 0., 0.), (1., 0., 0.), (1., 0., 1.), (0., 0., 1.))
        # Visibility-only, invisible solid, invisible physics blocker, plain
        # non-solid, and a solid visibility blocker. Invisible does not mean passable.
        flags = (0x200104, 0x105, 0x20104, 0x100, 0x200105)
        polygons = tuple(Polygon(i, 0, vertices) for i, vertices in enumerate(
            ((0, 1, 2, 3), (0, 1, 2, 3), (0, 1, 2), (0, 2, 3), (0, 2, 3))))
        static = Model("PhysicsBSP", 20, points, ((0., 1., 0., 0.),), flags, polygons)
        movable = Model("door", 2, points, static.planes, flags, polygons)
        world = World(85, (static, movable), (WorldObject("b_door", {"Name": "door"}),))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "fixture.dat"
            source.write_bytes(b"source bytes remain unchanged")
            with patch("tools.export_world.read_world", return_value=world):
                obj, scene_path = export_world(source, root)
            scene = json.loads(scene_path.read_text(encoding="utf-8"))
            faces = [line for line in obj.read_text().splitlines() if line.startswith("f ")]
            self.assertEqual(faces, ["f 1 2 3", "f 1 3 4", "f 1 2 3", "f 1 3 4"])
            metadata = scene["collision_polygons"]
            self.assertEqual([p["source_polygon_index"] for p in metadata], [1, 2, 4])
            self.assertEqual([p["first_face"] for p in metadata], [0, 2, 3])
            self.assertEqual([p["face_count"] for p in metadata], [2, 1, 1])
            self.assertEqual([p["surface_flags"] for p in metadata], [0x105, 0x20104, 0x200105])
            self.assertEqual(scene["collision_faces"], 4)
            self.assertEqual(scene["collision_excluded_polygons"], 2)
            movable_faces = [line for line in (root / scene["movable_collision_obj"]).read_text().splitlines() if line.startswith("f ")]
            self.assertEqual(movable_faces, faces)
            self.assertEqual(scene["movable_world_models"][0]["face_count"], 4)
            self.assertEqual(scene["movable_world_models"][0]["object_indices"], [0])

    def test_collision_surface_texture_flags_follow_obj_face_order(self):
        points = ((0., 0., 0.), (1., 0., 0.), (1., 0., 1.), (0., 0., 1.))
        flags = (0x105, 0x100, 0x105)
        polygons = (Polygon(0, 0, (0, 1, 2, 3)), Polygon(1, 0, (0, 1, 2)), Polygon(2, 0, (0, 2, 3)))
        static = Model("PhysicsBSP", 20, points, ((0., 1., 0., 0.),), flags, polygons, (6, 1, 3))
        door = Model("b_door", 2, points, static.planes, flags, polygons, (2, 2, 3))
        world = World(85, (static, door), ())
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "fixture.dat"
            source.write_bytes(b"source")
            with patch("tools.export_world.read_world", return_value=world):
                obj, _ = export_world(source, root)
            surfaces = json.loads((root / "fixture.collision.surfaces.json").read_text(encoding="utf-8"))
            faces = [line for line in obj.read_text().splitlines() if line.startswith("f ")]
            self.assertEqual(len(surfaces["faces"]), len(faces))
            self.assertEqual(surfaces["faces"], [6, 6, 3])
            self.assertEqual(surfaces["world_models"], {"b_door": 2})


if __name__ == "__main__":
    unittest.main()
