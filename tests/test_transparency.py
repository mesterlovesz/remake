"""Source-driven transparency regressions; never infer holes from RGB colors."""
import json
from pathlib import Path
import struct
import tempfile
import unittest
import zlib

from tools.export_visual import export_visual, write_dtx_png, material_metadata
from tools.export_model_worlds import export
from tools.render_dat import RenderNode, RenderSection
from tools.lithtech_dat import WorldObject

GAME = Path(__file__).resolve().parents[2] / 'GYARI'


def png_alpha(path):
    data = path.read_bytes()
    size = struct.unpack_from('>I', data, 33)[0]
    raw = zlib.decompress(data[41:41 + size])
    width, height = struct.unpack_from('>II', data, 16)
    return b''.join(raw[y * (width * 4 + 1) + 4:(y + 1) * (width * 4 + 1):4] for y in range(height))


class TransparencyTests(unittest.TestCase):
    def test_explicit_mask_additive_and_reflection_metadata(self):
        section = RenderSection('texture.dtx', 4, 0)
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'test.dtx'
            data = bytearray(164)
            command = b'AlphaRef 127'
            data[36:36 + len(command)] = command
            source.write_bytes(data)
            metadata = material_metadata(source, section)
            self.assertEqual(metadata['alpha_mode'], 'mask')
            self.assertAlmostEqual(metadata['alpha_cutoff'], 128 / 255)
            metadata = material_metadata(source, section, WorldObject('b_transparent', {'Alpha': .25, 'Additive': 1}))
            self.assertEqual(metadata['alpha_mode'], 'add')
            self.assertEqual(metadata['alpha_factor'], .25)
            data[36:164] = bytes(128)
            command = b'EnvMapAlpha reflections.dtx'
            data[36:36 + len(command)] = command
            struct.pack_into('<II', data, 16, 137, 2)
            source.write_bytes(data)
            self.assertEqual(material_metadata(source, section)['alpha_mode'], 'opaque')

    def test_zero_alpha_can_be_authored_transparent_or_unused_opaque(self):
        with tempfile.TemporaryDirectory() as directory:
            source, target = Path(directory) / 'zero.dtx', Path(directory) / 'zero.png'
            data = bytearray(168)
            struct.pack_into('<HH', data, 8, 1, 1)
            data[26] = 3
            data[164:168] = bytes((40, 50, 60, 0))
            source.write_bytes(data)
            write_dtx_png(source, target, preserve_zero_alpha=True)
            self.assertEqual(png_alpha(target), b'\0')
            write_dtx_png(source, target)
            self.assertEqual(png_alpha(target), b'\xff')

    def test_same_texture_different_shader_keeps_source_material_identity(self):
        nodes = [RenderNode((RenderSection('unused', 1, 0), RenderSection('unused', 4, 0)), (), ())]
        with tempfile.TemporaryDirectory() as directory:
            export_visual(Path('fixture.dat'), GAME, Path(directory), nodes)
            materials = json.loads((Path(directory) / 'fixture.visual.materials.json').read_text())
            self.assertEqual({m['source_shader'] for m in materials.values()}, {1, 4})

    def test_retail_grates_glass_and_opaque_door_keep_authored_modes(self):
        with tempfile.TemporaryDirectory() as directory:
            out = Path(directory)
            export(GAME / 'worlds/rh1-wiezienie1.dat', GAME, out)
            folder = out / 'world_models/rh1-wiezienie1'
            def material(name):
                return next(iter(json.loads((folder / f'{name}.visual.materials.json').read_text()).values()))
            self.assertEqual(material('krataceladol')['alpha_mode'], 'blend')
            self.assertEqual(material('b_transparent1')['alpha_mode'], 'blend')
            self.assertAlmostEqual(material('b_transparent25')['alpha_factor'], .09, places=6)
            self.assertEqual(material('b_door25')['alpha_mode'], 'opaque')
            alpha = png_alpha(folder / 'textures/krataceladol/mat0000.png')
            self.assertIn(0, alpha)
            self.assertIn(255, alpha)


if __name__ == '__main__':
    unittest.main()
