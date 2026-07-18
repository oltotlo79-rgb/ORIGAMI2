use std::fmt::{self, Write as _};

use super::{
    CanonicalInstructionPlanV1, InstructionExportError,
    font::{GlyphPathCommand, InstructionFont},
    layout::{
        InstructionPage, PAGE_HEIGHT_POINTS, PAGE_WIDTH_POINTS, PageColor, PageLine, PageLineDash,
        PagePoint, PagePolygon, PageText,
    },
};

const PDF_HEADER: &[u8] = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n";
const MAX_PDF_NUMBER_CHARS: usize = 64;
const MAX_CLASSIC_XREF_OFFSET: usize = 9_999_999_999;

pub(super) fn serialize_instruction_pdf(
    plan: &CanonicalInstructionPlanV1,
    font: &InstructionFont<'_>,
    max_page_bytes: usize,
    max_output_bytes: usize,
) -> Result<Vec<u8>, InstructionExportError> {
    plan.validate(font)?;
    serialize_instruction_pdf_pages(
        &plan.title,
        &plan.pages,
        font,
        max_page_bytes,
        max_output_bytes,
    )
}

fn serialize_instruction_pdf_pages(
    title: &str,
    pages: &[InstructionPage],
    font: &InstructionFont<'_>,
    max_page_bytes: usize,
    max_output_bytes: usize,
) -> Result<Vec<u8>, InstructionExportError> {
    if pages.is_empty() {
        return Err(InstructionExportError::StructureNotRepresentable);
    }

    let mut content_bytes = 0_usize;
    let mut contents = Vec::with_capacity(pages.len());
    for page in pages {
        let content = serialize_page_content(page, font, max_page_bytes)?;
        content_bytes = content_bytes.checked_add(content.len()).ok_or(
            InstructionExportError::OutputTooLarge {
                actual: usize::MAX,
                maximum: max_output_bytes,
            },
        )?;
        if content_bytes > max_output_bytes {
            return Err(InstructionExportError::OutputTooLarge {
                actual: content_bytes,
                maximum: max_output_bytes,
            });
        }
        contents.push(content);
    }
    serialize_document(title, contents, max_output_bytes)
}

fn serialize_page_content(
    page: &InstructionPage,
    font: &InstructionFont<'_>,
    maximum: usize,
) -> Result<Vec<u8>, InstructionExportError> {
    if !page.has_white_page_background() {
        return Err(InstructionExportError::StructureNotRepresentable);
    }
    let mut output = ContentWriter::new(maximum);
    output.push_str("q\n1 J\n1 j\n")?;

    for polygon in &page.polygons {
        append_polygon(&mut output, polygon)?;
    }
    for line in &page.lines {
        append_line(&mut output, line)?;
    }
    for text in &page.texts {
        append_text(&mut output, text, font)?;
    }

    output.push_str("Q\n")?;
    Ok(output.finish().into_bytes())
}

fn append_polygon(
    output: &mut ContentWriter,
    polygon: &PagePolygon,
) -> Result<(), InstructionExportError> {
    if polygon.points.len() < 3 || !polygon.stroke_width.is_finite() || polygon.stroke_width <= 0.0
    {
        return Err(InstructionExportError::StructureNotRepresentable);
    }

    output.push_str("q\n")?;
    append_color(output, polygon.fill, "rg")?;
    append_color(output, polygon.stroke, "RG")?;
    output.push_fmt(format_args!("{} w\n", pdf_number(polygon.stroke_width)?))?;

    let (first_x, first_y) = pdf_page_point(polygon.points[0])?;
    output.push_fmt(format_args!(
        "{} {} m\n",
        pdf_number(first_x)?,
        pdf_number(first_y)?
    ))?;
    for point in &polygon.points[1..] {
        let (x, y) = pdf_page_point(*point)?;
        output.push_fmt(format_args!("{} {} l\n", pdf_number(x)?, pdf_number(y)?))?;
    }
    output.push_str("h\nB\nQ\n")
}

fn append_line(output: &mut ContentWriter, line: &PageLine) -> Result<(), InstructionExportError> {
    if !line.width.is_finite() || line.width <= 0.0 {
        return Err(InstructionExportError::StructureNotRepresentable);
    }
    let (start_x, start_y) = pdf_page_point(line.start)?;
    let (end_x, end_y) = pdf_page_point(line.end)?;

    output.push_str("q\n")?;
    append_color(output, line.color, "RG")?;
    output.push_fmt(format_args!("{} w\n", pdf_number(line.width)?))?;
    output.push_str(match line.dash {
        PageLineDash::Solid => "[] 0 d\n",
        PageLineDash::Dashed => "[5.669291 2.834646] 0 d\n",
        PageLineDash::DashDot => "[11.338583 2.834646 2.834646 2.834646] 0 d\n",
    })?;
    output.push_fmt(format_args!(
        "{} {} m\n{} {} l\nS\nQ\n",
        pdf_number(start_x)?,
        pdf_number(start_y)?,
        pdf_number(end_x)?,
        pdf_number(end_y)?
    ))
}

fn append_text(
    output: &mut ContentWriter,
    text: &PageText,
    font: &InstructionFont<'_>,
) -> Result<(), InstructionExportError> {
    text.validate()?;

    let face = font.face();
    let units_per_em = f64::from(face.units_per_em());
    let scale = text.font_size / units_per_em;
    let baseline_y = PAGE_HEIGHT_POINTS - text.baseline_y;
    if !scale.is_finite() || scale <= 0.0 || !baseline_y.is_finite() {
        return Err(InstructionExportError::StructureNotRepresentable);
    }

    output.push_str("q\n")?;
    append_color(output, text.color, "rg")?;
    for glyph in &text.glyphs {
        let outline = font.glyph_outline(glyph.glyph_id, glyph.x, baseline_y, scale, scale)?;
        if !outline.outlined && !glyph.scalar.is_whitespace() {
            return Err(InstructionExportError::StructureNotRepresentable);
        }
        if !outline.commands.is_empty() {
            append_glyph_commands(output, &outline.commands)?;
            output.push_str("f\n")?;
        }
    }
    output.push_str("Q\n")
}

fn append_glyph_commands(
    output: &mut ContentWriter,
    commands: &[GlyphPathCommand],
) -> Result<(), InstructionExportError> {
    for command in commands {
        match *command {
            GlyphPathCommand::Move { x, y } => {
                output.push_fmt(format_args!("{} {} m\n", pdf_number(x)?, pdf_number(y)?))?
            }
            GlyphPathCommand::Line { x, y } => {
                output.push_fmt(format_args!("{} {} l\n", pdf_number(x)?, pdf_number(y)?))?
            }
            GlyphPathCommand::Cubic {
                control_1_x,
                control_1_y,
                control_2_x,
                control_2_y,
                x,
                y,
            } => output.push_fmt(format_args!(
                "{} {} {} {} {} {} c\n",
                pdf_number(control_1_x)?,
                pdf_number(control_1_y)?,
                pdf_number(control_2_x)?,
                pdf_number(control_2_y)?,
                pdf_number(x)?,
                pdf_number(y)?
            ))?,
            GlyphPathCommand::Close => output.push_str("h\n")?,
        }
    }
    Ok(())
}

fn append_color(
    output: &mut ContentWriter,
    color: PageColor,
    operator: &str,
) -> Result<(), InstructionExportError> {
    let red = pdf_number(f64::from(color.red) / 255.0)?;
    let green = pdf_number(f64::from(color.green) / 255.0)?;
    let blue = pdf_number(f64::from(color.blue) / 255.0)?;
    output.push_fmt(format_args!("{red} {green} {blue} {operator}\n"))
}

fn pdf_page_point(point: PagePoint) -> Result<(f64, f64), InstructionExportError> {
    if !point.x.is_finite()
        || !point.y.is_finite()
        || !(0.0..=PAGE_WIDTH_POINTS).contains(&point.x)
        || !(0.0..=PAGE_HEIGHT_POINTS).contains(&point.y)
    {
        return Err(InstructionExportError::StructureNotRepresentable);
    }
    let y = PAGE_HEIGHT_POINTS - point.y;
    if !y.is_finite() {
        return Err(InstructionExportError::StructureNotRepresentable);
    }
    Ok((canonical_zero(point.x), canonical_zero(y)))
}

struct ContentWriter {
    output: String,
    maximum: usize,
}

impl ContentWriter {
    fn new(maximum: usize) -> Self {
        Self {
            output: String::with_capacity(maximum.min(8 * 1024)),
            maximum,
        }
    }

    fn push_str(&mut self, value: &str) -> Result<(), InstructionExportError> {
        let next = self.output.len().checked_add(value.len()).ok_or(
            InstructionExportError::PageTooLarge {
                maximum: self.maximum,
            },
        )?;
        if next > self.maximum {
            return Err(InstructionExportError::PageTooLarge {
                maximum: self.maximum,
            });
        }
        self.output.push_str(value);
        Ok(())
    }

    fn push_fmt(&mut self, arguments: fmt::Arguments<'_>) -> Result<(), InstructionExportError> {
        let mut rendered = String::new();
        rendered
            .write_fmt(arguments)
            .map_err(|_| InstructionExportError::StructureNotRepresentable)?;
        self.push_str(&rendered)
    }

    fn finish(self) -> String {
        self.output
    }
}

fn serialize_document(
    title: &str,
    contents: Vec<Vec<u8>>,
    max_output_bytes: usize,
) -> Result<Vec<u8>, InstructionExportError> {
    let object_count = contents
        .len()
        .checked_mul(2)
        .and_then(|count| count.checked_add(3))
        .ok_or(InstructionExportError::StructureNotRepresentable)?;
    let info_object_number = object_count;
    let width = pdf_number(PAGE_WIDTH_POINTS)?;
    let height = pdf_number(PAGE_HEIGHT_POINTS)?;

    let mut objects = Vec::with_capacity(object_count);
    objects.push(
        b"<< /Type /Catalog /Pages 2 0 R /ViewerPreferences << /PrintScaling /None >> >>".to_vec(),
    );

    let mut page_tree = String::from("<< /Type /Pages /Kids [");
    for index in 0..contents.len() {
        let page_object_number = index
            .checked_mul(2)
            .and_then(|number| number.checked_add(3))
            .ok_or(InstructionExportError::StructureNotRepresentable)?;
        if index != 0 {
            page_tree.push(' ');
        }
        page_tree.push_str(&format!("{page_object_number} 0 R"));
    }
    page_tree.push_str(&format!("] /Count {} >>", contents.len()));
    objects.push(page_tree.into_bytes());

    for (index, content) in contents.into_iter().enumerate() {
        let content_object_number = index
            .checked_mul(2)
            .and_then(|number| number.checked_add(4))
            .ok_or(InstructionExportError::StructureNotRepresentable)?;
        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] /CropBox [0 0 {width} {height}] /Resources << >> /Contents {content_object_number} 0 R >>"
            )
            .into_bytes(),
        );
        let mut stream = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
        stream.extend_from_slice(&content);
        stream.extend_from_slice(b"endstream");
        objects.push(stream);
    }

    let title_hex = pdf_utf16be_hex_string(title);
    objects.push(
        format!(
            "<< /Title <{title_hex}> /Creator (ORIGAMI2) /Producer (ORIGAMI2 deterministic instruction PDF exporter) >>"
        )
        .into_bytes(),
    );
    if objects.len() != object_count {
        return Err(InstructionExportError::StructureNotRepresentable);
    }

    serialize_pdf_objects(objects, info_object_number, max_output_bytes)
}

fn serialize_pdf_objects(
    objects: Vec<Vec<u8>>,
    info_object_number: usize,
    maximum: usize,
) -> Result<Vec<u8>, InstructionExportError> {
    if objects.is_empty() || info_object_number == 0 || info_object_number > objects.len() {
        return Err(InstructionExportError::StructureNotRepresentable);
    }
    let object_bytes = objects.iter().try_fold(0_usize, |total, object| {
        total
            .checked_add(object.len())
            .and_then(|value| value.checked_add(32))
    });
    let capacity = object_bytes
        .and_then(|value| value.checked_add(PDF_HEADER.len()))
        .and_then(|value| value.checked_add(512))
        .ok_or(InstructionExportError::StructureNotRepresentable)?;
    let mut output = Vec::with_capacity(capacity.min(maximum));
    append_bounded(&mut output, PDF_HEADER, maximum)?;

    let mut offsets = Vec::with_capacity(objects.len());
    for (index, object) in objects.into_iter().enumerate() {
        offsets.push(checked_classic_xref_offset(output.len())?);
        append_bounded(
            &mut output,
            format!("{} 0 obj\n", index + 1).as_bytes(),
            maximum,
        )?;
        append_bounded(&mut output, &object, maximum)?;
        append_bounded(&mut output, b"\nendobj\n", maximum)?;
    }

    let xref_offset = checked_classic_xref_offset(output.len())?;
    append_bounded(
        &mut output,
        format!("xref\n0 {}\n", offsets.len() + 1).as_bytes(),
        maximum,
    )?;
    append_bounded(&mut output, b"0000000000 65535 f \n", maximum)?;
    for offset in offsets {
        append_bounded(
            &mut output,
            format!("{offset:010} 00000 n \n").as_bytes(),
            maximum,
        )?;
    }
    append_bounded(
        &mut output,
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info {info_object_number} 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
            info_object_number + 1
        )
        .as_bytes(),
        maximum,
    )?;
    Ok(output)
}

fn append_bounded(
    output: &mut Vec<u8>,
    bytes: &[u8],
    maximum: usize,
) -> Result<(), InstructionExportError> {
    let next =
        output
            .len()
            .checked_add(bytes.len())
            .ok_or(InstructionExportError::OutputTooLarge {
                actual: usize::MAX,
                maximum,
            })?;
    if next > maximum {
        return Err(InstructionExportError::OutputTooLarge {
            actual: next,
            maximum,
        });
    }
    output.extend_from_slice(bytes);
    Ok(())
}

fn checked_classic_xref_offset(value: usize) -> Result<usize, InstructionExportError> {
    if value > MAX_CLASSIC_XREF_OFFSET {
        Err(InstructionExportError::StructureNotRepresentable)
    } else {
        Ok(value)
    }
}

fn pdf_number(value: f64) -> Result<String, InstructionExportError> {
    if !value.is_finite() {
        return Err(InstructionExportError::StructureNotRepresentable);
    }
    let rendered = canonical_zero(value).to_string();
    if rendered.contains(['e', 'E'])
        || rendered.len() > MAX_PDF_NUMBER_CHARS
        || !valid_pdf_number_syntax(&rendered)
    {
        return Err(InstructionExportError::StructureNotRepresentable);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn color(red: u8, green: u8, blue: u8) -> PageColor {
        PageColor { red, green, blue }
    }

    fn sample_page(font: &InstructionFont<'_>, text: &str) -> InstructionPage {
        InstructionPage {
            step_number: 1,
            continuation_number: 0,
            polygons: vec![
                PagePolygon {
                    points: vec![
                        PagePoint { x: 0.0, y: 0.0 },
                        PagePoint {
                            x: PAGE_WIDTH_POINTS,
                            y: 0.0,
                        },
                        PagePoint {
                            x: PAGE_WIDTH_POINTS,
                            y: PAGE_HEIGHT_POINTS,
                        },
                        PagePoint {
                            x: 0.0,
                            y: PAGE_HEIGHT_POINTS,
                        },
                    ],
                    fill: PageColor::WHITE,
                    stroke: PageColor::WHITE,
                    stroke_width: 0.1,
                },
                PagePolygon {
                    points: vec![
                        PagePoint { x: 36.0, y: 80.0 },
                        PagePoint { x: 180.0, y: 80.0 },
                        PagePoint { x: 180.0, y: 180.0 },
                        PagePoint { x: 36.0, y: 180.0 },
                    ],
                    fill: color(250, 250, 250),
                    stroke: color(32, 35, 42),
                    stroke_width: 0.7,
                },
            ],
            lines: vec![PageLine {
                start: PagePoint { x: 50.0, y: 95.0 },
                end: PagePoint { x: 165.0, y: 165.0 },
                color: color(196, 48, 62),
                width: 1.5,
                dash: PageLineDash::DashDot,
            }],
            texts: PageText::from_text(42.0, 54.0, 12.0, color(32, 35, 42), text, font)
                .expect("layout sample text")
                .into_iter()
                .collect(),
        }
    }

    fn find_bytes(haystack: &[u8], needle: &[u8]) -> usize {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
            .expect("expected PDF marker")
    }

    fn ascii_body(bytes: &[u8]) -> &str {
        std::str::from_utf8(&bytes[PDF_HEADER.len()..]).expect("PDF body is ASCII")
    }

    #[test]
    fn emits_a4_multipage_pdf17_with_consistent_classic_xref() {
        let font = InstructionFont::load().expect("bundled font");
        let pages = [sample_page(&font, "Step 1"), sample_page(&font, "Step 2")];
        let bytes =
            serialize_instruction_pdf_pages("折り図", &pages, &font, usize::MAX, usize::MAX)
                .expect("PDF");
        let body = ascii_body(&bytes);

        assert!(bytes.starts_with(PDF_HEADER));
        assert!(bytes.ends_with(b"%%EOF\n"));
        assert!(body.contains("/ViewerPreferences << /PrintScaling /None >>"));
        assert!(body.contains("/Kids [3 0 R 5 0 R] /Count 2"));
        assert_eq!(body.matches("/Type /Page ").count(), 2);
        let page_box = format!(
            "[0 0 {} {}]",
            pdf_number(PAGE_WIDTH_POINTS).unwrap(),
            pdf_number(PAGE_HEIGHT_POINTS).unwrap()
        );
        assert_eq!(body.matches(&page_box).count(), 4);
        assert_eq!(body.matches("\nstream\n").count(), 2);
        assert!(body.contains("trailer\n<< /Size 8 /Root 1 0 R /Info 7 0 R >>"));

        let xref_offset = find_bytes(&bytes, b"xref\n");
        let declared_xref = body
            .rsplit_once("startxref\n")
            .expect("startxref")
            .1
            .lines()
            .next()
            .expect("xref offset")
            .parse::<usize>()
            .expect("numeric offset");
        assert_eq!(declared_xref, xref_offset);

        let mut xref_lines = body[xref_offset - PDF_HEADER.len()..].lines();
        assert_eq!(xref_lines.next(), Some("xref"));
        assert_eq!(xref_lines.next(), Some("0 8"));
        assert_eq!(xref_lines.next(), Some("0000000000 65535 f "));
        for object_number in 1..=7 {
            let entry = xref_lines.next().expect("xref entry");
            assert_eq!(entry.len(), 19);
            let offset = entry[..10].parse::<usize>().expect("object offset");
            assert_eq!(
                &bytes[offset..offset + format!("{object_number} 0 obj\n").len()],
                format!("{object_number} 0 obj\n").as_bytes()
            );
        }
    }

    #[test]
    fn is_deterministic_and_embeds_japanese_as_outlines_only() {
        let font = InstructionFont::load().expect("bundled font");
        let pages = [sample_page(&font, "山折りと谷折り")];
        let first = serialize_instruction_pdf_pages(
            "日本語の折り図",
            &pages,
            &font,
            usize::MAX,
            usize::MAX,
        )
        .expect("PDF");
        let second = serialize_instruction_pdf_pages(
            "日本語の折り図",
            &pages,
            &font,
            usize::MAX,
            usize::MAX,
        )
        .expect("PDF");
        assert_eq!(first, second);

        let body = ascii_body(&first);
        assert!(body.contains(&format!(
            "/Title <{}>",
            pdf_utf16be_hex_string("日本語の折り図")
        )));
        assert!(body.contains("/Creator (ORIGAMI2)"));
        assert!(body.contains("/Producer (ORIGAMI2 deterministic instruction PDF exporter)"));
        assert!(body.contains(" c\n"));
        assert!(!body.contains("/Font"));
        assert!(!body.contains("BT"));
        assert!(!body.contains(" Tj"));
        assert!(!body.contains("/JavaScript"));
        assert!(!body.contains("/Launch"));
        assert!(!body.contains("/URI"));
        assert!(!body.contains("/CreationDate"));
        assert!(!body.contains("/ModDate"));
        assert!(
            !first
                .windows("日本語".len())
                .any(|window| window == "日本語".as_bytes())
        );
    }

    #[test]
    fn accepts_spaces_without_an_outline_and_advances_them() {
        let font = InstructionFont::load().expect("bundled font");
        let mut page = sample_page(&font, "   ");
        page.polygons.clear();
        page.polygons.push(PagePolygon {
            points: vec![
                PagePoint { x: 0.0, y: 0.0 },
                PagePoint {
                    x: PAGE_WIDTH_POINTS,
                    y: 0.0,
                },
                PagePoint {
                    x: PAGE_WIDTH_POINTS,
                    y: PAGE_HEIGHT_POINTS,
                },
                PagePoint {
                    x: 0.0,
                    y: PAGE_HEIGHT_POINTS,
                },
            ],
            fill: PageColor::WHITE,
            stroke: PageColor::WHITE,
            stroke_width: 0.1,
        });
        page.lines.clear();
        let content = serialize_page_content(&page, &font, usize::MAX).expect("space content");
        let content = std::str::from_utf8(&content).expect("ASCII");
        assert!(!content.lines().any(|line| line == "f"));
        assert!(content.contains("q\n"));
        assert!(content.ends_with("Q\n"));
    }

    #[test]
    fn consumes_frozen_glyph_positions_and_starts_with_a_white_page() {
        let font = InstructionFont::load().expect("bundled font");
        let page = sample_page(&font, "AB");
        let original = serialize_page_content(&page, &font, usize::MAX).expect("original content");
        let original_ascii = std::str::from_utf8(&original).expect("ASCII");
        assert!(original_ascii.starts_with("q\n1 J\n1 j\nq\n1 1 1 rg\n1 1 1 RG\n"));

        let mut moved = page;
        for glyph in &mut moved.texts[0].glyphs {
            glyph.x = ((glyph.x + 20.0) * 1_000_000.0).round() / 1_000_000.0;
        }
        let moved = serialize_page_content(&moved, &font, usize::MAX).expect("moved content");
        assert_ne!(original, moved);
    }

    #[test]
    fn enforces_the_page_content_limit_without_partial_output() {
        let font = InstructionFont::load().expect("bundled font");
        let page = sample_page(&font, "limit");
        let content = serialize_page_content(&page, &font, usize::MAX).expect("content");

        serialize_instruction_pdf_pages(
            "limit",
            std::slice::from_ref(&page),
            &font,
            content.len(),
            usize::MAX,
        )
        .expect("exact limit");
        assert!(matches!(
            serialize_instruction_pdf_pages(
                "limit",
                std::slice::from_ref(&page),
                &font,
                content.len() - 1,
                usize::MAX,
            ),
            Err(InstructionExportError::PageTooLarge { maximum }) if maximum == content.len() - 1
        ));

        let complete = serialize_instruction_pdf_pages(
            "output limit",
            std::slice::from_ref(&page),
            &font,
            usize::MAX,
            usize::MAX,
        )
        .expect("complete PDF");
        serialize_instruction_pdf_pages(
            "output limit",
            std::slice::from_ref(&page),
            &font,
            usize::MAX,
            complete.len(),
        )
        .expect("exact output limit");
        assert!(matches!(
            serialize_instruction_pdf_pages(
                "output limit",
                std::slice::from_ref(&page),
                &font,
                usize::MAX,
                complete.len() - 1,
            ),
            Err(InstructionExportError::OutputTooLarge { maximum, .. })
                if maximum == complete.len() - 1
        ));
    }

    #[test]
    fn rejects_empty_or_unrepresentable_page_structures_and_missing_glyphs() {
        let font = InstructionFont::load().expect("bundled font");
        assert!(matches!(
            serialize_instruction_pdf_pages("empty", &[], &font, usize::MAX, usize::MAX),
            Err(InstructionExportError::StructureNotRepresentable)
        ));

        let mut malformed_polygon = sample_page(&font, "valid");
        malformed_polygon.polygons[1].points.truncate(2);
        assert!(matches!(
            serialize_instruction_pdf_pages(
                "bad polygon",
                &[malformed_polygon],
                &font,
                usize::MAX,
                usize::MAX,
            ),
            Err(InstructionExportError::StructureNotRepresentable)
        ));

        let mut malformed_line = sample_page(&font, "valid");
        malformed_line.lines[0].end.x = f64::NAN;
        assert!(matches!(
            serialize_instruction_pdf_pages(
                "bad line",
                &[malformed_line],
                &font,
                usize::MAX,
                usize::MAX,
            ),
            Err(InstructionExportError::StructureNotRepresentable)
        ));

        let mut malformed_text = sample_page(&font, "valid");
        malformed_text.texts[0].font_size = 0.0;
        assert!(matches!(
            serialize_instruction_pdf_pages(
                "bad text",
                &[malformed_text],
                &font,
                usize::MAX,
                usize::MAX,
            ),
            Err(InstructionExportError::StructureNotRepresentable)
        ));

        let mut missing_glyph = sample_page(&font, "有効");
        missing_glyph.texts[0].glyphs[0].glyph_id = u16::MAX;
        assert!(matches!(
            serialize_instruction_pdf_pages(
                "missing glyph",
                &[missing_glyph],
                &font,
                usize::MAX,
                usize::MAX,
            ),
            Err(InstructionExportError::StructureNotRepresentable)
        ));
    }

    #[test]
    fn numeric_tokens_are_plain_finite_and_bounded() {
        for value in [
            -123.456,
            0.0,
            -0.0,
            1.0e-20,
            PAGE_WIDTH_POINTS,
            PAGE_HEIGHT_POINTS,
        ] {
            let rendered = pdf_number(value).expect("representable");
            assert!(!rendered.contains(['e', 'E']));
            assert!(rendered.len() <= MAX_PDF_NUMBER_CHARS);
            assert!(valid_pdf_number_syntax(&rendered));
        }
        assert_eq!(pdf_number(-0.0).unwrap(), "0");
        assert!(pdf_number(f64::NAN).is_err());
        assert!(pdf_number(1.0e-100).is_err());
        assert_eq!(
            checked_classic_xref_offset(MAX_CLASSIC_XREF_OFFSET).unwrap(),
            MAX_CLASSIC_XREF_OFFSET
        );
        assert!(checked_classic_xref_offset(MAX_CLASSIC_XREF_OFFSET + 1).is_err());
    }
}
