"""Mirror the retail sounds tree into output/audio/sounds.

Earlier exporters copied only the sounds their JSON references. The remake's
gunfire and positional audio name retail paths directly (ricochets, casings,
hero hit), so every retail WAV is mirrored. Existing files are never replaced;
a differing file is reported and kept.
"""

import argparse
import filecmp
import shutil
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def mirror(source: Path, target: Path) -> tuple[int, int, list[Path]]:
    copied, identical, differing = 0, 0, []
    for path in sorted(source.rglob("*")):
        if not path.is_file() or path.name.casefold() == "dirtypesounds":
            continue
        destination = target / path.relative_to(source)
        if destination.exists():
            if filecmp.cmp(path, destination, shallow=False):
                identical += 1
            else:
                differing.append(destination)
            continue
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, destination)
        copied += 1
    return copied, identical, differing


def wav_mp3_stream(data: bytes) -> bytes | None:
    """The MPEG Layer-3 frame stream inside a WAV container (format tag 0x55), else None.

    The retail music files are MPEG Layer-3 in RIFF/WAVE (fmt 0x55, 22050 Hz
    stereo, 56 kbit/s); the `data` chunk is the plain MP3 stream, so no
    re-encoding is needed and Bevy's mp3 decoder plays it as `.mp3`.
    """
    if data[:4] != b"RIFF" or data[8:12] != b"WAVE":
        return None
    position, tag = 12, None
    while position + 8 <= len(data):
        chunk, size = data[position:position + 4], struct.unpack_from("<I", data, position + 4)[0]
        if chunk == b"fmt ":
            tag = struct.unpack_from("<H", data, position + 8)[0]
        elif chunk == b"data":
            return data[position + 8:position + 8 + size] if tag == 0x55 else None
        position += 8 + size + (size & 1)
    return None


def extract_music(source: Path, target: Path) -> tuple[int, int]:
    """sounds/muza/*.wav -> <target>/muza/<lowercase stem>.mp3; identical files are left alone."""
    written = skipped = 0
    for path in sorted((source / "muza").glob("*")):
        stream = wav_mp3_stream(path.read_bytes()) if path.is_file() else None
        if stream is None:
            continue
        destination = target / "muza" / f"{path.stem.lower()}.mp3"
        if destination.exists() and destination.read_bytes() == stream:
            skipped += 1
            continue
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(stream)
        written += 1
    return written, skipped


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--game", type=Path, default=ROOT.parent / "GYARI")
    parser.add_argument("--output", type=Path, default=ROOT / "output")
    args = parser.parse_args()
    copied, identical, differing = mirror(args.game / "sounds", args.output / "audio" / "sounds")
    print(f"copied={copied} identical={identical} differing={len(differing)}")
    for path in differing:
        print(f"kept differing export: {path}")
    written, skipped = extract_music(args.game / "sounds", args.output / "audio" / "sounds")
    print(f"music mp3 written={written} unchanged={skipped}")


if __name__ == "__main__":
    main()
