"""Export original Hungarian menu/loading art and bitmap-font text, read-only inputs.

Usage: python -m tools.export_menu ../GYARI output
Requires Pillow for the retail PCX images. Text uses the shipped Mincho alpha
atlas, including its CP1250 character table; no replacement system fonts.
"""
import argparse
import json
from pathlib import Path

from PIL import Image

from .decode_scripts import read_script
from .export_visual import write_dtx_png


def export(game, output):
    output.mkdir(parents=True, exist_ok=True)
    def save(image, relative):
        target = output / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        image.save(target)
        return relative

    images = {}
    for folder, target in [('menu_l', 'menu'), ('loading_l', 'loading')]:
        for source in sorted((game / 'misc' / folder).glob('*.pcx')):
            relative = f'ui/{target}/{source.stem}.png'
            image = Image.open(source).convert('RGBA')
            save(image, relative)
            images[source.relative_to(game).as_posix()] = {
                'image': relative, 'width': image.width, 'height': image.height}

    atlas_path = 'ui/fonts/mincho.png'
    write_dtx_png(game / 'misc/fonts/Mincho/table.dtx', output / atlas_path,
                  preserve_zero_alpha=True)
    atlas = Image.open(output / atlas_path).convert('RGBA')
    table = (game / 'misc/fonts/Mincho/table.txt').read_text(encoding='cp1250')
    glyphs = {}
    for row, line in enumerate(table.splitlines()):
        for column, char in enumerate(line):
            glyphs[char] = {'rect': [column*32, row*64, 32, 64], 'advance': 32}

    def text_image(text, name, height=32, color=(220, 220, 220)):
        # Keep the original 32x64 character cells and authored alpha. The
        # explicit metadata lets a renderer change display scale losslessly.
        image = Image.new('RGBA', (max(1, len(text))*32, 64))
        for index, char in enumerate(text):
            if char not in glyphs:
                raise ValueError(f'Missing retail glyph {char!r} in {text!r}')
            x, y, width, h = glyphs[char]['rect']
            glyph = atlas.crop((x, y, x+width, y+h))
            colored = Image.new('RGBA', glyph.size, (*color, 255))
            colored.putalpha(glyph.getchannel('A'))
            image.alpha_composite(colored, (index*32, 0))
        relative = save(image, f'ui/text/{name}.png')
        return {'image': relative, 'width': image.width*height/64,
                'height': height, 'text': text}

    keys = {}
    for line in read_script(game / 'scripts/text_keys.txt').splitlines():
        if line.startswith('>'):
            key, _, value = line.partition(' ')
            keys[key] = value.strip()
    buttons = []
    entries = [('new', 'StartGame'), ('load', 'LoadGame'), ('save', 'SaveGame'),
               ('options', 'MiscOptions'), ('controls', 'Controls'),
               ('display', 'PerfAndDisplay'), ('audio', 'SoundOptions'),
               ('exit', 'QuitGame')]
    for index, (identifier, suffix) in enumerate(entries):
        label = keys['>MainMenu'+suffix]
        normal = text_image(label, identifier)
        hover = text_image(label, identifier+'_hover', color=(255, 230, 40))
        # cshell.dll 0x10027aa0: cream (199,194,158), hovered (255,224,0) and the drifting
        # ghost copy (71,66,54); the list starts at x=340, y=240+17 with a 52-pixel pitch.
        retail = {'label_image': text_image(label, identifier+'_label', color=(199, 194, 158))['image'],
                  'label_hover_image': text_image(label, identifier+'_label_hover', color=(255, 224, 0))['image'],
                  'ghost_image': text_image(label, identifier+'_ghost', color=(71, 66, 54))['image'],
                  'retail_x': 340, 'retail_y': 257+52*index}
        buttons.append({'id': identifier, **normal, 'hover_image': hover['image'],
                        'x': 338, 'y': 267+52*index, **retail})

    loading = {}
    world = None
    for line in read_script(game / 'scripts/ai/gameai.txt').splitlines():
        parts = line.strip().split(maxsplit=1)
        if len(parts) != 2:
            continue
        key, value = parts
        if key == 'level':
            world = value.replace('\\', '/').removeprefix('worlds/').lower()
        elif key == 'load_c' and world:
            loading[world] = {'image': f'ui/loading/{Path(value).stem}.png'}
        elif key == 'load_d' and world in loading:
            title = keys.get(value, value)
            label = text_image(title, 'loading_'+world, height=32, color=(255,255,255))
            loading[world].update(title=title, title_image=label['image'],
                                  title_width=label['width'], title_height=label['height'])

    data = {'format': 'mesterlovesz-retail-ui-v1', 'canvas': [1024, 768],
            'main_background': 'ui/menu/main_menu_1024.png',
            'menu_title': {**text_image(keys['>MainMenuHeader'], 'main_title', height=40),
                           'red_image': text_image(keys['>MainMenuHeader'], 'main_title_red', height=32, color=(255, 0, 0))['image']},
            'menu_ghost': {'scale': 0.35, 'ghost_scale': 1.2, 'x_amplitude': 0.1, 'y_amplitude': 0.125, 'pitch': 52, 'base_y': 240, 'evidence': 'cshell.dll 0x10027aa0; clock 0x1002ae45 (t += dt, wraps at 2*pi)'},
            'buttons': buttons, 'loading': loading,
            'status': {'world': text_image(keys['>Loading1'], 'status_world', height=24, color=(255,255,255)),
                       'assets': text_image(keys['>Loading3'], 'status_assets', height=24, color=(255,255,255))},
            'font': {'image': atlas_path, 'cell_size': [32,64], 'glyphs': glyphs},
            'source_images': images,
            'localization': keys,
            'layout_evidence': 'Menu button positions scaled from supplied 960x720 reference; original art and strings from retail files. Runtime text size and hover tint are reconstruction parameters, not recovered engine constants.',
            'loading_layout': {'image_size': [640,480], 'image_anchor': 'center',
                               'background': 'black', 'loading_word_baked_in_image': True}}
    (output / 'retail_ui.json').write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding='utf-8')
    print(f'Exported {len(images)} original PCX images, {len(glyphs)} font glyphs, {len(loading)} world loading screens')
    return data


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('game', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    export(args.game, args.output)
