use std::{
    collections::BTreeSet,
    fs,
    io::{Cursor, Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering},
        mpsc,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use ori_domain::{Edge, LayerContentKindV1, LayerRecordV1, Vertex};
use ori_formats::{
    Ori2Limits, read_project_archive_ori2, read_project_folder_v1, read_project_ori2_with_limits,
    write_project_archive_ori2, write_project_folder_v1, write_project_ori2,
};
#[cfg(target_os = "windows")]
use std::fs::OpenOptions;
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

use super::beginner_design_commands::{
    BeginnerReferenceSurfaceRangeV1, ReferenceModelSurfaceConnectivityControlV1,
    assess_beginner_generated_plan_with_control_v1, beginner_candidate_snapshot_is_current_v1,
    capture_beginner_candidate_analysis_snapshot_v1,
    reference_model_surface_range_is_connected_with_control_v1,
};
use super::*;

static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);
static BEGINNER_GRID_TEST_LOCK: Mutex<()> = Mutex::new(());

include!("tests/desktop_suite_01_beginner_effective_cut_assets.rs");
include!("tests/desktop_suite_02_beginner_reference_surface.rs");
include!("beginner_work_registry_tests.rs");

include!("tests/desktop_suite_03_beginner_grid_profiles.rs");
mod beginner_general_tree;
mod beginner_general_tree_root;

include!("tests/desktop_suite_04_beginner_landmark_round_trip.rs");
include!("tests/desktop_suite_05_beginner_certifier_tree.rs");
include!("tests/desktop_suite_06_beginner_grid_apply_and_support.rs");
include!("tests/desktop_suite_07_project_fixture_and_layer_history.rs");
include!("tests/desktop_suite_08_geometric_constraint_analysis.rs");
include!("tests/desktop_suite_09_geometric_constraint_worker_gates.rs");
include!("tests/desktop_suite_10_topology_instruction_pose.rs");
include!("tests/desktop_suite_11_instruction_validation_archive_helpers.rs");
include!("tests/desktop_suite_12_project_edit_fixtures_and_basics.rs");
include!("tests/desktop_suite_13_paper_properties_and_resize.rs");
include!("tests/desktop_suite_14_intersection_cluster.rs");
include!("tests/desktop_suite_15_junction_boundary_and_commands.rs");
include!("tests/desktop_suite_16_editor_validation.rs");
include!("tests/desktop_suite_17_native_archive_io.rs");
include!("tests/desktop_suite_18_save_paths_and_document_state.rs");
include!("tests/desktop_suite_19_import_staging_and_validation.rs");
include!("tests/desktop_suite_20_import_commit_and_fold_conversion.rs");
include!("tests/desktop_suite_21_fold_svg_import_conversion.rs");
include!("tests/desktop_suite_22_solver_and_expressions.rs");
include!("tests/desktop_suite_23_landmark_arrays.rs");
