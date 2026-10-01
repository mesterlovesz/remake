"""Setup checks for `Indit.cmd` / `tools/export_all.py`: where the original game is, whether Python, the helper
packages, Rust and the free disk space are fine. Every message is Hungarian and ends with a fix hint.

    python -m tools.setup_check            # report only
    python -m tools.setup_check --install  # also pip-install the missing helper packages

The original game folder (GYARI) is only ever READ by this project. `find_game()` accepts, in this order:
`--game` / the MESTER_GAME environment variable, `../GYARI` (the documented place, next to the project folder),
`./GYARI` (inside the project folder; it is in .gitignore).
"""
import argparse
import hashlib
import importlib.util
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
# (importable module, pip package, what needs it)
PACKAGES = [('PIL', 'Pillow', 'képek (DTX/PCX -> PNG)'), ('imageio_ffmpeg', 'imageio-ffmpeg', 'az indító logóvideók átalakítása')]
# Files that identify the Hungarian retail install ("A Mesterlövész", the version 2.33 the remake reads).
KEY_FILES = ['Lithtech.exe', 'cshell.dll', 'object.lto', 'scripts/postacie.txt', 'scripts/items.txt', 'scripts/ai/gameai.txt',
             'scripts/text_keys.txt', 'worlds/rh1-wiezienie1.dat', 'worlds/rh3-miasteczko0.dat', 'sounds', 'models', 'textures', 'skins', 'sprites']
# The known-good install (Hungarian "A mesterlövész v 2.33"): size and MD5 of its three code files. The same MD5 values are in the
# disc image's InstallShield header (data1.hdr), so a fresh install from the disc image gives exactly these files (docs/SETUP-hu.md).
KNOWN_VERSION = {
    'cshell.dll': (507904, '5ffc58c44f63978243ec7567de2c0b10'),
    'Lithtech.exe': (1695744, '7fa3aabd9b80f28133ead5ae9b479ac1'),
    'object.lto': (200704, 'a3dcccc54513a97c34b40a7bf49e6c0b'),
}
MIN_PYTHON = (3, 10)
MIN_RUST = (1, 89)
NEED_OUTPUT_GB = 4
NEED_BUILD_GB = 6


def find_game(explicit=None) -> Path | None:
    """The retail install folder, or None."""
    chosen = explicit or os.environ.get('MESTER_GAME')
    for candidate in ([chosen] if chosen else [ROOT.parent / 'GYARI', ROOT / 'GYARI']):
        if (Path(candidate) / 'Lithtech.exe').is_file():
            return Path(candidate).resolve()
    return None


def expected_game_dir() -> Path:
    return (ROOT.parent / 'GYARI').resolve()


def missing_game_files(game: Path) -> list[str]:
    return [name for name in KEY_FILES if not (game / name).exists()]


def game_version(game: Path) -> str:
    try:
        return (game / 'scripts/app_name.txt').read_bytes().decode('cp1250').strip()
    except OSError:
        return ''


def check_game(explicit=None):
    """(game path or None, [problems]). Problems carry their Hungarian fix hint."""
    game = find_game(explicit)
    if game is None:
        for base in (ROOT.parent / 'GYARI', ROOT / 'GYARI'):  # the usual slip: one folder level too many
            inner = next((child for child in base.glob('*') if (child / 'Lithtech.exe').is_file()), None) if base.is_dir() else None
            if inner:
                return None, [f'A GYARI mappa ({base}) még egy mappát tartalmaz, a játék ebben van: {inner.name}. A Lithtech.exe-nek közvetlenül a GYARI mappában kell lennie: '
                              f'húzd ki a(z) „{inner.name}” tartalmát a GYARI mappába (vagy nevezd át a(z) „{inner.name}” mappát GYARI-ra, és tedd a projektmappa mellé).']
        where = f' (a megadott útvonal: {explicit or os.environ.get("MESTER_GAME")})' if explicit or os.environ.get('MESTER_GAME') else ''
        return None, [f'A GYARI mappa nem található{where}. Másold ide az eredeti játék telepített mappáját, és nevezd át GYARI-ra:\n'
                      f'    {expected_game_dir()}\n'
                      f'  (Lithtech.exe, cshell.dll, worlds, sounds ... legyen közvetlenül benne. Lemezkép esetén előbb telepítsd a játékot: docs/SETUP-hu.md)']
    missing = missing_game_files(game)
    problems = []
    if missing:
        problems.append(f'A GYARI mappa ({game}) hiányos, nem találom: {", ".join(missing)}.\n'
                        '  Ellenőrizd, hogy a magyar „A Mesterlövész” TELEPÍTETT mappáját másoltad be (nem a lemezt vagy egy almappát), és hogy a telepítés végigfutott.')
    elif 'mesterl' not in game_version(game).lower():
        problems.append(f'A GYARI mappa ({game}) nem a magyar „A Mesterlövész”-nek látszik (scripts/app_name.txt: „{game_version(game)}”). '
                        'Az angol Sniper: Path of Vengeance fájljai mások; a magyar kiadás 2.33-as változata kell.')
    return game, problems


def check_version(game: Path) -> list[str]:
    """Hungarian warnings when the install is not the known 2.33 build (an English/other release or a patched copy reads differently)."""
    different = []
    for name, (size, md5) in KNOWN_VERSION.items():
        path = game / name
        if not path.is_file():
            continue
        if path.stat().st_size != size or hashlib.md5(path.read_bytes()).hexdigest() != md5:
            different.append(name)
    if not different:
        return []
    return [f'A GYARI mappa {", ".join(different)} fájlja nem egyezik az ismert magyar 2.33-as kiadással (a lemezképről telepítettel egyezne). '
            'Más kiadással vagy módosított fájlokkal a kinyerés hibázhat, vagy a játék másként viselkedhet. Ajánlott: telepítsd újra a magyar „A Mesterlövész” játékot, és azt másold GYARI néven.']


def check_python():
    if sys.version_info < MIN_PYTHON:
        return [f'A Python {sys.version.split()[0]} túl régi (legalább {MIN_PYTHON[0]}.{MIN_PYTHON[1]} kell). Telepítsd a legújabb Python 3-at a python.org oldalról, és pipáld be az „Add python.exe to PATH” jelölőnégyzetet.']
    return []


def missing_packages():
    return [(pip, why) for module, pip, why in PACKAGES if importlib.util.find_spec(module) is None]


def install_packages(packages) -> list[str]:
    problems = []
    for pip, why in packages:
        print(f'  pip install {pip}  ({why}) ...', flush=True)
        result = subprocess.run([sys.executable, '-m', 'pip', 'install', '--disable-pip-version-check', pip])
        if result.returncode != 0:
            problems.append(f'A(z) {pip} csomag telepítése nem sikerült (internet kell hozzá). Próbáld kézzel: python -m pip install {pip}')
    return problems


def rust_version(cargo_env=None):
    """(major, minor) of the rustc cargo would use, or None when Rust is missing."""
    env = cargo_env or os.environ
    exe = shutil.which('rustc', path=env.get('PATH'))
    if exe is None:
        return None
    try:
        text = subprocess.run([exe, '--version'], capture_output=True, text=True, env=cargo_env, timeout=120).stdout
    except (OSError, subprocess.SubprocessError):
        return None
    parts = text.split()
    try:
        major, minor = parts[1].split('.')[:2]
        return int(major), int(minor)
    except (IndexError, ValueError):
        return None


def check_rust(cargo_env=None):
    version = rust_version(cargo_env)
    if version is None:
        return ['A Rust (cargo) nem található. Telepítsd a https://rustup.rs oldalról (Windowson a „Visual Studio C++ Build Tools” telepítését is elfogadni), '
                'majd indítsd újra az Indit.cmd-t. (Ha már telepítetted: zárd be és nyisd meg újra az ablakot, hogy a PATH frissüljön; ha a rustup telepítve van, de nincs alapértelmezett eszközlánc: rustup default stable)']
    if version < MIN_RUST:
        return [f'A Rust {version[0]}.{version[1]} túl régi (legalább {MIN_RUST[0]}.{MIN_RUST[1]} kell). Frissítsd: rustup update']
    return []


def free_gb(path: Path) -> float:
    probe = path
    while not probe.exists():
        probe = probe.parent
    return shutil.disk_usage(probe).free / 1e9


def check_disk(output: Path | None, build_dir: Path | None):
    """Free-space problems for the places that still need space (None = nothing more to write there)."""
    problems = []
    if output is not None and free_gb(output) < NEED_OUTPUT_GB:
        problems.append(f'Kevés a szabad hely az exporthoz ({free_gb(output):.1f} GB, kb. {NEED_OUTPUT_GB} GB kell itt: {output}). Szabadíts fel helyet.')
    if build_dir is not None and free_gb(build_dir) < NEED_BUILD_GB:
        problems.append(f'Kevés a szabad hely a fordításhoz ({free_gb(build_dir):.1f} GB, kb. {NEED_BUILD_GB} GB kell itt: {build_dir}). Szabadíts fel helyet.')
    return problems


def main() -> int:
    for stream in (sys.stdout, sys.stderr):
        stream.reconfigure(encoding='utf-8', errors='replace')
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--game', type=Path)
    parser.add_argument('--output', type=Path, default=ROOT / 'output')
    parser.add_argument('--install', action='store_true', help='telepíti a hiányzó Python csomagokat')
    args = parser.parse_args()
    problems = check_python()
    game, game_problems = check_game(args.game)
    problems += game_problems
    todo = missing_packages()
    if todo and args.install:
        problems += install_packages(todo)
    elif todo:
        problems.append('Hiányzó Python csomagok: ' + ', '.join(pip for pip, _ in todo) + ' (python -m tools.setup_check --install telepíti)')
    from .export_all import cargo_env
    problems += check_rust(cargo_env())
    problems += check_disk(args.output, ROOT / 'crates/level-viewer')
    if game:
        print(f'Eredeti játék: {game}  ({game_version(game)})')
    if game:
        for warning in check_version(game):
            print('FIGYELEM: ' + warning)
    for problem in problems:
        print('HIBA: ' + problem)
    print('Minden rendben.' if not problems else f'{len(problems)} hiba.')
    return 1 if problems else 0


if __name__ == '__main__':
    sys.exit(main())
