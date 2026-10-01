from pathlib import Path
import json
import tempfile
import unittest

from tools.export_weapons import export


GAME = Path(__file__).resolve().parents[2] / 'GYARI'


class OriginalWeaponExportTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory()
        cls.output = Path(cls.temp.name)
        cls.manifest = export(GAME, cls.output)
        cls.weapons = {weapon['id']: weapon for weapon in cls.manifest['weapons']}

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def test_first_person_assets_and_every_referenced_animation_exist(self):
        self.assertEqual(len(self.weapons), 11)
        for weapon in self.weapons.values():
            model = json.loads((self.output / (weapon['model'] + '.json')).read_text(encoding='utf-8'))
            self.assertGreater(len(model['pieces']), 0)
            for path in [weapon['hud'], *weapon['skins'].values(), *weapon['sounds'].values()]:
                if path:
                    self.assertTrue((self.output / path).is_file(), path)
            for group in weapon['animations'].values():
                for clip in group if isinstance(group, list) else [group]:
                    if clip:
                        if clip in weapon['missing_animations']:
                            self.assertNotIn(clip, model['animations'])
                            continue
                        self.assertIn(clip, model['animations'], (weapon['id'], clip))
                        self.assertEqual(weapon['durations'][clip], max(model['animations'][clip]['times']))

    def test_original_inventory_values_and_order_are_preserved(self):
        self.assertEqual([w['id'] for w in self.manifest['weapons'][:3]],
                         ['Police nightstick', 'Glock', 'Smith and Wesson m. 625'])
        self.assertEqual([w['id'] for w in self.manifest['weapons'][5:9]],
                         ['FN shotgun', 'M-14', 'HK G8', 'P90'])
        glock = self.weapons['Glock']
        self.assertEqual((glock['ammo_index'], glock['ammo_amount'], glock['capacity']), (0, 17, 17))
        self.assertEqual(glock['animations']['shoot'], ['strzal1', 'strzal2'])
        self.assertTrue(glock['automatic'])  # Original held primary action, not a firearm realism rule.
        self.assertEqual(glock['offset'], [-2.1, -14.0, -0.8])
        shotgun = self.weapons['FN shotgun']
        self.assertTrue(shotgun['shotgun'])
        self.assertEqual(shotgun['pellets'], 10)
        self.assertEqual(shotgun['animations']['reload'], ['reload0', 'reload1', 'reload2'])
        self.assertEqual([v for k, v in shotgun['commands'] if k == 'descr'], ['>IdShotgun', '>IdShotgunT'])
        self.assertTrue(self.weapons['Hand grenade']['grenade'])
        self.assertFalse(self.weapons['heli_bron']['player_selectable'])
        self.assertEqual(self.weapons['Hand grenade']['animations']['hold'], 'zawleczka')
        self.assertEqual(self.weapons['Police nightstick']['missing_animations'], ['reload'])

    def test_source_hashes_and_hungarian_names(self):
        self.assertEqual(self.weapons['Police nightstick']['title'], 'Gumibot.')
        self.assertEqual(self.weapons['FN shotgun']['title'], 'Vadászpuska.')
        self.assertEqual(len(self.manifest['sources'][0]['sha256']), 64)
        self.assertEqual(self.weapons['Glock']['source']['model']['path'], 'models/bronie/glock22c.ltb')

    def test_refuses_any_output_inside_original_install(self):
        with self.assertRaisesRegex(ValueError, 'outside'):
            export(GAME, GAME / 'forbidden-export-test')

    def test_complete_pickup_catalog_supports_unplaced_weapon_drops(self):
        items = json.loads((self.output / 'retail_items.json').read_text(encoding='utf-8'))
        self.assertIn('MAC-10 Ingram', items)
        self.assertIn('Ingram ammo', items)
        self.assertEqual(items['Ingram ammo']['ammo'], 50)
        self.assertEqual(items['Ingram ammo']['ammo_for'], 3)
        self.assertEqual(items['Glock']['model'], 'models/pikapy/glock_low.ltb')
        self.assertTrue(items['Glock']['weapon'])
        for item in items.values():
            self.assertTrue((self.output / (item['model'] + '.json')).is_file())

    def test_grenade_exports_source_visual_frames_and_physics_quirk(self):
        effect = self.manifest['grenade_effect']
        self.assertTrue((self.output / effect['sound']).is_file())
        self.assertTrue((self.output / effect['impact_sound']).is_file())
        self.assertEqual(effect['layers'][0]['fps'], 15)
        self.assertEqual(len(effect['layers'][0]['frames']), 11)
        for layer in effect['layers']:
            for frame in layer['frames']:
                self.assertTrue((self.output / frame).is_file())
        physics = self.weapons['Hand grenade']['grenade_physics']
        self.assertEqual(physics['fuse_seconds'], 5.0)
        self.assertEqual(physics['thrown_fuse_seconds'], 3.0)
        self.assertEqual(physics['gravity'], 640.0)


if __name__ == '__main__':
    unittest.main()
