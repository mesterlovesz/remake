"""Fit a painted face to the umbrella man's head UV layout (`cywil1_glowa`).

The prologue's umbrella man (`cywil1p`, rh3-miasteczko0) wears a personal face
as an easter egg. The painted source keeps the retail head's horizontal layout,
but its features sit higher than the model's face vertices, so in game the
eyes land on the forehead. This remaps rows so the eyes, mouth and chin meet
the vertices measured on `cywil1_parasol.ltb` (v = 0.545 / 0.69 / 0.795), and
extends the neck down to the bottom of the UV island, which the source leaves
black.

    python -m tools.fit_umbrella_face "<source.png>" output/mods/umbrella_face.png
"""
import argparse
from pathlib import Path

import numpy as np
from PIL import Image

# (target v, source v): target is the retail UV row, source is the painted row.
# The defaults are the ChatGPT source of 2026-09-29: eyes 0.478, mouth 0.635,
# chin 0.765. The hair above 0.25 and the teeth island stay where they are.
ROWS = [(0.0, 0.0), (0.25, 0.25), (0.545, 0.478), (0.69, 0.635), (0.795, 0.765), (1.0, 0.90)]


def extend_neck(pixels: np.ndarray, start: float = 0.72, dark: float = 62.0, blend: int = 12) -> np.ndarray:
    """Replace the dark fade under the neck with the column's own skin colour."""
    height, width = pixels.shape[:2]
    lum = pixels.astype(np.float32) @ np.array([0.299, 0.587, 0.114], np.float32)
    y0 = int(start * height)
    below = lum[y0:] < dark
    skin = lum[y0 - 8:y0].mean(axis=0) >= dark  # columns inside the head island
    edge = np.where(below.any(axis=0), y0 + below.argmax(axis=0), height)
    # The painted outline is a jagged polygon with a soft glow: start the fill
    # at a smoothed edge, a few rows before the glow.
    window = 20
    padded = np.pad(edge, window, mode="edge")
    smoothed = np.array([np.median(padded[x:x + 2 * window + 1]) for x in range(width)])
    edge = np.clip(np.minimum(edge, smoothed).astype(int) - 8, y0, height)
    filled = (skin & (edge < height)).astype(np.float32)
    colour = np.array([pixels[max(y0, e - 20):max(y0 + 1, e), x].mean(axis=0) for x, e in enumerate(edge)], np.float32)
    # Neighbouring columns differ; smooth the fill sideways so the stretched
    # neck has no vertical streaks.
    kernel = np.exp(-0.5 * (np.arange(-40, 41) / 14.0) ** 2)
    weight = np.convolve(filled, kernel, "same")
    smooth = np.stack([np.convolve(colour[:, c] * filled, kernel, "same") for c in range(3)], 1)
    smooth /= np.maximum(weight, 1e-6)[:, None]
    out = pixels.astype(np.float32)
    for x in np.flatnonzero(filled):
        top = max(y0, edge[x] - blend)
        ramp = np.clip((np.arange(top, height) - top) / blend, 0.0, 1.0)[:, None]
        out[top:, x] = out[top:, x] * (1.0 - ramp) + smooth[x] * ramp
    return np.clip(out + 0.5, 0, 255).astype(pixels.dtype)


def fit(source: Path, target: Path) -> None:
    image = Image.open(source).convert("RGB")
    pixels = extend_neck(np.asarray(image))
    height = pixels.shape[0]
    rows = (np.arange(height) + 0.5) / height
    wanted, painted = zip(*ROWS)
    source_rows = np.interp(rows, wanted, painted) * height - 0.5
    below = np.clip(np.floor(source_rows).astype(int), 0, height - 1)
    above = np.clip(below + 1, 0, height - 1)
    blend = (source_rows - below)[:, None, None]
    warped = pixels[below] * (1.0 - blend) + pixels[above] * blend
    target.parent.mkdir(parents=True, exist_ok=True)
    Image.fromarray(np.clip(warped + 0.5, 0, 255).astype(np.uint8)).save(target)
    print("wrote", target)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("source", type=Path)
    parser.add_argument("target", type=Path)
    arguments = parser.parse_args()
    fit(arguments.source, arguments.target)
