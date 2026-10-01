import struct
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from tools.export_audio import extract_music, wav_mp3_stream

GAME = Path(__file__).resolve().parents[2] / "GYARI"


def riff(tag: int, payload: bytes, extra: bytes = b"") -> bytes:
    fmt = struct.pack("<HHIIHH", tag, 2, 22050, 7000, 1, 0) + struct.pack("<H", len(extra)) + extra
    body = b"WAVE" + b"fmt " + struct.pack("<I", len(fmt)) + fmt + b"fact" + struct.pack("<II", 4, 9)
    body += b"data" + struct.pack("<I", len(payload)) + payload + (b"\0" if len(payload) & 1 else b"")
    return b"RIFF" + struct.pack("<I", len(body)) + body


class MusicExportTests(unittest.TestCase):
    def test_only_layer3_wave_data_is_extracted(self):
        stream = bytes.fromhex("fff3705400") + b"\x11" * 12
        self.assertEqual(wav_mp3_stream(riff(0x55, stream, b"\x01\0\2\0\0\0\0\0\0\0\1\0\1\0")), stream)
        self.assertIsNone(wav_mp3_stream(riff(1, b"\0\0")))
        self.assertIsNone(wav_mp3_stream(b"not a riff"))

    def test_retail_music_becomes_lowercase_mp3(self):
        source = GAME / "sounds"
        if not (source / "muza" / "prolog.wav").is_file():
            self.skipTest("retail install not available")
        with tempfile.TemporaryDirectory() as directory:
            written, _ = extract_music(source, Path(directory))
            self.assertEqual(written, 28)
            data = (Path(directory) / "muza" / "prolog.mp3").read_bytes()
            self.assertEqual(data[:2], b"\xff\xf3")
            self.assertTrue((Path(directory) / "muza" / "wiezienie1_spokoj.mp3").is_file())
            self.assertEqual(extract_music(source, Path(directory)), (0, 28))


if __name__ == "__main__":
    unittest.main()
