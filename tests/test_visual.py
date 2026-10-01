import math
import struct
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from tools.export_visual import export_visual, write_dtx_png
from tools.render_dat import read_render_nodes


class VisualExportTests(unittest.TestCase):
    def test_opaque_retail_bus_skin_becomes_png(self):
        source = ROOT / "GYARI/skins/levelowe/autobus.dtx"
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "bus.png"
            write_dtx_png(source, target)
            png = target.read_bytes()
            self.assertEqual(png[:8], b"\x89PNG\r\n\x1a\n")
            self.assertEqual(struct.unpack_from(">II", png, 16), (512, 512))

    def test_first_world_visual_faces_match_render_data(self):
        source = ROOT / "GYARI/worlds/rh1-wiezienie1.dat"
        expected = sum(section.triangle_count for node in read_render_nodes(source) for section in node.sections if section.shader != 2)
        with tempfile.TemporaryDirectory() as directory:
            obj, mtl = export_visual(source, ROOT / "GYARI", Path(directory))
            self.assertEqual(sum(line.startswith("f ") for line in obj.read_text().splitlines()), expected)
            self.assertIn("map_Kd", mtl.read_text())
            self.assertTrue(any((Path(directory) / "textures").rglob("*.png")))
            self.assertEqual(expected, 15638)

    def test_missing_texture_sections_get_finite_uvs(self):
        # rh1-wiezienie1 has 12 faces whose texture the retail map compiler could not find: their DAT UVs are inf/NaN.
        source = ROOT / "GYARI/worlds/rh1-wiezienie1.dat"
        self.assertTrue(any(not all(map(math.isfinite, vertex.uv)) for node in read_render_nodes(source) for vertex in node.vertices))
        with tempfile.TemporaryDirectory() as directory:
            obj, _ = export_visual(source, ROOT / "GYARI", Path(directory))
            self.assertTrue(all(math.isfinite(float(value)) for line in obj.read_text().splitlines() if line.startswith("vt ") for value in line.split()[1:]))


if __name__ == "__main__":
    unittest.main()
