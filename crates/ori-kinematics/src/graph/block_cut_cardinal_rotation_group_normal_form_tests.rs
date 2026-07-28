use super::normal_form::CardinalRotationV1;

#[test]
fn cardinal_quarter_turns_have_exact_orders_and_inverses() {
    for axis in 0..3 {
        let identity = CardinalRotationV1::identity();
        let positive = CardinalRotationV1::quarter_turn(axis, 1).unwrap();
        let negative = CardinalRotationV1::quarter_turn(axis, -1).unwrap();
        let half = CardinalRotationV1::quarter_turn(axis, 2).unwrap();
        assert!(positive.is_valid());
        assert_eq!(positive.inverse(), Some(negative));
        assert_eq!(positive.right_product(positive), Some(half));
        assert_eq!(half.right_product(half), Some(identity));
        assert_eq!(CardinalRotationV1::quarter_turn(axis, 4), Some(identity));
    }
}

#[test]
fn perpendicular_quarter_turn_product_has_exact_order_three() {
    let first = CardinalRotationV1::quarter_turn(0, 1).unwrap();
    let second = CardinalRotationV1::quarter_turn(1, 1).unwrap();
    let product = first.right_product(second).unwrap();
    assert_eq!(
        product,
        CardinalRotationV1::from_matrix_for_test([[0, 0, 1], [1, 0, 0], [0, 1, 0]])
    );
    assert_ne!(product, second.right_product(first).unwrap());
    assert_eq!(
        product
            .right_product(product)
            .unwrap()
            .right_product(product),
        Some(CardinalRotationV1::identity())
    );
}

#[test]
fn cardinal_normal_form_rejects_invalid_axes_and_matrices() {
    assert!(CardinalRotationV1::quarter_turn(3, 1).is_none());
    for matrix in [
        [[0, 0, 0], [0, 1, 0], [0, 0, 1]],
        [[1, 0, 0], [1, 0, 0], [0, 0, 1]],
        [[-1, 0, 0], [0, 1, 0], [0, 0, 1]],
        [[2, 0, 0], [0, 1, 0], [0, 0, 1]],
    ] {
        let invalid = CardinalRotationV1::from_matrix_for_test(matrix);
        assert!(!invalid.is_valid());
        assert!(invalid.inverse().is_none());
    }
}
