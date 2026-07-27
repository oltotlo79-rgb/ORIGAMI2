use super::normal_form::{
    DirectedOrthogonalLabelV1, OrthogonalNormalFormSchemaV1, OrthogonalNormalFormV1,
};

fn apply_v1(
    schema: OrthogonalNormalFormSchemaV1,
    labels: &[DirectedOrthogonalLabelV1],
) -> Option<OrthogonalNormalFormV1> {
    let mut state = OrthogonalNormalFormV1::identity(schema)?;
    for label in labels {
        state = state.right_product(schema, *label)?;
    }
    Some(state)
}

#[test]
fn orthogonal_normal_form_fixes_involutions_and_twisted_relation() {
    let schema = OrthogonalNormalFormSchemaV1 {
        profile_count: 1,
        has_half_turn: true,
        has_reflection: true,
        has_twisted_reflection: true,
    };
    let identity = OrthogonalNormalFormV1::identity(schema).unwrap();
    for involution in [
        DirectedOrthogonalLabelV1::PrimaryHalfTurn,
        DirectedOrthogonalLabelV1::Reflection,
        DirectedOrthogonalLabelV1::TwistedReflection,
    ] {
        assert_eq!(
            apply_v1(schema, &[involution, involution]).unwrap(),
            identity
        );
        assert_eq!(involution.inverse().unwrap(), involution);
    }
    assert_eq!(
        apply_v1(
            schema,
            &[
                DirectedOrthogonalLabelV1::PrimaryHalfTurn,
                DirectedOrthogonalLabelV1::Reflection,
            ],
        )
        .unwrap(),
        apply_v1(schema, &[DirectedOrthogonalLabelV1::TwistedReflection]).unwrap()
    );
    assert_eq!(
        apply_v1(
            schema,
            &[
                DirectedOrthogonalLabelV1::PrimaryHalfTurn,
                DirectedOrthogonalLabelV1::Reflection,
                DirectedOrthogonalLabelV1::TwistedReflection,
            ],
        )
        .unwrap(),
        identity
    );
}

#[test]
fn orthogonal_normal_form_directly_fixes_conjugation_and_inverse_edges() {
    let schema = OrthogonalNormalFormSchemaV1 {
        profile_count: 2,
        has_half_turn: true,
        has_reflection: true,
        has_twisted_reflection: true,
    };
    let positive = DirectedOrthogonalLabelV1::Primary {
        profile: 1,
        sign: 1,
    };
    let negative = positive.inverse().unwrap();
    let state = apply_v1(
        schema,
        &[
            DirectedOrthogonalLabelV1::Reflection,
            positive,
            DirectedOrthogonalLabelV1::Reflection,
        ],
    )
    .unwrap();
    assert_eq!(state.components(), (&[0, -1][..], Some(false), Some(false)));
    assert_eq!(state, apply_v1(schema, &[negative]).unwrap());

    for prefix in [
        Vec::new(),
        vec![DirectedOrthogonalLabelV1::Reflection],
        vec![DirectedOrthogonalLabelV1::TwistedReflection],
    ] {
        let mut word = prefix;
        word.extend([positive, negative]);
        let state = apply_v1(schema, &word).unwrap();
        let prefix_state = apply_v1(schema, &word[..word.len() - 2]).unwrap();
        assert_eq!(state, prefix_state);
    }
}

#[test]
fn orthogonal_normal_form_rejects_unavailable_or_invalid_labels() {
    let free_only = OrthogonalNormalFormSchemaV1 {
        profile_count: 1,
        has_half_turn: false,
        has_reflection: false,
        has_twisted_reflection: false,
    };
    let identity = OrthogonalNormalFormV1::identity(free_only).unwrap();
    assert!(
        identity
            .right_product(free_only, DirectedOrthogonalLabelV1::PrimaryHalfTurn,)
            .is_none()
    );
    assert!(
        identity
            .right_product(free_only, DirectedOrthogonalLabelV1::Reflection)
            .is_none()
    );
    assert!(
        identity
            .right_product(free_only, DirectedOrthogonalLabelV1::TwistedReflection,)
            .is_none()
    );
    for invalid in [
        DirectedOrthogonalLabelV1::Primary {
            profile: 1,
            sign: 1,
        },
        DirectedOrthogonalLabelV1::Primary {
            profile: 0,
            sign: 0,
        },
    ] {
        assert!(identity.right_product(free_only, invalid).is_none());
    }

    let malformed = OrthogonalNormalFormSchemaV1 {
        profile_count: 0,
        has_half_turn: false,
        has_reflection: true,
        has_twisted_reflection: true,
    };
    assert!(OrthogonalNormalFormV1::identity(malformed).is_none());
}
