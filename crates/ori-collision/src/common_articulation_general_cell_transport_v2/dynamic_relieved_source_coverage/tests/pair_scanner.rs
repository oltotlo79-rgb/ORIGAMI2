use ori_foldability::LayerOrderSnapshot;

use crate::CommonArticulationGeneralCellTransportErrorV2;

#[test]
fn shared_pair_scanner_rejects_noncanonical_endpoints_duplicates_and_support_drift() {
    let fixture = super::super::super::test_support::transport_fixture_v2();
    let source =
        super::super::super::test_support::bounded_source_clone_for_test_v2(&fixture.source);
    assert!(
        super::super::super::source_binding::validate_pair_orders_v2(&source, &mut || Ok(()))
            .is_ok()
    );

    let mut equal = super::super::super::test_support::bounded_source_clone_for_test_v2(&source);
    equal.face_pair_orders[0].upper_face = equal.face_pair_orders[0].lower_face;
    assert_pair_scan_mismatch_v2(&equal);

    let mut unknown = super::super::super::test_support::bounded_source_clone_for_test_v2(&source);
    unknown.face_pair_orders[0].lower_face.face_key.0 = [0xff; 32];
    assert_pair_scan_mismatch_v2(&unknown);

    let mut duplicate =
        super::super::super::test_support::bounded_source_clone_for_test_v2(&source);
    duplicate
        .face_pair_orders
        .push(duplicate.face_pair_orders[0].clone());
    assert_pair_scan_mismatch_v2(&duplicate);

    let mut reversed = super::super::super::test_support::bounded_source_clone_for_test_v2(&source);
    let mut opposite = reversed.face_pair_orders[0].clone();
    std::mem::swap(&mut opposite.lower_face, &mut opposite.upper_face);
    reversed.face_pair_orders.push(opposite);
    reversed.face_pair_orders.sort_unstable_by_key(|pair| {
        (
            pair.lower_face.face_key.0,
            pair.upper_face.face_key.0,
            pair.lower_face.face_id.canonical_bytes(),
            pair.upper_face.face_id.canonical_bytes(),
        )
    });
    assert_pair_scan_mismatch_v2(&reversed);

    let mut no_support =
        super::super::super::test_support::bounded_source_clone_for_test_v2(&source);
    no_support.face_pair_orders[0].supporting_cells.clear();
    assert_pair_scan_mismatch_v2(&no_support);
}

fn assert_pair_scan_mismatch_v2(source: &LayerOrderSnapshot) {
    assert_eq!(
        super::super::super::source_binding::validate_pair_orders_v2(source, &mut || Ok(())),
        Err(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch)
    );
}
