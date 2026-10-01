"""Export original placed NPCs, models, script graphs and dialogue assets read-only."""
from pathlib import Path, PurePosixPath
from hashlib import sha256
import argparse
import json
import re
import shutil
import struct

from .decode_scripts import decode_text, read_script
from .export_visual import write_dtx_png
from .lithtech_dat import Reader
from .ltb import read_ltb


DEFAULT_WORLDS = ('rh1-wiezienie1', 'rh1-wiezienie2', 'rh1-wiezienie3')
# Characters the client spawns itself: the chapel's Special1 detector creates "laska czapel" behind the player (cshell.dll 0x1002c5d0).
CLIENT_SPAWNED = {'chapel_mniejszy': ('laska czapel',)}


def records(text):
    """Retain order, repeated commands and source line numbers; ignore comments only."""
    result = []
    for number, raw in enumerate(text.splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith('//'):
            continue
        fields = line.split(None, 1)
        result.append((fields[0], fields[1] if len(fields) > 1 else '', number))
    return result


def sections(lines, keyword):
    header, groups, current = [], [], None
    for key, value, line in lines:
        if key == keyword:
            current = {'name': value, 'source_line': line, 'records': []}
            groups.append(current)
        elif current is None:
            header.append((key, value, line))
        else:
            current['records'].append((key, value, line))
    return header, groups


def commands(lines):
    return [[key, value] for key, value, _ in lines]


def fields(lines):
    # Convenience view only. The ordered command array is always exported too.
    return {key: value for key, value, _ in lines}


def unique(groups, source):
    result = {}
    for group in groups:
        if group['name'] in result:
            raise ValueError(f'duplicate block {group["name"]!r} in {source}')
        result[group['name']] = group
    return result


def relative_path(value):
    path = PurePosixPath(value.replace('\\', '/'))
    if path.is_absolute() or '..' in path.parts or ':' in str(path):
        raise ValueError(f'asset path outside game directory: {value!r}')
    return path.as_posix()


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, allow_nan=False, separators=(',', ':')) + '\n', encoding='utf-8')


def provenance(path, root):
    return {'path': path.relative_to(root).as_posix(), 'sha256': sha256(path.read_bytes()).hexdigest()}


def object_property_metadata(path):
    """Recover the original property type/flag words omitted from scene-v1 JSON."""
    reader = Reader(path.read_bytes())
    version, objects_at = struct.unpack_from('<II', reader.data)
    if version not in (83, 85):
        raise ValueError(f'unsupported DAT version {version}')
    reader.position = objects_at
    result = []
    for _ in range(reader.read('<I')):
        reader.read('<H')
        kind = reader.string('<H')
        metadata = {}
        for _ in range(reader.read('<I')):
            name = reader.string('<H')
            property_type, flags, size = reader.read('<BIH')
            reader.bytes(size)
            metadata[name] = {'type': property_type, 'flags': flags}
        result.append((kind, metadata))
    return result


class Assets:
    def __init__(self, game, output):
        self.game, self.output = game, output
        self.models, self.textures, self.audio = {}, {}, set()
        self.warnings = []

    def model(self, value):
        path = relative_path(value)
        if path not in self.models:
            model = read_ltb(self.game / path)
            vertices = [position for piece in model['pieces'] for position in piece['positions']]
            model['bounds'] = {'min': [min(p[axis] for p in vertices) for axis in range(3)],
                               'max': [max(p[axis] for p in vertices) for axis in range(3)]} if vertices else None
            write_json(self.output / f'{path}.json', model)
            self.models[path] = {
                'model': path, 'asset': f'{path}.json',
                'source_sha256': sha256((self.game / path).read_bytes()).hexdigest(),
                'bones': len(model['nodes']), 'pieces': len(model['pieces']),
                'bounds': model['bounds'],
                'animation_names': list(model['animations']),
                'animations': {name: {'duration': max(animation['times'], default=0.0),
                                     'translation': animation['translation'],
                                     'dimensions': animation['dimensions'],
                                     'events': animation['events']}
                               for name, animation in model['animations'].items()},
                'sockets': model['sockets'], 'children': model['children'],
            }
            for child in model['children']:
                self.model(child)
            print(f'Model {path}: {len(model["nodes"])} bones, {len(model["animations"])} animations', flush=True)
        return self.models[path]

    def texture(self, value):
        path = relative_path(value)
        if not PurePosixPath(path).suffix:
            path += '.dtx'
        target = f'model_textures/{path}.png'
        if path not in self.textures:
            write_dtx_png(self.game / path, self.output / target)
            self.textures[path] = target
        return target

    def sound(self, value):
        if not value or not value.lower().startswith(('sounds\\', 'sounds/')):
            return None
        path = relative_path(value)
        source = self.game / path
        if not source.is_file():
            self.warnings.append(f'missing sound: {path}')
            return None
        target = f'audio/{path}'
        if path not in self.audio:
            (self.output / target).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, self.output / target)
            self.audio.add(path)
        return target


def character_definition(group, assets, item_catalog):
    header, phases = sections(group['records'], 'faza')
    values = fields(header)
    model_path = relative_path(values['model'])
    model = assets.model(model_path)
    phase_map = {}
    for name, phase in unique(phases, group['name']).items():
        phase_fields = fields(phase['records'])
        animation = phase_fields.get('animacja')
        if animation and animation not in model['animation_names']:
            assets.warnings.append(f'{group["name"]}/{name}: animation {animation!r} absent in {model_path}')
        animation_info = model['animations'].get(animation)
        phase_map[name] = {'animation': animation, 'animation_available': animation_info is not None,
                           'duration': animation_info['duration'] if animation_info else None,
                           'dimensions': animation_info['dimensions'] if animation_info else None,
                           'loop': 'loop' in phase_fields,
                           'commands': commands(phase['records']), 'source_line': phase['source_line']}
        for _, value, _ in phase['records']:
            if value.lower().endswith('.wav'):
                assets.sound(value)
    default_phase = values.get('default_faza')
    default = phase_map.get(default_phase, {})
    if not default:
        raise ValueError(f'missing default phase {group["name"]}/{default_phase}')
    skins = {key[4:]: assets.texture(value) for key, value in values.items()
             if key.startswith('skin') and key[4:].isdigit()}
    variants = {}
    variant_count = int(values.get('ile_skinow', '0'))
    for index in range(variant_count):
        variant = {}
        for key, value in values.items():
            if key.startswith('skin') and key[4:].isdigit():
                path = PurePosixPath(relative_path(value))
                variant_path = path.with_name(re.sub(r'\d+$', str(index), path.stem) + (path.suffix or '.dtx'))
                if (assets.game / variant_path).is_file():
                    variant[key[4:]] = assets.texture(str(variant_path))
        if variant:
            variants[str(index)] = variant
    weapon = values.get('weapon')
    weapon_asset = None
    if weapon in item_catalog:
        item = fields(item_catalog[weapon]['records'])
        if 'mesh' in item:
            weapon_model = assets.model(item['mesh'])
            # Original NPC gunfire; no synthesized or first-person replacement audio.
            if item.get('sound_shoot'):
                assets.sound(item['sound_shoot'])
            weapon_asset = {'model': weapon_model['model'],
                            'animation': weapon_model['animation_names'][0] if weapon_model['animation_names'] else None,
                            'skins': {key[3:]: assets.texture(value) for key, value in item.items()
                                      if key.startswith('tex') and key[3:].isdigit()},
                            'styles': {key[2:]: value for key, value in item.items()
                                       if key.startswith('rs') and key[2:].isdigit()},
                            'socket': values.get('socket_weapon'),
                            'commands': commands(item_catalog[weapon]['records'])}
    # glowa_specjalna characters (mayor, bartenders, Stella, the sergeant) carry their head as a separate model on the body's head socket.
    head_asset = None
    if values.get('model_glowa') and values.get('socket_glowa'):
        head = assets.model(values['model_glowa'])
        head_asset = {'model': head['model'], 'animation': head['animation_names'][0] if head['animation_names'] else None,
                      'skins': {'0': assets.texture(values['skin_glowa'])} if values.get('skin_glowa') else {},
                      'styles': {}, 'socket': values['socket_glowa'], 'commands': []}
    return {
        'definition_name': group['name'], 'source_line': group['source_line'],
        'model': model_path, 'model_asset': model['asset'], 'skins': skins,
        'skin_variants': variants, 'source_skin_variant_count': variant_count,
        'styles': {key[2:]: value for key, value in values.items() if key.startswith('rs') and key[2:].isdigit()},
        'default_phase': default_phase, 'animation': default['animation'],
        'animation_available': default['animation_available'], 'bounds': model['bounds'],
        'collision_half_extents': default['dimensions'],
        'model_animations': model['animations'],
        'loop': default['loop'], 'hp': float(values['HP']) if 'HP' in values else None,
        'hostile': 'hostile' in values, 'faction': values.get('faction'),
        'weapon': weapon, 'weapon_asset': weapon_asset, 'head_asset': head_asset,
        'flags': [key for key, value, _ in header if not value],
        'header': commands(header), 'phases': phase_map,
    }


def text_key_map(text):
    return {key: value for key, value, _ in records(text) if key.startswith('>')}


def dialogue_definition(group, keys, assets):
    values = fields(group['records'])
    title_key = values.get('title', '')
    speech = keys.get(values.get('titlesnd', ''), values.get('titlesnd', ''))
    choices = []
    for key, value in values.items():
        match = re.fullmatch(r'answer(\d+)', key)
        if match:
            index = match[1]
            answer_sound = keys.get(values.get(f'answer{index}snd', ''), values.get(f'answer{index}snd', ''))
            choices.append({'index': int(index), 'text_key': value, 'text': keys.get(value, value),
                            'speech': relative_path(answer_sound) if answer_sound else None,
                            'audio_asset': assets.sound(answer_sound),
                            'next_dialogue': values.get(f'onchoice{index}dialog')})
    return {'source_line': group['source_line'], 'person': values.get('person'),
            'title_key': title_key, 'title': keys.get(title_key, title_key),
            'speech': relative_path(speech) if speech else None, 'audio_asset': assets.sound(speech),
            'choices': sorted(choices, key=lambda choice: choice['index']), 'commands': commands(group['records'])}


def dialogue_targets(lines):
    for key, value, _ in lines:
        if key == 'dialog' or re.fullmatch(r'onchoice\d+dialog', key):
            if value:
                yield value
        elif key in ('ontimeexceeded', 'onfurtherthan'):
            values = value.split(None, 1)
            if len(values) == 2:
                yield values[1]


def navigation(path):
    if not path.is_file():
        return None
    header, places = sections(records(path.read_text(encoding='cp1250')), 'place')
    nodes = []
    # A .pth holds several independent `path` graphs whose place ids restart at 0;
    # `graph` keys a node as (graph, id). A later `path` line trails the previous place.
    graph = max(sum(key == 'path' for key, _, _ in header) - 1, 0)
    for place in places:
        values = fields(place['records'])
        nodes.append({'id': int(place['name']), 'graph': graph,
                      'pos': [float(values[f'pos{axis}']) for axis in 'xyz'],
                      'real_pos': [float(values[f'realpos{axis}']) for axis in 'xyz'],
                      'neighbors': [int(value) for key, value, _ in place['records'] if re.fullmatch(r'idx\d+', key)],
                      'commands': commands(place['records'])})
        graph += sum(key == 'path' for key, _, _ in place['records'])
    return {'source': path.name, 'sha256': sha256(path.read_bytes()).hexdigest(), 'header': commands(header), 'nodes': nodes}


def export(game, output, worlds=DEFAULT_WORLDS):
    game, output = Path(game), Path(output)
    if game.resolve() == output.resolve() or game.resolve() in output.resolve().parents:
        raise ValueError('output must be outside the original game directory')
    output.mkdir(parents=True, exist_ok=True)
    source_names = {'characters': 'scripts/postacie.txt', 'items': 'scripts/items.txt',
                    'gameai': 'scripts/ai/gameai.txt', 'dialogues': 'scripts/ai/dialogi.txt', 'text_keys': 'scripts/text_keys.txt'}
    decoded = {name: read_script(game / path) for name, path in source_names.items()}
    sources = [provenance(game / path, game) for path in source_names.values()]
    write_json(output / 'gameplay_scripts.json', {**{name: decoded[name] for name in ('gameai', 'dialogues', 'text_keys')}, 'sources': sources})
    (output / 'decoded_scripts').mkdir(exist_ok=True)
    for key, filename in [('gameai', 'gameai.txt'), ('dialogues', 'dialogi.txt')]:
        (output / 'decoded_scripts' / filename).write_text(decoded[key], encoding='utf-8')
    character_catalog = unique(sections(records(decoded['characters']), 'postac')[1], 'postacie.txt')
    items = unique(sections(records(decoded['items']), 'item')[1], 'items.txt')
    global_ai, levels = sections(records(decoded['gameai']), 'level')
    levels = {relative_path(level['name']).removeprefix('worlds/').lower(): level for level in levels}
    all_dialogues = unique(sections(records(decoded['dialogues']), 'dialog')[1], 'dialogi.txt')
    keys = text_key_map(decoded['text_keys'])
    by_model = {}
    for name, group in character_catalog.items():
        header = fields(sections(group['records'], 'faza')[0])
        if 'model' in header:
            by_model.setdefault(relative_path(header['model']).casefold(), []).append(name)
    assets = Assets(game, output)
    definitions = {}
    summaries = []
    for world in worlds:
        scene = json.loads((output / f'{world}.scene.json').read_text(encoding='utf-8'))
        source = game / 'worlds' / f'{world}.dat'
        if sha256(source.read_bytes()).hexdigest() != scene['source_sha256']:
            raise ValueError(f'stale scene export for {world}')
        metadata = object_property_metadata(source)
        if len(metadata) != len(scene['objects']):
            raise ValueError(f'object count mismatch for {world}')
        level_header, actions = sections(levels[world.lower()]['records'], 'action')
        ai = {'global_commands': commands(global_ai), 'header': commands(level_header),
              'actions': [{'name': action['name'], 'source_line': action['source_line'], 'commands': commands(action['records'])} for action in actions]}
        npcs, emitters, markers, interactables = [], [], [], []
        used_definitions = set()
        roots = set(dialogue_targets(levels[world.lower()]['records']))
        for index, obj in enumerate(scene['objects']):
            kind, properties = obj['kind'], obj['properties']
            if kind != metadata[index][0]:
                raise ValueError(f'object class mismatch at {world}:{index}')
            entity = {'name': properties.get('Name'), 'kind': kind, 'source_object_index': index,
                      'pos': properties.get('Pos'), 'rotation': properties.get('Rotation'),
                      'properties': properties, 'property_metadata': metadata[index][1]}
            if kind in ('o_postac', 'o_emiter_postaci'):
                model_path = relative_path(properties['Model'])
                candidates = by_model.get(model_path.casefold(), [])
                if not candidates and model_path.lower().endswith('.lta'):
                    candidates = by_model.get(model_path[:-4].casefold() + '.ltb', [])
                    if candidates: assets.warnings.append(f'{world}/{entity["name"]}: source LTA reference resolved to shipped LTB {model_path}')
                if len(candidates) != 1:
                    raise ValueError(f'{world}/{entity["name"]}: ambiguous character definitions {candidates} for {model_path}')
                name = candidates[0]
                if name not in definitions:
                    definitions[name] = character_definition(character_catalog[name], assets, items)
                definition = definitions[name]
                used_definitions.add(name)
                entity.update({key: definition[key] for key in ('definition_name', 'model', 'skins', 'styles', 'default_phase', 'animation', 'animation_available', 'bounds', 'collision_half_extents', 'loop', 'hp', 'hostile', 'faction', 'weapon', 'flags')})
                entity['initial_state'] = {'phase': definition['default_phase'], 'hp': definition['hp'], 'hostile': definition['hostile']}
                entity['script_links'] = [{'action': action['name'], 'source_line': action['source_line']}
                                          for action in actions if any(value == name or value.endswith(' ' + name) for _, value, _ in action['records'])]
                (npcs if kind == 'o_postac' else emitters).append(entity)
            elif kind.startswith('o_marker'):
                markers.append(entity)
            elif kind.startswith('b_door') or kind.startswith('b_szuflada') or kind in ('o_item_podnoszony', 'o_cutscene'):
                interactables.append(entity)
            roots.update(value for value in properties.values() if isinstance(value, str) and value in all_dialogues)
        for extra in CLIENT_SPAWNED.get(world, ()):
            if extra not in definitions:
                definitions[extra] = character_definition(character_catalog[extra], assets, items)
            used_definitions.add(extra)
        reachable, pending = {}, list(sorted(roots))
        while pending:
            name = pending.pop()
            if name in reachable:
                continue
            group = all_dialogues.get(name)
            if group is None:
                assets.warnings.append(f'{world}: unresolved dialogue {name!r}')
                continue
            reachable[name] = dialogue_definition(group, keys, assets)
            pending.extend(dialogue_targets(group['records']))
        payload = {'format': 'mesterlovesz-gameplay-v1', 'world': world,
                   'coordinate_system': scene['coordinate_system'], 'sources': sources + [provenance(source, game)],
                   'characters': {name: definitions[name] for name in sorted(used_definitions)},
                   'npcs': npcs, 'emitters': emitters, 'markers': markers, 'interactables': interactables,
                   'ai': ai, 'dialogues': reachable, 'navigation': navigation(game / 'worlds' / f'{world}.pth')}
        write_json(output / f'{world}.gameplay.json', payload)
        summaries.append({'world': world, 'npcs': len(npcs), 'emitters': len(emitters),
                          'definitions': len(used_definitions), 'actions': len(actions), 'dialogues': len(reachable)})
        print(json.dumps(summaries[-1]), flush=True)
    # The next prison episode's original lines are useful when crossing the final
    # exported transition. Copy speech assets only; do not synthesize any dialogue.
    for key, value in keys.items():
        if key.startswith(('>Mutiny', '>Fight', '>Run')):
            assets.sound(value)
    report = {'worlds': summaries, 'models': assets.models, 'textures': assets.textures,
              'audio': sorted(assets.audio), 'warnings': sorted(set(assets.warnings))}
    write_json(output / 'gameplay-export.report.json', report)
    print(f'Exported {len(assets.models)} models, {len(assets.textures)} textures, {len(assets.audio)} audio assets; {len(report["warnings"])} warnings', flush=True)
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('game', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--worlds', nargs='+', default=DEFAULT_WORLDS)
    args = parser.parse_args()
    export(args.game, args.output, args.worlds)
