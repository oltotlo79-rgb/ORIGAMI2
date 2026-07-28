//! Strict, bounded wire records for persisted material-relief state.
//!
//! This module intentionally defines data, canonical hashes, and structural
//! validation only. Deserializing or validating a [`MaterialReliefDocumentV1`]
//! never grants mutation, topology, simulation, collision, proof, or export
//! authority. A later topology layer must independently reconstruct and
//! validate the requested removal from the current paper and crease pattern
//! before an editor transaction may consume it.
//!
//! Raw serde decoding must still run behind the persistence layer's hard byte
//! ceiling. The collection ceilings below are enforced before hashing or
//! validation allocates collection-sized scratch storage; deriving
//! `Deserialize` does not itself impose those semantic element ceilings.

use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{EdgeId, ProjectId, VertexId};

mod bounded;
pub use bounded::{
    material_relief_geometry_sha256_v1, material_relief_state_sha256_v1,
    material_relief_substrate_sha256_v1, validate_material_relief_document_v1,
};

/// Exact persisted schema version accepted by this implementation.
pub const MATERIAL_RELIEF_DOCUMENT_VERSION_V1: u8 = 1;
/// Model label for this untrusted persistence record.
pub const MATERIAL_RELIEF_DOCUMENT_MODEL_ID_V1: &str = "untrusted_material_relief_document_v1";
/// Maximum number of independently selected removal regions.
pub const MAX_MATERIAL_RELIEF_REGIONS_V1: usize = 64;
/// Maximum number of removed component keys in one region closure.
pub const MAX_MATERIAL_RELIEF_REMOVED_COMPONENTS_V1: usize = 64;
/// Maximum number of removed component keys across the whole document.
pub const MAX_MATERIAL_RELIEF_TOTAL_REMOVED_COMPONENTS_V1: usize =
    MAX_MATERIAL_RELIEF_REGIONS_V1 * MAX_MATERIAL_RELIEF_REMOVED_COMPONENTS_V1;
/// Maximum number of cut edges in one canonical boundary loop.
pub const MAX_MATERIAL_RELIEF_LOOP_EDGES_V1: usize = 16_384;
/// Maximum number of cut-loop edge references across the whole document.
pub const MAX_MATERIAL_RELIEF_TOTAL_LOOP_EDGES_V1: usize = 16_384;
/// Maximum crease-pattern vertex count admitted while material relief exists.
pub const MAX_MATERIAL_RELIEF_PATTERN_VERTICES_V1: usize = 100_000;
/// Maximum crease-pattern edge count admitted while material relief exists.
pub const MAX_MATERIAL_RELIEF_PATTERN_EDGES_V1: usize = 100_000;
/// Maximum paper-boundary vertex references admitted while relief exists.
pub const MAX_MATERIAL_RELIEF_PAPER_BOUNDARY_VERTICES_V1: usize = 100_000;

const MATERIAL_RELIEF_LINEAGE_NAME_DOMAIN_V1: &[u8] =
    b"ORIGAMI2\0material-relief-lineage-name\0v1\0";
const MATERIAL_RELIEF_SUBSTRATE_HASH_DOMAIN_V1: &[u8] =
    b"ORIGAMI2\0material-relief-substrate\0v1\0";
const MATERIAL_RELIEF_GEOMETRY_HASH_DOMAIN_V1: &[u8] = b"ORIGAMI2\0material-relief-geometry\0v1\0";
const MATERIAL_RELIEF_STATE_HASH_DOMAIN_V1: &[u8] = b"ORIGAMI2\0material-relief-state\0v1\0";

/// Stable project-scoped lineage of one selected material-removal region.
///
/// The deterministic constructor binds the project, revision-independent
/// substrate fingerprint, and requested component key. There is deliberately
/// no random or `Default` constructor: builders must derive this identity from
/// all three bindings, while deserialized values remain untrusted until
/// validation recomputes that derivation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MaterialReliefLineageId(Uuid);

impl MaterialReliefLineageId {
    /// Derives a stable lineage from the project and canonical removal root.
    #[must_use]
    pub fn derive_v5(
        source_project_id: ProjectId,
        substrate_fingerprint_sha256: [u8; 32],
        requested_component_key: [u8; 32],
    ) -> Self {
        let mut name = FramedSha256::new(MATERIAL_RELIEF_LINEAGE_NAME_DOMAIN_V1);
        name.frame(&substrate_fingerprint_sha256);
        name.frame(&requested_component_key);
        let name_sha256 = name.finish();
        let namespace = Uuid::from_bytes(source_project_id.canonical_bytes());
        Self(Uuid::new_v5(&namespace, &name_sha256))
    }

    /// Returns the UUID in canonical RFC byte order.
    #[must_use]
    pub const fn canonical_bytes(&self) -> [u8; 16] {
        self.0.into_bytes()
    }

    /// Returns whether this is the reserved nil UUID.
    #[must_use]
    pub fn is_nil(&self) -> bool {
        self.canonical_bytes() == [0; 16]
    }
}

/// One canonical removal root and its freshly reconstructed component closure.
///
/// `requested_component_key` is required to occur in
/// `removed_component_keys`. The closure is strictly ordered, and
/// `boundary_edge_loop` is the least edge-ID rotation with the lesser of its
/// two directions. Validation also requires every loop edge to be a connected
/// non-degenerate `Cut` edge and forbids cross-region reuse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialReliefRegionV1 {
    pub lineage_id: MaterialReliefLineageId,
    pub requested_component_key: [u8; 32],
    pub removed_component_keys: Vec<[u8; 32]>,
    pub boundary_edge_loop: Vec<EdgeId>,
}

/// Untrusted persisted description of material-relief geometry.
///
/// This document deliberately carries no live capability. Even a structurally
/// valid, hash-consistent instance is only input to a future fresh topology
/// reconstruction. In particular it must never be manufactured from
/// `MaterialVoidEvidenceDocumentV1` or admitted to simulation by possession.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialReliefDocumentV1 {
    pub version: u8,
    pub source_project_id: Option<ProjectId>,
    pub substrate_fingerprint_sha256: [u8; 32],
    pub state_sha256: [u8; 32],
    /// Regions strictly ordered by `requested_component_key`.
    pub regions: Vec<MaterialReliefRegionV1>,
}

impl Default for MaterialReliefDocumentV1 {
    fn default() -> Self {
        Self {
            version: MATERIAL_RELIEF_DOCUMENT_VERSION_V1,
            source_project_id: None,
            substrate_fingerprint_sha256: [0; 32],
            state_sha256: [0; 32],
            regions: Vec::new(),
        }
    }
}

impl MaterialReliefDocumentV1 {
    /// Returns whether this is the one exact legacy-compatible empty value.
    #[must_use]
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }

    /// Returns whether no material-removal regions are represented.
    ///
    /// Callers must use [`Self::is_default`] when deciding whether a document
    /// is validly empty; non-default empty envelopes are rejected.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        MATERIAL_RELIEF_DOCUMENT_MODEL_ID_V1
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_material_removal(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_persistence(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_topology_admission(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_simulation_admission(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_collision_admission(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_proof_issuance(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_export(&self) -> bool {
        false
    }
}

/// Structural or binding failure in an untrusted relief document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialReliefDocumentValidationErrorV1 {
    UnsupportedVersion {
        actual: u8,
        expected: u8,
    },
    NonDefaultEmptyDocument,
    MissingSourceProjectId,
    NilSourceProjectId,
    CuttingNotAllowed,
    TooManyRegions {
        actual: usize,
        maximum: usize,
    },
    TooManyPatternVertices {
        actual: usize,
        maximum: usize,
    },
    TooManyPatternEdges {
        actual: usize,
        maximum: usize,
    },
    TooManyPaperBoundaryVertices {
        actual: usize,
        maximum: usize,
    },
    TooManyRemovedComponents {
        region_index: usize,
        actual: usize,
        maximum: usize,
    },
    TooManyTotalRemovedComponents {
        actual: usize,
        maximum: usize,
    },
    TooManyLoopEdges {
        region_index: usize,
        actual: usize,
        maximum: usize,
    },
    TooManyTotalLoopEdges {
        actual: usize,
        maximum: usize,
    },
    ZeroSubstrateFingerprint,
    SubstrateFingerprintMismatch,
    ZeroStateDigest,
    StateDigestMismatch,
    NonCanonicalRegionOrder {
        region_index: usize,
    },
    DuplicateLineageId {
        region_index: usize,
    },
    DuplicateRequestedComponentKey {
        region_index: usize,
    },
    NilLineageId {
        region_index: usize,
    },
    InvalidLineageId {
        region_index: usize,
    },
    ZeroRequestedComponentKey {
        region_index: usize,
    },
    EmptyRemovedComponents {
        region_index: usize,
    },
    RemovedComponentsNotCanonical {
        region_index: usize,
    },
    RequestedComponentMissingFromClosure {
        region_index: usize,
    },
    RemovedComponentClosureOverlap {
        region_index: usize,
    },
    InvalidBoundaryLoop {
        region_index: usize,
    },
    BoundaryEdgeReused {
        region_index: usize,
        edge: EdgeId,
    },
    NilPatternVertex {
        vertex: VertexId,
    },
    DuplicatePatternVertex {
        vertex: VertexId,
    },
    NonFinitePatternVertex {
        vertex: VertexId,
    },
    DuplicatePatternEdge {
        edge: EdgeId,
    },
    PatternEdgeReferencesUnknownVertex {
        edge: EdgeId,
    },
    NilPatternEdge {
        edge: EdgeId,
    },
    DegeneratePatternEdge {
        edge: EdgeId,
    },
    PaperBoundaryReferencesUnknownVertex {
        vertex: VertexId,
    },
    InvalidPaperBoundary,
    DuplicatePaperBoundaryVertex {
        vertex: VertexId,
    },
    UnknownBoundaryEdge {
        region_index: usize,
        edge: EdgeId,
    },
    NonCutBoundaryEdge {
        region_index: usize,
        edge: EdgeId,
    },
    ResourceAllocation,
}

impl fmt::Display for MaterialReliefDocumentValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion { actual, expected } => write!(
                formatter,
                "unsupported material-relief version {actual}; expected {expected}"
            ),
            Self::NonDefaultEmptyDocument => {
                formatter.write_str("an empty material-relief document must be the exact default")
            }
            Self::MissingSourceProjectId => {
                formatter.write_str("non-empty material relief has no source project ID")
            }
            Self::NilSourceProjectId => {
                formatter.write_str("non-empty material relief uses the nil source project ID")
            }
            Self::CuttingNotAllowed => {
                formatter.write_str("material relief exists while paper cutting is disabled")
            }
            Self::TooManyRegions { actual, maximum } => write!(
                formatter,
                "material-relief region count {actual} exceeds {maximum}"
            ),
            Self::TooManyPatternVertices { actual, maximum } => write!(
                formatter,
                "material-relief pattern vertex count {actual} exceeds {maximum}"
            ),
            Self::TooManyPatternEdges { actual, maximum } => write!(
                formatter,
                "material-relief pattern edge count {actual} exceeds {maximum}"
            ),
            Self::TooManyPaperBoundaryVertices { actual, maximum } => write!(
                formatter,
                "material-relief paper-boundary vertex count {actual} exceeds {maximum}"
            ),
            Self::TooManyRemovedComponents {
                region_index,
                actual,
                maximum,
            } => write!(
                formatter,
                "material-relief region {region_index} removed-component count {actual} exceeds {maximum}"
            ),
            Self::TooManyTotalRemovedComponents { actual, maximum } => write!(
                formatter,
                "material-relief removed-component count {actual} exceeds {maximum}"
            ),
            Self::TooManyLoopEdges {
                region_index,
                actual,
                maximum,
            } => write!(
                formatter,
                "material-relief region {region_index} loop length {actual} exceeds {maximum}"
            ),
            Self::TooManyTotalLoopEdges { actual, maximum } => write!(
                formatter,
                "material-relief total loop length {actual} exceeds {maximum}"
            ),
            Self::ZeroSubstrateFingerprint => {
                formatter.write_str("material-relief substrate fingerprint is zero")
            }
            Self::SubstrateFingerprintMismatch => {
                formatter.write_str("material-relief substrate fingerprint does not match")
            }
            Self::ZeroStateDigest => formatter.write_str("material-relief state digest is zero"),
            Self::StateDigestMismatch => {
                formatter.write_str("material-relief state digest does not match")
            }
            Self::NonCanonicalRegionOrder { region_index } => write!(
                formatter,
                "material-relief region {region_index} is not in canonical order"
            ),
            Self::DuplicateLineageId { region_index } => write!(
                formatter,
                "material-relief region {region_index} repeats a lineage ID"
            ),
            Self::DuplicateRequestedComponentKey { region_index } => write!(
                formatter,
                "material-relief region {region_index} repeats a requested component key"
            ),
            Self::NilLineageId { region_index } => write!(
                formatter,
                "material-relief region {region_index} uses the nil lineage ID"
            ),
            Self::InvalidLineageId { region_index } => write!(
                formatter,
                "material-relief region {region_index} has an invalid derived lineage ID"
            ),
            Self::ZeroRequestedComponentKey { region_index } => write!(
                formatter,
                "material-relief region {region_index} has a zero requested component key"
            ),
            Self::EmptyRemovedComponents { region_index } => write!(
                formatter,
                "material-relief region {region_index} has an empty removed-component closure"
            ),
            Self::RemovedComponentsNotCanonical { region_index } => write!(
                formatter,
                "material-relief region {region_index} removed components are not canonical"
            ),
            Self::RequestedComponentMissingFromClosure { region_index } => write!(
                formatter,
                "material-relief region {region_index} closure omits its requested component"
            ),
            Self::RemovedComponentClosureOverlap { region_index } => write!(
                formatter,
                "material-relief region {region_index} overlaps another component closure"
            ),
            Self::InvalidBoundaryLoop { region_index } => write!(
                formatter,
                "material-relief region {region_index} has an invalid boundary loop"
            ),
            Self::BoundaryEdgeReused { region_index, edge } => write!(
                formatter,
                "material-relief region {region_index} reuses boundary edge {edge:?}"
            ),
            Self::NilPatternVertex { vertex } => {
                write!(formatter, "crease pattern uses nil vertex {vertex:?}")
            }
            Self::DuplicatePatternVertex { vertex } => {
                write!(formatter, "crease pattern repeats vertex {vertex:?}")
            }
            Self::NonFinitePatternVertex { vertex } => {
                write!(formatter, "crease pattern vertex {vertex:?} is non-finite")
            }
            Self::DuplicatePatternEdge { edge } => {
                write!(formatter, "crease pattern repeats edge {edge:?}")
            }
            Self::PatternEdgeReferencesUnknownVertex { edge } => {
                write!(
                    formatter,
                    "crease pattern edge {edge:?} references an unknown vertex"
                )
            }
            Self::NilPatternEdge { edge } => {
                write!(formatter, "crease pattern uses nil edge {edge:?}")
            }
            Self::DegeneratePatternEdge { edge } => {
                write!(formatter, "crease pattern edge {edge:?} is degenerate")
            }
            Self::PaperBoundaryReferencesUnknownVertex { vertex } => {
                write!(
                    formatter,
                    "paper boundary references unknown vertex {vertex:?}"
                )
            }
            Self::InvalidPaperBoundary => {
                formatter.write_str("non-empty material relief requires a paper boundary cycle")
            }
            Self::DuplicatePaperBoundaryVertex { vertex } => {
                write!(formatter, "paper boundary repeats vertex {vertex:?}")
            }
            Self::UnknownBoundaryEdge { region_index, edge } => write!(
                formatter,
                "material-relief region {region_index} references unknown edge {edge:?}"
            ),
            Self::NonCutBoundaryEdge { region_index, edge } => write!(
                formatter,
                "material-relief region {region_index} edge {edge:?} is not a Cut edge"
            ),
            Self::ResourceAllocation => {
                formatter.write_str("material-relief operation could not reserve bounded memory")
            }
        }
    }
}

impl Error for MaterialReliefDocumentValidationErrorV1 {}

struct FramedSha256(Sha256);

impl FramedSha256 {
    fn new(domain: &[u8]) -> Self {
        let mut hash = Self(Sha256::new());
        hash.frame(domain);
        hash
    }

    fn frame(&mut self, value: &[u8]) {
        self.0.update(
            u64::try_from(value.len())
                .expect("a slice length must fit in u64 on supported targets")
                .to_be_bytes(),
        );
        self.0.update(value);
    }

    fn frame_usize(&mut self, value: usize) {
        self.frame(
            &u64::try_from(value)
                .expect("a collection length must fit in u64 on supported targets")
                .to_be_bytes(),
        );
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[cfg(test)]
#[path = "material_relief_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "material_relief_tests.rs"]
mod tests;
