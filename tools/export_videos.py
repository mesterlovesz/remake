"""Export the three retail start-up logo videos (1.avi Cenega, 2.avi Mirage, 3.avi Lithtech).

play1.exe (launcher stage) plays 1.avi, 2.avi, 3.avi in that order before lithtech.exe
starts the main menu (code order at 0x40108c, 0x4010aa, 0x4010c8). The AVIs use old
MPEG-4 codecs, so they are converted to JPEG frames plus an mp3 sound track that Bevy can
read: output/videos/<n>/0001.jpg.., output/videos/<n>.mp3 and output/videos/videos.json.
Needs ffmpeg (pip install imageio-ffmpeg); retail files are only read.
"""
import argparse
import json
import re
import subprocess
from pathlib import Path


def ffmpeg() -> str:
    import imageio_ffmpeg
    return imageio_ffmpeg.get_ffmpeg_exe()


def probe(exe: str, source: Path) -> dict:
    text = subprocess.run([exe, '-hide_banner', '-i', str(source)], capture_output=True, text=True).stderr
    video = re.search(r'Video:.*?, (\d+)x(\d+).*?([\d.]+) fps', text)
    duration = re.search(r'Duration: (\d+):(\d+):([\d.]+)', text)
    seconds = int(duration[1]) * 3600 + int(duration[2]) * 60 + float(duration[3])
    return {'width': int(video[1]), 'height': int(video[2]), 'fps': float(video[3]), 'seconds': seconds,
            'has_audio': 'Audio:' in text}


def export(game: Path, output: Path) -> dict:
    exe = ffmpeg()
    folder = output / 'videos'
    manifest = {'order': [1, 2, 3], 'clips': {}}
    for number in (1, 2, 3):
        source = game / f'{number}.avi'
        info = probe(exe, source)
        frames = folder / str(number)
        frames.mkdir(parents=True, exist_ok=True)
        for old in frames.glob('*.jpg'):
            old.unlink()
        subprocess.run([exe, '-v', 'error', '-y', '-i', str(source), '-vsync', 'cfr', '-r', str(info['fps']), '-q:v', '3',
                        str(frames / '%04d.jpg')], check=True)
        count = len(list(frames.glob('*.jpg')))
        audio = None
        if info['has_audio']:
            audio = f'videos/{number}.mp3'
            subprocess.run([exe, '-v', 'error', '-y', '-i', str(source), '-vn', '-c:a', 'libmp3lame', '-q:a', '2',
                            str(folder / f'{number}.mp3')], check=True)
        manifest['clips'][str(number)] = {**info, 'frames': count, 'dir': f'videos/{number}', 'audio': audio}
        print(number, manifest['clips'][str(number)])
    (folder / 'videos.json').write_text(json.dumps(manifest, indent=1), encoding='utf-8')
    return manifest


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('game', type=Path)
    parser.add_argument('output', type=Path)
    arguments = parser.parse_args()
    export(arguments.game, arguments.output)
