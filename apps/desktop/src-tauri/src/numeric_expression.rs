use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::atomic::{AtomicBool, Ordering},
};

use ori_numeric::{
    ExpressionError, ExpressionLimits, HARD_MAX_OPERATIONS, HARD_MAX_SOURCE_BYTES,
    MAX_PRECISION_BITS, MIN_PRECISION_BITS, ScalarExpression,
};
use serde::{Deserialize, Serialize};

const NUMERIC_EXPRESSION_SCHEMA: &str = "origami2.numeric-expression-evaluation.v1";
const MAX_DISPLAY_BYTES: usize = 32;
pub(super) const USER_INPUT_PRECISION_BITS: u16 = 192;
static NUMERIC_EXPRESSION_WORKER_GATE: NumericExpressionWorkerGate =
    NumericExpressionWorkerGate::new();
#[cfg(test)]
static SYNCHRONOUS_EVALUATION_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct NumericExpressionWorkerGate(AtomicBool);

impl NumericExpressionWorkerGate {
    const fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    fn try_acquire(&self) -> Option<NumericExpressionWorkerPermit<'_>> {
        self.0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| NumericExpressionWorkerPermit { busy: &self.0 })
    }

    #[cfg(test)]
    fn is_busy(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

struct NumericExpressionWorkerPermit<'a> {
    busy: &'a AtomicBool,
}

impl Drop for NumericExpressionWorkerPermit<'_> {
    fn drop(&mut self) {
        let was_busy = self.busy.swap(false, Ordering::Release);
        debug_assert!(was_busy, "numeric expression worker permit released twice");
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct NumericExpressionRequest {
    source: String,
    precision_bits: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct NumericExpressionCommandEnvelope {
    request: NumericExpressionRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum NumericExpressionErrorCategory {
    InvalidRequest,
    InvalidExpression,
    ResourceLimit,
    ResultOutOfRange,
    InternalFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct NumericExpressionCommandError {
    category: NumericExpressionErrorCategory,
}

impl NumericExpressionCommandError {
    const fn new(category: NumericExpressionErrorCategory) -> Self {
        Self { category }
    }

    pub(super) const fn user_input_message(self) -> &'static str {
        match self.category {
            NumericExpressionErrorCategory::InvalidRequest => {
                "numeric expression request is invalid"
            }
            NumericExpressionErrorCategory::InvalidExpression => "numeric expression is invalid",
            NumericExpressionErrorCategory::ResourceLimit => {
                "numeric expression exceeded its resource limit"
            }
            NumericExpressionErrorCategory::ResultOutOfRange => {
                "numeric expression cannot be adopted as a positive millimetre value"
            }
            NumericExpressionErrorCategory::InternalFailure => {
                "numeric expression evaluation failed internally"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PositiveMillimetrePairError {
    WorkerBusy,
    Evaluation(NumericExpressionCommandError),
}

impl PositiveMillimetrePairError {
    pub(super) const fn is_worker_busy(self) -> bool {
        matches!(self, Self::WorkerBusy)
    }

    pub(super) const fn user_input_message(self) -> &'static str {
        match self {
            Self::WorkerBusy => "numeric expression evaluation is already in progress",
            Self::Evaluation(error) => error.user_input_message(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct NumericExpressionResponse {
    schema: &'static str,
    source: String,
    requested_precision_bits: u16,
    exact: bool,
    operations: u32,
    lower_bound: f64,
    upper_bound: f64,
    lower_display: String,
    upper_display: String,
}

/// Evaluates one scalar expression away from the WebView and returns only a
/// finite, outward-rounded binary64 enclosure plus fixed-size display text.
///
/// The exact `BigRational` endpoints stay inside native memory. The echoed
/// source is bounded by `HARD_MAX_SOURCE_BYTES` and lets the frontend reject a
/// delayed response belonging to another request.
#[tauri::command]
pub(super) async fn evaluate_numeric_expression(
    ipc_request: tauri::ipc::Request<'_>,
) -> Result<NumericExpressionResponse, NumericExpressionCommandError> {
    let request = decode_request_wire(ipc_request.body())?;
    validate_request_envelope(&request)?;
    let permit = NUMERIC_EXPRESSION_WORKER_GATE
        .try_acquire()
        .ok_or_else(|| {
            NumericExpressionCommandError::new(NumericExpressionErrorCategory::ResourceLimit)
        })?;
    tauri::async_runtime::spawn_blocking(move || {
        run_guarded_worker(permit, || evaluate_request(request))
    })
    .await
    .map_err(|_| {
        NumericExpressionCommandError::new(NumericExpressionErrorCategory::InternalFailure)
    })?
}

fn decode_request_wire(
    body: &tauri::ipc::InvokeBody,
) -> Result<NumericExpressionRequest, NumericExpressionCommandError> {
    let envelope = match body {
        tauri::ipc::InvokeBody::Json(value) => NumericExpressionCommandEnvelope::deserialize(value),
        tauri::ipc::InvokeBody::Raw(_) => {
            return Err(NumericExpressionCommandError::new(
                NumericExpressionErrorCategory::InvalidRequest,
            ));
        }
    }
    .map_err(|_| {
        NumericExpressionCommandError::new(NumericExpressionErrorCategory::InvalidRequest)
    })?;
    Ok(envelope.request)
}

fn run_guarded_worker<T, F>(
    permit: NumericExpressionWorkerPermit<'_>,
    worker: F,
) -> Result<T, NumericExpressionCommandError>
where
    F: FnOnce() -> Result<T, NumericExpressionCommandError>,
{
    let _permit = permit;
    catch_unwind(AssertUnwindSafe(worker)).unwrap_or_else(|_| {
        Err(NumericExpressionCommandError::new(
            NumericExpressionErrorCategory::InternalFailure,
        ))
    })
}

fn validate_request_envelope(
    request: &NumericExpressionRequest,
) -> Result<(), NumericExpressionCommandError> {
    let precision_bits = usize::from(request.precision_bits);
    if request.source.len() > HARD_MAX_SOURCE_BYTES
        || !(MIN_PRECISION_BITS..=MAX_PRECISION_BITS).contains(&precision_bits)
    {
        return Err(NumericExpressionCommandError::new(
            NumericExpressionErrorCategory::InvalidRequest,
        ));
    }
    Ok(())
}

fn evaluate_request(
    request: NumericExpressionRequest,
) -> Result<NumericExpressionResponse, NumericExpressionCommandError> {
    let limits = ExpressionLimits {
        precision_bits: usize::from(request.precision_bits),
        ..ExpressionLimits::default()
    };
    let expression =
        ScalarExpression::parse(&request.source, limits).map_err(map_expression_error)?;
    let value = expression.evaluate(limits).map_err(map_expression_error)?;
    let interval = value.certified_f64_interval().map_err(|_| {
        NumericExpressionCommandError::new(NumericExpressionErrorCategory::ResultOutOfRange)
    })?;
    let operations = u32::try_from(value.operations()).map_err(|_| {
        NumericExpressionCommandError::new(NumericExpressionErrorCategory::InternalFailure)
    })?;
    if value.operations() > HARD_MAX_OPERATIONS {
        return Err(NumericExpressionCommandError::new(
            NumericExpressionErrorCategory::InternalFailure,
        ));
    }

    let lower_bound = normalize_zero(interval.lower());
    let upper_bound = normalize_zero(interval.upper());
    let lower_display = format!("{lower_bound:.17e}");
    let upper_display = format!("{upper_bound:.17e}");
    if !lower_bound.is_finite()
        || !upper_bound.is_finite()
        || lower_bound > upper_bound
        || lower_display.len() > MAX_DISPLAY_BYTES
        || upper_display.len() > MAX_DISPLAY_BYTES
    {
        return Err(NumericExpressionCommandError::new(
            NumericExpressionErrorCategory::InternalFailure,
        ));
    }

    Ok(NumericExpressionResponse {
        schema: NUMERIC_EXPRESSION_SCHEMA,
        source: request.source,
        requested_precision_bits: request.precision_bits,
        exact: value.is_exact(),
        operations,
        lower_bound,
        upper_bound,
        lower_display,
        upper_display,
    })
}

pub(super) fn evaluate_positive_millimetre_pair(
    width_source: String,
    height_source: String,
) -> Result<(f64, f64), PositiveMillimetrePairError> {
    #[cfg(test)]
    let _test_serial = SYNCHRONOUS_EVALUATION_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let permit = NUMERIC_EXPRESSION_WORKER_GATE
        .try_acquire()
        .ok_or(PositiveMillimetrePairError::WorkerBusy)?;
    run_guarded_worker(permit, || {
        evaluate_positive_millimetre_pair_in_current_worker(width_source, height_source)
    })
    .map_err(PositiveMillimetrePairError::Evaluation)
}

pub(super) fn evaluate_finite_millimetre_pair(
    x_source: String,
    y_source: String,
) -> Result<(f64, f64), PositiveMillimetrePairError> {
    #[cfg(test)]
    let _test_serial = SYNCHRONOUS_EVALUATION_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let permit = NUMERIC_EXPRESSION_WORKER_GATE
        .try_acquire()
        .ok_or(PositiveMillimetrePairError::WorkerBusy)?;
    run_guarded_worker(permit, || {
        Ok((
            evaluate_finite_millimetre_expression(x_source)?,
            evaluate_finite_millimetre_expression(y_source)?,
        ))
    })
    .map_err(PositiveMillimetrePairError::Evaluation)
}

pub(super) async fn evaluate_positive_millimetre_pair_in_worker(
    width_source: String,
    height_source: String,
) -> Result<(f64, f64), PositiveMillimetrePairError> {
    let permit = NUMERIC_EXPRESSION_WORKER_GATE
        .try_acquire()
        .ok_or(PositiveMillimetrePairError::WorkerBusy)?;
    tauri::async_runtime::spawn_blocking(move || {
        run_guarded_worker(permit, || {
            evaluate_positive_millimetre_pair_in_current_worker(width_source, height_source)
        })
    })
    .await
    .map_err(|_| {
        PositiveMillimetrePairError::Evaluation(NumericExpressionCommandError::new(
            NumericExpressionErrorCategory::InternalFailure,
        ))
    })?
    .map_err(PositiveMillimetrePairError::Evaluation)
}

fn evaluate_positive_millimetre_pair_in_current_worker(
    width_source: String,
    height_source: String,
) -> Result<(f64, f64), NumericExpressionCommandError> {
    Ok((
        evaluate_positive_millimetre_expression(width_source)?,
        evaluate_positive_millimetre_expression(height_source)?,
    ))
}

fn evaluate_positive_millimetre_expression(
    source: String,
) -> Result<f64, NumericExpressionCommandError> {
    if source.trim().is_empty() || source.chars().any(char::is_control) {
        return Err(NumericExpressionCommandError::new(
            NumericExpressionErrorCategory::InvalidRequest,
        ));
    }
    let response = evaluate_request(NumericExpressionRequest {
        source,
        precision_bits: USER_INPUT_PRECISION_BITS,
    })?;
    adopt_positive_adjacent_interval(response.lower_bound, response.upper_bound).ok_or_else(|| {
        NumericExpressionCommandError::new(NumericExpressionErrorCategory::ResultOutOfRange)
    })
}

fn evaluate_finite_millimetre_expression(
    source: String,
) -> Result<f64, NumericExpressionCommandError> {
    if source.trim().is_empty() || source.chars().any(char::is_control) {
        return Err(NumericExpressionCommandError::new(
            NumericExpressionErrorCategory::InvalidRequest,
        ));
    }
    let response = evaluate_request(NumericExpressionRequest {
        source,
        precision_bits: USER_INPUT_PRECISION_BITS,
    })?;
    adopt_finite_adjacent_interval(response.lower_bound, response.upper_bound).ok_or_else(|| {
        NumericExpressionCommandError::new(NumericExpressionErrorCategory::ResultOutOfRange)
    })
}

fn adopt_finite_adjacent_interval(lower: f64, upper: f64) -> Option<f64> {
    if !lower.is_finite() || !upper.is_finite() || lower > upper {
        return None;
    }
    if lower == upper {
        return Some(normalize_zero(lower));
    }
    let adjacent = if lower.is_sign_negative() == upper.is_sign_negative() {
        lower.to_bits().abs_diff(upper.to_bits()) == 1
    } else {
        lower == -0.0 && upper == 0.0
    };
    adjacent.then(|| normalize_zero(lower))
}

fn adopt_positive_adjacent_interval(lower: f64, upper: f64) -> Option<f64> {
    if !lower.is_finite() || !upper.is_finite() || lower <= 0.0 || lower > upper {
        return None;
    }
    if lower == upper {
        return Some(lower);
    }
    if upper.to_bits().checked_sub(lower.to_bits()) != Some(1) {
        return None;
    }
    Some(lower)
}

const fn normalize_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

const fn map_expression_error(error: ExpressionError) -> NumericExpressionCommandError {
    let category = match error {
        ExpressionError::InvalidLimits | ExpressionError::PrecisionOutOfRange => {
            NumericExpressionErrorCategory::InvalidRequest
        }
        ExpressionError::ResourceLimit(_) => NumericExpressionErrorCategory::ResourceLimit,
        ExpressionError::InconsistentState => NumericExpressionErrorCategory::InternalFailure,
        ExpressionError::Empty
        | ExpressionError::InvalidToken { .. }
        | ExpressionError::InvalidNumber { .. }
        | ExpressionError::UnexpectedToken { .. }
        | ExpressionError::UnexpectedEnd
        | ExpressionError::DivisionByZero
        | ExpressionError::NegativeSquareRoot => NumericExpressionErrorCategory::InvalidExpression,
    };
    NumericExpressionCommandError::new(category)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering as AtomicOrdering},
        mpsc,
    };

    fn request(source: &str, precision_bits: u16) -> NumericExpressionRequest {
        NumericExpressionRequest {
            source: source.to_owned(),
            precision_bits,
        }
    }

    #[test]
    fn exact_rational_stays_native_and_returns_a_bounded_f64_enclosure() {
        let response = evaluate_request(request("1 / 10", 96)).unwrap();

        assert_eq!(response.schema, NUMERIC_EXPRESSION_SCHEMA);
        assert_eq!(response.source, "1 / 10");
        assert_eq!(response.requested_precision_bits, 96);
        assert!(response.exact);
        assert!(response.operations <= HARD_MAX_OPERATIONS as u32);
        assert!(response.lower_bound <= 0.1);
        assert!(0.1 <= response.upper_bound);
        assert!(response.lower_display.len() <= MAX_DISPLAY_BYTES);
        assert!(response.upper_display.len() <= MAX_DISPLAY_BYTES);
        assert_eq!(
            response.lower_display,
            format!("{:.17e}", response.lower_bound)
        );
        assert_eq!(
            response.upper_display,
            format!("{:.17e}", response.upper_bound)
        );
    }

    #[test]
    fn irrational_result_preserves_requested_precision_and_certified_order() {
        for precision_bits in [MIN_PRECISION_BITS as u16, 192, MAX_PRECISION_BITS as u16] {
            let response = evaluate_request(request("sqrt(2) + pi", precision_bits)).unwrap();
            assert_eq!(response.requested_precision_bits, precision_bits);
            assert!(!response.exact);
            assert!(response.lower_bound < response.upper_bound);
        }
    }

    #[test]
    fn request_and_evaluation_fail_with_fixed_categories() {
        assert_eq!(
            validate_request_envelope(&request("1", MIN_PRECISION_BITS as u16 - 1)),
            Err(NumericExpressionCommandError::new(
                NumericExpressionErrorCategory::InvalidRequest
            ))
        );
        assert_eq!(
            evaluate_request(request("1 / 0", 96)),
            Err(NumericExpressionCommandError::new(
                NumericExpressionErrorCategory::InvalidExpression
            ))
        );
        assert_eq!(
            evaluate_request(request("1e400", 96)),
            Err(NumericExpressionCommandError::new(
                NumericExpressionErrorCategory::ResultOutOfRange
            ))
        );
        let busy = PositiveMillimetrePairError::WorkerBusy;
        let invalid = PositiveMillimetrePairError::Evaluation(NumericExpressionCommandError::new(
            NumericExpressionErrorCategory::InvalidExpression,
        ));
        assert!(busy.is_worker_busy());
        assert!(!invalid.is_worker_busy());
        assert_ne!(busy.user_input_message(), invalid.user_input_message());
    }

    #[test]
    fn positive_millimetre_adoption_accepts_exact_and_one_ulp_enclosures_only() {
        assert_eq!(
            evaluate_positive_millimetre_expression("400".to_owned()).unwrap(),
            400.0
        );
        let irrational =
            evaluate_positive_millimetre_expression("100 * sqrt(2)".to_owned()).unwrap();
        assert!(irrational.is_finite());
        assert!(irrational > 141.0 && irrational < 142.0);

        assert_eq!(adopt_positive_adjacent_interval(1.0, 1.0), Some(1.0));
        for (lower, upper) in [
            (f64::from_bits(1), f64::from_bits(2)),
            (1.0, f64::from_bits(1.0_f64.to_bits() + 1)),
            (f64::from_bits(2.0_f64.to_bits() - 1), 2.0),
            (f64::from_bits(f64::MAX.to_bits() - 1), f64::MAX),
        ] {
            assert_eq!(
                adopt_positive_adjacent_interval(lower, upper),
                Some(lower),
                "adjacent adoption is the positive lower bound at bits {:016x}",
                lower.to_bits()
            );
        }
        assert!(adopt_positive_adjacent_interval(0.0, f64::MIN_POSITIVE).is_none());
        assert!(adopt_positive_adjacent_interval(-1.0, -1.0).is_none());
        assert!(adopt_positive_adjacent_interval(2.0, 1.0).is_none());
        assert!(adopt_positive_adjacent_interval(1.0, f64::INFINITY).is_none());
        assert!(
            adopt_positive_adjacent_interval(1.0, f64::from_bits(1.0_f64.to_bits() + 2),).is_none()
        );
    }

    #[test]
    fn finite_pair_adoption_preserves_signed_coordinates_and_zero() {
        assert_eq!(
            evaluate_finite_millimetre_pair("-sqrt(4)".to_owned(), "1 / 2".to_owned()).unwrap(),
            (-2.0, 0.5)
        );
        assert_eq!(
            evaluate_finite_millimetre_pair("-0".to_owned(), "0".to_owned()).unwrap(),
            (0.0, 0.0)
        );
    }

    #[test]
    fn request_wire_rejects_unknown_fields_and_wrong_scalar_types() {
        for json in [
            r#"{"source":"1","precisionBits":96,"private":"x"}"#,
            r#"{"source":"1","precisionBits":96.5}"#,
            r#"{"source":1,"precisionBits":96}"#,
            r#"{"source":"1"}"#,
        ] {
            assert!(serde_json::from_str::<NumericExpressionRequest>(json).is_err());
        }
        assert_eq!(
            serde_json::from_str::<NumericExpressionRequest>(
                r#"{"source":"1","precisionBits":96}"#
            )
            .unwrap(),
            request("1", 96)
        );
    }

    #[test]
    fn command_wire_closes_raw_missing_null_and_malformed_requests_to_one_category() {
        let invalid = [
            tauri::ipc::InvokeBody::Raw(vec![1, 2, 3]),
            tauri::ipc::InvokeBody::Json(serde_json::Value::Null),
            tauri::ipc::InvokeBody::Json(serde_json::json!({})),
            tauri::ipc::InvokeBody::Json(serde_json::json!({"request": null})),
            tauri::ipc::InvokeBody::Json(serde_json::json!({
                "request": {"source": "1"}
            })),
            tauri::ipc::InvokeBody::Json(serde_json::json!({
                "request": {"source": 1, "precisionBits": 96}
            })),
            tauri::ipc::InvokeBody::Json(serde_json::json!({
                "request": {
                    "source": "1",
                    "precisionBits": 96,
                    "private": "C:\\private\\expression.txt"
                }
            })),
            tauri::ipc::InvokeBody::Json(serde_json::json!({
                "request": {"source": "1", "precisionBits": 96},
                "private": "C:\\private\\expression.txt"
            })),
        ];
        for body in &invalid {
            assert_eq!(
                decode_request_wire(body),
                Err(NumericExpressionCommandError::new(
                    NumericExpressionErrorCategory::InvalidRequest
                ))
            );
        }
        assert_eq!(
            decode_request_wire(&tauri::ipc::InvokeBody::Json(serde_json::json!({
                "request": {"source": "1", "precisionBits": 96}
            }))),
            Ok(request("1", 96))
        );
    }

    #[test]
    fn response_wire_has_only_the_documented_bounded_shape() {
        let response = evaluate_request(request("3 / 2", 64)).unwrap();
        let json = serde_json::to_value(response).unwrap();
        let object = json.as_object().unwrap();
        assert_eq!(
            object.keys().map(String::as_str).collect::<Vec<_>>(),
            vec![
                "exact",
                "lowerBound",
                "lowerDisplay",
                "operations",
                "requestedPrecisionBits",
                "schema",
                "source",
                "upperBound",
                "upperDisplay",
            ]
        );
        assert!(
            object
                .values()
                .all(|value| { !value.is_array() && !value.is_object() })
        );
    }

    #[test]
    fn process_worker_gate_rejects_parallel_work_before_spawn() {
        let gate = NumericExpressionWorkerGate::new();
        let first = gate.try_acquire().expect("first worker permit");
        let mut spawned_second_worker = false;
        let second = gate.try_acquire();
        if second.is_some() {
            spawned_second_worker = true;
        }

        assert!(second.is_none());
        assert!(!spawned_second_worker);
        assert!(gate.is_busy());
        drop(first);
        assert!(!gate.is_busy());
        drop(gate.try_acquire().expect("permit is reusable"));
    }

    #[test]
    fn worker_permit_releases_after_success_error_panic_and_abandonment() {
        let gate = NumericExpressionWorkerGate::new();

        let success = run_guarded_worker(gate.try_acquire().unwrap(), || Ok::<_, _>(7));
        assert_eq!(success, Ok(7));
        assert!(!gate.is_busy());

        let expected_error =
            NumericExpressionCommandError::new(NumericExpressionErrorCategory::InvalidExpression);
        let failure =
            run_guarded_worker(gate.try_acquire().unwrap(), || Err::<(), _>(expected_error));
        assert_eq!(failure, Err(expected_error));
        assert!(!gate.is_busy());

        let panic = run_guarded_worker(gate.try_acquire().unwrap(), || -> Result<(), _> {
            panic!("synthetic worker panic")
        });
        assert_eq!(
            panic,
            Err(NumericExpressionCommandError::new(
                NumericExpressionErrorCategory::InternalFailure
            ))
        );
        assert!(!gate.is_busy());

        let permit = gate.try_acquire().unwrap();
        let queued_but_abandoned = move || drop(permit);
        assert!(gate.is_busy());
        drop(queued_but_abandoned);
        assert!(!gate.is_busy());
    }

    #[test]
    fn active_worker_keeps_the_gate_after_its_waiter_is_abandoned() {
        let gate = NumericExpressionWorkerGate::new();
        let (entered_tx, entered_rx) = mpsc::sync_channel(0);
        let (release_tx, release_rx) = mpsc::sync_channel(0);

        std::thread::scope(|scope| {
            let permit = gate.try_acquire().unwrap();
            let worker = scope.spawn(move || {
                run_guarded_worker(permit, || {
                    entered_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    Ok::<_, NumericExpressionCommandError>(())
                })
            });

            entered_rx.recv().unwrap();
            let abandoned_waiter = gate.try_acquire();
            assert!(abandoned_waiter.is_none());
            assert!(gate.is_busy());

            release_tx.send(()).unwrap();
            assert_eq!(worker.join().unwrap(), Ok(()));
        });
        assert!(!gate.is_busy());
    }

    #[test]
    fn gate_compare_exchange_has_exactly_one_concurrent_winner() {
        const CONTENDERS: usize = 16;
        let gate = NumericExpressionWorkerGate::new();
        let entered = Arc::new(Barrier::new(CONTENDERS));
        let attempted = Arc::new(Barrier::new(CONTENDERS));
        let winners = Arc::new(AtomicUsize::new(0));

        std::thread::scope(|scope| {
            for _ in 0..CONTENDERS {
                let entered = Arc::clone(&entered);
                let attempted = Arc::clone(&attempted);
                let winners = Arc::clone(&winners);
                let gate = &gate;
                scope.spawn(move || {
                    entered.wait();
                    let permit = gate.try_acquire();
                    if permit.is_some() {
                        winners.fetch_add(1, AtomicOrdering::Relaxed);
                    }
                    attempted.wait();
                    drop(permit);
                });
            }
        });

        assert_eq!(winners.load(AtomicOrdering::Relaxed), 1);
        assert!(!gate.is_busy());
    }
}
