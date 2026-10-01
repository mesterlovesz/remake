from pathlib import Path
import unittest
from tools.export_presentation import opening_timeline
GAME=Path(__file__).resolve().parents[2]/'GYARI'

class OpeningTests(unittest.TestCase):
    def test_original_scene_order_subtitle_and_transition(self):
        phases=opening_timeline(GAME)
        self.assertEqual(phases[0]['id'],'001')
        self.assertAlmostEqual(phases[2]['start'],3.12)
        self.assertEqual(phases[2]['subtitle'],'A munka elvégezve.')
        self.assertEqual(phases[-1]['runworld'],'rh1-wiezienie2')
        self.assertEqual(phases[-2]['scene'],'intro zwei')
        for p in phases:
            for key in ('sound','speech'):
                if p.get(key): self.assertTrue((GAME/p[key]).is_file(),p[key])
