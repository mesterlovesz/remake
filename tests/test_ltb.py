from pathlib import Path
import unittest
from tools.ltb import read_ltb
GAME=Path(__file__).resolve().parents[2]/'GYARI'

class ModelTests(unittest.TestCase):
    def test_cutscene_animation_bindings_keep_authored_scene_offsets(self):
        for name,offset in [('intro',[-700.0,-25.0,0.0]),('intro1',[-48.0,200.0,420.0])]:
            model=read_ltb(GAME/f'models/cutscenes/{name}.ltb')
            for anim_name,animation in model['animations'].items():
                expected=[0.0,0.0,0.0] if name=='intro' and anim_name=='scena17' else offset
                self.assertEqual(animation['translation'],expected)
                self.assertEqual(animation['dimensions'],[1.0,1.0,1.0])

    def test_retail_camera_rig_has_complete_animation_tracks(self):
        m=read_ltb(GAME/'models/cutscenes/intro.ltb')
        self.assertEqual(len(m['nodes']),7)
        self.assertEqual(len(m['animations']),19)
        self.assertIn('kamera',m['sockets'])
        self.assertIn('scena17',m['animations'])
        for a in m['animations'].values():
            self.assertEqual(len(a['tracks']),7)
            for t in a['tracks']:
                self.assertIn(len(t['pos']),(1,len(a['times'])))
                self.assertIn(len(t['rot']),(1,len(a['times'])))

    def test_car_mesh_weights_and_indices_are_valid(self):
        m=read_ltb(GAME/'models/postacie/cutsceny/limuzyna.ltb')
        self.assertEqual(len(m['nodes']),65)
        self.assertGreater(sum(len(p['indices']) for p in m['pieces']),1000)
        for p in m['pieces']:
            self.assertTrue(all(i<len(p['positions']) for i in p['indices']))
            for weights in p['weights']:
                # Ten retail car/body vertices have weights summing to 0.5.
                # Keep these source values; normalization is a renderer concern.
                self.assertGreater(sum(w[1] for w in weights),0.0)
                self.assertTrue(all(w[0]<65 for w in weights))
