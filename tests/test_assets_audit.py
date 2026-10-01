import unittest
from pathlib import Path

from tools.audit_assets import failures, run

ROOT = Path(__file__).resolve().parents[1]


@unittest.skipUnless((ROOT.parent / 'GYARI').is_dir() and (ROOT / 'output').is_dir(), 'needs the retail install and the local exports')
class AssetAuditTest(unittest.TestCase):
    def test_every_referenced_asset_is_exported_or_missing_in_retail_too(self):
        report = run(ROOT.parent / 'GYARI', ROOT / 'output')
        self.assertGreater(report['references'], 8000)
        self.assertEqual(failures(report), {})
        self.assertEqual(report['character_problems'], [])
        self.assertEqual(report['exported_path_problems'], [])

    def test_heads_are_exported_for_every_glowa_specjalna_character(self):
        report = run(ROOT.parent / 'GYARI', ROOT / 'output')
        self.assertFalse([p for p in report['character_problems'] if 'glowa_specjalna' in p or 'head' in p])


if __name__ == '__main__':
    unittest.main()
