use std::{cmp::Ordering, collections::HashMap, fmt::Write as _};

use ori_domain::{CreasePattern, EdgeKind, Point2, VertexId};

use super::CreasePatternExportError;

const PDF_HEADER: &[u8] = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n";
const PDF_OBJECT_COUNT: usize = 5;
const PDF_POINTS_PER_MILLIMETRE: f64 = 360.0 / 127.0;
const PDF_MARGIN_MILLIMETRES: f64 = 10.0;
const PDF_MAX_PAGE_POINTS: f64 = 14_400.0;
const MAX_PDF_NUMBER_CHARS: usize = 64;
const MAX_CLASSIC_XREF_OFFSET: usize = 9_999_999_999;

#[derive(Clone, Copy)]
struct DrawingBounds {
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
}

#[derive(Clone, Copy)]
struct PdfSegment {
    kind: EdgeKind,
    start: Point2,
    end: Point2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PdfLineStyle {
    kind: EdgeKind,
    stroke_width_points: f64,
    line_cap: u8,
    dash_mm: &'static [f64],
}

const PDF_LINE_STYLES: [PdfLineStyle; 5] = [
    PdfLineStyle {
        kind: EdgeKind::Boundary,
        stroke_width_points: 0.50,
        line_cap: 0,
        dash_mm: &[],
    },
    PdfLineStyle {
        kind: EdgeKind::Mountain,
        stroke_width_points: 0.35,
        line_cap: 0,
        dash_mm: &[6.0, 2.0, 1.0, 2.0],
    },
    PdfLineStyle {
        kind: EdgeKind::Valley,
        stroke_width_points: 0.35,
        line_cap: 0,
        dash_mm: &[3.0, 1.5],
    },
    PdfLineStyle {
        kind: EdgeKind::Auxiliary,
        stroke_width_points: 0.25,
        line_cap: 1,
        dash_mm: &[0.5, 1.5],
    },
    PdfLineStyle {
        kind: EdgeKind::Cut,
        stroke_width_points: 0.60,
        line_cap: 0,
        dash_mm: &[8.0, 2.0, 1.0, 2.0, 1.0, 2.0],
    },
];

pub(super) fn serialize_pdf17(
    title: &str,
    crease_pattern: &CreasePattern,
    vertex_indices: &HashMap<VertexId, usize>,
) -> Result<Vec<u8>, CreasePatternExportError> {
    let bounds = drawing_bounds(crease_pattern, vertex_indices)?;
    let drawing_width_mm = finite_positive_difference(bounds.max_x, bounds.min_x)?;
    let drawing_height_mm = finite_positive_difference(bounds.max_y, bounds.min_y)?;
    let page_width_mm = drawing_width_mm + PDF_MARGIN_MILLIMETRES * 2.0;
    let page_height_mm = drawing_height_mm + PDF_MARGIN_MILLIMETRES * 2.0;
    if !page_width_mm.is_finite() || !page_height_mm.is_finite() {
        return Err(CreasePatternExportError::DrawingBoundsNotRepresentable);
    }
    let page_width_points = page_width_mm * PDF_POINTS_PER_MILLIMETRE;
    let page_height_points = page_height_mm * PDF_POINTS_PER_MILLIMETRE;
    if !page_width_points.is_finite()
        || !page_height_points.is_finite()
        || page_width_points > PDF_MAX_PAGE_POINTS
        || page_height_points > PDF_MAX_PAGE_POINTS
    {
        return Err(CreasePatternExportError::PdfPageTooLarge);
    }

    let page_width = pdf_number(page_width_points)?;
    let page_height = pdf_number(page_height_points)?;
    let content = serialize_content(crease_pattern, vertex_indices, bounds)?;
    let title_hex = pdf_utf16be_hex_string(title);

    let catalog =
        b"<< /Type /Catalog /Pages 2 0 R /ViewerPreferences << /PrintScaling /None >> >>".to_vec();
    let pages = b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec();
    let page = format!(
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {page_width} {page_height}] /CropBox [0 0 {page_width} {page_height}] /Resources << >> /Contents 4 0 R >>"
    )
    .into_bytes();
    let mut contents = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
    contents.extend_from_slice(content.as_bytes());
    contents.extend_from_slice(b"endstream");
    let info = format!(
        "<< /Title <{title_hex}> /Creator (ORIGAMI2) /Producer (ORIGAMI2 deterministic PDF exporter) >>"
    )
    .into_bytes();

    serialize_pdf_objects([catalog, pages, page, contents, info])
}

fn drawing_bounds(
    crease_pattern: &CreasePattern,
    vertex_indices: &HashMap<VertexId, usize>,
) -> Result<DrawingBounds, CreasePatternExportError> {
    let mut bounds: Option<DrawingBounds> = None;
    for edge in &crease_pattern.edges {
        for vertex_id in [edge.start, edge.end] {
            let vertex_index = vertex_indices
                .get(&vertex_id)
                .copied()
                .ok_or(CreasePatternExportError::PdfStructureNotRepresentable)?;
            let position = crease_pattern
                .vertices
                .get(vertex_index)
                .ok_or(CreasePatternExportError::PdfStructureNotRepresentable)?
                .position;
            if !position.x.is_finite() || !position.y.is_finite() {
                return Err(CreasePatternExportError::DrawingBoundsNotRepresentable);
            }
            bounds = Some(match bounds {
                None => DrawingBounds {
                    min_x: position.x,
                    max_x: position.x,
                    min_y: position.y,
                    max_y: position.y,
                },
                Some(current) => DrawingBounds {
                    min_x: current.min_x.min(position.x),
                    max_x: current.max_x.max(position.x),
                    min_y: current.min_y.min(position.y),
                    max_y: current.max_y.max(position.y),
                },
            });
        }
    }
    bounds.ok_or(CreasePatternExportError::DrawingBoundsNotRepresentable)
}

fn finite_positive_difference(maximum: f64, minimum: f64) -> Result<f64, CreasePatternExportError> {
    let difference = maximum - minimum;
    if difference.is_finite() && difference > 0.0 {
        Ok(difference)
    } else {
        Err(CreasePatternExportError::DrawingBoundsNotRepresentable)
    }
}

fn serialize_content(
    crease_pattern: &CreasePattern,
    vertex_indices: &HashMap<VertexId, usize>,
    bounds: DrawingBounds,
) -> Result<String, CreasePatternExportError> {
    let mut segments = Vec::with_capacity(crease_pattern.edges.len());
    for edge in &crease_pattern.edges {
        let start = rebased_pdf_point(
            edge_position(crease_pattern, vertex_indices, edge.start)?,
            bounds,
        )?;
        let end = rebased_pdf_point(
            edge_position(crease_pattern, vertex_indices, edge.end)?,
            bounds,
        )?;
        let (start, end) = if compare_points(start, end) == Ordering::Greater {
            (end, start)
        } else {
            (start, end)
        };
        segments.push(PdfSegment {
            kind: edge.kind,
            start,
            end,
        });
    }
    segments.sort_unstable_by(compare_segments);

    let scale = pdf_number(PDF_POINTS_PER_MILLIMETRE)?;
    let margin_points = pdf_number(PDF_MARGIN_MILLIMETRES * PDF_POINTS_PER_MILLIMETRE)?;
    let estimated_capacity = 256_usize
        .checked_add(crease_pattern.edges.len().saturating_mul(100))
        .ok_or(CreasePatternExportError::PdfStructureNotRepresentable)?;
    let mut output = String::with_capacity(estimated_capacity);
    writeln!(
        output,
        "q\n{scale} 0 0 {scale} {margin_points} {margin_points} cm\n0 G"
    )
    .map_err(|_| CreasePatternExportError::PdfStructureNotRepresentable)?;

    for style in PDF_LINE_STYLES {
        let matching = segments
            .iter()
            .filter(|segment| segment.kind == style.kind)
            .collect::<Vec<_>>();
        if matching.is_empty() {
            continue;
        }

        output.push_str("q\n");
        // The content coordinate system is millimetres, while the public line
        // style contract specifies physical PDF points. Dividing by the CTM
        // scale makes `w` produce the requested point width after painting.
        let stroke_width_user_units = style.stroke_width_points / PDF_POINTS_PER_MILLIMETRE;
        writeln!(output, "{} w", pdf_number(stroke_width_user_units)?)
            .map_err(|_| CreasePatternExportError::PdfStructureNotRepresentable)?;
        writeln!(output, "{} J", style.line_cap)
            .map_err(|_| CreasePatternExportError::PdfStructureNotRepresentable)?;
        output.push('[');
        for (index, dash) in style.dash_mm.iter().copied().enumerate() {
            if index != 0 {
                output.push(' ');
            }
            output.push_str(&pdf_number(dash)?);
        }
        output.push_str("] 0 d\n");

        for segment in matching {
            writeln!(
                output,
                "{} {} m {} {} l",
                pdf_number(segment.start.x)?,
                pdf_number(segment.start.y)?,
                pdf_number(segment.end.x)?,
                pdf_number(segment.end.y)?,
            )
            .map_err(|_| CreasePatternExportError::PdfStructureNotRepresentable)?;
        }
        output.push_str("S\nQ\n");
    }
    output.push_str("Q\n");
    Ok(output)
}

fn edge_position(
    crease_pattern: &CreasePattern,
    vertex_indices: &HashMap<VertexId, usize>,
    vertex_id: VertexId,
) -> Result<Point2, CreasePatternExportError> {
    let index = vertex_indices
        .get(&vertex_id)
        .copied()
        .ok_or(CreasePatternExportError::PdfStructureNotRepresentable)?;
    crease_pattern
        .vertices
        .get(index)
        .map(|vertex| vertex.position)
        .ok_or(CreasePatternExportError::PdfStructureNotRepresentable)
}

fn rebased_pdf_point(
    point: Point2,
    bounds: DrawingBounds,
) -> Result<Point2, CreasePatternExportError> {
    let x = canonical_zero(point.x - bounds.min_x);
    let y = canonical_zero(bounds.max_y - point.y);
    if !x.is_finite() || !y.is_finite() {
        return Err(CreasePatternExportError::DrawingBoundsNotRepresentable);
    }
    Ok(Point2::new(x, y))
}

fn compare_segments(left: &PdfSegment, right: &PdfSegment) -> Ordering {
    edge_kind_index(left.kind)
        .cmp(&edge_kind_index(right.kind))
        .then_with(|| compare_points(left.start, right.start))
        .then_with(|| compare_points(left.end, right.end))
}

fn compare_points(left: Point2, right: Point2) -> Ordering {
    left.x
        .total_cmp(&right.x)
        .then_with(|| left.y.total_cmp(&right.y))
}

const fn edge_kind_index(kind: EdgeKind) -> u8 {
    match kind {
        EdgeKind::Boundary => 0,
        EdgeKind::Mountain => 1,
        EdgeKind::Valley => 2,
        EdgeKind::Auxiliary => 3,
        EdgeKind::Cut => 4,
    }
}

fn pdf_number(value: f64) -> Result<String, CreasePatternExportError> {
    if !value.is_finite() {
        return Err(CreasePatternExportError::PdfStructureNotRepresentable);
    }
    let value = canonical_zero(value);
    let rendered = value.to_string();
    if rendered.contains(['e', 'E']) || !valid_pdf_number_syntax(&rendered) {
        return Err(CreasePatternExportError::PdfStructureNotRepresentable);
    }
    if rendered.len() > MAX_PDF_NUMBER_CHARS {
        return Err(CreasePatternExportError::PdfNumberTooLong {
            maximum: MAX_PDF_NUMBER_CHARS,
        });
    }
    Ok(rendered)
}

fn valid_pdf_number_syntax(value: &str) -> bool {
    let bytes = value.as_bytes();
    let digits = bytes.iter().filter(|byte| byte.is_ascii_digit()).count();
    digits != 0
        && bytes.iter().enumerate().all(|(index, byte)| {
            byte.is_ascii_digit() || *byte == b'.' || (*byte == b'-' && index == 0)
        })
        && bytes.iter().filter(|byte| **byte == b'.').count() <= 1
}

const fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn pdf_utf16be_hex_string(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";

    let code_units = value.encode_utf16();
    let (minimum_units, _) = code_units.size_hint();
    let mut output = String::with_capacity(4_usize.saturating_add(minimum_units.saturating_mul(4)));
    output.push_str("FEFF");
    for code_unit in code_units {
        for byte in code_unit.to_be_bytes() {
            output.push(char::from(HEX[usize::from(byte >> 4)]));
            output.push(char::from(HEX[usize::from(byte & 0x0F)]));
        }
    }
    output
}

fn serialize_pdf_objects(
    objects: [Vec<u8>; PDF_OBJECT_COUNT],
) -> Result<Vec<u8>, CreasePatternExportError> {
    let object_bytes = objects.iter().try_fold(0_usize, |total, object| {
        total
            .checked_add(object.len())
            .and_then(|value| value.checked_add(32))
    });
    let capacity = object_bytes
        .and_then(|value| value.checked_add(PDF_HEADER.len()))
        .and_then(|value| value.checked_add(256))
        .ok_or(CreasePatternExportError::PdfStructureNotRepresentable)?;
    let mut output = Vec::with_capacity(capacity);
    output.extend_from_slice(PDF_HEADER);

    let mut offsets = [0_usize; PDF_OBJECT_COUNT];
    for (index, object) in objects.into_iter().enumerate() {
        offsets[index] = output.len();
        if offsets[index] > MAX_CLASSIC_XREF_OFFSET {
            return Err(CreasePatternExportError::PdfStructureNotRepresentable);
        }
        output.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
        output.extend_from_slice(&object);
        output.extend_from_slice(b"\nendobj\n");
    }

    let xref_offset = output.len();
    if xref_offset > MAX_CLASSIC_XREF_OFFSET {
        return Err(CreasePatternExportError::PdfStructureNotRepresentable);
    }
    output.extend_from_slice(format!("xref\n0 {}\n", PDF_OBJECT_COUNT + 1).as_bytes());
    output.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets {
        output.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    output.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info 5 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
            PDF_OBJECT_COUNT + 1
        )
        .as_bytes(),
    );
    Ok(output)
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Point2, Vertex, VertexId};

    use super::*;

    fn sample_pattern() -> (CreasePattern, HashMap<VertexId, usize>) {
        let positions = [
            Point2::new(0.0, 0.0),
            Point2::new(20.0, 0.0),
            Point2::new(20.0, 20.0),
            Point2::new(0.0, 20.0),
            Point2::new(10.0, 10.0),
            Point2::new(-5.0, 10.0),
        ];
        let vertices = positions
            .into_iter()
            .map(|position| Vertex {
                id: VertexId::new(),
                position,
            })
            .collect::<Vec<_>>();
        let specifications = [
            (0, 1, EdgeKind::Boundary),
            (1, 2, EdgeKind::Boundary),
            (2, 3, EdgeKind::Boundary),
            (3, 0, EdgeKind::Boundary),
            (4, 0, EdgeKind::Mountain),
            (4, 1, EdgeKind::Valley),
            (5, 4, EdgeKind::Auxiliary),
            (4, 3, EdgeKind::Cut),
        ];
        let edges = specifications
            .into_iter()
            .map(|(start, end, kind)| Edge {
                id: EdgeId::new(),
                start: vertices[start].id,
                end: vertices[end].id,
                kind,
            })
            .collect();
        let indices = vertices
            .iter()
            .enumerate()
            .map(|(index, vertex)| (vertex.id, index))
            .collect();
        (CreasePattern { vertices, edges }, indices)
    }

    fn rectangle_pattern(width: f64, height: f64) -> (CreasePattern, HashMap<VertexId, usize>) {
        let positions = [
            Point2::new(0.0, 0.0),
            Point2::new(width, 0.0),
            Point2::new(width, height),
            Point2::new(0.0, height),
        ];
        let vertices = positions
            .into_iter()
            .map(|position| Vertex {
                id: VertexId::new(),
                position,
            })
            .collect::<Vec<_>>();
        let edges = [(0, 1), (1, 2), (2, 3), (3, 0)]
            .into_iter()
            .map(|(start, end)| Edge {
                id: EdgeId::new(),
                start: vertices[start].id,
                end: vertices[end].id,
                kind: EdgeKind::Boundary,
            })
            .collect();
        let indices = vertices
            .iter()
            .enumerate()
            .map(|(index, vertex)| (vertex.id, index))
            .collect();
        (CreasePattern { vertices, edges }, indices)
    }

    fn ascii_body(bytes: &[u8]) -> &str {
        std::str::from_utf8(&bytes[PDF_HEADER.len()..]).expect("PDF body is ASCII")
    }

    fn find_bytes(haystack: &[u8], needle: &[u8]) -> usize {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
            .expect("expected PDF marker")
    }

    fn content_stream(bytes: &[u8]) -> &str {
        let object_start = find_bytes(bytes, b"4 0 obj\n");
        let object = &bytes[object_start..];
        let stream_marker = b"\nstream\n";
        let stream_relative = find_bytes(object, stream_marker) + stream_marker.len();
        let length_prefix = b"<< /Length ";
        let length_start = find_bytes(object, length_prefix) + length_prefix.len();
        let length_end = object[length_start..]
            .iter()
            .position(|byte| *byte == b' ')
            .map(|relative| length_start + relative)
            .expect("stream length terminator");
        let declared_length = std::str::from_utf8(&object[length_start..length_end])
            .expect("ASCII length")
            .parse::<usize>()
            .expect("numeric length");
        let stream = &object[stream_relative..stream_relative + declared_length];
        assert_eq!(
            &object[stream_relative + declared_length..stream_relative + declared_length + 9],
            b"endstream"
        );
        std::str::from_utf8(stream).expect("ASCII content")
    }

    fn decode_title(bytes: &[u8]) -> String {
        let body = ascii_body(bytes);
        let marker = "/Title <";
        let start = body.find(marker).expect("title") + marker.len();
        let end = body[start..].find('>').expect("title end") + start;
        let hex = &body[start..end];
        assert_eq!(&hex[..4], "FEFF");
        let raw = hex.as_bytes()[4..]
            .chunks_exact(4)
            .map(|chunk| {
                let text = std::str::from_utf8(chunk).expect("hex text");
                u16::from_str_radix(text, 16).expect("UTF-16 code unit")
            })
            .collect::<Vec<_>>();
        char::decode_utf16(raw)
            .collect::<Result<String, _>>()
            .expect("valid UTF-16")
    }

    #[test]
    fn emits_a_deterministic_structurally_consistent_pdf17() {
        let (pattern, indices) = sample_pattern();
        let first = serialize_pdf17("構造検証", &pattern, &indices).expect("PDF");
        let second = serialize_pdf17("構造検証", &pattern, &indices).expect("PDF");
        assert_eq!(first, second);
        assert!(first.starts_with(PDF_HEADER));
        assert!(first.ends_with(b"%%EOF\n"));

        let xref_offset = find_bytes(&first, b"xref\n");
        let body = ascii_body(&first);
        let declared_start = body
            .rsplit_once("startxref\n")
            .expect("startxref")
            .1
            .lines()
            .next()
            .expect("xref offset")
            .parse::<usize>()
            .expect("numeric xref offset");
        assert_eq!(declared_start, xref_offset);

        let xref = &body[xref_offset - PDF_HEADER.len()..];
        let mut lines = xref.lines();
        assert_eq!(lines.next(), Some("xref"));
        assert_eq!(lines.next(), Some("0 6"));
        assert_eq!(lines.next(), Some("0000000000 65535 f "));
        for object_number in 1..=PDF_OBJECT_COUNT {
            let entry = lines.next().expect("xref entry");
            assert_eq!(entry.len(), 19);
            assert_eq!(&entry[10..], " 00000 n ");
            let offset = entry[..10].parse::<usize>().expect("object offset");
            assert_eq!(
                &first[offset..offset + format!("{object_number} 0 obj\n").len()],
                format!("{object_number} 0 obj\n").as_bytes()
            );
        }
        assert!(body.contains("trailer\n<< /Size 6 /Root 1 0 R /Info 5 0 R >>"));
        assert!(!body.contains("/CreationDate"));
        assert!(!body.contains("/ModDate"));
        assert!(!body.contains("/ID"));

        let stream = content_stream(&first);
        assert!(stream.starts_with("q\n"));
        assert!(stream.ends_with("Q\n"));
    }

    #[test]
    fn preserves_full_scale_bounds_and_uses_five_distinct_black_point_line_styles() {
        let (pattern, indices) = sample_pattern();
        let bytes = serialize_pdf17("線種", &pattern, &indices).expect("PDF");
        let body = ascii_body(&bytes);
        let stream = content_stream(&bytes);
        let expected_width = pdf_number(45.0 * PDF_POINTS_PER_MILLIMETRE).unwrap();
        let expected_height = pdf_number(40.0 * PDF_POINTS_PER_MILLIMETRE).unwrap();
        let expected_box = format!("[0 0 {expected_width} {expected_height}]");
        assert_eq!(body.matches(&expected_box).count(), 2);
        assert!(body.contains("/ViewerPreferences << /PrintScaling /None >>"));

        let scale = pdf_number(PDF_POINTS_PER_MILLIMETRE).unwrap();
        let margin = pdf_number(10.0 * PDF_POINTS_PER_MILLIMETRE).unwrap();
        assert!(stream.contains(&format!("{scale} 0 0 {scale} {margin} {margin} cm")));
        assert_eq!(stream.matches(" G\n").count(), 1);
        assert!(stream.contains("0 G\n"));
        assert!(!stream.contains(" RG"));
        assert!(!stream.contains(" K"));

        let style_signatures = [
            (0.50, 0, "[] 0 d\n"),
            (0.35, 0, "[6 2 1 2] 0 d\n"),
            (0.35, 0, "[3 1.5] 0 d\n"),
            (0.25, 1, "[0.5 1.5] 0 d\n"),
            (0.60, 0, "[8 2 1 2 1 2] 0 d\n"),
        ]
        .map(|(width_points, line_cap, dash)| {
            let user_width = width_points / PDF_POINTS_PER_MILLIMETRE;
            let encoded_width = pdf_number(user_width).expect("PDF width");
            assert!(
                (encoded_width.parse::<f64>().unwrap() * PDF_POINTS_PER_MILLIMETRE - width_points)
                    .abs()
                    <= f64::EPSILON
            );
            format!("{encoded_width} w\n{line_cap} J\n{dash}")
        });
        assert_eq!(style_signatures.iter().collect::<HashSet<_>>().len(), 5);
        for signature in style_signatures {
            assert!(stream.contains(&signature), "missing {signature:?}");
        }
        assert_eq!(stream.matches(" m ").count(), pattern.edges.len());
        assert_eq!(stream.lines().filter(|line| *line == "S").count(), 5);

        // The auxiliary endpoint at x=-5 expands the drawing bounds rather than
        // being clipped to the paper's 0..20 mm bounds.
        assert!(stream.contains("0 10 m 15 10 l"));
    }

    #[test]
    fn canonicalizes_edge_direction_order_and_negative_zero() {
        let (pattern, indices) = sample_pattern();
        let expected = serialize_pdf17("canonical", &pattern, &indices).expect("PDF");

        let mut reordered = pattern.clone();
        for edge in &mut reordered.edges {
            std::mem::swap(&mut edge.start, &mut edge.end);
        }
        reordered.edges.reverse();
        let actual = serialize_pdf17("canonical", &reordered, &indices).expect("PDF");
        assert_eq!(actual, expected);
        assert!(
            !content_stream(&actual)
                .split_ascii_whitespace()
                .any(|token| token == "-0" || token == "-0.0")
        );
    }

    #[test]
    fn stores_unicode_title_as_injection_safe_utf16be_metadata() {
        let (pattern, indices) = sample_pattern();
        let title = "鶴🪽\0\r\n () \\\\ <> /Title <BAD>";
        let bytes = serialize_pdf17(title, &pattern, &indices).expect("PDF");
        assert_eq!(decode_title(&bytes), title);
        let body = ascii_body(&bytes);
        assert!(!body.contains(title));
        assert_eq!(body.matches("/Title <").count(), 1);
        assert!(body.contains("/Creator (ORIGAMI2)"));
        assert!(body.contains("/Producer (ORIGAMI2 deterministic PDF exporter)"));

        let maximum_non_bmp_title = "🙂".repeat(512);
        let bytes =
            serialize_pdf17(&maximum_non_bmp_title, &pattern, &indices).expect("maximum title");
        assert_eq!(decode_title(&bytes), maximum_non_bmp_title);
    }

    #[test]
    fn accepts_the_exact_page_limit_on_both_axes_and_rejects_larger_pages() {
        let (exact_width, width_indices) = rectangle_pattern(5_060.0, 1.0);
        let bytes =
            serialize_pdf17("exact width", &exact_width, &width_indices).expect("exact width");
        assert!(ascii_body(&bytes).contains("/MediaBox [0 0 14400 "));

        let (exact_height, height_indices) = rectangle_pattern(1.0, 5_060.0);
        let bytes =
            serialize_pdf17("exact height", &exact_height, &height_indices).expect("exact height");
        assert!(ascii_body(&bytes).contains(" 14400] /CropBox"));

        let (exact_square, square_indices) = rectangle_pattern(5_060.0, 5_060.0);
        let bytes =
            serialize_pdf17("exact square", &exact_square, &square_indices).expect("exact square");
        assert!(ascii_body(&bytes).contains("/MediaBox [0 0 14400 14400]"));

        let (oversized_width, oversized_width_indices) = rectangle_pattern(5_060.000_000_1, 1.0);
        assert!(matches!(
            serialize_pdf17(
                "oversized width",
                &oversized_width,
                &oversized_width_indices
            ),
            Err(CreasePatternExportError::PdfPageTooLarge)
        ));
        let (oversized_height, oversized_height_indices) = rectangle_pattern(1.0, 5_060.000_000_1);
        assert!(matches!(
            serialize_pdf17(
                "oversized height",
                &oversized_height,
                &oversized_height_indices
            ),
            Err(CreasePatternExportError::PdfPageTooLarge)
        ));
    }

    #[test]
    fn rejects_unrepresentable_bounds_numeric_tokens_and_vertex_maps() {
        let (mut non_finite, indices) = rectangle_pattern(1.0, 1.0);
        non_finite.vertices[0].position.x = f64::NAN;
        assert!(matches!(
            serialize_pdf17("non-finite", &non_finite, &indices),
            Err(CreasePatternExportError::DrawingBoundsNotRepresentable)
        ));

        let (flat, flat_indices) = rectangle_pattern(0.0, 1.0);
        assert!(matches!(
            serialize_pdf17("flat", &flat, &flat_indices),
            Err(CreasePatternExportError::DrawingBoundsNotRepresentable)
        ));

        let (mut tiny, tiny_indices) = rectangle_pattern(1.0, 1.0);
        tiny.vertices[0].position.x = 1.0e-100;
        assert!(matches!(
            serialize_pdf17("tiny", &tiny, &tiny_indices),
            Err(CreasePatternExportError::PdfNumberTooLong {
                maximum: MAX_PDF_NUMBER_CHARS
            })
        ));

        let (pattern, mut missing_indices) = sample_pattern();
        missing_indices.remove(&pattern.edges[0].start);
        assert!(matches!(
            serialize_pdf17("missing", &pattern, &missing_indices),
            Err(CreasePatternExportError::PdfStructureNotRepresentable)
        ));
    }

    #[test]
    fn emitted_numeric_tokens_are_plain_decimal_and_bounded() {
        for value in [
            -123.456,
            0.0,
            -0.0,
            1.0e-20,
            PDF_POINTS_PER_MILLIMETRE,
            PDF_MAX_PAGE_POINTS,
        ] {
            let number = pdf_number(value).expect("representable number");
            assert!(!number.contains(['e', 'E']));
            assert!(number.len() <= MAX_PDF_NUMBER_CHARS);
            assert!(valid_pdf_number_syntax(&number));
        }
        assert_eq!(pdf_number(-0.0).unwrap(), "0");
        assert!(matches!(
            pdf_number(1.0e-100),
            Err(CreasePatternExportError::PdfNumberTooLong {
                maximum: MAX_PDF_NUMBER_CHARS
            })
        ));
    }
}
