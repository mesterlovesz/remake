import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from tools.export_visual import export_visual
from tools.lightmaps import face_uvs, pack, read_effect_groups
from tools.render_dat import RenderNode, RenderSection, RenderVertex, decode_lightmap, read_render_nodes


def vertex(position, uv=(0.0, 0.0)):
    return RenderVertex(position, uv, (0.0, 0.0), 0xFF808080, (0.0, 1.0, 0.0))


class LightmapTests(unittest.TestCase):
    def test_rle_control_byte_repeats_or_copies_pixels(self):
        # 0x82: repeat the next pixel 3 times; 0x01: two literal pixels.
        data = bytes([0x82, 10, 20, 30, 0x01, 1, 2, 3, 4, 5, 6])
        self.assertEqual(decode_lightmap(data, 5, 1), bytes([10, 20, 30] * 3 + [1, 2, 3, 4, 5, 6]))
        with self.assertRaises(ValueError):
            decode_lightmap(data, 4, 1)

    def test_base_pass_triangle_takes_the_uv_of_its_lightmap_pass_twin(self):
        # The lightmap pass (shader 2) has its own vertices with the lightmap UV in .uv; the base pass (shader 4) has none.
        base = [vertex((0, 0, 0), (5, 5)), vertex((10, 0, 0), (6, 5)), vertex((0, 10, 0), (5, 6))]
        twin = [vertex((0, 10, 0), (0.25, 0.75)), vertex((0, 0, 0), (0.25, 0.25)), vertex((10, 0, 0), (0.75, 0.25))]
        lightmap = bytes([0x83, 40, 40, 40]) * 2  # 8 pixels, 4 x 2
        node = RenderNode(
            (RenderSection("t.dtx", 4, 1), RenderSection("LightAnim_BASE", 2, 1, "", (4, 2), lightmap)),
            tuple(base + twin), ((0, 1, 2, 0), (5, 3, 4, 0)))  # twin rotated: same winding, other start vertex
        atlas, width, height, placements = pack([node], width=64)
        self.assertEqual(len(atlas), width * height * 3)
        x, y, w, h = placements[(0, 1)]
        self.assertEqual((w, h), (4, 2))
        values = face_uvs(0, node, placements, (width, height))
        self.assertEqual(len(values), 6)
        # Base vertex 0 (0,0,0) sits at twin uv (0.25, 0.25) of the 4x2 image placed at (x, y).
        self.assertAlmostEqual(values[0], (x + 0.25 * 4) / width)
        self.assertAlmostEqual(values[1], (y + 0.25 * 2) / height)
        self.assertAlmostEqual(values[4], (x + 0.25 * 4) / width)
        self.assertAlmostEqual(values[5], (y + 0.75 * 2) / height)

    def test_effect_groups_carry_the_pan_speed_of_the_water(self):
        groups = read_effect_groups(ROOT / "GYARI")
        pan = groups["pan.tfg"]
        self.assertEqual(pan["script"], "UVPan")
        self.assertAlmostEqual(pan["params"]["SpeedX"], 0.002, places=5)
        self.assertAlmostEqual(pan["params"]["SpeedY"], 0.004, places=5)
        self.assertAlmostEqual(groups["pan_szybszy.tfg"]["params"]["SpeedY"], 0.006, places=5)

    def test_first_world_is_fully_lightmapped_and_every_lightmap_decodes(self):
        source = ROOT / "GYARI/worlds/rh1-wiezienie1.dat"
        nodes = read_render_nodes(source)
        for node in nodes:
            for section in node.sections:
                if section.shader == 2:
                    width, height = section.lightmap_size
                    self.assertEqual(len(decode_lightmap(section.lightmap_data, width, height)), width * height * 3)
        with tempfile.TemporaryDirectory() as directory:
            export_visual(source, ROOT / "GYARI", Path(directory))
            report = (Path(directory) / "rh1-wiezienie1.visual.report.json").read_text()
            self.assertIn('"lightmapped_faces": 15638', report)
            self.assertTrue((Path(directory) / "textures/rh1-wiezienie1/lightmap.png").is_file())
            self.assertEqual((Path(directory) / "rh1-wiezienie1.visual.lightmap.bin").stat().st_size, 15638 * 24)


if __name__ == "__main__":
    unittest.main()
