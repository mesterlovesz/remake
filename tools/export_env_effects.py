"""Export the DTX header texture effects the retail world renderer draws: EnvMap / EnvMapAlpha / DetailTex.

The effect is not a DTX flag: the 128-byte CommandString at header offset 36 names it (Lithtech.exe 0x465080..0x4652dd: `DetailTex` = 1,
`EnvMap` = 2, `EnvMapAlpha` = 3, followed by the path of the second texture). The detail scale is the header float at offset 30 and its angle
the int16 at 34. Writes `output/env_effects.json` (keyed by the base texture path, lower case, forward slashes) and the second textures as
`output/env_textures/*.png`. Additive: existing exports are never touched. See docs/retail-visual.md.

    python -m tools.export_env_effects [--game ../GYARI] [--output output]
"""
import argparse
import json
import struct
from pathlib import Path

from .export_visual import write_dtx_png

KINDS = {"detailtex": "detail", "envmap": "envmap", "envmapalpha": "envmapalpha"}


def normal(path: str) -> str:
    return path.replace("\\", "/").lstrip("/").casefold()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--game", default=Path(__file__).resolve().parents[2] / "GYARI")
    parser.add_argument("--output", default=Path(__file__).resolve().parents[1] / "output")
    args = parser.parse_args()
    game, output = Path(args.game), Path(args.output)
    index = {path.relative_to(game).as_posix().casefold(): path for path in game.rglob("*.dtx")}
    effects, written = {}, {}
    for key, path in sorted(index.items()):
        with path.open("rb") as stream:
            header = stream.read(164)
        if len(header) < 164:
            continue
        command = header[36:164].split(b"\0", 1)[0].decode("latin1").strip()
        tokens = command.split(None, 1)
        if not tokens or tokens[0].casefold() not in KINDS:
            continue
        kind, reference = KINDS[tokens[0].casefold()], normal(tokens[1]) if len(tokens) > 1 else ""
        target = index.get(reference) or index.get(reference + ".dtx")
        scale, angle = struct.unpack_from("<f", header, 30)[0], struct.unpack_from("<h", header, 34)[0]
        entry = {"kind": kind, "command": command, "texture": None, "scale": scale, "angle": angle}
        if target is not None:
            name = "env_textures/" + target.relative_to(game).with_suffix("").as_posix().casefold().replace("/", "_") + ".png"
            if name not in written:
                write_dtx_png(target, output / name)
                written[name] = True
            entry["texture"] = name
        effects[key] = entry
    (output / "env_effects.json").write_text(json.dumps(effects, indent=1, sort_keys=True), encoding="utf-8")
    kinds = {}
    for entry in effects.values():
        kinds[entry["kind"], entry["texture"] is not None] = kinds.get((entry["kind"], entry["texture"] is not None), 0) + 1
    print(f"{len(effects)} textures carry an effect command, {len(written)} second textures written; (kind, file found): {kinds}")


if __name__ == "__main__":
    main()
