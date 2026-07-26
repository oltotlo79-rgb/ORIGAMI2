use super::GeometricConstraintKindV1;
use super::general_ratio_graph_tests::{
    Fixture, directed_cycle_records, prepare, record, remote_two_cycle_records, target,
};

#[test]
fn roots_groups_and_direction_are_never_combined_or_reversed() {
    let fixture = Fixture::new();
    let [
        first_root,
        second_root,
        merge,
        cycle_a,
        cycle_b,
        unrelated,
        ..,
    ] = fixture.edges;
    let separate_roots = [
        record(GeometricConstraintKindV1::FixedLength {
            edge: first_root,
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: second_root,
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: merge,
            denominator_edge: first_root,
            ratio: 2.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: merge,
            denominator_edge: second_root,
            ratio: 3.0,
        }),
    ];
    assert!(target(&prepare(&fixture, separate_roots).preflight()).is_none());

    let mut inconsistent =
        directed_cycle_records(&fixture, [0, 1, 2, 3], 0, 1.0, [2.0, 3.0, 5.0, 0.1]);
    inconsistent.push(record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge: fixture.edges[3],
        denominator_edge: fixture.edges[2],
        ratio: 7.0,
    }));
    assert!(target(&prepare(&fixture, inconsistent).preflight()).is_none());

    let mut inconsistent_fixed =
        directed_cycle_records(&fixture, [0, 1, 2, 3], 0, 1.0, [2.0, 3.0, 5.0, 0.1]);
    inconsistent_fixed.push(record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[0],
        length_mm: 2.0,
    }));
    assert!(target(&prepare(&fixture, inconsistent_fixed).preflight()).is_none());

    let disconnected = [
        record(GeometricConstraintKindV1::FixedLength {
            edge: unrelated,
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: cycle_b,
            denominator_edge: cycle_a,
            ratio: 2.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: cycle_a,
            denominator_edge: cycle_b,
            ratio: 0.25,
        }),
    ];
    assert!(target(&prepare(&fixture, disconnected).preflight()).is_none());

    let reverse_only = remote_two_cycle_records(&fixture)
        .into_iter()
        .map(|mut item| {
            if let GeometricConstraintKindV1::LengthRatio {
                numerator_edge,
                denominator_edge,
                ratio,
            } = item.constraint
            {
                item.constraint = GeometricConstraintKindV1::LengthRatio {
                    numerator_edge: denominator_edge,
                    denominator_edge: numerator_edge,
                    ratio: ratio.recip(),
                };
            }
            item
        });
    assert!(target(&prepare(&fixture, reverse_only).preflight()).is_none());
}
