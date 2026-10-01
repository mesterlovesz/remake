"""Audits the sound files the retail game references (docs/retail-audio-parity.md "Files").

Collects every `sounds\\...wav` path from the decoded scripts (items, objects, postacie, scenki, dialogi, text keys, gameai), the exported world
scene/gameplay JSON (door sounds, detectors, cutscene voices) and the strings of cshell.dll / object.lto, resolves them case-insensitively in the retail
install (GYARI/sounds) and reports the ones that do not exist plus the files nothing references.

  python -m tools.audio_audit
"""
import re, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
GYARI = ROOT.parent / 'GYARI'
BACKSLASH = chr(92)
WAV = re.compile(r'sounds[\\/]+[\w\\/\-\.&]+?\.wav', re.I)


def normal(path):
    return path.replace(BACKSLASH, '/').strip().lower()


def retail_files():
    return {normal(str(p.relative_to(GYARI))) for p in (GYARI / 'sounds').rglob('*') if p.is_file()}


def references():
    found = {}
    def add(path, source):
        found.setdefault(normal(path), set()).add(source)
    for script in (ROOT / 'output' / 'decoded_scripts').glob('*.txt'):
        for match in WAV.finditer(script.read_text(encoding='utf-8', errors='replace')):
            add(match[0], script.name)
    for scene in list((ROOT / 'output').glob('*.scene.json')) + list((ROOT / 'output').glob('*.gameplay.json')) + [ROOT / 'output' / 'retail_ui.json']:
        if scene.is_file():
            for match in WAV.finditer(scene.read_text(encoding='utf-8', errors='replace').replace(BACKSLASH * 2, BACKSLASH)):
                add(match[0], scene.name.split('.', 1)[-1])
    for module in ('cshell.dll', 'object.lto'):
        data = (GYARI / module).read_bytes()
        for match in re.finditer(rb'sounds[\x5c/][\x20-\x7e]*?\.wav', data, re.I):
            add(match[0].decode(), module)
    # `graj_dzwiek_smierci` characters scream deadN.wav from the folder of their haltN.wav (cshell 0x10042dc5).
    for path in list(found):
        folder = re.match(r'(.*/)halt\d\.wav$', path)
        if folder:
            for n in range(8):
                add(folder[1] + f'dead{n}.wav', 'derived from halt')
    return found


def main():
    files, refs = retail_files(), references()
    missing = sorted(p for p in refs if p not in files)
    unused = sorted(f for f in files if f not in refs and not f.endswith('dirtypesounds'))
    print(f'{len(files)} retail sound files, {len(refs)} distinct references, {len(missing)} referenced but absent from the install:')
    for path in missing:
        print('  missing', path, sorted(refs[path])[:3])
    print(f'{len(unused)} files no script, scene or module string references (speech tracks reached only through text keys are counted as referenced when exported):')
    for path in unused[:400]:
        print('  unused', path)
    return 1 if missing and '--strict' in sys.argv else 0


if __name__ == '__main__':
    sys.exit(main())
