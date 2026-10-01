r"""Asset completeness audit: every model / skin / texture / sound / sprite / picture the retail game refers to must exist in the
retail install (GYARI) AND be exported to `output/` (docs/retail-assets.md).

References come from (1) every retail script (`scripts/**/*.txt`, decoded with the paired-letter cipher when needed), (2) the path-like
strings inside cshell.dll / object.lto / Lithtech.exe / server.dll (hard-coded effects, sounds, textures), (3) the properties of every
placed object in `output/*.scene.json`, and (4) the exported JSON itself (`gameplay.json`, `props.json`, `retail_*.json`, ...): every
output-relative path the runtime will `output.join(..)` must be a file.  A character with `glowa_specjalna` must carry a `head_asset`
whose model, skin and socket exist (the bug this audit was written for: stara_dziwka_glowa was exported by no level because the
exporter had only been re-run for three worlds).

    python -m tools.audit_assets ../GYARI output            # writes output/assets-audit.json, prints the table, exit 1 on a problem
    python -m tools.audit_assets --list                     # the classification rules

Classes: ok | a exporter bug (retail file exists, output lacks it: FAILS) | b missing in retail too (must be listed in
KNOWN_MISSING_IN_RETAIL with the reason, an unknown one FAILS as `b?`) | c unused / superseded (commented out in the script, a
resolution variant, a render style, the inert katscena prototype ...).
"""
import argparse, json, re, struct, sys
from collections import Counter, defaultdict
from pathlib import Path

from .decode_scripts import read_script

EXT = 'ltb|lta|dtx|spr|wav|mp3|avi|pcx|tga|bmp|jpg|ogg'
TOKEN_RE = re.compile(r'[\w\-\\/\.&]+\.(?:%s)\b' % EXT, re.I)
VALUE_RE = re.compile(r'[^"<>|*?]+\.(?:%s)' % EXT, re.I)
BINARIES = ('cshell.dll', 'object.lto', 'Lithtech.exe', 'server.dll', 'CRes.dll', 'sres.dll')
OUTPUT_DIRS = 'model_textures|models|audio|ui|hud|textures|decor|world_models|env_textures|birds|mods|videos'

# (b): retail script / binary references whose target the retail install does not contain either (normalised path -> why).
KNOWN_MISSING_IN_RETAIL = {
    'sounds/enemies/macaroni/halt0.wav': 'postacie.txt names the macaroni folder, retail ships halt0.wav only for china/civilian/italiano/police/russian',
    'sounds/weapons/laser_on.wav': 'items.txt laser sight sounds are not shipped',
    'sounds/weapons/laser_off.wav': 'items.txt laser sight sounds are not shipped',
    'rs/metal_z_maska.dtx': 'items.txt line 596 typo: the render style is rs/metal_z_maska.ltb (.dtx does not exist)',
    'misc/panel/hud/ramka_pick_healingkit.dtx': 'items.txt line 1085 typo: the shipped frame is ramka_pick_helingkit.dtx',
    'sprites/para.spr': 'objects.txt steam skin, not shipped',
    'sprites/mortyr.spr': 'd_sprite property of rh10-wiezowiec3 / wiez_wn1, not shipped (docs/retail-decorations.md)',
    'models/default.ltb': 'engine fallback name (Lithtech.exe / server.dll), never shipped',
    'renderstyles/default.ltb': 'engine fallback name (Lithtech.exe / server.dll), never shipped',
    'skins/default.dtx': 'engine fallback name (Lithtech.exe / server.dll), never shipped',
    'sprites/default.spr': 'engine fallback name (Lithtech.exe / server.dll), never shipped',
    'textures/default_texture.dtx': 'engine fallback name (Lithtech.exe), never shipped',
    **{f'sprites/flare1/{n}.spr': 'lens flare part: retail lacks it, that part draws nothing (docs/retail-decorations.md)'
       for n in ('centrum', 'przed1', 'przed2', 'przed3', 'przed4', 'przed5', 'za1', 'za2', 'za3')},
    'szum.wav': 'd_odglos property of rh2-wiezienie2, no such sound anywhere in the install',
    'wnt1.wav': 'd_odglos property of rh1-wiezienie3, no such sound anywhere in the install',
    'wnt2.wav': 'd_odglos property of rh1-wiezienie3, no such sound anywhere in the install',
    'wnt3.wav': 'd_odglos property of rh1-wiezienie3, no such sound anywhere in the install',
}
# (c): referenced but never played / used by a remade code path.  Prefix -> why.
UNUSED_PREFIXES = {
    'models/postacie/cutsceny/bohater_intro.ltb': 'only the katscena.dat prototype (DAT v70, not playable) places it',
    'models/postacie/cutsceny/policjant_intro.ltb': 'only the katscena.dat prototype (DAT v70, not playable) places it',
    'textures/sprajty/blik.dtx': 'd_emiter_iskier is inert in retail (docs/retail-decorations.md)',
}


def norm(path):
    return path.replace('\\', '/').strip().strip('"').lower().lstrip('./')


def category(path):
    p, e = norm(path), norm(path).rsplit('.', 1)[-1]
    if p.startswith('rs/'): return 'renderstyle'
    if e in ('ltb', 'lta'): return 'model'
    if e == 'dtx': return 'skin' if p.startswith('skins/') else 'texture'
    if e == 'spr': return 'sprite'
    if e in ('wav', 'mp3', 'ogg'): return 'sound'
    if e == 'avi': return 'video'
    return 'picture'


class Index:
    """Lower-case relative path -> real path of a directory tree."""
    def __init__(self, root):
        self.root = Path(root)
        self.files = {}
        for p in self.root.rglob('*'):
            # textures_hd/ (tools/upscale_textures.py) mirrors the names of the retail exports: it is an optional add-on, never a reference target
            if p.is_file() and p.relative_to(self.root).parts[0] != 'textures_hd': self.files[norm(p.relative_to(self.root).as_posix())] = p
        self.by_name = defaultdict(list)
        for k in self.files: self.by_name[k.rsplit('/', 1)[-1]].append(k)

    def has(self, rel): return norm(rel) in self.files


def ui_candidates(p):
    """misc/<dir>[_l]/<name>.(pcx|dtx): export_menu / export_inventory write ui/<dir>/<name>.png, the HUD frames hud/<name>.png,
    the panel .dtx skins model_textures/<p>.png."""
    parts = p.split('/')
    stem = parts[-1].rsplit('.', 1)[0]
    folder = parts[1].removesuffix('_l') if len(parts) > 2 else ''
    sub = '/'.join(parts[2:-1])
    return ['hud/' + stem + '.png', 'ui/' + folder + '/' + (sub + '/' if sub else '') + stem + '.png', 'ui/' + '/'.join(parts[1:-1]) + '/' + stem + '.png', 'model_textures/' + p + '.png']


def output_candidates(path):
    """Where the exporters put a retail file inside `output/` (case-insensitive)."""
    p, c = norm(path), category(path)
    if c == 'model': return [p + '.json']
    if c in ('skin', 'texture', 'picture') and p.startswith('misc/'): return ui_candidates(p)
    if c in ('skin', 'texture'): return ['model_textures/' + p + '.png', p + '.png']
    if c == 'sound': return ['audio/' + p]
    return []


def resolution_variant(path):
    """misc/**/x640.pcx and x800.pcx: the remake only uses the 1024 art."""
    return bool(re.search(r'(640|800)\.pcx$', norm(path)))


def json_strings(node):
    if isinstance(node, str): yield node
    elif isinstance(node, dict):
        for v in node.values(): yield from json_strings(v)
    elif isinstance(node, list):
        for v in node: yield from json_strings(v)


def line_paths(line):
    r"""`key value` lines carry one path with spaces allowed (`mesh models\pikapy\corn flakes.ltb`); anything else falls back to tokens."""
    _, _, value = line.strip().partition(' ')
    value = value.split('//')[0].strip()
    if value and VALUE_RE.fullmatch(value): return [value]
    return [m.group(0) for m in TOKEN_RE.finditer(line)]


def script_references(game):
    """(source, line number, path, commented) for every path-like token of every retail script."""
    for f in sorted((game / 'scripts').rglob('*.txt')):
        try: text = read_script(f)
        except Exception: continue
        rel = f.relative_to(game).as_posix()
        for number, raw in enumerate(text.splitlines(), 1):
            commented = raw.lstrip().startswith('//')
            for t in line_paths(raw.lstrip().removeprefix('//').strip() if commented else raw): yield rel, number, t, commented


def binary_references(game):
    for name in BINARIES:
        f = game / name
        if not f.is_file(): continue
        for m in re.finditer(rb'[\x20-\x7e]{4,}', f.read_bytes()):
            for t in TOKEN_RE.finditer(m.group(0).decode('ascii')):
                if '\\' in t.group(0) or '/' in t.group(0): yield name, 0, t.group(0), False


def scene_references(output):
    for f in sorted(output.glob('*.scene.json')):
        for obj in json.loads(f.read_text(encoding='utf-8')).get('objects', []):
            for value in json_strings(obj.get('properties', {})):
                for t in line_paths('x ' + value): yield f.name, obj.get('properties', {}).get('Name', obj.get('kind', '')), t, False


def resolve_retail(retail, path):
    """Exact root-relative path, else the unique file of that name (bare names such as `load_c prolog.pcx` live in misc/loading*)."""
    p = norm(path)
    if retail.has(p): return p
    if p.endswith('.lta') and retail.has(p[:-4] + '.ltb'): return p[:-4] + '.ltb'      # export_gameplay resolves LTA property values to the shipped LTB
    if '/' not in p:
        hits = retail.by_name.get(p, [])
        if hits: return hits[0]
    return None


def localised_variant(retail, p):
    """cshell names both misc/menu/x.pcx and misc/menu_l/x.pcx; only the localised folder ships the file."""
    parts = p.split('/')
    return len(parts) > 2 and parts[0] == 'misc' and retail.has('/'.join([parts[0], parts[1] + '_l', *parts[2:]]))


def sprite_frames(retail, real):
    """Frame texture paths of a retail .spr (5 u32 header, then u16-length-prefixed cp1250 paths; export_weapons.sprite_layer)."""
    data = retail.files[real].read_bytes()
    count = struct.unpack_from('<I', data)[0]
    offset, frames = 20, []
    for _ in range(count):
        length = struct.unpack_from('<H', data, offset)[0]
        frames.append(data[offset + 2:offset + 2 + length].decode('cp1250'))
        offset += 2 + length
    return frames


def exported(retail, out, real):
    if category(real) == 'sprite':
        try: frames = sprite_frames(retail, real)
        except Exception: return False
        # weapon/effect/prop sprites: model_textures/<frame>.png; d_sprite / lens flare / rain art: decor/<frame>.png (export_decorations)
        return all(out.has('model_textures/' + norm(f) + '.png') or out.has('decor/' + norm(f) + '.png') for f in frames)
    return any(out.has(c) for c in output_candidates(real))


def audit_characters(output):
    """glowa_specjalna / weapon / body: model, skins, sockets of every character definition of every gameplay.json exist."""
    problems, characters = [], 0
    sockets_cache = {}

    def sockets(model_asset):
        if model_asset not in sockets_cache:
            path = output / model_asset
            sockets_cache[model_asset] = set(json.loads(path.read_text(encoding='utf-8')).get('sockets', [])) if path.is_file() else None
        return sockets_cache[model_asset]

    for f in sorted(output.glob('*.gameplay.json')):
        world = f.name.split('.')[0]
        for name, c in json.loads(f.read_text(encoding='utf-8')).get('characters', {}).items():
            characters += 1
            where = f'{world}/{name}'
            header = {k: v for k, v in c.get('header', [])}
            body = sockets(c['model_asset'])
            if body is None: problems.append(f'{where}: body model {c["model_asset"]} is not exported')
            for slot, skin in c.get('skins', {}).items():
                if not (output / skin).is_file(): problems.append(f'{where}: skin{slot} {skin} is not exported')
            for label in ('weapon_asset', 'head_asset'):
                a = c.get(label)
                if not a: continue
                if not (output / (a['model'] + '.json')).is_file(): problems.append(f'{where}: {label} model {a["model"]} is not exported')
                for slot, skin in a.get('skins', {}).items():
                    if not (output / skin).is_file(): problems.append(f'{where}: {label} skin{slot} {skin} is not exported')
                if label == 'head_asset' and body is not None and a.get('socket') not in body:
                    problems.append(f'{where}: head socket {a.get("socket")!r} is missing on {c["model_asset"]} ({sorted(body)})')
            if 'glowa_specjalna' in c.get('flags', []) and not c.get('head_asset'):
                problems.append(f'{where}: glowa_specjalna but the export carries no head_asset (stale export?)')
            if header.get('socket_papieros') and body is not None and header['socket_papieros'] not in body:
                problems.append(f'{where}: socket_papieros {header["socket_papieros"]!r} is missing on the body model')
    return characters, problems


def audit_exported_paths(output):
    """Every output-relative file a top-level JSON export names must exist (the runtime output.join()s them); `model` fields are retail
    .ltb paths whose export is `<path>.json`."""
    out = Index(output)
    problems, checked = [], 0
    prefix = re.compile(r'^(%s)/' % OUTPUT_DIRS, re.I)
    for f in sorted(output.glob('*.json')):
        if f.name.endswith(('.scene.json', 'assets-audit.json')): continue
        node = json.loads(f.read_text(encoding='utf-8', errors='replace'))
        for s in set(json_strings(node)):
            wanted = None
            if prefix.match(s) and re.search(r'\.(json|png|wav|mp3|mp4)$', s, re.I): wanted = s
            elif re.match(r'^models/.+\.ltb$', s, re.I): wanted = s + '.json'
            if wanted:
                checked += 1
                if not out.has(wanted): problems.append(f'{f.name}: {s}')
    return checked, problems


def audit_hd_textures(output):
    """The optional HD textures (docs/retail-hd-textures.md) are not an audit target (a partial or absent textures_hd/ is fine), but what
    the manifest lists must be consistent: original present, HD file present, exact integer scale (UV / lightmap alignment)."""
    manifest = Path(output) / 'textures_hd' / 'manifest.json'
    if not manifest.is_file(): return 0, []
    from PIL import Image
    problems, files = [], json.loads(manifest.read_text(encoding='utf-8')).get('files', {})
    for rel, e in files.items():
        hd = Path(output) / 'textures_hd' / rel
        if not (Path(output) / rel).is_file(): problems.append(f'textures_hd: {rel} has no original any more'); continue
        if not hd.is_file(): problems.append(f'textures_hd: {rel} listed but missing'); continue
        with Image.open(hd) as im:
            if im.size != (e['src'][0] * e['scale'], e['src'][1] * e['scale']): problems.append(f'textures_hd: {rel} is {im.size}, expected {e["src"]} x {e["scale"]}')
    return len(files), problems


def world_texture_rows(game, output):
    """Textures a world DAT names that export_visual could not convert: (b) when the retail install lacks the file at that path (the world
    is lightmap-only there, like in retail: docs/retail-visual.md), (a) when it exists (an exporter bug)."""
    retail = Index(game)
    rows = []
    for f in sorted(output.glob('*.visual.report.json')):
        for texture in json.loads(f.read_text(encoding='utf-8')).get('missing_textures') or []:
            p = norm(texture)
            cls, note = ('a', 'retail file exists, export_visual did not convert it') if retail.has(p) else ('b', 'the world DAT names a texture the retail install lacks (lightmap-only in retail too)')
            rows.append({'source': f.name.split('.')[0] + '.dat', 'line': 0, 'path': p, 'category': 'texture', 'class': cls, 'note': note})
    return rows


def run(game, output):
    game, output = Path(game), Path(output)
    retail, out = Index(game), Index(output)
    rows = []
    for source, number, path, commented in [*script_references(game), *binary_references(game), *scene_references(output)]:
        p = norm(path)
        cls, note = 'ok', ''
        real = resolve_retail(retail, p)
        if commented:
            cls, note = 'c', 'commented out in the script'
        elif source == 'katscena.scene.json':
            cls, note = 'c', 'katscena.dat is an unplayable DAT v70 prototype (docs/retail-visual.md)'
        elif p in UNUSED_PREFIXES:
            cls, note = 'c', UNUSED_PREFIXES[p]
        elif real is None:
            if p in KNOWN_MISSING_IN_RETAIL: cls, note = 'b', KNOWN_MISSING_IN_RETAIL[p]
            elif localised_variant(retail, p): cls, note = 'c', 'the retail file lives in the localised _l folder (also referenced there)'
            else: cls, note = 'b?', 'not in the retail install'
        elif resolution_variant(real):
            cls, note = 'c', 'resolution variant (only the 1024 art is used)'
        elif category(real) == 'renderstyle':
            cls, note = 'c', 'render style: blend state, chosen by name in npcs.rs/props.rs, the file is not loaded'
        elif not exported(retail, out, real):
            cls, note = 'a', 'retail file exists, no exported counterpart'
        rows.append({'source': source, 'line': number, 'path': p, 'category': category(p), 'class': cls, 'note': note})
    rows += world_texture_rows(game, output)
    chars, char_problems = audit_characters(output)
    checked, path_problems = audit_exported_paths(output)
    hd_checked, hd_problems = audit_hd_textures(output)
    return {'hd_textures_checked': hd_checked, 'hd_texture_problems': hd_problems, 'format': 'mesterlovesz-assets-audit-v1', 'references': len(rows), 'characters_checked': chars, 'exported_paths_checked': checked,
            'character_problems': char_problems, 'exported_path_problems': path_problems, 'rows': rows}


def failures(report):
    """(class, path) -> [source:line ...] of every unexplained reference."""
    bad = {}
    for r in report['rows']:
        if r['class'] in ('a', 'b?'): bad.setdefault((r['class'], r['path']), []).append(f"{r['source']}:{r['line']}")
    return bad


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('game', type=Path, nargs='?', default=Path('../GYARI'))
    ap.add_argument('output', type=Path, nargs='?', default=Path('output'))
    ap.add_argument('--list', action='store_true', help='print the classification rules and exit')
    ap.add_argument('--all', action='store_true', help='also print the (b) and (c) rows')
    args = ap.parse_args()
    if args.list:
        print(__doc__)
        for path, why in sorted(KNOWN_MISSING_IN_RETAIL.items()): print(f'  b {path}: {why}')
        return 0
    report = run(args.game, args.output)
    rows = report['rows']
    counts = Counter(f"{r['class']}/{r['category']}" for r in rows)
    slim = {k: v for k, v in report.items() if k != 'rows'}
    slim['counts'] = dict(sorted(counts.items()))
    slim['rows'] = [r for r in rows if r['class'] != 'ok']
    (args.output / 'assets-audit.json').write_text(json.dumps(slim, ensure_ascii=False, indent=1) + '\n', encoding='utf-8')
    print(f"{report['references']} references, {report['characters_checked']} character definitions, {report['exported_paths_checked']} exported paths checked")
    for k, n in sorted(counts.items()): print(f'  {k:22s} {n}')
    bad = failures(report)
    if args.all:
        for r in rows:
            if r['class'] in ('b', 'c'): print(f"  {r['class']} {r['category']:11s} {r['path']}  <- {r['source']}:{r['line']}  ({r['note']})")
    for (cls, path), where in sorted(bad.items()): print(f'  {cls:2s} {path}  <- {", ".join(where[:3])}{" ..." if len(where) > 3 else ""}')
    for p in report['character_problems']: print('  character:', p)
    for p in report['exported_path_problems']: print('  exported path:', p)
    for p in report['hd_texture_problems']: print('  hd:', p)
    return 1 if bad or report['character_problems'] or report['exported_path_problems'] or report['hd_texture_problems'] else 0


if __name__ == '__main__':
    sys.exit(main())
