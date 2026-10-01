"""Regression for the shipped, unlinked v83 Kiskína map, not Chinatown2."""
import unittest
from pathlib import Path
from tools.lithtech_dat import read_world
from tools.render_dat import read_render_nodes, read_model_render_nodes

SOURCE = Path(__file__).resolve().parents[2] / 'GYARI/worlds/chinatown.dat'


class LegacyChinatownTests(unittest.TestCase):
    def test_complete_authored_geometry_and_objects(self):
        world = read_world(SOURCE)
        self.assertEqual(world.version, 83)
        self.assertEqual(len(world.objects), 310)
        self.assertEqual(len(world.models), 13)
        physics = next(m for m in world.models if m.name == 'PhysicsBSP')
        self.assertEqual((len(physics.points), len(physics.polygons)), (10057, 6626))
        visibility = next(m for m in world.models if m.name == 'VisBSP')
        self.assertEqual(len(visibility.polygons), 6)
        self.assertEqual(visibility.points[0], (2464.0, 0.0, -1072.0))
        nodes = read_render_nodes(SOURCE)
        self.assertEqual(len(nodes), 33)
        self.assertEqual(sum(len(n.triangles) for n in nodes), 25134)
        models = read_model_render_nodes(SOURCE)
        self.assertEqual(set(models), {m.name for m in world.models if m.flags == 2})


if __name__ == '__main__':
    unittest.main()
