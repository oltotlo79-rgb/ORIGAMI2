use std::{cmp::Ordering, collections::HashMap};

use ori_domain::{CreasePattern, EdgeKind, Paper, Point2, VertexId};

use super::CreasePatternExportError;

const CRLF: &str = "\r\n";
const MAX_DXF_GROUP_PAIRS: usize = 100_000;
const MAX_DXF_VALUE_BYTES: usize = 255;
const MAX_DXF_REAL_BYTES: usize = 64;
const MAX_DXF_TITLE_BYTES: usize = 2_048;
const MAX_DXF_TITLE_CHUNK_BYTES: usize = 224;
const MAX_DXF_TITLE_CHUNKS: usize = 10;
const TITLE_MARKER: &str = "ORIGAMI2_EXPORT AC1021";
const TITLE_CHUNK_PREFIX: &str = "ORIGAMI2_TITLE ";

#[derive(Debug, Clone, Copy)]
struct DxfInternalError;

type DxfResult<T> = Result<T, DxfInternalError>;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Bounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct CanonicalLine {
    kind: EdgeKind,
    start: Point2,
    end: Point2,
}

#[derive(Debug, Default)]
struct BoundsAccumulator {
    bounds: Option<Bounds>,
}

impl BoundsAccumulator {
    fn include(&mut self, point: Point2) -> DxfResult<()> {
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(DxfInternalError);
        }
        match &mut self.bounds {
            Some(bounds) => {
                bounds.min_x = bounds.min_x.min(point.x);
                bounds.min_y = bounds.min_y.min(point.y);
                bounds.max_x = bounds.max_x.max(point.x);
                bounds.max_y = bounds.max_y.max(point.y);
            }
            None => {
                self.bounds = Some(Bounds {
                    min_x: point.x,
                    min_y: point.y,
                    max_x: point.x,
                    max_y: point.y,
                });
            }
        }
        Ok(())
    }

    fn finish(self) -> DxfResult<Bounds> {
        let bounds = self.bounds.ok_or(DxfInternalError)?;
        let width = bounds.max_x - bounds.min_x;
        let height = bounds.max_y - bounds.min_y;
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return Err(DxfInternalError);
        }
        Ok(Bounds {
            min_x: canonical_zero(bounds.min_x),
            min_y: canonical_zero(bounds.min_y),
            max_x: canonical_zero(bounds.max_x),
            max_y: canonical_zero(bounds.max_y),
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct LinetypeDefinition {
    handle: &'static str,
    name: &'static str,
    description: &'static str,
    pattern: &'static [f64],
}

const MOUNTAIN_PATTERN: &[f64] = &[8.0, -2.0, 0.0, -2.0];
const VALLEY_PATTERN: &[f64] = &[6.0, -3.0];
const AUXILIARY_PATTERN: &[f64] = &[0.0, -2.0];
const CUT_PATTERN: &[f64] = &[8.0, -2.0, 2.0, -2.0];

const LINETYPE_DEFINITIONS: &[LinetypeDefinition] = &[
    LinetypeDefinition {
        handle: "2",
        name: "BYBLOCK",
        description: "ByBlock",
        pattern: &[],
    },
    LinetypeDefinition {
        handle: "3",
        name: "BYLAYER",
        description: "ByLayer",
        pattern: &[],
    },
    LinetypeDefinition {
        handle: "4",
        name: "CONTINUOUS",
        description: "Solid line",
        pattern: &[],
    },
    LinetypeDefinition {
        handle: "5",
        name: "ORI_MOUNTAIN",
        description: "Origami mountain fold",
        pattern: MOUNTAIN_PATTERN,
    },
    LinetypeDefinition {
        handle: "6",
        name: "ORI_VALLEY",
        description: "Origami valley fold",
        pattern: VALLEY_PATTERN,
    },
    LinetypeDefinition {
        handle: "7",
        name: "ORI_AUXILIARY",
        description: "Origami auxiliary line",
        pattern: AUXILIARY_PATTERN,
    },
    LinetypeDefinition {
        handle: "8",
        name: "ORI_CUT",
        description: "Origami cut line",
        pattern: CUT_PATTERN,
    },
];

#[derive(Debug, Clone, Copy)]
struct LayerDefinition {
    handle: &'static str,
    name: &'static str,
    color: i64,
    linetype: &'static str,
}

const LAYER_DEFINITIONS: &[LayerDefinition] = &[
    LayerDefinition {
        handle: "A",
        name: "0",
        color: 7,
        linetype: "CONTINUOUS",
    },
    LayerDefinition {
        handle: "B",
        name: "ORIGAMI_BOUNDARY",
        color: 7,
        linetype: "CONTINUOUS",
    },
    LayerDefinition {
        handle: "C",
        name: "ORIGAMI_MOUNTAIN",
        color: 1,
        linetype: "ORI_MOUNTAIN",
    },
    LayerDefinition {
        handle: "D",
        name: "ORIGAMI_VALLEY",
        color: 5,
        linetype: "ORI_VALLEY",
    },
    LayerDefinition {
        handle: "E",
        name: "ORIGAMI_AUXILIARY",
        color: 8,
        linetype: "ORI_AUXILIARY",
    },
    LayerDefinition {
        handle: "F",
        name: "ORIGAMI_CUT",
        color: 6,
        linetype: "ORI_CUT",
    },
];

trait PairSink {
    fn text(&mut self, code: u16, value: &str) -> DxfResult<()>;
    fn integer(&mut self, code: u16, value: i64) -> DxfResult<()>;
    fn real(&mut self, code: u16, value: f64) -> DxfResult<()>;
}

#[derive(Debug)]
struct DxfWriter {
    output: String,
    pair_count: usize,
}

impl DxfWriter {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            output: String::with_capacity(capacity),
            pair_count: 0,
        }
    }

    fn into_bytes(self) -> Vec<u8> {
        self.output.into_bytes()
    }

    fn raw_pair(&mut self, code: u16, value: &str) -> DxfResult<()> {
        if value.len() > MAX_DXF_VALUE_BYTES
            || value.contains('\r')
            || value.contains('\n')
            || self.pair_count >= MAX_DXF_GROUP_PAIRS
        {
            return Err(DxfInternalError);
        }
        self.pair_count = self.pair_count.checked_add(1).ok_or(DxfInternalError)?;
        self.output.push_str(&code.to_string());
        self.output.push_str(CRLF);
        self.output.push_str(value);
        self.output.push_str(CRLF);
        Ok(())
    }
}

impl PairSink for DxfWriter {
    fn text(&mut self, code: u16, value: &str) -> DxfResult<()> {
        self.raw_pair(code, value)
    }

    fn integer(&mut self, code: u16, value: i64) -> DxfResult<()> {
        self.raw_pair(code, &value.to_string())
    }

    fn real(&mut self, code: u16, value: f64) -> DxfResult<()> {
        self.raw_pair(code, &format_real(value)?)
    }
}

#[derive(Debug)]
struct PairCursor<'a> {
    lines: Vec<&'a str>,
    next_line: usize,
}

impl<'a> PairCursor<'a> {
    fn new(bytes: &'a [u8]) -> DxfResult<Self> {
        let text = std::str::from_utf8(bytes).map_err(|_| DxfInternalError)?;
        if text.starts_with('\u{feff}') {
            return Err(DxfInternalError);
        }
        let body = text.strip_suffix(CRLF).ok_or(DxfInternalError)?;
        if body.is_empty() {
            return Err(DxfInternalError);
        }
        let lines: Vec<_> = body.split(CRLF).collect();
        if lines.len() % 2 != 0
            || lines.len() / 2 > MAX_DXF_GROUP_PAIRS
            || lines
                .iter()
                .any(|line| line.contains('\r') || line.contains('\n'))
            || lines
                .chunks_exact(2)
                .any(|pair| pair[1].len() > MAX_DXF_VALUE_BYTES)
        {
            return Err(DxfInternalError);
        }
        Ok(Self {
            lines,
            next_line: 0,
        })
    }

    fn parse_pair_at(&self, line_index: usize) -> DxfResult<(u16, &'a str)> {
        let code_text = *self.lines.get(line_index).ok_or(DxfInternalError)?;
        let value = *self
            .lines
            .get(line_index.checked_add(1).ok_or(DxfInternalError)?)
            .ok_or(DxfInternalError)?;
        let code = code_text.parse::<u16>().map_err(|_| DxfInternalError)?;
        if code.to_string() != code_text {
            return Err(DxfInternalError);
        }
        Ok((code, value))
    }

    fn peek(&self) -> DxfResult<(u16, &'a str)> {
        self.parse_pair_at(self.next_line)
    }

    fn next(&mut self) -> DxfResult<(u16, &'a str)> {
        let pair = self.parse_pair_at(self.next_line)?;
        self.next_line = self.next_line.checked_add(2).ok_or(DxfInternalError)?;
        Ok(pair)
    }

    fn is_exhausted(&self) -> bool {
        self.next_line == self.lines.len()
    }
}

impl PairSink for PairCursor<'_> {
    fn text(&mut self, code: u16, value: &str) -> DxfResult<()> {
        let actual = self.next()?;
        if actual == (code, value) {
            Ok(())
        } else {
            Err(DxfInternalError)
        }
    }

    fn integer(&mut self, code: u16, value: i64) -> DxfResult<()> {
        self.text(code, &value.to_string())
    }

    fn real(&mut self, code: u16, value: f64) -> DxfResult<()> {
        let (actual_code, actual_text) = self.next()?;
        let actual = actual_text.parse::<f64>().map_err(|_| DxfInternalError)?;
        if actual_code != code
            || !actual.is_finite()
            || canonical_zero(actual).to_bits() != canonical_zero(value).to_bits()
            || format_real(actual)? != actual_text
        {
            return Err(DxfInternalError);
        }
        Ok(())
    }
}

pub(super) fn serialize_dxf2007_ascii(
    title: &str,
    crease_pattern: &CreasePattern,
    paper: &Paper,
    vertex_indices: &HashMap<VertexId, usize>,
) -> Result<Vec<u8>, CreasePatternExportError> {
    validate_dxf_title(title)?;
    let title_chunks =
        split_title(title).ok_or(CreasePatternExportError::DxfStructureNotRepresentable)?;
    let lines = canonical_lines(crease_pattern, vertex_indices)
        .map_err(|_| CreasePatternExportError::DxfStructureNotRepresentable)?;
    let drawing_bounds = drawing_bounds(&lines)
        .map_err(|_| CreasePatternExportError::DrawingBoundsNotRepresentable)?;
    let paper_bounds = paper_bounds(paper, crease_pattern, vertex_indices)
        .map_err(|_| CreasePatternExportError::DrawingBoundsNotRepresentable)?;
    let capacity = crease_pattern
        .edges
        .len()
        .checked_mul(160)
        .and_then(|edge_bytes| edge_bytes.checked_add(4_096))
        .and_then(|bytes| bytes.checked_add(title.len()))
        .ok_or(CreasePatternExportError::DxfStructureNotRepresentable)?;
    let mut writer = DxfWriter::with_capacity(capacity);
    write_title(&mut writer, &title_chunks)
        .and_then(|()| emit_document(&mut writer, &lines, drawing_bounds, paper_bounds))
        .map_err(|_| CreasePatternExportError::DxfStructureNotRepresentable)?;
    let bytes = writer.into_bytes();
    verify_serialized_dxf(&bytes, title, &lines, drawing_bounds, paper_bounds)
        .map_err(|_| CreasePatternExportError::DxfStructureNotRepresentable)?;
    Ok(bytes)
}

fn emit_document<S: PairSink>(
    sink: &mut S,
    lines: &[CanonicalLine],
    drawing_bounds: Bounds,
    paper_bounds: Bounds,
) -> DxfResult<()> {
    emit_header(sink, drawing_bounds, paper_bounds)?;
    emit_tables(sink)?;
    emit_entities(sink, lines)?;
    sink.text(0, "EOF")
}

fn emit_header<S: PairSink>(
    sink: &mut S,
    drawing_bounds: Bounds,
    paper_bounds: Bounds,
) -> DxfResult<()> {
    sink.text(0, "SECTION")?;
    sink.text(2, "HEADER")?;
    sink.text(9, "$ACADVER")?;
    sink.text(1, "AC1021")?;
    sink.text(9, "$INSUNITS")?;
    sink.integer(70, 4)?;
    sink.text(9, "$MEASUREMENT")?;
    sink.integer(70, 1)?;
    sink.text(9, "$LUNITS")?;
    sink.integer(70, 2)?;
    sink.text(9, "$LUPREC")?;
    sink.integer(70, 8)?;
    sink.text(9, "$LTSCALE")?;
    sink.real(40, 1.0)?;
    sink.text(9, "$EXTNAMES")?;
    sink.integer(290, 0)?;
    sink.text(9, "$EXTMIN")?;
    sink.real(10, drawing_bounds.min_x)?;
    sink.real(20, drawing_bounds.min_y)?;
    sink.real(30, 0.0)?;
    sink.text(9, "$EXTMAX")?;
    sink.real(10, drawing_bounds.max_x)?;
    sink.real(20, drawing_bounds.max_y)?;
    sink.real(30, 0.0)?;
    sink.text(9, "$LIMMIN")?;
    sink.real(10, paper_bounds.min_x)?;
    sink.real(20, paper_bounds.min_y)?;
    sink.text(9, "$LIMMAX")?;
    sink.real(10, paper_bounds.max_x)?;
    sink.real(20, paper_bounds.max_y)?;
    sink.text(9, "$HANDSEED")?;
    sink.text(5, "10")?;
    sink.text(0, "ENDSEC")
}

fn emit_tables<S: PairSink>(sink: &mut S) -> DxfResult<()> {
    sink.text(0, "SECTION")?;
    sink.text(2, "TABLES")?;
    emit_linetype_table(sink)?;
    emit_layer_table(sink)?;
    sink.text(0, "ENDSEC")
}

fn emit_linetype_table<S: PairSink>(sink: &mut S) -> DxfResult<()> {
    sink.text(0, "TABLE")?;
    sink.text(2, "LTYPE")?;
    sink.text(5, "1")?;
    sink.text(330, "0")?;
    sink.text(100, "AcDbSymbolTable")?;
    sink.integer(
        70,
        i64::try_from(LINETYPE_DEFINITIONS.len()).map_err(|_| DxfInternalError)?,
    )?;
    for definition in LINETYPE_DEFINITIONS {
        sink.text(0, "LTYPE")?;
        sink.text(5, definition.handle)?;
        sink.text(330, "1")?;
        sink.text(100, "AcDbSymbolTableRecord")?;
        sink.text(100, "AcDbLinetypeTableRecord")?;
        sink.text(2, definition.name)?;
        sink.integer(70, 0)?;
        sink.text(3, definition.description)?;
        sink.integer(72, 65)?;
        sink.integer(
            73,
            i64::try_from(definition.pattern.len()).map_err(|_| DxfInternalError)?,
        )?;
        let total_length = definition.pattern.iter().map(|element| element.abs()).sum();
        sink.real(40, total_length)?;
        for element in definition.pattern {
            sink.real(49, *element)?;
            sink.integer(74, 0)?;
        }
    }
    sink.text(0, "ENDTAB")
}

fn emit_layer_table<S: PairSink>(sink: &mut S) -> DxfResult<()> {
    sink.text(0, "TABLE")?;
    sink.text(2, "LAYER")?;
    sink.text(5, "9")?;
    sink.text(330, "0")?;
    sink.text(100, "AcDbSymbolTable")?;
    sink.integer(
        70,
        i64::try_from(LAYER_DEFINITIONS.len()).map_err(|_| DxfInternalError)?,
    )?;
    for definition in LAYER_DEFINITIONS {
        sink.text(0, "LAYER")?;
        sink.text(5, definition.handle)?;
        sink.text(330, "9")?;
        sink.text(100, "AcDbSymbolTableRecord")?;
        sink.text(100, "AcDbLayerTableRecord")?;
        sink.text(2, definition.name)?;
        sink.integer(70, 0)?;
        sink.integer(62, definition.color)?;
        sink.text(6, definition.linetype)?;
        sink.integer(290, 1)?;
        sink.integer(370, -3)?;
    }
    sink.text(0, "ENDTAB")
}

fn emit_entities<S: PairSink>(sink: &mut S, lines: &[CanonicalLine]) -> DxfResult<()> {
    sink.text(0, "SECTION")?;
    sink.text(2, "ENTITIES")?;
    for line in lines {
        sink.text(0, "LINE")?;
        sink.text(8, edge_layer(line.kind))?;
        sink.real(10, line.start.x)?;
        sink.real(20, line.start.y)?;
        sink.real(11, line.end.x)?;
        sink.real(21, line.end.y)?;
    }
    sink.text(0, "ENDSEC")
}

fn write_title(writer: &mut DxfWriter, chunks: &[&str]) -> DxfResult<()> {
    writer.text(999, TITLE_MARKER)?;
    for (chunk_index, chunk) in chunks.iter().enumerate() {
        let value = format!(
            "{TITLE_CHUNK_PREFIX}{:02}/{:02} {chunk}",
            chunk_index + 1,
            chunks.len()
        );
        if value.len() > MAX_DXF_VALUE_BYTES {
            return Err(DxfInternalError);
        }
        writer.text(999, &value)?;
    }
    Ok(())
}

fn verify_serialized_dxf(
    bytes: &[u8],
    expected_title: &str,
    lines: &[CanonicalLine],
    drawing_bounds: Bounds,
    paper_bounds: Bounds,
) -> DxfResult<()> {
    let mut cursor = PairCursor::new(bytes)?;
    let actual_title = read_title(&mut cursor)?;
    if actual_title != expected_title {
        return Err(DxfInternalError);
    }
    emit_document(&mut cursor, lines, drawing_bounds, paper_bounds)?;
    if !cursor.is_exhausted() {
        return Err(DxfInternalError);
    }
    Ok(())
}

fn read_title(cursor: &mut PairCursor<'_>) -> DxfResult<String> {
    cursor.text(999, TITLE_MARKER)?;
    let mut payloads = Vec::new();
    let mut declared_total = None;
    loop {
        let (code, _) = cursor.peek()?;
        if code != 999 {
            break;
        }
        let (_, value) = cursor.next()?;
        let suffix = value
            .strip_prefix(TITLE_CHUNK_PREFIX)
            .ok_or(DxfInternalError)?;
        let bytes = suffix.as_bytes();
        if bytes.len() < 6
            || !bytes[0].is_ascii_digit()
            || !bytes[1].is_ascii_digit()
            || bytes[2] != b'/'
            || !bytes[3].is_ascii_digit()
            || !bytes[4].is_ascii_digit()
            || bytes[5] != b' '
        {
            return Err(DxfInternalError);
        }
        let index = suffix[0..2]
            .parse::<usize>()
            .map_err(|_| DxfInternalError)?;
        let total = suffix[3..5]
            .parse::<usize>()
            .map_err(|_| DxfInternalError)?;
        let payload = &suffix[6..];
        if total == 0
            || total > MAX_DXF_TITLE_CHUNKS
            || index != payloads.len() + 1
            || payload.len() > MAX_DXF_TITLE_CHUNK_BYTES
            || declared_total.is_some_and(|declared| declared != total)
        {
            return Err(DxfInternalError);
        }
        declared_total = Some(total);
        payloads.push(payload.to_owned());
        if payloads.len() == total {
            break;
        }
    }
    if declared_total.is_none() {
        return Ok(String::new());
    }
    if declared_total != Some(payloads.len()) {
        return Err(DxfInternalError);
    }
    let title = payloads.concat();
    if invalid_title_character(&title).is_some()
        || title.len() > MAX_DXF_TITLE_BYTES
        || split_title(&title)
            .ok_or(DxfInternalError)?
            .iter()
            .copied()
            .ne(payloads.iter().map(String::as_str))
    {
        return Err(DxfInternalError);
    }
    Ok(title)
}

fn validate_dxf_title(title: &str) -> Result<(), CreasePatternExportError> {
    if let Some((character_index, code_point)) = invalid_title_character(title) {
        return Err(CreasePatternExportError::InvalidDxfTitleCharacter {
            character_index,
            code_point,
        });
    }
    Ok(())
}

fn invalid_title_character(title: &str) -> Option<(usize, u32)> {
    title
        .chars()
        .enumerate()
        .find_map(|(character_index, character)| {
            character
                .is_control()
                .then_some((character_index, u32::from(character)))
        })
}

fn split_title(title: &str) -> Option<Vec<&str>> {
    if title.len() > MAX_DXF_TITLE_BYTES {
        return None;
    }
    if title.is_empty() {
        return Some(Vec::new());
    }
    let mut chunks = Vec::new();
    let mut chunk_start = 0;
    let mut chunk_bytes = 0_usize;
    for (byte_index, character) in title.char_indices() {
        let character_bytes = character.len_utf8();
        if chunk_bytes.checked_add(character_bytes)? > MAX_DXF_TITLE_CHUNK_BYTES {
            chunks.push(&title[chunk_start..byte_index]);
            chunk_start = byte_index;
            chunk_bytes = 0;
        }
        chunk_bytes = chunk_bytes.checked_add(character_bytes)?;
    }
    chunks.push(&title[chunk_start..]);
    (chunks.len() <= MAX_DXF_TITLE_CHUNKS).then_some(chunks)
}

fn canonical_lines(
    crease_pattern: &CreasePattern,
    vertex_indices: &HashMap<VertexId, usize>,
) -> DxfResult<Vec<CanonicalLine>> {
    let mut lines = Vec::with_capacity(crease_pattern.edges.len());
    for edge in &crease_pattern.edges {
        let first = canonical_point(vertex_position(crease_pattern, vertex_indices, edge.start)?);
        let second = canonical_point(vertex_position(crease_pattern, vertex_indices, edge.end)?);
        let (start, end) = if compare_points(first, second) == Ordering::Greater {
            (second, first)
        } else {
            (first, second)
        };
        lines.push(CanonicalLine {
            kind: edge.kind,
            start,
            end,
        });
    }
    lines.sort_unstable_by(compare_lines);
    Ok(lines)
}

fn drawing_bounds(lines: &[CanonicalLine]) -> DxfResult<Bounds> {
    let mut accumulator = BoundsAccumulator::default();
    for line in lines {
        accumulator.include(line.start)?;
        accumulator.include(line.end)?;
    }
    accumulator.finish()
}

fn paper_bounds(
    paper: &Paper,
    crease_pattern: &CreasePattern,
    vertex_indices: &HashMap<VertexId, usize>,
) -> DxfResult<Bounds> {
    let mut accumulator = BoundsAccumulator::default();
    for vertex_id in &paper.boundary_vertices {
        accumulator.include(vertex_position(crease_pattern, vertex_indices, *vertex_id)?)?;
    }
    accumulator.finish()
}

fn vertex_position(
    crease_pattern: &CreasePattern,
    vertex_indices: &HashMap<VertexId, usize>,
    vertex_id: VertexId,
) -> DxfResult<Point2> {
    vertex_indices
        .get(&vertex_id)
        .and_then(|index| crease_pattern.vertices.get(*index))
        .map(|vertex| vertex.position)
        .ok_or(DxfInternalError)
}

fn compare_lines(left: &CanonicalLine, right: &CanonicalLine) -> Ordering {
    edge_kind_rank(left.kind)
        .cmp(&edge_kind_rank(right.kind))
        .then_with(|| compare_points(left.start, right.start))
        .then_with(|| compare_points(left.end, right.end))
}

fn compare_points(left: Point2, right: Point2) -> Ordering {
    left.x
        .total_cmp(&right.x)
        .then_with(|| left.y.total_cmp(&right.y))
}

const fn canonical_point(point: Point2) -> Point2 {
    Point2::new(canonical_zero(point.x), canonical_zero(point.y))
}

const fn edge_kind_rank(kind: EdgeKind) -> u8 {
    match kind {
        EdgeKind::Boundary => 0,
        EdgeKind::Mountain => 1,
        EdgeKind::Valley => 2,
        EdgeKind::Auxiliary => 3,
        EdgeKind::Cut => 4,
    }
}

const fn edge_layer(kind: EdgeKind) -> &'static str {
    match kind {
        EdgeKind::Boundary => "ORIGAMI_BOUNDARY",
        EdgeKind::Mountain => "ORIGAMI_MOUNTAIN",
        EdgeKind::Valley => "ORIGAMI_VALLEY",
        EdgeKind::Auxiliary => "ORIGAMI_AUXILIARY",
        EdgeKind::Cut => "ORIGAMI_CUT",
    }
}

fn format_real(value: f64) -> DxfResult<String> {
    if !value.is_finite() {
        return Err(DxfInternalError);
    }
    let value = canonical_zero(value);
    if value == 0.0 {
        return Ok("0".to_owned());
    }
    let plain = value.to_string();
    let text = if plain.len() <= MAX_DXF_REAL_BYTES {
        plain
    } else {
        format!("{value:e}")
    };
    let parsed = text.parse::<f64>().map_err(|_| DxfInternalError)?;
    if text.len() > MAX_DXF_REAL_BYTES || parsed.to_bits() != value.to_bits() {
        return Err(DxfInternalError);
    }
    Ok(text)
}

const fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, Vertex, VertexId};
    use sha2::{Digest, Sha256};

    use super::{
        BoundsAccumulator, DxfWriter, MAX_DXF_GROUP_PAIRS, MAX_DXF_TITLE_CHUNK_BYTES, PairCursor,
        TITLE_CHUNK_PREFIX, canonical_zero, format_real, read_title, serialize_dxf2007_ascii,
        verify_serialized_dxf,
    };
    use crate::crease_pattern_export::CreasePatternExportError;

    fn add_vertex(vertices: &mut Vec<Vertex>, x: f64, y: f64) -> VertexId {
        let id = VertexId::new();
        vertices.push(Vertex {
            id,
            position: Point2::new(x, y),
        });
        id
    }

    fn add_edge(edges: &mut Vec<Edge>, start: VertexId, end: VertexId, kind: EdgeKind) {
        edges.push(Edge {
            id: EdgeId::new(),
            start,
            end,
            kind,
        });
    }

    fn fixture() -> (CreasePattern, Paper, HashMap<VertexId, usize>) {
        let mut vertices = Vec::new();
        let boundary = [
            add_vertex(&mut vertices, 0.0, 0.0),
            add_vertex(&mut vertices, 100.0, 0.0),
            add_vertex(&mut vertices, 100.0, 100.0),
            add_vertex(&mut vertices, 0.0, 100.0),
        ];
        let mountain = [
            add_vertex(&mut vertices, 10.0, 20.0),
            add_vertex(&mut vertices, 20.0, 20.0),
        ];
        let valley = [
            add_vertex(&mut vertices, 30.0, 30.0),
            add_vertex(&mut vertices, 40.0, 30.0),
        ];
        let auxiliary = [
            add_vertex(&mut vertices, 50.0, 40.0),
            add_vertex(&mut vertices, 60.0, 40.0),
        ];
        let cut = [
            add_vertex(&mut vertices, 70.0, 50.0),
            add_vertex(&mut vertices, 80.0, 50.0),
        ];
        let mut edges = Vec::new();
        for index in 0..boundary.len() {
            add_edge(
                &mut edges,
                boundary[index],
                boundary[(index + 1) % boundary.len()],
                EdgeKind::Boundary,
            );
        }
        add_edge(&mut edges, mountain[0], mountain[1], EdgeKind::Mountain);
        add_edge(&mut edges, valley[0], valley[1], EdgeKind::Valley);
        add_edge(&mut edges, auxiliary[0], auxiliary[1], EdgeKind::Auxiliary);
        add_edge(&mut edges, cut[0], cut[1], EdgeKind::Cut);
        let crease_pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary.to_vec(),
            cutting_allowed: true,
            ..Paper::default()
        };
        let vertex_indices = crease_pattern
            .vertices
            .iter()
            .enumerate()
            .map(|(index, vertex)| (vertex.id, index))
            .collect();
        (crease_pattern, paper, vertex_indices)
    }

    fn triangle_fixture() -> (CreasePattern, Paper, HashMap<VertexId, usize>) {
        let mut vertices = Vec::new();
        let boundary = [
            add_vertex(&mut vertices, 0.0, 0.0),
            add_vertex(&mut vertices, 10.0, 0.0),
            add_vertex(&mut vertices, 0.0, 10.0),
        ];
        let mut edges = Vec::new();
        for index in 0..boundary.len() {
            add_edge(
                &mut edges,
                boundary[index],
                boundary[(index + 1) % boundary.len()],
                EdgeKind::Boundary,
            );
        }
        let crease_pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary.to_vec(),
            ..Paper::default()
        };
        let vertex_indices = crease_pattern
            .vertices
            .iter()
            .enumerate()
            .map(|(index, vertex)| (vertex.id, index))
            .collect();
        (crease_pattern, paper, vertex_indices)
    }

    fn replace_once(bytes: &mut [u8], needle: &[u8], replacement: &[u8]) {
        assert_eq!(needle.len(), replacement.len());
        let start = bytes
            .windows(needle.len())
            .position(|window| window == needle)
            .expect("needle must exist");
        bytes[start..start + needle.len()].copy_from_slice(replacement);
    }

    #[test]
    fn emits_stable_ac1021_golden_bytes() {
        let (crease_pattern, paper, vertex_indices) = triangle_fixture();
        let bytes =
            serialize_dxf2007_ascii("鶴", &crease_pattern, &paper, &vertex_indices).unwrap();

        assert!(bytes.starts_with(
            "999\r\nORIGAMI2_EXPORT AC1021\r\n999\r\nORIGAMI2_TITLE 01/01 鶴\r\n".as_bytes()
        ));
        assert!(bytes.ends_with(b"0\r\nEOF\r\n"));
        assert!(!bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
        assert!(
            bytes
                .windows(b"9\r\n$LUPREC\r\n70\r\n8\r\n".len())
                .any(|window| window == b"9\r\n$LUPREC\r\n70\r\n8\r\n")
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            "98e7efd62cacf4f858fd43c9f3c36897ed1d88d3a75e7c6c03e9714e50fa3b73"
        );
    }

    #[test]
    fn emits_five_semantic_layers_and_only_minimal_line_groups() {
        let (crease_pattern, paper, vertex_indices) = fixture();
        let bytes =
            serialize_dxf2007_ascii("layers", &crease_pattern, &paper, &vertex_indices).unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        let ltype_position = text.find("2\r\nLTYPE\r\n").unwrap();
        let layer_position = text.find("2\r\nLAYER\r\n").unwrap();
        assert!(ltype_position < layer_position);
        for layer in [
            "ORIGAMI_BOUNDARY",
            "ORIGAMI_MOUNTAIN",
            "ORIGAMI_VALLEY",
            "ORIGAMI_AUXILIARY",
            "ORIGAMI_CUT",
        ] {
            assert!(text.contains(layer));
        }

        let lines: Vec<_> = text.strip_suffix("\r\n").unwrap().split("\r\n").collect();
        let pairs: Vec<_> = lines
            .chunks_exact(2)
            .map(|pair| (pair[0], pair[1]))
            .collect();
        let entities = pairs
            .iter()
            .position(|pair| *pair == ("2", "ENTITIES"))
            .unwrap();
        let expected_layers = [
            "ORIGAMI_BOUNDARY",
            "ORIGAMI_BOUNDARY",
            "ORIGAMI_BOUNDARY",
            "ORIGAMI_BOUNDARY",
            "ORIGAMI_MOUNTAIN",
            "ORIGAMI_VALLEY",
            "ORIGAMI_AUXILIARY",
            "ORIGAMI_CUT",
        ];
        let entity_pairs = &pairs[entities + 1..];
        let line_positions: Vec<_> = entity_pairs
            .iter()
            .enumerate()
            .filter_map(|(index, pair)| (*pair == ("0", "LINE")).then_some(index))
            .collect();
        assert_eq!(line_positions.len(), expected_layers.len());
        for (position, expected_layer) in line_positions.into_iter().zip(expected_layers) {
            assert_eq!(
                &entity_pairs[position + 1..position + 6],
                &[
                    ("8", expected_layer),
                    ("10", entity_pairs[position + 2].1),
                    ("20", entity_pairs[position + 3].1),
                    ("11", entity_pairs[position + 4].1),
                    ("21", entity_pairs[position + 5].1),
                ]
            );
            assert_eq!(
                entity_pairs[position + 1..position + 6]
                    .iter()
                    .map(|pair| pair.0)
                    .collect::<Vec<_>>(),
                ["8", "10", "20", "11", "21"]
            );
        }
    }

    #[test]
    fn preserves_unicode_title_across_scalar_aligned_chunks() {
        let (crease_pattern, paper, vertex_indices) = triangle_fixture();
        let title = "🙂".repeat(512);
        let bytes =
            serialize_dxf2007_ascii(&title, &crease_pattern, &paper, &vertex_indices).unwrap();
        let mut cursor = PairCursor::new(&bytes).unwrap();
        assert_eq!(read_title(&mut cursor).unwrap(), title);

        let text = std::str::from_utf8(&bytes).unwrap();
        let title_values: Vec<_> = text
            .split("\r\n")
            .filter(|line| line.starts_with(TITLE_CHUNK_PREFIX))
            .collect();
        assert_eq!(title_values.len(), 10);
        assert!(title_values.iter().all(|value| value.len() <= 255));
        assert!(title_values.iter().all(|value| {
            let payload = &value[TITLE_CHUNK_PREFIX.len() + 6..];
            payload.len() <= MAX_DXF_TITLE_CHUNK_BYTES && payload.is_char_boundary(payload.len())
        }));
    }

    #[test]
    fn empty_title_emits_only_the_creator_marker() {
        let (crease_pattern, paper, vertex_indices) = triangle_fixture();
        let bytes = serialize_dxf2007_ascii("", &crease_pattern, &paper, &vertex_indices).unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(text.starts_with("999\r\nORIGAMI2_EXPORT AC1021\r\n0\r\nSECTION\r\n"));
        assert!(!text.contains(TITLE_CHUNK_PREFIX));
        let mut cursor = PairCursor::new(&bytes).unwrap();
        assert_eq!(read_title(&mut cursor).unwrap(), "");
    }

    #[test]
    fn rejects_control_characters_without_replacement() {
        let (crease_pattern, paper, vertex_indices) = triangle_fixture();
        for (title, expected_index, expected_code_point) in [
            ("ok\nbad", 2, 0x0A),
            ("鶴\tbad", 1, 0x09),
            ("ok\u{0085}bad", 2, 0x85),
        ] {
            let error = serialize_dxf2007_ascii(title, &crease_pattern, &paper, &vertex_indices)
                .unwrap_err();
            assert!(matches!(
                error,
                CreasePatternExportError::InvalidDxfTitleCharacter {
                    character_index,
                    code_point,
                } if character_index == expected_index && code_point == expected_code_point
            ));
        }
    }

    #[test]
    fn separates_drawing_extents_from_paper_limits() {
        let (mut crease_pattern, paper, vertex_indices) = fixture();
        crease_pattern.vertices[8].position = Point2::new(150.0, 40.0);
        crease_pattern.vertices[9].position = Point2::new(160.0, 40.0);
        let bytes =
            serialize_dxf2007_ascii("outside", &crease_pattern, &paper, &vertex_indices).unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(text.contains("9\r\n$EXTMAX\r\n10\r\n160\r\n20\r\n100\r\n30\r\n0\r\n"));
        assert!(text.contains("9\r\n$LIMMAX\r\n10\r\n100\r\n20\r\n100\r\n"));
    }

    #[test]
    fn verifier_rejects_mutation_truncation_and_non_crlf_input() {
        let (crease_pattern, paper, vertex_indices) = fixture();
        let bytes =
            serialize_dxf2007_ascii("verify", &crease_pattern, &paper, &vertex_indices).unwrap();
        let lines = super::canonical_lines(&crease_pattern, &vertex_indices).unwrap();
        let drawing_bounds = super::drawing_bounds(&lines).unwrap();
        let paper_bounds = super::paper_bounds(&paper, &crease_pattern, &vertex_indices).unwrap();
        assert!(
            verify_serialized_dxf(&bytes, "verify", &lines, drawing_bounds, paper_bounds).is_ok()
        );

        let mut mutated = bytes.clone();
        replace_once(&mut mutated, b"ORI_VALLEY", b"ORI_VXLLEY");
        assert!(
            verify_serialized_dxf(&mutated, "verify", &lines, drawing_bounds, paper_bounds)
                .is_err()
        );

        let mut wrong_extents = bytes.clone();
        replace_once(
            &mut wrong_extents,
            b"$EXTMAX\r\n10\r\n100\r\n",
            b"$EXTMAX\r\n10\r\n101\r\n",
        );
        assert!(
            verify_serialized_dxf(
                &wrong_extents,
                "verify",
                &lines,
                drawing_bounds,
                paper_bounds
            )
            .is_err()
        );

        let mut wrong_title_index = bytes.clone();
        replace_once(
            &mut wrong_title_index,
            b"ORIGAMI2_TITLE 01/01",
            b"ORIGAMI2_TITLE 02/01",
        );
        assert!(
            verify_serialized_dxf(
                &wrong_title_index,
                "verify",
                &lines,
                drawing_bounds,
                paper_bounds
            )
            .is_err()
        );

        let truncated = &bytes[..bytes.len() - b"0\r\nEOF\r\n".len()];
        assert!(
            verify_serialized_dxf(truncated, "verify", &lines, drawing_bounds, paper_bounds)
                .is_err()
        );

        let mut bare_lf = bytes.clone();
        bare_lf.remove(3);
        assert!(PairCursor::new(&bare_lf).is_err());
    }

    #[test]
    fn output_is_deterministic_and_ids_are_not_serialized() {
        let (crease_pattern, paper, vertex_indices) = fixture();
        let first =
            serialize_dxf2007_ascii("deterministic", &crease_pattern, &paper, &vertex_indices)
                .unwrap();
        let second =
            serialize_dxf2007_ascii("deterministic", &crease_pattern, &paper, &vertex_indices)
                .unwrap();
        assert_eq!(first, second);

        let mut reordered = crease_pattern.clone();
        let mut reordered_paper = paper.clone();
        let mut replacement_ids = HashMap::new();
        for vertex in &mut reordered.vertices {
            let replacement = VertexId::new();
            replacement_ids.insert(vertex.id, replacement);
            vertex.id = replacement;
        }
        for edge in &mut reordered.edges {
            edge.id = EdgeId::new();
            edge.start = replacement_ids[&edge.start];
            edge.end = replacement_ids[&edge.end];
            std::mem::swap(&mut edge.start, &mut edge.end);
        }
        for boundary_vertex in &mut reordered_paper.boundary_vertices {
            *boundary_vertex = replacement_ids[boundary_vertex];
        }
        reordered.vertices.reverse();
        reordered.edges.reverse();
        reordered_paper.boundary_vertices.reverse();
        let reordered_indices = reordered
            .vertices
            .iter()
            .enumerate()
            .map(|(index, vertex)| (vertex.id, index))
            .collect();
        let canonical = serialize_dxf2007_ascii(
            "deterministic",
            &reordered,
            &reordered_paper,
            &reordered_indices,
        )
        .unwrap();
        assert_eq!(first, canonical);

        for vertex in &crease_pattern.vertices {
            let uuid_hex = vertex
                .id
                .canonical_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            assert!(
                !first
                    .windows(uuid_hex.len())
                    .any(|window| window == uuid_hex.as_bytes())
            );
        }
    }

    #[test]
    fn real_tokens_are_finite_short_roundtrip_values_and_canonicalize_zero() {
        for value in [
            0.0,
            -0.0,
            1.0,
            -123.456,
            1.0e20,
            1.0e100,
            1.0e-100,
            f64::MIN_POSITIVE,
            f64::from_bits(1),
            f64::MAX,
        ] {
            let text = format_real(value).unwrap();
            let parsed = text.parse::<f64>().unwrap();
            assert!(text.len() <= 64);
            assert_eq!(parsed.to_bits(), canonical_zero(value).to_bits());
        }
        assert_eq!(format_real(-0.0).unwrap(), "0");
        assert!(format_real(f64::NAN).is_err());
        assert!(format_real(f64::INFINITY).is_err());
    }

    #[test]
    fn module_rejects_titles_beyond_the_shared_utf8_envelope() {
        let (crease_pattern, paper, vertex_indices) = triangle_fixture();
        let title = "🙂".repeat(513);
        assert!(matches!(
            serialize_dxf2007_ascii(&title, &crease_pattern, &paper, &vertex_indices),
            Err(CreasePatternExportError::DxfStructureNotRepresentable)
        ));
    }

    #[test]
    fn rejects_non_positive_or_overflowing_ranges() {
        let mut collinear = BoundsAccumulator::default();
        collinear.include(Point2::new(0.0, 0.0)).unwrap();
        collinear.include(Point2::new(1.0, 0.0)).unwrap();
        assert!(collinear.finish().is_err());

        let mut overflowing = BoundsAccumulator::default();
        overflowing.include(Point2::new(-f64::MAX, -1.0)).unwrap();
        overflowing.include(Point2::new(f64::MAX, 1.0)).unwrap();
        assert!(overflowing.finish().is_err());
    }

    #[test]
    fn writer_enforces_group_pair_limit() {
        let mut writer = DxfWriter {
            output: String::new(),
            pair_count: MAX_DXF_GROUP_PAIRS,
        };
        assert!(super::PairSink::text(&mut writer, 999, "too many").is_err());
    }
}
