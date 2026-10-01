"""Collect dependency notices and unchanged Cargo source archives for a release."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import zipfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--metadata', type=Path, required=True)
    parser.add_argument('--cargo-home', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--sources', type=Path, required=True)
    args = parser.parse_args()
    raw = args.metadata.read_bytes()
    metadata = json.loads(raw.decode('utf-16' if raw.startswith(b'\xff\xfe') else 'utf-8'))
    args.out.mkdir(parents=True, exist_ok=True)
    catalog = []
    with zipfile.ZipFile(args.sources, 'w', zipfile.ZIP_STORED) as archive:
        for package in sorted(metadata['packages'], key=lambda p: (p['name'], p['version'])):
            if not package['source']:
                continue
            key = package['name'] + '-' + package['version']
            root = Path(package['manifest_path']).parent
            target = args.out / key
            target.mkdir(exist_ok=True)
            for file in root.iterdir():
                if file.is_file() and file.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE', 'README')):
                    shutil.copyfile(file, target / file.name)
            if package.get('license_file'):
                file = root / package['license_file']
                shutil.copyfile(file, target / file.name)
            matches = list((args.cargo_home / 'registry' / 'cache').glob(f'*/{key}.crate'))
            if len(matches) != 1:
                raise RuntimeError(f'Missing or ambiguous Cargo source archive: {key}')
            file = matches[0]
            digest = hashlib.sha256(file.read_bytes()).hexdigest()
            archive.write(file, file.name)
            catalog.append({'name': package['name'], 'version': package['version'], 'license': package['license'],
                            'repository': package['repository'], 'source_sha256': digest})
        source_index = json.dumps(catalog, ensure_ascii=False, indent=2)
        archive.writestr('dependencies.json', source_index)
    (args.out / 'dependencies.json').write_text(source_index, encoding='utf-8')
    (args.out / 'SOURCES.txt').write_text(
        'Unchanged published Cargo source archives and license/readme notices for this release.\n'
        'Sources: https://github.com/mesterlovesz/remake/releases/download/v0.1.0-player.1/Rust-dependency-sources.zip\n'
        'The catalog includes all resolved platforms, including dependencies not used by the Windows binary.\n', encoding='utf-8')
    print(f'{len(catalog)} dependency archives collected: {args.sources}')


if __name__ == '__main__':
    main()
