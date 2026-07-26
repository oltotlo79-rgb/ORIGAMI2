use ori_domain::ProjectId;

use super::{super::MAX_REVISION, super::Revision, SpeculativeUnprovenFoldMetadataErrorV1};

/// Coarse observation made by the approximate preview layer.
///
/// Binary64 values are held by their exact bit pattern. This runtime metadata
/// intentionally does not implement Serde.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeculativeApproximateBlockingObservationV1 {
    NoBlockingSampleObserved,
    BlockingSampleObserved { first_blocking_angle_bits: u64 },
}

impl SpeculativeApproximateBlockingObservationV1 {
    #[must_use]
    pub const fn no_blocking_sample_observed() -> Self {
        Self::NoBlockingSampleObserved
    }

    pub fn blocking_sample_observed(
        first_blocking_angle_degrees: f64,
    ) -> Result<Self, SpeculativeUnprovenFoldMetadataErrorV1> {
        if !first_blocking_angle_degrees.is_finite()
            || !(0.0..=180.0).contains(&first_blocking_angle_degrees)
        {
            return Err(SpeculativeUnprovenFoldMetadataErrorV1::InvalidBlockingAngle);
        }
        Ok(Self::BlockingSampleObserved {
            first_blocking_angle_bits: first_blocking_angle_degrees.to_bits(),
        })
    }

    #[must_use]
    pub fn first_blocking_angle_degrees(self) -> Option<f64> {
        match self {
            Self::NoBlockingSampleObserved => None,
            Self::BlockingSampleObserved {
                first_blocking_angle_bits,
            } => Some(f64::from_bits(first_blocking_angle_bits)),
        }
    }
}

/// Complete source-state binding copied from the desktop one-shot token.
///
/// Project identities are required binding data. No vertex, edge, face,
/// coordinate, or shape data is admitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeculativeUnprovenFoldBindingV1 {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    source_revision: Revision,
    source_geometry_fingerprint_sha256: String,
    pose_generation: u64,
    request_generation_id: ProjectId,
    paper_thickness_bits: u64,
    approximate_blocking_observation: SpeculativeApproximateBlockingObservationV1,
}

impl SpeculativeUnprovenFoldBindingV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        project_instance_id: ProjectId,
        project_id: ProjectId,
        source_revision: Revision,
        source_geometry_fingerprint_sha256: String,
        pose_generation: u64,
        request_generation_id: ProjectId,
        paper_thickness_mm: f64,
        approximate_blocking_observation: SpeculativeApproximateBlockingObservationV1,
    ) -> Result<Self, SpeculativeUnprovenFoldMetadataErrorV1> {
        Self::from_exact_parts(
            project_instance_id,
            project_id,
            source_revision,
            source_geometry_fingerprint_sha256,
            pose_generation,
            request_generation_id,
            paper_thickness_mm.to_bits(),
            approximate_blocking_observation,
        )
    }

    #[must_use]
    pub const fn project_instance_id(&self) -> ProjectId {
        self.project_instance_id
    }

    #[must_use]
    pub const fn project_id(&self) -> ProjectId {
        self.project_id
    }

    #[must_use]
    pub const fn source_revision(&self) -> Revision {
        self.source_revision
    }

    #[must_use]
    pub fn source_geometry_fingerprint_sha256(&self) -> &str {
        &self.source_geometry_fingerprint_sha256
    }

    #[must_use]
    pub const fn pose_generation(&self) -> u64 {
        self.pose_generation
    }

    #[must_use]
    pub const fn request_generation_id(&self) -> ProjectId {
        self.request_generation_id
    }

    #[must_use]
    pub const fn paper_thickness_bits(&self) -> u64 {
        self.paper_thickness_bits
    }

    #[must_use]
    pub const fn approximate_blocking_observation(
        &self,
    ) -> SpeculativeApproximateBlockingObservationV1 {
        self.approximate_blocking_observation
    }

    pub(crate) fn has_same_request_identity(&self, other: &Self) -> bool {
        self.project_instance_id == other.project_instance_id
            && self.project_id == other.project_id
            && self.request_generation_id == other.request_generation_id
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_exact_parts(
        project_instance_id: ProjectId,
        project_id: ProjectId,
        source_revision: Revision,
        source_geometry_fingerprint_sha256: String,
        pose_generation: u64,
        request_generation_id: ProjectId,
        paper_thickness_bits: u64,
        approximate_blocking_observation: SpeculativeApproximateBlockingObservationV1,
    ) -> Result<Self, SpeculativeUnprovenFoldMetadataErrorV1> {
        let binding = Self {
            project_instance_id,
            project_id,
            source_revision,
            source_geometry_fingerprint_sha256,
            pose_generation,
            request_generation_id,
            paper_thickness_bits,
            approximate_blocking_observation,
        };
        binding.validate()?;
        Ok(binding)
    }

    pub(crate) fn validate(&self) -> Result<(), SpeculativeUnprovenFoldMetadataErrorV1> {
        if self.project_instance_id.canonical_bytes() == [0; 16] {
            return Err(SpeculativeUnprovenFoldMetadataErrorV1::NilProjectInstanceId);
        }
        if self.project_id.canonical_bytes() == [0; 16] {
            return Err(SpeculativeUnprovenFoldMetadataErrorV1::NilProjectId);
        }
        if self.request_generation_id.canonical_bytes() == [0; 16] {
            return Err(SpeculativeUnprovenFoldMetadataErrorV1::NilRequestGenerationId);
        }
        if self.source_revision > MAX_REVISION {
            return Err(SpeculativeUnprovenFoldMetadataErrorV1::RevisionOutOfRange);
        }
        if self.pose_generation > MAX_REVISION {
            return Err(SpeculativeUnprovenFoldMetadataErrorV1::PoseGenerationOutOfRange);
        }
        if !is_lowercase_sha256_hex(&self.source_geometry_fingerprint_sha256) {
            return Err(SpeculativeUnprovenFoldMetadataErrorV1::InvalidGeometryFingerprint);
        }
        let thickness = f64::from_bits(self.paper_thickness_bits);
        if !thickness.is_finite() || thickness < 0.0 {
            return Err(SpeculativeUnprovenFoldMetadataErrorV1::InvalidPaperThickness);
        }
        if let SpeculativeApproximateBlockingObservationV1::BlockingSampleObserved {
            first_blocking_angle_bits,
        } = self.approximate_blocking_observation
        {
            let angle = f64::from_bits(first_blocking_angle_bits);
            if !angle.is_finite() || !(0.0..=180.0).contains(&angle) {
                return Err(SpeculativeUnprovenFoldMetadataErrorV1::InvalidBlockingAngle);
            }
        }
        Ok(())
    }
}

fn is_lowercase_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
