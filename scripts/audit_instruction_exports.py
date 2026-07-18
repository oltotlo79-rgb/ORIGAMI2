#!/usr/bin/env python3
"""Independent parser audit for ORIGAMI2 instruction-export fixtures."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import zipfile
from pathlib import Path, PurePosixPath
from xml.etree import ElementTree

import fitz
from pypdf import PdfReader


FONT_SHA256 = "c2f3b4d463500a2ddcd3849cded1fceeb9fd6d1c32e6cbecd568453ba50fc68f"
LICENSE_SHA256 = "1c05c68c34f9708415aada51f17e1b0092d2cea709bf4a94cd38114f9e73d7d9"
LICENSE_PATH = "licenses/NotoSansJP-OFL.txt"
SVG_NAMESPACE = "http://www.w3.org/2000/svg"
FORBIDDEN_SVG_ELEMENTS = {
    "script",
    "foreignObject",
    "animate",
    "animateMotion",
    "animateTransform",
    "set",
    "image",
    "use",
    "text",
    "tspan",
}
EXPECTED_WARNINGS = [
    (
        "fixed_automatic_camera",
        "固定自動カメラで生成され、現在のカメラや作家指定カメラは使用されません。",
    ),
    (
        "visual_effects_omitted",
        "テクスチャ、照明、影、透明効果を省略し、単色の表裏色と白背景で描画します。",
    ),
    (
        "authored_guides_omitted",
        "カメラ遷移、矢印、注目箇所、指先、つまみ、押さえ、手の移動、持ち替えは出力されません。",
    ),
    (
        "discrete_step_endpoints_only",
        "各手順は保存済みの終端姿勢のみを表し、手順間の連続動作は出力されません。",
    ),
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    return parser.parse_args()


def audit_pdf(path: Path) -> None:
    data = path.read_bytes()
    if not data.startswith(b"%PDF-1.7\n"):
        raise AssertionError("PDF 1.7 header is missing")
    for forbidden in (b"/JavaScript", b"/Launch", b"/URI", b"/EmbeddedFile", b"/AcroForm"):
        if forbidden in data:
            raise AssertionError(f"forbidden PDF feature: {forbidden!r}")

    reader = PdfReader(path, strict=True)
    if len(reader.pages) != 2:
        raise AssertionError(f"unexpected PDF page count: {len(reader.pages)}")
    if reader.metadata is None or reader.metadata.title != "鶴の試作":
        raise AssertionError("PDF Unicode title did not round-trip")
    root = reader.trailer["/Root"]
    for forbidden_key in ("/OpenAction", "/AA", "/Names", "/AcroForm"):
        if forbidden_key in root:
            raise AssertionError(f"forbidden catalog key: {forbidden_key}")
    for page in reader.pages:
        width = float(page.mediabox.width)
        height = float(page.mediabox.height)
        if not math.isclose(width, 595.2755905511812, abs_tol=1.0e-6):
            raise AssertionError(f"unexpected PDF width: {width}")
        if not math.isclose(height, 841.8897637795277, abs_tol=1.0e-6):
            raise AssertionError(f"unexpected PDF height: {height}")
        if page.get("/Resources") != {}:
            raise AssertionError("PDF page unexpectedly references resources")

    document = fitz.open(path)
    try:
        if document.page_count != 2:
            raise AssertionError("MuPDF page count differs")
        for page in document:
            if not math.isclose(page.rect.width, 595.2755905511812, abs_tol=1.0e-3):
                raise AssertionError("MuPDF A4 width differs")
            if not math.isclose(page.rect.height, 841.8897637795277, abs_tol=1.0e-3):
                raise AssertionError("MuPDF A4 height differs")
            pixmap = page.get_pixmap(matrix=fitz.Matrix(0.25, 0.25), alpha=False)
            if pixmap.width <= 0 or pixmap.height <= 0:
                raise AssertionError("MuPDF could not render the page")
            if not any(sample < 250 for sample in pixmap.samples):
                raise AssertionError("MuPDF rendered an empty white PDF page")
    finally:
        document.close()


def audit_zip(path: Path) -> None:
    with zipfile.ZipFile(path) as archive:
        infos = archive.infolist()
        corrupt = archive.testzip()
        if corrupt is not None:
            raise AssertionError(f"ZIP CRC mismatch: {corrupt}")
        names = [info.filename for info in infos]
        if len(names) != len(set(names)):
            raise AssertionError("duplicate ZIP entry")
        if not names or names[0] != "manifest.json":
            raise AssertionError("manifest is not the first ZIP entry")
        if not names or names[-1] != LICENSE_PATH:
            raise AssertionError("license entry order differs")
        if any(name.startswith("fonts/") for name in names):
            raise AssertionError("unused font file is present in the SVG ZIP")
        for info in infos:
            candidate = PurePosixPath(info.filename)
            if (
                candidate.is_absolute()
                or ".." in candidate.parts
                or "\\" in info.filename
                or "\x00" in info.filename
            ):
                raise AssertionError(f"unsafe ZIP path: {info.filename!r}")
            if info.date_time != (1980, 1, 1, 0, 0, 0):
                raise AssertionError(f"non-canonical ZIP timestamp: {info.filename}")

        manifest = json.loads(archive.read("manifest.json"))
        if manifest["schema"] != "origami2.instruction-svg-pages.v2":
            raise AssertionError("manifest schema differs")
        if manifest["profile"] != "instruction_export_v1":
            raise AssertionError("manifest profile differs")
        if manifest["projection_profile"] != "orthographic_isometric_v1":
            raise AssertionError("projection profile differs")
        if manifest["title"] != "鶴の試作":
            raise AssertionError("manifest Unicode title differs")
        if manifest["page_count"] != len(manifest["pages"]) or manifest["step_count"] != 2:
            raise AssertionError("manifest counts differ")
        expected_pages = [entry["file"] for entry in manifest["pages"]]
        if names[1:-1] != expected_pages:
            raise AssertionError("manifest page mapping differs from ZIP order")
        if manifest["font"]["rendering"] != "glyph_outlines":
            raise AssertionError("manifest font rendering mode differs")
        if manifest["font"]["source_sha256"] != FONT_SHA256:
            raise AssertionError("manifest font source digest differs")
        if manifest["font"]["license_path"] != LICENSE_PATH:
            raise AssertionError("manifest font license path differs")
        if "path" in manifest["font"] or "sha256" in manifest["font"]:
            raise AssertionError("manifest retains the removed font-file contract")
        if manifest["font"]["license_sha256"] != LICENSE_SHA256:
            raise AssertionError("manifest license digest differs")
        warnings = [
            (warning.get("category"), warning.get("message_ja"))
            for warning in manifest.get("warnings", [])
        ]
        if warnings != EXPECTED_WARNINGS:
            raise AssertionError("manifest warning categories or messages differ")

        license_text = archive.read(LICENSE_PATH)
        if hashlib.sha256(license_text).hexdigest() != LICENSE_SHA256:
            raise AssertionError("license digest differs")

        for page_path in expected_pages:
            audit_svg(archive.read(page_path), page_path)


def audit_svg(data: bytes, page_path: str) -> None:
    root = ElementTree.fromstring(data)
    if root.tag != f"{{{SVG_NAMESPACE}}}svg":
        raise AssertionError(f"unexpected SVG root: {page_path}")
    if root.attrib.get("width") != "210mm" or root.attrib.get("height") != "297mm":
        raise AssertionError(f"unexpected SVG physical size: {page_path}")
    view_box = root.attrib.get("viewBox", "")
    if not re.fullmatch(r"0 0 595\.275591 841\.889764", view_box):
        raise AssertionError(f"unexpected SVG viewBox: {view_box!r}")

    decoded = data.decode("utf-8")
    if re.search(r"@font-face|font-family\s*:|url\s*\(|@import\b", decoded, re.IGNORECASE):
        raise AssertionError(f"SVG retains a font or external resource reference: {page_path}")
    page_group = next(
        (
            element
            for element in root
            if element.tag == f"{{{SVG_NAMESPACE}}}g"
            and element.attrib.get("id") == "page"
        ),
        None,
    )
    if page_group is None or not list(page_group):
        raise AssertionError(f"page group is missing: {page_path}")
    background = list(page_group)[0]
    if (
        background.tag != f"{{{SVG_NAMESPACE}}}polygon"
        or background.attrib.get("points")
        != "0,0 595.275591,0 595.275591,841.889764 0,841.889764"
        or background.attrib.get("fill") != "#ffffff"
        or background.attrib.get("stroke") != "#ffffff"
    ):
        raise AssertionError(f"explicit white A4 background is missing: {page_path}")

    text_run_count = 0
    glyph_count = 0
    for element in root.iter():
        local_name = element.tag.rsplit("}", 1)[-1]
        if local_name in FORBIDDEN_SVG_ELEMENTS:
            raise AssertionError(f"forbidden SVG element {local_name}: {page_path}")
        for name, value in element.attrib.items():
            if name.lower().startswith("on"):
                raise AssertionError(f"event handler attribute {name}: {page_path}")
            attribute_name = name.rsplit("}", 1)[-1].lower()
            if attribute_name in {"href", "src"} and value.strip().lower().startswith(
                ("http://", "https://", "data:", "//")
            ):
                raise AssertionError(
                    f"external SVG resource attribute {name}: {page_path}"
                )
        if local_name == "g" and element.attrib.get("data-text-run") == "1":
            text_run_count += 1
            glyph_count += audit_vector_text_run(element, page_path)

    if text_run_count == 0 or glyph_count == 0:
        raise AssertionError(f"vector text runs are missing: {page_path}")

    # Rendering an in-memory SVG provides neither the archived font file nor a
    # base URL. Because all visible glyphs are pinned vector paths and <text>
    # is forbidden above, this is independent of network and system fonts.
    document = fitz.open(stream=data, filetype="svg")
    try:
        if document.page_count != 1:
            raise AssertionError(f"unexpected rendered SVG page count: {page_path}")
        pixmap = document[0].get_pixmap(matrix=fitz.Matrix(0.25, 0.25), alpha=False)
        if pixmap.width <= 0 or pixmap.height <= 0:
            raise AssertionError(f"MuPDF could not render SVG: {page_path}")
        if not any(sample < 250 for sample in pixmap.samples):
            raise AssertionError(f"MuPDF rendered an empty white SVG: {page_path}")
    finally:
        document.close()


def audit_vector_text_run(element: ElementTree.Element, page_path: str) -> int:
    label = element.attrib.get("aria-label")
    if not label:
        raise AssertionError(f"vector text run has no accessible label: {page_path}")
    for attribute in ("data-baseline-y", "data-font-size"):
        value = parse_plain_number(element.attrib.get(attribute), page_path)
        if not math.isfinite(value):
            raise AssertionError(f"non-finite text metric {attribute}: {page_path}")

    scalars: list[str] = []
    expected_x: float | None = None
    glyphs = list(element)
    if not glyphs:
        raise AssertionError(f"empty vector text run: {page_path}")
    for glyph in glyphs:
        if (
            glyph.tag != f"{{{SVG_NAMESPACE}}}g"
            or glyph.attrib.get("data-glyph") != "1"
        ):
            raise AssertionError(f"unexpected vector text child: {page_path}")
        scalar_token = glyph.attrib.get("data-scalar", "")
        if not re.fullmatch(r"U\+[0-9A-F]{4,6}", scalar_token):
            raise AssertionError(f"invalid scalar token: {page_path}")
        scalar = chr(int(scalar_token[2:], 16))
        scalars.append(scalar)
        glyph_id = glyph.attrib.get("data-glyph-id", "")
        if not glyph_id.isdecimal() or int(glyph_id) <= 0:
            raise AssertionError(f"invalid glyph id: {page_path}")
        x = parse_plain_number(glyph.attrib.get("data-x"), page_path)
        advance = parse_plain_number(glyph.attrib.get("data-advance"), page_path)
        if advance < 0.0:
            raise AssertionError(f"negative glyph advance: {page_path}")
        if expected_x is not None and not math.isclose(x, expected_x, abs_tol=1.0e-6):
            raise AssertionError(f"glyph positions do not follow frozen advances: {page_path}")
        expected_x = x + advance

        paths = [
            child
            for child in glyph
            if child.tag == f"{{{SVG_NAMESPACE}}}path"
        ]
        if scalar.isspace():
            if paths:
                raise AssertionError(f"whitespace unexpectedly has an outline: {page_path}")
        elif len(paths) != 1 or not paths[0].attrib.get("d"):
            raise AssertionError(f"visible glyph outline is missing: {page_path}")

    if "".join(scalars) != label:
        raise AssertionError(f"vector glyph scalar order differs from label: {page_path}")
    return len(glyphs)


def parse_plain_number(value: str | None, page_path: str) -> float:
    if value is None or not re.fullmatch(r"-?(?:\d+(?:\.\d*)?|\.\d+)", value):
        raise AssertionError(f"invalid plain SVG number {value!r}: {page_path}")
    parsed = float(value)
    if not math.isfinite(parsed):
        raise AssertionError(f"non-finite SVG number {value!r}: {page_path}")
    return parsed


def main() -> None:
    directory = parse_args().directory.resolve(strict=True)
    audit_pdf(directory / "instruction-sample.pdf")
    audit_zip(directory / "instruction-sample.zip")
    print("instruction export external parser audit: OK")


if __name__ == "__main__":
    main()
