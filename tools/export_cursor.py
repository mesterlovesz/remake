"""Export the retail golden arrow mouse cursor (misc/menu/Cursor1024.pcx).

Palette index 0 is the transparent backdrop, as in the retail menu renderer.
"""
import argparse
from pathlib import Path

from PIL import Image


def export(game: Path, output: Path) -> Path:
    source = Image.open(game / 'misc/menu/Cursor1024.pcx')
    rgba = source.convert('RGBA')
    rgba.putalpha(Image.frombytes('L', source.size, bytes(0 if index == 0 else 255 for index in source.tobytes())))
    target = output / 'ui/cursor.png'
    target.parent.mkdir(parents=True, exist_ok=True)
    rgba.save(target)
    print('wrote', target)
    return target


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('game', type=Path)
    parser.add_argument('output', type=Path)
    arguments = parser.parse_args()
    export(arguments.game, arguments.output)
