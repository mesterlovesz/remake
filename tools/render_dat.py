"""Read the 44-byte LithTech Jupiter render vertices in retail DAT v83/v85 maps."""

from dataclasses import dataclass
from pathlib import Path

from .lithtech_dat import Reader


@dataclass(frozen=True)
class RenderSection:
    texture: str
    shader: int
    triangle_count: int
    # Texture effect group file (`pan.tfg`, GYARI/TextureEffectGroups) and, for shader 2 (the lightmap pass), the
    # lightmap size with its RLE data: shader 2 triangles carry the lightmap UV in `RenderVertex.uv`.
    effect: str = ""
    lightmap_size: tuple[int, int] = (0, 0)
    lightmap_data: bytes = b""


@dataclass(frozen=True)
class RenderVertex:
    position: tuple[float, float, float]
    uv: tuple[float, float]
    lightmap_uv: tuple[float, float]
    color: int
    normal: tuple[float, float, float]


@dataclass(frozen=True)
class LightGroup:
    """A named group of lights whose colour the game can change (`SetLightGroupColor`). `frames[section]` lists the lightmap
    rectangles (x, y, width, height, RLE intensities) the group adds to that section's lightmap; `vertex_stream` is the
    per-vertex intensity list of the node (0 n = skip n vertices, else the intensity of one vertex)."""
    name: str
    color: tuple[float, float, float]
    vertex_stream: bytes
    frames: dict


@dataclass(frozen=True)
class RenderNode:
    sections: tuple[RenderSection, ...]
    vertices: tuple[RenderVertex, ...]
    triangles: tuple[tuple[int, int, int, int], ...]
    light_groups: tuple[LightGroup, ...] = ()


def decode_lightmap(data: bytes, width: int, height: int) -> bytes:
    """RGB bytes (rows top to bottom) of one lightmap. Control byte: bit 7 set = repeat the next RGB (c & 127) + 1 times,
    else (c + 1) literal RGB pixels follow. Every retail lightmap decodes to exactly width*height pixels."""
    out = bytearray()
    position = 0
    while position < len(data) and len(out) < width * height * 3:
        control = data[position]
        position += 1
        if control & 0x80:
            out += data[position:position + 3] * ((control & 0x7F) + 1)
            position += 3
        else:
            count = control + 1
            out += data[position:position + 3 * count]
            position += 3 * count
    if len(out) != width * height * 3 or position != len(data):
        raise ValueError("lightmap data does not decode to its declared size")
    return bytes(out)


def _skip_portals(reader: Reader, count: int, occluder: bool) -> None:
    for _ in range(count):
        vertex_count = reader.read("<B")
        reader.bytes(vertex_count * 12 + 16 + (4 if occluder else 0))


def _read_light_groups(reader: Reader, count: int) -> tuple[LightGroup, ...]:
    groups = []
    for _ in range(count):
        name = reader.string("<H")
        color = reader.read("<3f")
        stream = reader.bytes(reader.read("<I"))
        frames = {}
        for section in range(reader.read("<I")):
            for _ in range(reader.read("<I")):
                x, y, width, height = reader.read("<4I")
                frames.setdefault(section, []).append((x, y, width, height, reader.bytes(reader.read("<I"))))
        groups.append(LightGroup(name, color, stream, frames))
    return tuple(groups)


def decode_light_frame(data: bytes, width: int, height: int) -> bytes:
    """One byte of light intensity per pixel (Lithtech.exe 0x516a60): `ff n v` repeats v n + 1 times, any other byte is one pixel."""
    out = bytearray()
    position = 0
    while position < len(data) and len(out) < width * height:
        if data[position] == 0xFF:
            out += bytes([data[position + 2]]) * (data[position + 1] + 1)
            position += 3
        else:
            out.append(data[position])
            position += 1
    if len(out) != width * height or position != len(data):
        raise ValueError("light group frame does not decode to its declared size")
    return bytes(out)


def _node(reader: Reader) -> RenderNode:
    reader.bytes(24)  # center and half extents
    sections = []
    for _ in range(reader.read("<I")):
        texture = reader.string("<H")
        reader.string("<H")  # secondary texture
        shader = reader.read("<B")
        triangles = reader.read("<I")
        effect = reader.string("<H")
        lightmap_width, lightmap_height, lightmap_length = reader.read("<3I")
        lightmap_data = reader.bytes(lightmap_length)
        sections.append(RenderSection(texture, shader, triangles, effect, (lightmap_width, lightmap_height), lightmap_data))
    vertices = []
    for _ in range(reader.read("<I")):
        position = reader.read("<3f")
        uv = reader.read("<2f")
        lightmap_uv = reader.read("<2f")
        color = reader.read("<I")
        normal = reader.read("<3f")
        vertices.append(RenderVertex(position, uv, lightmap_uv, color, normal))
    triangles = tuple(reader.read("<4I") for _ in range(reader.read("<I")))
    if sum(section.triangle_count for section in sections) != len(triangles):
        raise ValueError("render section triangle count mismatch")
    if any(index >= len(vertices) for triangle in triangles for index in triangle[:3]):
        raise ValueError("render triangle references a missing vertex")
    _skip_portals(reader, reader.read("<I"), False)
    _skip_portals(reader, reader.read("<I"), True)
    light_groups = _read_light_groups(reader, reader.read("<I"))
    reader.bytes(9)  # child flags and two child indices
    return RenderNode(tuple(sections), tuple(vertices), triangles, light_groups)


def _render_reader(path: str | Path) -> Reader:
    reader = Reader(Path(path).read_bytes())
    if reader.read("<I") not in (83, 85):
        raise ValueError("render reader supports DAT v83 and v85 only")
    reader.position = 24
    render_offset = reader.read("<I")
    if render_offset >= len(reader.data):
        raise ValueError("render offset outside DAT")
    reader.position = render_offset
    return reader

def read_render_nodes(path: str | Path) -> tuple[RenderNode, ...]:
    reader=_render_reader(path)
    count = reader.read("<I")
    if count > 10000:
        raise ValueError("invalid render node count")
    return tuple(_node(reader) for _ in range(count))

def read_model_render_nodes(path: str | Path) -> dict[str,tuple[RenderNode,...]]:
    reader=_render_reader(path)
    for _ in range(reader.read('<I')):_node(reader)
    result={}
    for _ in range(reader.read('<I')):
        name=reader.string('<H')
        result[name]=tuple(_node(reader) for _ in range(reader.read('<I')))
        if reader.read('<I')!=0:raise ValueError('unexpected world model child flag')
    return result
