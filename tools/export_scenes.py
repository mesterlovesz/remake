"""Export what the retail cutscene controller and the end-game sequence need beyond opening.json.

* actors.json gains a `definition` per scene character (head model, weapon, phases with their
  commands) from the same postacie.txt reader the level exporter uses; the controller creates
  the characters through the character manager (cshell.dll 0x100130d0), so they keep heads,
  weapons and phase commands such as strzal_raz.
* endgame.json / ui/outro/*.png: c_outromgr slides (outro1..4, credits) with their locale text
  (cshell.dll 0x10031a40..0x100323a0), read exactly as the game reads them.
* scenes-audit.json: every model, animation, socket, sound and speech file a scene names.

Usage: python -m tools.export_scenes ../GYARI output   (inputs are read only)
"""
from pathlib import Path
import argparse, json, shutil

from PIL import Image

from .decode_scripts import read_script
from .export_gameplay import Assets, character_definition, records, sections, unique, write_json
from .export_presentation import blocks, opening_timeline

# c_outromgr loads at most this many lines of credits.txt (cmp ebx, 0x3f at cshell.dll 0x100321a7).
CREDITS_LINE_LIMIT = 63
SLIDES = (('outro1', 'outro1', 4, 540), ('outro2', 'outro2', 4, 540), ('outro3', 'outro3', 4, 540))


def text_keys(game):
    keys = {}
    for line in read_script(game / 'scripts/text_keys.txt').splitlines():
        if line.startswith('>'):
            key, _, value = line.partition(' ')
            keys[key] = value.strip()
    return keys


def locale_lines(path, keys, limit):
    """fgets-style: one entry per line (blank lines kept), a lone >key is replaced by its text."""
    raw = path.read_bytes().decode('cp1250').replace('\r\n', '\n').split('\n')
    if raw and raw[-1] == '':
        raw.pop()
    return [keys.get(line.strip(), line.strip()) for line in raw[:limit]]


def export_endgame(game, output, keys):
    images = {}
    for stem in ('outro1', 'outro2', 'outro3', 'outro4', 'credits', 'credits_l'):
        source = game / 'misc/outro' / f'{stem}_1024.pcx'
        target = output / 'ui/outro' / f'{stem}_1024.png'
        target.parent.mkdir(parents=True, exist_ok=True)
        Image.open(source).convert('RGB').save(target)
        images[stem] = f'ui/outro/{stem}_1024.png'
    locale = game / 'scripts/locale'
    data = {
        'format': 'mesterlovesz-endgame-v1',
        'evidence': 'cshell.dll c_outromgr 0x10031a00..0x10032756; music sounds/muza/credits.wav',
        'images': images,
        'slides': [
            {'image': images['outro1'], 'lines': locale_lines(locale / 'outro1.txt', keys, 4), 'y': 540, 'seconds': 10.0},
            {'image': images['outro2'], 'lines': locale_lines(locale / 'outro2.txt', keys, 4), 'y': 540, 'seconds': 10.0},
            {'image': images['outro3'], 'lines': locale_lines(locale / 'outro3.txt', keys, 4), 'y': 540, 'seconds': 10.0}],
        'credits': {'image': images['credits'], 'image_large': images['credits_l'],
                    'lines': locale_lines(locale / 'credits.txt', keys, CREDITS_LINE_LIMIT), 'scroll_speed': 16.0},
        'last': {'image': images['outro4'], 'lines': locale_lines(locale / 'outro4.txt', keys, 16), 'y': 140, 'seconds': 20.0},
    }
    write_json(output / 'endgame.json', data)
    return data


def export_actor_definitions(game, output, timelines):
    decoded = {name: read_script(game / f'scripts/{name}.txt') for name in ('postacie', 'items')}
    catalog = unique(sections(records(decoded['postacie']), 'postac')[1], 'postacie.txt')
    items = unique(sections(records(decoded['items']), 'item')[1], 'items.txt')
    assets = Assets(game, output)
    actors = json.loads((output / 'actors.json').read_text(encoding='utf-8'))
    kinds = {kind for phases in timelines.values() for phase in phases for key, kind in phase['settings'].items() if key.startswith('postac')}
    problems = []
    for kind in sorted(kinds):
        if kind not in catalog:
            problems.append(f'{kind}: not in postacie.txt')
            continue
        try:
            definition = character_definition(catalog[kind], assets, items)
        except Exception as error:  # a scene character with a broken definition is reported, not fatal
            problems.append(f'{kind}: {error}')
            continue
        slim = {key: definition[key] for key in ('default_phase', 'weapon', 'weapon_asset', 'head_asset', 'header', 'phases', 'flags', 'hp')}
        actors.setdefault(kind, {})['definition'] = slim
    write_json(output / 'actors.json', actors)
    return kinds, problems + assets.warnings


def audit(game, output, timelines, kinds, problems):
    report = {'scenes': {}, 'problems': list(problems)}
    for name, phases in timelines.items():
        entry = {'phases': len(phases), 'seconds': round(sum(p['duration'] for p in phases), 3), 'missing': []}
        scene_models = {}
        for phase in phases:
            model_path = output / f'{phase["model"]}.json'
            if phase['model'] not in scene_models:
                scene_models[phase['model']] = json.loads(model_path.read_text(encoding='utf-8')) if model_path.is_file() else None
            model = scene_models[phase['model']]
            if model is None:
                entry['missing'].append(f'model {phase["model"]}')
                continue
            animation = phase['phase'].get('anim')
            if animation and animation not in model['animations']:
                entry['missing'].append(f'{phase["id"]}: animation {animation}')
            for key, value in phase['phase'].items():
                if key.startswith('socket_') and key != 'socket_glos' or key == 'socket_glos':
                    socket = phase['settings'].get(f'socket{value}')
                    if socket is None or socket not in model['sockets']:
                        entry['missing'].append(f'{phase["id"]}: {key} {value} -> {socket}')
            for field in ('sound', 'speech'):
                if phase[field] and not (output / 'audio' / phase[field]).is_file():
                    entry['missing'].append(f'{phase["id"]}: {field} {phase[field]}')
        report['scenes'][name] = entry
    write_json(output / 'scenes-audit.json', report)
    return report


def export(game, output):
    game, output = Path(game), Path(output)
    scenes = blocks(read_script(game / 'scripts/scenki.txt'), 'scena')
    timelines = {name: opening_timeline(game, name) for name in scenes}
    keys = text_keys(game)
    data = export_endgame(game, output, keys)
    kinds, problems = export_actor_definitions(game, output, timelines)
    report = audit(game, output, timelines, kinds, problems)
    print(f'Exported {len(data["slides"])+2} end-game screens, {len(kinds)} scene characters, {sum(len(s["missing"]) for s in report["scenes"].values())} missing references')
    for name, entry in report['scenes'].items():
        print(f'  {name}: {entry["phases"]} phases, {entry["seconds"]} s, missing {entry["missing"]}')
    for problem in report['problems']:
        print('  problem:', problem)
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('game', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    export(args.game, args.output)
