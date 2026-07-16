use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! entity_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

entity_id!(ProjectId);
entity_id!(VertexId);
entity_id!(EdgeId);
entity_id!(FaceId);
entity_id!(AssetId);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

impl Point2 {
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Mountain,
    Valley,
    Auxiliary,
    Boundary,
    Cut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RgbaColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl RgbaColor {
    #[must_use]
    pub const fn opaque(red: u8, green: u8, blue: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha: 255,
        }
    }
}

impl Default for RgbaColor {
    fn default() -> Self {
        Self::opaque(255, 255, 255)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PaperAppearance {
    pub color: RgbaColor,
    pub texture_asset: Option<AssetId>,
}

pub const DEFAULT_PAPER_THICKNESS_MM: f64 = 0.10;
pub const DEFAULT_PAPER_FRONT_COLOR: RgbaColor = RgbaColor::opaque(255, 255, 255);
pub const DEFAULT_PAPER_BACK_COLOR: RgbaColor = RgbaColor::opaque(248, 248, 245);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Paper {
    pub boundary_vertices: Vec<VertexId>,
    /// Physical thickness in millimetres.
    ///
    /// Persistence deliberately neither clamps nor applies a sign policy to
    /// negative or non-finite values. Their admissibility belongs to the
    /// domain-validation workflow; the JSON codec's number representation is
    /// the interchange boundary rather than a reason to mutate design data.
    pub thickness_mm: f64,
    pub cutting_allowed: bool,
    pub front: PaperAppearance,
    pub back: PaperAppearance,
}

impl Default for Paper {
    fn default() -> Self {
        Self {
            boundary_vertices: Vec::new(),
            thickness_mm: DEFAULT_PAPER_THICKNESS_MM,
            cutting_allowed: false,
            front: PaperAppearance {
                color: DEFAULT_PAPER_FRONT_COLOR,
                texture_asset: None,
            },
            back: PaperAppearance {
                color: DEFAULT_PAPER_BACK_COLOR,
                texture_asset: None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vertex {
    pub id: VertexId,
    pub position: Point2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub id: EdgeId,
    pub start: VertexId,
    pub end: VertexId,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreasePattern {
    pub vertices: Vec<Vertex>,
    pub edges: Vec<Edge>,
}

impl CreasePattern {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            vertices: Vec::new(),
            edges: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_survive_json_round_trip() {
        let vertex = Vertex {
            id: VertexId::new(),
            position: Point2::new(1.0, 2.0),
        };
        let json = serde_json::to_string(&vertex).expect("serialize vertex");
        let restored: Vertex = serde_json::from_str(&json).expect("deserialize vertex");
        assert_eq!(restored, vertex);
    }

    #[test]
    fn default_paper_is_safe_for_legacy_projects() {
        let paper = Paper::default();
        assert!(paper.boundary_vertices.is_empty());
        assert_eq!(paper.thickness_mm, 0.10);
        assert!(!paper.cutting_allowed);
        assert_eq!(paper.front.color, DEFAULT_PAPER_FRONT_COLOR);
        assert_eq!(paper.back.color, DEFAULT_PAPER_BACK_COLOR);
        assert_eq!(paper.front.texture_asset, None);
        assert_eq!(paper.back.texture_asset, None);
    }
}
