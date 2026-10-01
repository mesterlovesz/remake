"""Export DAT v85 render meshes and uncompressed DTX images as portable OBJ/PNG."""

import argparse
import json
import math
import re
from pathlib import Path
import struct
import zlib

from .lightmaps import face_uvs, pack, read_effect_groups, write_face_uvs
from .render_dat import read_render_nodes


def _png_chunk(kind: bytes, data: bytes) -> bytes:
    payload = kind + data
    return struct.pack(">I", len(data)) + payload + struct.pack(">I", zlib.crc32(payload))


def _rgb565(value: int) -> tuple[int, int, int]:
    r, g, b = (value >> 11) & 31, (value >> 5) & 63, value & 31
    return (r << 3 | r >> 2, g << 2 | g >> 4, b << 3 | b >> 2)


def _decode_dxt(data: bytes, width: int, height: int, alpha_block: bool) -> bytearray:
    """S3TC DXT1 (alpha_block=False) or DXT5 blocks to RGBA rows."""
    rgba = bytearray(width * height * 4)
    block = 16 if alpha_block else 8
    offset = 0
    for by in range(0, height, 4):
        for bx in range(0, width, 4):
            alphas = [255] * 16
            if alpha_block:
                a0, a1 = data[offset], data[offset + 1]
                bits = int.from_bytes(data[offset + 2:offset + 8], "little")
                table = [a0, a1] + ([((6 - i) * a0 + (1 + i) * a1) // 7 for i in range(6)] if a0 > a1
                                    else [((4 - i) * a0 + (1 + i) * a1) // 5 for i in range(4)] + [0, 255])
                alphas = [table[(bits >> (3 * i)) & 7] for i in range(16)]
            colour = offset + (8 if alpha_block else 0)
            c0, c1 = struct.unpack_from("<HH", data, colour)
            p0, p1 = _rgb565(c0), _rgb565(c1)
            if c0 > c1 or alpha_block:
                palette = [p0 + (255,), p1 + (255,), tuple((2 * a + b) // 3 for a, b in zip(p0, p1)) + (255,),
                           tuple((a + 2 * b) // 3 for a, b in zip(p0, p1)) + (255,)]
            else:
                palette = [p0 + (255,), p1 + (255,), tuple((a + b) // 2 for a, b in zip(p0, p1)) + (255,), (0, 0, 0, 0)]
            indices = struct.unpack_from("<I", data, colour + 4)[0]
            for i in range(16):
                x, y = bx + (i & 3), by + (i >> 2)
                if x < width and y < height:
                    r, g, b, a = palette[(indices >> (2 * i)) & 3]
                    rgba[(y * width + x) * 4:(y * width + x) * 4 + 4] = bytes((r, g, b, min(a, alphas[i])))
            offset += block
    return rgba


def write_rgb_png(target: Path, width: int, height: int, rgb: bytes) -> None:
    stride = width * 3
    scanlines = b"".join(b"\0" + rgb[y * stride:(y + 1) * stride] for y in range(height))
    header = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    Path(target).parent.mkdir(parents=True, exist_ok=True)
    Path(target).write_bytes(b"\x89PNG\r\n\x1a\n" + _png_chunk(b"IHDR", header)
                             + _png_chunk(b"IDAT", zlib.compress(scanlines, 6)) + _png_chunk(b"IEND", b""))


def write_dtx_png(source: Path, target: Path, *, preserve_zero_alpha=False) -> None:
    data = Path(source).read_bytes()
    # LithTech BPP codes: 3 = 32-bit BGRA, 4 = DXT1, 6 = DXT5.
    if len(data) < 164 or data[26] not in (3, 4, 6):
        raise ValueError(f"unsupported DTX image format: {source}")
    width, height = struct.unpack_from("<HH", data, 8)
    blocks = ((width + 3) // 4) * ((height + 3) // 4)
    size = width * height * 4 if data[26] == 3 else blocks * (8 if data[26] == 4 else 16)
    if width == 0 or height == 0 or len(data) < 164 + size:
        raise ValueError(f"invalid DTX dimensions or pixels: {source}")
    if data[26] == 3:
        bgra = data[164:164 + size]
        rgba = bytearray(len(bgra))
        rgba[0::4] = bgra[2::4]
        rgba[1::4] = bgra[1::4]
        rgba[2::4] = bgra[0::4]
        alpha = bgra[3::4]
        rgba[3::4] = alpha if preserve_zero_alpha or any(alpha) else b"\xff" * (width * height)
    else:
        rgba = _decode_dxt(data[164:164 + size], width, height, data[26] == 6)
    stride = width * 4
    scanlines = b"".join(b"\0" + rgba[y * stride:(y + 1) * stride] for y in range(height))
    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + _png_chunk(b"IHDR", header)
        + _png_chunk(b"IDAT", zlib.compress(scanlines, 6))
        + _png_chunk(b"IEND", b"")
    )


def dtx_content_u(source: Path):
    """Fraction of the DTX width in use before a uniform right-hand pad.

    Retail door atlases (e.g. drzwi.dtx: 96x160 art in a 128x256 image) pad
    the unused texels with one flat colour. The viewer mirrors U to correct
    handedness; it must mirror about this region, not the whole texture.
    Returns None when there is no such pad.
    """
    data = Path(source).read_bytes()
    if len(data) < 164 or data[26] != 3:
        return None
    width, height = struct.unpack_from("<HH", data, 8)
    if width < 8 or height == 0 or len(data) < 164 + width * height * 4:
        return None
    rows = [data[164 + y * width * 4:164 + (y + 1) * width * 4] for y in range(height)]
    pad = rows[0][(width - 1) * 4:width * 4 - 1]
    used = width
    while used > 0 and all(row[(used - 1) * 4:used * 4 - 1] == pad for row in rows):
        used -= 1
    if used == 0 or width - used < width // 8:
        return None
    return used / width


def material_metadata(image, section, model_object=None, effects=None):
    """Alpha is a rendering property, not a guess from RGB or DTX user flags.

    Retail transparent brushes and shoot-through sliding gates blend texture
    alpha. Other movers expose an Alpha/Alfa opacity scalar. DTX AlphaRef is
    the engine's explicit cutout command; EnvMapAlpha/fullbright alpha must
    never turn an ordinary wall into transparent geometry.
    """
    props = model_object.properties if model_object else {}
    kind = model_object.kind if model_object else ''
    factor = max(0.0, min(1.0, float(props.get('Alpha', props.get('Alfa', 1.0)))))
    flags, user_flags, command = 0, 0, ''
    if image is not None:
        with image.open('rb') as stream:
            header = stream.read(164)
        if len(header) >= 164:
            flags, user_flags = struct.unpack_from('<II', header, 16)
            command = header[36:164].split(b'\0', 1)[0].decode('latin1')
    reference = re.search(r'\bAlphaRef\s+(-?\d+)', command, re.IGNORECASE)
    mode = 'opaque'
    cutoff = .5
    if reference:
        mode = 'mask'
        # Direct3D's GREATER comparison rejects values equal to the reference.
        cutoff = min(1.0, max(0, int(reference[1]) + 1) / 255.0)
    if kind in {'b_transparent', 'b_transparent_nieprzestrzelny', 'b_szuflada_przestrzelna'} or factor < 1:
        mode = 'blend'
    if props.get('Additive'):
        mode = 'add'
    data = dict(alpha_mode=mode, alpha_cutoff=cutoff, alpha_factor=factor,
                source_texture=section.texture, source_shader=section.shader,
                source_class=kind, dtx_flags=flags, dtx_user_flags=user_flags,
                dtx_command=command)
    # DTX_FULLBRITE (bit 0): drawn without vertex colour or lightmap (IsFullbrite -> CRenderShader_*_Fullbright).
    data['fullbright'] = bool(flags & 1)
    if getattr(section, 'effect', '') and effects:
        group = effects.get(section.effect.lower())
        if group:
            data['texture_effect'] = dict(file=section.effect, **group)
    return data


def export_visual(source: Path, game_dir: Path, output: Path, nodes=None, asset_prefix='', model_object=None, available=None) -> tuple[Path, Path]:
    source, game_dir, output = Path(source), Path(game_dir), Path(output)
    nodes = read_render_nodes(source) if nodes is None else nodes
    output.mkdir(parents=True, exist_ok=True)
    obj_path = output / f"{source.stem}.visual.obj"
    mtl_path = output / f"{source.stem}.visual.mtl"
    image_dir = output / "textures" / source.stem
    available = available if available is not None else {
        path.relative_to(game_dir).as_posix().casefold(): path
        for path in game_dir.rglob("*.dtx")
    }
    effects = read_effect_groups(game_dir)
    materials = {}
    metadata = {}
    mtl_lines = []
    missing = []
    lines = [f"mtllib {mtl_path.name}", "o WorldRender"]
    vertex_base = 0
    rendered_triangles = 0
    lightmap_triangles = 0
    for node in nodes:
        for vertex in node.vertices:
            x, y, z = vertex.position
            # Sections whose texture the retail map compiler could not find carry inf/NaN UVs (u = texel / 0) in the DAT; those surfaces
            # have no base texture at run time (lightmap pass only), so the OBJ gets a valid 0 instead of a non-standard "inf"/"nan" token.
            u, v = (value if math.isfinite(value) else 0.0 for value in vertex.uv)
            nx, ny, nz = vertex.normal
            r, g, b = ((vertex.color >> shift & 255) / 255 for shift in (16, 8, 0))
            lines.extend((f"v {x:.9g} {y:.9g} {z:.9g} {r:.9g} {g:.9g} {b:.9g}", f"vt {u:.9g} {1 - v:.9g}", f"vn {nx:.9g} {ny:.9g} {nz:.9g}"))
        offset = 0
        for section in node.sections:
            # Shader 2 is the extra lightmap pass over the same surface, not
            # another opaque mesh. Retail autoexec.cfg has lightmaps=0.
            if section.shader == 2:
                lightmap_triangles += section.triangle_count
                offset += section.triangle_count
                continue
            texture = section.texture
            key = (texture.replace('\\', '/').casefold(), section.shader)
            if key not in materials:
                material = f"mat{len(materials):04d}"
                materials[key] = material
                mtl_lines.extend((f"newmtl {material}", "Kd 1 1 1", "d 1"))
                image = available.get(texture.replace("\\", "/").casefold())
                metadata[material] = material_metadata(image, section, model_object, effects)
                if image is not None:
                    target = image_dir / f"{material}.png"
                    try:
                        write_dtx_png(image, target, preserve_zero_alpha=metadata[material]['alpha_mode'] != 'opaque')
                        content_u = dtx_content_u(image)
                        if content_u is not None:
                            metadata[material]['content_u'] = content_u
                        mtl_lines.append(f"map_Kd {asset_prefix}textures/{source.stem}/{target.name}")
                    except ValueError:
                        missing.append(texture)
                elif texture.lower().endswith(".dtx"):
                    missing.append(texture)
                mtl_lines.append("")
            lines.append(f"usemtl {materials[key]}")
            for a, b, c, _ in node.triangles[offset:offset + section.triangle_count]:
                indices = (a + vertex_base + 1, b + vertex_base + 1, c + vertex_base + 1)
                lines.append("f " + " ".join(f"{index}/{index}/{index}" for index in indices))
            offset += section.triangle_count
            rendered_triangles += section.triangle_count
        vertex_base += len(node.vertices)
    obj_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    atlas, atlas_width, atlas_height, placements = pack(nodes)
    lightmap_faces = 0
    if placements:
        values = []
        for node_index, node in enumerate(nodes):
            values.extend(face_uvs(node_index, node, placements, (atlas_width, atlas_height)))
        lightmap_faces = sum(1 for i in range(0, len(values), 6) if values[i] == values[i])
        write_rgb_png(image_dir / "lightmap.png", atlas_width, atlas_height, atlas)
        write_face_uvs(output / f"{source.stem}.visual.lightmap.bin", values)
        (output / f"{source.stem}.visual.lightmap.json").write_text(json.dumps(
            {"atlas": f"{asset_prefix}textures/{source.stem}/lightmap.png", "width": atlas_width, "height": atlas_height,
             "faces": len(values) // 6, "lightmapped_faces": lightmap_faces, "uvs": f"{source.stem}.visual.lightmap.bin",
             "note": "6 little-endian f32 per exported face (u,v of its 3 vertices in the atlas, NaN = not lightmapped)"}, indent=2) + "\n", encoding="utf-8")
    mtl_path.write_text("\n".join(mtl_lines) + "\n", encoding="utf-8")
    (output / f"{source.stem}.visual.materials.json").write_text(
        json.dumps(metadata, indent=2) + '\n', encoding='utf-8')
    (output / f"{source.stem}.visual.report.json").write_text(
        json.dumps({"render_nodes": len(nodes), "triangles": rendered_triangles, "source_triangles": sum(len(node.triangles) for node in nodes), "lightmap_pass_triangles": lightmap_triangles, "lightmapped_faces": lightmap_faces, "materials": len(materials), "missing_textures": sorted(set(missing))}, indent=2) + "\n",
        encoding="utf-8",
    )
    return obj_path, mtl_path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("world", type=Path)
    parser.add_argument("game_dir", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    obj, mtl = export_visual(args.world, args.game_dir, args.output)
    print(obj)
    print(mtl)


if __name__ == "__main__":
    main()
