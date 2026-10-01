"""Export original carried weapons and authored LTB clips without changing retail files.

The `automatic` convenience field means primary-fire repeats while held. Retail
does this for every non-grenade weapon, including pistols; it is not inferred
from a real-world gun category. See docs/retail-weapons-export.md for addresses.
"""
from pathlib import Path
import argparse
import re
import struct

from .decode_scripts import read_script
from .export_gameplay import (Assets, commands, fields, provenance, records,
                              sections, text_key_map, unique, write_json)


def numbered(values, prefix):
    return [value for _, value in sorted(
        (int(key[len(prefix):]), value) for key, value in values.items()
        if re.fullmatch(re.escape(prefix) + r'\d+', key))]


def weapon_definition(group, keys, assets):
    values = fields(group['records'])
    model = assets.model(values['carried_mesh'])
    animations = {
        'base': values.get('anim_base', ''),
        'shoot': numbered(values, 'anim_shoot'),
        'reload': numbered(values, 'anim_reload'),
        'alt_shoot': numbered(values, 'anim_altshoot'),
        'to_alt': values.get('anim_doalt', ''),
        'from_alt': values.get('anim_odalt', ''),
        'hold': values.get('anim_trzyma', ''),
    }
    missing_animations = []
    for clips in animations.values():
        for clip in clips if isinstance(clips, list) else [clips]:
            if clip and clip not in model['animations']:
                missing_animations.append(clip)
                assets.warnings.append(f'{group["name"]}: original script references absent animation {clip!r}')

    # FN shotgun accidentally has `descr >IdShotgunT` instead of `title`.
    # Resolve its original title text while retaining both unmodified records.
    title_key = values.get('title', '')
    if not title_key:
        title_key = next((value for key, value, _ in group['records']
                          if key == 'descr' and value.endswith('T') and value in keys), '')
    skins = {key[len('carried_tex'):]: assets.texture(value)
             for key, value in values.items() if re.fullmatch(r'carried_tex\d+', key)}
    sounds = {name: assets.sound(values.get(source, '')) for name, source in (
        ('shoot', 'sound_shoot'), ('reload', 'sound_reload'), ('hit', 'hit_sound'),
        ('pickup', 'pickup_sound'), ('alt_on', 'alt_sound_on'), ('alt_off', 'alt_sound_off'))}
    return {
        'id': group['name'], 'title': keys.get(title_key, group['name']),
        'model': model['model'], 'skins': skins,
        'hud': assets.texture(values['hud']) if 'hud' in values else None,
        'offset': [float(values.get('przes_' + axis, 0)) for axis in ('right', 'up', 'forward')],
        'scale': [float(values.get('skala_' + axis, 1)) for axis in 'xyz'],
        'melee': 'melee' in values, 'grenade': 'grenade' in values,
        'shotgun': 'shotgun' in values, 'pellets': int(values.get('kul_na_raz', 1)),
        'grenade_physics': {'fuse_seconds': 5.0, 'thrown_fuse_seconds': 3.0,
                           'throw_speed': 640.0, 'throw_up': 256.0, 'gravity': 640.0,
                           'dimensions': [8.0, 4.0, 8.0], 'model_scale': 2.0,
                           'blast_radius': 640.0, 'npc_max_damage': 500.0,
                           'player_max_damage': 100.0,
                           'ground_friction_numerator': 30.0, 'ground_friction_scale': 630.0,
                           'ground_friction_min_dt': 0.05, 'bounce_threshold': -16.0,
                           'bounce_y': -0.3, 'bounce_all': 0.65}
                           if 'grenade' in values else None,
        'automatic': 'grenade' not in values,
        'player_selectable': group['name'] != 'heli_bron',
        'ammo_index': int(values.get('ammo_index', -1)),
        'ammo_amount': int(values.get('ammo_amount', 0)),
        'capacity': int(values.get('max_ammo', 0)),
        'damage': float(values.get('sila_strzalu', 0)),
        'shot_latency': float(values.get('shot_latency', 0)),
        'spread': float(values.get('rozrzut', 0)),
        'experience_index': int(values.get('experience_index', 0)),
        'animations': animations,
        'missing_animations': missing_animations,
        'durations': {name: info['duration'] for name, info in model['animations'].items()},
        'animation_events': {name: info['events'] for name, info in model['animations'].items()},
        'sounds': sounds, 'commands': commands(group['records']),
        'source': {'items_line': group['source_line'],
                   'model': provenance(assets.game / model['model'], assets.game)},
    }


def pickup_definition(group, keys, assets):
    """Same schema as items.json, for every native gun/ammo including NPC drops."""
    values = fields(group['records'])
    model = assets.model(values['mesh'])
    sound = assets.sound(values.get('pickup_sound', ''))
    return {
        'model': model['model'],
        'skins': {key[3:]: assets.texture(value) for key, value in values.items()
                  if re.fullmatch(r'tex\d+', key)},
        'animation': next(iter(model['animations'])),
        'title': keys.get(values.get('title', values.get('descr', '')), group['name']),
        'health': float(values.get('health', 0)),
        'ammo': int(values.get('amount', values.get('ammo_amount', 0))),
        'ammo_for': int(values.get('ammo_for', values.get('ammo_index', -1))),
        'weapon': 'weapon' in values, 'scale': float(values.get('scale', 1)),
        # Historical pickup schema resolves audio/ at playback time.
        'sound': sound.removeprefix('audio/') if sound else '',
        'commands': commands(group['records']),
    }


def sprite_layer(assets, path, start, duration, scale):
    """Read the observed retail SPR header and ordered length-prefixed DTX paths."""
    data = (assets.game / path).read_bytes()
    if len(data) < 20:
        raise ValueError(f'truncated sprite header: {path}')
    count, fps, *flags = struct.unpack_from('<5I', data)
    if not 0 < count <= 1000 or not 0 < fps <= 1000:
        raise ValueError(f'invalid sprite frame count/rate: {path}')
    offset, frames, sources = 20, [], []
    for _ in range(count):
        if offset + 2 > len(data):
            raise ValueError(f'truncated sprite frame length: {path}')
        length = struct.unpack_from('<H', data, offset)[0]
        offset += 2
        if offset + length > len(data):
            raise ValueError(f'truncated sprite frame path: {path}')
        frame_path = data[offset:offset + length].decode('cp1250')
        offset += length
        frame = assets.texture(frame_path)
        frames.append(frame)
        sources.append(provenance(assets.game / frame_path.replace('\\', '/'), assets.game))
    if offset != len(data):
        raise ValueError(f'unparsed sprite trailing bytes: {path}')
    width, height = struct.unpack_from('>II', (assets.output / frames[0]).read_bytes(), 16)
    return {'sprite': path, 'frames': frames, 'width': width, 'height': height,
            'fps': fps, 'flags': flags, 'start': start, 'duration': duration, 'scale': scale,
            'source': provenance(assets.game / path, assets.game), 'frame_sources': sources}


def export(game, output):
    game, output = Path(game), Path(output)
    if output.resolve() == game.resolve() or game.resolve() in output.resolve().parents:
        raise ValueError('output must be outside the original game directory')
    source_names = ('scripts/items.txt', 'scripts/text_keys.txt')
    decoded = {name: read_script(game / name) for name in source_names}
    _, groups = sections(records(decoded['scripts/items.txt']), 'item')
    groups = unique(groups, 'scripts/items.txt')
    keys = text_key_map(decoded['scripts/text_keys.txt'])
    assets = Assets(game, output)
    weapons = [weapon_definition(group, keys, assets) for group in groups.values()
               if 'weapon' in fields(group['records']) and 'carried_mesh' in fields(group['records'])]
    # Skill ordering from original character HUD: baton, experience 1..8, grenade.
    # Heli's NPC-only definition is retained after player-selectable weapons.
    weapons.sort(key=lambda weapon: (not weapon['player_selectable'], weapon['grenade'], weapon['experience_index']))
    selectable = {weapon['id'] for weapon in weapons if weapon['player_selectable']}
    pickups = {name: pickup_definition(group, keys, assets) for name, group in groups.items()
               if name in selectable or 'ammo_for' in fields(group['records'])}
    write_json(output / 'retail_items.json', pickups)
    grenade_effect = {
        'sound': assets.sound('sounds/weapons/rock_lup.wav'),
        'impact_sound': assets.sound('sounds/weapons/grt_ryko.wav'),
        'layers': [sprite_layer(assets, 'sprites/weapons/systemblikwybuch.spr', 0.0, 0.45, 4.0),
                   sprite_layer(assets, 'sprites/weapons/systemwybduzy1.spr', 0.1, 1.45, 1.1)],
    }
    result = {
        'schema_version': 1,
        'coordinate_system': 'Native LithTech model coordinates; scales and camera offsets unmodified',
        'sources': [provenance(game / name, game) for name in (*source_names, 'cshell.dll')],
        'weapons': weapons,
        'grenade_effect': grenade_effect,
        'warnings': assets.warnings,
        'behavior_evidence': {
            'primary_held': 'cshell.dll 0x10060c9a..0x10060ccf -> 0x10003f90; no edge latch',
            'shot_latency': 'cshell.dll 0x1000400d..0x1000401e; item +0x8fc',
            'shoot_sequence': 'cshell.dll 0x1000f442..0x1000f4b5; anim_shoot0 then anim_shoot1 then base',
            'offsets': 'cshell.dll 0x10010635..0x100106c5; forward/right/up in camera basis',
            'grenade_physics': 'cshell.dll 0x1000ad40..0x1000b0fa: 5s in-hand limit after pin, release throw, forward640 + up256; type7 constructor0x1004f2c7 resets thrown fuse to3s. Gravity/bounce0x10051e50..0x1005213c.',
            'grenade_blast': 'cshell.dll NPC0x10043180 radius640 max500 linear with LOS; player0x1005fd20 radius640 max100 linear with LOS.',
            'grenade_effect': 'cshell.dll 0x10053ab0..0x10053c21: original flash sprite, explosion sprite, rock_lup sound; secondary debris/sparks not exported.',
        },
    }
    write_json(output / 'retail_weapons.json', result)
    print(f'{len(weapons)} original carried weapons, {len(assets.models)} models, '
          f'{len(assets.textures)} textures/HUD panels, {len(assets.audio)} sounds', flush=True)
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--game', type=Path, default=Path('../GYARI'))
    parser.add_argument('--output', type=Path, default=Path('output'))
    args = parser.parse_args()
    export(args.game, args.output)
