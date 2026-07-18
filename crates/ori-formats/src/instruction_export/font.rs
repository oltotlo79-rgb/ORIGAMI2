use super::InstructionExportError;
use sha2::{Digest, Sha256};
use ttf_parser::OutlineBuilder;

pub(super) const NOTO_SANS_JP_BYTES: &[u8] =
    include_bytes!("../../assets/fonts/NotoSansJP-Variable.ttf");
pub(super) const NOTO_SANS_JP_LICENSE: &[u8] =
    include_bytes!("../../assets/fonts/NotoSansJP-OFL.txt");
pub(super) const NOTO_SANS_JP_SHA256: &str =
    "c2f3b4d463500a2ddcd3849cded1fceeb9fd6d1c32e6cbecd568453ba50fc68f";
pub(super) const NOTO_SANS_JP_LICENSE_SHA256: &str =
    "1c05c68c34f9708415aada51f17e1b0092d2cea709bf4a94cd38114f9e73d7d9";

pub(super) struct InstructionFont<'a> {
    face: ttf_parser::Face<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum GlyphPathCommand {
    Move {
        x: f64,
        y: f64,
    },
    Line {
        x: f64,
        y: f64,
    },
    Cubic {
        control_1_x: f64,
        control_1_y: f64,
        control_2_x: f64,
        control_2_y: f64,
        x: f64,
        y: f64,
    },
    Close,
}

pub(super) struct GlyphOutline {
    pub outlined: bool,
    pub commands: Vec<GlyphPathCommand>,
}

impl InstructionFont<'static> {
    pub(super) fn load() -> Result<Self, InstructionExportError> {
        if format!("{:x}", Sha256::digest(NOTO_SANS_JP_BYTES)) != NOTO_SANS_JP_SHA256
            || format!("{:x}", Sha256::digest(NOTO_SANS_JP_LICENSE)) != NOTO_SANS_JP_LICENSE_SHA256
        {
            return Err(InstructionExportError::FontAssetMismatch);
        }
        let mut face = ttf_parser::Face::parse(NOTO_SANS_JP_BYTES, 0)
            .map_err(|_| InstructionExportError::InvalidBundledFont)?;
        let _ = face.set_variation(ttf_parser::Tag::from_bytes(b"wght"), 400.0);
        Ok(Self { face })
    }
}

impl InstructionFont<'_> {
    pub(super) const fn face(&self) -> &ttf_parser::Face<'_> {
        &self.face
    }

    pub(super) fn units_per_em(&self) -> f64 {
        f64::from(self.face.units_per_em())
    }

    pub(super) fn glyph_id(
        &self,
        character: char,
    ) -> Result<ttf_parser::GlyphId, InstructionExportError> {
        self.face
            .glyph_index(character)
            .ok_or(InstructionExportError::UnsupportedGlyph {
                code_point: u32::from(character),
            })
    }

    pub(super) fn glyph_advance(
        &self,
        character: char,
        font_size: f64,
    ) -> Result<f64, InstructionExportError> {
        let glyph = self.glyph_id(character)?;
        let advance = self
            .face
            .glyph_hor_advance(glyph)
            .ok_or(InstructionExportError::InvalidBundledFont)?;
        let width = f64::from(advance) * font_size / self.units_per_em();
        if width.is_finite() && width >= 0.0 {
            Ok(width)
        } else {
            Err(InstructionExportError::InvalidBundledFont)
        }
    }

    pub(super) fn glyph_outline(
        &self,
        glyph_id: u16,
        origin_x: f64,
        origin_y: f64,
        scale_x: f64,
        scale_y: f64,
    ) -> Result<GlyphOutline, InstructionExportError> {
        let mut builder = GlyphOutlineBuilder::new(origin_x, origin_y, scale_x, scale_y);
        let outlined = self
            .face
            .outline_glyph(ttf_parser::GlyphId(glyph_id), &mut builder)
            .is_some();
        Ok(GlyphOutline {
            outlined,
            commands: builder.finish()?,
        })
    }
}

struct GlyphOutlineBuilder {
    origin_x: f64,
    origin_y: f64,
    scale_x: f64,
    scale_y: f64,
    current: Option<(f64, f64)>,
    contour_start: Option<(f64, f64)>,
    contour_open: bool,
    invalid: bool,
    commands: Vec<GlyphPathCommand>,
}

impl GlyphOutlineBuilder {
    fn new(origin_x: f64, origin_y: f64, scale_x: f64, scale_y: f64) -> Self {
        Self {
            origin_x,
            origin_y,
            scale_x,
            scale_y,
            current: None,
            contour_start: None,
            contour_open: false,
            invalid: !origin_x.is_finite()
                || !origin_y.is_finite()
                || !scale_x.is_finite()
                || !scale_y.is_finite()
                || scale_x == 0.0
                || scale_y == 0.0,
            commands: Vec::new(),
        }
    }

    fn transform(&mut self, x: f32, y: f32) -> Option<(f64, f64)> {
        let x = self.origin_x + f64::from(x) * self.scale_x;
        let y = self.origin_y + f64::from(y) * self.scale_y;
        if x.is_finite() && y.is_finite() {
            Some((canonical_zero(x), canonical_zero(y)))
        } else {
            self.invalid = true;
            None
        }
    }

    fn finish(self) -> Result<Vec<GlyphPathCommand>, InstructionExportError> {
        if self.invalid || self.contour_open {
            Err(InstructionExportError::StructureNotRepresentable)
        } else {
            Ok(self.commands)
        }
    }
}

impl OutlineBuilder for GlyphOutlineBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        if self.contour_open {
            self.invalid = true;
            return;
        }
        let Some((x, y)) = self.transform(x, y) else {
            return;
        };
        self.commands.push(GlyphPathCommand::Move { x, y });
        self.current = Some((x, y));
        self.contour_start = Some((x, y));
        self.contour_open = true;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        if !self.contour_open || self.current.is_none() {
            self.invalid = true;
            return;
        }
        let Some((x, y)) = self.transform(x, y) else {
            return;
        };
        self.commands.push(GlyphPathCommand::Line { x, y });
        self.current = Some((x, y));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let Some((current_x, current_y)) = self.current else {
            self.invalid = true;
            return;
        };
        if !self.contour_open {
            self.invalid = true;
            return;
        }
        let Some((quadratic_x, quadratic_y)) = self.transform(x1, y1) else {
            return;
        };
        let Some((x, y)) = self.transform(x, y) else {
            return;
        };

        // A quadratic Bezier P0/P1/P2 is exactly the cubic
        // P0, P0 + 2/3(P1-P0), P2 + 2/3(P1-P2), P2.
        let two_thirds = 2.0 / 3.0;
        let control_1_x = current_x + two_thirds * (quadratic_x - current_x);
        let control_1_y = current_y + two_thirds * (quadratic_y - current_y);
        let control_2_x = x + two_thirds * (quadratic_x - x);
        let control_2_y = y + two_thirds * (quadratic_y - y);
        if ![control_1_x, control_1_y, control_2_x, control_2_y]
            .into_iter()
            .all(f64::is_finite)
        {
            self.invalid = true;
            return;
        }
        self.commands.push(GlyphPathCommand::Cubic {
            control_1_x: canonical_zero(control_1_x),
            control_1_y: canonical_zero(control_1_y),
            control_2_x: canonical_zero(control_2_x),
            control_2_y: canonical_zero(control_2_y),
            x,
            y,
        });
        self.current = Some((x, y));
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        if !self.contour_open || self.current.is_none() {
            self.invalid = true;
            return;
        }
        let Some((control_1_x, control_1_y)) = self.transform(x1, y1) else {
            return;
        };
        let Some((control_2_x, control_2_y)) = self.transform(x2, y2) else {
            return;
        };
        let Some((x, y)) = self.transform(x, y) else {
            return;
        };
        self.commands.push(GlyphPathCommand::Cubic {
            control_1_x,
            control_1_y,
            control_2_x,
            control_2_y,
            x,
            y,
        });
        self.current = Some((x, y));
    }

    fn close(&mut self) {
        if !self.contour_open || self.current.is_none() || self.contour_start.is_none() {
            self.invalid = true;
            return;
        }
        self.commands.push(GlyphPathCommand::Close);
        self.current = self.contour_start;
        self.contour_start = None;
        self.contour_open = false;
    }
}

const fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outline_transform_supports_pdf_and_svg_axis_directions() {
        let font = InstructionFont::load().expect("bundled font");
        let glyph = font.glyph_id('折').expect("glyph").0;
        let pdf = font
            .glyph_outline(glyph, 20.0, 40.0, 0.01, 0.01)
            .expect("PDF-oriented outline");
        let svg = font
            .glyph_outline(glyph, 20.0, 40.0, 0.01, -0.01)
            .expect("SVG-oriented outline");
        assert!(pdf.outlined && svg.outlined);
        assert_eq!(pdf.commands.len(), svg.commands.len());
        assert!(!pdf.commands.is_empty());
    }

    #[test]
    fn quadratic_curves_are_converted_to_exact_cubic_control_points() {
        let mut builder = GlyphOutlineBuilder::new(0.0, 0.0, 1.0, 1.0);
        builder.move_to(0.0, 0.0);
        builder.quad_to(3.0, 3.0, 6.0, 0.0);
        builder.close();
        let commands = builder.finish().expect("valid outline");
        assert_eq!(
            commands,
            vec![
                GlyphPathCommand::Move { x: 0.0, y: 0.0 },
                GlyphPathCommand::Cubic {
                    control_1_x: 2.0,
                    control_1_y: 2.0,
                    control_2_x: 4.0,
                    control_2_y: 2.0,
                    x: 6.0,
                    y: 0.0,
                },
                GlyphPathCommand::Close
            ]
        );
    }
}
