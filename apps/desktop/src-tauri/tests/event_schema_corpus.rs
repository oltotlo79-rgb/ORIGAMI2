use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CurrentCyclePoseProgressDtoV1<'a> {
    version: u32,
    request_id: &'a str,
    status: &'a str,
    completed_work: usize,
    total_work: usize,
    authorizes_project_mutation: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StackedFoldReadProgressDtoV1<'a> {
    version: u32,
    request_id: &'a str,
    explored_state_count: usize,
    evaluated_transition_count: usize,
    state_limit: usize,
    transition_limit: usize,
    authorizes_project_mutation: bool,
}

#[test]
fn rust_serialization_is_the_canonical_event_corpus() {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/tauri-event-v1-corpus.json"
    ))
    .expect("canonical event corpus must be JSON");
    let cycle = CurrentCyclePoseProgressDtoV1 {
        version: 1,
        request_id: "00000000-0000-4000-8000-000000000001",
        status: "running",
        completed_work: 1,
        total_work: 2,
        authorizes_project_mutation: false,
    };
    let stacked = StackedFoldReadProgressDtoV1 {
        version: 1,
        request_id: "00000000-0000-4000-8000-000000000002",
        explored_state_count: 31,
        evaluated_transition_count: 63,
        state_limit: 32,
        transition_limit: 64,
        authorizes_project_mutation: false,
    };
    assert_eq!(
        serde_json::to_value(cycle).unwrap(),
        corpus["current-cycle-pose-progress-v1"]
    );
    assert_eq!(
        serde_json::to_value(stacked).unwrap(),
        corpus["stacked-fold-read-progress-v1"]
    );
}

#[test]
fn rust_wire_key_order_and_integer_boundaries_are_pinned() {
    let wire = serde_json::to_string(&StackedFoldReadProgressDtoV1 {
        version: 1,
        request_id: "boundary",
        explored_state_count: 32,
        evaluated_transition_count: 64,
        state_limit: 32,
        transition_limit: 64,
        authorizes_project_mutation: false,
    })
    .unwrap();
    assert_eq!(
        wire,
        r#"{"version":1,"requestId":"boundary","exploredStateCount":32,"evaluatedTransitionCount":64,"stateLimit":32,"transitionLimit":64,"authorizesProjectMutation":false}"#
    );
}
