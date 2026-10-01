r"""Systematic inventory of the retail install (GYARI) versus what the remake uses (docs/unused-content.md).

    python -m tools.inventory_unused ../GYARI output            # writes output/unused-inventory.json, prints the summary
    python -m tools.inventory_unused ../GYARI output --list     # also prints every unused / missing row

What it does (read-only on GYARI, only ADDS output/unused-inventory.json):
 1. every file of the install gets a category, a size, the set of things that refer to it and a status
      USED-IN-REMAKE                      live retail reference and the remake's output/ has its exported counterpart
      USED-BY-RETAIL-BUT-MISSING-IN-REMAKE  live retail reference but no exported counterpart / no reader in the remake
      UNUSED-IN-RETAIL                    nothing live refers to it (cut content, unplaced definition, commented out, orphan)
      ENGINE-INTERNAL                     DLL/EXE/engine resource, editor leftovers, directory markers
    "live" = the reference is in a script block that some level places (or that a retail binary / script names), in a world DAT, or
    hard-coded in cshell.dll / object.lto / server.dll / Lithtech.exe.
 2. worlds: DAT/PTH, campaign membership (output/campaign-inventory.json), object counts.
 3. script vocabulary: every key of postacie / items / objects / scenki / gameai / dialogi with its number of uses, whether a retail
    binary contains the keyword and whether the remake's Rust / exporter sources contain it.
 4. definitions never placed / never referenced: characters (postacie.txt), items (items.txt), objects (objects.txt), scenes
    (scenki.txt), dialogues, actions, mission variables, text keys, speech wavs.
 5. object classes: per class and property, instances, levels and whether the remake reads it.
The classification is static and heuristic (string matches); the curated conclusions live in docs/unused-content.md.
"""
import argparse, json, re, sys
from collections import Counter, defaultdict
from pathlib import Path

from .audit_assets import norm, Index, category as asset_category, exported, resolve_retail, TOKEN_RE, VALUE_RE, json_strings, sprite_frames
from .decode_scripts import read_script

BINARIES = ('cshell.dll', 'object.lto', 'server.dll', 'Lithtech.exe')
ENGINE_FILES = {'lithtech.exe', 'cshell.dll', 'object.lto', 'server.dll', 'sndDrv.dll'.lower(), 'cdaudio.dll', 'l3codeca.acm', 'mss32.dll', 'msvcrt.dll', 'msvcp60.dll',
                'msvcirtd.dll', 'msvcrtd.dll', 'mfc42d.dll', 'cres.dll', 'sres.dll', 'ltmsg.dll', 'dx8.snd', 'engine.rez', 'clientfx.fxd', 'autoexec.cfg',
                'launcher.exe', 'play1.exe', 'cipher.exe', 'lanuchp.txt', 'readme.txt', 'classhlp.but', 'sniper.ico'}
# script files the game really reads (cshell strings: scripts\...), everything else under scripts/ is an empty stub / editor leftover
MARKER_FILES = ('dirtypeworlds', 'dirtypemodels', 'dirtypesounds', 'dirtypetextures')
CAMPAIGN_JSON = 'campaign-inventory.json'


def read(path): return Path(path).read_text(encoding='cp1250', errors='replace')


# ------------------------------------------------------------------ script parsing
def parse_blocks(game, rel, block_kw, sub_kw=()):
    """[(line, kind, name, key, value)] for `key value` scripts; block headers (`postac NAME`, `faza X`) give kind/name."""
    rows, kind, name, sub = [], 'HEAD', '', ''
    for number, raw in enumerate(read_script(game / rel).splitlines(), 1):
        line = raw.split('//')[0].strip()
        if not line: continue
        key, _, value = line.partition(' ')
        value = value.strip()
        if key in block_kw: kind, name, sub = key, value, ''; rows.append((number, kind, name, key, value)); continue
        if key in sub_kw: sub = value; rows.append((number, kind, name, key, value)); continue
        rows.append((number, kind if not sub else f'{kind}/{sub_kw and "faza"}', name, key, value))
    return rows


def scripts(game):
    g = {}
    g['postacie'] = parse_blocks(game, 'scripts/postacie.txt', {'postac'}, {'faza'})
    g['items'] = parse_blocks(game, 'scripts/items.txt', {'item'})
    g['objects'] = parse_blocks(game, 'scripts/objects.txt', {'object'})
    g['scenki'] = parse_blocks(game, 'scripts/scenki.txt', {'scena'}, {'faza'})
    g['gameai'] = parse_blocks(game, 'scripts/ai/gameai.txt', {'level', 'action'})
    g['dialogi'] = parse_blocks(game, 'scripts/ai/dialogi.txt', {'dialog'})
    return g


def text_keys(game):
    """{key: (line, text)} of scripts/text_keys.txt (`>Key text`); speech keys carry a sounds\\..wav path."""
    keys = {}
    for number, raw in enumerate(read_script(game / 'scripts/text_keys.txt').splitlines(), 1):
        m = re.match(r'(>\S+)\s*(.*)', raw.strip())
        if m: keys[m.group(1)] = (number, m.group(2).split('//')[0].strip())
    return keys


def binary_strings(game):
    out = {}
    for n in BINARIES:
        f = game / n
        out[n] = {m.group(0).decode('latin1') for m in re.finditer(rb'[\x20-\x7e]{3,}', f.read_bytes())} if f.is_file() else set()
    return out


def dat_strings(game):
    """Path-like strings of every world DAT (textures of the world models, object Model / sound / sprite properties)."""
    res = {}
    for f in sorted((game / 'worlds').glob('*.dat')):
        data = f.read_bytes()
        paths = {m.group(0).decode('latin1') for m in re.finditer(rb'[\w\-\\/ \.&]+\.(?:ltb|lta|dtx|spr|wav|mp3|pcx|avi)', data, re.I)}
        res[f.stem.lower()] = {p.strip() for p in paths if 4 < len(p) < 200}
    return res


def rust_sources(root):
    text = ''
    for p in sorted(Path(root).rglob('*.rs')): text += p.read_text(encoding='utf-8', errors='replace') + '\n'
    return text


def exporter_sources(root):
    text = ''
    for p in sorted(Path(root).glob('export_*.py')): text += p.read_text(encoding='utf-8', errors='replace') + '\n'
    return text


# ------------------------------------------------------------------ worlds / campaign
def worlds_section(game, output):
    inv = json.loads((output / CAMPAIGN_JSON).read_text(encoding='utf-8')) if (output / CAMPAIGN_JSON).is_file() else {}
    by = {w['world'].lower(): w for w in inv.get('worlds', [])}
    rows = []
    for dat in sorted((game / 'worlds').glob('*.dat')):
        w = dat.stem.lower()
        scene = output / f'{dat.stem}.scene.json'
        kinds = Counter()
        if scene.is_file():
            for o in json.loads(scene.read_text(encoding='utf-8')).get('objects', []): kinds[o['kind']] += 1
        info = by.get(w, {})
        if info.get('linked_main_route'): status = 'USED-IN-REMAKE' if info.get('playable') or w == 'rh1-wiezienie1' else 'USED-IN-REMAKE'
        elif w == 'chinatown': status = 'UNUSED-IN-RETAIL'   # authored playable level without an inbound transition; the remake can load it from the menu
        else: status = 'UNUSED-IN-RETAIL'
        rows.append({'world': dat.stem, 'dat_bytes': dat.stat().st_size, 'pth_bytes': (game / 'worlds' / (dat.stem + '.pth')).stat().st_size if (game / 'worlds' / (dat.stem + '.pth')).is_file() else 0,
                     'linked_main_route': bool(info.get('linked_main_route')), 'title': info.get('title'), 'scene_export': scene.is_file(),
                     'gameplay_export': (output / f'{dat.stem}.gameplay.json').is_file(), 'objects': sum(kinds.values()), 'status': status,
                     'characters_placed': kinds.get('o_postac', 0), 'emitters': kinds.get('o_emiter_postaci', 0), 'items_placed': kinds.get('o_item_podnoszony', 0),
                     'extra': 'see docs/cut-content.md' if not info.get('linked_main_route') else ''})
    return rows


# ------------------------------------------------------------------ live sets
def placed_sets(output):
    """What the 31 level exports place: character definition names (gameplay.json npcs + emitters), item kinds, o_obiekt models, cutscene names."""
    chars, items, models, scenes, cs_models = defaultdict(set), defaultdict(set), defaultdict(set), defaultdict(set), defaultdict(set)
    for f in sorted(output.glob('*.gameplay.json')):
        w = f.name.split('.')[0].lower()
        d = json.loads(f.read_text(encoding='utf-8'))
        for n in d.get('npcs', []) + d.get('emitters', []):
            if n.get('definition_name'): chars[n['definition_name'].lower()].add(w)
        for name in d.get('characters', {}): chars[name.lower()].add(w)
    for f in sorted(output.glob('*.scene.json')):
        w = f.name.split('.')[0].lower()
        for o in json.loads(f.read_text(encoding='utf-8')).get('objects', []):
            p = o.get('properties', {})
            if o['kind'] == 'o_item_podnoszony' and p.get('Rodzaj_item'): items[p['Rodzaj_item'].lower()].add(w)
            if o['kind'] == 'o_obiekt' and p.get('Model'): models[norm(p['Model'])].add(w)
            if o['kind'] == 'o_cutscene' and p.get('Rodzaj_cuts'): scenes[p['Rodzaj_cuts'].lower()].add(w)
            if o['kind'] in ('o_postac', 'o_emiter_postaci') and p.get('Model'): cs_models[norm(p['Model'])].add(w)
    return chars, items, models, scenes, cs_models


def script_text_names(game):
    """All words of gameai / dialogi / scenki / text blocks: names of characters, items, scenes mentioned by scripts (live by reference)."""
    t = ''
    for rel in ('scripts/ai/gameai.txt', 'scripts/ai/dialogi.txt', 'scripts/scenki.txt'): t += read_script(game / rel) + '\n'
    return t.lower()


def definitions_section(game, output, sc, binstr):
    chars, items, models, scenes, cs_models = placed_sets(output)
    alltext = script_text_names(game)
    bin_all = {s.lower() for v in binstr.values() for s in v}
    bintext = '\n'.join(sorted(bin_all))
    campaign = json.loads((output / CAMPAIGN_JSON).read_text(encoding='utf-8'))['worlds'] if (output / CAMPAIGN_JSON).is_file() else []
    main_worlds = {w['world'].lower() for w in campaign if w['linked_main_route']}
    res = {'characters': [], 'items': [], 'objects': [], 'scenes': []}
    live_scene_actors = set()
    cur_scene = None
    for line, kind, name, key, value in sc['scenki']:
        if kind == 'scena' and key == 'scena':
            where = sorted(scenes.get(name.lower(), []))
            named = name.lower() in alltext.replace('scena ' + name.lower(), '')
            res['scenes'].append({'name': name, 'line': line, 'placed_in': where, 'status': 'USED-IN-REMAKE' if where or named else 'UNUSED-IN-RETAIL',
                                  'note': '' if where else ('named by a script' if named else 'no o_cutscene uses it')})
            cur_scene = bool(where or named)
        elif kind.split('/')[0] == 'scena' and re.fullmatch(r'postac\d+', key) and cur_scene: live_scene_actors.add(value.lower())
    # characters: postac NAME blocks
    char_model = {}
    for line, kind, name, key, value in sc['postacie']:
        if kind == 'postac' and key == 'model': char_model[name.lower()] = norm(value)
    weapons_used = Counter()
    for line, kind, name, key, value in sc['postacie']:
        if kind == 'postac' and key == 'weapon': weapons_used[value.lower()] += 1
    seen = set()
    for line, kind, name, key, value in sc['postacie']:
        if key == 'postac' and kind == 'postac':
            lname = name.lower()
            where = sorted(chars.get(lname, []))
            status = 'USED-IN-REMAKE' if where else ('USED-BY-RETAIL-BUT-MISSING-IN-REMAKE' if (lname in alltext or lname in bintext) else 'UNUSED-IN-RETAIL')
            note = ''
            if not where and lname in live_scene_actors: status, note = 'USED-IN-REMAKE', 'cutscene actor (scenki.txt postacN of a scene that runs)'
            if where and not (set(where) & main_worlds): status, note = 'UNUSED-IN-RETAIL', 'placed only in worlds off the linked route: ' + ','.join(where)
            if not where and lname in alltext and not note: note = 'named by a script but placed by no level and no running scene (spawned by script/code?)'
            if not where and lname not in alltext and not (lname in bintext): note = 'defined, never placed, never named'
            res['characters'].append({'name': name, 'line': line, 'model': char_model.get(lname), 'placed_in': where, 'status': status, 'note': note})
    item_names = []
    for line, kind, name, key, value in sc['items']:
        if kind == 'item' and key == 'item': item_names.append((name, line))
    received = set(re.findall(r'^\s*receive\s+(.+?)\s*$', alltext, re.M)) | set(re.findall(r'^\s*ifplayerhas\s+(.+?)\s*$', alltext, re.M))
    ammo_for = defaultdict(set)
    for line, kind, name, key, value in sc['items']:
        if key == 'ammo_for': ammo_for[value.lower()].add(name.lower())
    for name, line in item_names:
        ln = name.lower()
        where = sorted(items.get(ln, []))
        status, note = 'UNUSED-IN-RETAIL', 'never placed, never received, never carried by a character'
        if where: status, note = 'USED-IN-REMAKE', ''
        elif ln in received: status, note = 'USED-IN-REMAKE', 'given by a script (receive / ifplayerhas)'
        elif ln in weapons_used: status, note = 'USED-IN-REMAKE', f'carried by {weapons_used[ln]} character definition(s)'
        elif any(ln in v for v in ammo_for.values()) or ln in ammo_for: status, note = 'USED-IN-REMAKE', 'ammo of a used weapon'
        res['items'].append({'name': name, 'line': line, 'placed_in': where, 'status': status, 'note': note})
    obj_model = {}
    for line, kind, name, key, value in sc['objects']:
        if kind == 'object' and key == 'model': obj_model[name] = (norm(value), line)
    bymodel = defaultdict(list)
    for n, (m, l) in obj_model.items(): bymodel[m].append(n)
    for line, kind, name, key, value in sc['objects']:
        if kind == 'object' and key == 'object':
            m = obj_model.get(name, (None, 0))[0]
            where = sorted(models.get(m, [])) if m else []
            res['objects'].append({'name': name, 'line': line, 'model': m, 'placed_in': where, 'same_model_defs': len(bymodel.get(m, [])),
                                   'status': 'USED-IN-REMAKE' if where else 'UNUSED-IN-RETAIL', 'note': '' if where else 'no o_obiekt with this model in any level'})
    return res


# ------------------------------------------------------------------ keywords
def stem(key):
    """numbered keys (skin0..7, on_koniec_widzi3, answer2snd, anim_postac4, socket7) share one keyword."""
    return re.sub(r'(?<=[a-z_])\d+', '#', key.lstrip('/'))


def remake_has(k, *texts):
    """The keyword occurs as (the start of) a string literal of the Rust / exporter sources or a key of a retail_*.json export.
    Numbered keys (`answer2snd`, `skin3`, `on_koniec_widzi4`) also match `format!("answer{index}snd")`-style literals."""
    nd = re.sub(r'\d+$', '', k)
    Q = r'''["']'''
    first = re.split(r'\d', k)[0]
    pats = [Q + re.escape(k) + r'(?![A-Za-z0-9_])', Q + re.escape(nd) + r'''(?:\{|%|["']|\d)''']
    if first != k and len(first) >= 4: pats.append(Q + re.escape(first))     # numbered key: `slot_list(&c, "on_koniec_widzi", 8)`, `starts_with("skin")`
    if re.search(r'\d', k[:-1]):
        parts = re.split(r'\d+', k)
        pats.append(Q + r'''(?:\{[^}"]*\}|%d|\d+)'''.join(re.escape(x) for x in parts))
    return any(re.search(p, t) for p in pats for t in texts)


def keywords_section(sc, binstr, rs, py, exports=''):
    bin_blobs = {n: chr(10).join(v) for n, v in binstr.items()}
    rows = []
    for fname, data in sc.items():
        counts = defaultdict(Counter)
        first_line = {}
        for line, kind, name, key, value in data:
            if key in ('postac', 'item', 'object', 'scena', 'level', 'action', 'dialog', 'faza') and kind == key: continue
            counts[kind][key] += 1
            first_line.setdefault((kind, key), line)
        for kind, cnt in counts.items():
            for key, n in cnt.items():
                k = key.lstrip('/')
                nd = re.sub(r'\d+$', '', k)
                # the retail parsers compare the un-numbered stem for numbered keys and sometimes only a prefix (`nie_wspinaj` for `nie_wspinaj_sie`)
                in_bin = sorted(b for b, blob in bin_blobs.items() if re.search(r'(?<![A-Za-z0-9_])' + re.escape(k) + r'(?![A-Za-z_])', blob) or (nd != k and re.search(r'(?<![A-Za-z0-9_])' + re.escape(nd) + r'(?![A-Za-z_])', blob)))
                prefix = sorted(b for b, blob in bin_blobs.items() if not in_bin and len(k) > 8 and re.search(r'(?<![A-Za-z0-9_])' + re.escape(k[:len(k) // 2 + 3]), blob) and any(x.startswith(k[:len(k) // 2 + 3]) and k.startswith(x) for x in binstr[b]))
                in_rs = remake_has(k, rs, py, exports)
                rows.append({'file': fname, 'context': kind.split('/')[0], 'key': key, 'uses': n, 'first_line': first_line[(kind, key)], 'in_retail_binary': in_bin or [f'{b} (prefix)' for b in prefix], 'in_remake_sources': bool(in_rs)})
    return rows


# ------------------------------------------------------------------ text keys / dialogues / variables
def logic_section(game, sc, keys, binstr, output):
    text_all = {}
    for rel in ('scripts/ai/gameai.txt', 'scripts/ai/dialogi.txt', 'scripts/scenki.txt', 'scripts/postacie.txt', 'scripts/items.txt', 'scripts/objects.txt'):
        text_all[rel] = read_script(game / rel)
    loc = ''
    for f in sorted((game / 'scripts/locale').glob('*.txt')): loc += read(f) + '\n'
    scene_text = ''
    for f in sorted(output.glob('*.scene.json')): scene_text += f.read_text(encoding='utf-8') + '\n'
    binblob = '\n'.join(s for v in binstr.values() for s in v)
    used_keys = defaultdict(list)
    for rel, t in text_all.items():
        for number, line in enumerate(t.splitlines(), 1):
            for m in re.finditer(r'>[A-Za-z0-9_]+', line.split('//')[0]): used_keys[m.group(0)].append(f'{rel}:{number}')
    for m in re.finditer(r'>[A-Za-z0-9_]+', loc): used_keys[m.group(0)].append('scripts/locale')
    for m in re.finditer(r'>[A-Za-z0-9_]+', scene_text): used_keys[m.group(0)].append('scene.json')
    for m in re.finditer(r'>[A-Za-z0-9_]+', binblob): used_keys[m.group(0)].append('binary')
    lower_used = {k.lower(): v for k, v in used_keys.items()}
    bare = defaultdict(list)
    for rel, t in text_all.items():
        for number, line in enumerate(t.splitlines(), 1):
            for w in re.findall(r'[A-Za-z][A-Za-z0-9_]+', line.split('//')[0]): bare['>' + w.lower()].append(f'{rel}:{number}')
    for k, v in bare.items():
        if k not in lower_used and k in {x.lower() for x in keys}: lower_used[k] = v
    tk = []
    for k, (line, value) in keys.items():
        refs = lower_used.get(k.lower(), [])
        # the game builds some keys with sprintf-like suffixes (Person000_text%03d, answer keys); keep the flag visible
        tk.append({'key': k, 'line': line, 'is_speech_path': bool(re.search(r'\.(wav|mp3)$', value, re.I)), 'value': value[:80], 'references': len(refs), 'first_ref': refs[0] if refs else None})
    # dialogues
    defined = {name: line for line, kind, name, key, value in sc['dialogi'] if key == 'dialog' and kind == 'dialog'}
    ref_dialogs = Counter()
    for line, kind, name, key, value in sc['dialogi']:
        if key.startswith('onchoice') and key.endswith('dialog'): ref_dialogs[value] += 1
        if key in ('ontimeexceeded', 'onfurtherthan'):
            parts = value.split(None, 1)
            if len(parts) == 2: ref_dialogs[parts[1].strip()] += 1
    for line, kind, name, key, value in sc['gameai']:
        if key == 'dialog': ref_dialogs[value] += 1
    for f in sorted(output.glob('*.scene.json')):
        for o in json.loads(f.read_text(encoding='utf-8')).get('objects', []):
            for v in json_strings(o.get('properties', {})):
                if v in defined: ref_dialogs[v] += 1
    dlg = [{'dialog': n, 'line': l, 'references': ref_dialogs.get(n, 0)} for n, l in defined.items()]
    # variables
    vars_defined, set_n, test_n = {}, Counter(), Counter()
    for line, kind, name, key, value in sc['gameai']:
        if key in ('bool', 'licznik') and kind in ('HEAD', 'level', 'action'): vars_defined.setdefault(value, (key, line))
        if key == 'set': set_n[value] += 1
        if key in ('if', 'ifnot'): test_n[value] += 1
        if key in ('iflicznikwiekszyniz', 'iflicznikmniejszyniz'): test_n[value.split(None, 1)[-1]] += 1
    for line, kind, name, key, value in sc['dialogi']:
        if key == 'set': set_n[value] += 1
        if key == 'unset': set_n[value] += 1
    for line, kind, name, key, value in sc['gameai']:
        if key == 'unset': set_n[value] += 1
    scene_vars = Counter()
    for f in sorted(output.glob('*.scene.json')):
        for o in json.loads(f.read_text(encoding='utf-8')).get('objects', []):
            if o['kind'] == 'o_marker_zmienna':
                for k in ('Set', 'Unset'):
                    if o['properties'].get(k): scene_vars[o['properties'][k]] += 1
    vr = [{'name': n, 'kind': k, 'line': l, 'set_by_script': set_n.get(n, 0), 'set_by_marker': scene_vars.get(n, 0), 'tested': test_n.get(n, 0)} for n, (k, l) in vars_defined.items()]
    return tk, dlg, vr


def defects_section(sc, output, variables):
    """Places where a retail script names something that does not exist / can never happen (retail has these bugs too; the remake must not 'fix' them)."""
    chars = defaultdict(lambda: {'phases': set(), 'hdr': {}, 'line': 0})
    cur = ph = None
    for line, kind, name, key, value in sc['postacie']:
        if key == 'postac' and kind == 'postac': cur, ph = name.lower(), None; chars[cur]['line'] = line; continue
        if key == 'faza' and kind.startswith('postac'): ph = value; chars[cur]['phases'].add(value); continue
        if cur and kind == 'postac': chars[cur]['hdr'][key] = value
    items = {n.lower() for l, k, n, key, v in sc['items'] if key == 'item'}
    markers = set()
    for f in sorted(output.glob('*.scene.json')):
        for o in json.loads(f.read_text(encoding='utf-8')).get('objects', []):
            if o['kind'] == 'o_marker_zmienna':
                for k in ('Set', 'Unset'):
                    if o['properties'].get(k): markers.add(o['properties'][k])
    var = {v['name']: v for v in variables}
    rows = []
    def add(where, line, what, detail): rows.append({'where': where, 'line': line, 'what': what, 'detail': detail})
    level = action = ''
    for line, kind, name, key, value in sc['gameai']:
        if key == 'level' and kind == 'level': level = value
        if key == 'action' and kind == 'action': action = value; continue
        if kind != 'action': continue
        w = f'gameai {level} / action {action}'
        if key in ('if', 'ifnot'):
            v = var.get(value)
            if v is None: add(w, line, 'undeclared-variable', f'{key} {value}')
            elif key == 'if' and not v['set_by_script'] and not v['set_by_marker'] and value not in ('LaskaCzapelRespawnowana', 'LaskaChapelZagadana'): add(w, line, 'if-never-set', value)
        if key in ('ifalive', 'ifdead', 'ifplayerseenby', 'ifaction', 'ifactionhostile') and not (key == 'ifactionhostile' and not value) and value.lower() not in chars: add(w, line, 'unknown-character', f'{key} {value!r}')
        if key in ('ifgraczblizejniz', 'ifgraczdalejniz') and len(value.split(None, 1)) == 2 and value.split(None, 1)[1].lower() not in chars: add(w, line, 'unknown-character', f'{key} {value}')
        if key == 'ifplayerhas' and value.lower() not in items: add(w, line, 'unknown-item', value)
        if key in ('setfaza', 'setallfaza') and len(value.split(None, 1)) == 2:
            phase, who = value.split(None, 1)
            if who.lower() not in chars: add(w, line, 'unknown-character', f'{key} {value}')
            elif phase not in chars[who.lower()]['phases']: add(w, line, 'unknown-phase', f'{key} {value}')
    for c, d in chars.items():
        if d['hdr'].get('default_faza') and d['hdr']['default_faza'] not in d['phases']: add('postacie ' + c, d['line'], 'unknown-phase', 'default_faza ' + d['hdr']['default_faza'])
    cur = ph = None
    for line, kind, name, key, value in sc['postacie']:
        if key == 'postac' and kind == 'postac': cur = name.lower(); continue
        if kind.startswith('postac/') and re.fullmatch(r'on_(death|closer|further|koniec|hurt|reload)[a-z_]*\d*', key):
            tgt = value.split()[0] if value else ''
            if tgt and tgt not in chars[cur]['phases'] and tgt != 'estimate': add(f'postacie {cur}', line, 'unknown-phase', f'{key} {value}')
    for m in sorted(markers - set(var)): add('scene o_marker_zmienna', 0, 'undeclared-variable', m)
    return rows


# ------------------------------------------------------------------ object class properties
def object_properties(output, rs, py, registered=frozenset()):
    classes = defaultdict(lambda: {'instances': 0, 'levels': set(), 'props': defaultdict(lambda: {'n': 0, 'levels': set(), 'values': Counter()})})
    for f in sorted(output.glob('*.scene.json')):
        w = f.name.split('.')[0]
        for o in json.loads(f.read_text(encoding='utf-8')).get('objects', []):
            c = classes[o['kind']]
            c['instances'] += 1; c['levels'].add(w)
            for k, v in o.get('properties', {}).items():
                e = c['props'][k]; e['n'] += 1; e['levels'].add(w)
                if isinstance(v, (str, int, float, bool)) and len(e['values']) < 40: e['values'][str(v)[:40]] += 1
    rows = []
    for kind, c in sorted(classes.items()):
        for k, e in sorted(c['props'].items()):
            read_in_rs = remake_has(k, rs, py)
            distinct = len(e['values'])
            rows.append({'class': kind, 'property': k, 'instances_with': e['n'], 'class_instances': c['instances'], 'levels': len(e['levels']), 'distinct_values_seen': distinct,
                         'constant': distinct == 1, 'read_by_remake': read_in_rs, 'in_object_lto': k in registered})
    return rows


def classes_section(output, binstr):
    """Game classes object.lto registers (o_*, d_*, b_* strings + the light / world classes) against the kinds the 31 level exports place."""
    placed = Counter()
    for f in sorted(output.glob('*.scene.json')):
        for o in json.loads(f.read_text(encoding='utf-8')).get('objects', []): placed[o['kind']] += 1
    names = sorted({x for x in binstr['object.lto'] if re.fullmatch(r'(o|d|b)_[a-z0-9_]+', x)} | {'StartPoint', 'WorldProperties', 'SkyPointer', 'DemoSkyWorldModel', 'StaticSunLight', 'DirLight', 'ObjectLight', 'AmbientLight', 'LightGroup'})
    rows = [{'class': n, 'instances': placed.get(n, 0), 'status': 'USED-IN-REMAKE' if placed.get(n) else 'UNUSED-IN-RETAIL'} for n in names]
    rows += [{'class': n, 'instances': c, 'status': 'NOT-REGISTERED-IN-OBJECT.LTO'} for n, c in sorted(placed.items()) if n not in names]
    return rows


def font_glyph_coverage(game, binstr):
    """cshell.dll asks for these special glyph file names (`nawiaslewy.pcx`, `apostrof.pcx` ...); which font folders ship them."""
    wanted = sorted({m.group(1).lower() for x in binstr['cshell.dll'] for m in [re.fullmatch(r'([a-z]+)\.pcx', x)] if m})
    rows = []
    for d in sorted((game / 'misc/fonts').glob('*/1024')) + sorted((game / 'misc/fonts').glob('Un/1024/*')):
        have = {f.stem.lower() for f in d.glob('*.pcx')}
        rows.append({'folder': d.relative_to(game).as_posix(), 'asked_but_missing': [w for w in wanted if w not in have], 'shipped_but_never_asked': sorted(h for h in have if not re.fullmatch(r'[0-9]|d[a-z]|m[a-z]', h) and h not in wanted)})
    return rows


# ------------------------------------------------------------------ the file walk
def wav_seconds(path):
    """RIFF/WAVE duration from the fmt + data chunk sizes (PCM and MPEG-in-WAV alike); None when unreadable."""
    import struct
    try:
        with open(path, 'rb') as f:
            head = f.read(12)
            if head[:4] != b'RIFF' or head[8:12] != b'WAVE': return None
            rate = None
            while True:
                c = f.read(8)
                if len(c) < 8: return None
                name, size = c[:4], struct.unpack('<I', c[4:])[0]
                if name == b'fmt ':
                    fmt = f.read(size + (size & 1)); rate = struct.unpack_from('<I', fmt, 8)[0]
                elif name == b'data':
                    return round(size / rate, 2) if rate else None
                else: f.seek(size + (size & 1), 1)
    except Exception: return None


def duplicates(game, rows):
    """Byte-identical files (same size first, then SHA-1): renamed copies such as menu.wav == Fear_the_Sniper_s.wav."""
    import hashlib
    by_size = defaultdict(list)
    for r in rows:
        if r['bytes'] > 64: by_size[r['bytes']].append(r)
    groups = []
    for size, rs_ in by_size.items():
        if len(rs_) < 2: continue
        by_hash = defaultdict(list)
        for r in rs_: by_hash[hashlib.sha1((game / r['path']).read_bytes()).hexdigest()].append(r['path'])
        groups += [{'bytes': size, 'files': sorted(v)} for v in by_hash.values() if len(v) > 1]
    return sorted(groups, key=lambda g: -g['bytes'] * (len(g['files']) - 1))


def file_category(rel):
    p = rel.lower()
    e = p.rsplit('.', 1)[-1] if '.' in p else ''
    top = p.split('/')[0]
    if '/' not in p:
        return 'engine' if p in ENGINE_FILES or e in ('dll', 'exe', 'avi') else 'root'
    if top in ('classicons', 'textureeffectgroups', 'lightattenuationpresets'): return 'engine'
    if top == 'worlds': return 'world'
    if top in ('models',): return 'model'
    if top == 'skins': return 'skin'
    if top == 'textures': return 'texture'
    if top == 'sprites': return 'sprite'
    if top == 'rs': return 'renderstyle'
    if top == 'sounds':
        sub = p.split('/')[1] if p.count('/') > 1 else ''
        return {'muza': 'music', 'speech': 'speech', 'scenes': 'cutscene-audio'}.get(sub, 'sound')
    if top == 'misc': return 'picture'
    if top == 'scripts': return 'script'
    return top


def subgroup(rel):
    parts = rel.split('/')
    return '/'.join(parts[:2]) if len(parts) > 2 else parts[0]


def build_refs(game, output, sc, keys_used, binstr, dat, live):
    """norm path -> list of (source, live) plus the set of live script blocks."""
    refs = defaultdict(list)
    chars, items, models, scenes, cs_models = live['placed']
    char_live = {c['name'].lower(): c['status'] != 'UNUSED-IN-RETAIL' for c in live['defs']['characters']}
    item_live = {c['name'].lower(): c['status'] != 'UNUSED-IN-RETAIL' for c in live['defs']['items']}
    obj_live = {c['name']: c['status'] != 'UNUSED-IN-RETAIL' for c in live['defs']['objects']}
    scene_live = {c['name'].lower(): c['status'] != 'UNUSED-IN-RETAIL' for c in live['defs']['scenes']}
    blockmap = {'postacie': ('postac', char_live, True), 'items': ('item', item_live, True), 'objects': ('object', obj_live, False), 'scenki': ('scena', scene_live, True)}
    for fname, data in sc.items():
        for line, kind, name, key, value in data:
            if key in ('postac', 'item', 'object', 'scena', 'level', 'action', 'dialog', 'faza') and kind == key: continue
            v = value.split('//')[0].strip()
            paths = [v] if VALUE_RE.fullmatch(v) else [m.group(0) for m in TOKEN_RE.finditer(v)]
            for p in paths:
                lv = True
                if fname in blockmap:
                    bk, table, lower = blockmap[fname]
                    lv = table.get(name.lower() if lower else name, True) if kind.split('/')[0] == bk else True
                refs[norm(p)].append((f'scripts/{fname}.txt:{line} [{kind.split("/")[0]} {name}] {key}', lv))
    # `ile_skinow N`: skinK variants are the same path with the trailing digits of the stem replaced by 0..N-1 (export_gameplay.py, cshell 'ile_skinow')
    blocks = defaultdict(dict)
    for line, kind, name, key, value in sc['postacie']:
        if kind.split('/')[0] == 'postac': blocks[name][key] = (line, value)
    for name, b in blocks.items():
        try: n = int(b.get('ile_skinow', (0, '0'))[1].split()[0])
        except Exception: n = 0
        for key, (line, value) in b.items():
            if n > 0 and re.fullmatch(r'skin\d+', key) and re.search(r'\d+\.dtx$', norm(value)):
                for i in range(n): refs[re.sub(r'\d+(\.dtx)$', lambda m: str(i) + m.group(1), norm(value))].append((f'scripts/postacie.txt:{line} [postac {name}] {key} (ile_skinow {n} variant {i})', char_live.get(name.lower(), True)))
    # commented-out references in scripts (dead by definition)
    for f in sorted((game / 'scripts').rglob('*.txt')):
        try: text = read_script(f)
        except Exception: continue
        for number, raw in enumerate(text.splitlines(), 1):
            if raw.lstrip().startswith('//'):
                for m in TOKEN_RE.finditer(raw):
                    refs[norm(m.group(0))].append((f'{f.relative_to(game).as_posix()}:{number} (commented out)', False))
    # the lensflare / other plain-list scripts
    for f in sorted((game / 'scripts/cs').glob('*.txt')):
        for number, raw in enumerate(read(f).splitlines(), 1):
            for m in TOKEN_RE.finditer(raw): refs[norm(m.group(0))].append((f'scripts/cs/{f.name}:{number}', True))
    # speech keys: a wav named by a text key is live when the key is referenced
    for k, (line, value) in live['text_keys'].items():
        if re.search(r'\.(wav|mp3)$', value, re.I):
            used = bool(keys_used.get(k.lower()))
            refs[norm(value)].append((f'scripts/text_keys.txt:{line} {k}', used))
    for b, strs in binstr.items():
        for s in strs:
            for m in TOKEN_RE.finditer(s):
                if '\\' in m.group(0) or '/' in m.group(0): refs[norm(m.group(0))].append((f'{b} (hard-coded)', True))
    for w, paths in dat.items():
        for p in paths:
            refs[norm(p)].append((f'worlds/{w}.dat', True))
    return refs


def expand_aliases(retail, refs):
    """bare names (`load_c prolog.pcx`, `szum.wav`) and .lta names resolve to the shipped file; misc/<dir>/x is shipped in misc/<dir>_l/x."""
    add = []
    for p, r in list(refs.items()):
        real = resolve_retail(retail, p)
        if real and real != p: add += [(real, x) for x in r]
        if '/' not in p:
            # a bare file name is searched by name by the engine: every file of that name counts
            for other in retail.by_name.get(p, []):
                if other != real: add += [(other, x) for x in r]
        if not retail.has(p):
            parts = p.split('/')
            if len(parts) > 2 and parts[0] == 'misc':
                loc = '/'.join([parts[0], parts[1] + '_l', *parts[2:]])
                if retail.has(loc): add += [(loc, x) for x in r]
        else:
            # locale 1 (the shipped autoexec.cfg): cshell.dll tries misc/<dir>_l/<name> first, so the localised twin is the live file
            parts = p.split('/')
            if len(parts) > 2 and parts[0] == 'misc' and not parts[1].endswith('_l'):
                loc = '/'.join([parts[0], parts[1] + '_l', *parts[2:]])
                if retail.has(loc): add += [(loc, x) for x in r]
    for p, x in add: refs[p].append(x)


def sequence_expansion(retail, refs):
    r"""`sound_on_kontakt <dir>\halt0.wav` + `sounds_on_kontakt N`: cshell 0x10049a9d replaces the digit with rand() % N, and the death scream
    (`dead0.wav` string, docs/retail-audio.md) uses `dead<rand % 8>.wav` of the same folder: every haltN / deadN file of that folder is live."""
    add = []
    for p, r in list(refs.items()):
        live = [x for x in r if x[1] and 'sound_on_kontakt' in x[0]]
        if not live or not re.search(r'/halt0\.wav$', p): continue
        folder = p.rsplit('/', 1)[0] + '/'
        for k in retail.files:
            if k.startswith(folder) and re.fullmatch(r'(halt|dead)\d+\.wav', k[len(folder):]): add += [(k, (live[0][0] + ' (haltN/deadN sequence)', True))]
    for k, x in add: refs[k].append(x)


def sprite_dependencies(game, retail, refs):
    """A live .spr makes its frame textures live; a live .ltb-less chain stops here (LTB skins come from scripts)."""
    add = []
    for p, r in list(refs.items()):
        if p.endswith('.spr') and retail.has(p) and any(lv for _, lv in r):
            try:
                for frame in sprite_frames(retail, p): add.append((norm(frame), (f'{p} (sprite frame)', True)))
            except Exception: pass
    for p, r in add: refs[p].append(r)


def prefix_live(p, prefixes):
    """misc/menu_l/save_1024.pcx, outro/outro1_640.pcx: cshell.dll holds only the prefix `save_` / `outro` and appends the resolution."""
    stem_ = p.rsplit('/', 1)[-1].rsplit('.', 1)[0]
    base_ = re.sub(r'_?(640|800|1024)$', '', stem_)
    return base_ != stem_ and base_ in prefixes


def classify_file(rel, size, retail, out, refs, patterns, world_rows, ctx):
    p = norm(rel)
    cat = file_category(p)
    base = p.rsplit('/', 1)[-1]
    parts_ = p.split('/')
    if len(parts_) > 2 and parts_[0] == 'misc' and not parts_[1].endswith('_l') and retail.has('/'.join([parts_[0], parts_[1] + '_l', *parts_[2:]])) and base not in MARKER_FILES and not base.startswith('dirtype'):
        same = (retail.root / rel).read_bytes() == retail.files['/'.join([parts_[0], parts_[1] + '_l', *parts_[2:]])].read_bytes()
        return file_category(p), 'UNUSED-IN-RETAIL', [], 'locale-0 fallback twin of misc/%s_l (shipped autoexec.cfg has locale 1; the twin is %s)' % (parts_[1], 'byte-identical' if same else 'different art, e.g. other language')
    if cat == 'renderstyle' and p.rsplit('.', 1)[-1] in ('lta', 'vsh', 'ash'): return 'renderstyle', 'ENGINE-INTERNAL', [], 'D3D render style source / vertex shader of the LithTech renderer (the .ltb is what scripts name)'
    if cat == 'renderstyle' and base == 'dirtyperenderstyles': return 'renderstyle', 'ENGINE-INTERNAL', [], 'directory marker'
    if cat == 'engine' or base in MARKER_FILES or base.startswith('dirtype') or p.startswith('classicons/') or p.startswith('textureeffectgroups/') or p.startswith('lightattenuationpresets'):
        return cat if cat != 'root' else 'engine', 'ENGINE-INTERNAL', [], ''
    r = refs.get(p, [])
    if not r and '/' in p and p.count('/') >= 1:
        pass
    live = [s for s, lv in r if lv]
    dead = [s for s, lv in r if not lv]
    # a DAT of a world off the linked route (chinatown, nic, katscena, outro) makes its content cut content, not campaign content
    off = [s for s in live if s.startswith('worlds/') and s[7:-4].lower() not in ctx['main_worlds']]
    if off and len(off) == len(live): live, dead = [], off + dead
    else: live = [s for s in live if s not in off]
    if cat == 'world':
        w = base.rsplit('.', 1)[0]
        row = next((x for x in world_rows if x['world'].lower() == w), None)
        linked = bool(row and row['linked_main_route'])
        return 'world', ('USED-IN-REMAKE' if linked else 'UNUSED-IN-RETAIL'), [f'campaign-inventory.json linked={linked}'], '' if linked else 'not on the linked campaign route'
    if cat == 'script':
        return 'script', ('USED-IN-REMAKE' if size > 0 else 'UNUSED-IN-RETAIL'), live[:3], '' if size > 0 else 'empty 0-byte stub (editor leftover, the merged gameai/dialogi/postacie/objects files are the real ones)'
    # pattern (sprintf) references from binaries: sounds\hero\krok%d.wav etc.
    pat = next((pt for pt, rx in patterns if rx.fullmatch(p)), None)
    if pat and not live: live = [f'sprintf pattern {pat}']
    if not live and cat == 'picture' and prefix_live(p, ctx['prefixes']): live = ['cshell.dll name prefix + resolution suffix']
    in_remake = exported(retail, out, p) or (cat == 'picture' and exported(retail, out, re.sub(r'^(misc/[a-z]+)_l/', r'/', p))) or (cat == 'texture' and (p in ctx['world_textures'] or p[:-4].replace('/', '_') in ctx.get('env_flat', ()))) or (cat == 'picture' and (base.rsplit('.', 1)[0] in ctx['ui_stems'] or (base == 'kursor.dtx' and out.has('ui/cursor.png'))))
    variant = bool(re.search(r'/(640|800)/|(640|800)\.(pcx|dtx)$', p)) and cat == 'picture'
    if cat == 'picture' and p.startswith('misc/fonts/'):
        # glyph pcx files are opened by the retail font loader from a name built at run time (letters/digits + the special names in cshell.dll)
        glyph = re.match(r'misc/fonts/(un|cyfry|info|sub|mincho)/(1024|800|640)/?(normal|podswietl|big)?/?', p)
        stem_ = base.rsplit('.', 1)[0]
        known = re.fullmatch(r'[0-9]|d[a-z]|m[a-z]|apostrof|dollar|procent|nawiasprawy|nawiaslewy|minus|plus|gwiazdka|slash|backslash|cudzyslow|maupa|wykrzyknik|pytajnik|krzyzyk|dwukropek|przecinek|kropka|spacja|table', stem_)
        if not known and p.startswith(('misc/fonts/info/', 'misc/fonts/cyfry/')): return cat, 'UNUSED-IN-RETAIL', [], 'glyph name cshell.dll never asks for (its info / cyfry font loader builds: digits, D<letter>, m<letter> and the special names in cshell.dll)'
        if stem_ in ('untitled-1', 'uszy'): return cat, 'UNUSED-IN-RETAIL', [], 'stray editor file in the font folder (not a glyph)'
        live = live or ['retail font loader (glyph name built at run time; the Un / sub fonts are not loaded by cshell.dll but by the engine font manager)']
        in_remake = base.rsplit('.', 1)[0] in ctx['font_stems'] or p.startswith('misc/fonts/mincho')
        if glyph and glyph.group(2) != '1024': return cat, 'USED-IN-REMAKE' if in_remake else 'USED-BY-RETAIL-BUT-MISSING-IN-REMAKE', live[:1], 'resolution variant: retail picks the folder by screen width, the remake always uses the 1024 set'
    if variant and live: return cat, 'USED-IN-REMAKE', live[:3], 'resolution variant: retail picks 640/800/1024 by screen width, the remake always uses the 1024 art'
    if live and cat == 'renderstyle': return cat, 'USED-IN-REMAKE', live[:3], 'approximated: the remake picks a blend state by the style name, the .ltb is not parsed'
    if live:
        if in_remake: return cat, 'USED-IN-REMAKE', live[:3], ''
        return cat, 'USED-BY-RETAIL-BUT-MISSING-IN-REMAKE', live[:3], 'retail references it, no exported counterpart found in output/ (may be a pure resolution variant, see audit_assets)'
    if dead: return cat, 'UNUSED-IN-RETAIL', dead[:3], ('only in worlds off the linked route (cut content; exported: %s)' % ('yes' if in_remake else 'no')) if any(d.startswith('worlds/') for d in dead) else 'only referenced from commented-out lines / definitions no level places'
    return cat, 'UNUSED-IN-RETAIL', [], 'nothing refers to it by name'


def sprintf_patterns(binstr):
    pats = []
    for b, strs in binstr.items():
        for s in strs:
            if ('%d' in s or '%s' in s or '%i' in s or '%02d' in s or '%03d' in s) and re.search(r'\.(wav|mp3|pcx|dtx|ltb|spr)\b', s, re.I):
                rx = re.escape(norm(s)).replace('%d', r'\d+').replace('%i', r'\d+').replace('%02d', r'\d{2}').replace('%03d', r'\d{3}').replace('%s', r'[^/]+')
                pats.append((norm(s), re.compile(rx)))
    return pats


def run(game, output, repo):
    game, output, repo = Path(game), Path(output), Path(repo)
    retail, out = Index(game), Index(output)
    sc = scripts(game)
    keys = text_keys(game)
    binstr = binary_strings(game)
    dat = dat_strings(game)
    rs = rust_sources(repo / 'crates')
    py = exporter_sources(repo / 'tools')
    worlds = worlds_section(game, output)
    placed = placed_sets(output)
    defs = definitions_section(game, output, sc, binstr)
    tk, dlg, variables = logic_section(game, sc, keys, binstr, output)
    keys_used = {t['key'].lower(): t['references'] for t in tk if t['references']}
    live = {'placed': placed, 'defs': defs, 'text_keys': keys}
    refs = build_refs(game, output, sc, {k: v for k, v in keys_used.items()}, binstr, dat, live)
    expand_aliases(retail, refs)
    sequence_expansion(retail, refs)
    sprite_dependencies(game, retail, refs)
    patterns = sprintf_patterns(binstr)
    ctx = {'world_textures': set(), 'ui_stems': set(), 'font_stems': set(), 'main_worlds': {w['world'].lower() for w in worlds if w['linked_main_route']}}
    for f in [*sorted(output.glob('*.visual.materials.json')), *sorted((output / 'world_models').glob('*/*.visual.materials.json'))]:
        for m in json.loads(f.read_text(encoding='utf-8')).values():
            if m.get('source_texture'): ctx['world_textures'].add(norm(m['source_texture']))
    for k in out.files:
        if k.startswith('decor/textures/') and k.endswith('.png'): ctx['world_textures'].add('textures/' + k[len('decor/textures/'):-4].removesuffix('.dtx') + '.dtx')
        if k.startswith('env_textures/'): ctx.setdefault('env_flat', set()).add(k.rsplit('/', 1)[-1].rsplit('.', 1)[0])
    for k in out.files:
        if k.startswith(('ui/', 'hud/')): ctx['ui_stems'].add(k.rsplit('/', 1)[-1].rsplit('.', 1)[0])
        if k.startswith('ui/fonts/'): ctx['font_stems'].add(k.rsplit('/', 1)[-1].rsplit('.', 1)[0])
    ctx['prefixes'] = {x.lower().rstrip('_') for strs in binstr.values() for x in strs if 3 < len(x) < 30 and re.fullmatch(r'[A-Za-z0-9_]+', x)}
    rows = []
    for p in sorted(game.rglob('*')):
        if not p.is_file(): continue
        rel = p.relative_to(game).as_posix()
        cat, status, refl, note = classify_file(rel, p.stat().st_size, retail, out, refs, patterns, worlds, ctx)
        row = {'path': rel, 'bytes': p.stat().st_size, 'category': cat, 'group': subgroup(rel), 'status': status, 'referenced_by': refl, 'note': note}
        if rel.lower().endswith('.wav'): row['seconds'] = wav_seconds(p)
        rows.append(row)
    exports = ''
    for f in ('retail_items.json', 'retail_weapons.json', 'retail_altfire.json', 'retail_effects.json', 'props_defs.json', 'retail_inventory.json'):
        if (output / f).is_file(): exports += (output / f).read_text(encoding='utf-8', errors='replace') + chr(10)
    keywords = keywords_section(sc, binstr, rs, py, exports)
    props = object_properties(output, rs, py, binstr['object.lto'])
    summary = defaultdict(lambda: Counter())
    sizes = defaultdict(lambda: Counter())
    for r in rows:
        summary[r['category']][r['status']] += 1
        sizes[r['category']][r['status']] += r['bytes']
    groups = defaultdict(lambda: Counter())
    for r in rows:
        if r['status'] in ('UNUSED-IN-RETAIL', 'USED-BY-RETAIL-BUT-MISSING-IN-REMAKE'): groups[(r['category'], r['group'], r['status'])][''] += 1
    report = {'format': 'mesterlovesz-unused-inventory-v1', 'install_files': len(rows), 'install_bytes': sum(r['bytes'] for r in rows),
              'summary_files': {k: dict(v) for k, v in sorted(summary.items())}, 'summary_bytes': {k: dict(v) for k, v in sorted(sizes.items())},
              'worlds': worlds, 'definitions': defs, 'text_keys': tk, 'dialogues': dlg, 'mission_variables': variables, 'keywords': keywords, 'object_properties': props,
              'script_defects': defects_section(sc, output, variables), 'duplicate_groups': duplicates(game, rows), 'classes': classes_section(output, binstr), 'font_glyphs': font_glyph_coverage(game, binstr), 'unused_groups': [{'category': c, 'group': g, 'status': s, 'files': n['']} for (c, g, s), n in sorted(groups.items())], 'files': rows}
    return report


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('game', type=Path, nargs='?', default=Path('../GYARI'))
    ap.add_argument('output', type=Path, nargs='?', default=Path('output'))
    ap.add_argument('--list', action='store_true')
    args = ap.parse_args()
    repo = Path(__file__).resolve().parents[1]
    report = run(args.game, args.output, repo)
    (args.output / 'unused-inventory.json').write_text(json.dumps(report, ensure_ascii=False, indent=1) + '\n', encoding='utf-8')
    print(f"{report['install_files']} files, {report['install_bytes'] / 1e6:.0f} MB")
    for cat, c in report['summary_files'].items(): print(f'  {cat:15s}', '  '.join(f'{k.split("-")[0]}{"-" + k.split("-")[1] if "-" in k else ""}={v}' for k, v in sorted(c.items())))
    d = report['definitions']
    for k in ('characters', 'items', 'objects', 'scenes'):
        print(f'  defs {k:11s}', dict(Counter(x['status'] for x in d[k])))
    print('  text keys unreferenced', sum(1 for t in report['text_keys'] if not t['references']), '/', len(report['text_keys']))
    print('  dialogues unreferenced', sum(1 for t in report['dialogues'] if not t['references']), '/', len(report['dialogues']))
    print('  keywords not in binary', sum(1 for k in report['keywords'] if not k['in_retail_binary']), ', not in remake', sum(1 for k in report['keywords'] if not k['in_remake_sources']))
    if args.list:
        for r in report['files']:
            if r['status'] in ('UNUSED-IN-RETAIL', 'USED-BY-RETAIL-BUT-MISSING-IN-REMAKE'): print(f"  {r['status'][:6]} {r['path']} ({r['bytes']}) {r['note']}")
    return 0


if __name__ == '__main__':
    sys.exit(main())
