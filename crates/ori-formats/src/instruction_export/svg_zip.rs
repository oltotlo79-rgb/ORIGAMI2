use std::{
    fmt::Write as _,
    io::{Cursor, Write as _},
};

use serde::Serialize;
use zip::{CompressionMethod, DateTime, ZipWriter, write::SimpleFileOptions};

use super::{
    CanonicalInstructionPlanV1, InstructionExportError,
    font::{
        GlyphPathCommand, InstructionFont, NOTO_SANS_JP_BYTES, NOTO_SANS_JP_LICENSE,
        NOTO_SANS_JP_LICENSE_SHA256, NOTO_SANS_JP_SHA256,
    },
    layout::{InstructionPage, PAGE_HEIGHT_POINTS, PAGE_WIDTH_POINTS, PageColor, PageLineDash},
};

const MANIFEST_PATH: &str = "manifest.json";
const FONT_PATH: &str = "fonts/NotoSansJP-Variable.ttf";
const FONT_LICENSE_PATH: &str = "licenses/NotoSansJP-OFL.txt";
const MAX_NUMBER_CHARS: usize = 64;

#[derive(Serialize)]
struct SvgPageManifest<'a> {
    schema: &'static str,
    generator: &'static str,
    profile: &'static str,
    projection_profile: &'static str,
    format: &'static str,
    title: &'a str,
    page_count: usize,
    step_count: usize,
    page_size: &'static str,
    pages: Vec<SvgPageEntry>,
    font: SvgFontEntry,
    warnings: Vec<SvgWarningEntry>,
}

#[derive(Serialize)]
struct SvgPageEntry {
    page_number: usize,
    step_number: usize,
    kind: &'static str,
    continuation_number: usize,
    file: String,
}

#[derive(Serialize)]
struct SvgFontEntry {
    family: &'static str,
    path: &'static str,
    license_path: &'static str,
    sha256: &'static str,
    license_sha256: &'static str,
}

#[derive(Serialize)]
struct SvgWarningEntry {
    category: &'static str,
    message_ja: &'static str,
}

pub(super) fn serialize_instruction_svg_zip(
    plan: &CanonicalInstructionPlanV1,
    font: &InstructionFont<'_>,
    max_page_bytes: usize,
    max_output_bytes: usize,
) -> Result<Vec<u8>, InstructionExportError> {
    plan.validate(font)?;
    serialize_instruction_svg_zip_pages(
        &plan.title,
        &plan.pages,
        &plan.warnings,
        font,
        max_page_bytes,
        max_output_bytes,
    )
}

fn serialize_instruction_svg_zip_pages(
    title: &str,
    pages: &[InstructionPage],
    warnings: &[super::InstructionExportWarning],
    font: &InstructionFont<'_>,
    max_page_bytes: usize,
    max_output_bytes: usize,
) -> Result<Vec<u8>, InstructionExportError> {
    if pages.is_empty() {
        return Err(InstructionExportError::StructureNotRepresentable);
    }

    let mut manifest_pages = Vec::with_capacity(pages.len());
    for (index, page) in pages.iter().enumerate() {
        let page_number = index + 1;
        let file = format!("pages/page-{page_number:04}.svg");
        manifest_pages.push(SvgPageEntry {
            page_number,
            step_number: page.step_number,
            kind: if page.continuation_number == 0 {
                "step_start"
            } else {
                "continuation"
            },
            continuation_number: page.continuation_number,
            file,
        });
    }

    let manifest = SvgPageManifest {
        schema: "origami2.instruction-svg-pages.v1",
        generator: "ORIGAMI2",
        profile: super::INSTRUCTION_EXPORT_PROFILE,
        projection_profile: super::INSTRUCTION_PROJECTION_PROFILE,
        format: "svg_page_zip",
        title,
        page_count: pages.len(),
        step_count: pages
            .iter()
            .map(|page| page.step_number)
            .max()
            .ok_or(InstructionExportError::StructureNotRepresentable)?,
        page_size: "A4 portrait (210 mm x 297 mm)",
        pages: manifest_pages,
        font: SvgFontEntry {
            family: "Noto Sans JP",
            path: FONT_PATH,
            license_path: FONT_LICENSE_PATH,
            sha256: NOTO_SANS_JP_SHA256,
            license_sha256: NOTO_SANS_JP_LICENSE_SHA256,
        },
        warnings: warnings
            .iter()
            .copied()
            .map(|warning| SvgWarningEntry {
                category: warning.category(),
                message_ja: warning.message_ja(),
            })
            .collect(),
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;

    let cursor = Cursor::new(Vec::new());
    let mut archive = ZipWriter::new(cursor);
    let deflated = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(6))
        .last_modified_time(DateTime::DEFAULT)
        .unix_permissions(0o644);
    let stored = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(DateTime::DEFAULT)
        .unix_permissions(0o644);

    write_entry(
        &mut archive,
        MANIFEST_PATH,
        &manifest_bytes,
        deflated,
        max_output_bytes,
    )?;
    for (index, page) in pages.iter().enumerate() {
        let page_number = index + 1;
        let path = format!("pages/page-{page_number:04}.svg");
        let bytes = serialize_svg_page(title, page, page_number, pages.len(), font)?;
        if bytes.len() > max_page_bytes {
            return Err(InstructionExportError::PageTooLarge {
                maximum: max_page_bytes,
            });
        }
        write_entry(&mut archive, &path, &bytes, deflated, max_output_bytes)?;
    }
    write_entry(
        &mut archive,
        FONT_PATH,
        NOTO_SANS_JP_BYTES,
        stored,
        max_output_bytes,
    )?;
    write_entry(
        &mut archive,
        FONT_LICENSE_PATH,
        NOTO_SANS_JP_LICENSE,
        deflated,
        max_output_bytes,
    )?;
    let bytes = archive.finish()?.into_inner();
    if bytes.len() > max_output_bytes {
        return Err(InstructionExportError::OutputTooLarge {
            actual: bytes.len(),
            maximum: max_output_bytes,
        });
    }
    Ok(bytes)
}

fn write_entry(
    archive: &mut ZipWriter<Cursor<Vec<u8>>>,
    path: &str,
    bytes: &[u8],
    options: SimpleFileOptions,
    maximum: usize,
) -> Result<(), InstructionExportError> {
    archive.start_file(path, options)?;
    archive.write_all(bytes)?;
    archive.flush()?;
    let actual = archive
        .get_ref()
        .and_then(|cursor| usize::try_from(cursor.position()).ok())
        .unwrap_or(usize::MAX);
    if actual > maximum {
        return Err(InstructionExportError::OutputTooLarge { actual, maximum });
    }
    Ok(())
}

fn serialize_svg_page(
    title: &str,
    page: &InstructionPage,
    page_number: usize,
    page_count: usize,
    font: &InstructionFont<'_>,
) -> Result<Vec<u8>, InstructionExportError> {
    if !page.has_white_page_background() {
        return Err(InstructionExportError::StructureNotRepresentable);
    }
    let width = number(PAGE_WIDTH_POINTS)?;
    let height = number(PAGE_HEIGHT_POINTS)?;
    let mut output = String::with_capacity(
        1_024_usize
            .saturating_add(page.polygons.len().saturating_mul(200))
            .saturating_add(page.lines.len().saturating_mul(180))
            .saturating_add(
                page.texts
                    .iter()
                    .map(|text| text.glyphs.len().saturating_mul(160))
                    .sum::<usize>(),
            ),
    );
    writeln!(output, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>")
        .map_err(|_| InstructionExportError::StructureNotRepresentable)?;
    writeln!(
        output,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"210mm\" height=\"297mm\" viewBox=\"0 0 {width} {height}\" role=\"img\">"
    )
    .map_err(|_| InstructionExportError::StructureNotRepresentable)?;
    writeln!(
        output,
        "<title>{} — {page_number} / {page_count}</title>",
        xml_escape(title)
    )
    .map_err(|_| InstructionExportError::StructureNotRepresentable)?;
    output.push_str(
        "<defs><style><![CDATA[@font-face{font-family:'Noto Sans JP';src:url('../fonts/NotoSansJP-Variable.ttf') format('truetype');font-weight:100 900;font-style:normal}]]></style></defs>\n",
    );
    output.push_str("<g id=\"page\">\n");

    for polygon in &page.polygons {
        if polygon.points.len() < 3 {
            return Err(InstructionExportError::StructureNotRepresentable);
        }
        let mut points = String::new();
        for (index, point) in polygon.points.iter().enumerate() {
            validate_point(point.x, point.y)?;
            if index != 0 {
                points.push(' ');
            }
            write!(points, "{},{}", number(point.x)?, number(point.y)?)
                .map_err(|_| InstructionExportError::StructureNotRepresentable)?;
        }
        writeln!(
            output,
            "<polygon points=\"{points}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\" stroke-linejoin=\"round\"/>",
            color(polygon.fill),
            color(polygon.stroke),
            number(polygon.stroke_width)?
        )
        .map_err(|_| InstructionExportError::StructureNotRepresentable)?;
    }

    for line in &page.lines {
        validate_point(line.start.x, line.start.y)?;
        validate_point(line.end.x, line.end.y)?;
        let dash = match line.dash {
            PageLineDash::Solid => "",
            PageLineDash::Dashed => " stroke-dasharray=\"5.669291 2.834646\"",
            PageLineDash::DashDot => " stroke-dasharray=\"11.338583 2.834646 2.834646 2.834646\"",
        };
        writeln!(
            output,
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"{}\"{dash} stroke-linecap=\"round\"/>",
            number(line.start.x)?,
            number(line.start.y)?,
            number(line.end.x)?,
            number(line.end.y)?,
            color(line.color),
            number(line.width)?
        )
        .map_err(|_| InstructionExportError::StructureNotRepresentable)?;
    }

    for text in &page.texts {
        text.validate()?;
        writeln!(
            output,
            "<g role=\"text\" data-text-run=\"1\" data-baseline-y=\"{}\" data-font-size=\"{}\" fill=\"{}\" aria-label=\"{}\">",
            number(text.baseline_y)?,
            number(text.font_size)?,
            color(text.color),
            xml_escape(&text.scalar_text())
        )
        .map_err(|_| InstructionExportError::StructureNotRepresentable)?;
        let scale = text.font_size / font.units_per_em();
        if !scale.is_finite() || scale <= 0.0 {
            return Err(InstructionExportError::StructureNotRepresentable);
        }
        for glyph in &text.glyphs {
            let outline =
                font.glyph_outline(glyph.glyph_id, glyph.x, text.baseline_y, scale, -scale)?;
            if !outline.outlined && !glyph.scalar.is_whitespace() {
                return Err(InstructionExportError::StructureNotRepresentable);
            }
            writeln!(
                output,
                "<g data-glyph=\"1\" data-x=\"{}\" data-scalar=\"U+{:04X}\" data-glyph-id=\"{}\" data-advance=\"{}\">",
                number(glyph.x)?,
                u32::from(glyph.scalar),
                glyph.glyph_id,
                number(glyph.advance)?
            )
            .map_err(|_| InstructionExportError::StructureNotRepresentable)?;
            if !outline.commands.is_empty() {
                writeln!(
                    output,
                    "<path d=\"{}\"/>",
                    svg_path_data(&outline.commands)?
                )
                .map_err(|_| InstructionExportError::StructureNotRepresentable)?;
            }
            output.push_str("</g>\n");
        }
        output.push_str("</g>\n");
    }
    output.push_str("</g>\n</svg>\n");
    Ok(output.into_bytes())
}

fn svg_path_data(commands: &[GlyphPathCommand]) -> Result<String, InstructionExportError> {
    let mut output = String::with_capacity(commands.len().saturating_mul(48));
    for command in commands {
        if !output.is_empty() {
            output.push(' ');
        }
        match *command {
            GlyphPathCommand::Move { x, y } => {
                write!(output, "M {} {}", number(x)?, number(y)?)
            }
            GlyphPathCommand::Line { x, y } => {
                write!(output, "L {} {}", number(x)?, number(y)?)
            }
            GlyphPathCommand::Cubic {
                control_1_x,
                control_1_y,
                control_2_x,
                control_2_y,
                x,
                y,
            } => write!(
                output,
                "C {} {} {} {} {} {}",
                number(control_1_x)?,
                number(control_1_y)?,
                number(control_2_x)?,
                number(control_2_y)?,
                number(x)?,
                number(y)?
            ),
            GlyphPathCommand::Close => {
                output.push('Z');
                Ok(())
            }
        }
        .map_err(|_| InstructionExportError::StructureNotRepresentable)?;
    }
    Ok(output)
}

fn validate_point(x: f64, y: f64) -> Result<(), InstructionExportError> {
    if x.is_finite()
        && y.is_finite()
        && (0.0..=PAGE_WIDTH_POINTS).contains(&x)
        && (0.0..=PAGE_HEIGHT_POINTS).contains(&y)
    {
        Ok(())
    } else {
        Err(InstructionExportError::StructureNotRepresentable)
    }
}

fn number(value: f64) -> Result<String, InstructionExportError> {
    if !value.is_finite() {
        return Err(InstructionExportError::StructureNotRepresentable);
    }
    let canonical = if value == 0.0 { 0.0 } else { value };
    let mut rendered = format!("{canonical:.6}");
    while rendered.contains('.') && rendered.ends_with('0') {
        rendered.pop();
    }
    if rendered.ends_with('.') {
        rendered.pop();
    }
    if rendered.len() > MAX_NUMBER_CHARS
        || rendered.contains(['e', 'E'])
        || !valid_number(&rendered)
    {
        return Err(InstructionExportError::StructureNotRepresentable);
    }
    Ok(rendered)
}

fn valid_number(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.iter().any(u8::is_ascii_digit)
        && bytes.iter().enumerate().all(|(index, byte)| {
            byte.is_ascii_digit() || *byte == b'.' || (*byte == b'-' && index == 0)
        })
        && bytes.iter().filter(|byte| **byte == b'.').count() <= 1
}

fn color(value: PageColor) -> String {
    format!("#{:02x}{:02x}{:02x}", value.red, value.green, value.blue)
}

fn xml_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use zip::ZipArchive;

    use super::*;
    use crate::instruction_export::{
        font::InstructionFont,
        layout::{PageLine, PagePoint, PagePolygon, PageText},
    };

    fn sample_page(font: &InstructionFont<'_>) -> InstructionPage {
        InstructionPage {
            step_number: 3,
            continuation_number: 0,
            polygons: vec![PagePolygon {
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
            }],
            lines: vec![PageLine {
                start: PagePoint { x: 1.0, y: 2.0 },
                end: PagePoint { x: 3.0, y: 4.0 },
                color: PageColor {
                    red: 1,
                    green: 2,
                    blue: 3,
                },
                width: 1.25,
                dash: PageLineDash::Dashed,
            }],
            texts: PageText::from_text(
                20.0,
                30.0,
                10.0,
                PageColor {
                    red: 32,
                    green: 35,
                    blue: 42,
                },
                "折る & <確認>",
                font,
            )
            .expect("layout sample text")
            .into_iter()
            .collect(),
        }
    }

    #[test]
    fn archive_is_deterministic_closed_and_well_formed() {
        let font = InstructionFont::load().expect("bundled font");
        let page = sample_page(&font);
        let first = serialize_instruction_svg_zip_pages(
            "作品 & 一",
            std::slice::from_ref(&page),
            &crate::instruction_export::INSTRUCTION_EXPORT_WARNINGS,
            &font,
            64 * 1024,
            usize::MAX,
        )
        .expect("serialize archive");
        let second = serialize_instruction_svg_zip_pages(
            "作品 & 一",
            &[page],
            &crate::instruction_export::INSTRUCTION_EXPORT_WARNINGS,
            &font,
            64 * 1024,
            usize::MAX,
        )
        .expect("serialize archive again");
        assert_eq!(first, second);

        let mut archive = ZipArchive::new(Cursor::new(first)).expect("open ZIP");
        let entries = (0..archive.len())
            .map(|index| {
                let entry = archive.by_index(index).expect("entry");
                (entry.name().to_owned(), entry.last_modified())
            })
            .collect::<Vec<_>>();
        let names = entries
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            [
                MANIFEST_PATH,
                "pages/page-0001.svg",
                FONT_PATH,
                FONT_LICENSE_PATH
            ]
        );
        assert!(
            entries
                .iter()
                .all(|(_, modified)| *modified == Some(DateTime::DEFAULT))
        );
        let mut manifest = String::new();
        archive
            .by_name(MANIFEST_PATH)
            .expect("manifest")
            .read_to_string(&mut manifest)
            .expect("read manifest");
        let manifest: serde_json::Value = serde_json::from_str(&manifest).expect("parse manifest");
        assert_eq!(manifest["schema"], "origami2.instruction-svg-pages.v1");
        assert_eq!(manifest["profile"], "instruction_export_v1");
        assert_eq!(manifest["projection_profile"], "orthographic_isometric_v1");
        assert_eq!(manifest["font"]["sha256"], NOTO_SANS_JP_SHA256);
        assert_eq!(
            manifest["font"]["license_sha256"],
            NOTO_SANS_JP_LICENSE_SHA256
        );
        assert_eq!(
            manifest["warnings"]
                .as_array()
                .expect("warning array")
                .iter()
                .map(|warning| warning["category"].as_str().expect("category"))
                .collect::<Vec<_>>(),
            crate::instruction_export::INSTRUCTION_EXPORT_WARNINGS
                .iter()
                .copied()
                .map(crate::instruction_export::InstructionExportWarning::category)
                .collect::<Vec<_>>()
        );
        let mut svg = String::new();
        archive
            .by_name("pages/page-0001.svg")
            .expect("SVG")
            .read_to_string(&mut svg)
            .expect("read SVG");
        assert!(svg.contains("作品 &amp; 一"));
        assert!(svg.contains("折る &amp; &lt;確認&gt;"));
        assert!(svg.contains("../fonts/NotoSansJP-Variable.ttf"));
        assert!(svg.contains("<g data-glyph=\"1\" data-x=\"20\" data-scalar=\"U+6298\""));
        assert!(svg.contains("data-glyph-id="));
        assert!(svg.contains("data-advance="));
        assert!(svg.contains("<path d=\"M "));
        assert!(!svg.contains("<text"));
        assert!(!svg.contains("<tspan"));
        assert!(svg.contains(
            "<g id=\"page\">\n<polygon points=\"0,0 595.275591,0 595.275591,841.889764 0,841.889764\" fill=\"#ffffff\""
        ));
        assert!(!svg.to_ascii_lowercase().contains("<script"));

        let mut font = Vec::new();
        archive
            .by_name(FONT_PATH)
            .expect("font")
            .read_to_end(&mut font)
            .expect("read font");
        assert_eq!(font, NOTO_SANS_JP_BYTES);
    }

    #[test]
    fn page_limit_and_invalid_coordinates_reject_the_complete_archive() {
        let font = InstructionFont::load().expect("bundled font");
        let page = sample_page(&font);
        let page_bytes =
            serialize_svg_page("作品", &page, 1, 1, &font).expect("standalone SVG page");
        serialize_instruction_svg_zip_pages(
            "作品",
            std::slice::from_ref(&page),
            &crate::instruction_export::INSTRUCTION_EXPORT_WARNINGS,
            &font,
            page_bytes.len(),
            usize::MAX,
        )
        .expect("page payload equal to its limit");
        assert!(matches!(
            serialize_instruction_svg_zip_pages(
                "作品",
                std::slice::from_ref(&page),
                &crate::instruction_export::INSTRUCTION_EXPORT_WARNINGS,
                &font,
                page_bytes.len() - 1,
                usize::MAX,
            ),
            Err(InstructionExportError::PageTooLarge { maximum })
                if maximum == page_bytes.len() - 1
        ));
        let complete = serialize_instruction_svg_zip_pages(
            "作品",
            std::slice::from_ref(&page),
            &crate::instruction_export::INSTRUCTION_EXPORT_WARNINGS,
            &font,
            usize::MAX,
            usize::MAX,
        )
        .expect("complete ZIP");
        serialize_instruction_svg_zip_pages(
            "作品",
            std::slice::from_ref(&page),
            &crate::instruction_export::INSTRUCTION_EXPORT_WARNINGS,
            &font,
            usize::MAX,
            complete.len(),
        )
        .expect("archive equal to its output limit");
        assert!(matches!(
            serialize_instruction_svg_zip_pages(
                "作品",
                std::slice::from_ref(&page),
                &crate::instruction_export::INSTRUCTION_EXPORT_WARNINGS,
                &font,
                usize::MAX,
                complete.len() - 1,
            ),
            Err(InstructionExportError::OutputTooLarge { maximum, .. })
                if maximum == complete.len() - 1
        ));
        let mut invalid = page;
        invalid.lines[0].start.x = f64::NAN;
        assert!(matches!(
            serialize_instruction_svg_zip_pages(
                "作品",
                &[invalid],
                &crate::instruction_export::INSTRUCTION_EXPORT_WARNINGS,
                &font,
                64 * 1024,
                usize::MAX,
            ),
            Err(InstructionExportError::StructureNotRepresentable)
        ));
        assert!(matches!(
            serialize_instruction_svg_zip_pages(
                "作品",
                &[sample_page(&font)],
                &crate::instruction_export::INSTRUCTION_EXPORT_WARNINGS,
                &font,
                64 * 1024,
                64,
            ),
            Err(InstructionExportError::OutputTooLarge { maximum: 64, .. })
        ));
    }
}
