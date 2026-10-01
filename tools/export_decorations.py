"""Export the sprite textures the retail decoration objects draw (lamp glows, halos, rain ripples,
lens flares, smoke, sparks) without modifying GYARI.

Every `*.spr` a level object names (d_sprite Nazwa_plika, emitter sprites, lens flare parts) plus a few
class-owned fixed sprites is read (header: frame count, fps, 3 flags; length-prefixed DTX paths) and each
DTX frame is written as an OPAQUE RGB PNG: retail additive sprites blend with ONE/ONE (Lithtech.exe
0x53d5b0..0x53d5e1) and never read texture alpha. Output: decor/<dtx path>.png and decor/sprites.json.
"""
import argparse, json, struct
from pathlib import Path

from PIL import Image

from .export_visual import write_dtx_png

# Sprites a class loads by name inside cshell.dll (not stored in the level DAT properties).
FIXED_SPRITES = ['sprites/kregi.spr']


def normal(path):
    return path.replace('\\', '/').lower()


def scene_sprites(output: Path):
    """All *.spr strings any level object property names."""
    found = set()
    for scene in sorted(output.glob('*.scene.json')):
        for obj in json.loads(scene.read_text(encoding='utf-8')).get('objects', []):
            for value in obj.get('properties', {}).values():
                if isinstance(value, str) and value.lower().endswith('.spr'):
                    found.add(normal(value))
    return found


def read_spr(data: bytes, name: str):
    count, fps, *_flags = struct.unpack_from('<5I', data)
    if not 0 < count <= 1000 or not 0 < fps <= 1000:
        raise ValueError(f'invalid sprite frame count/rate: {name}')
    offset, frames = 20, []
    for _ in range(count):
        length = struct.unpack_from('<H', data, offset)[0]
        offset += 2
        frames.append(data[offset:offset + length].decode('cp1250'))
        offset += length
    return fps, frames


def export(game: Path, output: Path):
    game, output = Path(game), Path(output)
    if output.resolve() == game.resolve() or game.resolve() in output.resolve().parents:
        raise ValueError('output must be outside the original game directory')
    sprites, missing = {}, []
    for name in sorted(scene_sprites(output) | set(FIXED_SPRITES)):
        source = game / name
        if not source.is_file():
            missing.append(name)  # e.g. sprites\mortyr.spr: the retail game draws nothing for it
            continue
        fps, frame_paths = read_spr(source.read_bytes(), name)
        frames, size = [], None
        for frame in frame_paths:
            dtx = normal(frame)
            target = f'decor/{dtx}.png'
            if not (game / dtx).is_file():
                frames = None
                break
            if not (output / target).is_file():
                temporary = output / 'decor' / '_tmp.png'
                write_dtx_png(game / dtx, temporary)
                (output / target).parent.mkdir(parents=True, exist_ok=True)
                with Image.open(temporary) as image:
                    image.convert('RGB').save(output / target)
                temporary.unlink()
            with Image.open(output / target) as image:
                size = size or image.size
            frames.append(target)
        if not frames:
            missing.append(name)
            continue
        sprites[name] = {'fps': fps, 'frames': frames, 'width': size[0], 'height': size[1]}
    manifest = {'format': 'mesterlovesz-decor-sprites-v1', 'sprites': sprites, 'missing': missing}
    (output / 'decor').mkdir(parents=True, exist_ok=True)
    (output / 'decor/sprites.json').write_text(json.dumps(manifest, indent=1), encoding='utf-8')
    print(f'{len(sprites)} sprites exported, missing: {missing}')
    return manifest


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('game', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    export(args.game, args.output)
