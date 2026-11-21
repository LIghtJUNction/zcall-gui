#!/usr/bin/env python3
"""Convert the reviewed, local monochrome SVG assets to native triangles.

Developer-only dependencies: svgpathtools, shapely, numpy, mapbox-earcut.
Normal Cargo builds and the application need none of these packages.
SVG fill rules and holes are retained; zgpui accepts only convex polygons.
No network access is performed. See assets/agents/README.md for provenance.
"""
import json
import pathlib
import xml.etree.ElementTree as ET

import mapbox_earcut
import numpy as np
from shapely.geometry import LineString, Polygon, box as bounds
from shapely.ops import polygonize, unary_union
from svgpathtools import Document, Line

ROOT = pathlib.Path(__file__).resolve().parents[1]
ASSETS = ROOT / "assets/agents"


def flatten(segment, tolerance):
    if isinstance(segment, Line):
        return [segment.start, segment.end]

    def split(a, b, depth=0):
        start, end = segment.point(a), segment.point(b)
        chord = end - start
        points = [segment.point(a + (b - a) * t) for t in (.25, .5, .75)]
        error = max(
            abs((p - start).imag * chord.real - (p - start).real * chord.imag)
            / abs(chord) if abs(chord) else abs(p - start)
            for p in points
        )
        if error <= tolerance or depth >= 14:
            return [start, end]
        middle = (a + b) / 2
        return split(a, middle, depth + 1)[:-1] + split(middle, b, depth + 1)

    return split(0., 1.)


def winding(ring, point):
    x, y = point
    result = 0
    for (ax, ay), (bx, by) in zip(ring, ring[1:] + ring[:1]):
        cross = (bx - ax) * (y - ay) - (x - ax) * (by - ay)
        if ay <= y < by and cross > 0:
            result += 1
        elif by <= y < ay and cross < 0:
            result -= 1
    return result


def geometry(filename):
    root = ET.parse(filename).getroot()
    box = [float(v) for v in root.attrib["viewBox"].replace(",", " ").split()]
    bx, by, width, height = box
    scale = 1024 / max(width, height)
    # These inputs are explicitly reviewed filled silhouettes, not arbitrary SVG.
    unsupported = {"mask", "clipPath", "filter", "image", "text", "use"}
    for element in root.iter():
        if element.tag.split("}")[-1] in unsupported:
            raise ValueError(f"Unsupported SVG element in {filename}: {element.tag}")
        if any(key in element.attrib for key in ("clip-path", "mask", "filter")):
            raise ValueError(f"Normalize the clipping/filter in {filename} first")
        if element.attrib.get("stroke", "none") != "none":
            raise ValueError(f"Convert strokes to filled outlines in {filename} first")
    shapes = []
    for path in Document(str(filename)).paths():
        rings = []
        for subpath in path.continuous_subpaths():
            raw = []
            for segment in subpath:
                raw.extend(flatten(segment, .6 / scale)[:-1])
            raw.append(subpath.end)
            ring = []
            for point in raw:
                point = (
                    (point.real - bx) * scale + (1024 - width * scale) / 2,
                    (point.imag - by) * scale + (1024 - height * scale) / 2,
                )
                if not ring or point != ring[-1]:
                    ring.append(point)
            if ring and ring[0] == ring[-1]:
                ring.pop()
            if len(ring) >= 3:
                rings.append(ring)
        if not rings:
            continue
        rule = path.element.attrib.get("fill-rule", root.attrib.get("fill-rule", "nonzero"))
        lines = unary_union([LineString(ring + ring[:1]) for ring in rings])
        faces = []
        for face in polygonize(lines):
            point = face.representative_point().coords[0]
            count = sum(winding(ring, point) for ring in rings)
            if (count % 2 != 0) if rule == "evenodd" else (count != 0):
                faces.append(face)
        shapes.extend(faces)
    # SVG viewport clipping also applies to a few slightly overshooting curves.
    ox, oy = (1024 - width * scale) / 2, (1024 - height * scale) / 2
    result = unary_union(shapes).intersection(bounds(ox, oy, ox + width * scale, oy + height * scale))
    if result.is_empty or not result.is_valid:
        raise ValueError(f"No valid filled silhouette in {filename}")
    return result


def mesh(filename):
    shape = geometry(filename)
    polygons = [shape] if isinstance(shape, Polygon) else list(shape.geoms)
    points, lookup, triangles = [], {}, []
    for polygon in polygons:
        rings = [polygon.exterior, *polygon.interiors]
        vertices = np.array([point for ring in rings for point in list(ring.coords)[:-1]])
        ends = np.cumsum([len(ring.coords) - 1 for ring in rings], dtype=np.uint32)
        indices = mapbox_earcut.triangulate_float64(vertices, ends).reshape(-1, 3)
        for triangle in indices:
            output = []
            for index in triangle:
                point = tuple(round(float(v)) for v in vertices[index])
                if any(v < 0 or v > 1024 for v in point):
                    raise ValueError(f"Logo escapes its viewBox: {filename}, {point}")
                if point not in lookup:
                    lookup[point] = len(points)
                    points.append(point)
                output.append(lookup[point])
            a, b, c = [points[index] for index in output]
            if (b[0] - a[0]) * (c[1] - a[1]) != (b[1] - a[1]) * (c[0] - a[0]):
                triangles.append(output)
    if not triangles or len(points) >= 65536:
        raise ValueError(f"Invalid native mesh: {filename}")
    rebuilt = unary_union([Polygon([points[i] for i in triangle]) for triangle in triangles])
    # Quantizing to 1/1024 of an icon must not lose a meaningful filled region.
    if shape.symmetric_difference(rebuilt).area > 0.006 * 1024**2:
        raise ValueError(f"Triangulation changed the silhouette: {filename}")
    return points, triangles


def main():
    manifest = json.loads((ASSETS / "sources.json").read_text())
    result = [
        "// Generated by scripts/generate_agent_logos.py; do not edit by hand.",
        "// Source licenses and attribution: assets/agents/README.md.",
        "pub struct Mesh {",
        "    pub points: &'static [[u16; 2]],",
        "    pub triangles: &'static [[u16; 3]],",
        "}",
    ]
    identifiers = []
    for asset in manifest["assets"]:
        identifier = asset["agent_id"]
        filename = ASSETS / asset.get("render_file", asset["file"])
        points, triangles = mesh(filename)
        name = identifier.upper()
        result.extend(["#[rustfmt::skip]", f"const {name}: Mesh = Mesh {{"])
        for field, values in [("points", points), ("triangles", triangles)]:
            result.append(f"    {field}: &[")
            for offset in range(0, len(values), 6):
                result.append("        " + " ".join(
                    "[" + ", ".join(str(v) for v in value) + "],"
                    for value in values[offset:offset + 6]
                ))
            result.append("    ],")
        result.append("};")
        identifiers.append(identifier)
        print(f"{identifier}: {len(points)} vertices, {len(triangles)} triangles")
    result += ["pub fn get(id: &str) -> Option<&'static Mesh> {", "    match id {"]
    result += [f'        "{id}" => Some(&{id.upper()}),' for id in identifiers]
    result += ["        _ => None,", "    }", "}", ""]
    (ROOT / "src/agent_logos.rs").write_text("\n".join(result))


if __name__ == "__main__":
    main()
