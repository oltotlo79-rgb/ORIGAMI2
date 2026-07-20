use ori_domain::{InstructionStep, InstructionTimeline};
use ori_instructions::{
    DiagramBounds, DiagramColor, DiagramPoint, InstructionDiagramFoldKind, InstructionDiagramPlan,
    InstructionDiagramStep,
};

use super::{InstructionExportError, InstructionExportLimits, font::InstructionFont};

pub(super) const PAGE_WIDTH_POINTS: f64 = 595.275_590_551_181_2;
pub(super) const PAGE_HEIGHT_POINTS: f64 = 841.889_763_779_527_7;

const PAGE_MARGIN: f64 = 36.0;
const CONTENT_WIDTH: f64 = PAGE_WIDTH_POINTS - PAGE_MARGIN * 2.0;
const CONTENT_BOTTOM: f64 = 795.0;
const FOOTER_BASELINE: f64 = 821.0;
const BODY_FONT_SIZE: f64 = 10.5;
const BODY_LINE_HEIGHT: f64 = 15.5;
const DIAGRAM_HEIGHT: f64 = 360.0;
const DIAGRAM_PADDING: f64 = 18.0;
const DIAGRAM_LEGEND_HEIGHT: f64 = 26.0;
const TEXT_METRIC_SCALE: f64 = 1_000_000.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PagePoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PageColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl PageColor {
    pub(super) const WHITE: Self = Self::rgb(255, 255, 255);
    const BLACK: Self = Self::rgb(32, 33, 36);
    const MUTED: Self = Self::rgb(92, 99, 112);
    const BORDER: Self = Self::rgb(174, 181, 194);
    const MOUNTAIN: Self = Self::rgb(217, 48, 37);
    const VALLEY: Self = Self::rgb(26, 115, 232);
    const CHANGED_HALO: Self = Self::rgb(249, 171, 0);

    const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }
}

impl From<DiagramColor> for PageColor {
    fn from(value: DiagramColor) -> Self {
        Self::rgb(value.red, value.green, value.blue)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PageLineDash {
    Solid,
    Dashed,
    DashDot,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PagePolygon {
    pub points: Vec<PagePoint>,
    pub fill: PageColor,
    pub stroke: PageColor,
    pub stroke_width: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PageLine {
    pub start: PagePoint,
    pub end: PagePoint,
    pub color: PageColor,
    pub width: f64,
    pub dash: PageLineDash,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PageGlyph {
    pub scalar: char,
    pub glyph_id: u16,
    pub x: f64,
    pub advance: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PageText {
    pub baseline_y: f64,
    pub font_size: f64,
    pub color: PageColor,
    pub glyphs: Vec<PageGlyph>,
}

impl PageText {
    pub(super) fn from_text(
        x: f64,
        baseline_y: f64,
        font_size: f64,
        color: PageColor,
        text: &str,
        font: &InstructionFont<'_>,
    ) -> Result<Option<Self>, InstructionExportError> {
        if ![x, baseline_y, font_size].into_iter().all(f64::is_finite)
            || font_size <= 0.0
            || text.contains(['\n', '\r', '\t'])
        {
            return Err(InstructionExportError::StructureNotRepresentable);
        }
        if text.is_empty() {
            return Ok(None);
        }

        let mut glyphs = Vec::with_capacity(text.chars().count());
        let mut cursor_x = fixed_plan_metric(x)?;
        for scalar in text.chars() {
            let glyph_id = font.glyph_id(scalar)?.0;
            let advance = fixed_plan_metric(font.glyph_advance(scalar, font_size)?)?;
            glyphs.push(PageGlyph {
                scalar,
                glyph_id,
                x: cursor_x,
                advance,
            });
            cursor_x = fixed_plan_metric(cursor_x + advance)?;
        }
        let text = Self {
            baseline_y,
            font_size,
            color,
            glyphs,
        };
        text.validate()?;
        Ok(Some(text))
    }

    pub(super) fn validate(&self) -> Result<(), InstructionExportError> {
        if ![self.baseline_y, self.font_size]
            .into_iter()
            .all(f64::is_finite)
            || self.font_size <= 0.0
            || self.glyphs.is_empty()
        {
            return Err(InstructionExportError::StructureNotRepresentable);
        }
        for (index, glyph) in self.glyphs.iter().enumerate() {
            if glyph.scalar.is_control()
                || matches!(glyph.scalar, '\u{2028}' | '\u{2029}')
                || !glyph.x.is_finite()
                || !glyph.advance.is_finite()
                || glyph.advance < 0.0
                || !(0.0..=PAGE_WIDTH_POINTS).contains(&glyph.x)
                || glyph.x + glyph.advance > PAGE_WIDTH_POINTS + 0.000_001
            {
                return Err(InstructionExportError::StructureNotRepresentable);
            }
            if let Some(next) = self.glyphs.get(index + 1)
                && next.x != fixed_plan_metric(glyph.x + glyph.advance)?
            {
                return Err(InstructionExportError::StructureNotRepresentable);
            }
        }
        if !(0.0..=PAGE_HEIGHT_POINTS).contains(&self.baseline_y) {
            return Err(InstructionExportError::StructureNotRepresentable);
        }
        Ok(())
    }

    pub(super) fn scalar_text(&self) -> String {
        self.glyphs.iter().map(|glyph| glyph.scalar).collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct InstructionPage {
    pub step_number: usize,
    pub continuation_number: usize,
    pub polygons: Vec<PagePolygon>,
    pub lines: Vec<PageLine>,
    pub texts: Vec<PageText>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct InstructionLayout {
    pub pages: Vec<InstructionPage>,
    pub glyph_count: usize,
}

impl InstructionPage {
    fn new(step_number: usize, continuation_number: usize) -> Self {
        Self {
            step_number,
            continuation_number,
            polygons: vec![page_background()],
            lines: Vec::new(),
            texts: Vec::new(),
        }
    }

    pub(super) fn has_white_page_background(&self) -> bool {
        let Some(background) = self.polygons.first() else {
            return false;
        };
        background.fill == PageColor::WHITE
            && background.points
                == [
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
                ]
    }
}

struct GlyphBudget {
    used: usize,
    maximum: usize,
}

impl GlyphBudget {
    fn claim(&mut self, text: &str) -> Result<(), InstructionExportError> {
        let count = text.chars().count();
        self.used = self
            .used
            .checked_add(count)
            .ok_or(InstructionExportError::LayoutLimitExceeded)?;
        if self.used > self.maximum {
            return Err(InstructionExportError::LayoutLimitExceeded);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlowSection {
    Description,
    Caution,
}

impl FlowSection {
    const fn label(self) -> &'static str {
        match self {
            Self::Description => "説明",
            Self::Caution => "注意事項",
        }
    }
}

struct FlowLine {
    text: String,
    size: f64,
    line_height: f64,
    color: PageColor,
    gap_before: f64,
    section: FlowSection,
    is_heading: bool,
}

fn page_background() -> PagePolygon {
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
    }
}

fn fixed_plan_metric(value: f64) -> Result<f64, InstructionExportError> {
    if !value.is_finite() {
        return Err(InstructionExportError::StructureNotRepresentable);
    }
    let scaled = value * TEXT_METRIC_SCALE;
    if !scaled.is_finite() {
        return Err(InstructionExportError::StructureNotRepresentable);
    }
    let fixed = scaled.round() / TEXT_METRIC_SCALE;
    Ok(if fixed == 0.0 { 0.0 } else { fixed })
}

pub(super) fn layout_instruction_pages(
    title: &str,
    timeline: &InstructionTimeline,
    diagram: &InstructionDiagramPlan,
    font: &InstructionFont<'_>,
    limits: InstructionExportLimits,
) -> Result<InstructionLayout, InstructionExportError> {
    if timeline.steps.len() != diagram.steps.len()
        || timeline.steps.is_empty()
        || limits.max_pages == 0
    {
        return Err(InstructionExportError::StructureNotRepresentable);
    }

    if title.is_empty() {
        return Err(InstructionExportError::StructureNotRepresentable);
    }
    let step_count = timeline.steps.len();
    let mut glyphs = GlyphBudget {
        used: 0,
        maximum: limits.max_glyphs,
    };
    let mut pages = Vec::new();

    for (step_index, (step, diagram_step)) in timeline.steps.iter().zip(&diagram.steps).enumerate()
    {
        let step_number = step_index + 1;
        let (mut page, mut cursor) = first_step_page(
            title,
            step,
            diagram_step,
            diagram.bounds,
            step_number,
            step_count,
            font,
            &mut glyphs,
        )?;
        let flow = body_flow(step, font)?;
        let mut continuation_number = 0;

        for (flow_index, item) in flow.iter().enumerate() {
            let mut required = item.gap_before + item.line_height;
            if item.is_heading
                && let Some(next) = flow.get(flow_index + 1)
                && next.section == item.section
            {
                required += next.gap_before + next.line_height;
            }
            let mut heading_replaced_by_continuation = false;
            if cursor + required > CONTENT_BOTTOM {
                push_page(&mut pages, page, limits.max_pages)?;
                continuation_number += 1;
                let continuation = continuation_page(
                    title,
                    step,
                    step_number,
                    step_count,
                    continuation_number,
                    item.section,
                    font,
                    &mut glyphs,
                )?;
                page = continuation.0;
                cursor = continuation.1;
                heading_replaced_by_continuation = item.is_heading;
            }
            if heading_replaced_by_continuation {
                continue;
            }
            cursor += item.gap_before;
            let baseline = cursor + item.size;
            add_text(
                &mut page,
                PAGE_MARGIN,
                baseline,
                item.size,
                item.color,
                item.text.clone(),
                font,
                &mut glyphs,
            )?;
            cursor += item.line_height;
        }
        push_page(&mut pages, page, limits.max_pages)?;
    }

    let total_pages = pages.len();
    for (index, page) in pages.iter_mut().enumerate() {
        add_text(
            page,
            PAGE_MARGIN,
            FOOTER_BASELINE,
            7.5,
            PageColor::MUTED,
            "固定等角投影 / 折り順・干渉・層順は作家が確認してください".to_owned(),
            font,
            &mut glyphs,
        )?;
        let page_number = format!("{} / {}", index + 1, total_pages);
        let width = fixed_text_width(&page_number, 8.0, font)?;
        add_text(
            page,
            PAGE_WIDTH_POINTS - PAGE_MARGIN - width,
            FOOTER_BASELINE,
            8.0,
            PageColor::MUTED,
            page_number,
            font,
            &mut glyphs,
        )?;
    }

    Ok(InstructionLayout {
        pages,
        glyph_count: glyphs.used,
    })
}

#[allow(clippy::too_many_arguments)]
fn first_step_page(
    project_title: &str,
    step: &InstructionStep,
    diagram_step: &InstructionDiagramStep,
    bounds: DiagramBounds,
    step_number: usize,
    step_count: usize,
    font: &InstructionFont<'_>,
    glyphs: &mut GlyphBudget,
) -> Result<(InstructionPage, f64), InstructionExportError> {
    let mut page = InstructionPage::new(step_number, 0);
    let mut cursor = PAGE_MARGIN;
    cursor = add_wrapped_header(
        &mut page,
        project_title,
        9.0,
        12.0,
        cursor,
        PageColor::MUTED,
        font,
        glyphs,
    )?;
    cursor += 7.0;
    let step_counter = format!("手順 {step_number} / {step_count}");
    cursor = add_wrapped_header(
        &mut page,
        &step_counter,
        13.0,
        18.0,
        cursor,
        PageColor::BLACK,
        font,
        glyphs,
    )?;
    cursor += 3.0;
    cursor = add_wrapped_header(
        &mut page,
        &step.title,
        17.0,
        22.0,
        cursor,
        PageColor::BLACK,
        font,
        glyphs,
    )?;
    cursor += 8.0;

    if diagram_step.declarative_only {
        draw_declarative_placeholder(&mut page, cursor, font, glyphs)?;
        cursor += DIAGRAM_HEIGHT + 10.0;
        add_text(
            &mut page,
            PAGE_MARGIN,
            cursor + 9.0,
            9.0,
            PageColor::MUTED,
            format!(
                "所要時間: {} / 説明専用・3D姿勢なし",
                format_duration(step.duration_ms)
            ),
            font,
            glyphs,
        )?;
        cursor += 20.0;
        cursor = add_hand_guide_summary(&mut page, step, cursor, font, glyphs)?;
        cursor = add_direction_and_focus_summary(&mut page, step, cursor, font, glyphs)?;
        return Ok((page, cursor));
    }

    draw_diagram(&mut page, diagram_step, bounds, cursor)?;
    let legend_baseline = cursor + DIAGRAM_HEIGHT - 9.0;
    for (x, label, color) in [
        (96.0, "山折り", PageColor::MOUNTAIN),
        (236.0, "谷折り", PageColor::VALLEY),
        (330.0, "太線 = 今回変化", PageColor::MUTED),
    ] {
        add_text(
            &mut page,
            x,
            legend_baseline,
            8.0,
            color,
            label.to_owned(),
            font,
            glyphs,
        )?;
    }
    cursor += DIAGRAM_HEIGHT + 10.0;
    let metadata = format!(
        "所要時間: {} / 今回動かす折り線: {}本",
        format_duration(step.duration_ms),
        diagram_step.changed_hinge_count
    );
    add_text(
        &mut page,
        PAGE_MARGIN,
        cursor + 9.0,
        9.0,
        PageColor::MUTED,
        metadata,
        font,
        glyphs,
    )?;
    cursor += 20.0;
    cursor = add_hand_guide_summary(&mut page, step, cursor, font, glyphs)?;
    cursor = add_direction_and_focus_summary(&mut page, step, cursor, font, glyphs)?;
    Ok((page, cursor))
}

fn add_hand_guide_summary(
    page: &mut InstructionPage,
    step: &ori_domain::InstructionStep,
    cursor: f64,
    font: &InstructionFont<'_>,
    glyphs: &mut GlyphBudget,
) -> Result<f64, InstructionExportError> {
    if step.visual.hand_guides.is_empty() {
        return Ok(cursor);
    }
    let labels = step
        .visual
        .hand_guides
        .iter()
        .map(|guide| {
            let kind = match guide.kind {
                ori_domain::InstructionHandGuideKind::Pinch => "pinch",
                ori_domain::InstructionHandGuideKind::Hold => "hold",
                ori_domain::InstructionHandGuideKind::Push => "push",
                ori_domain::InstructionHandGuideKind::Regrip => "regrip",
            };
            if guide.label.is_empty() {
                kind.to_owned()
            } else {
                format!(
                    "{kind}: {} @ ({:.2}, {:.2}, {:.2}) -> ({:.2}, {:.2}, {:.2})",
                    guide.label,
                    guide.position.x,
                    guide.position.y,
                    guide.position.z,
                    guide.direction.x,
                    guide.direction.y,
                    guide.direction.z,
                )
            }
        })
        .collect::<Vec<_>>()
        .join(" / ");
    add_wrapped_header(
        page,
        &format!("Hand guides: {labels}"),
        8.0,
        11.0,
        cursor,
        PageColor::MUTED,
        font,
        glyphs,
    )
}

fn add_direction_and_focus_summary(
    page: &mut InstructionPage,
    step: &ori_domain::InstructionStep,
    mut cursor: f64,
    font: &InstructionFont<'_>,
    glyphs: &mut GlyphBudget,
) -> Result<f64, InstructionExportError> {
    if !step.visual.arrows.is_empty() {
        let arrows = step
            .visual
            .arrows
            .iter()
            .map(|arrow| {
                format!(
                    "{}: ({:.2}, {:.2}, {:.2}) -> ({:.2}, {:.2}, {:.2})",
                    arrow.label,
                    arrow.start.x,
                    arrow.start.y,
                    arrow.start.z,
                    arrow.end.x,
                    arrow.end.y,
                    arrow.end.z,
                )
            })
            .collect::<Vec<_>>()
            .join(" / ");
        cursor = add_wrapped_header(
            page,
            &format!("Fold directions: {arrows}"),
            8.0,
            11.0,
            cursor,
            PageColor::MUTED,
            font,
            glyphs,
        )?;
    }
    if !step.visual.focus_points.is_empty() {
        let points = step
            .visual
            .focus_points
            .iter()
            .map(|focus| {
                format!(
                    "{}: ({:.2}, {:.2}, {:.2}), r={:.2}",
                    focus.label, focus.position.x, focus.position.y, focus.position.z, focus.radius,
                )
            })
            .collect::<Vec<_>>()
            .join(" / ");
        cursor = add_wrapped_header(
            page,
            &format!("Focus points: {points}"),
            8.0,
            11.0,
            cursor,
            PageColor::MUTED,
            font,
            glyphs,
        )?;
    }
    Ok(cursor)
}

fn draw_declarative_placeholder(
    page: &mut InstructionPage,
    top: f64,
    font: &InstructionFont<'_>,
    glyphs: &mut GlyphBudget,
) -> Result<(), InstructionExportError> {
    let left = PAGE_MARGIN;
    let right = PAGE_WIDTH_POINTS - PAGE_MARGIN;
    let bottom = top + DIAGRAM_HEIGHT;
    page.polygons.push(PagePolygon {
        points: vec![
            PagePoint { x: left, y: top },
            PagePoint { x: right, y: top },
            PagePoint {
                x: right,
                y: bottom,
            },
            PagePoint { x: left, y: bottom },
        ],
        fill: PageColor::WHITE,
        stroke: PageColor::BORDER,
        stroke_width: 0.7,
    });
    add_text(
        page,
        left + 24.0,
        top + DIAGRAM_HEIGHT / 2.0,
        13.0,
        PageColor::MUTED,
        "説明専用ステップ（3D姿勢・物理操作なし）".to_owned(),
        font,
        glyphs,
    )
}

#[allow(clippy::too_many_arguments)]
fn continuation_page(
    project_title: &str,
    step: &InstructionStep,
    step_number: usize,
    step_count: usize,
    continuation_number: usize,
    section: FlowSection,
    font: &InstructionFont<'_>,
    glyphs: &mut GlyphBudget,
) -> Result<(InstructionPage, f64), InstructionExportError> {
    let mut page = InstructionPage::new(step_number, continuation_number);
    let mut cursor = PAGE_MARGIN;
    cursor = add_wrapped_header(
        &mut page,
        project_title,
        9.0,
        12.0,
        cursor,
        PageColor::MUTED,
        font,
        glyphs,
    )?;
    cursor += 7.0;
    let heading = format!("手順 {step_number} / {step_count}（続き {continuation_number}）");
    cursor = add_wrapped_header(
        &mut page,
        &heading,
        14.0,
        19.0,
        cursor,
        PageColor::BLACK,
        font,
        glyphs,
    )?;
    cursor += 3.0;
    cursor = add_wrapped_header(
        &mut page,
        &step.title,
        11.5,
        16.0,
        cursor,
        PageColor::BLACK,
        font,
        glyphs,
    )?;
    cursor += 3.0;
    let section_heading = format!("継続セクション: {}", section.label());
    cursor = add_wrapped_header(
        &mut page,
        &section_heading,
        10.5,
        15.0,
        cursor,
        if section == FlowSection::Caution {
            PageColor::MOUNTAIN
        } else {
            PageColor::BLACK
        },
        font,
        glyphs,
    )?;
    cursor += 8.0;
    page.lines.push(PageLine {
        start: PagePoint {
            x: PAGE_MARGIN,
            y: cursor,
        },
        end: PagePoint {
            x: PAGE_WIDTH_POINTS - PAGE_MARGIN,
            y: cursor,
        },
        color: PageColor::BORDER,
        width: 0.7,
        dash: PageLineDash::Solid,
    });
    Ok((page, cursor + 16.0))
}

#[allow(clippy::too_many_arguments)]
fn add_wrapped_header(
    page: &mut InstructionPage,
    text: &str,
    size: f64,
    line_height: f64,
    mut cursor: f64,
    color: PageColor,
    font: &InstructionFont<'_>,
    glyphs: &mut GlyphBudget,
) -> Result<f64, InstructionExportError> {
    for line in wrap_text(text, CONTENT_WIDTH, size, font)? {
        add_text(
            page,
            PAGE_MARGIN,
            cursor + size,
            size,
            color,
            line,
            font,
            glyphs,
        )?;
        cursor += line_height;
    }
    Ok(cursor)
}

fn body_flow(
    step: &InstructionStep,
    font: &InstructionFont<'_>,
) -> Result<Vec<FlowLine>, InstructionExportError> {
    let mut flow = Vec::new();
    append_section(
        &mut flow,
        FlowSection::Description,
        if step.description.is_empty() {
            "説明は登録されていません。"
        } else {
            &step.description
        },
        PageColor::BLACK,
        font,
    )?;
    append_section(
        &mut flow,
        FlowSection::Caution,
        if step.caution.is_empty() {
            "注意事項は登録されていません。"
        } else {
            &step.caution
        },
        PageColor::MOUNTAIN,
        font,
    )?;
    Ok(flow)
}

fn append_section(
    flow: &mut Vec<FlowLine>,
    section: FlowSection,
    body: &str,
    body_color: PageColor,
    font: &InstructionFont<'_>,
) -> Result<(), InstructionExportError> {
    flow.push(FlowLine {
        text: section.label().to_owned(),
        size: 11.5,
        line_height: 17.0,
        color: body_color,
        gap_before: if flow.is_empty() { 0.0 } else { 9.0 },
        section,
        is_heading: true,
    });
    for line in wrap_text(body, CONTENT_WIDTH, BODY_FONT_SIZE, font)? {
        flow.push(FlowLine {
            text: line,
            size: BODY_FONT_SIZE,
            line_height: BODY_LINE_HEIGHT,
            color: body_color,
            gap_before: 0.0,
            section,
            is_heading: false,
        });
    }
    Ok(())
}

fn wrap_text(
    text: &str,
    maximum_width: f64,
    font_size: f64,
    font: &InstructionFont<'_>,
) -> Result<Vec<String>, InstructionExportError> {
    if !maximum_width.is_finite() || maximum_width <= 0.0 {
        return Err(InstructionExportError::StructureNotRepresentable);
    }
    let normalized = text.replace('\t', "    ");
    let mut output = Vec::new();
    for paragraph in normalized.split('\n') {
        if paragraph.is_empty() {
            output.push(String::new());
            continue;
        }
        let mut line = String::new();
        let mut width = 0.0;
        for character in paragraph.chars() {
            let advance = fixed_plan_metric(font.glyph_advance(character, font_size)?)?;
            if advance > maximum_width {
                return Err(InstructionExportError::StructureNotRepresentable);
            }
            if !line.is_empty() && width + advance > maximum_width {
                output.push(line);
                line = String::new();
                width = 0.0;
            }
            line.push(character);
            width = fixed_plan_metric(width + advance)?;
        }
        output.push(line);
    }
    Ok(output)
}

fn fixed_text_width(
    text: &str,
    font_size: f64,
    font: &InstructionFont<'_>,
) -> Result<f64, InstructionExportError> {
    text.chars().try_fold(0.0, |width, scalar| {
        let advance = fixed_plan_metric(font.glyph_advance(scalar, font_size)?)?;
        fixed_plan_metric(width + advance)
    })
}

#[allow(clippy::too_many_arguments)]
fn add_text(
    page: &mut InstructionPage,
    x: f64,
    baseline_y: f64,
    font_size: f64,
    color: PageColor,
    text: String,
    font: &InstructionFont<'_>,
    glyphs: &mut GlyphBudget,
) -> Result<(), InstructionExportError> {
    if ![x, baseline_y, font_size].into_iter().all(f64::is_finite)
        || font_size <= 0.0
        || text.contains(['\n', '\r', '\t'])
    {
        return Err(InstructionExportError::StructureNotRepresentable);
    }
    glyphs.claim(&text)?;
    if let Some(text) = PageText::from_text(x, baseline_y, font_size, color, &text, font)? {
        page.texts.push(text);
    }
    Ok(())
}

fn draw_diagram(
    page: &mut InstructionPage,
    step: &InstructionDiagramStep,
    bounds: DiagramBounds,
    top: f64,
) -> Result<(), InstructionExportError> {
    let left = PAGE_MARGIN;
    let right = PAGE_WIDTH_POINTS - PAGE_MARGIN;
    let bottom = top + DIAGRAM_HEIGHT;
    page.polygons.push(PagePolygon {
        points: vec![
            PagePoint { x: left, y: top },
            PagePoint { x: right, y: top },
            PagePoint {
                x: right,
                y: bottom,
            },
            PagePoint { x: left, y: bottom },
        ],
        fill: PageColor::WHITE,
        stroke: PageColor::BORDER,
        stroke_width: 0.7,
    });

    let drawing_width = bounds.max_x - bounds.min_x;
    let drawing_height = bounds.max_y - bounds.min_y;
    let available_width = CONTENT_WIDTH - DIAGRAM_PADDING * 2.0;
    let available_height = DIAGRAM_HEIGHT - DIAGRAM_LEGEND_HEIGHT - DIAGRAM_PADDING * 2.0;
    let scale = (available_width / drawing_width).min(available_height / drawing_height);
    if ![drawing_width, drawing_height, scale]
        .into_iter()
        .all(f64::is_finite)
        || drawing_width <= 0.0
        || drawing_height <= 0.0
        || scale <= 0.0
    {
        return Err(InstructionExportError::StructureNotRepresentable);
    }
    let drawing_center_x = (bounds.min_x + bounds.max_x) / 2.0;
    let drawing_center_y = (bounds.min_y + bounds.max_y) / 2.0;
    let target_center_x = PAGE_WIDTH_POINTS / 2.0;
    let target_center_y = top + (DIAGRAM_HEIGHT - DIAGRAM_LEGEND_HEIGHT) / 2.0;
    let map = |point: DiagramPoint| -> Result<PagePoint, InstructionExportError> {
        Ok(PagePoint {
            x: fixed_plan_metric(target_center_x + (point.x - drawing_center_x) * scale)?,
            y: fixed_plan_metric(target_center_y - (point.y - drawing_center_y) * scale)?,
        })
    };

    for face in &step.faces {
        if face.points.len() < 3 {
            return Err(InstructionExportError::StructureNotRepresentable);
        }
        page.polygons.push(PagePolygon {
            points: face
                .points
                .iter()
                .copied()
                .map(map)
                .collect::<Result<Vec<_>, _>>()?,
            fill: face.fill.into(),
            stroke: PageColor::BLACK,
            stroke_width: 0.7,
        });
    }
    for hinge in step.hinges.iter().filter(|hinge| hinge.changed) {
        page.lines.push(PageLine {
            start: map(hinge.start)?,
            end: map(hinge.end)?,
            color: PageColor::CHANGED_HALO,
            width: 3.4,
            dash: PageLineDash::Solid,
        });
    }
    for hinge in &step.hinges {
        page.lines.push(PageLine {
            start: map(hinge.start)?,
            end: map(hinge.end)?,
            color: match hinge.kind {
                InstructionDiagramFoldKind::Mountain => PageColor::MOUNTAIN,
                InstructionDiagramFoldKind::Valley => PageColor::VALLEY,
            },
            width: 1.0,
            dash: match hinge.kind {
                InstructionDiagramFoldKind::Mountain => PageLineDash::DashDot,
                InstructionDiagramFoldKind::Valley => PageLineDash::Dashed,
            },
        });
    }

    let legend_y = bottom - 13.0;
    page.lines.extend([
        PageLine {
            start: PagePoint {
                x: left + 18.0,
                y: legend_y,
            },
            end: PagePoint {
                x: left + 55.0,
                y: legend_y,
            },
            color: PageColor::MOUNTAIN,
            width: 1.5,
            dash: PageLineDash::DashDot,
        },
        PageLine {
            start: PagePoint {
                x: left + 158.0,
                y: legend_y,
            },
            end: PagePoint {
                x: left + 195.0,
                y: legend_y,
            },
            color: PageColor::VALLEY,
            width: 1.5,
            dash: PageLineDash::Dashed,
        },
    ]);
    Ok(())
}

fn push_page(
    pages: &mut Vec<InstructionPage>,
    page: InstructionPage,
    maximum: usize,
) -> Result<(), InstructionExportError> {
    if pages.len() >= maximum {
        return Err(InstructionExportError::LayoutLimitExceeded);
    }
    pages.push(page);
    Ok(())
}

fn format_duration(duration_ms: u32) -> String {
    if duration_ms.is_multiple_of(1_000) {
        format!("{}秒", duration_ms / 1_000)
    } else {
        let seconds = f64::from(duration_ms) / 1_000.0;
        let rendered = format!("{seconds:.3}");
        format!("{}秒", rendered.trim_end_matches('0').trim_end_matches('.'))
    }
}

#[cfg(test)]
mod tests {
    use ori_domain::{
        InstructionArrow, InstructionFocusPoint, InstructionHandGuide, InstructionHandGuideKind,
        InstructionPoint3, InstructionPose, InstructionPoseModel, InstructionStep,
        InstructionStepId, InstructionTimeline,
    };

    use super::*;

    #[test]
    fn page_text_freezes_scalar_glyph_id_x_and_advance_during_layout() {
        let font = InstructionFont::load().expect("bundled font");
        let text = PageText::from_text(42.125, 80.0, 11.0, PageColor::BLACK, "AV折", &font)
            .expect("layout")
            .expect("non-empty");
        assert_eq!(text.scalar_text(), "AV折");
        assert_eq!(text.glyphs.len(), 3);
        assert_eq!(text.glyphs[0].x, 42.125);
        assert!(
            text.glyphs
                .iter()
                .all(|glyph| glyph.glyph_id != 0 && glyph.advance >= 0.0)
        );
        for pair in text.glyphs.windows(2) {
            assert_eq!(
                pair[1].x,
                fixed_plan_metric(pair[0].x + pair[0].advance).unwrap()
            );
        }
    }

    #[test]
    fn every_new_page_begins_with_an_explicit_white_a4_background() {
        let page = InstructionPage::new(1, 0);
        assert!(page.has_white_page_background());
        assert_eq!(page.polygons[0].fill, PageColor::WHITE);
    }

    #[test]
    fn duration_is_compact_and_exact_to_milliseconds() {
        assert_eq!(format_duration(1_000), "1秒");
        assert_eq!(format_duration(1_250), "1.25秒");
        assert_eq!(format_duration(100), "0.1秒");
    }

    #[test]
    fn declarative_layout_preserves_text_and_draws_no_fold_pose() {
        let mut timeline = InstructionTimeline {
            steps: vec![InstructionStep {
                id: InstructionStepId::new(),
                title: "中割り折り（説明）".to_owned(),
                description: "この説明はPDFとSVGに残ります。".to_owned(),
                caution: "自動実行せず層を確認してください。".to_owned(),
                duration_ms: 1_500,
                visual: Default::default(),
                pose: InstructionPose {
                    model: InstructionPoseModel::DeclarativeOnlyV1,
                    source_model_fingerprint: "f".repeat(64),
                    fixed_face: None,
                    hinge_angles: Vec::new(),
                },
            }],
        };
        timeline.steps[0]
            .visual
            .hand_guides
            .push(InstructionHandGuide {
                kind: InstructionHandGuideKind::Regrip,
                position: InstructionPoint3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                direction: InstructionPoint3 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                },
                label: "right hand".to_owned(),
            });
        timeline.steps[0].visual.arrows.push(InstructionArrow {
            start: InstructionPoint3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            end: InstructionPoint3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            label: "fold up".to_owned(),
        });
        timeline.steps[0]
            .visual
            .focus_points
            .push(InstructionFocusPoint {
                position: InstructionPoint3 {
                    x: 2.0,
                    y: 3.0,
                    z: 4.0,
                },
                radius: 0.5,
                label: "corner".to_owned(),
            });
        let diagram = InstructionDiagramPlan {
            bounds: DiagramBounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 10.0,
                max_y: 10.0,
            },
            steps: vec![InstructionDiagramStep {
                faces: Vec::new(),
                hinges: Vec::new(),
                changed_hinge_count: 0,
                declarative_only: true,
            }],
            projected_vertex_visits: 1,
        };
        let font = InstructionFont::load().expect("bundled font");
        let layout = layout_instruction_pages(
            "説明書",
            &timeline,
            &diagram,
            &font,
            InstructionExportLimits::default(),
        )
        .expect("declarative layout");
        let text = layout
            .pages
            .iter()
            .flat_map(|page| page.texts.iter())
            .map(PageText::scalar_text)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(text.contains("説明専用ステップ（3D姿勢・物理操作なし）"));
        assert!(text.contains("この説明はPDFとSVGに残ります。"));
        assert!(text.contains("自動実行せず層を確認してください。"));
        assert!(text.contains("説明専用・3D姿勢なし"));
        assert!(text.contains("Hand guides: regrip: right hand"));
        assert!(text.contains("Fold directions: fold up:"));
        assert!(text.contains("Focus points: corner:"));
        assert!(layout.pages.iter().all(|page| page.lines.is_empty()));
    }

    #[test]
    fn default_page_and_glyph_limits_accept_the_boundary_and_reject_one_more() {
        let mut glyphs = GlyphBudget {
            used: 0,
            maximum: super::super::MAX_INSTRUCTION_EXPORT_GLYPHS,
        };
        glyphs
            .claim(&"a".repeat(super::super::MAX_INSTRUCTION_EXPORT_GLYPHS))
            .expect("exact default glyph limit");
        assert!(matches!(
            glyphs.claim("a"),
            Err(InstructionExportError::LayoutLimitExceeded)
        ));

        let mut pages = (1..super::super::MAX_INSTRUCTION_EXPORT_PAGES)
            .map(|page| InstructionPage::new(page, 0))
            .collect::<Vec<_>>();
        push_page(
            &mut pages,
            InstructionPage::new(super::super::MAX_INSTRUCTION_EXPORT_PAGES, 0),
            super::super::MAX_INSTRUCTION_EXPORT_PAGES,
        )
        .expect("exact default page limit");
        assert_eq!(pages.len(), super::super::MAX_INSTRUCTION_EXPORT_PAGES);
        assert!(matches!(
            push_page(
                &mut pages,
                InstructionPage::new(super::super::MAX_INSTRUCTION_EXPORT_PAGES + 1, 0),
                super::super::MAX_INSTRUCTION_EXPORT_PAGES,
            ),
            Err(InstructionExportError::LayoutLimitExceeded)
        ));
    }
}
