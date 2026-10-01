import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from tools.render_dat import read_render_nodes,read_model_render_nodes


class RenderDataTests(unittest.TestCase):
    def test_authored_doors_and_grates_have_separate_render_meshes(self):
        models=read_model_render_nodes(ROOT/'GYARI/worlds/rh1-wiezienie1.dat')
        self.assertEqual(len(models),50)
        self.assertIn('b_door25',models)
        self.assertIn('krata1',models)
        self.assertEqual(models['b_transparent1'][0].sections[0].texture.lower(),'textures\\ogolne\\kratka.dtx')

    def test_first_world_render_sections_and_triangles(self):
        nodes = read_render_nodes(ROOT / "GYARI/worlds/rh1-wiezienie1.dat")
        self.assertEqual(len(nodes), 47)
        self.assertEqual(len(nodes[1].vertices), 926)
        self.assertEqual(len(nodes[1].triangles), 464)
        self.assertEqual(sum(section.triangle_count for section in nodes[1].sections), 464)
        self.assertEqual(nodes[1].sections[0].texture.lower(), "textures\\sciany_sufity\\tynk 2.dtx")


if __name__ == "__main__":
    unittest.main()
