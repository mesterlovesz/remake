"""Player launcher; Python, exporter tools and the game binary are bundled."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path, PureWindowsPath
import re
import shutil
import subprocess
import sys
import urllib.error
import urllib.request
import zipfile


class SetupError(Exception):
    pass


MEDIA_URL = 'https://files.pythonhosted.org/packages/2c/c6/fa760e12a2483469e2bf5058c5faff664acf66cadb4df2ad6205b016a73d/imageio_ffmpeg-0.6.0-py3-none-win_amd64.whl'
MEDIA_SHA256 = '02fa47c83703c37df6bfe4896aab339013f62bf02c5ebf2dce6da56af04ffc0a'
MEDIA_ENTRY = 'imageio_ffmpeg/binaries/ffmpeg-win-x86_64-v7.1.exe'


def ensure_media_converter(internal: Path):
    """Fetch the unchanged upstream converter once, without a system installation."""
    packages = internal / 'python' / 'Lib' / 'site-packages'
    converter = packages / MEDIA_ENTRY
    if converter.is_file():
        return
    print('      Médiafeldolgozó letöltése az első indításhoz (31 MB)', flush=True)
    try:
        with urllib.request.urlopen(MEDIA_URL, timeout=90) as response:
            data = response.read()
    except (OSError, urllib.error.URLError) as error:
        raise SetupError('A médiafeldolgozó letöltéséhez internet kell. Ellenőrizd a kapcsolatot, majd indítsd újra az Indit.bat-ot.') from error
    if hashlib.sha256(data).hexdigest() != MEDIA_SHA256:
        raise SetupError('A médiafeldolgozó letöltése hiányos vagy hibás. Indítsd újra az Indit.bat-ot.')
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        archive.extract(MEDIA_ENTRY, packages)


def select_iso(root: Path, explicit=None) -> Path:
    if explicit:
        path = Path(explicit).resolve()
        if not path.is_file():
            raise SetupError(f'Nem találom az ISO-t: {path}')
        return path
    files = sorted(p for p in root.iterdir() if p.is_file() and p.suffix.lower() == '.iso')
    if not files:
        raise SetupError('Tedd a letöltött ISO fájlt az Indit.bat mellé, majd indítsd újra az Indit.bat-ot.')
    if len(files) != 1:
        raise SetupError('A mappában több ISO van. Csak A Mesterlövész ISO-ja maradjon az Indit.bat mellett.')
    return files[0]


def read_cabinets(source: Path, destination: Path):
    import pycdlib
    disc = pycdlib.PyCdlib()
    wanted = {'data1.cab', 'data2.cab', 'data1.hdr'}
    opened = False
    try:
        with source.open('rb') as stream:
            disc.open_fp(stream)
            opened = True
            paths = {}
            for folder, _, files in disc.walk(joliet_path='/'):
                for name in files:
                    if name.casefold() in wanted:
                        if name.casefold() in paths:
                            raise SetupError('Az ISO több telepítőt tartalmaz; a magyar Archive.org-os ISO szükséges.')
                        paths[name.casefold()] = folder.rstrip('/') + '/' + name
            if paths.keys() != wanted:
                raise SetupError('Ez az ISO nem tartalmazza a játék telepítőadatait. A magyar Archive.org-os ISO szükséges.')
            destination.mkdir(parents=True, exist_ok=True)
            for name, path in sorted(paths.items()):
                print(f'      ISO: {name}', flush=True)
                disc.get_file_from_iso(str(destination / name), joliet_path=path)
    except (OSError, pycdlib.pycdlibexception.PyCdlibException) as error:
        raise SetupError(f'Az ISO nem olvasható vagy hiányos: {error}') from error
    finally:
        if opened:
            disc.close()


def check_install(game: Path):
    from .setup_check import missing_game_files, check_version
    missing = missing_game_files(game)
    if missing:
        raise SetupError('Hiányos játékadatok: ' + ', '.join(missing[:8]))
    if check_version(game):
        raise SetupError('Más játékverzió: a magyar 2.33-as Archive.org-os ISO szükséges.')


def find_extracted_install(staging: Path) -> Path:
    folders = [p for p in staging.iterdir() if p.is_dir() and (p / 'Lithtech.exe').is_file()]
    if len(folders) != 1:
        raise SetupError(f'A kinyert játék mappája hiányzik vagy nem egyértelmű: {staging}')
    return folders[0]


def restore_installer_paths(game: Path, listing: str):
    """Unshield normalizes directory spaces even with -R; restore the disc's names."""
    conversions = str.maketrans(' <>[]', '_____')
    for line in listing.splitlines():
        match = re.match(r'^\s*\d+\s+App Executables\\(.+)$', line)
        if not match:
            continue
        path = PureWindowsPath(match[1])
        if path.is_absolute() or path.drive or '..' in path.parts:
            raise SetupError('Érvénytelen fájlútvonal az ISO-ban.')
        target = game.joinpath(*path.parts)
        current = game.joinpath(*(part.translate(conversions) for part in path.parts[:-1]), path.name)
        if current == target:
            continue
        if target.exists():
            raise SetupError(f'Ütköző fájlnevek a lemezen: {path}')
        if not current.is_file():
            raise SetupError(f'Hiányzik egy kinyert lemezfájl: {path}')
        target.parent.mkdir(parents=True, exist_ok=True)
        current.rename(target)


def extract_install(iso: Path, root: Path, internal: Path) -> Path:
    game = root / 'GYARI'
    if game.exists():
        check_install(game)
        return game
    cache = internal / 'cache'
    print('[1/3] Az eredeti játék adatainak kinyerése az ISO-ból', flush=True)
    read_cabinets(iso, cache / 'disc')
    staging = cache / 'install'
    staging.mkdir(parents=True, exist_ok=True)
    with (cache / 'iso-extract.log').open('w', encoding='utf-8') as log:
        result = subprocess.run([str(internal / 'unshield' / 'unshield.exe'),
                                 '-R', '-g', 'App Executables', '-d', str(staging), 'x',
                                 str(cache / 'disc' / 'data1.cab')],
                                stdout=log, stderr=subprocess.STDOUT)
    if result.returncode:
        raise SetupError(f'Az ISO kinyerése nem sikerült. Napló: {cache / "iso-extract.log"}')
    extracted = find_extracted_install(staging)
    listing = subprocess.check_output([str(internal / 'unshield' / 'unshield.exe'), '-g', 'App Executables',
                                       'l', str(cache / 'disc' / 'data1.cab')], text=True, encoding='mbcs')
    restore_installer_paths(extracted, listing)
    check_install(extracted)
    extracted.rename(game)
    # Only our three generated cabinet copies; the user's ISO is never modified.
    for name in ('data1.cab', 'data2.cab', 'data1.hdr'):
        (cache / 'disc' / name).unlink()
    return game


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--iso', type=Path)
    parser.add_argument('--prepare-only', action='store_true')
    parser.add_argument('--capture', type=Path, help='silent verification capture')
    args = parser.parse_args()
    internal = Path(__file__).resolve().parents[1]
    root = internal.parent
    output = root / 'output'
    marker = output / '.setup' / 'player-ready.json'
    exe = internal / 'Mesterlovesz.exe'
    try:
        try:
            manifest = json.loads((internal / 'package.json').read_text(encoding='utf-8'))
        except (OSError, ValueError):
            raise SetupError('Hiányos játékoscsomag. Csomagold ki a teljes ZIP-et egy írható mappába.')
        if not exe.is_file():
            raise SetupError('Hiányos játékoscsomag. Csomagold ki a teljes ZIP-et egy írható mappába.')
        try:
            ready = json.loads(marker.read_text(encoding='utf-8')).get('package') == manifest['package']
        except (OSError, ValueError):
            ready = False
        repair = False
        if ready:
            from .export_all import verify, Context, EXPORT_STEPS, StepFailed
            context = Context(root / 'GYARI', output, 1, False)
            try:
                for step, *_ in EXPORT_STEPS:
                    verify(context, step)
            except StepFailed:
                print('Hiányos előkészített adatok; újra elkészítem őket.', flush=True)
                ready, repair = False, True
        if not ready:
            if shutil.disk_usage(root).free < 5_000_000_000:
                raise SetupError('Az első indításhoz legalább 5 GB szabad hely szükséges ebben a mappában.')
            game = root / 'GYARI'
            if game.exists():
                check_install(game)
            else:
                game = extract_install(select_iso(root, args.iso), root, internal)
            ensure_media_converter(internal)
            print('[2/3] A pályák, modellek, menük, hangok és átvezetők előkészítése', flush=True)
            command = [sys.executable, '-X', 'utf8', '-m', 'tools.export_all',
                       '--game', str(game), '--output', str(output), '--skip-build']
            if repair:
                command.append('--force')
            code = subprocess.call(command, cwd=internal)
            if code:
                raise SetupError('Az előkészítés megszakadt. Indítsd újra az Indit.bat-ot: a kész lépéseket folytatja.')
            marker.write_text(json.dumps({'package': manifest['package']}), encoding='utf-8')
        if args.prepare_only:
            print('Az előkészítés kész. Indítás: Indit.bat', flush=True)
            return 0
        print('[3/3] A játék indul', flush=True)
        env = dict(os.environ)
        env.setdefault('WGPU_BACKEND', 'vulkan')
        command = [str(exe), 'menu', str(output)]
        if args.capture:
            env['MESTER_SILENT'] = '1'
            command += [str(args.capture.resolve()), '8']
        with (root / 'inditas.log').open('w', encoding='utf-8') as log:
            code = subprocess.call(command, cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT)
        if code:
            raise SetupError(f'A játék hibakóddal leállt ({code}). Részletek: inditas.log')
        return 0
    except (SetupError, OSError) as error:
        print(f'\nHIBA: {error}', flush=True)
        return 1
    except Exception as error:
        from .export_all import StepFailed
        if isinstance(error, StepFailed):
            print(f'\nHIBA: {error}\nIndítsd újra az Indit.bat-ot -- az előkészítés folytatható.', flush=True)
            return 1
        raise


if __name__ == '__main__':
    sys.exit(main())
