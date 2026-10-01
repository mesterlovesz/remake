"""Export the dynamic light group layer of a world (Lithtech.exe `SetLightGroupColor` 0x516a60): additive to the existing lightmap export.

The retail lightmap of a section is the stored one plus, for every light group frame, `int(intensity * group colour)` per channel (saturating at 255);
`SetLightGroupColor` (sent by the LightGroup object, object.lto 0x10010200) changes the colour at run time. The historical `lightmap.png` has the group
baked in at its authored colour. This writes, for worlds that have groups (only podziemia1c):
  textures/<stem>/lightmap_base.png   the lightmaps without the groups (same layout as lightmap.png)
  textures/<stem>/lightgroup.png      the summed group frames as grey intensity (same layout)
  <stem>.visual.lightgroup.bin        one u8 group intensity per OBJ vertex (the per-vertex stream for Gouraud surfaces: `0 n` skips n + 1 vertices)
and adds a `light_group` entry to `<stem>.visual.lightmap.json`.

    python -m tools.export_light_groups ../GYARI/worlds/podziemia1c.dat output
"""
import argparse
import json
from pathlib import Path

from .export_visual import write_rgb_png
from .lightmaps import pack
from .render_dat import read_render_nodes


def vertex_intensities(node) -> bytes:
    """Per-vertex group intensity of a node: the stream is `0 n` = n + 1 vertices at 0, any other byte = one vertex's intensity."""
    values = bytearray(len(node.vertices))
    for group in node.light_groups:
        stream, at, index = group.vertex_stream, 0, 0
        while at < len(stream) and index < len(values):
            if stream[at] == 0:
                index += stream[at + 1] + 1
                at += 2
            else:
                values[index] = min(255, values[index] + stream[at])
                index += 1
                at += 1
    return bytes(values)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("world", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    stem, output = args.world.stem, args.output
    nodes = read_render_nodes(args.world)
    groups = [group for node in nodes for group in node.light_groups]
    if not groups:
        print(f"{stem}: no light groups")
        return
    names = {group.name for group in groups}
    base, width, height, _ = pack(nodes, groups="base")
    intensity = pack(nodes, groups="intensity")[0]
    image_dir = output / "textures" / stem
    write_rgb_png(image_dir / "lightmap_base.png", width, height, base)
    write_rgb_png(image_dir / "lightgroup.png", width, height, intensity)
    (output / f"{stem}.visual.lightgroup.bin").write_bytes(b"".join(vertex_intensities(node) for node in nodes))
    info_path = output / f"{stem}.visual.lightmap.json"
    info = json.loads(info_path.read_text(encoding="utf-8"))
    info["light_group"] = {
        "names": sorted(names), "authored_color": list(groups[0].color),
        "base": f"textures/{stem}/lightmap_base.png", "intensity": f"textures/{stem}/lightgroup.png", "vertices": f"{stem}.visual.lightgroup.bin",
        "note": "lightmap = base + int(intensity * colour) per channel (colour set at run time by SetLightGroupColor); Gouraud surfaces add vertices[i] * colour to their vertex colour",
    }
    info_path.write_text(json.dumps(info, indent=2) + "\n", encoding="utf-8")
    print(f"{stem}: {len(groups)} light groups {sorted(names)}, atlas {width}x{height}, {sum(len(n.vertices) for n in nodes)} vertices")


if __name__ == "__main__":
    main()
