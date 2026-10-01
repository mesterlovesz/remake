"""Export the original inventory, character-info and player-attribute panels, item icons,
bitmap fonts and item rules for the remake. Retail inputs are only read.

Usage: python -m tools.export_inventory ../GYARI output
Writes `retail_inventory.json` plus PNGs under `ui/panel`, `ui/items` and `ui/fonts`.
Existing exports are never removed or renamed.
"""
import argparse
import json
from pathlib import Path

from PIL import Image, ImageChops

from .decode_scripts import read_script
from .export_presentation import blocks

# File stems of the retail bitmap fonts (misc/fonts/<font>/1024/*.pcx).
SPECIAL = {'kropka': '.', 'przecinek': ',', 'dwukropek': ':', 'srednik': ';', 'minus': '-',
           'plus': '+', 'procent': '%', 'slash': '/', 'backslash': '\\', 'nawiasL': '(',
           'nawiasP': ')', 'pytajnik': '?', 'wykrzyknik': '!', 'spacja': ' ', 'cudzyslow': '"',
           'gwiazdka': '*', 'krzyzyk': '#', 'hash': '#', 'maupa': '@', 'dollar': '$', 'apostrof': "'"}
FONTS = {'info': 'misc/fonts/info/1024', 'cyfry': 'misc/fonts/cyfry/1024',
         'un': 'misc/fonts/Un/1024/Normal', 'un_highlight': 'misc/fonts/Un/1024/Podswietl',
         'un_big': 'misc/fonts/Un/1024/big', 'sub': 'misc/fonts/sub/1024'}
LOCALE = ('inv_panel', 'char_panel', 'player_panel', 'char_create_menu', 'gameai', 'shell')


def glyph_char(stem):
    if len(stem) == 1 and stem.isdigit():
        return stem
    if len(stem) == 2 and stem[0] == 'D' and stem[1].isupper():
        return stem[1]
    if len(stem) == 2 and stem[0] == 'm' and stem[1].islower():
        return stem[1]
    return SPECIAL.get(stem)


def keyed(image):
    """LithTech draws these PCX surfaces with black as the transparent colour."""
    rgba = image.convert('RGB')
    r, g, b = rgba.split()
    rgba.putalpha(ImageChops.lighter(ImageChops.lighter(r, g), b).point(lambda v: 255 if v else 0))
    return rgba


def export(game, output):
    game, output = Path(game), Path(output)
    if output.resolve().is_relative_to(game.resolve()):
        raise ValueError('refusing to write inside the retail installation')
    output.mkdir(parents=True, exist_ok=True)

    def save(image, relative):
        target = output / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        image.save(target)
        return relative

    keys = {}
    for line in read_script(game / 'scripts/text_keys.txt').splitlines():
        if line.startswith('>'):
            key, _, value = line.partition(' ')
            keys[key] = value.strip()
    text = {}
    for name in LOCALE:
        for line in (game / 'scripts/locale' / f'{name}.txt').read_text(encoding='cp1250').splitlines():
            key = line.strip()
            if key.startswith('>'):
                text[key[1:]] = keys.get(key, key[1:])

    panel = {}
    for source in sorted((game / 'misc/panel_l').glob('*.pcx')):
        with Image.open(source) as image:
            panel[source.stem] = {'image': save(image.convert('RGBA'), f'ui/panel/{source.stem}.png'),
                                  'keyed': save(keyed(image), f'ui/panel/{source.stem}_keyed.png'),
                                  'width': image.width, 'height': image.height,
                                  'source': source.relative_to(game).as_posix()}
    with Image.open(game / 'misc/menu/cursor1024.pcx') as image:
        panel['cursor1024'] = {'image': save(keyed(image), 'ui/panel/cursor1024_keyed.png'),
                               'keyed': 'ui/panel/cursor1024_keyed.png', 'width': image.width,
                               'height': image.height, 'source': 'misc/menu/cursor1024.pcx'}

    fonts, warnings = {}, []
    for name, folder in FONTS.items():
        glyphs = {}
        for source in sorted((game / folder).glob('*.pcx')):
            char = glyph_char(source.stem)
            if char is None or char in glyphs:
                continue
            try:
                with Image.open(source) as image:
                    glyphs[char] = {'image': save(keyed(image), f'ui/fonts/{name}/{source.stem}.png'),
                                    'width': image.width, 'height': image.height}
            except OSError as error:  # one shipped glyph is truncated; retail cannot draw it either
                warnings.append(f'{source.relative_to(game).as_posix()}: {error}')
        fonts[name] = {'height': max(g['height'] for g in glyphs.values()), 'glyphs': glyphs,
                       'source': folder}

    catalog = blocks(read_script(game / 'scripts/items.txt'), 'item')
    items = {}
    for name, records in catalog.items():
        values = {}
        for key, value in records:
            values[key] = value  # later duplicate records override, as in the retail parser
        def number(key, default=0.0):
            try:
                return float(values.get(key, default))
            except ValueError:
                return default
        def localized(key):
            value = values.get(key, '')
            return keys.get(value, keys.get('>' + value, value))
        icon = values.get('icon1024', '').replace('\\', '/')
        icon_path = None
        if icon and (game / icon).is_file():
            with Image.open(game / icon) as image:
                icon_path = save(keyed(image), f'ui/items/{Path(icon).stem}.png')
        pickup = Path(values.get('pickup_icon', '').replace('\\', '/')).stem
        items[name] = {
            'title': localized('title') or name, 'description': localized('descr'),
            'weight': number('weight'), 'health': number('health'), 'alcohol': number('alcohol'),
            'power_up': number('power_up'), 'pain_killer': number('pain_killer'),
            'eaten': 'eaten' in values, 'fixed': 'nie_ruszaj' in values,
            'weapon': 'weapon' in values, 'grenade': 'grenade' in values, 'melee': 'melee' in values,
            'ammo_for': int(number('ammo_for', -1)), 'amount': int(number('amount')),
            'ammo_index': int(number('ammo_index', -1)), 'ammo_amount': int(number('ammo_amount')),
            'experience_index': int(number('experience_index', -1)) if 'experience_index' in values else None,
            'icon': icon_path, 'pickup_icon': f'hud/{pickup}.png' if pickup and (output / f'hud/{pickup}.png').is_file() else None,
        }

    data = {'format': 'mesterlovesz-retail-inventory-v1', 'canvas': [1024, 768],
            'items': items, 'panel': panel, 'fonts': fonts, 'text': text, 'warnings': warnings,
            'evidence': 'Art, glyphs, strings and item values are converted from the retail files; '
                        'layout and rules are documented in docs/retail-inventory.md.'}
    (output / 'retail_inventory.json').write_text(json.dumps(data, ensure_ascii=False, indent=1), encoding='utf-8')
    print(f'Exported {len(items)} items, {len(panel)} panel images, {sum(len(f["glyphs"]) for f in fonts.values())} glyphs')
    return data


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('game', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    export(args.game, args.output)
