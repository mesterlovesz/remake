"""Retail world lightmaps (LithTech Jupiter render blocks) and texture effect groups.

A render block draws a lightmapped surface in two passes over the same polygons: shader 2 draws the section lightmap
(its own vertices, lightmap UV in `RenderVertex.uv`), shader 4 modulates the base texture with it (D3DBLEND_DESTCOLOR,
D3DBLEND_ZERO: texture * lightmap, vertex colour unused). The base pass triangles carry no lightmap UV, so each is matched to
its lightmap-pass twin by its vertex positions and winding. The lightmaps of a world are packed into one atlas so the viewer
binds a single texture; every lightmap keeps a replicated 1-texel border, which reproduces D3DTADDRESS_CLAMP.
"""
import re
import struct
from pathlib import Path

from .render_dat import decode_light_frame, decode_lightmap

PAD = 1
NAN = float("nan")


def add_light_groups(node, section_index, rgb, width, height):
    """The lightmap with every light group of the node at its authored colour: each frame adds int(intensity * colour) to its rectangle
    (`SetLightGroupColor`, Lithtech.exe 0x516a60, saturating at 255). Only podziemia1c has groups (LightGroup0, 1006 frames)."""
    rgb = bytearray(rgb)
    for group in node.light_groups:
        for x, y, w, h, data in group.frames.get(section_index, ()):
            intensity = decode_light_frame(data, w, h)
            for row in range(h):
                for column in range(w):
                    value = intensity[row * w + column]
                    if value and x + column < width and y + row < height:
                        at = ((y + row) * width + x + column) * 3
                        for channel in range(3):
                            rgb[at + channel] = min(255, rgb[at + channel] + int(value * group.color[channel]))
    return bytes(rgb)


def group_intensity(node, section_index, width, height):
    """Grey RGB of the summed light group frames of one lightmap (0 where no frame reaches), saturating at 255."""
    total = bytearray(width * height)
    for group in node.light_groups:
        for x, y, w, h, data in group.frames.get(section_index, ()):
            intensity = decode_light_frame(data, w, h)
            for row in range(h):
                for column in range(w):
                    if x + column < width and y + row < height:
                        at = (y + row) * width + x + column
                        total[at] = min(255, total[at] + intensity[row * w + column])
    return bytes(value for value in total for _ in range(3))


def pack(nodes, width=4096, groups="baked"):
    """Atlas RGB bytes, its size and the placement {(node, section): (x, y, w, h)} of every shader 2 lightmap.
    `groups`: "baked" adds every light group at its authored colour (the historical atlas), "base" leaves the groups out (what the
    game holds before `SetLightGroupColor`), "intensity" is the grey sum of the group frames alone. The layout is the same for all three."""
    items = []
    for node_index, node in enumerate(nodes):
        for section_index, section in enumerate(node.sections):
            w, h = section.lightmap_size
            if section.shader == 2 and w and h:
                if groups == "intensity":
                    rgb = group_intensity(node, section_index, w, h)
                else:
                    rgb = decode_lightmap(section.lightmap_data, w, h)
                    if node.light_groups and groups == "baked":
                        rgb = add_light_groups(node, section_index, rgb, w, h)
                items.append((h, w, node_index, section_index, rgb))
    items.sort(key=lambda item: (-item[0], -item[1], item[2], item[3]))
    placements, cursor_x, cursor_y, shelf = {}, 0, 0, 0
    for h, w, node_index, section_index, _ in items:
        if cursor_x + w + 2 * PAD > width:
            cursor_x, cursor_y, shelf = 0, cursor_y + shelf, 0
        placements[(node_index, section_index)] = (cursor_x + PAD, cursor_y + PAD, w, h)
        cursor_x += w + 2 * PAD
        shelf = max(shelf, h + 2 * PAD)
    height = max(cursor_y + shelf, 1)
    atlas = bytearray(width * height * 3)
    for h, w, node_index, section_index, rgb in items:
        x, y, _, _ = placements[(node_index, section_index)]
        for row in range(-PAD, h + PAD):
            source = min(max(row, 0), h - 1)
            line = rgb[source * w * 3:(source + 1) * w * 3]
            line = line[:3] * PAD + line + line[-3:] * PAD
            start = ((y + row) * width + x - PAD) * 3
            atlas[start:start + len(line)] = line
    return bytes(atlas), width, height, placements


def _canonical(points):
    first = points.index(min(points))
    return tuple(points[first:] + points[:first]), first


def face_uvs(node_index, node, placements, atlas_size):
    """Six floats per shader != 2 triangle, in exported face order: the atlas UV (u, v per vertex) of the lightmap under it.
    NaN where the surface has none (Gouraud sections)."""
    width, height = atlas_size
    position = lambda i: tuple(round(c, 2) for c in node.vertices[i].position)
    twins, offset, sections = {}, 0, []
    for section_index, section in enumerate(node.sections):
        triangles = node.triangles[offset:offset + section.triangle_count]
        offset += section.triangle_count
        sections.append((section, triangles))
        if section.shader != 2 or (node_index, section_index) not in placements:
            continue
        x, y, w, h = placements[(node_index, section_index)]
        for triangle in triangles:
            key, first = _canonical([position(i) for i in triangle[:3]])
            uvs = [node.vertices[i].uv for i in triangle[:3]]
            uvs = uvs[first:] + uvs[:first]
            twins[key] = [((x + min(max(u, 0.0), 1.0) * w) / width, (y + min(max(v, 0.0), 1.0) * h) / height) for u, v in uvs]
    result = []
    for section, triangles in sections:
        if section.shader == 2:
            continue
        for triangle in triangles:
            key, first = _canonical([position(i) for i in triangle[:3]])
            found = twins.get(key) if section.shader == 4 else None
            if found is None:
                result.extend([NAN] * 6)
                continue
            for j in range(3):
                result.extend(found[(j - first) % 3])
    return result


def write_face_uvs(path: Path, values) -> None:
    Path(path).write_bytes(struct.pack(f"<{len(values)}f", *values))


def read_effect_groups(game_dir: Path) -> dict:
    """`textures effect` group files: {name: {"script": "UVPan", "params": {"SpeedX": 0.002, ...}}}. The script text in
    TextureEffectGroups/<script>.txt declares the parameter names; the .tfg stores its float values after the script path."""
    folder = Path(game_dir) / "TextureEffectGroups"
    groups = {}
    for path in sorted(folder.glob("*.tfg")) if folder.is_dir() else []:
        data = path.read_bytes()
        length = struct.unpack_from("<H", data, 16)[0]
        script = re.split(r"[\\/]", data[18:18 + length].decode("latin1"))[-1]
        script = script.rsplit(".", 1)[0]
        source = folder / f"{script}.txt"
        names = []
        if source.is_file():
            match = re.search(r"UserParams\s+([^;]*);", source.read_text(errors="replace"))
            names = [n.strip() for n in match[1].split(",")] if match else []
        values = struct.unpack_from(f"<{len(names)}f", data, 18 + length + 4) if names else ()
        groups[path.name.lower()] = {"script": script, "params": dict(zip(names, (round(v, 9) for v in values)))}
    return groups
