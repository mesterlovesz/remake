import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from tools.decode_scripts import decode_text


class ScriptCipherTests(unittest.TestCase):
    def test_decodes_known_retail_words_and_round_trips(self):
        encoded = "npcfk npcfkt\\kfufkpxf\\bvspavt.ksa // Njqbhf Jmsfqbdsjuf 3113"
        decoded = decode_text(encoded)
        self.assertEqual(decoded, "model models\\levelowe\\autobus.ltb // Mirage Interactive 2002")
        self.assertEqual(decode_text(decoded), encoded)

    def test_does_not_alter_hungarian_accents_or_path_separators(self):
        self.assertEqual(decode_text("éáő_\\/ ."), "éáő_\\/ .")


if __name__ == "__main__":
    unittest.main()
