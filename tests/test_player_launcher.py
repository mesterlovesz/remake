"""Player setup boundaries: ISO discovery and reading installer data without a mount."""
import importlib
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


class PlayerSetupTests(unittest.TestCase):
    def setUp(self):
        self.assertIsNotNone(importlib.util.find_spec('tools.player_launcher'),
                             'Az ISO-s játékosindító még hiányzik.')
        self.launcher = importlib.import_module('tools.player_launcher')
        self.folder = tempfile.TemporaryDirectory(prefix='mester játékos ')
        self.addCleanup(self.folder.cleanup)
        self.root = Path(self.folder.name)

    def test_iso_with_spaces_is_selected_without_renaming(self):
        iso = self.root / 'A mesterlövész.ISO'
        iso.write_bytes(b'not read during discovery')
        self.assertEqual(self.launcher.select_iso(self.root), iso)

    def test_multiple_isos_do_not_silently_choose_the_wrong_game(self):
        for name in ('game.iso', 'another.iso'):
            (self.root / name).touch()
        with self.assertRaisesRegex(self.launcher.SetupError, 'több ISO'):
            self.launcher.select_iso(self.root)

    def test_missing_iso_has_an_actionable_error(self):
        with self.assertRaisesRegex(self.launcher.SetupError, 'ISO.*Indit.bat'):
            self.launcher.select_iso(self.root)

    def test_download_corruption_is_rejected_before_executing_the_converter(self):
        import io
        with patch('urllib.request.urlopen', return_value=io.BytesIO(b'incomplete download')):
            with self.assertRaisesRegex(self.launcher.SetupError, 'hiányos vagy hibás'):
                self.launcher.ensure_media_converter(self.root)
        self.assertFalse(any(self.root.rglob('*.exe')))

    def test_prepared_converter_does_not_need_network(self):
        converter = self.root / 'python' / 'Lib' / 'site-packages' / self.launcher.MEDIA_ENTRY
        converter.parent.mkdir(parents=True)
        converter.touch()
        with patch('urllib.request.urlopen', side_effect=AssertionError('must work offline')):
            self.launcher.ensure_media_converter(self.root)

    def test_extracted_game_is_found_even_when_unshield_normalizes_group_name(self):
        folder = self.root / 'App_Executables'
        folder.mkdir()
        (folder / 'Lithtech.exe').touch()
        self.assertTrue(hasattr(self.launcher, 'find_extracted_install'),
                        'Az Unshield normalizált csoportnevét még nem kezeli az indító.')
        self.assertEqual(self.launcher.find_extracted_install(self.root), folder)

    def test_incomplete_iso_is_reported_as_setup_error(self):
        iso = self.root / 'incomplete.iso'
        iso.write_bytes(b'incomplete download')
        with self.assertRaises(self.launcher.SetupError):
            self.launcher.read_cabinets(iso, self.root / 'cache')

    def test_installer_directory_spaces_are_restored_without_changing_file_data(self):
        original = self.root / 'textures' / 'sprajty' / 'ingram_wrogow' / 'spriteA1.dtx'
        original.parent.mkdir(parents=True)
        original.write_bytes(b'original texture data')
        listing = '   348324  App Executables\\textures\\sprajty\\ingram wrogow\\spriteA1.dtx\n'
        self.assertTrue(hasattr(self.launcher, 'restore_installer_paths'),
                        'Az eredeti lemezfájlnevek visszaállítása még hiányzik.')
        self.launcher.restore_installer_paths(self.root, listing)
        restored = self.root / 'textures' / 'sprajty' / 'ingram wrogow' / 'spriteA1.dtx'
        self.assertEqual(restored.read_bytes(), b'original texture data')
        self.assertFalse(original.exists())

    def test_only_installer_cabinets_are_read_from_joliet_iso(self):
        import io
        import pycdlib
        disc = pycdlib.PyCdlib()
        disc.new(joliet=3)
        disc.add_directory(iso_path='/GAME', joliet_path='/A mesterlövész')
        for name, data in [('DATA1.CAB', b'cab1'), ('DATA2.CAB', b'cab2'),
                           ('DATA1.HDR', b'header'), ('SETUP.EXE', b'do not run')]:
            disc.add_fp(io.BytesIO(data), len(data), iso_path=f'/GAME/{name};1',
                        joliet_path=f'/A mesterlövész/{name.lower()}')
        iso = self.root / 'game.iso'
        disc.write(str(iso))
        disc.close()
        destination = self.root / 'cache'
        self.launcher.read_cabinets(iso, destination)
        self.assertEqual(sorted(p.name for p in destination.iterdir()),
                         ['data1.cab', 'data1.hdr', 'data2.cab'])
        self.assertEqual((destination / 'data2.cab').read_bytes(), b'cab2')


if __name__ == '__main__':
    unittest.main()
