use std::panic::{AssertUnwindSafe, catch_unwind};

use super::*;

#[test]
fn apply_request_requires_explicit_confirmation_and_has_a_closed_schema() {
    let token = ProjectId::new();
    let valid = serde_json::json!({
        "transactionToken": token,
        "explicitConfirmation": true
    });
    let parsed: ApplySpeculativeStackedFoldRequestV1 =
        serde_json::from_value(valid.clone()).expect("closed request");
    assert_eq!(parsed.transaction_token, token);
    assert!(parsed.explicit_confirmation);

    let mut missing = valid.clone();
    missing
        .as_object_mut()
        .expect("request object")
        .remove("explicitConfirmation");
    assert!(serde_json::from_value::<ApplySpeculativeStackedFoldRequestV1>(missing).is_err());

    let mut open = valid;
    open.as_object_mut().expect("request object").insert(
        "authorizesProjectMutation".to_owned(),
        serde_json::json!(true),
    );
    assert!(serde_json::from_value::<ApplySpeculativeStackedFoldRequestV1>(open).is_err());
}

#[test]
fn rejected_confirmation_does_not_consume_or_lock_the_pending_generation() {
    let app_state = AppState::new(super::super::super::initial_project_state());
    let foldability_state = GlobalFlatFoldabilityState::default();
    let transaction_state = StackedFoldTransactionState::default();
    let token = ProjectId::new();
    {
        let mut slot = lock_speculative_slot_v1(&transaction_state).expect("registry");
        slot.active_generation = Some(token);
    }
    assert_eq!(
        transaction_state.speculative_pending_token_for_test_v1(),
        Some(token)
    );

    let result = apply_speculative_stacked_fold_transaction_inner_v1(
        &app_state,
        &foldability_state,
        &transaction_state,
        ApplySpeculativeStackedFoldRequestV1 {
            transaction_token: token,
            explicit_confirmation: false,
        },
    );
    assert!(result.is_err());
    assert_eq!(
        lock_speculative_slot_v1(&transaction_state)
            .expect("registry")
            .active_generation,
        Some(token)
    );
}

#[test]
fn shared_cancel_command_cancels_speculative_tokens_idempotently() {
    let state = StackedFoldTransactionState::default();
    let token = ProjectId::new();
    {
        let mut slot = lock_speculative_slot_v1(&state).expect("registry");
        slot.active_generation = Some(token);
    }
    super::super::cancel_pending_stacked_fold(&state, token).expect("cancel");
    super::super::cancel_pending_stacked_fold(&state, token).expect("idempotent cancel");
    let slot = lock_speculative_slot_v1(&state).expect("registry");
    assert_eq!(slot.active_generation, None);
    assert_eq!(slot.last_cancelled, Some(token));
}

#[test]
fn speculative_registry_poison_fails_closed_without_poisoning_certified_registry() {
    let state = StackedFoldTransactionState::default();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _guard = state.1.lock().expect("speculative registry");
            panic!("poison speculative registry");
        }))
        .is_err()
    );
    assert!(lock_speculative_slot_v1(&state).is_err());
    assert!(super::super::lock_slot(&state).is_ok());
    assert!(super::super::cancel_pending_stacked_fold(&state, ProjectId::new()).is_err());
}

#[test]
fn apply_commands_reject_tokens_from_the_other_mode_without_consuming_them() {
    let app_state = AppState::new(super::super::super::initial_project_state());
    let foldability_state = GlobalFlatFoldabilityState::default();
    let state = StackedFoldTransactionState::default();

    let speculative_token = ProjectId::new();
    {
        let mut slot = lock_speculative_slot_v1(&state).expect("speculative registry");
        slot.active_generation = Some(speculative_token);
    }
    assert!(
        super::super::apply_stacked_fold_transaction_inner(
            &app_state,
            &foldability_state,
            &state,
            speculative_token,
        )
        .is_err()
    );
    assert_eq!(
        lock_speculative_slot_v1(&state)
            .expect("speculative registry")
            .active_generation,
        Some(speculative_token)
    );

    let certified_token = ProjectId::new();
    {
        let _gate = super::super::lock_transaction_mode_gate_v1(&state).expect("mode gate");
        clear_pending_speculative_stacked_fold_v1(&state).expect("clear speculative");
        let mut slot = super::super::lock_slot(&state).expect("certified registry");
        slot.active_generation = Some(certified_token);
    }
    assert!(
        apply_speculative_stacked_fold_transaction_inner_v1(
            &app_state,
            &foldability_state,
            &state,
            ApplySpeculativeStackedFoldRequestV1 {
                transaction_token: certified_token,
                explicit_confirmation: true,
            },
        )
        .is_err()
    );
    assert_eq!(
        super::super::lock_slot(&state)
            .expect("certified registry")
            .active_generation,
        Some(certified_token)
    );
}

#[test]
fn poisoned_mode_gate_fails_closed_before_either_registry_is_touched() {
    let state = StackedFoldTransactionState::default();
    let token = ProjectId::new();
    {
        let mut slot = lock_speculative_slot_v1(&state).expect("speculative registry");
        slot.active_generation = Some(token);
    }
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _guard = state.2.lock().expect("mode gate");
            panic!("poison mode gate");
        }))
        .is_err()
    );
    assert!(super::super::cancel_pending_stacked_fold(&state, token).is_err());
    assert_eq!(
        lock_speculative_slot_v1(&state)
            .expect("speculative registry")
            .active_generation,
        Some(token)
    );
}
