"""Bundle a compiled Windows game and offline ISO exporter. No retail assets are copied."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import zipfile


DEPENDENCIES = {
    'python-3.13.16-embed-amd64.zip': '97dae5274cc54867065e8d5a3226e48c35017ed332a0fdb0e27d5b5821961297',
    'pillow-12.3.0-cp313-cp313-win_amd64.whl': '1cca606cd25738df4ed873d5ad46bbdb3d83b5cbca291f6b4ff13a4df6b0bbe8',
    'pycdlib-1.21.0-py3-none-any.whl': '1a82cd735542921d0dd7bc40381e7b7b9cbc70bd4c03e4f67a0531d59d2f518e',
    'imageio_ffmpeg-0.6.0-py3-none-win_amd64.whl': '02fa47c83703c37df6bfe4896aab339013f62bf02c5ebf2dce6da56af04ffc0a',
    'unshield-1.6.2-x64.7z': '662fefefc6988a3744ddc6b5b8d2e96ac21876d532f7d23a597c4ca797764a6b',
}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def build(source: Path, exe: Path, dependencies: Path, out: Path):
    if out.exists():
        raise SystemExit(f'Choose a new output directory: {out}')
    for name, expected in DEPENDENCIES.items():
        path = dependencies / ('wheels' if name.endswith('.whl') else '') / name
        if sha(path) != expected:
            raise SystemExit(f'Dependency checksum mismatch: {name}')
    internal = out / '.player'
    runtime = internal / 'python'
    runtime.mkdir(parents=True)
    shutil.copytree(dependencies / 'rust-licenses', internal / 'licenses' / 'rust')
    python_zip = dependencies / 'python-3.13.16-embed-amd64.zip'
    if sha(python_zip) != '97dae5274cc54867065e8d5a3226e48c35017ed332a0fdb0e27d5b5821961297':
        raise SystemExit('Python runtime checksum mismatch')
    with zipfile.ZipFile(python_zip) as archive:
        archive.extractall(runtime)
    for wheel in sorted((dependencies / 'wheels').glob('*.whl')):
        if wheel.name not in DEPENDENCIES:
            raise SystemExit(f'Unexpected dependency: {wheel.name}')
        with zipfile.ZipFile(wheel) as archive:
            # Fetch the GPL media executable directly from upstream on first run.
            # We redistribute the BSD Python wrapper, not the Gyan binary.
            for entry in archive.infolist():
                if not entry.filename.lower().endswith('.exe'):
                    archive.extract(entry, runtime / 'Lib' / 'site-packages')
    (runtime / 'python313._pth').write_text('python313.zip\n.\nLib/site-packages\n..\n', encoding='ascii')
    shutil.copytree(dependencies / 'unshield', internal / 'unshield')
    shutil.copyfile(dependencies / 'unshield-LICENSE.txt', internal / 'unshield' / 'LICENSE.txt')
    shutil.copyfile(Path(__file__).resolve().parent.parent / 'packaging' / 'zlib-LICENSE.txt',
                    internal / 'unshield' / 'zlib-LICENSE.txt')
    shutil.copyfile(exe, internal / 'Mesterlovesz.exe')
    # The game may import the Microsoft C runtime; Python ships its redistributable DLLs.
    for dll in runtime.glob('vcruntime*.dll'):
        shutil.copyfile(dll, internal / dll.name)
    tracked = subprocess.check_output(['git', '-C', str(source), 'ls-files', 'tools'], text=True).splitlines()
    for name in tracked:
        if name.endswith('.py'):
            target = internal / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source / name, target)
    own = Path(__file__).resolve().parent
    for name in ('export_all.py', 'setup_check.py', 'player_launcher.py'):
        shutil.copyfile(own / name, internal / 'tools' / name)
    shutil.copyfile(own.parent / 'packaging' / 'Indit.bat', out / 'Indit.bat')
    digest = hashlib.sha256()
    for path in sorted(internal.rglob('*')):
        if path.is_file():
            digest.update(str(path.relative_to(internal)).encode())
            digest.update(bytes.fromhex(sha(path)))
    dependencies_used = {p.name: sha(p) for p in [python_zip, *sorted((dependencies / 'wheels').glob('*.whl')),
                                                dependencies / 'unshield-1.6.2-x64.7z']}
    manifest = {'version': '0.1.0-player.2', 'package': digest.hexdigest(), 'engine_sha256': sha(exe),
                'engine_source_commit': subprocess.check_output(['git', '-C', str(source), 'rev-parse', 'HEAD'], text=True).strip(),
                'dependencies': dependencies_used}
    (internal / 'package.json').write_text(json.dumps(manifest, indent=2), encoding='utf-8')
    (out / 'OLVASS-EL.txt').write_text(
        'A Mesterlövész: Újratöltve — Windows játékoscsomag\n\n'
        '1. Csomagold ki a ZIP teljes tartalmát egy írható mappába.\n'
        '2. Töltsd le a magyar ISO-t, és tedd az Indit.bat mellé:\n'
        '   https://archive.org/download/a_mesterlovesz_pc_windows_thesniper_hu_iso_202010/A_mesterlovesz.iso\n'
        '3. Dupla kattintás az Indit.bat-ra. Az első előkészítés néhány perc; utána indul a játék.\n\n'
        'Első alkalommal internet szükséges a médiafeldolgozó automatikus letöltéséhez (31 MB).\n'
        'A további indításokhoz már nem szükséges internet.\n'
        'Nem kell Rust, Python, az eredeti telepítő vagy rendszergazdai jog.\n'
        'Windows 10/11 64 bit, Vulkan-képes videokártya/illesztő és az első indításhoz 5 GB szabad hely kell.\n'
        'Az ISO és a GYARI fájlok olvasva vannak; a csomag nem tartalmaz eredeti játékadatokat.\n'
        'Első indítás után az ISO eltávolítható. A mentések és beállítások az output mappában vannak.\n'
        'Elakadáskor indítsd újra az Indit.bat-ot: a kész lépéseket kihagyja.\n'
        'Az output és a .player mappát hagyd az indító mellett. Naplók: output/.setup/logs és inditas.log.\n\n'
        'Web: https://sniper.gay/ — Forrás és hibajelentés: https://github.com/mesterlovesz/remake\n', encoding='utf-8')
    (out / 'THIRD-PARTY-NOTICES.txt').write_text(
        'Bundled components (replaceable; license texts are included in their package directories):\n'
        'Python 3.13.16: PSF License; .player/python/LICENSE.txt; https://www.python.org/downloads/release/python-31316/\n'
        'Pillow 12.3.0: HPND; .player/python/Lib/site-packages/pillow-12.3.0.dist-info/licenses/\n'
        'PyCdlib 1.21.0: LGPL-2.1; complete Python sources and license in site-packages; https://github.com/clalancette/pycdlib\n'
        'Unshield 1.6.2: MIT; .player/unshield/LICENSE.txt; https://github.com/twogood/unshield/tree/1.6.2\n'
        'Zlib: zlib license; .player/unshield/zlib-LICENSE.txt; https://zlib.net/zlib_license.html\n'
        'Imageio-ffmpeg 0.6.0: BSD-2-Clause; site-packages/imageio_ffmpeg-0.6.0.dist-info/LICENSE\n'
        'FFmpeg is NOT bundled. On first run the unmodified 0.6.0 wheel above is downloaded directly from PyPI,\n'
        'and its Gyan FFmpeg 7.1 converter is extracted for the user. It is GPL-3.0:\n'
        'https://github.com/imageio/imageio-ffmpeg/tree/v0.6.0 ; https://www.gyan.dev/ffmpeg/builds/\n'
        'https://github.com/FFmpeg/FFmpeg/tree/n7.1 ; https://ffmpeg.org/legal.html\n'
        'Rust game dependencies: .player/licenses/rust/ contains original notices, READMEs and the license catalog.\n'
        'Unchanged corresponding Cargo source archives are available in Rust-dependency-sources.zip in the same release:\n'
        'https://github.com/mesterlovesz/remake/releases/tag/v0.1.0-player.2\n'
        'The game and exporters are available at https://github.com/mesterlovesz/remake\n', encoding='utf-8')
    print(json.dumps(manifest, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--exe', type=Path, required=True)
    parser.add_argument('--dependencies', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    build(args.source.resolve(), args.exe.resolve(), args.dependencies.resolve(), args.out.resolve())
