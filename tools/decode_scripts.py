"""Decode retail script text into a separate research copy.

The retail cipher exchanges adjacent letters (a/b, c/d, etc.) and digits
(0/1, 2/3, etc.). Applying it twice restores the original. Other characters
stay byte-for-byte equivalent when read and written with cp1250.
"""

import argparse
from pathlib import Path


SCRIPT_NAMES = ("items.txt", "objects.txt", "postacie.txt", "scenki.txt", "text_keys.txt")


def decode_text(source: str) -> str:
    def swap(character: str, start: str, end: str) -> str:
        if start <= character <= end:
            offset = ord(character) - ord(start)
            return chr(ord(start) + (offset ^ 1))
        return character

    return "".join(swap(swap(swap(char, "a", "z"), "A", "Z"), "0", "9") for char in source)


def decode_retail_scripts(game_dir: Path, output: Path) -> list[Path]:
    source_dir = Path(game_dir) / "scripts"
    output = Path(output)
    output.mkdir(parents=True, exist_ok=True)
    written = []
    for name in SCRIPT_NAMES:
        source = source_dir / name
        target = output / name
        target.write_text(decode_text(source.read_text(encoding="cp1250")), encoding="utf-8")
        written.append(target)
    return written


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("game_dir", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    for path in decode_retail_scripts(args.game_dir, args.output):
        print(path)


if __name__ == "__main__":
    main()


def read_script(path):
    """Accept plaintext and retail paired-letter encoded script distributions."""
    text = Path(path).read_text(encoding='cp1250')
    first = next((line.strip().split()[0] for line in text.splitlines()
                  if line.strip() and not line.lstrip().startswith('//')), '')
    return text if first in {'scena', 'postac', 'item', 'object', 'bool', 'level', 'dialog'} or first.startswith('>LN') else decode_text(text)
