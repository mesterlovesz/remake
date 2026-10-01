from pathlib import Path
import tempfile
import unittest

from PIL import Image

from tools.export_menu import export

GAME = Path(__file__).resolve().parents[2] / 'GYARI'


class RetailMenuTests(unittest.TestCase):
    def test_retail_pixels_localization_and_campaign_coverage(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            data = export(GAME, output)
            self.assertEqual(len(data['loading']), 28)
            self.assertEqual(data['loading']['chinatown']['title'], 'Kiskína')
            self.assertEqual(data['loading']['rh3-miasteczko0']['image'], 'ui/loading/prolog.png')
            self.assertEqual(data['loading']['rh1-wiezienie2']['image'], 'ui/loading/mutiny.png')
            self.assertEqual(data['menu_title']['text'], 'Főmenü')
            self.assertEqual([button['id'] for button in data['buttons']],
                             ['new', 'load', 'save', 'options', 'controls', 'display', 'audio', 'exit'])
            for character in 'áéíóöőúüűÁÉÍÓÖŐÚÜ':
                self.assertIn(character, data['font']['glyphs'])
            for source, metadata in data['source_images'].items():
                with Image.open(GAME/source) as original, Image.open(output/metadata['image']) as converted:
                    self.assertEqual(original.convert('RGB').tobytes(), converted.convert('RGB').tobytes(), source)
            for row in data['loading'].values():
                self.assertTrue((output/row['image']).is_file())
                self.assertTrue((output/row['title_image']).is_file())
            with Image.open(output/data['buttons'][0]['image']) as label:
                self.assertEqual(label.height, 64)
                self.assertEqual(label.getchannel('A').getextrema(), (0, 255))


if __name__ == '__main__':
    unittest.main()
