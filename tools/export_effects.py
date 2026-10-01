"""Export the retail gunfire, impact, smoke and blood sprites without modifying GYARI."""

import argparse
from pathlib import Path

from .decode_scripts import read_script
from .export_gameplay import Assets, fields, records, sections, unique, write_json
from .export_weapons import sprite_layer


def weapon_sprite_paths(game: Path):
    """Every sprite a weapon definition names (muzzle flashes, hits, smoke),
    with the `ile_sprite0 N` variants cshell picks at random (0x1000440c)."""
    _, groups = sections(records(read_script(game / 'scripts/items.txt')), 'item')
    paths = set()
    for group in unique(groups, 'scripts/items.txt').values():
        values = fields(group['records'])
        for value in values.values():
            if value.lower().endswith('.spr'):
                paths.add(value.replace(chr(92), '/').lower())
        sprite0, variants = values.get('sprite0', ''), values.get('ile_sprite0', '')
        if sprite0 and variants.isdigit():
            base = sprite0.replace(chr(92), '/').lower()
            for index in range(min(int(variants), 10)):
                variant = base[:-5] + str(index) + base[-4:]
                if (game / variant).is_file():
                    paths.add(variant)
    return sorted(paths)


# Hard-coded in cshell.dll: impacts, blood, tracers, casings, debris (docs/retail-gunfire.md).
EFFECT_SPRITES = [
    'sprites/iskra.spr', 'sprites/dymek.spr', 'sprites/papieros.spr', 'sprites/smuga.spr', 'sprites/ogon.spr',
    # Named in cshell.dll / object.lto but not (yet) played by the remake: enemy flashlight, snow, muzzle flashes, blast smoke (audit_assets.py).
    'sprites/latarkawrogow.spr', 'sprites/snieg.spr', 'sprites/blikred.spr', 'sprites/krew.spr', 'sprites/sig/main.spr', 'sprites/weapons/ingram.spr', 'sprites/weapons/ingramwrogow.spr',
    'sprites/weapons/systemdymduzy.spr', 'sprites/weapons/systemwybmortyr.spr', 'sprites/weapons/systemwybred.spr',
    'sprites/krew1.spr', 'sprites/krew2.spr', 'sprites/krew3.spr', 'sprites/krew4.spr',
    *[f'sprites/krew_dodatki/{index}.spr' for index in range(1, 12)],
    'sprites/bryzg.spr', 'sprites/bryzg2.spr', 'sprites/bryzgmaly.spr', 'sprites/bryzgmaly1.spr', 'sprites/bryzgmaly2.spr',
    *[f'sprites/krewpodloga{index}.spr' for index in range(1, 5)],
    'sprites/sladkrwipoziom.spr', 'sprites/krewmonitor.spr',
]
EFFECT_TEXTURES = ['textures/sprajty/dziura0.dtx', 'textures/sprajty/dziura1.dtx', 'textures/sprajty/dziura2.dtx', 'textures/sprajty/slad.dtx',
                   # loaded by name from cshell.dll / object.lto (enemy flashlight, snow, sniper reticle, blob shadow, menu pointer)
                   'textures/sprajty/latarka.dtx', 'textures/sprajty/snieg.dtx', 'textures/sprajty/system/celownik.dtx', 'textures/cien.dtx', 'misc/menu/kursor.dtx']
EFFECT_MODELS = {
    'models/misc/luska.ltb': 'skins/misc/luska.dtx',
    'models/misc/luska_do_obrzyna.ltb': 'skins/misc/luska_do_obrzyna.dtx',
    **{f'models/levelowe/kawalki/gruz0{index}.ltb': 'skins/levelowe/kawalki/gruz.dtx' for index in range(1, 5)},
    # No skin string sits next to the splinter models in cshell.dll; the
    # plank texture of the same debris set is the closest retail match.
    **{f'models/levelowe/kawalki/drewienko0{index}.ltb': 'skins/levelowe/kawalki/deski.dtx' for index in range(1, 4)},
    # EffectMgr::Spawn types 0x24 (three drumstick chunks, skin string right before the models in .data 0x1006cd8c) and 0x0b (k1..k4, skin kx.dtx at 0x1006cd28).
    **{f'models/levelowe/kawalki/miecho0{index}.ltb': 'skins/pikapy/udko_fin.dtx' for index in range(1, 4)},
    **{f'models/misc/k{index}.ltb': 'skins/misc/kx.dtx' for index in range(1, 5)},
}


def export(game: Path, output: Path):
    game, output = Path(game), Path(output)
    if output.resolve() == game.resolve() or game.resolve() in output.resolve().parents:
        raise ValueError('output must be outside the original game directory')
    assets = Assets(game, output)

    def sprite(path):
        return sprite_layer(assets, 'sprites/' + path + '.spr', 0.0, 0.0, 1.0)

    def image(path):
        texture = assets.texture('textures/sprajty/' + path + '.dtx')
        return {'sprite': None, 'frames': [texture], 'fps': 0,
                'source': 'textures/sprajty/' + path + '.dtx'}

    effects = {
        'format': 'mesterlovesz-retail-effects-v1',
        'flash': sprite('blik'),
        'smoke': sprite('dymek'),
        'blood': sprite('krew1'),
        'wall_hit': sprite('weapons/hit0'),
        'wall_marks': [image('dziura' + str(index)) for index in range(3)],
        'pools': [sprite('krewpodloga' + str(index)) for index in range(1, 5)],
        'blood_mark': sprite('sladkrwipoziom'),
        'sparks': sprite('iskra'),
        # Keyed by the lower-case items.txt path, e.g. "sprites/glock/main.spr".
        'weapon_sprites': {path: sprite_layer(assets, path, 0.0, 0.0, 1.0) for path in weapon_sprite_paths(game)},
        # Every sprite the gunfire code uses, keyed like weapon_sprites.
        'sprites': {path: sprite_layer(assets, path, 0.0, 0.0, 1.0) for path in sorted({*EFFECT_SPRITES, *weapon_sprite_paths(game)})},
        'textures': {path: assets.texture(path) for path in EFFECT_TEXTURES},
        'models': {path: {'asset': assets.model(path)['asset'], 'skin': assets.texture(skin)} for path, skin in EFFECT_MODELS.items()},
    }
    write_json(output / 'retail_effects.json', effects)
    return effects


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('game', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    export(args.game, args.output)
