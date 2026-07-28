use super::bounded::{
    checked_cycle_degree_capacity, checked_total, is_canonical_loop,
    preflight_material_relief_region_collections_v1,
    preflight_material_relief_substrate_collections_v1,
};
use super::test_support::{project_id, valid_fixture};
use super::*;
use crate::{CreasePattern, EdgeKind, Paper};

#[test]
fn exact_default_is_legacy_safe_strict_and_nonauthoritative() {
    let document = MaterialReliefDocumentV1::default();
    assert!(document.is_default());
    assert!(document.is_empty());
    assert_eq!(document.model_id(), MATERIAL_RELIEF_DOCUMENT_MODEL_ID_V1);
    assert!(!document.authorizes_project_mutation());
    assert!(!document.authorizes_material_removal());
    assert!(!document.authorizes_persistence());
    assert!(!document.authorizes_topology_admission());
    assert!(!document.authorizes_simulation_admission());
    assert!(!document.authorizes_collision_admission());
    assert!(!document.authorizes_proof_issuance());
    assert!(!document.authorizes_export());
    assert_eq!(
        validate_material_relief_document_v1(&document, &CreasePattern::empty(), &Paper::default()),
        Ok(())
    );

    let zeros = serde_json::to_string(&[0_u8; 32]).unwrap();
    assert_eq!(
        serde_json::to_string(&document).unwrap(),
        format!(
            r#"{{"version":1,"source_project_id":null,"substrate_fingerprint_sha256":{zeros},"state_sha256":{zeros},"regions":[]}}"#
        )
    );
    let restored: MaterialReliefDocumentV1 =
        serde_json::from_str(&serde_json::to_string(&document).unwrap()).unwrap();
    assert_eq!(restored, document);

    let mut non_default_empty = document;
    non_default_empty.source_project_id = Some(project_id());
    assert_eq!(
        validate_material_relief_document_v1(
            &non_default_empty,
            &CreasePattern::empty(),
            &Paper::default()
        ),
        Err(MaterialReliefDocumentValidationErrorV1::NonDefaultEmptyDocument)
    );
}

#[test]
fn serde_rejects_unknown_missing_and_malformed_fields() {
    let (_, _, document) = valid_fixture(1);
    let mut value = serde_json::to_value(&document).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("unknown".to_owned(), serde_json::json!(true));
    assert!(serde_json::from_value::<MaterialReliefDocumentV1>(value).is_err());

    let mut value = serde_json::to_value(&document).unwrap();
    value["regions"][0]
        .as_object_mut()
        .unwrap()
        .insert("unknown".to_owned(), serde_json::json!(true));
    assert!(serde_json::from_value::<MaterialReliefDocumentV1>(value).is_err());

    let mut value = serde_json::to_value(&document).unwrap();
    value.as_object_mut().unwrap().remove("state_sha256");
    assert!(serde_json::from_value::<MaterialReliefDocumentV1>(value).is_err());

    let mut value = serde_json::to_value(&document).unwrap();
    value["substrate_fingerprint_sha256"] = serde_json::json!([1]);
    assert!(serde_json::from_value::<MaterialReliefDocumentV1>(value).is_err());
}

#[test]
fn valid_document_round_trips_but_never_becomes_authority() {
    let (pattern, paper, document) = valid_fixture(2);
    validate_material_relief_document_v1(&document, &pattern, &paper).unwrap();
    let restored: MaterialReliefDocumentV1 =
        serde_json::from_str(&serde_json::to_string(&document).unwrap()).unwrap();
    assert_eq!(restored, document);
    validate_material_relief_document_v1(&restored, &pattern, &paper).unwrap();
    assert!(!restored.authorizes_project_mutation());
    assert!(!restored.authorizes_material_removal());
    assert!(!restored.authorizes_persistence());
    assert!(!restored.authorizes_topology_admission());
    assert!(!restored.authorizes_simulation_admission());
    assert!(!restored.authorizes_collision_admission());
    assert!(!restored.authorizes_proof_issuance());
    assert!(!restored.authorizes_export());
}

#[test]
fn v1_hash_and_lineage_golden_vectors_are_frozen() {
    let (_, _, document) = valid_fixture(2);
    let geometry = material_relief_geometry_sha256_v1(
        document.substrate_fingerprint_sha256,
        &document.regions,
    )
    .unwrap();
    let actual = (
        document.substrate_fingerprint_sha256,
        geometry,
        document.state_sha256,
        document.regions[0].lineage_id.canonical_bytes(),
    );
    let expected = (
        [
            0x1a, 0x21, 0x58, 0x89, 0x44, 0xc6, 0x9b, 0x86, 0x23, 0x01, 0x5f, 0xf6, 0xcf, 0x53,
            0xa8, 0xcb, 0x51, 0x52, 0x34, 0x7f, 0x5f, 0xdb, 0x2e, 0xe7, 0x72, 0xdb, 0xbe, 0x90,
            0x78, 0xfe, 0x3c, 0xbe,
        ],
        [
            0xb7, 0xf1, 0x96, 0x61, 0x28, 0x6e, 0x9d, 0x64, 0x27, 0x46, 0x0d, 0x35, 0xd8, 0xa3,
            0x4c, 0x33, 0x9a, 0xbf, 0x3e, 0xb8, 0xaa, 0xc7, 0x6e, 0xd3, 0xaa, 0x19, 0x98, 0x94,
            0xc6, 0xc8, 0x4c, 0x1e,
        ],
        [
            0x60, 0x1a, 0xdb, 0x66, 0xe0, 0x36, 0x2d, 0x89, 0x6d, 0xf4, 0xa3, 0xce, 0x49, 0x7f,
            0x93, 0x58, 0x0e, 0xa5, 0x53, 0x84, 0xac, 0x36, 0x8a, 0xe5, 0x84, 0x7b, 0x5f, 0x3e,
            0x3a, 0x25, 0x3b, 0x33,
        ],
        [
            0xdd, 0xe6, 0xc7, 0x93, 0x0e, 0x19, 0x52, 0x66, 0x85, 0xc4, 0x86, 0xb9, 0xe1, 0x60,
            0x08, 0x28,
        ],
    );
    assert_eq!(
        actual, expected,
        "V1 material-relief hashes and lineage are persistence contracts",
    );
}

#[test]
fn version_source_digest_and_cutting_bindings_fail_closed() {
    let (pattern, paper, document) = valid_fixture(1);

    let mut unsupported = document.clone();
    unsupported.version += 1;
    assert!(matches!(
        validate_material_relief_document_v1(&unsupported, &pattern, &paper),
        Err(MaterialReliefDocumentValidationErrorV1::UnsupportedVersion { .. })
    ));

    let mut missing_source = document.clone();
    missing_source.source_project_id = None;
    assert_eq!(
        validate_material_relief_document_v1(&missing_source, &pattern, &paper),
        Err(MaterialReliefDocumentValidationErrorV1::MissingSourceProjectId)
    );

    let nil_project: ProjectId =
        serde_json::from_str(r#""00000000-0000-0000-0000-000000000000""#).unwrap();
    let mut nil_source = document.clone();
    nil_source.source_project_id = Some(nil_project);
    assert_eq!(
        validate_material_relief_document_v1(&nil_source, &pattern, &paper),
        Err(MaterialReliefDocumentValidationErrorV1::NilSourceProjectId)
    );

    let mut zero_substrate = document.clone();
    zero_substrate.substrate_fingerprint_sha256 = [0; 32];
    assert_eq!(
        validate_material_relief_document_v1(&zero_substrate, &pattern, &paper),
        Err(MaterialReliefDocumentValidationErrorV1::ZeroSubstrateFingerprint)
    );

    let mut zero_state = document.clone();
    zero_state.state_sha256 = [0; 32];
    assert_eq!(
        validate_material_relief_document_v1(&zero_state, &pattern, &paper),
        Err(MaterialReliefDocumentValidationErrorV1::ZeroStateDigest)
    );

    let mut cutting_disabled = paper;
    cutting_disabled.cutting_allowed = false;
    assert_eq!(
        validate_material_relief_document_v1(&document, &pattern, &cutting_disabled),
        Err(MaterialReliefDocumentValidationErrorV1::CuttingNotAllowed)
    );
}

#[test]
fn substrate_hash_normalizes_storage_direction_and_boundary_cycle() {
    let (pattern, paper, _) = valid_fixture(2);
    let expected = material_relief_substrate_sha256_v1(&pattern, &paper).unwrap();
    let mut reordered_pattern = pattern.clone();
    reordered_pattern.vertices.reverse();
    reordered_pattern.edges.reverse();
    for edge in &mut reordered_pattern.edges {
        std::mem::swap(&mut edge.start, &mut edge.end);
    }
    let mut reordered_paper = paper.clone();
    reordered_paper.boundary_vertices.rotate_left(1);
    reordered_paper.boundary_vertices.reverse();
    assert_eq!(
        material_relief_substrate_sha256_v1(&reordered_pattern, &reordered_paper).unwrap(),
        expected
    );

    reordered_paper.thickness_mm = 9.5;
    reordered_paper.length_display_unit = crate::LengthDisplayUnit::Centimeter;
    reordered_paper.front.color = crate::RgbaColor::opaque(1, 2, 3);
    assert_eq!(
        material_relief_substrate_sha256_v1(&reordered_pattern, &reordered_paper).unwrap(),
        expected
    );

    reordered_paper.cutting_allowed = false;
    assert_ne!(
        material_relief_substrate_sha256_v1(&reordered_pattern, &reordered_paper).unwrap(),
        expected
    );
    reordered_paper.cutting_allowed = true;
    reordered_pattern.vertices[0].position.x += 0.25;
    assert_ne!(
        material_relief_substrate_sha256_v1(&reordered_pattern, &reordered_paper).unwrap(),
        expected
    );
}

#[test]
fn substrate_and_geometry_hashes_are_id_preserving_not_rekeying_invariant() {
    let (pattern, paper, document) = valid_fixture(1);
    let original_substrate = material_relief_substrate_sha256_v1(&pattern, &paper).unwrap();
    let original_geometry =
        material_relief_geometry_sha256_v1(original_substrate, &document.regions).unwrap();

    let rekey_namespace = ProjectId::schema_namespace([0x66; 16]);
    let vertex_remap = pattern
        .vertices
        .iter()
        .enumerate()
        .map(|(index, vertex)| {
            (
                vertex.id,
                VertexId::derive_v5(rekey_namespace, &(index as u64).to_be_bytes()),
            )
        })
        .collect::<Vec<_>>();
    let mut rekeyed_pattern = pattern.clone();
    for vertex in &mut rekeyed_pattern.vertices {
        vertex.id = vertex_remap
            .iter()
            .find_map(|(source, target)| (*source == vertex.id).then_some(*target))
            .unwrap();
    }
    for (index, edge) in rekeyed_pattern.edges.iter_mut().enumerate() {
        edge.id = EdgeId::derive_v5(rekey_namespace, &(index as u64).to_be_bytes());
        edge.start = vertex_remap
            .iter()
            .find_map(|(source, target)| (*source == edge.start).then_some(*target))
            .unwrap();
        edge.end = vertex_remap
            .iter()
            .find_map(|(source, target)| (*source == edge.end).then_some(*target))
            .unwrap();
    }
    let mut rekeyed_paper = paper;
    for boundary_vertex in &mut rekeyed_paper.boundary_vertices {
        *boundary_vertex = vertex_remap
            .iter()
            .find_map(|(source, target)| (*source == *boundary_vertex).then_some(*target))
            .unwrap();
    }

    let rekeyed_substrate =
        material_relief_substrate_sha256_v1(&rekeyed_pattern, &rekeyed_paper).unwrap();
    assert_ne!(rekeyed_substrate, original_substrate);
    assert_ne!(
        material_relief_geometry_sha256_v1(rekeyed_substrate, &document.regions).unwrap(),
        original_geometry
    );
}

#[test]
fn geometry_and_state_hashes_have_separate_bindings() {
    let (_, _, document) = valid_fixture(2);
    let geometry = material_relief_geometry_sha256_v1(
        document.substrate_fingerprint_sha256,
        &document.regions,
    )
    .unwrap();
    let other_project = ProjectId::schema_namespace([0x77; 16]);
    let mut other_lineages = document.regions.clone();
    other_lineages[0].lineage_id = MaterialReliefLineageId::derive_v5(
        other_project,
        document.substrate_fingerprint_sha256,
        other_lineages[0].requested_component_key,
    );
    assert_eq!(
        material_relief_geometry_sha256_v1(document.substrate_fingerprint_sha256, &other_lineages)
            .unwrap(),
        geometry
    );
    assert_ne!(
        material_relief_state_sha256_v1(
            project_id(),
            document.substrate_fingerprint_sha256,
            &other_lineages
        )
        .unwrap(),
        document.state_sha256
    );

    assert_eq!(
        material_relief_geometry_sha256_v1(
            document.substrate_fingerprint_sha256,
            &document.regions
        )
        .unwrap(),
        geometry
    );
    assert_ne!(
        material_relief_state_sha256_v1(
            other_project,
            document.substrate_fingerprint_sha256,
            &document.regions
        )
        .unwrap(),
        document.state_sha256
    );
}

#[test]
fn lineage_is_stable_and_binds_project_substrate_and_request() {
    let (_, _, document) = valid_fixture(1);
    let request = document.regions[0].requested_component_key;
    let first = MaterialReliefLineageId::derive_v5(
        project_id(),
        document.substrate_fingerprint_sha256,
        request,
    );
    let second = MaterialReliefLineageId::derive_v5(
        project_id(),
        document.substrate_fingerprint_sha256,
        request,
    );
    assert_eq!(first, second);
    assert_ne!(
        first,
        MaterialReliefLineageId::derive_v5(
            ProjectId::schema_namespace([0x99; 16]),
            document.substrate_fingerprint_sha256,
            request,
        )
    );
    assert_ne!(
        first,
        MaterialReliefLineageId::derive_v5(project_id(), [0x44; 32], request)
    );
    assert_ne!(
        first,
        MaterialReliefLineageId::derive_v5(
            project_id(),
            document.substrate_fingerprint_sha256,
            [0x55; 32],
        )
    );
}

#[test]
fn substrate_state_and_structural_tampering_fail_closed() {
    let (pattern, paper, document) = valid_fixture(1);

    let mut tampered = document.clone();
    tampered.substrate_fingerprint_sha256[0] ^= 1;
    assert_eq!(
        validate_material_relief_document_v1(&tampered, &pattern, &paper),
        Err(MaterialReliefDocumentValidationErrorV1::SubstrateFingerprintMismatch)
    );

    let mut tampered = document.clone();
    tampered.state_sha256[0] ^= 1;
    assert_eq!(
        validate_material_relief_document_v1(&tampered, &pattern, &paper),
        Err(MaterialReliefDocumentValidationErrorV1::StateDigestMismatch)
    );

    let mut tampered = document.clone();
    tampered.regions[0].lineage_id = MaterialReliefLineageId::derive_v5(
        ProjectId::schema_namespace([0x88; 16]),
        tampered.substrate_fingerprint_sha256,
        tampered.regions[0].requested_component_key,
    );
    assert_eq!(
        validate_material_relief_document_v1(&tampered, &pattern, &paper),
        Err(MaterialReliefDocumentValidationErrorV1::InvalidLineageId { region_index: 0 })
    );

    let mut tampered = document.clone();
    tampered.regions[0].removed_component_keys.push([0xff; 32]);
    tampered.regions[0].removed_component_keys.sort_unstable();
    assert_eq!(
        validate_material_relief_document_v1(&tampered, &pattern, &paper),
        Err(MaterialReliefDocumentValidationErrorV1::StateDigestMismatch)
    );

    let mut tampered = document;
    tampered.regions[0].boundary_edge_loop.reverse();
    assert_eq!(
        validate_material_relief_document_v1(&tampered, &pattern, &paper),
        Err(MaterialReliefDocumentValidationErrorV1::InvalidBoundaryLoop { region_index: 0 })
    );
}

#[test]
fn canonical_edge_loop_has_exactly_one_rotation_and_direction() {
    let (_, _, document) = valid_fixture(1);
    let canonical = document.regions[0].boundary_edge_loop.clone();
    let mut admitted = Vec::new();
    for reverse in [false, true] {
        let mut direction = canonical.clone();
        if reverse {
            direction.reverse();
        }
        for rotation in 0..direction.len() {
            let mut candidate = direction.clone();
            candidate.rotate_left(rotation);
            if is_canonical_loop(&candidate) {
                admitted.push(candidate);
            }
        }
    }
    assert_eq!(admitted, vec![canonical]);
}

#[test]
fn noncanonical_regions_closures_and_edge_reuse_are_rejected() {
    let (pattern, paper, document) = valid_fixture(2);

    let mut noncanonical = document.clone();
    noncanonical.regions.reverse();
    assert!(matches!(
        validate_material_relief_document_v1(&noncanonical, &pattern, &paper),
        Err(MaterialReliefDocumentValidationErrorV1::NonCanonicalRegionOrder { .. })
    ));

    let mut duplicate_lineage = document.clone();
    duplicate_lineage.regions[1].lineage_id = duplicate_lineage.regions[0].lineage_id;
    assert_eq!(
        validate_material_relief_document_v1(&duplicate_lineage, &pattern, &paper),
        Err(MaterialReliefDocumentValidationErrorV1::DuplicateLineageId { region_index: 1 })
    );

    let mut noncanonical_closure = document.clone();
    noncanonical_closure.regions[0]
        .removed_component_keys
        .reverse();
    assert_eq!(
        validate_material_relief_document_v1(&noncanonical_closure, &pattern, &paper),
        Err(
            MaterialReliefDocumentValidationErrorV1::RemovedComponentsNotCanonical {
                region_index: 0
            }
        )
    );

    let mut overlap = document.clone();
    let overlapping_component_key = overlap.regions[0].removed_component_keys[0];
    overlap.regions[1]
        .removed_component_keys
        .push(overlapping_component_key);
    overlap.regions[1].removed_component_keys.sort_unstable();
    assert_eq!(
        validate_material_relief_document_v1(&overlap, &pattern, &paper),
        Err(
            MaterialReliefDocumentValidationErrorV1::RemovedComponentClosureOverlap {
                region_index: 1
            }
        )
    );

    let mut reused_edge = document;
    reused_edge.regions[1].boundary_edge_loop = reused_edge.regions[0].boundary_edge_loop.clone();
    assert!(matches!(
        validate_material_relief_document_v1(&reused_edge, &pattern, &paper),
        Err(
            MaterialReliefDocumentValidationErrorV1::BoundaryEdgeReused {
                region_index: 1,
                ..
            }
        )
    ));
}

#[test]
fn disconnected_unknown_and_noncut_loops_are_rejected() {
    let (pattern, paper, document) = valid_fixture(1);
    let mut disconnected_pattern = pattern.clone();
    disconnected_pattern.edges[1].start =
        disconnected_pattern.vertices[disconnected_pattern.vertices.len() - 1].id;
    let disconnected_substrate =
        material_relief_substrate_sha256_v1(&disconnected_pattern, &paper).unwrap();
    let mut disconnected = document.clone();
    disconnected.substrate_fingerprint_sha256 = disconnected_substrate;
    disconnected.regions[0].lineage_id = MaterialReliefLineageId::derive_v5(
        project_id(),
        disconnected_substrate,
        disconnected.regions[0].requested_component_key,
    );
    disconnected.state_sha256 = material_relief_state_sha256_v1(
        project_id(),
        disconnected_substrate,
        &disconnected.regions,
    )
    .unwrap();
    assert_eq!(
        validate_material_relief_document_v1(&disconnected, &disconnected_pattern, &paper),
        Err(MaterialReliefDocumentValidationErrorV1::InvalidBoundaryLoop { region_index: 0 })
    );

    let mut noncut_pattern = pattern.clone();
    noncut_pattern.edges[0].kind = EdgeKind::Mountain;
    let noncut_substrate = material_relief_substrate_sha256_v1(&noncut_pattern, &paper).unwrap();
    let mut noncut = document.clone();
    noncut.substrate_fingerprint_sha256 = noncut_substrate;
    noncut.regions[0].lineage_id = MaterialReliefLineageId::derive_v5(
        project_id(),
        noncut_substrate,
        noncut.regions[0].requested_component_key,
    );
    noncut.state_sha256 =
        material_relief_state_sha256_v1(project_id(), noncut_substrate, &noncut.regions).unwrap();
    assert!(matches!(
        validate_material_relief_document_v1(&noncut, &noncut_pattern, &paper),
        Err(
            MaterialReliefDocumentValidationErrorV1::NonCutBoundaryEdge {
                region_index: 0,
                ..
            }
        )
    ));

    let mut unknown = document;
    unknown.regions[0].boundary_edge_loop[1] = EdgeId::derive_v5(project_id(), b"unknown-edge");
    unknown.regions[0]
        .boundary_edge_loop
        .sort_unstable_by_key(EdgeId::canonical_bytes);
    assert!(matches!(
        validate_material_relief_document_v1(&unknown, &pattern, &paper),
        Err(MaterialReliefDocumentValidationErrorV1::UnknownBoundaryEdge { .. })
    ));
}

#[test]
fn nil_pattern_vertex_and_edge_ids_are_rejected_before_hash_binding() {
    let (pattern, paper, document) = valid_fixture(1);
    let nil_vertex: VertexId =
        serde_json::from_str(r#""00000000-0000-0000-0000-000000000000""#).unwrap();
    let nil_edge: EdgeId =
        serde_json::from_str(r#""00000000-0000-0000-0000-000000000000""#).unwrap();

    let mut nil_vertex_pattern = pattern.clone();
    nil_vertex_pattern.vertices[0].id = nil_vertex;
    assert_eq!(
        material_relief_substrate_sha256_v1(&nil_vertex_pattern, &paper),
        Err(MaterialReliefDocumentValidationErrorV1::NilPatternVertex { vertex: nil_vertex })
    );
    assert_eq!(
        validate_material_relief_document_v1(&document, &nil_vertex_pattern, &paper),
        Err(MaterialReliefDocumentValidationErrorV1::NilPatternVertex { vertex: nil_vertex })
    );

    let mut nil_edge_pattern = pattern;
    nil_edge_pattern.edges[0].id = nil_edge;
    assert_eq!(
        material_relief_substrate_sha256_v1(&nil_edge_pattern, &paper),
        Err(MaterialReliefDocumentValidationErrorV1::NilPatternEdge { edge: nil_edge })
    );
    assert_eq!(
        validate_material_relief_document_v1(&document, &nil_edge_pattern, &paper),
        Err(MaterialReliefDocumentValidationErrorV1::NilPatternEdge { edge: nil_edge })
    );
}

#[test]
fn public_hash_collection_preflights_are_exact_and_bounded() {
    assert_eq!(
        preflight_material_relief_substrate_collections_v1(
            MAX_MATERIAL_RELIEF_PATTERN_VERTICES_V1,
            MAX_MATERIAL_RELIEF_PATTERN_EDGES_V1,
            MAX_MATERIAL_RELIEF_PAPER_BOUNDARY_VERTICES_V1,
        ),
        Ok(())
    );
    assert_eq!(
        preflight_material_relief_substrate_collections_v1(
            MAX_MATERIAL_RELIEF_PATTERN_VERTICES_V1 + 1,
            0,
            0,
        ),
        Err(
            MaterialReliefDocumentValidationErrorV1::TooManyPatternVertices {
                actual: MAX_MATERIAL_RELIEF_PATTERN_VERTICES_V1 + 1,
                maximum: MAX_MATERIAL_RELIEF_PATTERN_VERTICES_V1,
            }
        )
    );
    assert_eq!(
        preflight_material_relief_substrate_collections_v1(
            0,
            MAX_MATERIAL_RELIEF_PATTERN_EDGES_V1 + 1,
            0,
        ),
        Err(
            MaterialReliefDocumentValidationErrorV1::TooManyPatternEdges {
                actual: MAX_MATERIAL_RELIEF_PATTERN_EDGES_V1 + 1,
                maximum: MAX_MATERIAL_RELIEF_PATTERN_EDGES_V1,
            }
        )
    );
    assert_eq!(
        preflight_material_relief_substrate_collections_v1(
            0,
            0,
            MAX_MATERIAL_RELIEF_PAPER_BOUNDARY_VERTICES_V1 + 1,
        ),
        Err(
            MaterialReliefDocumentValidationErrorV1::TooManyPaperBoundaryVertices {
                actual: MAX_MATERIAL_RELIEF_PAPER_BOUNDARY_VERTICES_V1 + 1,
                maximum: MAX_MATERIAL_RELIEF_PAPER_BOUNDARY_VERTICES_V1,
            }
        )
    );

    let (_, _, document) = valid_fixture(1);
    let template = document.regions[0].clone();
    let mut exact = vec![template.clone(); MAX_MATERIAL_RELIEF_REGIONS_V1];
    for (index, region) in exact.iter_mut().enumerate() {
        region.removed_component_keys =
            vec![[u8::try_from(index + 1).unwrap(); 32]; MAX_MATERIAL_RELIEF_REMOVED_COMPONENTS_V1];
        region.boundary_edge_loop.clear();
    }
    exact[0].boundary_edge_loop =
        vec![template.boundary_edge_loop[0]; MAX_MATERIAL_RELIEF_TOTAL_LOOP_EDGES_V1];
    assert_eq!(
        preflight_material_relief_region_collections_v1(&exact),
        Ok((
            MAX_MATERIAL_RELIEF_TOTAL_REMOVED_COMPONENTS_V1,
            MAX_MATERIAL_RELIEF_TOTAL_LOOP_EDGES_V1,
        ))
    );
    assert!(
        material_relief_geometry_sha256_v1(document.substrate_fingerprint_sha256, &exact).is_ok()
    );
    assert!(
        material_relief_state_sha256_v1(
            project_id(),
            document.substrate_fingerprint_sha256,
            &exact,
        )
        .is_ok()
    );

    let mut too_many_regions = exact.clone();
    too_many_regions.push(template.clone());
    assert_eq!(
        material_relief_geometry_sha256_v1(
            document.substrate_fingerprint_sha256,
            &too_many_regions,
        ),
        Err(MaterialReliefDocumentValidationErrorV1::TooManyRegions {
            actual: MAX_MATERIAL_RELIEF_REGIONS_V1 + 1,
            maximum: MAX_MATERIAL_RELIEF_REGIONS_V1,
        })
    );
    assert_eq!(
        material_relief_state_sha256_v1(
            project_id(),
            document.substrate_fingerprint_sha256,
            &too_many_regions,
        ),
        Err(MaterialReliefDocumentValidationErrorV1::TooManyRegions {
            actual: MAX_MATERIAL_RELIEF_REGIONS_V1 + 1,
            maximum: MAX_MATERIAL_RELIEF_REGIONS_V1,
        })
    );

    exact[0].removed_component_keys.push([0xfe; 32]);
    assert_eq!(
        material_relief_geometry_sha256_v1(document.substrate_fingerprint_sha256, &exact),
        Err(
            MaterialReliefDocumentValidationErrorV1::TooManyTotalRemovedComponents {
                actual: MAX_MATERIAL_RELIEF_TOTAL_REMOVED_COMPONENTS_V1 + 1,
                maximum: MAX_MATERIAL_RELIEF_TOTAL_REMOVED_COMPONENTS_V1,
            }
        )
    );
    exact[0].removed_component_keys = vec![[1; 32]; MAX_MATERIAL_RELIEF_REMOVED_COMPONENTS_V1];

    let mut too_many_removed_in_one_region = vec![template.clone()];
    too_many_removed_in_one_region[0].removed_component_keys =
        vec![[1; 32]; MAX_MATERIAL_RELIEF_REMOVED_COMPONENTS_V1 + 1];
    assert_eq!(
        material_relief_geometry_sha256_v1(
            document.substrate_fingerprint_sha256,
            &too_many_removed_in_one_region,
        ),
        Err(
            MaterialReliefDocumentValidationErrorV1::TooManyRemovedComponents {
                region_index: 0,
                actual: MAX_MATERIAL_RELIEF_REMOVED_COMPONENTS_V1 + 1,
                maximum: MAX_MATERIAL_RELIEF_REMOVED_COMPONENTS_V1,
            }
        )
    );

    exact[0].boundary_edge_loop =
        vec![template.boundary_edge_loop[0]; MAX_MATERIAL_RELIEF_LOOP_EDGES_V1 + 1];
    assert_eq!(
        material_relief_geometry_sha256_v1(document.substrate_fingerprint_sha256, &exact),
        Err(MaterialReliefDocumentValidationErrorV1::TooManyLoopEdges {
            region_index: 0,
            actual: MAX_MATERIAL_RELIEF_LOOP_EDGES_V1 + 1,
            maximum: MAX_MATERIAL_RELIEF_LOOP_EDGES_V1,
        })
    );
    exact[0].boundary_edge_loop =
        vec![template.boundary_edge_loop[0]; MAX_MATERIAL_RELIEF_TOTAL_LOOP_EDGES_V1];
    exact[1]
        .boundary_edge_loop
        .push(template.boundary_edge_loop[1]);
    assert_eq!(
        material_relief_geometry_sha256_v1(document.substrate_fingerprint_sha256, &exact),
        Err(
            MaterialReliefDocumentValidationErrorV1::TooManyTotalLoopEdges {
                actual: MAX_MATERIAL_RELIEF_TOTAL_LOOP_EDGES_V1 + 1,
                maximum: MAX_MATERIAL_RELIEF_TOTAL_LOOP_EDGES_V1,
            }
        )
    );

    assert_eq!(
        MAX_MATERIAL_RELIEF_REGIONS_V1.checked_mul(MAX_MATERIAL_RELIEF_REMOVED_COMPONENTS_V1),
        Some(MAX_MATERIAL_RELIEF_TOTAL_REMOVED_COMPONENTS_V1)
    );
    assert_eq!(
        checked_cycle_degree_capacity(MAX_MATERIAL_RELIEF_LOOP_EDGES_V1),
        Ok(MAX_MATERIAL_RELIEF_LOOP_EDGES_V1 * 2)
    );
    assert_eq!(
        checked_cycle_degree_capacity(usize::MAX),
        Err(MaterialReliefDocumentValidationErrorV1::ResourceAllocation)
    );
    assert_eq!(
        checked_total([usize::MAX, 1]),
        Err(MaterialReliefDocumentValidationErrorV1::ResourceAllocation)
    );
}

#[test]
fn resource_limits_fail_before_unbounded_cross_reference_work() {
    let (pattern, paper, document) = valid_fixture(1);

    let mut too_many_regions = document.clone();
    too_many_regions.regions =
        vec![document.regions[0].clone(); MAX_MATERIAL_RELIEF_REGIONS_V1 + 1];
    assert!(matches!(
        validate_material_relief_document_v1(&too_many_regions, &pattern, &paper),
        Err(MaterialReliefDocumentValidationErrorV1::TooManyRegions { .. })
    ));

    let mut too_many_components = document.clone();
    too_many_components.regions[0].removed_component_keys =
        vec![[1; 32]; MAX_MATERIAL_RELIEF_REMOVED_COMPONENTS_V1 + 1];
    assert!(matches!(
        validate_material_relief_document_v1(&too_many_components, &pattern, &paper),
        Err(MaterialReliefDocumentValidationErrorV1::TooManyRemovedComponents { .. })
    ));

    let mut too_many_loop_edges = document.clone();
    too_many_loop_edges.regions[0].boundary_edge_loop =
        vec![pattern.edges[0].id; MAX_MATERIAL_RELIEF_LOOP_EDGES_V1 + 1];
    assert!(matches!(
        validate_material_relief_document_v1(&too_many_loop_edges, &pattern, &paper),
        Err(MaterialReliefDocumentValidationErrorV1::TooManyLoopEdges { .. })
    ));

    let (two_region_pattern, two_region_paper, mut too_many_total_loop_edges) = valid_fixture(2);
    let half_plus_one = MAX_MATERIAL_RELIEF_TOTAL_LOOP_EDGES_V1 / 2 + 1;
    too_many_total_loop_edges.regions[0].boundary_edge_loop =
        vec![two_region_pattern.edges[0].id; half_plus_one];
    too_many_total_loop_edges.regions[1].boundary_edge_loop =
        vec![two_region_pattern.edges[4].id; half_plus_one];
    assert!(matches!(
        validate_material_relief_document_v1(
            &too_many_total_loop_edges,
            &two_region_pattern,
            &two_region_paper
        ),
        Err(MaterialReliefDocumentValidationErrorV1::TooManyTotalLoopEdges { .. })
    ));

    let mut too_many_pattern_vertices = pattern.clone();
    too_many_pattern_vertices.vertices = vec![
        too_many_pattern_vertices.vertices[0].clone();
        MAX_MATERIAL_RELIEF_PATTERN_VERTICES_V1 + 1
    ];
    assert!(matches!(
        material_relief_substrate_sha256_v1(&too_many_pattern_vertices, &paper),
        Err(MaterialReliefDocumentValidationErrorV1::TooManyPatternVertices { .. })
    ));
    assert!(matches!(
        validate_material_relief_document_v1(&document, &too_many_pattern_vertices, &paper),
        Err(MaterialReliefDocumentValidationErrorV1::TooManyPatternVertices { .. })
    ));

    let mut too_many_boundary_vertices = paper.clone();
    too_many_boundary_vertices.boundary_vertices =
        vec![pattern.vertices[0].id; MAX_MATERIAL_RELIEF_PAPER_BOUNDARY_VERTICES_V1 + 1];
    assert!(matches!(
        material_relief_substrate_sha256_v1(&pattern, &too_many_boundary_vertices),
        Err(MaterialReliefDocumentValidationErrorV1::TooManyPaperBoundaryVertices { .. })
    ));
    assert!(matches!(
        validate_material_relief_document_v1(&document, &pattern, &too_many_boundary_vertices),
        Err(MaterialReliefDocumentValidationErrorV1::TooManyPaperBoundaryVertices { .. })
    ));

    let mut too_many_pattern_edges = pattern;
    too_many_pattern_edges.edges =
        vec![too_many_pattern_edges.edges[0].clone(); MAX_MATERIAL_RELIEF_PATTERN_EDGES_V1 + 1];
    assert!(matches!(
        material_relief_substrate_sha256_v1(&too_many_pattern_edges, &paper),
        Err(MaterialReliefDocumentValidationErrorV1::TooManyPatternEdges { .. })
    ));
    assert!(matches!(
        validate_material_relief_document_v1(&document, &too_many_pattern_edges, &paper),
        Err(MaterialReliefDocumentValidationErrorV1::TooManyPatternEdges { .. })
    ));
}
