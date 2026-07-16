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

            /// Returns the UUID in its canonical RFC byte order.
            ///
            /// The returned value is an owned copy, so callers can use it in
            /// deterministic keys without borrowing the ID.
            #[must_use]
            pub const fn canonical_bytes(&self) -> [u8; 16] {
                self.0.into_bytes()
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

impl FaceId {
    /// Derives a stable face ID from a project namespace and canonical name.
    ///
    /// UUID v5 makes the same namespace/name pair deterministic. Callers are
    /// responsible for constructing a collision-resistant canonical name.
    #[must_use]
    pub fn derive_v5(namespace: ProjectId, name: &[u8]) -> Self {
        Self(Uuid::new_v5(&namespace.0, name))
    }
}

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
    fn all_entity_ids_expose_canonical_rfc_byte_order() {
        const EXPECTED: [u8; 16] = [
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ];
        const JSON_ID: &str = r#""00112233-4455-6677-8899-aabbccddeeff""#;

        let project: ProjectId = serde_json::from_str(JSON_ID).expect("deserialize project ID");
        let vertex: VertexId = serde_json::from_str(JSON_ID).expect("deserialize vertex ID");
        let edge: EdgeId = serde_json::from_str(JSON_ID).expect("deserialize edge ID");
        let face: FaceId = serde_json::from_str(JSON_ID).expect("deserialize face ID");
        let asset: AssetId = serde_json::from_str(JSON_ID).expect("deserialize asset ID");

        for bytes in [
            project.canonical_bytes(),
            vertex.canonical_bytes(),
            edge.canonical_bytes(),
            face.canonical_bytes(),
            asset.canonical_bytes(),
        ] {
            assert_eq!(bytes, EXPECTED);
        }
    }

    #[test]
    fn face_v5_derivation_is_deterministic() {
        let namespace: ProjectId =
            serde_json::from_str(r#""00112233-4455-6677-8899-aabbccddeeff""#)
                .expect("deserialize namespace");
        let expected: FaceId = serde_json::from_str(r#""2c99010b-dc57-5a6b-9e5d-9c16280876d7""#)
            .expect("deserialize expected face ID");

        let first = FaceId::derive_v5(namespace, b"face-key");
        let second = FaceId::derive_v5(namespace, b"face-key");

        assert_eq!(first, expected);
        assert_eq!(second, expected);
    }

    #[test]
    fn face_v5_derivation_separates_namespaces_and_names() {
        let first_namespace: ProjectId =
            serde_json::from_str(r#""00112233-4455-6677-8899-aabbccddeeff""#)
                .expect("deserialize first namespace");
        let second_namespace: ProjectId =
            serde_json::from_str(r#""ffffffff-ffff-ffff-ffff-ffffffffffff""#)
                .expect("deserialize second namespace");

        let baseline = FaceId::derive_v5(first_namespace, b"face-key");
        let different_name = FaceId::derive_v5(first_namespace, b"face-key-2");
        let different_namespace = FaceId::derive_v5(second_namespace, b"face-key");

        assert_ne!(baseline, different_name);
        assert_ne!(baseline, different_namespace);
        assert_ne!(different_name, different_namespace);
    }

    #[test]
    fn derived_face_id_survives_json_round_trip() {
        let namespace: ProjectId =
            serde_json::from_str(r#""00112233-4455-6677-8899-aabbccddeeff""#)
                .expect("deserialize namespace");
        let face = FaceId::derive_v5(namespace, b"\0binary\xffface-key");

        let json = serde_json::to_string(&face).expect("serialize derived face ID");
        let restored: FaceId = serde_json::from_str(&json).expect("deserialize derived face ID");

        assert_eq!(restored, face);
        assert_eq!(restored.canonical_bytes(), face.canonical_bytes());
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
