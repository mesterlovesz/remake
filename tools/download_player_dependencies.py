"""Maintainer-only download of the pinned Windows player dependencies."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import urllib.request

from .build_player_package import DEPENDENCIES


def download(url, target, expected=None):
    if not target.exists():
        print(f'Downloading {target.name}', flush=True)
        with urllib.request.urlopen(url, timeout=120) as response:
            data = response.read()
        if expected and hashlib.sha256(data).hexdigest() != expected:
            raise RuntimeError(f'Checksum mismatch: {target.name}')
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    if expected and hashlib.sha256(target.read_bytes()).hexdigest() != expected:
        raise RuntimeError(f'Checksum mismatch: {target.name}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, default=Path('player-deps'))
    args = parser.parse_args()
    folder = args.out.resolve()
    download('https://www.python.org/ftp/python/3.13.16/python-3.13.16-embed-amd64.zip',
             folder / 'python-3.13.16-embed-amd64.zip', DEPENDENCIES['python-3.13.16-embed-amd64.zip'])
    name = 'unshield-1.6.2-x64.7z'
    download(f'https://raw.githubusercontent.com/ScoopInstaller/Binary/master/unshield/{name}',
             folder / name, DEPENDENCIES[name])
    download('https://raw.githubusercontent.com/twogood/unshield/1.6.2/LICENSE', folder / 'unshield-LICENSE.txt')
    for project, version in [('pillow', '12.3.0'), ('pycdlib', '1.21.0'), ('imageio-ffmpeg', '0.6.0')]:
        with urllib.request.urlopen(f'https://pypi.org/pypi/{project}/{version}/json', timeout=60) as response:
            metadata = json.load(response)
        matches = [entry for entry in metadata['urls'] if entry['filename'] in DEPENDENCIES]
        if len(matches) != 1:
            raise RuntimeError(f'Pinned wheel unavailable: {project}')
        entry = matches[0]
        download(entry['url'], folder / 'wheels' / entry['filename'], DEPENDENCIES[entry['filename']])
    unshield = folder / 'unshield'
    unshield.mkdir(exist_ok=True)
    subprocess.run(['tar.exe', '-xf', str(folder / name), '-C', str(unshield)], check=True)


if __name__ == '__main__':
    main()
