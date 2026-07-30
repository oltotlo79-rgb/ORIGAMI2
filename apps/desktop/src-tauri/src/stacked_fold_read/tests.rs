use super::*;

#[path = "tests/cycle_authority_control_tests.rs"]
mod cycle_authority_control_tests;
#[path = "tests/dyadic_preview_tests.rs"]
mod dyadic_preview_tests;

// Keep the established `stacked_fold_read::tests::*` exact-filter surface
// while storing the large strict-scope fixture family separately.
include!("../stacked_fold_dyadic_scope_tests.rs");

include!("tests/read_suite_01_endpoint_cycle.rs");
include!("tests/read_suite_02_generation_fixtures.rs");
include!("tests/read_suite_03_tree_apply_four_to_six.rs");
include!("tests/read_suite_04_tree_apply_seven_eight.rs");
include!("tests/read_suite_05_projective_round_trip.rs");
include!("tests/read_suite_06_projective_dense.rs");
include!("tests/read_suite_07_cycle_boundary_fixtures.rs");
include!("tests/read_suite_08_even_cycle_archives.rs");
include!("tests/read_suite_09_upper_bound_cactus.rs");
include!("tests/read_suite_10_cactus_rank_theta.rs");
include!("tests/read_suite_11_large_cactus_and_generation.rs");

include!("../stacked_fold_speculative_unproven_tests.rs");
