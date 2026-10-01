import tempfile
import unittest
from pathlib import Path

from tools.export_effects import export


class EffectExportTest(unittest.TestCase):
    def test_original_effect_frames_and_materials(self):
        game = Path(__file__).resolve().parents[2] / 'GYARI'
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder)
            manifest = export(game, output)
            self.assertEqual(len(manifest['smoke']['frames']), 10)
            self.assertEqual(len(manifest['blood']['frames']), 35)
            self.assertEqual(len(manifest['pools']), 4)
            self.assertEqual(len(manifest['wall_marks']), 3)
            self.assertEqual(manifest['smoke']['fps'], 15)
            for effect in [manifest['flash'], manifest['smoke'], manifest['blood'],
                           *manifest['pools'], *manifest['wall_marks']]:
                for frame in effect['frames']:
                    self.assertTrue((output / frame).is_file(), frame)


if __name__ == '__main__':
    unittest.main()
