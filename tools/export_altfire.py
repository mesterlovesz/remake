"""Export the assets of the retail laser, sniper scope and nightstick without modifying GYARI (docs/retail-weapons-alt.md)."""

import argparse
from pathlib import Path

from .export_gameplay import Assets, write_json
from .export_weapons import sprite_layer


def export(game: Path, output: Path):
    game, output = Path(game), Path(output)
    if output.resolve() == game.resolve() or game.resolve() in output.resolve().parents:
        raise ValueError('output must be outside the original game directory')
    assets = Assets(game, output)
    result = {
        'format': 'mesterlovesz-retail-altfire-v1',
        # cshell 0x1000e870: the dot (scale 0.1) and the dust (scale 0.02) use this 32x32 additive spot.
        'laser_dot': sprite_layer(assets, 'sprites/weapons/pyleklasera.spr', 0.0, 0.0, 1.0),
        # cshell 0x10036340: one quarter of the reticle, centred at its bottom-right corner (512x512).
        'scope': {'texture': assets.texture('misc/panel/celownik/Snajper_celownik.dtx'), 'width': 512, 'height': 512},
        # cshell 0x1000ff90: the nightstick against a wall.
        'melee': {'wall_sound': assets.sound('sounds\\weapons\\palka_sciana.wav')},
    }
    write_json(output / 'retail_altfire.json', result)
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('game', type=Path, nargs='?', default=Path('../GYARI'))
    parser.add_argument('output', type=Path, nargs='?', default=Path('output'))
    args = parser.parse_args()
    export(args.game, args.output)
