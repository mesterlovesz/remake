import os, struct, tempfile, unittest
from pathlib import Path

from tools import inventory_unused as inv
from tools.audit_assets import Index

ROOT = Path(__file__).resolve().parents[1]


class HelperTests(unittest.TestCase):
    def test_numbered_keywords_match_format_and_stem_literals(self):
        rs = 'let a = format!("answer{index}snd"); slot_list(&c, "on_koniec_widzi", 8); k.starts_with("skin"); get("HP")'
        self.assertTrue(inv.remake_has('answer2snd', rs))
        self.assertTrue(inv.remake_has('on_koniec_widzi3', rs))
        self.assertTrue(inv.remake_has('skin4', rs))
        self.assertTrue(inv.remake_has('HP', rs))
        self.assertFalse(inv.remake_has('obrot_postrzalu0', rs))
        self.assertFalse(inv.remake_has('malutki', rs))

    def test_resolution_prefix_rule(self):
        self.assertTrue(inv.prefix_live('misc/menu_l/save_1024.pcx', {'save', 'load'}))
        self.assertFalse(inv.prefix_live('misc/menu_l/kursor.dtx', {'save'}))
        self.assertFalse(inv.prefix_live('misc/menu_l/other_640.pcx', {'save'}))

    def test_wav_duration_from_chunks(self):
        with tempfile.TemporaryDirectory() as t:
            f = Path(t) / 'a.wav'
            fmt = struct.pack('<HHIIHH', 1, 1, 8000, 16000, 2, 16)
            data = bytes(32000)
            f.write_bytes(b'RIFF' + struct.pack('<I', 36 + len(data)) + b'WAVE' + b'fmt ' + struct.pack('<I', 16) + fmt + b'data' + struct.pack('<I', len(data)) + data)
            self.assertEqual(inv.wav_seconds(f), 2.0)
            (Path(t) / 'b.wav').write_bytes(b'not a wav at all')
            self.assertIsNone(inv.wav_seconds(Path(t) / 'b.wav'))

    def test_halt_dead_sequence_makes_the_whole_folder_live(self):
        with tempfile.TemporaryDirectory() as t:
            for n in ('halt1.wav', 'halt2.wav', 'dead0.wav', 'other.wav'):
                (Path(t) / 'sounds/enemies/china').mkdir(parents=True, exist_ok=True)
                (Path(t) / 'sounds/enemies/china' / n).write_bytes(b'x')
            retail = Index(t)
            refs = {'sounds/enemies/china/halt0.wav': [('scripts/postacie.txt:1 [postac a] sound_on_kontakt', True)]}
            refs = inv.defaultdict(list, refs)
            inv.sequence_expansion(retail, refs)
            self.assertIn('sounds/enemies/china/halt2.wav', refs)
            self.assertIn('sounds/enemies/china/dead0.wav', refs)
            self.assertNotIn('sounds/enemies/china/other.wav', refs)

    def test_categories(self):
        self.assertEqual(inv.file_category('sounds/muza/a.wav'), 'music')
        self.assertEqual(inv.file_category('sounds/speech/x/1.wav'), 'speech')
        self.assertEqual(inv.file_category('worlds/chinatown.dat'), 'world')
        self.assertEqual(inv.file_category('cshell.dll'), 'engine')


@unittest.skipUnless(os.environ.get('INVENTORY_FULL') and (ROOT.parent / 'GYARI').is_dir() and (ROOT / 'output').is_dir(), 'set INVENTORY_FULL=1 (needs the retail install and the local exports, ~1 min)')
class FullInventoryTest(unittest.TestCase):
    def test_inventory_runs_and_the_known_cut_content_is_found(self):
        report = inv.run(ROOT.parent / 'GYARI', ROOT / 'output', ROOT)
        self.assertEqual(report['install_files'], sum(sum(c.values()) for c in report['summary_files'].values()))
        worlds = {w['world']: w for w in report['worlds']}
        self.assertEqual(len(worlds), 31)
        self.assertEqual(sorted(w for w, v in worlds.items() if not v['linked_main_route']), ['chinatown', 'katscena', 'nic', 'outro'])
        unused = {r['path'] for r in report['files'] if r['status'] == 'UNUSED-IN-RETAIL'}
        self.assertIn('sounds/muza/czapel.wav', unused)
        self.assertIn('models/bronie/proximity_bomb.ltb', unused)
        used = {r['path'] for r in report['files'] if r['status'] == 'USED-IN-REMAKE'}
        self.assertIn('sounds/muza/menu.wav', used)


if __name__ == '__main__':
    unittest.main()
