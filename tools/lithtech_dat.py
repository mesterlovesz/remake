"""Read the DAT v83/v85 world geometry and objects used by A Mesterlövész.

This is a small, independent reader for the sections the remake needs first.
The original files are never modified. Field layout was checked against the
retail levels and the public Kaitai LithTech DAT v85 schema.
"""

from dataclasses import dataclass
from pathlib import Path
import struct


@dataclass(frozen=True)
class Polygon:
    surface: int
    plane: int
    vertices: tuple[int, ...]


@dataclass(frozen=True)
class Model:
    name: str
    flags: int
    points: tuple[tuple[float, float, float], ...]
    planes: tuple[tuple[float, float, float, float], ...]
    surfaces: tuple[int, ...]
    polygons: tuple[Polygon, ...]
    # Per surface: the u16 texture flags. cshell reads them as
    # IntersectInfo.m_SurfaceFlags (0x10005ce0, the stack slot at esp+0x168
    # filled by ILTClient::IntersectSegment at 0x100062fc) to choose debris
    # and bullet holes: 0 stone, 1 sky, 2 metal, 3 wood, 4 water, 5 soft, 6 tiles.
    texture_flags: tuple[int, ...] = ()


@dataclass(frozen=True)
class WorldObject:
    kind: str
    properties: dict[str, object]


@dataclass(frozen=True)
class World:
    version: int
    models: tuple[Model, ...]
    objects: tuple[WorldObject, ...]


class Reader:
    def __init__(self, data: bytes, position: int = 0):
        self.data = data
        self.position = position

    def read(self, fmt: str):
        size = struct.calcsize(fmt)
        if self.position + size > len(self.data):
            raise ValueError(f"DAT ends at {self.position}, need {size} bytes")
        value = struct.unpack_from(fmt, self.data, self.position)
        self.position += size
        return value[0] if len(value) == 1 else value

    def bytes(self, count: int) -> bytes:
        if count < 0 or self.position + count > len(self.data):
            raise ValueError(f"invalid DAT byte count {count} at {self.position}")
        result = self.data[self.position:self.position + count]
        self.position += count
        return result

    def string(self, length_fmt: str) -> str:
        size = self.read(length_fmt)
        return self.bytes(size).decode("cp1250")

    def cstring(self) -> str:
        end = self.data.find(b"\0", self.position)
        if end < 0:
            raise ValueError(f"unterminated DAT string at {self.position}")
        return self.bytes(end - self.position + 1)[:-1].decode("cp1250")


def _model(reader: Reader, version: int = 85) -> Model:
    model_end = reader.read("<I")  # v83: next model offset; v85: reserved
    flags = reader.read("<I")
    name = reader.string("<H")
    points_count, planes_count, surfaces_count, portals_count, polygons_count, leaves_count, _, _, _, nodes_count = reader.read("<10I")
    reader.read("<9f")  # bounds and translation; vertices are world coordinates
    reader.read("<I")  # texture names byte size
    texture_names_count = reader.read("<I")
    for _ in range(texture_names_count):
        reader.cstring()
    lengths = reader.bytes(polygons_count)
    if version == 83:
        # v83 retains the visibility leaf lists removed in v85. Their payload
        # precedes planes; bound it using the authored next-model pointer and
        # the exact sizes of all following geometry records. No portal-bearing
        # v83 BSP is supported here (the retail Chinatown BSPs have none).
        if portals_count:
            raise ValueError("unsupported v83 BSP portals")
        geometry_size = (planes_count * 16 + surfaces_count * 8
                         + polygons_count * 8 + sum(lengths) * 4
                         + nodes_count * 14 + points_count * 12 + 8)
        visibility_size = model_end - reader.position - geometry_size
        if visibility_size < 0 or (not leaves_count and visibility_size):
            raise ValueError(f"invalid v83 model boundary in {name}")
        reader.bytes(visibility_size)
    planes = tuple(reader.read("<4f") for _ in range(planes_count))
    # Each surface is: flags u32, texture index u16, texture flags u16.
    surface_records = tuple(_surface(reader) for _ in range(surfaces_count))
    surfaces = tuple(flags for flags, _ in surface_records)
    texture_flags = tuple(texture for _, texture in surface_records)
    polygons = []
    for count in lengths:
        if count < 3:
            raise ValueError(f"invalid polygon with {count} vertices in {name}")
        surface, plane = reader.read("<2I")
        polygons.append(Polygon(surface, plane, reader.read(f"<{count}I")))
    reader.bytes(nodes_count * 14)
    points = tuple(reader.read("<3f") for _ in range(points_count))
    reader.read("<iI")  # BSP root index, sections
    if version == 83 and reader.position != model_end:
        raise ValueError(f"v83 model boundary mismatch in {name}")
    for polygon in polygons:
        if polygon.surface >= surfaces_count or polygon.plane >= planes_count:
            raise ValueError(f"invalid polygon plane/surface in {name}")
        if any(index >= points_count for index in polygon.vertices):
            raise ValueError(f"invalid polygon vertex in {name}")
    return Model(name, flags, points, planes, surfaces, tuple(polygons), texture_flags)


def _surface(reader: Reader) -> tuple[int, int]:
    flags = reader.read("<I")
    _, texture_flags = reader.read("<HH")
    return flags, texture_flags


def _property(reader: Reader):
    name = reader.string("<H")
    kind = reader.read("<B")
    reader.read("<I")  # property flags
    size = reader.read("<H")
    raw = reader.bytes(size)
    value_reader = Reader(raw)
    if kind == 0:
        value = value_reader.string("<H")
    elif kind == 1:
        value = value_reader.read("<3f")
    elif kind == 2:
        value = value_reader.read("<3f")
    elif kind == 3:
        value = value_reader.read("<f")
    elif kind == 4:
        value = value_reader.read("<I")
    elif kind == 5:
        value = value_reader.read("<B")
    elif kind == 6:
        value = value_reader.read("<i")
    elif kind == 7:
        value = value_reader.read("<4f")
    else:
        value = {"unknown_type": kind, "bytes_hex": raw.hex()}
    return name, value


def read_world(path: str | Path) -> World:
    reader = Reader(Path(path).read_bytes())
    version, objects_at, _, _, _, _, _ = reader.read("<7I")
    if version == 70:
        # katscena.dat (LithTech 1.5 era prototype of the intro cutscene): the object list is laid out like v83/v85, the world
        # tree and models are not decoded, so only the objects are read.
        return World(version, (), _read_objects(reader, objects_at))
    if version not in (83, 85):
        raise ValueError(f"unsupported LithTech DAT version {version}; expected 70 (objects only), 83 or 85")
    reader.bytes(8 * 4)
    reader.string("<I")  # world info
    # v83 has a leading world scalar and bounds; v85 adds the source offset.
    reader.read("<7f" if version == 83 else "<9f")
    reader.read("<6f")  # world tree bounds
    subnodes, _ = reader.read("<2I")
    reader.bytes(subnodes // 8 + 1)
    models_count = reader.read("<I")
    models = tuple(_model(reader, version) for _ in range(models_count))
    return World(version, models, _read_objects(reader, objects_at))


def _read_objects(reader: Reader, objects_at: int):
    if objects_at >= len(reader.data):
        raise ValueError("world object offset outside DAT")
    reader.position = objects_at
    objects_count = reader.read("<I")
    objects = []
    for _ in range(objects_count):
        reader.read("<H")  # object record length
        kind = reader.string("<H")
        properties_count = reader.read("<I")
        properties = dict(_property(reader) for _ in range(properties_count))
        objects.append(WorldObject(kind, properties))
    return tuple(objects)
