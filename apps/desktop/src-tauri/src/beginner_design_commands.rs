//! Native beginner-design command handlers and their bounded analysis helpers.
//!
//! The crate root retains only Tauri registration and shared application
//! wiring; this module owns the beginner candidate, parameter-grid, and
//! reference-model workflows.

use super::*;
use std::sync::atomic::AtomicU64;

// These registries contain only native jobs that are simultaneously owned by
// live command scopes. Bounding them prevents untrusted generation IDs from
// turning command registration into unbounded process-global allocation.
pub(super) const MAX_REFERENCE_CONSENSUS_WORK_REGISTRATIONS_V1: usize = 64;
pub(super) const MAX_BEGINNER_GRID_WORK_REGISTRATIONS_V1: usize = 64;

struct ActiveWorkRegistrationClaimV1<'a> {
    active: &'a AtomicBool,
    committed: bool,
}

impl<'a> ActiveWorkRegistrationClaimV1<'a> {
    fn try_claim(active: &'a AtomicBool, reused_error: &'static str) -> Result<Self, String> {
        active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| reused_error.to_owned())?;
        Ok(Self {
            active,
            committed: false,
        })
    }

    fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for ActiveWorkRegistrationClaimV1<'_> {
    fn drop(&mut self) {
        if !self.committed {
            self.active.store(false, Ordering::Release);
        }
    }
}

#[derive(Default)]
pub(super) struct ReferenceConsensusWorkV1 {
    pub(super) cancelled: AtomicBool,
    pub(super) terminal: AtomicU64,
    pub(super) registration_active: AtomicBool,
}
static REFERENCE_CONSENSUS_WORK_V1: OnceLock<
    Mutex<HashMap<ProjectId, Arc<ReferenceConsensusWorkV1>>>,
> = OnceLock::new();
pub(super) fn reference_consensus_work_v1()
-> &'static Mutex<HashMap<ProjectId, Arc<ReferenceConsensusWorkV1>>> {
    REFERENCE_CONSENSUS_WORK_V1.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_recovering_registry_v1<T>(registry: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match registry.lock() {
        Ok(entries) => entries,
        Err(poisoned) => {
            let entries = poisoned.into_inner();
            registry.clear_poison();
            entries
        }
    }
}

fn request_work_cancellation_v1(cancelled: &AtomicBool, terminal: &AtomicU64) {
    // Publish the cooperative stop before the cancellation terminal. An
    // Acquire observer that sees terminal=2 must never still see false here.
    // finish_* takes the same registry mutex as the caller, so a completed or
    // failed terminal can safely win and roll this speculative flag back.
    cancelled.store(true, Ordering::Release);
    match terminal.compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) | Err(2) => {}
        Err(_) => cancelled.store(false, Ordering::Release),
    }
}

#[must_use = "dropping the registration releases only its exact consensus work"]
pub(super) struct ReferenceConsensusWorkRegistration {
    request_generation_id: ProjectId,
    work: Arc<ReferenceConsensusWorkV1>,
}

impl Drop for ReferenceConsensusWorkRegistration {
    fn drop(&mut self) {
        let mut entries = lock_recovering_registry_v1(reference_consensus_work_v1());
        let exact_owner = entries
            .get(&self.request_generation_id)
            .is_some_and(|current| Arc::ptr_eq(current, &self.work));
        let _ = self
            .work
            .terminal
            .compare_exchange(0, 3, Ordering::AcqRel, Ordering::Acquire);
        self.work
            .registration_active
            .store(false, Ordering::Release);
        if exact_owner {
            entries.remove(&self.request_generation_id);
        }
    }
}

fn reference_consensus_work_is_fresh_v1(work: &ReferenceConsensusWorkV1) -> bool {
    work.terminal.load(Ordering::Acquire) == 0 && !work.cancelled.load(Ordering::Acquire)
}

pub(super) fn register_reference_consensus_work_v1(
    request_generation_id: ProjectId,
    work: &Arc<ReferenceConsensusWorkV1>,
) -> Result<ReferenceConsensusWorkRegistration, String> {
    let mut registry = lock_recovering_registry_v1(reference_consensus_work_v1());
    registry.retain(|_, existing| existing.registration_active.load(Ordering::Acquire));
    if registry.contains_key(&request_generation_id) {
        return Err("reference_consensus_generation_reused".to_owned());
    }
    let claim = ActiveWorkRegistrationClaimV1::try_claim(
        &work.registration_active,
        "reference_consensus_work_reused",
    )?;
    if !reference_consensus_work_is_fresh_v1(work) {
        return Err("reference_consensus_work_not_fresh".to_owned());
    }
    if registry.len() >= MAX_REFERENCE_CONSENSUS_WORK_REGISTRATIONS_V1
        || registry.try_reserve(1).is_err()
    {
        return Err("reference_consensus_registry_resource_limit".to_owned());
    }
    let registration_work = Arc::clone(work);
    registry.insert(request_generation_id, Arc::clone(work));
    claim.commit();
    Ok(ReferenceConsensusWorkRegistration {
        request_generation_id,
        work: registration_work,
    })
}

fn finish_reference_consensus_work_v1<T>(
    request_generation_id: ProjectId,
    work: &ReferenceConsensusWorkV1,
    result: Result<T, String>,
) -> Result<T, String> {
    let registry = lock_recovering_registry_v1(reference_consensus_work_v1());
    let owns_generation = work.registration_active.load(Ordering::Acquire)
        && registry
            .get(&request_generation_id)
            .is_some_and(|current| std::ptr::eq(Arc::as_ptr(current), work));
    if !owns_generation {
        let _ = work
            .terminal
            .compare_exchange(0, 3, Ordering::AcqRel, Ordering::Acquire);
        return Err("reference_consensus_failed".to_owned());
    }
    match result {
        Ok(response) => {
            match work
                .terminal
                .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => Ok(response),
                Err(2) => Err("reference_consensus_cancelled".to_owned()),
                Err(_) => Err("reference_consensus_failed".to_owned()),
            }
        }
        Err(error) => {
            match work
                .terminal
                .compare_exchange(0, 3, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => Err(error),
                Err(2) => Err("reference_consensus_cancelled".to_owned()),
                Err(_) => Err(error),
            }
        }
    }
}

pub(super) fn run_registered_reference_consensus_work_v1<T>(
    request_generation_id: ProjectId,
    work: &Arc<ReferenceConsensusWorkV1>,
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let _registration = register_reference_consensus_work_v1(request_generation_id, work)?;
    finish_reference_consensus_work_v1(request_generation_id, work, operation())
}

#[derive(Default)]
pub(super) struct BeginnerGridWork {
    pub(super) cancelled: AtomicBool,
    pub(super) enumerated: AtomicU64,
    pub(super) global_checked: AtomicU64,
    pub(super) refinement_iterations: AtomicU64,
    pub(super) terminal: AtomicU64,
    pub(super) registration_active: AtomicBool,
}

static BEGINNER_GRID_WORK: OnceLock<Mutex<HashMap<ProjectId, Arc<BeginnerGridWork>>>> =
    OnceLock::new();

pub(super) fn beginner_grid_work() -> &'static Mutex<HashMap<ProjectId, Arc<BeginnerGridWork>>> {
    BEGINNER_GRID_WORK.get_or_init(|| Mutex::new(HashMap::new()))
}

#[must_use = "dropping the registration completes only its exact grid work"]
pub(super) struct BeginnerGridWorkRegistration {
    work: Arc<BeginnerGridWork>,
}

impl Drop for BeginnerGridWorkRegistration {
    fn drop(&mut self) {
        // Serialize release with registry pruning/registration. The registry
        // intentionally retains exact terminal grid work as queryable history.
        let _entries = lock_recovering_registry_v1(beginner_grid_work());
        let _ = self
            .work
            .terminal
            .compare_exchange(0, 3, Ordering::AcqRel, Ordering::Acquire);
        self.work
            .registration_active
            .store(false, Ordering::Release);
    }
}

fn beginner_grid_work_is_fresh_v1(work: &BeginnerGridWork) -> bool {
    work.terminal.load(Ordering::Acquire) == 0
        && !work.cancelled.load(Ordering::Acquire)
        && work.enumerated.load(Ordering::Acquire) == 0
        && work.global_checked.load(Ordering::Acquire) == 0
        && work.refinement_iterations.load(Ordering::Acquire) == 0
}

pub(super) fn register_beginner_grid_work_v1(
    request_generation_id: ProjectId,
    work: &Arc<BeginnerGridWork>,
) -> Result<BeginnerGridWorkRegistration, String> {
    let mut registry = lock_recovering_registry_v1(beginner_grid_work());
    registry.retain(|_, existing| existing.registration_active.load(Ordering::Acquire));
    if registry.contains_key(&request_generation_id) {
        return Err("grid_generation_reused".to_owned());
    }
    let claim =
        ActiveWorkRegistrationClaimV1::try_claim(&work.registration_active, "grid_work_reused")?;
    if !beginner_grid_work_is_fresh_v1(work) {
        return Err("grid_work_not_fresh".to_owned());
    }
    if registry.len() >= MAX_BEGINNER_GRID_WORK_REGISTRATIONS_V1 || registry.try_reserve(1).is_err()
    {
        return Err("grid_registry_resource_limit".to_owned());
    }
    let registration_work = Arc::clone(work);
    registry.insert(request_generation_id, Arc::clone(work));
    claim.commit();
    Ok(BeginnerGridWorkRegistration {
        work: registration_work,
    })
}

fn beginner_grid_cancelled_v1(work: &BeginnerGridWork) -> bool {
    work.terminal.load(Ordering::Acquire) == 2 || work.cancelled.load(Ordering::Acquire)
}

pub(super) fn finish_beginner_grid_work_v1<T>(
    request_generation_id: ProjectId,
    work: &BeginnerGridWork,
    result: Result<T, String>,
) -> Result<T, String> {
    let registry = lock_recovering_registry_v1(beginner_grid_work());
    let owns_generation = work.registration_active.load(Ordering::Acquire)
        && registry
            .get(&request_generation_id)
            .is_some_and(|current| std::ptr::eq(Arc::as_ptr(current), work));
    if !owns_generation {
        let _ = work
            .terminal
            .compare_exchange(0, 3, Ordering::AcqRel, Ordering::Acquire);
        return Err("grid_evaluation_failed".to_owned());
    }
    match result {
        Ok(response) => {
            match work
                .terminal
                .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => Ok(response),
                Err(2) => Err("grid_evaluation_cancelled".to_owned()),
                Err(_) => Err("grid_evaluation_failed".to_owned()),
            }
        }
        Err(error) => {
            match work
                .terminal
                .compare_exchange(0, 3, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => Err(error),
                Err(2) => Err("grid_evaluation_cancelled".to_owned()),
                Err(_) => Err(error),
            }
        }
    }
}

pub(super) fn run_registered_beginner_grid_work_v1<T>(
    request_generation_id: ProjectId,
    work: &Arc<BeginnerGridWork>,
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let _registration = register_beginner_grid_work_v1(request_generation_id, work)?;
    finish_beginner_grid_work_v1(request_generation_id, work, operation())
}

#[derive(Debug, Serialize)]
pub(super) struct BeginnerCandidateResponse {
    schema_version: u32,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    requested_candidate_count: u8,
    bulge_treatment: ori_domain::BeginnerBulgeTreatmentV1,
    elasticity_model: ori_domain::BeginnerElasticityModelV1,
    candidates: Vec<ori_domain::BeginnerCandidateScoreV1>,
    generation_status: &'static str,
    generated_plans: Vec<ori_domain::BeginnerGeneratedPlanV1>,
    plan_assessments: Vec<BeginnerGeneratedPlanAssessment>,
    multi_reference_fusion: Option<BeginnerMultiReferenceFusionV1>,
    reference_consensus_analysis: Option<BeginnerReferenceConsensusAnalysisV1>,
}

/// The candidate evaluator deliberately works from this detached copy.  A
/// reference image/model can be expensive to decode, and retaining the live
/// project mutex while doing so would otherwise block unrelated editor work.
/// Asset bytes are copied with the archive's aggregate bounds and fallible
/// reservations before the mutex is released.
pub(super) struct BeginnerCandidateAnalysisSnapshotV1 {
    project: ProjectState,
    expectation: ProjectExpectation,
}

const BEGINNER_CANDIDATE_ANALYSIS_TIMEOUT: std::time::Duration =
    std::time::Duration::from_millis(750);

struct BeginnerCandidateAnalysisControlV1<'a> {
    deadline: std::time::Instant,
    cancelled: Option<&'a AtomicBool>,
    cancelled_error: &'static str,
    deadline_error: &'static str,
}

impl BeginnerCandidateAnalysisControlV1<'_> {
    fn checkpoint(&self) -> Result<(), String> {
        if self
            .cancelled
            .is_some_and(|cancelled| cancelled.load(Ordering::Acquire))
        {
            return Err(self.cancelled_error.to_owned());
        }
        if std::time::Instant::now() >= self.deadline {
            return Err(self.deadline_error.to_owned());
        }
        Ok(())
    }
}

fn beginner_candidate_analysis_control_v1<'a>(
    cancelled: Option<&'a AtomicBool>,
    cancelled_error: &'static str,
    deadline_error: &'static str,
) -> BeginnerCandidateAnalysisControlV1<'a> {
    let now = std::time::Instant::now();
    BeginnerCandidateAnalysisControlV1 {
        deadline: now
            .checked_add(BEGINNER_CANDIDATE_ANALYSIS_TIMEOUT)
            .unwrap_or(now),
        cancelled,
        cancelled_error,
        deadline_error,
    }
}

fn clone_beginner_texture_assets_v1(
    assets: &[ori_formats::ProjectTextureAssetV1],
    control: &BeginnerCandidateAnalysisControlV1<'_>,
) -> Result<Vec<ori_formats::ProjectTextureAssetV1>, String> {
    control.checkpoint()?;
    let total = assets
        .iter()
        .try_fold(0_usize, |total, asset| total.checked_add(asset.bytes.len()));
    if assets.len() > ori_formats::MAX_PROJECT_TEXTURE_ASSETS
        || total.is_none_or(|total| total > MAX_PROJECT_TEXTURE_ASSET_TOTAL_BYTES)
    {
        return Err("beginner_candidate_snapshot_resource_limit".to_owned());
    }
    let mut copied = Vec::new();
    copied
        .try_reserve_exact(assets.len())
        .map_err(|_| "beginner_candidate_snapshot_resource_limit".to_owned())?;
    for asset in assets {
        control.checkpoint()?;
        if asset.bytes.len() > MAX_PROJECT_TEXTURE_ASSET_BYTES {
            return Err("beginner_candidate_snapshot_resource_limit".to_owned());
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(asset.bytes.len())
            .map_err(|_| "beginner_candidate_snapshot_resource_limit".to_owned())?;
        bytes.extend_from_slice(&asset.bytes);
        copied.push(ori_formats::ProjectTextureAssetV1 {
            id: asset.id,
            media_type: asset.media_type,
            bytes,
        });
    }
    control.checkpoint()?;
    Ok(copied)
}

fn clone_beginner_reference_model_assets_v1(
    assets: &[ori_formats::ProjectReferenceModelAssetV1],
    control: &BeginnerCandidateAnalysisControlV1<'_>,
) -> Result<Vec<ori_formats::ProjectReferenceModelAssetV1>, String> {
    control.checkpoint()?;
    let total = assets
        .iter()
        .try_fold(0_usize, |total, asset| total.checked_add(asset.bytes.len()));
    if assets.len() > ori_formats::MAX_PROJECT_REFERENCE_MODEL_ASSETS
        || total
            .is_none_or(|total| total > ori_formats::MAX_PROJECT_REFERENCE_MODEL_ASSET_TOTAL_BYTES)
    {
        return Err("beginner_candidate_snapshot_resource_limit".to_owned());
    }
    let mut copied = Vec::new();
    copied
        .try_reserve_exact(assets.len())
        .map_err(|_| "beginner_candidate_snapshot_resource_limit".to_owned())?;
    for asset in assets {
        control.checkpoint()?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(asset.bytes.len())
            .map_err(|_| "beginner_candidate_snapshot_resource_limit".to_owned())?;
        bytes.extend_from_slice(&asset.bytes);
        copied.push(ori_formats::ProjectReferenceModelAssetV1 {
            id: asset.id,
            bytes,
        });
    }
    control.checkpoint()?;
    Ok(copied)
}

#[cfg(test)]
pub(super) fn capture_beginner_candidate_analysis_snapshot_v1(
    project: &ProjectState,
    expectation: ProjectExpectation,
) -> Result<BeginnerCandidateAnalysisSnapshotV1, String> {
    let control = beginner_candidate_analysis_control_v1(
        None,
        "beginner_candidate_cancelled",
        "beginner_candidate_deadline_exceeded",
    );
    capture_beginner_candidate_analysis_snapshot_with_control_v1(project, expectation, &control)
}

fn capture_beginner_candidate_analysis_snapshot_with_control_v1(
    project: &ProjectState,
    expectation: ProjectExpectation,
    control: &BeginnerCandidateAnalysisControlV1<'_>,
) -> Result<BeginnerCandidateAnalysisSnapshotV1, String> {
    control.checkpoint()?;
    ensure_project_expectation(project, expectation)?;
    let texture_assets = clone_beginner_texture_assets_v1(&project.texture_assets, control)?;
    let reference_model_assets =
        clone_beginner_reference_model_assets_v1(&project.reference_model_assets, control)?;
    control.checkpoint()?;
    // Candidate analysis needs only the current geometry, paper, underlays,
    // and beginner profile. Rebuilding that view avoids copying undo/redo
    // history while the live project mutex is held.
    let pattern = project.editor.pattern().clone();
    control.checkpoint()?;
    let paper = project.editor.paper().clone();
    let underlays = project.editor.underlays().clone();
    let profile = project.editor.beginner_design_profile().clone();
    control.checkpoint()?;
    let mut editor = EditorState::with_paper(pattern, paper);
    editor.restore_underlays(underlays);
    editor
        .restore_beginner_design_profile(profile)
        .map_err(|_| "beginner_candidate_snapshot_invalid".to_owned())?;
    control.checkpoint()?;
    Ok(BeginnerCandidateAnalysisSnapshotV1 {
        project: ProjectState {
            instance_id: project.instance_id,
            project_id: project.project_id,
            name: String::new(),
            current_path: None,
            editor,
            applied_pose_authority: CurrentAppliedPoseAuthority::default(),
            current_layer_evidence: None,
            numeric_expressions: ProjectNumericExpressions::default(),
            texture_assets,
            reference_model_assets,
            material_void_evidence: Default::default(),
            saved_revision: None,
            saved_document: None,
            saved_speculative_unproven_state: None,
        },
        expectation,
    })
}

pub(super) fn beginner_candidate_snapshot_is_current_v1(
    project: &ProjectState,
    snapshot: &BeginnerCandidateAnalysisSnapshotV1,
) -> Result<(), String> {
    ensure_project_expectation(project, snapshot.expectation)
}

#[derive(Debug, Clone, Serialize)]
struct BeginnerReferenceConsensusPairV1 {
    left_asset_id: AssetId,
    right_asset_id: AssetId,
    component_error: u8,
    normalized_extent_error: u8,
    branch_error: u8,
    agreement_score: u8,
    disagrees: bool,
    pair_digest_sha256: [u8; 32],
    left_component_count: u8,
    right_component_count: u8,
    left_normalized_extents: [u8; 2],
    right_normalized_extents: [u8; 2],
    left_branch_count: u8,
    right_branch_count: u8,
}

#[derive(Debug, Clone, Serialize)]
struct BeginnerReferenceConsensusAnalysisV1 {
    schema_version: u32,
    revision: u64,
    source_count: u8,
    excluded_asset_id: Option<AssetId>,
    pair_count: u8,
    disagreement_count: u8,
    agreement_score: u8,
    apply_allowed: bool,
    reason: &'static str,
    pairs: Vec<BeginnerReferenceConsensusPairV1>,
}

#[derive(Clone, Copy)]
struct BeginnerReferenceShapeDescriptorV1 {
    asset_id: AssetId,
    sha256: [u8; 32],
    components: u8,
    extents: [u64; 2],
    branches: u8,
}

fn beginner_reference_consensus_analysis_v1(
    project: &ProjectState,
    progress: Option<(&AppHandle, ProjectId, &ReferenceConsensusWorkV1)>,
) -> Option<BeginnerReferenceConsensusAnalysisV1> {
    let now = std::time::Instant::now();
    beginner_reference_consensus_analysis_with_deadline_v1(
        project,
        progress,
        now.checked_add(BEGINNER_CANDIDATE_ANALYSIS_TIMEOUT)
            .unwrap_or(now),
    )
}

fn beginner_reference_consensus_analysis_with_deadline_v1(
    project: &ProjectState,
    progress: Option<(&AppHandle, ProjectId, &ReferenceConsensusWorkV1)>,
    deadline: std::time::Instant,
) -> Option<BeginnerReferenceConsensusAnalysisV1> {
    use ori_domain::BeginnerReferenceBindingKindV1::{Image, ReferenceModel};
    let stopped = || {
        std::time::Instant::now() >= deadline
            || progress.is_some_and(|(_, _, work)| work.cancelled.load(Ordering::Acquire))
    };
    if stopped() {
        return None;
    }
    let profile = project.editor.beginner_design_profile();
    let consensus = profile.reference_consensus_v1.as_ref()?;
    let mut descriptors = Vec::with_capacity(consensus.bindings.len());
    let total_assets = consensus
        .bindings
        .len()
        .saturating_sub(usize::from(consensus.excluded_asset_id.is_some()))
        as u8;
    let total_pairs =
        (usize::from(total_assets) * usize::from(total_assets.saturating_sub(1)) / 2).min(6) as u8;
    for binding in &consensus.bindings {
        if stopped() {
            return None;
        }
        if consensus.excluded_asset_id == Some(binding.asset_id) {
            continue;
        }
        let descriptor = match binding.kind {
            Image => {
                let asset = project
                    .texture_assets
                    .iter()
                    .find(|asset| asset.id == binding.asset_id)?;
                let hash: [u8; 32] = sha2::Sha256::digest(&asset.bytes).into();
                if hash != binding.sha256 {
                    return None;
                }
                let (width, height, rgba) =
                    beginner_recognition::decode_general_image(&asset.bytes).ok()?;
                let outlines =
                    ori_domain::analyze_outline_candidates_rgba_v1(width, height, &rgba).ok()?;
                let min_x = outlines.iter().map(|item| item.bounds.min_x).min()?;
                let max_x = outlines.iter().map(|item| item.bounds.max_x).max()?;
                let min_y = outlines.iter().map(|item| item.bounds.min_y).min()?;
                let max_y = outlines.iter().map(|item| item.bounds.max_y).max()?;
                let components = u8::try_from(outlines.len()).ok()?;
                BeginnerReferenceShapeDescriptorV1 {
                    asset_id: binding.asset_id,
                    sha256: hash,
                    components,
                    extents: [u64::from(max_x - min_x + 1), u64::from(max_y - min_y + 1)],
                    branches: components.saturating_mul(2).saturating_sub(1),
                }
            }
            ReferenceModel => {
                let asset = project
                    .reference_model_assets
                    .iter()
                    .find(|asset| asset.id == binding.asset_id)?;
                let hash: [u8; 32] = sha2::Sha256::digest(&asset.bytes).into();
                if hash != binding.sha256 {
                    return None;
                }
                let geometry = ori_formats::read_reference_glb_geometry_v1(&asset.bytes).ok()?;
                let suggestion = derive_reference_model_suggestion_v1(
                    binding.asset_id,
                    &geometry,
                    profile.generation_constraints.target_category,
                    &profile.generation_constraints.target_parts,
                )
                .ok()?;
                BeginnerReferenceShapeDescriptorV1 {
                    asset_id: binding.asset_id,
                    sha256: hash,
                    components: suggestion.component_count,
                    extents: [
                        u64::from(suggestion.principal_axis_extents_tenths_mm[0]),
                        u64::from(suggestion.principal_axis_extents_tenths_mm[1]),
                    ],
                    branches: u8::try_from(suggestion.stick_bars.len()).ok()?,
                }
            }
        };
        descriptors.push(descriptor);
        if let Some((app, request_generation_id, work)) = progress {
            if work.cancelled.load(Ordering::Acquire) {
                return None;
            }
            let _ = app.emit("reference-consensus-progress-v1", serde_json::json!({
                "request_generation_id": request_generation_id, "processed_assets": descriptors.len(),
                "total_assets": total_assets, "processed_pairs": 0, "total_pairs": total_pairs,
                "authorizes_project_mutation": false
            }));
        }
    }
    if stopped() {
        return None;
    }
    if descriptors.len() < 2 {
        return None;
    }
    let normalize = |values: [u64; 2]| {
        let major = values[0].max(values[1]).max(1);
        values.map(|v| v.saturating_mul(100) / major)
    };
    let mut pairs = Vec::new();
    for left in 0..descriptors.len() {
        for right in (left + 1)..descriptors.len() {
            if stopped() {
                return None;
            }
            if pairs.len() == 6 {
                return None;
            }
            let a = descriptors[left];
            let b = descriptors[right];
            let component_error = a.components.abs_diff(b.components);
            let branch_error = a.branches.abs_diff(b.branches);
            let an = normalize(a.extents);
            let bn = normalize(b.extents);
            let extent_error = an[0].abs_diff(bn[0]).max(an[1].abs_diff(bn[1])).min(100) as u8;
            let disagrees = component_error > 1 || branch_error > 2 || extent_error > 20;
            let score = 100_u8.saturating_sub(
                extent_error
                    .saturating_mul(2)
                    .saturating_add(component_error.saturating_mul(20))
                    .saturating_add(branch_error.saturating_mul(10))
                    .min(100),
            );
            let mut digest = sha2::Sha256::new();
            digest.update(b"origami2-reference-consensus-pair-v1\0");
            digest.update(a.asset_id.canonical_bytes());
            digest.update(a.sha256);
            digest.update(b.asset_id.canonical_bytes());
            digest.update(b.sha256);
            digest.update([
                component_error,
                extent_error,
                branch_error,
                score,
                u8::from(disagrees),
            ]);
            pairs.push(BeginnerReferenceConsensusPairV1 {
                left_asset_id: a.asset_id,
                right_asset_id: b.asset_id,
                component_error,
                normalized_extent_error: extent_error,
                branch_error,
                agreement_score: score,
                disagrees,
                pair_digest_sha256: digest.finalize().into(),
                left_component_count: a.components,
                right_component_count: b.components,
                left_normalized_extents: [an[0] as u8, an[1] as u8],
                right_normalized_extents: [bn[0] as u8, bn[1] as u8],
                left_branch_count: a.branches,
                right_branch_count: b.branches,
            });
            if let Some((app, request_generation_id, work)) = progress {
                if work.cancelled.load(Ordering::Acquire) {
                    return None;
                }
                let _ = app.emit("reference-consensus-progress-v1", serde_json::json!({
                    "request_generation_id": request_generation_id, "processed_assets": total_assets,
                    "total_assets": total_assets, "processed_pairs": pairs.len(), "total_pairs": total_pairs,
                    "authorizes_project_mutation": false
                }));
            }
        }
    }
    let disagreement_count = pairs.iter().filter(|pair| pair.disagrees).count() as u8;
    let agreement_score = pairs
        .iter()
        .map(|pair| u16::from(pair.agreement_score))
        .sum::<u16>()
        / pairs.len() as u16;
    let apply_allowed = disagreement_count < 2;
    if stopped() {
        return None;
    }
    Some(BeginnerReferenceConsensusAnalysisV1 {
        schema_version: 1,
        revision: project.editor.revision(),
        source_count: descriptors.len() as u8,
        excluded_asset_id: consensus.excluded_asset_id,
        pair_count: pairs.len() as u8,
        disagreement_count,
        agreement_score: agreement_score as u8,
        apply_allowed,
        reason: if apply_allowed {
            "reference_consensus_agreement_v1"
        } else {
            "reference_consensus_multiple_disagreements_v1"
        },
        pairs,
    })
}

#[derive(Debug, Clone, Serialize)]
struct BeginnerMultiReferenceFusionV1 {
    revision: u64,
    image_sha256: [u8; 32],
    reference_sha256: [u8; 32],
    source_count: u8,
    image_component_count: u8,
    reference_component_count: u8,
    image_branch_count: u8,
    reference_branch_count: u8,
    normalized_extent_error: u8,
    agreement_score: u8,
    apply_allowed: bool,
    reason: &'static str,
}

fn beginner_multi_reference_fusion_v1(
    project: &ProjectState,
    reference: &BeginnerReferenceModelSuggestionV1,
) -> Option<BeginnerMultiReferenceFusionV1> {
    let underlay = project.editor.underlays().underlays.first()?;
    let image = project
        .texture_assets
        .iter()
        .find(|asset| asset.id == underlay.asset)?;
    let model = project
        .reference_model_assets
        .iter()
        .find(|asset| asset.id == reference.asset_id)?;
    let (width, height, rgba) = beginner_recognition::decode_general_image(&image.bytes).ok()?;
    let outlines = ori_domain::analyze_outline_candidates_rgba_v1(width, height, &rgba).ok()?;
    if outlines.is_empty() || outlines.len() > 8 {
        return None;
    }
    let min_x = outlines.iter().map(|item| item.bounds.min_x).min()?;
    let max_x = outlines.iter().map(|item| item.bounds.max_x).max()?;
    let min_y = outlines.iter().map(|item| item.bounds.min_y).min()?;
    let max_y = outlines.iter().map(|item| item.bounds.max_y).max()?;
    let image_extents = [u64::from(max_x - min_x + 1), u64::from(max_y - min_y + 1)];
    let reference_extents = [
        u64::from(reference.principal_axis_extents_tenths_mm[0]),
        u64::from(reference.principal_axis_extents_tenths_mm[1]),
    ];
    let normalize = |values: [u64; 2]| {
        let major = values[0].max(values[1]).max(1);
        values.map(|value| value.saturating_mul(100) / major)
    };
    let image_normalized = normalize(image_extents);
    let reference_normalized = normalize(reference_extents);
    let extent_error = image_normalized[0]
        .abs_diff(reference_normalized[0])
        .max(image_normalized[1].abs_diff(reference_normalized[1]))
        .min(100) as u8;
    let image_components = outlines.len() as u8;
    let image_branches = image_components.saturating_mul(2).saturating_sub(1);
    let reference_branches = u8::try_from(reference.stick_bars.len()).ok()?;
    let component_error = image_components
        .abs_diff(reference.component_count)
        .saturating_mul(20);
    let branch_error = image_branches
        .abs_diff(reference_branches)
        .saturating_mul(10);
    let agreement_score = 100_u8.saturating_sub(
        extent_error
            .saturating_mul(2)
            .saturating_add(component_error)
            .saturating_add(branch_error)
            .min(100),
    );
    let apply_allowed = extent_error <= 20
        && image_components.abs_diff(reference.component_count) <= 1
        && image_branches.abs_diff(reference_branches) <= 2;
    Some(BeginnerMultiReferenceFusionV1 {
        revision: project.editor.revision(),
        image_sha256: sha2::Sha256::digest(&image.bytes).into(),
        reference_sha256: sha2::Sha256::digest(&model.bytes).into(),
        source_count: 2,
        image_component_count: image_components,
        reference_component_count: reference.component_count,
        image_branch_count: image_branches,
        reference_branch_count: reference_branches,
        normalized_extent_error: extent_error,
        agreement_score,
        apply_allowed,
        reason: if apply_allowed {
            "image_glb_agreement_v1"
        } else {
            "image_glb_disagreement_v1"
        },
    })
}

#[derive(Debug, Serialize)]
pub(super) struct BeginnerGeneratedPlanAssessment {
    pub(super) kind: ori_domain::BeginnerGeneratedPlanKindV1,
    pub(super) expected_candidate_edge_id: EdgeId,
    pub(super) proof_scope: &'static str,
    pub(super) apply_allowed: bool,
    pub(super) reason: &'static str,
    pub(super) shape_approximation_score: Option<u8>,
    pub(super) shape_difference_reason: Option<&'static str>,
    pub(super) component_shape_comparison: Option<BeginnerComponentShapeComparisonV1>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub(super) struct BeginnerComponentShapeComparisonV1 {
    pub(super) component_count: u8,
    pub(super) matched_branch_count: u8,
    pub(super) work_units: u8,
    pub(super) extent_score: u8,
    pub(super) branch_score: u8,
    pub(super) bridge_score: u8,
    pub(super) extent_weight: u8,
    pub(super) branch_weight: u8,
    pub(super) bridge_weight: u8,
}

fn component_shape_comparison_v1(
    plan: &ori_domain::BeginnerGeneratedPlanV1,
    reference: &BeginnerReferenceModelSuggestionV1,
) -> Option<BeginnerComponentShapeComparisonV1> {
    if !(2..=8).contains(&reference.component_count)
        || !reference.inferred_component_bridges
        || reference.stick_bars.len() > 16
        || plan.skeleton_segments.len() > 16
    {
        return None;
    }
    let length = |dx: i64, dy: i64, dz: i64| {
        (dx.saturating_mul(dx)
            .saturating_add(dy.saturating_mul(dy))
            .saturating_add(dz.saturating_mul(dz)) as u64)
            .isqrt()
    };
    let mut targets = reference
        .stick_bars
        .iter()
        .map(|bar| {
            length(
                i64::from(bar.end_tenths_mm[0]) - i64::from(bar.start_tenths_mm[0]),
                i64::from(bar.end_tenths_mm[1]) - i64::from(bar.start_tenths_mm[1]),
                i64::from(bar.end_tenths_mm[2]) - i64::from(bar.start_tenths_mm[2]),
            )
        })
        .collect::<Vec<_>>();
    let mut branches = plan
        .skeleton_segments
        .iter()
        .map(|bar| {
            length(
                i64::from(bar.end.x_tenths_mm) - i64::from(bar.start.x_tenths_mm),
                i64::from(bar.end.y_tenths_mm) - i64::from(bar.start.y_tenths_mm),
                0,
            )
        })
        .collect::<Vec<_>>();
    targets.sort_unstable_by(|a, b| b.cmp(a));
    branches.sort_unstable_by(|a, b| b.cmp(a));
    let branch_count = usize::from(reference.component_count).min(targets.len());
    let targets = &targets[..branch_count];
    if targets.is_empty() || branches.is_empty() {
        return None;
    }
    let major = targets
        .iter()
        .chain(branches.iter())
        .copied()
        .max()
        .unwrap_or(1)
        .max(1);
    let mut used = vec![false; branches.len()];
    let mut error = 0_u64;
    let mut matched = 0_u8;
    let mut work = 0_u8;
    for target in targets {
        let mut best = None;
        for (index, candidate) in branches.iter().enumerate() {
            if used[index] {
                continue;
            }
            work = work.checked_add(1)?;
            if work > 64 {
                return None;
            }
            let key = (target.abs_diff(*candidate), index);
            if best.is_none_or(|(current, _)| key < current) {
                best = Some((key, index));
            }
        }
        if let Some((key, index)) = best {
            used[index] = true;
            error = error.saturating_add(key.0);
            matched += 1;
        }
    }
    let branch_score = 100_u64.saturating_sub(
        error.saturating_mul(100) / major.saturating_mul(targets.len() as u64).max(1),
    ) as u8;
    let target_extent = reference
        .principal_axis_extents_tenths_mm
        .iter()
        .copied()
        .max()
        .unwrap_or(1)
        .max(1) as u64;
    let candidate_extent = branches.iter().copied().max().unwrap_or(0);
    let extent_score = 100_u64.saturating_sub(
        target_extent.abs_diff(candidate_extent).saturating_mul(100)
            / target_extent.max(candidate_extent).max(1),
    ) as u8;
    let target_bridges = u64::from(reference.component_count - 1);
    let candidate_bridges = branches.len().saturating_sub(branch_count) as u64;
    let bridge_score = 100_u64.saturating_sub(
        target_bridges
            .abs_diff(candidate_bridges)
            .saturating_mul(100)
            / target_bridges.max(candidate_bridges).max(1),
    ) as u8;
    Some(BeginnerComponentShapeComparisonV1 {
        component_count: reference.component_count,
        matched_branch_count: matched,
        work_units: work,
        extent_score,
        branch_score,
        bridge_score,
        extent_weight: 45,
        branch_weight: 35,
        bridge_weight: 20,
    })
}

fn compare_plan_to_reference_model_v1(
    plan: &ori_domain::BeginnerGeneratedPlanV1,
    reference: &BeginnerReferenceModelSuggestionV1,
) -> (u8, &'static str) {
    if let Some(parts) = component_shape_comparison_v1(plan, reference) {
        let score = (u16::from(parts.extent_score) * u16::from(parts.extent_weight)
            + u16::from(parts.branch_score) * u16::from(parts.branch_weight)
            + u16::from(parts.bridge_score) * u16::from(parts.bridge_weight))
            / 100;
        return (score as u8, "component_aware_quantized_shape_v1");
    }
    if let Some(score) = bounded_folded_pose_landmark_score_v1(plan, reference) {
        return (score, "bounded_folded_pose_landmarks_v1");
    }
    let mut min = [f64::INFINITY; 2];
    let mut max = [f64::NEG_INFINITY; 2];
    for vertex in &plan.crease_pattern.vertices {
        min[0] = min[0].min(vertex.position.x);
        min[1] = min[1].min(vertex.position.y);
        max[0] = max[0].max(vertex.position.x);
        max[1] = max[1].max(vertex.position.y);
    }
    let candidate = [
        ((max[0] - min[0]).max(0.0) * 10.0).round() as u64,
        ((max[1] - min[1]).max(0.0) * 10.0).round() as u64,
        0,
    ];
    let target = std::array::from_fn::<_, 3, _>(|axis| {
        u64::try_from(
            reference.bbox_max_tenths_mm[axis].saturating_sub(reference.bbox_min_tenths_mm[axis]),
        )
        .unwrap_or(0)
    });
    let normalize = |extents: [u64; 3]| {
        let major = *extents.iter().max().unwrap_or(&1).max(&1);
        extents.map(|extent| extent.saturating_mul(1000) / major)
    };
    let candidate_normalized = normalize(candidate);
    let target_normalized = normalize(target);
    let bbox_difference = candidate_normalized
        .iter()
        .zip(target_normalized)
        .map(|(left, right)| left.abs_diff(right))
        .sum::<u64>()
        / 3;
    let candidate_major = *candidate.iter().max().unwrap_or(&1).max(&1);
    let target_major = *target.iter().max().unwrap_or(&1).max(&1);
    let candidate_area_ratio = candidate[0]
        .saturating_mul(candidate[1])
        .saturating_mul(1000)
        / candidate_major.saturating_mul(candidate_major).max(1);
    let target_area_ratio = reference.surface_area_milli.saturating_mul(100_000_000)
        / target_major.saturating_mul(target_major).max(1);
    let area_difference = candidate_area_ratio.abs_diff(target_area_ratio.min(10_000));
    let candidate_major_axis = (0..3).max_by_key(|axis| candidate[*axis]).unwrap_or(0);
    let target_major_axis = (0..3).max_by_key(|axis| target[*axis]).unwrap_or(0);
    let axis_penalty = if candidate_major_axis == target_major_axis {
        0
    } else {
        20
    };
    let score = 100_u64.saturating_sub(
        (bbox_difference / 10)
            .saturating_add(area_difference / 100)
            .saturating_add(axis_penalty),
    );
    (
        u8::try_from(score).unwrap_or(0),
        "crease_preview_has_no_surface_mesh",
    )
}

pub(super) const MAX_BEGINNER_FOLDED_LANDMARKS_V1: usize = 256;
const MAX_BEGINNER_FOLDED_COLLISION_PAIRS_V1: usize = 32_640;

/// Deterministic bounded forward approximation used only for candidate ranking.
/// It produces body/local 3D landmarks from the generated crease graph; it is
/// not mutation authority or a foldability/collision certificate.
pub(super) fn bounded_folded_pose_landmark_score_v1(
    plan: &ori_domain::BeginnerGeneratedPlanV1,
    reference: &BeginnerReferenceModelSuggestionV1,
) -> Option<u8> {
    let vertices = &plan.crease_pattern.vertices;
    if vertices.is_empty() || vertices.len() > MAX_BEGINNER_FOLDED_LANDMARKS_V1 {
        return None;
    }
    let pair_count = vertices
        .len()
        .checked_mul(vertices.len().saturating_sub(1))?
        / 2;
    if pair_count > MAX_BEGINNER_FOLDED_COLLISION_PAIRS_V1 {
        return None;
    }
    let mut incident = HashMap::<VertexId, i16>::with_capacity(vertices.len());
    for edge in &plan.crease_pattern.edges {
        let sign = match edge.kind {
            EdgeKind::Mountain => 1_i16,
            EdgeKind::Valley => -1_i16,
            _ => 0_i16,
        };
        *incident.entry(edge.start).or_default() += sign;
        *incident.entry(edge.end).or_default() += sign;
    }
    let mut min = [i64::MAX; 3];
    let mut max = [i64::MIN; 3];
    let mut landmarks = Vec::with_capacity(vertices.len());
    for vertex in vertices {
        let x = (vertex.position.x * 10.0).round() as i64;
        let y = (vertex.position.y * 10.0).round() as i64;
        let radial = x
            .unsigned_abs()
            .saturating_add(y.unsigned_abs())
            .min(10_000) as i64;
        let z = i64::from(*incident.get(&vertex.id).unwrap_or(&0)) * radial / 8;
        let point = [x, y, z];
        for axis in 0..3 {
            min[axis] = min[axis].min(point[axis]);
            max[axis] = max[axis].max(point[axis]);
        }
        landmarks.push(point);
    }
    for (index, left) in landmarks.iter().enumerate() {
        for right in &landmarks[index + 1..] {
            if left[0] == right[0] && left[1] == right[1] && left[2] != right[2] {
                return None;
            }
        }
    }
    let mut normal = [0_i128; 3];
    let mut doubled_area_units = 0_u128;
    if landmarks.len() >= 3 {
        let origin = landmarks[0];
        for pair in landmarks[1..].windows(2) {
            let a = std::array::from_fn::<_, 3, _>(|axis| i128::from(pair[0][axis] - origin[axis]));
            let b = std::array::from_fn::<_, 3, _>(|axis| i128::from(pair[1][axis] - origin[axis]));
            let cross = [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ];
            for axis in 0..3 {
                normal[axis] = normal[axis].saturating_add(cross[axis]);
            }
            doubled_area_units = doubled_area_units
                .saturating_add(cross.iter().map(|value| value.unsigned_abs()).sum::<u128>());
        }
        if doubled_area_units == 0 {
            return None;
        }
    }
    let candidate =
        std::array::from_fn::<_, 3, _>(|axis| max[axis].saturating_sub(min[axis]) as u64);
    let target = std::array::from_fn::<_, 3, _>(|axis| {
        i64::from(reference.bbox_max_tenths_mm[axis])
            .saturating_sub(i64::from(reference.bbox_min_tenths_mm[axis])) as u64
    });
    let major = *candidate
        .iter()
        .chain(target.iter())
        .max()
        .unwrap_or(&1)
        .max(&1);
    let normalized = |extent: u64| extent.saturating_mul(1_000_000) / major;
    let bbox_hausdorff = (0..3)
        .map(|axis| normalized(candidate[axis]).abs_diff(normalized(target[axis])))
        .max()
        .unwrap_or(1_000_000);
    let landmark_hausdorff = if reference.surface_landmarks_tenths_mm.is_empty() {
        bbox_hausdorff
    } else {
        let squared = |left: [i64; 3], right: [i64; 3]| {
            (0..3)
                .map(|axis| left[axis].abs_diff(right[axis]).saturating_pow(2))
                .sum::<u64>()
        };
        let target_landmarks = reference
            .surface_landmarks_tenths_mm
            .iter()
            .map(|point| std::array::from_fn(|axis| i64::from(point[axis])))
            .collect::<Vec<_>>();
        let directed = |from: &[[i64; 3]], to: &[[i64; 3]]| {
            from.iter()
                .map(|point| {
                    to.iter()
                        .map(|target| squared(*point, *target))
                        .min()
                        .unwrap_or(u64::MAX)
                })
                .max()
                .unwrap_or(u64::MAX)
        };
        let squared_error =
            directed(&landmarks, &target_landmarks).max(directed(&target_landmarks, &landmarks));
        squared_error
            .isqrt()
            .saturating_mul(1_000_000)
            .checked_div(major)
            .unwrap_or(1_000_000)
    };
    let hausdorff = bbox_hausdorff.max(landmark_hausdorff.min(1_000_000));
    let depth_error = normalized(candidate[2]).abs_diff(normalized(target[2]));
    let candidate_bulge = landmarks.iter().filter(|point| point[2] != 0).count() as u64;
    let target_bulge = reference.protrusions.len() as u64;
    let bulge_error = candidate_bulge
        .abs_diff(target_bulge)
        .saturating_mul(100_000)
        / target_bulge.max(1);
    let reference_normal = reference.dominant_normal_milli.map(i128::from);
    let dot = normal
        .iter()
        .zip(reference_normal)
        .map(|(a, b)| a.saturating_mul(b))
        .sum::<i128>();
    let normal_l1 = normal
        .iter()
        .map(|value| value.unsigned_abs())
        .sum::<u128>()
        .max(1);
    let reference_l1 = reference_normal
        .iter()
        .map(|value| value.unsigned_abs())
        .sum::<u128>()
        .max(1);
    let alignment_millionths =
        dot.max(0) as u128 * 1_000_000 / normal_l1.saturating_mul(reference_l1);
    let orientation_error =
        1_000_000_u64.saturating_sub(alignment_millionths.min(1_000_000) as u64);
    let candidate_area = u64::try_from(doubled_area_units / 2).unwrap_or(u64::MAX);
    let target_area = reference.surface_area_milli.max(1);
    let coverage_error = candidate_area
        .abs_diff(target_area)
        .saturating_mul(1_000_000)
        / candidate_area.max(target_area).max(1);
    let combined = hausdorff.saturating_mul(35) / 100
        + depth_error.saturating_mul(25) / 100
        + bulge_error.min(1_000_000).saturating_mul(15) / 100
        + orientation_error.saturating_mul(15) / 100
        + coverage_error.min(1_000_000).saturating_mul(10) / 100;
    Some(u8::try_from(100_u64.saturating_sub(combined.min(1_000_000) / 10_000)).unwrap_or(0))
}

pub(super) fn preset_weighted_refinement_score_v1(
    plan: &ori_domain::BeginnerGeneratedPlanV1,
    reference: &BeginnerReferenceModelSuggestionV1,
    profile: &ori_domain::BeginnerDesignProfileV1,
) -> u8 {
    let shape_3d = bounded_folded_pose_landmark_score_v1(plan, reference).unwrap_or(0);
    let shape_2d = {
        let mut min = [f64::INFINITY; 2];
        let mut max = [f64::NEG_INFINITY; 2];
        for vertex in &plan.crease_pattern.vertices {
            min[0] = min[0].min(vertex.position.x);
            min[1] = min[1].min(vertex.position.y);
            max[0] = max[0].max(vertex.position.x);
            max[1] = max[1].max(vertex.position.y);
        }
        let target_x = i64::from(reference.bbox_max_tenths_mm[0])
            .saturating_sub(i64::from(reference.bbox_min_tenths_mm[0]))
            .unsigned_abs();
        let target_y = i64::from(reference.bbox_max_tenths_mm[1])
            .saturating_sub(i64::from(reference.bbox_min_tenths_mm[1]))
            .unsigned_abs();
        let candidate_x = ((max[0] - min[0]).max(0.0) * 10.0).round() as u64;
        let candidate_y = ((max[1] - min[1]).max(0.0) * 10.0).round() as u64;
        let major = target_x
            .max(target_y)
            .max(candidate_x)
            .max(candidate_y)
            .max(1);
        u8::try_from(
            100_u64.saturating_sub(
                candidate_x
                    .abs_diff(target_x)
                    .max(candidate_y.abs_diff(target_y))
                    * 100
                    / major,
            ),
        )
        .unwrap_or(0)
    };
    let shape = (u16::from(shape_2d) * 35 + u16::from(shape_3d) * 65) / 100;
    let foldability = 100_u16.saturating_sub(
        u16::try_from(plan.crease_pattern.edges.len().saturating_mul(2)).unwrap_or(100),
    );
    let step = 100_u16.saturating_sub(
        u16::try_from(plan.instruction_codes.len().saturating_mul(5)).unwrap_or(100),
    );
    let paper =
        100_u16.saturating_sub(u16::try_from(plan.crease_pattern.vertices.len()).unwrap_or(100));
    let total = shape * u16::from(profile.shape_fidelity_weight)
        + foldability * u16::from(profile.foldability_weight)
        + step * u16::from(profile.step_count_weight)
        + paper * u16::from(profile.paper_efficiency_weight);
    u8::try_from(total / 100).unwrap_or(0)
}

fn compare_flat_surface_to_reference_model_v1(
    surface: &ori_core::CertifiedFlatSurfaceV1,
    reference: &BeginnerReferenceModelSuggestionV1,
) -> u8 {
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    let mut area = 0.0_f64;
    for face in &surface.faces {
        for point in &face.boundary {
            for axis in 0..3 {
                min[axis] = min[axis].min(point[axis]);
                max[axis] = max[axis].max(point[axis]);
            }
        }
        area += face
            .boundary
            .iter()
            .zip(face.boundary.iter().cycle().skip(1))
            .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
            .sum::<f64>()
            .abs()
            * 0.5;
    }
    let candidate = std::array::from_fn::<_, 3, _>(|axis| {
        ((max[axis] - min[axis]).max(0.0) * 10.0).round() as u64
    });
    let target = std::array::from_fn::<_, 3, _>(|axis| {
        u64::try_from(
            reference.bbox_max_tenths_mm[axis].saturating_sub(reference.bbox_min_tenths_mm[axis]),
        )
        .unwrap_or(0)
    });
    let normalize = |v: [u64; 3]| {
        let major = *v.iter().max().unwrap_or(&1).max(&1);
        v.map(|x| x.saturating_mul(1000) / major)
    };
    let bbox = normalize(candidate)
        .iter()
        .zip(normalize(target))
        .map(|(a, b)| a.abs_diff(b))
        .sum::<u64>()
        / 3;
    let cm = *candidate.iter().max().unwrap_or(&1).max(&1);
    let tm = *target.iter().max().unwrap_or(&1).max(&1);
    let ca = ((area * 100.0).round() as u64).saturating_mul(1000) / cm.saturating_mul(cm).max(1);
    let ta =
        reference.surface_area_milli.saturating_mul(100_000_000) / tm.saturating_mul(tm).max(1);
    let axis = if (0..3).max_by_key(|i| candidate[*i]) == (0..3).max_by_key(|i| target[*i]) {
        0
    } else {
        20
    };
    u8::try_from(100_u64.saturating_sub(bbox / 10 + ca.abs_diff(ta.min(10_000)) / 100 + axis))
        .unwrap_or(0)
}

struct BeginnerGlobalFoldabilityDeadline<'a> {
    deadline: std::time::Instant,
    cancelled: Option<&'a AtomicBool>,
}

const MAX_BEGINNER_FOLD_PATH_CREASES_V1: usize = 256;

pub(super) fn certify_beginner_fold_path_v1(
    plan: &ori_domain::BeginnerGeneratedPlanV1,
    paper: &Paper,
    candidate_pattern: &CreasePattern,
    topology: &TopologySnapshot,
) -> Option<[u8; 32]> {
    certify_beginner_fold_path_with_control_v1(
        plan,
        paper,
        candidate_pattern,
        topology,
        &ori_collision::CooperativeOperationControlV1::unbounded(),
    )
}

fn certify_beginner_fold_path_with_control_v1(
    plan: &ori_domain::BeginnerGeneratedPlanV1,
    paper: &Paper,
    candidate_pattern: &CreasePattern,
    topology: &TopologySnapshot,
    control: &ori_collision::CooperativeOperationControlV1<'_>,
) -> Option<[u8; 32]> {
    if control.checkpoint().is_err() {
        return None;
    }
    let creases = plan
        .crease_pattern
        .edges
        .iter()
        .filter(|edge| matches!(edge.kind, EdgeKind::Mountain | EdgeKind::Valley))
        .collect::<Vec<_>>();
    if creases.is_empty() || creases.len() > MAX_BEGINNER_FOLD_PATH_CREASES_V1 {
        return None;
    }
    let mut ids = HashSet::with_capacity(creases.len());
    if creases.iter().any(|edge| !ids.insert(edge.id)) {
        return None;
    }
    let mut ordered = creases
        .iter()
        .map(|edge| (edge.id.canonical_bytes(), edge.kind, edge.start, edge.end))
        .collect::<Vec<_>>();
    ordered.sort_unstable_by_key(|record| record.0);
    let tree_model = ori_kinematics::MaterialTreeKinematicsModel::prepare(
        candidate_pattern,
        paper,
        topology,
        ori_kinematics::TreeKinematicsLimits::default(),
    );
    let (certificate_model, requested_angle_degrees) = if let Ok(model) = tree_model {
        let initial = ori_kinematics::CanonicalHingeAngles::new(
            model
                .hinges()
                .iter()
                .map(|hinge| ori_kinematics::HingeAngle::new(hinge.edge(), 0.0))
                .collect::<Result<Vec<_>, _>>()
                .ok()?,
        )
        .ok()?;
        let mut fixed_faces = model.face_ids().to_vec();
        fixed_faces.sort_unstable_by_key(|face| face.canonical_bytes());
        let initial_pose = model.solve(fixed_faces.first().copied(), &initial).ok()?;
        let moving_hinges = model
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>();
        let requested = if model.hinges().len() == 1 || paper.thickness_mm == 0.0 {
            90.0
        } else {
            0.001
        };
        let path = ori_collision::diagnose_collective_hinge_path_v1(
            &model,
            &initial_pose,
            &moving_hinges,
            requested,
            paper.thickness_mm,
            ori_collision::StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .ok()?;
        if control.checkpoint().is_err() {
            return None;
        }
        (path.continuous_certificate_model_id()?, requested)
    } else {
        let geometry = ori_kinematics::MaterialHingeGraphGeometry::prepare(
            candidate_pattern,
            paper,
            topology,
            ori_kinematics::TreeKinematicsLimits::default(),
        )
        .ok()?;
        let audit = ori_kinematics::MaterialHingeGraphAudit::prepare(
            topology,
            ori_kinematics::TreeKinematicsLimits::default(),
        )
        .ok()?;
        let mut fixed_faces = geometry.face_ids().to_vec();
        fixed_faces.sort_unstable_by_key(|face| face.canonical_bytes());
        let requested = 1.0e-8;
        let certificate_model = fixed_faces.into_iter().find_map(|fixed_face| {
            if control.checkpoint().is_err() {
                return None;
            }
            let generated = if geometry.hinges().len() == 4 {
                ori_kinematics::generate_kawasaki_120_120_60_60_path_candidate_v1(
                    &geometry,
                    &audit,
                    fixed_face,
                    ori_kinematics::CycleScheduleLimitsV1::default(),
                )
                .ok()?
            } else {
                let initial = ori_kinematics::CanonicalHingeAngles::new(
                    geometry
                        .hinges()
                        .iter()
                        .map(|hinge| ori_kinematics::HingeAngle::new(hinge.edge(), 0.0))
                        .collect::<Result<Vec<_>, _>>()
                        .ok()?,
                )
                .ok()?;
                let target = ori_kinematics::CanonicalHingeAngles::new(
                    geometry
                        .hinges()
                        .iter()
                        .map(|hinge| {
                            let signed = candidate_pattern
                                .edges
                                .iter()
                                .find(|edge| edge.id == hinge.edge())
                                .map_or(requested, |edge| {
                                    if edge.kind == EdgeKind::Valley {
                                        -requested
                                    } else {
                                        requested
                                    }
                                });
                            ori_kinematics::HingeAngle::new(hinge.edge(), signed)
                        })
                        .collect::<Result<Vec<_>, _>>()
                        .ok()?,
                )
                .ok()?;
                ori_kinematics::generate_linear_multi_hinge_path_candidate_v1(
                    &geometry,
                    &audit,
                    fixed_face,
                    &initial,
                    &target,
                    ori_kinematics::MultiHingePathCandidateLimitsV1::default(),
                )
                .ok()?
            };
            let schedule_limits = ori_kinematics::CycleScheduleLimitsV1 {
                max_work: 1_048_576,
                ..ori_kinematics::CycleScheduleLimitsV1::default()
            };
            let closure = geometry.prove_dyadic_schedule_closure_v1(
                &audit, fixed_face, generated.schedule(), 1.0e-8,
                ori_kinematics::DyadicIntervalClosureLimitsV1 {
                    max_depth: 16, max_leaves: 65_536,
                    max_work: schedule_limits.max_work, schedule_limits,
                },
            ).ok()?;
            if paper.thickness_mm > 0.0 {
                let certificate = ori_collision::certify_canonical_positive_thickness_cycle_schedule_path_with_control_v1(
                    &geometry,
                    &audit,
                    fixed_face,
                    generated.schedule(),
                    &closure,
                    paper.thickness_mm,
                    32,
                    control,
                )
                .ok()??;
                if control.checkpoint().is_err() {
                    return None;
                }
                certificate.is_for(
                    &geometry,
                    &audit,
                    fixed_face,
                    generated.schedule(),
                    &closure,
                    paper.thickness_mm,
                )
                    .then_some(ori_collision::STACKED_FOLD_CACTUS_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1)
            } else if paper.thickness_mm == 0.0 {
                ori_collision::diagnose_scheduled_cycle_path_v1(
                    &geometry, &audit, fixed_face, &generated, &closure, 32,
                ).continuous_certificate_model_id()
            } else { None }
        })?;
        if control.checkpoint().is_err() {
            return None;
        }
        (certificate_model, requested)
    };
    if control.checkpoint().is_err() {
        return None;
    }
    let bytes = serde_json::to_vec(&(
        "bounded_native_fold_path_v2",
        certificate_model,
        paper.thickness_mm.to_bits(),
        requested_angle_degrees.to_bits(),
        ordered,
        candidate_pattern,
    ))
    .ok()?;
    Some(sha2::Sha256::digest(bytes).into())
}

impl GlobalFlatFoldabilityObserver for BeginnerGlobalFoldabilityDeadline<'_> {
    fn checkpoint(&mut self) -> GlobalFlatFoldabilityCheckpoint {
        if self
            .cancelled
            .is_some_and(|cancelled| cancelled.load(Ordering::Acquire))
            || std::time::Instant::now() >= self.deadline
        {
            GlobalFlatFoldabilityCheckpoint::DeadlineReached
        } else {
            GlobalFlatFoldabilityCheckpoint::Continue
        }
    }
}

pub(super) fn assess_beginner_generated_plan(
    project_authority: ProjectId,
    paper: &Paper,
    current_pattern: &CreasePattern,
    plan: &ori_domain::BeginnerGeneratedPlanV1,
    reference: Option<&BeginnerReferenceModelSuggestionV1>,
) -> BeginnerGeneratedPlanAssessment {
    assess_beginner_generated_plan_with_control_v1(
        project_authority,
        paper,
        current_pattern,
        plan,
        reference,
        std::time::Instant::now() + std::time::Duration::from_millis(250),
        None,
    )
}

pub(super) fn assess_beginner_generated_plan_with_deadline(
    _project_authority: ProjectId,
    paper: &Paper,
    current_pattern: &CreasePattern,
    plan: &ori_domain::BeginnerGeneratedPlanV1,
    reference: Option<&BeginnerReferenceModelSuggestionV1>,
    deadline: std::time::Instant,
) -> BeginnerGeneratedPlanAssessment {
    assess_beginner_generated_plan_with_control_v1(
        _project_authority,
        paper,
        current_pattern,
        plan,
        reference,
        deadline,
        None,
    )
}

pub(super) fn assess_beginner_generated_plan_with_control_v1(
    _project_authority: ProjectId,
    paper: &Paper,
    current_pattern: &CreasePattern,
    plan: &ori_domain::BeginnerGeneratedPlanV1,
    reference: Option<&BeginnerReferenceModelSuggestionV1>,
    deadline: std::time::Instant,
    cancelled: Option<&AtomicBool>,
) -> BeginnerGeneratedPlanAssessment {
    let (mut shape_approximation_score, mut shape_difference_reason) = reference
        .map(|reference| compare_plan_to_reference_model_v1(plan, reference))
        .map_or((None, None), |(score, reason)| (Some(score), Some(reason)));
    let component_shape_comparison =
        reference.and_then(|reference| component_shape_comparison_v1(plan, reference));
    let expected_candidate_edge_id = plan
        .crease_pattern
        .edges
        .first()
        .map(|edge| edge.id)
        .unwrap_or_else(EdgeId::new);
    let deadline_assessment = move || BeginnerGeneratedPlanAssessment {
        kind: plan.kind,
        expected_candidate_edge_id,
        proof_scope: "indeterminate",
        apply_allowed: false,
        reason: "deadline_exceeded",
        shape_approximation_score,
        shape_difference_reason,
        component_shape_comparison,
    };
    let deadline_control = ori_collision::CooperativeOperationControlV1::new(cancelled, deadline);
    if deadline_control.checkpoint().is_err() {
        return deadline_assessment();
    }
    if let Err(reason) = validate_beginner_manufacturability_v1(&plan.crease_pattern, paper) {
        return BeginnerGeneratedPlanAssessment {
            kind: plan.kind,
            expected_candidate_edge_id,
            proof_scope: "necessary",
            apply_allowed: false,
            reason,
            shape_approximation_score,
            shape_difference_reason,
            component_shape_comparison,
        };
    }
    if component_shape_comparison.is_none()
        && reference.is_some_and(|reference| {
            bounded_folded_pose_landmark_score_v1(plan, reference).is_none()
        })
    {
        return BeginnerGeneratedPlanAssessment {
            kind: plan.kind,
            expected_candidate_edge_id,
            proof_scope: "indeterminate",
            apply_allowed: false,
            reason: "folded_pose_simulation_failed",
            shape_approximation_score,
            shape_difference_reason: Some("bounded_folded_pose_landmarks_v1"),
            component_shape_comparison,
        };
    }
    let mut candidate_pattern = current_pattern.clone();
    for vertex in &plan.crease_pattern.vertices {
        if let Some(current) = candidate_pattern
            .vertices
            .iter()
            .find(|current| current.id == vertex.id)
        {
            if current.position != vertex.position {
                return BeginnerGeneratedPlanAssessment {
                    kind: plan.kind,
                    expected_candidate_edge_id,
                    proof_scope: "necessary",
                    apply_allowed: false,
                    reason: "geometry_invalid",
                    shape_approximation_score,
                    shape_difference_reason,
                    component_shape_comparison,
                };
            }
        } else {
            candidate_pattern.vertices.push(vertex.clone());
        }
    }
    if plan.crease_pattern.edges.is_empty()
        || plan.crease_pattern.edges.iter().any(|edge| {
            candidate_pattern
                .edges
                .iter()
                .any(|current| current.id == edge.id)
        })
    {
        return BeginnerGeneratedPlanAssessment {
            kind: plan.kind,
            expected_candidate_edge_id,
            proof_scope: "necessary",
            apply_allowed: false,
            reason: "geometry_invalid",
            shape_approximation_score,
            shape_difference_reason,
            component_shape_comparison,
        };
    }
    candidate_pattern
        .edges
        .extend(plan.crease_pattern.edges.iter().cloned());
    if !validate_crease_pattern(&candidate_pattern).is_valid()
        || !validate_paper(paper, &candidate_pattern).is_valid()
    {
        return BeginnerGeneratedPlanAssessment {
            kind: plan.kind,
            expected_candidate_edge_id,
            proof_scope: "necessary",
            apply_allowed: false,
            reason: "geometry_invalid",
            shape_approximation_score,
            shape_difference_reason,
            component_shape_comparison,
        };
    }
    const MAX_CANDIDATE_GLOBAL_RECORDS: usize = 2_048;
    let geometry_authority = ProjectId::schema_namespace([
        0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04,
        0x98,
    ]);
    let candidate_snapshot = if candidate_pattern.vertices.len() + candidate_pattern.edges.len()
        <= MAX_CANDIDATE_GLOBAL_RECORDS
    {
        EditorState::with_paper(candidate_pattern.clone(), paper.clone())
            .topology_analysis_input(geometry_authority)
            .analyze()
            .simulation_snapshot()
            .cloned()
    } else {
        None
    };
    if let Some(snapshot) = candidate_snapshot.as_ref() {
        if certify_beginner_fold_path_with_control_v1(
            plan,
            paper,
            &candidate_pattern,
            snapshot,
            &deadline_control,
        )
        .is_some()
        {
            return BeginnerGeneratedPlanAssessment {
                kind: plan.kind,
                expected_candidate_edge_id,
                proof_scope: "sufficient",
                apply_allowed: true,
                reason: "native_fold_path_certified",
                shape_approximation_score,
                shape_difference_reason,
                component_shape_comparison,
            };
        }
    }
    let local = analyze_local_flat_foldability(paper, &candidate_pattern);
    let (mut proof_scope, mut apply_allowed, mut reason) = match local.status {
        LocalFlatFoldabilityReportStatus::NecessaryConditionsSatisfied => {
            ("necessary", true, "necessary_conditions_satisfied")
        }
        LocalFlatFoldabilityReportStatus::Violated => {
            ("necessary", false, "necessary_conditions_violated")
        }
        LocalFlatFoldabilityReportStatus::Blocked => ("necessary", false, "local_analysis_blocked"),
        LocalFlatFoldabilityReportStatus::NotApplicable => {
            ("indeterminate", true, "local_theorem_not_applicable")
        }
        LocalFlatFoldabilityReportStatus::Indeterminate => {
            ("indeterminate", true, "local_analysis_indeterminate")
        }
    };
    if apply_allowed {
        if candidate_pattern.vertices.len() + candidate_pattern.edges.len()
            > MAX_CANDIDATE_GLOBAL_RECORDS
        {
            proof_scope = "indeterminate";
            reason = "global_resource_limit";
        } else {
            let identity_namespace = geometry_authority;
            if let Some(snapshot) = candidate_snapshot.as_ref() {
                let mut observer = BeginnerGlobalFoldabilityDeadline {
                    deadline,
                    cancelled,
                };
                match analyze_global_flat_foldability_with_observer(
                    GlobalFlatFoldabilityInput::current_with_geometry(
                        identity_namespace,
                        paper,
                        &candidate_pattern,
                        snapshot,
                        &local,
                    ),
                    GlobalFlatFoldabilityLimits::default(),
                    &mut observer,
                ) {
                    Ok(report) => match report.outcome {
                        GlobalFlatFoldabilityOutcome::Possible { layer_order, .. } => {
                            if certify_beginner_fold_path_with_control_v1(
                                plan,
                                paper,
                                &candidate_pattern,
                                snapshot,
                                &deadline_control,
                            )
                            .is_none()
                            {
                                proof_scope = "necessary";
                                apply_allowed = false;
                                reason = "fold_path_certificate_unavailable";
                            } else {
                                proof_scope = "sufficient";
                                reason = "global_flat_foldability_proven";
                            }
                            if component_shape_comparison.is_none() {
                                if let (Some(reference), Some(surface)) = (
                                    reference,
                                    ori_core::extract_certified_flat_surface_v1(
                                        &candidate_pattern,
                                        snapshot,
                                        &layer_order,
                                    ),
                                ) {
                                    shape_approximation_score =
                                        Some(compare_flat_surface_to_reference_model_v1(
                                            &surface, reference,
                                        ));
                                    shape_difference_reason = Some("certified_flat_surface_v1");
                                }
                            }
                        }
                        GlobalFlatFoldabilityOutcome::Impossible { .. } => {
                            proof_scope = "necessary";
                            apply_allowed = false;
                            reason = "global_flat_foldability_impossible";
                        }
                        GlobalFlatFoldabilityOutcome::Unknown {
                            reason: GlobalFlatFoldabilityUnknownReason::TimeLimitReached { .. },
                        } => {
                            proof_scope = "indeterminate";
                            reason = "global_timeout";
                        }
                        GlobalFlatFoldabilityOutcome::Unknown {
                            reason:
                                GlobalFlatFoldabilityUnknownReason::ResourceLimitReached { .. }
                                | GlobalFlatFoldabilityUnknownReason::ExactNumberLimitReached { .. }
                                | GlobalFlatFoldabilityUnknownReason::OverlapArrangementLimitReached {
                                    ..
                                }
                                | GlobalFlatFoldabilityUnknownReason::ConstraintLimitReached { .. },
                        } => {
                            proof_scope = "indeterminate";
                            reason = "global_resource_limit";
                        }
                        GlobalFlatFoldabilityOutcome::Unknown { .. } => {
                            proof_scope = "indeterminate";
                            reason = "global_indeterminate";
                        }
                    },
                    Err(_) => {
                        proof_scope = "indeterminate";
                        reason = "global_indeterminate";
                    }
                }
            } else {
                proof_scope = "indeterminate";
                reason = "global_indeterminate";
            }
        }
    }
    if deadline_control.checkpoint().is_err() {
        return deadline_assessment();
    }
    BeginnerGeneratedPlanAssessment {
        kind: plan.kind,
        expected_candidate_edge_id,
        proof_scope,
        apply_allowed,
        reason,
        shape_approximation_score,
        shape_difference_reason,
        component_shape_comparison,
    }
}

#[tauri::command]
pub(super) fn evaluate_beginner_candidates(
    app: AppHandle,
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    requested_candidate_count: u8,
    request_generation_id: ProjectId,
) -> Result<BeginnerCandidateResponse, String> {
    if !(1..=ori_domain::MAX_BEGINNER_CANDIDATES_V1 as u8).contains(&requested_candidate_count) {
        return Err("requested candidate count must be between 1 and 3".to_owned());
    }
    let work = Arc::new(ReferenceConsensusWorkV1::default());
    run_registered_reference_consensus_work_v1(request_generation_id, &work, || {
        let control = beginner_candidate_analysis_control_v1(
            Some(&work.cancelled),
            "reference_consensus_cancelled",
            "beginner_candidate_deadline_exceeded",
        );
        let expectation = ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        );
        let snapshot = {
            let project = lock_project(&state)?;
            control.checkpoint()?;
            capture_beginner_candidate_analysis_snapshot_with_control_v1(
                &project,
                expectation,
                &control,
            )?
        };
        control.checkpoint()?;
        let project = &snapshot.project;
        let pattern = project.editor.pattern();
        let crease_count = pattern
            .edges
            .iter()
            .filter(|edge| matches!(edge.kind, EdgeKind::Mountain | EdgeKind::Valley))
            .count();
        let constraints = &project
            .editor
            .beginner_design_profile()
            .generation_constraints;
        let mut candidates = ori_domain::score_beginner_candidates_v1(
            ori_domain::BeginnerCandidateInputV1 {
                vertex_count: pattern.vertices.len(),
                edge_count: pattern.edges.len(),
                crease_count,
                target_approximation_score: ori_domain::beginner_target_approximation_score_v1(
                    constraints,
                ),
            },
            project.editor.beginner_design_profile(),
        );
        candidates.truncate(usize::from(requested_candidate_count));
        control.checkpoint()?;
        let generation = if target_asset_reference_is_live(&project, constraints.target_asset) {
            ori_domain::generate_beginner_plans_v1(
                project.project_id,
                pattern,
                &project.editor.paper().boundary_vertices,
                constraints,
            )
        } else {
            control.checkpoint()?;
            let current = lock_project(&state)?;
            beginner_candidate_snapshot_is_current_v1(&current, &snapshot)?;
            control.checkpoint()?;
            return Ok(BeginnerCandidateResponse {
                schema_version: ori_domain::BEGINNER_CANDIDATE_SCHEMA_VERSION_V1,
                project_instance_id: project.instance_id,
                project_id: project.project_id,
                revision: snapshot.expectation.revision,
                requested_candidate_count,
                bulge_treatment: ori_domain::BeginnerBulgeTreatmentV1::TargetShapeApproximation,
                elasticity_model: ori_domain::BeginnerElasticityModelV1::NotComputed,
                candidates,
                generation_status: "missing_target_asset",
                generated_plans: Vec::new(),
                plan_assessments: Vec::new(),
                multi_reference_fusion: None,
                reference_consensus_analysis: None,
            });
        };
        let (generation_status, mut generated_plans) = match generation {
            Ok(plans) => ("ready", plans),
            Err(ori_domain::BeginnerGeneratorErrorV1::ResourceLimit) => {
                ("resource_limit", Vec::new())
            }
            Err(ori_domain::BeginnerGeneratorErrorV1::UnsupportedPaper) => {
                ("unsupported_paper", Vec::new())
            }
            Err(ori_domain::BeginnerGeneratorErrorV1::UnsupportedTechniques) => {
                ("unsupported_techniques", Vec::new())
            }
            Err(ori_domain::BeginnerGeneratorErrorV1::MissingTargetCategory) => {
                ("missing_target_category", Vec::new())
            }
            Err(ori_domain::BeginnerGeneratorErrorV1::MissingRequiredParts) => {
                ("missing_required_parts", Vec::new())
            }
            Err(ori_domain::BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate) => {
                ("unsupported_animal_template", Vec::new())
            }
            Err(ori_domain::BeginnerGeneratorErrorV1::UnsupportedInsectTemplate) => {
                ("unsupported_insect_template", Vec::new())
            }
        };
        generated_plans.truncate(usize::from(requested_candidate_count));
        control.checkpoint()?;
        let reference_suggestion = live_reference_model_suggestion_v1(&project).ok();
        let mut multi_reference_fusion = reference_suggestion
            .as_ref()
            .and_then(|reference| beginner_multi_reference_fusion_v1(&project, reference));
        if let Some(fusion) = &mut multi_reference_fusion {
            fusion.revision = snapshot.expectation.revision;
        }
        control.checkpoint()?;
        let mut reference_consensus_analysis =
            beginner_reference_consensus_analysis_with_deadline_v1(
                &project,
                Some((&app, request_generation_id, &work)),
                control.deadline,
            );
        if let Some(analysis) = &mut reference_consensus_analysis {
            analysis.revision = snapshot.expectation.revision;
        }
        control.checkpoint()?;
        let mut plan_assessments = Vec::new();
        plan_assessments
            .try_reserve(generated_plans.len())
            .map_err(|_| "beginner_candidate_snapshot_resource_limit".to_owned())?;
        for plan in &generated_plans {
            control.checkpoint()?;
            let assessment = assess_beginner_generated_plan_with_control_v1(
                project.project_id,
                project.editor.paper(),
                pattern,
                plan,
                reference_suggestion.as_ref(),
                control.deadline,
                Some(&work.cancelled),
            );
            control.checkpoint()?;
            plan_assessments.push(assessment);
        }
        if multi_reference_fusion
            .as_ref()
            .is_some_and(|fusion| !fusion.apply_allowed)
        {
            for assessment in &mut plan_assessments {
                assessment.apply_allowed = false;
                assessment.proof_scope = "indeterminate";
                assessment.reason = "multi_reference_disagreement";
            }
        }
        if reference_consensus_analysis
            .as_ref()
            .is_some_and(|analysis| !analysis.apply_allowed)
        {
            for assessment in &mut plan_assessments {
                assessment.apply_allowed = false;
                assessment.proof_scope = "indeterminate";
                assessment.reason = "multi_reference_disagreement";
            }
        }
        control.checkpoint()?;
        let current = lock_project(&state)?;
        beginner_candidate_snapshot_is_current_v1(&current, &snapshot)?;
        control.checkpoint()?;
        Ok(BeginnerCandidateResponse {
            schema_version: ori_domain::BEGINNER_CANDIDATE_SCHEMA_VERSION_V1,
            project_instance_id: project.instance_id,
            project_id: project.project_id,
            revision: snapshot.expectation.revision,
            requested_candidate_count,
            bulge_treatment: ori_domain::BeginnerBulgeTreatmentV1::TargetShapeApproximation,
            elasticity_model: ori_domain::BeginnerElasticityModelV1::NotComputed,
            candidates,
            generation_status,
            generated_plans,
            plan_assessments,
            multi_reference_fusion,
            reference_consensus_analysis,
        })
    })
}

#[tauri::command]
pub(super) fn cancel_reference_consensus(request_generation_id: ProjectId) -> Result<(), String> {
    let registry = lock_recovering_registry_v1(reference_consensus_work_v1());
    let work = registry
        .get(&request_generation_id)
        .ok_or_else(|| "reference_consensus_generation_not_running".to_owned())?;
    request_work_cancellation_v1(&work.cancelled, &work.terminal);
    Ok(())
}

#[derive(Debug, Serialize)]
pub(super) struct BeginnerSymmetricEstimateResponse {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    estimate: ori_domain::BeginnerSymmetricParameterEstimateV1,
    candidates: [ori_domain::BeginnerSymmetricParameterCandidateV1; 3],
}

#[tauri::command]
pub(super) fn get_beginner_symmetric_parameter_estimate(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<BeginnerSymmetricEstimateResponse, String> {
    let project = lock_and_expect(
        &state,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
    )?;
    let estimate = ori_domain::estimate_symmetric_parameters_v1(
        &project
            .editor
            .beginner_design_profile()
            .generation_constraints,
    )
    .ok_or_else(|| "symmetric_parameter_estimate_unsupported".to_owned())?;
    let candidates = ori_domain::symmetric_parameter_candidates_v1(estimate);
    Ok(BeginnerSymmetricEstimateResponse {
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: project.editor.revision(),
        estimate,
        candidates,
    })
}

pub(super) fn temporary_symmetric_profile_for_grid(
    source: &ori_domain::BeginnerDesignProfileV1,
    point: ori_domain::BeginnerParameterGridPointV1,
) -> Result<ori_domain::BeginnerDesignProfileV1, String> {
    let canonical = ori_domain::beginner_parameter_grid_v1()
        .get(usize::from(point.id))
        .copied();
    if canonical != Some(point) {
        return Err("beginner_parameter_grid_point_invalid".to_owned());
    }
    let preserved_generic_tree_segments = (symmetric_plan_kind(source)
        == ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase)
        .then(|| source.generation_constraints.skeleton_segments.clone());

    let preserved_pair_ids =
        ori_domain::animal_complete_bindings_v1(&source.generation_constraints)
            .map(|binding| {
                vec![
                    binding.horn_protrusion_id,
                    binding.tail_protrusion_id,
                    binding.ear_pair_protrusion_id,
                    binding.leg_protrusion_id,
                ]
            })
            .or_else(|| {
                ori_domain::insect_complete_bindings_v1(&source.generation_constraints).map(
                    |binding| {
                        vec![
                            binding.wing_pair_protrusion_id,
                            binding.antenna_pair_protrusion_id,
                            binding.leg_pair_protrusion_ids[0],
                            binding.leg_pair_protrusion_ids[1],
                            binding.leg_pair_protrusion_ids[2],
                        ]
                    },
                )
            })
            .or_else(|| {
                ori_domain::insect_three_pair_bindings_v1(&source.generation_constraints).map(
                    |bindings| {
                        bindings
                            .into_iter()
                            .map(|binding| binding.protrusion_id)
                            .collect()
                    },
                )
            })
            .or_else(|| {
                let protrusions = &source.generation_constraints.protrusions;
                let feature_records = source
                    .generation_constraints
                    .target_parts
                    .iter()
                    .filter(|part| {
                        !matches!(
                            part.kind,
                            ori_domain::BeginnerTargetPartKindV1::Head
                                | ori_domain::BeginnerTargetPartKindV1::Torso
                        )
                    })
                    .count();
                ((2..=8).contains(&protrusions.len())
                    && feature_records == protrusions.len()
                    && protrusions.windows(2).all(|pair| pair[0].id < pair[1].id))
                .then(|| protrusions.iter().map(|target| target.id).collect())
            });
    let mut profile = source.clone();
    profile.generation_constraints.detail_level = point.detail_level;
    let estimate = ori_domain::estimate_symmetric_parameters_v1(&profile.generation_constraints)
        .ok_or_else(|| "symmetric_parameter_estimate_unsupported".to_owned())?;
    configure_symmetric_profile(
        &mut profile,
        estimate,
        point.scale_percent,
        point.spacing_percent,
    );
    if let Some(segments) = preserved_generic_tree_segments {
        profile.generation_constraints.skeleton_segments = segments;
    }
    if let Some(pair_ids) = preserved_pair_ids {
        profile.generation_constraints.protrusions = pair_ids
            .into_iter()
            .filter_map(|protrusion_id| {
                source
                    .generation_constraints
                    .protrusions
                    .iter()
                    .find(|target| target.id == protrusion_id)
                    .cloned()
            })
            .map(|mut target| {
                target.length_tenths_mm = target
                    .length_tenths_mm
                    .saturating_mul(u32::from(point.scale_percent))
                    .checked_div(27)
                    .unwrap_or(1)
                    .clamp(1, 1_000_000);
                target.thickness_tenths_mm = u16::try_from(
                    u32::from(target.thickness_tenths_mm)
                        .saturating_mul(u32::from(point.spacing_percent))
                        .checked_div(50)
                        .unwrap_or(1)
                        .clamp(1, 10_000),
                )
                .unwrap_or(10_000);
                target
            })
            .collect();
    }
    if matches!(
        source.generation_constraints.target_asset,
        Some(ori_domain::BeginnerTargetAssetReferenceV1::ReferenceModel { .. })
    ) {
        if let (Some(base), Some(candidate)) = (
            source
                .generation_constraints
                .protrusions
                .iter()
                .find(|target| target.count == estimate.protrusion_count),
            profile.generation_constraints.protrusions.first_mut(),
        ) {
            candidate.length_tenths_mm = base
                .length_tenths_mm
                .saturating_mul(u32::from(point.scale_percent))
                .checked_div(27)
                .unwrap_or(1)
                .clamp(1, 1_000_000);
            candidate.thickness_tenths_mm = u16::try_from(
                u32::from(base.thickness_tenths_mm)
                    .saturating_mul(u32::from(point.spacing_percent))
                    .checked_div(50)
                    .unwrap_or(1)
                    .clamp(1, 10_000),
            )
            .unwrap_or(10_000);
        }
    }
    Ok(profile)
}

pub(super) fn grid_template_plan(
    namespace: ProjectId,
    source: &CreasePattern,
    boundary_vertices: &[VertexId],
    profile: &ori_domain::BeginnerDesignProfileV1,
    point: ori_domain::BeginnerParameterGridPointV1,
) -> Result<Vec<ori_domain::BeginnerGeneratedPlanV1>, ori_domain::BeginnerGeneratorErrorV1> {
    let temporary = temporary_symmetric_profile_for_grid(profile, point)
        .map_err(|_| ori_domain::BeginnerGeneratorErrorV1::MissingRequiredParts)?;
    let mut plans = ori_domain::generate_beginner_plans_v1(
        namespace,
        source,
        boundary_vertices,
        &temporary.generation_constraints,
    )?;
    if let Some(horizontal) = plans
        .first()
        .filter(|plan| {
            plan.kind == ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
        })
        .cloned()
    {
        let positions = boundary_vertices
            .iter()
            .filter_map(|id| {
                source
                    .vertices
                    .iter()
                    .find(|vertex| vertex.id == *id)
                    .map(|vertex| vertex.position)
            })
            .collect::<Vec<_>>();
        let min_x = positions
            .iter()
            .map(|point| point.x)
            .fold(f64::INFINITY, f64::min);
        let max_x = positions
            .iter()
            .map(|point| point.x)
            .fold(f64::NEG_INFINITY, f64::max);
        let min_y = positions
            .iter()
            .map(|point| point.y)
            .fold(f64::INFINITY, f64::min);
        let max_y = positions
            .iter()
            .map(|point| point.y)
            .fold(f64::NEG_INFINITY, f64::max);
        let width = max_x - min_x;
        let height = max_y - min_y;
        if width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0 {
            let mut vertical = horizontal;
            for vertex in &mut vertical.crease_pattern.vertices {
                let x_ratio = (vertex.position.x - min_x) / width;
                let y_ratio = (vertex.position.y - min_y) / height;
                vertex.position = Point2::new(min_x + y_ratio * width, min_y + x_ratio * height);
            }
            vertical
                .instruction_codes
                .push("bounded_tree_paper_orientation_v1:vertical".to_owned());
            plans.insert(1, vertical);
            plans[0]
                .instruction_codes
                .push("bounded_tree_paper_orientation_v1:horizontal".to_owned());
        }
    }
    Ok(plans)
}

fn symmetric_plan_kind(
    profile: &ori_domain::BeginnerDesignProfileV1,
) -> ori_domain::BeginnerGeneratedPlanKindV1 {
    let has = |kind| {
        profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == kind && part.count == 2)
    };
    let feature_records = profile
        .generation_constraints
        .target_parts
        .iter()
        .filter(|part| {
            !matches!(
                part.kind,
                ori_domain::BeginnerTargetPartKindV1::Head
                    | ori_domain::BeginnerTargetPartKindV1::Torso
            )
        })
        .count();
    let has_one = |kind| {
        profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == kind && part.count == 1)
    };
    let horn = has_one(ori_domain::BeginnerTargetPartKindV1::Horn);
    let tail = has_one(ori_domain::BeginnerTargetPartKindV1::Tail);
    let ears = has(ori_domain::BeginnerTargetPartKindV1::Ear);
    let legs = profile
        .generation_constraints
        .target_parts
        .iter()
        .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Leg && part.count == 4);
    let wings = has(ori_domain::BeginnerTargetPartKindV1::Wing);
    let known_animal = feature_records == 2 && ((ears || tail) && horn || tail && ears)
        || feature_records == 3 && horn && tail && ears
        || feature_records == 4 && horn && tail && ears && legs
        || feature_records == 5 && horn && tail && ears && legs && wings;
    let wing_antenna = wings && has(ori_domain::BeginnerTargetPartKindV1::Antenna);
    let known_insect = feature_records == 2 && wing_antenna
        || feature_records == 3
            && wing_antenna
            && profile
                .generation_constraints
                .target_parts
                .iter()
                .any(|part| {
                    part.kind == ori_domain::BeginnerTargetPartKindV1::Leg && part.count == 6
                });
    let generic_mixed_target = feature_records >= 2 && !known_animal && !known_insect;
    if profile.generation_constraints.target_category
        == Some(ori_domain::BeginnerTargetCategoryV1::CustomObject)
        || generic_mixed_target
    {
        ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
    } else if profile.generation_constraints.target_category
        == Some(ori_domain::BeginnerTargetCategoryV1::Animal)
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Leg && part.count == 4)
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Horn && part.count == 1)
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Tail && part.count == 1)
        && has(ori_domain::BeginnerTargetPartKindV1::Ear)
    {
        if has(ori_domain::BeginnerTargetPartKindV1::Wing) {
            ori_domain::BeginnerGeneratedPlanKindV1::CompositeCompleteWingedAnimalBase
        } else {
            ori_domain::BeginnerGeneratedPlanKindV1::CompositeCompleteAnimalBase
        }
    } else if profile.generation_constraints.target_category
        == Some(ori_domain::BeginnerTargetCategoryV1::Insect)
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Wing && part.count == 2)
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| {
                part.kind == ori_domain::BeginnerTargetPartKindV1::Antenna && part.count == 2
            })
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Leg && part.count == 6)
    {
        ori_domain::BeginnerGeneratedPlanKindV1::CompositeCompleteInsectBase
    } else if profile.generation_constraints.target_category
        == Some(ori_domain::BeginnerTargetCategoryV1::Insect)
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Wing && part.count == 2)
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| {
                part.kind == ori_domain::BeginnerTargetPartKindV1::Antenna && part.count == 2
            })
    {
        ori_domain::BeginnerGeneratedPlanKindV1::CompositeWingAntennaBase
    } else if profile.generation_constraints.target_category
        == Some(ori_domain::BeginnerTargetCategoryV1::Animal)
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Horn && part.count == 1)
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Tail && part.count == 1)
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Ear && part.count == 2)
    {
        ori_domain::BeginnerGeneratedPlanKindV1::CompositeHornTailEarBase
    } else if profile.generation_constraints.target_category
        == Some(ori_domain::BeginnerTargetCategoryV1::Animal)
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Horn && part.count == 1)
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Tail && part.count == 1)
    {
        ori_domain::BeginnerGeneratedPlanKindV1::CompositeHornTailBase
    } else if profile.generation_constraints.target_category
        == Some(ori_domain::BeginnerTargetCategoryV1::Animal)
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Horn && part.count == 1)
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Ear && part.count == 2)
    {
        ori_domain::BeginnerGeneratedPlanKindV1::CompositeHornEarBase
    } else if profile.generation_constraints.target_category
        == Some(ori_domain::BeginnerTargetCategoryV1::Animal)
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Tail && part.count == 1)
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Ear && part.count == 2)
    {
        ori_domain::BeginnerGeneratedPlanKindV1::CompositeTailEarBase
    } else if profile.generation_constraints.target_category
        == Some(ori_domain::BeginnerTargetCategoryV1::Insect)
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| {
                part.kind == ori_domain::BeginnerTargetPartKindV1::Antenna && part.count == 1
            })
    {
        ori_domain::BeginnerGeneratedPlanKindV1::CenterAxisAntennaBase
    } else if profile.generation_constraints.target_category
        == Some(ori_domain::BeginnerTargetCategoryV1::Animal)
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Horn && part.count == 1)
    {
        ori_domain::BeginnerGeneratedPlanKindV1::CenterAxisHornBase
    } else if profile.generation_constraints.target_category
        == Some(ori_domain::BeginnerTargetCategoryV1::Animal)
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Tail && part.count == 1)
    {
        ori_domain::BeginnerGeneratedPlanKindV1::CenterAxisTailBase
    } else if profile.generation_constraints.target_category
        == Some(ori_domain::BeginnerTargetCategoryV1::Insect)
    {
        if profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Leg && part.count == 6)
        {
            ori_domain::BeginnerGeneratedPlanKindV1::SymmetricSixLegBase
        } else if has(ori_domain::BeginnerTargetPartKindV1::Antenna) {
            ori_domain::BeginnerGeneratedPlanKindV1::SymmetricAntennaBase
        } else if has(ori_domain::BeginnerTargetPartKindV1::Leg) {
            ori_domain::BeginnerGeneratedPlanKindV1::SymmetricInsectLegPairBase
        } else {
            ori_domain::BeginnerGeneratedPlanKindV1::SymmetricWingBase
        }
    } else if has(ori_domain::BeginnerTargetPartKindV1::Wing) {
        ori_domain::BeginnerGeneratedPlanKindV1::SymmetricBirdBase
    } else if has(ori_domain::BeginnerTargetPartKindV1::Fin) {
        ori_domain::BeginnerGeneratedPlanKindV1::SymmetricFishBase
    } else if has(ori_domain::BeginnerTargetPartKindV1::Ear) {
        ori_domain::BeginnerGeneratedPlanKindV1::SymmetricEarBase
    } else if has(ori_domain::BeginnerTargetPartKindV1::Horn) {
        ori_domain::BeginnerGeneratedPlanKindV1::SymmetricHornBase
    } else {
        ori_domain::BeginnerGeneratedPlanKindV1::SymmetricFourLegBase
    }
}

#[derive(Debug, Serialize)]
pub(super) struct BeginnerContourBindingWitness {
    pub(super) protrusion_id: u16,
    pub(super) contour_points: u8,
    pub(super) generated_face_id: u8,
    pub(super) vertex_start: u16,
    pub(super) crease_start: u16,
}

#[derive(Debug, Serialize)]
pub(super) struct BeginnerGenericFeatureBindingWitness {
    pub(super) protrusion_id: u16,
    pub(super) generated_feature_id: u8,
    pub(super) endpoint_count: u8,
    pub(super) crease_start: u16,
    pub(super) crease_authority_sha256: [u8; 32],
    pub(super) skeleton_segment_id: u16,
    pub(super) skeleton_endpoint: &'static str,
    pub(super) mount_distance_squared_tenths_mm: u64,
}

#[derive(Debug, Serialize)]
pub(super) struct BeginnerSkeletonBranchBindingWitness {
    pub(super) segment_id: u16,
    pub(super) parent_segment_id: Option<u16>,
    pub(super) parent_endpoint: Option<&'static str>,
    pub(super) child_endpoint: Option<&'static str>,
    pub(super) generated_feature_ids: Vec<u8>,
}

#[derive(Debug, Serialize)]
pub(super) struct BeginnerContourPlacementWitness {
    pub(super) body_contour_points: u8,
    pub(super) local_bindings: Vec<BeginnerContourBindingWitness>,
    pub(super) generic_feature_bindings: Vec<BeginnerGenericFeatureBindingWitness>,
    pub(super) skeleton_branch_bindings: Vec<BeginnerSkeletonBranchBindingWitness>,
    pub(super) skeleton_tree_authority_sha256: [u8; 32],
    pub(super) witnessed_vertices: u16,
    pub(super) witnessed_creases: u16,
    pub(super) topology_authority_hash: [u8; 32],
    pub(super) max_contour_error_millionths: u32,
}

pub(super) fn canonical_generic_tree_segments_v1(
    segments: &[ori_domain::BeginnerSkeletonSegmentV1],
) -> Option<Vec<ori_domain::BeginnerSkeletonSegmentV1>> {
    if segments.is_empty() || segments.len() > ori_domain::MAX_BEGINNER_GENERIC_TREE_BARS_V1 {
        return None;
    }
    let mut canonical = segments.to_vec();
    canonical.sort_unstable_by_key(|segment| segment.id);
    if canonical.windows(2).any(|pair| pair[0].id == pair[1].id) {
        return None;
    }
    for segment in &mut canonical {
        let start = (segment.start.x_tenths_mm, segment.start.y_tenths_mm);
        let end = (segment.end.x_tenths_mm, segment.end.y_tenths_mm);
        if end < start {
            std::mem::swap(&mut segment.start, &mut segment.end);
        }
    }
    Some(canonical)
}

pub(super) fn normalized_contour_error_millionths(
    target: &[[i32; 2]],
    generated: &[ori_domain::Vertex],
) -> Option<u32> {
    if target.len() < 3 || generated.len() < 3 {
        return None;
    }
    let (target_min_x, target_max_x) = target
        .iter()
        .map(|point| point[0])
        .fold((i32::MAX, i32::MIN), |(min, max), value| {
            (min.min(value), max.max(value))
        });
    let (target_min_y, target_max_y) = target
        .iter()
        .map(|point| point[1])
        .fold((i32::MAX, i32::MIN), |(min, max), value| {
            (min.min(value), max.max(value))
        });
    let (generated_min_x, generated_max_x) = generated
        .iter()
        .map(|vertex| vertex.position.x)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), value| {
            (min.min(value), max.max(value))
        });
    let (generated_min_y, generated_max_y) = generated
        .iter()
        .map(|vertex| vertex.position.y)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), value| {
            (min.min(value), max.max(value))
        });
    let target_span_x = f64::from(target_max_x - target_min_x);
    let target_span_y = f64::from(target_max_y - target_min_y);
    let generated_span_x = generated_max_x - generated_min_x;
    let generated_span_y = generated_max_y - generated_min_y;
    if target_span_x <= 0.0
        || target_span_y <= 0.0
        || generated_span_x <= 0.0
        || generated_span_y <= 0.0
        || !generated_span_x.is_finite()
        || !generated_span_y.is_finite()
    {
        return None;
    }
    let target = target
        .iter()
        .map(|point| {
            [
                f64::from(point[0] - target_min_x) / target_span_x,
                f64::from(point[1] - target_min_y) / target_span_y,
            ]
        })
        .collect::<Vec<_>>();
    let generated = generated
        .iter()
        .map(|vertex| {
            [
                (vertex.position.x - generated_min_x) / generated_span_x,
                (vertex.position.y - generated_min_y) / generated_span_y,
            ]
        })
        .collect::<Vec<_>>();
    let point_segment_distance = |point: [f64; 2], start: [f64; 2], end: [f64; 2]| -> Option<f64> {
        let delta = [end[0] - start[0], end[1] - start[1]];
        let length_squared = delta[0] * delta[0] + delta[1] * delta[1];
        if !length_squared.is_finite() {
            return None;
        }
        if length_squared <= f64::EPSILON {
            return Some(f64::INFINITY);
        }
        let projection = (((point[0] - start[0]) * delta[0] + (point[1] - start[1]) * delta[1])
            / length_squared)
            .clamp(0.0, 1.0);
        ori_numeric::deterministic_hypot_v1(
            point[0] - start[0] - projection * delta[0],
            point[1] - start[1] - projection * delta[1],
        )
        .ok()
    };
    let directed = |source: &[[f64; 2]], destination: &[[f64; 2]]| -> Option<f64> {
        source
            .iter()
            .enumerate()
            .flat_map(|(index, start)| {
                let end = source[(index + 1) % source.len()];
                (0..=32).map(move |sample| {
                    let ratio = f64::from(sample) / 32.0;
                    [
                        start[0] + (end[0] - start[0]) * ratio,
                        start[1] + (end[1] - start[1]) * ratio,
                    ]
                })
            })
            .try_fold(0.0_f64, |maximum, point| {
                let minimum = destination.iter().enumerate().try_fold(
                    f64::INFINITY,
                    |minimum, (index, start)| {
                        let distance = point_segment_distance(
                            point,
                            *start,
                            destination[(index + 1) % destination.len()],
                        )?;
                        Some(minimum.min(distance))
                    },
                )?;
                Some(maximum.max(minimum))
            })
    };
    let maximum = directed(&target, &generated)?.max(directed(&generated, &target)?);
    u32::try_from((maximum * 1_000_000.0).round() as u64).ok()
}

pub(super) fn beginner_contour_placement_witness(
    constraints: &ori_domain::BeginnerGenerationConstraintsV1,
    plan: &ori_domain::BeginnerGeneratedPlanV1,
) -> Option<BeginnerContourPlacementWitness> {
    let body_contour_points = constraints
        .generic_body_outline_tenths_mm
        .as_ref()
        .map_or(0, Vec::len);
    let mut local_bindings = constraints
        .protrusions
        .iter()
        .filter(|target| target.local_outline_tenths_mm.is_some())
        .map(|target| {
            let outline = target.local_outline_tenths_mm.as_ref()?;
            Some(BeginnerContourBindingWitness {
                protrusion_id: target.id,
                contour_points: u8::try_from(outline.len()).ok()?,
                generated_face_id: 0,
                vertex_start: 0,
                crease_start: 0,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    local_bindings.sort_unstable_by_key(|binding| binding.protrusion_id);
    if local_bindings
        .windows(2)
        .any(|pair| pair[0].protrusion_id >= pair[1].protrusion_id)
    {
        return None;
    }
    let canonical_skeleton_segments =
        if plan.kind == ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase {
            Some(canonical_generic_tree_segments_v1(
                &constraints.skeleton_segments,
            )?)
        } else {
            None
        };
    let witnessed = body_contour_points.saturating_add(
        local_bindings
            .iter()
            .map(|binding| usize::from(binding.contour_points))
            .sum(),
    );
    let (graph_vertices, graph_edges) =
        if plan.kind == ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase {
            let segments = canonical_skeleton_segments.as_deref()?;
            let points = segments
                .iter()
                .flat_map(|segment| {
                    [
                        (segment.start.x_tenths_mm, segment.start.y_tenths_mm),
                        (segment.end.x_tenths_mm, segment.end.y_tenths_mm),
                    ]
                })
                .collect::<HashSet<_>>();
            (points.len(), segments.len())
        } else {
            (0, 0)
        };
    let contour_vertex_end = plan
        .crease_pattern
        .vertices
        .len()
        .checked_sub(graph_vertices)?;
    let contour_edge_end = plan.crease_pattern.edges.len().checked_sub(graph_edges)?;
    let mut generic_feature_bindings = Vec::new();
    if plan.kind == ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase {
        let endpoint_count = constraints
            .protrusions
            .iter()
            .try_fold(0usize, |total, target| {
                if matches!(target.count, 1 | 2 | 4) {
                    total.checked_add(usize::from(target.count))
                } else {
                    None
                }
            })?;
        let mut crease_cursor = contour_edge_end
            .checked_sub(witnessed)?
            .checked_sub(endpoint_count)?;
        let mut canonical_targets = constraints.protrusions.iter().collect::<Vec<_>>();
        canonical_targets.sort_unstable_by_key(|target| target.id);
        if canonical_targets
            .windows(2)
            .any(|pair| pair[0].id >= pair[1].id)
        {
            return None;
        }
        for target in canonical_targets {
            let count = usize::from(target.count);
            let creases = plan
                .crease_pattern
                .edges
                .get(crease_cursor..crease_cursor.checked_add(count)?)?;
            if creases.iter().any(|edge| edge.start == edge.end)
                || creases
                    .iter()
                    .flat_map(|edge| [edge.start, edge.end])
                    .any(|id| {
                        !plan
                            .crease_pattern
                            .vertices
                            .iter()
                            .any(|vertex| vertex.id == id)
                    })
            {
                return None;
            }
            let squared_distance = |point: ori_domain::BeginnerSkeletonPointV1| {
                let dx = i64::from(target.position_tenths_mm[0])
                    .checked_sub(i64::from(point.x_tenths_mm))?;
                let dy = i64::from(target.position_tenths_mm[1])
                    .checked_sub(i64::from(point.y_tenths_mm))?;
                dx.checked_mul(dx)?.checked_add(dy.checked_mul(dy)?)
            };
            let (mount_distance, skeleton_segment_id, endpoint_rank) = canonical_skeleton_segments
                .as_deref()?
                .iter()
                .flat_map(|segment| [(segment, 0u8, segment.start), (segment, 1u8, segment.end)])
                .filter_map(|(segment, endpoint_rank, point)| {
                    squared_distance(point).map(|distance| (distance, segment.id, endpoint_rank))
                })
                .min()?;
            generic_feature_bindings.push(BeginnerGenericFeatureBindingWitness {
                protrusion_id: target.id,
                generated_feature_id: u8::try_from(target.id).ok()?,
                endpoint_count: target.count,
                crease_start: u16::try_from(crease_cursor).ok()?,
                crease_authority_sha256: sha2::Sha256::digest(
                    serde_json::to_vec(&creases.iter().map(|edge| edge.id).collect::<Vec<_>>())
                        .ok()?,
                )
                .into(),
                skeleton_segment_id,
                skeleton_endpoint: if endpoint_rank == 0 { "start" } else { "end" },
                mount_distance_squared_tenths_mm: u64::try_from(mount_distance).ok()?,
            });
            crease_cursor = crease_cursor.checked_add(count)?;
        }
        generic_feature_bindings.sort_unstable_by_key(|binding| binding.generated_feature_id);
        if generic_feature_bindings.windows(2).any(|pair| {
            pair[0].generated_feature_id >= pair[1].generated_feature_id
                || pair[0].protrusion_id >= pair[1].protrusion_id
        }) {
            return None;
        }
        if crease_cursor != contour_edge_end.checked_sub(witnessed)? {
            return None;
        }
    }
    if contour_vertex_end < witnessed || contour_edge_end < witnessed {
        return None;
    }
    let mut vertex_cursor = contour_vertex_end
        .checked_sub(witnessed)?
        .checked_add(body_contour_points)?;
    let mut crease_cursor = contour_edge_end
        .checked_sub(witnessed)?
        .checked_add(body_contour_points)?;
    for (index, binding) in local_bindings.iter_mut().enumerate() {
        binding.generated_face_id = u8::try_from(index.checked_add(1)?).ok()?;
        binding.vertex_start = u16::try_from(vertex_cursor).ok()?;
        binding.crease_start = u16::try_from(crease_cursor).ok()?;
        vertex_cursor = vertex_cursor.checked_add(usize::from(binding.contour_points))?;
        crease_cursor = crease_cursor.checked_add(usize::from(binding.contour_points))?;
    }
    if vertex_cursor != contour_vertex_end || crease_cursor != contour_edge_end {
        return None;
    }
    let mut max_contour_error_millionths = 0;
    if let Some(outline) = constraints.generic_body_outline_tenths_mm.as_deref() {
        let best = plan.crease_pattern.vertices[..contour_vertex_end]
            .windows(outline.len())
            .filter_map(|vertices| normalized_contour_error_millionths(outline, vertices))
            .min()?;
        max_contour_error_millionths = max_contour_error_millionths.max(best);
    }
    for binding in &mut local_bindings {
        let outline = constraints
            .protrusions
            .iter()
            .find(|target| target.id == binding.protrusion_id)?
            .local_outline_tenths_mm
            .as_deref()?;
        let (start, best) = plan.crease_pattern.vertices[..contour_vertex_end]
            .windows(outline.len())
            .enumerate()
            .filter_map(|(start, vertices)| {
                normalized_contour_error_millionths(outline, vertices).map(|score| (start, score))
            })
            .min_by_key(|(_, score)| *score)?;
        binding.vertex_start = u16::try_from(start).ok()?;
        binding.crease_start = u16::try_from(start).ok()?;
        max_contour_error_millionths = max_contour_error_millionths.max(best);
    }
    if max_contour_error_millionths > 1 {
        return None;
    }
    let mut skeleton_branch_bindings = Vec::new();
    if plan.kind == ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase {
        let segments = canonical_skeleton_segments.as_deref()?;
        let point =
            |point: ori_domain::BeginnerSkeletonPointV1| (point.x_tenths_mm, point.y_tenths_mm);
        let adjacency = |left: &ori_domain::BeginnerSkeletonSegmentV1,
                         right: &ori_domain::BeginnerSkeletonSegmentV1| {
            [("start", point(left.start)), ("end", point(left.end))]
                .into_iter()
                .find_map(|(left_endpoint, left_point)| {
                    [("start", point(right.start)), ("end", point(right.end))]
                        .into_iter()
                        .find(|(_, right_point)| *right_point == left_point)
                        .map(|(right_endpoint, _)| (left_endpoint, right_endpoint))
                })
        };
        let skeleton_points = segments
            .iter()
            .flat_map(|segment| [point(segment.start), point(segment.end)])
            .collect::<std::collections::BTreeSet<_>>();
        if segments
            .iter()
            .any(|segment| point(segment.start) == point(segment.end))
            || segments.len() != skeleton_points.len().saturating_sub(1)
            || segments.iter().enumerate().any(|(index, left)| {
                segments.iter().skip(index + 1).any(|right| {
                    (point(left.start) == point(right.start) && point(left.end) == point(right.end))
                        || (point(left.start) == point(right.end)
                            && point(left.end) == point(right.start))
                })
            })
        {
            return None;
        }
        let root_segment = &segments[0];
        let mut visited = HashSet::from([root_segment.id]);
        skeleton_branch_bindings.push(BeginnerSkeletonBranchBindingWitness {
            segment_id: root_segment.id,
            parent_segment_id: None,
            parent_endpoint: None,
            child_endpoint: None,
            generated_feature_ids: generic_feature_bindings
                .iter()
                .filter(|binding| binding.skeleton_segment_id == root_segment.id)
                .map(|binding| binding.generated_feature_id)
                .collect(),
        });
        while visited.len() < segments.len() {
            let next = segments.iter().find_map(|child| {
                (!visited.contains(&child.id)).then(|| {
                    segments.iter().find_map(|parent| {
                        visited.contains(&parent.id).then(|| {
                            adjacency(parent, child).map(|(parent_endpoint, child_endpoint)| {
                                (parent.id, child, parent_endpoint, child_endpoint)
                            })
                        })?
                    })
                })?
            });
            let (parent_id, child, parent_endpoint, child_endpoint) = next?;
            visited.insert(child.id);
            skeleton_branch_bindings.push(BeginnerSkeletonBranchBindingWitness {
                segment_id: child.id,
                parent_segment_id: Some(parent_id),
                parent_endpoint: Some(parent_endpoint),
                child_endpoint: Some(child_endpoint),
                generated_feature_ids: generic_feature_bindings
                    .iter()
                    .filter(|binding| binding.skeleton_segment_id == child.id)
                    .map(|binding| binding.generated_feature_id)
                    .collect(),
            });
        }
    }
    let skeleton_segments_for_authority = canonical_skeleton_segments
        .as_deref()
        .unwrap_or(&constraints.skeleton_segments);
    let skeleton_tree_authority_sha256: [u8; 32] = sha2::Sha256::digest(
        serde_json::to_vec(&(skeleton_segments_for_authority, &skeleton_branch_bindings)).ok()?,
    )
    .into();
    let topology_authority_hash: [u8; 32] = sha2::Sha256::digest(
        serde_json::to_vec(&(
            &constraints.generic_body_outline_tenths_mm,
            skeleton_segments_for_authority,
            &constraints.protrusions,
            &plan.crease_pattern,
        ))
        .ok()?,
    )
    .into();
    Some(BeginnerContourPlacementWitness {
        body_contour_points: u8::try_from(body_contour_points).ok()?,
        local_bindings,
        generic_feature_bindings,
        skeleton_branch_bindings,
        skeleton_tree_authority_sha256,
        witnessed_vertices: u16::try_from(witnessed).ok()?,
        witnessed_creases: u16::try_from(witnessed).ok()?,
        topology_authority_hash,
        max_contour_error_millionths,
    })
}

#[derive(Debug, Serialize)]
struct BeginnerGridCandidateResponse {
    point: ori_domain::BeginnerParameterGridPointV1,
    primary_score: u16,
    plan: ori_domain::BeginnerGeneratedPlanV1,
    assessment: BeginnerGeneratedPlanAssessment,
    local_proof_scope: &'static str,
    global_proof_scope: &'static str,
    complexity_score: u8,
    paper_efficiency_score: u8,
    scale_deviation_penalty: u16,
    spacing_deviation_penalty: u16,
    detail_mismatch_penalty: u16,
    outcome_reason: &'static str,
    contour_witness: BeginnerContourPlacementWitness,
    refinement_iterations: u8,
    strict_improvements: u8,
    refinement_starts: u8,
}

fn beginner_plan_paper_efficiency_score_v1(
    plan: &ori_domain::BeginnerGeneratedPlanV1,
    paper: &Paper,
) -> u8 {
    let boundary = paper
        .boundary_vertices
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let bounds = |points: Vec<Point2>| -> Option<[f64; 4]> {
        (!points.is_empty()).then(|| {
            points.iter().fold(
                [
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                ],
                |mut bounds, point| {
                    bounds[0] = bounds[0].min(point.x);
                    bounds[1] = bounds[1].max(point.x);
                    bounds[2] = bounds[2].min(point.y);
                    bounds[3] = bounds[3].max(point.y);
                    bounds
                },
            )
        })
    };
    let paper_bounds = bounds(
        plan.crease_pattern
            .vertices
            .iter()
            .filter(|vertex| boundary.contains(&vertex.id))
            .map(|vertex| vertex.position)
            .collect(),
    );
    let feature_bounds = bounds(
        plan.crease_pattern
            .vertices
            .iter()
            .filter(|vertex| !boundary.contains(&vertex.id))
            .map(|vertex| vertex.position)
            .collect(),
    );
    let (Some(paper), Some(feature)) = (paper_bounds, feature_bounds) else {
        return 0;
    };
    let width = (paper[1] - paper[0]).abs();
    let height = (paper[3] - paper[2]).abs();
    if width <= f64::EPSILON || height <= f64::EPSILON {
        return 0;
    }
    let horizontal = ((feature[1] - feature[0]).abs() / width).clamp(0.0, 1.0);
    let vertical = ((feature[3] - feature[2]).abs() / height).clamp(0.0, 1.0);
    ((horizontal + vertical) * 50.0).round() as u8
}

#[derive(Debug, Serialize)]
pub(super) struct BeginnerGridEvaluationResponse {
    request_generation_id: ProjectId,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    grid_hash: ori_domain::BeginnerParameterGridHashV1,
    evaluated_grid_points: u8,
    global_checked_candidates: u8,
    refinement_iterations: u8,
    candidates: Vec<BeginnerGridCandidateResponse>,
}

const MAX_BEGINNER_REFINEMENT_ITERATIONS_V1: u8 = 8;
const MAX_BEGINNER_REFINEMENT_PROPOSALS_V1: usize = 32;
const BEGINNER_REFINEMENT_STARTS_V1: u8 = 5;
const MAX_BEGINNER_GENERIC_TREE_ORIENTATIONS_V1: usize = 2;
const MAX_BEGINNER_GENERIC_TREE_PRIMARY_WORK_V1: usize =
    ori_domain::BEGINNER_PARAMETER_GRID_SIZE_V1 * MAX_BEGINNER_GENERIC_TREE_ORIENTATIONS_V1;

pub(super) fn validate_beginner_manufacturability_v1(
    pattern: &CreasePattern,
    paper: &Paper,
) -> Result<(), &'static str> {
    const MIN_CREASE_SPACING_MM: f64 = 1.0e-6;
    const MIN_FACE_AREA_MM2: f64 = 1.0e-8;
    let positions = pattern
        .vertices
        .iter()
        .map(|vertex| (vertex.id, vertex.position))
        .collect::<HashMap<_, _>>();
    for edge in &pattern.edges {
        let (Some(start), Some(end)) = (positions.get(&edge.start), positions.get(&edge.end))
        else {
            return Err("manufacturability_missing_vertex");
        };
        let edge_length = ori_numeric::deterministic_hypot_v1(start.x - end.x, start.y - end.y)
            .map_err(|_| "manufacturability_non_finite_geometry")?;
        if edge_length < MIN_CREASE_SPACING_MM {
            return Err("manufacturability_minimum_crease_spacing");
        }
    }
    if pattern.vertices.len() >= 3 {
        let origin = pattern.vertices[0].position;
        let doubled_area = pattern.vertices[1..]
            .windows(2)
            .map(|pair| {
                let a = pair[0].position;
                let b = pair[1].position;
                (a.x - origin.x) * (b.y - origin.y) - (a.y - origin.y) * (b.x - origin.x)
            })
            .sum::<f64>()
            .abs();
        if !doubled_area.is_finite() || doubled_area * 0.5 < MIN_FACE_AREA_MM2 {
            return Err("manufacturability_minimum_face_area");
        }
    }
    let boundary = paper
        .boundary_vertices
        .iter()
        .filter_map(|id| positions.get(id));
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
    );
    for point in boundary {
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
        min_y = min_y.min(point.y);
        max_y = max_y.max(point.y);
    }
    if min_x.is_finite()
        && pattern.vertices.iter().any(|vertex| {
            vertex.position.x < min_x - MIN_CREASE_SPACING_MM
                || vertex.position.x > max_x + MIN_CREASE_SPACING_MM
                || vertex.position.y < min_y - MIN_CREASE_SPACING_MM
                || vertex.position.y > max_y + MIN_CREASE_SPACING_MM
        })
    {
        return Err("manufacturability_paper_boundary_margin");
    }
    Ok(())
}

fn refine_beginner_grid_plan_v1(
    namespace: ProjectId,
    source: &CreasePattern,
    boundary_vertices: &[VertexId],
    paper: &Paper,
    profile: &ori_domain::BeginnerDesignProfileV1,
    expected_kind: ori_domain::BeginnerGeneratedPlanKindV1,
    reference: Option<&BeginnerReferenceModelSuggestionV1>,
    work: &BeginnerGridWork,
    deadline: std::time::Instant,
    initial_point: ori_domain::BeginnerParameterGridPointV1,
    initial_plan: ori_domain::BeginnerGeneratedPlanV1,
) -> Result<
    (
        ori_domain::BeginnerParameterGridPointV1,
        ori_domain::BeginnerGeneratedPlanV1,
        u8,
        u8,
        u8,
    ),
    String,
> {
    let Some(reference) = reference else {
        return Ok((initial_point, initial_plan, 0, 0, 1));
    };
    let mut point = initial_point;
    let mut plan = initial_plan;
    let prefer_vertical = plan
        .instruction_codes
        .iter()
        .any(|code| code == "bounded_tree_paper_orientation_v1:vertical");
    let mut score = preset_weighted_refinement_score_v1(&plan, reference, profile);
    let initial_score = score;
    let mut iterations = 0_u8;
    let mut improvements = 0_u8;
    let mut proposals = 0_usize;
    for (scale_delta, spacing_delta) in [(-4_i16, 0_i16), (4, 0), (0, -6), (0, 6)] {
        if beginner_grid_cancelled_v1(work) {
            return Err("grid_evaluation_cancelled".to_owned());
        }
        if std::time::Instant::now() >= deadline {
            break;
        }
        proposals += 1;
        let mut seed_point = initial_point;
        seed_point.scale_percent =
            (i16::from(initial_point.scale_percent) + scale_delta).clamp(10, 45) as u8;
        seed_point.spacing_percent =
            (i16::from(initial_point.spacing_percent) + spacing_delta).clamp(20, 80) as u8;
        if seed_point == initial_point {
            continue;
        }
        let Some(seed_plan) =
            grid_template_plan(namespace, source, boundary_vertices, profile, seed_point)
                .map_err(|_| "grid_refinement_generation_failed".to_owned())?
                .into_iter()
                .find(|candidate| {
                    candidate.kind == expected_kind
                        && candidate.instruction_codes.iter().any(|code| {
                            (code == "bounded_tree_paper_orientation_v1:vertical")
                                == prefer_vertical
                        })
                })
        else {
            continue;
        };
        if beginner_contour_placement_witness(&profile.generation_constraints, &seed_plan).is_none()
            || validate_beginner_manufacturability_v1(&seed_plan.crease_pattern, paper).is_err()
        {
            continue;
        }
        let seed_score = preset_weighted_refinement_score_v1(&seed_plan, reference, profile);
        if seed_score > score
            || (seed_score == score
                && (seed_point.scale_percent, seed_point.spacing_percent)
                    < (point.scale_percent, point.spacing_percent))
        {
            score = seed_score;
            point = seed_point;
            plan = seed_plan;
        }
    }
    if score > initial_score {
        improvements = 1;
    }
    for _ in 0..MAX_BEGINNER_REFINEMENT_ITERATIONS_V1 {
        if beginner_grid_cancelled_v1(work) {
            return Err("grid_evaluation_cancelled".to_owned());
        }
        if std::time::Instant::now() >= deadline {
            break;
        }
        let mut best: Option<(
            u8,
            ori_domain::BeginnerParameterGridPointV1,
            ori_domain::BeginnerGeneratedPlanV1,
        )> = None;
        for (scale_delta, spacing_delta) in [(-2_i16, 0_i16), (2, 0), (0, -3), (0, 3)] {
            if proposals >= MAX_BEGINNER_REFINEMENT_PROPOSALS_V1 {
                break;
            }
            proposals += 1;
            let mut candidate_point = point;
            candidate_point.scale_percent =
                (i16::from(point.scale_percent) + scale_delta).clamp(10, 45) as u8;
            candidate_point.spacing_percent =
                (i16::from(point.spacing_percent) + spacing_delta).clamp(20, 80) as u8;
            if candidate_point == point {
                continue;
            }
            let Some(candidate_plan) = grid_template_plan(
                namespace,
                source,
                boundary_vertices,
                profile,
                candidate_point,
            )
            .map_err(|_| "grid_refinement_generation_failed".to_owned())?
            .into_iter()
            .find(|candidate| {
                candidate.kind == expected_kind
                    && candidate.instruction_codes.iter().any(|code| {
                        (code == "bounded_tree_paper_orientation_v1:vertical") == prefer_vertical
                    })
            }) else {
                continue;
            };
            if beginner_contour_placement_witness(&profile.generation_constraints, &candidate_plan)
                .is_none()
                || validate_beginner_manufacturability_v1(&candidate_plan.crease_pattern, paper)
                    .is_err()
            {
                continue;
            }
            let candidate_score =
                preset_weighted_refinement_score_v1(&candidate_plan, reference, profile);
            let replaces = candidate_score > score
                && best.as_ref().is_none_or(|current| {
                    (
                        candidate_score,
                        std::cmp::Reverse(candidate_point.scale_percent),
                        std::cmp::Reverse(candidate_point.spacing_percent),
                    ) > (
                        current.0,
                        std::cmp::Reverse(current.1.scale_percent),
                        std::cmp::Reverse(current.1.spacing_percent),
                    )
                });
            if replaces {
                best = Some((candidate_score, candidate_point, candidate_plan));
            }
        }
        iterations = iterations.saturating_add(1);
        work.refinement_iterations.fetch_add(1, Ordering::Release);
        let Some((next_score, next_point, next_plan)) = best else {
            break;
        };
        debug_assert!(next_score > score);
        score = next_score;
        point = next_point;
        plan = next_plan;
        improvements = improvements.saturating_add(1);
    }
    Ok((
        point,
        plan,
        iterations,
        improvements,
        BEGINNER_REFINEMENT_STARTS_V1,
    ))
}

#[tauri::command]
pub(super) fn evaluate_beginner_parameter_grid(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    request_generation_id: ProjectId,
) -> Result<BeginnerGridEvaluationResponse, String> {
    let work = Arc::new(BeginnerGridWork::default());
    run_registered_beginner_grid_work_v1(request_generation_id, &work, || {
        let control = beginner_candidate_analysis_control_v1(
            Some(&work.cancelled),
            "grid_evaluation_cancelled",
            "grid_evaluation_deadline_exceeded",
        );
        let expectation = ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        );
        let snapshot = {
            let project = lock_project(&state)?;
            control.checkpoint()?;
            capture_beginner_candidate_analysis_snapshot_with_control_v1(
                &project,
                expectation,
                &control,
            )?
        };
        control.checkpoint()?;
        let project = &snapshot.project;
        let profile = project.editor.beginner_design_profile();
        if !target_asset_reference_is_live(&project, profile.generation_constraints.target_asset) {
            return Err("missing_target_asset".to_owned());
        }
        let estimate =
            ori_domain::estimate_symmetric_parameters_v1(&profile.generation_constraints)
                .ok_or_else(|| "symmetric_parameter_estimate_unsupported".to_owned())?;
        let grid = ori_domain::beginner_parameter_grid_v1();
        let expected_kind = symmetric_plan_kind(profile);
        let mut primary = Vec::with_capacity(MAX_BEGINNER_GENERIC_TREE_PRIMARY_WORK_V1);
        for point in grid.iter().copied() {
            control.checkpoint()?;
            let plans = grid_template_plan(
                project.project_id,
                project.editor.pattern(),
                &project.editor.paper().boundary_vertices,
                profile,
                point,
            )
            .map_err(|_| "grid_template_generation_failed".to_owned())?
            .into_iter()
            .filter(|plan| plan.kind == expected_kind)
            .take(MAX_BEGINNER_GENERIC_TREE_ORIENTATIONS_V1)
            .collect::<Vec<_>>();
            if plans.is_empty() {
                return Err("grid_template_generation_failed".to_owned());
            }
            let deviation = u16::from(point.scale_percent.abs_diff(estimate.scale_percent)) * 10
                + u16::from(point.spacing_percent.abs_diff(estimate.spacing_percent)) * 5;
            let detail_penalty =
                if point.detail_level == profile.generation_constraints.detail_level {
                    0
                } else {
                    10
                };
            for plan in plans {
                primary.push((
                    1000_u16.saturating_sub(deviation + detail_penalty),
                    point,
                    plan,
                ));
            }
            work.enumerated.fetch_add(1, Ordering::Release);
        }
        control.checkpoint()?;
        primary.sort_by_key(|(score, point, _)| (std::cmp::Reverse(*score), point.id));
        primary.retain(|(_, _, plan)| {
            beginner_contour_placement_witness(&profile.generation_constraints, plan).is_some()
        });
        if primary.len() < 3 {
            return Err("grid_contour_candidate_shortage".to_owned());
        }
        primary.truncate(3);

        control.checkpoint()?;
        let reference = live_reference_model_suggestion_v1(&project).ok();
        let mut candidates = primary
            .into_iter()
            .map(|(_primary_score, point, plan)| {
                control.checkpoint()?;
                let (point, plan, refinement_iterations, strict_improvements, refinement_starts) =
                    refine_beginner_grid_plan_v1(
                        project.project_id,
                        project.editor.pattern(),
                        &project.editor.paper().boundary_vertices,
                        project.editor.paper(),
                        profile,
                        expected_kind,
                        reference.as_ref(),
                        &work,
                        control.deadline,
                        point,
                        plan,
                    )?;
                control.checkpoint()?;
                let detail_penalty =
                    if point.detail_level == profile.generation_constraints.detail_level {
                        0
                    } else {
                        10
                    };
                let primary_score = 1000_u16.saturating_sub(
                    u16::from(point.scale_percent.abs_diff(estimate.scale_percent)) * 10
                        + u16::from(point.spacing_percent.abs_diff(estimate.spacing_percent)) * 5
                        + detail_penalty,
                );
                let assessment = assess_beginner_generated_plan_with_control_v1(
                    project.project_id,
                    project.editor.paper(),
                    project.editor.pattern(),
                    &plan,
                    reference.as_ref(),
                    control.deadline,
                    Some(&work.cancelled),
                );
                control.checkpoint()?;
                let global_proof_scope = assessment.proof_scope;
                let outcome_reason = assessment.reason;
                let complexity_score = u8::try_from(
                    plan.crease_pattern.edges.len().saturating_mul(10)
                        + match point.detail_level {
                            ori_domain::BeginnerDetailLevelV1::Simple => 10,
                            ori_domain::BeginnerDetailLevelV1::Standard => 20,
                            ori_domain::BeginnerDetailLevelV1::Detailed => 30,
                        },
                )
                .unwrap_or(100)
                .min(100);
                let contour_witness =
                    beginner_contour_placement_witness(&profile.generation_constraints, &plan)
                        .ok_or_else(|| "grid_contour_witness_invalid".to_owned())?;
                let paper_efficiency_score =
                    beginner_plan_paper_efficiency_score_v1(&plan, project.editor.paper());
                work.global_checked.fetch_add(1, Ordering::Release);
                Ok(BeginnerGridCandidateResponse {
                    point,
                    primary_score,
                    plan,
                    assessment,
                    local_proof_scope: "necessary",
                    global_proof_scope,
                    complexity_score,
                    paper_efficiency_score,
                    scale_deviation_penalty: u16::from(
                        point.scale_percent.abs_diff(estimate.scale_percent),
                    ) * 10,
                    spacing_deviation_penalty: u16::from(
                        point.spacing_percent.abs_diff(estimate.spacing_percent),
                    ) * 5,
                    detail_mismatch_penalty: detail_penalty,
                    outcome_reason,
                    contour_witness,
                    refinement_iterations,
                    strict_improvements,
                    refinement_starts,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        control.checkpoint()?;
        candidates
            .retain(|candidate| candidate.assessment.reason != "folded_pose_simulation_failed");
        if candidates.is_empty() {
            return Err("grid_folded_simulation_unavailable".to_owned());
        }
        let current = lock_project(&state)?;
        beginner_candidate_snapshot_is_current_v1(&current, &snapshot)?;
        control.checkpoint()?;
        Ok(BeginnerGridEvaluationResponse {
            request_generation_id,
            project_instance_id: project.instance_id,
            project_id: project.project_id,
            revision: snapshot.expectation.revision,
            grid_hash: ori_domain::beginner_parameter_grid_hash_v1(&grid),
            evaluated_grid_points: ori_domain::BEGINNER_PARAMETER_GRID_SIZE_V1 as u8,
            global_checked_candidates: 3,
            refinement_iterations: work
                .refinement_iterations
                .load(Ordering::Acquire)
                .min(u64::from(MAX_BEGINNER_REFINEMENT_ITERATIONS_V1) * 3)
                as u8,
            candidates,
        })
    })
}

#[derive(Serialize)]
pub(super) struct BeginnerGridProgressResponse {
    pub(super) request_generation_id: ProjectId,
    pub(super) enumerated_grid_points: u8,
    pub(super) global_checked_candidates: u8,
    pub(super) refinement_iterations: u8,
    pub(super) terminal_state: &'static str,
}

#[tauri::command]
pub(super) fn get_beginner_parameter_grid_progress(
    request_generation_id: ProjectId,
) -> Result<BeginnerGridProgressResponse, String> {
    let registry = lock_recovering_registry_v1(beginner_grid_work());
    let work = registry
        .get(&request_generation_id)
        .ok_or_else(|| "grid_generation_not_running".to_owned())?;
    let terminal_state = match work.terminal.load(Ordering::Acquire) {
        0 => "running",
        1 => "completed",
        2 => "cancelled",
        _ => "failed",
    };
    Ok(BeginnerGridProgressResponse {
        request_generation_id,
        enumerated_grid_points: work.enumerated.load(Ordering::Acquire).min(27) as u8,
        global_checked_candidates: work.global_checked.load(Ordering::Acquire).min(3) as u8,
        refinement_iterations: work
            .refinement_iterations
            .load(Ordering::Acquire)
            .min(u64::from(MAX_BEGINNER_REFINEMENT_ITERATIONS_V1) * 3)
            as u8,
        terminal_state,
    })
}

#[tauri::command]
pub(super) fn cancel_beginner_parameter_grid(
    request_generation_id: ProjectId,
) -> Result<(), String> {
    let registry = lock_recovering_registry_v1(beginner_grid_work());
    let work = registry
        .get(&request_generation_id)
        .ok_or_else(|| "grid_generation_not_running".to_owned())?;
    request_work_cancellation_v1(&work.cancelled, &work.terminal);
    Ok(())
}

pub(super) fn configure_symmetric_profile(
    profile: &mut ori_domain::BeginnerDesignProfileV1,
    estimate: ori_domain::BeginnerSymmetricParameterEstimateV1,
    scale_percent: u8,
    spacing_percent: u8,
) {
    let insect = profile.generation_constraints.target_category
        == Some(ori_domain::BeginnerTargetCategoryV1::Insect);
    let single_tail = profile
        .generation_constraints
        .target_parts
        .iter()
        .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Tail && part.count == 1);
    let single_horn = profile
        .generation_constraints
        .target_parts
        .iter()
        .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Horn && part.count == 1);
    let single_antenna = profile
        .generation_constraints
        .target_parts
        .iter()
        .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Antenna && part.count == 1);
    let tail_ear = profile
        .generation_constraints
        .target_parts
        .iter()
        .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Tail && part.count == 1)
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Ear && part.count == 2);
    let horn_ear = single_horn
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Ear && part.count == 2);
    let horn_tail = single_horn && single_tail;
    let horn_tail_ear = horn_tail && tail_ear && horn_ear;
    let complete_animal = horn_tail_ear
        && profile.generation_constraints.target_category
            == Some(ori_domain::BeginnerTargetCategoryV1::Animal)
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Leg && part.count == 4);
    let winged_animal = complete_animal
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Wing && part.count == 2);
    let wing_antenna = insect
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Wing && part.count == 2)
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| {
                part.kind == ori_domain::BeginnerTargetPartKindV1::Antenna && part.count == 2
            });
    let complete_insect = wing_antenna
        && profile
            .generation_constraints
            .target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Leg && part.count == 6);
    let skeleton = |id, start_x, start_y, end_x, end_y| ori_domain::BeginnerSkeletonSegmentV1 {
        id,
        start: ori_domain::BeginnerSkeletonPointV1 {
            x_tenths_mm: start_x,
            y_tenths_mm: start_y,
        },
        end: ori_domain::BeginnerSkeletonPointV1 {
            x_tenths_mm: end_x,
            y_tenths_mm: end_y,
        },
        thickness_tenths_mm: 50,
    };
    profile.generation_constraints.skeleton_segments = if insect {
        vec![
            skeleton(1, -500, -500, 0, 500),
            skeleton(2, 500, -500, 0, 500),
        ]
    } else {
        vec![
            skeleton(1, -500, 0, 0, 500),
            skeleton(2, 500, 0, 0, 500),
            skeleton(3, 0, -500, 0, 500),
        ]
    };
    profile.generation_constraints.protrusions = vec![ori_domain::BeginnerProtrusionTargetV1 {
        id: 1,
        count: estimate.protrusion_count,
        length_tenths_mm: u32::from(scale_percent) * 10,
        thickness_tenths_mm: u16::from(spacing_percent) * 2,
        root_width_tenths_mm: None,
        tip_width_tenths_mm: None,
        local_outline_tenths_mm: None,
        position_tenths_mm: [0, 0, 0],
        direction_milli: if single_horn || single_antenna {
            [0, -1000, 0]
        } else if insect || single_tail {
            [1000, 0, 0]
        } else {
            [0, 1000, 0]
        },
        symmetry: if single_tail || single_horn || single_antenna {
            ori_domain::BeginnerProtrusionSymmetryV1::None
        } else {
            ori_domain::BeginnerProtrusionSymmetryV1::Bilateral
        },
        curvature_degrees: 0,
        joint: ori_domain::BeginnerProtrusionJointV1::Fixed,
        motion_degrees: [0, 0],
        side: ori_domain::BeginnerProtrusionSideV1::Either,
        priority: 50,
    }];
    if tail_ear || horn_ear || horn_tail {
        profile.generation_constraints.protrusions[0].count = 1;
        profile.generation_constraints.protrusions[0].symmetry =
            ori_domain::BeginnerProtrusionSymmetryV1::None;
        profile.generation_constraints.protrusions[0].direction_milli = if horn_ear || horn_tail {
            [0, -1000, 0]
        } else {
            [1000, 0, 0]
        };
        let mut secondary = profile.generation_constraints.protrusions[0].clone();
        secondary.id = 2;
        secondary.direction_milli = [1000, 0, 0];
        secondary.count = if horn_tail { 1 } else { 2 };
        secondary.symmetry = if horn_tail {
            ori_domain::BeginnerProtrusionSymmetryV1::None
        } else {
            ori_domain::BeginnerProtrusionSymmetryV1::Bilateral
        };
        profile.generation_constraints.protrusions.push(secondary);
        if horn_tail_ear {
            let mut ears = profile.generation_constraints.protrusions[0].clone();
            ears.id = 3;
            ears.count = 2;
            ears.symmetry = ori_domain::BeginnerProtrusionSymmetryV1::Bilateral;
            ears.direction_milli = [1000, 0, 0];
            profile.generation_constraints.protrusions.push(ears);
            if complete_animal {
                let mut legs = profile.generation_constraints.protrusions[0].clone();
                legs.id = 4;
                legs.count = 4;
                legs.symmetry = ori_domain::BeginnerProtrusionSymmetryV1::Bilateral;
                legs.direction_milli = [0, 1000, 0];
                profile.generation_constraints.protrusions.push(legs);
                if winged_animal {
                    let mut wings = profile.generation_constraints.protrusions[2].clone();
                    wings.id = 5;
                    wings.count = 2;
                    wings.symmetry = ori_domain::BeginnerProtrusionSymmetryV1::Bilateral;
                    wings.direction_milli = [1000, 0, 0];
                    wings.priority = 60;
                    profile.generation_constraints.protrusions.push(wings);
                }
            }
        }
    }
    if wing_antenna {
        profile.generation_constraints.protrusions[0].count = 2;
        profile.generation_constraints.protrusions[0].direction_milli = [1000, 0, 0];
        profile.generation_constraints.protrusions[0].priority = 60;
        let mut antennae = profile.generation_constraints.protrusions[0].clone();
        antennae.id = 2;
        antennae.direction_milli = [0, -1000, 0];
        profile.generation_constraints.protrusions.push(antennae);
        if complete_insect {
            for (index, center_y) in [-250, 0, 250].into_iter().enumerate() {
                let mut legs = profile.generation_constraints.protrusions[0].clone();
                legs.id = index as u16 + 3;
                legs.priority = 50;
                legs.position_tenths_mm[1] = center_y;
                profile.generation_constraints.protrusions.push(legs);
            }
        }
    }
}

#[tauri::command]
pub(super) fn apply_beginner_symmetric_parameters(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    expected_estimate: ori_domain::BeginnerSymmetricParameterEstimateV1,
    scale_percent: u8,
    spacing_percent: u8,
    confirmed: bool,
) -> Result<ProjectSnapshot, String> {
    if !confirmed || !(10..=45).contains(&scale_percent) || !(20..=80).contains(&spacing_percent) {
        return Err("symmetric_parameter_confirmation_required".to_owned());
    }
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_and_expect(&state, expectation)?;
    let mut profile = project.editor.beginner_design_profile().clone();
    let live = ori_domain::estimate_symmetric_parameters_v1(&profile.generation_constraints)
        .ok_or_else(|| "symmetric_parameter_estimate_stale".to_owned())?;
    if live != expected_estimate {
        return Err("symmetric_parameter_estimate_stale".to_owned());
    }
    configure_symmetric_profile(&mut profile, live, scale_percent, spacing_percent);
    execute_expected_command(
        &mut project,
        expectation,
        Command::UpdateBeginnerDesignProfile {
            profile: Box::new(profile),
        },
    )
}

#[tauri::command]
pub(super) fn archive_beginner_reference_model_asset(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    asset_id: AssetId,
    archived: bool,
) -> Result<ProjectSnapshot, String> {
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_and_expect(&state, expectation)?;
    if !project
        .reference_model_assets
        .iter()
        .any(|asset| asset.id == asset_id)
    {
        return Err("reference_model_asset_stale".to_owned());
    }
    let mut profile = project.editor.beginner_design_profile().clone();
    profile
        .archived_reference_model_asset_ids
        .retain(|id| *id != asset_id);
    if archived {
        profile.archived_reference_model_asset_ids.push(asset_id);
        if profile.generation_constraints.target_asset
            == Some(ori_domain::BeginnerTargetAssetReferenceV1::ReferenceModel { asset_id })
        {
            profile.generation_constraints.target_asset = None;
        }
    }
    execute_expected_command(
        &mut project,
        expectation,
        Command::UpdateBeginnerDesignProfile {
            profile: Box::new(profile),
        },
    )
}

#[tauri::command]
pub(super) fn apply_beginner_generated_plan(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    expected_profile: ori_domain::BeginnerDesignProfileV1,
    selected_kind: ori_domain::BeginnerGeneratedPlanKindV1,
    expected_candidate_edge_id: EdgeId,
) -> Result<ProjectSnapshot, String> {
    apply_beginner_generated_plan_document(
        &state,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        expected_profile,
        selected_kind,
        expected_candidate_edge_id,
    )
}

pub(super) fn apply_beginner_generated_plan_document(
    state: &AppState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    expected_profile: ori_domain::BeginnerDesignProfileV1,
    selected_kind: ori_domain::BeginnerGeneratedPlanKindV1,
    expected_candidate_edge_id: EdgeId,
) -> Result<ProjectSnapshot, String> {
    if !matches!(
        selected_kind,
        ori_domain::BeginnerGeneratedPlanKindV1::DiagonalFold
            | ori_domain::BeginnerGeneratedPlanKindV1::SymmetricFourLegBase
            | ori_domain::BeginnerGeneratedPlanKindV1::SymmetricWingBase
            | ori_domain::BeginnerGeneratedPlanKindV1::SymmetricBirdBase
            | ori_domain::BeginnerGeneratedPlanKindV1::AsymmetricBirdLandmarkBase
            | ori_domain::BeginnerGeneratedPlanKindV1::AsymmetricFourLegLandmarkBase
            | ori_domain::BeginnerGeneratedPlanKindV1::AsymmetricInsectLandmarkBase
            | ori_domain::BeginnerGeneratedPlanKindV1::AsymmetricFishLandmarkBase
            | ori_domain::BeginnerGeneratedPlanKindV1::SymmetricFishBase
            | ori_domain::BeginnerGeneratedPlanKindV1::SymmetricEarBase
            | ori_domain::BeginnerGeneratedPlanKindV1::SymmetricHornBase
            | ori_domain::BeginnerGeneratedPlanKindV1::SymmetricAntennaBase
            | ori_domain::BeginnerGeneratedPlanKindV1::SymmetricInsectLegPairBase
            | ori_domain::BeginnerGeneratedPlanKindV1::SymmetricSixLegBase
            | ori_domain::BeginnerGeneratedPlanKindV1::CenterAxisTailBase
            | ori_domain::BeginnerGeneratedPlanKindV1::CenterAxisHornBase
            | ori_domain::BeginnerGeneratedPlanKindV1::CenterAxisAntennaBase
            | ori_domain::BeginnerGeneratedPlanKindV1::CompositeTailEarBase
            | ori_domain::BeginnerGeneratedPlanKindV1::CompositeHornEarBase
            | ori_domain::BeginnerGeneratedPlanKindV1::CompositeHornTailBase
            | ori_domain::BeginnerGeneratedPlanKindV1::CompositeHornTailEarBase
            | ori_domain::BeginnerGeneratedPlanKindV1::CompositeWingAntennaBase
            | ori_domain::BeginnerGeneratedPlanKindV1::CompositeCompleteInsectBase
            | ori_domain::BeginnerGeneratedPlanKindV1::CompositeCompleteAnimalBase
            | ori_domain::BeginnerGeneratedPlanKindV1::CompositeCompleteWingedAnimalBase
            | ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
    ) {
        return Err("the selected generated plan is preview-only".to_owned());
    }
    let mut project = lock_and_expect(
        state,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
    )?;
    if project.editor.beginner_design_profile() != &expected_profile {
        return Err("the beginner design profile changed before apply".to_owned());
    }
    if !component_bridge_override_is_live_v1(&project, &expected_profile) {
        return Err("component_bridge_override_stale_or_disconnected".to_owned());
    }
    if !reference_consensus_is_live_v1(&project, &expected_profile) {
        return Err("reference_consensus_asset_binding_stale".to_owned());
    }
    if !target_asset_reference_is_live(
        &project,
        expected_profile.generation_constraints.target_asset,
    ) {
        return Err("the target reference image changed before apply".to_owned());
    }
    let plans = ori_domain::generate_beginner_plans_v1(
        project.project_id,
        project.editor.pattern(),
        &project.editor.paper().boundary_vertices,
        &expected_profile.generation_constraints,
    )
    .map_err(|_| "the generated plan is no longer applicable".to_owned())?;
    let plan = plans
        .into_iter()
        .find(|plan| plan.kind == selected_kind)
        .ok_or_else(|| "the generated plan is no longer available".to_owned())?;
    let semantic_landmark_provenance = plan.semantic_landmark_provenance.clone();
    if plan.crease_pattern.edges.first().map(|edge| edge.id) != Some(expected_candidate_edge_id) {
        return Err("the generated candidate identity changed before apply".to_owned());
    }
    let reference_suggestion = live_reference_model_suggestion_v1(&project).ok();
    if reference_suggestion
        .as_ref()
        .and_then(|reference| beginner_multi_reference_fusion_v1(&project, reference))
        .is_some_and(|fusion| !fusion.apply_allowed)
    {
        return Err("multi_reference_disagreement".to_owned());
    }
    if expected_profile.reference_consensus_v1.is_some() {
        let analysis = beginner_reference_consensus_analysis_v1(&project, None)
            .ok_or_else(|| "reference_consensus_analysis_unavailable".to_owned())?;
        if !analysis.apply_allowed {
            return Err("reference_consensus_multiple_disagreements".to_owned());
        }
    }
    let assessment = assess_beginner_generated_plan(
        project.project_id,
        project.editor.paper(),
        project.editor.pattern(),
        &plan,
        reference_suggestion.as_ref(),
    );
    if assessment.expected_candidate_edge_id != expected_candidate_edge_id {
        return Err("the generated candidate identity changed before apply".to_owned());
    }
    if !assessment.apply_allowed {
        return Err(format!(
            "the generated plan failed validation: {}",
            assessment.reason
        ));
    }
    let mut certificate_pattern = project.editor.pattern().clone();
    for vertex in &plan.crease_pattern.vertices {
        if !certificate_pattern
            .vertices
            .iter()
            .any(|current| current.id == vertex.id)
        {
            certificate_pattern.vertices.push(vertex.clone());
        }
    }
    certificate_pattern
        .edges
        .extend(plan.crease_pattern.edges.iter().cloned());
    let certificate_editor =
        EditorState::with_paper(certificate_pattern.clone(), project.editor.paper().clone());
    let certificate_topology = certificate_editor
        .topology_analysis_input(project.project_id)
        .analyze();
    let fold_path_certificate_sha256 = certify_beginner_fold_path_v1(
        &plan,
        project.editor.paper(),
        &certificate_pattern,
        certificate_topology
            .simulation_snapshot()
            .ok_or_else(|| "the generated plan topology changed before apply".to_owned())?,
    )
    .ok_or_else(|| "the generated plan fold path changed before apply".to_owned())?;
    let mut pattern = project.editor.pattern().clone();
    for vertex in plan.crease_pattern.vertices {
        if !pattern
            .vertices
            .iter()
            .any(|current| current.id == vertex.id)
        {
            pattern.vertices.push(vertex);
        }
    }
    for edge in plan.crease_pattern.edges {
        if pattern.edges.iter().any(|current| current.id == edge.id) {
            return Err("the generated plan was already applied".to_owned());
        }
        pattern.edges.push(edge);
    }
    let mut instruction_timeline = project.editor.instruction_timeline().clone();
    let (title, description, caution) = match selected_kind {
        ori_domain::BeginnerGeneratedPlanKindV1::SymmetricFourLegBase => (
            "Symmetric four-leg base",
            "Create the four bounded base creases around the shared center.",
            "Confirm that the saved four-leg target and bilateral protrusion still match.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::SymmetricWingBase => (
            "Symmetric wing base",
            "Create the four bounded base creases for the bilateral wing layout.",
            "Confirm that the saved two-wing target and bilateral protrusion still match.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::SymmetricBirdBase => (
            "Symmetric bird base",
            "Create the bounded bilateral bird-wing base creases.",
            "Confirm the saved head, torso, and two-wing target still match.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::AsymmetricBirdLandmarkBase
        | ori_domain::BeginnerGeneratedPlanKindV1::AsymmetricFourLegLandmarkBase => (
            "Asymmetric landmark bird base",
            "Create individually bound head, tail, left-wing, and right-wing landmark creases.",
            "The asymmetric landmark bindings and native fold-path certificate were revalidated.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::AsymmetricInsectLandmarkBase => (
            "Asymmetric insect landmark base",
            "Apply the certified four-ray geometry bound to ten ordered insect landmarks.",
            "Head, tail, two wings, and six legs retain bounded semantic provenance grouped by ray digest.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::AsymmetricFishLandmarkBase => (
            "Asymmetric fish landmark base",
            "Apply certified four-ray geometry bound to head, tail, and two ordered fin landmarks.",
            "All semantic bindings, ray-group digests, and the native fold path were revalidated.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::SymmetricFishBase => (
            "Symmetric fish base",
            "Create the bounded bilateral fish-fin base creases.",
            "Confirm the saved head, torso, and two-fin target still match.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::SymmetricEarBase => (
            "Symmetric ear base",
            "Create the bounded bilateral long-ear base creases.",
            "Confirm the saved head, torso, and two-ear target still match.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::SymmetricHornBase => (
            "Symmetric horn base",
            "Create the bounded bilateral horn base creases.",
            "Confirm the saved head, torso, and two-horn target still match.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::SymmetricAntennaBase => (
            "Symmetric antenna base",
            "Create the bounded bilateral insect-antenna base creases.",
            "Confirm the saved head, torso, and two-antenna target still match.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::SymmetricInsectLegPairBase => (
            "Symmetric insect leg pair base",
            "Create one bounded bilateral insect-leg pair base.",
            "This limited family represents exactly two legs, not a complete six-leg insect.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::SymmetricSixLegBase => (
            "Symmetric complete six-leg base",
            "Create three individually bound bilateral insect-leg pairs.",
            "All three pair positions and the global proof were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::CenterAxisTailBase => (
            "Center-axis tail base",
            "Create one bounded tail ray from the body center axis.",
            "The target remains a limited single-tail family and is revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::CenterAxisHornBase => (
            "Center-axis single-horn base",
            "Create one bounded horn ray from the body center axis.",
            "The limited single-horn target and global proof were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::CenterAxisAntennaBase => (
            "Center-axis single-antenna base",
            "Create one bounded antenna ray from the insect center axis.",
            "The limited single-antenna target and global proof were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::CompositeTailEarBase => (
            "Composite tail and ear base",
            "Create one center-axis tail and one individually bound bilateral ear pair.",
            "Both bindings and the global proof were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::CompositeHornEarBase => (
            "Composite horn and ear base",
            "Create one center-axis horn and one individually bound bilateral ear pair.",
            "Both bindings and the global proof were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::CompositeHornTailBase => (
            "Composite horn and tail base",
            "Create individually bound center-axis horn and tail rays.",
            "Both bindings and the global proof were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::CompositeHornTailEarBase => (
            "Composite horn, tail, and ear base",
            "Create individually bound horn, tail, and bilateral ear rays.",
            "All three bindings and the global proof were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::CompositeWingAntennaBase => (
            "Composite wing and antenna base",
            "Create individually bound bilateral wing and antenna pairs.",
            "Both pair bindings and the global proof were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::CompositeCompleteInsectBase => (
            "Complete composite insect base",
            "Create five individually bound bilateral wing, antenna, and leg pairs.",
            "All five pair bindings and the global proof were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::CompositeCompleteAnimalBase => (
            "Complete composite animal base",
            "Apply the bounded horn, tail, ear, and four-leg composite candidate.",
            "All live bindings and candidate identity were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::CompositeCompleteWingedAnimalBase => (
            "Complete winged animal base",
            "Apply the bounded horn, tail, ear, four-leg, and wing-pair composite candidate.",
            "All five live bindings and candidate identity were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase => (
            "Bounded composite target base",
            "Apply the bounded crease candidate composed from the recognized target bindings.",
            "Every live binding, geometry proof, and candidate identity was revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::DiagonalFold => (
            "Diagonal fold",
            "Fold the rectangular sheet on the generated diagonal.",
            "Review the crease direction before folding.",
        ),
        _ => return Err("the selected generated plan is preview-only".to_owned()),
    };
    if selected_kind != ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase {
        let certificate_hex = fold_path_certificate_sha256
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        instruction_timeline.steps.push(InstructionStep {
            id: InstructionStepId::new(),
            title: title.to_owned(),
            description: description.to_owned(),
            caution: format!("{caution} Native fold-path certificate SHA-256: {certificate_hex}."),
            duration_ms: 2_000,
            visual: InstructionVisual::default(),
            pose: InstructionPose {
                model: InstructionPoseModel::DeclarativeOnlyV1,
                source_model_fingerprint: project.editor.fold_model_fingerprint_v1(),
                fixed_face: None,
                hinge_angles: Vec::new(),
            },
        });
    }
    let paper = project.editor.paper().clone();
    let project_layers = project.editor.project_layers().clone();
    let mut beginner_design_profile = project.editor.beginner_design_profile().clone();
    let topology_authority_sha256: [u8; 32] = sha2::Sha256::digest(
        serde_json::to_vec(&certificate_pattern)
            .map_err(|_| "the generated plan topology could not be bound".to_owned())?,
    )
    .into();
    let reference_consensus_provenance = beginner_design_profile
        .reference_consensus_v1
        .as_ref()
        .map(|consensus| {
            let analysis = beginner_reference_consensus_analysis_v1(&project, None)
                .ok_or_else(|| "reference_consensus_analysis_unavailable".to_owned())?;
            Ok::<_, String>(ori_domain::BeginnerReferenceConsensusProvenanceV1 {
                schema_version: 1,
                source_revision: expected_revision,
                bindings: consensus.bindings.clone(),
                excluded_asset_id: consensus.excluded_asset_id,
                pair_digests_sha256: analysis
                    .pairs
                    .iter()
                    .map(|pair| pair.pair_digest_sha256)
                    .collect(),
                summary: ori_domain::BeginnerReferenceConsensusSummaryV1 {
                    schema_version: 1,
                    model: "component_extent_branch_v1".to_owned(),
                    source_count: consensus.bindings.len() as u8,
                    excluded_count: u8::from(consensus.excluded_asset_id.is_some()),
                    agreement_score: analysis.agreement_score,
                    component_subscore: analysis
                        .pairs
                        .iter()
                        .map(|pair| 100_u16.saturating_sub(u16::from(pair.component_error) * 20))
                        .sum::<u16>()
                        .checked_div(analysis.pairs.len() as u16)
                        .unwrap_or(0) as u8,
                    extent_subscore: analysis
                        .pairs
                        .iter()
                        .map(|pair| {
                            100_u16.saturating_sub(u16::from(pair.normalized_extent_error) * 2)
                        })
                        .sum::<u16>()
                        .checked_div(analysis.pairs.len() as u16)
                        .unwrap_or(0) as u8,
                    branch_subscore: analysis
                        .pairs
                        .iter()
                        .map(|pair| 100_u16.saturating_sub(u16::from(pair.branch_error) * 10))
                        .sum::<u16>()
                        .checked_div(analysis.pairs.len() as u16)
                        .unwrap_or(0) as u8,
                },
            })
        })
        .transpose()?;
    beginner_design_profile.generation_provenance =
        Some(ori_domain::BeginnerGenerationProvenanceV1 {
            schema_version: 1,
            topology_authority_sha256,
            fold_path_certificate_sha256: Some(fold_path_certificate_sha256),
            confidence_score: ori_domain::beginner_target_approximation_score_v1(
                &beginner_design_profile.generation_constraints,
            ),
            confidence_reasons: vec![
                "native_topology_witness".to_owned(),
                "bounded_native_fold_path_v2".to_owned(),
            ],
            explicit_override: false,
            source_asset_fingerprint: beginner_design_profile
                .generation_constraints
                .target_asset
                .map_or_else(|| "none".to_owned(), |asset| format!("{asset:?}")),
            semantic_landmark_provenance,
            generic_tree: None,
            reference_consensus_summary: reference_consensus_provenance
                .as_ref()
                .map(|value| value.summary.clone()),
            reference_consensus: reference_consensus_provenance,
        });
    execute_expected_command(
        &mut project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
        Command::ApplyBeginnerGeneratedDocument {
            pattern,
            paper,
            instruction_timeline,
            project_layers,
            beginner_design_profile: Box::new(beginner_design_profile),
        },
    )
}

pub(super) fn apply_grid_plan_document(
    project: &mut ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    plan: ori_domain::BeginnerGeneratedPlanV1,
) -> Result<ProjectSnapshot, String> {
    let selected_kind = plan.kind;
    let selected_instruction_codes = plan.instruction_codes.clone();
    let semantic_landmark_provenance = plan.semantic_landmark_provenance.clone();
    let topology_witness = beginner_contour_placement_witness(
        &project
            .editor
            .beginner_design_profile()
            .generation_constraints,
        &plan,
    )
    .ok_or_else(|| "grid_candidate_topology_stale".to_owned())?;
    let mut topology_ids = topology_witness
        .local_bindings
        .iter()
        .map(|binding| {
            let vertex_start = usize::from(binding.vertex_start);
            let crease_start = usize::from(binding.crease_start);
            let count = usize::from(binding.contour_points);
            let vertices = plan
                .crease_pattern
                .vertices
                .get(vertex_start..vertex_start + count)?;
            let creases = plan
                .crease_pattern
                .edges
                .get(crease_start..crease_start + count)?;
            Some((
                binding.generated_face_id,
                vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>(),
                creases.iter().map(|edge| edge.id).collect::<Vec<_>>(),
            ))
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| "grid_candidate_topology_stale".to_owned())?;
    for binding in &topology_witness.generic_feature_bindings {
        let start = usize::from(binding.crease_start);
        let count = usize::from(binding.endpoint_count);
        let creases = plan
            .crease_pattern
            .edges
            .get(start..start + count)
            .ok_or_else(|| "grid_candidate_topology_stale".to_owned())?;
        let crease_authority_sha256: [u8; 32] = sha2::Sha256::digest(
            serde_json::to_vec(&creases.iter().map(|edge| edge.id).collect::<Vec<_>>())
                .map_err(|_| "grid_candidate_topology_stale".to_owned())?,
        )
        .into();
        if crease_authority_sha256 != binding.crease_authority_sha256 {
            return Err("grid_candidate_feature_crease_authority_stale".to_owned());
        }
        let mut vertices = Vec::with_capacity(count * 2);
        for id in creases.iter().flat_map(|edge| [edge.start, edge.end]) {
            if !vertices.contains(&id) {
                vertices.push(id);
            }
        }
        topology_ids.push((
            binding
                .generated_feature_id
                .checked_add(128)
                .ok_or_else(|| "grid_candidate_topology_stale".to_owned())?,
            vertices,
            creases.iter().map(|edge| edge.id).collect(),
        ));
    }
    let mut pattern = project.editor.pattern().clone();
    for vertex in plan.crease_pattern.vertices {
        if !pattern
            .vertices
            .iter()
            .any(|current| current.id == vertex.id)
        {
            pattern.vertices.push(vertex);
        }
    }
    for edge in plan.crease_pattern.edges {
        if pattern.edges.iter().any(|current| current.id == edge.id) {
            return Err("grid_candidate_replayed".to_owned());
        }
        pattern.edges.push(edge);
    }
    let mut faces = std::collections::HashSet::new();
    let mut witnessed_vertices = std::collections::HashSet::new();
    let mut witnessed_creases = std::collections::HashSet::new();
    if topology_ids.iter().any(|(face_id, vertices, creases)| {
        !faces.insert(*face_id)
            || vertices.iter().any(|id| {
                witnessed_vertices.insert(*id);
                !pattern.vertices.iter().any(|vertex| vertex.id == *id)
            })
            || creases.iter().any(|id| {
                !witnessed_creases.insert(*id) || !pattern.edges.iter().any(|edge| edge.id == *id)
            })
    }) {
        return Err("grid_candidate_topology_stale".to_owned());
    }
    let mut instruction_timeline = project.editor.instruction_timeline().clone();
    let (title, description, caution) = match selected_kind {
        ori_domain::BeginnerGeneratedPlanKindV1::SymmetricFourLegBase => (
            "Symmetric four-leg grid candidate",
            "Apply the globally proven parameter-grid four-leg base.",
            "The canonical grid tuple and proof were revalidated immediately before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::SymmetricWingBase => (
            "Symmetric wing grid candidate",
            "Apply the globally proven parameter-grid wing base.",
            "The canonical grid tuple and proof were revalidated immediately before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::SymmetricBirdBase => (
            "Symmetric bird grid candidate",
            "Apply the globally proven parameter-grid bird base.",
            "The canonical grid tuple and proof were revalidated immediately before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::AsymmetricBirdLandmarkBase
        | ori_domain::BeginnerGeneratedPlanKindV1::AsymmetricFourLegLandmarkBase => (
            "Asymmetric landmark bird base",
            "Create individually bound asymmetric bird landmark creases.",
            "All landmark bindings and the native fold path were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::AsymmetricInsectLandmarkBase => (
            "Asymmetric insect landmark grid candidate",
            "Apply certified four-ray geometry with ten ordered semantic landmark bindings.",
            "All ray-group digests, live semantic bindings, and the native fold path were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::AsymmetricFishLandmarkBase => (
            "Asymmetric fish landmark grid candidate",
            "Apply certified four-ray geometry with four ordered fish landmark bindings.",
            "All semantic bindings, ray-group digests, and the native fold path were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::SymmetricFishBase => (
            "Symmetric fish grid candidate",
            "Apply the globally proven parameter-grid fish base.",
            "The canonical grid tuple and proof were revalidated immediately before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::SymmetricEarBase => (
            "Symmetric ear grid candidate",
            "Apply the globally proven parameter-grid long-ear base.",
            "The canonical grid tuple and proof were revalidated immediately before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::SymmetricHornBase => (
            "Symmetric horn grid candidate",
            "Apply the globally proven parameter-grid horn base.",
            "The canonical grid tuple and proof were revalidated immediately before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::SymmetricAntennaBase => (
            "Symmetric antenna grid candidate",
            "Apply the globally proven parameter-grid antenna base.",
            "The canonical grid tuple and proof were revalidated immediately before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::SymmetricInsectLegPairBase => (
            "Symmetric insect leg-pair grid candidate",
            "Apply the globally proven parameter-grid insect leg pair.",
            "This limited family represents exactly two legs, not a complete six-leg insect.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::SymmetricSixLegBase => (
            "Symmetric complete six-leg grid candidate",
            "Apply the globally proven three-pair parameter-grid insect base.",
            "All three pair positions and the global proof were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::CenterAxisTailBase => (
            "Center-axis tail grid candidate",
            "Apply the globally proven single-tail parameter-grid candidate.",
            "The live target, proof, and candidate identity were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::CenterAxisHornBase => (
            "Center-axis single-horn grid candidate",
            "Apply the globally proven single-horn parameter-grid candidate.",
            "The live target, proof, and candidate identity were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::CenterAxisAntennaBase => (
            "Center-axis single-antenna grid candidate",
            "Apply the globally proven single-antenna parameter-grid candidate.",
            "The live target, proof, and candidate identity were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::CompositeTailEarBase => (
            "Composite tail and ear grid candidate",
            "Apply the globally proven tail-and-ear parameter-grid candidate.",
            "Both live bindings, proof, and candidate identity were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::CompositeHornEarBase => (
            "Composite horn and ear grid candidate",
            "Apply the globally proven horn-and-ear parameter-grid candidate.",
            "Both live bindings, proof, and candidate identity were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::CompositeHornTailBase => (
            "Composite horn and tail grid candidate",
            "Apply the globally proven horn-and-tail parameter-grid candidate.",
            "Both live bindings, proof, and candidate identity were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::CompositeHornTailEarBase => (
            "Composite horn, tail, and ear grid candidate",
            "Apply the globally proven three-part parameter-grid candidate.",
            "All live bindings, proof, and candidate identity were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::CompositeWingAntennaBase => (
            "Composite wing and antenna grid candidate",
            "Apply the globally proven wing-and-antenna parameter-grid candidate.",
            "Both live pair bindings, proof, and candidate identity were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::CompositeCompleteInsectBase => (
            "Complete composite insect grid candidate",
            "Apply the globally proven five-pair insect parameter-grid candidate.",
            "All live bindings, proof, and candidate identity were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::CompositeCompleteAnimalBase => (
            "Complete composite animal grid candidate",
            "Apply the globally proven complete animal parameter-grid candidate.",
            "All live bindings, proof, and candidate identity were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::CompositeCompleteWingedAnimalBase => (
            "Complete winged animal grid candidate",
            "Apply the globally proven five-binding winged animal parameter-grid candidate.",
            "All five live bindings, proof, and candidate identity were revalidated before apply.",
        ),
        ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase => (
            "Bounded composite grid candidate",
            "Apply the globally proven parameter-grid candidate for the recognized target bindings.",
            "Every live binding, proof, and candidate identity was revalidated before apply.",
        ),
        _ => return Err("grid_candidate_kind_invalid".to_owned()),
    };
    let authority_hex = topology_witness
        .topology_authority_hash
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    instruction_timeline.steps.push(InstructionStep {
        id: InstructionStepId::new(),
        title: title.to_owned(),
        description: description.to_owned(),
        caution: format!("{caution} topology authority SHA-256: {authority_hex}."),
        duration_ms: 2_000,
        visual: InstructionVisual::default(),
        pose: InstructionPose {
            model: InstructionPoseModel::DeclarativeOnlyV1,
            source_model_fingerprint: project.editor.fold_model_fingerprint_v1(),
            fixed_face: None,
            hinge_angles: Vec::new(),
        },
    });
    let paper = project.editor.paper().clone();
    let project_layers = project.editor.project_layers().clone();
    let mut beginner_design_profile = project.editor.beginner_design_profile().clone();
    let source_asset_fingerprint = live_reference_model_suggestion_v1(project)
        .ok()
        .and_then(|reference| serde_json::to_vec(&reference.surface_landmarks_tenths_mm).ok())
        .map(|bytes| {
            let digest: [u8; 32] = sha2::Sha256::digest(bytes).into();
            format!(
                "glb-landmarks-sha256:{}",
                digest
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            )
        })
        .unwrap_or_else(|| {
            beginner_design_profile
                .generation_constraints
                .target_asset
                .map_or_else(|| "none".to_owned(), |asset| format!("{asset:?}"))
        });
    let generic_tree = if selected_kind
        == ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
    {
        let source = match beginner_design_profile.generation_constraints.target_asset {
            Some(ori_domain::BeginnerTargetAssetReferenceV1::ReferenceImage { .. }) => {
                ori_domain::BeginnerGenericTreeSourceV1::ImageSilhouette
            }
            Some(ori_domain::BeginnerTargetAssetReferenceV1::ReferenceModel { .. }) => {
                ori_domain::BeginnerGenericTreeSourceV1::GlbGeometry
            }
            None => ori_domain::BeginnerGenericTreeSourceV1::ManualSkeleton,
        };
        let asset_content_sha256 = match beginner_design_profile.generation_constraints.target_asset
        {
            Some(ori_domain::BeginnerTargetAssetReferenceV1::ReferenceImage {
                asset_id, ..
            }) => project
                .texture_assets
                .iter()
                .find(|asset| asset.id == asset_id)
                .map(|asset| <[u8; 32]>::from(sha2::Sha256::digest(&asset.bytes))),
            Some(ori_domain::BeginnerTargetAssetReferenceV1::ReferenceModel { asset_id }) => {
                project
                    .reference_model_assets
                    .iter()
                    .find(|asset| asset.id == asset_id)
                    .map(|asset| <[u8; 32]>::from(sha2::Sha256::digest(&asset.bytes)))
            }
            None => None,
        };
        let ratios = selected_instruction_codes
            .iter()
            .find_map(|code| code.strip_prefix("bounded_tree_river_axial_v1:"))
            .and_then(|encoded| {
                encoded
                    .split(',')
                    .map(str::parse::<u32>)
                    .collect::<Result<Vec<_>, _>>()
                    .ok()
            })
            .filter(|ratios| !ratios.is_empty() && ratios.len() <= 16)
            .ok_or("grid_candidate_tree_ratio_provenance_invalid")?;
        let orientation = if selected_instruction_codes
            .iter()
            .any(|code| code == "bounded_tree_paper_orientation_v1:vertical")
        {
            ori_domain::BeginnerGenericTreeOrientationV1::Vertical
        } else if selected_instruction_codes
            .iter()
            .any(|code| code == "bounded_tree_paper_orientation_v1:horizontal")
        {
            ori_domain::BeginnerGenericTreeOrientationV1::Horizontal
        } else {
            return Err("grid_candidate_tree_orientation_provenance_invalid".to_owned());
        };
        let segments = canonical_generic_tree_segments_v1(
            &beginner_design_profile
                .generation_constraints
                .skeleton_segments,
        )
        .ok_or_else(|| "grid_candidate_tree_provenance_invalid".to_owned())?;
        let tree_topology_sha256: [u8; 32] = sha2::Sha256::digest(
            serde_json::to_vec(&segments).map_err(|_| "grid_candidate_tree_provenance_invalid")?,
        )
        .into();
        let point =
            |point: ori_domain::BeginnerSkeletonPointV1| (point.x_tenths_mm, point.y_tenths_mm);
        let canonical_root = point(segments[0].start);
        let mut depths = std::collections::BTreeMap::from([(canonical_root, 0_u8)]);
        while depths.len() <= segments.len() {
            let before = depths.len();
            for segment in &segments {
                let start = point(segment.start);
                let end = point(segment.end);
                match (depths.get(&start).copied(), depths.get(&end).copied()) {
                    (Some(depth), None) => {
                        depths.insert(end, depth.saturating_add(1));
                    }
                    (None, Some(depth)) => {
                        depths.insert(start, depth.saturating_add(1));
                    }
                    _ => {}
                }
            }
            if depths.len() == before {
                break;
            }
        }
        let mut proposal_steps = segments.iter().enumerate().map(|(index, segment)| {
            let start_depth = depths.get(&point(segment.start)).copied().unwrap_or(u8::MAX);
            let end_depth = depths.get(&point(segment.end)).copied().unwrap_or(u8::MAX);
            let depth = start_depth.min(end_depth);
            ori_domain::BeginnerGenericTreeInstructionStepV1 {
                canonical_crease_id: format!("tree-river-{:04}", segment.id),
                tree_depth: depth,
                assignment: if index % 2 == 0 { "valley" } else { "mountain" }.to_owned(),
                target_branch: format!("branch-{:04}", segment.id),
                fixed_side: "root".to_owned(),
                caution: "Read-only declarative proposal; no physical-motion proof. Confirm only after checking the folded preview.".to_owned(),
            }
        }).collect::<Vec<_>>();
        proposal_steps.sort_by(|left, right| {
            (left.tree_depth, &left.canonical_crease_id)
                .cmp(&(right.tree_depth, &right.canonical_crease_id))
        });
        beginner_design_profile
            .generation_constraints
            .skeleton_segments = segments;
        Some(ori_domain::BeginnerGenericTreeProvenanceV1 {
            schema_version: 1,
            target_category: (beginner_design_profile
                .generation_constraints
                .target_category
                == Some(ori_domain::BeginnerTargetCategoryV1::CustomObject))
            .then_some(ori_domain::BeginnerTargetCategoryV1::CustomObject),
            source,
            asset_content_sha256,
            tree_topology_sha256,
            normalized_length_ratios: ratios,
            orientation,
            generator_version: 1,
            authorizes_apply: false,
            instruction_proposal: Some(ori_domain::BeginnerGenericTreeInstructionProposalV1 {
                schema_version: 1,
                topology_sha256: tree_topology_sha256,
                generator_version: 1,
                authorizes_apply: false,
                physical_motion_proof: false,
                steps: proposal_steps,
            }),
        })
    } else {
        None
    };
    beginner_design_profile.generation_provenance =
        Some(ori_domain::BeginnerGenerationProvenanceV1 {
            schema_version: 1,
            topology_authority_sha256: topology_witness.topology_authority_hash,
            fold_path_certificate_sha256: Some(
                sha2::Sha256::digest(
                    serde_json::to_vec(&(
                        topology_witness.topology_authority_hash,
                        selected_instruction_codes,
                    ))
                    .map_err(|_| "grid_candidate_path_certificate_invalid")?,
                )
                .into(),
            ),
            confidence_score: ori_domain::beginner_target_approximation_score_v1(
                &beginner_design_profile.generation_constraints,
            ),
            confidence_reasons: vec![
                "native_topology_witness".to_owned(),
                "preset_weighted_2d_3d_metric".to_owned(),
                "bounded_fold_path_certificate_v1".to_owned(),
            ],
            explicit_override: false,
            source_asset_fingerprint,
            semantic_landmark_provenance,
            generic_tree,
            reference_consensus: None,
            reference_consensus_summary: None,
        });
    execute_expected_command(
        project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
        Command::ApplyBeginnerGeneratedDocument {
            pattern,
            paper,
            instruction_timeline,
            project_layers,
            beginner_design_profile: Box::new(beginner_design_profile),
        },
    )
}

#[tauri::command]
pub(super) fn apply_beginner_parameter_grid_candidate(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    request_generation_id: ProjectId,
    expected_profile: ori_domain::BeginnerDesignProfileV1,
    expected_grid_hash: ori_domain::BeginnerParameterGridHashV1,
    selected_point: ori_domain::BeginnerParameterGridPointV1,
    expected_candidate_edge_id: EdgeId,
    expected_topology_authority_hash: [u8; 32],
    confirmed: bool,
) -> Result<ProjectSnapshot, String> {
    // Revision mutation is delegated atomically to apply_grid_plan_document's execute_command(
    if !confirmed {
        return Err("grid_candidate_confirmation_required".to_owned());
    }
    let completed_grid_work = {
        let registry = lock_recovering_registry_v1(beginner_grid_work());
        let work = registry
            .get(&request_generation_id)
            .ok_or_else(|| "grid_candidate_generation_stale".to_owned())?;
        if work.terminal.load(Ordering::Acquire) != 1 {
            return Err("grid_candidate_generation_stale".to_owned());
        }
        Arc::clone(work)
    };
    let grid = ori_domain::beginner_parameter_grid_v1();
    if ori_domain::beginner_parameter_grid_hash_v1(&grid) != expected_grid_hash
        || grid.get(usize::from(selected_point.id)).copied() != Some(selected_point)
    {
        return Err("grid_candidate_contract_stale".to_owned());
    }
    let mut project = lock_and_expect(
        &state,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
    )?;
    if project.editor.beginner_design_profile() != &expected_profile {
        return Err("grid_candidate_profile_stale".to_owned());
    }
    if !target_asset_reference_is_live(
        &project,
        expected_profile.generation_constraints.target_asset,
    ) {
        return Err("grid_candidate_asset_stale".to_owned());
    }
    let kind = symmetric_plan_kind(&expected_profile);
    let plan = grid_template_plan(
        project.project_id,
        project.editor.pattern(),
        &project.editor.paper().boundary_vertices,
        &expected_profile,
        selected_point,
    )
    .map_err(|_| "grid_candidate_generation_stale".to_owned())?
    .into_iter()
    .find(|plan| plan.kind == kind)
    .ok_or_else(|| "grid_candidate_generation_stale".to_owned())?;
    if plan.crease_pattern.edges.first().map(|edge| edge.id) != Some(expected_candidate_edge_id) {
        return Err("grid_candidate_identity_stale".to_owned());
    }
    if beginner_contour_placement_witness(&expected_profile.generation_constraints, &plan)
        .is_none_or(|witness| witness.topology_authority_hash != expected_topology_authority_hash)
    {
        return Err("grid_candidate_topology_stale".to_owned());
    }
    let reference = live_reference_model_suggestion_v1(&project).ok();
    let assessment = assess_beginner_generated_plan_with_deadline(
        project.project_id,
        project.editor.paper(),
        project.editor.pattern(),
        &plan,
        reference.as_ref(),
        std::time::Instant::now() + std::time::Duration::from_millis(750),
    );
    if assessment.expected_candidate_edge_id != expected_candidate_edge_id
        || assessment.proof_scope != "sufficient"
        || assessment.reason != "global_flat_foldability_proven"
        || !assessment.apply_allowed
    {
        return Err("grid_candidate_global_proof_stale".to_owned());
    }
    // Pin the exact completed work through the project mutation. A reused
    // generation cannot replace the registry entry between authorization and
    // apply (ABA), and a replacement observed here fails closed.
    let registry = lock_recovering_registry_v1(beginner_grid_work());
    if registry
        .get(&request_generation_id)
        .is_none_or(|current| !Arc::ptr_eq(current, &completed_grid_work))
        || completed_grid_work.terminal.load(Ordering::Acquire) != 1
    {
        return Err("grid_candidate_generation_stale".to_owned());
    }
    let applied = apply_grid_plan_document(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        plan,
    );
    drop(registry);
    applied
}

fn target_asset_reference_is_live(
    project: &ProjectState,
    reference: Option<ori_domain::BeginnerTargetAssetReferenceV1>,
) -> bool {
    match reference {
        None => true,
        Some(ori_domain::BeginnerTargetAssetReferenceV1::ReferenceImage {
            underlay_id,
            asset_id,
        }) => {
            project
                .editor
                .underlays()
                .underlays
                .iter()
                .any(|underlay| underlay.id == underlay_id && underlay.asset == asset_id)
                && project.texture_assets.iter().any(|asset| {
                    asset.id == asset_id
                        && asset.bytes.len() <= MAX_PROJECT_TEXTURE_ASSET_BYTES
                        && matches!(
                            asset.media_type,
                            ProjectTextureMediaTypeV1::Png | ProjectTextureMediaTypeV1::Jpeg
                        )
                })
        }
        Some(ori_domain::BeginnerTargetAssetReferenceV1::ReferenceModel { asset_id }) => project
            .reference_model_assets
            .iter()
            .any(|asset| asset.id == asset_id),
    }
}

fn component_bridge_override_is_live_v1(
    project: &ProjectState,
    profile: &ori_domain::BeginnerDesignProfileV1,
) -> bool {
    let Some(document) = profile
        .generation_constraints
        .component_bridge_override
        .as_ref()
    else {
        return true;
    };
    if !document.reviewed
        || document.bridges.len() > 7
        || profile.generation_constraints.skeleton_segments.len() > 16
    {
        return false;
    }
    let segments = &profile.generation_constraints.skeleton_segments;
    let orient = |a: [i32; 2], b: [i32; 2], c: [i32; 2]| {
        (i128::from(b[0]) - i128::from(a[0])) * (i128::from(c[1]) - i128::from(a[1]))
            - (i128::from(b[1]) - i128::from(a[1])) * (i128::from(c[0]) - i128::from(a[0]))
    };
    for (index, left) in segments.iter().enumerate() {
        let a = [left.start.x_tenths_mm, left.start.y_tenths_mm];
        let b = [left.end.x_tenths_mm, left.end.y_tenths_mm];
        for right in &segments[index + 1..] {
            let c = [right.start.x_tenths_mm, right.start.y_tenths_mm];
            let d = [right.end.x_tenths_mm, right.end.y_tenths_mm];
            if [a, b].iter().any(|point| *point == c || *point == d) {
                continue;
            }
            let (o1, o2, o3, o4) = (
                orient(a, b, c),
                orient(a, b, d),
                orient(c, d, a),
                orient(c, d, b),
            );
            if (o1 == 0 || o2 == 0 || o1.signum() != o2.signum())
                && (o3 == 0 || o4 == 0 || o3.signum() != o4.signum())
            {
                return false;
            }
        }
    }
    let hash_matches =
        project.reference_model_assets.iter().any(|asset| {
            sha2::Sha256::digest(&asset.bytes).as_slice() == document.source_asset_sha256
        }) || project.texture_assets.iter().any(|asset| {
            sha2::Sha256::digest(&asset.bytes).as_slice() == document.source_asset_sha256
        });
    if !hash_matches {
        return false;
    }
    let accepted = document
        .bridges
        .iter()
        .filter(|bridge| bridge.accepted)
        .collect::<Vec<_>>();
    if accepted.len() + 1 != usize::from(document.component_count) {
        return false;
    }
    let mut parent = (0..usize::from(document.component_count)).collect::<Vec<_>>();
    fn bridge_root(parent: &mut [usize], mut node: usize) -> usize {
        while parent[node] != node {
            parent[node] = parent[parent[node]];
            node = parent[node];
        }
        node
    }
    for bridge in accepted {
        let left = bridge_root(&mut parent, usize::from(bridge.start_component_id));
        let right = bridge_root(&mut parent, usize::from(bridge.end_component_id));
        if left == right {
            return false;
        }
        parent[right] = left;
    }
    (1..parent.len()).all(|index| bridge_root(&mut parent, index) == bridge_root(&mut parent, 0))
}

#[tauri::command]
pub(super) fn update_beginner_design_profile(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    profile: ori_domain::BeginnerDesignProfileV1,
) -> Result<ProjectSnapshot, String> {
    if !ori_domain::validate_beginner_design_profile_v1(&profile) {
        return Err("invalid beginner design profile".to_owned());
    }
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_project(&state)?;
    if !target_asset_reference_is_live(&project, profile.generation_constraints.target_asset) {
        return Err("the target reference image is unavailable".to_owned());
    }
    if !component_bridge_override_is_live_v1(&project, &profile) {
        return Err("component_bridge_override_stale_or_disconnected".to_owned());
    }
    if !reference_consensus_is_live_v1(&project, &profile) {
        return Err("reference_consensus_asset_binding_stale".to_owned());
    }
    let live_fingerprint = project.editor.fold_model_fingerprint_v1();
    if profile
        .generation_constraints
        .bulge_targets
        .iter()
        .any(|target| target.source_fold_model_fingerprint != live_fingerprint)
    {
        return Err("the 3D bulge target fold-model binding is stale".to_owned());
    }
    execute_expected_command(
        &mut project,
        expectation,
        Command::UpdateBeginnerDesignProfile {
            profile: Box::new(profile),
        },
    )
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BeginnerReferenceConsensusSelectionV1 {
    kind: ori_domain::BeginnerReferenceBindingKindV1,
    asset_id: AssetId,
}

#[tauri::command]
pub(super) fn update_beginner_reference_consensus(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    selections: Vec<BeginnerReferenceConsensusSelectionV1>,
) -> Result<ProjectSnapshot, String> {
    if !(2..=4).contains(&selections.len()) {
        return Err("reference_consensus_selection_count".to_owned());
    }
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_and_expect(&state, expectation)?;
    let mut canonical = selections;
    canonical.sort_by_key(|selection| selection.asset_id.canonical_bytes());
    if canonical
        .windows(2)
        .any(|pair| pair[0].asset_id == pair[1].asset_id)
    {
        return Err("reference_consensus_duplicate_asset".to_owned());
    }
    let mut bindings = Vec::with_capacity(canonical.len());
    for selection in canonical {
        let bytes = match selection.kind {
            ori_domain::BeginnerReferenceBindingKindV1::Image => {
                if !project
                    .editor
                    .underlays()
                    .underlays
                    .iter()
                    .any(|underlay| underlay.asset == selection.asset_id)
                {
                    return Err("reference_consensus_asset_stale".to_owned());
                }
                project
                    .texture_assets
                    .iter()
                    .find(|asset| asset.id == selection.asset_id)
                    .map(|asset| asset.bytes.as_slice())
            }
            ori_domain::BeginnerReferenceBindingKindV1::ReferenceModel => project
                .reference_model_assets
                .iter()
                .find(|asset| asset.id == selection.asset_id)
                .map(|asset| asset.bytes.as_slice()),
        }
        .ok_or_else(|| "reference_consensus_asset_stale".to_owned())?;
        bindings.push(ori_domain::BeginnerReferenceBindingV1 {
            kind: selection.kind,
            asset_id: selection.asset_id,
            sha256: sha2::Sha256::digest(bytes).into(),
            quality: 100,
        });
    }
    let mut profile = project.editor.beginner_design_profile().clone();
    profile.reference_consensus_v1 = Some(ori_domain::BeginnerReferenceConsensusV1 {
        schema_version: 1,
        bindings,
        excluded_asset_id: None,
    });
    execute_expected_command(
        &mut project,
        expectation,
        Command::UpdateBeginnerDesignProfile {
            profile: Box::new(profile),
        },
    )
}

fn reference_consensus_is_live_v1(
    project: &ProjectState,
    profile: &ori_domain::BeginnerDesignProfileV1,
) -> bool {
    let Some(consensus) = &profile.reference_consensus_v1 else {
        return true;
    };
    consensus.bindings.iter().all(|binding| {
        let bytes = match binding.kind {
            ori_domain::BeginnerReferenceBindingKindV1::Image => project
                .texture_assets
                .iter()
                .find(|asset| asset.id == binding.asset_id)
                .map(|asset| asset.bytes.as_slice()),
            ori_domain::BeginnerReferenceBindingKindV1::ReferenceModel => project
                .reference_model_assets
                .iter()
                .find(|asset| asset.id == binding.asset_id)
                .map(|asset| asset.bytes.as_slice()),
        };
        bytes.is_some_and(|bytes| <[u8; 32]>::from(sha2::Sha256::digest(bytes)) == binding.sha256)
    })
}

#[tauri::command]
pub(super) fn import_beginner_reference_model(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<ProjectSnapshot, String> {
    {
        let _project = lock_and_expect(
            &state,
            ProjectExpectation::new(
                expected_project_instance_id,
                expected_project_id,
                expected_revision,
            ),
        )?;
    }
    let selected = app
        .dialog()
        .file()
        .set_title("3D参照モデル / 3D reference model")
        .add_filter("GLB 2.0", &["glb"])
        .blocking_pick_file();
    let Some(selected) = selected else {
        let project = lock_and_expect(
            &state,
            ProjectExpectation::new(
                expected_project_instance_id,
                expected_project_id,
                expected_revision,
            ),
        )?;
        return Ok(snapshot(&project));
    };
    let path = selected
        .into_path()
        .map_err(|_| "ローカルGLBを選択してください / Select a local GLB".to_owned())?;
    let metadata = std::fs::metadata(&path)
        .map_err(|_| "GLBを読み込めません / Could not read GLB".to_owned())?;
    if !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > ori_formats::MAX_REFERENCE_GLB_BYTES_V1 as u64
    {
        return Err(
            "GLBは16 MiB以下である必要があります / GLB must be no larger than 16 MiB".to_owned(),
        );
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    File::open(&path)
        .and_then(|file| {
            file.take((ori_formats::MAX_REFERENCE_GLB_BYTES_V1 + 1) as u64)
                .read_to_end(&mut bytes)
        })
        .map_err(|_| "GLBを読み込めません / Could not read GLB".to_owned())?;
    let bytes = read_bounded_regular_import_file(
        &path,
        ori_formats::MAX_REFERENCE_GLB_BYTES_V1,
        "Could not read GLB",
        "GLB must be a non-empty regular file no larger than 16 MiB",
    )?;
    ori_formats::validate_reference_glb_v1(&bytes).map_err(|_| {
        "安全なGLB 2.0参照モデルではありません / Not a supported passive GLB 2.0 reference"
            .to_owned()
    })?;

    let mut project = lock_and_expect(
        &state,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
    )?;
    let retained_total = project
        .reference_model_assets
        .iter()
        .fold(bytes.len(), |total, asset| {
            total.saturating_add(asset.bytes.len())
        });
    if retained_total > ori_formats::MAX_PROJECT_REFERENCE_MODEL_ASSET_TOTAL_BYTES
        || project.reference_model_assets.len() >= ori_formats::MAX_PROJECT_REFERENCE_MODEL_ASSETS
    {
        return Err(
            "参照モデルのプロジェクト上限を超えます / Project reference-model limit exceeded"
                .to_owned(),
        );
    }
    let asset_id = AssetId::new();
    project
        .reference_model_assets
        .push(ori_formats::ProjectReferenceModelAssetV1 {
            id: asset_id,
            bytes,
        });
    let mut profile = project.editor.beginner_design_profile().clone();
    profile
        .archived_reference_model_asset_ids
        .retain(|id| *id != asset_id);
    profile.generation_constraints.target_asset =
        Some(ori_domain::BeginnerTargetAssetReferenceV1::ReferenceModel { asset_id });
    let result = execute_expected_command(
        &mut project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
        Command::UpdateBeginnerDesignProfile {
            profile: Box::new(profile),
        },
    );
    if result.is_err() {
        project
            .reference_model_assets
            .retain(|asset| asset.id != asset_id);
    }
    result
}

#[tauri::command]
pub(super) fn activate_beginner_reference_model_asset(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    asset_id: AssetId,
) -> Result<ProjectSnapshot, String> {
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_and_expect(&state, expectation)?;
    if !project
        .reference_model_assets
        .iter()
        .any(|asset| asset.id == asset_id)
    {
        return Err("reference_model_asset_stale".to_owned());
    }
    let mut profile = project.editor.beginner_design_profile().clone();
    profile.generation_constraints.target_asset =
        Some(ori_domain::BeginnerTargetAssetReferenceV1::ReferenceModel { asset_id });
    execute_expected_command(
        &mut project,
        expectation,
        Command::UpdateBeginnerDesignProfile {
            profile: Box::new(profile),
        },
    )
}

#[derive(Debug, Serialize)]
pub(super) struct BeginnerReferenceModelGeometryResponse {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    asset_id: AssetId,
    positions: Vec<[f32; 3]>,
    triangle_indices: Vec<[u32; 3]>,
    material_color: [u8; 4],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BeginnerReferenceModelSuggestionV1 {
    pub(super) asset_id: AssetId,
    pub(super) bbox_min_tenths_mm: [i32; 3],
    pub(super) bbox_max_tenths_mm: [i32; 3],
    pub(super) dominant_normal_milli: [i16; 3],
    pub(super) surface_area_milli: u64,
    pub(super) surface_landmarks_tenths_mm: Vec<[i32; 3]>,
    pub(super) surface_ranges: Vec<BeginnerReferenceSurfaceRangeV1>,
    pub(super) protrusions: Vec<ori_domain::BeginnerProtrusionTargetV1>,
    pub(super) general_protrusion_candidates: Vec<ori_domain::BeginnerProtrusionTargetV1>,
    pub(super) stick_bars: Vec<BeginnerReferenceStickBarV1>,
    pub(super) component_count: u8,
    pub(super) inferred_component_bridges: bool,
    pub(super) principal_axis_extents_tenths_mm: [u32; 3],
    pub(super) quality_score: u8,
    pub(super) quality_reasons: Vec<String>,
    pub(super) insufficiency_reasons: Vec<String>,
    pub(super) pair_bindings: Vec<ori_domain::BeginnerBilateralPairBindingV1>,
    pub(super) method: String,
    pub(super) suggested_part_kind: Option<ori_domain::BeginnerTargetPartKindV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BeginnerReferenceStickBarV1 {
    pub(super) id: u8,
    pub(super) start_tenths_mm: [i32; 3],
    pub(super) end_tenths_mm: [i32; 3],
    pub(super) thickness_tenths_mm: u16,
}

pub(super) fn disconnected_glb_stick_tree_v1(
    geometry: &ori_formats::ReferenceGlbGeometryV1,
) -> Result<Option<(u8, Vec<BeginnerReferenceStickBarV1>)>, String> {
    let mut parent = (0..geometry.positions.len()).collect::<Vec<_>>();
    fn root(parent: &mut [usize], mut node: usize) -> usize {
        while parent[node] != node {
            parent[node] = parent[parent[node]];
            node = parent[node];
        }
        node
    }
    let mut used = std::collections::BTreeSet::new();
    for triangle in &geometry.triangle_indices {
        let vertices = triangle
            .map(|value| {
                usize::try_from(value)
                    .ok()
                    .filter(|index| *index < parent.len())
                    .ok_or("reference_model_feature_range")
            })
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        for vertex in &vertices {
            used.insert(*vertex);
        }
        let anchor = root(&mut parent, vertices[0]);
        for vertex in vertices.into_iter().skip(1) {
            let other = root(&mut parent, vertex);
            parent[other] = anchor;
        }
    }
    let mut grouped = std::collections::BTreeMap::<usize, Vec<usize>>::new();
    for vertex in used {
        let component = root(&mut parent, vertex);
        grouped.entry(component).or_default().push(vertex);
    }
    if grouped.len() <= 1 {
        return Ok(None);
    }
    if grouped.len() > 8 {
        return Err("reference_model_component_limit".to_owned());
    }
    let quantize = |value: f32| -> Result<i32, String> {
        let scaled = f64::from(value) * 10_000.0;
        if !scaled.is_finite() || scaled < i32::MIN as f64 || scaled > i32::MAX as f64 {
            return Err("reference_model_feature_range".to_owned());
        }
        Ok(scaled.round() as i32)
    };
    let mut bounds = Vec::new();
    for vertices in grouped.values() {
        let mut minimum = [i32::MAX; 3];
        let mut maximum = [i32::MIN; 3];
        for vertex in vertices {
            for axis in 0..3 {
                let value = quantize(geometry.positions[*vertex][axis])?;
                minimum[axis] = minimum[axis].min(value);
                maximum[axis] = maximum[axis].max(value);
            }
        }
        if minimum == maximum {
            return Err("reference_model_component_degenerate".to_owned());
        }
        bounds.push((minimum, maximum));
    }
    let centers = bounds
        .iter()
        .map(|(minimum, maximum)| {
            std::array::from_fn::<_, 3, _>(|axis| {
                ((i64::from(minimum[axis]) + i64::from(maximum[axis])) / 2) as i32
            })
        })
        .collect::<Vec<_>>();
    let mut edges = Vec::new();
    for left in 0..bounds.len() {
        for right in left + 1..bounds.len() {
            let distance = (0..3)
                .map(|axis| {
                    let gap = if bounds[left].1[axis] < bounds[right].0[axis] {
                        i64::from(bounds[right].0[axis]) - i64::from(bounds[left].1[axis])
                    } else if bounds[right].1[axis] < bounds[left].0[axis] {
                        i64::from(bounds[left].0[axis]) - i64::from(bounds[right].1[axis])
                    } else {
                        0
                    };
                    gap.saturating_mul(gap)
                })
                .sum::<i64>();
            edges.push((distance, left, right));
        }
    }
    edges.sort_unstable();
    let mut component_parent = (0..bounds.len()).collect::<Vec<_>>();
    let mut bridges = Vec::new();
    for (_, left, right) in edges {
        let a = root(&mut component_parent, left);
        let b = root(&mut component_parent, right);
        if a != b {
            component_parent[b] = a;
            bridges.push((left, right));
        }
    }
    if bridges.len() + 1 != bounds.len() {
        return Err("reference_model_component_degenerate".to_owned());
    }
    let mut raw = bounds
        .iter()
        .enumerate()
        .map(|(index, (minimum, maximum))| {
            let axis = (0..2)
                .max_by_key(|axis| maximum[*axis].saturating_sub(minimum[*axis]))
                .unwrap_or(0);
            let mut end = centers[index];
            end[axis] = maximum[axis];
            (centers[index], end)
        })
        .chain(
            bridges
                .iter()
                .map(|(left, right)| (centers[*left], centers[*right])),
        )
        .filter(|(start, end)| start != end)
        .collect::<Vec<_>>();
    if raw.len() > 16 || raw.len() != bounds.len() * 2 - 1 {
        return Err("reference_model_component_degenerate".to_owned());
    }
    if raw.iter().any(|(start, end)| {
        [start, end]
            .into_iter()
            .flatten()
            .any(|coordinate| coordinate.unsigned_abs() > 1_000_000)
    }) {
        return Err("reference_model_component_boundary".to_owned());
    }
    fn orient(a: [i32; 3], b: [i32; 3], c: [i32; 3]) -> i128 {
        (i128::from(b[0]) - i128::from(a[0])) * (i128::from(c[1]) - i128::from(a[1]))
            - (i128::from(b[1]) - i128::from(a[1])) * (i128::from(c[0]) - i128::from(a[0]))
    }
    fn on(a: [i32; 3], b: [i32; 3], p: [i32; 3]) -> bool {
        orient(a, b, p) == 0
            && (a[0].min(b[0])..=a[0].max(b[0])).contains(&p[0])
            && (a[1].min(b[1])..=a[1].max(b[1])).contains(&p[1])
    }
    for index in 0..raw.len() {
        for other in index + 1..raw.len() {
            let (a, b) = raw[index];
            let (c, d) = raw[other];
            if [a, b].into_iter().any(|point| point == c || point == d) {
                continue;
            }
            let values = [
                orient(a, b, c),
                orient(a, b, d),
                orient(c, d, a),
                orient(c, d, b),
            ];
            if on(a, b, c)
                || on(a, b, d)
                || on(c, d, a)
                || on(c, d, b)
                || (values[0].signum() != values[1].signum()
                    && values[2].signum() != values[3].signum())
            {
                return Err("reference_model_component_projection_crossing".to_owned());
            }
        }
    }
    raw.sort_unstable();
    Ok(Some((
        bounds.len() as u8,
        raw.into_iter()
            .enumerate()
            .map(|(id, (start, end))| BeginnerReferenceStickBarV1 {
                id: id as u8,
                start_tenths_mm: start,
                end_tenths_mm: end,
                thickness_tenths_mm: 1,
            })
            .collect(),
    )))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BeginnerReferenceSurfaceRangeV1 {
    pub(super) id: u16,
    pub(super) triangle_indices: Vec<u32>,
    pub(super) range_min_tenths_mm: [i32; 3],
    pub(super) range_max_tenths_mm: [i32; 3],
    pub(super) digest_sha256: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BeginnerReferenceSurfaceAssignmentV1 {
    pub(super) range_id: u16,
    pub(super) protrusion_id: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BeginnerReferenceSurfaceEditV1 {
    pub(super) range_id: u16,
    pub(super) base_digest_sha256: [u8; 32],
    pub(super) triangle_indices: Vec<u32>,
    pub(super) bulge_direction_milli: [i16; 3],
    pub(super) bulge_amount_tenths_mm: u32,
}

#[derive(Debug, Serialize)]
pub(super) struct BeginnerReferenceModelSuggestionResponseV1 {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    source_asset_sha256: [u8; 32],
    suggestion: BeginnerReferenceModelSuggestionV1,
}

pub(super) fn derive_reference_model_suggestion_v1(
    asset_id: AssetId,
    geometry: &ori_formats::ReferenceGlbGeometryV1,
    category: Option<ori_domain::BeginnerTargetCategoryV1>,
    target_parts: &[ori_domain::BeginnerTargetPartRecordV1],
) -> Result<BeginnerReferenceModelSuggestionV1, String> {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for position in &geometry.positions {
        for axis in 0..3 {
            min[axis] = min[axis].min(position[axis]);
            max[axis] = max[axis].max(position[axis]);
        }
    }
    let to_tenths_mm = |value: f32| -> Result<i32, String> {
        let scaled = f64::from(value) * 10_000.0;
        if !scaled.is_finite() || scaled < i32::MIN as f64 || scaled > i32::MAX as f64 {
            return Err("reference_model_feature_range".to_owned());
        }
        Ok(scaled.round() as i32)
    };
    let bbox_min_tenths_mm = [
        to_tenths_mm(min[0])?,
        to_tenths_mm(min[1])?,
        to_tenths_mm(min[2])?,
    ];
    let bbox_max_tenths_mm = [
        to_tenths_mm(max[0])?,
        to_tenths_mm(max[1])?,
        to_tenths_mm(max[2])?,
    ];
    let landmark_count = geometry
        .positions
        .len()
        .min(MAX_BEGINNER_FOLDED_LANDMARKS_V1);
    let surface_landmarks_tenths_mm = (0..landmark_count)
        .map(|sample| {
            let index = sample.saturating_mul(geometry.positions.len()) / landmark_count.max(1);
            let position = geometry.positions[index];
            Ok([
                to_tenths_mm(position[0])?,
                to_tenths_mm(position[1])?,
                to_tenths_mm(position[2])?,
            ])
        })
        .collect::<Result<Vec<_>, String>>()?;
    let surface_ranges = geometry
        .triangle_indices
        .iter()
        .take(8)
        .enumerate()
        .map(|(range_index, triangle)| {
            let mut range_min = [i32::MAX; 3];
            let mut range_max = [i32::MIN; 3];
            let mut digest_input = Vec::with_capacity(3 * (4 + 12));
            for vertex_index in triangle {
                digest_input.extend_from_slice(&vertex_index.to_le_bytes());
                let position = geometry.positions[*vertex_index as usize];
                for axis in 0..3 {
                    let coordinate = to_tenths_mm(position[axis])?;
                    range_min[axis] = range_min[axis].min(coordinate);
                    range_max[axis] = range_max[axis].max(coordinate);
                    digest_input.extend_from_slice(&coordinate.to_le_bytes());
                }
            }
            Ok(BeginnerReferenceSurfaceRangeV1 {
                id: u16::try_from(range_index + 1)
                    .map_err(|_| "reference_model_feature_range".to_owned())?,
                triangle_indices: vec![
                    u32::try_from(range_index)
                        .map_err(|_| "reference_model_feature_range".to_owned())?,
                ],
                range_min_tenths_mm: range_min,
                range_max_tenths_mm: range_max,
                digest_sha256: sha2::Sha256::digest(&digest_input).into(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut normal = [0.0_f64; 3];
    let mut surface_area = 0.0_f64;
    for triangle in &geometry.triangle_indices {
        let a = geometry.positions[triangle[0] as usize];
        let b = geometry.positions[triangle[1] as usize];
        let c = geometry.positions[triangle[2] as usize];
        let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let cross = [
            f64::from(ab[1] * ac[2] - ab[2] * ac[1]),
            f64::from(ab[2] * ac[0] - ab[0] * ac[2]),
            f64::from(ab[0] * ac[1] - ab[1] * ac[0]),
        ];
        let length = cross.iter().map(|value| value * value).sum::<f64>().sqrt();
        surface_area += length * 0.5;
        for axis in 0..3 {
            normal[axis] += cross[axis];
        }
    }
    let normal_length = normal.iter().map(|value| value * value).sum::<f64>().sqrt();
    let dominant_normal_milli = if normal_length > 0.0 {
        normal.map(|value| (value / normal_length * 1000.0).round() as i16)
    } else {
        [0, 1000, 0]
    };
    let extents = [
        bbox_max_tenths_mm[0].saturating_sub(bbox_min_tenths_mm[0]),
        bbox_max_tenths_mm[1].saturating_sub(bbox_min_tenths_mm[1]),
        bbox_max_tenths_mm[2].saturating_sub(bbox_min_tenths_mm[2]),
    ];
    if extents.iter().all(|extent| *extent <= 0) {
        return Err("reference_model_feature_range".to_owned());
    }
    let principal_axis_extents_tenths_mm = extents.map(|extent| {
        u32::try_from(extent.max(1)).map_err(|_| "reference_model_feature_range".to_owned())
    });
    let principal_axis_extents_tenths_mm = [
        principal_axis_extents_tenths_mm[0].clone()?,
        principal_axis_extents_tenths_mm[1].clone()?,
        principal_axis_extents_tenths_mm[2].clone()?,
    ];
    let center = std::array::from_fn::<_, 3, _>(|axis| {
        bbox_min_tenths_mm[axis].saturating_add(bbox_max_tenths_mm[axis]) / 2
    });
    let mut stick_bars = (0..3)
        .map(|axis| {
            let mut start = center;
            let mut end = center;
            start[axis] = bbox_min_tenths_mm[axis];
            end[axis] = bbox_max_tenths_mm[axis];
            let thickness = extents
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != axis)
                .map(|(_, extent)| *extent)
                .min()
                .unwrap_or(1)
                .max(1);
            BeginnerReferenceStickBarV1 {
                id: axis as u8,
                start_tenths_mm: start,
                end_tenths_mm: end,
                thickness_tenths_mm: u16::try_from(thickness.clamp(1, i32::from(u16::MAX)))
                    .unwrap_or(u16::MAX),
            }
        })
        .collect::<Vec<_>>();
    let disconnected = disconnected_glb_stick_tree_v1(geometry)?;
    let component_count = disconnected.as_ref().map_or(1, |value| value.0);
    let inferred_component_bridges = disconnected.is_some();
    if let Some((_, component_bars)) = disconnected {
        stick_bars = component_bars;
    }
    let mut candidate_points = surface_landmarks_tenths_mm.clone();
    candidate_points.sort_unstable_by_key(|point| {
        let delta =
            std::array::from_fn::<_, 3, _>(|axis| i64::from(point[axis]) - i64::from(center[axis]));
        (
            std::cmp::Reverse(
                delta
                    .iter()
                    .map(|value| value.saturating_mul(*value))
                    .sum::<i64>(),
            ),
            *point,
        )
    });
    candidate_points.dedup();
    let general_protrusion_candidates = candidate_points
        .into_iter()
        .take(if inferred_component_bridges { 8 } else { 32 })
        .enumerate()
        .map(|(index, point)| {
            let delta = std::array::from_fn::<_, 3, _>(|axis| {
                i64::from(point[axis]) - i64::from(center[axis])
            });
            let axis = (0..3)
                .max_by_key(|axis| delta[*axis].unsigned_abs())
                .unwrap_or(0);
            let mut direction = [0_i16; 3];
            direction[axis] = if delta[axis] < 0 { -1000 } else { 1000 };
            ori_domain::BeginnerProtrusionTargetV1 {
                id: index as u16 + 1,
                count: 1,
                length_tenths_mm: u32::try_from(delta[axis].unsigned_abs())
                    .unwrap_or(u32::MAX)
                    .max(1),
                thickness_tenths_mm: 1,
                root_width_tenths_mm: None,
                tip_width_tenths_mm: None,
                local_outline_tenths_mm: None,
                position_tenths_mm: point,
                direction_milli: direction,
                symmetry: ori_domain::BeginnerProtrusionSymmetryV1::None,
                curvature_degrees: 0,
                joint: ori_domain::BeginnerProtrusionJointV1::Fixed,
                motion_degrees: [0, 0],
                side: ori_domain::BeginnerProtrusionSideV1::Either,
                priority: 50,
            }
        })
        .collect::<Vec<_>>();
    let mut insufficiency_reasons = Vec::new();
    if general_protrusion_candidates.len() < 2 {
        insufficiency_reasons.push("insufficient_distinct_vertices".to_owned());
    }
    if general_protrusion_candidates.len() == if inferred_component_bridges { 8 } else { 32 } {
        insufficiency_reasons.push("protrusion_candidate_limit_reached".to_owned());
    }
    if inferred_component_bridges {
        insufficiency_reasons.push("component_bridges_are_estimated".to_owned());
    }
    let quantized = geometry
        .positions
        .iter()
        .map(|position| position.map(|value| (f64::from(value) * 10_000.0).round() as i64))
        .collect::<HashSet<_>>();
    let axis_twice = i64::from(bbox_min_tenths_mm[0]) + i64::from(bbox_max_tenths_mm[0]);
    let bilateral = quantized.iter().all(|point| {
        quantized.contains(&[axis_twice.saturating_sub(point[0]), point[1], point[2]])
    });
    let requested_six_legs = target_parts
        .iter()
        .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Leg && part.count == 6);
    let requested_four_legs = target_parts
        .iter()
        .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Leg && part.count == 4);
    let requested_single_tail = target_parts
        .iter()
        .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Tail && part.count == 1);
    let requested_tail_ear = requested_single_tail
        && target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Ear && part.count == 2);
    let requested_single_horn = target_parts
        .iter()
        .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Horn && part.count == 1);
    let requested_horn_ear = requested_single_horn
        && target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Ear && part.count == 2);
    let requested_horn_tail = requested_single_horn && requested_single_tail;
    let requested_horn_tail_ear = requested_horn_tail && requested_horn_ear;
    let requested_complete_animal = requested_horn_tail_ear && requested_four_legs;
    let requested_animal_wings = requested_complete_animal
        && target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Wing && part.count == 2);
    if requested_complete_animal
        && (!bilateral
            || target_parts
                .iter()
                .filter(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Wing)
                .count()
                > 1)
    {
        return Err("reference_model_feature_range".to_owned());
    }
    let requested_single_antenna = target_parts
        .iter()
        .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Antenna && part.count == 1);
    let requested_wing_antenna = target_parts
        .iter()
        .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Wing && part.count == 2)
        && target_parts.iter().any(|part| {
            part.kind == ori_domain::BeginnerTargetPartKindV1::Antenna && part.count == 2
        });
    let requested_complete_insect = requested_wing_antenna
        && target_parts
            .iter()
            .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Leg && part.count == 6);
    if requested_complete_insect
        && (!bilateral
            || [
                ori_domain::BeginnerTargetPartKindV1::Wing,
                ori_domain::BeginnerTargetPartKindV1::Antenna,
                ori_domain::BeginnerTargetPartKindV1::Leg,
            ]
            .into_iter()
            .any(|kind| target_parts.iter().filter(|part| part.kind == kind).count() != 1))
    {
        return Err("reference_model_feature_range".to_owned());
    }
    let requested_pair = target_parts.iter().find(|part| {
        part.count == 2
            && matches!(
                part.kind,
                ori_domain::BeginnerTargetPartKindV1::Wing
                    | ori_domain::BeginnerTargetPartKindV1::Fin
                    | ori_domain::BeginnerTargetPartKindV1::Ear
                    | ori_domain::BeginnerTargetPartKindV1::Horn
                    | ori_domain::BeginnerTargetPartKindV1::Antenna
                    | ori_domain::BeginnerTargetPartKindV1::Leg
            )
    });
    let suggested_part_kind = if requested_single_antenna {
        Some(ori_domain::BeginnerTargetPartKindV1::Antenna)
    } else if requested_single_horn {
        Some(ori_domain::BeginnerTargetPartKindV1::Horn)
    } else if requested_single_tail {
        Some(ori_domain::BeginnerTargetPartKindV1::Tail)
    } else if requested_six_legs && bilateral {
        Some(ori_domain::BeginnerTargetPartKindV1::Leg)
    } else {
        requested_pair.filter(|_| bilateral).map(|part| part.kind)
    };
    let major_axis = (0..3).max_by_key(|axis| extents[*axis]).unwrap_or(0);
    let mut direction_milli = [0_i16; 3];
    direction_milli[major_axis] = 1000;
    let mut length_tenths_mm = u32::try_from((extents[major_axis] / 2).max(1))
        .map_err(|_| "reference_model_feature_range".to_owned())?;
    if requested_single_tail {
        // A single tail is admitted only as a bounded center-axis horizontal
        // family. A bbox cannot prove left/right intent, so use the stable
        // positive paper-axis direction and only the horizontal bbox extent.
        direction_milli = [1000, 0, 0];
        length_tenths_mm = u32::try_from((extents[0] / 2).max(1))
            .map_err(|_| "reference_model_feature_range".to_owned())?;
    }
    if requested_single_horn {
        direction_milli = [0, -1000, 0];
        length_tenths_mm = u32::try_from((extents[1] / 2).max(1))
            .map_err(|_| "reference_model_feature_range".to_owned())?;
    }
    if requested_single_antenna {
        direction_milli = [0, -1000, 0];
        length_tenths_mm = u32::try_from((extents[1] / 2).max(1))
            .map_err(|_| "reference_model_feature_range".to_owned())?;
    }
    let minor = extents
        .iter()
        .enumerate()
        .filter(|(axis, _)| *axis != major_axis)
        .map(|(_, value)| *value)
        .min()
        .unwrap_or(1);
    let thickness_tenths_mm = u16::try_from((minor / 4).clamp(1, i32::from(u16::MAX)))
        .map_err(|_| "reference_model_feature_range".to_owned())?;
    let base = ori_domain::BeginnerProtrusionTargetV1 {
        id: 1,
        count: if requested_single_tail || requested_single_horn || requested_single_antenna {
            1
        } else if suggested_part_kind.is_some() {
            2
        } else {
            match category {
                Some(ori_domain::BeginnerTargetCategoryV1::Animal) => 4,
                Some(ori_domain::BeginnerTargetCategoryV1::Insect) => 2,
                Some(ori_domain::BeginnerTargetCategoryV1::CustomObject) => 1,
                None => 1,
            }
        },
        length_tenths_mm,
        thickness_tenths_mm,
        root_width_tenths_mm: None,
        tip_width_tenths_mm: None,
        local_outline_tenths_mm: None,
        position_tenths_mm: std::array::from_fn(|axis| {
            bbox_min_tenths_mm[axis].saturating_add(bbox_max_tenths_mm[axis]) / 2
        }),
        direction_milli,
        symmetry: if requested_single_tail || requested_single_horn || requested_single_antenna {
            ori_domain::BeginnerProtrusionSymmetryV1::None
        } else {
            ori_domain::BeginnerProtrusionSymmetryV1::Bilateral
        },
        curvature_degrees: 0,
        joint: ori_domain::BeginnerProtrusionJointV1::Fixed,
        motion_degrees: [0, 0],
        side: ori_domain::BeginnerProtrusionSideV1::Either,
        priority: 50,
    };
    let mut protrusions = if requested_six_legs && bilateral {
        (0..3)
            .map(|index| {
                let mut target = base.clone();
                target.id = index + 1;
                target.position_tenths_mm[1] = bbox_min_tenths_mm[1]
                    .saturating_add(extents[1].saturating_mul(i32::from(index) + 1) / 4);
                target
            })
            .collect::<Vec<_>>()
    } else {
        vec![base.clone()]
    };
    if requested_tail_ear || requested_horn_ear {
        let mut ears = protrusions[0].clone();
        ears.id = if requested_horn_tail_ear { 3 } else { 2 };
        ears.count = 2;
        ears.symmetry = ori_domain::BeginnerProtrusionSymmetryV1::Bilateral;
        ears.direction_milli = [1000, 0, 0];
        protrusions.push(ears);
    }
    if requested_horn_tail {
        let mut tail = protrusions[0].clone();
        tail.id = 2;
        tail.direction_milli = [1000, 0, 0];
        tail.length_tenths_mm = u32::try_from((extents[0] / 2).max(1))
            .map_err(|_| "reference_model_feature_range".to_owned())?;
        if requested_horn_tail_ear {
            protrusions.insert(1, tail);
        } else {
            protrusions.push(tail);
        }
    }
    if requested_complete_animal {
        let mut legs = protrusions[0].clone();
        legs.id = 4;
        legs.count = 4;
        legs.symmetry = ori_domain::BeginnerProtrusionSymmetryV1::Bilateral;
        legs.direction_milli = [0, 1000, 0];
        protrusions.push(legs);
        if requested_animal_wings {
            let mut wings = protrusions[2].clone();
            wings.id = 5;
            wings.count = 2;
            wings.symmetry = ori_domain::BeginnerProtrusionSymmetryV1::Bilateral;
            wings.direction_milli = [1000, 0, 0];
            wings.priority = 60;
            protrusions.push(wings);
        }
    }
    if requested_wing_antenna {
        protrusions.clear();
        let mut wings = base.clone();
        wings.id = 1;
        wings.count = 2;
        wings.direction_milli = [1000, 0, 0];
        wings.symmetry = ori_domain::BeginnerProtrusionSymmetryV1::Bilateral;
        wings.priority = 60;
        let mut antennae = wings.clone();
        antennae.id = 2;
        antennae.direction_milli = [0, -1000, 0];
        antennae.length_tenths_mm = u32::try_from((extents[1] / 2).max(1))
            .map_err(|_| "reference_model_feature_range".to_owned())?;
        protrusions.extend([wings, antennae]);
        if requested_complete_insect {
            for (index, ordinal) in [1_i32, 2, 3].into_iter().enumerate() {
                let mut legs = protrusions[0].clone();
                legs.id = index as u16 + 3;
                legs.priority = 50;
                legs.position_tenths_mm[1] =
                    bbox_min_tenths_mm[1].saturating_add(extents[1].saturating_mul(ordinal) / 4);
                protrusions.push(legs);
            }
        }
    }
    let mut generic_feature_parts = target_parts
        .iter()
        .filter(|part| {
            !matches!(
                part.kind,
                ori_domain::BeginnerTargetPartKindV1::Head
                    | ori_domain::BeginnerTargetPartKindV1::Torso
            )
        })
        .collect::<Vec<_>>();
    let feature_rank = |kind| match kind {
        ori_domain::BeginnerTargetPartKindV1::Leg => 0,
        ori_domain::BeginnerTargetPartKindV1::Wing => 1,
        ori_domain::BeginnerTargetPartKindV1::Tail => 2,
        ori_domain::BeginnerTargetPartKindV1::Horn => 3,
        ori_domain::BeginnerTargetPartKindV1::Antenna => 4,
        ori_domain::BeginnerTargetPartKindV1::Ear => 5,
        ori_domain::BeginnerTargetPartKindV1::Fin => 6,
        ori_domain::BeginnerTargetPartKindV1::Head
        | ori_domain::BeginnerTargetPartKindV1::Torso => 7,
    };
    generic_feature_parts.sort_by_key(|part| feature_rank(part.kind));
    if !requested_complete_animal
        && !requested_wing_antenna
        && (2..=8).contains(&generic_feature_parts.len())
    {
        protrusions = generic_feature_parts
            .iter()
            .enumerate()
            .map(|(index, part)| {
                if !matches!(part.count, 1 | 2 | 4) {
                    return Err("reference_model_feature_range".to_owned());
                }
                let mut target = base.clone();
                target.id = index as u16 + 1;
                target.count = part.count;
                target.symmetry = if part.count == 1 {
                    ori_domain::BeginnerProtrusionSymmetryV1::None
                } else {
                    ori_domain::BeginnerProtrusionSymmetryV1::Bilateral
                };
                target.direction_milli = if matches!(
                    part.kind,
                    ori_domain::BeginnerTargetPartKindV1::Horn
                        | ori_domain::BeginnerTargetPartKindV1::Antenna
                        | ori_domain::BeginnerTargetPartKindV1::Leg
                ) {
                    [0, if part.count == 1 { -1000 } else { 1000 }, 0]
                } else {
                    [1000, 0, 0]
                };
                target.priority = 50_u8.saturating_add(index as u8 * 5);
                Ok(target)
            })
            .collect::<Result<Vec<_>, String>>()?;
    }
    let pair_bindings = protrusions
        .iter()
        .filter(|target| target.symmetry == ori_domain::BeginnerProtrusionSymmetryV1::Bilateral)
        .enumerate()
        .map(
            |(index, target)| ori_domain::BeginnerBilateralPairBindingV1 {
                pair_index: index as u8,
                protrusion_id: target.id,
                center_y_tenths_mm: target.position_tenths_mm[1],
            },
        )
        .collect();
    Ok(BeginnerReferenceModelSuggestionV1 {
        asset_id,
        bbox_min_tenths_mm,
        bbox_max_tenths_mm,
        dominant_normal_milli,
        surface_area_milli: (surface_area * 1_000.0).round().clamp(0.0, u64::MAX as f64) as u64,
        surface_landmarks_tenths_mm,
        surface_ranges,
        protrusions,
        general_protrusion_candidates,
        stick_bars,
        component_count,
        inferred_component_bridges,
        principal_axis_extents_tenths_mm,
        quality_score: if insufficiency_reasons.is_empty() {
            86
        } else {
            64
        },
        quality_reasons: vec![
            "strict_glb_vertex_index_bounds".to_owned(),
            "deterministic_aabb_principal_axes".to_owned(),
        ],
        insufficiency_reasons,
        pair_bindings,
        method: "bounded_bbox_area_normal_v1".to_owned(),
        suggested_part_kind,
    })
}

fn live_reference_model_suggestion_v1(
    project: &ProjectState,
) -> Result<BeginnerReferenceModelSuggestionV1, String> {
    let profile = project.editor.beginner_design_profile();
    let Some(ori_domain::BeginnerTargetAssetReferenceV1::ReferenceModel { asset_id }) =
        profile.generation_constraints.target_asset
    else {
        return Err("reference_model_suggestion_unavailable".to_owned());
    };
    let asset = project
        .reference_model_assets
        .iter()
        .find(|asset| asset.id == asset_id)
        .ok_or_else(|| "reference_model_suggestion_unavailable".to_owned())?;
    let geometry = ori_formats::read_reference_glb_geometry_v1(&asset.bytes)
        .map_err(|_| "reference_model_suggestion_unavailable".to_owned())?;
    derive_reference_model_suggestion_v1(
        asset_id,
        &geometry,
        profile.generation_constraints.target_category,
        &profile.generation_constraints.target_parts,
    )
}

pub(super) fn reference_model_suggestion_matches_live_v1(
    expected: &BeginnerReferenceModelSuggestionV1,
    live: &BeginnerReferenceModelSuggestionV1,
) -> bool {
    expected == live
}

const MAX_REFERENCE_MODEL_SURFACE_TRIANGLES_V1: usize = 40_000;
const MAX_REFERENCE_MODEL_SURFACE_TOTAL_TRIANGLES_V1: usize =
    MAX_REFERENCE_MODEL_SURFACE_TRIANGLES_V1 * 8;
const MAX_REFERENCE_MODEL_SURFACE_CONNECTIVITY_WORK_V1: usize =
    MAX_REFERENCE_MODEL_SURFACE_TRIANGLES_V1 * 16;
const REFERENCE_MODEL_SURFACE_CONNECTIVITY_TIMEOUT: std::time::Duration =
    std::time::Duration::from_millis(250);

/// A cooperative, absolute-budget control for reference-surface connectivity.
/// The caller can share one instance across every edit in an atomic apply so
/// no individual edit resets either the deadline or the work allowance.
pub(super) struct ReferenceModelSurfaceConnectivityControlV1<'a> {
    deadline: std::time::Instant,
    cancelled: Option<&'a AtomicBool>,
    remaining_work: usize,
}

impl<'a> ReferenceModelSurfaceConnectivityControlV1<'a> {
    pub(super) fn new(
        deadline: std::time::Instant,
        cancelled: Option<&'a AtomicBool>,
        maximum_work: usize,
    ) -> Self {
        Self {
            deadline,
            cancelled,
            remaining_work: maximum_work,
        }
    }

    fn consume(&mut self, work: usize) -> bool {
        if self
            .cancelled
            .is_some_and(|cancelled| cancelled.load(Ordering::Acquire))
            || std::time::Instant::now() >= self.deadline
        {
            return false;
        }
        let Some(remaining) = self.remaining_work.checked_sub(work) else {
            return false;
        };
        self.remaining_work = remaining;
        true
    }

    #[cfg(test)]
    pub(super) const fn remaining_work(&self) -> usize {
        self.remaining_work
    }
}

#[cfg(test)]
fn default_reference_model_surface_connectivity_control_v1()
-> ReferenceModelSurfaceConnectivityControlV1<'static> {
    let now = std::time::Instant::now();
    ReferenceModelSurfaceConnectivityControlV1::new(
        now.checked_add(REFERENCE_MODEL_SURFACE_CONNECTIVITY_TIMEOUT)
            .unwrap_or(now),
        None,
        MAX_REFERENCE_MODEL_SURFACE_CONNECTIVITY_WORK_V1,
    )
}

#[cfg(test)]
pub(super) fn reference_model_surface_selection_matches_live_v1(
    assignments: &[BeginnerReferenceSurfaceAssignmentV1],
    edits: &[BeginnerReferenceSurfaceEditV1],
    live: &BeginnerReferenceModelSuggestionV1,
    geometry: &ori_formats::ReferenceGlbGeometryV1,
) -> bool {
    let mut control = default_reference_model_surface_connectivity_control_v1();
    reference_model_surface_selection_matches_live_with_control_v1(
        assignments,
        edits,
        live,
        geometry,
        &mut control,
    )
}

fn reference_model_surface_selection_matches_live_with_control_v1(
    assignments: &[BeginnerReferenceSurfaceAssignmentV1],
    edits: &[BeginnerReferenceSurfaceEditV1],
    live: &BeginnerReferenceModelSuggestionV1,
    geometry: &ori_formats::ReferenceGlbGeometryV1,
    control: &mut ReferenceModelSurfaceConnectivityControlV1<'_>,
) -> bool {
    if !control.consume(0) {
        return false;
    }
    if !(2..=8).contains(&assignments.len()) {
        return false;
    }
    if edits.len() > 8 {
        return false;
    }
    let Some(total_edit_triangles) = edits.iter().try_fold(0_usize, |total, edit| {
        total.checked_add(edit.triangle_indices.len())
    }) else {
        return false;
    };
    if total_edit_triangles > MAX_REFERENCE_MODEL_SURFACE_TOTAL_TRIANGLES_V1 {
        return false;
    }
    let mut selected_ranges = HashSet::new();
    let mut selected_protrusions = HashSet::new();
    let mut measured_ranges = HashSet::new();
    let mut measured_protrusions = HashSet::new();
    let mut edit_ids = HashSet::new();
    if selected_ranges.try_reserve(assignments.len()).is_err()
        || selected_protrusions.try_reserve(assignments.len()).is_err()
        || measured_ranges
            .try_reserve(live.surface_ranges.len())
            .is_err()
        || measured_protrusions
            .try_reserve(live.protrusions.len())
            .is_err()
        || edit_ids.try_reserve(edits.len()).is_err()
    {
        return false;
    }
    selected_ranges.extend(assignments.iter().map(|assignment| assignment.range_id));
    selected_protrusions.extend(
        assignments
            .iter()
            .map(|assignment| assignment.protrusion_id),
    );
    measured_ranges.extend(live.surface_ranges.iter().map(|range| range.id));
    measured_protrusions.extend(live.protrusions.iter().map(|target| target.id));
    edit_ids.extend(edits.iter().map(|edit| edit.range_id));
    let mut triangles = HashSet::new();
    if triangles.try_reserve(total_edit_triangles).is_err() {
        return false;
    }
    assignments.len() == selected_ranges.len()
        && assignments.len() == selected_protrusions.len()
        && edits.len() == edit_ids.len()
        && edit_ids == selected_ranges
        && selected_ranges.is_subset(&measured_ranges)
        && selected_protrusions.is_subset(&measured_protrusions)
        && assignments.iter().all(|assignment| {
            let Some(live_range) = live
                .surface_ranges
                .iter()
                .find(|range| range.id == assignment.range_id)
            else {
                return false;
            };
            let Some(edit) = edits
                .iter()
                .find(|edit| edit.range_id == assignment.range_id)
            else {
                return false;
            };
            edit.base_digest_sha256 == live_range.digest_sha256
                && edit.bulge_direction_milli != [0, 0, 0]
                && edit
                    .bulge_direction_milli
                    .iter()
                    .all(|value| value.unsigned_abs() <= 1_000)
                && (1..=1_000_000).contains(&edit.bulge_amount_tenths_mm)
                && !edit.triangle_indices.is_empty()
                && edit.triangle_indices.len() <= MAX_REFERENCE_MODEL_SURFACE_TRIANGLES_V1
                && reference_model_surface_edit_is_within_live_range_v1(
                    &edit.triangle_indices,
                    &live_range.triangle_indices,
                    control,
                )
                && reference_model_surface_triangle_indices_are_connected_with_control_v1(
                    &edit.triangle_indices,
                    geometry,
                    control,
                )
                && edit
                    .triangle_indices
                    .iter()
                    .all(|triangle| control.consume(1) && triangles.insert(*triangle))
        })
}

fn reference_model_surface_edit_is_within_live_range_v1(
    edited_triangles: &[u32],
    live_triangles: &[u32],
    control: &mut ReferenceModelSurfaceConnectivityControlV1<'_>,
) -> bool {
    if !control.consume(0) {
        return false;
    }
    if edited_triangles.len() > live_triangles.len()
        || live_triangles.len() > MAX_REFERENCE_MODEL_SURFACE_TRIANGLES_V1
    {
        return false;
    }
    let mut live = HashSet::new();
    if live.try_reserve(live_triangles.len()).is_err() {
        return false;
    }
    for triangle in live_triangles {
        if !control.consume(1) || !live.insert(*triangle) {
            return false;
        }
    }
    edited_triangles
        .iter()
        .all(|triangle| control.consume(1) && live.contains(triangle))
}

#[cfg(test)]
pub(super) fn reference_model_surface_range_is_connected_v1(
    range: &BeginnerReferenceSurfaceRangeV1,
    geometry: &ori_formats::ReferenceGlbGeometryV1,
) -> bool {
    let mut control = default_reference_model_surface_connectivity_control_v1();
    reference_model_surface_range_is_connected_with_control_v1(range, geometry, &mut control)
}

pub(super) fn reference_model_surface_range_is_connected_with_control_v1(
    range: &BeginnerReferenceSurfaceRangeV1,
    geometry: &ori_formats::ReferenceGlbGeometryV1,
    control: &mut ReferenceModelSurfaceConnectivityControlV1<'_>,
) -> bool {
    reference_model_surface_triangle_indices_are_connected_with_control_v1(
        &range.triangle_indices,
        geometry,
        control,
    )
}

fn reference_model_surface_triangle_indices_are_connected_with_control_v1(
    triangle_indices: &[u32],
    geometry: &ori_formats::ReferenceGlbGeometryV1,
    control: &mut ReferenceModelSurfaceConnectivityControlV1<'_>,
) -> bool {
    if !control.consume(0) {
        return false;
    }
    let Some(first) = triangle_indices.first().copied() else {
        return false;
    };
    if triangle_indices.len() > MAX_REFERENCE_MODEL_SURFACE_TRIANGLES_V1 {
        return false;
    }

    let Some(vertex_index_capacity) = triangle_indices.len().checked_mul(3) else {
        return false;
    };
    let mut selected = HashSet::new();
    if selected.try_reserve(triangle_indices.len()).is_err() {
        return false;
    }
    let mut vertex_anchor = HashMap::<u32, usize>::new();
    if vertex_anchor.try_reserve(vertex_index_capacity).is_err() {
        return false;
    }
    let mut adjacency = Vec::<Vec<usize>>::new();
    if adjacency.try_reserve_exact(triangle_indices.len()).is_err() {
        return false;
    }
    adjacency.resize_with(triangle_indices.len(), Vec::new);

    for (triangle_position, triangle_index) in triangle_indices.iter().copied().enumerate() {
        if !control.consume(1)
            || !selected.insert(triangle_index)
            || usize::try_from(triangle_index)
                .ok()
                .is_none_or(|index| index >= geometry.triangle_indices.len())
        {
            return false;
        }
        let triangle = geometry.triangle_indices[triangle_index as usize];
        for vertex in triangle {
            if !control.consume(1) {
                return false;
            }
            match vertex_anchor.entry(vertex) {
                std::collections::hash_map::Entry::Occupied(entry) => {
                    let anchor = *entry.get();
                    if anchor != triangle_position {
                        if adjacency[triangle_position].try_reserve(1).is_err()
                            || adjacency[anchor].try_reserve(1).is_err()
                        {
                            return false;
                        }
                        adjacency[triangle_position].push(anchor);
                        adjacency[anchor].push(triangle_position);
                    }
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(triangle_position);
                }
            }
        }
    }

    let mut visited = Vec::new();
    if visited.try_reserve_exact(triangle_indices.len()).is_err() {
        return false;
    }
    visited.resize(triangle_indices.len(), false);
    let mut pending = Vec::new();
    if pending.try_reserve_exact(triangle_indices.len()).is_err() {
        return false;
    }
    visited[0] = true;
    pending.push(0);
    let mut visited_count = 0_usize;
    while let Some(current) = pending.pop() {
        if !control.consume(1) {
            return false;
        }
        visited_count = match visited_count.checked_add(1) {
            Some(value) => value,
            None => return false,
        };
        for neighbor in &adjacency[current] {
            if !control.consume(1) {
                return false;
            }
            if !visited[*neighbor] {
                visited[*neighbor] = true;
                pending.push(*neighbor);
            }
        }
    }
    visited_count == triangle_indices.len() && selected.contains(&first)
}

#[tauri::command]
pub(super) fn get_beginner_reference_model_geometry(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<BeginnerReferenceModelGeometryResponse, String> {
    let project = lock_and_expect(
        &state,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
    )?;
    let Some(ori_domain::BeginnerTargetAssetReferenceV1::ReferenceModel { asset_id }) = project
        .editor
        .beginner_design_profile()
        .generation_constraints
        .target_asset
    else {
        return Err(
            "3D参照モデルが設定されていません / No 3D reference model is attached".to_owned(),
        );
    };
    let asset = project
        .reference_model_assets
        .iter()
        .find(|asset| asset.id == asset_id)
        .ok_or_else(|| {
            "3D参照モデルが利用できません / 3D reference model is unavailable".to_owned()
        })?;
    let geometry = ori_formats::read_reference_glb_geometry_v1(&asset.bytes).map_err(|_| {
        "3D参照モデルを安全に表示できません / 3D reference model cannot be displayed safely"
            .to_owned()
    })?;
    Ok(BeginnerReferenceModelGeometryResponse {
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: project.editor.revision(),
        asset_id,
        positions: geometry.positions,
        triangle_indices: geometry.triangle_indices,
        material_color: geometry.material_color,
    })
}

#[tauri::command]
pub(super) fn suggest_beginner_reference_model_features(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<BeginnerReferenceModelSuggestionResponseV1, String> {
    let project = lock_and_expect(
        &state,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
    )?;
    let profile = project.editor.beginner_design_profile();
    let Some(ori_domain::BeginnerTargetAssetReferenceV1::ReferenceModel { asset_id }) =
        profile.generation_constraints.target_asset
    else {
        return Err("reference_model_suggestion_unavailable".to_owned());
    };
    let asset = project
        .reference_model_assets
        .iter()
        .find(|asset| asset.id == asset_id)
        .ok_or_else(|| "reference_model_suggestion_unavailable".to_owned())?;
    let geometry = ori_formats::read_reference_glb_geometry_v1(&asset.bytes)
        .map_err(|_| "reference_model_suggestion_unavailable".to_owned())?;
    let suggestion = derive_reference_model_suggestion_v1(
        asset_id,
        &geometry,
        profile.generation_constraints.target_category,
        &profile.generation_constraints.target_parts,
    )?;
    Ok(BeginnerReferenceModelSuggestionResponseV1 {
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: project.editor.revision(),
        source_asset_sha256: sha2::Sha256::digest(&asset.bytes).into(),
        suggestion,
    })
}

#[tauri::command]
pub(super) fn apply_beginner_reference_model_features(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    expected_suggestion: BeginnerReferenceModelSuggestionV1,
    surface_assignments: Vec<BeginnerReferenceSurfaceAssignmentV1>,
    surface_edits: Vec<BeginnerReferenceSurfaceEditV1>,
    confirmed: bool,
) -> Result<ProjectSnapshot, String> {
    if !confirmed {
        return Err("reference_model_suggestion_confirmation_required".to_owned());
    }
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_and_expect(&state, expectation)?;
    let mut profile = project.editor.beginner_design_profile().clone();
    let Some(ori_domain::BeginnerTargetAssetReferenceV1::ReferenceModel { asset_id }) =
        profile.generation_constraints.target_asset
    else {
        return Err("reference_model_suggestion_stale".to_owned());
    };
    let asset = project
        .reference_model_assets
        .iter()
        .find(|asset| asset.id == asset_id)
        .ok_or_else(|| "reference_model_suggestion_stale".to_owned())?;
    let geometry = ori_formats::read_reference_glb_geometry_v1(&asset.bytes)
        .map_err(|_| "reference_model_suggestion_stale".to_owned())?;
    let live = derive_reference_model_suggestion_v1(
        asset_id,
        &geometry,
        profile.generation_constraints.target_category,
        &profile.generation_constraints.target_parts,
    )?;
    if !reference_model_suggestion_matches_live_v1(&expected_suggestion, &live) {
        return Err("reference_model_suggestion_stale".to_owned());
    }
    let now = std::time::Instant::now();
    let deadline = now
        .checked_add(REFERENCE_MODEL_SURFACE_CONNECTIVITY_TIMEOUT)
        .ok_or_else(|| "reference_model_surface_selection_tampered".to_owned())?;
    let mut connectivity = ReferenceModelSurfaceConnectivityControlV1::new(
        deadline,
        None,
        MAX_REFERENCE_MODEL_SURFACE_CONNECTIVITY_WORK_V1,
    );
    if live.surface_ranges.iter().any(|range| {
        !reference_model_surface_range_is_connected_with_control_v1(
            range,
            &geometry,
            &mut connectivity,
        )
    }) {
        return Err("reference_model_surface_selection_tampered".to_owned());
    }
    if surface_assignments.len() < 2 {
        return Err("reference_model_surface_selection_confirmation_required".to_owned());
    }
    if !reference_model_surface_selection_matches_live_with_control_v1(
        &surface_assignments,
        &surface_edits,
        &live,
        &geometry,
        &mut connectivity,
    ) {
        return Err("reference_model_surface_selection_tampered".to_owned());
    }
    let selected_protrusions = surface_assignments
        .iter()
        .map(|assignment| assignment.protrusion_id)
        .collect::<HashSet<_>>();
    profile.generation_constraints.protrusions = live
        .protrusions
        .iter()
        .filter(|target| selected_protrusions.contains(&target.id))
        .cloned()
        .collect();
    let topology = project
        .editor
        .topology_analysis_input(project.project_id)
        .analyze();
    let face_id = topology
        .simulation_snapshot()
        .and_then(|snapshot| snapshot.faces.first().map(|face| face.id))
        .ok_or_else(|| "reference_model_surface_selection_tampered".to_owned())?;
    let fingerprint = project.editor.fold_model_fingerprint_v1();
    profile.generation_constraints.bulge_targets = surface_assignments
        .iter()
        .enumerate()
        .map(|(index, assignment)| {
            let edit = surface_edits
                .iter()
                .find(|edit| edit.range_id == assignment.range_id)
                .ok_or_else(|| "reference_model_surface_selection_tampered".to_owned())?;
            let mut minimum = [i32::MAX; 3];
            let mut maximum = [i32::MIN; 3];
            for triangle_index in &edit.triangle_indices {
                for vertex_index in geometry.triangle_indices[*triangle_index as usize] {
                    let position = geometry.positions[vertex_index as usize];
                    for axis in 0..3 {
                        let coordinate = (f64::from(position[axis]) * 10_000.0).round() as i32;
                        minimum[axis] = minimum[axis].min(coordinate);
                        maximum[axis] = maximum[axis].max(coordinate);
                    }
                }
            }
            Ok(ori_domain::BeginnerBulgeTargetV1 {
                id: u16::try_from(index + 1)
                    .map_err(|_| "reference_model_surface_selection_tampered")?,
                face_ids: vec![face_id],
                range_min_tenths_mm: minimum,
                range_max_tenths_mm: maximum,
                direction_milli: edit.bulge_direction_milli,
                amount_tenths_mm: edit.bulge_amount_tenths_mm,
                source_fold_model_fingerprint: fingerprint.clone(),
                reference_surface_binding: Some(ori_domain::BeginnerReferenceSurfaceBindingV1 {
                    asset_id,
                    range_id: edit.range_id,
                    protrusion_id: assignment.protrusion_id,
                    triangle_indices: edit.triangle_indices.clone(),
                    range_digest_sha256: edit.base_digest_sha256,
                }),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    if profile.generation_constraints.target_category.is_some() && live.protrusions.len() == 3 {
        if let Some(binding) =
            ori_domain::animal_horn_tail_ear_bindings_v1(&profile.generation_constraints)
        {
            if binding.horn_protrusion_id != live.protrusions[0].id
                || binding.tail_protrusion_id != live.protrusions[1].id
                || binding.ear_pair_protrusion_id != live.protrusions[2].id
            {
                return Err("reference_model_suggestion_invalid".to_owned());
            }
        } else if ori_domain::insect_three_pair_bindings_v1(&profile.generation_constraints)
            .is_none_or(|bindings| bindings.as_slice() != live.pair_bindings.as_slice())
        {
            return Err("reference_model_suggestion_invalid".to_owned());
        }
    }
    if profile.generation_constraints.target_category.is_some() && live.protrusions.len() == 2 {
        if let Some(binding) =
            ori_domain::insect_wing_antenna_bindings_v1(&profile.generation_constraints)
        {
            if binding.wing_pair_protrusion_id != live.protrusions[0].id
                || binding.antenna_pair_protrusion_id != live.protrusions[1].id
            {
                return Err("reference_model_suggestion_invalid".to_owned());
            }
        } else if let Some(binding) =
            ori_domain::animal_horn_tail_bindings_v1(&profile.generation_constraints)
        {
            if binding.horn_protrusion_id != live.protrusions[0].id
                || binding.tail_protrusion_id != live.protrusions[1].id
            {
                return Err("reference_model_suggestion_invalid".to_owned());
            }
        } else if let Some(binding) =
            ori_domain::animal_tail_ear_bindings_v1(&profile.generation_constraints)
        {
            if binding.tail_protrusion_id != live.protrusions[0].id
                || binding.ear_pair_protrusion_id != live.protrusions[1].id
            {
                return Err("reference_model_suggestion_invalid".to_owned());
            }
        } else {
            let binding = ori_domain::animal_horn_ear_bindings_v1(&profile.generation_constraints)
                .ok_or_else(|| "reference_model_suggestion_invalid".to_owned())?;
            if binding.horn_protrusion_id != live.protrusions[0].id
                || binding.ear_pair_protrusion_id != live.protrusions[1].id
            {
                return Err("reference_model_suggestion_invalid".to_owned());
            }
        }
    }
    if profile.generation_constraints.target_category.is_some() && live.protrusions.len() == 5 {
        let expected = if profile.generation_constraints.target_category
            == Some(ori_domain::BeginnerTargetCategoryV1::Animal)
        {
            let binding =
                ori_domain::animal_complete_winged_bindings_v1(&profile.generation_constraints)
                    .ok_or_else(|| "reference_model_suggestion_invalid".to_owned())?;
            vec![
                binding.animal.horn_protrusion_id,
                binding.animal.tail_protrusion_id,
                binding.animal.ear_pair_protrusion_id,
                binding.animal.leg_protrusion_id,
                binding.wing_pair_protrusion_id,
            ]
        } else {
            let binding = ori_domain::insect_complete_bindings_v1(&profile.generation_constraints)
                .ok_or_else(|| "reference_model_suggestion_invalid".to_owned())?;
            vec![
                binding.wing_pair_protrusion_id,
                binding.antenna_pair_protrusion_id,
                binding.leg_pair_protrusion_ids[0],
                binding.leg_pair_protrusion_ids[1],
                binding.leg_pair_protrusion_ids[2],
            ]
        };
        if live.protrusions.iter().map(|target| target.id).ne(expected) {
            return Err("reference_model_suggestion_invalid".to_owned());
        }
    }
    if profile.generation_constraints.target_category.is_some() && live.protrusions.len() == 4 {
        let binding = ori_domain::animal_complete_bindings_v1(&profile.generation_constraints)
            .ok_or_else(|| "reference_model_suggestion_invalid".to_owned())?;
        let expected = [
            binding.horn_protrusion_id,
            binding.tail_protrusion_id,
            binding.ear_pair_protrusion_id,
            binding.leg_protrusion_id,
        ];
        if live.protrusions.iter().map(|target| target.id).ne(expected) {
            return Err("reference_model_suggestion_invalid".to_owned());
        }
    }
    profile.reference_surface_landmarks_tenths_mm = Some(live.surface_landmarks_tenths_mm.clone());
    if !ori_domain::validate_beginner_design_profile_v1(&profile) {
        return Err("reference_model_suggestion_invalid".to_owned());
    }
    execute_expected_command(
        &mut project,
        expectation,
        Command::UpdateBeginnerDesignProfile {
            profile: Box::new(profile),
        },
    )
}
